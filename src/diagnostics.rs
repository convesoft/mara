use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    ProjectInvalid,
    SchemaInvalid,
    FormatUnsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

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
