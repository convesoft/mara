//! Watching the sessions' inputs (DES-DAEMON-SESSIONS in `docs/daemon/index.mara.md`): one
//! inotify instance per daemon watches the directories that may hold an input of an open
//! session ([`Session::directories`]). A burst of changes in a session's directories is
//! debounced ([`DEBOUNCE`], at most [`LONGEST_DELAY`] after its first change), then the session
//! re-derives in the background ([`Session::derive_in_background`]) and its watches follow the
//! directories its inputs now lie in. A watch is on a directory's inode: one whose directory was
//! deleted or moved away (the kernel ends it, or reports it moved), or whose path now names
//! another directory, as after a branch checkout, is replaced by a watch of what the path names
//! now. A lost event (queue overflow) or an error of the instance
//! re-derives every session: a full rescan, since a derivation checks every input on disk.
//!
//! Watching only makes requests cheap: every request checks the inputs on disk whatever the
//! watcher saw (REQ-NEVER-STALE). Where no watch can be added (no inotify, a per-user limit
//! reached), a session is served the same, only without background derivation.

use std::sync::Arc;
use std::time::Duration;

use crate::session::Session;

/// How long a session's directories stay quiet before it re-derives.
pub const DEBOUNCE: Duration = Duration::from_millis(50);
/// How long after the first change of a burst a session re-derives, however long it lasts.
pub const LONGEST_DELAY: Duration = Duration::from_millis(500);

/// The daemon's watcher.
#[derive(Clone)]
pub struct Watcher {
    #[cfg(target_os = "linux")]
    inner: Arc<linux::Inner>,
}

impl Watcher {
    /// Starts watching on a thread of its own: `None` when the system offers no watching.
    pub fn start() -> Option<Watcher> {
        #[cfg(target_os = "linux")]
        {
            linux::Inner::start().map(|inner| Watcher { inner })
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }

    /// Makes the watches of `session` those of the directories its last check listed.
    pub fn sync(&self, session: &Arc<Session>) {
        #[cfg(target_os = "linux")]
        self.inner.sync(session, true);
        #[cfg(not(target_os = "linux"))]
        let _ = session;
    }

    /// Removes the watches of a dropped session.
    pub fn forget(&self, session: &Session) {
        #[cfg(target_os = "linux")]
        self.inner.forget(session.root());
        #[cfg(not(target_os = "linux"))]
        let _ = session;
    }

    /// Re-derives `session` in the background, then makes its watches follow its directories.
    /// The session is watched from then on, also when this is its first derivation.
    pub fn derive(&self, session: &Arc<Session>) {
        #[cfg(target_os = "linux")]
        self.inner.register(session);
        let watcher = self.clone();
        session.derive_in_background(move |session| {
            #[cfg(target_os = "linux")]
            watcher.inner.sync_root(session);
            #[cfg(not(target_os = "linux"))]
            let _ = (&watcher, session);
        });
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::{HashMap, HashSet};
    use std::ffi::CString;
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
    use std::thread;
    use std::time::{Duration, Instant};

    use super::{DEBOUNCE, LONGEST_DELAY, Watcher};
    use crate::hooks::{self, Hook};
    use crate::session::Session;

    /// What a directory's watch reports: entries created, deleted, moved or changed, and the
    /// directory itself deleted or moved.
    const MASK: u32 = libc::IN_CREATE
        | libc::IN_DELETE
        | libc::IN_MODIFY
        | libc::IN_ATTRIB
        | libc::IN_CLOSE_WRITE
        | libc::IN_MOVED_FROM
        | libc::IN_MOVED_TO
        | libc::IN_DELETE_SELF
        | libc::IN_MOVE_SELF
        | libc::IN_ONLYDIR
        | libc::IN_DONT_FOLLOW
        | libc::IN_EXCL_UNLINK;

    pub(super) struct Inner {
        fd: OwnedFd,
        state: Mutex<State>,
    }

    #[derive(Default)]
    struct State {
        /// Each watch and the roots of the sessions that watch through it.
        watches: HashMap<i32, HashSet<PathBuf>>,
        /// Each session by root, with the directories it watches.
        sessions: HashMap<PathBuf, (Weak<Session>, HashMap<PathBuf, Watched>)>,
        /// The sessions whose directories changed: the first and the last change not yet
        /// derived.
        pending: HashMap<PathBuf, (Instant, Instant)>,
    }

    /// A session's watch of a directory: the watch, and the device and inode of the directory
    /// that its path named when the watch was added.
    #[derive(Clone, Copy)]
    struct Watched {
        wd: i32,
        inode: (u64, u64),
    }

    fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
        state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The watches of `root` while `session` is the one registered for it.
    fn owned<'s>(
        state: &'s mut State,
        root: &Path,
        session: &Weak<Session>,
    ) -> Option<&'s mut HashMap<PathBuf, Watched>> {
        let (open, watched) = state.sessions.get_mut(root)?;
        Weak::ptr_eq(open, session).then_some(watched)
    }

    /// The device and inode of the directory at `path`, without following a link.
    fn inode(path: &Path) -> Option<(u64, u64)> {
        let metadata = std::fs::symlink_metadata(path).ok()?;
        metadata.is_dir().then(|| (metadata.dev(), metadata.ino()))
    }

    impl Inner {
        pub(super) fn start() -> Option<Arc<Inner>> {
            // SAFETY: inotify_init1 takes flags only.
            let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
            if fd < 0 {
                return None;
            }
            // SAFETY: the descriptor was just opened and nothing else owns it.
            let fd = unsafe { OwnedFd::from_raw_fd(fd) };
            let inner = Arc::new(Inner {
                fd,
                state: Mutex::default(),
            });
            let watching = Arc::clone(&inner);
            thread::Builder::new()
                .name("mara-watch".to_owned())
                .spawn(move || watching.run())
                .ok()?;
            Some(inner)
        }

        /// Watches `session` from now on, with the directories it watches, if any.
        pub(super) fn register(&self, session: &Arc<Session>) {
            let mut state = lock(&self.state);
            (state.sessions)
                .entry(session.root().to_owned())
                .or_insert_with(|| (Arc::downgrade(session), HashMap::new()));
        }

        /// Makes the watches of `session` those of the directories its last check listed, each
        /// on the directory its path names now. A request's sync (`claim`) makes `session` the
        /// one its root's watches serve; a background sync goes on only while it is.
        ///
        /// Directories are looked up without the lock. Each change of a watch, the system call
        /// with its record, is one step under the lock, which is then held for one
        /// `inotify_add_watch` or `inotify_rm_watch` and released before the next, so that the
        /// thread that reads events and other sessions get in between. Before each step the
        /// sync checks that its session is still the one registered for its root: one whose
        /// session was forgotten or replaced changes nothing more. No descriptor is recorded
        /// after it was removed, and the thread that reads events sees every watch recorded
        /// from its first event on.
        pub(super) fn sync(&self, session: &Arc<Session>, claim: bool) {
            let directories: HashSet<PathBuf> = session.directories().into_iter().collect();
            let root = session.root().to_owned();
            let this = Arc::downgrade(session);
            // The directories no longer listed, whose watches end.
            let gone: Vec<PathBuf> = {
                let mut state = lock(&self.state);
                if claim {
                    let entry = (state.sessions)
                        .entry(root.clone())
                        .or_insert_with(|| (this.clone(), HashMap::new()));
                    entry.0 = this.clone();
                }
                let Some((open, watched)) = state.sessions.get(&root) else {
                    return;
                };
                if !Weak::ptr_eq(open, &this) {
                    return;
                }
                (watched.keys())
                    .filter(|directory| !directories.contains(*directory))
                    .cloned()
                    .collect()
            };
            for directory in gone {
                let mut state = lock(&self.state);
                let Some(watched) = owned(&mut state, &root, &this) else {
                    return;
                };
                if let Some(watch) = watched.remove(&directory) {
                    self.unwatch(&mut state, watch.wd, &root);
                }
            }
            // A pause point for tests (ADR-DEBUG-PAUSE-POINT): before the lookups and new
            // watches, while no lock is held.
            let _ = hooks::pause(Hook::PauseWatch, "");
            for directory in directories {
                // Looked up before the watch is added: a directory replaced in between is
                // found by the next sync, whose lookup tells another inode.
                let now = inode(&directory);
                let mut state = lock(&self.state);
                let Some(watched) = owned(&mut state, &root, &this) else {
                    return;
                };
                let before = watched.get(&directory).copied();
                if let Some(watch) = before {
                    // The kernel still holds the watch, and the path names its directory.
                    if Some(watch.inode) == now && state.watches.contains_key(&watch.wd) {
                        continue;
                    }
                    if let Some(watched) = owned(&mut state, &root, &this) {
                        watched.remove(&directory);
                    }
                    self.unwatch(&mut state, watch.wd, &root);
                    // The next watch is a step of its own.
                    drop(state);
                    state = lock(&self.state);
                    if owned(&mut state, &root, &this).is_none() {
                        return;
                    }
                }
                if let Some(now) = now
                    && let Some(wd) = self.add(&directory)
                {
                    state.watches.entry(wd).or_default().insert(root.clone());
                    if let Some(watched) = owned(&mut state, &root, &this) {
                        watched.insert(directory, Watched { wd, inode: now });
                    }
                }
            }
        }

        /// [`Inner::sync`] of the session at `session`'s root, while it is open.
        pub(super) fn sync_root(&self, session: &Session) {
            let open = lock(&self.state)
                .sessions
                .get(session.root())
                .and_then(|(session, _)| session.upgrade());
            if let Some(session) = open {
                self.sync(&session, false);
            }
        }

        pub(super) fn forget(&self, root: &Path) {
            let mut state = lock(&self.state);
            state.pending.remove(root);
            if let Some((_, watched)) = state.sessions.remove(root) {
                for watch in watched.values() {
                    self.unwatch(&mut state, watch.wd, root);
                }
            }
        }

        /// Adds a watch of `directory`: `None` when it cannot be watched, for instance past the
        /// per-user limit, or because it is gone. One inode watched through another path has
        /// the same watch.
        fn add(&self, directory: &Path) -> Option<i32> {
            let path = CString::new(directory.as_os_str().as_bytes()).ok()?;
            // SAFETY: the descriptor is open and the path is a NUL-terminated string.
            let wd = unsafe { libc::inotify_add_watch(self.fd.as_raw_fd(), path.as_ptr(), MASK) };
            (wd >= 0).then_some(wd)
        }

        /// Ends `root`'s use of the watch `wd`, and the watch once no session uses it.
        fn unwatch(&self, state: &mut State, wd: i32, root: &Path) {
            let Some(roots) = state.watches.get_mut(&wd) else {
                return;
            };
            roots.remove(root);
            if roots.is_empty() {
                state.watches.remove(&wd);
                // SAFETY: the descriptor is open; a watch that is gone already fails harmlessly.
                unsafe { libc::inotify_rm_watch(self.fd.as_raw_fd(), wd) };
            }
        }

        /// Reads events and re-derives the sessions whose bursts ended, until the daemon exits.
        fn run(self: Arc<Self>) {
            let mut buffer = vec![0u8; 64 << 10];
            loop {
                let timeout = self
                    .next_due()
                    .map(|due| due.saturating_duration_since(Instant::now()));
                let mut poll = libc::pollfd {
                    fd: self.fd.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                let millis = timeout.map_or(-1, |timeout| {
                    i32::try_from(timeout.as_millis().saturating_add(1)).unwrap_or(i32::MAX)
                });
                // SAFETY: one pollfd that lives across the call.
                unsafe { libc::poll(&mut poll, 1, millis) };
                if let Err(error) = self.read_events(&mut buffer) {
                    // Events may be lost: every session checks all its inputs again.
                    self.everything_changed();
                    if error.kind() != io::ErrorKind::Interrupted {
                        thread::sleep(Duration::from_millis(100));
                    }
                }
                self.derive_due();
            }
        }

        fn next_due(&self) -> Option<Instant> {
            let state = lock(&self.state);
            (state.pending.values())
                .map(|&(first, last)| (last + DEBOUNCE).min(first + LONGEST_DELAY))
                .min()
        }

        /// Reads every event queued, marking the sessions they concern.
        fn read_events(&self, buffer: &mut [u8]) -> io::Result<()> {
            loop {
                // SAFETY: the buffer is writable for its length.
                let read = unsafe {
                    libc::read(
                        self.fd.as_raw_fd(),
                        buffer.as_mut_ptr().cast(),
                        buffer.len(),
                    )
                };
                if read < 0 {
                    let error = io::Error::last_os_error();
                    return match error.kind() {
                        io::ErrorKind::WouldBlock => Ok(()),
                        _ => Err(error),
                    };
                }
                let read = read.cast_unsigned();
                let now = Instant::now();
                let mut state = lock(&self.state);
                let mut offset = 0;
                let header = std::mem::size_of::<libc::inotify_event>();
                while offset + header <= read {
                    // SAFETY: the kernel wrote a whole event header here; it may be unaligned.
                    let event: libc::inotify_event =
                        unsafe { std::ptr::read_unaligned(buffer[offset..].as_ptr().cast()) };
                    offset += header + event.len as usize;
                    if event.mask & libc::IN_Q_OVERFLOW != 0 {
                        drop(state);
                        self.everything_changed();
                        state = lock(&self.state);
                        continue;
                    }
                    let roots: Vec<PathBuf> = match state.watches.get(&event.wd) {
                        Some(roots) => roots.iter().cloned().collect(),
                        None => continue,
                    };
                    // The watch ended, or follows its directory out of the project: forgotten,
                    // so that the sessions' next sync watches what their paths name then.
                    if event.mask & libc::IN_IGNORED != 0 {
                        state.watches.remove(&event.wd);
                    } else if event.mask & (libc::IN_DELETE_SELF | libc::IN_MOVE_SELF) != 0 {
                        state.watches.remove(&event.wd);
                        // SAFETY: the descriptor is open; a watch that is gone fails harmlessly.
                        unsafe { libc::inotify_rm_watch(self.fd.as_raw_fd(), event.wd) };
                    }
                    for root in roots {
                        let entry = state.pending.entry(root).or_insert((now, now));
                        entry.1 = now;
                    }
                }
            }
        }

        fn everything_changed(&self) {
            let now = Instant::now();
            let mut state = lock(&self.state);
            let roots: Vec<PathBuf> = state.sessions.keys().cloned().collect();
            for root in roots {
                let entry = state.pending.entry(root).or_insert((now, now));
                entry.1 = now;
            }
        }

        fn derive_due(self: &Arc<Self>) {
            let now = Instant::now();
            let due: Vec<Arc<Session>> = {
                let mut state = lock(&self.state);
                let roots: Vec<PathBuf> = (state.pending.iter())
                    .filter(|(_, (first, last))| {
                        (*last + DEBOUNCE).min(*first + LONGEST_DELAY) <= now
                    })
                    .map(|(root, _)| root.clone())
                    .collect();
                roots
                    .into_iter()
                    .filter_map(|root| {
                        state.pending.remove(&root);
                        state.sessions.get(&root)?.0.upgrade()
                    })
                    .collect()
            };
            let watcher = Watcher {
                inner: Arc::clone(self),
            };
            for session in due {
                watcher.derive(&session);
            }
        }
    }
}
