//! Work-counter and scaling tests (VER-WORK-COUNTERS and VER-SCALING in
//! `docs/verification.mara.md`) of the real `mara` binary on generated corpora
//! (DES-PERFORMANCE-HARNESS in `docs/performance.mara.md`).

mod cli;
mod convert;
mod corpus;
mod cpu_time;
mod init;
mod mcp;
mod pause_points;
mod prime;
mod restore;
mod scaling;
mod work_counters;

/// The seed of every generated corpus.
const SEED: u64 = 0x4D41_5241;
/// Corpus sizes run on every pull request.
const PULL_REQUEST_SIZES: [usize; 3] = [1, 4, 16];
/// Corpus size run nightly.
const NIGHTLY_SIZE: usize = 64;
