//! Read operations, each answered in bounded pages: search, `get`, `relation list` and
//! `item list` over a snapshot (`docs/retrieval/index.mara.md`), `schema list` and `schema get` over
//! a schema (REQ-SCHEMA-INSPECTION in `docs/project.mara.md`).
//!
//! An operation computes its whole result, then writes the page that its cursor names
//! (DES-PAGING). The page's JSON form, which bounds its bytes, is written here, so that every
//! host returns the same pages.

mod distance;
mod node;
mod page;
mod query;
mod relations;
mod schema_view;
mod search;
#[cfg(test)]
mod tests;

use std::fmt;

use crate::diagnostic::Diagnostic;
use crate::hash::{Hash, KeyHasher};
use crate::paging::{Cursor, DEFAULT_LIMIT};
use crate::snapshot::Snapshot;

pub use node::{ItemSummary, NodeSummary, Portion, RowPart, get, get_code};
pub(crate) use node::{Named, resolve};
pub use page::{DIAGNOSTIC_BYTES, shown_diagnostics};
pub use query::{
    ExplainedRow, GateRequest, JudgedItem, QueryRow, ReportRequest, RowsRequest, gate_page,
    query_list, query_rows, report_page,
};
pub use relations::{
    Connection, ConnectionKind, Direction, Location, Neighbour, RelationPage, relation_list,
};
pub use schema_view::{Declaration, SchemaLayer, schema_get, schema_layer, schema_list};
pub use search::{EXCERPT_CHARS, EXCERPT_LEAD, Excerpt, Filters, SearchHit, item_list, search};

/// The version of the JSON form of every read result.
pub const FORMAT_VERSION: u64 = 1;
/// The version of what the read operations derive, part of every result key.
const READ_VERSION: u32 = 2;

/// The documents a read runs on: the snapshot, and the inventory's problems. Every result
/// carries them: without the inputs they concern, it may be incomplete.
#[derive(Clone, Copy, Debug)]
pub struct Documents<'a> {
    pub snapshot: &'a Snapshot,
    pub diagnostics: &'a [Diagnostic],
}

/// Which page of a result a request asks for.
#[derive(Clone, Copy, Debug)]
pub struct Paging<'c> {
    /// Records per page: 1 to [`crate::paging::MAX_LIMIT`].
    pub limit: usize,
    /// The cursor of the page before, or `None` for the first page.
    pub cursor: Option<&'c Cursor>,
}

impl Default for Paging<'_> {
    fn default() -> Self {
        Paging {
            limit: DEFAULT_LIMIT,
            cursor: None,
        }
    }
}

/// One page of a result: its records and its JSON form, which `--format json` prints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page<R> {
    pub records: Vec<R>,
    /// The cursor of the next page, while records remain.
    pub next_cursor: Option<Cursor>,
    pub json: String,
}

/// Why a read returns no page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// No item has the ID or MID given.
    UnknownItem(String),
    /// Several items have the ID or MID given.
    AmbiguousItem(String),
    /// The path, or path and line, names no document, item or narrative unit that the read
    /// takes.
    UnknownNode(String),
    /// The schema declares no type or relation of the name given.
    UnknownDeclaration(String),
    /// The cursor belongs to another result: the request, or the inputs it reads, changed.
    StaleCursor,
    /// A record cannot fit one page: the diagnostic names it.
    TooLarge(Box<Diagnostic>),
    /// The document is not UTF-8 text, so it has no content to return.
    NotText(String),
}

impl ReadError {
    /// The stable code of the failure (DES-OPERATIONS).
    pub fn code(&self) -> &'static str {
        match self {
            ReadError::UnknownItem(_) => "unknown-item",
            ReadError::AmbiguousItem(_) => "ambiguous-item",
            ReadError::UnknownNode(_) => "unknown-node",
            ReadError::UnknownDeclaration(_) => "unknown-declaration",
            ReadError::StaleCursor => "stale-cursor",
            ReadError::TooLarge(diagnostic) => diagnostic.code,
            ReadError::NotText(_) => "invalid-utf8",
        }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::UnknownItem(message)
            | ReadError::AmbiguousItem(message)
            | ReadError::UnknownNode(message)
            | ReadError::UnknownDeclaration(message)
            | ReadError::NotText(message) => f.write_str(message),
            ReadError::StaleCursor => f.write_str(
                "the cursor belongs to another result: the project or the request changed since \
                 the page that returned it; read again without `--cursor`",
            ),
            ReadError::TooLarge(diagnostic) => f.write_str(&diagnostic.message),
        }
    }
}

/// The code of a record that cannot fit one page.
const TOO_LARGE: &str = "record-too-large";

/// The key of a read result: the key of the layer it reads ([`Snapshot::key`] or
/// [`schema_layer`]), the operation, every input but the page size and the cursor, and the
/// diagnostics it carries that no layer key covers, the inventory's: their code, path, line and
/// message. Each part is distinct from any other split of the same bytes. A cursor carries the
/// key (DES-PAGING).
pub(crate) fn result_key<'p>(
    layer: Hash,
    operation: &str,
    parts: impl IntoIterator<Item = &'p str>,
    diagnostics: &[Diagnostic],
) -> Hash {
    let mut key = KeyHasher::new("mara read result", READ_VERSION);
    key.hash(&layer);
    key.bytes(operation.as_bytes());
    let parts: Vec<&str> = parts.into_iter().collect();
    key.bytes(&(parts.len() as u64).to_le_bytes());
    for part in parts {
        key.bytes(part.as_bytes());
    }
    for diagnostic in diagnostics {
        key.bytes(diagnostic.code.as_bytes());
        key.bytes(diagnostic.path.as_bytes());
        key.bytes(&(diagnostic.line as u64).to_le_bytes());
        key.bytes(diagnostic.message.as_bytes());
    }
    key.finish()
}
