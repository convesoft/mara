//! VER-SCALING: growth per doubling of each operation's time on a release build.
//!
//! Each operation adds its arguments and exit status to `growth_per_doubling`.

use std::time::{Duration, Instant, SystemTime};

use mara::inputs::Inputs;
use mara_engine::work::WorkCounts;

use crate::corpus::Corpus;
use crate::{PULL_REQUEST_SIZES, SEED, cli};

/// Most growth of time per doubling of corpus size (REQ-NEAR-LINEAR-SCALING).
const LIMIT: f64 = 2.3;
/// Timed rounds after one warm-up round; each round runs every size once.
const ROUNDS: usize = 5;

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "times a release build: cargo test --release --package mara --test performance"
)]
fn growth_per_doubling() {
    if cfg!(debug_assertions) {
        panic!("VER-SCALING times a release build");
    }
    let corpora: Vec<(usize, Corpus)> = PULL_REQUEST_SIZES
        .iter()
        .map(|&size| (size, Corpus::generate(size, SEED)))
        .collect();
    // Without an operation: the CLI's start-up does not grow with the corpus.
    check_growth(&corpora, "mara", |corpus| {
        cli::run(corpus.root(), &[], 0);
    });
    // Below the CLI, the part of every operation that grows with the project: the inventory, a
    // cold read, hash and parse of every input, and the snapshot build.
    check_growth(&corpora, "a cold snapshot build", |corpus| {
        let mut inputs = Inputs::new(corpus.root().to_owned());
        inputs.snapshot(SystemTime::now(), &mut WorkCounts::default());
    });
}

/// Times `run`, named `name`, on every corpus. Interleaves sizes so that machine load affects
/// them alike, and fails when the median grows faster than [`LIMIT`] per doubling.
fn check_growth(corpora: &[(usize, Corpus)], name: &str, mut run: impl FnMut(&Corpus)) {
    let mut times: Vec<Vec<Duration>> = vec![Vec::with_capacity(ROUNDS); corpora.len()];
    for round in 0..=ROUNDS {
        for ((_, corpus), times) in corpora.iter().zip(&mut times) {
            let start = Instant::now();
            run(corpus);
            if round > 0 {
                times.push(start.elapsed());
            }
        }
    }
    let medians: Vec<(usize, Duration)> = corpora
        .iter()
        .zip(&mut times)
        .map(|((size, _), times)| {
            times.sort();
            (*size, times[times.len() / 2])
        })
        .collect();
    for pair in medians.windows(2) {
        let ((small, small_time), (large, large_time)) = (pair[0], pair[1]);
        let doublings = (large as f64 / small as f64).log2();
        let growth = (large_time.as_secs_f64() / small_time.as_secs_f64()).powf(1.0 / doublings);
        assert!(
            growth <= LIMIT,
            "{name} grows {growth:.2}× per doubling from {small}× to {large}×; medians {medians:?}"
        );
    }
}
