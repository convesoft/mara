//! Public operation parameters/results shared by native CLI and MCP adapters.
use crate::InitialRelation;
pub use crate::runtime::NativeRuntime as OperationContext;
pub use mara_engine::validation::{
    ValidationDiagnostic, ValidationResult, ValidationScope, ValidationTarget, ValidationTargetKind,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, JsonSchema, serde::Deserialize)]
pub struct ProjectInitializationResult {
    pub project: ProjectSummary,
    pub created: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, serde::Deserialize)]
pub struct ProjectSummary {
    pub root: PathBuf,
    pub name: String,
    pub schema_path: PathBuf,
    pub content_patterns: Vec<String>,
}

pub use crate::runtime::project_initialize;
pub use mara_engine::operations::*;
#[derive(Debug, Clone, Serialize, JsonSchema, serde::Deserialize)]
pub struct TransactionRollbackResult {
    pub project: PathBuf,
    pub restored: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, serde::Deserialize)]
pub struct ProjectMidBackfillResult {
    pub project: PathBuf,
    pub changed: Vec<BackfilledMidResult>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, serde::Deserialize)]
pub struct BackfilledMidResult {
    pub id: String,
    pub mid: String,
    pub path: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ItemCreateParams {
    pub flavour: String,
    pub id: String,
    pub file: PathBuf,
    pub title: String,
    /// Schema-declared custom fields only; excludes title, MID, and typed relations.
    #[serde(default)]
    pub fields: Vec<FieldValue>,
    /// Initial outgoing edges, validated and created atomically. Omitted or empty adds none.
    #[serde(default)]
    pub relations: Vec<InitialRelation>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, serde::Deserialize)]
pub struct ItemCreationResult {
    pub id: String,
    pub mid: String,
    pub path: PathBuf,
    pub line: usize,
    pub complete: bool,
    pub missing: Vec<String>,
}

pub use mara_engine::{ItemMoveParams, ItemUpdateParams, RelationAction, RelationMutationResult};
