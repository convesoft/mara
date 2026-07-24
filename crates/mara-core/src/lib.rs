//! Infrastructure-free domain kernel for Mara.

pub mod diagnostic;
pub mod schema;
pub mod source;

pub use diagnostic::{
    Diagnostic, DiagnosticCode, DiagnosticContext, DiagnosticItem, DiagnosticNumber,
    DiagnosticSeverity, DiagnosticValue, RelatedDiagnostic, SchemaDiagnosticCode,
};
pub use schema::{
    IdentityConfiguration, MidFormat, MidIdentity, SchemaDocument, SchemaField, SchemaIdentity,
    SchemaSection,
};
pub use source::{InvalidSourceSpan, SourceSpan};
