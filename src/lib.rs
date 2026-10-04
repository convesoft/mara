//! Native hosting and adapters for the explicit-input engine.
pub use mara_engine::*;
mod host;
mod rule_bridge;
mod source_bridge;
pub use host::*;
pub use rule_bridge::load_rule_checks;
pub use source_bridge::{load_native_source_engine, load_source_engine};
pub mod code;
pub use code::{CodeFile, CodeIndex, CodeMarker, CodeProblem, CodeSymbol};
pub mod corpus;
pub use corpus::{
    Corpus, Diagnostic, Document, DocumentReference, DocumentSet, Item, MarkdownBlock,
    MarkdownBlockKind, Mention, MetadataEntry, ReferenceKind, Relation, SourceLocation, SourceSpan,
    load_corpus, load_corpus_for_validation, load_corpus_syntax_for_validation, load_documents,
    load_documents_for_validation, load_documents_syntax_for_validation, validate_corpus,
    validate_corpus_independent,
};
pub use discovery::{
    ConnectionKind, DiscoveryConnection, DiscoveryContext, DiscoveryGraph, DiscoveryKind,
    DiscoveryNode, DiscoveryNodeKind, DiscoveryNodeSummary,
};
pub use mara_engine::diagnostics;
pub use mara_engine::discovery;
pub use mara_engine::external;
pub use mara_engine::query;
pub use mara_engine::relations;
pub use query::{
    EntryRange, FieldFilter, GetResult, ItemCollectionResult, ItemFilters, ItemSource, ItemSummary,
    MetadataFragment, QueryError, RelatedConnection, RelatedFilters, RelatedNeighbour,
    RelatedResult, RelationDirection, SearchExcerpt, SearchHit, SearchResult, TextRange, get,
    list_items, related, search,
};
pub use relations::{
    RelationEdge, RelationEndpoint, RelationError, RelationErrorDetail, RelationInspection,
    RelationOccurrence,
};
mod mutation;
pub use mara_engine::graph_constraints;
pub use mutation::{
    BackfilledMid, BackfilledMids, TransactionRollback, backfill_mids, rollback_transaction,
};
pub use mutation::{InitialRelation, ItemCreation, ItemCreationRequest, create_item};
pub mod client;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod http;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod ipc;
#[path = "rules.rs"]
mod native_rules;
#[path = "trace.rs"]
mod native_trace;
pub use client::{ClientError, ClientOptions, ExecutionMode, NativeClient};
pub mod operations;
pub mod protocol;
pub mod runtime;
pub use diagnostics::{
    ConfigurationDiagnostic, DiagnosticCode, DiagnosticItem, DiagnosticLocation,
    DiagnosticObligation, Severity, ValidationError, ValidationOptions, ValidationSummary,
};
pub use native_trace::{
    TraceCheck, TraceField, TraceMatrixParams, TraceMatrixResult, TraceMatrixSummary,
    TraceSelection,
};
pub use operations::{
    BackfilledMidResult, DeclarationSummary, FieldValue, GetParams, ItemCreateParams,
    ItemCreationResult, ItemFilterParams, OperationContext, ProjectInitializationResult,
    ProjectMidBackfillResult, ProjectSummary, RelatedParams, RelationParams, SchemaGetResult,
    SchemaKind, SchemaListResult, SearchParams, TransactionRollbackResult, ValidationDiagnostic,
    ValidationResult, ValidationScope, ValidationTarget, ValidationTargetKind, project_initialize,
};
pub use runtime::{NativeRuntime, NativeSessions, SessionActivity, SessionPolicy};

pub use operations::{RelationAction, RelationMutationResult};

pub use mutation::{ItemUpdate, ItemUpdateWarning, update_item};
pub use operations::ItemUpdateParams;

pub use mutation::{ItemDeletion, delete_item};

pub use mutation::{ItemLocation, ItemMove, move_item};
pub use operations::ItemMoveParams;

pub use mutation::{ItemRename, rename_item};
