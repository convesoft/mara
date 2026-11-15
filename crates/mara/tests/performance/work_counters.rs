//! VER-WORK-COUNTERS: exact work counts of cold requests through the real CLI, and of input
//! tracking and the snapshot build below it.
//!
//! Each operation adds its case to `cli::cases`, which `cold_counts_at` runs. The warm rows run
//! against the daemon (0.4.0-alpha.4).

use std::fs;
use std::time::{Duration, SystemTime};

use mara::inputs::{Generation, Inputs};
use mara_engine::work::WorkCounts;

use crate::cli::Reads;
use crate::corpus::{Corpus, Hub};
use crate::restore::{self, Written};
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

    // Without an operation the CLI prints its usage and reads nothing, `.git` and undeclared
    // files included.
    let run = cli::run(corpus.root(), &[] as &[&str], 0);
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

    // Two documents with the same contents are parsed once (DES-INPUTS-AND-KEYS). Narrative
    // without items keeps the corpus valid.
    let shared = "# Shared notes\n\nThe same narrative in two documents.\n";
    for path in ["docs/a000/shared-1.mara.md", "docs/a000/shared-2.mara.md"] {
        fs::write(corpus.root().join(path), shared).unwrap();
    }
    let documents = corpus.documents as u64 + 2;
    let mut last_write = SystemTime::now();
    for case in cli::cases(&corpus) {
        let mut written = None;
        let run = match case.reads {
            // Every file it writes last changed at least 2 s before.
            Reads::Writes => {
                restore::settle(last_write);
                let mut run = None;
                written = Some(Written::learn(corpus.root(), || {
                    run = Some(cli::run(corpus.root(), &case.args, case.status));
                }));
                run.expect("the case ran")
            }
            _ => cli::run(corpus.root(), &case.args, case.status),
        };
        let name = format!("mara {} at {size}×", case.args.join(" "));
        // The configuration and the schema; for a check, also the root `.gitignore` and every
        // document. `.git`, ignored and undeclared files are never read. No code is read and
        // no program runs.
        let expected = match case.reads {
            Reads::Schema => WorkCounts {
                file_reads: 2,
                hashes: 2,
                ..WorkCounts::default()
            },
            Reads::Checked => WorkCounts {
                file_reads: documents + 3,
                hashes: documents + 3,
                parses: documents - 1,
                snapshot_builds: 1,
                check_steps: run.work.check_steps,
                ..WorkCounts::default()
            },
            Reads::Documents => WorkCounts {
                file_reads: documents + 3,
                hashes: documents + 3,
                parses: documents - 1,
                snapshot_builds: 1,
                ..WorkCounts::default()
            },
            // A cold request, then 1 parse per distinct content written, before the renames,
            // and nothing after them (DES-WRITE-PROCEDURE, ADR-READ-BACK-EVERY-EDIT).
            Reads::Writes => WorkCounts {
                file_reads: documents + 3,
                hashes: documents + 3,
                parses: documents - 1 + written.as_ref().map_or(0, Written::distinct),
                snapshot_builds: 1,
                ..WorkCounts::default()
            },
        };
        assert_eq!(run.work, expected, "{name}");
        if let Some(written) = written {
            assert!(!written.contents.is_empty(), "{name} wrote nothing");
            written.restore();
            last_write = SystemTime::now();
        }
        // Built-in checks visit each item twice and each edge once, and the items and edges of
        // acyclic relations once more (REQ-NEAR-LINEAR-SCALING).
        if case.reads == Reads::Checked {
            let steps = run.work.check_steps;
            let (items, edges) = (corpus.items as u64, corpus.edges as u64);
            assert!(
                (2 * items + edges..=3 * items + 2 * edges).contains(&steps),
                "{name}: {steps} check steps for {items} items and {edges} edges"
            );
        }
    }
}

/// Late metadata and many incoming edges: each visit's work is bounded, so the check steps are
/// exactly the visits (DES-PERFORMANCE-HARNESS).
#[test]
fn hub_counts() {
    for size in PULL_REQUEST_SIZES {
        let hub = Hub::generate(size);
        let generated = SystemTime::now();
        let run = cli::run(hub.root(), &["project", "validate"], 0);
        let documents = hub.edges.div_ceil(100) as u64 + 1;
        let (items, edges) = (hub.items as u64, hub.edges as u64);
        let expected = WorkCounts {
            file_reads: documents + 2,
            hashes: documents + 2,
            parses: documents,
            snapshot_builds: 1,
            // The facts and item passes, the edge pass, and the cycle pass over every edge and
            // item of the acyclic `refines`.
            check_steps: 2 * items + edges + (items + edges),
            ..WorkCounts::default()
        };
        assert_eq!(run.work, expected, "hub at {size}×");
        // A rename and a move of the hub rewrite every edge's link (DES-PERFORMANCE-HARNESS).
        let mut last_write = generated;
        for args in hub_writes() {
            restore::settle(last_write);
            let mut run = None;
            let written = Written::learn(hub.root(), || run = Some(cli::run(hub.root(), &args, 0)));
            let expected = WorkCounts {
                file_reads: documents + 2,
                hashes: documents + 2,
                parses: documents + written.distinct(),
                snapshot_builds: 1,
                ..WorkCounts::default()
            };
            assert_eq!(
                written.contents.len() as u64,
                documents + u64::from(args[1] == "move")
            );
            assert_eq!(
                run.expect("the case ran").work,
                expected,
                "{args:?} at {size}×"
            );
            written.restore();
            last_write = SystemTime::now();
        }
    }
}

/// The rename and the move of the hub item.
pub fn hub_writes() -> [[&'static str; 4]; 2] {
    [
        ["item", "rename", "REQ-HUB", "REQ-HUB-RENAMED"],
        ["item", "move", "REQ-HUB", "docs/moved/hub.mara.md"],
    ]
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
