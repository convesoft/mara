//! The cases of VER-DAEMON-START (`docs/verification-daemon.mara.md`) that need no client to
//! start a daemon on demand: `mara daemon status | start | stop`, the runtime directory, the
//! lock and the build, each through `mara daemon start`; and the daemon's idle exit
//! (DES-DAEMON-LIFECYCLE, ADR-TEST-IDLE-PERIOD in `docs/daemon.mara.md`).
//!
//! Every test has a runtime directory of its own, set as `XDG_RUNTIME_DIR`. A daemon of the
//! test is a process of a binary under test whose environment holds that directory; a test
//! stops only those, and kills any left when it ends, also when it fails. Only Linux runs
//! these tests: they find processes in `/proc`.

#![cfg(target_os = "linux")]

use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

#[path = "support/daemons.rs"]
mod support;

use support::Daemons;

/// How long a test waits for a process to end, or for a daemon to start or exit.
const WAIT: Duration = Duration::from_secs(10);
/// How long a run of `mara` may take: `daemon start` waits up to 10 s for a held lock.
const RUN_WAIT: Duration = Duration::from_secs(30);

/// The binary under test.
fn mara() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mara"))
}

/// A run of `mara` whose stdout and stderr both reached end of file.
struct Run {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

impl Run {
    fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|error| panic!("{error}: {}{}", self.stdout, self.stderr))
    }

    fn succeeded(self) -> Run {
        assert!(self.status.success(), "{}{}", self.stdout, self.stderr);
        self
    }
}

/// A test's temporary directory and runtime directory, and the daemons it starts.
struct Test {
    /// Kills the daemons of the test that still run when it ends, before its directory goes.
    daemons: Daemons,
    dir: tempfile::TempDir,
    /// `XDG_RUNTIME_DIR` of every run.
    base: PathBuf,
}

impl Test {
    /// A test whose runtime directory is `run/` in its directory, with mode 0700.
    fn new() -> Test {
        Test::with_base(|dir| {
            let base = dir.join("run");
            private_directory(&base);
            base
        })
    }

    /// A test whose runtime directory is the path `base` returns for its directory.
    fn with_base(base: impl FnOnce(&Path) -> PathBuf) -> Test {
        let dir = tempfile::Builder::new()
            .prefix("mara-daemon-")
            .tempdir()
            .expect("create a temporary directory");
        let base = base(dir.path());
        Test {
            daemons: Daemons::new(&base, Some(dir.path())),
            dir,
            base,
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    /// `binary` with `args`, in the test's directory, its environment holding only the runtime
    /// directory and `env`.
    fn command(&self, binary: &Path, args: &[&str], env: &[(&str, &str)]) -> Command {
        let mut command = Command::new(binary);
        command
            .args(args)
            .current_dir(self.path())
            .env_clear()
            .env("XDG_RUNTIME_DIR", &self.base)
            .envs(env.iter().copied())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// Starts `binary` with `args`.
    fn spawn(&self, binary: &Path, args: &[&str], env: &[(&str, &str)]) -> Child {
        spawn(&mut self.command(binary, args, env))
    }

    /// Runs `binary` with `args` and requires both pipes to end when it exits: no daemon it
    /// started holds them.
    fn run_binary(&self, binary: &Path, args: &[&str], env: &[(&str, &str)]) -> Run {
        finish(self.spawn(binary, args, env))
    }

    fn run(&self, args: &[&str]) -> Run {
        self.run_binary(&mara(), args, &[])
    }

    /// `daemon status --format json` of `binary`, which exits 0.
    fn status_of(&self, binary: &Path) -> Value {
        self.run_binary(binary, &["--format", "json", "daemon", "status"], &[])
            .succeeded()
            .json()
    }

    fn status(&self) -> Value {
        self.status_of(&mara())
    }

    /// The process ID that `daemon start` of `binary` reports, with whether it started it.
    fn start_of(&self, binary: &Path, env: &[(&str, &str)]) -> (bool, u32) {
        let json = self
            .run_binary(binary, &["--format", "json", "daemon", "start"], env)
            .succeeded()
            .json();
        let pid = json["pid"].as_u64().expect("a process ID");
        (json["started"] == true, u32::try_from(pid).unwrap())
    }

    fn start(&self) -> (bool, u32) {
        self.start_of(&mara(), &[])
    }

    /// The daemons of the test that run.
    fn daemons(&self) -> Vec<u32> {
        self.daemons.running()
    }

    /// Waits until the test has no daemon but `except`.
    fn wait_for_no_daemon(&self, except: &[u32]) {
        let deadline = Instant::now() + WAIT;
        loop {
            let left: Vec<u32> = (self.daemons().into_iter())
                .filter(|pid| !except.contains(pid))
                .collect();
            if left.is_empty() {
                return;
            }
            assert!(Instant::now() < deadline, "daemons still run: {left:?}");
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// The runtime directory of the build of `binary`: its socket and lock.
    fn paths_of(&self, binary: &Path) -> (PathBuf, PathBuf) {
        let status = self.status_of(binary);
        let path = |name: &str| PathBuf::from(status[name].as_str().expect("a path"));
        (path("socket"), path("lock"))
    }
}

fn signal(pid: u32, signal: libc::c_int) {
    // SAFETY: kill takes no pointer.
    unsafe { libc::kill(pid.cast_signed(), signal) };
}

/// Creates `path` with mode 0700.
fn private_directory(path: &Path) {
    fs::create_dir(path).expect("create a directory");
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

/// Starts `command`, again while another thread's child still holds a binary just written
/// open (`ETXTBSY`).
fn spawn(command: &mut Command) -> Child {
    let deadline = Instant::now() + WAIT;
    loop {
        match command.spawn() {
            Ok(child) => return child,
            Err(error) if error.raw_os_error() == Some(libc::ETXTBSY) => {
                assert!(Instant::now() < deadline, "{error}");
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("run {command:?}: {error}"),
        }
    }
}

/// Waits for `child` and reads both its pipes to their end, which must come within
/// [`RUN_WAIT`].
fn finish(mut child: Child) -> Run {
    let (sender, receiver) = mpsc::channel();
    for (index, pipe) in [
        Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
        Box::new(child.stderr.take().unwrap()),
    ]
    .into_iter()
    .enumerate()
    {
        let sender = sender.clone();
        let mut pipe = pipe;
        thread::spawn(move || {
            let mut text = String::new();
            let _ = pipe.read_to_string(&mut text);
            let _ = sender.send((index, text));
        });
    }
    let mut texts = [String::new(), String::new()];
    for _ in 0..2 {
        let (index, text) = receiver
            .recv_timeout(RUN_WAIT)
            .expect("stdout and stderr reach end of file");
        texts[index] = text;
    }
    let status = child.wait().unwrap();
    let [stdout, stderr] = texts;
    Run {
        status,
        stdout,
        stderr,
    }
}

/// A copy of the binary under test at `path`, with modification time `modified`, written
/// beside it and renamed into place.
fn copy_binary(path: &Path, modified: SystemTime) {
    let staged = path.with_extension("new");
    fs::copy(mara(), &staged).unwrap();
    File::options()
        .write(true)
        .open(&staged)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    fs::rename(&staged, path).unwrap();
}

/// Takes the lock at `path` as a starting daemon takes it.
fn hold_lock(path: &Path) -> File {
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.lock().unwrap();
    file
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().mode() & 0o7777
}

#[test]
fn status_start_and_stop_with_and_without_a_daemon() {
    let test = Test::new();
    let status = test.status();
    assert_eq!(status["running"], false, "{status}");
    assert_eq!(status["pid"], Value::Null);
    let stopped = test
        .run(&["--format", "json", "daemon", "stop"])
        .succeeded();
    assert_eq!(stopped.json()["stopped"], false);
    assert!(test.daemons().is_empty());

    let (started, pid) = test.start();
    assert!(started);
    assert_eq!(test.daemons(), [pid]);
    let status = test.status();
    assert_eq!(
        (&status["running"], &status["pid"], &status["sessions"]),
        (
            &Value::Bool(true),
            &Value::from(pid),
            &Value::Array(Vec::new())
        ),
        "{status}"
    );
    let human = test.run(&["daemon", "status"]).succeeded();
    assert!(
        human.stdout.contains(&format!("process {pid}")),
        "{}",
        human.stdout
    );

    // The runtime directory and its files are private.
    let (socket, lock) = test.paths_of(&mara());
    assert_eq!(socket.parent(), Some(test.base.join("mara").as_path()));
    assert_eq!(mode(socket.parent().unwrap()), 0o700);
    assert_eq!(mode(&socket), 0o600);
    assert_eq!(mode(&lock), 0o600);

    assert_eq!(test.start(), (false, pid));
    assert_eq!(test.daemons(), [pid]);

    let stopped = test
        .run(&["--format", "json", "daemon", "stop"])
        .succeeded();
    assert_eq!(stopped.json()["stopped"], true);
    assert_eq!(stopped.json()["pid"], pid);
    assert!(test.daemons().is_empty(), "the daemon runs after `stop`");
    assert!(!socket.exists(), "the daemon left its socket");
    assert_eq!(test.status()["running"], false);
    let stopped = test.run(&["daemon", "stop"]).succeeded();
    assert_eq!(stopped.stdout, "No daemon of this build runs.\n");
}

#[test]
fn ten_starts_at_once_leave_one_daemon() {
    let test = Test::new();
    let children: Vec<Child> = (0..10)
        .map(|_| test.spawn(&mara(), &["--format", "json", "daemon", "start"], &[]))
        .collect();
    let answers: Vec<Value> = children
        .into_iter()
        .map(|child| finish(child).succeeded().json())
        .collect();
    let started = answers
        .iter()
        .filter(|json| json["started"] == true)
        .count();
    assert_eq!(started, 1, "{answers:?}");
    let pid = &answers[0]["pid"];
    assert!(
        answers.iter().all(|json| &json["pid"] == pid),
        "{answers:?}"
    );
    assert_eq!(
        test.daemons(),
        [u32::try_from(pid.as_u64().unwrap()).unwrap()]
    );
}

#[test]
fn a_held_lock_keeps_start_from_starting_a_daemon() {
    let test = Test::new();
    let (socket, lock) = test.paths_of(&mara());
    private_directory(socket.parent().unwrap());
    let held = hold_lock(&lock);
    let starting = test.spawn(&mara(), &["--format", "json", "daemon", "start"], &[]);
    let launcher = starting.id();
    thread::sleep(Duration::from_millis(1500));
    assert_eq!(
        test.daemons(),
        [launcher],
        "a daemon started while the lock was held"
    );
    drop(held);
    let json = finish(starting).succeeded().json();
    assert_eq!(json["started"], true);
    assert_eq!(
        test.daemons(),
        [u32::try_from(json["pid"].as_u64().unwrap()).unwrap()]
    );
}

#[test]
fn start_gives_up_on_a_lock_that_stays_held() {
    let test = Test::new();
    let (socket, lock) = test.paths_of(&mara());
    private_directory(socket.parent().unwrap());
    let _held = hold_lock(&lock);
    let run = test.run(&["daemon", "start"]);
    assert_eq!(run.status.code(), Some(1));
    assert!(
        run.stderr.starts_with("mara: error[io-error]: ") && run.stderr.contains("daemon lock"),
        "{}",
        run.stderr
    );
    assert!(test.daemons().is_empty());
}

#[test]
fn a_killed_daemon_leaves_a_socket_that_the_next_start_replaces() {
    let test = Test::new();
    let (_, killed) = test.start();
    let (socket, _) = test.paths_of(&mara());
    signal(killed, libc::SIGKILL);
    test.wait_for_no_daemon(&[]);
    assert!(socket.exists(), "SIGKILL left no socket");
    assert_eq!(test.status()["running"], false);
    let (started, pid) = test.start();
    assert!(started && pid != killed);
    assert_eq!(test.status()["pid"], pid);
    assert_eq!(test.daemons(), [pid]);
}

#[test]
fn sigterm_ends_the_daemon_and_removes_its_socket() {
    let test = Test::new();
    let (_, pid) = test.start();
    let (socket, _) = test.paths_of(&mara());
    signal(pid, libc::SIGTERM);
    test.wait_for_no_daemon(&[]);
    assert!(!socket.exists(), "the daemon left its socket");
}

#[test]
fn each_build_has_its_own_daemon() {
    let test = Test::new();
    let (one, two) = (test.path().join("one/mara"), test.path().join("two/mara"));
    let now = SystemTime::now();
    for path in [&one, &two] {
        fs::create_dir(path.parent().unwrap()).unwrap();
        copy_binary(path, now);
    }
    let (_, first) = test.start_of(&one, &[]);
    let (_, second) = test.start_of(&two, &[]);
    assert_ne!(first, second);
    assert_eq!(test.status_of(&one)["pid"], first);
    assert_eq!(test.status_of(&two)["pid"], second);

    // Replaced in its place while its daemon runs: same path and size, another time.
    copy_binary(&two, now + Duration::from_secs(3600));
    assert_eq!(test.status_of(&two)["running"], false);
    let (started, third) = test.start_of(&two, &[]);
    assert!(started && third != first && third != second);
    assert_eq!(test.status_of(&two)["pid"], third);
    assert_eq!(test.status_of(&one)["pid"], first);
    let mut daemons = test.daemons();
    daemons.sort_unstable();
    let mut expected = vec![first, second, third];
    expected.sort_unstable();
    assert_eq!(daemons, expected);
}

#[test]
fn a_base_that_names_no_directory_runs_no_daemon_and_creates_nothing() {
    let file_test = Test::with_base(|dir| {
        let base = dir.join("file");
        fs::write(&base, "x").unwrap();
        base
    });
    let missing = Test::with_base(|dir| dir.join("missing"));
    for test in [&missing, &file_test] {
        let run = test.run(&["daemon", "start"]);
        assert_eq!(run.status.code(), Some(1), "{}", run.stderr);
        assert!(
            run.stderr
                .starts_with("mara: error[io-error]: the daemon cannot run")
        );
        let status = test.status();
        assert_eq!(status["running"], false);
        assert!(status["unavailable"].is_string(), "{status}");
        assert_eq!(
            test.run(&["daemon", "stop"]).succeeded().status.code(),
            Some(0)
        );
        assert!(test.daemons().is_empty());
    }
    assert!(!missing.base.exists());
    assert_eq!(fs::read_to_string(&file_test.base).unwrap(), "x");
}

#[test]
fn a_base_too_long_for_the_socket_runs_no_daemon_and_creates_nothing() {
    let test = Test::with_base(|dir| {
        // With `/mara/` and a socket name of 21 bytes, a base of 82 bytes passes 107.
        let base = dir.join("b".repeat(81 - dir.as_os_str().len()));
        private_directory(&base);
        base
    });
    assert_eq!(test.base.as_os_str().len(), 82);
    let run = test.run(&["daemon", "start"]);
    assert_eq!(run.status.code(), Some(1));
    assert!(run.stderr.contains("socket path"), "{}", run.stderr);
    assert!(test.status()["unavailable"].is_string());
    assert!(test.daemons().is_empty());
    assert_eq!(fs::read_dir(&test.base).unwrap().count(), 0);
}

#[test]
fn a_base_of_60_bytes_serves_as_usual() {
    let test = Test::with_base(|dir| {
        let length = dir.as_os_str().len();
        assert!(
            length < 59,
            "the temporary directory `{}` is too long",
            dir.display()
        );
        let base = dir.join("b".repeat(60 - length - 1));
        private_directory(&base);
        base
    });
    assert_eq!(test.base.as_os_str().len(), 60);
    let (started, pid) = test.start();
    assert!(started);
    assert_eq!(test.status()["pid"], pid);
    assert_eq!(test.daemons(), [pid]);
    assert_eq!(
        test.run(&["--format", "json", "daemon", "stop"]).json()["stopped"],
        true
    );
    assert!(test.daemons().is_empty());
}

#[test]
fn a_runtime_directory_that_others_could_reach_is_refused() {
    // A base with group access or a special bit, a base that is a link, and a `mara/` that is
    // a link, has group access or has a special bit: start refuses each, follows no link and
    // creates nothing.
    let mode_base = |mode| {
        Test::with_base(move |dir| {
            let base = dir.join("run");
            private_directory(&base);
            fs::set_permissions(&base, fs::Permissions::from_mode(mode)).unwrap();
            base
        })
    };
    let sticky_base = mode_base(0o1700);
    let setgid_base = mode_base(0o2700);
    let mode_mara = |mode| {
        let test = Test::new();
        private_directory(&test.base.join("mara"));
        fs::set_permissions(test.base.join("mara"), fs::Permissions::from_mode(mode)).unwrap();
        test
    };
    let sticky_mara = mode_mara(0o1700);
    let open_base = Test::with_base(|dir| {
        let base = dir.join("run");
        private_directory(&base);
        fs::set_permissions(&base, fs::Permissions::from_mode(0o750)).unwrap();
        base
    });
    let linked_base = Test::with_base(|dir| {
        private_directory(&dir.join("target"));
        std::os::unix::fs::symlink("target", dir.join("run")).unwrap();
        dir.join("run")
    });
    let linked_mara = Test::new();
    private_directory(&linked_mara.path().join("elsewhere"));
    std::os::unix::fs::symlink("../elsewhere", linked_mara.base.join("mara")).unwrap();
    let open_mara = Test::new();
    private_directory(&open_mara.base.join("mara"));
    fs::set_permissions(
        open_mara.base.join("mara"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    for test in [
        &open_base,
        &sticky_base,
        &setgid_base,
        &linked_base,
        &linked_mara,
        &open_mara,
        &sticky_mara,
    ] {
        let run = test.run(&["daemon", "start"]);
        assert_eq!(run.status.code(), Some(1), "{}", run.stdout);
        assert!(
            run.stderr.starts_with("mara: error[io-error]: "),
            "{}",
            run.stderr
        );
        assert!(test.daemons().is_empty());
    }
    for test in [&sticky_base, &setgid_base] {
        assert!(test.status()["unavailable"].is_string(), "direct mode");
    }
    let run = sticky_mara.run(&["daemon", "status"]);
    assert_eq!(run.status.code(), Some(1));
    assert!(
        run.stderr.contains("has mode 1700, not 0700"),
        "{}",
        run.stderr
    );
    for (test, directory) in [
        (&open_base, open_base.base.clone()),
        (&linked_base, linked_base.path().join("target")),
        (&linked_mara, linked_mara.path().join("elsewhere")),
        (&open_mara, open_mara.base.join("mara")),
        (&sticky_base, sticky_base.base.clone()),
        (&setgid_base, setgid_base.base.clone()),
        (&sticky_mara, sticky_mara.base.join("mara")),
    ] {
        let entries: Vec<_> = fs::read_dir(&directory).unwrap().collect();
        assert!(entries.is_empty(), "{}: {entries:?}", test.base.display());
    }
}

#[test]
fn an_idle_daemon_exits_after_the_idle_period() {
    let test = Test::new();
    let (_, pid) = test.start_of(&mara(), &[("MARA_TEST_IDLE_SECONDS", "2")]);
    let (socket, _) = test.paths_of(&mara());
    let started = Instant::now();
    assert_eq!(test.status()["pid"], pid);
    // Status requests, sent all the while, do not keep it alive.
    while test.status()["running"] == true {
        assert!(started.elapsed() < WAIT, "the idle daemon still runs");
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        started.elapsed() >= Duration::from_millis(1500),
        "the daemon ended before its idle period"
    );
    test.wait_for_no_daemon(&[]);
    assert!(!socket.exists(), "the daemon left its socket");
}

/// `command` with `prepare` run in the child before it executes.
fn with_child_setup(mut command: Command, prepare: fn() -> std::io::Result<()>) -> Command {
    use std::os::unix::process::CommandExt;
    // SAFETY: each `prepare` below calls only async-signal-safe functions.
    unsafe { command.pre_exec(prepare) };
    command
}

/// Sets the umask to 0777 in the child.
fn closed_umask() -> std::io::Result<()> {
    // SAFETY: umask takes no pointer.
    unsafe { libc::umask(0o777) };
    Ok(())
}

#[test]
fn modes_hold_whatever_the_umask_and_a_lock_of_another_mode_is_refused() {
    let test = Test::new();
    let start = |umask: bool| {
        let command = test.command(&mara(), &["--format", "json", "daemon", "start"], &[]);
        let command = if umask {
            with_child_setup(command, closed_umask)
        } else {
            command
        };
        finish(spawn(&mut { command })).succeeded().json()
    };
    let started = start(true);
    assert_eq!(started["started"], true, "{started}");
    let (socket, lock) = test.paths_of(&mara());
    assert_eq!(mode(socket.parent().unwrap()), 0o700);
    assert_eq!(mode(&lock), 0o600);
    assert_eq!(mode(&socket), 0o600);
    assert_eq!(start(false)["started"], false);
    test.run(&["daemon", "stop"]).succeeded();
    // An existing lock, under the same umask.
    assert_eq!(start(true)["started"], true);
    assert_eq!(mode(&socket), 0o600);
    // A socket of another mode is no daemon's: the client refuses it.
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o666)).unwrap();
    let run = test.run(&["daemon", "status"]);
    assert_eq!(run.status.code(), Some(1));
    assert!(
        run.stderr.contains("has mode 0666, not 0600"),
        "{}",
        run.stderr
    );
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    test.run(&["daemon", "stop"]).succeeded();

    // A lock of another mode is refused, not repaired.
    for wrong in [0o700, 0o400] {
        fs::set_permissions(&lock, fs::Permissions::from_mode(wrong)).unwrap();
        let run = test.run(&["daemon", "start"]);
        assert_eq!(run.status.code(), Some(1), "{}", run.stdout);
        assert!(run.stderr.contains("with mode 0600"), "{}", run.stderr);
        assert_eq!(mode(&lock), wrong);
        assert!(test.daemons().is_empty());
    }
}

/// Limits files that the child writes to 0 bytes, and ignores SIGXFSZ, so that a write fails.
fn no_file_size() -> std::io::Result<()> {
    let limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: setrlimit reads the limit; signal takes no pointer.
    unsafe {
        libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
        if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[test]
fn a_start_that_fails_after_binding_leaves_no_socket() {
    let test = Test::new();
    let (socket, _) = test.paths_of(&mara());
    let command = test.command(&mara(), &["daemon", "start"], &[]);
    let run = finish(spawn(&mut with_child_setup(command, no_file_size)));
    assert_eq!(run.status.code(), Some(1), "{}", run.stdout);
    assert!(
        run.stderr.contains("cannot write the lock"),
        "{}",
        run.stderr
    );
    assert!(test.daemons().is_empty());
    assert!(!socket.exists(), "the failed start left its socket");
    assert_eq!(test.status()["running"], false);
}

#[test]
fn a_failed_test_ends_its_daemons_and_only_those() {
    let other = Test::new();
    let (_, kept) = other.start();
    let (sender, receiver) = mpsc::channel();
    let failed = thread::spawn(move || {
        let test = Test::new();
        let (_, pid) = test.start();
        // `daemon stop` cannot find a daemon whose socket is gone.
        fs::remove_file(test.paths_of(&mara()).0).unwrap();
        sender.send((test.base.clone(), pid)).unwrap();
        panic!("the test fails with its daemon running");
    })
    .join();
    assert!(failed.is_err());
    let (base, pid) = receiver.recv().unwrap();
    assert!(Daemons::new(&base, None).running().is_empty(), "{pid} runs");
    assert!(
        fs::read(format!("/proc/{pid}/environ")).map_or(true, |environment| environment.is_empty()),
        "the daemon {pid} runs"
    );
    assert_eq!(other.daemons(), [kept]);
}
