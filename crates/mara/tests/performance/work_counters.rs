//! VER-WORK-COUNTERS: exact work counts of cold requests through the real CLI, and of input
//! tracking and the snapshot build below it.
//!
//! Each operation adds its case to `cold_counts_at`, with its exit status. The warm rows run
//! against the daemon (0.4.0-alpha.4).

use std::fs;
use std::time::{Duration, SystemTime};

use mara::inputs::{Generation, Inputs};
use mara_engine::work::WorkCounts;

use crate::corpus::Corpus;
use crate::{NIGHTLY_SIZE, PULL_REQUEST_SIZES, SEED, cli};

#[test]
fn cold_counts() {
    for size in PULL_REQUEST_SIZES {
        cold_counts_at(size);
    }
}

#[test]
#[ignore = "nightly: cargo test --release --package mara --test performance -- --ignored"]
fn cold_counts_64x() {
    cold_counts_at(NIGHTLY_SIZE);
}

fn cold_counts_at(size: usize) {
    let corpus = Corpus::generate(size, SEED);

    // Without an operation the CLI reads nothing, `.git` and undeclared files included. The
    // exit status is what `mara` does today.
    let run = cli::run(corpus.root(), &[], 0);
    assert_eq!(run.work, WorkCounts::default(), "no operation at {size}×");

    // An undeclared argument is rejected before anything runs (REQ-CLI-MCP-PARITY), with its
    // diagnostic before the stats line.
    let run = cli::run(corpus.root(), &["--no-such-option"], 2);
    assert_eq!(
        run.work,
        WorkCounts::default(),
        "rejected argument at {size}×"
    );
    let stderr = String::from_utf8_lossy(&run.output.stderr);
    let lines: Vec<&str> = stderr.lines().collect();
    assert!(
        lines[..lines.len() - 1]
            .iter()
            .any(|line| line.contains("--no-such-option")),
        "no diagnostic before the stats line: {stderr:?}"
    );
}

/// Input tracking and the snapshot build below the CLI: a cold build, then builds in the same
/// process that do work only for what changed, as a host that keeps state between requests
/// does (REQ-WORK-FOLLOWS-CHANGE).
#[test]
fn snapshot_counts() {
    for size in PULL_REQUEST_SIZES {
        snapshot_counts_at(size);
    }
}

#[test]
#[ignore = "nightly: cargo test --release --package mara --test performance -- --ignored"]
fn snapshot_counts_64x() {
    snapshot_counts_at(NIGHTLY_SIZE);
}

fn snapshot_counts_at(size: usize) {
    let corpus = Corpus::generate(size, SEED);
    let root = corpus.root();
    let mut inputs = Inputs::new(root.to_owned());
    // Each request starts at least 3 s after the last one and after every file written before
    // it, so that every check settles what it read.
    let mut last = SystemTime::UNIX_EPOCH;
    let mut request = |inputs: &mut Inputs| -> (Generation, WorkCounts) {
        last = last.max(SystemTime::now()) + Duration::from_secs(3);
        let mut work = WorkCounts::default();
        (inputs.snapshot(last, &mut work), work)
    };

    // Cold: the configuration, the schema, the root `.gitignore` and each declared document are
    // read and hashed once; `.git`, ignored and undeclared files never.
    let documents = corpus.documents as u64;
    let (cold, work) = request(&mut inputs);
    let read = documents + 3;
    let expected = WorkCounts {
        file_reads: read,
        hashes: read,
        parses: documents,
        snapshot_builds: 1,
        ..WorkCounts::default()
    };
    assert_eq!(work, expected, "cold build at {size}×");
    assert_eq!(cold.project.diagnostics, [], "at {size}×");
    assert_eq!(cold.diagnostics, [], "at {size}×");
    let snapshot = &cold.snapshot;
    assert_eq!(snapshot.documents().len(), corpus.documents, "at {size}×");
    assert_eq!(snapshot.items().len(), corpus.items, "at {size}×");

    let (_, work) = request(&mut inputs);
    assert_eq!(work, WorkCounts::default(), "unchanged at {size}×");

    let edited = root.join(&snapshot.documents()[snapshot.documents().len() / 2].path);
    let mut contents = fs::read_to_string(&edited).unwrap();
    contents.push_str("\nOne more paragraph.\n");
    fs::write(&edited, contents).unwrap();
    let (_, work) = request(&mut inputs);
    let one = WorkCounts {
        file_reads: 1,
        hashes: 1,
        parses: 1,
        snapshot_builds: 1,
        ..WorkCounts::default()
    };
    assert_eq!(work, one, "one document edited at {size}×");

    for (path, contents) in [
        ("README.md", "# Changed\n"),
        ("notes/draft.mara.md", "Changed.\n"),
        ("docs/generated/index.mara.md", "Changed.\n"),
        ("target/debug/app", "changed"),
        ("src/a000/c0000/new.rs", "fn main() {}\n"),
        (".git/objects/00/new", "object"),
        ("docs/a000/notes.txt", "Not a document.\n"),
    ] {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    let (_, work) = request(&mut inputs);
    assert_eq!(
        work,
        WorkCounts::default(),
        "undeclared files changed at {size}×"
    );
}
