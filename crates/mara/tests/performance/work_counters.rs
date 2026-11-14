//! VER-WORK-COUNTERS: exact work counts of cold requests through the real CLI.
//!
//! Each operation adds its case to `cold_counts_at`, with its exit status. The warm rows run
//! against the daemon (0.4.0-alpha.4).

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
