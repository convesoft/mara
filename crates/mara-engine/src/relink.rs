//! Moving an item and renaming its ID, with the links that follow (REQ-MOVE-ITEM, REQ-RENAME-ID
//! and DES-MOVE-AND-RENAME in `docs/authoring/index.mara.md`). Each computes the splices of every
//! document it writes, in the order of DES-WRITE-PROCEDURE in `docs/authoring/writes.mara.md`. The host
//! reads them back ([`crate::write::ReadBack`]) and writes them.

mod links;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::ops::Range;

use crate::diagnostic::Diagnostic;
use crate::document::{Document, Reference, ReferenceKind, has_scheme, is_mid, label_name, lines};
use crate::snapshot::{Edge, EdgeSource, Snapshot};
use crate::validation::{is_id, list, resolve};
use crate::write::{self, Expect, Splice, UNREADABLE_EDIT, cell, relative, set_value};

/// The failure code of a line, given for an item, that lies inside an item or past the end of
/// the document (DES-OPERATIONS).
pub const INVALID_POSITION: &str = "invalid-position";

/// The splices of one document of an edit, in byte order and apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentEdit {
    /// Relative to the project root.
    pub path: String,
    pub splices: Vec<Splice>,
}

/// Why a move or rename is refused before anything is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelinkError {
    /// A failure code of DES-OPERATIONS.
    pub code: &'static str,
    pub message: String,
    /// The problems in project files that caused it.
    pub diagnostics: Vec<Diagnostic>,
}

impl RelinkError {
    fn new(code: &'static str, message: String) -> RelinkError {
        RelinkError {
            code,
            message,
            diagnostics: Vec::new(),
        }
    }
}

/// A move of an item: the edits in write order, destination first, then the source, then the
/// documents whose links to the item change (DES-WRITE-PROCEDURE).
#[derive(Debug)]
pub struct Move {
    pub edits: Vec<DocumentEdit>,
    /// The snapshot index of the item that moves.
    pub item: usize,
}

// @mara implements REQ-MOVE-ITEM
// @mara implements REQ-STABLE-IDENTITY
// @mara implements REQ-INTERRUPTED-EDIT-LOSES-NOTHING
/// Moves the item that `reference`, an ID or a MID, names into the document at `destination`:
/// before line `line` of it, which lies outside every item, or at its end. Its bytes stay, except
/// that its own links name the same files from the destination; every link to it elsewhere
/// names the destination. A document the snapshot lacks is created, so the host checks first
/// that it would be an input. When the destination already holds the item as this move would
/// write it, left there by an interrupted move, the move only removes the other copy and
/// rewrites the links. A move to where the item already is writes no item.
pub fn move_item(
    snapshot: &Snapshot,
    reference: &str,
    destination: &str,
    line: Option<usize>,
) -> Result<Move, RelinkError> {
    let mid = named_mid(snapshot, reference)?;
    let target = snapshot.document(destination);
    let copies = snapshot.items_with_mid(&mid);
    let in_destination = |item: usize| Some(snapshot.items()[item].document) == target;
    let (item, present) = match *copies {
        [item] => (item, None),
        [a, b] if in_destination(a) != in_destination(b) => match in_destination(a) {
            true => (b, Some(a)),
            false => (a, Some(b)),
        },
        _ => return Err(ambiguous(snapshot, copies, "MID", &mid)),
    };
    let moved = moved_text(snapshot, item, destination)?;
    if let Some(copy) = present
        && item_text(snapshot, copy)? != moved
    {
        let mut error = ambiguous(snapshot, copies, "MID", &mid);
        error.message.push_str(
            "; the copy in the destination differs from what this move would write there, so \
             it is not one that an interrupted move left",
        );
        return Err(error);
    }
    let source = snapshot.items()[item].document;
    let source_text = text(snapshot, source)?;
    let span = snapshot.item(item).span.clone();
    let empty = Document::default();
    let (destination_text, destination_document) = match target {
        Some(index) => (
            text(snapshot, index)?,
            &snapshot.documents()[index].file.document,
        ),
        None => ("", &empty),
    };
    let removal = write::remove(source_text, span.clone());
    let mut insertion = None;
    let mut removed = true;
    if present.is_none() {
        let at = position(destination_text, destination_document, destination, line)?;
        if target == Some(source) && beside(source_text, &removal.range, at) {
            removed = false;
        } else {
            insertion = Some(write::insert(destination_text, at, &moved));
        }
    }

    // Every link to the item outside the copy that goes names the destination.
    let mut retargeted: BTreeMap<usize, Vec<(&Edge, String)>> = BTreeMap::new();
    for edge in snapshot.incoming(&mid) {
        let link = snapshot.link(edge);
        let inside = span.start <= link.span.start && link.span.end <= span.end;
        if (removed && edge.document == source && inside) || has_scheme(&link.path) {
            continue;
        }
        let from = snapshot.documents()[edge.document].path.as_str();
        if resolve(from, &link.path).as_deref() != Some(destination) {
            let path = relative(from, destination);
            retargeted
                .entry(edge.document)
                .or_default()
                .push((edge, path));
        }
    }
    let removal = removed.then_some(removal);
    let removal_range = removal.as_ref().map(|splice| splice.range.clone());
    let mut edits = Vec::new();
    let mut first: Vec<Splice> = insertion.into_iter().collect();
    if let Some(index) = target {
        let inside = removal_range.as_ref().filter(|_| index == source);
        if let Some(links) = retargeted.remove(&index) {
            first.extend(retarget(snapshot, index, &links, inside)?);
        }
        if index == source {
            first.extend(removal.clone());
        }
    }
    edits.push(finish(destination, first)?);
    if target != Some(source) {
        let mut second: Vec<Splice> = removal.into_iter().collect();
        if let Some(links) = retargeted.remove(&source) {
            second.extend(retarget(snapshot, source, &links, removal_range.as_ref())?);
        }
        edits.push(finish(&snapshot.documents()[source].path, second)?);
    }
    for (document, links) in retargeted {
        let path = &snapshot.documents()[document].path;
        edits.push(finish(path, retarget(snapshot, document, &links, None)?)?);
    }
    edits.retain(|edit| !edit.splices.is_empty());
    Ok(Move { edits, item })
}

/// A rename of an item's ID: the edits in write order, first the documents with mention text to
/// rewrite, then the item's document with its `ID` row, then the documents with only relation
/// link text to rewrite (DES-WRITE-PROCEDURE, ADR-MENTIONS-BEFORE-ID-ROW).
#[derive(Debug)]
pub struct Rename {
    pub edits: Vec<DocumentEdit>,
    /// How many of `edits` rewrite mention text before the `ID` row: code markers are written
    /// with them, in path order.
    pub before_id_row: usize,
    /// The snapshot index of the item.
    pub item: usize,
    pub old_id: String,
}

// @mara implements REQ-RENAME-ID
// @mara implements REQ-STABLE-IDENTITY
/// Renames the ID of the item that `reference`, an ID or a MID, names to `new_id`: its `ID` rows
/// that hold the old ID, the text of every mention of it whose text is the old ID, and the text
/// of every relation link to it. Refused when `new_id` lacks the form of an ID or the prefix of
/// the item's type, or another item has it.
pub fn rename(snapshot: &Snapshot, reference: &str, new_id: &str) -> Result<Rename, RelinkError> {
    let mid = named_mid(snapshot, reference)?;
    let item = match *snapshot.items_with_mid(&mid) {
        [item] => item,
        ref copies => return Err(ambiguous(snapshot, copies, "MID", &mid)),
    };
    let parsed = snapshot.item(item);
    let Some(old_id) = parsed.value("id") else {
        let message = format!(
            "the item with MID {mid} has no `ID` row, so it has no ID to rename: add the row by \
             hand"
        );
        return Err(RelinkError::new("missing-row", message));
    };
    if !is_id(new_id) {
        let message = format!(
            "`{new_id}` is not an ID: uppercase letters and digits in two or more parts joined by \
             `-`, the first starting with a letter, such as `REQ-SIGN-IN`"
        );
        return Err(RelinkError::new("invalid-argument", message));
    }
    let kind = label_name(parsed.value("type").unwrap_or_default());
    let prefix = snapshot
        .schema()
        .and_then(|schema| schema.types.get(&kind))
        .and_then(|declaration| declaration.id_prefix.as_deref());
    if let Some(prefix) = prefix
        && !new_id.starts_with(prefix)
    {
        let message =
            format!("`{new_id}` does not start with `{prefix}`, the ID prefix of type `{kind}`");
        return Err(RelinkError::new("wrong-id-prefix", message));
    }
    let others: Vec<usize> = (snapshot.items_with_id(new_id).iter())
        .copied()
        .filter(|&other| other != item)
        .collect();
    if !others.is_empty() {
        let places = others.iter().map(|&other| place(snapshot, other));
        let message = format!(
            "`{new_id}` is already the ID of the item at {}: every item has its own ID",
            list(places, others.len())
        );
        return Err(RelinkError::new("duplicate-id", message));
    }

    // By document: whether it holds mention text to rewrite, and its splices.
    let mut documents: BTreeMap<usize, (bool, Vec<Splice>)> = BTreeMap::new();
    for edge in snapshot.incoming(&mid) {
        let link = snapshot.link(edge);
        let relation = snapshot.relation(edge).is_some();
        let rewritten = match relation {
            true => link.text != new_id,
            false => link.text == old_id && old_id != new_id,
        };
        if !rewritten {
            continue;
        }
        let contents = text(snapshot, edge.document)?;
        let path = &snapshot.documents()[edge.document].path;
        let (range, label) = links::relabel(contents, &link.span, &link.text, new_id)
            .ok_or_else(|| unlocatable(path, link.line, "its text"))?;
        let splice = match (edge.source, edge.item) {
            (EdgeSource::Row(row), Some(source)) => {
                let row = &snapshot.item(source).rows[row];
                let value = in_value(contents, &row.value_span, &range, &label)
                    .ok_or_else(|| unlocatable(path, link.line, "its text"))?;
                set_value(row, &value)
            }
            _ => Splice {
                range,
                text: label,
                expect: Expect::Bytes,
            },
        };
        let entry = documents.entry(edge.document).or_default();
        entry.0 |= !relation;
        entry.1.push(splice);
    }
    let home = snapshot.items()[item].document;
    if old_id != new_id {
        let written = cell(new_id).expect("an ID is a cell");
        let rows = parsed.rows.iter();
        let id_rows = rows.filter(|row| row.name == "id" && row.value == old_id);
        let entry = documents.entry(home).or_default();
        entry.1.extend(id_rows.map(|row| set_value(row, &written)));
    }
    let mut first = Vec::new();
    let mut last = Vec::new();
    let mut own = None;
    for (document, (mentions, splices)) in documents {
        let edit = finish(&snapshot.documents()[document].path, splices)?;
        match document {
            _ if document == home => own = Some(edit),
            _ if mentions => first.push(edit),
            _ => last.push(edit),
        }
    }
    let before_id_row = first.len();
    let edits = first.into_iter().chain(own).chain(last).collect();
    Ok(Rename {
        edits,
        before_id_row,
        item,
        old_id: old_id.to_owned(),
    })
}

/// The MID of the item or items that `reference`, an ID or a MID, names.
fn named_mid(snapshot: &Snapshot, reference: &str) -> Result<String, RelinkError> {
    let (kind, items) = match is_mid(reference) {
        true => ("MID", snapshot.items_with_mid(reference)),
        false => ("ID", snapshot.items_with_id(reference)),
    };
    let Some(&first) = items.first() else {
        let message = format!("no item has the {kind} `{reference}`");
        return Err(RelinkError::new("unknown-item", message));
    };
    let mid = &snapshot.item(first).mid;
    if items.iter().any(|&item| &snapshot.item(item).mid != mid) {
        return Err(ambiguous(snapshot, items, kind, reference));
    }
    Ok(mid.clone())
}

fn ambiguous(snapshot: &Snapshot, items: &[usize], kind: &str, value: &str) -> RelinkError {
    let places = items.iter().map(|&item| place(snapshot, item));
    let message = format!(
        "{} items have the {kind} `{value}`, at {}, so Mara cannot tell which item, or which \
         links, the edit concerns; `mara project validate` reports the repeat",
        items.len(),
        list(places, items.len())
    );
    RelinkError::new("ambiguous-item", message)
}

fn place(snapshot: &Snapshot, item: usize) -> String {
    let path = &snapshot.documents()[snapshot.items()[item].document].path;
    format!("{path}:{}", snapshot.item(item).line)
}

/// The contents of the document at `document` as text.
fn text(snapshot: &Snapshot, document: usize) -> Result<&str, RelinkError> {
    let document = &snapshot.documents()[document];
    std::str::from_utf8(&document.file.contents).map_err(|_| {
        let message = format!(
            "{} is not UTF-8 text, so Mara cannot edit it",
            document.path
        );
        RelinkError::new("invalid-utf8", message)
    })
}

fn item_text(snapshot: &Snapshot, item: usize) -> Result<&str, RelinkError> {
    let contents = text(snapshot, snapshot.items()[item].document)?;
    Ok(&contents[snapshot.item(item).span.clone()])
}

/// The refusal of a link whose `part` Mara cannot locate in its bytes.
fn unlocatable(path: &str, line: usize, part: &str) -> RelinkError {
    let message = format!(
        "Mara cannot tell which bytes of the link at {path}:{line} write {part}, so it cannot \
         rewrite them: write it as `[text](path#MID)`"
    );
    RelinkError {
        diagnostics: vec![Diagnostic::error(
            UNREADABLE_EDIT,
            path,
            line,
            message.clone(),
        )],
        ..RelinkError::new(UNREADABLE_EDIT, message)
    }
}

/// The bytes of the item at `item` as they go into the document at `destination`: each of its
/// links to itself names the destination, and each other link the file it named before.
fn moved_text(snapshot: &Snapshot, item: usize, destination: &str) -> Result<String, RelinkError> {
    let document = &snapshot.documents()[snapshot.items()[item].document];
    let contents = text(snapshot, snapshot.items()[item].document)?;
    let parsed = snapshot.item(item);
    let span = parsed.span.clone();
    let from = document.path.as_str();
    // A definition outside the item stays behind, and what uses it would read as plain text in
    // another document: refused, never lost (mara-jnvzxr).
    let references = &document.file.document.references;
    let first = references.partition_point(|reference| reference.span.start < span.start);
    for reference in references[first..]
        .iter()
        .take_while(|reference| reference.span.start < span.end)
    {
        let Some(definition) = &reference.definition else {
            continue;
        };
        if span.start <= definition.start && definition.end <= span.end {
            continue;
        }
        let line = |at: usize| {
            lines(&contents[..at])
                .filter(|line| line.ends_with(['\n', '\r']))
                .count()
                + 1
        };
        let (at, defined) = (line(reference.span.start), line(definition.start));
        let message = format!(
            "`{}` in the item `{}` with MID {} uses the definition at line {defined} of {from}, \
             outside the item, which a move leaves behind: write it inline, or put the \
             definition inside the item, before its last paragraph",
            &contents[reference.span.clone()],
            parsed.title,
            parsed.mid
        );
        return Err(RelinkError {
            diagnostics: vec![Diagnostic::error(
                UNREADABLE_EDIT,
                from,
                at,
                message.clone(),
            )],
            ..RelinkError::new(UNREADABLE_EDIT, message)
        });
    }
    let mut replacements: Vec<(Range<usize>, String)> = Vec::new();
    let mut definitions = Vec::new();
    for edge in snapshot.outgoing(item) {
        let link = snapshot.link(edge);
        let mid = link.fragment.as_deref().expect("an edge links to a MID");
        if has_scheme(&link.path) {
            continue;
        }
        let file = match mid == parsed.mid {
            true => destination.to_owned(),
            false => match resolve(from, &link.path) {
                Some(file) => file,
                None => continue,
            },
        };
        if resolve(destination, &link.path).as_deref() == Some(file.as_str()) {
            continue;
        }
        let path = relative(destination, &file);
        let unlocatable = || unlocatable(from, link.line, "its path");
        match reference_at(&document.file.document, link.span.start).and_then(definition) {
            Some(definition) if span.start <= definition.start && definition.end <= span.end => {
                if !definitions.contains(&definition.start) {
                    definitions.push(definition.start);
                    let range = links::definition_path(contents, &definition, &link.path, mid)
                        .ok_or_else(unlocatable)?;
                    replacements.push((range, path));
                }
            }
            Some(_) => unreachable!("refused above: a definition outside the item"),
            None => {
                let range = links::inline_path(contents, &link.span, &link.path, mid)
                    .ok_or_else(unlocatable)?;
                replacements.push((range, path));
            }
        }
    }
    replacements.sort_by_key(|(range, _)| range.start);
    let mut out = String::with_capacity(span.len());
    let mut at = span.start;
    for (range, path) in replacements {
        out.push_str(&contents[at..range.start]);
        out.push_str(&path);
        at = range.end;
    }
    out.push_str(&contents[at..span.end]);
    Ok(out)
}

/// The link of `document` that starts at byte `at`.
fn reference_at(document: &Document, at: usize) -> Option<&Reference> {
    let references = &document.references;
    let first = references.partition_point(|reference| reference.span.start < at);
    references[first..]
        .iter()
        .take_while(|reference| reference.span.start == at)
        .find(|reference| reference.kind == ReferenceKind::Link)
}

fn definition(reference: &Reference) -> Option<Range<usize>> {
    reference.definition.clone()
}

/// The splices that give each link of `links`, all in the document at `document`, its new path.
/// A link that a definition decides gets it through the definition, which the edit writes
/// together with every link and image that uses it (DES-ITEM-WRITER). Nothing is written inside
/// `removal`.
fn retarget(
    snapshot: &Snapshot,
    document: usize,
    links: &[(&Edge, String)],
    removal: Option<&Range<usize>>,
) -> Result<Vec<Splice>, RelinkError> {
    let entry = &snapshot.documents()[document];
    let contents = text(snapshot, document)?;
    let parsed = &entry.file.document;
    let removed = |span: &Range<usize>| {
        removal.is_some_and(|removal| removal.start < span.end && span.start < removal.end)
    };
    let mut splices = Vec::new();
    let mut definitions: Vec<Range<usize>> = Vec::new();
    for (edge, path) in links {
        let link = snapshot.link(edge);
        let mid = link.fragment.as_deref().expect("an edge links to a MID");
        let unlocatable = || unlocatable(&entry.path, link.line, "its path");
        if let Some(definition) = reference_at(parsed, link.span.start).and_then(definition) {
            if !removed(&definition) && !definitions.contains(&definition) {
                let range = links::definition_path(contents, &definition, &link.path, mid)
                    .ok_or_else(unlocatable)?;
                splices.push(Splice {
                    range,
                    text: path.clone(),
                    expect: Expect::Bytes,
                });
                definitions.push(definition);
            }
            continue;
        }
        let range =
            links::inline_path(contents, &link.span, &link.path, mid).ok_or_else(unlocatable)?;
        splices.push(match (edge.source, edge.item) {
            (EdgeSource::Row(row), Some(item)) => {
                let row = &snapshot.item(item).rows[row];
                let value =
                    in_value(contents, &row.value_span, &range, path).ok_or_else(unlocatable)?;
                set_value(row, &value)
            }
            _ => Splice {
                range,
                text: path.clone(),
                expect: Expect::Bytes,
            },
        });
    }
    // What a rewritten definition decides reads differently, so the edit writes it too, as it is.
    for reference in &parsed.references {
        let decided = reference
            .definition
            .as_ref()
            .is_some_and(|definition| definitions.contains(definition));
        if decided && !removed(&reference.span) {
            splices.push(Splice {
                range: reference.span.clone(),
                text: contents[reference.span.clone()].to_owned(),
                expect: Expect::Bytes,
            });
        }
    }
    Ok(splices)
}

/// The value written at `value` of `contents` with its bytes `range` replaced by `text`, or
/// `None` when they lie outside it.
fn in_value(
    contents: &str,
    value: &Range<usize>,
    range: &Range<usize>,
    text: &str,
) -> Option<String> {
    if range.start < value.start || value.end < range.end {
        return None;
    }
    let mut written = contents[value.clone()].to_owned();
    written.replace_range(range.start - value.start..range.end - value.start, text);
    Some(written)
}

/// The splices of the document at `path` in byte order, refused when two of them overlap.
fn finish(path: &str, mut splices: Vec<Splice>) -> Result<DocumentEdit, RelinkError> {
    splices.sort_by_key(|splice| (splice.range.start, splice.range.end));
    if let Some(pair) = splices
        .windows(2)
        .find(|pair| pair[1].range.start < pair[0].range.end)
    {
        let message = format!(
            "two changes of this edit to {path} meet at byte {}, such as a link inside the text \
             where the item goes, so Mara cannot make both",
            pair[1].range.start
        );
        return Err(RelinkError::new(UNREADABLE_EDIT, message));
    }
    Ok(DocumentEdit {
        path: path.to_owned(),
        splices,
    })
}

/// The byte where an item goes in the document at `path` of `contents`: the start of line
/// `line`, which lies outside every item, or the end without a line.
fn position(
    contents: &str,
    document: &Document,
    path: &str,
    line: Option<usize>,
) -> Result<usize, RelinkError> {
    let Some(line) = line else {
        return Ok(contents.len());
    };
    let mut at = 0;
    let mut number = 1;
    for text in lines(contents) {
        if number == line {
            break;
        }
        at += text.len();
        number += 1;
    }
    if number != line {
        let message = format!(
            "{path} has {} lines, so line {line} is past its end: give a line from 1 to {number}, \
             or none for the end",
            number - 1
        );
        return Err(RelinkError::new(INVALID_POSITION, message));
    }
    if let Some(item) =
        (document.items.iter()).find(|item| item.span.start < at && at < item.span.end)
    {
        let message = format!(
            "line {line} of {path} is inside the item `{}` with MID {}, lines {} to {}: give a \
             line outside every item, or none for the end",
            item.title, item.mid, item.line, item.end_line
        );
        return Err(RelinkError::new(INVALID_POSITION, message));
    }
    Ok(at)
}

/// Whether an item placed at byte `at` would be where the item that `removal` takes is: nothing
/// but blank lines lies between them.
fn beside(contents: &str, removal: &Range<usize>, at: usize) -> bool {
    let gap = match at {
        _ if at < removal.start => &contents[at..removal.start],
        _ if at > removal.end => &contents[removal.end..at],
        _ => "",
    };
    gap.trim_matches([' ', '\t', '\n', '\r']).is_empty()
}
