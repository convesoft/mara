//! Writes through the daemon: VER-DAEMON-WRITES (REQ-WRITERS-SERIALIZED,
//! REQ-REFUSED-EDIT-CHANGES-NOTHING, REQ-INTERRUPTED-EDIT-LOSES-NOTHING, DES-DAEMON-LIFECYCLE),
//! a write that the daemon runs to its end, and the case of VER-NEVER-STALE that reads while a
//! write waits. Where a case needs it, the daemon is started by `mara daemon start` with
//! `MARA_TEST_PAUSE`, so that its writer waits at the pause points of DES-WRITE-PROCEDURE
//! (`docs/authoring/writes.mara.md`); the clients run without it.

#![cfg(debug_assertions)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

use crate::fresh::item;
use crate::{Run, Test, WAIT, finish, signal};

/// A daemon of the test whose writer waits at each pause point, and the directory of its
/// pauses.
struct Paused {
    pid: u32,
    pauses: PathBuf,
}

impl Paused {
    fn start(test: &Test) -> Paused {
        let pauses = test.path().join("pauses");
        fs::create_dir(&pauses).unwrap();
        let (started, pid) = test.start_of(
            &crate::mara(),
            &[("MARA_TEST_PAUSE", pauses.to_str().unwrap())],
        );
        assert!(started);
        Paused { pid, pauses }
    }

    /// Waits until the writer pauses, while `client` runs: what `paused` holds.
    fn wait(&self, client: &mut Child) -> String {
        let paused = self.pauses.join("paused");
        let deadline = Instant::now() + WAIT;
        loop {
            if let Ok(contents) = fs::read_to_string(&paused) {
                return contents;
            }
            assert!(
                client.try_wait().unwrap().is_none(),
                "the client ended before the daemon's writer paused"
            );
            assert!(
                Instant::now() < deadline,
                "the daemon's writer did not pause"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// Lets the writer go on from its pause.
    fn release(&self) {
        fs::remove_file(self.pauses.join("paused")).unwrap();
    }

    /// Lets the writer go on from every pause until `client` ends: what it printed.
    fn release_all(&self, mut client: Child) -> Run {
        let paused = self.pauses.join("paused");
        let deadline = Instant::now() + crate::RUN_WAIT;
        while client.try_wait().unwrap().is_none() {
            let _ = fs::remove_file(&paused);
            assert!(Instant::now() < deadline, "the client does not end");
            thread::sleep(Duration::from_millis(5));
        }
        finish(client)
    }

    /// Starts `item move REQ-TWO docs/moved.mara.md` and waits until the writer pauses in step 3
    /// of its file number `file`, 1 or 2.
    fn move_paused_at(&self, test: &Test, project: &Path, file: usize) -> Child {
        let mut client = test.spawn_in(project, MOVE);
        let mut files = Vec::new();
        loop {
            let paused = self.wait(&mut client);
            if !paused.is_empty() {
                files.push(paused);
            }
            if files.len() == file {
                break;
            }
            self.release();
        }
        assert_eq!(files, ["docs/moved.mara.md", "docs/reqs.mara.md"][..file]);
        client
    }
}

const MOVE: &[&str] = &["item", "move", "REQ-TWO", "docs/moved.mara.md"];

/// The files of a completed move of REQ-TWO.
#[track_caller]
fn assert_moved(project: &Path) {
    let moved = fs::read_to_string(project.join("docs/moved.mara.md")).unwrap();
    let source = fs::read_to_string(project.join("docs/reqs.mara.md")).unwrap();
    assert!(moved.contains("REQ-TWO") && !source.contains("REQ-TWO"));
}

/// `project validate` in direct mode finds no problem.
#[track_caller]
fn assert_valid(test: &Test, project: &Path) {
    let validation = test.direct(project, &["project", "validate"]);
    assert!(validation.status.success(), "{validation:?}");
    assert!(
        validation.stdout.contains("valid: 0 errors, 0 warnings"),
        "{validation:?}"
    );
}

/// Every file of the project with its bytes.
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else {
                let bytes = fs::read(&path).unwrap();
                files.insert(path.strip_prefix(root).unwrap().to_owned(), bytes);
            }
        }
    }
    files
}

const CREATE: &[&str] = &[
    "item",
    "create",
    "requirement",
    "docs/reqs.mara.md",
    "--title",
    "Four",
    "--field",
    "status=draft",
    "--body",
    "Text.",
];

/// The client fails with `daemon-lost` and exit status 1.
#[track_caller]
fn assert_lost(run: &Run, pid: u32) {
    assert_eq!(run.status.code(), Some(1), "{run:?}");
    assert!(
        run.stderr.starts_with(&format!(
            "mara: error[daemon-lost]: the daemon (process {pid}) gave no answer to"
        )),
        "{run:?}"
    );
    assert_eq!(run.stdout, "");
}

#[test]
fn a_create_whose_daemon_is_killed_before_step_2_changes_nothing() {
    let test = Test::new();
    let project = test.project();
    let before = tree(&project);
    let daemon = Paused::start(&test);
    let mut client = test.spawn_in(&project, CREATE);
    assert_eq!(daemon.wait(&mut client), "", "the pause before step 2");
    signal(daemon.pid, libc::SIGKILL);
    let run = finish(client);
    assert_lost(&run, daemon.pid);
    // The client did not run the create itself, and leaves nothing behind.
    test.wait_for_no_daemon(&[]);
    assert_eq!(tree(&project), before);
}

#[test]
fn a_move_whose_daemon_is_killed_at_its_second_file_is_left_interrupted_and_reported() {
    let test = Test::new();
    let project = test.project();
    let daemon = Paused::start(&test);
    let client = daemon.move_paused_at(&test, &project, 2);
    signal(daemon.pid, libc::SIGKILL);
    let run = finish(client);
    assert_lost(&run, daemon.pid);
    test.wait_for_no_daemon(&[]);
    // The destination holds the item, and the source still does: an interrupted move.
    let moved = fs::read_to_string(project.join("docs/moved.mara.md")).unwrap();
    let source = fs::read_to_string(project.join("docs/reqs.mara.md")).unwrap();
    assert!(moved.contains("REQ-TWO") && source.contains("REQ-TWO"));
    // No daemon runs, so the validation is served in-process, and reports it.
    let validation = test.run_in(&project, &["--format", "json", "project", "validate"], &[]);
    assert_eq!(validation.status.code(), Some(1), "{validation:?}");
    let codes: Vec<String> = (validation.json()["diagnostics"].as_array().unwrap().iter())
        .map(|diagnostic| diagnostic["code"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        codes.iter().any(|code| code.starts_with("duplicate-")),
        "{codes:?}"
    );
}

#[test]
fn a_write_through_the_daemon_runs_the_write_procedure_to_its_end() {
    let test = Test::new();
    let project = test.project();
    let daemon = Paused::start(&test);
    let mut client = test.spawn_in(&project, CREATE);
    // The daemon's writer, not the client, pauses: the daemon runs the write.
    assert_eq!(daemon.wait(&mut client), "");
    daemon.release();
    assert_eq!(daemon.wait(&mut client), "docs/reqs.mara.md");
    daemon.release();
    let run = finish(client).succeeded();
    assert!(run.stdout.contains("REQ-FOUR"), "{run:?}");
    let document = fs::read_to_string(project.join("docs/reqs.mara.md")).unwrap();
    assert!(document.contains("REQ-FOUR"));
    let validation = test.direct(&project, &["project", "validate"]);
    assert!(validation.status.success(), "{validation:?}");
}

#[test]
fn ten_creates_sent_at_once_all_land() {
    let test = Test::new();
    let project = test.project();
    test.start_in(&project, &[]);
    let titles: Vec<String> = (0..10).map(|n| format!("Many {n}")).collect();
    let children: Vec<Child> = (titles.iter())
        .map(|title| {
            let args = [
                "item",
                "create",
                "requirement",
                "docs/reqs.mara.md",
                "--title",
                title,
                "--field",
                "status=draft",
                "--body",
                "Text.",
            ];
            test.spawn_in(&project, &args)
        })
        .collect();
    for child in children {
        finish(child).succeeded();
    }
    let document = fs::read_to_string(project.join("docs/reqs.mara.md")).unwrap();
    for title in &titles {
        assert!(
            document.contains(&format!("## {title}\n")),
            "{title}: {document}"
        );
    }
    assert_valid(&test, &project);
}

/// A document saved by another program, then at once a write of the same document through
/// the daemon: the write is planned on the saved document.
#[test]
fn a_write_after_another_program_saved_keeps_what_it_saved() {
    let test = Test::new();
    let project = test.project();
    test.start_in(&project, &[]);
    test.wait_for_session(&project);
    test.run_in(&project, &["item", "list"], &[]).succeeded();
    let path = project.join("docs/reqs.mara.md");
    let saved = fs::read_to_string(&path).unwrap() + "\n---\n\n" + &item(9, "REQ-ADDED", "Added");
    fs::write(&path, saved).unwrap();
    let update = ["item", "update", "REQ-ONE", "--title", "One changed"];
    test.run_in(&project, &update, &[]).succeeded();
    let document = fs::read_to_string(&path).unwrap();
    assert!(document.contains("| ID | REQ-ADDED |"), "{document}");
    assert!(document.contains("## One changed\n"), "{document}");
    assert_valid(&test, &project);
}

#[test]
fn a_write_whose_target_changed_while_it_waited_fails_and_changes_nothing() {
    let test = Test::new();
    let project = test.project();
    let daemon = Paused::start(&test);
    let update = [
        "--format",
        "json",
        "item",
        "update",
        "REQ-ONE",
        "--title",
        "One changed",
    ];
    let mut client = test.spawn_in(&project, &update);
    assert_eq!(daemon.wait(&mut client), "", "the pause before step 2");
    let path = project.join("docs/reqs.mara.md");
    let edited = fs::read_to_string(&path)
        .unwrap()
        .replace("Text.", "Edited by hand.");
    fs::write(&path, &edited).unwrap();
    let left = tree(&project);
    let run = daemon.release_all(client);
    assert_eq!(run.status.code(), Some(1), "{run:?}");
    assert_eq!(run.json()["error"]["code"], "changed-file", "{run:?}");
    assert_eq!(tree(&project), left);
    let client = test.spawn_in(&project, &update);
    daemon.release_all(client).succeeded();
    let document = fs::read_to_string(&path).unwrap();
    assert!(document.contains("## One changed\n") && document.contains("Edited by hand."));
}

/// `mara daemon stop`, or SIGTERM, while a move waits in step 3 of its first file: the move
/// completes, then the daemon exits.
fn a_running_write_finishes_when_the_daemon_stops(by_signal: bool) {
    let test = Test::new();
    let project = test.project();
    let daemon = Paused::start(&test);
    let client = daemon.move_paused_at(&test, &project, 1);
    let stopping = match by_signal {
        true => {
            signal(daemon.pid, libc::SIGTERM);
            None
        }
        false => Some(test.spawn(&crate::mara(), &["daemon", "stop"], &[])),
    };
    thread::sleep(Duration::from_millis(500));
    assert!(
        test.daemons().contains(&daemon.pid),
        "the daemon ended before its write"
    );
    let run = daemon.release_all(client).succeeded();
    assert!(run.stdout.contains("REQ-TWO"), "{run:?}");
    assert_moved(&project);
    test.wait_for_no_daemon(&[]);
    if let Some(stopping) = stopping {
        let stopped = finish(stopping).succeeded();
        assert!(
            stopped.stdout.starts_with("Stopped the daemon"),
            "{stopped:?}"
        );
    }
    assert_valid(&test, &project);
}

#[test]
fn a_running_write_finishes_when_the_daemon_is_told_to_stop() {
    a_running_write_finishes_when_the_daemon_stops(false);
}

#[test]
fn a_running_write_finishes_on_sigterm() {
    a_running_write_finishes_when_the_daemon_stops(true);
}

/// The last case of VER-NEVER-STALE: a read while a move waits in step 3 of its second file
/// returns what direct mode returns then, the destination already written.
#[test]
fn a_read_while_a_move_waits_answers_from_the_files_as_they_are() {
    let test = Test::new();
    let project = test.project();
    let daemon = Paused::start(&test);
    let read = ["--format", "json", "item", "list"];
    test.run_in(&project, &read, &[]).succeeded();
    let client = daemon.move_paused_at(&test, &project, 2);
    let served = test.run_in(&project, &read, &[]);
    let direct = test.direct(&project, &read);
    assert_eq!(served, direct);
    let listed = direct.json();
    let paths: Vec<&str> = (listed["items"].as_array().unwrap().iter())
        .map(|item| item["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"docs/moved.mara.md"), "{paths:?}");
    daemon.release_all(client).succeeded();
    assert_moved(&project);
}
