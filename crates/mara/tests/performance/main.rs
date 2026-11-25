//! Work-counter and scaling tests (VER-WORK-COUNTERS and VER-SCALING in
//! `docs/verification.mara.md`) of the real `mara` binary on generated corpora
//! (DES-PERFORMANCE-HARNESS in `docs/performance.mara.md`).

mod cli;
mod code_add;
mod code_get;
mod convert;
mod corpus;
mod cpu_time;
// The daemon's cases find the daemons of a test in `/proc`.
#[cfg(target_os = "linux")]
mod daemon;
#[cfg(target_os = "linux")]
#[path = "../support/daemons.rs"]
mod daemons;
#[cfg(all(target_os = "linux", debug_assertions))]
#[path = "../support/derived.rs"]
mod derived;
#[path = "../support/grammar_server.rs"]
mod grammar_server;
#[cfg(target_os = "linux")]
mod idle_period;
mod init;
#[path = "../support/legacy_project.rs"]
mod legacy_project;
mod mcp;
mod pause_points;
mod prime;
mod restore;
mod scaling;
#[cfg(target_os = "linux")]
mod warm;
#[cfg(target_os = "linux")]
mod warm_code;
#[cfg(target_os = "linux")]
mod warm_scaling;
mod work_counters;

/// The seed of every generated corpus.
const SEED: u64 = 0x4D41_5241;
/// Corpus sizes run on every pull request.
const PULL_REQUEST_SIZES: [usize; 3] = [1, 4, 16];
/// Corpus size run nightly.
const NIGHTLY_SIZE: usize = 64;
