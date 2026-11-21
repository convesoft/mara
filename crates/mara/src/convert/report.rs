//! The conversion report and its file, `.mara/conversion-report.json` (DES-CONVERSION-MAPPING
//! and ADR-CONVERSION-REPORT-FILE in `docs/migration.mara.md`).

use std::collections::BTreeSet;

use mara_engine::diagnostic::Diagnostic;
use mara_engine::json::Object;
use serde_json::Value;

/// The report file, relative to the project root.
pub const REPORT_PATH: &str = ".mara/conversion-report.json";

/// The version of the report file's format.
pub const FORMAT_VERSION: u64 = 1;

/// One thing the conversion leaves out or changes: in the 0.3.0 file at `path`, relative to the
/// project root, at `line`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    pub path: String,
    pub line: usize,
    pub message: String,
}

impl Entry {
    pub fn new(path: &str, line: usize, message: String) -> Entry {
        Entry {
            path: path.to_owned(),
            line,
            message,
        }
    }

    /// `{"path","line","message"}`.
    pub fn write_json(&self, out: &mut String) {
        let mut object = Object::new(out);
        object.string("path", &self.path);
        object.number("line", self.line as u64);
        object.string("message", &self.message);
        object.end();
    }
}

/// The entries of an existing report file, or why it cannot be read as one: a diagnostic at the
/// file, which refuses the conversion.
pub fn read(contents: &[u8]) -> Result<Vec<Entry>, Box<Diagnostic>> {
    let refuse = |code, message: &str| {
        Err(Box::new(Diagnostic::error(
            code,
            REPORT_PATH,
            1,
            format!(
                "{REPORT_PATH} {message}, so Mara cannot keep the report of the earlier conversion: \
                 move the file away and run the conversion again"
            ),
        )))
    };
    let Ok(value) = serde_json::from_slice::<Value>(contents) else {
        return refuse("invalid-value", "is not JSON");
    };
    let Some(object) = value.as_object() else {
        return refuse("invalid-value", "is not a JSON object");
    };
    match object.get("format_version").and_then(Value::as_u64) {
        Some(FORMAT_VERSION) => {}
        Some(version) => {
            let message = format!("has `format_version` {version}, and this Mara reads only 1");
            return refuse("unsupported-format-version", &message);
        }
        None => return refuse("invalid-value", "has no `format_version` 1"),
    }
    let entries = match object.get("entries").and_then(Value::as_array) {
        Some(entries) if object.len() == 2 => entries,
        _ => {
            return refuse(
                "invalid-value",
                "is not an object of `format_version` and `entries`",
            );
        }
    };
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let read = entry
            .as_object()
            .filter(|entry| entry.len() == 3)
            .and_then(|entry| {
                Some(Entry {
                    path: entry.get("path")?.as_str()?.to_owned(),
                    line: usize::try_from(entry.get("line")?.as_u64()?).ok()?,
                    message: entry.get("message")?.as_str()?.to_owned(),
                })
            });
        match read {
            Some(entry) => out.push(entry),
            None => {
                return refuse(
                    "invalid-value",
                    "holds an entry that is not an object of `path`, `line` and `message`",
                );
            }
        }
    }
    Ok(out)
}

/// The entries of `recorded` and `computed` together, each once, in path, line and message
/// order: no recorded entry is dropped.
pub fn merge(recorded: Vec<Entry>, computed: Vec<Entry>) -> Vec<Entry> {
    let all: BTreeSet<Entry> = recorded.into_iter().chain(computed).collect();
    all.into_iter().collect()
}

/// The report file's contents: one entry per line.
pub fn render(entries: &[Entry]) -> String {
    let mut out = format!("{{\"format_version\":{FORMAT_VERSION},\"entries\":[");
    for (index, entry) in entries.iter().enumerate() {
        out.push_str(if index == 0 { "\n" } else { ",\n" });
        entry.write_json(&mut out);
    }
    out.push_str(if entries.is_empty() { "]}\n" } else { "\n]}\n" });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_back_what_it_renders() {
        let entries = vec![
            Entry::new("docs/a.mara.md", 3, "a \"quoted\" message".to_owned()),
            Entry::new("docs/b.mara.md", 1, "another".to_owned()),
        ];
        assert_eq!(read(render(&entries).as_bytes()).unwrap(), entries);
        assert_eq!(read(render(&[]).as_bytes()).unwrap(), []);
    }

    #[test]
    fn refuses_anything_else() {
        for (contents, code) in [
            ("not json", "invalid-value"),
            ("[]", "invalid-value"),
            (
                "{\"format_version\":2,\"entries\":[]}",
                "unsupported-format-version",
            ),
            ("{\"entries\":[]}", "invalid-value"),
            ("{\"format_version\":1}", "invalid-value"),
            (
                "{\"format_version\":1,\"entries\":[],\"x\":1}",
                "invalid-value",
            ),
            (
                "{\"format_version\":1,\"entries\":[{\"path\":\"a\",\"line\":1}]}",
                "invalid-value",
            ),
        ] {
            let diagnostic = read(contents.as_bytes()).unwrap_err();
            assert_eq!(
                (&*diagnostic.code, diagnostic.path.as_str()),
                (code, REPORT_PATH),
                "{contents}"
            );
        }
    }

    #[test]
    fn merges_without_dropping_or_repeating() {
        let entry = |path: &str, line, message: &str| Entry::new(path, line, message.to_owned());
        let recorded = vec![entry("b", 2, "moved"), entry("a", 1, "kept")];
        let computed = vec![entry("a", 1, "kept"), entry("b", 5, "moved")];
        assert_eq!(
            merge(recorded, computed),
            [
                entry("a", 1, "kept"),
                entry("b", 2, "moved"),
                entry("b", 5, "moved")
            ]
        );
    }
}
