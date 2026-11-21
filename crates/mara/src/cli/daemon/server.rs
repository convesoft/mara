//! The daemon process (DES-DAEMON-LIFECYCLE and DES-DAEMON-SESSIONS in
//! `docs/daemon/index.mara.md`).
//!
//! `daemon start` forks it while holding the build's lock, which the daemon keeps until it
//! exits. It leaves the caller's session, standard streams and working directory, binds the
//! build's socket, opens the session of the project it was started in, and answers each
//! connection on a thread of its own ([`Daemon::answer`]): an `operation` request runs a document
//! operation for a client in its project's session ([`client::answer`]), on a worker thread,
//! while the connection's thread tells the client every [`daemon::HEARTBEAT`] that it still runs.
//! A watcher re-derives a session in the background after its inputs changed ([`Watcher`]).
//!
//! A session is dropped after the idle period without a request ([`idle_period`]), and the
//! daemon exits with its last session, or after the idle period when it opened none. It also
//! ends on a `stop` request and on SIGTERM, SIGINT or SIGHUP: it takes no new request, removes
//! its socket, ends its rule evaluations, lets its writes finish and exits, which ends its
//! other requests.

use std::fs::{self, File};
use std::io::{self, PipeWriter, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use mara::daemon::{self, Build, PROTOCOL_VERSION, Runtime};
use mara::hooks::{self, Hook};
use mara::rules;
use mara::session::Sessions;
use mara::watch::Watcher;
use serde_json::{Value, json};

use crate::cli::client;

/// How long a session lasts without a request, and the daemon without a request while it opened
/// no session (DES-IDLE-SESSIONS).
const IDLE_PERIOD: Duration = Duration::from_secs(30 * 60);
/// How long the daemon waits for a client to take an answer.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// The idle period: in a debug build, `MARA_TEST_IDLE_SECONDS` when it holds a whole number
/// (ADR-TEST-IDLE-PERIOD).
fn idle_period() -> Duration {
    hooks::number(Hook::IdleSeconds).map_or(IDLE_PERIOD, Duration::from_secs)
}

/// Forks the daemon, which inherits `lock`, already taken, and opens the session of the project
/// at the canonical root `project`, if any. Returns its process ID once it listens, or why it
/// could not start. The calling process must run no other thread.
pub(super) fn launch(
    runtime: &Runtime,
    build: &Build,
    lock: File,
    project: Option<PathBuf>,
) -> Result<u32, String> {
    let idle = idle_period();
    let (mut reader, writer) =
        io::pipe().map_err(|error| format!("cannot start the daemon: {error}"))?;
    // SAFETY: the process runs one thread, so the child may run any code until it exits.
    match unsafe { libc::fork() } {
        -1 => Err(format!(
            "cannot start the daemon: {}",
            io::Error::last_os_error()
        )),
        0 => {
            drop(reader);
            child(runtime, build, &lock, writer, idle, project)
        }
        pid => {
            drop(writer);
            drop(lock);
            let mut report = String::new();
            let _ = reader.read_to_string(&mut report);
            match report.strip_prefix("error: ") {
                _ if report == "ready" => Ok(pid.cast_unsigned()),
                Some(message) => Err(format!("the daemon could not start: {message}")),
                None => Err("the daemon exited while it started".to_owned()),
            }
        }
    }
}

/// The forked process: reports `ready` or `error: <why>` on `ready`, then serves.
fn child(
    runtime: &Runtime,
    build: &Build,
    lock: &File,
    mut ready: PipeWriter,
    idle: Duration,
    project: Option<PathBuf>,
) -> ! {
    match listen(runtime, lock, ready.as_raw_fd()) {
        Ok((listener, signals)) => {
            let _ = ready.write_all(b"ready");
            drop(ready);
            serve(listener, runtime, build, signals, idle, project)
        }
        Err(message) => {
            let _ = ready.write_all(format!("error: {message}").as_bytes());
            // SAFETY: ends the forked process without running the parent's exit handlers.
            unsafe { libc::_exit(1) }
        }
    }
}

/// Leaves the caller's session and streams, blocks the signals that stop the daemon so that
/// one thread waits for them, replaces a stale socket and records this process in the lock.
fn listen(runtime: &Runtime, lock: &File, ready: RawFd) -> Result<(UnixListener, Signals), String> {
    detach(&[lock.as_raw_fd(), ready]).map_err(|error| format!("cannot detach: {error}"))?;
    let signals = Signals::block().map_err(|error| format!("cannot block signals: {error}"))?;
    runtime.check().map_err(|error| error.to_string())?;
    // The lock is ours, so a socket left here belongs to a daemon that ended.
    match fs::symlink_metadata(&runtime.socket) {
        Ok(metadata) if metadata.file_type().is_socket() => fs::remove_file(&runtime.socket)
            .map_err(|error| format!("cannot remove the stale socket: {error}"))?,
        Ok(_) => {
            return Err(format!(
                "`{}` is no socket; remove it",
                runtime.socket.display()
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot examine the socket: {error}")),
    }
    let listener = runtime.bind().map_err(|error| error.to_string())?;
    // From here on, a failure removes the socket it bound, so that no client finds it.
    let mut lock = lock;
    let recorded = lock
        .set_len(0)
        .and_then(|()| write!(lock, "{}", std::process::id()));
    if let Err(error) = recorded {
        let _ = fs::remove_file(&runtime.socket);
        return Err(format!("cannot write the lock: {error}"));
    }
    Ok((listener, signals))
}

/// Starts a new session without a terminal, points the standard streams at `/dev/null`, closes
/// every other descriptor but `keep`, so that the caller's pipes end when it exits, and moves to
/// `/`, so that it keeps no directory of the caller's in use. A request names the client's
/// directory.
fn detach(keep: &[RawFd]) -> io::Result<()> {
    std::env::set_current_dir("/")?;
    // SAFETY: setsid takes no pointer; the forked child leads no process group yet.
    if unsafe { libc::setsid() } == -1 {
        return Err(io::Error::last_os_error());
    }
    let null = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")?;
    for stream in 0..3 {
        // SAFETY: both descriptors are open.
        if unsafe { libc::dup2(null.as_raw_fd(), stream) } == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    let open: Vec<RawFd> = fs::read_dir("/dev/fd")?
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    for fd in open {
        if fd > 2 && fd != null.as_raw_fd() && !keep.contains(&fd) {
            // SAFETY: closes a descriptor that nothing in this process uses; the listing's own,
            // closed already, fails harmlessly.
            unsafe { libc::close(fd) };
        }
    }
    Ok(())
}

/// The signals that stop the daemon, blocked in every thread and taken by one.
struct Signals(libc::sigset_t);

impl Signals {
    /// Blocks SIGTERM, SIGINT and SIGHUP in this thread and every thread it starts. A child
    /// process inherits the mask, also through `std::process::Command`: whatever starts one
    /// must unblock them in the child, or the child ignores them.
    fn block() -> io::Result<Signals> {
        let mut set = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
        // SAFETY: sigemptyset initializes the set; sigaddset and pthread_sigmask take it
        // initialized.
        unsafe {
            libc::sigemptyset(set.as_mut_ptr());
            for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
                libc::sigaddset(set.as_mut_ptr(), signal);
            }
            let set = set.assume_init();
            let status = libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
            if status != 0 {
                return Err(io::Error::from_raw_os_error(status));
            }
            Ok(Signals(set))
        }
    }

    /// Waits for one of the signals.
    fn wait(&self) {
        let mut signal = 0;
        // SAFETY: the set is initialized and `signal` is writable.
        unsafe { libc::sigwait(&self.0, &mut signal) };
    }
}

// SAFETY: a sigset_t is plain data.
unsafe impl Send for Signals {}

/// What the daemon's threads share.
struct Daemon {
    pid: u32,
    /// The build's key, which every request must carry.
    build: String,
    idle: Duration,
    state: Mutex<State>,
    changed: Condvar,
    sessions: Sessions,
    /// `None` where the system offers no watching: requests check the inputs all the same.
    watcher: Option<Watcher>,
}

struct State {
    /// The daemon takes no new request.
    stopping: bool,
    /// Requests being answered.
    active: usize,
    /// Write requests being answered, which the daemon lets finish when it stops.
    writes: usize,
    /// When the last request that counts as activity arrived, or when the daemon started.
    /// `status` and `stop` do not count, so that a client watching the daemon does not keep
    /// it alive.
    last_request: Instant,
    /// Operation requests run and answered since the daemon started, which `status` reports.
    served: u64,
}

impl Daemon {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn stop(&self) {
        self.state().stopping = true;
        self.changed.notify_all();
    }

    /// Records a request that counts as activity, at its start and its end, so that the idle
    /// period runs from the last one.
    fn touch(&self) {
        self.state().last_request = Instant::now();
        self.changed.notify_all();
    }

    /// The answer to one request, as [`parse`] read it. `stop` makes the daemon stop.
    fn answer(&self, request: &Result<Value, String>) -> Value {
        let request = match request {
            Ok(request) => request,
            Err(error) => return self.refuse("malformed-request", error),
        };
        if request["version"].as_u64() != Some(PROTOCOL_VERSION) {
            return self.refuse("malformed-request", "the request has another version");
        }
        if request["build"].as_str() != Some(self.build.as_str()) {
            return self.refuse("other-build", "this daemon serves another build of Mara");
        }
        match request["request"].as_str() {
            Some("status") => {
                let sessions: Vec<String> = (self.sessions.roots().iter())
                    .map(|root| root.to_string_lossy().into_owned())
                    .collect();
                json!({
                    "version": PROTOCOL_VERSION,
                    "pid": self.pid,
                    "sessions": sessions,
                    "served": self.state().served,
                })
            }
            // The daemon stops once this answer is sent ([`connection`]), so the client gets it
            // before the process can exit.
            Some("stop") => json!({"version": PROTOCOL_VERSION, "pid": self.pid, "stopping": true}),
            Some("operation") => {
                self.touch();
                let answer = client::answer(request, &self.sessions, self.watcher.as_ref());
                self.touch();
                match answer {
                    Ok(mut answer) => {
                        // Counted before the answer is sent, so a client that has it sees it in
                        // `status`.
                        self.state().served += 1;
                        answer["version"] = json!(PROTOCOL_VERSION);
                        answer["pid"] = json!(self.pid);
                        answer
                    }
                    Err(message) => self.refuse("malformed-request", &message),
                }
            }
            _ => self.refuse("unknown-request", "the daemon knows no such request"),
        }
    }

    fn refuse(&self, code: &str, message: &str) -> Value {
        json!({
            "version": PROTOCOL_VERSION,
            "pid": self.pid,
            "error": code,
            "message": message,
        })
    }
}

/// Serves until the daemon stops, then exits.
fn serve(
    listener: UnixListener,
    runtime: &Runtime,
    build: &Build,
    signals: Signals,
    idle: Duration,
    project: Option<PathBuf>,
) -> ! {
    let socket = fs::symlink_metadata(&runtime.socket)
        .map(|metadata| (metadata.dev(), metadata.ino()))
        .ok();
    let daemon = Arc::new(Daemon {
        pid: std::process::id(),
        build: build.key(),
        idle,
        state: Mutex::new(State {
            stopping: false,
            active: 0,
            writes: 0,
            last_request: Instant::now(),
            served: 0,
        }),
        changed: Condvar::new(),
        sessions: Sessions::default(),
        watcher: Watcher::start(),
    });
    // The project the daemon was started for is warm before its first request.
    if let Some(root) = project {
        let (session, _) = daemon.sessions.open(&root);
        match &daemon.watcher {
            Some(watcher) => watcher.derive(&session),
            None => session.derive_in_background(|_| {}),
        }
    }
    let on_signal = Arc::clone(&daemon);
    let spawned = thread::Builder::new()
        .name("mara-signals".to_owned())
        .spawn(move || {
            signals.wait();
            on_signal.stop();
        });
    let on_connection = Arc::clone(&daemon);
    let spawned = spawned.and_then(|_| {
        thread::Builder::new()
            .name("mara-accept".to_owned())
            .spawn(move || accept(&on_connection, &listener))
    });
    if spawned.is_ok() {
        wait_until_stopping(&daemon);
    }
    // Reads end with the process; rule evaluations end first, each killing its child and
    // removing its directory; writes run to their end. Cancelled before the socket goes, so
    // that whoever finds it gone finds the evaluations cancelled.
    rules::cancel::all();
    // No new client finds the daemon from here on; a socket another daemon has bound since
    // stays.
    let current = fs::symlink_metadata(&runtime.socket)
        .map(|metadata| (metadata.dev(), metadata.ino()))
        .ok();
    if current.is_some() && current == socket {
        let _ = fs::remove_file(&runtime.socket);
    }
    let mut state = daemon.state();
    while state.writes > 0 || rules::cancel::running() > 0 {
        state = daemon
            .changed
            .wait_timeout(state, Duration::from_millis(10))
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
    std::process::exit(0)
}

/// Returns once the daemon stops: by request, by signal, when its last session is dropped, or
/// after the idle period when it opened none. Drops each session idle for the idle period.
fn wait_until_stopping(daemon: &Daemon) {
    let mut state = daemon.state();
    loop {
        if state.stopping {
            return;
        }
        let now = Instant::now();
        let (dropped, next) = daemon.sessions.drop_idle(daemon.idle, now);
        if let Some(watcher) = &daemon.watcher {
            for session in &dropped {
                watcher.forget(session);
            }
        }
        let deadline = if daemon.sessions.opened() {
            match next {
                Some(next) => next,
                None if state.active == 0 => {
                    state.stopping = true;
                    return;
                }
                // A request that opens a session runs: its end wakes this thread.
                None => now + daemon.idle,
            }
        } else {
            let deadline = state.last_request + daemon.idle;
            match deadline <= now {
                true if state.active == 0 => {
                    state.stopping = true;
                    return;
                }
                // A request runs: its end wakes this thread.
                true => now + daemon.idle,
                false => deadline,
            }
        };
        let (next_state, _) = daemon
            .changed
            .wait_timeout(state, deadline.saturating_duration_since(now))
            .unwrap_or_else(PoisonError::into_inner);
        state = next_state;
    }
}

// @mara implements REQ-READS-DURING-VALIDATION
/// Accepts connections of this user, each on a thread of its own.
fn accept(daemon: &Arc<Daemon>, listener: &UnixListener) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            // Out of descriptors, for instance: let connections end before the next.
            thread::sleep(Duration::from_millis(10));
            continue;
        };
        if daemon::peer_user(&stream).ok() != Some(daemon::user()) {
            continue;
        }
        let daemon = Arc::clone(daemon);
        let _ = thread::Builder::new()
            .name("mara-connection".to_owned())
            .spawn(move || connection(&daemon, stream));
    }
}

/// The JSON request of one frame, read once for both [`connection`] and [`Daemon::answer`], or
/// why it is none.
fn parse(frame: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(frame).map_err(|error| error.to_string())
}

/// Answers the requests of one connection in order. After `stop`, whose answer it sends before
/// the daemon stops, keeps the connection open until the process exits, which tells the client
/// that the daemon has ended.
fn connection(daemon: &Daemon, mut stream: UnixStream) {
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    while let Ok(Some(frame)) = daemon::read_frame(&mut stream) {
        let request = parse(&frame);
        drop(frame);
        let operation = request
            .as_ref()
            .is_ok_and(|request| request["request"] == "operation");
        let write = operation && request.as_ref().is_ok_and(client::writes);
        let taken = {
            let mut state = daemon.state();
            if !state.stopping {
                state.active += 1;
                state.writes += usize::from(write);
            }
            !state.stopping
        };
        if !taken {
            let refusal = daemon.refuse("stopping", "the daemon is stopping");
            if daemon::send_frame(&stream, refusal.to_string().as_bytes()).is_err() {
                return;
            }
            continue;
        }
        // A panic is a defect: the connection closes without an answer, and the client reports
        // `daemon-lost`, but the daemon still counts the request as finished.
        let answer = match operation {
            true => answer_working(daemon, &request, &stream),
            false => panic::catch_unwind(AssertUnwindSafe(|| daemon.answer(&request))),
        };
        // A stopping daemon ends its reads: one that ends meanwhile, its evaluation cancelled
        // perhaps, is not answered, and its client reports `daemon-lost`. Writes run to their
        // end and are answered.
        let ended = operation && !write && daemon.state().stopping;
        let written = match &answer {
            Ok(_) if ended => Err(io::ErrorKind::Interrupted.into()),
            Ok(answer) => daemon::send_frame(&stream, answer.to_string().as_bytes()),
            Err(_) => Err(io::ErrorKind::Other.into()),
        };
        {
            let mut state = daemon.state();
            state.active -= 1;
            state.writes -= usize::from(write);
        }
        daemon.changed.notify_all();
        let Ok(answer) = answer else {
            return;
        };
        if answer["stopping"] == true {
            daemon.stop();
            loop {
                thread::park();
            }
        }
        if written.is_err() {
            return;
        }
    }
}

/// Answers an operation request on a worker thread, so that this thread can tell the client
/// every [`daemon::HEARTBEAT`] that it still runs: a client takes silence for a daemon that
/// stopped or hangs. A panic of the worker is returned as such.
fn answer_working(
    daemon: &Daemon,
    request: &Result<Value, String>,
    stream: &UnixStream,
) -> thread::Result<Value> {
    thread::scope(|scope| {
        let (done, finished) = mpsc::channel::<()>();
        let work = move || {
            let answer = daemon.answer(request);
            let _ = done.send(());
            answer
        };
        let worker = match thread::Builder::new()
            .name("mara-request".to_owned())
            .spawn_scoped(scope, work)
        {
            Ok(worker) => worker,
            // No thread to spare: the request runs here, without heartbeats.
            Err(_) => return panic::catch_unwind(AssertUnwindSafe(|| daemon.answer(request))),
        };
        let heartbeat = json!({"version": PROTOCOL_VERSION, "pid": daemon.pid, "working": true});
        let heartbeat = heartbeat.to_string();
        let mut beating = true;
        // Ends when the worker has answered, or has panicked and dropped `done`.
        while let Err(mpsc::RecvTimeoutError::Timeout) = finished.recv_timeout(daemon::HEARTBEAT) {
            if beating {
                beating = daemon::send_frame(stream, heartbeat.as_bytes()).is_ok();
            }
        }
        worker.join()
    })
}
