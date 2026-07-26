use std::collections::BTreeSet;

use crate::{
    CardinalityBound, CardinalityMaximum, Diagnostic, DiagnosticCode, DiagnosticContext,
    DiagnosticItem, DiagnosticSeverity, DiagnosticValue, FieldDiagnosticCode, FieldRuleSelection,
    NodeRef, NormalizedItem, NormalizedScalar, QueryGraph, ReferenceDiagnosticCode,
    RelatedDiagnostic, RelationDefinition, RelationDiagnosticCode, RelationRuleSelection,
    RuleConditionValue, RuleConfiguration, RuleDefinition, RuleDiagnosticCode, RuleDirection,
    RuleSeverity, SchemaDocument, SourceSpan,
};

/// Ordered stages in the dependency-aware validation pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ValidationPhase {
    Project,
    Schema,
    Content,
    Parse,
    Semantic,
    Graph,
    Rules,
}

impl ValidationPhase {
    pub const ALL: [Self; 7] = [
        Self::Project,
        Self::Schema,
        Self::Content,
        Self::Parse,
        Self::Semantic,
        Self::Graph,
        Self::Rules,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Schema => "schema",
            Self::Content => "content",
            Self::Parse => "parse",
            Self::Semantic => "semantic",
            Self::Graph => "graph",
            Self::Rules => "rules",
        }
    }
}

/// Completion state for one validation phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationPhaseState {
    Completed,
    Skipped {
        reason: String,
        prerequisite: Option<ValidationPhase>,
    },
}

impl ValidationPhaseState {
    pub fn skipped(reason: impl Into<String>, prerequisite: Option<ValidationPhase>) -> Self {
        Self::Skipped {
            reason: reason.into(),
            prerequisite,
        }
    }

    pub const fn reason(&self) -> Option<&str> {
        match self {
            Self::Completed => None,
            Self::Skipped { reason, .. } => Some(reason.as_str()),
        }
    }

    pub const fn prerequisite(&self) -> Option<ValidationPhase> {
        match self {
            Self::Completed => None,
            Self::Skipped { prerequisite, .. } => *prerequisite,
        }
    }
}

/// One ordered phase entry retained by an immutable validation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationPhaseResult {
    phase: ValidationPhase,
    state: ValidationPhaseState,
}

impl ValidationPhaseResult {
    pub fn new(phase: ValidationPhase, state: ValidationPhaseState) -> Self {
        Self { phase, state }
    }

    pub const fn phase(&self) -> ValidationPhase {
        self.phase
    }

    pub const fn state(&self) -> &ValidationPhaseState {
        &self.state
    }
}

/// Deterministic diagnostic totals without changing declared severities.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SeverityCounts {
    errors: usize,
    warnings: usize,
    info: usize,
}

impl SeverityCounts {
    pub fn from_diagnostics(diagnostics: &[Diagnostic]) -> Self {
        let mut counts = Self::default();
        for diagnostic in diagnostics {
            match diagnostic.severity() {
                DiagnosticSeverity::Error => counts.errors += 1,
                DiagnosticSeverity::Warning => counts.warnings += 1,
                DiagnosticSeverity::Info => counts.info += 1,
            }
        }
        counts
    }

    pub const fn errors(self) -> usize {
        self.errors
    }

    pub const fn warnings(self) -> usize {
        self.warnings
    }

    pub const fn info(self) -> usize {
        self.info
    }

    pub const fn is_valid(self, warnings_as_errors: bool) -> bool {
        self.errors == 0 && (!warnings_as_errors || self.warnings == 0)
    }
}

/// Evaluates declared relation constraints and schema rules over one compiled graph.
pub fn evaluate_model(
    schema: &SchemaDocument,
    items: &[NormalizedItem],
    graph: Option<&QueryGraph>,
    prerequisite_diagnostics: &[Diagnostic],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if let Some(graph) = graph {
        evaluate_relation_constraints(
            schema,
            items,
            graph,
            prerequisite_diagnostics,
            &mut diagnostics,
        );
    }
    evaluate_rules(
        schema,
        items,
        graph,
        prerequisite_diagnostics,
        &mut diagnostics,
    );
    crate::sort_diagnostics(&mut diagnostics);
    diagnostics
}

fn evaluate_relation_constraints(
    schema: &SchemaDocument,
    items: &[NormalizedItem],
    graph: &QueryGraph,
    prerequisite_diagnostics: &[Diagnostic],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(relations) = schema.relations() else {
        return;
    };
    for definition in relations.definitions().values() {
        if let Some(cardinality) = definition.cardinality() {
            if let Some(outgoing) = cardinality.value().outgoing() {
                let mut sources = items
                    .iter()
                    .filter(|item| relation_source_applies(definition, item))
                    .map(|item| NodeRef::item(item.mid().clone()))
                    .collect::<BTreeSet<_>>();
                sources.extend(
                    graph
                        .edges()
                        .iter()
                        .filter(|edge| edge.relation() == definition.name())
                        .map(|edge| edge.source().clone()),
                );
                for source in sources {
                    let count = graph
                        .edges()
                        .iter()
                        .filter(|edge| {
                            edge.relation() == definition.name() && edge.source() == &source
                        })
                        .count() as u64;
                    if node_relation_prerequisite_unavailable(
                        &source,
                        definition.name(),
                        RuleDirection::Outgoing,
                        prerequisite_diagnostics,
                    ) && count_outcome_uncertain(
                        count,
                        outgoing.value().minimum(),
                        outgoing.value().maximum(),
                    ) {
                        continue;
                    }
                    if !count_satisfies(count, outgoing.value()) {
                        diagnostics.push(cardinality_diagnostic(
                            definition,
                            items,
                            &source,
                            "outgoing",
                            count,
                            outgoing.value(),
                        ));
                    }
                }
            }
            if let Some(incoming) = cardinality.value().incoming() {
                let mut targets = items
                    .iter()
                    .filter(|item| relation_target_applies(definition, item))
                    .map(|item| NodeRef::item(item.mid().clone()))
                    .collect::<BTreeSet<_>>();
                targets.extend(
                    graph
                        .edges()
                        .iter()
                        .filter(|edge| edge.relation() == definition.name())
                        .map(|edge| edge.target().clone()),
                );
                for target in targets {
                    let count = graph
                        .edges()
                        .iter()
                        .filter(|edge| {
                            edge.relation() == definition.name() && edge.target() == &target
                        })
                        .count() as u64;
                    if node_relation_prerequisite_unavailable(
                        &target,
                        definition.name(),
                        RuleDirection::Incoming,
                        prerequisite_diagnostics,
                    ) && count_outcome_uncertain(
                        count,
                        incoming.value().minimum(),
                        incoming.value().maximum(),
                    ) {
                        continue;
                    }
                    if !count_satisfies(count, incoming.value()) {
                        diagnostics.push(cardinality_diagnostic(
                            definition,
                            items,
                            &target,
                            "incoming",
                            count,
                            incoming.value(),
                        ));
                    }
                }
            }
        }
        if definition.is_acyclic()
            && let Some(cycle) = graph.cycle_path(definition.name())
        {
            diagnostics.push(cycle_diagnostic(definition, items, &cycle));
        }
    }
}

fn relation_source_applies(definition: &RelationDefinition, item: &NormalizedItem) -> bool {
    definition
        .source()
        .value()
        .flavours()
        .value()
        .iter()
        .any(|flavour| flavour.value() == item.flavour())
}

fn relation_target_applies(definition: &RelationDefinition, item: &NormalizedItem) -> bool {
    definition
        .target()
        .value()
        .flavours()
        .is_some_and(|flavours| {
            flavours
                .value()
                .iter()
                .any(|flavour| flavour.value() == item.flavour())
        })
}

fn count_satisfies(count: u64, bound: &CardinalityBound) -> bool {
    count >= bound.minimum()
        && match bound.maximum() {
            CardinalityMaximum::Bounded(maximum) => count <= maximum,
            CardinalityMaximum::Many => true,
        }
}

fn cardinality_diagnostic(
    definition: &RelationDefinition,
    items: &[NormalizedItem],
    node: &NodeRef,
    direction: &str,
    count: u64,
    bound: &CardinalityBound,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(
        RelationDiagnosticCode::Cardinality,
        format!(
            "{direction} relation count for {:?} violates its declared bounds",
            definition.name()
        ),
        node_primary(items, node),
    )
    .with_related(RelatedDiagnostic::new(
        "relation cardinality declared here",
        definition.value_source().clone(),
    ))
    .with_context(DiagnosticContext::new(
        None,
        Some(definition.name().to_owned()),
        node_target(node),
    ))
    .with_detail("actual", DiagnosticValue::Unsigned(count))
    .with_detail("direction", direction)
    .with_detail("maximum", maximum_detail(bound.maximum()))
    .with_detail("minimum", DiagnosticValue::Unsigned(bound.minimum()))
    .with_detail("node", node_identity(node))
    .with_detail("node_kind", node.kind())
    .with_detail("relation", definition.name());
    if let Some(item) = node_item(items, node) {
        diagnostic = diagnostic.with_item(diagnostic_item(item));
    }
    diagnostic
}

fn cycle_diagnostic(
    definition: &RelationDefinition,
    items: &[NormalizedItem],
    cycle: &[NodeRef],
) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(
        RelationDiagnosticCode::Cycle,
        format!("acyclic relation {:?} contains a cycle", definition.name()),
        cycle.first().and_then(|node| node_primary(items, node)),
    )
    .with_related(RelatedDiagnostic::new(
        "acyclic relation declared here",
        definition.value_source().clone(),
    ))
    .with_context(DiagnosticContext::new(
        None,
        Some(definition.name().to_owned()),
        None,
    ))
    .with_detail(
        "cycle_path",
        DiagnosticValue::Array(
            cycle
                .iter()
                .map(|node| DiagnosticValue::String(node_identity(node)))
                .collect(),
        ),
    )
    .with_detail("relation", definition.name());
    if let Some(item) = cycle.first().and_then(|node| node_item(items, node)) {
        diagnostic = diagnostic.with_item(diagnostic_item(item));
    }
    let mut related_nodes = BTreeSet::new();
    for node in cycle.iter().skip(1).take(cycle.len().saturating_sub(2)) {
        if related_nodes.insert(node.clone())
            && let Some(span) = node_primary(items, node)
        {
            diagnostic = diagnostic.with_related(RelatedDiagnostic::new(
                format!("cycle includes {}", node_identity(node)),
                span,
            ));
        }
    }
    diagnostic
}

fn evaluate_rules(
    schema: &SchemaDocument,
    items: &[NormalizedItem],
    graph: Option<&QueryGraph>,
    prerequisite_diagnostics: &[Diagnostic],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(rules) = schema.rules() else {
        return;
    };
    for rule in rules.definitions() {
        if graph.is_none()
            && matches!(
                rule.configuration(),
                RuleConfiguration::RequiresRelation(_) | RuleConfiguration::Orphan(_)
            )
        {
            diagnostics.push(rule_skipped(
                rule,
                None,
                "graph model is globally ambiguous",
                "graph",
            ));
            continue;
        }
        for item in items {
            if !rule_applies_to(rule, item) {
                continue;
            }
            match condition_matches(rule, item, prerequisite_diagnostics) {
                ConditionResult::DoesNotMatch => continue,
                ConditionResult::Unavailable => {
                    diagnostics.push(rule_skipped(
                        rule,
                        Some(item),
                        "condition field is unavailable",
                        "condition",
                    ));
                    continue;
                }
                ConditionResult::Matches => {}
            }
            match rule.configuration() {
                RuleConfiguration::RequiresField(configuration) => {
                    let fields = selected_fields(configuration.fields());
                    if fields.iter().any(|field| {
                        item_field_prerequisite_unavailable(item, field, prerequisite_diagnostics)
                    }) {
                        diagnostics.push(rule_skipped(
                            rule,
                            Some(item),
                            "selected field value is unavailable",
                            "field",
                        ));
                        continue;
                    }
                    let count = fields
                        .iter()
                        .map(|field| item.fields().get(field).map_or(0, Vec::len))
                        .sum::<usize>() as u64;
                    if !rule_count_satisfies(count, configuration.count()) {
                        diagnostics.push(rule_failure(
                            rule,
                            item,
                            count,
                            configuration.count().min().value().to_owned(),
                            configuration.count().maximum(),
                            Some(fields),
                            None,
                        ));
                    }
                }
                RuleConfiguration::RequiresRelation(configuration) => {
                    let relations = selected_relations(configuration.relations());
                    let node = NodeRef::item(item.mid().clone());
                    let direction = *configuration.direction().value();
                    let count = graph
                        .expect("relation rules were skipped when the graph was unavailable")
                        .edges()
                        .iter()
                        .filter(|edge| {
                            relations.iter().any(|relation| relation == edge.relation())
                                && match direction {
                                    RuleDirection::Outgoing => edge.source() == &node,
                                    RuleDirection::Incoming => edge.target() == &node,
                                }
                        })
                        .count() as u64;
                    if relations.iter().any(|relation| {
                        item_relation_prerequisite_unavailable(
                            item,
                            relation,
                            direction,
                            prerequisite_diagnostics,
                        )
                    }) && count_outcome_uncertain(
                        count,
                        *configuration.count().min().value(),
                        configuration.count().maximum(),
                    ) {
                        diagnostics.push(rule_skipped(
                            rule,
                            Some(item),
                            "selected relation value is unavailable",
                            "relation",
                        ));
                        continue;
                    }
                    if !rule_count_satisfies(count, configuration.count()) {
                        diagnostics.push(rule_failure(
                            rule,
                            item,
                            count,
                            *configuration.count().min().value(),
                            configuration.count().maximum(),
                            None,
                            Some(relations),
                        ));
                    }
                }
                RuleConfiguration::Orphan(configuration) => {
                    let relations = sorted_schema_values(configuration.relations().value());
                    let node = NodeRef::item(item.mid().clone());
                    let connected = graph
                        .expect("orphan rules were skipped when the graph was unavailable")
                        .edges()
                        .iter()
                        .any(|edge| {
                            relations.iter().any(|relation| relation == edge.relation())
                                && (edge.source() == &node || edge.target() == &node)
                        });
                    if !connected
                        && relations.iter().any(|relation| {
                            item_relation_prerequisite_unavailable(
                                item,
                                relation,
                                RuleDirection::Outgoing,
                                prerequisite_diagnostics,
                            ) || item_relation_prerequisite_unavailable(
                                item,
                                relation,
                                RuleDirection::Incoming,
                                prerequisite_diagnostics,
                            )
                        })
                    {
                        diagnostics.push(rule_skipped(
                            rule,
                            Some(item),
                            "configured connectivity is unavailable",
                            "relation",
                        ));
                        continue;
                    }
                    if !connected {
                        diagnostics.push(rule_failure(
                            rule,
                            item,
                            0,
                            1,
                            CardinalityMaximum::Many,
                            None,
                            Some(relations),
                        ));
                    }
                }
            }
        }
    }
}

enum ConditionResult {
    Matches,
    DoesNotMatch,
    Unavailable,
}

fn condition_matches(
    rule: &RuleDefinition,
    item: &NormalizedItem,
    prerequisite_diagnostics: &[Diagnostic],
) -> ConditionResult {
    let Some(condition) = rule.condition() else {
        return ConditionResult::Matches;
    };
    let field = condition.value().field().value();
    if item_field_prerequisite_unavailable(item, field, prerequisite_diagnostics) {
        return ConditionResult::Unavailable;
    }
    let Some(values) = item.fields().get(field) else {
        return ConditionResult::DoesNotMatch;
    };
    if values.len() != 1 {
        return ConditionResult::Unavailable;
    }
    if condition
        .value()
        .values()
        .value()
        .iter()
        .any(|candidate| condition_value_matches(values[0].value(), candidate.value()))
    {
        ConditionResult::Matches
    } else {
        ConditionResult::DoesNotMatch
    }
}

fn condition_value_matches(value: &NormalizedScalar, condition: &RuleConditionValue) -> bool {
    match (value, condition) {
        (
            NormalizedScalar::String(left) | NormalizedScalar::Enum(left),
            RuleConditionValue::String(right),
        ) => left == right,
        (NormalizedScalar::Integer(left), RuleConditionValue::Integer(right)) => left == right,
        (NormalizedScalar::Number(left), RuleConditionValue::Number(right)) => {
            left.get() == right.get()
        }
        (NormalizedScalar::Boolean(left), RuleConditionValue::Boolean(right)) => left == right,
        _ => false,
    }
}

fn rule_applies_to(rule: &RuleDefinition, item: &NormalizedItem) -> bool {
    rule.applies_to()
        .value()
        .flavours()
        .value()
        .iter()
        .any(|flavour| flavour.value() == item.flavour())
}

fn rule_count_satisfies(count: u64, rule: &crate::RuleCount) -> bool {
    count >= *rule.min().value()
        && match rule.maximum() {
            CardinalityMaximum::Bounded(maximum) => count <= maximum,
            CardinalityMaximum::Many => true,
        }
}

#[allow(clippy::too_many_arguments)]
fn rule_failure(
    rule: &RuleDefinition,
    item: &NormalizedItem,
    actual: u64,
    minimum: u64,
    maximum: CardinalityMaximum,
    fields: Option<Vec<String>>,
    relations: Option<Vec<String>>,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::rule_failure(
        rule_severity(*rule.severity().value()),
        format!("configured rule {:?} failed", rule.name().value()),
        Some(item.header_source().clone()),
    )
    .with_related(RelatedDiagnostic::new(
        "rule declared here",
        rule.source().clone(),
    ))
    .with_item(diagnostic_item(item))
    .with_detail("actual", DiagnosticValue::Unsigned(actual))
    .with_detail("flavour", item.flavour())
    .with_detail("kind", rule.kind().value().as_str())
    .with_detail("maximum", maximum_detail(maximum))
    .with_detail("minimum", DiagnosticValue::Unsigned(minimum))
    .with_detail("rule", rule.name().value().as_str());
    if let Some(fields) = fields {
        diagnostic = diagnostic
            .with_context(DiagnosticContext::new(
                (fields.len() == 1).then(|| fields[0].clone()),
                None,
                None,
            ))
            .with_detail("fields", string_array(fields));
    }
    if let Some(relations) = relations {
        diagnostic = diagnostic
            .with_context(DiagnosticContext::new(
                None,
                (relations.len() == 1).then(|| relations[0].clone()),
                None,
            ))
            .with_detail("relations", string_array(relations));
    }
    diagnostic
}

fn rule_skipped(
    rule: &RuleDefinition,
    item: Option<&NormalizedItem>,
    reason: &str,
    prerequisite: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(
        RuleDiagnosticCode::Skipped,
        format!("configured rule {:?} was skipped", rule.name().value()),
        item.map_or_else(
            || Some(rule.source().clone()),
            |item| Some(item.header_source().clone()),
        ),
    )
    .with_related(RelatedDiagnostic::new(
        "rule declared here",
        rule.source().clone(),
    ))
    .with_detail("kind", rule.kind().value().as_str())
    .with_detail("prerequisite", prerequisite)
    .with_detail("reason", reason)
    .with_detail("rule", rule.name().value().as_str());
    diagnostic = diagnostic.with_context(rule_context(rule));
    if let Some(item) = item {
        diagnostic = diagnostic
            .with_item(diagnostic_item(item))
            .with_detail("flavour", item.flavour());
    }
    diagnostic
}

fn selected_fields(selection: &FieldRuleSelection) -> Vec<String> {
    match selection {
        FieldRuleSelection::Field(field) => vec![field.value().clone()],
        FieldRuleSelection::AnyOf(fields) => sorted_schema_values(fields.value()),
    }
}

fn selected_relations(selection: &RelationRuleSelection) -> Vec<String> {
    match selection {
        RelationRuleSelection::Relation(relation) => vec![relation.value().clone()],
        RelationRuleSelection::AnyOf(relations) => sorted_schema_values(relations.value()),
    }
}

fn sorted_schema_values(values: &[crate::SchemaValue<String>]) -> Vec<String> {
    let mut values = values
        .iter()
        .map(|value| value.value().clone())
        .collect::<Vec<_>>();
    values.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    values
}

fn item_field_prerequisite_unavailable(
    item: &NormalizedItem,
    field: &str,
    diagnostics: &[Diagnostic],
) -> bool {
    diagnostics.iter().any(|diagnostic| {
        diagnostic_belongs_to_item(diagnostic, item)
            && diagnostic.context().field() == Some(field)
            && matches!(
                diagnostic.code(),
                DiagnosticCode::Field(
                    FieldDiagnosticCode::InvalidScalar
                        | FieldDiagnosticCode::InvalidEnum
                        | FieldDiagnosticCode::PatternMismatch
                        | FieldDiagnosticCode::Repetition
                )
            )
    })
}

fn item_relation_prerequisite_unavailable(
    item: &NormalizedItem,
    relation: &str,
    direction: RuleDirection,
    diagnostics: &[Diagnostic],
) -> bool {
    diagnostics.iter().any(|diagnostic| {
        diagnostic_belongs_to_item(diagnostic, item)
            && (diagnostic.context().relation() == Some(relation)
                || diagnostic.details().get("canonical_relation")
                    == Some(&DiagnosticValue::String(relation.to_owned())))
            && diagnostic_direction_matches(diagnostic, direction)
            && diagnostic.severity() == DiagnosticSeverity::Error
            && matches!(
                diagnostic.code(),
                DiagnosticCode::Reference(
                    ReferenceDiagnosticCode::Unresolved
                        | ReferenceDiagnosticCode::Ambiguous
                        | ReferenceDiagnosticCode::ExternalScheme
                ) | DiagnosticCode::Relation(_)
            )
    })
}

fn rule_context(rule: &RuleDefinition) -> DiagnosticContext {
    match rule.configuration() {
        RuleConfiguration::RequiresField(configuration) => DiagnosticContext::new(
            match configuration.fields() {
                FieldRuleSelection::Field(field) => Some(field.value().clone()),
                FieldRuleSelection::AnyOf(_) => None,
            },
            None,
            None,
        ),
        RuleConfiguration::RequiresRelation(configuration) => DiagnosticContext::new(
            None,
            match configuration.relations() {
                RelationRuleSelection::Relation(relation) => Some(relation.value().clone()),
                RelationRuleSelection::AnyOf(_) => None,
            },
            None,
        ),
        RuleConfiguration::Orphan(configuration) => DiagnosticContext::new(
            None,
            (configuration.relations().value().len() == 1)
                .then(|| configuration.relations().value()[0].value().clone()),
            None,
        ),
    }
}

fn node_relation_prerequisite_unavailable(
    node: &NodeRef,
    relation: &str,
    direction: RuleDirection,
    diagnostics: &[Diagnostic],
) -> bool {
    let Some(mid) = node.mid() else {
        return false;
    };
    diagnostics.iter().any(|diagnostic| {
        diagnostic.item().is_some_and(|item| item.mid() == mid)
            && (diagnostic.context().relation() == Some(relation)
                || diagnostic.details().get("canonical_relation")
                    == Some(&DiagnosticValue::String(relation.to_owned())))
            && diagnostic_direction_matches(diagnostic, direction)
            && diagnostic.severity() == DiagnosticSeverity::Error
    })
}

fn diagnostic_belongs_to_item(diagnostic: &Diagnostic, item: &NormalizedItem) -> bool {
    diagnostic
        .item()
        .is_some_and(|candidate| candidate.mid() == item.mid())
        && diagnostic
            .primary()
            .is_some_and(|primary| span_is_within(primary, item.source()))
}

fn span_is_within(candidate: &SourceSpan, container: &SourceSpan) -> bool {
    candidate.path() == container.path()
        && candidate.start_byte() >= container.start_byte()
        && candidate.end_byte() <= container.end_byte()
}

fn diagnostic_direction_matches(diagnostic: &Diagnostic, direction: RuleDirection) -> bool {
    diagnostic
        .details()
        .get("canonical_direction")
        .is_none_or(|candidate| {
            candidate == &DiagnosticValue::String(direction.as_str().to_owned())
        })
}

fn count_outcome_uncertain(count: u64, minimum: u64, maximum: CardinalityMaximum) -> bool {
    if count < minimum {
        return true;
    }
    match maximum {
        CardinalityMaximum::Bounded(maximum) => count <= maximum,
        CardinalityMaximum::Many => false,
    }
}

fn node_item<'a>(items: &'a [NormalizedItem], node: &NodeRef) -> Option<&'a NormalizedItem> {
    let mid = node.mid()?;
    items.iter().find(|item| item.mid() == mid)
}

fn node_primary(items: &[NormalizedItem], node: &NodeRef) -> Option<SourceSpan> {
    match node {
        NodeRef::Item { .. } => node_item(items, node).map(|item| item.header_source().clone()),
        NodeRef::SourceSpan { source, .. } => Some(source.clone()),
        NodeRef::External { .. } => None,
    }
}

fn node_target(node: &NodeRef) -> Option<String> {
    match node {
        NodeRef::External { uri } => Some(uri.clone()),
        NodeRef::Item { mid } => Some(mid.to_string()),
        NodeRef::SourceSpan { .. } => None,
    }
}

fn node_identity(node: &NodeRef) -> String {
    match node {
        NodeRef::Item { mid } => mid.to_string(),
        NodeRef::External { uri } => uri.clone(),
        NodeRef::SourceSpan { source, symbol } => format!(
            "{}:{}-{}{}",
            source.path(),
            source.start_byte(),
            source.end_byte(),
            symbol
                .as_deref()
                .map_or_else(String::new, |symbol| format!("#{symbol}"))
        ),
    }
}

fn diagnostic_item(item: &NormalizedItem) -> DiagnosticItem {
    DiagnosticItem::new(
        item.mid().clone(),
        item.display_id()
            .map(|display_id| display_id.value().clone()),
    )
}

fn maximum_detail(maximum: CardinalityMaximum) -> DiagnosticValue {
    match maximum {
        CardinalityMaximum::Bounded(value) => DiagnosticValue::Unsigned(value),
        CardinalityMaximum::Many => DiagnosticValue::String("many".to_owned()),
    }
}

fn string_array(values: Vec<String>) -> DiagnosticValue {
    DiagnosticValue::Array(values.into_iter().map(DiagnosticValue::String).collect())
}

const fn rule_severity(severity: RuleSeverity) -> DiagnosticSeverity {
    match severity {
        RuleSeverity::Error => DiagnosticSeverity::Error,
        RuleSeverity::Warning => DiagnosticSeverity::Warning,
        RuleSeverity::Info => DiagnosticSeverity::Info,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_phase_fixture_and_severity_policy_are_deterministic() {
        let phases = ValidationPhase::ALL
            .into_iter()
            .map(|phase| {
                let state = match phase {
                    ValidationPhase::Content => {
                        ValidationPhaseState::skipped("documents were supplied in memory", None)
                    }
                    ValidationPhase::Graph => ValidationPhaseState::skipped(
                        "semantic model unavailable",
                        Some(ValidationPhase::Semantic),
                    ),
                    _ => ValidationPhaseState::Completed,
                };
                ValidationPhaseResult::new(phase, state)
            })
            .collect::<Vec<_>>();

        assert_eq!(
            phases
                .iter()
                .map(|result| result.phase().as_str())
                .collect::<Vec<_>>(),
            vec![
                "project", "schema", "content", "parse", "semantic", "graph", "rules"
            ]
        );
        assert_eq!(
            phases[5].state().prerequisite(),
            Some(ValidationPhase::Semantic)
        );

        let diagnostics = vec![
            Diagnostic::new(crate::RelationDiagnosticCode::Duplicate, "duplicate", None),
            Diagnostic::new(crate::RuleDiagnosticCode::Skipped, "skipped", None),
        ];
        let counts = SeverityCounts::from_diagnostics(&diagnostics);
        assert_eq!(
            (counts.errors(), counts.warnings(), counts.info()),
            (0, 1, 1)
        );
        assert!(counts.is_valid(false));
        assert!(!counts.is_valid(true));
        assert_eq!(diagnostics[0].severity(), DiagnosticSeverity::Warning);
    }
}
