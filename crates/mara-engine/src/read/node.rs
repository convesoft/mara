//! Nodes: how a request names one, what a result says of one, and `get` (REQ-READ-NODE,
//! DES-NARRATIVE-UNITS and DES-PAGING in `docs/retrieval/index.mara.md`).

use std::ops::Range;

use crate::diagnostic::Diagnostic;
use crate::document::{Node, is_mid, label_name};
use crate::hash::Hash;
use crate::json::Object;
use crate::paging::{Cursor, PAGE_BYTES};
use crate::snapshot::Snapshot;
use crate::validation::list;

use super::page::{self, text_len};
use super::{Documents, FORMAT_VERSION, ReadError, TOO_LARGE, result_key};

/// A node that a request names: an item, a narrative unit or a whole document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Named {
    Node(Node),
    Document(usize),
}

/// What a result says of an item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemSummary {
    pub path: String,
    /// The anchor's line.
    pub line: usize,
    /// The value of its first `ID` row.
    pub id: Option<String>,
    pub mid: String,
    /// The schema name its `Type` label matches.
    pub type_name: String,
    pub title: String,
}

/// What a result says of a node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeSummary {
    Item(ItemSummary),
    Unit {
        path: String,
        line: usize,
        heading: Option<String>,
    },
    Document {
        path: String,
    },
    /// A code location at the basic level, `<path>:<line>`, or a whole code file
    /// (DES-CODE-MARKERS).
    Code {
        path: String,
        line: Option<usize>,
    },
}

impl ItemSummary {
    pub(crate) fn of(snapshot: &Snapshot, item: usize) -> ItemSummary {
        let entry = &snapshot.items()[item];
        let parsed = snapshot.item(item);
        ItemSummary {
            path: snapshot.documents()[entry.document].path.clone(),
            line: parsed.line,
            id: parsed.value("id").map(str::to_owned),
            mid: parsed.mid.clone(),
            type_name: parsed.value("type").map(label_name).unwrap_or_default(),
            title: parsed.title.clone(),
        }
    }

    fn write(&self, object: &mut Object<'_>) {
        object.string("kind", "item");
        object.string("path", &self.path);
        object.number("line", self.line as u64);
        object.optional_string("id", self.id.as_deref());
        object.string("mid", &self.mid);
        object.string("type", &self.type_name);
        object.string("title", &self.title);
    }

    /// The JSON form of the summary.
    pub(crate) fn json(&self) -> String {
        NodeSummary::Item(self.clone()).json(|_| {})
    }
}

impl NodeSummary {
    pub(crate) fn of(snapshot: &Snapshot, named: Named) -> NodeSummary {
        match named {
            Named::Node(Node::Item(item)) => NodeSummary::Item(ItemSummary::of(snapshot, item)),
            Named::Node(Node::Unit(unit)) => {
                let entry = snapshot.units()[unit];
                let parsed = snapshot.unit(unit);
                NodeSummary::Unit {
                    path: snapshot.documents()[entry.document].path.clone(),
                    line: parsed.line,
                    heading: parsed.heading.clone(),
                }
            }
            Named::Document(document) => NodeSummary::Document {
                path: snapshot.documents()[document].path.clone(),
            },
        }
    }

    /// The JSON form of the summary, `members` added at its end.
    pub(crate) fn json(&self, members: impl FnOnce(&mut Object<'_>)) -> String {
        let mut out = String::new();
        let mut object = Object::new(&mut out);
        match self {
            NodeSummary::Item(item) => item.write(&mut object),
            NodeSummary::Unit {
                path,
                line,
                heading,
            } => {
                object.string("kind", "narrative");
                object.string("path", path);
                object.number("line", *line as u64);
                object.optional_string("heading", heading.as_deref());
            }
            NodeSummary::Document { path } => {
                object.string("kind", "document");
                object.string("path", path);
            }
            NodeSummary::Code { path, line } => {
                object.string("kind", "code");
                object.string("path", path);
                match line {
                    Some(line) => object.number("line", *line as u64),
                    None => object.key("line").push_str("null"),
                }
                object.optional_string("declaration", None);
            }
        }
        members(&mut object);
        object.end();
        out
    }

    /// The path and line where the node starts.
    pub fn location(&self) -> (&str, usize) {
        match self {
            NodeSummary::Item(item) => (&item.path, item.line),
            NodeSummary::Unit { path, line, .. } => (path, *line),
            NodeSummary::Document { path } => (path, 1),
            NodeSummary::Code { path, line } => (path, line.unwrap_or(1)),
        }
    }
}

/// The node that `address` names: an item by MID or `ID` value, `<path>:<line>` for the item or
/// narrative unit that holds that line, or `<path>` for the whole document. Paths are relative
/// to the project root. An ID or MID that several items have names none of them.
pub(crate) fn resolve(snapshot: &Snapshot, address: &str) -> Result<Named, ReadError> {
    if is_mid(address) {
        return one_item(snapshot, snapshot.items_with_mid(address), "MID", address);
    }
    let (path, line) = match address.rsplit_once(':') {
        Some((path, line))
            if path.ends_with(".mara.md")
                && !line.is_empty()
                && line.bytes().all(|b| b.is_ascii_digit()) =>
        {
            (path, Some(line))
        }
        _ if address.ends_with(".mara.md") => (address, None),
        _ => return one_item(snapshot, snapshot.items_with_id(address), "ID", address),
    };
    let document = snapshot.document(path).ok_or_else(|| {
        ReadError::UnknownNode(format!(
            "{path} is not a document of the project: name it by its path from the project root, \
             as search returns it"
        ))
    })?;
    let Some(line) = line else {
        return Ok(Named::Document(document));
    };
    let missing = || {
        ReadError::UnknownNode(format!(
            "{path} has no item or narrative unit at line {line}"
        ))
    };
    let line: usize = line.parse().map_err(|_| missing())?;
    let entry = &snapshot.documents()[document];
    let lines = line_range(&entry.file.contents, line).ok_or_else(missing)?;
    let holds = |span: &Range<usize>| span.start < lines.end && lines.start < span.end;
    // At most one item and one unit share a line: the text before a merged anchor and the item
    // that starts at it. The node that starts first on the line is the one search returns there.
    let items = &entry.file.document.items;
    let local = items.partition_point(|item| item.span.end <= lines.start);
    let item = items
        .get(local)
        .filter(|item| holds(&item.span))
        .map(|item| (item.span.start, Node::Item(entry.items.start + local)));
    let units = &entry.file.document.units;
    let local = units.partition_point(|unit| unit.span.end <= lines.start);
    let unit = units
        .get(local)
        .filter(|unit| holds(&unit.span))
        .map(|unit| (unit.span.start, Node::Unit(entry.units.start + local)));
    match (item, unit) {
        (Some(item), Some(unit)) => Ok(Named::Node(if unit.0 < item.0 { unit.1 } else { item.1 })),
        (Some((_, node)), None) | (None, Some((_, node))) => Ok(Named::Node(node)),
        (None, None) => Err(missing()),
    }
}

/// The one item of `items`, those with `kind` value `value`, each once.
fn one_item(
    snapshot: &Snapshot,
    items: &[usize],
    kind: &str,
    value: &str,
) -> Result<Named, ReadError> {
    match items {
        [item] => Ok(Named::Node(Node::Item(*item))),
        [] => Err(ReadError::UnknownItem(format!(
            "no item has the {kind} `{value}`"
        ))),
        several => {
            let places = several.iter().map(|&item| {
                let summary = ItemSummary::of(snapshot, item);
                format!("{}:{}", summary.path, summary.line)
            });
            Err(ReadError::AmbiguousItem(format!(
                "{} items have the {kind} `{value}`, at {}: name one by `<path>:<line>`",
                several.len(),
                list(places, several.len())
            )))
        }
    }
}

/// Bytes of line `line`, 1-based, of `contents`, its line ending included. A line ends at LF,
/// CRLF or a lone CR (DES-REGIONS).
fn line_range(contents: &[u8], line: usize) -> Option<Range<usize>> {
    if line == 0 {
        return None;
    }
    let mut start = 0;
    for _ in 1..line {
        start += line_end(&contents[start..])?;
    }
    if start >= contents.len() && !(start == 0 && line == 1) {
        return None;
    }
    let end = start + line_end(&contents[start..]).unwrap_or(contents.len() - start);
    Some(start..end)
}

/// The length of the first line of `bytes` with its line ending, or `None` without a line
/// ending.
fn line_end(bytes: &[u8]) -> Option<usize> {
    let at = bytes.iter().position(|&b| b == b'\n' || b == b'\r')?;
    Some(if bytes[at..].starts_with(b"\r\n") {
        at + 2
    } else {
        at + 1
    })
}

/// One page of a node that `get` reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Portion {
    pub node: NodeSummary,
    /// The hash of an item's region (DES-FACT-BRIDGE): lowercase hexadecimal BLAKE3.
    pub revision: Option<String>,
    /// The metadata rows of this page, in source order.
    pub rows: Vec<RowPart>,
    /// The content of this page: it continues the content of the pages before.
    pub content: String,
    pub next_cursor: Option<Cursor>,
    pub json: String,
}

/// A metadata row, or a part of one whose value is longer than a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowPart {
    /// The key as written.
    pub key: String,
    /// The value as written, or a part of it: the parts of one row concatenate to its value.
    pub value: String,
    pub line: usize,
    /// The rest of the value follows on the next page.
    pub partial: bool,
}

impl RowPart {
    fn json(key: &str, value: &str, line: usize, partial: bool) -> String {
        let mut out = String::new();
        let mut object = Object::new(&mut out);
        object.string("key", key);
        object.string("value", value);
        object.number("line", line as u64);
        object.bool("partial", partial);
        object.end();
        out
    }
}

/// The smallest room that lets a page take part of a value or a line: one escaped character.
const LEAST_ROOM: usize = 6;

/// `get`: the node that `address` names ([`resolve`]), its metadata rows first, then its
/// content, in consecutive pages of at most [`PAGE_BYTES`] of JSON. A line is cut only when it
/// does not fit an empty page, and then at a character boundary; so is a row's value. A
/// cursor's offset counts the bytes of the row values, then of the content, before its page.
pub fn get(
    documents: &Documents<'_>,
    address: &str,
    cursor: Option<&Cursor>,
) -> Result<Portion, ReadError> {
    let snapshot = documents.snapshot;
    let named = resolve(snapshot, address)?;
    let node = NodeSummary::of(snapshot, named);
    let (contents, rows, content, revision) = match named {
        Named::Node(Node::Item(item)) => {
            let document = &snapshot.documents()[snapshot.items()[item].document];
            let contents = text(&document.file.contents, &document.path)?;
            let parsed = snapshot.item(item);
            let rows: Vec<(&str, &str, usize)> = parsed
                .rows
                .iter()
                .map(|row| (row.key.as_str(), row.value.as_str(), row.line))
                .collect();
            let revision = snapshot.revision(item);
            // The text after a merged anchor starts the item's content (DES-NARRATIVE-UNITS).
            let content = match parsed.lead.is_empty() {
                true => parsed.body.clone(),
                false => parsed.lead.start..parsed.span.end,
            };
            (contents, rows, content, Some(revision))
        }
        Named::Node(Node::Unit(unit)) => {
            let document = &snapshot.documents()[snapshot.units()[unit].document];
            let span = snapshot.unit(unit).span.clone();
            let contents = text(&document.file.contents, &document.path)?;
            (contents, Vec::new(), span, None)
        }
        Named::Document(index) => {
            let document = &snapshot.documents()[index];
            let contents = std::str::from_utf8(&document.file.contents)
                .map_err(|_| ReadError::NotText(format!("{} is not UTF-8 text", document.path)))?;
            (contents, Vec::new(), 0..contents.len(), None)
        }
    };
    let content = &contents[content];
    let key = result_key(snapshot.key().0, "get", [address], documents.diagnostics);
    page_of(
        node,
        revision,
        &rows,
        content,
        key,
        cursor,
        documents.diagnostics,
    )
}

/// `get` of a code location or file at the basic level (DES-CODE-MARKERS): `node` names the
/// code file whose contents are `contents`, with hash `hash`; the content is the file from the
/// node's line to its end, in pages as [`get`] gives them.
pub fn get_code(
    node: NodeSummary,
    hash: Hash,
    contents: &[u8],
    cursor: Option<&Cursor>,
    diagnostics: &[Diagnostic],
) -> Result<Portion, ReadError> {
    let NodeSummary::Code { path, line } = &node else {
        unreachable!("get_code takes a code node");
    };
    let text = text(contents, path)?;
    let start = match line {
        Some(line) => {
            line_starts(text.as_bytes(), *line).ok_or_else(|| missing_line(path, *line))?
        }
        None => 0,
    };
    let address = match line {
        Some(line) => format!("{path}:{line}"),
        None => path.clone(),
    };
    let key = result_key(hash, "get", [address.as_str()], diagnostics);
    page_of(node, None, &[], &text[start..], key, cursor, diagnostics)
}

/// The failure of a line that a code file lacks.
fn missing_line(path: &str, line: usize) -> ReadError {
    ReadError::UnknownNode(format!("{path} has no line {line}"))
}

/// Where line `line`, 1-based, of `contents` starts, when it has that line: a line ends at a
/// line feed, and a last line without one counts.
fn line_starts(contents: &[u8], line: usize) -> Option<usize> {
    let mut start = 0;
    for _ in 1..line.max(1) {
        start += contents[start..].iter().position(|&b| b == b'\n')? + 1;
    }
    (line >= 1 && start < contents.len()).then_some(start)
}

/// One page, at `cursor`, of a node's `rows` and then its `content`.
fn page_of(
    node: NodeSummary,
    revision: Option<String>,
    rows: &[(&str, &str, usize)],
    content: &str,
    key: Hash,
    cursor: Option<&Cursor>,
    diagnostics: &[Diagnostic],
) -> Result<Portion, ReadError> {
    let start = match cursor {
        Some(cursor) if cursor.key != key => return Err(ReadError::StaleCursor),
        Some(cursor) => cursor.offset,
        None => 0,
    };
    // The row and the offset in its value, or the offset in the content, where the page starts.
    let (mut row, mut offset, mut at) = (0, start, 0);
    while let Some((_, value, _)) = rows.get(row) {
        if offset < value.len() {
            break;
        }
        offset -= value.len();
        row += 1;
    }
    if row == rows.len() {
        (at, offset) = (offset, 0);
    }
    let starts_inside = match rows.get(row) {
        Some((_, value, _)) => value.is_char_boundary(offset),
        None => at <= content.len() && content.is_char_boundary(at),
    };
    if !starts_inside {
        return Err(ReadError::StaleCursor);
    }

    let node_json = node.json(|object| {
        if let Some(revision) = &revision {
            object.string("revision", revision);
        }
    });
    let write = |rows: &[String], content: &str, next: Option<&Cursor>| {
        let mut out = String::new();
        let mut object = Object::new(&mut out);
        object.number("format_version", FORMAT_VERSION);
        object.key("node").push_str(&node_json);
        object.array("rows", rows.iter().map(String::as_str));
        object.string("content", content);
        page::diagnostics(&mut object, diagnostics, 0);
        let cursor = next.map(Cursor::to_string);
        object.optional_string("next_cursor", cursor.as_deref());
        object.end();
        out
    };
    let (path, node_line) = node.location();
    let too_large = |what: String, line: usize| {
        let message = format!("{what} cannot fit a page of {PAGE_BYTES} bytes");
        ReadError::TooLarge(Box::new(Diagnostic::error(TOO_LARGE, path, line, message)))
    };
    let fixed = write(&[], "", Some(&page::longest_cursor(key))).len();
    let budget = PAGE_BYTES
        .checked_sub(fixed)
        .filter(|&room| room >= LEAST_ROOM)
        .ok_or_else(|| {
            too_large(
                format!("what the result says of {path}:{node_line}"),
                node_line,
            )
        })?;

    let mut used = 0;
    let mut parts = Vec::new();
    let mut texts = Vec::new();
    let mut rows_left = false;
    while let Some(&(key, value, line)) = rows.get(row) {
        let rest = &value[offset..];
        let whole = RowPart::json(key, rest, line, false);
        let cost = whole.len() + usize::from(!texts.is_empty());
        if used + cost <= budget {
            used += cost;
            texts.push(whole);
            parts.push(RowPart {
                key: key.to_owned(),
                value: rest.to_owned(),
                line,
                partial: false,
            });
            (row, offset) = (row + 1, 0);
            continue;
        }
        rows_left = true;
        if !texts.is_empty() {
            break;
        }
        // The row does not fit an empty page: its value is cut at a character boundary.
        let room = budget.checked_sub(RowPart::json(key, "", line, true).len());
        let Some(room) = room.filter(|&room| room >= LEAST_ROOM) else {
            return Err(too_large(
                format!("the metadata row at {path}:{line}"),
                line,
            ));
        };
        let length = cut(rest, room);
        texts.push(RowPart::json(key, &rest[..length], line, true));
        parts.push(RowPart {
            key: key.to_owned(),
            value: rest[..length].to_owned(),
            line,
            partial: true,
        });
        offset += length;
        break;
    }
    let mut page_content = "";
    if !rows_left {
        let room = budget - used;
        let rest = &content[at..];
        let (mut length, mut size) = (0, 0);
        while length < rest.len() {
            let tail = &rest.as_bytes()[length..];
            let end = length + line_end(tail).unwrap_or(tail.len());
            let line_size = text_len(&rest[length..end]);
            if size + line_size > room {
                break;
            }
            size += line_size;
            length = end;
        }
        if length == 0 && !rest.is_empty() && texts.is_empty() {
            // The line does not fit an empty page: it is cut at a character boundary.
            if room < LEAST_ROOM {
                return Err(too_large(
                    format!("the content of {path}:{node_line}"),
                    node_line,
                ));
            }
            length = cut(rest, room);
        }
        page_content = &rest[..length];
        at += length;
    }
    let read: usize = rows[..row].iter().map(|(_, value, _)| value.len()).sum();
    let position = read + offset + at;
    let more = row < rows.len() || at < content.len();
    let next_cursor = more.then_some(Cursor {
        key,
        offset: position,
    });
    let json = write(&texts, page_content, next_cursor.as_ref());
    debug_assert!(json.len() <= PAGE_BYTES);
    Ok(Portion {
        node,
        revision,
        rows: parts,
        content: page_content.to_owned(),
        next_cursor,
        json,
    })
}

/// The contents of the document at `path` as text; a document with items or units is UTF-8.
fn text<'c>(contents: &'c [u8], path: &str) -> Result<&'c str, ReadError> {
    std::str::from_utf8(contents)
        .map_err(|_| ReadError::NotText(format!("{path} is not UTF-8 text")))
}

/// The longest start of `text`, at a character boundary, that takes at most `room` bytes in a
/// JSON string; at least one character when `room` is at least [`LEAST_ROOM`].
fn cut(text: &str, room: usize) -> usize {
    let mut size = 0;
    let mut buffer = [0; 4];
    for (at, c) in text.char_indices() {
        size += text_len(c.encode_utf8(&mut buffer));
        if size > room {
            return at;
        }
    }
    text.len()
}
