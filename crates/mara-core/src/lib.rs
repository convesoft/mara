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
    FieldRuleSelection, FieldType, FlavourDefinition, FlavourDefinitions, FlavourGuidance,
    IdentityConfiguration, MidFormat, MidIdentity, OrphanRule, RelationCardinality,
    RelationDefinition, RelationDefinitions, RelationRuleSelection, RelationSourceEndpoint,
    RelationTargetEndpoint, RequiredBuiltInDefinition, RequiresFieldRule, RequiresRelationRule,
    RuleApplicability, RuleCondition, RuleConditionNumber, RuleConditionValue, RuleConfiguration,
    RuleCount, RuleDefinition, RuleDefinitions, RuleDirection, RuleKind, RuleSeverity,
    SchemaDocument, SchemaField, SchemaIdentity, SchemaSection, SchemaValue,
};
pub use source::{InvalidSourceSpan, SourceIndex, SourceSpan};
