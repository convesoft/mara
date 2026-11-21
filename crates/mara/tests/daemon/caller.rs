//! What a request served by the daemon takes from its client's process (DES-DAEMON-LIFECYCLE):
//! files and directories get the modes the client's umask gives them in direct mode, existing
//! files keep theirs, and rule evaluations use the client's temporary directory. Each daemon is
//! started with another umask and `TMPDIR` than its clients have.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::{Test, WAIT, finish, mara, served_by, spawn};

/// `command` with the umask `umask` set in the child.
fn with_umask(mut command: Command, umask: u32) -> Command {
    let umask = umask as libc::mode_t;
    // SAFETY: umask is async-signal-safe and takes no pointer.
    unsafe {
        command.pre_exec(move || {
            libc::umask(umask);
            Ok(())
        })
    };
    command
}

/// Starts the daemon with `umask` and `env`: its process ID.
fn start_daemon(test: &Test, umask: u32, env: &[(&str, &str)]) -> u32 {
    let command = test.command(&mara(), &["--format", "json", "daemon", "start"], env);
    let run = finish(spawn(&mut with_umask(command, umask))).succeeded();
    let json = run.json();
    assert_eq!(json["started"], true);
    u32::try_from(json["pid"].as_u64().unwrap()).unwrap()
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
}

/// Writes that create a directory and a document, and replace an existing document.
const WRITES: [&[&str]; 2] = [
    &[
        "item",
        "create",
        "requirement",
        "docs/new/a.mara.md",
        "--title",
        "Direct",
        "--field",
        "status=draft",
        "--body",
        "Secret",
    ],
    &["item", "move", "REQ-TWO", "docs/moved/m.mara.md"],
];

/// The paths whose modes the writes set or keep.
const WRITTEN: [&str; 5] = [
    "docs/new",
    "docs/new/a.mara.md",
    "docs/moved",
    "docs/moved/m.mara.md",
    "docs/reqs.mara.md",
];

#[test]
fn a_write_through_the_daemon_gives_files_the_modes_of_the_clients_umask() {
    let test = Test::new();
    let pid = start_daemon(&test, 0o022, &[]);
    for (umask, directory, file) in [(0o077, 0o700, 0o600), (0o027, 0o750, 0o640)] {
        let direct = test.project_in(&format!("direct-{umask:o}"));
        let served = test.project_in(&format!("served-{umask:o}"));
        for project in [&direct, &served] {
            let existing = project.join("docs/reqs.mara.md");
            fs::set_permissions(existing, fs::Permissions::from_mode(0o604)).unwrap();
        }
        for args in WRITES {
            let env = [("MARA_NO_DAEMON", "1")];
            let command = test.command_in(&direct, &mara(), args, &env);
            finish(spawn(&mut with_umask(command, umask))).succeeded();
            let command = test.command_in(&served, &mara(), args, &[]);
            served_by(pid, with_umask(command, umask)).succeeded();
        }
        for path in WRITTEN {
            let expected = match path {
                "docs/reqs.mara.md" => 0o604,
                "docs/new" | "docs/moved" => directory,
                _ => file,
            };
            assert_eq!(mode(&direct.join(path)), expected, "{path} in direct mode");
            assert_eq!(
                mode(&served.join(path)),
                expected,
                "{path} through the daemon, umask {umask:o}"
            );
        }
    }
}

/// A configuration with the query `ids`, fast, and `slow`, which runs until its time limit.
fn programs(project: &Path) {
    fs::create_dir(project.join(".mara/rules")).unwrap();
    fs::write(
        project.join(".mara/rules/fast.dl"),
        ".include \"mara.dl\"\n.decl r_id(id: Id)\nr_id(i) :- item(_, _, i, _, _, _).\n",
    )
    .unwrap();
    fs::write(
        project.join(".mara/rules/slow.dl"),
        ".include \"mara.dl\"\n.decl n(x: number)\nn(x) :- x = range(0, 1000).\n\
         .decl bad(m: Mid, c: number)\nbad(m, c) :- item(m, _, _, _, _, _), \
         c = count : { n(w), n(x), n(y), n(z), w + x + y + z < 0 }.\n",
    )
    .unwrap();
    let config = format!(
        "{}\n[rules]\nprograms = [\".mara/rules/*.dl\"]\ntime_limit_seconds = 3\n\n\
         [queries.ids]\nprogram = \".mara/rules/fast.dl\"\nrelation = \"r_id\"\n\
         description = \"The IDs\"\n\n\
         [queries.slow]\nprogram = \".mara/rules/slow.dl\"\nrelation = \"bad\"\n\
         description = \"Never ends in time\"\n",
        crate::CONFIG
    );
    fs::write(project.join(".mara/config.toml"), config).unwrap();
}

/// The private directory of a running evaluation in `temporary`, once Soufflé runs in it, and
/// the process ID of that Soufflé.
fn running_evaluation(temporary: &Path) -> (PathBuf, u32) {
    let deadline = Instant::now() + WAIT;
    loop {
        for process in fs::read_dir("/proc").unwrap().flatten() {
            let Some(pid) = process.file_name().to_str().and_then(|n| n.parse().ok()) else {
                continue;
            };
            let (Ok(exe), Ok(cwd)) = (
                fs::read_link(process.path().join("exe")),
                fs::read_link(process.path().join("cwd")),
            ) else {
                continue;
            };
            if exe.file_name().is_some_and(|name| name == "souffle") && cwd.starts_with(temporary) {
                return (cwd, pid);
            }
        }
        assert!(
            Instant::now() < deadline,
            "no evaluation ran in {temporary:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn an_evaluation_through_the_daemon_uses_the_clients_temporary_directory_and_umask() {
    let test = Test::new();
    let project = test.project();
    programs(&project);
    let (daemon_tmp, client_tmp) = (
        test.path().join("daemon-tmp"),
        test.path().join("client-tmp"),
    );
    for directory in [&daemon_tmp, &client_tmp] {
        crate::private_directory(directory);
    }
    let pid = start_daemon(&test, 0o022, &[("TMPDIR", daemon_tmp.to_str().unwrap())]);
    // The temporary directory of whoever started the daemon is gone.
    fs::remove_dir(&daemon_tmp).unwrap();
    let client = [("TMPDIR", client_tmp.to_str().unwrap())];
    let args = [
        "--debug-stats",
        "--format",
        "json",
        "query",
        "execute",
        "ids",
    ];
    let direct = test.run_in(&project, &args, &[client[0], ("MARA_NO_DAEMON", "1")]);
    assert!(direct.status.success(), "{direct:?}");
    let served = served_by(pid, test.command_in(&project, &mara(), &args, &client));
    assert_eq!(served, direct);

    let command = test.command_in(&project, &mara(), &["query", "execute", "slow"], &client);
    let mut child = spawn(&mut with_umask(command, 0o077));
    let (evaluation, souffle) = running_evaluation(&client_tmp);
    let status = fs::read_to_string(format!("/proc/{souffle}/status")).unwrap();
    // The daemon runs it, with the client's umask.
    assert!(
        status.contains(&format!("\nPPid:\t{pid}\n")),
        "Soufflé's {status}"
    );
    assert!(status.contains("\nUmask:\t0077\n"), "Soufflé's {status}");
    assert_eq!(mode(&evaluation), 0o700);
    for entry in fs::read_dir(&evaluation).unwrap() {
        let path = entry.unwrap().path();
        let expected = if path.is_dir() { 0o700 } else { 0o600 };
        assert_eq!(mode(&path), expected, "{path:?}");
    }
    assert!(child.try_wait().unwrap().is_none());
    let run = finish(child);
    assert_eq!(run.status.code(), Some(1), "{run:?}");
    assert!(run.stderr.contains("program-failed"), "{run:?}");
    assert_eq!(fs::read_dir(&client_tmp).unwrap().count(), 0, "left behind");
}

/// `mara daemon stop` while the daemon runs an evaluation for a read, `args`: the evaluation
/// ends through its own cancellation, its sandboxed Soufflé killed and its directory removed,
/// and the daemon exits long before the time limit; the client reports `daemon-lost`
/// (DES-DAEMON-LIFECYCLE).
fn stop_ends_a_running_evaluation(args: &[&str]) {
    let test = Test::new();
    let project = slow_project(&test);
    let client_tmp = test.path().join("client-tmp");
    crate::private_directory(&client_tmp);
    start_daemon(&test, 0o022, &[]);
    let client = [("TMPDIR", client_tmp.to_str().unwrap())];
    let command = test.command_in(&project, &mara(), args, &client);
    let child = spawn(&mut with_umask(command, 0o022));
    let (evaluation, souffle) = running_evaluation(&client_tmp);
    let started = Instant::now();
    let stopped = test.run(&["daemon", "stop"]).succeeded();
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
    assert!(
        stopped.stdout.starts_with("Stopped the daemon"),
        "{stopped:?}"
    );
    assert!(test.daemons().is_empty(), "the daemon outlived `stop`");
    assert!(
        !Path::new(&format!("/proc/{souffle}")).exists(),
        "Soufflé outlived the daemon"
    );
    assert!(!evaluation.exists(), "the evaluation's directory is left");
    let run = finish(child);
    assert_eq!(run.status.code(), Some(1), "{run:?}");
    assert!(run.stderr.contains("daemon-lost"), "{run:?}");
}

/// A project with the programs of [`programs`], a time limit of 60 s and the invariant
/// `slow-check` over `slow`.
fn slow_project(test: &Test) -> PathBuf {
    let project = test.project();
    programs(&project);
    let config = project.join(".mara/config.toml");
    let text = fs::read_to_string(&config).unwrap();
    let text = text.replace("time_limit_seconds = 3", "time_limit_seconds = 60")
        + "\n[invariants.slow-check]\nprogram = \".mara/rules/slow.dl\"\nrelation = \"bad\"\n\
           severity = \"error\"\nmessage = \"Never ends in time\"\n";
    fs::write(&config, text).unwrap();
    project
}

/// `mara daemon stop` while an evaluation for a read, `args`, has begun and has no private
/// directory yet, held there by `MARA_TEST_PAUSE_EVALUATION` (ADR-DEBUG-PAUSE-POINT): the
/// evaluation creates none, and the daemon exits once it ended (DES-DAEMON-LIFECYCLE).
#[cfg(debug_assertions)]
fn stop_before_an_evaluation_creates_its_directory(args: &[&str]) {
    let test = Test::new();
    let project = slow_project(&test);
    let pauses = test.path().join("pauses");
    fs::create_dir(&pauses).unwrap();
    let paused = pauses.join("paused");
    let client_tmp = test.path().join("client-tmp");
    crate::private_directory(&client_tmp);
    let pause = [("MARA_TEST_PAUSE_EVALUATION", pauses.to_str().unwrap())];
    start_daemon(&test, 0o022, &pause);
    let (socket, _) = test.paths_of(&mara());
    let client = [("TMPDIR", client_tmp.to_str().unwrap())];
    let child = spawn(&mut test.command_in(&project, &mara(), args, &client));
    let deadline = Instant::now() + WAIT;
    while !paused.exists() {
        assert!(Instant::now() < deadline, "no evaluation began");
        thread::sleep(Duration::from_millis(5));
    }
    let mut stop = test.spawn(&mara(), &["daemon", "stop"], &[]);
    // The daemon cancels its evaluations before it removes its socket.
    while fs::symlink_metadata(&socket).is_ok() {
        assert!(Instant::now() < deadline, "the daemon did not stop");
        thread::sleep(Duration::from_millis(5));
    }
    let created = Creations::watch(&client_tmp);
    // The paused evaluation goes on, and so does any that begins after it.
    while stop.try_wait().unwrap().is_none() {
        let _ = fs::remove_file(&paused);
        assert!(Instant::now() < deadline, "the daemon did not exit");
        thread::sleep(Duration::from_millis(5));
    }
    let stopped = finish(stop).succeeded();
    assert!(
        stopped.stdout.starts_with("Stopped the daemon"),
        "{stopped:?}"
    );
    assert!(test.daemons().is_empty(), "the daemon outlived `stop`");
    assert!(
        !created.any(),
        "a cancelled evaluation created its directory"
    );
    let left: Vec<_> = fs::read_dir(&client_tmp).unwrap().collect();
    assert!(left.is_empty(), "left behind: {left:?}");
    let run = finish(child);
    assert_eq!(run.status.code(), Some(1), "{run:?}");
    assert!(
        run.stderr.contains("daemon-lost") || run.stderr.contains("the daemon stops"),
        "{run:?}"
    );
}

/// The entries created in a directory since the watch began (inotify): one created and removed
/// again counts too.
#[cfg(debug_assertions)]
struct Creations(fs::File);

#[cfg(debug_assertions)]
impl Creations {
    fn watch(directory: &Path) -> Creations {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        use std::os::unix::ffi::OsStrExt;
        // SAFETY: inotify_init1 takes no pointer.
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        assert!(fd >= 0, "inotify: {}", std::io::Error::last_os_error());
        // SAFETY: inotify_init1 returned a new descriptor, owned by nothing else.
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        let path = std::ffi::CString::new(directory.as_os_str().as_bytes()).unwrap();
        // SAFETY: the path is NUL-terminated and outlives the call.
        let watch =
            unsafe { libc::inotify_add_watch(fd.as_raw_fd(), path.as_ptr(), libc::IN_CREATE) };
        assert!(watch >= 0, "inotify: {}", std::io::Error::last_os_error());
        Creations(fs::File::from(fd))
    }

    /// Whether any entry was created.
    fn any(&self) -> bool {
        use std::io::Read;
        let mut buffer = [0; 4096];
        match (&self.0).read(&mut buffer) {
            Ok(read) => read > 0,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => false,
            Err(error) => panic!("inotify: {error}"),
        }
    }
}

#[test]
#[cfg(debug_assertions)]
fn stop_before_an_evaluation_creates_its_directory_leaves_none() {
    stop_before_an_evaluation_creates_its_directory(&["query", "execute", "slow"]);
}

/// The provenance run of `query report` checks the same.
#[test]
#[cfg(debug_assertions)]
fn stop_before_a_report_creates_its_directory_leaves_none() {
    stop_before_an_evaluation_creates_its_directory(&[
        "query",
        "report",
        "slow-check",
        "--id",
        "REQ-ONE",
    ]);
}

#[test]
fn stop_ends_a_running_evaluation_and_leaves_no_souffle() {
    stop_ends_a_running_evaluation(&["query", "execute", "slow"]);
}

/// The provenance run of `query report` ends the same way.
#[test]
fn stop_ends_a_running_report_and_leaves_no_souffle() {
    stop_ends_a_running_evaluation(&["query", "report", "slow-check", "--id", "REQ-ONE"]);
}
