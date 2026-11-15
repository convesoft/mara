//! The 0.3.0 configuration, `.mara/project.toml`, and the configuration that replaces it
//! (DES-CONVERSION-MAPPING in `docs/migration.mara.md`, DES-PROJECT-CONFIGURATION in
//! `docs/project.mara.md`).

use std::ops::Range;

use mara_engine::config::LEGACY_CONFIG_PATH;
use mara_engine::diagnostic::Diagnostic;
use toml::Spanned;
use toml::de::{DeTable, DeValue};

use super::report::Entry;
use super::toml_string;

/// The schema that the converted configuration names, as `project init` writes it.
pub const SCHEMA_PATH: &str = ".mara/schema.toml";

/// What the conversion takes from a 0.3.0 configuration.
#[derive(Debug)]
pub struct Legacy {
    /// The schema path, relative to the project root, and the line that names it.
    pub schema: (String, usize),
    /// The document include patterns as written, each with its line.
    pub include: Vec<(String, usize)>,
}

/// The 0.3.0 configuration versions.
const VERSIONS: [i64; 3] = [1, 2, 3];

/// Reads the 0.3.0 configuration. What it leaves out joins `entries`. A configuration that 0.3.0
/// would not load is refused with diagnostics at its lines.
pub fn read(contents: &[u8], entries: &mut Vec<Entry>) -> Result<Legacy, Vec<Diagnostic>> {
    let line_of = |span: Range<usize>| {
        1 + contents[..span.start.min(contents.len())]
            .iter()
            .filter(|&&byte| byte == b'\n')
            .count()
    };
    let error = |code, line, message: String| {
        vec![Diagnostic::error(code, LEGACY_CONFIG_PATH, line, message)]
    };
    let Ok(text) = std::str::from_utf8(contents) else {
        return Err(error(
            "toml-syntax",
            1,
            "the file is not UTF-8 text".to_owned(),
        ));
    };
    let table = match DeTable::parse(text) {
        Ok(table) => table.into_inner(),
        Err(problem) => {
            let line = line_of(problem.span().unwrap_or(0..0));
            return Err(error(
                "toml-syntax",
                line,
                format!("invalid TOML: {}", problem.message()),
            ));
        }
    };
    let version = table
        .get("format_version")
        .and_then(|value| match value.get_ref() {
            DeValue::Integer(integer) => {
                i64::from_str_radix(integer.as_str(), integer.radix()).ok()
            }
            _ => None,
        });
    if !version.is_some_and(|version| VERSIONS.contains(&version)) {
        return Err(error(
            "unsupported-format-version",
            table
                .get("format_version")
                .map_or(1, |value| line_of(value.span())),
            format!(
                "{LEGACY_CONFIG_PATH} is not a 0.3.0 configuration of `format_version` 1, 2 or 3, \
                 which is what the conversion reads"
            ),
        ));
    }
    let section = |name: &str| match table.get(name).map(Spanned::get_ref) {
        Some(DeValue::Table(table)) => Some(table),
        _ => None,
    };
    let string = |value: &Spanned<DeValue>| match value.get_ref() {
        DeValue::String(text) => Some(text.to_string()),
        _ => None,
    };
    let mut schema = None;
    let mut include = None;
    for key in table.keys() {
        let line = line_of(key.span());
        match key.get_ref().as_ref() {
            "format_version" => {}
            "project" => {
                for (key, value) in section("project").into_iter().flatten() {
                    let line = line_of(key.span());
                    match key.get_ref().as_ref() {
                        "schema" => schema = string(value).map(|path| (path, line)),
                        "name" => entries.push(Entry::new(
                            LEGACY_CONFIG_PATH,
                            line,
                            "the project name is left out: a 0.4.0 configuration names none"
                                .to_owned(),
                        )),
                        other => entries.push(left_out(line, &format!("`project.{other}`"))),
                    }
                }
            }
            "content" => {
                for (key, value) in section("content").into_iter().flatten() {
                    let line = line_of(key.span());
                    match (key.get_ref().as_ref(), value.get_ref()) {
                        ("include", DeValue::Array(patterns)) => {
                            include = patterns
                                .iter()
                                .map(|pattern| Some((string(pattern)?, line_of(pattern.span()))))
                                .collect::<Option<Vec<_>>>();
                        }
                        (other, _) => entries.push(left_out(line, &format!("`content.{other}`"))),
                    }
                }
            }
            "rules" => {
                let files = section("rules")
                    .and_then(|rules| rules.get("files"))
                    .map(|files| match files.get_ref() {
                        DeValue::Array(files) => files
                            .iter()
                            .filter_map(string)
                            .map(|file| format!("`{file}`"))
                            .collect::<Vec<_>>()
                            .join(", "),
                        _ => String::new(),
                    })
                    .unwrap_or_default();
                entries.push(Entry::new(
                    LEGACY_CONFIG_PATH,
                    line,
                    format!(
                        "the YAML rule files {files} are left out: 0.4.0 checks are Soufflé \
                         programs (DES-RULE-CONSUMERS), and the files stay where they are"
                    ),
                ));
            }
            "code" => {
                let languages = section("code").and_then(|code| code.get("languages"));
                let languages = match languages.map(Spanned::get_ref) {
                    Some(DeValue::Array(languages)) => &languages[..],
                    _ => &[],
                };
                for language in languages {
                    let name = match language.get_ref() {
                        DeValue::Table(language) => language.get("name").and_then(string),
                        _ => None,
                    };
                    entries.push(Entry::new(
                        LEGACY_CONFIG_PATH,
                        line_of(language.span()),
                        format!(
                            "code language `{}` is left out: 0.4.0 finds code links in code \
                             markers, not in SCIP indexes (DES-CODE-MARKERS)",
                            name.unwrap_or_default()
                        ),
                    ));
                }
                if languages.is_empty() {
                    entries.push(left_out(line, "`code`"));
                }
            }
            other => entries.push(left_out(line, &format!("`{other}`"))),
        }
    }
    let Some((path, schema_line)) = schema else {
        return Err(error(
            "missing-key",
            1,
            format!("{LEGACY_CONFIG_PATH} names no schema in `project.schema`"),
        ));
    };
    let Some(normalized) = relative(&path) else {
        return Err(error(
            "invalid-value",
            schema_line,
            format!(
                "the schema path `{path}` is not below the project root, so the conversion \
                 cannot read it: move the schema into the project"
            ),
        ));
    };
    let Some(include) = include else {
        return Err(error(
            "missing-key",
            1,
            format!("{LEGACY_CONFIG_PATH} has no `content.include` list of patterns"),
        ));
    };
    for (pattern, line) in &include {
        if pattern.contains(['{', '}', '[', ']']) {
            entries.push(Entry::new(
                LEGACY_CONFIG_PATH,
                *line,
                format!(
                    "the include pattern `{pattern}` is kept as written, but 0.4.0 patterns have no \
                     brace alternatives or character classes: `{{`, `}}`, `[` and `]` match \
                     themselves, so list each path, or use `*` and `?`"
                ),
            ));
        }
    }
    Ok(Legacy {
        schema: (normalized, schema_line),
        include,
    })
}

fn left_out(line: usize, what: &str) -> Entry {
    Entry::new(
        LEGACY_CONFIG_PATH,
        line,
        format!("{what} is left out: a 0.4.0 configuration has no such setting"),
    )
}

/// `path` relative to the project root, without empty and `.` parts; `None` when it is absolute
/// or has a `..` part.
fn relative(path: &str) -> Option<String> {
    if path.starts_with('/') {
        return None;
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The converted configuration: the example of DES-PROJECT-CONFIGURATION with the 0.3.0 include
/// patterns, kept as written.
pub fn render(include: &[(String, usize)]) -> String {
    let patterns: Vec<String> = include
        .iter()
        .map(|(pattern, _)| toml_string(pattern))
        .collect();
    format!(
        "format_version = 1\nschema = {}\n\n[documents]\nformat_version = 1\ninclude = [{}]\n",
        toml_string(SCHEMA_PATH),
        patterns.join(", ")
    )
}
