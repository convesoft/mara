use std::collections::BTreeMap;

use crate::{Mid, SourceSpan};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

impl DiagnosticSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

/// The project-discovery diagnostic-code family in wire format version 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProjectDiagnosticCode {
    NotFound,
    PathOutsideRoot,
    SymlinkRejected,
    DuplicateFile,
}

impl ProjectDiagnosticCode {
    pub const ALL: [Self; 4] = [
        Self::NotFound,
        Self::PathOutsideRoot,
        Self::SymlinkRejected,
        Self::DuplicateFile,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "project.not_found",
            Self::PathOutsideRoot => "project.path_outside_root",
            Self::SymlinkRejected => "project.symlink_rejected",
            Self::DuplicateFile => "project.duplicate_file",
        }
    }

    pub const fn default_severity(self) -> DiagnosticSeverity {
        DiagnosticSeverity::Error
    }
}

/// The content-loading diagnostic-code family in wire format version 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContentDiagnosticCode {
    Io,
    InvalidUtf8,
}

impl ContentDiagnosticCode {
    pub const ALL: [Self; 2] = [Self::Io, Self::InvalidUtf8];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Io => "content.io",
            Self::InvalidUtf8 => "content.invalid_utf8",
        }
    }

    pub const fn default_severity(self) -> DiagnosticSeverity {
        DiagnosticSeverity::Error
    }
}

/// Item-syntax diagnostic codes implemented by the Markdown adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SyntaxDiagnosticCode {
    InvalidItemHeader,
    InvalidMetadata,
    UnclosedItem,
}

impl SyntaxDiagnosticCode {
    pub const ALL: [Self; 3] = [
        Self::InvalidItemHeader,
        Self::InvalidMetadata,
        Self::UnclosedItem,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidItemHeader => "syntax.invalid_item_header",
            Self::InvalidMetadata => "syntax.invalid_metadata",
            Self::UnclosedItem => "syntax.unclosed_item",
        }
    }

    pub const fn default_severity(self) -> DiagnosticSeverity {
        DiagnosticSeverity::Error
    }
}

/// Identity diagnostic codes implemented by the Markdown adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IdentityDiagnosticCode {
    InvalidMid,
}

impl IdentityDiagnosticCode {
    pub const ALL: [Self; 1] = [Self::InvalidMid];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidMid => "identity.invalid_mid",
        }
    }

    pub const fn default_severity(self) -> DiagnosticSeverity {
        DiagnosticSeverity::Error
    }
}

/// The complete schema diagnostic-code family in wire format version 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SchemaDiagnosticCode {
    Io,
    Syntax,
    DuplicateKey,
    UnsupportedFormat,
    UnknownKey,
    InvalidName,
    InvalidPattern,
    InvalidDeclaration,
}

impl SchemaDiagnosticCode {
    pub const ALL: [Self; 8] = [
        Self::Io,
        Self::Syntax,
        Self::DuplicateKey,
        Self::UnsupportedFormat,
        Self::UnknownKey,
        Self::InvalidName,
        Self::InvalidPattern,
        Self::InvalidDeclaration,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Io => "schema.io",
            Self::Syntax => "schema.syntax",
            Self::DuplicateKey => "schema.duplicate_key",
            Self::UnsupportedFormat => "schema.unsupported_format",
            Self::UnknownKey => "schema.unknown_key",
            Self::InvalidName => "schema.invalid_name",
            Self::InvalidPattern => "schema.invalid_pattern",
            Self::InvalidDeclaration => "schema.invalid_declaration",
        }
    }

    pub const fn default_severity(self) -> DiagnosticSeverity {
        DiagnosticSeverity::Error
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagnosticCode {
    Project(ProjectDiagnosticCode),
    Content(ContentDiagnosticCode),
    Syntax(SyntaxDiagnosticCode),
    Identity(IdentityDiagnosticCode),
    Schema(SchemaDiagnosticCode),
}

impl DiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project(code) => code.as_str(),
            Self::Content(code) => code.as_str(),
            Self::Syntax(code) => code.as_str(),
            Self::Identity(code) => code.as_str(),
            Self::Schema(code) => code.as_str(),
        }
    }

    pub const fn default_severity(self) -> DiagnosticSeverity {
        match self {
            Self::Project(code) => code.default_severity(),
            Self::Content(code) => code.default_severity(),
            Self::Syntax(code) => code.default_severity(),
            Self::Identity(code) => code.default_severity(),
            Self::Schema(code) => code.default_severity(),
        }
    }
}

impl From<ProjectDiagnosticCode> for DiagnosticCode {
    fn from(value: ProjectDiagnosticCode) -> Self {
        Self::Project(value)
    }
}

impl From<ContentDiagnosticCode> for DiagnosticCode {
    fn from(value: ContentDiagnosticCode) -> Self {
        Self::Content(value)
    }
}

impl From<SyntaxDiagnosticCode> for DiagnosticCode {
    fn from(value: SyntaxDiagnosticCode) -> Self {
        Self::Syntax(value)
    }
}

impl From<IdentityDiagnosticCode> for DiagnosticCode {
    fn from(value: IdentityDiagnosticCode) -> Self {
        Self::Identity(value)
    }
}

impl From<SchemaDiagnosticCode> for DiagnosticCode {
    fn from(value: SchemaDiagnosticCode) -> Self {
        Self::Schema(value)
    }
}

/// A finite JSON number used by structured diagnostic details.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct DiagnosticNumber(f64);

impl DiagnosticNumber {
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self(value))
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}

impl Eq for DiagnosticNumber {}

/// Mara-owned JSON-compatible diagnostic detail data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Unsigned(u64),
    Number(DiagnosticNumber),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl From<&str> for DiagnosticValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for DiagnosticValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedDiagnostic {
    message: String,
    span: SourceSpan,
}

impl RelatedDiagnostic {
    pub fn new(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticItem {
    mid: Mid,
    id: Option<String>,
}

impl DiagnosticItem {
    pub fn new(mid: Mid, id: Option<String>) -> Self {
        Self { mid, id }
    }

    pub const fn mid(&self) -> &Mid {
        &self.mid
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticContext {
    field: Option<String>,
    relation: Option<String>,
    target: Option<String>,
}

impl DiagnosticContext {
    pub fn new(field: Option<String>, relation: Option<String>, target: Option<String>) -> Self {
        Self {
            field,
            relation,
            target,
        }
    }

    pub fn field(&self) -> Option<&str> {
        self.field.as_deref()
    }

    pub fn relation(&self) -> Option<&str> {
        self.relation.as_deref()
    }

    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }
}

/// A structured diagnostic matching the v1 wire model without serialization coupling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    code: DiagnosticCode,
    severity: DiagnosticSeverity,
    message: String,
    primary: Option<SourceSpan>,
    related: Vec<RelatedDiagnostic>,
    item: Option<DiagnosticItem>,
    context: DiagnosticContext,
    details: BTreeMap<String, DiagnosticValue>,
}

impl Diagnostic {
    pub fn new(
        code: impl Into<DiagnosticCode>,
        message: impl Into<String>,
        primary: Option<SourceSpan>,
    ) -> Self {
        let code = code.into();
        Self {
            severity: code.default_severity(),
            code,
            message: message.into(),
            primary,
            related: Vec::new(),
            item: None,
            context: DiagnosticContext::default(),
            details: BTreeMap::new(),
        }
    }

    pub fn with_related(mut self, related: RelatedDiagnostic) -> Self {
        self.related.push(related);
        self
    }

    pub fn with_item(mut self, item: DiagnosticItem) -> Self {
        self.item = Some(item);
        self
    }

    pub fn with_context(mut self, context: DiagnosticContext) -> Self {
        self.context = context;
        self
    }

    pub fn with_detail(
        mut self,
        key: impl Into<String>,
        value: impl Into<DiagnosticValue>,
    ) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn primary(&self) -> Option<&SourceSpan> {
        self.primary.as_ref()
    }

    pub fn related(&self) -> &[RelatedDiagnostic] {
        &self.related
    }

    pub fn item(&self) -> Option<&DiagnosticItem> {
        self.item.as_ref()
    }

    pub const fn context(&self) -> &DiagnosticContext {
        &self.context
    }

    pub const fn details(&self) -> &BTreeMap<String, DiagnosticValue> {
        &self.details
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_code_catalogue_is_closed_and_stable() {
        let codes = SchemaDiagnosticCode::ALL.map(SchemaDiagnosticCode::as_str);
        assert_eq!(
            codes,
            [
                "schema.io",
                "schema.syntax",
                "schema.duplicate_key",
                "schema.unsupported_format",
                "schema.unknown_key",
                "schema.invalid_name",
                "schema.invalid_pattern",
                "schema.invalid_declaration",
            ]
        );
    }

    #[test]
    fn project_and_content_code_catalogues_are_closed_and_stable() {
        assert_eq!(
            ProjectDiagnosticCode::ALL.map(ProjectDiagnosticCode::as_str),
            [
                "project.not_found",
                "project.path_outside_root",
                "project.symlink_rejected",
                "project.duplicate_file",
            ]
        );
        assert_eq!(
            ContentDiagnosticCode::ALL.map(ContentDiagnosticCode::as_str),
            ["content.io", "content.invalid_utf8"]
        );
        assert_eq!(
            SyntaxDiagnosticCode::ALL.map(SyntaxDiagnosticCode::as_str),
            [
                "syntax.invalid_item_header",
                "syntax.invalid_metadata",
                "syntax.unclosed_item",
            ]
        );
        assert_eq!(
            IdentityDiagnosticCode::ALL.map(IdentityDiagnosticCode::as_str),
            ["identity.invalid_mid"]
        );
    }

    #[test]
    fn diagnostic_details_are_utf8_byte_ordered() {
        let diagnostic = Diagnostic::new(SchemaDiagnosticCode::Syntax, "invalid YAML", None)
            .with_detail("target", "value")
            .with_detail("field", "schema");
        assert_eq!(
            diagnostic
                .details()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["field", "target"]
        );
    }
}
