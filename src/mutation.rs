//! Native guarded publication of portable semantic plans.
use crate::*;
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};
mod rename;
mod transaction;
pub use mara_engine::{
    BackfilledMid, BackfilledMids, InitialRelation, ItemCreation, ItemCreationRequest,
    ItemDeletion, ItemLocation, ItemMove, ItemRename, ItemUpdate, ItemUpdateWarning,
};
pub use rename::rename_item;
pub use transaction::{TransactionRollback, rollback_transaction};

fn invalid<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error::InvalidMutation {
        message: message.into(),
    })
}

fn inputs(project: &Project, schema: &Schema) -> Result<(SourceEngine, Corpus), Error> {
    let corpus = load_corpus(project, schema)?;
    let engine = crate::source_bridge::load_mutation_engine(project, &corpus.documents)?;
    let (_, actual_schema, actual_corpus) = engine.corpus()?;
    if actual_schema != *schema
        || actual_corpus.documents != corpus.documents
        || crate::resolve_project(Some(project.root()), project.root())? != *project
    {
        return invalid(
            "project changed while loading semantic planning inputs; retry the operation",
        );
    }
    Ok((engine, corpus))
}
fn verify<T>(project: &Project, schema: &Schema, plan: &ChangePlan<T>) -> Result<(), Error> {
    let (engine, corpus) = inputs(project, schema)?;
    plan.check_preconditions(&engine, Some(corpus.code()))
}
fn changes<T>(project: &Project, plan: &ChangePlan<T>) -> Result<Vec<transaction::Change>, Error> {
    plan.changes()
        .iter()
        .map(|change| {
            let after = change.after.clone().ok_or_else(|| Error::InvalidMutation {
                message: "native document operations do not delete containing files".into(),
            })?;
            transaction::Change::new(
                project,
                change.path.clone().into(),
                change.before.clone(),
                after,
            )
        })
        .collect()
}
fn publish<T: Clone>(
    project: &Project,
    schema: &Schema,
    plan: ChangePlan<T>,
    journal: bool,
) -> Result<T, Error> {
    let changes = changes(project, &plan)?;
    if !changes.is_empty() {
        if journal {
            transaction::commit(project, changes, || verify(project, schema, &plan))?;
        } else if changes.len() == 1 {
            transaction::commit_single(
                project,
                changes.into_iter().next().expect("one change"),
                || verify(project, schema, &plan),
            )?;
        } else {
            transaction::commit_unjournaled(project, changes, || verify(project, schema, &plan))?;
        }
    }
    Ok(plan.result().clone())
}
fn generate_mids(corpus: &Corpus, count: usize) -> Vec<String> {
    let mut known = corpus
        .items()
        .filter_map(Item::mid)
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    (0..count)
        .map(|_| {
            loop {
                let mid = ulid::Ulid::new().to_string();
                if known.insert(mid.clone()) {
                    break mid;
                }
            }
        })
        .collect()
}
// @mara implements DES-MID-BACKFILL
pub fn backfill_mids(project: &Project, schema: &Schema) -> Result<BackfilledMids, Error> {
    let _lock = transaction::MutationLock::acquire(project)?;
    let (engine, corpus) = inputs(project, schema)?;
    let count = corpus.items().filter(|item| item.mid().is_none()).count();
    let plan = engine
        .planner_with_code(corpus.code())?
        .plan_backfill(&generate_mids(&corpus, count))?;
    publish(project, schema, plan, false)
}
pub fn create_item(
    project: &Project,
    schema: &Schema,
    request: ItemCreationRequest,
) -> Result<ItemCreation, Error> {
    let _lock = transaction::MutationLock::acquire(project)?;
    let (path, _, _) = resolve_document_path(project, &request.file)?;
    if !crate::corpus::document_is_discoverable(project, &path)? {
        return invalid("destination file is excluded by project content discovery");
    }
    let (engine, corpus) = inputs(project, schema)?;
    let plan = engine
        .planner_with_code(corpus.code())?
        .plan_create(request, &generate_mids(&corpus, 1)[0])?;
    publish(project, schema, plan, false)
}
pub fn update_item(
    project: &Project,
    schema: &Schema,
    request: ItemUpdateParams,
) -> Result<ItemUpdate, Error> {
    let _lock = transaction::MutationLock::acquire(project)?;
    let (engine, corpus) = inputs(project, schema)?;
    let plan = engine
        .planner_with_code(corpus.code())?
        .plan_update(request)?;
    publish(project, schema, plan, false)
}
pub fn delete_item(
    project: &Project,
    schema: &Schema,
    reference: &str,
) -> Result<ItemDeletion, Error> {
    let _lock = transaction::MutationLock::acquire(project)?;
    let (engine, corpus) = inputs(project, schema)?;
    let plan = engine
        .planner_with_code(corpus.code())?
        .plan_delete(reference)?;
    publish(project, schema, plan, false)
}
pub fn move_item(
    project: &Project,
    schema: &Schema,
    reference: &str,
    file: &Path,
    line: Option<usize>,
) -> Result<ItemMove, Error> {
    let _lock = transaction::MutationLock::acquire(project)?;
    let (path, _, _) = resolve_document_path(project, file)?;
    if !crate::corpus::document_is_discoverable(project, &path)? {
        return invalid("destination file is excluded by project content discovery");
    }
    let (engine, corpus) = inputs(project, schema)?;
    let plan = engine
        .planner_with_code(corpus.code())?
        .plan_move(reference, file, line)?;
    publish(project, schema, plan, true)
}
pub(crate) fn mutate_semantic_relation(
    project: &Project,
    schema: &Schema,
    params: &RelationParams,
    add: bool,
    occurrence: Option<&str>,
) -> Result<RelationMutationResult, RelationError> {
    if params.source.starts_with("code:") {
        return Err(RelationError::new(
            "unsupported_mutation",
            "relation add/remove cannot modify code source files; use the declared inverse with an item source, or edit the code comment directly",
        ));
    }
    let _lock = transaction::MutationLock::acquire(project)?;
    let (engine, corpus) = inputs(project, schema)?;
    let plan = engine
        .planner_with_code(corpus.code())?
        .plan_relation(params, add, occurrence)?;
    Ok(publish(project, schema, plan, true)?)
}
fn resolve_document_path(
    project: &Project,
    requested: &Path,
) -> Result<(PathBuf, PathBuf, bool), Error> {
    if requested.as_os_str().is_empty()
        || requested.is_absolute()
        || requested.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return invalid("destination file must be a project-relative path");
    }
    let mut relative = PathBuf::new();
    for component in requested.components() {
        if component != Component::CurDir {
            relative.push(component.as_os_str());
        }
    }
    if relative
        .file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| !name.ends_with(".mara.md"))
    {
        return invalid("destination file must be named '*.mara.md'");
    }
    let absolute = project.root().join(&relative);
    let parent = absolute
        .parent()
        .expect("a project-relative file has a parent");
    let canonical_parent = fs::canonicalize(parent).map_err(|source| Error::Io {
        action: "resolve destination parent",
        path: parent.to_path_buf(),
        source,
    })?;
    if !canonical_parent.starts_with(project.root()) {
        return invalid("destination file must remain inside the project root");
    }
    let existed = absolute.try_exists().map_err(|source| Error::Io {
        action: "inspect destination file",
        path: absolute.clone(),
        source,
    })?;
    if existed {
        let metadata = fs::symlink_metadata(&absolute).map_err(|source| Error::Io {
            action: "inspect destination file",
            path: absolute.clone(),
            source,
        })?;
        if !metadata.file_type().is_file() {
            return invalid("destination must be a regular file");
        }
    }
    Ok((relative, absolute, existed))
}
