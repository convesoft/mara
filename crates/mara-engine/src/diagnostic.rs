//! Problems reported with their location (DES-DIAGNOSTICS in `docs/validation.mara.md`).

use std::fmt;

/// Whether a problem makes the project invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One problem at one line of one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable kebab-case name of the problem.
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    /// The file, relative to the project root, with `/` separators.
    pub path: String,
    /// 1-based.
    pub line: usize,
}

impl fmt::Display for Diagnostic {
    /// `<path>:<line>: <severity>[<code>]: <message>`, as the CLI prints it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(
            f,
            "{}:{}: {}[{}]: {}",
            self.path, self.line, severity, self.code, self.message
        )
    }
}
