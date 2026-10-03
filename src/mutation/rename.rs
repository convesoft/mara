use super::{changes, inputs, transaction, verify};
use crate::*;
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
    let (engine, corpus) = inputs(project, schema)?;
    let plan = engine
        .planner_with_code(corpus.code())?
        .plan_rename(reference, new_id)?;
    let changes = changes(project, &plan)?;
    if !changes.is_empty() {
        transaction::commit_with_hook(project, changes, || verify(project, schema, &plan), hook)?;
    }
    Ok(plan.result().clone())
}
#[cfg(test)]
mod tests {
    use super::super::invalid;
    use super::*;
    use std::{collections::BTreeMap, path::PathBuf};
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
        crate::mutation::mutate_semantic_relation(
            &project,
            &schema,
            &crate::RelationParams {
                source: "REQ-OTHER".into(),
                relation: "depends_on".into(),
                target: "REQ-OLD".into(),
            },
            true,
            None,
        )
        .unwrap();
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

    // @mara implements VER-ITEM-RENAME
    // @mara checks REQ-ITEM-RENAME
    // @mara checks DES-ITEM-RENAME
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

    // @mara checks DES-ITEM-RENAME
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

    // @mara checks DES-ITEM-RENAME
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

    // @mara checks DES-ITEM-RENAME
    #[test]
    fn rename_interrupted_process_rolls_back_after_restart() {
        let (_directory, project, schema, before) = fixture();
        for stop in ["prepared", "0", "1"] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child.current_dir(project.root());
            for (key, _) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("GIT_") {
                    child.env_remove(key);
                }
            }
            child
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CEILING_DIRECTORIES", project.root())
                .env("XDG_CONFIG_HOME", project.root().join(".test-config"));
            let status = child
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
}
