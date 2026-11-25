//! Grammars in a browser workspace, which loads none (DES-BROWSER-WORKSPACE in
//! `docs/hosts/index.mara.md`): every pinned grammar is `unavailable-grammar`, and its language's
//! files are scanned at the basic level (DES-GRAMMAR-ASSETS in `docs/code/index.mara.md`). The
//! native hosts' module, `grammar.rs`, loads them with tree-sitter, which this build leaves out.

use std::sync::Arc;

use mara_engine::code::Syntax;
use mara_engine::config::{CONFIG_PATH, Language};
use mara_engine::diagnostic::Diagnostic;

/// A pinned grammar that is not in the cache, does not match its pin or cannot be used.
pub const UNAVAILABLE_GRAMMAR: &str = "unavailable-grammar";

// @mara implements REQ-UNAVAILABLE-NOT-PARTIAL
/// The grammar that `language` pins: `None` without a pin, else its `unavailable-grammar`
/// diagnostic, since a browser loads no grammar.
pub fn get(language: &Language) -> Option<Result<Arc<Grammar>, Diagnostic>> {
    language.grammar.as_ref()?;
    Some(Err(unavailable(language, "")))
}

/// The `unavailable-grammar` diagnostic of the grammar that `language` pins, at the pin's
/// configuration line: the browser cannot load grammars. `reason` is the native hosts' and is
/// not used.
pub fn unavailable(language: &Language, reason: &str) -> Diagnostic {
    let _ = reason;
    let (line, version) = match &language.grammar {
        Some(pin) => (pin.line, pin.version.as_str()),
        None => (1, ""),
    };
    let message = format!(
        "the {} grammar {version} is not used: the browser cannot load grammars, so Mara scans \
         its files at the basic level",
        language.name
    );
    Diagnostic::error(UNAVAILABLE_GRAMMAR, CONFIG_PATH, line, message)
}

/// The `unavailable-grammar` diagnostic of a file that a grammar could not parse, which no
/// grammar of a browser does: the same as [`unavailable`].
pub fn fallback(language: &Language, path: &str, reason: &str) -> Diagnostic {
    let _ = (path, reason);
    unavailable(language, "")
}

/// A loaded grammar, of which a browser has none.
#[derive(Debug)]
pub struct Grammar(Never);

#[derive(Debug)]
enum Never {}

/// Why a file was not scanned at the grammar level.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseFailure {
    File(String),
    Abandoned,
}

impl Grammar {
    pub fn abandoned(&self) -> Option<String> {
        match self.0 {}
    }

    pub fn parse(&self, contents: &[u8], path: &str) -> Result<Syntax, ParseFailure> {
        let _ = (contents, path);
        match self.0 {}
    }
}
