//! How a daemon's session keeps the work of the warm rows of VER-WORK-COUNTERS small after a
//! change (DES-DAEMON-SESSIONS): the hand-over after a write reads no other input again, no
//! background derivation runs in a write's window, and the background derivation reads no
//! unsettled schema again. The rows themselves run at 1× in the `performance` test
//! (`tests/performance/warm.rs`).
//!
//! The test waits for the watcher through the daemon's reports of its derivations
//! (`MARA_TEST_DERIVED`, debug builds only), never for a deadline, and learns from them which
//! case a run is in (`derived::Case`): a derivation that checked a change less than 2 s after
//! it, or one that checked it later and so settled what it read. The expected counts are fixed
//! for each case.

#![cfg(debug_assertions)]

use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use crate::derived::{self, Case, Reports, decided, until_settled};
use crate::fresh::project;
use crate::{Test, WAIT, stats};

const LIST: &[&str] = &["--format", "json", "item", "list"];

/// The counts of `args` with `--debug-stats`, sent to the daemon in `root`; the run succeeds.
fn counted(test: &Test, root: &Path, args: &[&str]) -> Value {
    let counted: Vec<&str> = ["--debug-stats"].iter().chain(args).copied().collect();
    let run = test.run_in(root, &counted, &[]).succeeded();
    stats(&run)
}

/// Asserts each of `expected` in `stats`.
#[track_caller]
fn assert_counts(stats: &Value, expected: &[(&str, u64)], what: &str) {
    for &(counter, value) in expected {
        let count = stats[counter].as_u64();
        assert_eq!(count, Some(value), "{counter} of {what}: {stats}");
    }
}

/// Starts a daemon serving the project at `root`, with `env` in its environment too, its
/// session open and settled, which reports each background derivation to a file of the test.
fn start(test: &Test, root: &Path, env: &[(&str, &str)]) -> Reports {
    let reports = Reports::new(test.path().join("derived"));
    let mut env = env.to_vec();
    env.push(("MARA_TEST_DERIVED", reports.path().to_str().unwrap()));
    test.start_in(root, &env);
    test.wait_for_session(root);
    for args in [LIST, &["--format", "json", "project", "validate"]] {
        test.run_in(root, args, &[]);
    }
    reports
}

/// Settles the files changed at `changed` and the session's record of them: a request 2 s
/// after the change. The number of derivations reported then.
fn settled(test: &Test, root: &Path, reports: &Reports, changed: SystemTime) -> usize {
    until_settled(changed);
    counted(test, root, LIST);
    reports.seen()
}

/// DES-WRITE-PROCEDURE step 4: the hand-over after a write re-derives only the files written,
/// and reads no other input again, not even one that the write's own check found unsettled.
#[test]
fn a_write_reads_no_other_input_again_after_its_renames() {
    let test = Test::new();
    let root = project(&test, "project");
    let reports = start(&test, &root, &[]);
    let reqs = root.join("docs/reqs.mara.md");
    let mut changed = SystemTime::now();
    decided("a write after a saved document", |run| {
        let seen = settled(&test, &root, &reports, changed);
        let saved = fs::read_to_string(&reqs)
            .unwrap()
            .replace("Two pear", &format!("Two quince {run}"));
        fs::write(&reqs, saved).unwrap();
        changed = derived::changed(&[&reqs]);
        // The watcher reads, hashes and parses the saved document and builds the snapshot.
        // Unsettled, the write's check reads and hashes it again; settled, not. The write adds
        // 1 hash, 1 parse and 1 snapshot build of the file it writes.
        reports.wait_after(seen);
        let title = format!("Three peach {run}");
        let update = ["item", "update", "REQ-THREE", "--title", &title];
        let (write, before) = reports.around(seen, || counted(&test, &root, &update))?;
        let again = match Case::of(&before, changed) {
            Case::None => panic!("the watcher's derivation was awaited"),
            Case::Early(_) => 1,
            Case::Late(_) => 0,
        };
        let expected = [
            ("file_reads", 1 + again),
            ("hashes", 2 + again),
            ("parses", 2),
            ("snapshot_builds", 2),
        ];
        assert_counts(&write, &expected, "the write after the watcher");
        changed = derived::changed(&[&reqs, &root.join("docs/more.mara.md")]);
        Ok(())
    });
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
    let reports = start(
        &test,
        &root,
        &[("MARA_TEST_PAUSE", pauses.to_str().unwrap())],
    );
    let paused = pauses.join("paused");
    let mut from = "docs/reqs.mara.md".to_owned();
    let mut changed = SystemTime::now();
    decided("a move", |run| {
        let seen = settled(&test, &root, &reports, changed);
        // Each run moves REQ-TWO to a new document.
        let to = format!("docs/moved-{run}.mara.md");
        let args = ["--debug-stats", "item", "move", "REQ-TWO", &to];
        let mut client = test.spawn_in(&root, &args);
        // Before step 2, then in step 3 of each file: the destination, then the source.
        for expected in ["", to.as_str(), from.as_str()] {
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
            if expected == from {
                thread::sleep(Duration::from_millis(500));
                let during = reports.all().split_off(seen);
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
        let written = [root.join(&from), root.join(&to)];
        from = to;
        // The derivation after the hand-over reads neither written file again within 2 s of the
        // first rename, and reads both, to settle them, 2 s or more after the second.
        let first = written.iter().map(|path| derived::changed(&[path])).min();
        changed = derived::changed(&written);
        let derivations = reports.wait_after(seen);
        let reads = match (
            Case::of(&derivations, first.unwrap()),
            Case::of(&derivations, changed),
        ) {
            (Case::Early(_), _) => 0,
            (_, Case::Late(_)) => 2,
            _ => return Err("the watcher came between the two renames' 2 s".to_owned()),
        };
        let derived = derivations[0];
        assert_eq!(
            (derived.file_reads, derived.parses),
            (reads, 0),
            "{derived:?}"
        );
        Ok(())
    });
    let served = test.run_in(&root, LIST, &[]);
    assert_eq!(served, test.direct(&root, LIST));
}

/// The background derivation keeps the configuration and the schema, like every input, when
/// their stat is unchanged and reading them could not settle them.
#[test]
fn the_watcher_reads_no_unsettled_schema_again() {
    let test = Test::new();
    let root = project(&test, "project");
    let reports = start(&test, &root, &[]);
    let schema = root.join(".mara/schema.toml");
    let more = root.join("docs/more.mara.md");
    let mut changed = SystemTime::now();
    decided("a document saved after the schema", |run| {
        let seen = settled(&test, &root, &reports, changed);
        let edited = fs::read_to_string(&schema)
            .unwrap()
            .replace("A requirement.", &format!("A need {run}."));
        fs::write(&schema, edited).unwrap();
        let schema_changed = derived::changed(&[&schema]);
        let derivations = reports.wait_after(seen);
        assert_eq!(derivations[0].file_reads, 1, "the schema: {derivations:?}");
        // Within 2 s of the schema's change, a document saved.
        let seen = reports.seen();
        let saved = fs::read_to_string(&more)
            .unwrap()
            .replace("Three plum", &format!("Three fig {run}"));
        fs::write(&more, saved).unwrap();
        changed = derived::changed(&[&more]);
        let derivations = reports.wait_after(seen);
        // The document alone, or with the schema when the derivation checked it 2 s or more
        // after its change, which settles it.
        let reads = match Case::of(&derivations, schema_changed) {
            Case::Late(_) => 2,
            _ => 1,
        };
        assert_eq!(derivations[0].file_reads, reads, "{derivations:?}");
        Ok(())
    });
    let served = test.run_in(&root, LIST, &[]);
    assert_eq!(served, test.direct(&root, LIST));
}
