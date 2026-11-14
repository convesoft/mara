//! VER-SCALING: growth per doubling of each operation's time on a release build.
//!
//! Each operation adds its case to `cli::cases`, which `growth_per_doubling` times.

use std::time::{Duration, Instant, SystemTime};

use mara::inputs::Inputs;
use mara_engine::work::WorkCounts;

use crate::corpus::{Corpus, Hub, Shape, Stress};
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
    check_growth(&corpora, "mara", |_, corpus| {
        cli::run(corpus.root(), &[] as &[&str], 0);
    });
    // Each operation, cold, through the real CLI. Validation times the parse of every document
    // too, which has no case of its own.
    let cases: Vec<Vec<cli::Case>> = corpora
        .iter()
        .map(|(_, corpus)| cli::cases(corpus))
        .collect();
    for index in 0..cases[0].len() {
        let name = format!("mara {}", cases[0][index].args.join(" "));
        check_growth(&corpora, &name, |at, corpus| {
            let case = &cases[at][index];
            cli::run(corpus.root(), &case.args, case.status);
        });
    }
    // Late metadata and many incoming edges: every check reads an item's `Type` and `ID` once.
    let hubs: Vec<(usize, Hub)> = PULL_REQUEST_SIZES
        .iter()
        .map(|&size| (size, Hub::generate(size)))
        .collect();
    check_growth(&hubs, "mara project validate on a hub", |_, hub| {
        cli::run(hub.root(), &["project", "validate"], 0);
    });
    // Below the CLI, the part of every operation that grows with the project: the inventory, a
    // cold read, hash and parse of every input, and the snapshot build.
    check_growth(&corpora, "a cold snapshot build", |_, corpus| {
        let mut inputs = Inputs::new(corpus.root().to_owned());
        inputs.snapshot(SystemTime::now(), &mut WorkCounts::default());
    });
}

/// Work per row, holder or finding, which the item-and-edge check steps do not measure, so
/// that no change makes it grow faster than the project (DES-PERFORMANCE-HARNESS).
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "times a release build: cargo test --release --package mara --test performance"
)]
fn per_row_growth_per_doubling() {
    if cfg!(debug_assertions) {
        panic!("VER-SCALING times a release build");
    }
    for shape in Shape::ALL {
        let projects: Vec<(usize, Stress)> = PULL_REQUEST_SIZES
            .iter()
            .map(|&size| (size, Stress::generate(shape, size)))
            .collect();
        let name = format!("mara project validate on {shape:?}");
        check_growth(&projects, &name, |_, project| {
            let args = ["project", "validate", "--limit", "1"];
            cli::run(project.root(), &args, shape.status());
        });
    }
}

/// Times `run`, named `name`, on every corpus, given with its index. Interleaves sizes so that
/// machine load affects them alike, and fails when the median grows faster than [`LIMIT`] per
/// doubling.
fn check_growth<P>(corpora: &[(usize, P)], name: &str, mut run: impl FnMut(usize, &P)) {
    let mut times: Vec<Vec<Duration>> = vec![Vec::with_capacity(ROUNDS); corpora.len()];
    for round in 0..=ROUNDS {
        for (at, ((_, corpus), times)) in corpora.iter().zip(&mut times).enumerate() {
            let start = Instant::now();
            run(at, corpus);
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
