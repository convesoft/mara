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

/// Source evidence for a root collection whose declarations compile in later slices.
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

/// Source-preserving format-v1 schema identity and root collection presence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDocument {
    source: SourceSpan,
    format_version: SchemaField<u32>,
    schema: SchemaField<SchemaIdentity>,
    identity: SchemaField<IdentityConfiguration>,
    flavours: SchemaSection,
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
        flavours: SchemaSection,
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

    pub const fn flavours(&self) -> &SchemaSection {
        &self.flavours
    }

    pub const fn relations(&self) -> Option<&SchemaSection> {
        self.relations.as_ref()
    }

    pub const fn rules(&self) -> Option<&SchemaSection> {
        self.rules.as_ref()
    }
}
