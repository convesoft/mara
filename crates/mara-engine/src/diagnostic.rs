//! Problems reported with their location (DES-DIAGNOSTICS in `docs/validation.mara.md`).

use std::borrow::Cow;
use std::fmt;

use crate::json::Object;

/// Whether a problem makes the project invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    /// `error` or `warning`, as results show it.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// One problem at one line of one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable kebab-case name of the problem: borrowed for a built-in check, owned for an
    /// invariant's name, which a configuration sets and which goes with the diagnostic.
    pub code: Cow<'static, str>,
    pub severity: Severity,
    pub message: String,
    /// The file, relative to the project root, with `/` separators.
    pub path: String,
    /// 1-based.
    pub line: usize,
    /// The first `ID` value of the item the problem concerns, if it concerns an item that has
    /// one.
    pub id: Option<String>,
    /// The MID of the item the problem concerns, if it concerns an item.
    pub mid: Option<String>,
}

impl Diagnostic {
    /// An error at `line` of `path` that concerns no item.
    pub fn error(
        code: impl Into<Cow<'static, str>>,
        path: impl Into<String>,
        line: usize,
        message: String,
    ) -> Self {
        Diagnostic {
            code: code.into(),
            severity: Severity::Error,
            message,
            path: path.into(),
            line,
            id: None,
            mid: None,
        }
    }

    /// Appends the JSON form: an object of `code`, `severity`, `message`, `path`, `line`, `id`
    /// and `mid`, the last two `null` when absent.
    pub fn write_json(&self, out: &mut String) {
        let mut object = Object::new(out);
        object.string("code", &self.code);
        object.string("severity", self.severity.as_str());
        object.string("message", &self.message);
        object.string("path", &self.path);
        object.number("line", self.line as u64);
        object.optional_string("id", self.id.as_deref());
        object.optional_string("mid", self.mid.as_deref());
        object.end();
    }
}

#[cfg(test)]
impl Diagnostic {
    /// The code of a built-in check, which is static. Panics on an invariant's name.
    pub(crate) fn built_in_code(&self) -> &'static str {
        match self.code {
            Cow::Borrowed(code) => code,
            Cow::Owned(ref code) => panic!("`{code}` is not the code of a built-in check"),
        }
    }
}

impl fmt::Display for Diagnostic {
    /// `<path>:<line>: <severity>[<code>]: <message>`, as the CLI prints it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {}[{}]: {}",
            self.path,
            self.line,
            self.severity.as_str(),
            self.code,
            self.message
        )
    }
}
