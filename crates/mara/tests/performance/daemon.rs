//! `daemon status | start | stop`: their row of VER-WORK-COUNTERS and their VER-SCALING case.
//! They do no project work. Each run has a runtime directory of the test, so it reaches no
//! daemon of the user's (VER-DAEMON-START); the test kills its daemons also when it fails.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use mara_engine::work::WorkCounts;

use crate::corpus::Corpus;
use crate::daemons::Daemons;
use crate::{PULL_REQUEST_SIZES, SEED, cli, scaling};

/// A runtime directory of the test, mode 0700, whose daemons are killed when it is dropped.
struct Runtime {
    daemons: Daemons,
    dir: tempfile::TempDir,
}

impl Runtime {
    fn new() -> Runtime {
        // Short enough for the socket path.
        let dir = tempfile::Builder::new()
            .prefix("mara-run-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()
            .expect("create a temporary directory");
        Runtime {
            daemons: Daemons::new(dir.path(), None),
            dir,
        }
    }

    /// `mara <args>` in `directory` with this runtime directory: the work it reports.
    fn run(&self, directory: &Path, args: &[&str]) -> WorkCounts {
        let env = [
            ("XDG_RUNTIME_DIR", self.dir.path()),
            ("TMPDIR", self.dir.path()),
        ];
        cli::run_with(directory, args, &env, 0).work
    }
}

/// The operations in the order a test runs them: each with and without a daemon.
const CYCLE: [&[&str]; 5] = [
    &["daemon", "status"],
    &["daemon", "start"],
    &["daemon", "status"],
    &["daemon", "start"],
    &["daemon", "stop"],
];

#[test]
fn daemon_counts() {
    let runtime = Runtime::new();
    for size in PULL_REQUEST_SIZES {
        let corpus = Corpus::generate(size, SEED);
        for args in CYCLE.iter().chain([&&["daemon", "stop"][..]]) {
            // Nothing read, hashed, parsed or built, and no other work.
            let work = runtime.run(corpus.root(), args);
            assert_eq!(work, WorkCounts::default(), "mara {args:?} at {size}×");
        }
        assert!(
            runtime.daemons.running().is_empty(),
            "a daemon runs after `stop`"
        );
    }
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "times a release build: cargo test --release --package mara --test performance"
)]
fn daemon_growth_per_doubling() {
    if cfg!(debug_assertions) {
        panic!("VER-SCALING times a release build");
    }
    let runtime = Runtime::new();
    let corpora: Vec<(usize, Corpus)> = PULL_REQUEST_SIZES
        .iter()
        .map(|&size| (size, Corpus::generate(size, SEED)))
        .collect();
    scaling::check_growth(
        &corpora,
        "mara daemon status, start and stop",
        |_, corpus| {
            for args in CYCLE {
                runtime.run(corpus.root(), args);
            }
        },
    );
}
