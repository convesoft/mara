//! VER-WORK-COUNTERS: exact work counts of cold requests through the real CLI, and of input
//! tracking and the snapshot build below it.
//!
//! Each operation adds its case to `cli::cases`, which `cold_counts_at` runs, as do the calls of
//! `mcp::cases` through `mara mcp`. The warm rows run against the daemon (0.4.0-alpha.4).

use std::fs;
use std::time::{Duration, SystemTime};

use mara::inputs::{Generation, Inputs};
use mara_engine::work::WorkCounts;

use crate::cli::{Code, Reads};
use crate::corpus::{Corpus, Hub, PROGRAMS};
use crate::restore::{self, Written};
use crate::{NIGHTLY_SIZE, PULL_REQUEST_SIZES, SEED, cli, mcp};

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
    // Two code files with the same contents are scanned once.
    let copied = "fn shared() {}\n// @mara implements REQ-NONE-SUCH\n";
    let copied = copied.replace("REQ-NONE-SUCH", &corpus.ids[5].0);
    for path in ["src/a000/shared-1.rs", "src/a000/shared-2.rs"] {
        fs::write(corpus.root().join(path), &copied).unwrap();
    }
    let code = (
        corpus.code_files.len() as u64 + 2,
        corpus.code_contents as u64 + 1,
    );
    let totals = Totals { documents, code };
    let mut last_write = SystemTime::now();
    for case in cli::cases(&corpus) {
        let name = format!("mara {} at {size}×", case.args.join(" "));
        let run = || cli::run(corpus.root(), &case.args, case.status);
        check_cold(
            &corpus,
            &name,
            (case.reads, case.code),
            totals,
            &mut last_write,
            run,
        );
    }
    // `mara mcp`: a session without a call does no work, and a call's counts are those of the
    // same request through the CLI.
    let run = mcp::run(corpus.root(), &[]);
    assert_eq!(run.work, WorkCounts::default(), "mara mcp at {size}×");
    for case in mcp::cases(&corpus) {
        let name = format!("mara mcp {} {} at {size}×", case.tool, case.arguments);
        let run = || mcp::run(corpus.root(), &[(case.tool, &case.arguments)]);
        check_cold(
            &corpus,
            &name,
            (case.reads, case.code),
            totals,
            &mut last_write,
            run,
        );
    }
}

/// The documents of a corpus, and its code files with their distinct contents.
#[derive(Clone, Copy)]
struct Totals {
    documents: u64,
    code: (u64, u64),
}

/// Runs a case once and requires the counts of a cold request that reads `reads` and `code`,
/// with `totals` in the corpus. A case that writes starts when every file it writes last
/// changed at least 2 s before, and its files are restored after it.
fn check_cold(
    corpus: &Corpus,
    name: &str,
    (reads, code): (Reads, Code),
    totals: Totals,
    last_write: &mut SystemTime,
    run: impl FnOnce() -> cli::Run,
) {
    let documents = totals.documents;
    let mut written = None;
    let run = match reads {
        Reads::Writes => {
            restore::settle(*last_write);
            let mut ran = None;
            written = Some(Written::learn(corpus.root(), || ran = Some(run())));
            ran.expect("the case ran")
        }
        _ => run(),
    };
    // The configuration and the schema; for a request that reads documents, also the root
    // `.gitignore`, every document and every program file. `.git`, ignored and undeclared files
    // are never read. Code is read only by the operations that read it (DES-CODE-READERS).
    // Validation runs the programs DES-RULE-EXECUTION selects, each once, as one subprocess.
    let read = documents + 3 + PROGRAMS;
    let mut expected = match reads {
        Reads::Schema => WorkCounts {
            file_reads: 2,
            hashes: 2,
            ..WorkCounts::default()
        },
        Reads::Checked { runs } => WorkCounts {
            file_reads: read,
            hashes: read,
            parses: documents - 1,
            snapshot_builds: 1,
            check_steps: run.work.check_steps,
            subprocess_spawns: runs,
            program_runs: runs,
            ..WorkCounts::default()
        },
        Reads::Documents => WorkCounts {
            file_reads: read,
            hashes: read,
            parses: documents - 1,
            snapshot_builds: 1,
            ..WorkCounts::default()
        },
        // `query execute`, `query view`, `query gate` and `query report`, whose one run is in
        // provenance mode: 1 program run, as 1 subprocess, and a read of each file that
        // `--input` names, which is no input of the project and is not hashed.
        Reads::Evaluates { input_files } => WorkCounts {
            file_reads: read + input_files,
            hashes: read,
            parses: documents - 1,
            snapshot_builds: 1,
            subprocess_spawns: 1,
            program_runs: 1,
            ..WorkCounts::default()
        },
        // A cold request, then 1 parse per distinct content written, before the renames, and
        // nothing after them (DES-WRITE-PROCEDURE, ADR-READ-BACK-EVERY-EDIT).
        Reads::Writes => WorkCounts {
            file_reads: read,
            hashes: read,
            parses: documents - 1 + written.as_ref().map_or(0, Written::distinct_documents),
            snapshot_builds: 1,
            ..WorkCounts::default()
        },
    };
    let (code_reads, code_scans) = match code {
        Code::None => (0, 0),
        Code::All => totals.code,
        Code::One => (1, 0),
    };
    expected.file_reads += code_reads;
    expected.hashes += code_reads;
    expected.code_scans = code_scans;
    assert_eq!(run.work, expected, "{name}");
    if let Some(written) = written {
        assert!(!written.contents.is_empty(), "{name} wrote nothing");
        written.restore();
        *last_write = SystemTime::now();
    }
    // Built-in checks visit each item twice and each edge once, and the items and edges of
    // acyclic relations once more (REQ-NEAR-LINEAR-SCALING).
    if matches!(reads, Reads::Checked { .. }) {
        let steps = run.work.check_steps;
        let (items, edges) = (corpus.items as u64, corpus.edges as u64);
        assert!(
            (2 * items + edges..=3 * items + 2 * edges).contains(&steps),
            "{name}: {steps} check steps for {items} items and {edges} edges"
        );
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
        // Every code file is read, hashed and scanned once.
        let code = hub.code_files as u64;
        let expected = WorkCounts {
            file_reads: documents + 2 + code,
            hashes: documents + 2 + code,
            parses: documents,
            snapshot_builds: 1,
            code_scans: code,
            // The facts and item passes, the edge pass, and the cycle pass over every edge and
            // item of the acyclic `refines`.
            check_steps: 2 * items + edges + (items + edges),
            ..WorkCounts::default()
        };
        assert_eq!(run.work, expected, "hub at {size}×");
        let run = cli::run(hub.root(), &hub_relations(), 0);
        let listed = WorkCounts {
            check_steps: 0,
            ..expected
        };
        assert_eq!(run.work, listed, "relation list of the hub at {size}×");
        // A rename and a move of the hub rewrite every edge's link, and the rename every marker
        // (DES-PERFORMANCE-HARNESS).
        let mut last_write = generated;
        for args in hub_writes() {
            restore::settle(last_write);
            let mut run = None;
            let written = Written::learn(hub.root(), || run = Some(cli::run(hub.root(), &args, 0)));
            // The rename reads and rewrites code; the move reads none. No code is scanned after
            // the edit.
            let code = match args[1] {
                "rename" => code,
                _ => 0,
            };
            let expected = WorkCounts {
                file_reads: documents + 2 + code,
                hashes: documents + 2 + code,
                parses: documents + written.distinct_documents(),
                snapshot_builds: 1,
                code_scans: code,
                ..WorkCounts::default()
            };
            assert_eq!(written.code_files(), code, "{args:?} at {size}×");
            assert_eq!(
                written.contents.len() as u64,
                documents + u64::from(args[1] == "move") + code
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

/// `relation list` of the hub item: every source's relation and every marker.
pub fn hub_relations() -> [&'static str; 3] {
    ["relation", "list", "REQ-HUB"]
}

/// The rename and the move of the hub item.
pub fn hub_writes() -> [[&'static str; 4]; 2] {
    [
        ["item", "rename", "REQ-HUB", "REQ-HUB-RENAMED"],
        ["item", "move", "REQ-HUB", "docs/moved/hub.mara.md"],
    ]
}

/// `project fix` after the hub item is moved by hand rewrites every edge's link: a cold request
/// and 1 parse per distinct content written, and nothing more (DES-PERFORMANCE-HARNESS). A second
/// fix finds nothing to write.
#[test]
fn moved_hub_fix_counts() {
    let hubs: Vec<(usize, Hub)> = PULL_REQUEST_SIZES
        .iter()
        .map(|&size| (size, Hub::generate(size)))
        .collect();
    for (_, hub) in &hubs {
        hub.move_hub();
    }
    // Every file the fix writes last changed at least 2 s before.
    restore::settle(SystemTime::now());
    for (size, hub) in &hubs {
        let documents = hub.edges.div_ceil(100) as u64 + 1;
        let cold = WorkCounts {
            file_reads: documents + 2,
            hashes: documents + 2,
            parses: documents,
            snapshot_builds: 1,
            ..WorkCounts::default()
        };
        let mut run = None;
        let fix = ["project", "fix"];
        let written = Written::learn(hub.root(), || run = Some(cli::run(hub.root(), &fix, 0)));
        assert_eq!(written.contents.len() as u64, documents - 1, "at {size}×");
        let expected = WorkCounts {
            parses: documents + written.distinct_documents(),
            ..cold
        };
        let run = run.expect("the case ran");
        assert_eq!(run.work, expected, "fix of the moved hub at {size}×");
        let run = cli::run(hub.root(), &fix, 0);
        assert_eq!(run.work, cold, "fix with nothing to write at {size}×");
        let run = cli::run(hub.root(), &["project", "validate"], 0);
        assert!(
            run.output.stdout.starts_with(b"valid"),
            "after the fix at {size}×"
        );
    }
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

    // Cold: the configuration, the schema, the root `.gitignore`, each declared document and
    // each program file are read and hashed once; `.git`, ignored and undeclared files never.
    let documents = corpus.documents as u64;
    let (cold, work) = request(&mut inputs);
    let read = documents + 3 + PROGRAMS;
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
