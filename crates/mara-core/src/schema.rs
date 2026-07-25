use std::collections::BTreeMap;

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

/// Source-preserving compiled format-v1 schema values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDocument {
    source: SourceSpan,
    format_version: SchemaField<u32>,
    schema: SchemaField<SchemaIdentity>,
    identity: SchemaField<IdentityConfiguration>,
    flavours: FlavourDefinitions,
    relations: Option<SchemaSection>,
    rules: Option<SchemaSection>,
}

impl SchemaDocument {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: SourceSpan,
        format_version: SchemaField<u32>,
        schema: SchemaField<SchemaIdentity>,
        identity: SchemaField<IdentityConfiguration>,
        flavours: FlavourDefinitions,
        relations: Option<SchemaSection>,
        rules: Option<SchemaSection>,
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

    pub const fn relations(&self) -> Option<&SchemaSection> {
        self.relations.as_ref()
    }

    pub const fn rules(&self) -> Option<&SchemaSection> {
        self.rules.as_ref()
    }
}
