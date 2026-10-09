//! Factual query views and source-linked policy evaluation reports.
//!
//! These are presentation over already-composed results. A view or report never
//! re-runs a traversal, reinterprets a policy outcome or repairs incomplete
//! facts; it preserves the owning result's identity, membership, counts,
//! conditions and completeness while exposing typed support and source links.
use crate::query_consumers::{
    ConsumerState, QueryConsumerError, QueryGateRequest, QueryGateResult, QueryPolicyValidation,
};
use crate::{
    DiagnosticLocation, ItemSource, Severity, SourceEngine, ValidationDiagnostic, ValidationTarget,
    ValidationTargetKind, named_query,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Factual view format 1.
pub const VIEW_FORMAT_VERSION: u32 = 1;
/// Evaluation report format 1.
pub const REPORT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewColumn {
    /// Exact declared tuple column name.
    pub name: String,
    /// Declared value type checked against returned values.
    pub r#type: named_query::ValueType,
    /// True when an absent value is allowed.
    pub optional: bool,
    #[serde(default)]
    pub flavours: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewSummary {
    /// Logical selected roots across the complete result.
    pub selected_roots: usize,
    /// Logical distinct binding count, absent when evaluation is incomplete.
    pub binding_count: Option<usize>,
    /// Logical bindings whose values are known; equals `binding_count` when the complete result is evaluated.
    pub known_bindings: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewRow {
    /// Unchanged typed tuple values keyed by declared column name.
    pub values: BTreeMap<String, Option<named_query::Value>>,
    /// Explicitly projected contributing source support.
    #[serde(default)]
    pub contributors: Vec<named_query::Value>,
    /// Explicitly projected canonical edge support with authored occurrences.
    #[serde(default)]
    pub support: Vec<named_query::Support>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewRoot {
    pub root: named_query::Endpoint,
    /// Absent when this page begins inside a root whose root record was on an earlier page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ItemSource>,
    /// Logical binding count, absent when it is unknown or evaluation is incomplete.
    pub binding_count: Option<usize>,
    pub rows: Vec<ViewRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FactualView {
    pub format_version: u32,
    pub kind: String,
    pub definition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition_file: Option<String>,
    pub input_identity: String,
    pub evaluation_complete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    pub columns: Vec<ViewColumn>,
    pub summary: ViewSummary,
    pub roots: Vec<ViewRoot>,
    pub has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Same rendered page as the human CLI default when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
}

/// Factual view request: an existing named-query request plus optional rendering.
#[derive(Debug, Clone)]
pub struct ViewParams {
    pub request: named_query::Request,
    pub render: Option<String>,
}

/// Whether an obligation's returned support qualifies it or is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SupportState {
    /// The obligation holds and the returned bindings are the qualifying support.
    Qualifying,
    /// The obligation does not hold; returned bindings are partial or offending support.
    Missing,
    /// The obligation was not applicable, so support was not required.
    NotApplicable,
    /// Support could not be evaluated; no pass or failure is claimed.
    Unavailable,
}

/// Gate identity, source and explicit scope/context retained for the report.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GateReportSubject {
    /// Exact gate identity from the request-local YAML gate file.
    pub gate: String,
    /// Source-linked gate definition location from the gate file.
    pub source: DiagnosticLocation,
    /// Explicit root selection kept unchanged from the gate request.
    pub scope: named_query::Selection,
    /// Typed context bindings kept unchanged from the gate request.
    pub context: Vec<named_query::BindingParameter>,
}

/// What the report explains: a request-local gate or enabled documentation rules.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReportSubject {
    Gate(Box<GateReportSubject>),
    DocumentationRules {
        /// Exact validation target whose enabled rules were evaluated.
        target: ValidationTarget,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportSummary {
    /// Checks retained in the report.
    pub total: usize,
    /// Checks whose applicability is known (not not_applicable).
    pub applicable: usize,
    pub passed: usize,
    pub failed: usize,
    pub not_applicable: usize,
    pub unavailable: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportRoot {
    /// Evaluated gate root identity; absent for project-level rule records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<named_query::Endpoint>,
    /// Gate root outcome; absent for documentation-rule roots.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<ConsumerState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportCheck {
    /// Rule identity or gate condition identity from its definition.
    pub obligation: String,
    /// `documentation_rule` or `gate_condition`.
    pub kind: String,
    pub definition: String,
    pub shape: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<named_query::Endpoint>,
    /// Source-linked obligation location from the rule or gate file.
    pub source: DiagnosticLocation,
    pub severity: Severity,
    pub state: ConsumerState,
    pub applicability: ConsumerState,
    pub support_state: SupportState,
    pub binding_count: Option<usize>,
    /// Failed SHACL constraint component IRIs.
    #[serde(default)]
    pub components: Vec<String>,
    /// Unchanged typed returned facts and explicitly projected support.
    #[serde(default)]
    pub bindings: Vec<named_query::Binding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationReport {
    pub format_version: u32,
    pub subject: ReportSubject,
    pub input_identity: String,
    pub evaluation_complete: bool,
    /// Gate aggregate outcome; absent for documentation-rule reports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<ConsumerState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    pub summary: ReportSummary,
    /// Evaluated gate roots and outcomes, independent of condition records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roots: Vec<ReportRoot>,
    pub checks: Vec<ReportCheck>,
    /// Policy-source diagnostics retained beside the checks; empty for gates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ValidationDiagnostic>,
    /// Same rendered page as the human CLI default when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
}

/// Evaluation report request: a gate evaluation or enabled documentation rules.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(transform = schemars::transform::RecursiveTransform(report_kind_guidance))]
pub enum ReportRequest {
    Gate(QueryGateRequest),
    DocumentationRules {
        /// Exact validation target whose enabled rules were evaluated.
        target: ValidationTarget,
    },
}

fn report_kind_guidance(schema: &mut schemars::Schema) {
    let Some(variants) = schema
        .get_mut("oneOf")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    for variant in variants {
        if let Some(kind) = variant
            .get_mut("properties")
            .and_then(|properties| properties.get_mut("kind"))
            .and_then(serde_json::Value::as_object_mut)
        {
            kind.insert(
                "description".into(),
                serde_json::Value::String(
                    "Report subject kind: gate or documentation_rules.".into(),
                ),
            );
        }
    }
}

// @mara implements REQ-QUERY-VIEWS-AND-REPORTS
// @mara implements DES-QUERY-CONSUMER-CONTRACT
impl SourceEngine {
    /// Compose an exact named query and render its factual view. Rendering never
    /// changes returned facts, membership, counts, identity or completeness.
    pub fn query_view(&self, params: ViewParams) -> Result<FactualView, named_query::Error> {
        if params
            .render
            .as_deref()
            .is_some_and(|render| render != "markdown")
        {
            return Err(named_query::Error::argument("render must be markdown"));
        }
        let definition_files = params.request.definition_files.clone();
        let page = self.query_execute(params.request)?;
        let definitions = self.validated_query_definitions(&definition_files)?;
        let columns = definitions
            .get(&page.definition)
            .map(declared_columns)
            .ok_or_else(|| {
                named_query::Error::argument(format!("unknown definition {}", page.definition))
            })?;
        let mut view = factual_view(&page, columns);
        if params.render.as_deref() == Some("markdown") {
            view.markdown = Some(view_markdown(&view));
        }
        Ok(view)
    }

    /// Compose a gate or documentation-rule evaluation and render its report.
    /// Reports identify obligations and outcomes; they never grant permission or
    /// change documentation validity.
    pub fn query_report(
        &self,
        request: ReportRequest,
        render: Option<String>,
    ) -> Result<EvaluationReport, QueryConsumerError> {
        if render.as_deref().is_some_and(|render| render != "markdown") {
            return Err(QueryConsumerError {
                code: "invalid_argument".into(),
                message: "render must be markdown".into(),
            });
        }
        let mut report = match request {
            ReportRequest::Gate(request) => {
                let result = self.query_gate(request)?;
                gate_report(&result)
            }
            ReportRequest::DocumentationRules { target } => {
                let (validation, diagnostics) = self.query_policy_validation(target.clone());
                let Some(validation) = validation else {
                    return Err(QueryConsumerError {
                        code: "invalid_argument".into(),
                        message: "no enabled query policy is configured".into(),
                    });
                };
                rules_report(&validation, target, diagnostics)
            }
        };
        if render.as_deref() == Some("markdown") {
            report.markdown = Some(report_markdown(&report));
        }
        Ok(report)
    }
}

fn declared_columns(definition: &named_query::Definition) -> Vec<ViewColumn> {
    definition
        .columns
        .iter()
        .map(|(name, column)| ViewColumn {
            name: name.clone(),
            r#type: column.r#type,
            optional: column.optional,
            flavours: column.flavours.clone(),
        })
        .collect()
}

/// Pure factual view over one page. Bindings and roots are preserved in order.
// @mara implements REQ-QUERY-VIEWS-AND-REPORTS
pub fn factual_view(page: &named_query::Page, columns: Vec<ViewColumn>) -> FactualView {
    let mut roots: Vec<ViewRoot> = Vec::new();
    let mut groups: BTreeMap<named_query::Endpoint, usize> = BTreeMap::new();
    for record in &page.records {
        match record {
            named_query::Record::Root {
                root,
                source,
                binding_count,
            } => {
                groups.insert(root.clone(), roots.len());
                roots.push(ViewRoot {
                    root: root.clone(),
                    source: Some(source.clone()),
                    binding_count: *binding_count,
                    rows: Vec::new(),
                });
            }
            named_query::Record::Binding { root, binding } => {
                let index = *groups.entry(root.clone()).or_insert_with(|| {
                    roots.push(ViewRoot {
                        root: root.clone(),
                        source: None,
                        binding_count: None,
                        rows: Vec::new(),
                    });
                    roots.len() - 1
                });
                roots[index].rows.push(ViewRow {
                    values: binding.values.clone(),
                    contributors: binding.contributors.clone(),
                    support: binding.support.clone(),
                });
            }
        }
    }
    FactualView {
        format_version: VIEW_FORMAT_VERSION,
        kind: "query_view".into(),
        definition: page.definition.clone(),
        definition_file: page.definition_file.clone(),
        input_identity: page.input_identity.clone(),
        evaluation_complete: page.evaluation_complete,
        unavailable_reason: page.unavailable_reason.clone(),
        columns,
        summary: ViewSummary {
            selected_roots: page.summary.selected_roots,
            binding_count: page.summary.binding_count,
            known_bindings: page.summary.known_bindings,
        },
        roots,
        has_more: page.has_more,
        next_cursor: page.next_cursor.clone(),
        markdown: None,
    }
}

/// Pure gate evaluation report. Gate aggregate and per-root states are preserved.
// @mara implements REQ-QUERY-VIEWS-AND-REPORTS
pub fn gate_report(result: &QueryGateResult) -> EvaluationReport {
    let roots = result
        .roots
        .iter()
        .map(|root| ReportRoot {
            root: Some(root.root.clone()),
            state: Some(root.state),
        })
        .collect::<Vec<_>>();
    let mut checks = Vec::new();
    for root in &result.roots {
        for condition in &root.conditions {
            checks.push(ReportCheck {
                obligation: condition.id.clone(),
                kind: "gate_condition".into(),
                definition: condition.definition.clone(),
                shape: condition.shape.clone(),
                shape_file: Some(condition.shape_file.clone()),
                root: Some(root.root.clone()),
                source: condition.source.clone(),
                severity: condition.severity,
                state: condition.state,
                applicability: condition.applicability,
                support_state: support_state(condition.state),
                binding_count: condition.binding_count,
                components: condition.components.clone(),
                bindings: condition.bindings.clone(),
            });
        }
    }
    let summary = summarize(&checks);
    EvaluationReport {
        format_version: REPORT_FORMAT_VERSION,
        subject: ReportSubject::Gate(Box::new(GateReportSubject {
            gate: result.gate.clone(),
            source: result.source.clone(),
            scope: result.scope.clone(),
            context: result.context.clone(),
        })),
        input_identity: result.input_identity.clone(),
        evaluation_complete: result.evaluation_complete,
        state: Some(result.state),
        empty_reason: result.empty_reason.clone(),
        unavailable_reason: result.unavailable_reason.clone(),
        summary,
        roots,
        checks,
        diagnostics: Vec::new(),
        markdown: None,
    }
}

/// Pure documentation-rule evaluation report. Policy-source diagnostics are retained.
// @mara implements REQ-QUERY-VIEWS-AND-REPORTS
pub fn rules_report(
    validation: &QueryPolicyValidation,
    target: ValidationTarget,
    diagnostics: Vec<ValidationDiagnostic>,
) -> EvaluationReport {
    let checks = validation
        .checks
        .iter()
        .map(|check| ReportCheck {
            obligation: check.rule.clone(),
            kind: "documentation_rule".into(),
            definition: check.definition.clone(),
            shape: check.shape.clone(),
            shape_file: None,
            root: check.root.clone(),
            source: check.source.clone(),
            severity: check.severity,
            state: check.state,
            applicability: check.applicability,
            support_state: support_state(check.state),
            binding_count: check.binding_count,
            components: check.components.clone(),
            bindings: check.bindings.clone(),
        })
        .collect::<Vec<_>>();
    let summary = summarize(&checks);
    EvaluationReport {
        format_version: REPORT_FORMAT_VERSION,
        subject: ReportSubject::DocumentationRules { target },
        input_identity: validation.input_identity.clone(),
        evaluation_complete: validation.evaluation_complete,
        state: None,
        empty_reason: None,
        unavailable_reason: None,
        summary,
        roots: Vec::new(),
        checks,
        diagnostics,
        markdown: None,
    }
}

fn support_state(state: ConsumerState) -> SupportState {
    match state {
        ConsumerState::Passed => SupportState::Qualifying,
        ConsumerState::Failed => SupportState::Missing,
        ConsumerState::NotApplicable => SupportState::NotApplicable,
        ConsumerState::Unavailable => SupportState::Unavailable,
    }
}

fn summarize(checks: &[ReportCheck]) -> ReportSummary {
    let mut summary = ReportSummary {
        total: checks.len(),
        applicable: 0,
        passed: 0,
        failed: 0,
        not_applicable: 0,
        unavailable: 0,
    };
    for check in checks {
        match check.state {
            ConsumerState::Passed => summary.passed += 1,
            ConsumerState::Failed => summary.failed += 1,
            ConsumerState::NotApplicable => summary.not_applicable += 1,
            ConsumerState::Unavailable => summary.unavailable += 1,
        }
        if check.applicability != ConsumerState::NotApplicable {
            summary.applicable += 1;
        }
    }
    summary
}

fn state_name(state: ConsumerState) -> &'static str {
    match state {
        ConsumerState::Passed => "passed",
        ConsumerState::Failed => "failed",
        ConsumerState::NotApplicable => "not_applicable",
        ConsumerState::Unavailable => "unavailable",
    }
}

fn support_name(state: SupportState) -> &'static str {
    match state {
        SupportState::Qualifying => "qualifying",
        SupportState::Missing => "missing",
        SupportState::NotApplicable => "not_applicable",
        SupportState::Unavailable => "unavailable",
    }
}

fn type_name(kind: named_query::ValueType) -> &'static str {
    match kind {
        named_query::ValueType::Item => "item",
        named_query::ValueType::Code => "code",
        named_query::ValueType::External => "external",
        named_query::ValueType::Iri => "iri",
        named_query::ValueType::Text => "text",
        named_query::ValueType::Integer => "integer",
        named_query::ValueType::Number => "number",
        named_query::ValueType::Boolean => "boolean",
        named_query::ValueType::Edge => "edge",
    }
}

fn diagnostic_code(code: &crate::DiagnosticCode) -> String {
    serde_json::to_value(code)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{code:?}"))
}

fn source_link(source: &DiagnosticLocation) -> String {
    let path = source
        .path
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    match source.line {
        Some(line) => format!(
            "[{}](<{}>) (line {})",
            md_text(&path),
            md_target(&path),
            line
        ),
        None => format!("[{}](<{}>)", md_text(&path), md_target(&path)),
    }
}

fn item_link(endpoint: &named_query::Endpoint, source: &ItemSource) -> String {
    let path = source.path().to_string_lossy();
    match endpoint {
        named_query::Endpoint::Item { id, .. } => format!(
            "[{}](<{}>) (line {})",
            md_text(id),
            md_target(&path),
            source.start_line()
        ),
        _ => endpoint_text(endpoint),
    }
}

fn endpoint_text(endpoint: &named_query::Endpoint) -> String {
    match endpoint {
        named_query::Endpoint::Item { id, .. } => format!("item {}", md_text(id)),
        named_query::Endpoint::Code { reference } => md_text(reference),
        named_query::Endpoint::External { address } => {
            format!("external [{}](<{}>)", md_text(address), url_target(address))
        }
    }
}

fn value_text(value: &named_query::Value) -> String {
    match value {
        named_query::Value::Item { endpoint, source } => item_link(endpoint, source),
        named_query::Value::Code { reference } => md_text(reference),
        named_query::Value::External { address } => {
            format!("[{}](<{}>)", md_text(address), url_target(address))
        }
        named_query::Value::Iri { value } => format!("&lt;{}&gt;", md_text(value)),
        named_query::Value::Text { value, language } => match language {
            Some(language) => format!("{}@{}", md_text(value), language),
            None => md_text(value),
        },
        named_query::Value::Integer { value } => value.to_string(),
        named_query::Value::Number { value } => md_text(value),
        named_query::Value::Boolean { value } => value.to_string(),
        named_query::Value::Edge { edge } => edge_text(edge),
    }
}

fn edge_text(edge: &named_query::Edge) -> String {
    format!(
        "{} {} {}",
        endpoint_text(&edge.source),
        md_text(&edge.relation),
        endpoint_text(&edge.target)
    )
}

fn occurrence_text(occurrence: &named_query::Occurrence) -> String {
    let path = occurrence.source.path().to_string_lossy();
    format!(
        "[{}](<{}>) (line {})",
        md_text(&path),
        md_target(&path),
        occurrence.source.start_line()
    )
}

fn md_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch == '\n' || ch == '\r' {
            out.push(' ');
            continue;
        }
        if matches!(
            ch,
            '\\' | '`'
                | '*'
                | '_'
                | '['
                | ']'
                | '<'
                | '>'
                | '('
                | ')'
                | '|'
                | '~'
                | '!'
                | '&'
                | '#'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn md_target(value: &str) -> String {
    crate::trace::md_target(value).replace('|', "%7C")
}

/// Angle-bracket link destination for an external URL. Unlike a filesystem path,
/// a URL keeps its query, fragment and existing percent escapes; only characters
/// that cannot appear unescaped inside `<...>` are encoded.
fn url_target(address: &str) -> String {
    address
        .replace('\\', "%5C")
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace('|', "%7C")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

/// Markdown rendering of one factual view page.
pub fn view_markdown(view: &FactualView) -> String {
    let mut out = String::from(
        "# Factual query view\n\nA view preserves returned facts. An empty root is not a failure.\n\n",
    );
    out.push_str(&format!("Definition {}", md_text(&view.definition)));
    if let Some(file) = &view.definition_file {
        out.push_str(&format!(" from [{}](<{}>)", md_text(file), md_target(file)));
    }
    out.push_str(&format!(
        ". Input identity {}. Evaluation complete: **{}**. More records: **{}**.\n\n",
        view.input_identity, view.evaluation_complete, view.has_more
    ));
    match view.summary.binding_count {
        Some(count) => out.push_str(&format!(
            "Summary: {} selected roots; {} bindings; {} known bindings.\n\n",
            view.summary.selected_roots, count, view.summary.known_bindings
        )),
        None => out.push_str(&format!(
            "Summary: {} selected roots; binding count unavailable; {} known bindings.\n\n",
            view.summary.selected_roots, view.summary.known_bindings
        )),
    }
    if view.columns.is_empty() {
        out.push_str("Declared columns: none.\n\n");
    } else {
        let columns = view
            .columns
            .iter()
            .map(|column| {
                format!(
                    "{} ({}{})",
                    md_text(&column.name),
                    type_name(column.r#type),
                    if column.optional { " optional" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("Declared columns: {columns}.\n\n"));
    }
    if !view.evaluation_complete {
        out.push_str(&format!(
            "Evaluation unavailable: {}.\n\n",
            md_text(
                view.unavailable_reason
                    .as_deref()
                    .unwrap_or("no reason reported")
            )
        ));
    }
    for root in &view.roots {
        out.push_str(&root_markdown(view, root));
    }
    if view.has_more {
        out.push_str(&format!(
            "\nContinued: use next_cursor {} with unchanged inputs.\n",
            view.next_cursor.as_deref().unwrap_or("")
        ));
    }
    out
}

fn support_line(
    contributors: &[named_query::Value],
    support: &[named_query::Support],
) -> Option<String> {
    if contributors.is_empty() && support.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    if !contributors.is_empty() {
        parts.push(format!(
            "contributors {}",
            contributors
                .iter()
                .map(value_text)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for entry in support {
        let occurrences = entry
            .occurrences
            .iter()
            .map(occurrence_text)
            .collect::<Vec<_>>();
        let detail = if occurrences.is_empty() {
            String::new()
        } else {
            format!(" ({})", occurrences.join("; "))
        };
        parts.push(format!("{}{}", edge_text(&entry.edge), detail));
    }
    Some(parts.join("; "))
}

fn root_markdown(view: &FactualView, root: &ViewRoot) -> String {
    let label = match &root.source {
        Some(source) => item_link(&root.root, source),
        None => endpoint_text(&root.root),
    };
    let count = match root.binding_count {
        Some(count) => format!("{count} bindings"),
        None => "binding count unavailable".to_owned(),
    };
    let mut out = format!("\n## {label} — {count}\n\n");
    if root.rows.is_empty() {
        out.push_str(&match root.binding_count {
            Some(0) if view.evaluation_complete => {
                "No related matches were returned for this root in the complete result.\n\n"
                    .to_owned()
            }
            Some(count) => format!(
                "No rows on this page; the complete result has {count} bindings. Follow the continuation for the remaining rows.\n\n"
            ),
            None => {
                "No rows on this page; the logical binding count is unavailable.\n\n".to_owned()
            }
        });
        return out;
    }
    let mut names = view
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let mut extras = BTreeSet::new();
    for row in &root.rows {
        for name in row.values.keys() {
            if !names.contains(name) {
                extras.insert(name.clone());
            }
        }
    }
    names.extend(extras);
    let mut header = vec!["#".to_owned()];
    header.extend(names.iter().map(|name| md_text(name)));
    out.push_str(&format!("| {} |\n", header.join(" | ")));
    out.push_str(&format!("|{}|\n", vec!["---"; header.len()].join(" | ")));
    for (index, row) in root.rows.iter().enumerate() {
        let mut cells = vec![(index + 1).to_string()];
        for name in &names {
            cells.push(match row.values.get(name) {
                Some(Some(value)) => value_text(value),
                Some(None) | None => "—".to_owned(),
            });
        }
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    out.push('\n');
    if root
        .rows
        .iter()
        .any(|row| !row.support.is_empty() || !row.contributors.is_empty())
    {
        out.push_str("Support:\n\n");
        for (index, row) in root.rows.iter().enumerate() {
            if let Some(line) = support_line(&row.contributors, &row.support) {
                out.push_str(&format!("- Row {}: {}.\n", index + 1, line));
            }
        }
        out.push('\n');
    }
    out
}

/// Markdown rendering of one evaluation report.
pub fn report_markdown(report: &EvaluationReport) -> String {
    let mut out = match &report.subject {
        ReportSubject::Gate(subject) => format!(
            "# Gate evaluation report: {}\n\nGate definition: {}. ",
            md_text(&subject.gate),
            source_link(&subject.source)
        ),
        ReportSubject::DocumentationRules { target } => format!(
            "# Documentation rule evaluation report\n\nTarget: {}. ",
            target_text(target)
        ),
    };
    if let ReportSubject::Gate(subject) = &report.subject {
        out.push_str(&format!(
            "Scope: {}. Context: {}.\n\n",
            selection_text(&subject.scope),
            context_text(&subject.context)
        ));
    }
    out.push_str(&format!(
        "Input identity {}. Evaluation complete: **{}**.\n\n",
        report.input_identity, report.evaluation_complete
    ));
    if let Some(state) = report.state {
        out.push_str(&format!("Overall state: **{}**.\n\n", state_name(state)));
    }
    if let Some(reason) = &report.empty_reason {
        out.push_str(&format!("Empty reason: {}.\n\n", md_text(reason)));
    }
    if let Some(reason) = &report.unavailable_reason {
        out.push_str(&format!("Unavailable reason: {}.\n\n", md_text(reason)));
    }
    out.push_str(&format!(
        "Summary: {} checks; {} applicable; {} passed, {} failed, {} not applicable, {} unavailable.\n\n",
        report.summary.total,
        report.summary.applicable,
        report.summary.passed,
        report.summary.failed,
        report.summary.not_applicable,
        report.summary.unavailable
    ));
    if !report.roots.is_empty() {
        out.push_str("| Root | Outcome |\n|---|---|\n");
        for root in &report.roots {
            let label = match &root.root {
                Some(root) => endpoint_text(root),
                None => "project".to_owned(),
            };
            out.push_str(&format!(
                "| {} | {} |\n",
                label,
                root.state.map_or_else(
                    || "unknown".to_owned(),
                    |state| state_name(state).to_owned()
                )
            ));
        }
        out.push('\n');
    }
    if !report.diagnostics.is_empty() {
        out.push_str("Diagnostics:\n\n");
        for diagnostic in &report.diagnostics {
            out.push_str(&format!(
                "- [{}] {}: {}\n",
                match diagnostic.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                },
                diagnostic_code(&diagnostic.code),
                md_text(&diagnostic.message)
            ));
        }
        out.push('\n');
    }
    for check in &report.checks {
        out.push_str(&check_markdown(check));
    }
    out
}

fn selection_text(selection: &named_query::Selection) -> String {
    if selection.all {
        return "all roots".into();
    }
    let mut parts = Vec::new();
    if !selection.ids.is_empty() {
        parts.push(format!("ids {}", quoted_list(&selection.ids)));
    }
    if !selection.flavours.is_empty() {
        parts.push(format!("flavours {}", quoted_list(&selection.flavours)));
    }
    if !selection.fields.is_empty() {
        let fields = selection
            .fields
            .iter()
            .map(|field| format!("{}={}", md_text(&field.key), md_text(&field.value)))
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(format!("fields {fields}"));
    }
    if !selection.paths.is_empty() {
        parts.push(format!("paths {}", quoted_list(&selection.paths)));
    }
    if parts.is_empty() {
        "no explicit root selection".into()
    } else {
        parts.join("; ")
    }
}

fn quoted_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| md_text(value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn context_text(context: &[named_query::BindingParameter]) -> String {
    if context.is_empty() {
        return "none".into();
    }
    context
        .iter()
        .map(|parameter| {
            format!(
                "{} = {}",
                md_text(&parameter.name),
                parameter_value_text(&parameter.value)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn parameter_value_text(value: &named_query::ParameterValue) -> String {
    use named_query::ParameterValue as P;
    match value {
        P::Text(value) => format!("text {}", md_text(value)),
        P::Integer(value) => format!("integer {value}"),
        P::Number(value) => format!("number {}", md_text(&value.to_string())),
        P::Boolean(value) => format!("boolean {value}"),
        P::Item(value) => format!("item {}", md_text(value)),
        P::Iri(value) => format!("iri {}", md_text(value)),
    }
}

fn target_text(target: &ValidationTarget) -> String {
    match target.kind {
        ValidationTargetKind::Item => {
            format!("item {}", md_text(target.id.as_deref().unwrap_or("")))
        }
        ValidationTargetKind::Project => "project".to_owned(),
        ValidationTargetKind::Schema => "schema".to_owned(),
    }
}

fn check_markdown(check: &ReportCheck) -> String {
    let mut out = format!(
        "\n## {} — {} — {}\n\n",
        md_text(&check.obligation),
        md_text(&check.kind),
        state_name(check.state)
    );
    if let Some(root) = &check.root {
        out.push_str(&format!("Root: {}.\n\n", endpoint_text(root)));
    }
    let summary = format!(
        "Definition {}; shape {}{}; severity {}; outcome **{}**; applicability **{}**; support **{}**; returned bindings {}; components {}.",
        md_text(&check.definition),
        md_text(&check.shape),
        match &check.shape_file {
            Some(file) => format!(" from [{}](<{}>)", md_text(file), md_target(file)),
            None => String::new(),
        },
        match check.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        },
        state_name(check.state),
        state_name(check.applicability),
        support_name(check.support_state),
        match check.binding_count {
            Some(count) => count.to_string(),
            None => "unavailable".to_owned(),
        },
        if check.components.is_empty() {
            "none".to_owned()
        } else {
            check
                .components
                .iter()
                .map(|component| md_text(component))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    out.push_str(&format!(
        "Obligation source: {}. {summary}\n\n",
        source_link(&check.source)
    ));
    if !check.bindings.is_empty() {
        out.push_str(&bindings_markdown(&check.bindings));
    }
    out
}

fn bindings_markdown(bindings: &[named_query::Binding]) -> String {
    let mut names = BTreeSet::new();
    for binding in bindings {
        names.extend(binding.values.keys().cloned());
    }
    let names = names.into_iter().collect::<Vec<_>>();
    let mut out = String::new();
    if !names.is_empty() {
        let mut header = vec!["#".to_owned()];
        header.extend(names.iter().map(|name| md_text(name)));
        out.push_str(&format!("| {} |\n", header.join(" | ")));
        out.push_str(&format!("|{}|\n", vec!["---"; header.len()].join(" | ")));
        for (index, binding) in bindings.iter().enumerate() {
            let mut cells = vec![(index + 1).to_string()];
            for name in &names {
                cells.push(match binding.values.get(name) {
                    Some(Some(value)) => value_text(value),
                    Some(None) | None => "—".to_owned(),
                });
            }
            out.push_str(&format!("| {} |\n", cells.join(" | ")));
        }
        out.push('\n');
    }
    for (index, binding) in bindings.iter().enumerate() {
        if let Some(line) = support_line(&binding.contributors, &binding.support) {
            out.push_str(&format!("- Support {}: {}.\n", index + 1, line));
        }
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ItemSource;
    use crate::named_query::{Endpoint, Value};
    use serde_json::json;

    #[test]
    fn literal_values_render_without_markdown_or_html_reinterpretation() {
        let value = Value::Text {
            value: "<pending> [missing](other.md) `code` &amp; *em*".into(),
            language: None,
        };
        let rendered = value_text(&value);
        assert!(rendered.contains("\\<pending\\>"), "{rendered}");
        assert!(!rendered.contains("<pending>"), "{rendered}");
        assert!(
            rendered.contains("\\[missing\\]\\(other.md\\)"),
            "{rendered}"
        );
        assert!(rendered.contains("\\`code\\`"), "{rendered}");
        assert!(rendered.contains("\\&amp;"), "{rendered}");
        assert!(rendered.contains("\\*em\\*"), "{rendered}");
    }

    #[test]
    fn code_references_render_literally() {
        let value = Value::Code {
            reference: "crates/a.rs::rust::foo_bar`baz|qux".into(),
        };
        let rendered = value_text(&value);
        assert!(rendered.contains("foo\\_bar"), "{rendered}");
        assert!(rendered.contains("\\`baz"), "{rendered}");
        assert!(rendered.contains("\\|qux"), "{rendered}");
    }

    #[test]
    fn generated_item_links_keep_their_target() {
        let source: ItemSource = serde_json::from_value(json!({
            "path": "docs/example.mara.md",
            "start_byte": 0,
            "end_byte": 7,
            "start_line": 3,
            "end_line": 3
        }))
        .unwrap();
        let value = Value::Item {
            endpoint: Endpoint::Item {
                id: "SUB-A".into(),
                mid: None,
            },
            source,
        };
        assert_eq!(
            value_text(&value),
            "[SUB-A](<docs/example.mara.md>) (line 3)"
        );
    }

    #[test]
    fn filesystem_link_targets_encode_table_delimiters() {
        assert_eq!(md_target("docs/a|b.mara.md"), "docs/a%7Cb.mara.md");
        let source: ItemSource = serde_json::from_value(json!({
            "path": "docs/a|b.mara.md",
            "start_byte": 0,
            "end_byte": 7,
            "start_line": 1,
            "end_line": 1
        }))
        .unwrap();
        let value = Value::Item {
            endpoint: Endpoint::Item {
                id: "SUB-A".into(),
                mid: None,
            },
            source,
        };
        assert_eq!(value_text(&value), "[SUB-A](<docs/a%7Cb.mara.md>) (line 1)");
    }

    #[test]
    fn external_addresses_keep_their_url_destination() {
        let value = Value::External {
            address: "https://example.org/report?candidate=A#result%20x".into(),
        };
        let rendered = value_text(&value);
        assert!(
            rendered.contains("](<https://example.org/report?candidate=A#result%20x>)"),
            "{rendered}"
        );
        for encoded in ["%2520", "%3F", "%23"] {
            assert!(!rendered.contains(encoded), "{rendered}");
        }
        let piped = Value::External {
            address: "https://example.org/report?filter=a|b".into(),
        };
        let rendered = value_text(&piped);
        assert!(
            rendered.contains("](<https://example.org/report?filter=a%7Cb>)"),
            "{rendered}"
        );
    }
}
