//! `item create` (REQ-CREATE-ITEM).

use std::sync::Arc;

use super::{
    EditError, Written, WrittenFile, body, describe, document_text, field_values, heading,
    own_item, read_back, schema, summary,
};
use crate::document::{label_name, lines};
use crate::relink::INVALID_POSITION;
use crate::snapshot::{ParsedFile, Snapshot};
use crate::validation::{self, is_id};
use crate::work::WorkCounts;
use crate::write::{self, ReadBack, label};

/// A request to create an item.
#[derive(Clone, Copy, Debug)]
pub struct Create<'r> {
    /// A type name, matched as a label.
    pub type_name: &'r str,
    /// The document, relative to the project root. One that the snapshot lacks is created; the
    /// host has checked that it would be an input (DES-INPUTS-AND-KEYS).
    pub path: &'r str,
    pub title: &'r str,
    /// The ID, or `None` to take the one proposed from the type's prefix and the title.
    pub id: Option<&'r str>,
    /// `<key>=<value>` pairs, `Status` among them.
    pub fields: &'r [(String, String)],
    pub body: &'r str,
    /// The line where the item goes, 1-based; `None` for the end of the document.
    pub line: Option<usize>,
    /// The new MID, which the host generates (DES-ENGINE-BOUNDARY) and no item has.
    pub mid: &'r str,
}

// @mara implements REQ-CREATE-ITEM
/// Creates the item of `request` in its document (REQ-CREATE-ITEM): checks the request against
/// the schema, writes the item with the item writer and reads the document back.
pub fn create(
    snapshot: &Snapshot,
    request: &Create<'_>,
    work: &mut WorkCounts,
) -> Result<Written, EditError> {
    let schema = schema(snapshot)?;
    let kind = label_name(request.type_name.trim());
    let Some(declared) = schema.types.get(&kind) else {
        let names: Vec<String> = schema
            .types
            .keys()
            .map(|name| format!("`{name}`"))
            .collect();
        let count = names.len();
        return Err(EditError::new(
            "unknown-declaration",
            format!(
                "the schema declares no type `{}`: {}",
                request.type_name,
                validation::list(names, count)
            ),
        ));
    };
    write::heading(1, request.title)
        .map_err(|why| EditError::invalid(format!("the title {why}")))?;
    let id = match request.id {
        Some(id) => id.to_owned(),
        None => propose(declared.id_prefix.as_deref(), request.title),
    };
    check_id(
        snapshot,
        &kind,
        declared.id_prefix.as_deref(),
        &id,
        request.id.is_none(),
    )?;
    if !snapshot.items_with_mid(request.mid).is_empty() {
        return Err(EditError::invalid(format!(
            "MID {} is already the MID of {}",
            request.mid,
            describe(snapshot, snapshot.items_with_mid(request.mid)[0])
        )));
    }
    let fields = field_values(schema, &kind, declared, request.fields)?;
    let given = |name: &str| fields.iter().any(|(given, _)| given == name);
    let mut missing: Vec<String> = Vec::new();
    if !given("status") {
        missing.push("`Status`".to_owned());
    }
    for (name, field) in &declared.fields {
        if field.required && !given(name) {
            missing.push(format!("`{}`", label(name)));
        }
    }
    if !missing.is_empty() {
        let count = missing.len();
        return Err(EditError::invalid(format!(
            "an item of type `{kind}` needs {}: give each with `--field <key>=<value>`",
            validation::list(missing, count)
        )));
    }
    let body = body(request.body)?;

    let path = request.path;
    let index = snapshot.document(path);
    let old = match index {
        Some(index) => Arc::clone(&snapshot.documents()[index].file),
        None => Arc::new(ParsedFile::default()),
    };
    let text = match index {
        Some(index) => document_text(snapshot, index)?,
        None => "",
    };
    let at = position(text, &old, request.line, path)?;
    let level = level(&old, at);
    let mut rows = vec![write::row("type", &label(&kind)), write::row("id", &id)];
    let mut expected = vec![
        ("type".to_owned(), label(&kind)),
        ("id".to_owned(), id.clone()),
    ];
    for (name, values) in &fields {
        for value in values {
            rows.push(write::row(name, value));
            expected.push((name.clone(), value.replace("\\|", "|")));
        }
    }
    let eol = write::line_ending(text);
    let item = write::item(request.mid, level, request.title, &rows, body, eol)
        .map_err(|why| EditError::invalid(format!("the title {why}")))?;
    let splice = write::insert(text, at, &item);
    let write::Expect::Item { at: offset, len } = splice.expect else {
        unreachable!("an insertion expects an item");
    };
    let start = splice.range.start + offset;
    let new = read_back(
        &mut ReadBack::knowing(snapshot),
        path,
        &old,
        &[splice],
        work,
    )?;
    let written = own_item(path, &new, start, start..start + len, 0)?;
    let new_item = &new.document.items[written];
    let read: Vec<(String, String)> = new_item
        .rows
        .iter()
        .map(|row| (row.name.clone(), row.value.clone()))
        .collect();
    if read != expected {
        return Err(EditError::new(
            write::UNREADABLE_EDIT,
            format!("{path}: the item's rows would not read back as written; nothing is written"),
        ));
    }
    Ok(Written {
        item: summary(path, new_item),
        files: vec![WrittenFile {
            path: path.to_owned(),
            contents: new.contents,
            document: new.document,
            created: index.is_none(),
        }],
        relation: None,
    })
}

/// The ID proposed for an item of the type with ID prefix `prefix` and title `title`: the
/// prefix, then the title's ASCII letters and digits in uppercase, each run of other characters
/// written as one `-` (DES-ITEM-IDS).
pub(crate) fn propose(prefix: Option<&str>, title: &str) -> String {
    let mut id = prefix.unwrap_or_default().to_owned();
    let mut slug = String::new();
    let mut gap = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !slug.is_empty() {
                slug.push('-');
            }
            gap = false;
            slug.push(c.to_ascii_uppercase());
        } else {
            gap = true;
        }
    }
    id.push_str(&slug);
    id
}

/// Refuses an ID that has not the form of one, lacks the type's prefix, or that another item
/// has (DES-ITEM-IDS).
fn check_id(
    snapshot: &Snapshot,
    kind: &str,
    prefix: Option<&str>,
    id: &str,
    proposed: bool,
) -> Result<(), EditError> {
    let advice = match proposed {
        true => format!(" The ID `{id}` is proposed from the title: give one with `--id`."),
        false => String::new(),
    };
    if !is_id(id) {
        return Err(EditError::invalid(format!(
            "`{id}` is not an ID: uppercase letters and digits in two or more parts joined by \
             `-`, the first starting with a letter, such as `REQ-SIGN-IN`.{advice}"
        )));
    }
    if let Some(prefix) = prefix
        && !id.starts_with(prefix)
    {
        return Err(EditError::invalid(format!(
            "`{id}` does not start with `{prefix}`, the ID prefix of type `{kind}`"
        )));
    }
    if let Some(&other) = snapshot.items_with_id(id).first() {
        return Err(EditError::invalid(format!(
            "ID `{id}` is already the ID of {}: every item has its own ID.{advice}",
            describe(snapshot, other)
        )));
    }
    Ok(())
}

/// The byte where an item at `line` of `text` goes: the start of that line, or the end of the
/// document for `None` or the line after the last. Refuses a line inside an item or further on.
fn position(
    text: &str,
    old: &ParsedFile,
    line: Option<usize>,
    path: &str,
) -> Result<usize, EditError> {
    let Some(line) = line else {
        return Ok(text.len());
    };
    let mut start = 0;
    let mut count = 0;
    for content in lines(text) {
        count += 1;
        if count == line {
            break;
        }
        start += content.len();
    }
    if line > count + 1 || (line == count + 1 && start != text.len()) {
        let last = count + 1;
        return Err(EditError::new(
            INVALID_POSITION,
            format!(
                "{path} has {count} lines, so a new item goes at line 1 to {last}, the end; line \
                 {line} is past it"
            ),
        ));
    }
    if line == count + 1 {
        start = text.len();
    }
    if let Some(item) = old
        .document
        .items
        .iter()
        .find(|item| (item.line..=item.end_line).contains(&line))
    {
        let name = item.value("id").unwrap_or(&item.mid);
        return Err(EditError::new(
            INVALID_POSITION,
            format!(
                "line {line} of {path} is inside {name}, at lines {} to {}: a new item goes \
                 before or after every item",
                item.line, item.end_line
            ),
        ));
    }
    Ok(start)
}

/// The heading level of a new item at byte `at` of `old`: that of the item before it, else of
/// the item after it, else 2, under a document title of level 1.
fn level(old: &ParsedFile, at: usize) -> usize {
    let items = &old.document.items;
    let after = items.partition_point(|item| item.span.start < at);
    let neighbours = items[..after].iter().rev().chain(&items[after..]);
    neighbours
        .filter_map(|item| heading(old, item))
        .map(|heading| heading.level)
        .next()
        .unwrap_or(2)
}
