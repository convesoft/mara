//! A daemon's sessions (DES-DAEMON-SESSIONS and DES-IDLE-SESSIONS in `docs/daemon/index.mara.md`):
//! one per canonical project root, holding that project's inputs between requests.
//!
//! A request runs against a view of its session's inputs ([`Inputs::shared`]): every request
//! checks the inputs on disk, so its answer reflects the files as they are when it is served,
//! whatever the watcher has delivered (REQ-NEVER-STALE). The watcher ([`crate::watch`]) only
//! makes that check cheap, by re-deriving in the background after a change ([`Session::derive`]).
//! The work a session does, in requests and in the background, is reported by its next
//! response ([`Session::take_work`], DES-PERFORMANCE-HARNESS).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use mara_engine::work::WorkCounts;

use crate::inputs::Inputs;

/// One project's session.
#[derive(Debug)]
pub struct Session {
    root: PathBuf,
    inputs: Arc<Mutex<Inputs>>,
    /// The work done since the previous response.
    work: Mutex<WorkCounts>,
    activity: Mutex<Activity>,
    /// Whether a background derivation runs, and whether another change came meanwhile.
    deriving: Mutex<(bool, bool)>,
}

#[derive(Debug)]
struct Activity {
    /// When the last request began or ended, or when the session opened.
    last: Instant,
    /// Requests being served.
    active: usize,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Session {
    fn new(root: PathBuf) -> Session {
        Session {
            inputs: Arc::new(Mutex::new(Inputs::new(root.clone()))),
            root,
            work: Mutex::default(),
            activity: Mutex::new(Activity {
                last: Instant::now(),
                active: 0,
            }),
            deriving: Mutex::default(),
        }
    }

    /// The canonical project root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A view of the session's inputs for one request.
    pub fn inputs(&self) -> Inputs {
        Inputs::shared(Arc::clone(&self.inputs))
    }

    /// Adds the work of a request, or of the background, to what the next response reports.
    pub fn add(&self, work: &WorkCounts) {
        let mut guard = lock(&self.work);
        let total = &mut *guard;
        for (field, value) in [
            (&mut total.file_reads, work.file_reads),
            (&mut total.hashes, work.hashes),
            (&mut total.parses, work.parses),
            (&mut total.snapshot_builds, work.snapshot_builds),
            (&mut total.code_scans, work.code_scans),
            (&mut total.subprocess_spawns, work.subprocess_spawns),
            (&mut total.check_steps, work.check_steps),
            (&mut total.program_runs, work.program_runs),
        ] {
            *field += value;
        }
    }

    /// The work done since the previous response, which a response reports.
    pub fn take_work(&self) -> WorkCounts {
        std::mem::take(&mut *lock(&self.work))
    }

    /// The directories that may hold inputs, as the last check listed them, with `.mara`,
    /// which holds the configuration, and the schema's directory: what the watcher watches.
    pub fn directories(&self) -> Vec<PathBuf> {
        let inputs = lock(&self.inputs);
        let mut directories: Vec<PathBuf> = (inputs.directories().into_iter())
            .map(|relative| self.root.join(relative))
            .collect();
        directories.push(self.root.join(".mara"));
        let schema = inputs.schema_path();
        drop(inputs);
        if let Some(parent) = schema.as_deref().map(Path::new).and_then(Path::parent) {
            directories.push(self.root.join(parent));
        }
        directories.sort_unstable();
        directories.dedup();
        directories
    }

    /// Checks the inputs and re-derives what changed, as a request that reads documents would,
    /// so that the next request finds the work done. Its work goes to the next response.
    pub fn derive(&self) {
        let mut work = WorkCounts::default();
        self.inputs().snapshot(SystemTime::now(), &mut work);
        self.add(&work);
    }

    /// [`Session::derive`] on a thread of its own, then `after`. A change that comes while one
    /// runs makes it run once more; at most one runs per session.
    pub fn derive_in_background(self: &Arc<Self>, after: impl Fn(&Session) + Send + 'static) {
        {
            let mut deriving = lock(&self.deriving);
            if deriving.0 {
                deriving.1 = true;
                return;
            }
            *deriving = (true, false);
        }
        let session = Arc::clone(self);
        let spawned = thread::Builder::new()
            .name("mara-derive".to_owned())
            .spawn(move || {
                loop {
                    session.derive();
                    after(&session);
                    let mut deriving = lock(&session.deriving);
                    if !deriving.1 {
                        deriving.0 = false;
                        return;
                    }
                    deriving.1 = false;
                }
            });
        if spawned.is_err() {
            // The next request checks the inputs all the same.
            *lock(&self.deriving) = (false, false);
        }
    }

    /// Marks a request as begun or ended: the idle period runs from the last of them.
    fn touch(&self, begins: bool) {
        let mut activity = lock(&self.activity);
        activity.last = Instant::now();
        match begins {
            true => activity.active += 1,
            false => activity.active -= 1,
        }
    }

    /// When the session falls idle, if no request is being served.
    fn idle_from(&self) -> Option<Instant> {
        let activity = lock(&self.activity);
        (activity.active == 0).then_some(activity.last)
    }
}

/// The open sessions of a daemon.
#[derive(Debug, Default)]
pub struct Sessions {
    open: Mutex<HashMap<PathBuf, Arc<Session>>>,
    /// Whether any session was opened: then the daemon exits with its last session.
    opened: Mutex<bool>,
}

/// A request being served in its session: it keeps the session from being dropped.
#[derive(Debug)]
pub struct Entered(Arc<Session>);

impl std::ops::Deref for Entered {
    type Target = Arc<Session>;

    fn deref(&self) -> &Arc<Session> {
        &self.0
    }
}

impl Drop for Entered {
    fn drop(&mut self) {
        self.0.touch(false);
    }
}

impl Sessions {
    /// The session of the canonical root `root`, opened unless it is open, and whether it was
    /// just opened.
    pub fn open(&self, root: &Path) -> (Arc<Session>, bool) {
        let mut open = lock(&self.open);
        self.find_or_open(&mut open, root)
    }

    /// Enters the session of `root` for a request, opening it unless it is open, and whether
    /// it was just opened. The session is not dropped while the request is served.
    pub fn enter(&self, root: &Path) -> (Entered, bool) {
        let mut open = lock(&self.open);
        let (session, opened) = self.find_or_open(&mut open, root);
        // Under the lock that dropping an idle session takes.
        session.touch(true);
        (Entered(session), opened)
    }

    fn find_or_open(
        &self,
        open: &mut HashMap<PathBuf, Arc<Session>>,
        root: &Path,
    ) -> (Arc<Session>, bool) {
        if let Some(session) = open.get(root) {
            return (Arc::clone(session), false);
        }
        let session = Arc::new(Session::new(root.to_owned()));
        open.insert(root.to_owned(), Arc::clone(&session));
        *lock(&self.opened) = true;
        (session, true)
    }

    /// The roots of the open sessions, in order.
    pub fn roots(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = lock(&self.open).keys().cloned().collect();
        roots.sort_unstable();
        roots
    }

    /// Whether any session was ever opened.
    pub fn opened(&self) -> bool {
        *lock(&self.opened)
    }

    /// Drops each session idle for `idle` at `now`, with its inputs. Returns the dropped
    /// sessions and when the next one falls due, if any session stays open.
    pub fn drop_idle(&self, idle: Duration, now: Instant) -> (Vec<Arc<Session>>, Option<Instant>) {
        let mut open = lock(&self.open);
        let mut dropped = Vec::new();
        let mut next: Option<Instant> = None;
        open.retain(|_, session| {
            let due = session.idle_from().map(|last| last + idle);
            match due {
                Some(due) if due <= now => {
                    dropped.push(Arc::clone(session));
                    false
                }
                // A session serving a request falls due an idle period after it ends.
                due => {
                    let due = due.unwrap_or(now + idle);
                    next = Some(next.map_or(due, |next| next.min(due)));
                    true
                }
            }
        });
        (dropped, next)
    }

    /// Whether no session is open.
    pub fn is_empty(&self) -> bool {
        lock(&self.open).is_empty()
    }
}
