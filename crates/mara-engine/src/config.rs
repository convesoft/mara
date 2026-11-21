//! The project configuration and project roots (DES-PROJECT-CONFIGURATION in
//! `docs/project.mara.md`).

mod rules;

use crate::diagnostic::Diagnostic;
use crate::hash::Hash;
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
    /// The programs and their consumers (DES-RULE-CONSUMERS in `docs/rules/index.mara.md`); `None`
    /// without `[rules]`.
    pub rules: Option<Rules>,
    /// The code languages that `[code]` declares, in the configuration's order, each valid
    /// one (DES-CODE-MARKERS); none without `[code]`.
    pub code: Vec<Language>,
}

/// A code language: the files it covers, its comment prefixes and the grammar it pins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Language {
    pub name: String,
    /// Include patterns of its code files; an invalid pattern is left out.
    pub include: Vec<PathSetting>,
    /// Comment prefixes, as written.
    pub prefixes: Vec<String>,
    /// The grammar of the grammar level (DES-GRAMMAR-ASSETS); `None` at the basic level, also
    /// when the pin is invalid.
    pub grammar: Option<GrammarPin>,
}

/// The version and checksum of a grammar that `code add` stored in the per-user cache, and the
/// repository it came from (DES-GRAMMAR-ASSETS).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrammarPin {
    /// Letters, digits, `.`, `-`, `_` and `+`, not starting with `.`.
    pub version: String,
    /// The BLAKE3 hash of the BLAKE3 hashes of the `.wasm` file and of `tags.scm`, in that order.
    pub checksum: Hash,
    /// The repository the user gave `code add`, in the one form of [`repository`]; `None` for a
    /// grammar of the built-in catalog.
    pub repository: Option<String>,
    /// The configuration line of `grammar`, where its problems are reported.
    pub line: usize,
}

/// The prefix of a grammar checksum's text.
pub const CHECKSUM_PREFIX: &str = "blake3:";

/// Whether `name` can name a code language: lowercase ASCII letters and digits in parts joined by
/// single `_` or `-`.
pub fn is_language_name(name: &str) -> bool {
    is_name(name)
}

/// Whether `version` is a grammar version: letters, digits, `.`, `-`, `_` and `+`, not starting
/// with `.`.
pub fn is_grammar_version(version: &str) -> bool {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+');
    !version.is_empty() && !version.starts_with('.') && version.chars().all(allowed)
}

/// A URL that `code add` fetches from, read by [`repository`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    /// Its one form: the scheme and the host in lowercase, an IPv6 address in its shortest form,
    /// the scheme's default port left out and no `/` at the end. Two URLs of the same form name
    /// the same repository.
    pub url: String,
    /// Whether the scheme is `https`.
    pub secure: bool,
    /// The host in lowercase; an IPv6 address without brackets.
    pub host: String,
    /// The port, the scheme's default when the URL names none.
    pub port: u16,
}

/// Reads a grammar repository, or a file URL of the catalog (DES-GRAMMAR-ASSETS): an `https://`
/// URL, or an `http://` URL whose host is a loopback address (`127.0.0.0/8` or `[::1]`). Its host
/// is a name of letters, digits, `-` and `.`, or an IP address, a port is 1 to 65535 in at most 5
/// digits, and it has no credentials, query, fragment, backslash, space or control character,
/// since `code add` appends file paths to it and pins it. Anything else is refused, with a reason
/// that names no option.
pub fn repository(url: &str) -> Result<Repository, String> {
    let refuse = |why: &str| Err(format!("`{url}` is no grammar repository: {why}"));
    let (secure, rest) = if let Some(rest) = strip_prefix_ignore_case(url, "https://") {
        (true, rest)
    } else if let Some(rest) = strip_prefix_ignore_case(url, "http://") {
        (false, rest)
    } else {
        return refuse("give an `https://` URL, or an `http://` URL of a loopback address");
    };
    let odd = |c: char| c.is_whitespace() || c.is_control() || matches!(c, '?' | '#' | '\\');
    if url.chars().any(odd) {
        return refuse("it holds a space, a backslash, a query or a fragment");
    }
    let (authority, path) = match rest.find('/') {
        Some(at) => rest.split_at(at),
        None => (rest, ""),
    };
    if authority.contains('@') {
        return refuse("it holds credentials, which a pin would publish");
    }
    let (host, port) = match authority.strip_prefix('[') {
        Some(bracketed) => {
            let Some((address, port)) = bracketed.split_once(']') else {
                return refuse("its host is no IPv6 address");
            };
            let Ok(address) = address.parse::<std::net::Ipv6Addr>() else {
                return refuse("its host is no IPv6 address");
            };
            (address.to_string(), port)
        }
        None => {
            let (host, port) = authority.split_at(authority.find(':').unwrap_or(authority.len()));
            // A final root dot names the same host.
            let host = host.strip_suffix('.').unwrap_or(host);
            let named = !host.is_empty()
                && host.split('.').all(|label| !label.is_empty())
                && (host.chars()).all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'));
            if !named {
                return refuse(
                    "its host is no name of letters, digits, `-` and `.`, and no IP address",
                );
            }
            (host.to_ascii_lowercase(), port)
        }
    };
    let default = if secure { 443 } else { 80 };
    let port = match port.strip_prefix(':') {
        None if port.is_empty() => default,
        Some(digits)
            if (1..=5).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit()) =>
        {
            match digits.parse::<u16>() {
                Ok(port) if port > 0 => port,
                _ => return refuse("its port is not 1 to 65535"),
            }
        }
        _ => return refuse("its port is not 1 to 65535"),
    };
    if !secure && !is_loopback(&host) {
        return Err(format!(
            "`{url}` is plain HTTP to a host that is not a loopback address, which anyone on the \
             path could answer: use `https://`"
        ));
    }
    let shown = match host.contains(':') {
        true => format!("[{host}]"),
        false => host.clone(),
    };
    let port_text = match port == default {
        true => String::new(),
        false => format!(":{port}"),
    };
    let scheme = if secure { "https" } else { "http" };
    Ok(Repository {
        url: format!(
            "{scheme}://{shown}{port_text}{}",
            path.trim_end_matches('/')
        ),
        secure,
        host,
        port,
    })
}

/// Checks a grammar repository as [`repository`] reads it.
pub fn check_repository(url: &str) -> Result<(), String> {
    repository(url).map(drop)
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// Whether `host` is a loopback address: an IPv4 address in `127.0.0.0/8` or the IPv6 address
/// `::1`. A name such as `localhost` is none: what it resolves to is not the URL's to say.
fn is_loopback(host: &str) -> bool {
    match host.parse::<std::net::IpAddr>() {
        Ok(address) => address.is_loopback(),
        Err(_) => false,
    }
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
        file.unknown_keys(
            settings,
            &["include", "comment_prefixes", "grammar"],
            &place,
        );
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
        let grammar = settings
            .get_key_value("grammar")
            .and_then(|(key, value)| grammar(file, key.span(), value));
        if let (true, Some(include), Some(prefixes)) = (named, include, prefixes) {
            languages.push(Language {
                name: name.to_owned(),
                include,
                prefixes,
                grammar,
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

/// A language's grammar pin, `{ version = "…", checksum = "blake3:…" }` and optionally
/// `repository = "…"`, whose key is at `at`.
/// `None` when it is invalid: the language is then read at the basic level.
fn grammar(file: &mut TomlFile, at: std::ops::Range<usize>, value: &Value) -> Option<GrammarPin> {
    const PLACE: &str = "`grammar`";
    let table = file.table("grammar", value)?;
    file.unknown_keys(table, &["version", "checksum", "repository"], PLACE);
    let version = file
        .required(table, "version", at.clone(), PLACE)
        .and_then(|value| {
            let version = file.string("version", value)?;
            if !is_grammar_version(version) {
                let message = format!(
                    "a grammar version is letters, digits, `.`, `-`, `_` and `+`, not starting \
                     with `.`: `{version}`"
                );
                file.error("invalid-value", value.span(), message);
                return None;
            }
            Some(version.to_owned())
        });
    let checksum = file
        .required(table, "checksum", at.clone(), PLACE)
        .and_then(|value| {
            let text = file.string("checksum", value)?;
            let hash = text.strip_prefix(CHECKSUM_PREFIX).and_then(Hash::from_hex);
            if hash.is_none() {
                let message = format!(
                    "a grammar checksum is `{CHECKSUM_PREFIX}` and 64 lowercase hexadecimal \
                     digits: `{text}`"
                );
                file.error("invalid-value", value.span(), message);
            }
            hash
        });
    let repository = match table.get("repository") {
        None => Some(None),
        Some(value) => file
            .string("repository", value)
            .and_then(|url| match repository(url) {
                Ok(read) => Some(Some(read.url)),
                Err(reason) => {
                    file.error("invalid-value", value.span(), reason);
                    None
                }
            }),
    };
    Some(GrammarPin {
        version: version?,
        checksum: checksum?,
        repository: repository?,
        line: file.line(at.start),
    })
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
        diagnostics
            .iter()
            .map(|d| (d.line, d.built_in_code()))
            .collect()
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
                (23, "invalid-value"),
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
    fn reads_grammar_pins_and_reads_a_language_with_an_invalid_one_at_the_basic_level() {
        let base = "format_version = 1\nschema = \"s.toml\"\n[documents]\nformat_version = 1\ninclude = []\n[code]\nformat_version = 1\n";
        let hex = "ab".repeat(32);
        let languages = [
            format!("grammar = {{ version = \"0.24.2\", checksum = \"blake3:{hex}\" }}"),
            format!("grammar = {{ version = \"../x\", checksum = \"blake3:{hex}\" }}"),
            "grammar = { version = \"1\", checksum = \"sha256:00\" }".to_owned(),
            "grammar = { version = \"1\", extra = 1 }".to_owned(),
            "[code.languages.e.grammar]\nversion = \"1\"".to_owned(),
            format!(
                "grammar = {{ version = \"1\", checksum = \"blake3:{hex}\", repository = \"HTTP://127.0.0.1:8080/g/\" }}"
            ),
            format!(
                "grammar = {{ version = \"1\", checksum = \"blake3:{hex}\", repository = \"http://example.com/g\" }}"
            ),
        ];
        let code: String = ["a", "b", "c", "d", "e", "f", "g"]
            .iter()
            .zip(&languages)
            .map(|(name, grammar)| {
                format!(
                    "[code.languages.{name}]\ninclude = []\ncomment_prefixes = [\"//\"]\n{grammar}\n"
                )
            })
            .collect();
        let (config, diagnostics) = parse_config(format!("{base}{code}").as_bytes());
        let pins: Vec<_> = config.code.iter().map(|l| l.grammar.clone()).collect();
        assert_eq!(
            pins,
            [
                Some(GrammarPin {
                    version: "0.24.2".to_owned(),
                    checksum: Hash::from_hex(&hex).unwrap(),
                    repository: None,
                    line: 11,
                }),
                None,
                None,
                None,
                None,
                Some(GrammarPin {
                    version: "1".to_owned(),
                    checksum: Hash::from_hex(&hex).unwrap(),
                    repository: Some("http://127.0.0.1:8080/g".to_owned()),
                    line: 32,
                }),
                None,
            ]
        );
        assert_eq!(
            codes(&diagnostics),
            [
                (15, "invalid-value"),
                (19, "invalid-value"),
                (23, "unknown-key"),
                (23, "missing-key"),
                (27, "missing-key"),
                (36, "invalid-value"),
            ]
        );
    }

    #[test]
    fn takes_https_repositories_and_plain_http_only_to_loopback_addresses() {
        for url in [
            "https://github.com/tree-sitter/tree-sitter-rust/releases/download",
            "HTTPS://example.com",
            "http://127.0.0.1:8080/grammars",
            "http://127.1.2.3",
            "http://[::1]:9000/",
        ] {
            assert_eq!(check_repository(url), Ok(()), "{url}");
        }
        for url in [
            "http://example.com/g",
            "http://localhost:8080",
            "http://128.0.0.1",
            "http://[::2]",
            "ftp://127.0.0.1/g",
            "file:///tmp/g",
            "127.0.0.1:8080",
            "https://",
            "https://user:secret@example.com/g",
            "https://example.com/g?x=1",
            "https://example.com/g#x",
            "https://example.com/a b",
            "https://example.org:abc/g",
            "https://example.org:/g",
            "https://example.org:0/g",
            "https://example.org:65536/g",
            "https://example.org:123456/g",
            "https://example.org:443:443/g",
            "https://exa_mple.org/g",
            "https://example..org/g",
            "https://%65xample.org/g",
            "https://[::1/g",
            "https://[example.org]/g",
            "https://example.org\\@other.org/g",
            "",
        ] {
            assert!(check_repository(url).is_err(), "{url}");
        }
    }

    #[test]
    fn gives_each_repository_one_form() {
        for (url, form, port) in [
            ("https://example.com/g", "https://example.com/g", 443),
            ("HTTPS://Example.COM:443/g/", "https://example.com/g", 443),
            (
                "https://example.com:8443//",
                "https://example.com:8443",
                8443,
            ),
            ("https://Example.org.:443/g", "https://example.org/g", 443),
            (
                "https://example.com/G/Rust/",
                "https://example.com/G/Rust",
                443,
            ),
            ("http://127.0.0.1:80/", "http://127.0.0.1", 80),
            ("Http://127.0.0.1:0080/g", "http://127.0.0.1/g", 80),
            ("http://[0:0:0:0:0:0:0:1]:9/x", "http://[::1]:9/x", 9),
        ] {
            let read = repository(url).unwrap();
            assert_eq!((read.url.as_str(), read.port), (form, port), "{url}");
        }
        let read = repository("http://[::1]:9").unwrap();
        assert_eq!((read.host.as_str(), read.secure), ("::1", false));
    }

    #[test]
    fn recognizes_roots() {
        assert_eq!(Root::recognize(true, false), Some(Root::Project));
        assert_eq!(Root::recognize(true, true), Some(Root::Project));
        assert_eq!(Root::recognize(false, true), Some(Root::Legacy));
        assert_eq!(Root::recognize(false, false), None);
    }
}
