//! Query-backed documentation invariants and request-local contextual gates.
//! Both consumers run named definitions and validate only their projected facts.
use crate::{
    DiagnosticCode, DiagnosticLocation, QueryPolicyConfig, Schema, Severity, ValidationDiagnostic,
    ValidationResult, ValidationScope, ValidationTarget, ValidationTargetKind, named_query,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConsumerState {
    Passed,
    Failed,
    NotApplicable,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EmptyOutcome {
    Passed,
    Failed,
    NotApplicable,
}
impl From<EmptyOutcome> for ConsumerState {
    fn from(value: EmptyOutcome) -> Self {
        match value {
            EmptyOutcome::Passed => Self::Passed,
            EmptyOutcome::Failed => Self::Failed,
            EmptyOutcome::NotApplicable => Self::NotApplicable,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryPolicyValidation {
    pub format_version: u32,
    pub input_identity: String,
    pub evaluation_complete: bool,
    pub checks: Vec<QueryPolicyCheck>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryPolicyCheck {
    pub rule: String,
    pub definition: String,
    pub shape: String,
    pub root: Option<named_query::Endpoint>,
    pub state: ConsumerState,
    pub applicability: ConsumerState,
    pub severity: Severity,
    pub binding_count: Option<usize>,
    pub components: Vec<String>,
    /// Typed returned facts and explicitly projected source support explain the result.
    pub bindings: Vec<named_query::Binding>,
    pub source: DiagnosticLocation,
}

fn policy_input_identity(
    engine: &crate::SourceEngine,
    target: &ValidationTarget,
    fact_snapshots: &[named_query::FactSnapshot],
    mappings: &[named_query::TypedMapping],
) -> String {
    let bytes = serde_json::to_vec(&(
        "query-policy-input-v2", // Includes the revised named-query stream ordering.
        engine.query_fingerprint(),
        target,
        fact_snapshots,
        mappings,
    ))
    .unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryGateRequest {
    /// Explicit project-relative named-query definition files using format_version 1.
    pub definition_files: Vec<String>,
    /// Request-local YAML gate definitions; not enabled project policy.
    pub gate_file: String,
    /// Explicit Turtle SHACL source files used by the selected gate.
    pub shape_files: Vec<String>,
    /// Exact gate identity from the request-local YAML gate file.
    pub gate: String,
    /// Explicit root workload. Omitted scope is never interpreted as all roots.
    pub scope: named_query::Selection,
    /// Typed context bindings are passed only to conditions that name them.
    #[serde(default)]
    pub context: Vec<named_query::BindingParameter>,
    /// Host-supplied typed fact snapshots passed unchanged to named conditions.
    #[serde(default)]
    pub fact_snapshots: Vec<named_query::FactSnapshot>,
    /// Caller/project lookup mappings passed unchanged to named conditions.
    #[serde(default)]
    pub mappings: Vec<named_query::TypedMapping>,
    /// Observed solution-row budget for every named condition query.
    pub max_rows: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryGateResult {
    pub format_version: u32,
    pub gate: String,
    pub source: DiagnosticLocation,
    pub scope: named_query::Selection,
    pub context: Vec<named_query::BindingParameter>,
    pub input_identity: String,
    pub evaluation_complete: bool,
    pub state: ConsumerState,
    pub empty_reason: Option<String>,
    pub unavailable_reason: Option<String>,
    pub roots: Vec<QueryGateRoot>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryGateRoot {
    pub root: named_query::Endpoint,
    pub state: ConsumerState,
    pub conditions: Vec<QueryGateCondition>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryGateCondition {
    pub id: String,
    pub source: DiagnosticLocation,
    pub definition: String,
    pub shape_file: String,
    pub shape: String,
    pub severity: Severity,
    pub state: ConsumerState,
    pub applicability: ConsumerState,
    pub binding_count: Option<usize>,
    pub components: Vec<String>,
    pub bindings: Vec<named_query::Binding>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvariantFile {
    format_version: u32,
    invariants: Vec<Invariant>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Invariant {
    id: String,
    definition: String,
    shape: String,
    flavours: Vec<String>,
    #[serde(default)]
    paths: Vec<String>,
    severity: Severity,
    #[serde(default)]
    parameters: Vec<named_query::BindingParameter>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct GateFile {
    format_version: u32,
    gates: Vec<GateDefinition>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct GateDefinition {
    id: String,
    empty_roots: EmptyOutcome,
    empty_required: EmptyOutcome,
    conditions: Vec<GateConditionDefinition>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct GateConditionDefinition {
    id: String,
    definition: String,
    shape_file: String,
    shape: String,
    severity: Severity,
    #[serde(default)]
    context_parameters: Vec<String>,
    #[serde(default)]
    applies_when: Option<ApplicabilityPredicate>,
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ApplicabilityPredicate {
    BindingsPresent,
    BindingsAbsent,
}

#[derive(Debug, Clone)]
pub struct QueryConsumerError {
    pub code: String,
    pub message: String,
}
impl QueryConsumerError {
    fn argument(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_argument".into(),
            message: message.into(),
        }
    }
    fn definition(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_definition".into(),
            message: message.into(),
        }
    }
}
impl std::fmt::Display for QueryConsumerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for QueryConsumerError {}

impl crate::SourceEngine {
    /// Evaluate enabled query-backed invariants using the policy's declared default inputs.
    pub fn query_policy_validation(
        &self,
        target: ValidationTarget,
    ) -> (Option<QueryPolicyValidation>, Vec<ValidationDiagnostic>) {
        self.query_policy_validation_with_inputs(target, &[], &[])
            .unwrap_or_else(|error| {
                policy_unavailable(None, "", error.to_string(), ValidationScope::Project)
            })
    }

    /// Evaluate enabled query-backed invariants from explicit host-supplied facts and mappings.
    pub fn query_policy_validation_with_inputs(
        &self,
        target: ValidationTarget,
        fact_snapshots: &[named_query::FactSnapshot],
        mappings: &[named_query::TypedMapping],
    ) -> Result<(Option<QueryPolicyValidation>, Vec<ValidationDiagnostic>), QueryConsumerError>
    {
        if target.kind == ValidationTargetKind::Schema
            && (!fact_snapshots.is_empty() || !mappings.is_empty())
        {
            return Err(QueryConsumerError::argument(
                "query facts and mappings apply only to project or item policy evaluation",
            ));
        }
        let (project, schema) = match self.strict_project() {
            Ok(value) => value,
            Err(error) => {
                return Ok(policy_unavailable(
                    None,
                    "",
                    format!("project inputs unavailable: {error}"),
                    ValidationScope::Project,
                ));
            }
        };
        let Some(config) = project.query_policy() else {
            if !fact_snapshots.is_empty() || !mappings.is_empty() {
                return Err(QueryConsumerError::argument(
                    "query facts and mappings require an enabled query policy",
                ));
            }
            return Ok((None, Vec::new()));
        };
        let mut result = self.evaluate_query_policies(
            &schema,
            config,
            target.clone(),
            fact_snapshots,
            mappings,
        )?;
        if let Some(report) = &mut result.0 {
            report.input_identity = policy_input_identity(self, &target, fact_snapshots, mappings);
        }
        Ok(result)
    }

    fn evaluate_query_policies(
        &self,
        schema: &Schema,
        config: &QueryPolicyConfig,
        target: ValidationTarget,
        fact_snapshots: &[named_query::FactSnapshot],
        mappings: &[named_query::TypedMapping],
    ) -> Result<(Option<QueryPolicyValidation>, Vec<ValidationDiagnostic>), QueryConsumerError>
    {
        let mut diagnostics = Vec::new();
        let invariants = match load_invariants(self, config) {
            Ok(invariants) => invariants,
            Err((path, message)) => {
                let (report, errors) = policy_unavailable(
                    Some(QueryPolicyValidation {
                        format_version: 1,
                        input_identity: self.source_fingerprint().into(),
                        evaluation_complete: false,
                        checks: vec![],
                    }),
                    &path,
                    message,
                    ValidationScope::Schema,
                );
                diagnostics.extend(errors);
                return Ok((report, diagnostics));
            }
        };
        let definitions = match self.validated_query_definitions(&config.definition_files) {
            Ok(value) => value,
            Err(error) => {
                let (report, mut errors) = policy_unavailable(
                    Some(QueryPolicyValidation {
                        format_version: 1,
                        input_identity: self.source_fingerprint().into(),
                        evaluation_complete: false,
                        checks: vec![],
                    }),
                    error.source.as_deref().unwrap_or(""),
                    error.message,
                    ValidationScope::Schema,
                );
                diagnostics.append(&mut errors);
                return Ok((report, diagnostics));
            }
        };
        if target.kind != ValidationTargetKind::Schema
            && let Err(error) =
                self.preflight_query_inputs(&config.definition_files, fact_snapshots, mappings)
        {
            if error.code == "invalid_argument" {
                return Err(QueryConsumerError {
                    code: error.code,
                    message: error.message,
                });
            }
            if error.code == "invalid_definition" {
                diagnostics.push(invalid_policy(
                    error
                        .source
                        .as_deref()
                        .map(PathBuf::from)
                        .map(|path| DiagnosticLocation {
                            path: Some(path),
                            ..Default::default()
                        })
                        .unwrap_or_default(),
                    "query-policy",
                    error.message,
                ));
                return Ok((
                    Some(QueryPolicyValidation {
                        format_version: 1,
                        input_identity: self.source_fingerprint().into(),
                        evaluation_complete: false,
                        checks: vec![],
                    }),
                    diagnostics,
                ));
            }
            let (report, mut errors) = policy_unavailable(
                Some(QueryPolicyValidation {
                    format_version: 1,
                    input_identity: self.source_fingerprint().into(),
                    evaluation_complete: false,
                    checks: vec![],
                }),
                error.source.as_deref().unwrap_or(""),
                error.message,
                validation_scope(target.kind),
            );
            diagnostics.append(&mut errors);
            return Ok((report, diagnostics));
        }
        let shape_paths = config
            .shape_files
            .iter()
            .map(|path| {
                normalized_policy_path(path)
                    .map_err(|message| (path.to_string_lossy().into_owned(), message))
            })
            .collect::<Result<Vec<_>, _>>();
        let shape_paths = match shape_paths {
            Ok(paths) => paths,
            Err((path, message)) => {
                let (report, mut errors) = policy_unavailable(
                    Some(QueryPolicyValidation {
                        format_version: 1,
                        input_identity: self.source_fingerprint().into(),
                        evaluation_complete: false,
                        checks: vec![],
                    }),
                    &path,
                    message,
                    ValidationScope::Schema,
                );
                diagnostics.append(&mut errors);
                return Ok((report, diagnostics));
            }
        };
        let ir = match crate::query_backend::compile_shape_files(self.files(), &shape_paths) {
            Ok(ir) => ir,
            Err(message) => {
                let (report, mut errors) = policy_unavailable(
                    Some(QueryPolicyValidation {
                        format_version: 1,
                        input_identity: self.source_fingerprint().into(),
                        evaluation_complete: false,
                        checks: vec![],
                    }),
                    config
                        .files
                        .first()
                        .map_or("", |p| p.to_str().unwrap_or("")),
                    message,
                    ValidationScope::Schema,
                );
                diagnostics.append(&mut errors);
                return Ok((report, diagnostics));
            }
        };
        if target.kind == ValidationTargetKind::Schema {
            let mut checks = Vec::new();
            for (invariant, source) in &invariants {
                let Some(definition) = definitions.get(&invariant.definition) else {
                    diagnostics.push(invalid_policy(
                        source.clone(),
                        &invariant.id,
                        format!("unknown named query definition '{}'", invariant.definition),
                    ));
                    continue;
                };
                if invariant.flavours.is_empty()
                    || invariant
                        .flavours
                        .iter()
                        .any(|flavour| !schema.flavours().contains_key(flavour))
                    || invariant
                        .flavours
                        .iter()
                        .any(|flavour| !definition.root_flavours.contains(flavour))
                {
                    diagnostics.push(invalid_policy(source.clone(), &invariant.id, "invariant flavours must be declared schema flavours within the query root signature"));
                    continue;
                }
                if let Err(message) =
                    validate_invariant_parameters(definition, &invariant.parameters)
                {
                    diagnostics.push(invalid_policy(source.clone(), &invariant.id, message));
                    continue;
                }
                if crate::query::normalized_paths(
                    &invariant.paths.iter().map(Into::into).collect::<Vec<_>>(),
                )
                .is_err()
                {
                    diagnostics.push(invalid_policy(
                        source.clone(),
                        &invariant.id,
                        "invariant paths are invalid project-relative paths",
                    ));
                    continue;
                }
                if !shape_exists(&ir, &invariant.shape) {
                    diagnostics.push(invalid_policy(
                        source.clone(),
                        &invariant.id,
                        format!("unknown SHACL shape '{}'", invariant.shape),
                    ));
                    continue;
                }
                checks.push(QueryPolicyCheck {
                    rule: invariant.id.clone(),
                    definition: invariant.definition.clone(),
                    shape: invariant.shape.clone(),
                    root: None,
                    state: ConsumerState::NotApplicable,
                    applicability: ConsumerState::NotApplicable,
                    severity: invariant.severity,
                    binding_count: None,
                    components: vec![],
                    bindings: vec![],
                    source: source.clone(),
                });
            }
            return Ok((
                Some(QueryPolicyValidation {
                    format_version: 1,
                    input_identity: self.source_fingerprint().into(),
                    evaluation_complete: diagnostics.is_empty(),
                    checks,
                }),
                diagnostics,
            ));
        }
        let corpus = match self.corpus() {
            Ok((_, _, corpus)) => corpus,
            Err(error) => {
                let (report, mut errors) = policy_unavailable(
                    Some(QueryPolicyValidation {
                        format_version: 1,
                        input_identity: self.source_fingerprint().into(),
                        evaluation_complete: false,
                        checks: vec![],
                    }),
                    "",
                    format!("query policy source unavailable: {error}"),
                    ValidationScope::Project,
                );
                diagnostics.append(&mut errors);
                return Ok((report, diagnostics));
            }
        };
        let source_diagnostics = crate::validate_corpus(&corpus, schema);
        let mut report = QueryPolicyValidation {
            format_version: 1,
            input_identity: self.source_fingerprint().into(),
            evaluation_complete: true,
            checks: vec![],
        };
        if !source_diagnostics.is_empty() || !corpus.is_complete() {
            report.evaluation_complete = false;
            let (next, mut errors) = policy_unavailable(
                Some(report),
                "",
                "query policy evaluation unavailable until source conformance is complete".into(),
                ValidationScope::Project,
            );
            diagnostics.append(&mut errors);
            return Ok((next, diagnostics));
        }

        for (invariant, source) in invariants {
            let Some(definition) = definitions.get(&invariant.definition) else {
                report.evaluation_complete = false;
                diagnostics.push(invalid_policy(
                    source.clone(),
                    &invariant.id,
                    format!("unknown named query definition '{}'", invariant.definition),
                ));
                continue;
            };
            if invariant.flavours.is_empty()
                || invariant
                    .flavours
                    .iter()
                    .any(|f| !schema.flavours().contains_key(f))
                || invariant
                    .flavours
                    .iter()
                    .any(|f| !definition.root_flavours.iter().any(|root| root == f))
            {
                report.evaluation_complete = false;
                diagnostics.push(invalid_policy(source.clone(), &invariant.id, "invariant flavours must be declared schema flavours within the query root signature"));
                continue;
            }
            if let Err(message) = validate_invariant_parameters(definition, &invariant.parameters) {
                report.evaluation_complete = false;
                diagnostics.push(invalid_policy(source.clone(), &invariant.id, message));
                continue;
            }
            if !shape_exists(&ir, &invariant.shape) {
                report.evaluation_complete = false;
                diagnostics.push(invalid_policy(
                    source.clone(),
                    &invariant.id,
                    format!("unknown SHACL shape '{}'", invariant.shape),
                ));
                continue;
            }
            let paths = match crate::query::normalized_paths(
                &invariant.paths.iter().map(Into::into).collect::<Vec<_>>(),
            ) {
                Ok(paths) => paths,
                Err(error) => {
                    report.evaluation_complete = false;
                    diagnostics.push(invalid_policy(
                        source.clone(),
                        &invariant.id,
                        error.to_string(),
                    ));
                    continue;
                }
            };
            let selected = if target.kind == ValidationTargetKind::Item {
                let item = target.id.as_deref().and_then(|id| {
                    corpus
                        .items()
                        .find(|item| crate::validation::matches_handle(item, id))
                });
                match item {
                    Some(item)
                        if invariant.flavours.iter().any(|f| f == item.flavour())
                            && path_applies(item.source().path(), &paths) =>
                    {
                        vec![item.id().to_owned()]
                    }
                    Some(item) => {
                        report.checks.push(QueryPolicyCheck {
                            rule: invariant.id.clone(),
                            definition: invariant.definition.clone(),
                            shape: invariant.shape.clone(),
                            root: Some(named_query::Endpoint::item(item)),
                            state: ConsumerState::NotApplicable,
                            applicability: ConsumerState::NotApplicable,
                            severity: invariant.severity,
                            binding_count: None,
                            components: vec![],
                            bindings: vec![],
                            source,
                        });
                        continue;
                    }
                    None => {
                        report.evaluation_complete = false;
                        let (next, mut errors) = policy_unavailable(
                            Some(report),
                            &source
                                .path
                                .as_ref()
                                .map_or(String::new(), |p| p.to_string_lossy().into_owned()),
                            "selected item is unavailable to query policy evaluation".into(),
                            ValidationScope::Item,
                        );
                        diagnostics.append(&mut errors);
                        report = next.unwrap();
                        continue;
                    }
                }
            } else {
                vec![]
            };
            let selection = if target.kind == ValidationTargetKind::Item {
                named_query::Selection {
                    ids: selected,
                    ..Default::default()
                }
            } else {
                named_query::Selection {
                    flavours: invariant.flavours.clone(),
                    paths: paths
                        .iter()
                        .map(|p| p.to_string_lossy().into_owned())
                        .collect(),
                    ..Default::default()
                }
            };
            let request = named_query::Request {
                definition_files: config.definition_files.clone(),
                definition: invariant.definition.clone(),
                selection,
                parameters: invariant.parameters.clone(),
                max_rows: config.max_rows,
                limit: 100,
                cursor: None,
                fact_snapshots: fact_snapshots.to_vec(),
                mappings: mappings.to_vec(),
            };
            let result = match collect_named_query(self, request) {
                Ok(result) => result,
                Err(error) => {
                    report.evaluation_complete = false;
                    let diagnostic = if error.code == "input_unavailable" {
                        unavailable_policy(
                            source.clone(),
                            &invariant.id,
                            error.to_string(),
                            validation_scope(target.kind),
                        )
                    } else {
                        invalid_policy(source.clone(), &invariant.id, error.to_string())
                    };
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            if !result.evaluation_complete {
                report.evaluation_complete = false;
            }
            let roots = root_bindings(&result.records);
            let has_roots = !roots.is_empty();
            for (root, source_item, binding_count, bindings) in roots {
                let (state, components) = if !result.evaluation_complete || binding_count.is_none()
                {
                    report.evaluation_complete = false;
                    (ConsumerState::Unavailable, vec![])
                } else {
                    let rows = bindings.iter().map(binding_json_ld).collect::<Vec<_>>();
                    let focus = root.iri();
                    match crate::query_backend::evaluate_projection_shape(
                        &ir,
                        &invariant.shape,
                        &focus,
                        &rows,
                    ) {
                        Ok(outcome) if outcome.conforms => {
                            (ConsumerState::Passed, outcome.components)
                        }
                        Ok(outcome) => (ConsumerState::Failed, outcome.components),
                        Err(message) => {
                            report.evaluation_complete = false;
                            diagnostics.push(unavailable_policy(
                                source.clone(),
                                &invariant.id,
                                message,
                                ValidationScope::Item,
                            ));
                            (ConsumerState::Unavailable, vec![])
                        }
                    }
                };
                let check = QueryPolicyCheck {
                    rule: invariant.id.clone(),
                    definition: invariant.definition.clone(),
                    shape: invariant.shape.clone(),
                    root: Some(root.clone()),
                    applicability: if state == ConsumerState::NotApplicable {
                        ConsumerState::NotApplicable
                    } else if state == ConsumerState::Unavailable {
                        ConsumerState::Unavailable
                    } else {
                        ConsumerState::Passed
                    },
                    state,
                    severity: invariant.severity,
                    binding_count,
                    components: components.clone(),
                    bindings: bindings.clone(),
                    source: source.clone(),
                };
                if state == ConsumerState::Failed {
                    let location = source_location(&source_item);
                    let mut diagnostic = ValidationDiagnostic::new(
                        DiagnosticCode::RuleFailed,
                        invariant.severity,
                        ValidationScope::Item,
                        location,
                        format!(
                            "query invariant {} failed its SHACL condition",
                            invariant.id
                        ),
                    );
                    diagnostic.rule = Some(invariant.id.clone());
                    diagnostic.details = Some(
                        json!({"state":"failed","definition":invariant.definition,"shape":invariant.shape,"input_identity":result.input_identity,"binding_count":binding_count,"components":components,"bindings":bindings}),
                    );
                    diagnostics.push(diagnostic);
                } else if state == ConsumerState::Unavailable {
                    let mut diagnostic = ValidationDiagnostic::new(
                        DiagnosticCode::EvaluationUnavailable,
                        Severity::Error,
                        ValidationScope::Item,
                        source_location(&source_item),
                        result.unavailable_reason.clone().unwrap_or_else(|| {
                            "query invariant inputs or required evaluation are unavailable".into()
                        }),
                    );
                    diagnostic.rule = Some(invariant.id.clone());
                    diagnostics.push(diagnostic);
                }
                report.checks.push(check);
            }
            if !has_roots {
                let state = if result.evaluation_complete {
                    ConsumerState::NotApplicable
                } else {
                    ConsumerState::Unavailable
                };
                report.checks.push(QueryPolicyCheck {
                    rule: invariant.id.clone(),
                    definition: invariant.definition.clone(),
                    shape: invariant.shape.clone(),
                    root: None,
                    state,
                    applicability: state,
                    severity: invariant.severity,
                    binding_count: result.evaluation_complete.then_some(0),
                    components: vec![],
                    bindings: vec![],
                    source: source.clone(),
                });
                if state == ConsumerState::Unavailable {
                    report.evaluation_complete = false;
                    diagnostics.push(unavailable_policy(
                        source,
                        &invariant.id,
                        result
                            .unavailable_reason
                            .unwrap_or_else(|| "query policy applicability is unavailable".into()),
                        validation_scope(target.kind),
                    ));
                }
            }
        }
        report.evaluation_complete &= !diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::EvaluationUnavailable);
        Ok((Some(report), diagnostics))
    }

    /// Evaluate a request-local gate. It returns conditions only and cannot mutate validation or execute actions.
    pub fn query_gate(
        &self,
        request: QueryGateRequest,
    ) -> Result<QueryGateResult, QueryConsumerError> {
        validate_scope(&request.scope)?;
        if request.max_rows == 0 {
            return Err(QueryConsumerError::argument(
                "gate max_rows must be positive",
            ));
        }
        let policy_path = self
            .normalize_path(&request.gate_file)
            .map_err(|e| QueryConsumerError::argument(e.to_string()))?;
        let policy_text = self.files().get(&policy_path).ok_or_else(|| {
            QueryConsumerError::definition(format!("missing gate definition source: {policy_path}"))
        })?;
        let (yaml, source_locations) =
            crate::rules::yaml::parse(policy_text, std::path::Path::new(&policy_path))
                .map_err(QueryConsumerError::definition)?;
        let file: GateFile = serde_json::from_value(yaml)
            .map_err(|e| QueryConsumerError::definition(e.to_string()))?;
        if file.format_version != 1 {
            return Err(QueryConsumerError::definition(
                "expected contextual gate format_version 1",
            ));
        }
        let mut gate_ids = BTreeSet::new();
        if file
            .gates
            .iter()
            .any(|gate| gate.id.trim().is_empty() || !gate_ids.insert(gate.id.as_str()))
        {
            return Err(QueryConsumerError::definition(
                "gate IDs must be nonempty and unique",
            ));
        }
        let gate = file
            .gates
            .iter()
            .find(|gate| gate.id == request.gate)
            .ok_or_else(|| {
                QueryConsumerError::definition(format!("unknown exact gate '{}'", request.gate))
            })?;
        let gate_index = file
            .gates
            .iter()
            .position(|candidate| candidate.id == request.gate)
            .expect("selected gate is present");
        let gate_source = source_locations
            .get(&format!("/gates/{gate_index}"))
            .cloned()
            .unwrap_or_else(|| DiagnosticLocation {
                path: Some(policy_path.clone().into()),
                ..Default::default()
            });
        if !gate.conditions.is_empty()
            && (request.definition_files.is_empty() || request.shape_files.is_empty())
        {
            return Err(QueryConsumerError::argument(
                "gates with conditions require explicit named definitions and SHACL sources",
            ));
        }
        let shape_paths = request
            .shape_files
            .iter()
            .map(|p| {
                self.normalize_path(p)
                    .map_err(|e| QueryConsumerError::argument(e.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if shape_paths.iter().collect::<BTreeSet<_>>().len() != shape_paths.len() {
            return Err(QueryConsumerError::argument(
                "duplicate normalized gate SHACL source",
            ));
        }
        let preflight = if request.definition_files.is_empty() {
            if !request.fact_snapshots.is_empty() || !request.mappings.is_empty() {
                return Err(QueryConsumerError::argument(
                    "fact snapshots and mappings require an explicit definition set",
                ));
            }
            None
        } else {
            Some(
                self.preflight_query_inputs(
                    &request.definition_files,
                    &request.fact_snapshots,
                    &request.mappings,
                )
                .map_err(|error| QueryConsumerError {
                    code: error.code,
                    message: error.message,
                })?,
            )
        };
        let definitions = preflight
            .as_ref()
            .map(|inputs| inputs.definitions.clone())
            .unwrap_or_default();
        let context = unique_parameters(&request.context)?;
        let mut used_context = BTreeSet::new();
        let scope = select_scope_roots(self, &request.scope)?;
        let scope_roots = scope.roots;
        let scope_complete = scope.complete;
        let mut condition_defs = Vec::new();
        let mut condition_records = Vec::new();
        let mut condition_root_ids = Vec::new();
        let mut unavailable_reasons = scope.unavailable_reason.into_iter().collect::<Vec<_>>();
        let mut complete = scope_complete;
        if scope_roots.is_empty()
            && let Some(preflight) = &preflight
        {
            for condition in &gate.conditions {
                if let Some(reason) = preflight.unavailable_for(&condition.definition) {
                    complete = false;
                    unavailable_reasons.push(reason);
                }
            }
        }
        let mut identity_parts = vec![
            self.query_fingerprint(),
            request.gate_file.clone(),
            request.gate.clone(),
            serde_json::to_string(&request.scope).unwrap_or_default(),
        ];
        let mut condition_ids = BTreeSet::new();
        for (condition_index, condition) in gate.conditions.iter().enumerate() {
            let condition_source = source_locations
                .get(&format!("/gates/{gate_index}/conditions/{condition_index}"))
                .cloned()
                .unwrap_or_else(|| gate_source.clone());
            if condition.id.trim().is_empty()
                || !condition_ids.insert(condition.id.clone())
                || condition
                    .context_parameters
                    .iter()
                    .any(|name| name.trim().is_empty())
                || condition
                    .context_parameters
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != condition.context_parameters.len()
            {
                return Err(QueryConsumerError::definition(
                    "gate condition IDs and context parameter names must be nonempty and unique",
                ));
            }
            let Some(definition) = definitions.get(&condition.definition) else {
                return Err(QueryConsumerError::definition(format!(
                    "unknown query definition '{}'",
                    condition.definition
                )));
            };
            let shape_file = self
                .normalize_path(&condition.shape_file)
                .map_err(|e| QueryConsumerError::definition(e.to_string()))?;
            if !shape_paths.contains(&shape_file) {
                return Err(QueryConsumerError::argument(format!(
                    "gate shape source was not supplied: {shape_file}"
                )));
            }
            let condition_ir = crate::query_backend::compile_shape_files(
                self.files(),
                std::slice::from_ref(&shape_file),
            )
            .map_err(QueryConsumerError::definition)?;
            if !shape_exists(&condition_ir, &condition.shape) {
                return Err(QueryConsumerError::definition(format!(
                    "unknown SHACL shape '{}' in '{}'",
                    condition.shape, condition.shape_file
                )));
            }
            let applicable_roots = scope_roots
                .iter()
                .filter(|(_, flavour)| {
                    definition
                        .root_flavours
                        .iter()
                        .any(|candidate| candidate == flavour)
                })
                .map(|(root, _)| root)
                .collect::<Vec<_>>();
            let applicable_ids = applicable_roots
                .iter()
                .filter_map(|root| match root {
                    named_query::Endpoint::Item { id, .. } => Some(id.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            condition_root_ids.push(
                applicable_roots
                    .iter()
                    .map(|root| root.iri())
                    .collect::<BTreeSet<_>>(),
            );
            condition_defs.push((condition, shape_file, condition_source, condition_ir));
            if applicable_ids.is_empty() {
                let unavailable_reason: Option<String> = (scope_roots.is_empty()
                    && definition
                        .required_fact_sources
                        .contains(&named_query::FactSource::Code))
                .then(|| {
                    "required code facts were not supplied to this named-query snapshot".into()
                });
                if let Some(reason) = &unavailable_reason {
                    complete = false;
                    unavailable_reasons.push(reason.clone());
                }
                condition_records.push(CollectedQuery {
                    records: vec![],
                    evaluation_complete: unavailable_reason.is_none(),
                    unavailable_reason,
                    input_identity: String::new(),
                });
                continue;
            }
            let mut parameters = Vec::new();
            for name in &condition.context_parameters {
                used_context.insert(name.clone());
                let value = context.get(name).ok_or_else(|| {
                    QueryConsumerError::argument(format!("missing gate context binding '{name}'"))
                })?;
                parameters.push(value.clone());
            }
            let query = named_query::Request {
                definition_files: request.definition_files.clone(),
                definition: condition.definition.clone(),
                selection: named_query::Selection {
                    ids: applicable_ids,
                    ..Default::default()
                },
                parameters,
                max_rows: request.max_rows,
                limit: 100,
                cursor: None,
                fact_snapshots: request.fact_snapshots.clone(),
                mappings: request.mappings.clone(),
            };
            let collected = collect_named_query(self, query).map_err(|e| QueryConsumerError {
                code: e.code,
                message: e.message,
            })?;
            complete &= collected.evaluation_complete;
            if let Some(reason) = &collected.unavailable_reason {
                unavailable_reasons.push(reason.clone());
            }
            identity_parts.push(collected.input_identity.clone());
            condition_records.push(collected);
        }
        if let Some(name) = context.keys().find(|name| !used_context.contains(*name)) {
            return Err(QueryConsumerError::argument(format!(
                "unused gate context binding '{name}'"
            )));
        }
        let mut roots = Vec::new();
        for (root, _) in scope_roots {
            let mut conditions = Vec::new();
            for (index, ((definition, shape_file, source, ir), collected)) in
                condition_defs.iter().zip(&condition_records).enumerate()
            {
                if !condition_root_ids[index].contains(&root.iri()) {
                    conditions.push(QueryGateCondition {
                        id: definition.id.clone(),
                        source: source.clone(),
                        definition: definition.definition.clone(),
                        shape_file: shape_file.clone(),
                        shape: definition.shape.clone(),
                        severity: definition.severity,
                        state: ConsumerState::NotApplicable,
                        applicability: ConsumerState::NotApplicable,
                        binding_count: None,
                        components: vec![],
                        bindings: vec![],
                    });
                    continue;
                }
                let matching = root_bindings(&collected.records)
                    .into_iter()
                    .find(|(candidate, _, _, _)| candidate == &root);
                let (bindings, count) = matching
                    .map(|(_, _, count, bindings)| (bindings, count))
                    .unwrap_or_default();
                let is_complete = collected.evaluation_complete && count.is_some();
                let applicability = if !is_complete {
                    ConsumerState::Unavailable
                } else {
                    match definition.applies_when {
                        None => ConsumerState::Passed,
                        Some(ApplicabilityPredicate::BindingsPresent) if !bindings.is_empty() => {
                            ConsumerState::Passed
                        }
                        Some(ApplicabilityPredicate::BindingsAbsent) if bindings.is_empty() => {
                            ConsumerState::Passed
                        }
                        Some(_) => ConsumerState::NotApplicable,
                    }
                };
                let (state, components) = if applicability == ConsumerState::Unavailable {
                    (ConsumerState::Unavailable, vec![])
                } else if applicability == ConsumerState::NotApplicable {
                    (ConsumerState::NotApplicable, vec![])
                } else {
                    let rows = bindings.iter().map(binding_json_ld).collect::<Vec<_>>();
                    let focus = root.iri();
                    match crate::query_backend::evaluate_projection_shape(
                        ir,
                        &definition.shape,
                        &focus,
                        &rows,
                    ) {
                        Ok(outcome) => (
                            if outcome.conforms {
                                ConsumerState::Passed
                            } else {
                                ConsumerState::Failed
                            },
                            outcome.components,
                        ),
                        Err(message) => {
                            complete = false;
                            unavailable_reasons.push(message);
                            (ConsumerState::Unavailable, vec![])
                        }
                    }
                };
                conditions.push(QueryGateCondition {
                    id: definition.id.clone(),
                    source: source.clone(),
                    definition: definition.definition.clone(),
                    shape_file: shape_file.clone(),
                    shape: definition.shape.clone(),
                    severity: definition.severity,
                    state,
                    applicability,
                    binding_count: count,
                    components,
                    bindings,
                });
            }
            let applicable = conditions
                .iter()
                .filter(|c| c.applicability != ConsumerState::NotApplicable)
                .count();
            let state = if !scope_complete
                || conditions
                    .iter()
                    .any(|c| c.state == ConsumerState::Unavailable)
            {
                ConsumerState::Unavailable
            } else if conditions.iter().any(|c| c.state == ConsumerState::Failed) {
                ConsumerState::Failed
            } else if applicable == 0 {
                gate.empty_required.into()
            } else {
                ConsumerState::Passed
            };
            roots.push(QueryGateRoot {
                root,
                state,
                conditions,
            });
        }
        let empty_reason = if !complete {
            None
        } else if roots.is_empty() {
            Some("roots".into())
        } else if roots.iter().all(|root| {
            root.conditions
                .iter()
                .all(|c| c.applicability == ConsumerState::NotApplicable)
        }) {
            Some("required_conditions".into())
        } else {
            None
        };
        let state = if !complete
            || roots
                .iter()
                .any(|root| root.state == ConsumerState::Unavailable)
        {
            ConsumerState::Unavailable
        } else if roots.is_empty() {
            gate.empty_roots.into()
        } else if roots.iter().any(|root| root.state == ConsumerState::Failed) {
            ConsumerState::Failed
        } else if roots
            .iter()
            .all(|root| root.state == ConsumerState::NotApplicable)
        {
            gate.empty_required.into()
        } else {
            ConsumerState::Passed
        };
        let identity_bytes = serde_json::to_vec(&(
            "query-gate-1",
            &identity_parts,
            &request.scope,
            &request.context,
            &request.fact_snapshots,
            &request.mappings,
            request.max_rows,
        ))
        .unwrap_or_default();
        let input_identity = format!("{:x}", Sha256::digest(identity_bytes));
        Ok(QueryGateResult {
            format_version: 1,
            gate: gate.id.clone(),
            source: gate_source,
            scope: request.scope,
            context: request.context,
            input_identity,
            evaluation_complete: complete && state != ConsumerState::Unavailable,
            state,
            empty_reason,
            unavailable_reason: (!unavailable_reasons.is_empty())
                .then(|| unavailable_reasons.join("; ")),
            roots,
        })
    }
}

fn normalized_policy_path(path: &std::path::Path) -> Result<String, String> {
    crate::normalize_source_path(&path.to_string_lossy()).map_err(|error| error.to_string())
}

fn load_invariants(
    engine: &crate::SourceEngine,
    config: &QueryPolicyConfig,
) -> Result<Vec<(Invariant, DiagnosticLocation)>, (String, String)> {
    let mut invariants = Vec::new();
    let mut ids = BTreeSet::new();
    for path in &config.files {
        let configured_name = path.to_string_lossy().into_owned();
        let name =
            normalized_policy_path(path).map_err(|message| (configured_name.clone(), message))?;
        let normalized_path = std::path::PathBuf::from(&name);
        let text = engine
            .files()
            .get(&name)
            .ok_or_else(|| (name.clone(), "missing query policy source".into()))?;
        let (value, locations) = crate::rules::yaml::parse(text, &normalized_path)
            .map_err(|error| (name.clone(), error))?;
        let file: InvariantFile =
            serde_json::from_value(value).map_err(|error| (name.clone(), error.to_string()))?;
        if file.format_version != 1 {
            return Err((name, "expected query invariant format_version 1".into()));
        }
        for (index, invariant) in file.invariants.into_iter().enumerate() {
            if invariant.id.trim().is_empty() || !ids.insert(invariant.id.clone()) {
                return Err((
                    name,
                    "query invariant IDs must be nonempty and unique".into(),
                ));
            }
            let location = locations
                .get(&format!("/invariants/{index}"))
                .cloned()
                .unwrap_or_else(|| DiagnosticLocation {
                    path: Some(normalized_path.clone()),
                    ..Default::default()
                });
            invariants.push((invariant, location));
        }
    }
    Ok(invariants)
}

struct CollectedQuery {
    records: Vec<named_query::Record>,
    evaluation_complete: bool,
    unavailable_reason: Option<String>,
    input_identity: String,
}
fn collect_named_query(
    engine: &crate::SourceEngine,
    mut request: named_query::Request,
) -> Result<CollectedQuery, named_query::Error> {
    let mut records = Vec::new();
    let mut cursor = None;
    let mut complete = true;
    let mut reason = None;
    let mut identity = None;
    loop {
        request.cursor = cursor.take();
        let page = engine.query_execute(request.clone())?;
        complete &= page.evaluation_complete;
        if reason.is_none() {
            reason = page.unavailable_reason.clone();
        }
        if identity
            .as_ref()
            .is_some_and(|old| old != &page.input_identity)
        {
            return Err(named_query::Error::argument(
                "query input identity changed during gate evaluation",
            ));
        }
        identity.get_or_insert(page.input_identity);
        records.extend(page.records);
        if !page.has_more {
            break;
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            return Err(named_query::Error::argument(
                "query continuation is missing",
            ));
        }
    }
    Ok(CollectedQuery {
        records,
        evaluation_complete: complete,
        unavailable_reason: reason,
        input_identity: identity.unwrap_or_default(),
    })
}
fn validate_invariant_parameters(
    definition: &named_query::Definition,
    supplied: &[named_query::BindingParameter],
) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for parameter in supplied {
        if !names.insert(parameter.name.as_str()) {
            return Err(format!(
                "duplicate invariant parameter '{}'",
                parameter.name
            ));
        }
        let Some(declaration) = definition.parameters.get(&parameter.name) else {
            return Err(format!(
                "undeclared invariant parameter '{}'",
                parameter.name
            ));
        };
        if parameter.value.kind() != declaration.r#type {
            return Err(format!(
                "wrong type for invariant parameter '{}'",
                parameter.name
            ));
        }
    }
    for (name, declaration) in &definition.parameters {
        if declaration.required && declaration.default.is_none() && !names.contains(name.as_str()) {
            return Err(format!("missing required invariant parameter '{name}'"));
        }
    }
    Ok(())
}

fn root_bindings(
    records: &[named_query::Record],
) -> Vec<(
    named_query::Endpoint,
    crate::ItemSource,
    Option<usize>,
    Vec<named_query::Binding>,
)> {
    let mut roots: BTreeMap<
        String,
        (
            named_query::Endpoint,
            crate::ItemSource,
            Option<usize>,
            Vec<named_query::Binding>,
        ),
    > = BTreeMap::new();
    for record in records {
        match record {
            named_query::Record::Root {
                root,
                source,
                binding_count,
            } => {
                roots.insert(
                    root.iri(),
                    (root.clone(), source.clone(), *binding_count, vec![]),
                );
            }
            named_query::Record::Binding { root, binding } => {
                if let Some(entry) = roots.get_mut(&root.iri()) {
                    entry.3.push(binding.clone());
                }
            }
        }
    }
    roots.into_values().collect()
}
fn binding_json_ld(binding: &named_query::Binding) -> BTreeMap<String, JsonValue> {
    binding
        .values
        .iter()
        .filter_map(|(name, value)| {
            value
                .as_ref()
                .map(|value| (name.clone(), named_value_json_ld(value)))
        })
        .collect()
}
fn named_value_json_ld(value: &named_query::Value) -> JsonValue {
    match value {
        named_query::Value::Item { endpoint, .. } => json!({"@id":endpoint.iri()}),
        named_query::Value::Code { reference } => {
            json!({"@id":format!("urn:mara:code:{}", url::form_urlencoded::byte_serialize(reference.as_bytes()).collect::<String>())})
        }
        named_query::Value::External { address } => {
            json!({"@id":format!("urn:mara:external:{}", url::form_urlencoded::byte_serialize(address.as_bytes()).collect::<String>())})
        }
        named_query::Value::Iri { value } => json!({"@id":value}),
        named_query::Value::Text {
            value,
            language: Some(language),
        } => json!({"@value":value,"@language":language}),
        named_query::Value::Text {
            value,
            language: None,
        } => json!({"@value":value}),
        named_query::Value::Integer { value } => {
            json!({"@value":value.to_string(),"@type":"http://www.w3.org/2001/XMLSchema#integer"})
        }
        named_query::Value::Number { value } => {
            json!({"@value":value,"@type":"http://www.w3.org/2001/XMLSchema#double"})
        }
        named_query::Value::Boolean { value } => {
            json!({"@value":value.to_string(),"@type":"http://www.w3.org/2001/XMLSchema#boolean"})
        }
        named_query::Value::Edge { edge } => {
            json!({"@id":format!("urn:mara:query:edge:{}", Sha256::digest(serde_json::to_vec(edge).unwrap_or_default()).iter().map(|b|format!("{b:02x}")).collect::<String>())})
        }
    }
}
fn source_location(source: &crate::ItemSource) -> DiagnosticLocation {
    DiagnosticLocation {
        path: Some(source.path().to_owned()),
        line: Some(source.start_line()),
        start_byte: Some(source.start_byte()),
        end_byte: Some(source.end_byte()),
        pointer: None,
    }
}
fn shape_exists(ir: &shacl::ir::IRSchema, shape: &str) -> bool {
    use rudof_iri::IriS;
    IriS::new(shape).ok().is_some_and(|iri| {
        ir.get_shape(&rudof_rdf::rdf_core::term::Object::iri(iri))
            .is_some()
    })
}
fn validation_scope(target: ValidationTargetKind) -> ValidationScope {
    match target {
        ValidationTargetKind::Project => ValidationScope::Project,
        ValidationTargetKind::Item => ValidationScope::Item,
        ValidationTargetKind::Schema => ValidationScope::Schema,
    }
}
fn unavailable_policy(
    location: DiagnosticLocation,
    rule: &str,
    message: impl Into<String>,
    scope: ValidationScope,
) -> ValidationDiagnostic {
    let mut diagnostic = ValidationDiagnostic::new(
        DiagnosticCode::EvaluationUnavailable,
        Severity::Error,
        scope,
        location,
        message,
    );
    diagnostic.rule = Some(rule.into());
    diagnostic
}
fn invalid_policy(
    location: DiagnosticLocation,
    rule: &str,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    let mut diagnostic = ValidationDiagnostic::new(
        DiagnosticCode::RuleInvalid,
        Severity::Error,
        ValidationScope::Schema,
        location,
        message,
    );
    diagnostic.rule = Some(rule.into());
    diagnostic
}
fn policy_unavailable(
    report: Option<QueryPolicyValidation>,
    path: &str,
    message: String,
    scope: ValidationScope,
) -> (Option<QueryPolicyValidation>, Vec<ValidationDiagnostic>) {
    let location = DiagnosticLocation {
        path: (!path.is_empty()).then(|| path.into()),
        ..Default::default()
    };
    let diagnostic = ValidationDiagnostic::new(
        DiagnosticCode::EvaluationUnavailable,
        Severity::Error,
        scope,
        location,
        message,
    );
    (report, vec![diagnostic])
}
fn path_applies(path: &std::path::Path, filters: &[std::path::PathBuf]) -> bool {
    filters.is_empty() || filters.iter().any(|filter| path.starts_with(filter))
}
fn unique_parameters(
    values: &[named_query::BindingParameter],
) -> Result<BTreeMap<String, named_query::BindingParameter>, QueryConsumerError> {
    let mut result = BTreeMap::new();
    for value in values {
        if result.insert(value.name.clone(), value.clone()).is_some() {
            return Err(QueryConsumerError::argument(format!(
                "duplicate gate context binding '{}'",
                value.name
            )));
        }
    }
    Ok(result)
}
fn validate_scope(scope: &named_query::Selection) -> Result<(), QueryConsumerError> {
    let filtered = !scope.ids.is_empty()
        || !scope.flavours.is_empty()
        || !scope.fields.is_empty()
        || !scope.paths.is_empty();
    if scope.all == filtered {
        return Err(QueryConsumerError::argument(
            "gate scope must select explicit all or at least one root filter; all is exclusive",
        ));
    }
    Ok(())
}
struct ScopeRoots {
    roots: Vec<(named_query::Endpoint, String)>,
    complete: bool,
    unavailable_reason: Option<String>,
}

fn select_scope_roots(
    engine: &crate::SourceEngine,
    selection: &named_query::Selection,
) -> Result<ScopeRoots, QueryConsumerError> {
    let (_project, schema, corpus) = engine.corpus().map_err(|e| QueryConsumerError {
        code: "input_unavailable".into(),
        message: e.to_string(),
    })?;
    let source_diagnostics = crate::validate_corpus(&corpus, &schema);
    let source_reason = if !corpus.is_complete() {
        Some("query scope source inventory is incomplete".to_owned())
    } else if !named_query::graph_source_prerequisites_valid(&source_diagnostics) {
        Some("graph source prerequisites are invalid".to_owned())
    } else {
        None
    };
    let filters = crate::ItemFilters::new(
        selection.flavours.clone(),
        selection
            .fields
            .iter()
            .map(|field| crate::FieldFilter::new(&field.key, &field.value))
            .collect(),
        vec![],
        selection.paths.iter().map(Into::into).collect(),
        None,
    )
    .with_ids(selection.ids.clone());
    let roots = crate::filtered_items(&corpus, &schema, &filters)
        .map(|items| {
            items
                .into_iter()
                .map(|item| (named_query::Endpoint::item(item), item.flavour().to_owned()))
                .collect()
        })
        .map_err(|e| QueryConsumerError::argument(e.to_string()))?;
    Ok(ScopeRoots {
        roots,
        complete: source_reason.is_none(),
        unavailable_reason: source_reason,
    })
}

pub fn append_query_policy_result(
    result: &mut ValidationResult,
    source_engine: &crate::SourceEngine,
    fact_snapshots: &[named_query::FactSnapshot],
    mappings: &[named_query::TypedMapping],
) -> Result<(), QueryConsumerError> {
    let (policy, diagnostics) = source_engine.query_policy_validation_with_inputs(
        result.target.clone(),
        fact_snapshots,
        mappings,
    )?;
    if let Some(policy) = &policy {
        result.evaluation_complete &= policy.evaluation_complete;
    }
    result.query_policies = policy;
    result.diagnostics.extend(diagnostics);
    Ok(())
}

pub fn append_query_policy_unavailable(result: &mut ValidationResult, message: impl Into<String>) {
    result.evaluation_complete = false;
    result.query_policies = Some(QueryPolicyValidation {
        format_version: 1,
        input_identity: String::new(),
        evaluation_complete: false,
        checks: vec![],
    });
    result.diagnostics.push(ValidationDiagnostic::new(
        DiagnosticCode::EvaluationUnavailable,
        Severity::Error,
        ValidationScope::Project,
        DiagnosticLocation::default(),
        message,
    ));
}
