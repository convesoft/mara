//! Reading one TOML file of the project into values and located diagnostics.

use std::borrow::Cow;
use std::ops::Range;

use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::diagnostic::Diagnostic;

pub(crate) type Key<'i> = Spanned<Cow<'i, str>>;
pub(crate) type Value<'i> = Spanned<DeValue<'i>>;
pub(crate) type Table<'i> = DeTable<'i>;

/// The only `format_version` of the configuration, the schema and the document syntax.
const FORMAT_VERSION: i64 = 1;

pub(crate) struct TomlFile<'p> {
    path: &'p str,
    /// Byte offset of the start of each line.
    line_starts: Vec<usize>,
    diagnostics: Vec<Diagnostic>,
}

impl<'p> TomlFile<'p> {
    /// Decodes and parses `bytes`. A file that is not UTF-8 TOML yields a `toml-syntax`
    /// diagnostic and no table.
    pub(crate) fn parse<'i>(path: &'p str, bytes: &'i [u8]) -> (Self, Option<Table<'i>>) {
        let line_starts = std::iter::once(0)
            .chain(newlines(bytes).map(|offset| offset + 1))
            .collect();
        let mut file = Self {
            path,
            line_starts,
            diagnostics: Vec::new(),
        };
        let text = match std::str::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                let at = error.valid_up_to();
                file.error(
                    "toml-syntax",
                    at..at,
                    "the file is not UTF-8 text".to_owned(),
                );
                return (file, None);
            }
        };
        match DeTable::parse(text) {
            Ok(table) => (file, Some(table.into_inner())),
            Err(error) => {
                let span = error.span().unwrap_or(0..0);
                file.error(
                    "toml-syntax",
                    span,
                    format!("invalid TOML: {}", error.message()),
                );
                (file, None)
            }
        }
    }

    /// The diagnostics in line order.
    pub(crate) fn finish(mut self) -> Vec<Diagnostic> {
        self.diagnostics.sort_by_key(|diagnostic| diagnostic.line);
        self.diagnostics
    }

    /// The 1-based line that holds byte `offset`.
    pub(crate) fn line(&self, offset: usize) -> usize {
        self.line_starts.partition_point(|&start| start <= offset)
    }

    pub(crate) fn error(&mut self, code: &'static str, span: Range<usize>, message: String) {
        let line = self.line(span.start);
        let diagnostic = Diagnostic::error(code, self.path, line, message);
        self.diagnostics.push(diagnostic);
    }

    /// Checks `format_version` of `table`, whose header is at `at`. Returns `false` for a
    /// version other than 1: nothing else in the table can then be read. A missing or
    /// malformed version is reported and the table read as version 1.
    pub(crate) fn format_version(&mut self, table: &Table, at: Range<usize>, place: &str) -> bool {
        let Some(value) = self.required(table, "format_version", at, place) else {
            return true;
        };
        let version = match value.get_ref() {
            DeValue::Integer(integer) => {
                i64::from_str_radix(integer.as_str(), integer.radix()).ok()
            }
            _ => None,
        };
        match version {
            None => {
                self.error(
                    "invalid-value",
                    value.span(),
                    format!("`format_version` of {place} must be the integer {FORMAT_VERSION}"),
                );
                true
            }
            Some(FORMAT_VERSION) => true,
            Some(version) => {
                let advice = if version > FORMAT_VERSION {
                    "upgrade Mara"
                } else {
                    "set it to 1"
                };
                self.error(
                    "unsupported-format-version",
                    value.span(),
                    format!(
                        "{place} has `format_version` {version}, and this Mara reads only version {FORMAT_VERSION}; {advice}"
                    ),
                );
                false
            }
        }
    }

    /// Reports each key of `table` that is not in `known`.
    pub(crate) fn unknown_keys(&mut self, table: &Table, known: &[&str], place: &str) {
        for key in table.keys() {
            if !known.contains(&key.get_ref().as_ref()) {
                let expected = known
                    .iter()
                    .map(|key| format!("`{key}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.error(
                    "unknown-key",
                    key.span(),
                    format!(
                        "unknown key `{}` in {place}; the keys are {expected}",
                        key.get_ref()
                    ),
                );
            }
        }
    }

    /// The value of `key`, reported as missing from `place`, whose header is at `at`, when absent.
    pub(crate) fn required<'t, 'i>(
        &mut self,
        table: &'t Table<'i>,
        key: &str,
        at: Range<usize>,
        place: &str,
    ) -> Option<&'t Value<'i>> {
        let value = table.get(key);
        if value.is_none() {
            self.error("missing-key", at, format!("{place} has no `{key}`"));
        }
        value
    }

    pub(crate) fn string<'t>(&mut self, key: &str, value: &'t Value) -> Option<&'t str> {
        match value.get_ref() {
            DeValue::String(string) => Some(string),
            _ => self.wrong_type(key, value, "a string"),
        }
    }

    /// A string that must not be empty.
    pub(crate) fn text<'t>(&mut self, key: &str, value: &'t Value) -> Option<&'t str> {
        let text = self.string(key, value)?;
        if text.trim().is_empty() {
            self.error(
                "invalid-value",
                value.span(),
                format!("`{key}` must not be empty"),
            );
            return None;
        }
        Some(text)
    }

    pub(crate) fn boolean(&mut self, key: &str, value: &Value) -> Option<bool> {
        match value.get_ref() {
            DeValue::Boolean(boolean) => Some(*boolean),
            _ => self.wrong_type(key, value, "`true` or `false`"),
        }
    }

    pub(crate) fn table<'t, 'i>(
        &mut self,
        key: &str,
        value: &'t Value<'i>,
    ) -> Option<&'t Table<'i>> {
        match value.get_ref() {
            DeValue::Table(table) => Some(table),
            _ => self.wrong_type(key, value, "a table"),
        }
    }

    pub(crate) fn array<'t, 'i>(
        &mut self,
        key: &str,
        value: &'t Value<'i>,
    ) -> Option<&'t [Value<'i>]> {
        match value.get_ref() {
            DeValue::Array(array) => Some(array),
            _ => self.wrong_type(key, value, "an array"),
        }
    }

    /// A path or pattern relative to the project root.
    pub(crate) fn relative_path<'t>(&mut self, key: &str, value: &'t Value) -> Option<&'t str> {
        let path = self.string(key, value)?;
        if path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        {
            self.error(
                "invalid-value",
                value.span(),
                format!(
                    "`{key}` must be relative to the project root, with `/` between non-empty parts and no `.` or `..` part: `{path}`"
                ),
            );
            return None;
        }
        Some(path)
    }

    fn wrong_type<T>(&mut self, key: &str, value: &Value, expected: &str) -> Option<T> {
        self.error(
            "invalid-value",
            value.span(),
            format!(
                "`{key}` must be {expected}; found {}",
                value.get_ref().type_str()
            ),
        );
        None
    }
}

fn newlines(bytes: &[u8]) -> impl Iterator<Item = usize> + '_ {
    bytes
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte == b'\n')
        .map(|(offset, _)| offset)
}
