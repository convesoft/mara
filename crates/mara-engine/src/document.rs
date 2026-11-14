//! One Mara document parsed into items, metadata rows, links and problems: DES-REGIONS,
//! DES-ITEM-STRUCTURE and DES-METADATA-TABLE in `docs/format.mara.md`, DES-MID-ANCHOR and
//! DES-RELATION-LINKS in `docs/identity.mara.md`.
//!
//! One `pulldown-cmark` pass per document (DES-SNAPSHOT). The result depends only on the bytes:
//! the schema and the path are applied later (DES-INPUTS-AND-KEYS), so it is keyed by
//! [`DocumentKey`].

mod html;
mod region;
#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::ops::Range;

use pulldown_cmark::{Options, Parser};

use crate::diagnostic::Diagnostic;
use crate::hash::Hash;
use crate::work::WorkCounts;

/// The version of everything [`parse`] derives from a document's bytes, search words included.
/// A change to any of it takes a new version, which changes every [`DocumentKey`].
pub const PARSER_VERSION: u32 = 7;

/// The key of a parsed document: parser version and file hash (DES-INPUTS-AND-KEYS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentKey {
    pub parser_version: u32,
    pub file: Hash,
}

impl DocumentKey {
    /// The key of the document whose contents hash to `file`.
    pub fn new(file: Hash) -> DocumentKey {
        DocumentKey {
            parser_version: PARSER_VERSION,
            file,
        }
    }
}

/// A parsed document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    /// Items in source order.
    pub items: Vec<Item>,
    /// Narrative units in source order (DES-NARRATIVE-UNITS).
    pub units: Vec<Unit>,
    /// Mentions outside every item, in source order.
    pub mentions: Vec<Link>,
    /// The search words of the items and narrative units, each once, in byte order
    /// (DES-SNAPSHOT).
    pub words: Vec<Word>,
    /// In line order.
    problems: Vec<Problem>,
}

/// A search word ([`words`]) and the items and narrative units whose text holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    /// Each node and zone once, in source order of the nodes, then zone order.
    pub postings: Vec<Posting>,
}

/// A node that holds a word, the zone of the node that holds it, and where it first occurs
/// there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Posting {
    pub node: Node,
    pub zone: Zone,
    /// The byte of the document where the word's first occurrence in the zone starts, from the
    /// parser's source ranges; for a word that starts in a decoded entity, or in a code span over
    /// several lines, where that entity or code span starts (DES-SEARCH-REQUESTS). A parsed
    /// document holds at most [`MAX_DOCUMENT_BYTES`], so every offset fits.
    pub at: u32,
}

/// An item or a narrative unit: its index in [`Document::items`] or [`Document::units`]; in a
/// snapshot, its index among the snapshot's items or units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Node {
    Item(usize),
    Unit(usize),
}

/// Where in a node a search word occurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Zone {
    /// The MID or an `ID` value of an item.
    Id,
    /// The heading of an item.
    Title,
    /// A heading in an item's body, or the heading that starts a narrative unit.
    Heading,
    /// Another metadata value of an item; for a link, its text.
    Metadata,
    /// Any other text: neither HTML nor link destinations.
    Body,
}

impl Zone {
    /// Whether a match here outranks one in the body or a field (REQ-SEARCH-RANKING).
    pub fn ranks_high(self) -> bool {
        matches!(self, Zone::Id | Zone::Title | Zone::Heading)
    }
}

/// The search words of `text`: each maximal run of letters and digits, lowercased. Queries and
/// documents are normalized alike by this function and [`word_spans`].
pub fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    word_spans(text).map(|(_, word)| word.to_lowercase())
}

/// Each maximal run of letters and digits in `text`, with its byte offset, as written.
pub fn word_spans(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut chars = text.char_indices().peekable();
    std::iter::from_fn(move || {
        let (start, _) = chars.find(|(_, c)| c.is_alphanumeric())?;
        let mut end = text.len();
        while let Some(&(at, c)) = chars.peek() {
            if !c.is_alphanumeric() {
                end = at;
                break;
            }
            chars.next();
        }
        Some((start, &text[start..end]))
    })
}

/// A complete item: anchor, heading and a metadata table with a `Type` row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub mid: String,
    /// The heading's text, without its Markdown syntax.
    pub title: String,
    /// The metadata rows that have a key and a value, in source order, the header row first. A
    /// row with more than two cells holds its first two.
    pub rows: Vec<Row>,
    /// The mentions in the heading and the body (DES-RELATION-LINKS).
    pub mentions: Vec<Link>,
    /// Bytes from the anchor to the end of the item's last block.
    pub span: Range<usize>,
    /// The anchor's line.
    pub line: usize,
    /// The line of the item's last byte.
    pub end_line: usize,
    /// Bytes of the text after the anchor in its paragraph, when the anchor is merged into one;
    /// empty, at the item's start, otherwise. It starts the item's body (DES-NARRATIVE-UNITS).
    pub lead: Range<usize>,
    /// Bytes of the blocks after the metadata table; empty at the table's end when there are none.
    pub body: Range<usize>,
}

impl Item {
    /// The value of the first row whose key matches the schema name `name`.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.rows
            .iter()
            .find(|row| row.name == name)
            .map(|row| row.value.as_str())
    }
}

/// A narrative unit: in a region that is no item, a heading and the blocks up to the next
/// heading, or the blocks before the region's first heading (DES-NARRATIVE-UNITS).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    /// The text of the heading that starts the unit, without its Markdown syntax.
    pub heading: Option<String>,
    /// Bytes from the start of its first block to the end of its last.
    pub span: Range<usize>,
    /// The line where it starts.
    pub line: usize,
    /// Its mentions among [`Document::mentions`].
    pub mentions: Range<usize>,
}

/// One `Key | Value` row of a metadata table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The key as written.
    pub key: String,
    /// The schema name the key matches ([`label_name`]).
    pub name: String,
    /// The value as written, with `\|` read as `|`.
    pub value: String,
    /// The value's text without Markdown syntax: a link's text without its destination, and no
    /// HTML. Each run of spaces, and each HTML tag, is one space.
    pub text: String,
    /// The link that is the whole value, if it is one.
    pub link: Option<Link>,
    pub line: usize,
}

/// A Markdown link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// The link text, without its Markdown syntax.
    pub text: String,
    /// The destination before its first `#`: a path, or empty for a link within the document.
    pub path: String,
    /// The destination after its first `#`.
    pub fragment: Option<String>,
    /// Bytes of the whole link.
    pub span: Range<usize>,
    pub line: usize,
}

impl Link {
    fn new(text: String, destination: &str, span: Range<usize>, line: usize) -> Link {
        let (path, fragment) = match destination.split_once('#') {
            Some((path, fragment)) => (path, Some(fragment.to_owned())),
            None => (destination, None),
        };
        Link {
            text,
            path: path.to_owned(),
            fragment,
            span,
            line,
        }
    }

    /// A body or narrative link that is not an absolute URL and whose fragment is a MID
    /// (DES-RELATION-LINKS).
    fn is_mention(&self) -> bool {
        self.fragment.as_deref().is_some_and(is_mid) && !has_scheme(&self.path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Problem {
    code: &'static str,
    message: String,
    line: usize,
}

impl Document {
    /// The problems found, as diagnostics of the document at `path`, in line order. Every one is
    /// an error.
    pub fn diagnostics<'a>(&'a self, path: &'a str) -> impl Iterator<Item = Diagnostic> + 'a {
        self.problems.iter().map(move |problem| {
            Diagnostic::error(problem.code, path, problem.line, problem.message.clone())
        })
    }
}

/// The most bytes of a document: every byte offset of it fits the `u32` of a [`Posting`]. A
/// longer document is refused before it is read (DES-REGIONS).
pub const MAX_DOCUMENT_BYTES: usize = u32::MAX as usize;

/// Parses the contents of one document and counts one parse in `work`.
pub fn parse(bytes: &[u8], work: &mut WorkCounts) -> Document {
    parse_within(bytes, MAX_DOCUMENT_BYTES, work)
}

/// [`parse`], refusing a document of more than `limit` bytes, at most [`MAX_DOCUMENT_BYTES`].
fn parse_within(bytes: &[u8], limit: usize, work: &mut WorkCounts) -> Document {
    work.parses += 1;
    if bytes.len() > limit.min(MAX_DOCUMENT_BYTES) {
        return Document {
            problems: vec![Problem {
                code: "document-too-large",
                message: format!(
                    "the document holds {} bytes, more than the {limit} Mara reads, so none of its \
                     items is read: split it into smaller documents",
                    bytes.len()
                ),
                line: 1,
            }],
            ..Document::default()
        };
    }
    let lines = LineIndex::new(bytes);
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            return Document {
                problems: vec![Problem {
                    code: "invalid-utf8",
                    message: "the document is not UTF-8 text, so none of its items is read"
                        .to_owned(),
                    line: lines.line(error.valid_up_to()),
                }],
                ..Document::default()
            };
        }
    };
    let text = lone_cr_as_lf(text);
    let text = text.as_ref();
    let (start, anchors) = front_matter(text);
    // Front matter is not Markdown, so an item in it would be lost without a word (DES-REGIONS).
    let skipped: Vec<Problem> = anchors
        .into_iter()
        .map(|(mid, offset)| Problem {
            code: "item-in-front-matter",
            message: format!(
                "the anchor of MID {mid} is inside the front matter that the `---` on the first \
                 line opens, so its item is not read; a file never starts with `---`"
            ),
            line: lines.line(offset),
        })
        .collect();
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let mut reader = region::Reader::new(text, lines);
    for (event, span) in Parser::new_ext(&text[start..], options).into_offset_iter() {
        reader.event(event, span.start + start..span.end + start);
    }
    let mut document = reader.finish();
    // The front matter precedes every other line, so its problems come first.
    document.problems.splice(0..0, skipped);
    document
}

/// The schema name that a key or value label matches: labels match case-insensitively, and a
/// space in a label stands for `_` in the name (DES-METADATA-TABLE). Schema names are ASCII.
pub fn label_name(label: &str) -> String {
    label
        .chars()
        .map(|c| {
            if c == ' ' {
                '_'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

/// Whether `text` is a MID: a canonical uppercase ULID (DES-MID-ANCHOR).
pub fn is_mid(text: &str) -> bool {
    const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let bytes = text.as_bytes();
    bytes.len() == 26 && bytes[0] <= b'7' && bytes.iter().all(|byte| CROCKFORD.contains(byte))
}

/// Whether a link destination starts with a URL scheme, as an absolute URL does.
fn has_scheme(destination: &str) -> bool {
    let Some((scheme, _)) = destination.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Where the Markdown starts: after a UTF-8 byte order mark and a YAML front matter block. Also
/// the MID anchor lines of that block, with their offsets (DES-REGIONS).
fn front_matter(text: &str) -> (usize, Vec<(&str, usize)>) {
    let bom = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let is_fence = |line: &str| line.trim_end() == "---";
    let mut rest = lines(&text[bom..]);
    let Some(opening) = rest.next().filter(|line| is_fence(line)) else {
        return (bom, Vec::new());
    };
    let mut end = bom + opening.len();
    let mut anchors = Vec::new();
    for line in rest {
        let start = end;
        end += line.len();
        if is_fence(line) {
            return (end, anchors);
        }
        if let Some(mid) = anchor_line(line) {
            anchors.push((mid, start));
        }
    }
    (bom, Vec::new())
}

/// `text` with each lone CR read as LF. CommonMark ends a line at either, but `pulldown-cmark`
/// does not end fenced code, indented code or an HTML block at a lone CR (DES-REGIONS). One byte
/// replaces one byte, so every offset stays that of the file.
fn lone_cr_as_lf(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let is_lone = |at: usize| bytes[at] == b'\r' && bytes.get(at + 1) != Some(&b'\n');
    if !(0..bytes.len()).any(is_lone) {
        return Cow::Borrowed(text);
    }
    let text = text.char_indices().map(|(at, c)| match c {
        '\r' if is_lone(at) => '\n',
        c => c,
    });
    Cow::Owned(text.collect())
}

/// The MID of `line` when it is a MID anchor on a line of its own: exactly `<a id="<MID>"></a>`
/// once the spaces, tabs and line ending around it are removed (DES-MID-ANCHOR).
fn anchor_line(line: &str) -> Option<&str> {
    line.trim_matches([' ', '\t', '\r', '\n'])
        .strip_prefix("<a id=\"")
        .and_then(|rest| rest.strip_suffix("\"></a>"))
        .filter(|mid| is_mid(mid))
}

/// The lines of `text`, each with its line ending: `\n`, `\r\n` or a lone `\r`.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let end = match rest.find(['\n', '\r']) {
            Some(at) if rest[at..].starts_with("\r\n") => at + 2,
            Some(at) => at + 1,
            None => rest.len(),
        };
        let (line, tail) = rest.split_at(end);
        rest = tail;
        Some(line)
    })
}

/// 1-based line numbers of byte offsets. A line ends at `\n`, `\r\n` or a lone `\r`, as in
/// CommonMark.
struct LineIndex {
    /// Byte offset of the start of each line.
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(bytes: &[u8]) -> LineIndex {
        let mut starts = vec![0];
        for (offset, &byte) in bytes.iter().enumerate() {
            let ends_line =
                byte == b'\n' || (byte == b'\r' && bytes.get(offset + 1) != Some(&b'\n'));
            if ends_line {
                starts.push(offset + 1);
            }
        }
        LineIndex { starts }
    }

    /// The line that holds byte `offset`.
    fn line(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset)
    }
}
