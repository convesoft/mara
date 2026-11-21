//! List pages and the JSON form of read results (REQ-BOUNDED-PAGES and DES-PAGING in
//! `docs/retrieval/index.mara.md`, DES-READ-RESULTS in `docs/retrieval/index.mara.md`).

use crate::diagnostic::Diagnostic;
use crate::hash::Hash;
use crate::json::{self, Object};
use crate::paging::{self, Cursor, PAGE_BYTES};

use super::{FORMAT_VERSION, Page, Paging, ReadError, TOO_LARGE};

/// The most bytes of JSON that the inventory's diagnostics take on one page: the first of them,
/// in path and line order, as many as fit (DES-READ-RESULTS).
pub const DIAGNOSTIC_BYTES: usize = 8_192;

/// How many of `diagnostics`, from the first, a page shows: the first `kept`, whatever their
/// size, then as many of the others as fit in [`DIAGNOSTIC_BYTES`].
pub fn shown_diagnostics(diagnostics: &[Diagnostic], kept: usize) -> usize {
    shown(diagnostics, kept).len()
}

/// The JSON forms of the diagnostics that a page shows.
fn shown(diagnostics: &[Diagnostic], kept: usize) -> Vec<String> {
    let mut used = 0;
    let mut shown = Vec::new();
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        let mut record = String::new();
        diagnostic.write_json(&mut record);
        if index < kept {
            shown.push(record);
            continue;
        }
        let cost = record.len() + usize::from(used > 0);
        if used + cost > DIAGNOSTIC_BYTES {
            break;
        }
        used += cost;
        shown.push(record);
    }
    shown
}

/// Appends the members `diagnostics`, those of `diagnostics` that a page shows, the first
/// `kept` always, and `diagnostics_total`, how many there are.
pub(crate) fn diagnostics(object: &mut Object<'_>, diagnostics: &[Diagnostic], kept: usize) {
    let shown = shown(diagnostics, kept);
    object.array("diagnostics", shown.iter().map(String::as_str));
    object.number("diagnostics_total", diagnostics.len() as u64);
}

/// A member before a page's records, such as the node of `relation list`.
pub(crate) struct Head {
    pub(crate) name: &'static str,
    /// Its JSON form.
    pub(crate) json: String,
    /// Where the node it describes starts, to name it when it cannot fit a page.
    pub(crate) path: String,
    pub(crate) line: usize,
}

/// What a list page holds besides its records.
pub(crate) struct Envelope<'e> {
    pub(crate) head: Option<Head>,
    /// The name of the records' array.
    pub(crate) records: &'static str,
    pub(crate) diagnostics: &'e [Diagnostic],
    /// How many of `diagnostics`, from the first, every page shows whatever their size, such as
    /// the problems that made a gate unavailable; the budget bounds the others.
    pub(crate) kept: usize,
}

impl Envelope<'_> {
    /// The page's JSON form: its records, each already in JSON form, and its cursor.
    fn write(&self, records: &[String], next_cursor: Option<&Cursor>) -> String {
        let mut out = String::new();
        let mut object = Object::new(&mut out);
        object.number("format_version", FORMAT_VERSION);
        if let Some(head) = &self.head {
            object.key(head.name).push_str(&head.json);
        }
        object.array(self.records, records.iter().map(String::as_str));
        diagnostics(&mut object, self.diagnostics, self.kept);
        let cursor = next_cursor.map(Cursor::to_string);
        object.optional_string("next_cursor", cursor.as_deref());
        object.end();
        out
    }
}

/// Appends `values` as an array of JSON strings, the member `name`.
pub(crate) fn strings<'v>(
    object: &mut Object<'_>,
    name: &str,
    values: impl IntoIterator<Item = &'v str>,
) {
    let quoted: Vec<String> = values
        .into_iter()
        .map(|value| {
            let mut out = String::new();
            json::string(&mut out, value);
            out
        })
        .collect();
    object.array(name, quoted.iter().map(String::as_str));
}

/// The bytes that `text` takes inside a JSON string, without the quotes.
pub(crate) fn text_len(text: &str) -> usize {
    let mut out = String::new();
    json::string(&mut out, text);
    out.len() - 2
}

/// The longest cursor of a result of `key`, for the byte budget of its pages.
pub(crate) fn longest_cursor(key: Hash) -> Cursor {
    Cursor {
        key,
        offset: usize::MAX,
    }
}

/// The failure of a node that cannot fit a page, at `path` and `line`.
pub(crate) fn too_large(what: &str, path: &str, line: usize) -> ReadError {
    let message = format!("{what} cannot fit a page of {PAGE_BYTES} bytes");
    ReadError::TooLarge(Box::new(Diagnostic::error(TOO_LARGE, path, line, message)))
}

// @mara implements REQ-BOUNDED-PAGES
/// The page that `paging` names of the result of `key`, of `total` records. `record` gives
/// record `index` with its JSON form; a record that cannot fit an empty page fails the read
/// with the error that `too_large` gives for it, and so does a head that leaves no room. Every
/// page, with records or without, stays within [`PAGE_BYTES`]: what it holds besides its
/// records is measured with the longest cursor, and its diagnostics are bounded.
pub(crate) fn list<R>(
    key: Hash,
    paging: &Paging<'_>,
    total: usize,
    envelope: &Envelope<'_>,
    mut record: impl FnMut(usize) -> (R, String),
    too_large: impl FnOnce(&R) -> ReadError,
) -> Result<Page<R>, ReadError> {
    let offset = match paging.cursor {
        Some(cursor) if cursor.key != key => return Err(ReadError::StaleCursor),
        Some(cursor) => cursor.offset.min(total),
        None => 0,
    };
    let fixed = envelope.write(&[], Some(&longest_cursor(key))).len();
    if fixed > PAGE_BYTES
        && let Some(head) = &envelope.head
    {
        let what = format!("what the result says of {}:{}", head.path, head.line);
        return Err(self::too_large(&what, &head.path, head.line));
    }
    let mut values = Vec::new();
    let texts = (offset..total).map(|index| {
        let (value, text) = record(index);
        values.push(value);
        text
    });
    let filled = match paging::fill_within(texts, paging.limit, fixed) {
        Ok(filled) => filled,
        Err(_) => return Err(too_large(&values[0])),
    };
    values.truncate(filled.records.len());
    let next_cursor = filled.more.then_some(Cursor {
        key,
        offset: offset + filled.records.len(),
    });
    let json = envelope.write(&filled.records, next_cursor.as_ref());
    Ok(Page {
        records: values,
        next_cursor,
        json,
    })
}
