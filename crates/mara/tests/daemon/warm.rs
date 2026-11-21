//! Warm rows of VER-WORK-COUNTERS (`docs/verification.mara.md`) that follow an edit, a write or
//! an earlier evaluation, through a daemon's session (DES-DAEMON-SESSIONS): an edited document
//! is read again by the request after the watcher's derivation, a write leaves its read-back
//! parses in the session and reads no other input again, and a session keeps its evaluations
//! by key. Where a case depends on the watcher's derivation, the test waits for it through
//! `MARA_TEST_DERIVED`, which a debug build's daemon appends each background derivation's
//! counts to, never for a deadline.

#![cfg(debug_assertions)]

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::fresh::project;
use crate::{Test, WAIT, settle, stats};

const LIST: &[&str] = &["--format", "json", "item", "list"];

/// The counts of `args` with `--debug-stats`, sent to the daemon in `root`; the run succeeds.
fn counted(test: &Test, root: &Path, args: &[&str]) -> Value {
    let counted: Vec<&str> = ["--debug-stats"].iter().chain(args).copied().collect();
    let run = test.run_in(root, &counted, &[]).succeeded();
    stats(&run)
}

/// `counter` of `stats`.
fn count(stats: &Value, counter: &str) -> u64 {
    stats[counter]
        .as_u64()
        .unwrap_or_else(|| panic!("no {counter}: {stats}"))
}

/// Asserts each of `expected` in `stats`.
#[track_caller]
fn assert_counts(stats: &Value, expected: &[(&str, u64)], what: &str) {
    for &(counter, value) in expected {
        assert_eq!(count(stats, counter), value, "{counter} of {what}: {stats}");
    }
}

/// The counts of a background derivation, as the daemon appends them to `MARA_TEST_DERIVED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Derived {
    file_reads: u64,
    hashes: u64,
    parses: u64,
    snapshot_builds: u64,
}

/// A started daemon serving the project at `root`, its session open and settled, that reports
/// each background derivation to a file of the test.
struct Watched {
    derived: PathBuf,
}

impl Watched {
    fn start(test: &Test, root: &Path) -> Watched {
        Watched::start_with(test, root, &[])
    }

    /// [`Watched::start`] with `env` in the daemon's environment too.
    fn start_with(test: &Test, root: &Path, env: &[(&str, &str)]) -> Watched {
        let derived = test.path().join("derived");
        let mut env = env.to_vec();
        env.push(("MARA_TEST_DERIVED", derived.to_str().unwrap()));
        test.start_in(root, &env);
        test.wait_for_session(root);
        for args in [LIST, &["--format", "json", "project", "validate"]] {
            test.run_in(root, args, &[]);
        }
        Watched { derived }
    }

    /// The derivations reported so far.
    fn derivations(&self) -> Vec<Derived> {
        let text = fs::read_to_string(&self.derived).unwrap_or_default();
        (text.lines())
            .map(|line| {
                let n: Vec<u64> = line.split(' ').map(|n| n.parse().unwrap()).collect();
                Derived {
                    file_reads: n[0],
                    hashes: n[1],
                    parses: n[2],
                    snapshot_builds: n[3],
                }
            })
            .collect()
    }

    /// Waits until a derivation after the first `seen` is reported: those after them.
    fn wait_after(&self, seen: usize) -> Vec<Derived> {
        let deadline = Instant::now() + WAIT;
        loop {
            let derivations = self.derivations();
            if derivations.len() > seen {
                return derivations[seen..].to_vec();
            }
            assert!(Instant::now() < deadline, "the watcher did not derive");
            thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Settles the project's files and the session's record of them: 2 s, then a request.
fn settled(test: &Test, root: &Path) {
    settle();
    counted(test, root, LIST);
}

/// Sleeps until 2 s after `since`.
fn two_seconds_after(since: Instant) {
    let until = since + Duration::from_secs(2);
    thread::sleep(until.saturating_duration_since(Instant::now()));
}

#[test]
fn an_edited_document_is_read_by_the_watcher_and_again_by_the_request_that_settles_it() {
    let test = Test::new();
    let root = project(&test, "project");
    let watched = Watched::start(&test, &root);
    let more = root.join("docs/more.mara.md");
    let original = fs::read_to_string(&more).unwrap();
    for (title, later) in [("Three peach", false), ("Three fig", true)] {
        settled(&test, &root);
        let seen = watched.derivations().len();
        let edited = original.replace("Three plum", title);
        match later {
            false => fs::write(&more, &edited).unwrap(),
            true => {
                let staged = more.with_extension("swp");
                fs::write(&staged, &edited).unwrap();
                fs::rename(&staged, &more).unwrap();
            }
        }
        let edited_at = Instant::now();
        let reads = if later {
            // After the watcher's derivation, which read the document, and 2 s after the edit.
            let derivations = watched.wait_after(seen);
            let first = derivations[0];
            assert_eq!((first.file_reads, first.parses), (1, 1), "{derivations:?}");
            two_seconds_after(edited_at);
            2
        } else {
            0
        };
        let next = counted(&test, &root, LIST);
        let read = count(&next, "file_reads");
        match later {
            false => assert!((1..=2).contains(&read), "{next}"),
            true => assert_eq!(read, reads, "{next}"),
        }
        let expected = [
            ("hashes", read),
            ("parses", 1),
            ("snapshot_builds", 1),
            ("code_scans", 0),
        ];
        assert_counts(
            &next,
            &expected,
            if later { "2 s later" } else { "at once" },
        );
        let served = test.run_in(&root, LIST, &[]);
        assert_eq!(served, test.direct(&root, LIST));
        assert!(served.stdout.contains(title), "{}", served.stdout);
    }
}

#[test]
fn a_write_leaves_its_parses_and_the_next_request_reads_only_the_written_file() {
    let test = Test::new();
    let root = project(&test, "project");
    let watched = Watched::start(&test, &root);
    for (title, later) in [("Three peach", false), ("Three fig", true)] {
        settled(&test, &root);
        let seen = watched.derivations().len();
        let update = ["item", "update", "REQ-THREE", "--title", title];
        let write = counted(&test, &root, &update);
        let written_at = Instant::now();
        let expected = [
            ("file_reads", 0),
            ("hashes", 1),
            ("parses", 1),
            ("snapshot_builds", 1),
        ];
        assert_counts(&write, &expected, "the write");
        if later {
            // The watcher's derivation after the renames read nothing again: the hand-over
            // recorded the written file.
            let derivations = watched.wait_after(seen);
            for derived in &derivations {
                assert_eq!(derived.file_reads, 0, "{derivations:?}");
                assert_eq!(derived.parses, 0, "{derivations:?}");
            }
            two_seconds_after(written_at);
        }
        let next = counted(&test, &root, LIST);
        let expected = [
            ("file_reads", 1),
            ("hashes", 1),
            ("parses", 0),
            ("snapshot_builds", 0),
        ];
        assert_counts(
            &next,
            &expected,
            if later { "2 s later" } else { "at once" },
        );
        let served = test.run_in(&root, LIST, &[]);
        assert_eq!(served, test.direct(&root, LIST));
        assert!(served.stdout.contains(title), "{}", served.stdout);
    }
}

/// DES-WRITE-PROCEDURE step 4: the hand-over after a write re-derives only the files written,
/// and reads no other input again, not even one that the write's own check found unsettled.
#[test]
fn a_write_reads_no_other_input_again_after_its_renames() {
    let test = Test::new();
    let root = project(&test, "project");
    let watched = Watched::start(&test, &root);
    settled(&test, &root);
    let seen = watched.derivations().len();
    let reqs = root.join("docs/reqs.mara.md");
    let saved = fs::read_to_string(&reqs)
        .unwrap()
        .replace("Two pear", "Two quince");
    fs::write(&reqs, saved).unwrap();
    // The watcher reads the saved document; the write's check reads it again, unsettled.
    watched.wait_after(seen);
    let derivations = watched.derivations().split_off(seen);
    let sum = |counter: fn(&Derived) -> u64| derivations.iter().map(counter).sum::<u64>();
    let watcher = Derived {
        file_reads: sum(|d| d.file_reads),
        hashes: sum(|d| d.hashes),
        parses: sum(|d| d.parses),
        snapshot_builds: sum(|d| d.snapshot_builds),
    };
    assert_eq!(watcher.file_reads, 1, "{derivations:?}");
    let update = ["item", "update", "REQ-THREE", "--title", "Three peach"];
    let write = counted(&test, &root, &update);
    let expected = [
        ("file_reads", watcher.file_reads + 1),
        ("hashes", watcher.hashes + 2),
        ("parses", watcher.parses + 1),
        ("snapshot_builds", watcher.snapshot_builds + 1),
    ];
    assert_counts(&write, &expected, "the write after the watcher");
    let served = test.run_in(&root, LIST, &[]);
    assert_eq!(served, test.direct(&root, LIST));
}

/// A write's window: no background derivation runs between its first rename and its hand-over,
/// so none reads or parses a file that the hand-over brings with its read-back parse. Here the
/// writer waits at its pause point before the second file's rename, well past the watcher's
/// debounce, after the first file was renamed.
#[test]
fn no_derivation_runs_between_a_writes_first_rename_and_its_hand_over() {
    let test = Test::new();
    let root = project(&test, "project");
    let pauses = test.path().join("pauses");
    fs::create_dir(&pauses).unwrap();
    let watched = Watched::start_with(
        &test,
        &root,
        &[("MARA_TEST_PAUSE", pauses.to_str().unwrap())],
    );
    settled(&test, &root);
    let seen = watched.derivations().len();
    let paused = pauses.join("paused");
    let args = [
        "--debug-stats",
        "item",
        "move",
        "REQ-TWO",
        "docs/moved.mara.md",
    ];
    let mut client = test.spawn_in(&root, &args);
    // Before step 2, then in step 3 of each file: the destination, then the source.
    for expected in ["", "docs/moved.mara.md", "docs/reqs.mara.md"] {
        let deadline = Instant::now() + WAIT;
        let at = loop {
            if let Ok(at) = fs::read_to_string(&paused) {
                break at;
            }
            assert!(client.try_wait().unwrap().is_none(), "the client ended");
            assert!(Instant::now() < deadline, "the writer did not pause");
            thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(at, expected);
        if expected == "docs/reqs.mara.md" {
            thread::sleep(Duration::from_millis(500));
            let during = watched.derivations().split_off(seen);
            assert_eq!(during, [], "a derivation ran in the write's window");
        }
        fs::remove_file(&paused).unwrap();
    }
    let run = crate::finish(client).succeeded();
    let expected = [
        ("file_reads", 0),
        ("hashes", 2),
        ("parses", 2),
        ("snapshot_builds", 1),
    ];
    assert_counts(&stats(&run), &expected, "the move");
    for derived in watched.wait_after(seen) {
        assert_eq!((derived.file_reads, derived.parses), (0, 0), "{derived:?}");
    }
    let served = test.run_in(&root, LIST, &[]);
    assert_eq!(served, test.direct(&root, LIST));
}

/// The background derivation keeps the configuration and the schema, like every input, when
/// their stat is unchanged and reading them could not settle them.
#[test]
fn the_watcher_reads_no_unsettled_schema_again() {
    let test = Test::new();
    let root = project(&test, "project");
    let watched = Watched::start(&test, &root);
    settled(&test, &root);
    let schema = root.join(".mara/schema.toml");
    let edited = fs::read_to_string(&schema)
        .unwrap()
        .replace("A requirement.", "A need.");
    let seen = watched.derivations().len();
    fs::write(&schema, edited).unwrap();
    let derivations = watched.wait_after(seen);
    assert_eq!(derivations[0].file_reads, 1, "the schema: {derivations:?}");
    // Within 2 s of the schema's change, a document saved.
    let seen = watched.derivations().len();
    let more = root.join("docs/more.mara.md");
    let saved = fs::read_to_string(&more)
        .unwrap()
        .replace("Three plum", "Three fig");
    fs::write(&more, saved).unwrap();
    let derivations = watched.wait_after(seen);
    assert_eq!(
        derivations[0].file_reads, 1,
        "the document alone: {derivations:?}"
    );
    let served = test.run_in(&root, LIST, &[]);
    assert_eq!(served, test.direct(&root, LIST));
}

/// The row "A write that gives a file the contents of another input": the read-back takes the
/// parse of the document that has those contents.
#[test]
fn a_write_that_gives_a_file_the_contents_of_another_input_parses_nothing() {
    let test = Test::new();
    let root = project(&test, "project");
    let more = fs::read_to_string(root.join("docs/more.mara.md")).unwrap();
    let extra = crate::fresh::item(4, "REQ-FOUR", "Four kiwi");
    let copy = root.join("docs/copy.mara.md");
    fs::write(&copy, format!("{more}\n---\n\n{extra}")).unwrap();
    let watched = Watched::start(&test, &root);
    settled(&test, &root);
    let seen = watched.derivations().len();
    let write = counted(&test, &root, &["item", "delete", "REQ-FOUR"]);
    assert_eq!(
        fs::read_to_string(&copy).unwrap(),
        more,
        "the case's contents"
    );
    let expected = [("file_reads", 0), ("parses", 0), ("snapshot_builds", 1)];
    assert_counts(&write, &expected, "the delete");
    watched.wait_after(seen);
    let served = test.run_in(&root, LIST, &[]);
    assert_eq!(served, test.direct(&root, LIST));
}

#[test]
fn a_request_reads_the_evaluations_that_an_earlier_request_ran() {
    let test = Test::new();
    let root = project(&test, "project");
    Watched::start(&test, &root);
    let validate: &[&str] = &["--format", "json", "project", "validate"];
    let execute: &[&str] = &["--format", "json", "query", "execute", "flagged-items"];
    let notes: &[&str] = &["--format", "json", "query", "execute", "notes", "--input"];
    let counted = |test: &Test, root: &Path, args: &[&str]| {
        let counted: Vec<&str> = ["--debug-stats"].iter().chain(args).copied().collect();
        stats(&test.run_in(root, &counted, &[]))
    };
    // The project was validated once already: this validation finds every evaluation.
    assert_eq!(count(&counted(&test, &root, validate), "program_runs"), 0);
    assert_eq!(count(&counted(&test, &root, execute), "program_runs"), 0);
    let first = [notes, &["note=first"]].concat();
    let second = [notes, &["note=second"]].concat();
    assert_eq!(count(&counted(&test, &root, &first), "program_runs"), 1);
    assert_eq!(count(&counted(&test, &root, &second), "program_runs"), 1);
    assert_eq!(count(&counted(&test, &root, &first), "program_runs"), 0);
    for args in [validate, execute, &first] {
        assert_eq!(
            test.run_in(&root, args, &[]),
            test.direct(&root, args),
            "{args:?}"
        );
    }

    // A document edit: the program that an invariant gives the snapshot's facts runs again,
    // once; the other, run against empty facts, does not.
    settle();
    let reqs = root.join("docs/reqs.mara.md");
    let edited = fs::read_to_string(&reqs)
        .unwrap()
        .replace("REQ-ONE", "REQ-UNO");
    fs::write(&reqs, edited).unwrap();
    let validated = counted(&test, &root, validate);
    assert_eq!(count(&validated, "program_runs"), 1, "{validated}");
    assert_eq!(count(&counted(&test, &root, execute), "program_runs"), 0);
    assert_eq!(
        test.run_in(&root, execute, &[]),
        test.direct(&root, execute)
    );
}
