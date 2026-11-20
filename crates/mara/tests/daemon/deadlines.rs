//! A client never waits without bound on a daemon that does not answer (DES-DAEMON-LIFECYCLE):
//! before its request is dispatched it serves it in-process, once its connection or its send
//! takes too long; after, it waits as long as the daemon says that the request still runs, and
//! fails with `daemon-lost` after 15 s of silence, never running the request again.

use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::{Test, finish, mara, nothing, signal, stats};

const READ: &[&str] = &["--debug-stats", "--format", "json", "item", "list"];
/// The client's silence bound.
const SILENCE: Duration = Duration::from_secs(15);

/// Continues the daemon `pid` when dropped, also when the test fails.
struct Stopped(u32);

impl Stopped {
    fn new(pid: u32) -> Stopped {
        signal(pid, libc::SIGSTOP);
        Stopped(pid)
    }
}

impl Drop for Stopped {
    fn drop(&mut self) {
        signal(self.0, libc::SIGCONT);
    }
}

#[test]
fn a_client_of_a_daemon_that_stopped_answering_gives_up_and_runs_nothing() {
    let test = Test::new();
    let project = test.project();
    let pid = test.start_in(&project, &[]);
    test.wait_for_session(&project);
    let stopped = Stopped::new(pid);
    let started = Instant::now();
    let run = test.run_in(&project, READ, &[]);
    let elapsed = started.elapsed();
    drop(stopped);
    assert!(elapsed >= SILENCE, "{elapsed:?}");
    assert!(elapsed < SILENCE + Duration::from_secs(10), "{elapsed:?}");
    assert_eq!(run.status.code(), Some(1), "{run:?}");
    let error = &run.json()["error"];
    assert_eq!(error["code"], "daemon-lost", "{run:?}");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("sent nothing for 15 s"),
        "{error}"
    );
    // Not run in-process: no work.
    assert!(nothing(&stats(&run)), "{run:?}");
}

/// A request that runs longer than the silence bound is not cut off: the daemon says that it
/// still runs. Here a write waits at its first pause point.
#[test]
#[cfg(debug_assertions)]
fn a_request_that_runs_longer_than_the_silence_bound_is_answered() {
    let test = Test::new();
    let project = test.project();
    let pauses = test.path().join("pauses");
    std::fs::create_dir(&pauses).unwrap();
    test.start_in(&project, &[("MARA_TEST_PAUSE", pauses.to_str().unwrap())]);
    let create = [
        "item",
        "create",
        "requirement",
        "docs/reqs.mara.md",
        "--title",
        "Late",
        "--field",
        "status=draft",
        "--body",
        "Text.",
    ];
    let mut client = test.spawn_in(&project, &create);
    let paused = pauses.join("paused");
    let started = Instant::now();
    while started.elapsed() < SILENCE + Duration::from_secs(3) {
        assert!(client.try_wait().unwrap().is_none(), "the client gave up");
        thread::sleep(Duration::from_millis(100));
    }
    assert!(paused.exists(), "the writer did not pause");
    while client.try_wait().unwrap().is_none() {
        let _ = std::fs::remove_file(&paused);
        thread::sleep(Duration::from_millis(5));
    }
    let run = finish(client).succeeded();
    assert!(run.stdout.contains("REQ-LATE"), "{run:?}");
}

/// A connection to `socket` left in the daemon's queue, or `None` once the queue is full.
fn queued_connection(socket: &Path) -> Option<OwnedFd> {
    // SAFETY: socket takes no pointer; the descriptor is owned at once.
    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM | libc::SOCK_NONBLOCK, 0) };
    assert!(fd >= 0);
    // SAFETY: just opened.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: an all-zero sockaddr_un is valid.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (slot, byte) in address
        .sun_path
        .iter_mut()
        .zip(socket.as_os_str().as_bytes())
    {
        *slot = *byte as libc::c_char;
    }
    let length = std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
    // SAFETY: the address is initialized.
    let connected = unsafe { libc::connect(fd, (&raw const address).cast(), length) };
    if connected == 0 {
        return Some(owned);
    }
    let error = std::io::Error::last_os_error();
    assert_eq!(error.raw_os_error(), Some(libc::EAGAIN), "{error}");
    None
}

/// A daemon whose queue of connections is full: the client serves its request in-process
/// after its connection timeout, and starts no daemon.
#[test]
fn a_client_whose_connection_is_not_accepted_runs_in_process() {
    let test = Test::new();
    let project = test.project();
    let pid = test.start_in(&project, &[]);
    let (socket, _) = test.paths_of(&mara());
    let direct = test.direct(&project, READ);
    let stopped = Stopped::new(pid);
    let mut queued = Vec::new();
    while let Some(connection) = queued_connection(&socket) {
        queued.push(connection);
        assert!(queued.len() < 100_000, "the queue never fills");
    }
    let started = Instant::now();
    let run = test.run_in(&project, READ, &[]);
    let elapsed = started.elapsed();
    drop(stopped);
    drop(queued);
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
    assert_eq!(run, direct);
    assert_eq!(test.daemons(), [pid]);
}

/// A request too large for the socket's buffers to a daemon that reads nothing: the client
/// serves it in-process once its send bound of 10 s has passed in all.
#[test]
fn a_request_that_cannot_be_sent_in_time_runs_in_process() {
    let test = Test::new();
    let project = test.project();
    let pid = test.start_in(&project, &[]);
    test.wait_for_session(&project);
    let ids: Vec<String> = (0..4)
        .map(|n| format!("REQ-{n}{}", "X".repeat(120_000)))
        .collect();
    let mut args: Vec<&str> = READ.to_vec();
    for id in &ids {
        args.extend(["--id", id]);
    }
    let direct = test.direct(&project, &args);
    let stopped = Stopped::new(pid);
    let started = Instant::now();
    let run = test.run_in(&project, &args, &[]);
    let elapsed = started.elapsed();
    drop(stopped);
    assert!(elapsed >= Duration::from_secs(10), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(19), "{elapsed:?}");
    assert_eq!(run, direct);
    assert_eq!(test.daemons(), [pid]);
}
