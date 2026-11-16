//! The pause-point cases of VER-REFUSED-EDITS for the single-file operations, through the debug
//! binary (DES-WRITE-PROCEDURE, ADR-DEBUG-PAUSE-POINT): the only file of each edit changed at the
//! pause before step 2, and at its pause before its second check; for `item create` into a new
//! document, that document created.

#![cfg(debug_assertions)]

use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use crate::{Project, finish, item, mid, no_temporary_file};

/// Each operation, the file it writes, and whether it creates that file.
fn operations() -> Vec<(Vec<&'static str>, &'static str, bool)> {
    let create = |path| {
        vec![
            "item",
            "create",
            "requirement",
            path,
            "--title",
            "New",
            "--field",
            "status=draft",
            "--body",
            "B.",
        ]
    };
    vec![
        (create("docs/a.mara.md"), "docs/a.mara.md", false),
        (create("docs/new/b.mara.md"), "docs/new/b.mara.md", true),
        (
            vec!["item", "update", "REQ-A", "--body", "New."],
            "docs/a.mara.md",
            false,
        ),
        (vec!["item", "delete", "REQ-A"], "docs/a.mara.md", false),
        (
            vec!["relation", "add", "REQ-A", "satisfies", "NEED-N"],
            "docs/a.mara.md",
            false,
        ),
        (
            vec!["relation", "remove", "REQ-A", "refines", "REQ-B"],
            "docs/a.mara.md",
            false,
        ),
    ]
}

fn project() -> Project {
    let project = Project::new();
    let refines = format!("[REQ-B](#{})", mid(2));
    project.write(
        "docs/a.mara.md",
        &crate::document(
            "A",
            &[
                item(
                    &mid(1),
                    "Requirement",
                    "REQ-A",
                    "A",
                    &[("Refines", &refines)],
                    "A.",
                ),
                item(&mid(2), "Requirement", "REQ-B", "B", &[], "B."),
                item(&mid(3), "Need", "NEED-N", "N", &[], "N."),
            ],
        ),
    );
    project
}

/// Runs `args` with the pause points on. At each pause, calls `at` with the contents of
/// `paused` and the number of earlier pauses, then lets the writer go on.
fn run_paused(project: &Project, args: &[&str], mut at: impl FnMut(&str, usize)) -> crate::Run {
    let pauses = tempfile::Builder::new()
        .prefix("mara-pause-")
        .tempdir()
        .unwrap();
    let mut child = crate::command(&project.root(), args)
        .env("MARA_TEST_PAUSE", pauses.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start mara");
    let paused = pauses.path().join("paused");
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut count = 0;
    while child.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "mara {args:?} neither pauses nor ends"
        );
        match fs::read_to_string(&paused) {
            Ok(contents) => {
                at(&contents, count);
                count += 1;
                fs::remove_file(&paused).unwrap();
            }
            Err(_) => thread::sleep(Duration::from_millis(2)),
        }
    }
    finish(args, child.wait_with_output().unwrap())
}

/// Changes the file at `path`, or creates it.
fn change(root: &Path, path: &str, created: bool) -> String {
    let absolute = root.join(path);
    let contents = match created {
        true => "Created by another program.\n".to_owned(),
        false => format!(
            "{}\nChanged by another program.\n",
            fs::read_to_string(&absolute).unwrap()
        ),
    };
    fs::create_dir_all(absolute.parent().unwrap()).unwrap();
    fs::write(&absolute, &contents).unwrap();
    contents
}

#[test]
fn refuses_an_edit_whose_file_changed_at_either_pause() {
    for (args, path, created) in operations() {
        // 0: the pause before step 2; 1: the pause before the file's second check.
        for stop in [0, 1] {
            let project = project();
            let mut before = project.tree();
            let mut written = None;
            let run = run_paused(&project, &args, |contents, count| {
                let expected = match count {
                    0 => "",
                    _ => path,
                };
                assert_eq!(contents, expected, "{args:?}: pause {count}");
                if count == stop {
                    written = Some(change(&project.root(), path, created));
                }
            });
            assert_eq!(
                run.json()["error"]["code"],
                "changed-file",
                "{args:?} at {stop}: {run:#?}"
            );
            assert!(run.message().contains(path), "{run:#?}");
            let written = written.expect("the writer paused");
            assert_eq!(project.read(path), written, "{args:?} at {stop}");
            let mut after = project.tree();
            assert!(no_temporary_file(&after), "{args:?} at {stop}: {after:?}");
            after.remove(path);
            before.remove(path);
            if created {
                after.remove("docs/new/");
            }
            assert_eq!(after, before, "{args:?} at {stop}");
        }
    }
}

/// What the agent guide (`mara prime`) says after a `changed-file` stop that replaced files: run
/// `project fix` and `project validate`, then go on with the item's current ID or its MID. A
/// rename across two documents stops after the first, whose replaced file the message names.
#[test]
fn after_a_changed_file_stop_that_replaced_files_fix_validate_and_the_mid_recover() {
    let project = Project::new();
    let need = mid(3);
    project.write(
        "docs/needs.mara.md",
        &crate::document("Needs", &[item(&need, "Need", "NEED-N", "N", &[], "N.")]),
    );
    let satisfies = crate::link("NEED-N", "needs.mara.md", &need);
    let requirement = item(
        &mid(1),
        "Requirement",
        "REQ-A",
        "A",
        &[("Satisfies", &satisfies)],
        "A.",
    );
    project.write(
        "docs/reqs.mara.md",
        &crate::document("Reqs", &[requirement]),
    );
    let args = ["item", "rename", "NEED-N", "NEED-AUTH"];
    let mut files = Vec::new();
    let run = run_paused(&project, &args, |contents, count| {
        if count > 0 {
            files.push(contents.to_owned());
        }
        if count == 2 {
            change(&project.root(), contents, false);
        }
    });
    assert_eq!(files.len(), 2, "{files:?}");
    let run = run.refused("changed-file");
    let message = run.message();
    assert!(
        message.contains(&format!("stopped after replacing {}", files[0])),
        "{message}"
    );
    project.run(&["project", "fix"]).ok();
    project.valid();
    let item = project.run(&["get", &need]).ok().json();
    let id = item["node"]["id"].as_str().expect("an ID").to_owned();
    project.run(&["get", &id]).ok();
}
