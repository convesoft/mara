use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use crate::{
    Diagnostic, DiagnosticContext, DiagnosticItem, DiagnosticValue, IdentityDiagnosticCode, Mid,
    MidIdentity, ReferenceDiagnosticCode, RelatedDiagnostic, SourceSpan, sort_diagnostics,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenanced<T> {
    value: T,
    source: SourceSpan,
}

impl<T> Provenanced<T> {
    pub fn new(value: T, source: SourceSpan) -> Self {
        Self { value, source }
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct NormalizedNumber(f64);

impl NormalizedNumber {
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self(value))
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}

impl Eq for NormalizedNumber {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedScalar {
    String(String),
    Integer(i64),
    Number(NormalizedNumber),
    Boolean(bool),
    Enum(String),
}

pub type NormalizedFieldValue = Provenanced<NormalizedScalar>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceOrigin {
    Item {
        mid: Mid,
        display_id: Option<String>,
    },
    Narrative(SourceSpan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthoredReferenceSyntax {
    Inline,
    Metadata,
    Narrative,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredReference {
    target: String,
    label: Option<String>,
    relation: Option<String>,
    origin: ReferenceOrigin,
    syntax: AuthoredReferenceSyntax,
    source: SourceSpan,
    target_source: SourceSpan,
}

impl AuthoredReference {
    pub fn new(
        target: String,
        label: Option<String>,
        relation: Option<String>,
        origin: ReferenceOrigin,
        source: SourceSpan,
    ) -> Self {
        let syntax = match origin {
            ReferenceOrigin::Item { .. } => AuthoredReferenceSyntax::Inline,
            ReferenceOrigin::Narrative(_) => AuthoredReferenceSyntax::Narrative,
        };
        Self {
            target,
            label,
            relation,
            origin,
            syntax,
            target_source: source.clone(),
            source,
        }
    }

    pub fn with_target_source(mut self, target_source: SourceSpan) -> Self {
        self.target_source = target_source;
        self
    }

    pub fn with_syntax(mut self, syntax: AuthoredReferenceSyntax) -> Self {
        self.syntax = syntax;
        self
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn relation(&self) -> Option<&str> {
        self.relation.as_deref()
    }

    pub const fn origin(&self) -> &ReferenceOrigin {
        &self.origin
    }

    pub const fn syntax(&self) -> AuthoredReferenceSyntax {
        self.syntax
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn target_source(&self) -> &SourceSpan {
        &self.target_source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedReference {
    target: Mid,
    authored: AuthoredReference,
}

impl ResolvedReference {
    pub fn new(target: Mid, authored: AuthoredReference) -> Self {
        Self { target, authored }
    }

    pub const fn target(&self) -> &Mid {
        &self.target
    }

    pub const fn authored(&self) -> &AuthoredReference {
        &self.authored
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthoredRelationOrigin {
    Direct,
    InverseNormalized,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationOccurrence {
    reference: ResolvedReference,
    origin: AuthoredRelationOrigin,
}

impl RelationOccurrence {
    pub fn new(reference: ResolvedReference, origin: AuthoredRelationOrigin) -> Self {
        Self { reference, origin }
    }

    pub const fn reference(&self) -> &ResolvedReference {
        &self.reference
    }

    pub const fn origin(&self) -> AuthoredRelationOrigin {
        self.origin
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalRelationKey {
    relation: String,
    source: Mid,
    target: Mid,
}

impl CanonicalRelationKey {
    pub fn new(relation: String, source: Mid, target: Mid) -> Self {
        Self {
            relation,
            source,
            target,
        }
    }

    pub fn relation(&self) -> &str {
        &self.relation
    }

    pub const fn source(&self) -> &Mid {
        &self.source
    }

    pub const fn target(&self) -> &Mid {
        &self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalRelationInput {
    relation: String,
    source: Mid,
    target: Mid,
    occurrence: RelationOccurrence,
    inverse_relation: Option<String>,
    symmetric: bool,
}

impl CanonicalRelationInput {
    pub fn new(relation: String, source: Mid, target: Mid, occurrence: RelationOccurrence) -> Self {
        Self {
            relation,
            source,
            target,
            occurrence,
            inverse_relation: None,
            symmetric: false,
        }
    }

    pub fn with_inverse_relation(mut self, inverse_relation: Option<String>) -> Self {
        self.inverse_relation = inverse_relation;
        self
    }

    pub fn with_symmetric(mut self, symmetric: bool) -> Self {
        self.symmetric = symmetric;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalRelationEdge {
    key: CanonicalRelationKey,
    occurrences: Vec<RelationOccurrence>,
}

impl CanonicalRelationEdge {
    pub const fn key(&self) -> &CanonicalRelationKey {
        &self.key
    }

    pub fn relation(&self) -> &str {
        self.key.relation()
    }

    pub const fn source(&self) -> &Mid {
        self.key.source()
    }

    pub const fn target(&self) -> &Mid {
        self.key.target()
    }

    pub fn occurrences(&self) -> &[RelationOccurrence] {
        &self.occurrences
    }

    pub fn duplicate_metadata_occurrences(&self) -> Vec<&RelationOccurrence> {
        let mut seen = BTreeSet::new();
        self.occurrences
            .iter()
            .filter(|occurrence| {
                let authored = occurrence.reference().authored();
                authored.syntax() == AuthoredReferenceSyntax::Metadata
                    && !seen.insert((
                        occurrence.origin(),
                        authored.relation().map(str::to_owned),
                        authored.target().to_owned(),
                    ))
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeakMention {
    reference: ResolvedReference,
}

impl WeakMention {
    pub fn new(reference: ResolvedReference) -> Self {
        Self { reference }
    }

    pub const fn reference(&self) -> &ResolvedReference {
        &self.reference
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DerivedRelationOrigin {
    Backlink,
    Inverse,
    Symmetric,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedRelationView {
    relation: String,
    source: Mid,
    target: Mid,
    canonical: CanonicalRelationKey,
    origin: DerivedRelationOrigin,
}

impl DerivedRelationView {
    pub fn relation(&self) -> &str {
        &self.relation
    }

    pub const fn source(&self) -> &Mid {
        &self.source
    }

    pub const fn target(&self) -> &Mid {
        &self.target
    }

    pub const fn canonical(&self) -> &CanonicalRelationKey {
        &self.canonical
    }

    pub const fn origin(&self) -> DerivedRelationOrigin {
        self.origin
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalRelations {
    edges: Vec<CanonicalRelationEdge>,
    weak_mentions: Vec<WeakMention>,
    derived_views: Vec<DerivedRelationView>,
}

impl CanonicalRelations {
    pub fn build(
        inputs: impl IntoIterator<Item = CanonicalRelationInput>,
        weak_mentions: impl IntoIterator<Item = WeakMention>,
    ) -> Self {
        let mut grouped = BTreeMap::<
            CanonicalRelationKey,
            (Vec<RelationOccurrence>, BTreeSet<RelationViewSpecification>),
        >::new();
        for input in inputs {
            let (source, target) = if input.symmetric && input.target < input.source {
                (input.target, input.source)
            } else {
                (input.source, input.target)
            };
            let key = CanonicalRelationKey::new(input.relation, source, target);
            let (occurrences, views) = grouped.entry(key).or_default();
            occurrences.push(input.occurrence);
            views.insert(RelationViewSpecification::Backlink);
            if let Some(inverse) = input.inverse_relation {
                views.insert(RelationViewSpecification::Inverse(inverse));
            }
            if input.symmetric {
                views.insert(RelationViewSpecification::Symmetric);
            }
        }

        let mut edges = Vec::with_capacity(grouped.len());
        let mut derived_views = Vec::new();
        for (key, (mut occurrences, views)) in grouped {
            occurrences.sort_by(compare_relation_occurrences);
            for view in views {
                derived_views.push(view.materialize(&key));
            }
            edges.push(CanonicalRelationEdge { key, occurrences });
        }
        derived_views.sort_by(compare_derived_views);

        let mut weak_mentions = weak_mentions.into_iter().collect::<Vec<_>>();
        weak_mentions.sort_by(|left, right| {
            compare_resolved_references(left.reference(), right.reference())
        });

        Self {
            edges,
            weak_mentions,
            derived_views,
        }
    }

    pub fn edges(&self) -> &[CanonicalRelationEdge] {
        &self.edges
    }

    pub fn weak_mentions(&self) -> &[WeakMention] {
        &self.weak_mentions
    }

    pub fn derived_views(&self) -> &[DerivedRelationView] {
        &self.derived_views
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum RelationViewSpecification {
    Backlink,
    Inverse(String),
    Symmetric,
}

impl RelationViewSpecification {
    fn materialize(self, canonical: &CanonicalRelationKey) -> DerivedRelationView {
        let (relation, origin) = match self {
            Self::Backlink => (canonical.relation.clone(), DerivedRelationOrigin::Backlink),
            Self::Inverse(relation) => (relation, DerivedRelationOrigin::Inverse),
            Self::Symmetric => (canonical.relation.clone(), DerivedRelationOrigin::Symmetric),
        };
        DerivedRelationView {
            relation,
            source: canonical.target.clone(),
            target: canonical.source.clone(),
            canonical: canonical.clone(),
            origin,
        }
    }
}

fn compare_relation_occurrences(left: &RelationOccurrence, right: &RelationOccurrence) -> Ordering {
    compare_resolved_references(left.reference(), right.reference())
        .then_with(|| left.origin().cmp(&right.origin()))
}

fn compare_resolved_references(left: &ResolvedReference, right: &ResolvedReference) -> Ordering {
    compare_spans(left.authored().source(), right.authored().source())
        .then_with(|| left.target().cmp(right.target()))
        .then_with(|| left.authored().target().cmp(right.authored().target()))
        .then_with(|| left.authored().relation().cmp(&right.authored().relation()))
        .then_with(|| left.authored().label().cmp(&right.authored().label()))
        .then_with(|| left.authored().syntax().cmp(&right.authored().syntax()))
}

fn compare_derived_views(left: &DerivedRelationView, right: &DerivedRelationView) -> Ordering {
    left.relation
        .as_bytes()
        .cmp(right.relation.as_bytes())
        .then_with(|| left.source.cmp(&right.source))
        .then_with(|| left.target.cmp(&right.target))
        .then_with(|| left.origin.cmp(&right.origin))
        .then_with(|| left.canonical.cmp(&right.canonical))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedItem {
    mid: Mid,
    flavour: String,
    display_id: Option<Provenanced<String>>,
    title: Option<Provenanced<String>>,
    body: Provenanced<String>,
    fields: BTreeMap<String, Vec<NormalizedFieldValue>>,
    authored_references: Vec<AuthoredReference>,
    resolved_references: Vec<ResolvedReference>,
    source: SourceSpan,
    header_source: SourceSpan,
}

impl NormalizedItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mid: Mid,
        flavour: String,
        display_id: Option<Provenanced<String>>,
        title: Option<Provenanced<String>>,
        body: Provenanced<String>,
        fields: BTreeMap<String, Vec<NormalizedFieldValue>>,
        authored_references: Vec<AuthoredReference>,
        source: SourceSpan,
        header_source: SourceSpan,
    ) -> Self {
        Self {
            mid,
            flavour,
            display_id,
            title,
            body,
            fields,
            authored_references,
            resolved_references: Vec::new(),
            source,
            header_source,
        }
    }

    pub const fn mid(&self) -> &Mid {
        &self.mid
    }

    pub fn flavour(&self) -> &str {
        &self.flavour
    }

    pub const fn display_id(&self) -> Option<&Provenanced<String>> {
        self.display_id.as_ref()
    }

    pub const fn title(&self) -> Option<&Provenanced<String>> {
        self.title.as_ref()
    }

    pub const fn body(&self) -> &Provenanced<String> {
        &self.body
    }

    pub const fn fields(&self) -> &BTreeMap<String, Vec<NormalizedFieldValue>> {
        &self.fields
    }

    pub fn authored_references(&self) -> &[AuthoredReference] {
        &self.authored_references
    }

    pub fn resolved_references(&self) -> &[ResolvedReference] {
        &self.resolved_references
    }

    pub fn set_resolved_references(&mut self, references: Vec<ResolvedReference>) {
        self.resolved_references = references;
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn header_source(&self) -> &SourceSpan {
        &self.header_source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityRecord {
    mid: Mid,
    display_id: Option<Provenanced<String>>,
    display_id_active: bool,
    header_source: SourceSpan,
}

impl IdentityRecord {
    pub fn new(
        mid: Mid,
        display_id: Option<Provenanced<String>>,
        header_source: SourceSpan,
    ) -> Self {
        Self {
            mid,
            display_id,
            display_id_active: true,
            header_source,
        }
    }

    pub fn with_active_display_id(mut self, active: bool) -> Self {
        self.display_id_active = active;
        self
    }

    pub const fn mid(&self) -> &Mid {
        &self.mid
    }

    pub const fn display_id(&self) -> Option<&Provenanced<String>> {
        self.display_id.as_ref()
    }

    pub const fn display_id_is_active(&self) -> bool {
        self.display_id.is_some() && self.display_id_active
    }

    pub const fn header_source(&self) -> &SourceSpan {
        &self.header_source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityCandidate {
    mid: Mid,
    header_source: SourceSpan,
}

impl IdentityCandidate {
    pub const fn mid(&self) -> &Mid {
        &self.mid
    }

    pub const fn header_source(&self) -> &SourceSpan {
        &self.header_source
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentityIndex {
    mids: BTreeMap<Mid, Vec<IdentityCandidate>>,
    display_ids: BTreeMap<String, Vec<IdentityCandidate>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityIndexBuild {
    index: IdentityIndex,
    diagnostics: Vec<Diagnostic>,
}

impl IdentityIndexBuild {
    pub const fn index(&self) -> &IdentityIndex {
        &self.index
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn into_parts(self) -> (IdentityIndex, Vec<Diagnostic>) {
        (self.index, self.diagnostics)
    }
}

impl IdentityIndex {
    pub fn build(records: &[IdentityRecord]) -> IdentityIndexBuild {
        let mut index = Self::default();
        let mut all_display_ids = BTreeMap::<String, Vec<IdentityCandidate>>::new();
        for record in records {
            let candidate = IdentityCandidate {
                mid: record.mid.clone(),
                header_source: record.header_source.clone(),
            };
            index
                .mids
                .entry(record.mid.clone())
                .or_default()
                .push(candidate.clone());
            if let Some(display_id) = record.display_id() {
                all_display_ids
                    .entry(display_id.value().clone())
                    .or_default()
                    .push(candidate.clone());
                if record.display_id_is_active() {
                    index
                        .display_ids
                        .entry(display_id.value().clone())
                        .or_default()
                        .push(candidate);
                }
            }
        }
        for candidates in index.mids.values_mut() {
            sort_candidates(candidates);
        }
        for candidates in index.display_ids.values_mut() {
            sort_candidates(candidates);
        }
        for candidates in all_display_ids.values_mut() {
            sort_candidates(candidates);
        }

        let mut diagnostics = duplicate_diagnostics(&index, &all_display_ids, records);
        sort_diagnostics(&mut diagnostics);
        IdentityIndexBuild { index, diagnostics }
    }

    pub const fn mids(&self) -> &BTreeMap<Mid, Vec<IdentityCandidate>> {
        &self.mids
    }

    pub const fn display_ids(&self) -> &BTreeMap<String, Vec<IdentityCandidate>> {
        &self.display_ids
    }

    pub fn resolve(
        &self,
        reference: &AuthoredReference,
        identity: &MidIdentity,
    ) -> Result<ResolvedReference, Box<Diagnostic>> {
        let candidates = match Mid::parse(reference.target(), identity) {
            Ok(mid) => self.mids.get(&mid),
            Err(_) => self.display_ids.get(reference.target()),
        };
        match candidates {
            Some(candidates) if candidates.len() == 1 => Ok(ResolvedReference::new(
                candidates[0].mid.clone(),
                reference.clone(),
            )),
            Some(candidates) if !candidates.is_empty() => {
                Err(Box::new(ambiguous_reference(reference, candidates)))
            }
            _ => Err(Box::new(unresolved_reference(reference))),
        }
    }
}

fn sort_candidates(candidates: &mut [IdentityCandidate]) {
    candidates.sort_by(|left, right| {
        left.mid
            .as_str()
            .as_bytes()
            .cmp(right.mid.as_str().as_bytes())
            .then_with(|| compare_spans(&left.header_source, &right.header_source))
    });
}

fn compare_spans(left: &SourceSpan, right: &SourceSpan) -> Ordering {
    left.path()
        .as_bytes()
        .cmp(right.path().as_bytes())
        .then_with(|| left.start_byte().cmp(&right.start_byte()))
        .then_with(|| left.end_byte().cmp(&right.end_byte()))
}

fn duplicate_diagnostics(
    index: &IdentityIndex,
    all_display_ids: &BTreeMap<String, Vec<IdentityCandidate>>,
    records: &[IdentityRecord],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (mid, candidates) in &index.mids {
        if candidates.len() > 1 {
            let mut diagnostic = Diagnostic::new(
                IdentityDiagnosticCode::DuplicateMid,
                "machine identity is used by more than one item",
                None,
            )
            .with_detail("mid", mid.as_str());
            for candidate in candidates {
                diagnostic = diagnostic.with_related(RelatedDiagnostic::new(
                    "item with duplicate machine identity",
                    candidate.header_source.clone(),
                ));
            }
            diagnostics.push(diagnostic);
        }
    }
    for (display_id, candidates) in all_display_ids {
        if candidates.len() > 1 {
            let primary = records
                .iter()
                .filter_map(IdentityRecord::display_id)
                .filter(|candidate| candidate.value() == display_id)
                .min_by(|left, right| compare_spans(left.source(), right.source()))
                .map(|candidate| candidate.source().clone());
            let mut diagnostic = Diagnostic::new(
                IdentityDiagnosticCode::DuplicateDisplayId,
                "display ID is used by more than one item",
                primary,
            )
            .with_detail("display_id", display_id.clone())
            .with_detail(
                "candidate_mids",
                DiagnosticValue::Array(
                    candidates
                        .iter()
                        .map(|candidate| DiagnosticValue::String(candidate.mid.to_string()))
                        .collect(),
                ),
            );
            for candidate in candidates {
                diagnostic = diagnostic.with_related(RelatedDiagnostic::new(
                    format!("display ID belongs to {}", candidate.mid),
                    candidate.header_source.clone(),
                ));
            }
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

fn unresolved_reference(reference: &AuthoredReference) -> Diagnostic {
    decorate_reference_diagnostic(
        Diagnostic::new(
            ReferenceDiagnosticCode::Unresolved,
            "internal reference does not resolve to an active item",
            Some(reference.source.clone()),
        )
        .with_detail("reference", reference.target.clone()),
        reference,
    )
}

fn ambiguous_reference(
    reference: &AuthoredReference,
    candidates: &[IdentityCandidate],
) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(
        ReferenceDiagnosticCode::Ambiguous,
        "internal reference matches more than one active item",
        Some(reference.source.clone()),
    )
    .with_detail(
        "candidate_mids",
        DiagnosticValue::Array(
            candidates
                .iter()
                .map(|candidate| DiagnosticValue::String(candidate.mid.to_string()))
                .collect(),
        ),
    )
    .with_detail("reference", reference.target.clone());
    for candidate in candidates {
        diagnostic = diagnostic.with_related(RelatedDiagnostic::new(
            format!("candidate {}", candidate.mid),
            candidate.header_source.clone(),
        ));
    }
    decorate_reference_diagnostic(diagnostic, reference)
}

fn decorate_reference_diagnostic(
    mut diagnostic: Diagnostic,
    reference: &AuthoredReference,
) -> Diagnostic {
    if let ReferenceOrigin::Item { mid, display_id } = reference.origin() {
        diagnostic = diagnostic.with_item(DiagnosticItem::new(mid.clone(), display_id.clone()));
    }
    diagnostic.with_context(DiagnosticContext::new(
        None,
        reference.relation.clone(),
        Some(reference.target.clone()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MidFormat, SchemaField, SourceIndex};

    fn identity() -> MidIdentity {
        let span = span("schema.yaml", "x", 0, 1);
        MidIdentity::new(
            SchemaField::new(span.clone(), span.clone(), MidFormat::Ulid),
            SchemaField::new(span.clone(), span, "m_".to_owned()),
        )
    }

    fn mid(suffix: &str) -> Mid {
        Mid::parse(&format!("m_{suffix}"), &identity()).unwrap()
    }

    fn span(path: &str, source: &str, start: u64, end: u64) -> SourceSpan {
        let index = SourceIndex::try_new(path, source).unwrap();
        let (start_line, start_column) = index.coordinates_at(start).unwrap();
        let (end_line, end_column) = index.coordinates_at(end).unwrap();
        index
            .try_span(start, end, start_line, start_column, end_line, end_column)
            .unwrap()
    }

    fn record(mid: Mid, id: Option<&str>, path: &str) -> IdentityRecord {
        let source = "item";
        let source_span = span(path, source, 0, source.len() as u64);
        IdentityRecord::new(
            mid,
            id.map(|id| Provenanced::new(id.to_owned(), source_span.clone())),
            source_span,
        )
    }

    fn reference(target: &str) -> AuthoredReference {
        AuthoredReference::new(
            target.to_owned(),
            None,
            None,
            ReferenceOrigin::Narrative(span("ref.md", "ref", 0, 3)),
            span("ref.md", "ref", 0, 3),
        )
    }

    fn relation_input(
        relation: &str,
        source: Mid,
        target: Mid,
        path: &str,
        authored_relation: &str,
        origin: AuthoredRelationOrigin,
    ) -> CanonicalRelationInput {
        let reference_source = span(path, "ref", 0, 3);
        let (authored_source, authored_target) = match origin {
            AuthoredRelationOrigin::Direct => (source.clone(), target.clone()),
            AuthoredRelationOrigin::InverseNormalized => (target.clone(), source.clone()),
        };
        let authored = AuthoredReference::new(
            authored_target.to_string(),
            None,
            Some(authored_relation.to_owned()),
            ReferenceOrigin::Item {
                mid: authored_source,
                display_id: None,
            },
            reference_source,
        )
        .with_syntax(AuthoredReferenceSyntax::Metadata);
        CanonicalRelationInput::new(
            relation.to_owned(),
            source,
            target,
            RelationOccurrence::new(ResolvedReference::new(authored_target, authored), origin),
        )
    }

    #[test]
    fn exact_mid_wins_and_display_ids_are_exact_without_alias_fallback() {
        let first = mid("00000000000000000000000001");
        let second = mid("00000000000000000000000002");
        let build = IdentityIndex::build(&[
            record(first.clone(), Some("REQ-ONE"), "a.md"),
            record(second.clone(), Some(first.as_str()), "b.md"),
        ]);

        assert_eq!(
            build
                .index()
                .resolve(&reference(first.as_str()), &identity())
                .unwrap()
                .target(),
            &first
        );
        assert_eq!(
            build
                .index()
                .resolve(&reference("REQ-ONE"), &identity())
                .unwrap()
                .target(),
            &first
        );
        assert_eq!(
            build
                .index()
                .resolve(&reference("req-one"), &identity())
                .unwrap_err()
                .code(),
            ReferenceDiagnosticCode::Unresolved.into()
        );
    }

    #[test]
    fn duplicate_indexes_and_ambiguous_candidates_are_deterministic() {
        let first = mid("00000000000000000000000001");
        let second = mid("00000000000000000000000002");
        let records = vec![
            record(second.clone(), Some("DUP"), "z.md"),
            record(first.clone(), Some("DUP"), "a.md"),
            record(first, None, "b.md"),
        ];
        let build = IdentityIndex::build(&records);
        let mut reversed = records.clone();
        reversed.reverse();
        assert_eq!(build, IdentityIndex::build(&reversed));

        assert_eq!(build.diagnostics().len(), 2);
        let ambiguous = build
            .index()
            .resolve(&reference("DUP"), &identity())
            .unwrap_err();
        assert_eq!(ambiguous.code(), ReferenceDiagnosticCode::Ambiguous.into());
        assert_eq!(
            ambiguous.details().get("candidate_mids"),
            Some(&DiagnosticValue::Array(vec![
                DiagnosticValue::String("m_00000000000000000000000001".to_owned()),
                DiagnosticValue::String("m_00000000000000000000000002".to_owned()),
            ]))
        );
        assert_eq!(ambiguous.related()[0].span().path(), "a.md");
        assert_eq!(ambiguous.related()[1].span().path(), "z.md");
    }

    #[test]
    fn pure_index_output_is_independent_of_input_order() {
        let first = record(mid("00000000000000000000000001"), Some("ONE"), "a.md");
        let second = record(mid("00000000000000000000000002"), Some("TWO"), "b.md");
        assert_eq!(
            IdentityIndex::build(&[first.clone(), second.clone()]),
            IdentityIndex::build(&[second, first])
        );
    }

    #[test]
    fn inactive_display_ids_still_participate_in_duplicate_diagnostics() {
        let first = record(mid("00000000000000000000000001"), Some("invalid"), "a.md")
            .with_active_display_id(false);
        let second = record(mid("00000000000000000000000002"), Some("invalid"), "b.md")
            .with_active_display_id(false);
        let build = IdentityIndex::build(&[first, second]);

        assert!(build.index().display_ids().get("invalid").is_none());
        assert!(build.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == IdentityDiagnosticCode::DuplicateDisplayId.into()
        }));
        assert_eq!(
            build
                .index()
                .resolve(&reference("invalid"), &identity())
                .unwrap_err()
                .code(),
            ReferenceDiagnosticCode::Unresolved.into()
        );
    }

    #[test]
    fn canonical_edges_deduplicate_and_preserve_sorted_authored_occurrences() {
        let source = mid("00000000000000000000000001");
        let target = mid("00000000000000000000000002");
        let direct = relation_input(
            "blocks",
            source.clone(),
            target.clone(),
            "z.md",
            "blocks",
            AuthoredRelationOrigin::Direct,
        )
        .with_inverse_relation(Some("blocked_by".to_owned()));
        let inverse = relation_input(
            "blocks",
            source.clone(),
            target.clone(),
            "a.md",
            "blocked_by",
            AuthoredRelationOrigin::InverseNormalized,
        )
        .with_inverse_relation(Some("blocked_by".to_owned()));

        let model = CanonicalRelations::build([direct.clone(), inverse.clone()], []);
        let reversed = CanonicalRelations::build([inverse, direct], []);

        assert_eq!(model, reversed);
        assert_eq!(model.edges().len(), 1);
        let edge = &model.edges()[0];
        assert_eq!(edge.relation(), "blocks");
        assert_eq!(edge.source(), &source);
        assert_eq!(edge.target(), &target);
        assert_eq!(edge.occurrences().len(), 2);
        assert_eq!(
            edge.occurrences()[0].reference().authored().source().path(),
            "a.md"
        );
        assert_eq!(
            edge.occurrences()[0].origin(),
            AuthoredRelationOrigin::InverseNormalized
        );
        assert_eq!(
            edge.occurrences()[1].reference().authored().source().path(),
            "z.md"
        );
    }

    #[test]
    fn symmetric_identity_and_all_derived_views_are_explicit_and_deterministic() {
        let first = mid("00000000000000000000000001");
        let second = mid("00000000000000000000000002");
        let input = relation_input(
            "related_to",
            second.clone(),
            first.clone(),
            "relation.md",
            "related_to",
            AuthoredRelationOrigin::Direct,
        )
        .with_symmetric(true);

        let model = CanonicalRelations::build([input], []);

        assert_eq!(model.edges()[0].source(), &first);
        assert_eq!(model.edges()[0].target(), &second);
        assert_eq!(model.derived_views().len(), 2);
        assert_eq!(
            model
                .derived_views()
                .iter()
                .map(DerivedRelationView::origin)
                .collect::<Vec<_>>(),
            vec![
                DerivedRelationOrigin::Backlink,
                DerivedRelationOrigin::Symmetric,
            ]
        );
        assert!(
            model
                .derived_views()
                .iter()
                .all(|view| view.canonical() == model.edges()[0].key())
        );
    }

    #[test]
    fn exact_metadata_duplicates_are_reported_without_dropping_provenance() {
        let source = mid("00000000000000000000000001");
        let target = mid("00000000000000000000000002");
        let first = relation_input(
            "blocks",
            source.clone(),
            target.clone(),
            "a.md",
            "blocks",
            AuthoredRelationOrigin::Direct,
        );
        let duplicate = relation_input(
            "blocks",
            source,
            target,
            "b.md",
            "blocks",
            AuthoredRelationOrigin::Direct,
        );

        let model = CanonicalRelations::build([duplicate, first], []);
        let edge = &model.edges()[0];

        assert_eq!(edge.occurrences().len(), 2);
        assert_eq!(edge.duplicate_metadata_occurrences().len(), 1);
        assert_eq!(
            edge.duplicate_metadata_occurrences()[0]
                .reference()
                .authored()
                .source()
                .path(),
            "b.md"
        );
    }

    #[test]
    fn weak_mentions_keep_item_and_narrative_provenance_in_source_order() {
        let source = mid("00000000000000000000000001");
        let target = mid("00000000000000000000000002");
        let item = AuthoredReference::new(
            target.to_string(),
            None,
            None,
            ReferenceOrigin::Item {
                mid: source,
                display_id: Some("REQ-ONE".to_owned()),
            },
            span("z.md", "ref", 0, 3),
        );
        let narrative = AuthoredReference::new(
            target.to_string(),
            None,
            None,
            ReferenceOrigin::Narrative(span("a.md", "ref", 0, 3)),
            span("a.md", "ref", 0, 3),
        );

        let model = CanonicalRelations::build(
            [],
            [
                WeakMention::new(ResolvedReference::new(target.clone(), item)),
                WeakMention::new(ResolvedReference::new(target, narrative)),
            ],
        );

        assert!(model.edges().is_empty());
        assert_eq!(model.weak_mentions().len(), 2);
        assert!(matches!(
            model.weak_mentions()[0].reference().authored().origin(),
            ReferenceOrigin::Narrative(_)
        ));
        assert!(matches!(
            model.weak_mentions()[1].reference().authored().origin(),
            ReferenceOrigin::Item { .. }
        ));
    }
}
