//! `prime`: its row of VER-WORK-COUNTERS and its VER-SCALING case. It reads no file, inside a
//! project or outside one.

use mara_engine::work::WorkCounts;

use crate::corpus::Corpus;
use crate::{PULL_REQUEST_SIZES, SEED, cli, scaling};

#[test]
fn prime_counts() {
    let outside = tempfile::Builder::new()
        .prefix("mara-prime-runs-")
        .tempdir()
        .expect("create a temporary directory");
    let outside = outside
        .path()
        .to_str()
        .expect("a UTF-8 temporary directory");
    for size in PULL_REQUEST_SIZES {
        let corpus = Corpus::generate(size, SEED);
        for args in [&["prime"][..], &["prime", "--project", outside]] {
            let run = cli::run(corpus.root(), args, 0);
            // 0 file reads, 0 hashes, 0 parses, 0 snapshot builds, and no other work.
            assert_eq!(run.work, WorkCounts::default(), "mara {args:?} at {size}×");
        }
    }
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "times a release build: cargo test --release --package mara --test performance"
)]
fn prime_growth_per_doubling() {
    if cfg!(debug_assertions) {
        panic!("VER-SCALING times a release build");
    }
    let corpora: Vec<(usize, Corpus)> = PULL_REQUEST_SIZES
        .iter()
        .map(|&size| (size, Corpus::generate(size, SEED)))
        .collect();
    scaling::check_growth(&corpora, "mara prime", |_, corpus| {
        cli::run(corpus.root(), &["prime"], 0);
    });
}
