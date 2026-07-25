use std::collections::{BTreeMap, BTreeSet};

use crate::SourceSpan;

/// One decoded mapping entry with separate evidence for its key and value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaField<T> {
    key_source: SourceSpan,
    value_source: SourceSpan,
    value: T,
}

impl<T> SchemaField<T> {
    pub fn new(key_source: SourceSpan, value_source: SourceSpan, value: T) -> Self {
        Self {
            key_source,
            value_source,
            value,
        }
    }

    pub const fn key_source(&self) -> &SourceSpan {
        &self.key_source
    }

    pub const fn value_source(&self) -> &SourceSpan {
        &self.value_source
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub fn into_value(self) -> T {
        self.value
    }
}

/// One source-located value inside an ordered schema sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaValue<T> {
    source: SourceSpan,
    value: T,
}

impl<T> SchemaValue<T> {
    pub fn new(source: SourceSpan, value: T) -> Self {
        Self { source, value }
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub fn into_value(self) -> T {
        self.value
    }
}

/// Source evidence for a schema mapping or sequence represented elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaSection {
    key_source: SourceSpan,
    value_source: SourceSpan,
    len: usize,
}

impl SchemaSection {
    pub fn new(key_source: SourceSpan, value_source: SourceSpan, len: usize) -> Self {
        Self {
            key_source,
            value_source,
            len,
        }
    }

    pub const fn key_source(&self) -> &SourceSpan {
        &self.key_source
    }

    pub const fn value_source(&self) -> &SourceSpan {
        &self.value_source
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaIdentity {
    name: SchemaField<String>,
    version: SchemaField<String>,
}

impl SchemaIdentity {
    pub fn new(name: SchemaField<String>, version: SchemaField<String>) -> Self {
        Self { name, version }
    }

    pub const fn name(&self) -> &SchemaField<String> {
        &self.name
    }

    pub const fn version(&self) -> &SchemaField<String> {
        &self.version
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MidFormat {
    Ulid,
}

impl MidFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ulid => "ulid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidIdentity {
    format: SchemaField<MidFormat>,
    prefix: SchemaField<String>,
}

impl MidIdentity {
    pub fn new(format: SchemaField<MidFormat>, prefix: SchemaField<String>) -> Self {
        Self { format, prefix }
    }

    pub const fn format(&self) -> &SchemaField<MidFormat> {
        &self.format
    }

    pub const fn prefix(&self) -> &SchemaField<String> {
        &self.prefix
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityConfiguration {
    mid: SchemaField<MidIdentity>,
}

impl IdentityConfiguration {
    pub fn new(mid: SchemaField<MidIdentity>) -> Self {
        Self { mid }
    }

    pub const fn mid(&self) -> &SchemaField<MidIdentity> {
        &self.mid
    }
}

/// The closed scalar field type set supported by schema format version 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FieldType {
    String,
    Integer,
    Number,
    Boolean,
    Enum,
}

impl FieldType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Enum => "enum",
        }
    }
}

/// The source-preserving declaration for one flavour-local scalar field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDefinition {
    name: String,
    key_source: SourceSpan,
    value_source: SourceSpan,
    field_type: SchemaField<FieldType>,
    required: Option<SchemaField<bool>>,
    repeatable: Option<SchemaField<bool>>,
    values: Option<SchemaField<Vec<SchemaValue<String>>>>,
    pattern: Option<SchemaField<String>>,
}

impl FieldDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        key_source: SourceSpan,
        value_source: SourceSpan,
        field_type: SchemaField<FieldType>,
        required: Option<SchemaField<bool>>,
        repeatable: Option<SchemaField<bool>>,
        values: Option<SchemaField<Vec<SchemaValue<String>>>>,
        pattern: Option<SchemaField<String>>,
    ) -> Self {
        Self {
            name,
            key_source,
            value_source,
            field_type,
            required,
            repeatable,
            values,
            pattern,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn key_source(&self) -> &SourceSpan {
        &self.key_source
    }

    pub const fn value_source(&self) -> &SourceSpan {
        &self.value_source
    }

    pub const fn field_type(&self) -> &SchemaField<FieldType> {
        &self.field_type
    }

    pub const fn required(&self) -> Option<&SchemaField<bool>> {
        self.required.as_ref()
    }

    pub fn is_required(&self) -> bool {
        self.required
            .as_ref()
            .is_some_and(|required| *required.value())
    }

    pub const fn repeatable(&self) -> Option<&SchemaField<bool>> {
        self.repeatable.as_ref()
    }

    pub fn is_repeatable(&self) -> bool {
        self.repeatable
            .as_ref()
            .is_some_and(|repeatable| *repeatable.value())
    }

    pub const fn values(&self) -> Option<&SchemaField<Vec<SchemaValue<String>>>> {
        self.values.as_ref()
    }

    pub const fn pattern(&self) -> Option<&SchemaField<String>> {
        self.pattern.as_ref()
    }
}

/// Requiredness and optional whole-value pattern for the display-ID built-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayIdDefinition {
    required: Option<SchemaField<bool>>,
    pattern: Option<SchemaField<String>>,
}

impl DisplayIdDefinition {
    pub fn new(required: Option<SchemaField<bool>>, pattern: Option<SchemaField<String>>) -> Self {
        Self { required, pattern }
    }

    pub const fn required(&self) -> Option<&SchemaField<bool>> {
        self.required.as_ref()
    }

    pub fn is_required(&self) -> bool {
        self.required
            .as_ref()
            .is_some_and(|required| *required.value())
    }

    pub const fn pattern(&self) -> Option<&SchemaField<String>> {
        self.pattern.as_ref()
    }
}

/// Requiredness for a title or body built-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredBuiltInDefinition {
    required: Option<SchemaField<bool>>,
}

impl RequiredBuiltInDefinition {
    pub fn new(required: Option<SchemaField<bool>>) -> Self {
        Self { required }
    }

    pub const fn required(&self) -> Option<&SchemaField<bool>> {
        self.required.as_ref()
    }

    pub fn is_required(&self) -> bool {
        self.required
            .as_ref()
            .is_some_and(|required| *required.value())
    }
}

/// Structured authoring guidance retained for human and agent consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlavourGuidance {
    use_when: SchemaField<Vec<SchemaValue<String>>>,
    avoid_when: SchemaField<Vec<SchemaValue<String>>>,
    distinguish_from_source: Option<SchemaSection>,
    distinguish_from: BTreeMap<String, SchemaField<String>>,
}

impl FlavourGuidance {
    pub fn new(
        use_when: SchemaField<Vec<SchemaValue<String>>>,
        avoid_when: SchemaField<Vec<SchemaValue<String>>>,
        distinguish_from_source: Option<SchemaSection>,
        distinguish_from: BTreeMap<String, SchemaField<String>>,
    ) -> Self {
        Self {
            use_when,
            avoid_when,
            distinguish_from_source,
            distinguish_from,
        }
    }

    pub const fn use_when(&self) -> &SchemaField<Vec<SchemaValue<String>>> {
        &self.use_when
    }

    pub const fn avoid_when(&self) -> &SchemaField<Vec<SchemaValue<String>>> {
        &self.avoid_when
    }

    pub const fn distinguish_from_source(&self) -> Option<&SchemaSection> {
        self.distinguish_from_source.as_ref()
    }

    pub const fn distinguish_from(&self) -> &BTreeMap<String, SchemaField<String>> {
        &self.distinguish_from
    }
}

/// One compiled flavour declaration and all of its schema provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlavourDefinition {
    name: String,
    key_source: SourceSpan,
    value_source: SourceSpan,
    label: SchemaField<String>,
    description: SchemaField<String>,
    guidance: SchemaField<FlavourGuidance>,
    display_id: SchemaField<DisplayIdDefinition>,
    title: SchemaField<RequiredBuiltInDefinition>,
    body: SchemaField<RequiredBuiltInDefinition>,
    fields_source: Option<SchemaSection>,
    fields: BTreeMap<String, FieldDefinition>,
}

impl FlavourDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        key_source: SourceSpan,
        value_source: SourceSpan,
        label: SchemaField<String>,
        description: SchemaField<String>,
        guidance: SchemaField<FlavourGuidance>,
        display_id: SchemaField<DisplayIdDefinition>,
        title: SchemaField<RequiredBuiltInDefinition>,
        body: SchemaField<RequiredBuiltInDefinition>,
        fields_source: Option<SchemaSection>,
        fields: BTreeMap<String, FieldDefinition>,
    ) -> Self {
        Self {
            name,
            key_source,
            value_source,
            label,
            description,
            guidance,
            display_id,
            title,
            body,
            fields_source,
            fields,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn key_source(&self) -> &SourceSpan {
        &self.key_source
    }

    pub const fn value_source(&self) -> &SourceSpan {
        &self.value_source
    }

    pub const fn label(&self) -> &SchemaField<String> {
        &self.label
    }

    pub const fn description(&self) -> &SchemaField<String> {
        &self.description
    }

    pub const fn guidance(&self) -> &SchemaField<FlavourGuidance> {
        &self.guidance
    }

    pub const fn display_id(&self) -> &SchemaField<DisplayIdDefinition> {
        &self.display_id
    }

    pub const fn title(&self) -> &SchemaField<RequiredBuiltInDefinition> {
        &self.title
    }

    pub const fn body(&self) -> &SchemaField<RequiredBuiltInDefinition> {
        &self.body
    }

    pub const fn fields_source(&self) -> Option<&SchemaSection> {
        self.fields_source.as_ref()
    }

    pub const fn fields(&self) -> &BTreeMap<String, FieldDefinition> {
        &self.fields
    }
}

/// The required root flavour mapping, canonicalized by flavour name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlavourDefinitions {
    key_source: SourceSpan,
    value_source: SourceSpan,
    definitions: BTreeMap<String, FlavourDefinition>,
}

impl FlavourDefinitions {
    pub fn new(
        key_source: SourceSpan,
        value_source: SourceSpan,
        definitions: BTreeMap<String, FlavourDefinition>,
    ) -> Self {
        Self {
            key_source,
            value_source,
            definitions,
        }
    }

    pub const fn key_source(&self) -> &SourceSpan {
        &self.key_source
    }

    pub const fn value_source(&self) -> &SourceSpan {
        &self.value_source
    }

    pub const fn definitions(&self) -> &BTreeMap<String, FlavourDefinition> {
        &self.definitions
    }

    pub fn get(&self, name: &str) -> Option<&FlavourDefinition> {
        self.definitions.get(name)
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

/// The closed set of derived relation-source kinds in schema format version 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DerivedSourceKind {
    SourceSpan,
}

impl DerivedSourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceSpan => "source_span",
        }
    }
}

/// Permitted source endpoints for one relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationSourceEndpoint {
    flavours: SchemaField<Vec<SchemaValue<String>>>,
    derived: Option<SchemaField<Vec<SchemaValue<DerivedSourceKind>>>>,
}

impl RelationSourceEndpoint {
    pub fn new(
        flavours: SchemaField<Vec<SchemaValue<String>>>,
        derived: Option<SchemaField<Vec<SchemaValue<DerivedSourceKind>>>>,
    ) -> Self {
        Self { flavours, derived }
    }

    pub const fn flavours(&self) -> &SchemaField<Vec<SchemaValue<String>>> {
        &self.flavours
    }

    pub const fn derived(&self) -> Option<&SchemaField<Vec<SchemaValue<DerivedSourceKind>>>> {
        self.derived.as_ref()
    }
}

/// Permitted target endpoints for one relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationTargetEndpoint {
    flavours: Option<SchemaField<Vec<SchemaValue<String>>>>,
    external: Option<SchemaField<Vec<SchemaValue<String>>>>,
}

impl RelationTargetEndpoint {
    pub fn new(
        flavours: Option<SchemaField<Vec<SchemaValue<String>>>>,
        external: Option<SchemaField<Vec<SchemaValue<String>>>>,
    ) -> Self {
        Self { flavours, external }
    }

    pub const fn flavours(&self) -> Option<&SchemaField<Vec<SchemaValue<String>>>> {
        self.flavours.as_ref()
    }

    pub const fn external(&self) -> Option<&SchemaField<Vec<SchemaValue<String>>>> {
        self.external.as_ref()
    }
}

/// The effective upper bound for one relation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CardinalityMaximum {
    Bounded(u64),
    Many,
}

/// Source-preserving bounds for one relation direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardinalityBound {
    min: Option<SchemaField<u64>>,
    max: Option<SchemaField<CardinalityMaximum>>,
}

impl CardinalityBound {
    pub fn new(
        min: Option<SchemaField<u64>>,
        max: Option<SchemaField<CardinalityMaximum>>,
    ) -> Self {
        Self { min, max }
    }

    pub const fn min(&self) -> Option<&SchemaField<u64>> {
        self.min.as_ref()
    }

    pub fn minimum(&self) -> u64 {
        self.min.as_ref().map_or(0, |min| *min.value())
    }

    pub const fn max(&self) -> Option<&SchemaField<CardinalityMaximum>> {
        self.max.as_ref()
    }

    pub fn maximum(&self) -> CardinalityMaximum {
        self.max
            .as_ref()
            .map_or(CardinalityMaximum::Many, |max| *max.value())
    }
}

/// Optional outgoing and incoming relation bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationCardinality {
    outgoing: Option<SchemaField<CardinalityBound>>,
    incoming: Option<SchemaField<CardinalityBound>>,
}

impl RelationCardinality {
    pub fn new(
        outgoing: Option<SchemaField<CardinalityBound>>,
        incoming: Option<SchemaField<CardinalityBound>>,
    ) -> Self {
        Self { outgoing, incoming }
    }

    pub const fn outgoing(&self) -> Option<&SchemaField<CardinalityBound>> {
        self.outgoing.as_ref()
    }

    pub const fn incoming(&self) -> Option<&SchemaField<CardinalityBound>> {
        self.incoming.as_ref()
    }
}

/// One compiled relation declaration and all of its schema provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationDefinition {
    name: String,
    key_source: SourceSpan,
    value_source: SourceSpan,
    source: SchemaField<RelationSourceEndpoint>,
    target: SchemaField<RelationTargetEndpoint>,
    inverse: Option<SchemaField<String>>,
    inverse_authoring: Option<SchemaField<bool>>,
    symmetric: Option<SchemaField<bool>>,
    same_flavour: Option<SchemaField<bool>>,
    self_reference: Option<SchemaField<bool>>,
    acyclic: Option<SchemaField<bool>>,
    cardinality: Option<SchemaField<RelationCardinality>>,
}

impl RelationDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        key_source: SourceSpan,
        value_source: SourceSpan,
        source: SchemaField<RelationSourceEndpoint>,
        target: SchemaField<RelationTargetEndpoint>,
        inverse: Option<SchemaField<String>>,
        inverse_authoring: Option<SchemaField<bool>>,
        symmetric: Option<SchemaField<bool>>,
        same_flavour: Option<SchemaField<bool>>,
        self_reference: Option<SchemaField<bool>>,
        acyclic: Option<SchemaField<bool>>,
        cardinality: Option<SchemaField<RelationCardinality>>,
    ) -> Self {
        Self {
            name,
            key_source,
            value_source,
            source,
            target,
            inverse,
            inverse_authoring,
            symmetric,
            same_flavour,
            self_reference,
            acyclic,
            cardinality,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn key_source(&self) -> &SourceSpan {
        &self.key_source
    }

    pub const fn value_source(&self) -> &SourceSpan {
        &self.value_source
    }

    pub const fn source(&self) -> &SchemaField<RelationSourceEndpoint> {
        &self.source
    }

    pub const fn target(&self) -> &SchemaField<RelationTargetEndpoint> {
        &self.target
    }

    pub const fn inverse(&self) -> Option<&SchemaField<String>> {
        self.inverse.as_ref()
    }

    pub const fn inverse_authoring(&self) -> Option<&SchemaField<bool>> {
        self.inverse_authoring.as_ref()
    }

    pub fn permits_inverse_authoring(&self) -> bool {
        self.inverse_authoring
            .as_ref()
            .is_some_and(|enabled| *enabled.value())
    }

    pub const fn symmetric(&self) -> Option<&SchemaField<bool>> {
        self.symmetric.as_ref()
    }

    pub fn is_symmetric(&self) -> bool {
        self.symmetric
            .as_ref()
            .is_some_and(|symmetric| *symmetric.value())
    }

    pub const fn same_flavour(&self) -> Option<&SchemaField<bool>> {
        self.same_flavour.as_ref()
    }

    pub fn requires_same_flavour(&self) -> bool {
        self.same_flavour
            .as_ref()
            .is_some_and(|same_flavour| *same_flavour.value())
    }

    pub const fn self_reference(&self) -> Option<&SchemaField<bool>> {
        self.self_reference.as_ref()
    }

    pub fn permits_self_reference(&self) -> bool {
        self.self_reference
            .as_ref()
            .is_none_or(|self_reference| *self_reference.value())
    }

    pub const fn acyclic(&self) -> Option<&SchemaField<bool>> {
        self.acyclic.as_ref()
    }

    pub fn is_acyclic(&self) -> bool {
        self.acyclic
            .as_ref()
            .is_some_and(|acyclic| *acyclic.value())
    }

    pub const fn cardinality(&self) -> Option<&SchemaField<RelationCardinality>> {
        self.cardinality.as_ref()
    }
}

/// The optional root relation mapping, canonicalized by relation name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationDefinitions {
    key_source: SourceSpan,
    value_source: SourceSpan,
    definitions: BTreeMap<String, RelationDefinition>,
    external_mention_schemes: BTreeSet<String>,
}

impl RelationDefinitions {
    pub fn new(
        key_source: SourceSpan,
        value_source: SourceSpan,
        definitions: BTreeMap<String, RelationDefinition>,
    ) -> Self {
        let external_mention_schemes = definitions
            .values()
            .filter_map(|definition| definition.target().value().external())
            .flat_map(|external| external.value())
            .map(|scheme| scheme.value().clone())
            .collect();
        Self {
            key_source,
            value_source,
            definitions,
            external_mention_schemes,
        }
    }

    pub const fn key_source(&self) -> &SourceSpan {
        &self.key_source
    }

    pub const fn value_source(&self) -> &SourceSpan {
        &self.value_source
    }

    pub const fn definitions(&self) -> &BTreeMap<String, RelationDefinition> {
        &self.definitions
    }

    pub fn get(&self, name: &str) -> Option<&RelationDefinition> {
        self.definitions.get(name)
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub const fn external_mention_schemes(&self) -> &BTreeSet<String> {
        &self.external_mention_schemes
    }
}

/// The project-selected diagnostic severity for a validation rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RuleSeverity {
    Error,
    Warning,
    Info,
}

impl RuleSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

/// The closed validation-rule kind set supported by schema format version 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RuleKind {
    RequiresRelation,
    RequiresField,
    Orphan,
}

impl RuleKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RequiresRelation => "requires_relation",
            Self::RequiresField => "requires_field",
            Self::Orphan => "orphan",
        }
    }
}

/// The canonical edge direction inspected by a relation requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RuleDirection {
    Outgoing,
    Incoming,
}

impl RuleDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Outgoing => "outgoing",
            Self::Incoming => "incoming",
        }
    }
}

/// The source-preserving flavour selection shared by every validation rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleApplicability {
    flavours: SchemaField<Vec<SchemaValue<String>>>,
}

impl RuleApplicability {
    pub fn new(flavours: SchemaField<Vec<SchemaValue<String>>>) -> Self {
        Self { flavours }
    }

    pub const fn flavours(&self) -> &SchemaField<Vec<SchemaValue<String>>> {
        &self.flavours
    }
}

/// A finite floating-point value used by a rule condition.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct RuleConditionNumber(f64);

impl RuleConditionNumber {
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self(value))
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}

impl Eq for RuleConditionNumber {}

/// One typed scalar value used by a rule condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleConditionValue {
    String(String),
    Integer(i64),
    Number(RuleConditionNumber),
    Boolean(bool),
}

/// An optional single-valued field condition for a validation rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleCondition {
    field: SchemaField<String>,
    values: SchemaField<Vec<SchemaValue<RuleConditionValue>>>,
}

impl RuleCondition {
    pub fn new(
        field: SchemaField<String>,
        values: SchemaField<Vec<SchemaValue<RuleConditionValue>>>,
    ) -> Self {
        Self { field, values }
    }

    pub const fn field(&self) -> &SchemaField<String> {
        &self.field
    }

    pub const fn values(&self) -> &SchemaField<Vec<SchemaValue<RuleConditionValue>>> {
        &self.values
    }
}

/// One authored canonical-relation selector shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationRuleSelection {
    Relation(SchemaField<String>),
    AnyOf(SchemaField<Vec<SchemaValue<String>>>),
}

/// One authored flavour-local field selector shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldRuleSelection {
    Field(SchemaField<String>),
    AnyOf(SchemaField<Vec<SchemaValue<String>>>),
}

/// Required minimum and optional maximum used by counting rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleCount {
    min: SchemaField<u64>,
    max: Option<SchemaField<CardinalityMaximum>>,
}

impl RuleCount {
    pub fn new(min: SchemaField<u64>, max: Option<SchemaField<CardinalityMaximum>>) -> Self {
        Self { min, max }
    }

    pub const fn min(&self) -> &SchemaField<u64> {
        &self.min
    }

    pub const fn max(&self) -> Option<&SchemaField<CardinalityMaximum>> {
        self.max.as_ref()
    }

    pub fn maximum(&self) -> CardinalityMaximum {
        self.max
            .as_ref()
            .map_or(CardinalityMaximum::Many, |maximum| *maximum.value())
    }
}

/// Parameters for a `requires_relation` validation rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiresRelationRule {
    relations: RelationRuleSelection,
    direction: SchemaField<RuleDirection>,
    count: RuleCount,
}

impl RequiresRelationRule {
    pub fn new(
        relations: RelationRuleSelection,
        direction: SchemaField<RuleDirection>,
        count: RuleCount,
    ) -> Self {
        Self {
            relations,
            direction,
            count,
        }
    }

    pub const fn relations(&self) -> &RelationRuleSelection {
        &self.relations
    }

    pub const fn direction(&self) -> &SchemaField<RuleDirection> {
        &self.direction
    }

    pub const fn count(&self) -> &RuleCount {
        &self.count
    }
}

/// Parameters for a `requires_field` validation rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiresFieldRule {
    fields: FieldRuleSelection,
    count: RuleCount,
}

impl RequiresFieldRule {
    pub fn new(fields: FieldRuleSelection, count: RuleCount) -> Self {
        Self { fields, count }
    }

    pub const fn fields(&self) -> &FieldRuleSelection {
        &self.fields
    }

    pub const fn count(&self) -> &RuleCount {
        &self.count
    }
}

/// Parameters for an `orphan` validation rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanRule {
    relations: SchemaField<Vec<SchemaValue<String>>>,
}

impl OrphanRule {
    pub fn new(relations: SchemaField<Vec<SchemaValue<String>>>) -> Self {
        Self { relations }
    }

    pub const fn relations(&self) -> &SchemaField<Vec<SchemaValue<String>>> {
        &self.relations
    }
}

/// The kind-specific parameters for one compiled validation rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleConfiguration {
    RequiresRelation(RequiresRelationRule),
    RequiresField(RequiresFieldRule),
    Orphan(OrphanRule),
}

/// One compiled validation rule and all of its schema provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDefinition {
    source: SourceSpan,
    name: SchemaField<String>,
    kind: SchemaField<RuleKind>,
    severity: SchemaField<RuleSeverity>,
    applies_to: SchemaField<RuleApplicability>,
    condition: Option<SchemaField<RuleCondition>>,
    configuration: RuleConfiguration,
}

impl RuleDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: SourceSpan,
        name: SchemaField<String>,
        kind: SchemaField<RuleKind>,
        severity: SchemaField<RuleSeverity>,
        applies_to: SchemaField<RuleApplicability>,
        condition: Option<SchemaField<RuleCondition>>,
        configuration: RuleConfiguration,
    ) -> Self {
        Self {
            source,
            name,
            kind,
            severity,
            applies_to,
            condition,
            configuration,
        }
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn name(&self) -> &SchemaField<String> {
        &self.name
    }

    pub const fn kind(&self) -> &SchemaField<RuleKind> {
        &self.kind
    }

    pub const fn severity(&self) -> &SchemaField<RuleSeverity> {
        &self.severity
    }

    pub const fn applies_to(&self) -> &SchemaField<RuleApplicability> {
        &self.applies_to
    }

    pub const fn condition(&self) -> Option<&SchemaField<RuleCondition>> {
        self.condition.as_ref()
    }

    pub const fn configuration(&self) -> &RuleConfiguration {
        &self.configuration
    }
}

/// The optional ordered root rule sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDefinitions {
    key_source: SourceSpan,
    value_source: SourceSpan,
    definitions: Vec<RuleDefinition>,
}

impl RuleDefinitions {
    pub fn new(
        key_source: SourceSpan,
        value_source: SourceSpan,
        definitions: Vec<RuleDefinition>,
    ) -> Self {
        Self {
            key_source,
            value_source,
            definitions,
        }
    }

    pub const fn key_source(&self) -> &SourceSpan {
        &self.key_source
    }

    pub const fn value_source(&self) -> &SourceSpan {
        &self.value_source
    }

    pub fn definitions(&self) -> &[RuleDefinition] {
        &self.definitions
    }

    pub fn get(&self, name: &str) -> Option<&RuleDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.name().value() == name)
    }

    pub const fn len(&self) -> usize {
        self.definitions.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

/// Source-preserving compiled format-v1 schema values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDocument {
    source: SourceSpan,
    format_version: SchemaField<u32>,
    schema: SchemaField<SchemaIdentity>,
    identity: SchemaField<IdentityConfiguration>,
    flavours: FlavourDefinitions,
    relations: Option<RelationDefinitions>,
    rules: Option<RuleDefinitions>,
}

impl SchemaDocument {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: SourceSpan,
        format_version: SchemaField<u32>,
        schema: SchemaField<SchemaIdentity>,
        identity: SchemaField<IdentityConfiguration>,
        flavours: FlavourDefinitions,
        relations: Option<RelationDefinitions>,
        rules: Option<RuleDefinitions>,
    ) -> Self {
        Self {
            source,
            format_version,
            schema,
            identity,
            flavours,
            relations,
            rules,
        }
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn format_version(&self) -> &SchemaField<u32> {
        &self.format_version
    }

    pub const fn schema(&self) -> &SchemaField<SchemaIdentity> {
        &self.schema
    }

    pub const fn identity(&self) -> &SchemaField<IdentityConfiguration> {
        &self.identity
    }

    pub const fn flavours(&self) -> &FlavourDefinitions {
        &self.flavours
    }

    pub const fn relations(&self) -> Option<&RelationDefinitions> {
        self.relations.as_ref()
    }

    pub fn external_mention_schemes(&self) -> &BTreeSet<String> {
        static EMPTY: BTreeSet<String> = BTreeSet::new();
        self.relations
            .as_ref()
            .map_or(&EMPTY, RelationDefinitions::external_mention_schemes)
    }

    pub const fn rules(&self) -> Option<&RuleDefinitions> {
        self.rules.as_ref()
    }
}
