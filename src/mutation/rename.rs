use std::{collections::BTreeMap, ops::Range, path::PathBuf};

use super::{invalid, transaction};
use crate::{Corpus, Error, Project, Schema, load_corpus, validate_corpus};

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct ItemRename {
    pub mid: String,
    pub old_id: String,
    pub new_id: String,
    pub paths: Vec<PathBuf>,
}

pub fn rename_item(
    project: &Project,
    schema: &Schema,
    reference: &str,
    new_id: &str,
) -> Result<ItemRename, Error> {
    rename_with_hook(project, schema, reference, new_id, |_| Ok(()))
}

fn rename_with_hook(
    project: &Project,
    schema: &Schema,
    reference: &str,
    new_id: &str,
    hook: impl FnMut(Option<usize>) -> Result<(), Error>,
) -> Result<ItemRename, Error> {
    let _lock = transaction::MutationLock::acquire(project)?;
    let corpus = load_corpus(project, schema)?;
    require_valid(&corpus, schema)?;
    let resolved = crate::get_item(&corpus, reference).map_err(|error| Error::InvalidMutation {
        message: error.to_string(),
    })?;
    let item = corpus
        .items()
        .find(|item| item.id() == resolved.summary().id())
        .expect("resolved item belongs to corpus");
    if !crate::is_item_id(new_id) {
        return invalid(format!("invalid replacement item ID '{new_id}'"));
    }
    let prefix = &schema.flavours()[item.flavour()].id_prefix;
    if !new_id.starts_with(prefix) {
        return invalid(format!(
            "item ID '{new_id}' must start with '{prefix}' for flavour '{}'",
            item.flavour()
        ));
    }
    if corpus
        .items()
        .any(|other| other.id() == new_id && other.mid() != item.mid())
    {
        return invalid(format!("item '{new_id}' already exists"));
    }
    let old_id = item.id();
    let mut result = ItemRename {
        mid: item.mid().expect("validated MID").to_owned(),
        old_id: old_id.to_owned(),
        new_id: new_id.to_owned(),
        paths: Vec::new(),
    };
    if old_id == new_id {
        return Ok(result);
    }

    let mut candidates = BTreeMap::new();
    for document in corpus.documents() {
        let source = document.source();
        let mut patches = Vec::new();
        for current in document.items() {
            if current.mid() == item.mid() {
                let start = current.source().span().start_byte();
                let opener = format!(":::mara {} {old_id}", current.flavour());
                check_preimage(source, start..start + opener.len(), &opener)?;
                patches.push(start + opener.len() - old_id.len()..start + opener.len());
            }
            for relation in current
                .relations()
                .iter()
                .filter(|rel| rel.target() == old_id)
            {
                let span = relation.source().span();
                if relation.inline {
                    check_preimage(
                        source,
                        span.start_byte()..span.end_byte(),
                        &format!("[[{}:{old_id}]]", relation.name()),
                    )?;
                    patches.push(span.end_byte() - 2 - old_id.len()..span.end_byte() - 2);
                    continue;
                }
                let line = &source[span.start_byte()..span.end_byte()];
                let prefix = format!(":{}:", relation.name());
                let Some(value) = line.strip_prefix(&prefix) else {
                    return invalid("relation source preimage differs from parsed metadata");
                };
                if value.trim() != old_id {
                    return invalid("relation target preimage differs from parsed target");
                }
                let start =
                    span.start_byte() + prefix.len() + value.len() - value.trim_start().len();
                patches.push(start..start + old_id.len());
            }
        }
        for mention in document.references().iter().filter(|reference| {
            reference.kind() == crate::ReferenceKind::Item && reference.target() == old_id
        }) {
            let span = mention.source().span();
            check_preimage(
                source,
                span.start_byte()..span.end_byte(),
                &format!("[[{old_id}]]"),
            )?;
            patches.push(span.start_byte() + 2..span.end_byte() - 2);
        }
        if patches.is_empty() {
            continue;
        }
        candidates.insert(
            document.path().to_path_buf(),
            apply_patches(source, patches, old_id, new_id)?,
        );
    }
    let projected = corpus.with_replacements(&candidates, schema)?;
    super::references::preflight(&corpus, &projected, Some((old_id, new_id)), None)?;
    require_valid(&projected, schema)?;
    verify_identities_and_references(&corpus, &projected, &result)?;
    result.paths = candidates.keys().cloned().collect();
    let changes = corpus
        .documents()
        .iter()
        .filter_map(|document| {
            candidates.get(document.path()).map(|after| {
                transaction::Change::new(
                    project,
                    document.path().to_path_buf(),
                    Some(document.source().to_owned()),
                    after.clone(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    transaction::commit_with_hook(
        project,
        changes,
        || {
            if crate::resolve_project(Some(project.root()), project.root())? != *project
                || crate::load_schema(project)? != *schema
                || load_corpus(project, schema)? != corpus
            {
                return invalid("project changed since rename preflight; retry the rename");
            }
            require_valid(&projected, schema)
        },
        hook,
    )?;
    Ok(result)
}

fn check_preimage(source: &str, range: Range<usize>, expected: &str) -> Result<(), Error> {
    if source.get(range) != Some(expected) {
        return invalid("rename patch preimage differs from parsed source");
    }
    Ok(())
}

fn apply_patches(
    source: &str,
    mut patches: Vec<Range<usize>>,
    old_id: &str,
    new_id: &str,
) -> Result<String, Error> {
    patches.sort_by_key(|range| range.start);
    let mut end = 0;
    for range in &patches {
        if range.start < end {
            return invalid("rename patches overlap");
        }
        check_preimage(source, range.clone(), old_id)?;
        end = range.end;
    }
    let mut candidate = source.to_owned();
    for range in patches.into_iter().rev() {
        candidate.replace_range(range, new_id);
    }
    Ok(candidate)
}

fn require_valid(corpus: &Corpus, schema: &Schema) -> Result<(), Error> {
    if let Some(diagnostic) = validate_corpus(corpus, schema).first() {
        return invalid(format!(
            "cannot rename item while validation fails at {}:{}: {}",
            diagnostic.source().path().display(),
            diagnostic.source().span().start_line(),
            diagnostic.message()
        ));
    }
    Ok(())
}

fn verify_identities_and_references(
    before: &Corpus,
    after: &Corpus,
    rename: &ItemRename,
) -> Result<(), Error> {
    let targets = |corpus: &Corpus| {
        corpus
            .items()
            .flat_map(|item| {
                let mid = item.mid().expect("validated MID").to_owned();
                [(item.id().to_owned(), mid.clone()), (mid.clone(), mid)]
            })
            .collect::<BTreeMap<_, _>>()
    };
    let old_targets = targets(before);
    let new_targets = targets(after);
    if before.items().count() != after.items().count() || new_targets.contains_key(&rename.old_id) {
        return invalid("rename changed item recognition or retained the old ID");
    }
    for original in before.items() {
        let Some(candidate) = after.items().find(|item| item.mid() == original.mid()) else {
            return invalid("rename would hide an existing item");
        };
        let expected_id = if original.id() == rename.old_id {
            &rename.new_id
        } else {
            original.id()
        };
        if candidate.id() != expected_id
            || candidate.flavour() != original.flavour()
            || candidate.source().path() != original.source().path()
            || original
                .relations()
                .iter()
                .map(|rel| (rel.name(), relation_identity(&old_targets, rel.target())))
                .collect::<Vec<_>>()
                != candidate
                    .relations()
                    .iter()
                    .map(|rel| (rel.name(), relation_identity(&new_targets, rel.target())))
                    .collect::<Vec<_>>()
            || original
                .mentions()
                .iter()
                .map(|mention| &old_targets[mention.target()])
                .collect::<Vec<_>>()
                != candidate
                    .mentions()
                    .iter()
                    .map(|mention| &new_targets[mention.target()])
                    .collect::<Vec<_>>()
        {
            return invalid("rename would change an item's identity or resolved references");
        }
    }
    Ok(())
}

fn relation_identity<'a>(targets: &'a BTreeMap<String, String>, target: &'a str) -> &'a str {
    if crate::external::address(target).is_some() || target.starts_with("code:") {
        target
    } else {
        targets
            .get(target)
            .expect("validated internal target")
            .as_str()
    }
}
