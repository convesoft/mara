//! Shared validation classifications. Messages are deliberately not identifiers.
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    ProjectInvalid,
    SchemaInvalid,
    FormatUnsupported,
    SourceInvalid,
    IdentityInvalid,
    FieldInvalid,
    ReferenceUnresolved,
    RelationInvalid,
    RuleInvalid,
    RuleFailed,
    RelationCardinality,
    RelationCycle,
    EvaluationUnavailable,
    EvaluationLimit,
}

impl DiagnosticCode {
    pub fn severity(self, requested: Severity) -> Severity {
        match self {
            Self::RuleFailed | Self::RelationCardinality | Self::RelationCycle => requested,
            _ => Severity::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Error => "error",
            Self::Warning => "warning",
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
pub struct DiagnosticLocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_byte: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_byte: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pointer: Option<String>,
}

impl DiagnosticLocation {
    pub fn source(source: &crate::SourceLocation) -> Self {
        Self {
            path: Some(source.path().to_owned()),
            line: Some(source.span().start_line()),
            start_byte: Some(source.span().start_byte()),
            end_byte: Some(source.span().end_byte()),
            pointer: None,
        }
    }

    pub fn file(root: &Path, path: &Path) -> Self {
        Self {
            path: Some(path.strip_prefix(root).unwrap_or(path).to_owned()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct DiagnosticItem {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct DiagnosticObligation {
    pub shape: String,
    pub component: String,
    pub source: DiagnosticLocation,
}

/// A configuration failure retains its category and pointer at creation.
#[derive(Debug, Clone)]
pub struct ConfigurationDiagnostic {
    pub code: DiagnosticCode,
    pub pointer: Option<String>,
    pub message: String,
    pub line: Option<usize>,
    pub start_byte: Option<usize>,
    pub end_byte: Option<usize>,
}

impl ConfigurationDiagnostic {
    pub(crate) fn new(code: DiagnosticCode, pointer: String, message: String) -> Self {
        Self {
            code,
            pointer: Some(pointer),
            message,
            line: None,
            start_byte: None,
            end_byte: None,
        }
    }
    pub(crate) fn schema(parts: &[&str], message: String) -> Self {
        Self::new(DiagnosticCode::SchemaInvalid, pointer(parts), message)
    }
    pub(crate) fn project(parts: &[&str], message: String) -> Self {
        Self::new(DiagnosticCode::ProjectInvalid, pointer(parts), message)
    }
}

pub(crate) fn pointer(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|p| format!("/{}", p.replace('~', "~0").replace('/', "~1")))
        .collect()
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct ValidationOptions {
    /// Maximum diagnostics per page, 1 through 100; default 20. The serialized byte budget (65536 bytes) may return fewer.
    pub limit: Option<usize>,
    /// Opaque next_cursor; continue until has_more is false and repeat unchanged project, target, paths, limit and max_work. Omit to start or restart after source/schema/configuration changes; empty strings are invalid.
    pub cursor: Option<String>,
    /// Logical evaluation budget, 1 through 1,000,000; default 100,000. A higher budget requires a fresh request.
    pub max_work: Option<usize>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidationWork {
    pub used: usize,
    pub limit: usize,
}

/// Deterministic logical budget; charge before examining a unit of work.
pub(crate) struct WorkBudget {
    pub used: usize,
    pub limit: usize,
    pub exhausted: bool,
}

impl WorkBudget {
    pub fn new(limit: usize) -> Self {
        Self {
            used: 0,
            limit,
            exhausted: false,
        }
    }
    pub fn charge(&mut self, units: usize) -> bool {
        if self.exhausted || units > self.limit.saturating_sub(self.used) {
            self.exhausted = true;
            return false;
        }
        self.used += units;
        true
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidationSummary {
    pub errors: usize,
    pub warnings: usize,
    pub counts_exact: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidationError {
    pub code: String,
    pub message: String,
}

impl ValidationError {
    pub(crate) fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new("invalid_argument", message)
    }
    pub fn envelope(&self) -> serde_json::Value {
        serde_json::json!({"format_version": 1, "error": self})
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ValidationError {}
