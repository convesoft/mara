//! The project configuration and project roots (DES-PROJECT-CONFIGURATION in
//! `docs/project.mara.md`).

mod rules;

use crate::diagnostic::Diagnostic;
use crate::schema::is_name;
use crate::toml_file::{Table, TomlFile, Value};

pub use rules::{Consumer, DEFAULT_TIME_LIMIT_SECONDS, Entry, EntryKind, Named, Rules};

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
    /// The programs and their consumers (DES-RULE-CONSUMERS in `docs/rules.mara.md`); `None`
    /// without `[rules]`.
    pub rules: Option<Rules>,
    /// The code languages that `[code]` declares, in the configuration's order, each valid
    /// one (DES-CODE-MARKERS); none without `[code]`.
    pub code: Vec<Language>,
}

/// A code language at the basic level: the files it covers and its comment prefixes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Language {
    pub name: String,
    /// Include patterns of its code files; an invalid pattern is left out.
    pub include: Vec<PathSetting>,
    /// Comment prefixes, as written.
    pub prefixes: Vec<String>,
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
    file.unknown_keys(root, rules::ROOT_KEYS, PLACE);
    config.rules = rules::read(file, root);
    if let Some((key, value)) = root.get_key_value("code")
        && let Some(code) = file.table("code", value)
    {
        config.code = languages(file, key.span(), code);
    }
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

/// The languages of `[code]`, whose header is at `at`, in the file's order: none without
/// `languages`. A language with an invalid name, prefix list or include list is left out.
fn languages(file: &mut TomlFile, at: std::ops::Range<usize>, code: &Table) -> Vec<Language> {
    const CODE: &str = "`[code]`";
    if !file.format_version(code, at, CODE) {
        return Vec::new();
    }
    file.unknown_keys(code, &["format_version", "languages"], CODE);
    let Some(value) = code.get("languages") else {
        return Vec::new();
    };
    let Some(table) = file.table("languages", value) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = table.iter().collect();
    entries.sort_by_key(|(key, _)| key.span().start);
    let mut languages = Vec::new();
    for (key, value) in entries {
        let name = key.get_ref().as_ref();
        let Some(settings) = file.table(name, value) else {
            continue;
        };
        let place = format!("`[code.languages.{name}]`");
        let named = is_name(name);
        if !named {
            file.error(
                "invalid-value",
                key.span(),
                format!(
                    "the language name `{name}` must be lowercase ASCII letters and digits in \
                     parts joined by single `_` or `-`"
                ),
            );
        }
        file.unknown_keys(settings, &["include", "comment_prefixes"], &place);
        let include = file
            .required(settings, "include", key.span(), &place)
            .and_then(|value| file.array("include", value))
            .map(|patterns| {
                patterns
                    .iter()
                    .filter_map(|pattern| path(file, "include", pattern))
                    .collect::<Vec<_>>()
            });
        let prefixes = file
            .required(settings, "comment_prefixes", key.span(), &place)
            .and_then(|value| prefixes(file, value));
        if let (true, Some(include), Some(prefixes)) = (named, include, prefixes) {
            languages.push(Language {
                name: name.to_owned(),
                include,
                prefixes,
            });
        }
    }
    languages
}

/// Comment prefixes: at least one, each non-empty without spaces, tabs or line breaks, none
/// twice. `None` when any is invalid.
fn prefixes(file: &mut TomlFile, value: &Value) -> Option<Vec<String>> {
    const KEY: &str = "comment_prefixes";
    let values = file.array(KEY, value)?;
    if values.is_empty() {
        let message = format!("`{KEY}` must list at least one comment prefix");
        file.error("invalid-value", value.span(), message);
        return None;
    }
    let mut prefixes: Vec<String> = Vec::new();
    let mut valid = true;
    for value in values {
        let Some(prefix) = file.string(KEY, value) else {
            valid = false;
            continue;
        };
        if prefix.is_empty() || prefix.contains([' ', '\t', '\n', '\r']) {
            let message = format!(
                "a comment prefix is not empty and has no space, tab or line break: `{prefix}`"
            );
            file.error("invalid-value", value.span(), message);
            valid = false;
        } else if prefixes.iter().any(|known| known == prefix) {
            let message = format!("the comment prefix `{prefix}` is listed twice");
            file.error("duplicate-name", value.span(), message);
            valid = false;
        } else {
            prefixes.push(prefix.to_owned());
        }
    }
    valid.then_some(prefixes)
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
        let text = "format_version = 1\nschema = \".mara/schema.toml\"\nname = \"x\"\n\n[documents]\nformat_version = 1\ninclude = [\"docs/*.mara.md\"]\nexclude = []\n\n[daemon]\nport = 1\n";
        let (config, diagnostics) = parse_config(text.as_bytes());
        assert_eq!(
            codes(&diagnostics),
            [(3, "unknown-key"), (8, "unknown-key"), (10, "unknown-key")]
        );
        assert_eq!(diagnostics[0].path, CONFIG_PATH);
        assert_eq!(
            diagnostics[0].to_string(),
            ".mara/config.toml:3: error[unknown-key]: unknown key `name` in the configuration; the keys are `format_version`, `schema`, `documents`, `code`, `rules`, `queries`, `views`, `invariants`, `gates`"
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
    fn reads_code_languages_in_their_order_and_leaves_out_invalid_ones() {
        let base = "format_version = 1\nschema = \"s.toml\"\n[documents]\nformat_version = 1\ninclude = []\n";
        let code = "[code]\nformat_version = 1\n\
                    [code.languages.typescript]\ninclude = [\"web/**/*.ts\"]\ncomment_prefixes = [\"//\", \"*\"]\n\
                    [code.languages.rust]\ninclude = [\"src/**/*.rs\", \"../x.rs\"]\ncomment_prefixes = [\"//\"]\n\
                    [code.languages.Bad]\ninclude = []\ncomment_prefixes = [\"#\"]\n\
                    [code.languages.spaced]\ninclude = []\ncomment_prefixes = [\"# x\", \"#\", \"#\"]\n\
                    [code.languages.empty]\ninclude = []\ncomment_prefixes = []\ngrammar = 1\n\
                    [code.languages.missing]\n";
        let (config, diagnostics) = parse_config(format!("{base}{code}").as_bytes());
        let names: Vec<&str> = config.code.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["typescript", "rust"]);
        assert_eq!(config.code[0].prefixes, ["//", "*"]);
        let rust: Vec<&str> = config.code[1]
            .include
            .iter()
            .map(|p| p.path.as_str())
            .collect();
        assert_eq!(rust, ["src/**/*.rs"]);
        assert_eq!(
            codes(&diagnostics),
            [
                (12, "invalid-value"),
                (14, "invalid-value"),
                (19, "invalid-value"),
                (19, "duplicate-name"),
                (22, "invalid-value"),
                (23, "unknown-key"),
                (24, "missing-key"),
                (24, "missing-key"),
            ]
        );
        // `[code]` without a language declares none, and is no problem.
        let (config, diagnostics) =
            parse_config(format!("{base}[code]\nformat_version = 1\n").as_bytes());
        assert_eq!((config.code.len(), codes(&diagnostics)), (0, vec![]));
        let (config, diagnostics) =
            parse_config(format!("{base}[code]\nformat_version = 2\n").as_bytes());
        assert_eq!(
            (config.code.len(), codes(&diagnostics)),
            (0, vec![(7, "unsupported-format-version")])
        );
    }

    #[test]
    fn recognizes_roots() {
        assert_eq!(Root::recognize(true, false), Some(Root::Project));
        assert_eq!(Root::recognize(true, true), Some(Root::Project));
        assert_eq!(Root::recognize(false, true), Some(Root::Legacy));
        assert_eq!(Root::recognize(false, false), None);
    }
}
