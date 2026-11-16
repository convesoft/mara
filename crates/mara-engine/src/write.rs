//! The item writer: every MID anchor, heading, metadata table, relation link and item separator
//! that Mara writes (DES-ITEM-WRITER in `docs/authoring/index.mara.md`). It returns text, or splices at
//! the parser's offsets, so that a write changes only the bytes it must (REQ-MINIMAL-REWRITE in
//! `docs/authoring/writes.mara.md`). [`ReadBack::edit`] applies the splices of one document and reads the
//! result back before the host puts it on disk.

mod read_back;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fmt;
use std::ops::Range;

use crate::diagnostic::Diagnostic;
use crate::document::{self, Document, Item, Row, front_matter, lines};
use crate::snapshot::ParsedFile;
use crate::work::WorkCounts;

/// The bytes `range` of a file, to be replaced by `text`, and what the result must read as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Splice {
    pub range: Range<usize>,
    pub text: String,
    pub expect: Expect,
}

/// What a splice must leave for the parser to read, which [`ReadBack::edit`] checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expect {
    /// Nothing beyond its bytes: an item that holds the splice reads back with it applied.
    Bytes,
    /// `text[at..at + len]` is an item text that reads back as one item of exactly those bytes.
    Item { at: usize, len: usize },
    /// The splice removes the one item it covers, and no narrative.
    Removal,
    /// The item that holds the splice reads back with this change to its metadata rows.
    Row(RowChange),
}

/// A change to the metadata rows of an item, a row named by where its text starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowChange {
    Set { at: usize, value: String },
    Add { key: String, value: String },
    Remove { at: usize },
}

/// The failure code of an edit whose result would not read back as written.
pub const UNREADABLE_EDIT: &str = "unreadable-edit";

/// Why the contents an edit gives a document would not read as the edit wrote them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Misread {
    /// The line of the document as it is now where the edit goes wrong.
    pub line: usize,
    pub message: String,
}

impl Misread {
    /// The refusal as a diagnostic of the document at `path`.
    pub fn diagnostic(&self, path: &str) -> Diagnostic {
        Diagnostic::error(UNREADABLE_EDIT, path, self.line, self.message.clone())
    }
}

/// The read-back of the documents of one edit, before anything is written
/// (ADR-READ-BACK-EVERY-EDIT): each distinct new content is parsed once, as VER-WORK-COUNTERS
/// counts it.
#[derive(Debug, Default)]
pub struct ReadBack {
    parsed: HashMap<Box<[u8]>, Document>,
}

impl ReadBack {
    /// Applies `splices`, in byte order and not overlapping, to the document `old` and reads the
    /// result back, parsing contents that no document of this edit had before and counting the
    /// parse in `work` (DES-ITEM-WRITER). Refuses it unless the front matter is the same, every
    /// item reads as before or as its splices write it, each inserted item reads back with
    /// exactly its bytes, no other item appears, no removal takes more than its item, every
    /// narrative unit no splice touches reads as before, and no parse problem appears outside an
    /// inserted item. A new document's `old` is [`ParsedFile::default`].
    pub fn edit(
        &mut self,
        old: &ParsedFile,
        splices: &[Splice],
        work: &mut WorkCounts,
    ) -> Result<ParsedFile, Misread> {
        let contents = apply(&old.contents, splices).into_boxed_slice();
        let document = match self.parsed.get(&contents) {
            Some(document) => document.clone(),
            None => {
                let document = document::parse(&contents, work);
                self.parsed.insert(contents.clone(), document.clone());
                document
            }
        };
        let new = ParsedFile { contents, document };
        read_back::check(old, &new, splices)?;
        Ok(new)
    }
}

/// `contents` with each splice applied. The splices are in byte order and do not overlap.
pub(crate) fn apply(contents: &[u8], splices: &[Splice]) -> Vec<u8> {
    let mut out = Vec::with_capacity(contents.len());
    let mut at = 0;
    for splice in splices {
        assert!(
            at <= splice.range.start && splice.range.end <= contents.len(),
            "splices in byte order, within the file"
        );
        out.extend_from_slice(&contents[at..splice.range.start]);
        out.extend_from_slice(splice.text.as_bytes());
        at = splice.range.end;
    }
    out.extend_from_slice(&contents[at..]);
    out
}

/// Why a title or value cannot be written as Mara would read it back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unwritable {
    Empty,
    LineBreak,
    /// It starts or ends with a space or a tab, which reading drops.
    Spaces,
}

impl fmt::Display for Unwritable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Unwritable::Empty => "is empty",
            Unwritable::LineBreak => "holds a line break, which a heading or table cell cannot",
            Unwritable::Spaces => "starts or ends with a space or tab, which reading drops",
        })
    }
}

/// The line ending of new lines in `contents`: that of its first line, LF when it has none.
pub fn line_ending(contents: &str) -> &'static str {
    let bytes = contents.as_bytes();
    match bytes
        .iter()
        .position(|&byte| byte == b'\n' || byte == b'\r')
    {
        Some(at) if bytes[at] == b'\n' => "\n",
        Some(at) if bytes.get(at + 1) == Some(&b'\n') => "\r\n",
        Some(_) => "\r",
        None => "\n",
    }
}

/// A new MID: the ULID of `millis` since the Unix epoch, of which it keeps the low 48 bits, and
/// 80 random bits. The host supplies both (DES-ENGINE-BOUNDARY).
pub fn mid(millis: u64, random: [u8; 10]) -> String {
    const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut value = u128::from(millis & ((1 << 48) - 1)) << 80;
    for (index, byte) in random.iter().enumerate() {
        value |= u128::from(*byte) << (8 * (9 - index));
    }
    (0..26)
        .rev()
        .map(|digit| char::from(CROCKFORD[((value >> (5 * digit)) & 31) as usize]))
        .collect()
}

/// The anchor of the item with MID `mid` (DES-MID-ANCHOR).
pub fn anchor(mid: &str) -> String {
    format!("<a id=\"{mid}\"></a>")
}

/// The label of a schema name: `_` as a space, the first letter capitalized
/// (DES-METADATA-TABLE). Keys, types and enum values are written as labels.
pub fn label(name: &str) -> String {
    let mut label = name.replace('_', " ");
    if let Some(first) = label.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    label
}

/// `value` as a table cell that reads back as `value`: `|` written `\|` (DES-METADATA-TABLE).
pub fn cell(value: &str) -> Result<String, Unwritable> {
    check_line(value)?;
    Ok(value.replace('|', "\\|"))
}

/// The ATX heading of level `level`, 1 to 6, whose text is `title` as Markdown. A run of `#` at
/// the end that the heading would drop as its closing sequence is escaped.
pub fn heading(level: usize, title: &str) -> Result<String, Unwritable> {
    assert!((1..=6).contains(&level), "heading levels are 1 to 6");
    check_line(title)?;
    let kept = title.trim_end_matches('#');
    let title = match kept.is_empty() || kept.ends_with([' ', '\t']) {
        true if kept.len() < title.len() => format!("{kept}\\{}", &title[kept.len()..]),
        _ => title.to_owned(),
    };
    Ok(format!("{} {title}", "#".repeat(level)))
}

fn check_line(text: &str) -> Result<(), Unwritable> {
    if text.is_empty() {
        Err(Unwritable::Empty)
    } else if text.contains(['\n', '\r']) {
        Err(Unwritable::LineBreak)
    } else if text.starts_with([' ', '\t']) || text.ends_with([' ', '\t']) {
        Err(Unwritable::Spaces)
    } else {
        Ok(())
    }
}

/// A metadata row of the schema name `name`, written as its label, and the cell `value`: from
/// [`cell`], [`label`] or [`link`]. The built-in key `id` is written `ID`.
pub fn row(name: &str, value: &str) -> String {
    format!("| {} | {value} |", key(name))
}

fn key(name: &str) -> String {
    match name {
        "id" => "ID".to_owned(),
        name => label(name),
    }
}

/// A cell's value as the parser reads it: `\|` as `|` (DES-METADATA-TABLE).
fn read_value(cell: &str) -> String {
    cell.replace("\\|", "|")
}

/// The metadata table of `rows`, each from [`row`], joined by `eol`. The `Type` row, if any, is
/// the header row (DES-METADATA-TABLE).
pub fn table(rows: &[String], eol: &str) -> String {
    assert!(!rows.is_empty(), "a metadata table has a row");
    let is_type = |row: &&String| row.starts_with("| Type |");
    let header = rows.iter().find(is_type).or(rows.first());
    let mut lines: Vec<&str> = header.map(String::as_str).into_iter().collect();
    lines.push("| --- | --- |");
    let mut header_written = false;
    for row in rows {
        if !header_written && Some(row) == header {
            header_written = true;
        } else {
            lines.push(row);
        }
    }
    lines.join(eol)
}

/// The relation link from the document at `from` to the item with ID `id` and MID `mid` in the
/// document at `to`, both relative to the project root (DES-RELATION-LINKS).
pub fn link(id: &str, mid: &str, from: &str, to: &str) -> String {
    let mut text = String::with_capacity(id.len());
    for c in id.chars() {
        if c.is_ascii_punctuation() && c != '-' {
            text.push('\\');
        }
        text.push(c);
    }
    format!("[{text}]({}#{mid})", relative(from, to))
}

/// The path of `to` relative to the document at `from`, empty when they are the same file. The
/// bytes that would end or change a link destination, or make a part before a `:` read as a URL
/// scheme, are percent-encoded, as validation decodes them; other characters are kept.
pub fn relative(from: &str, to: &str) -> String {
    if from == to {
        return String::new();
    }
    let mut directory: Vec<&str> = from.split('/').collect();
    directory.pop();
    let target: Vec<&str> = to.split('/').collect();
    let (target_directory, file) = target.split_at(target.len() - 1);
    let common = directory
        .iter()
        .zip(target_directory)
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<&str> = vec![".."; directory.len() - common];
    parts.extend(&target_directory[common..]);
    parts.extend(file);
    let mut path = String::new();
    for c in parts.join("/").chars() {
        if c.is_ascii_control() || " \"%&#?()<>\\|:".contains(c) {
            path.push_str(&format!("%{:02X}", c as u8));
        } else {
            path.push(c);
        }
    }
    path
}

/// A complete item: the anchor of `mid`, the heading of `level` and `title`, the table of
/// `rows` and `body`, whose line endings become `eol`. Neither starts nor ends with a line
/// ending; [`insert`] places it in a document.
pub fn item(
    mid: &str,
    level: usize,
    title: &str,
    rows: &[String],
    body: &str,
    eol: &str,
) -> Result<String, Unwritable> {
    let mut text = format!(
        "{}{eol}{eol}{}{eol}{eol}",
        anchor(mid),
        heading(level, title)?
    );
    text.push_str(&table(rows, eol));
    let body: Vec<&str> = lines(body.trim_end_matches(['\n', '\r']))
        .map(|line| line.trim_end_matches(['\n', '\r']))
        .collect();
    if !body.is_empty() {
        text.push_str(eol);
        text.push_str(eol);
        text.push_str(&body.join(eol));
    }
    Ok(text)
}

/// Puts the item text `item` at byte `at` of `contents`, a line start outside every item or the
/// end of the file, as a region of its own, with `contents`' line ending ([`line_ending`]). A
/// `---` line with a blank line on each side separates it from what comes before and after,
/// unless an existing one does. It never starts a file with `---`: before the first Markdown
/// line, after any byte order mark and front matter, nothing goes before the item
/// (DES-REGIONS).
pub fn insert(contents: &str, at: usize, item: &str) -> Splice {
    let eol = line_ending(contents);
    let floor = front_matter(contents).0;
    let at = at.max(floor);
    let around = Around { contents, floor };
    let head = &contents[..at];
    let mut text = String::new();
    if let Some(last) = around.content_before(at) {
        if !head.ends_with(['\n', '\r']) {
            text.push_str(eol);
        }
        // A blank line already follows the last line before the item.
        if last.end == at {
            text.push_str(eol);
        }
        if !around.separates(&last) {
            text.push_str("---");
            text.push_str(eol);
            text.push_str(eol);
        }
    } else if !head.is_empty() && head != "\u{feff}" && !head.ends_with(['\n', '\r']) {
        // The closing line of front matter at the end of the file, or blanks after it.
        text.push_str(eol);
    }
    let item_at = text.len();
    text.push_str(item);
    text.push_str(eol);
    if let Some(next) = around.content_from(at) {
        // The blank line written before it makes a break line a separator.
        if !is_break(&contents[next.clone()]) {
            text.push_str(eol);
            text.push_str("---");
            text.push_str(eol);
        }
        if next.start == at {
            text.push_str(eol);
        }
    }
    Splice {
        range: at..at,
        text,
        expect: Expect::Item {
            at: item_at,
            len: item.len(),
        },
    }
}

/// Removes the item of `contents` whose bytes are `span` ([`Item::span`]) with one separator
/// next to it, so that the regions around it stay apart. When nothing but blank lines comes
/// before it, the separator after it goes, so the file never starts with `---` (DES-REGIONS).
/// Text before a merged anchor on its line stays, as narrative, and so does a blank line before
/// a line that follows the item directly.
pub fn remove(contents: &str, span: Range<usize>) -> Splice {
    let removal = |range: Range<usize>, text: &str| Splice {
        range,
        text: text.to_owned(),
        expect: Expect::Removal,
    };
    let around = Around {
        contents,
        floor: front_matter(contents).0,
    };
    let first = around.line_start(span.start);
    let last_end = around.line_end(span.end.saturating_sub(1).max(span.start));
    let lead = &contents[first..span.start];
    if !is_blank(lead) {
        // A merged anchor: the text before it ends its line again once the item is gone.
        let keep = first + lead.trim_end_matches([' ', '\t']).len();
        let rest = &contents[span.end..];
        let end = span.end + rest.len() - rest.trim_start_matches([' ', '\t']).len();
        return removal(keep..end, "");
    }
    let mut before = around.content_before(first);
    let mut separator = None;
    if let Some(line) = before.clone()
        && around.separates(&line)
    {
        separator = Some(line.start);
        before = around.content_before(line.start);
    }
    if let Some(before) = before {
        // A line right after the item, such as an anchor without its separator, keeps a blank
        // line before it.
        let follows = around
            .line(last_end)
            .is_some_and(|line| !is_blank(&contents[line]));
        let text = if follows { line_ending(contents) } else { "" };
        return removal(before.end..last_end, text);
    }
    // The item comes first: what follows it starts the file, without the separator between.
    let mut next = last_end;
    while let Some(line) = around.line(next)
        && (is_blank(&contents[line.clone()]) || around.separates(&line))
    {
        next = line.end;
    }
    removal(separator.unwrap_or(first)..next, "")
}

/// Adds the row of the schema name `name` and the cell `value` ([`row`]) after the last row of
/// the metadata table of `item`, with the line ending of that row.
pub fn add_row(contents: &str, item: &Item, name: &str, value: &str) -> Splice {
    let end = item.table.end;
    let rest = &contents[end..];
    let eol = match lines(rest).next() {
        Some(line) if line.ends_with("\r\n") => "\r\n",
        Some(line) if line.ends_with('\n') => "\n",
        Some(line) if line.ends_with('\r') => "\r",
        _ => line_ending(contents),
    };
    Splice {
        range: end..end,
        text: format!("{eol}{}", row(name, value)),
        expect: Expect::Row(RowChange::Add {
            key: key(name),
            value: read_value(value),
        }),
    }
}

/// Removes the row `row` of the metadata table of `item`, its line ending included. The header
/// row's place takes the next row's text, whose line goes instead, and the delimiter row is
/// rewritten when its number of columns differs from that row's cells, as a table needs. `None`
/// for a header row with no row after it, which would leave no table.
pub fn remove_row(contents: &str, item: &Item, row: &Row) -> Option<Splice> {
    let line_start = |at: usize| contents[..at].rfind(['\n', '\r']).map_or(0, |at| at + 1);
    let line_end = |at: usize| at + lines(&contents[at..]).next().map_or(0, str::len);
    let expect = Expect::Row(RowChange::Remove { at: row.span.start });
    if row.span.start != item.table.start {
        let (start, end) = (line_start(row.span.start), line_end(row.span.end));
        // A last line without a line ending takes the one before it, so the file still ends
        // without one.
        let ended = contents[..end].ends_with(['\n', '\r']);
        let start = match ended {
            true => start,
            false => contents[..start].trim_end_matches(['\n', '\r']).len(),
        };
        return Some(Splice {
            range: start..end,
            text: String::new(),
            expect,
        });
    }
    let next = item
        .rows
        .iter()
        .find(|other| other.span.start > row.span.end)?;
    let promoted = &contents[next.span.clone()];
    let header_end = line_end(row.span.end);
    let delimiter_end = line_end(header_end);
    let delimiter = &contents[header_end..delimiter_end];
    let content = delimiter.trim_end_matches(['\n', '\r']);
    let columns = cells(promoted);
    let delimiter = match cells(content) == columns {
        true => delimiter.to_owned(),
        false => format!(
            "|{}{}",
            " --- |".repeat(columns),
            &delimiter[content.len()..]
        ),
    };
    let rest = &contents[delimiter_end..line_start(next.span.start)];
    Some(Splice {
        range: row.span.start..line_end(next.span.end),
        text: format!(
            "{promoted}{}{delimiter}{rest}",
            &contents[row.span.end..header_end]
        ),
        expect,
    })
}

/// The number of cells of a table row: its `|` that no backslash escapes split it, a leading
/// and a trailing one aside.
fn cells(row: &str) -> usize {
    let row = row.trim_matches([' ', '\t']);
    let row = row.strip_prefix('|').unwrap_or(row);
    let bytes = row.as_bytes();
    let pipes: Vec<usize> = (0..bytes.len())
        .filter(|&at| bytes[at] == b'|' && (at == 0 || bytes[at - 1] != b'\\'))
        .collect();
    let trailing = pipes
        .last()
        .is_some_and(|&at| row[at + 1..].trim().is_empty());
    pipes.len() + 1 - usize::from(trailing)
}

/// Replaces the value of `row` by the cell `value`: from [`cell`], [`label`] or [`link`].
pub fn set_value(row: &Row, value: &str) -> Splice {
    Splice {
        range: row.value_span.clone(),
        text: value.to_owned(),
        expect: Expect::Row(RowChange::Set {
            at: row.span.start,
            value: read_value(value),
        }),
    }
}

/// Whether `line`, with its line ending, holds only spaces and tabs.
fn is_blank(line: &str) -> bool {
    line.trim_matches([' ', '\t', '\n', '\r']).is_empty()
}

/// Whether `line` is a thematic break: three or more `-`, `*` or `_` of one kind, with spaces or
/// tabs between, indented by at most three spaces (DES-REGIONS).
fn is_break(line: &str) -> bool {
    let line = line.trim_end_matches(['\n', '\r', ' ', '\t']);
    let unindented = line.trim_start_matches(' ');
    if line.len() - unindented.len() > 3 {
        return false;
    }
    let mut marks = unindented.chars().filter(|c| !matches!(c, ' ' | '\t'));
    let Some(mark) = marks.next().filter(|c| matches!(c, '-' | '*' | '_')) else {
        return false;
    };
    let rest: Vec<char> = marks.collect();
    rest.len() >= 2 && rest.iter().all(|&c| c == mark)
}

/// The lines of a document around a place in it, from its first Markdown byte, `floor`, on,
/// found by scanning only as far as they reach, so that an edit of many splices costs no more
/// than the document's size.
struct Around<'c> {
    contents: &'c str,
    floor: usize,
}

impl Around<'_> {
    /// The line, with its line ending, that ends where the line starting at `start` begins.
    fn before(&self, start: usize) -> Option<Range<usize>> {
        if start <= self.floor {
            return None;
        }
        let text = &self.contents[self.floor..start];
        let text = text
            .strip_suffix("\r\n")
            .or_else(|| text.strip_suffix(['\n', '\r']))
            .unwrap_or(text);
        let line_start = text.rfind(['\n', '\r']).map_or(0, |at| at + 1);
        Some(self.floor + line_start..start)
    }

    /// The line, with its line ending, that starts at `start`.
    fn line(&self, start: usize) -> Option<Range<usize>> {
        let rest = self.contents.get(start..).filter(|rest| !rest.is_empty())?;
        Some(start..start + lines(rest).next().map_or(0, str::len))
    }

    /// Where the line that holds byte `at` starts.
    fn line_start(&self, at: usize) -> usize {
        let text = &self.contents[self.floor..at];
        text.rfind(['\n', '\r'])
            .map_or(self.floor, |end| self.floor + end + 1)
    }

    /// Where the line that holds byte `at` ends, after its line ending.
    fn line_end(&self, at: usize) -> usize {
        at + lines(&self.contents[at..]).next().map_or(0, str::len)
    }

    /// The last line that is not blank before the line starting at `start`.
    fn content_before(&self, start: usize) -> Option<Range<usize>> {
        let mut at = start;
        while let Some(line) = self.before(at) {
            if !is_blank(&self.contents[line.clone()]) {
                return Some(line);
            }
            at = line.start;
        }
        None
    }

    /// The first line that is not blank from the line starting at `start` on.
    fn content_from(&self, start: usize) -> Option<Range<usize>> {
        let mut at = start;
        while let Some(line) = self.line(at) {
            if !is_blank(&self.contents[line.clone()]) {
                return Some(line);
            }
            at = line.end;
        }
        None
    }

    /// Whether `line` is a thematic break that separates regions: after a blank line or none.
    /// After text, `---` is a setext underline instead (DES-REGIONS).
    fn separates(&self, line: &Range<usize>) -> bool {
        is_break(&self.contents[line.clone()])
            && self
                .before(line.start)
                .is_none_or(|previous| is_blank(&self.contents[previous]))
    }
}
