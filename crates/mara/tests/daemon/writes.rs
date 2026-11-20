//! Writes through the daemon: the SIGKILL cases of VER-DAEMON-WRITES (REQ-INTERRUPTED-EDIT-LOSES-NOTHING,
//! DES-DAEMON-LIFECYCLE), and a write that the daemon runs to its end. The daemon is started by
//! `mara daemon start` with `MARA_TEST_PAUSE`, so that its writer waits at the pause points of
//! DES-WRITE-PROCEDURE (`docs/authoring/writes.mara.md`); the clients run without it.

#![cfg(debug_assertions)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

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
            "mara: error[daemon-lost]: the daemon (process {pid}) ended before it answered"
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
    let args = ["item", "move", "REQ-TWO", "docs/moved.mara.md"];
    let mut client = test.spawn_in(&project, &args);
    let mut files = Vec::new();
    loop {
        let paused = daemon.wait(&mut client);
        if !paused.is_empty() {
            files.push(paused);
        }
        if files.len() == 2 {
            break;
        }
        daemon.release();
    }
    assert_eq!(files, ["docs/moved.mara.md", "docs/reqs.mara.md"]);
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
