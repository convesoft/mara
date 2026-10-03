use crate::*;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ValidationTargetKind {
    Project,
    Item,
    Schema,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ValidationTarget {
    pub kind: ValidationTargetKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ValidationScope {
    Project,
    Schema,
    Document,
    Item,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidationDiagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub scope: ValidationScope,
    pub message: String,
    pub location: DiagnosticLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<DiagnosticItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obligation: Option<DiagnosticObligation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ValidationDiagnostic {
    pub fn new(
        code: DiagnosticCode,
        severity: Severity,
        scope: ValidationScope,
        location: DiagnosticLocation,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity: code.severity(severity),
            scope,
            message: message.into(),
            path: location.path.clone(),
            line: location.line,
            location,
            item: None,
            rule: None,
            obligation: None,
            details: None,
        }
    }
    pub fn from_source(diagnostic: &Diagnostic) -> Self {
        let mut location = DiagnosticLocation::source(diagnostic.source());
        if !diagnostic.coordinates_available() {
            location.line = None;
            location.start_byte = None;
            location.end_byte = None;
        }
        Self::new(
            diagnostic.code(),
            Severity::Error,
            ValidationScope::Document,
            location,
            diagnostic.message(),
        )
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidationResult {
    pub format_version: u8,
    pub project: PathBuf,
    pub target: ValidationTarget,
    /// Complete evaluation with no errors, before selection and pagination.
    pub valid: bool,
    pub evaluation_complete: bool,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub summary: ValidationSummary,
    pub selection: Option<ValidationSelection>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flavours: Option<Option<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations: Option<Option<usize>>,
}

impl ValidationResult {
    /// Finalize the full target before reporting filters or pagination.
    pub fn summarize(&mut self) {
        // Apply the invariant here too, so a producer cannot downgrade a structural failure.
        for diagnostic in &mut self.diagnostics {
            diagnostic.severity = diagnostic.code.severity(diagnostic.severity);
            diagnostic.path = diagnostic.location.path.clone();
            diagnostic.line = diagnostic.location.line;
        }
        self.summary = ValidationSummary {
            errors: self
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count(),
            warnings: self
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Warning)
                .count(),
            counts_exact: self.evaluation_complete,
        };
        self.valid = self.evaluation_complete && self.summary.errors == 0;
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidationSelection {
    pub paths: Vec<PathBuf>,
    /// Produced diagnostics hidden by paths, excluding later pages and unvisited checks.
    pub omitted_diagnostics: usize,
}

pub fn configuration_diagnostic(
    project: &Project,
    scope: ValidationScope,
    path: &Path,
    error: crate::ConfigurationDiagnostic,
) -> ValidationDiagnostic {
    let mut location = DiagnosticLocation::file(project.root(), path);
    location.pointer = error.pointer;
    location.line = error.line;
    location.start_byte = error.start_byte;
    location.end_byte = error.end_byte;
    ValidationDiagnostic::new(error.code, Severity::Error, scope, location, error.message)
}

pub fn matches_handle(item: &crate::Item, handle: &str) -> bool {
    if crate::is_mid(handle) {
        item.mid() == Some(handle)
    } else {
        item.id() == handle
    }
}

fn contains(item: &crate::Item, diagnostic: &Diagnostic) -> bool {
    item.source().path() == diagnostic.source().path()
        && item.source().span().start_byte() <= diagnostic.source().span().start_byte()
        && item.source().span().end_byte() >= diagnostic.source().span().end_byte()
}

pub fn collect_source_diagnostics(
    corpus: &Corpus,
    diagnostics: Vec<Diagnostic>,
    result: &mut ValidationResult,
) {
    // A configured policy uses the entire corpus. When that prerequisite gate
    // skips evaluation, retain the actual blockers even for an item request.
    let corpus_prerequisites = result.diagnostics.iter().any(|d| {
        d.code == DiagnosticCode::EvaluationUnavailable && d.scope == ValidationScope::Project
    });
    let selected = result.target.id.as_deref();
    let missing = selected.is_some_and(|id| {
        corpus.is_complete()
            && !corpus.items().any(|item| matches_handle(item, id))
            && !diagnostics.iter().any(|d| d.applies_to_item(id))
    });
    if let Some(selected) = selected
        && !corpus.is_complete()
    {
        result.diagnostics.push(ValidationDiagnostic::new(
            DiagnosticCode::EvaluationUnavailable,
            Severity::Error,
            ValidationScope::Item,
            DiagnosticLocation::default(),
            format!(
                "item '{}' could not be fully validated because the project corpus is incomplete",
                selected
            ),
        ));
    }
    if missing {
        result.diagnostics.push(ValidationDiagnostic::new(
            DiagnosticCode::ReferenceUnresolved,
            Severity::Error,
            ValidationScope::Item,
            DiagnosticLocation::default(),
            format!("item '{}' was not found", selected.unwrap()),
        ));
    }
    for diagnostic in diagnostics {
        if !corpus_prerequisites
            && selected.is_some_and(|id| {
                !diagnostic.applies_to_item(id)
                    && !corpus
                        .items()
                        .any(|item| matches_handle(item, id) && contains(item, &diagnostic))
            })
        {
            continue;
        }
        let mut entry = ValidationDiagnostic::from_source(&diagnostic);
        let owners = corpus
            .items()
            .filter(|item| contains(item, &diagnostic))
            .collect::<Vec<_>>();
        if let [item] = owners.as_slice() {
            let unique_id = corpus
                .items()
                .filter(|other| other.id() == item.id())
                .count()
                == 1;
            let unique_mid = item.mid().is_some_and(|mid| {
                corpus
                    .items()
                    .filter(|other| {
                        other
                            .metadata()
                            .iter()
                            .any(|e| e.key() == "mid" && e.value() == mid)
                    })
                    .count()
                    == 1
            });
            if unique_id && (unique_mid || item.mid().is_none()) {
                entry.item = Some(DiagnosticItem {
                    id: item.id().to_owned(),
                    mid: item.mid().map(str::to_owned),
                });
            }
        }
        result.diagnostics.push(entry);
    }
}

pub fn sort_diagnostics(diagnostics: &mut [ValidationDiagnostic]) {
    diagnostics.sort_by(|a, b| {
        let key = |d: &ValidationDiagnostic| {
            (
                d.scope,
                d.location.path.clone(),
                d.location.start_byte.or(d.location.line),
                d.item.as_ref().and_then(|i| i.mid.clone()),
                d.rule.clone(),
                d.obligation
                    .as_ref()
                    .map(|o| (o.source.clone(), o.shape.clone(), o.component.clone())),
                d.code,
                d.message.clone(),
            )
        };
        key(a).cmp(&key(b))
    });
}

pub fn paginate(
    mut result: ValidationResult,
    cursor: Option<&str>,
    limit: usize,
    fingerprint: &str,
) -> Result<ValidationResult, ValidationError> {
    let all = std::mem::take(&mut result.diagnostics);
    let stale = || {
        ValidationError::new(
            "stale_cursor",
            "invalid or stale validation cursor; restart without a cursor",
        )
    };
    let start = match cursor {
        None => 0,
        Some(cursor) => {
            let prefix = format!("validation-1-{fingerprint}-");
            let position = cursor.strip_prefix(&prefix).ok_or_else(stale)?;
            if position.len() != 16 || !position.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(stale());
            }
            let position = usize::from_str_radix(position, 16).map_err(|_| stale())?;
            if position == 0 || position >= all.len() {
                return Err(stale());
            }
            position
        }
    };
    let set_cursor = |result: &mut ValidationResult| {
        let next = start + result.diagnostics.len();
        result.has_more = next < all.len();
        result.next_cursor = result
            .has_more
            .then(|| format!("validation-1-{fingerprint}-{next:016x}"));
    };
    for diagnostic in all.iter().skip(start).take(limit) {
        result.diagnostics.push(diagnostic.clone());
        set_cursor(&mut result);
        if serde_json::to_vec(&result)
            .expect("validation serializes")
            .len()
            > 65_536
        {
            result.diagnostics.pop();
            if result.diagnostics.is_empty() {
                return Err(ValidationError::new(
                    "output_limit",
                    format!(
                        "diagnostic at {:?} cannot fit the 65536-byte response budget; shorten the source/configuration value",
                        diagnostic.location.path
                    ),
                ));
            }
            set_cursor(&mut result);
            break;
        }
    }
    if serde_json::to_vec(&result)
        .expect("validation serializes")
        .len()
        > 65_536
    {
        return Err(ValidationError::new(
            "output_limit",
            "validation envelope exceeds 65536 bytes; shorten project/selection paths",
        ));
    }
    Ok(result)
}
