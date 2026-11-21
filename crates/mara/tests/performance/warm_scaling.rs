//! The daemon cases of VER-SCALING (`docs/verification.mara.md`), timed as DES-PERFORMANCE-HARNESS
//! says: `search`, `item list` and `project validate`, each repeated on settled inputs, and
//! `search` and `item list` after one document edit, sent to one daemon the test starts with its
//! own runtime directory. A run's time adds the daemon's CPU time, from a reading taken while it
//! is idle, before the edit, to one taken once its CPU time has stopped advancing for 100 ms
//! longer than the watcher's debounce after the response, so re-derivation counts in the run
//! that needed it, whoever did it. Each run asserts the warm counts of its row of
//! VER-WORK-COUNTERS, so no run that reruns a program, whose `souffle` the daemon's clock would
//! miss, is timed, and the daemon's count of served requests rises by one with each, so no run
//! served in-process is timed as the daemon's.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use mara_engine::work::WorkCounts;

use crate::corpus::Corpus;
use crate::daemons::Daemons;
use crate::{PULL_REQUEST_SIZES, SEED, cli, cpu_time, scaling};

/// How long the daemon's CPU time stays the same before it counts as idle.
const QUIET: Duration = mara::watch::DEBOUNCE.saturating_add(Duration::from_millis(100));
/// How long the daemon may take to fall idle.
const IDLE_WAIT: Duration = Duration::from_secs(30);

/// A daemon serving the corpora, in a runtime directory of the test.
struct Daemon {
    daemons: Daemons,
    runtime: tempfile::TempDir,
    pid: u32,
    /// The count of served requests before the timed run, outside the timed span.
    served: Cell<Option<u64>>,
}

impl Daemon {
    fn start() -> Daemon {
        // Short enough for the socket path; mode 0700, as DES-DAEMON-LIFECYCLE requires.
        let runtime = tempfile::Builder::new()
            .prefix("mara-scale-")
            .permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .tempdir()
            .expect("create a temporary directory");
        let daemons = Daemons::new(runtime.path(), None);
        let env = [
            ("XDG_RUNTIME_DIR", runtime.path()),
            ("TMPDIR", runtime.path()),
        ];
        cli::run_with(runtime.path(), &["daemon", "start"], &env, 0);
        let running = daemons.running();
        assert_eq!(running.len(), 1, "daemons of the test: {running:?}");
        let pid = running[0];
        Daemon {
            daemons,
            runtime,
            pid,
            served: Cell::new(None),
        }
    }

    /// `mara <args>` in `root`, sent to the daemon: the work it reports.
    fn run(&self, root: &Path, args: &[&str]) -> WorkCounts {
        let env = [
            ("XDG_RUNTIME_DIR", self.runtime.path()),
            ("TMPDIR", self.runtime.path()),
        ];
        cli::run_with(root, args, &env, 0).work
    }

    /// How many operation requests the daemon has answered, as `mara daemon status` reports;
    /// its CPU time counts in no run.
    fn count_served(&self) -> u64 {
        let output = Command::new(env!("CARGO_BIN_EXE_mara"))
            .args(["--format", "json", "daemon", "status"])
            .current_dir(self.runtime.path())
            .env_clear()
            .env("XDG_RUNTIME_DIR", self.runtime.path())
            .env("TMPDIR", self.runtime.path())
            .stdin(Stdio::null())
            .output()
            .expect("run mara daemon status");
        assert!(output.status.success(), "{output:?}");
        let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        status["served"]
            .as_u64()
            .expect("a count of served requests")
    }

    /// Outside the timed span: the daemon served one request since the count was last taken,
    /// if it was, and the count is taken now.
    fn served_one(&self) {
        let now = self.count_served();
        if let Some(before) = self.served.get() {
            assert_eq!(
                now,
                before + 1,
                "a timed request was not served by the daemon"
            );
        }
        self.served.set(Some(now));
    }

    /// Waits until the daemon's CPU time has stood still for [`QUIET`]: that time.
    fn idle(&self) -> Duration {
        let deadline = Instant::now() + IDLE_WAIT;
        let mut last = cpu_time::of_process(self.pid);
        let mut since = Instant::now();
        loop {
            thread::sleep(Duration::from_millis(10));
            let now = cpu_time::of_process(self.pid);
            if now != last {
                (last, since) = (now, Instant::now());
            } else if since.elapsed() >= QUIET {
                return now;
            }
            assert!(Instant::now() < deadline, "the daemon does not fall idle");
        }
    }

    /// Runs `step` and charges the daemon's CPU time meanwhile to the timed run, from a reading
    /// while it is idle to one once it is idle again.
    fn timed(&self, step: impl FnOnce()) {
        let before = cpu_time::of_process(self.pid);
        step();
        cpu_time::charge(self.idle() - before);
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.daemons.kill();
    }
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "times a release build: cargo test --release --package mara --test performance"
)]
fn warm_daemon_growth_per_doubling() {
    if cfg!(debug_assertions) {
        panic!("VER-SCALING times a release build");
    }
    let corpora: Vec<(usize, Corpus)> = PULL_REQUEST_SIZES
        .iter()
        .map(|&size| (size, Corpus::generate(size, SEED)))
        .collect();
    // Every file settled before the first request, which opens each session.
    crate::restore::settle(std::time::SystemTime::now());
    let daemon = Daemon::start();
    let reads: [&[&str]; 3] = [
        &["search", "cache"],
        &["item", "list"],
        &["project", "validate"],
    ];
    for args in reads {
        let name = format!("mara {} again, through the daemon", args.join(" "));
        // Once on each corpus first, so that every run is the same request again.
        for (_, corpus) in &corpora {
            daemon.run(corpus.root(), args);
        }
        scaling::check_growth_after(
            &corpora,
            &name,
            |_| {
                daemon.idle();
                daemon.served_one();
            },
            |_, corpus| {
                daemon.timed(|| {
                    // The same read request again, inputs unchanged.
                    let work = daemon.run(corpus.root(), args);
                    assert_eq!(work, WorkCounts::default(), "{name}");
                });
            },
        );
        daemon.served_one();
        daemon.served.set(None);
    }
    // The same document of the first cluster in every corpus, edited with new contents in
    // every run, so that each run parses it; restored before the next, outside the timed span.
    let documents: Vec<(PathBuf, String)> = (corpora.iter())
        .map(|(_, corpus)| {
            let path = corpus.root().join(&corpus.ids[0].1);
            let contents = fs::read_to_string(&path).unwrap();
            (path, contents)
        })
        .collect();
    let edits = Cell::new(0_u64);
    for args in &reads[..2] {
        let name = format!("mara {} after an edit, through the daemon", args.join(" "));
        scaling::check_growth_after(
            &corpora,
            &name,
            |at| {
                daemon.served_one();
                let (path, contents) = &documents[at];
                fs::write(path, contents).unwrap();
                daemon.run(corpora[at].1.root(), args);
                daemon.idle();
                daemon.served.set(Some(daemon.count_served()));
            },
            |at, corpus| {
                daemon.timed(|| {
                    edits.set(edits.get() + 1);
                    let (path, contents) = &documents[at];
                    let edited = format!("{contents}\nEdited in run {}.\n", edits.get());
                    fs::write(path, edited).unwrap();
                    let work = daemon.run(corpus.root(), args);
                    // One document edited: read and hashed by the request, and once more by the
                    // watcher's derivation when it checked the edit first; the release build
                    // reports no derivation that would tell which.
                    let reads = work.file_reads;
                    assert!((1..=2).contains(&reads), "{name}: {work:?}");
                    let expected = WorkCounts {
                        file_reads: reads,
                        hashes: reads,
                        parses: 1,
                        snapshot_builds: 1,
                        ..WorkCounts::default()
                    };
                    assert_eq!(work, expected, "{name}");
                });
            },
        );
        daemon.served_one();
        daemon.served.set(None);
    }
    for (path, contents) in &documents {
        fs::write(path, contents).unwrap();
    }
}
