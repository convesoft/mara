//! VER-SCALING: growth per doubling of each operation's CPU time on a release build.
//!
//! Each operation adds its case to `cli::cases`, which `growth_per_doubling` times.

use std::f64::consts::FRAC_PI_2;
use std::time::{Duration, SystemTime};

use mara::inputs::Inputs;
use mara_engine::work::WorkCounts;

use crate::corpus::{Corpus, Hub, Shape, Stress};
use crate::{PULL_REQUEST_SIZES, SEED, cli, cpu_time};

/// Most growth of time per doubling of corpus size (REQ-NEAR-LINEAR-SCALING).
const LIMIT: f64 = 2.3;
/// Timed rounds after one warm-up round; each round runs every size once.
const ROUNDS: usize = 5;
/// Standard errors by which two adjacent sizes must grow less than [`LIMIT`] after the rounds to
/// pass without more runs.
const MARGIN: f64 = 4.0;
/// Timed runs of each size of a pair that does not pass after the rounds.
const RUNS: usize = 60;

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
    // Search with a vocabulary and a query that grow together: query words allowed one edit,
    // two, and none beyond 18 characters, and a mix. Candidates come from the vocabulary index,
    // never from a scan of every word per query word. At 16× each query stays within the 128 KiB
    // that one argument may take.
    for (words, lengths) in VOCABULARIES {
        let vocabularies: Vec<(usize, Vocabulary)> = PULL_REQUEST_SIZES
            .iter()
            .map(|&size| (size, Vocabulary::generate(words * size, lengths.clone())))
            .collect();
        let name = format!("mara search <every word of {lengths:?} letters>");
        check_growth(&vocabularies, &name, |_, vocabulary| {
            let args = ["search", vocabulary.query.as_str(), "--format", "json"];
            cli::run(vocabulary.dir.path(), &args, 0);
        });
    }
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

/// Times `run`, named `name`, on every corpus, given with its index, in CPU time: the calling
/// thread's and that of the processes [`cli::run`] waits for ([`cpu_time::now`]). Interleaves
/// sizes so that machine load affects them alike. After the rounds, looks once: two adjacent
/// sizes that grow more than [`MARGIN`] standard errors below [`LIMIT`] pass; every other pair
/// gets [`RUNS`] runs of both sizes and fails when the median grows faster than [`LIMIT`] per
/// doubling.
pub fn check_growth<P>(corpora: &[(usize, P)], name: &str, mut run: impl FnMut(usize, &P)) {
    let mut time = |at: usize| {
        let start = cpu_time::now();
        run(at, &corpora[at].1);
        cpu_time::now() - start
    };
    let mut times: Vec<Vec<Duration>> = vec![Vec::with_capacity(RUNS); corpora.len()];
    for round in 0..=ROUNDS {
        for (at, times) in times.iter_mut().enumerate() {
            let elapsed = time(at);
            if round > 0 {
                times.push(elapsed);
            }
        }
    }
    let judged: Vec<usize> = (1..corpora.len())
        .filter(|&at| !clearly_below(corpora, &times, at))
        .collect();
    let mut extended: Vec<usize> = judged.iter().flat_map(|&at| [at - 1, at]).collect();
    extended.dedup();
    for _ in ROUNDS..RUNS {
        for &at in &extended {
            let elapsed = time(at);
            times[at].push(elapsed);
        }
    }
    let medians: Vec<(usize, Duration, usize)> = corpora
        .iter()
        .zip(&times)
        .map(|((size, _), times)| (*size, median(times), times.len()))
        .collect();
    for at in judged {
        let (small, large) = (corpora[at - 1].0, corpora[at].0);
        let ratio = median(&times[at]).as_secs_f64() / median(&times[at - 1]).as_secs_f64();
        let growth = ratio.powf(1.0 / doublings(corpora, at));
        assert!(
            growth <= LIMIT,
            "{name} grows {growth:.2}× per doubling from {small}× to {large}×; median CPU time \
             and timed runs per size {medians:?}"
        );
    }
}

/// Whether the median time grows more than [`MARGIN`] standard errors below [`LIMIT`] from the
/// size before the one at `at` to that size. The standard error of a median's logarithm is
/// √(π/2) times that of a mean, from the standard deviation of the runs' logarithms around their
/// size's mean, pooled over the two sizes.
fn clearly_below<P>(corpora: &[(usize, P)], times: &[Vec<Duration>], at: usize) -> bool {
    let logs = |times: &[Duration]| -> Vec<f64> {
        times.iter().map(|time| time.as_secs_f64().ln()).collect()
    };
    let (small, large) = (logs(&times[at - 1]), logs(&times[at]));
    let squares = |logs: &[f64]| {
        let mean = logs.iter().sum::<f64>() / logs.len() as f64;
        logs.iter().map(|log| (log - mean).powi(2)).sum::<f64>()
    };
    let (n_small, n_large) = (small.len() as f64, large.len() as f64);
    let deviation = ((squares(&small) + squares(&large)) / (n_small + n_large - 2.0)).sqrt();
    let doublings = doublings(corpora, at);
    let error = FRAC_PI_2.sqrt() * deviation * (1.0 / n_small + 1.0 / n_large).sqrt() / doublings;
    let growth = (middle(large) - middle(small)) / doublings;
    growth + MARGIN * error < LIMIT.ln()
}

fn doublings<P>(corpora: &[(usize, P)], at: usize) -> f64 {
    (corpora[at].0 as f64 / corpora[at - 1].0 as f64).log2()
}

fn median(times: &[Duration]) -> Duration {
    let mut sorted = times.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

fn middle(mut values: Vec<f64>) -> f64 {
    values.sort_unstable_by(f64::total_cmp);
    values[values.len() / 2]
}

/// Distinct words at 1× and their lengths, of each vocabulary project.
const VOCABULARIES: [(usize, std::ops::RangeInclusive<usize>); 6] = [
    (500, 7..=7),
    (500, 9..=9),
    (300, 19..=19),
    (200, 32..=32),
    (30, 200..=200),
    (400, 3..=30),
];

/// A project whose one narrative unit holds distinct words of the given lengths, and a query of
/// all of them.
struct Vocabulary {
    dir: tempfile::TempDir,
    query: String,
}

impl Vocabulary {
    fn generate(count: usize, lengths: std::ops::RangeInclusive<usize>) -> Vocabulary {
        let dir = tempfile::Builder::new()
            .prefix("mara-vocabulary-")
            .tempdir()
            .expect("create a temporary directory");
        // SplitMix64, as the corpus generator draws.
        let mut state = SEED;
        let mut next = || {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        let mut words = std::collections::BTreeSet::new();
        while words.len() < count {
            let span = (lengths.end() - lengths.start() + 1) as u64;
            let length = lengths.start() + (next() % span) as usize;
            let word: String = (0..length)
                .map(|_| char::from(b'a' + (next() % 26) as u8))
                .collect();
            words.insert(word);
        }
        let words: Vec<String> = words.into_iter().collect();
        let mut text = String::from("# Words\n\n");
        for line in words.chunks(10) {
            text.push_str(&line.join(" "));
            text.push('\n');
        }
        let write = |path: &str, contents: &str| {
            let path = dir.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        };
        write(
            ".mara/config.toml",
            "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\nformat_version = 1\n\
             include = [\"docs/**/*.mara.md\"]\n",
        );
        write(
            ".mara/schema.toml",
            "format_version = 1\nstatuses = [\"draft\"]\n\n[types.note]\nid_prefix = \"NOTE-\"\n\
             description = \"A note.\"\n",
        );
        write("docs/words.mara.md", &text);
        Vocabulary {
            dir,
            query: words.join(" "),
        }
    }
}
