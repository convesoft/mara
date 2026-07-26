use mara_core::{
    Diagnostic, DiagnosticCode, IdentityDiagnosticCode, QueryGraph, SchemaDocument, SeverityCounts,
    ValidationPhase, ValidationPhaseResult, ValidationPhaseState, evaluate_model, sort_diagnostics,
};
use mara_markdown::ParsedDocument;

use std::path::{Path, PathBuf};

use crate::{
    SemanticCompilation, compile_documents,
    content::discover_content,
    project::{LoadedProject, ProjectLoadError, discover_and_load},
    schema::load_schema_with_source,
};

/// Immutable evidence and diagnostics produced from already parsed inputs.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    project: Option<LoadedProject>,
    schema: Option<SchemaDocument>,
    schema_source: Option<Vec<u8>>,
    documents: Vec<ParsedDocument>,
    content_paths: Vec<PathBuf>,
    semantic: Option<SemanticCompilation>,
    graph: Option<QueryGraph>,
    phases: Vec<ValidationPhaseResult>,
    diagnostics: Vec<Diagnostic>,
    severity_counts: SeverityCounts,
    warnings_as_errors: bool,
}

impl ValidationResult {
    pub const fn project(&self) -> Option<&LoadedProject> {
        self.project.as_ref()
    }

    pub const fn schema(&self) -> Option<&SchemaDocument> {
        self.schema.as_ref()
    }

    pub(crate) fn schema_source(&self) -> Option<&[u8]> {
        self.schema_source.as_deref()
    }

    pub fn documents(&self) -> &[ParsedDocument] {
        &self.documents
    }

    pub(crate) fn content_paths(&self) -> &[PathBuf] {
        &self.content_paths
    }

    pub const fn semantic(&self) -> Option<&SemanticCompilation> {
        self.semantic.as_ref()
    }

    pub const fn graph(&self) -> Option<&QueryGraph> {
        self.graph.as_ref()
    }

    pub fn phases(&self) -> &[ValidationPhaseResult] {
        &self.phases
    }

    pub fn phase(&self, phase: ValidationPhase) -> Option<&ValidationPhaseState> {
        self.phases
            .iter()
            .find(|result| result.phase() == phase)
            .map(ValidationPhaseResult::state)
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub const fn severity_counts(&self) -> SeverityCounts {
        self.severity_counts
    }

    pub const fn warnings_as_errors(&self) -> bool {
        self.warnings_as_errors
    }

    pub const fn is_valid(&self) -> bool {
        self.severity_counts.is_valid(self.warnings_as_errors)
    }
}

/// Validates compiled schema and parser values without performing I/O.
pub fn validate_documents(
    schema: &SchemaDocument,
    documents: &[ParsedDocument],
    warnings_as_errors: bool,
) -> ValidationResult {
    let semantic = compile_documents(schema, documents);
    let graph_ambiguous = semantic.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::Identity(IdentityDiagnosticCode::DuplicateMid)
    });
    let graph = (!graph_ambiguous).then(|| {
        QueryGraph::from_canonical(
            semantic.items().iter().map(mara_core::NormalizedItem::mid),
            semantic.relations().edges(),
            semantic.projection_edges().iter().cloned(),
        )
    });
    let mut diagnostics = semantic.diagnostics().to_vec();
    diagnostics.extend(evaluate_model(
        schema,
        semantic.items(),
        graph.as_ref(),
        semantic.diagnostics(),
    ));
    sort_diagnostics(&mut diagnostics);
    let severity_counts = SeverityCounts::from_diagnostics(&diagnostics);
    let phases = vec![
        ValidationPhaseResult::new(
            ValidationPhase::Project,
            ValidationPhaseState::skipped("project input was supplied in memory", None),
        ),
        ValidationPhaseResult::new(ValidationPhase::Schema, ValidationPhaseState::Completed),
        ValidationPhaseResult::new(
            ValidationPhase::Content,
            ValidationPhaseState::skipped("documents were supplied in memory", None),
        ),
        ValidationPhaseResult::new(ValidationPhase::Parse, ValidationPhaseState::Completed),
        ValidationPhaseResult::new(ValidationPhase::Semantic, ValidationPhaseState::Completed),
        ValidationPhaseResult::new(
            ValidationPhase::Graph,
            if graph_ambiguous {
                ValidationPhaseState::skipped(
                    "machine identity is globally ambiguous",
                    Some(ValidationPhase::Semantic),
                )
            } else {
                ValidationPhaseState::Completed
            },
        ),
        ValidationPhaseResult::new(ValidationPhase::Rules, ValidationPhaseState::Completed),
    ];

    ValidationResult {
        project: None,
        schema: Some(schema.clone()),
        schema_source: None,
        documents: documents.to_vec(),
        content_paths: Vec::new(),
        semantic: Some(semantic),
        graph,
        phases,
        diagnostics,
        severity_counts,
        warnings_as_errors,
    }
}

/// Runs the complete read-only project validation pipeline from a filesystem start path.
pub fn check_project(start: impl AsRef<Path>) -> Result<ValidationResult, ProjectLoadError> {
    let project = discover_and_load(start)?;
    let content = discover_content(&project);
    match load_schema_with_source(&project) {
        Ok((schema, schema_source)) => {
            let documents = content
                .documents()
                .iter()
                .cloned()
                .map(|source| {
                    mara_markdown::parse_document(source, schema.identity().value().mid().value())
                })
                .collect::<Vec<_>>();
            let mut result =
                validate_documents(&schema, &documents, project.validation.warnings_as_errors);
            result.project = Some(project);
            result.schema_source = Some(schema_source);
            result.content_paths = content.resolved_paths().to_vec();
            result.phases[0] = ValidationPhaseResult::new(
                ValidationPhase::Project,
                ValidationPhaseState::Completed,
            );
            result.phases[2] = ValidationPhaseResult::new(
                ValidationPhase::Content,
                ValidationPhaseState::Completed,
            );
            result
                .diagnostics
                .extend(content.diagnostics().iter().cloned());
            finish_diagnostics(&mut result);
            Ok(result)
        }
        Err(error) => {
            let mut diagnostics = error.diagnostics().to_vec();
            diagnostics.extend(content.diagnostics().iter().cloned());
            let warnings_as_errors = project.validation.warnings_as_errors;
            Ok(skipped_after_schema_failure(
                project,
                diagnostics,
                warnings_as_errors,
                true,
            ))
        }
    }
}

/// Loads and checks only project configuration and schema, without content discovery or parsing.
pub fn check_schema(start: impl AsRef<Path>) -> Result<ValidationResult, ProjectLoadError> {
    let project = discover_and_load(start)?;
    let warnings_as_errors = project.validation.warnings_as_errors;
    match load_schema_with_source(&project) {
        Ok((schema, schema_source)) => {
            let diagnostics = Vec::new();
            Ok(ValidationResult {
                project: Some(project),
                schema: Some(schema),
                schema_source: Some(schema_source),
                documents: Vec::new(),
                content_paths: Vec::new(),
                semantic: None,
                graph: None,
                phases: schema_only_phases(),
                severity_counts: SeverityCounts::from_diagnostics(&diagnostics),
                diagnostics,
                warnings_as_errors,
            })
        }
        Err(error) => Ok(skipped_after_schema_failure(
            project,
            error.diagnostics().to_vec(),
            warnings_as_errors,
            false,
        )),
    }
}

fn skipped_after_schema_failure(
    project: LoadedProject,
    mut diagnostics: Vec<Diagnostic>,
    warnings_as_errors: bool,
    content_completed: bool,
) -> ValidationResult {
    sort_diagnostics(&mut diagnostics);
    let severity_counts = SeverityCounts::from_diagnostics(&diagnostics);
    ValidationResult {
        project: Some(project),
        schema: None,
        schema_source: None,
        documents: Vec::new(),
        content_paths: Vec::new(),
        semantic: None,
        graph: None,
        phases: vec![
            ValidationPhaseResult::new(ValidationPhase::Project, ValidationPhaseState::Completed),
            ValidationPhaseResult::new(ValidationPhase::Schema, ValidationPhaseState::Completed),
            ValidationPhaseResult::new(
                ValidationPhase::Content,
                if content_completed {
                    ValidationPhaseState::Completed
                } else {
                    ValidationPhaseState::skipped("schema-only validation", None)
                },
            ),
            ValidationPhaseResult::new(
                ValidationPhase::Parse,
                if content_completed {
                    ValidationPhaseState::skipped(
                        "schema identity is unavailable",
                        Some(ValidationPhase::Schema),
                    )
                } else {
                    ValidationPhaseState::skipped("schema-only validation", None)
                },
            ),
            ValidationPhaseResult::new(
                ValidationPhase::Semantic,
                ValidationPhaseState::skipped(
                    "compiled schema is unavailable",
                    Some(ValidationPhase::Schema),
                ),
            ),
            ValidationPhaseResult::new(
                ValidationPhase::Graph,
                ValidationPhaseState::skipped(
                    "semantic model is unavailable",
                    Some(ValidationPhase::Semantic),
                ),
            ),
            ValidationPhaseResult::new(
                ValidationPhase::Rules,
                ValidationPhaseState::skipped(
                    "compiled schema is unavailable",
                    Some(ValidationPhase::Schema),
                ),
            ),
        ],
        diagnostics,
        severity_counts,
        warnings_as_errors,
    }
}

fn schema_only_phases() -> Vec<ValidationPhaseResult> {
    ValidationPhase::ALL
        .into_iter()
        .map(|phase| {
            let state = match phase {
                ValidationPhase::Project | ValidationPhase::Schema => {
                    ValidationPhaseState::Completed
                }
                _ => ValidationPhaseState::skipped("schema-only validation", None),
            };
            ValidationPhaseResult::new(phase, state)
        })
        .collect()
}

fn finish_diagnostics(result: &mut ValidationResult) {
    sort_diagnostics(&mut result.diagnostics);
    result.severity_counts = SeverityCounts::from_diagnostics(&result.diagnostics);
}
