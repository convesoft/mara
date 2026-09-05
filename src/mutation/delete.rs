use std::{collections::BTreeMap, path::PathBuf};

use super::{invalid, transaction};
use crate::{Corpus, Error, Project, Schema, load_corpus, validate_corpus};

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct ItemDeletion {
    pub id: String,
    pub mid: String,
    pub path: PathBuf,
}

pub fn delete_item(
    project: &Project,
    schema: &Schema,
    reference: &str,
) -> Result<ItemDeletion, Error> {
    let _lock = transaction::MutationLock::acquire(project)?;
    let corpus = load_corpus(project, schema)?;
    require_valid_corpus(&corpus, schema)?;
    let resolved = crate::get_item(&corpus, reference).map_err(|error| Error::InvalidMutation {
        message: error.to_string(),
    })?;
    let item = corpus
        .items()
        .find(|item| item.id() == resolved.summary().id())
        .expect("resolved item belongs to corpus");
    let mid = item.mid().expect("validated identity");
    let targets_item = |target: &str| target == item.id() || target == mid;
    let mut blockers = Vec::new();
    for survivor in corpus.items().filter(|other| other.mid() != Some(mid)) {
        for relation in survivor.relations() {
            if targets_item(relation.target()) {
                blockers.push((
                    relation.source(),
                    format!(
                        "item '{}' relation '{}' to '{}'",
                        survivor.id(),
                        relation.name(),
                        relation.target()
                    ),
                ));
            }
        }
        for mention in survivor.mentions() {
            if targets_item(mention.target()) {
                blockers.push((
                    mention.source(),
                    format!(
                        "item '{}' mention '[[{}]]'",
                        survivor.id(),
                        mention.target()
                    ),
                ));
            }
        }
    }
    blockers.sort_by_key(|(source, _)| (source.path(), source.span().start_byte()));
    if !blockers.is_empty() {
        let locations = blockers
            .into_iter()
            .map(|(source, message)| {
                format!(
                    "{}:{} (bytes {}..{}): {message}",
                    source.path().display(),
                    source.span().start_line(),
                    source.span().start_byte(),
                    source.span().end_byte()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        return invalid(format!(
            "cannot delete item '{}' with MID {mid}; remove surviving incoming references first:\n{locations}",
            item.id()
        ));
    }

    let path = item.source().path().to_path_buf();
    let document = corpus
        .documents()
        .iter()
        .find(|doc| doc.path() == path)
        .expect("item belongs to document");
    let span = item.source().span();
    let before = &document.source()[..span.start_byte()];
    let mut after = &document.source()[span.end_byte()..];
    // Coalesce the two adjacent separators by removing only one empty line.
    let preceding_empty_line = before
        .strip_suffix("\r\n")
        .or_else(|| before.strip_suffix('\n'))
        .is_some_and(|prefix| prefix.is_empty() || prefix.ends_with('\n'));
    if preceding_empty_line {
        after = after
            .strip_prefix("\r\n")
            .or_else(|| after.strip_prefix('\n'))
            .unwrap_or(after);
    }
    let candidate = format!("{before}{after}");
    let projected =
        corpus.with_replacements(&BTreeMap::from([(path.clone(), candidate.clone())]), schema)?;
    require_valid_corpus(&projected, schema)?;
    // A valid projection alone does not guarantee that Markdown still recognizes
    // every surviving item and mention exactly as before.
    if projected.items().count() + 1 != corpus.items().count()
        || projected.items().any(|other| other.mid() == Some(mid))
    {
        return invalid("deletion would change recognition of surviving items");
    }
    for original in corpus.items().filter(|other| other.mid() != Some(mid)) {
        let Some(survivor) = projected
            .items()
            .find(|other| other.mid() == original.mid())
        else {
            return invalid("deletion would hide a surviving item");
        };
        let original_doc = corpus
            .documents()
            .iter()
            .find(|doc| doc.path() == original.source().path())
            .expect("item document");
        let survivor_doc = projected
            .documents()
            .iter()
            .find(|doc| doc.path() == survivor.source().path())
            .expect("item document");
        let original_span = original.source().span();
        let survivor_span = survivor.source().span();
        if original.source().path() != survivor.source().path()
            || original_doc.source()[original_span.start_byte()..original_span.end_byte()]
                != survivor_doc.source()[survivor_span.start_byte()..survivor_span.end_byte()]
            || original
                .mentions()
                .iter()
                .map(|mention| mention.target())
                .collect::<Vec<_>>()
                != survivor
                    .mentions()
                    .iter()
                    .map(|mention| mention.target())
                    .collect::<Vec<_>>()
        {
            return invalid("deletion would change a surviving item's content or references");
        }
    }
    let change = transaction::Change::new(
        project,
        path.clone(),
        Some(document.source().to_owned()),
        candidate,
    )?;
    transaction::commit_single(project, change, || {
        if crate::resolve_project(Some(project.root()), project.root())? != *project
            || crate::load_schema(project)? != *schema
            || load_corpus(project, schema)? != corpus
            || !crate::corpus::document_is_discoverable(project, &path)?
        {
            return invalid("project changed since deletion preflight; retry the delete");
        }
        Ok(())
    })?;
    Ok(ItemDeletion {
        id: item.id().to_owned(),
        mid: mid.to_owned(),
        path,
    })
}

fn require_valid_corpus(corpus: &Corpus, schema: &Schema) -> Result<(), Error> {
    let diagnostics = validate_corpus(corpus, schema);
    if diagnostics.is_empty() {
        return Ok(());
    }
    invalid(format!(
        "cannot delete item while validation fails:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| {
                format!(
                    "{}:{}: {}",
                    diagnostic.source().path().display(),
                    diagnostic.source().span().start_line(),
                    diagnostic.message()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    ))
}
