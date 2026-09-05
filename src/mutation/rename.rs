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
            for mention in current
                .mentions()
                .iter()
                .filter(|mention| mention.target() == old_id)
            {
                let span = mention.source().span();
                check_preimage(
                    source,
                    span.start_byte()..span.end_byte(),
                    &format!("[[{old_id}]]"),
                )?;
                patches.push(span.start_byte() + 2..span.end_byte() - 2);
            }
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
                .map(|rel| (rel.name(), &old_targets[rel.target()]))
                .collect::<Vec<_>>()
                != candidate
                    .relations()
                    .iter()
                    .map(|rel| (rel.name(), &new_targets[rel.target()]))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};
    use tempfile::TempDir;

    fn fixture() -> (TempDir, Project, Schema, BTreeMap<PathBuf, String>) {
        let directory = TempDir::new().unwrap();
        let project =
            crate::initialize_project(directory.path(), crate::Template::Minimal).unwrap();
        let schema = crate::load_schema(&project).unwrap();
        for (id, file, body) in [
            ("REQ-OLD", "a.mara.md", "Self [[REQ-OLD]]."),
            ("REQ-OTHER", "b.mara.md", "See [[REQ-OLD]]."),
        ] {
            crate::create_item(
                &project,
                &schema,
                crate::ItemCreationRequest {
                    flavour: "requirement".into(),
                    id: id.into(),
                    file: file.into(),
                    title: id.into(),
                    fields: Vec::new(),
                    relations: Vec::new(),
                    body: Some(body.into()),
                    line: None,
                },
            )
            .unwrap();
        }
        crate::add_relation(&project, &schema, "REQ-OTHER", "depends_on", "REQ-OLD").unwrap();
        let before = load_corpus(&project, &schema)
            .unwrap()
            .documents()
            .iter()
            .map(|doc| (doc.path().to_owned(), doc.source().to_owned()))
            .collect();
        (directory, project, schema, before)
    }

    fn assert_sources(project: &Project, before: &BTreeMap<PathBuf, String>) {
        for (path, source) in before {
            assert_eq!(
                &fs::read_to_string(project.root().join(path)).unwrap(),
                source
            );
        }
        assert!(!project.root().join(".mara/transaction.json").exists());
    }

    #[test]
    fn rename_single_document_uses_recovery_and_noop_does_not_publish() {
        let (_directory, project, schema, before) = fixture();
        let source = format!(
            "{}\n{}",
            before[Path::new("a.mara.md")],
            before[Path::new("b.mara.md")]
        );
        fs::write(project.root().join("a.mara.md"), &source).unwrap();
        fs::remove_file(project.root().join("b.mara.md")).unwrap();
        let error = rename_with_hook(&project, &schema, "REQ-OLD", "REQ-NEW", |phase| {
            if phase == Some(0) {
                invalid("injected failure")
            } else {
                Ok(())
            }
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("originals restored"));
        assert_eq!(
            fs::read_to_string(project.root().join("a.mara.md")).unwrap(),
            source
        );
        let result = rename_item(&project, &schema, "REQ-OLD", "REQ-NEW").unwrap();
        assert_eq!(result.paths, vec![PathBuf::from("a.mara.md")]);
        assert_eq!(
            fs::read_to_string(project.root().join("a.mara.md")).unwrap(),
            source
                .replace("requirement REQ-OLD", "requirement REQ-NEW")
                .replace(":depends_on: REQ-OLD", ":depends_on: REQ-NEW")
                .replace("[[REQ-OLD]]", "[[REQ-NEW]]")
        );
        let result = rename_with_hook(&project, &schema, "REQ-NEW", "REQ-NEW", |_| {
            panic!("no-op must not publish a journal")
        })
        .unwrap();
        assert!(result.paths.is_empty());
        assert!(!project.root().join(".mara/transaction.json").exists());
    }

    #[test]
    fn rename_replacement_failures_restore_every_original() {
        for stop in [None, Some(0), Some(1)] {
            let (_directory, project, schema, before) = fixture();
            let error = rename_with_hook(&project, &schema, "REQ-OLD", "REQ-NEW", |phase| {
                if phase == stop {
                    invalid("injected rename failure")
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .to_string();
            assert!(
                error.contains("transaction rolled back; originals restored"),
                "{error}"
            );
            assert_sources(&project, &before);
            rename_item(&project, &schema, "REQ-OLD", "REQ-NEW").unwrap();
            assert!(validate_corpus(&load_corpus(&project, &schema).unwrap(), &schema).is_empty());
        }
    }

    #[test]
    fn rename_incomplete_rollback_preserves_manual_edits_and_blocks_until_recovery() {
        let (_directory, project, schema, before) = fixture();
        let error = rename_with_hook(&project, &schema, "REQ-OLD", "REQ-NEW", |phase| {
            if phase == Some(0) {
                fs::write(project.root().join("b.mara.md"), "manual edit").unwrap();
                invalid("injected rename failure")
            } else {
                Ok(())
            }
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("rollback incomplete"), "{error}");
        assert!(error.contains("project transaction rollback"));
        assert!(
            rename_item(&project, &schema, "REQ-NEW", "REQ-LATER")
                .unwrap_err()
                .to_string()
                .contains("pending transaction")
        );
        assert!(crate::rollback_transaction(&project).is_err());
        assert_eq!(
            fs::read_to_string(project.root().join("b.mara.md")).unwrap(),
            "manual edit"
        );
        fs::write(
            project.root().join("b.mara.md"),
            &before[Path::new("b.mara.md")],
        )
        .unwrap();
        assert_eq!(
            crate::rollback_transaction(&project)
                .unwrap()
                .restored
                .len(),
            2
        );
        assert_sources(&project, &before);
    }

    #[test]
    fn rename_interrupted_process_rolls_back_after_restart() {
        let (_directory, project, schema, before) = fixture();
        for stop in ["prepared", "0", "1"] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "mutation::rename::tests::rename_interruption_child",
                    "--ignored",
                ])
                .env("MARA_TEST_RENAME_PROJECT", project.root())
                .env("MARA_TEST_RENAME_STOP", stop)
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(73));
            assert!(
                rename_item(&project, &schema, "REQ-OLD", "REQ-NEW")
                    .unwrap_err()
                    .to_string()
                    .contains("pending transaction")
            );
            assert_eq!(
                crate::rollback_transaction(&project)
                    .unwrap()
                    .restored
                    .len(),
                2
            );
            assert_sources(&project, &before);
        }
    }

    #[test]
    #[ignore = "subprocess helper for rename interruption coverage"]
    fn rename_interruption_child() {
        let path = std::env::var_os("MARA_TEST_RENAME_PROJECT").unwrap();
        let project = crate::resolve_project(Some(Path::new(&path)), Path::new(&path)).unwrap();
        let schema = crate::load_schema(&project).unwrap();
        let stop = std::env::var("MARA_TEST_RENAME_STOP").unwrap();
        rename_with_hook(&project, &schema, "REQ-OLD", "REQ-NEW", |phase| {
            if phase.map_or_else(|| "prepared".into(), |index| index.to_string()) == stop {
                std::process::exit(73);
            }
            Ok(())
        })
        .unwrap();
        panic!("interruption point was not reached");
    }

    #[test]
    fn rename_rejects_mismatched_and_overlapping_parsed_patch_preimages() {
        assert!(
            apply_patches(
                "REQ-OLD",
                std::iter::once(0..7).collect(),
                "REQ-OTHER",
                "REQ-NEW"
            )
            .is_err()
        );
        assert!(apply_patches("REQ-OLD", vec![0..7, 0..7], "REQ-OLD", "REQ-NEW").is_err());
        assert!(
            apply_patches(
                "żREQ-OLD",
                std::iter::once(1..8).collect(),
                "REQ-OLD",
                "REQ-NEW"
            )
            .is_err()
        );
    }
}
