//! The single-file authoring operations over a snapshot: `item create`, `item update`,
//! `item delete`, `relation add` and `relation remove` (`docs/authoring/index.mara.md`,
//! DES-AUTHORING-REQUESTS in `docs/authoring/index.mara.md`).
//!
//! Each computes its edit with the item writer ([`crate::write`]), reads every document it
//! writes back through one [`ReadBack`] (ADR-READ-BACK-EVERY-EDIT), and returns the new contents
//! for the host to put on disk (DES-WRITE-PROCEDURE). It reads and writes nothing itself.

mod create;
mod delete;
mod relations;
#[cfg(test)]
mod tests;
mod update;

use std::collections::HashMap;
use std::fmt;
use std::ops::Range;

use crate::diagnostic::Diagnostic;
use crate::document::{Item, label_name};
use crate::json::Object;
use crate::read::{self, ItemSummary, Named, ReadError};
use crate::schema::{FieldKind, Schema, TypeDecl};
use crate::snapshot::{ParsedFile, Snapshot};
use crate::validation;
use crate::work::WorkCounts;
use crate::write::{self, Misread, ReadBack, Splice, label};

pub use create::{Create, create};
pub use delete::delete;
pub use relations::{relation_add, relation_remove};
pub use update::{Update, update};

/// The version of the JSON form of every authoring result.
pub const FORMAT_VERSION: u64 = 1;

/// The request would write an item or row that the schema or the identity rules do not allow.
pub const INVALID_ITEM: &str = "invalid-item";
/// A relation named by its inverse name, where only the forward name is taken.
pub const INVERSE_RELATION: &str = "inverse-relation";
/// An item that a delete would remove while other text targets it.
pub const TARGETED_ITEM: &str = "targeted-item";
/// A relation that `relation remove` names and the source item does not have.
pub const MISSING_RELATION: &str = "missing-relation";

/// Why an authoring operation writes nothing: a failure of DES-OPERATIONS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditError {
    pub code: &'static str,
    pub message: String,
    /// The places in project files that caused it.
    pub diagnostics: Vec<Diagnostic>,
}

impl EditError {
    fn new(code: &'static str, message: String) -> EditError {
        EditError {
            code,
            message,
            diagnostics: Vec::new(),
        }
    }

    fn invalid(message: String) -> EditError {
        EditError::new(INVALID_ITEM, message)
    }

    fn misread(path: &str, misread: &Misread) -> EditError {
        EditError {
            code: write::UNREADABLE_EDIT,
            message: format!("{path}: {}; nothing is written", misread.message),
            diagnostics: vec![misread.diagnostic(path)],
        }
    }
}

impl From<ReadError> for EditError {
    fn from(error: ReadError) -> EditError {
        let diagnostics = match &error {
            ReadError::TooLarge(diagnostic) => vec![(**diagnostic).clone()],
            _ => Vec::new(),
        };
        EditError {
            code: error.code(),
            message: error.to_string(),
            diagnostics,
        }
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// A document of an edit with its new contents, which the read-back accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WrittenFile {
    /// Relative to the project root.
    pub path: String,
    pub contents: Box<[u8]>,
    /// No document was there: the edit creates it.
    pub created: bool,
}

/// What an authoring operation writes, and its result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    /// In write order.
    pub files: Vec<WrittenFile>,
    /// The item created, updated or deleted, or the source of the relation: as written, or as
    /// it was for a delete.
    pub item: ItemSummary,
    /// For `relation add` and `relation remove`: the relation's forward name, the target, and
    /// the line of the row, as written or as it was.
    pub relation: Option<RelationRow>,
}

/// The row that `relation add` writes or `relation remove` deletes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationRow {
    pub name: String,
    pub target: ItemSummary,
    pub line: usize,
}

impl Written {
    /// The JSON form of the result: `format_version` 1, `item`, `relation` and `target` for a
    /// relation, and `files`, the paths written in order.
    pub fn json(&self) -> String {
        let mut out = String::new();
        let mut object = Object::new(&mut out);
        object.number("format_version", FORMAT_VERSION);
        object.key("item").push_str(&self.item.json());
        if let Some(relation) = &self.relation {
            object.string("relation", &relation.name);
            object.key("target").push_str(&relation.target.json());
            object.number("line", relation.line as u64);
        }
        let paths: Vec<String> = self
            .files
            .iter()
            .map(|file| {
                let mut path = String::new();
                crate::json::string(&mut path, &file.path);
                path
            })
            .collect();
        object.array("files", paths.iter().map(String::as_str));
        object.end();
        out
    }
}

/// The schema of `snapshot`. A host refuses a project without one before it computes an edit
/// (DES-PROJECT-CONFIGURATION).
fn schema(snapshot: &Snapshot) -> Result<&Schema, EditError> {
    snapshot.schema().map(|schema| &**schema).ok_or_else(|| {
        EditError::new(
            "invalid-project",
            "the project has no schema that Mara could read".to_owned(),
        )
    })
}

/// The item that `address` names: an ID, a MID or the `<path>:<line>` of an item.
fn resolve_item(snapshot: &Snapshot, address: &str) -> Result<usize, EditError> {
    match read::resolve(snapshot, address)? {
        Named::Node(crate::document::Node::Item(item)) => Ok(item),
        _ => Err(EditError::new(
            "unknown-node",
            format!("{address} names no item: name an item by its ID, its MID or `<path>:<line>`"),
        )),
    }
}

/// The schema name of the type of `item`, from its first `Type` row.
fn kind(item: &Item) -> String {
    item.value("type").map(label_name).unwrap_or_default()
}

/// How a message names the item at `index`: its ID, else its MID, and where it is.
fn describe(snapshot: &Snapshot, index: usize) -> String {
    let summary = ItemSummary::of(snapshot, index);
    let name = summary.id.as_deref().unwrap_or(&summary.mid);
    format!("{name} at {}:{}", summary.path, summary.line)
}

/// The values of a request's `--field` options for an item of type `kind`, declared as
/// `declared`: grouped by schema name in the order first given, each as its cell is written
/// (labels for `Status` and enum values). Refuses a key that is no field of the type, a value
/// outside the allowed set, several values of a key that takes one, and a value given twice
/// (DES-SCHEMA-CHECKS).
fn field_values(
    schema: &Schema,
    kind: &str,
    declared: &TypeDecl,
    fields: &[(String, String)],
) -> Result<Vec<(String, Vec<String>)>, EditError> {
    let mut grouped: Vec<(String, Vec<String>)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (key, value) in fields {
        let name = label_name(key);
        let (cell, repeatable) = match name.as_str() {
            "type" => {
                return Err(EditError::invalid(
                    "`Type` is no field: an item's type is given when it is created and does not \
                     change"
                        .to_owned(),
                ));
            }
            "id" => {
                return Err(EditError::invalid(
                    "`ID` is no field: `item create` takes it as `--id`, and `item rename` changes \
                     it"
                    .to_owned(),
                ));
            }
            "status" => (
                allowed(value, &schema.statuses, "a status of the schema")?,
                false,
            ),
            name => match declared.fields.get(name) {
                Some(field) => {
                    let cell = match &field.kind {
                        FieldKind::Enum(values) => {
                            allowed(value, values, &format!("a value of `{}`", label(name)))?
                        }
                        FieldKind::Text => text_cell(value)?,
                    };
                    (cell, field.repeatable)
                }
                None if is_relation(schema, name) => {
                    return Err(EditError::invalid(format!(
                        "`{}` is a relation, which `item create` and `item update` do not write: \
                         add it with `mara relation add`",
                        label(name)
                    )));
                }
                None => {
                    return Err(EditError::invalid(format!(
                        "type `{kind}` declares no field `{}`{}",
                        key.trim(),
                        fields_of(declared)
                    )));
                }
            },
        };
        let at = *index.entry(name.clone()).or_insert_with(|| {
            grouped.push((name.clone(), Vec::new()));
            grouped.len() - 1
        });
        let values = &mut grouped[at].1;
        if values.contains(&cell) {
            return Err(EditError::invalid(format!(
                "`{}` is given the value `{cell}` twice, and a row that repeats another is an error",
                label(&name)
            )));
        }
        values.push(cell);
        if !repeatable && values.len() > 1 {
            return Err(EditError::invalid(format!(
                "`{}` takes one value, and the request gives {}",
                label(&name),
                values.len()
            )));
        }
    }
    Ok(grouped)
}

/// The label of the name in `names` that `value` matches as a label, or a refusal that lists
/// them.
fn allowed(value: &str, names: &[String], what: &str) -> Result<String, EditError> {
    let name = label_name(value.trim());
    if names.contains(&name) {
        return Ok(label(&name));
    }
    let labels: Vec<String> = names
        .iter()
        .map(|name| format!("`{}`", label(name)))
        .collect();
    let count = labels.len();
    Err(EditError::invalid(format!(
        "`{value}` is not {what}: {}",
        validation::list(labels, count)
    )))
}

/// `value` as a table cell, or a refusal.
fn text_cell(value: &str) -> Result<String, EditError> {
    write::cell(value).map_err(|why| EditError::invalid(format!("the value `{value}` {why}")))
}

/// Whether `name` is a relation's forward or inverse name.
fn is_relation(schema: &Schema, name: &str) -> bool {
    schema.relations.contains_key(name)
        || schema
            .relations
            .values()
            .any(|relation| relation.inverse.as_deref() == Some(name))
}

/// `: it declares <labels>` for a message, or nothing when the type declares no field.
fn fields_of(declared: &TypeDecl) -> String {
    let labels: Vec<String> = declared
        .fields
        .keys()
        .map(|name| format!("`{}`", label(name)))
        .collect();
    match labels.len() {
        0 => ": it declares no field".to_owned(),
        count => format!(": it declares {}", validation::list(labels, count)),
    }
}

/// The body of a request, refused when it holds no text (`empty-body`, DES-ITEM-STRUCTURE).
fn body(body: &str) -> Result<&str, EditError> {
    match body.trim().is_empty() {
        true => Err(EditError::invalid(
            "the body is empty, and every item has a body".to_owned(),
        )),
        false => Ok(body),
    }
}

/// `body` with the line endings of `eol` and no line ending after its last line, as the writer
/// writes it ([`write::item`]).
fn body_text(body: &str, eol: &str) -> String {
    let lines: Vec<&str> = crate::document::lines(body.trim_end_matches(['\n', '\r']))
        .map(|line| line.trim_end_matches(['\n', '\r']))
        .collect();
    lines.join(eol)
}

/// The heading of an item: its level, whether it is an ATX heading, and the bytes of its text.
struct Heading {
    level: usize,
    atx: bool,
    text: Range<usize>,
}

/// The heading of `item` in `file`: the top-level block right before its metadata table
/// (DES-ITEM-STRUCTURE), an ATX or a setext heading.
fn heading(file: &ParsedFile, item: &Item) -> Option<Heading> {
    let blocks = &file.document.blocks;
    let index = blocks.partition_point(|block| block.end <= item.table.start);
    let block = blocks.get(index.checked_sub(1)?)?;
    if block.start < item.span.start {
        return None;
    }
    let source = std::str::from_utf8(&file.contents[block.clone()]).ok()?;
    let indent = source.len() - source.trim_start_matches(' ').len();
    let line = source.trim_end_matches(['\n', '\r']);
    let marks = line[indent..].len() - line[indent..].trim_start_matches('#').len();
    let opens = |rest: &str| rest.is_empty() || rest.starts_with([' ', '\t']);
    if (1..=6).contains(&marks) && opens(&line[indent + marks..]) {
        let after = indent + marks;
        let rest = &line[after..];
        let start = after + rest.len() - rest.trim_start_matches([' ', '\t']).len();
        let mut end = line.trim_end_matches([' ', '\t']).len().max(start);
        // A closing run of `#` after a space or a tab, or one that is all the text.
        let closing = line[start..end].len() - line[start..end].trim_end_matches('#').len();
        if closing > 0 {
            let before = &line[start..end - closing];
            if before.is_empty() || before.ends_with([' ', '\t']) {
                end = start + before.trim_end_matches([' ', '\t']).len();
            }
        }
        return Some(Heading {
            level: marks,
            atx: true,
            text: block.start + start..block.start + end,
        });
    }
    // A setext heading: its text lines, then the underline.
    let underline_start = line.rfind(['\n', '\r']).map(|at| at + 1)?;
    let level = match line[underline_start..].trim().starts_with('=') {
        true => 1,
        false => 2,
    };
    let text = line[..underline_start].trim_end();
    Some(Heading {
        level,
        atx: false,
        text: block.start + indent..block.start + text.len(),
    })
}

/// The document at `index` of `snapshot`, as text.
fn document_text(snapshot: &Snapshot, index: usize) -> Result<&str, EditError> {
    let document = &snapshot.documents()[index];
    std::str::from_utf8(&document.file.contents).map_err(|_| {
        EditError::new(
            "invalid-utf8",
            format!(
                "{} is not UTF-8 text, so Mara writes nothing into it",
                document.path
            ),
        )
    })
}

/// Applies `splices` to the document at `path`, whose old parse is `old`, and reads the result
/// back, counting its parse in `work`.
fn read_back(
    read_back: &mut ReadBack,
    path: &str,
    old: &ParsedFile,
    splices: &[Splice],
    work: &mut WorkCounts,
) -> Result<ParsedFile, EditError> {
    read_back
        .edit(old, splices, work)
        .map_err(|misread| EditError::misread(path, &misread))
}

/// Checks that the text written at `written` in `new` reads as part of the item that starts at
/// `start`: no other item or narrative unit starts there, as a thematic break or an anchor in a
/// body would make one (DES-REGIONS), and the item reads no parse problem that `old_problems`
/// does not count. Returns the item's index in `new`.
fn own_item(
    path: &str,
    new: &ParsedFile,
    start: usize,
    written: Range<usize>,
    old_problems: usize,
) -> Result<usize, EditError> {
    let document = &new.document;
    let items = &document.items;
    let index = items.partition_point(|item| item.span.start < start);
    let refuse = |why: String| {
        Err(EditError::new(
            write::UNREADABLE_EDIT,
            format!("{path}: {why}; nothing is written"),
        ))
    };
    let Some(item) = items.get(index).filter(|item| item.span.start == start) else {
        return refuse("the item would not read back where it is written".to_owned());
    };
    let starts_inside = |at: usize| written.start < at && at < written.end;
    if items.iter().any(|other| starts_inside(other.span.start))
        || document
            .units
            .iter()
            .any(|unit| starts_inside(unit.span.start))
    {
        return refuse(
            "the body would not read back as the item's body: a thematic break (`---`, `***` or \
             `___`) or a MID anchor in it would end the item, and the rest would read as \
             narrative or another item"
                .to_owned(),
        );
    }
    let problems: Vec<Diagnostic> = document
        .diagnostics(path)
        .filter(|diagnostic| (item.line..=item.end_line).contains(&diagnostic.line))
        .collect();
    if problems.len() > old_problems {
        let listed: Vec<String> = problems
            .iter()
            .map(|diagnostic| format!("{} ({})", diagnostic.message, diagnostic.code))
            .collect();
        let count = listed.len();
        return refuse(format!(
            "the item would read with problems: {}",
            validation::list(listed, count)
        ));
    }
    Ok(index)
}

/// The parse problems of `file` at the lines of `item`.
fn problems_of(path: &str, file: &ParsedFile, item: &Item) -> usize {
    file.document
        .diagnostics(path)
        .filter(|diagnostic| (item.line..=item.end_line).contains(&diagnostic.line))
        .count()
}

/// What a result says of the item `item` of the document at `path`.
fn summary(path: &str, item: &Item) -> ItemSummary {
    ItemSummary {
        path: path.to_owned(),
        line: item.line,
        id: item.value("id").map(str::to_owned),
        mid: item.mid.clone(),
        type_name: kind(item),
        title: item.title.clone(),
    }
}
