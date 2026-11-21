//! Whether a document that an edit creates would be an input of the project (DES-INPUTS-AND-KEYS
//! in `docs/engine.mara.md`), judged from what the last request listed.

use std::sync::Arc;

use mara_engine::pattern::Patterns;

use super::{Inputs, ignored, join};
use crate::project::SYMBOLIC_LINK;
use crate::tracked::Kind;

/// Why a document created at a path would not be an input.
#[derive(Debug)]
pub struct Inadmissible {
    /// `not-an-input` or `symbolic-link`.
    pub code: &'static str,
    pub message: String,
}

/// The failure code of a path where an edit would create a document that is no input
/// (DES-OPERATIONS).
pub const NOT_AN_INPUT: &str = "not-an-input";

impl Inputs {
    /// Whether a document created at `path`, relative to the project root and not a document of
    /// the last snapshot, would be an input: a `*.mara.md` name that the include patterns match,
    /// that no `.gitignore` ignores, below no other project's root, `.git` or symbolic link, with
    /// no file there yet. Directories that do not exist yet count as created.
    pub fn admits(&self, path: &str) -> Result<(), Inadmissible> {
        let refuse = |why: String| {
            Err(Inadmissible {
                code: NOT_AN_INPUT,
                message: format!("{path} would not be a document of the project: {why}"),
            })
        };
        let linked = |link: &str| {
            Err(Inadmissible {
                code: SYMBOLIC_LINK,
                message: format!(
                    "{link} is a symbolic link, and Mara writes through none: name a path below \
                     the project's own directories"
                ),
            })
        };
        if !path.ends_with(".mara.md") {
            return refuse("the name of a document ends in `.mara.md`".to_owned());
        }
        let Some((_, project)) = &self.project else {
            return refuse("the project was not read".to_owned());
        };
        let include = project.config.include.iter().flatten();
        let patterns = Patterns::new(include.map(|pattern| pattern.path.as_str()));
        let unmatched =
            || refuse("the include patterns of .mara/config.toml do not match it".to_owned());
        let mut at = patterns.start();
        let mut ignores = Vec::new();
        let mut directory = String::new();
        let mut listed = true;
        let parts: Vec<&str> = path.split('/').collect();
        for (index, part) in parts.iter().enumerate() {
            let mut kind = None;
            if listed {
                let Some(listing) = self.state.inventory.directories.get(&directory) else {
                    return unmatched();
                };
                let find = |name: &str| {
                    let entries = &listing.entries;
                    let found = entries.binary_search_by(|entry| entry.name.as_str().cmp(name));
                    found.ok().map(|index| entries[index].kind)
                };
                if find(".gitignore").is_some() {
                    match self
                        .state
                        .inventory
                        .gitignores
                        .get(&directory)
                        .map(|tracked| &tracked.value)
                    {
                        Some(Ok(gitignore)) => {
                            ignores.push((directory.clone(), Arc::clone(gitignore)))
                        }
                        _ => {
                            let file = join(&directory, ".gitignore");
                            return refuse(format!(
                                "{file} cannot be applied, so nothing below it is an input"
                            ));
                        }
                    }
                }
                kind = find(part);
            }
            let child = join(&directory, part);
            at = patterns.advance(&at, part);
            if *part == ".git" {
                return refuse("Mara reads nothing inside a `.git`".to_owned());
            }
            if index + 1 == parts.len() {
                if !patterns.is_match(&at) {
                    return unmatched();
                }
                if ignored(&ignores, &child, false) {
                    return refuse("a `.gitignore` ignores it".to_owned());
                }
                return match kind {
                    None => Ok(()),
                    Some(Kind::SymbolicLink) => linked(&child),
                    Some(_) => {
                        refuse("a file that Mara does not read as a document is there".to_owned())
                    }
                };
            }
            if !patterns.can_continue(&at) {
                return unmatched();
            }
            if ignored(&ignores, &child, true) {
                return refuse(format!("a `.gitignore` ignores {child}"));
            }
            match kind {
                None => listed = false,
                Some(Kind::Directory) if self.state.inventory.directories.contains_key(&child) => {}
                Some(Kind::Directory) => {
                    return refuse(format!(
                        "{child} is another project's root, or Mara cannot list it"
                    ));
                }
                Some(Kind::SymbolicLink) => return linked(&child),
                Some(_) => return refuse(format!("{child} is not a directory")),
            }
            directory = child;
        }
        unreachable!("a path has a last part")
    }
}
