//! Native source loading; compilation and evaluation live in mara-engine.
use crate::{Project, Schema};
pub(crate) use mara_engine::rules::{RuleSource, Rules};
use std::{collections::BTreeSet, fs, path::PathBuf};

pub(crate) fn load(project: &Project, schema: &Schema) -> std::sync::Arc<Rules> {
    let sources = sources(project, project.rule_files.clone());
    compile(schema, sources, None)
}
pub(crate) fn compile(
    schema: &Schema,
    sources: Vec<RuleSource>,
    bindings: Option<std::collections::BTreeMap<String, String>>,
) -> std::sync::Arc<Rules> {
    let key = crate::runtime::sessions::key(
        "rules",
        format!("{schema:?}{sources:?}{bindings:?}").as_bytes(),
    );
    crate::runtime::sessions::cached::<_, std::convert::Infallible>(key, || {
        crate::runtime::sessions::count("rules", 1);
        Ok(std::sync::Arc::new(Rules::load_sources(
            schema, sources, bindings,
        )))
    })
    .unwrap()
}

/// Preserve native regular-file containment and canonical duplicate checks.
pub(crate) fn sources(project: &Project, mut files: Vec<PathBuf>) -> Vec<RuleSource> {
    files.sort();
    let mut seen = BTreeSet::new();
    files
        .into_iter()
        .map(|path| {
            let content = (|| {
                if !crate::is_project_relative(&path)
                    || !matches!(
                        path.extension().and_then(|s| s.to_str()),
                        Some("yaml" | "yml")
                    )
                {
                    return Err("rule source must be a project-relative .yaml or .yml file".into());
                }
                let full = project.root().join(&path);
                let canonical = match fs::canonicalize(&full) {
                    Ok(p) if p.starts_with(project.root()) && p.is_file() => p,
                    _ => {
                        return Err(
                            "rule source must be an existing regular file inside the project"
                                .into(),
                        );
                    }
                };
                if !seen.insert(canonical) {
                    return Err("duplicate rule source path".into());
                }
                fs::read_to_string(full).map_err(|_| "cannot read UTF-8 rule source".into())
            })();
            // Request sources are re-read before every content-keyed lookup.
            // Configured rules are already recorded by the project loader.
            RuleSource { path, content }
        })
        .collect()
}
