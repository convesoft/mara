//! `project init`: its row of VER-WORK-COUNTERS and its VER-SCALING case. It reads nothing, so
//! each run, at each corpus size, initializes a new directory outside the corpus from the
//! corpus root, and no run has anything to restore.

use mara_engine::work::WorkCounts;

use crate::corpus::Corpus;
use crate::{PULL_REQUEST_SIZES, SEED, cli, scaling};

/// A directory under `$TMPDIR` whose subdirectories each run initializes.
fn targets() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("mara-init-runs-")
        .tempdir()
        .expect("create a temporary directory")
}

#[test]
fn project_init_counts() {
    let targets = targets();
    for size in PULL_REQUEST_SIZES {
        let corpus = Corpus::generate(size, SEED);
        for template in ["engineering", "empty"] {
            let target = targets.path().join(format!("{template}-{size}"));
            let target = target.to_str().expect("a UTF-8 temporary directory");
            let args = [
                "project",
                "init",
                "--template",
                template,
                "--project",
                target,
            ];
            let run = cli::run(corpus.root(), &args, 0);
            // 0 file reads, 0 hashes, 0 parses, 0 snapshot builds, and no other work.
            assert_eq!(
                run.work,
                WorkCounts::default(),
                "mara project init --template {template} at {size}×"
            );
        }
    }
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "times a release build: cargo test --release --package mara --test performance"
)]
fn project_init_growth_per_doubling() {
    if cfg!(debug_assertions) {
        panic!("VER-SCALING times a release build");
    }
    let corpora: Vec<(usize, Corpus)> = PULL_REQUEST_SIZES
        .iter()
        .map(|&size| (size, Corpus::generate(size, SEED)))
        .collect();
    let targets = targets();
    let mut runs = 0;
    scaling::check_growth(&corpora, "mara project init", |_, corpus| {
        runs += 1;
        let target = targets.path().join(runs.to_string());
        let target = target.to_str().expect("a UTF-8 temporary directory");
        cli::run(corpus.root(), &["project", "init", "--project", target], 0);
    });
}
