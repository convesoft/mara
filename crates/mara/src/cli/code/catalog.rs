//! The built-in grammar catalog of `code add` (DES-GRAMMAR-ASSETS): per language, the release it
//! pins, where its two files are fetched from, their checksum, and what declares the language in a
//! configuration that lacks it.
//!
//! The shipped catalog is `catalog.toml` (DES-GRAMMAR-ASSETS). Debug builds read another catalog
//! from the file that `MARA_TEST_GRAMMAR_CATALOG` names, so that tests fetch from a loopback server
//! of their own.

use mara::hooks::{self, Hook};
use mara_engine::config::{check_repository, is_grammar_version, is_language_name};
use mara_engine::hash::Hash;

/// A grammar of the catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub language: String,
    pub version: String,
    /// The URL of its `.wasm` release file.
    pub wasm: String,
    /// The URL of its `queries/tags.scm` at the same release.
    pub tags: String,
    /// The checksum of the two files (DES-GRAMMAR-ASSETS).
    pub checksum: Hash,
    /// The extensions of its files, without the dot: each gives the include pattern `**/*.<ext>`.
    pub extensions: Vec<String>,
    pub comment_prefixes: Vec<String>,
}

/// The catalog this build ships, in the form [`parse`] reads.
const BUILT_IN: &str = include_str!("catalog.toml");

/// The catalog: the built-in one, or in a debug build the one that `MARA_TEST_GRAMMAR_CATALOG`
/// names. An error is why it cannot be read.
pub fn load() -> Result<Vec<Entry>, String> {
    if let Some(path) = hooks::path(Hook::GrammarCatalog) {
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read the test catalog {}: {error}", path.display()))?;
        return parse(&text).map_err(|why| format!("the test catalog {}: {why}", path.display()));
    }
    parse(BUILT_IN).map_err(|why| format!("the built-in grammar catalog: {why}"))
}

/// A catalog of `[languages.<name>]` tables, each with the fields of [`Entry`] (`version`, `wasm`,
/// `tags`, `checksum` as `blake3:<hex>`, `extensions`, `comment_prefixes`).
fn parse(text: &str) -> Result<Vec<Entry>, String> {
    use mara_engine::config::CHECKSUM_PREFIX;
    use toml::de::{DeTable, DeValue};
    let root = DeTable::parse(text).map_err(|error| error.message().to_owned())?;
    let Some(DeValue::Table(languages)) = root.get_ref().get("languages").map(|v| v.get_ref())
    else {
        return Err("no `[languages]`".to_owned());
    };
    let mut entries = Vec::new();
    for (language, fields) in languages {
        let language = language.get_ref().to_string();
        let DeValue::Table(fields) = fields.get_ref() else {
            return Err(format!("`{language}` is no table"));
        };
        let text = |key: &str| match fields.get(key).map(|v| v.get_ref()) {
            Some(DeValue::String(text)) => Ok(text.to_string()),
            _ => Err(format!("`{language}` has no text `{key}`")),
        };
        let list = |key: &str| -> Result<Vec<String>, String> {
            let Some(DeValue::Array(values)) = fields.get(key).map(|v| v.get_ref()) else {
                return Err(format!("`{language}` has no list `{key}`"));
            };
            (values.iter())
                .map(|value| match value.get_ref() {
                    DeValue::String(text) => Ok(text.to_string()),
                    _ => Err(format!("`{language}.{key}` holds no text")),
                })
                .collect()
        };
        let checksum = text("checksum")?;
        let entry = Entry {
            version: text("version")?,
            wasm: text("wasm")?,
            tags: text("tags")?,
            checksum: (checksum
                .strip_prefix(CHECKSUM_PREFIX)
                .and_then(Hash::from_hex))
            .ok_or(format!("`{language}` has no checksum `blake3:<hex>`"))?,
            extensions: list("extensions")?,
            comment_prefixes: list("comment_prefixes")?,
            language,
        };
        check(&entry)?;
        entries.push(entry);
    }
    Ok(entries)
}

/// What every entry holds, built in or not.
fn check(entry: &Entry) -> Result<(), String> {
    let language = &entry.language;
    let valid = is_language_name(language)
        && is_grammar_version(&entry.version)
        && !entry.extensions.is_empty()
        && (entry.extensions.iter()).all(|e| !e.is_empty() && e.chars().all(char::is_alphanumeric))
        && !entry.comment_prefixes.is_empty();
    if !valid {
        return Err(format!("`{language}` is no valid entry"));
    }
    check_repository(&entry.wasm)?;
    check_repository(&entry.tags)
}

/// The entry of `language`, if the catalog has one.
pub fn find<'c>(catalog: &'c [Entry], language: &str) -> Option<&'c Entry> {
    catalog.iter().find(|entry| entry.language == language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_built_in_catalog_and_refuses_invalid_entries() {
        assert!(parse(BUILT_IN).is_ok());
        let hex = "ab".repeat(32);
        let entry = |wasm: &str| {
            format!(
                "[languages.rust]\nversion = \"0.24.2\"\nwasm = \"{wasm}\"\n\
                 tags = \"https://example.com/tags.scm\"\nchecksum = \"blake3:{hex}\"\n\
                 extensions = [\"rs\"]\ncomment_prefixes = [\"//\"]\n"
            )
        };
        let entries = parse(&entry("https://example.com/r.wasm")).unwrap();
        assert_eq!(entries[0].language, "rust");
        assert_eq!(entries[0].checksum, Hash::from_hex(&hex).unwrap());
        assert!(parse(&entry("http://example.com/r.wasm")).is_err());
        assert!(parse(&entry("https://example.com/r.wasm").replace("rs\"]", "\"]")).is_err());
    }
}
