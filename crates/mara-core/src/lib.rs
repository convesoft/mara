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
    CardinalityBound, CardinalityMaximum, DerivedSourceKind, DisplayIdDefinition, FieldDefinition,
    FieldType, FlavourDefinition, FlavourDefinitions, FlavourGuidance, IdentityConfiguration,
    MidFormat, MidIdentity, RelationCardinality, RelationDefinition, RelationDefinitions,
    RelationSourceEndpoint, RelationTargetEndpoint, RequiredBuiltInDefinition, SchemaDocument,
    SchemaField, SchemaIdentity, SchemaSection, SchemaValue,
};
pub use source::{InvalidSourceSpan, SourceIndex, SourceSpan};
