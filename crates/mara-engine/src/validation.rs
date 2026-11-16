//! Validation: the built-in checks (DES-SCHEMA-CHECKS) and the result of a validation
//! operation (DES-DIAGNOSTICS), in `docs/validation.mara.md`.

mod checks;
#[cfg(test)]
mod tests;

pub use checks::is_id;
pub(crate) use checks::{quote, resolve};

use crate::diagnostic::{Diagnostic, Severity};
use crate::hash::{Hash, KeyHasher};
use crate::json::Object;
use crate::paging::{self, Cursor};
use crate::snapshot::Snapshot;
use crate::work::WorkCounts;

/// The version of what validation derives, part of every result key.
pub const VALIDATION_VERSION: u32 = 1;

/// A problem that the checks of a snapshot found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub diagnostic: Diagnostic,
    /// The snapshot index of the item it concerns.
    pub item: Option<usize>,
    /// Whether project validation reports it. A problem that several items share, such as a
    /// repeated ID, is reported once to the project and to the validation of each item it
    /// concerns, each time at that item.
    pub primary: bool,
}

/// Runs every built-in check on `snapshot`: the parse problems of its documents, each with the
/// item that holds its line, then the checks of items and edges. Work linear in items plus
/// edges, each visit one check step in `work`.
pub fn check(snapshot: &Snapshot, work: &mut WorkCounts) -> Vec<Finding> {
    checks::run(snapshot, work)
}

/// The most names that a message lists (DES-SCHEMA-CHECKS).
pub const NAMED: usize = 10;

/// The first [`NAMED`] of `count` names, each already as the message shows it, joined by
/// commas, then `and <N> more` for the rest. Every message that lists names lists them with
/// it, so the wording is the same in all of them. Only the names listed are formatted.
pub fn list(names: impl IntoIterator<Item = String>, count: usize) -> String {
    let mut listed: Vec<String> = names.into_iter().take(NAMED).collect();
    if count > NAMED {
        listed.push(format!("and {} more", count - NAMED));
    }
    listed.join(", ")
}

/// The key of a validation result: the request, as the operation and its arguments that select
/// diagnostics, the keys of the inputs it read, and the code, path and line of the diagnostics
/// that no input key covers, such as an input that cannot be read.
pub fn result_key(request: &[&str], inputs: &[Option<Hash>], located: &[Diagnostic]) -> Hash {
    let mut key = KeyHasher::new("mara validation result", VALIDATION_VERSION);
    key.u32(request.len() as u32);
    for part in request {
        key.bytes(part.as_bytes());
    }
    for input in inputs {
        key.optional(input.as_ref());
    }
    for diagnostic in located {
        key.bytes(diagnostic.code.as_bytes());
        key.bytes(diagnostic.path.as_bytes());
        key.bytes(&(diagnostic.line as u64).to_le_bytes());
    }
    key.finish()
}

/// Every diagnostic of one validation target, with its validity and counts.
#[derive(Debug)]
pub struct Report {
    /// In path and line order.
    diagnostics: Vec<Diagnostic>,
    errors: usize,
    warnings: usize,
    key: Hash,
}

/// Why a page of a report cannot be returned.
#[derive(Debug, PartialEq, Eq)]
pub enum PageError {
    /// The cursor belongs to another result.
    StaleCursor,
    /// The diagnostic at this path and line alone exceeds a page.
    TooLarge { path: String, line: usize },
}

/// One page of a report.
#[derive(Debug)]
pub struct Page<'r> {
    pub report: &'r Report,
    pub diagnostics: Vec<&'r Diagnostic>,
    /// Their JSON forms.
    records: Vec<String>,
    pub next_cursor: Option<Cursor>,
}

impl Report {
    /// The report of `diagnostics`, sorted by path and line with their order kept otherwise.
    pub fn new(mut diagnostics: Vec<Diagnostic>, key: Hash) -> Report {
        diagnostics.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        let errors = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count();
        Report {
            warnings: diagnostics.len() - errors,
            errors,
            diagnostics,
            key,
        }
    }

    /// No error, whatever a page shows (REQ-VALIDITY-COVERS-WHOLE-PROJECT).
    pub fn valid(&self) -> bool {
        self.errors == 0
    }

    pub fn errors(&self) -> usize {
        self.errors
    }

    pub fn warnings(&self) -> usize {
        self.warnings
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// The page of at most `limit` diagnostics whose path is one of `paths` or below one,
    /// every diagnostic when `paths` is empty, from `cursor` on.
    pub fn page(
        &self,
        paths: &[&str],
        cursor: Option<&Cursor>,
        limit: usize,
    ) -> Result<Page<'_>, PageError> {
        let offset = match cursor {
            Some(cursor) if cursor.key != self.key => return Err(PageError::StaleCursor),
            Some(cursor) => cursor.offset,
            None => 0,
        };
        let selected = || {
            self.diagnostics
                .iter()
                .filter(|diagnostic| {
                    paths.is_empty() || paths.iter().any(|path| within(&diagnostic.path, path))
                })
                .skip(offset)
        };
        let records = selected().map(|diagnostic| {
            let mut record = String::new();
            diagnostic.write_json(&mut record);
            record
        });
        let filled = paging::fill(records, limit).map_err(|index| {
            let diagnostic = selected().nth(index).expect("the record was taken");
            PageError::TooLarge {
                path: diagnostic.path.clone(),
                line: diagnostic.line,
            }
        })?;
        let count = filled.records.len();
        Ok(Page {
            report: self,
            diagnostics: selected().take(count).collect(),
            records: filled.records,
            next_cursor: filled.more.then_some(Cursor {
                key: self.key,
                offset: offset + count,
            }),
        })
    }
}

impl Page<'_> {
    /// The JSON form of the result: `format_version` 1, `valid`, `errors`, `warnings`,
    /// `diagnostics` and `next_cursor`, `null` on the last page.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        let mut object = Object::new(&mut out);
        object.number("format_version", 1);
        object.bool("valid", self.report.valid());
        object.number("errors", self.report.errors as u64);
        object.number("warnings", self.report.warnings as u64);
        object.array("diagnostics", self.records.iter().map(String::as_str));
        let cursor = self.next_cursor.map(|cursor| cursor.to_string());
        object.optional_string("next_cursor", cursor.as_deref());
        object.end();
        out
    }
}

/// Whether `path` is `scope` or below it.
fn within(path: &str, scope: &str) -> bool {
    path.strip_prefix(scope)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}
