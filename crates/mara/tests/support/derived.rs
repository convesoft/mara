//! The reports of a debug daemon's background derivations (`MARA_TEST_DERIVED`, ADR-DEBUG-PAUSE-POINT
//! in `docs/authoring/writes.mara.md`): each derivation's counts and the time its check took as
//! now. A test of the warm rows of VER-WORK-COUNTERS waits for the watcher through them instead
//! of a deadline, and learns from them which case a request is in ([`Case`]); its expected
//! counts are the row's for that case, never taken from a report.
//!
//! A derivation's work goes to the next response of its session. The derivations whose work a
//! response holds are those reported before its request was sent ([`Reports::around`]). One
//! reported while the request ran may or may not be in the response: when it did work, the run
//! cannot be judged and is repeated, at most [`ATTEMPTS`] times ([`decided`]).

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

/// How long a test waits for the watcher's derivation after a change.
const WAIT: Duration = Duration::from_secs(10);
/// How long after a file's last change a check settles it (DES-INPUTS-AND-KEYS).
const SETTLE: Duration = Duration::from_secs(2);
/// Runs of a case at most, while a run cannot be judged.
const ATTEMPTS: usize = 3;

/// One background derivation, as the daemon reported it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Derived {
    pub file_reads: u64,
    pub hashes: u64,
    pub parses: u64,
    pub snapshot_builds: u64,
    /// The time its check took as now.
    pub checked: SystemTime,
}

impl Derived {
    fn worked(&self) -> bool {
        self.file_reads + self.hashes + self.parses + self.snapshot_builds > 0
    }
}

/// Which derivation, if any, checked a change before a request: what sets the request's row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// None did: the request's check is the first after the change.
    None,
    /// One checked less than 2 s after the change, which leaves what it read unsettled.
    Early(Derived),
    /// One checked 2 s or more after the change, which settles what it read.
    Late(Derived),
}

impl Case {
    /// The case of the derivations `before` a request, all reported after `changed`.
    pub fn of(before: &[Derived], changed: SystemTime) -> Case {
        match before.first() {
            None => Case::None,
            Some(first) if first.checked < changed + SETTLE => Case::Early(*first),
            Some(first) => Case::Late(*first),
        }
    }
}

/// The file a daemon of the test reports its derivations to.
pub struct Reports {
    path: PathBuf,
}

impl Reports {
    pub fn new(path: PathBuf) -> Reports {
        Reports { path }
    }

    /// The value of `MARA_TEST_DERIVED` for the daemon.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The derivations reported so far.
    pub fn all(&self) -> Vec<Derived> {
        let text = fs::read_to_string(&self.path).unwrap_or_default();
        (text.lines())
            .map(|line| {
                let n: Vec<u128> = line.split(' ').map(|n| n.parse().unwrap()).collect();
                let count = |at: usize| u64::try_from(n[at]).unwrap();
                let nanos = u64::try_from(n[4]).unwrap();
                Derived {
                    file_reads: count(0),
                    hashes: count(1),
                    parses: count(2),
                    snapshot_builds: count(3),
                    checked: SystemTime::UNIX_EPOCH + Duration::from_nanos(nanos),
                }
            })
            .collect()
    }

    /// How many derivations were reported so far.
    pub fn seen(&self) -> usize {
        self.all().len()
    }

    /// Waits until a derivation after the first `seen` is reported: those after them.
    pub fn wait_after(&self, seen: usize) -> Vec<Derived> {
        let deadline = Instant::now() + WAIT;
        loop {
            let all = self.all();
            if all.len() > seen {
                return all[seen..].to_vec();
            }
            assert!(Instant::now() < deadline, "the watcher did not derive");
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Runs `request`, whose response is the first after the first `seen` derivations were
    /// reported: its value, with the derivations whose work that response holds, those reported
    /// before it was sent. `Err` when one that did work was reported while it ran, before it
    /// ended: that response may or may not hold it.
    pub fn around<T>(
        &self,
        seen: usize,
        request: impl FnOnce() -> T,
    ) -> Result<(T, Vec<Derived>), String> {
        let sent = self.seen();
        let value = request();
        let ended = SystemTime::now();
        let all = self.all();
        let during = all[sent..]
            .iter()
            .find(|derived| derived.checked <= ended && derived.worked());
        if let Some(during) = during {
            return Err(format!(
                "a derivation reported while the request ran: {during:?}"
            ));
        }
        Ok((value, all[seen..sent].to_vec()))
    }
}

/// When the files `paths` last changed: the latest modification or status change among them.
pub fn changed<P: AsRef<Path>>(paths: &[P]) -> SystemTime {
    let time = |seconds: i64, nanos: i64| {
        SystemTime::UNIX_EPOCH
            + Duration::new(seconds.cast_unsigned(), u32::try_from(nanos).unwrap())
    };
    (paths.iter())
        .map(|path| {
            let metadata = fs::symlink_metadata(path.as_ref()).unwrap();
            let modified = time(metadata.mtime(), metadata.mtime_nsec());
            modified.max(time(metadata.ctime(), metadata.ctime_nsec()))
        })
        .max()
        .expect("a changed file")
}

/// Sleeps until 2 s after `changed`: a check then settles what changed.
pub fn until_settled(changed: SystemTime) {
    if let Ok(wait) = (changed + SETTLE).duration_since(SystemTime::now()) {
        thread::sleep(wait);
    }
}

/// Runs `attempt`, given its number from 0, again while it reports a run that cannot be judged,
/// [`ATTEMPTS`] times at most; then fails, naming `what` and why.
pub fn decided(what: &str, mut attempt: impl FnMut(usize) -> Result<(), String>) {
    let mut reasons = Vec::new();
    for number in 0..ATTEMPTS {
        match attempt(number) {
            Ok(()) => return,
            Err(reason) => reasons.push(reason),
        }
    }
    panic!("no run of {what} could be judged in {ATTEMPTS}: {reasons:?}");
}
