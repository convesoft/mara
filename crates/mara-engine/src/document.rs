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
pub const PARSER_VERSION: u32 = 1;

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
    /// Mentions outside every item, in source order.
    pub mentions: Vec<Link>,
    /// The search words of the items, each once, in byte order (DES-SNAPSHOT).
    pub words: Vec<Word>,
    /// In line order.
    problems: Vec<Problem>,
}

/// A search word ([`words`]) and the items whose text holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    /// Each item and zone once, in item order, then zone order.
    pub postings: Vec<Posting>,
}

/// An item that holds a word, and the zone of the item that holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Posting {
    /// The item's index in [`Document::items`]; in a snapshot, its index in the snapshot.
    pub item: usize,
    pub zone: Zone,
}

/// Where in an item a search word occurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Zone {
    /// The MID or an `ID` value.
    Id,
    Title,
    /// Another metadata value; for a link, its text.
    Metadata,
    /// The text of the body: neither HTML nor link destinations.
    Body,
}

/// The search words of `text`: each maximal run of letters and digits, lowercased. Queries and
/// documents are normalized alike by this one function.
pub fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
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

/// Parses the contents of one document and counts one parse in `work`.
pub fn parse(bytes: &[u8], work: &mut WorkCounts) -> Document {
    work.parses += 1;
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
