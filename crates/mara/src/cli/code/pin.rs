//! The configuration's new text with a grammar pinned (DES-GRAMMAR-ASSETS): the least change that
//! pins it, checked by reading the result back.

use std::ops::Range;

use mara_engine::config::{CHECKSUM_PREFIX, Config, GrammarPin, parse_config};
use toml::Spanned;
use toml::de::{DeTable, DeValue};

/// What a language that the configuration does not declare is declared with.
#[derive(Clone)]
pub struct Declaration<'d> {
    pub include: Vec<String>,
    pub comment_prefixes: &'d [String],
}

/// `text` with `pin` as the grammar of `language`: the existing pin's inline table replaced, or a
/// `grammar` line added at the end of the language's table, or, with `declare`, the language's
/// table appended with `[code]` when that is missing. An error says why no such text was made, as
/// when the result would not read back as this pin with every other setting unchanged.
pub fn pinned(
    text: &str,
    language: &str,
    pin: &GrammarPin,
    declare: Option<Declaration>,
) -> Result<String, String> {
    let line = pin_line(pin);
    let root = DeTable::parse(text).map_err(|error| error.message().to_owned())?;
    let root = root.get_ref();
    let code = table(root, "code");
    let settings = code
        .and_then(|code| table(code, "languages"))
        .and_then(|languages| languages.get(language));
    let edited = match (settings, declare) {
        (Some(settings), _) => {
            let DeValue::Table(settings_table) = settings.get_ref() else {
                return Err(format!("`code.languages.{language}` is no table"));
            };
            if text[settings.span()].starts_with('{') {
                return Err(format!(
                    "`{language}` is declared in an inline table: write it as a \
                     `[code.languages.{language}]` table"
                ));
            }
            match settings_table.get("grammar") {
                Some(grammar) if text[grammar.span()].starts_with('{') => replace(
                    text,
                    grammar.span(),
                    &line["grammar = ".len()..line.len() - 1],
                ),
                Some(_) => {
                    return Err(format!(
                        "the pin of `{language}` is a table of its own: write it as \
                         `grammar = {{ … }}` in `[code.languages.{language}]`, or remove it"
                    ));
                }
                None => {
                    let end = (settings_table.iter())
                        .map(|(key, value)| key.span().end.max(value.span().end))
                        .max()
                        .unwrap_or(settings.span().end);
                    insert_line(text, end, &line)
                }
            }
        }
        (None, Some(declaration)) => {
            let mut out = text.to_owned();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if code.is_none() {
                out.push_str("\n[code]\nformat_version = 1\n");
            }
            let list = |values: &[String]| {
                let values = values
                    .iter()
                    .map(|value| toml::Value::String(value.clone()));
                toml::Value::Array(values.collect()).to_string()
            };
            out.push_str(&format!(
                "\n[code.languages.{language}]\ninclude = {}\ncomment_prefixes = {}\n{line}",
                list(&declaration.include),
                list(declaration.comment_prefixes)
            ));
            out
        }
        (None, None) => return Err(format!("the configuration does not declare `{language}`")),
    };
    read_back(text, &edited, language, pin)?;
    Ok(edited)
}

/// `grammar = { … }` and a line end.
fn pin_line(pin: &GrammarPin) -> String {
    let repository = match &pin.repository {
        Some(url) => format!(", repository = {}", quote(url)),
        None => String::new(),
    };
    format!(
        "grammar = {{ version = {}, checksum = \"{CHECKSUM_PREFIX}{}\"{repository} }}\n",
        quote(&pin.version),
        pin.checksum
    )
}

/// `value` as a TOML string, written by the `toml` crate.
fn quote(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

fn table<'t, 'i>(table: &'t DeTable<'i>, key: &str) -> Option<&'t DeTable<'i>> {
    match table.get(key).map(Spanned::get_ref) {
        Some(DeValue::Table(table)) => Some(table),
        _ => None,
    }
}

fn replace(text: &str, span: Range<usize>, with: &str) -> String {
    format!("{}{with}{}", &text[..span.start], &text[span.end..])
}

/// `text` with `line` after the line that holds byte `at`.
fn insert_line(text: &str, at: usize, line: &str) -> String {
    match text[at..].find('\n') {
        Some(offset) => {
            let end = at + offset + 1;
            format!("{}{line}{}", &text[..end], &text[end..])
        }
        None => format!("{text}\n{line}"),
    }
}

/// Requires that `edited` reads as `original` with `pin` as the grammar of `language`, which
/// it may also declare, and with no new problem.
fn read_back(original: &str, edited: &str, language: &str, pin: &GrammarPin) -> Result<(), String> {
    let (before, before_problems) = parse_config(original.as_bytes());
    let (after, after_problems) = parse_config(edited.as_bytes());
    let refuse = || {
        Err(format!(
            "pinning the grammar would change more than the pin of `{language}`: edit \
             .mara/config.toml by hand"
        ))
    };
    if after_problems.len() > before_problems.len() {
        return refuse();
    }
    let mut expected = settings(&before);
    let found = settings(&after);
    match expected.iter_mut().find(|setting| setting.0 == language) {
        Some(setting) => setting.3 = Some(pin_key(pin)),
        None => match found.last() {
            Some(last) if last.0 == language && last.3 == Some(pin_key(pin)) => {
                expected.push(last.clone())
            }
            _ => return refuse(),
        },
    }
    if found != expected {
        return refuse();
    }
    let paths = |config: &Config| {
        let include = config.include.iter().flatten().map(|p| p.path.clone());
        (
            config.schema.as_ref().map(|s| s.path.clone()),
            include.collect::<Vec<_>>(),
        )
    };
    if paths(&before) != paths(&after) || before.rules.is_some() != after.rules.is_some() {
        return refuse();
    }
    Ok(())
}

/// A pin without its line.
type PinKey = (String, String, Option<String>);
/// A language's name, include patterns, comment prefixes and pin, without lines.
type Setting = (String, Vec<String>, Vec<String>, Option<PinKey>);

fn pin_key(pin: &GrammarPin) -> PinKey {
    (
        pin.version.clone(),
        pin.checksum.to_string(),
        pin.repository.clone(),
    )
}

fn settings(config: &Config) -> Vec<Setting> {
    (config.code.iter())
        .map(|language| {
            let include = language.include.iter().map(|p| p.path.clone()).collect();
            let pin = language.grammar.as_ref().map(pin_key);
            (
                language.name.clone(),
                include,
                language.prefixes.clone(),
                pin,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use mara_engine::hash::Hash;

    use super::*;

    const BASE: &str = "format_version = 1\nschema = \"s.toml\"\n\n[documents]\nformat_version = 1\ninclude = [\"docs/**/*.mara.md\"]\n";

    fn pin(repository: Option<&str>) -> GrammarPin {
        GrammarPin {
            version: "0.24.2".to_owned(),
            checksum: Hash::of_bytes(b"x"),
            repository: repository.map(str::to_owned),
            line: 0,
        }
    }

    #[test]
    fn adds_a_pin_line_at_the_end_of_the_language_table() {
        let text = format!(
            "{BASE}\n[code]\nformat_version = 1\n\n[code.languages.rust]\ninclude = [\"src/**/*.rs\"]\ncomment_prefixes = [\"//\"]\n\n# Python\n[code.languages.python]\ninclude = []\ncomment_prefixes = [\"#\"]\n"
        );
        let pin = pin(Some("http://127.0.0.1:9/g"));
        let edited = pinned(&text, "rust", &pin, None).unwrap();
        let line = format!(
            "comment_prefixes = [\"//\"]\ngrammar = {{ version = \"0.24.2\", checksum = \"blake3:{}\", repository = \"http://127.0.0.1:9/g\" }}\n\n# Python\n",
            pin.checksum
        );
        assert!(edited.contains(&line), "{edited}");
        assert_eq!(
            edited.len(),
            text.len() + line.len() - "comment_prefixes = [\"//\"]\n\n# Python\n".len()
        );
    }

    #[test]
    fn replaces_an_inline_pin_and_keeps_everything_around_it() {
        let old = format!(
            "grammar = {{ version = \"0.1\", checksum = \"blake3:{}\" }} # pinned",
            "ab".repeat(32)
        );
        let text = format!(
            "{BASE}[code]\nformat_version = 1\n[code.languages.rust]\ninclude = []\ncomment_prefixes = [\"//\"]\n{old}\n"
        );
        let pin = pin(None);
        let edited = pinned(&text, "rust", &pin, None).unwrap();
        assert_eq!(
            edited,
            text.replace(
                &old,
                &format!(
                    "grammar = {{ version = \"0.24.2\", checksum = \"blake3:{}\" }} # pinned",
                    pin.checksum
                )
            )
        );
    }

    #[test]
    fn declares_a_missing_language_and_code_table() {
        let prefixes = ["//".to_owned(), "///".to_owned()];
        for text in [
            BASE.to_owned(),
            BASE.trim_end().to_owned(),
            format!("{BASE}[code]\nformat_version = 1\n"),
        ] {
            let declaration = Declaration {
                include: vec!["**/*.rs".to_owned()],
                comment_prefixes: &prefixes,
            };
            let edited = pinned(&text, "rust", &pin(None), Some(declaration)).unwrap();
            assert!(edited.starts_with(text.as_str()), "{edited}");
            let (config, problems) = parse_config(edited.as_bytes());
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(config.code[0].prefixes, prefixes);
        }
    }

    #[test]
    fn refuses_forms_it_cannot_edit_in_place() {
        let table_pin = format!(
            "{BASE}[code]\nformat_version = 1\n[code.languages.rust]\ninclude = []\ncomment_prefixes = [\"//\"]\n[code.languages.rust.grammar]\nversion = \"1\"\nchecksum = \"blake3:{}\"\n",
            "ab".repeat(32)
        );
        let inline = format!(
            "{BASE}[code]\nformat_version = 1\nlanguages = {{ rust = {{ include = [], comment_prefixes = [\"//\"] }} }}\n"
        );
        let dotted = format!(
            "{BASE}[code]\nformat_version = 1\n[code.languages]\nrust.include = []\nrust.comment_prefixes = [\"//\"]\n"
        );
        for text in [table_pin, inline, dotted, BASE.to_owned()] {
            assert!(pinned(&text, "rust", &pin(None), None).is_err(), "{text}");
        }
    }
}
