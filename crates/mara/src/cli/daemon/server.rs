//! The daemon process (DES-DAEMON-LIFECYCLE in `docs/daemon/index.mara.md`).
//!
//! `daemon start` forks it while holding the build's lock, which the daemon keeps until it
//! exits. It leaves the caller's session, standard streams and working directory, binds the
//! build's socket and answers each connection on a thread of its own ([`Daemon::answer`]): an
//! `operation` request runs a document operation for a client ([`client::answer`]). It ends on a `stop`
//! request, on SIGTERM, SIGINT or SIGHUP, and after the idle period without a request when it
//! has no session ([`idle_period`]); then it removes its socket, lets the requests it runs
//! finish and exits.

use std::fs::{self, File};
use std::io::{self, PipeWriter, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use mara::daemon::{self, Build, PROTOCOL_VERSION, Runtime};
use serde_json::{Value, json};

use crate::cli::client;

/// How long the daemon runs without a request while it has no session (DES-IDLE-SESSIONS).
const IDLE_PERIOD: Duration = Duration::from_secs(30 * 60);
/// How long the daemon waits for a client to take an answer.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// The idle period: in a debug build, `MARA_TEST_IDLE_SECONDS` when it holds a whole number
/// (ADR-TEST-IDLE-PERIOD).
#[cfg(debug_assertions)]
fn idle_period() -> Duration {
    std::env::var("MARA_TEST_IDLE_SECONDS")
        .ok()
        .and_then(|seconds| seconds.parse().ok())
        .map_or(IDLE_PERIOD, Duration::from_secs)
}

#[cfg(not(debug_assertions))]
fn idle_period() -> Duration {
    IDLE_PERIOD
}

/// Forks the daemon, which inherits `lock`, already taken, and returns its process ID once it
/// listens, or why it could not start. The calling process must run no other thread.
pub(super) fn launch(runtime: &Runtime, build: &Build, lock: File) -> Result<u32, String> {
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
            child(runtime, build, &lock, writer, idle)
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
) -> ! {
    match listen(runtime, lock, ready.as_raw_fd()) {
        Ok((listener, signals)) => {
            let _ = ready.write_all(b"ready");
            drop(ready);
            serve(listener, runtime, build, signals, idle)
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
}

struct State {
    /// The daemon takes no new request.
    stopping: bool,
    /// Requests being answered.
    active: usize,
    /// When the last request that counts as activity arrived, or when the daemon started.
    /// `status` and `stop` do not count, so that a client watching the daemon does not keep
    /// it alive.
    last_request: Instant,
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

    /// The answer to one request. `stop` makes the daemon stop.
    fn answer(&self, frame: &[u8]) -> Value {
        let request: Value = match serde_json::from_slice(frame) {
            Ok(request) => request,
            Err(error) => return self.refuse("malformed-request", &error.to_string()),
        };
        if request["version"].as_u64() != Some(PROTOCOL_VERSION) {
            return self.refuse("malformed-request", "the request has another version");
        }
        if request["build"].as_str() != Some(self.build.as_str()) {
            return self.refuse("other-build", "this daemon serves another build of Mara");
        }
        match request["request"].as_str() {
            Some("status") => json!({
                "version": PROTOCOL_VERSION,
                "pid": self.pid,
                "sessions": Vec::<String>::new(),
            }),
            Some("stop") => {
                self.stop();
                json!({"version": PROTOCOL_VERSION, "pid": self.pid, "stopping": true})
            }
            Some("operation") => {
                self.touch();
                let answer = client::answer(&request);
                self.touch();
                match answer {
                    Ok(mut answer) => {
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
            last_request: Instant::now(),
        }),
        changed: Condvar::new(),
    });
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
    // No new client finds the daemon from here on; a socket another daemon has bound since
    // stays.
    let current = fs::symlink_metadata(&runtime.socket)
        .map(|metadata| (metadata.dev(), metadata.ino()))
        .ok();
    if current.is_some() && current == socket {
        let _ = fs::remove_file(&runtime.socket);
    }
    let mut state = daemon.state();
    while state.active > 0 {
        state = daemon
            .changed
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner);
    }
    std::process::exit(0)
}

/// Returns once the daemon stops: by request, by signal or after the idle period.
fn wait_until_stopping(daemon: &Daemon) {
    let mut state = daemon.state();
    loop {
        if state.stopping {
            return;
        }
        let deadline = state.last_request + daemon.idle;
        let now = Instant::now();
        state = if now < deadline {
            let (state, _) = daemon
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner);
            state
        } else if state.active == 0 {
            state.stopping = true;
            return;
        } else {
            daemon
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner)
        };
    }
}

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

/// Answers the requests of one connection in order. After `stop`, keeps the connection open
/// until the process exits, which tells the client that the daemon has ended.
fn connection(daemon: &Daemon, mut stream: UnixStream) {
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    while let Ok(Some(frame)) = daemon::read_frame(&mut stream) {
        let taken = {
            let mut state = daemon.state();
            state.active += usize::from(!state.stopping);
            !state.stopping
        };
        if !taken {
            let refusal = daemon.refuse("stopping", "the daemon is stopping");
            if daemon::write_frame(&mut stream, refusal.to_string().as_bytes()).is_err() {
                return;
            }
            continue;
        }
        // A panic is a defect: the connection closes without an answer, and the client reports
        // `daemon-lost`, but the daemon still counts the request as finished.
        let answer = panic::catch_unwind(AssertUnwindSafe(|| daemon.answer(&frame)));
        let written = match &answer {
            Ok(answer) => daemon::write_frame(&mut stream, answer.to_string().as_bytes()),
            Err(_) => Err(io::ErrorKind::Other.into()),
        };
        daemon.state().active -= 1;
        daemon.changed.notify_all();
        let Ok(answer) = answer else {
            return;
        };
        if answer["stopping"] == true {
            loop {
                thread::park();
            }
        }
        if written.is_err() {
            return;
        }
    }
}
