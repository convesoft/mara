//! Infrastructure-free domain kernel for Mara.

pub mod diagnostic;
pub mod identity;
pub mod query;
pub mod schema;
pub mod semantic;
pub mod source;
pub mod validation;

pub use diagnostic::{
    ContentDiagnosticCode, Diagnostic, DiagnosticCode, DiagnosticContext, DiagnosticItem,
    DiagnosticNumber, DiagnosticSeverity, DiagnosticValue, FieldDiagnosticCode,
    IdentityDiagnosticCode, ItemDiagnosticCode, ProjectDiagnosticCode, ReferenceDiagnosticCode,
    RelatedDiagnostic, RelationDiagnosticCode, RuleDiagnosticCode, SchemaDiagnosticCode,
    SyntaxDiagnosticCode, sort_diagnostics,
};
pub use identity::{Mid, MidParseError};
pub use query::{
    NodeRef, ProjectionEdge, ProjectionEdgeError, QueryGraph, TraceDirection, TraceError,
    TracePath, TraceResult, TraceStep, TraversalDirection,
};
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
pub use semantic::{
    AuthoredReference, AuthoredReferenceSyntax, AuthoredRelationOrigin, CanonicalRelationEdge,
    CanonicalRelationInput, CanonicalRelationKey, CanonicalRelations, DerivedRelationOrigin,
    DerivedRelationView, IdentityCandidate, IdentityIndex, IdentityIndexBuild, IdentityRecord,
    NormalizedFieldValue, NormalizedItem, NormalizedNumber, NormalizedScalar, Provenanced,
    ReferenceOrigin, RelationOccurrence, ResolvedReference, WeakMention,
};
pub use source::{
    InvalidSourceSpan, LineEnding, SourceDocument, SourceIndex, SourceSpan, SourceText,
};
pub use validation::{
    SeverityCounts, ValidationPhase, ValidationPhaseResult, ValidationPhaseState, evaluate_model,
};
