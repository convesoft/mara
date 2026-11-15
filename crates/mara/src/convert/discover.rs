//! The documents of a 0.3.0 project: the `*.mara.md` files that its include patterns match, read
//! with 0.3.0's pattern syntax, brace alternatives and character classes included, and that no
//! `.gitignore` file ignores. As in the input inventory (DES-INPUTS-AND-KEYS in
//! `docs/engine.mara.md`), `.git`, symbolic links and other projects' roots are skipped.

use std::fs;
use std::sync::Arc;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::gitignore::Gitignore;
use mara_engine::config::LEGACY_CONFIG_PATH;
use mara_engine::diagnostic::Diagnostic;
use mara_engine::work::WorkCounts;

use crate::inputs::{compile_gitignore, ignored};
use crate::project::recognize;
use crate::write::{Basis, WriteLock};

/// The `.gitignore` files that apply to a directory, root first, each with its directory.
type Ignores = Vec<(String, Arc<Gitignore>)>;

/// The document paths of the locked 0.3.0 project, relative to its root, in path order.
pub fn documents(
    lock: &WriteLock,
    root: &std::path::Path,
    include: &[(String, usize)],
    work: &mut WorkCounts,
) -> Result<Vec<String>, Vec<Diagnostic>> {
    let patterns = patterns(include)?;
    let mut found = Vec::new();
    let mut problems = Vec::new();
    let mut pending: Vec<(String, Ignores)> = vec![(String::new(), Vec::new())];
    while let Some((directory, mut ignores)) = pending.pop() {
        let shown = if directory.is_empty() {
            "."
        } else {
            directory.as_str()
        };
        let listed = fs::read_dir(root.join(&directory)).and_then(|entries| {
            entries
                .map(|entry| {
                    let entry = entry?;
                    Ok((entry.file_name(), entry.file_type()?))
                })
                .collect::<std::io::Result<Vec<_>>>()
        });
        let mut entries = match listed {
            Ok(entries) => entries,
            Err(error) => {
                let message = format!(
                    "cannot list {shown}, so the conversion could miss a document there: {error}"
                );
                problems.push(Diagnostic::error("io-error", shown, 1, message));
                continue;
            }
        };
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let join = |name: &str| match directory.as_str() {
            "" => name.to_owned(),
            directory => format!("{directory}/{name}"),
        };
        if entries
            .iter()
            .any(|(name, kind)| name == ".gitignore" && kind.is_file())
        {
            let path = join(".gitignore");
            match Basis::read(lock, &path, work) {
                Ok((Some(contents), _)) => match compile_gitignore(&contents) {
                    Ok(gitignore) => ignores.push((directory.clone(), Arc::new(gitignore))),
                    Err((line, reason)) => {
                        problems.push(Diagnostic::error(
                            "unsupported-gitignore",
                            path,
                            line,
                            reason,
                        ));
                        continue;
                    }
                },
                Ok((None, _)) => {}
                Err(error) => {
                    problems.push(error.diagnostic());
                    continue;
                }
            }
        }
        for (name, kind) in entries {
            let Some(name) = name.to_str() else { continue };
            if name == ".git" || kind.is_symlink() {
                continue;
            }
            let path = join(name);
            if kind.is_dir() {
                if ignored(&ignores, &path, true) {
                    continue;
                }
                match recognize(&root.join(&path)) {
                    Ok(None) => pending.push((path, ignores.clone())),
                    Ok(Some(_)) => {}
                    Err(error) => {
                        let message = format!(
                            "cannot tell whether {path} is another project's root, so the \
                             conversion could miss a document there: {error}"
                        );
                        problems.push(Diagnostic::error(error.code(), path, 1, message));
                    }
                }
            } else if kind.is_file()
                && name.ends_with(".mara.md")
                && patterns.is_match(&path)
                && !ignored(&ignores, &path, false)
            {
                found.push(path);
            }
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    found.sort();
    Ok(found)
}

/// The include patterns as 0.3.0 matched them: `*` and `?` within one path part.
fn patterns(include: &[(String, usize)]) -> Result<GlobSet, Vec<Diagnostic>> {
    let mut builder = GlobSetBuilder::new();
    for (pattern, line) in include {
        match GlobBuilder::new(pattern).literal_separator(true).build() {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(error) => {
                let message =
                    format!("the include pattern `{pattern}` is not a 0.3.0 pattern: {error}");
                return Err(vec![Diagnostic::error(
                    "invalid-value",
                    LEGACY_CONFIG_PATH,
                    *line,
                    message,
                )]);
            }
        }
    }
    builder.build().map_err(|error| {
        let message = format!("the include patterns cannot be read: {error}");
        vec![Diagnostic::error(
            "invalid-value",
            LEGACY_CONFIG_PATH,
            1,
            message,
        )]
    })
}
