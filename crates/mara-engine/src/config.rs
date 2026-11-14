//! The project configuration and project roots (DES-PROJECT-CONFIGURATION in
//! `docs/project.mara.md`).

use crate::diagnostic::Diagnostic;
use crate::toml_file::{Table, TomlFile, Value};

/// The configuration, relative to the project root.
pub const CONFIG_PATH: &str = ".mara/config.toml";
/// The configuration of a 0.3.0 project, relative to its root.
pub const LEGACY_CONFIG_PATH: &str = ".mara/project.toml";

/// What the files in a directory's `.mara/` make of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Root {
    /// The root of a project.
    Project,
    /// The root of a 0.3.0 project, which only the conversion reads.
    Legacy,
}

impl Root {
    /// Recognizes a directory by whether `CONFIG_PATH` and `LEGACY_CONFIG_PATH` exist in it.
    pub fn recognize(has_config: bool, has_legacy_config: bool) -> Option<Self> {
        if has_config {
            Some(Self::Project)
        } else if has_legacy_config {
            Some(Self::Legacy)
        } else {
            None
        }
    }
}

/// What the configuration declares. A setting that is missing or invalid is `None`, and an
/// invalid include pattern is left out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    /// The schema path.
    pub schema: Option<PathSetting>,
    /// Document include patterns.
    pub include: Option<Vec<PathSetting>>,
}

/// A path or pattern relative to the project root, with the configuration line that sets it,
/// where a problem with it is reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathSetting {
    pub path: String,
    pub line: usize,
}

/// Reads the contents of `CONFIG_PATH`. Every problem becomes a diagnostic, and every setting
/// that is valid is read even when others are not.
pub fn parse_config(bytes: &[u8]) -> (Config, Vec<Diagnostic>) {
    let (mut file, table) = TomlFile::parse(CONFIG_PATH, bytes);
    let mut config = Config::default();
    if let Some(table) = table {
        read(&mut file, &table, &mut config);
    }
    (config, file.finish())
}

fn read(file: &mut TomlFile, root: &Table, config: &mut Config) {
    const PLACE: &str = "the configuration";
    if !file.format_version(root, 0..0, PLACE) {
        return;
    }
    file.unknown_keys(root, &["format_version", "schema", "documents"], PLACE);
    if let Some(value) = file.required(root, "schema", 0..0, PLACE) {
        config.schema = path(file, "schema", value);
    }
    let Some((key, value)) = root.get_key_value("documents") else {
        file.error("missing-key", 0..0, format!("{PLACE} has no `[documents]`"));
        return;
    };
    let Some(documents) = file.table("documents", value) else {
        return;
    };
    const DOCUMENTS: &str = "`[documents]`";
    if !file.format_version(documents, key.span(), DOCUMENTS) {
        return;
    }
    file.unknown_keys(documents, &["format_version", "include"], DOCUMENTS);
    if let Some(value) = file.required(documents, "include", key.span(), DOCUMENTS)
        && let Some(patterns) = file.array("include", value)
    {
        let include = patterns
            .iter()
            .filter_map(|pattern| path(file, "include", pattern))
            .collect();
        config.include = Some(include);
    }
}

fn path(file: &mut TomlFile, key: &str, value: &Value) -> Option<PathSetting> {
    let path = file.relative_path(key, value)?.to_owned();
    let line = file.line(value.span().start);
    Some(PathSetting { path, line })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(diagnostics: &[Diagnostic]) -> Vec<(usize, &'static str)> {
        diagnostics.iter().map(|d| (d.line, d.code)).collect()
    }

    fn patterns(config: &Config) -> Vec<&str> {
        config
            .include
            .iter()
            .flatten()
            .map(|pattern| pattern.path.as_str())
            .collect()
    }

    #[test]
    fn reports_unknown_settings_and_reads_the_rest() {
        let text = "format_version = 1\nschema = \".mara/schema.toml\"\nname = \"x\"\n\n[documents]\nformat_version = 1\ninclude = [\"docs/*.mara.md\"]\nexclude = []\n\n[rules]\nprograms = []\n";
        let (config, diagnostics) = parse_config(text.as_bytes());
        assert_eq!(
            codes(&diagnostics),
            [(3, "unknown-key"), (8, "unknown-key"), (10, "unknown-key")]
        );
        assert_eq!(diagnostics[0].path, CONFIG_PATH);
        assert_eq!(
            diagnostics[0].to_string(),
            ".mara/config.toml:3: error[unknown-key]: unknown key `name` in the configuration; the keys are `format_version`, `schema`, `documents`"
        );
        assert_eq!(patterns(&config), ["docs/*.mara.md"]);
        assert_eq!(config.schema.unwrap().path, ".mara/schema.toml");
    }

    #[test]
    fn reports_missing_settings() {
        let (config, diagnostics) = parse_config(b"[documents]\n");
        assert_eq!(
            codes(&diagnostics),
            [
                (1, "missing-key"),
                (1, "missing-key"),
                (1, "missing-key"),
                (1, "missing-key")
            ]
        );
        assert_eq!(config, Config::default());
        let (_, diagnostics) = parse_config(b"format_version = 1\nschema = \"s.toml\"\n");
        assert_eq!(
            diagnostics[0].message,
            "the configuration has no `[documents]`"
        );
    }

    #[test]
    fn rejects_paths_outside_the_project_and_values_of_the_wrong_type() {
        let text = "format_version = \"1\"\nschema = \"/etc/schema.toml\"\n[documents]\nformat_version = 1\ninclude = [\"../other/*.mara.md\", \"docs/*.mara.md\", 3, \"./x.mara.md\", \"docs//a.mara.md\"]\n";
        let (config, diagnostics) = parse_config(text.as_bytes());
        assert_eq!(
            codes(&diagnostics),
            [
                (1, "invalid-value"),
                (2, "invalid-value"),
                (5, "invalid-value"),
                (5, "invalid-value"),
                (5, "invalid-value"),
                (5, "invalid-value")
            ]
        );
        assert_eq!(config.schema, None);
        assert_eq!(patterns(&config), ["docs/*.mara.md"]);
    }

    #[test]
    fn one_problem_never_hides_another() {
        let text = "format_version = \"one\"\nschema = \"../schema.toml\"\ndocuments = [\"docs\"]\nname = 1\n";
        let (config, diagnostics) = parse_config(text.as_bytes());
        assert_eq!(
            codes(&diagnostics),
            [
                (1, "invalid-value"),
                (2, "invalid-value"),
                (3, "invalid-value"),
                (4, "unknown-key")
            ]
        );
        assert_eq!(config, Config::default());
    }

    #[test]
    fn stops_at_a_newer_format_version() {
        let (config, diagnostics) =
            parse_config(b"format_version = 2\nschema = 5\nfuture = true\n");
        assert_eq!(codes(&diagnostics), [(1, "unsupported-format-version")]);
        assert!(
            diagnostics[0].message.contains("upgrade Mara"),
            "{}",
            diagnostics[0].message
        );
        assert_eq!(config, Config::default());
    }

    #[test]
    fn reads_no_document_of_a_newer_document_syntax() {
        let text = "format_version = 1\nschema = \"s.toml\"\n[documents]\nformat_version = 2\ninclude = [\"*.mara.md\"]\nfuture = 1\n";
        let (config, diagnostics) = parse_config(text.as_bytes());
        assert_eq!(codes(&diagnostics), [(4, "unsupported-format-version")]);
        assert_eq!(config.include, None);
        assert_eq!(config.schema.unwrap().path, "s.toml");
    }

    #[test]
    fn reports_syntax_errors_with_their_line() {
        let (config, diagnostics) = parse_config(b"format_version = 1\n\nschema = \n");
        assert_eq!(codes(&diagnostics), [(3, "toml-syntax")]);
        assert_eq!(config, Config::default());
        let (_, diagnostics) = parse_config(b"format_version = 1\nschema = \"\xff\"\n");
        assert_eq!(codes(&diagnostics), [(2, "toml-syntax")]);
    }

    #[test]
    fn recognizes_roots() {
        assert_eq!(Root::recognize(true, false), Some(Root::Project));
        assert_eq!(Root::recognize(true, true), Some(Root::Project));
        assert_eq!(Root::recognize(false, true), Some(Root::Legacy));
        assert_eq!(Root::recognize(false, false), None);
    }
}
