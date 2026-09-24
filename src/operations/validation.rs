use super::*;
use crate::{
    DiagnosticCode, DiagnosticItem, DiagnosticLocation, DiagnosticObligation, Severity,
    ValidationError, ValidationOptions, ValidationSummary,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ValidationTargetKind {
    Project,
    Item,
    Schema,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
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
    pub(crate) fn from_source(diagnostic: &Diagnostic) -> Self {
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

impl OperationContext {
    pub fn project_validate(&self, paths: &[PathBuf]) -> Result<ValidationResult, String> {
        self.project_validate_with_options(paths, &ValidationOptions::default())
            .map_err(|e| e.to_string())
    }
    pub fn item_validate(&self, id: &str) -> Result<ValidationResult, String> {
        self.item_validate_with_options(id, &ValidationOptions::default())
            .map_err(|e| e.to_string())
    }
    pub fn schema_validate(&self) -> Result<SchemaValidationResult, String> {
        self.schema_validate_with_options(&ValidationOptions::default())
            .map_err(|e| e.to_string())
    }
    pub fn project_validate_with_options(
        &self,
        paths: &[PathBuf],
        options: &ValidationOptions,
    ) -> Result<ValidationResult, ValidationError> {
        let paths = crate::query::normalized_paths(paths)
            .map_err(|e| ValidationError::invalid_argument(e.to_string()))?;
        self.validate_target(
            ValidationTarget {
                kind: ValidationTargetKind::Project,
                id: None,
            },
            paths,
            options,
        )
    }
    pub fn item_validate_with_options(
        &self,
        id: &str,
        options: &ValidationOptions,
    ) -> Result<ValidationResult, ValidationError> {
        if !crate::is_item_id(id) && !crate::is_mid(id) {
            return Err(ValidationError::invalid_argument(
                "item must be an exact human ID or canonical MID",
            ));
        }
        self.validate_target(
            ValidationTarget {
                kind: ValidationTargetKind::Item,
                id: Some(id.into()),
            },
            vec![],
            options,
        )
    }
    pub fn schema_validate_with_options(
        &self,
        options: &ValidationOptions,
    ) -> Result<ValidationResult, ValidationError> {
        self.validate_target(
            ValidationTarget {
                kind: ValidationTargetKind::Schema,
                id: None,
            },
            vec![],
            options,
        )
    }

    fn validate_target(
        &self,
        target: ValidationTarget,
        paths: Vec<PathBuf>,
        options: &ValidationOptions,
    ) -> Result<ValidationResult, ValidationError> {
        let limit = options.limit.unwrap_or(20);
        if !(1..=100).contains(&limit) {
            return Err(ValidationError::invalid_argument(
                "limit must be 1 through 100",
            ));
        }
        if options.cursor.as_deref() == Some("") {
            return Err(ValidationError::invalid_argument(
                "cursor must not be empty",
            ));
        }
        let (project, project_errors, schema_available) =
            match resolve_project_for_validation(self.selected.as_deref(), &self.current_directory)
            {
                Ok(value) => value.into_parts(),
                Err(error) => return Err(operation_error(error)),
            };
        let schema_only = target.kind == ValidationTargetKind::Schema;
        let mut result = ValidationResult {
            format_version: 1,
            project: project.root().to_owned(),
            target,
            valid: false,
            evaluation_complete: true,
            diagnostics: vec![],
            summary: ValidationSummary {
                errors: 0,
                warnings: 0,
                counts_exact: true,
            },
            selection: None,
            has_more: false,
            next_cursor: None,
            path: schema_only.then(|| project.schema_path().to_owned()),
            flavours: schema_only.then_some(None),
            relations: schema_only.then_some(None),
        };
        for error in project_errors {
            result.diagnostics.push(configuration_diagnostic(
                &project,
                ValidationScope::Project,
                &project.root().join(crate::PROJECT_FILE),
                error,
            ));
        }
        let schema = if schema_available {
            match crate::load_schema_for_validation(&project) {
                Ok((schema, errors)) => {
                    for error in errors {
                        result.diagnostics.push(configuration_diagnostic(
                            &project,
                            ValidationScope::Schema,
                            project.schema_path(),
                            error,
                        ));
                    }
                    if schema.format_version() == crate::SCHEMA_FORMAT_VERSION {
                        Some(schema)
                    } else {
                        None
                    }
                }
                Err(error) => return Err(operation_error(error)),
            }
        } else {
            None
        };
        result.evaluation_complete = result.diagnostics.is_empty() && schema.is_some();
        let mut snapshot = Sha256::new();
        snapshot.update(b"validation-1-complete-evaluation-1");
        hash_file(&mut snapshot, &project.root().join(crate::PROJECT_FILE));
        hash_file(&mut snapshot, project.schema_path());
        snapshot.update(b"yaml-shacl-binding-1-registry-0.3.21-adapter-9");
        let rules = schema
            .as_ref()
            .map(|schema| crate::rules::Rules::load(&project, schema));
        if let Some(rules) = &rules {
            for path in &rules.files {
                hash_file(&mut snapshot, path);
            }
            if !rules.diagnostics.is_empty() {
                result.evaluation_complete = false;
            }
            result.diagnostics.extend(rules.diagnostics.clone());
        }
        if let Some(schema) = &schema
            && schema_only
        {
            result.flavours = Some(Some(schema.flavours().len()));
            result.relations = Some(Some(schema.relations().len()));
        }
        if !schema_only {
            let (corpus, mut source_diagnostics) = match &schema {
                Some(schema) => load_corpus_for_validation(&project, schema),
                None => load_corpus_syntax_for_validation(&project),
            }
            .map_err(operation_error)?;
            let mut source_paths = BTreeSet::new();
            for document in corpus.documents() {
                source_paths.insert(document.path().to_owned());
            }
            for diagnostic in &source_diagnostics {
                source_paths.insert(diagnostic.source().path().to_owned());
            }
            for path in source_paths {
                hash_file(&mut snapshot, &project.root().join(path));
            }
            result.evaluation_complete &= corpus.is_complete();
            result.evaluation_complete &= corpus
                .items()
                .filter(|item| {
                    result
                        .target
                        .id
                        .as_deref()
                        .is_none_or(|id| matches_handle(item, id))
                })
                .all(crate::Item::validation_source_is_complete);
            source_diagnostics.extend(match &schema {
                Some(schema) => crate::corpus::validate_corpus(&corpus, schema),
                None => crate::corpus::validate_corpus_independent(&corpus),
            });
            if let (Some(rules), Some(schema)) = (&rules, &schema)
                && rules.diagnostics.is_empty()
                && !result
                    .diagnostics
                    .iter()
                    .any(|d| matches!(d.scope, ValidationScope::Project | ValidationScope::Schema))
            {
                rules.evaluate(&corpus, schema, &source_diagnostics, &mut result);
            }
            collect_source_diagnostics(&corpus, source_diagnostics, &mut result);
        }
        result.summarize();
        sort_diagnostics(&mut result.diagnostics);
        // Include diagnostics for unreadable sources and options as well as source bytes.
        snapshot
            .update(serde_json::to_vec(&(&result, &paths, limit)).expect("validation serializes"));
        let fingerprint = format!("{:x}", snapshot.finalize());
        if !paths.is_empty() {
            let total = result.diagnostics.len();
            result.diagnostics.retain(|diagnostic| {
                matches!(
                    diagnostic.scope,
                    ValidationScope::Project | ValidationScope::Schema
                ) || diagnostic
                    .path
                    .as_ref()
                    .is_none_or(|source| paths.iter().any(|path| source.starts_with(path)))
            });
            result.selection = Some(ValidationSelection {
                paths,
                omitted_diagnostics: total - result.diagnostics.len(),
            });
        }
        paginate(result, options.cursor.as_deref(), limit, &fingerprint)
    }
}

fn operation_error(error: crate::Error) -> ValidationError {
    let code = if matches!(error, crate::Error::Io { .. }) {
        "io_error"
    } else {
        "invalid_argument"
    };
    ValidationError::new(code, error.to_string())
}

fn configuration_diagnostic(
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

fn hash_file(hash: &mut Sha256, path: &Path) {
    let identity = path.to_string_lossy();
    hash.update(identity.len().to_le_bytes());
    hash.update(identity.as_bytes());
    match fs::read(path) {
        Ok(bytes) => {
            hash.update([1]);
            hash.update(bytes.len().to_le_bytes());
            hash.update(bytes);
        }
        Err(error) => {
            hash.update([0]);
            hash.update(format!("{:?}", error.kind()).as_bytes());
        }
    }
}

fn matches_handle(item: &crate::Item, handle: &str) -> bool {
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

fn collect_source_diagnostics(
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

fn sort_diagnostics(diagnostics: &mut [ValidationDiagnostic]) {
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

fn paginate(
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
