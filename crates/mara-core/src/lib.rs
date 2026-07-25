//! Infrastructure-free domain kernel for Mara.

pub mod diagnostic;
pub mod identity;
pub mod schema;
pub mod source;

pub use diagnostic::{
    Diagnostic, DiagnosticCode, DiagnosticContext, DiagnosticItem, DiagnosticNumber,
    DiagnosticSeverity, DiagnosticValue, RelatedDiagnostic, SchemaDiagnosticCode,
};
pub use identity::{Mid, MidParseError};
pub use schema::{
    DisplayIdDefinition, FieldDefinition, FieldType, FlavourDefinition, FlavourDefinitions,
    FlavourGuidance, IdentityConfiguration, MidFormat, MidIdentity, RequiredBuiltInDefinition,
    SchemaDocument, SchemaField, SchemaIdentity, SchemaSection, SchemaValue,
};
pub use source::{InvalidSourceSpan, SourceIndex, SourceSpan};
