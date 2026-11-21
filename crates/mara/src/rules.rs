//! Running programs: the files a program run reads, the fact relations of a snapshot and the
//! supplied inputs, one JSON file per relation (DES-FACT-BRIDGE in `docs/rules/index.mara.md`),
//! the inputs a command line supplies (DES-REQUEST-INPUTS in `docs/rules/index.mara.md`), and the
//! evaluation by the pinned Soufflé (DES-RULE-EXECUTION in `docs/rules/evaluation.mara.md`).

mod consumer;
mod explain;
mod invariants;
#[cfg(target_os = "linux")]
mod sandbox;

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use mara_engine::evaluation::{self, Exit, Fault, Limit, Limits, Rows};
use mara_engine::facts::{
    CodeFacts, Facts, InputError, Inputs, MARA_DL, Relation, Supplied, file_name,
};
use mara_engine::hash::Hash;
use mara_engine::program::Program;
use mara_engine::work::WorkCounts;

use crate::caller;

/// Writes `mara.dl`, every fact relation of `facts`, every relation of `inputs` and an empty
/// `code_link` into `dir`, the evaluation's private directory, each relation to the file
/// [`file_name`] names. The JSON text was rendered once with the snapshot's facts and is written
/// as is.
pub fn write_facts(dir: &Path, facts: &Facts, inputs: &Inputs) -> io::Result<()> {
    caller::write(&dir.join("mara.dl"), MARA_DL)?;
    let relations = facts.relations.iter().chain(&inputs.relations);
    let no_code = CodeFacts::unread().links;
    for relation in relations.chain([&inputs.source_complete, &no_code]) {
        caller::write(&dir.join(file_name(&relation.name)), &relation.json)?;
    }
    Ok(())
}

/// The input that one `--input` argument supplies: `<name>=<value>` for one value inline, or
/// `<name>=@<file>` for a JSON file of rows, relative to `base`, the directory the command runs
/// in. A file that cannot be read or is not JSON refuses the request. Counts a file read.
pub fn supplied_input(
    argument: &str,
    base: &Path,
    work: &mut WorkCounts,
) -> Result<(String, Supplied), InputError> {
    let Some((name, value)) = argument
        .split_once('=')
        .filter(|(name, _)| !name.is_empty())
    else {
        return Err(InputError::Invalid(format!(
            "`--input {argument}` names no input: write `<name>=<value>` or `<name>=@<file>`"
        )));
    };
    let Some(file) = value.strip_prefix('@') else {
        return Ok((name.to_owned(), Supplied::Inline(value.to_owned())));
    };
    let refuse = |problem: String| InputError::Invalid(format!("input `{name}`: {file} {problem}"));
    let bytes =
        fs::read(base.join(file)).map_err(|error| refuse(format!("cannot be read: {error}")))?;
    work.file_reads += 1;
    let rows =
        serde_json::from_slice(&bytes).map_err(|error| refuse(format!("is not JSON: {error}")))?;
    Ok((name.to_owned(), Supplied::Rows(rows)))
}

/// The pinned Soufflé (DES-SOUFFLE-BUILD in `docs/distribution.mara.md`): `souffle` beside the
/// `mara` binary, as a package installs them, else `../souffle-2.5/souffle` from it, where
/// `scripts/build-souffle.sh` puts it for a binary that Cargo built in the same `target/`.
/// Never a `souffle` on `PATH`, which could be another version.
pub fn souffle() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| {
        format!("cannot find the `mara` binary to find Soufflé beside it: {error}")
    })?;
    let dir = exe.parent().unwrap_or(Path::new("/"));
    let candidates = [
        dir.join("souffle"),
        dir.join(format!("../souffle-{SOUFFLE_VERSION}/souffle")),
    ];
    candidates
        .iter()
        .find(|path| path.is_file())
        .cloned()
        .ok_or_else(|| {
            format!(
                "Mara cannot find its Soufflé {SOUFFLE_VERSION}: neither {} nor {} exists; in a \
                 checkout of Mara, run scripts/build-souffle.sh",
                candidates[0].display(),
                candidates[1].display()
            )
        })
}

/// The Soufflé release Mara runs.
pub const SOUFFLE_VERSION: &str = "2.5";

/// One evaluation of an admitted program.
#[derive(Clone, Copy, Debug)]
pub struct Run<'a> {
    /// The pinned Soufflé ([`souffle`]).
    pub souffle: &'a Path,
    pub program: &'a Program,
    /// The fact relations and the supplied inputs. The wrapper reads those the program
    /// declares.
    pub relations: &'a [&'a Relation],
    /// The relations whose rows the run returns.
    pub outputs: &'a [&'a str],
    pub limits: Limits,
}

pub use consumer::{Consumed, consume, consume_explained};
pub use explain::{Explained, explain};
pub use invariants::{Invariants, Scope, invariants};

/// The evaluations of one request by their key ([`evaluation::key`]): every consumer of a
/// program with the same inputs reads the same run.
#[derive(Debug, Default)]
pub struct Evaluations(HashMap<Hash, Result<Rows, Fault>>);

impl Evaluations {
    /// The outcome of `run`, whose key is `key`, evaluated unless it already was.
    pub fn evaluate(
        &mut self,
        key: Hash,
        run: &Run,
        work: &mut WorkCounts,
    ) -> &Result<Rows, Fault> {
        self.0.entry(key).or_insert_with(|| evaluate(run, work))
    }
}

/// Evaluates `run.program` with Soufflé as a child process, in a new private directory under
/// the system's temporary directory that is removed when it ends: the parsed copies of the
/// program's files, the facts and inputs it reads and the generated wrapper, run as
/// `souffle --no-preprocessor -l "" wrapper.dl` there, with an empty environment and within
/// the limits (DES-RULE-EXECUTION). Returns the rows of every output relation, or the fault
/// that makes the outcome unavailable. Counts one subprocess spawn and one program run.
pub fn evaluate(run: &Run, work: &mut WorkCounts) -> Result<Rows, Fault> {
    // Declared before the directory, so that it is dropped after it.
    let _started = Started::begin()?;
    let dir = private_dir(run)?;
    run_in(run, dir.path(), work)
}

/// An evaluation that began, before its private directory exists: counted until it is dropped,
/// so that a stopping daemon waits for it (DES-DAEMON-LIFECYCLE), and with the signals deferred.
/// Dropped after the directory, it ends Mara for a deferred signal only once the directory is
/// removed.
struct Started {
    _interrupts: interrupt::Deferred,
    _running: cancel::Running,
}

impl Started {
    /// Begins an evaluation, or refuses it once the daemon stops. The check follows the count,
    /// so a stopping daemon either sees the evaluation counted and waits for it, or the
    /// evaluation sees the daemon stopping and creates no directory.
    fn begin() -> Result<Started, Fault> {
        let started = Started {
            _running: cancel::Running::begin(),
            _interrupts: interrupt::Deferred::begin(),
        };
        pause_evaluation();
        if cancel::cancelled() {
            return Err(host("cannot run Soufflé", &cancel::error()));
        }
        Ok(started)
    }
}

/// A pause point for tests, in debug builds only (ADR-DEBUG-PAUSE-POINT in
/// `docs/authoring/writes.mara.md`): when `MARA_TEST_PAUSE_EVALUATION` names a directory, an
/// evaluation that began creates `paused` there and waits until it is gone, before it checks
/// whether the daemon stops.
#[cfg(debug_assertions)]
fn pause_evaluation() {
    let Some(directory) =
        std::env::var_os("MARA_TEST_PAUSE_EVALUATION").filter(|value| !value.is_empty())
    else {
        return;
    };
    let paused = Path::new(&directory).join("paused");
    if fs::write(&paused, "").is_err() {
        return;
    }
    while fs::symlink_metadata(&paused).is_ok() {
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(not(debug_assertions))]
fn pause_evaluation() {}

/// Runs Soufflé on the prepared private directory `dir` and reads its outcome.
fn run_in(run: &Run, dir: &Path, work: &mut WorkCounts) -> Result<Rows, Fault> {
    let exit = spawn_and_wait(run.souffle, dir, run.limits, None, work)
        .map_err(|error| host("cannot run Soufflé", &error))?;
    let printed = |name: &str| bounded_read(&dir.join(name));
    let stderr = printed(evaluation::STDERR);
    #[cfg(target_os = "linux")]
    if !exit.success
        && let Some(problem) = sandbox::library_fault(run.souffle, &stderr)
    {
        return Err(Fault::Host(problem));
    }
    evaluation::interpret(
        run.program,
        exit,
        &printed(evaluation::STDOUT),
        &stderr,
        run.limits,
    )?;
    read_rows(dir, run)
}

/// A fault of the host, from `error` while doing `what`.
fn host(what: &str, error: &dyn std::fmt::Display) -> Fault {
    Fault::Host(format!("{what}: {error}"))
}

/// A new private directory, prepared for a run of `run.program`.
fn private_dir(run: &Run) -> Result<tempfile::TempDir, Fault> {
    let dir = caller::tempdir("mara-evaluation-")
        .map_err(|error| host("cannot create the evaluation's private directory", &error))?;
    let read = evaluation::read_relations(run.program, run.relations);
    let inputs: Vec<&str> = read.iter().map(|relation| relation.name.as_str()).collect();
    prepare(dir.path(), run, &read, &inputs)
        .map_err(|error| host("cannot write the evaluation's private directory", &error))?;
    Ok(dir)
}

/// The rows of each output relation of `run`, from the files Soufflé wrote in `dir`.
fn read_rows(dir: &Path, run: &Run) -> Result<Rows, Fault> {
    let mut rows = Rows::default();
    for relation in run.outputs {
        let path = dir.join(evaluation::output_file(relation));
        let bytes = fs::read(&path).map_err(|error| Fault::Output {
            relation: (*relation).to_owned(),
            problem: format!("Soufflé wrote no readable file: {error}"),
        })?;
        let read = evaluation::read_output(run.program, relation, &bytes, run.limits)?;
        rows.relations.insert((*relation).to_owned(), read);
    }
    Ok(rows)
}

/// Writes the private copies, the relations the wrapper reads, the output directory and the
/// wrapper into `dir`.
fn prepare(dir: &Path, run: &Run, read: &[&Relation], inputs: &[&str]) -> io::Result<()> {
    for (name, text) in evaluation::copies(run.program) {
        caller::write(&dir.join(name), text)?;
    }
    for relation in read {
        caller::write(&dir.join(file_name(&relation.name)), &relation.json)?;
    }
    caller::create_dir(&dir.join(evaluation::OUTPUT_DIR))?;
    let wrapper = evaluation::wrapper(run.program, inputs, run.outputs);
    caller::write(&dir.join(evaluation::WRAPPER), wrapper)
}

/// The first 64 KiB of the file at `path`; nothing when it cannot be read.
fn bounded_read(path: &Path) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Ok(file) = fs::File::open(path) {
        let _ = file.take(64 << 10).read_to_end(&mut bytes);
    }
    bytes
}

/// Starts Soufflé on the wrapper in `dir` and waits for it, at most the time limit: the one
/// place where Mara starts a process. Its standard error goes to a file in `dir`. Without a
/// console, so does its standard output, and its standard input is empty; with one, Soufflé
/// runs in provenance mode, the console's commands on its standard input and its replies read
/// from its standard output, both bounded ([`explain::Exchange`]). On Linux it runs in its
/// sandbox; where that cannot be built, it does not start.
fn spawn_and_wait(
    souffle: &Path,
    dir: &Path,
    limits: Limits,
    console: Option<&mut dyn explain::Console>,
    work: &mut WorkCounts,
) -> io::Result<Exit> {
    if cancel::cancelled() {
        return Err(cancel::error());
    }
    let mut command = Command::new(souffle);
    command.args(["--no-preprocessor", "-l", ""]);
    if console.is_some() {
        command.args(["-t", "explain"]);
    }
    command
        .arg(evaluation::WRAPPER)
        .current_dir(dir)
        .env_clear()
        .stderr(caller::create_new(&dir.join(evaluation::STDERR))?);
    match console {
        Some(_) => command.stdin(Stdio::piped()).stdout(Stdio::piped()),
        None => command
            .stdin(Stdio::null())
            .stdout(caller::create_new(&dir.join(evaluation::STDOUT))?),
    };
    limit(&mut command, limits);
    #[cfg(target_os = "linux")]
    let sandbox = sandbox::Sandbox::new(souffle, dir)?;
    #[cfg(target_os = "linux")]
    sandbox.apply(&mut command);
    #[cfg(target_os = "linux")]
    let spawned = command.spawn().map_err(sandbox::setup_error);
    #[cfg(not(target_os = "linux"))]
    let spawned = command.spawn();
    let mut child = spawned?;
    #[cfg(target_os = "linux")]
    drop(sandbox);
    work.subprocess_spawns += 1;
    work.program_runs += 1;
    let mut exchange = match console.map(|console| explain::Exchange::start(&mut child, console)) {
        None => None,
        Some(Ok(exchange)) => Some(exchange),
        Some(Err(error)) => {
            child.kill()?;
            child.wait()?;
            return Err(error);
        }
    };
    let started = Instant::now();
    let deadline = Duration::from_secs(limits.time_seconds.into());
    let mut pause = Duration::from_micros(50);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if interrupt::pending() {
            child.kill()?;
            child.wait()?;
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Mara was interrupted",
            ));
        }
        if cancel::cancelled() {
            child.kill()?;
            child.wait()?;
            return Err(cancel::error());
        }
        if started.elapsed() >= deadline {
            child.kill()?;
            child.wait()?;
            return Ok(Exit {
                success: false,
                limit: Some(Limit::Time),
            });
        }
        let wait = pause.min(deadline.saturating_sub(started.elapsed()));
        match &mut exchange {
            Some(exchange) => exchange.wait(wait),
            None => std::thread::sleep(wait),
        }
        pause = (pause * 2).min(Duration::from_millis(2));
    };
    Ok(Exit {
        success: status.success(),
        limit: output_limit(&status),
    })
}

/// Bounds the child's address space and the size of each file it writes. Past the file size it
/// receives `SIGXFSZ`, whose default ends it; past the address space Soufflé aborts. A soft core
/// limit of exactly 1 byte, not 0, keeps either end from being recorded as a crash where
/// `core_pattern` pipes to a handler such as systemd-coredump or names a file. A crash record
/// remains where `core_pattern` names a socket (`@@…`), whose handler the kernel starts at any
/// core limit, and where the inherited hard core limit is 0 and the pattern is a pipe
/// (ADR-LIMIT-STOPS-NOT-CRASHES in `docs/rules/decisions.mara.md`).
#[cfg(unix)]
fn limit(command: &mut Command, limits: Limits) {
    use std::os::unix::process::CommandExt;
    let set = |resource, bytes: u64| {
        let limit = libc::rlimit {
            rlim_cur: bytes as libc::rlim_t,
            rlim_max: bytes as libc::rlim_t,
        };
        // SAFETY: `setrlimit` only reads the struct it is given.
        if unsafe { libc::setrlimit(resource, &limit) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    };
    let parent = std::process::id();
    // The files Soufflé writes get the modes the client's umask gives them.
    let umask = caller::umask();
    let pre_exec = move || {
        // The child is killed when the thread that started it ends, however Mara ends; a
        // parent that ended before this call is no longer its parent.
        #[cfg(target_os = "linux")]
        {
            // SAFETY: `prctl` with these arguments only sets the calling process's attribute.
            if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } != 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: `getppid` has no preconditions.
            // `ESRCH`, no such process: an error that allocates nothing, which no one reads,
            // since Mara ended.
            if unsafe { libc::getppid() } as u32 != parent {
                return Err(io::Error::from_raw_os_error(libc::ESRCH));
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = parent;
        set(libc::RLIMIT_AS, limits.memory_bytes)?;
        set(libc::RLIMIT_FSIZE, limits.output_bytes)?;
        // The soft core limit only, and only where the hard limit allows it: raising a hard
        // limit of 0 fails without privilege, and the core limit never fails an evaluation.
        let mut core = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: `getrlimit` writes only the struct it is given; `setrlimit` only reads it.
        unsafe {
            if libc::getrlimit(libc::RLIMIT_CORE, &mut core) == 0
                && core.rlim_max >= CORE_LIMIT_BYTES as libc::rlim_t
            {
                core.rlim_cur = CORE_LIMIT_BYTES as libc::rlim_t;
                libc::setrlimit(libc::RLIMIT_CORE, &core);
            }
        }
        // SAFETY: restoring a signal's default action is async-signal-safe.
        unsafe { libc::signal(libc::SIGXFSZ, libc::SIG_DFL) };
        if let Some(umask) = umask {
            // SAFETY: umask is async-signal-safe; this process runs one thread.
            unsafe { libc::umask(umask as libc::mode_t) };
        }
        // The daemon blocks the signals that stop it in every thread, and a child inherits the
        // mask: Soufflé gets none blocked.
        // SAFETY: sigemptyset and sigprocmask are async-signal-safe and take a set on the stack.
        unsafe {
            let mut none = std::mem::zeroed::<libc::sigset_t>();
            libc::sigemptyset(&mut none);
            if libc::sigprocmask(libc::SIG_SETMASK, &none, std::ptr::null_mut()) != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    };
    // SAFETY: the closure calls only async-signal-safe functions and allocates nothing.
    unsafe { command.pre_exec(pre_exec) };
}

#[cfg(not(unix))]
fn limit(_: &mut Command, _: Limits) {}

/// The child's soft core limit. For a piped `core_pattern` the kernel treats 1 byte as "start no
/// core handler"; with 0 it still runs the handler, which records the crash without its core. A
/// file pattern writes nothing below a page. A socket pattern ignores the limit.
#[cfg(unix)]
const CORE_LIMIT_BYTES: u64 = 1;

/// Ending the evaluations of a daemon that stops (DES-DAEMON-LIFECYCLE): the daemon blocks the
/// signals that [`interrupt`] takes, so it ends them through this flag instead.
pub mod cancel {
    use std::io;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    static CANCELLED: AtomicBool = AtomicBool::new(false);
    static RUNNING: AtomicUsize = AtomicUsize::new(0);

    /// Ends every evaluation of this process that runs, and every later one, each with its
    /// child process killed and its private directory removed.
    pub fn all() {
        CANCELLED.store(true, Ordering::SeqCst);
    }

    /// How many evaluations have not yet ended: an evaluation counts until its child has ended
    /// and its private directory is removed.
    pub fn running() -> usize {
        RUNNING.load(Ordering::SeqCst)
    }

    pub(super) fn cancelled() -> bool {
        CANCELLED.load(Ordering::SeqCst)
    }

    pub(super) fn error() -> io::Error {
        io::Error::new(io::ErrorKind::Interrupted, "the daemon stops")
    }

    pub(super) struct Running;

    impl Running {
        pub(super) fn begin() -> Running {
            RUNNING.fetch_add(1, Ordering::SeqCst);
            Running
        }
    }

    impl Drop for Running {
        fn drop(&mut self) {
            RUNNING.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// SIGINT, SIGTERM and SIGHUP during an evaluation: the evaluation stops, its private directory
/// is removed, then Mara ends by the signal's default action. At any other time the signal acts
/// as by default.
#[cfg(unix)]
mod interrupt {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, OnceLock};

    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};

    /// Whether no evaluation runs, and the signal received during one, 0 for none.
    struct State {
        idle: Arc<AtomicBool>,
        signal: Arc<AtomicUsize>,
    }

    fn state() -> &'static State {
        static STATE: OnceLock<State> = OnceLock::new();
        STATE.get_or_init(|| {
            let state = State {
                idle: Arc::new(AtomicBool::new(true)),
                signal: Arc::new(AtomicUsize::new(0)),
            };
            for signal in [SIGINT, SIGTERM, SIGHUP] {
                // Without the handlers a signal keeps its default action, so failing to
                // register one only loses the removal of the directory.
                let _ = signal_hook::flag::register_conditional_default(
                    signal,
                    Arc::clone(&state.idle),
                );
                let _ = signal_hook::flag::register_usize(
                    signal,
                    Arc::clone(&state.signal),
                    signal as usize,
                );
            }
            state
        })
    }

    /// Defers the signals while it lives; dropping it acts on one received meanwhile.
    pub(super) struct Deferred;

    impl Deferred {
        pub(super) fn begin() -> Deferred {
            state().idle.store(false, Ordering::SeqCst);
            Deferred
        }
    }

    impl Drop for Deferred {
        fn drop(&mut self) {
            let state = state();
            state.idle.store(true, Ordering::SeqCst);
            let signal = state.signal.swap(0, Ordering::SeqCst);
            if signal != 0 {
                let _ = signal_hook::low_level::emulate_default_handler(signal as i32);
            }
        }
    }

    /// Whether a signal arrived during the evaluation.
    pub(super) fn pending() -> bool {
        state().signal.load(Ordering::SeqCst) != 0
    }
}

#[cfg(not(unix))]
mod interrupt {
    pub(super) struct Deferred;

    impl Deferred {
        pub(super) fn begin() -> Deferred {
            Deferred
        }
    }

    pub(super) fn pending() -> bool {
        false
    }
}

/// Whether the system ended the child for a file past the output-size limit.
#[cfg(unix)]
fn output_limit(status: &std::process::ExitStatus) -> Option<Limit> {
    use std::os::unix::process::ExitStatusExt;
    (status.signal() == Some(libc::SIGXFSZ)).then_some(Limit::Output)
}

#[cfg(not(unix))]
fn output_limit(_: &std::process::ExitStatus) -> Option<Limit> {
    None
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    /// A soft core limit of 0 would still leave a crash record where `core_pattern` is a pipe;
    /// the hard limit stays as inherited.
    #[test]
    fn the_child_gets_a_soft_core_limit_of_one_byte() {
        let mut command = Command::new("cat");
        command.arg("/proc/self/limits");
        limit(&mut command, Limits::with_time(10));
        let output = command.output().expect("run cat");
        let limits = String::from_utf8(output.stdout).unwrap();
        let core = limits
            .lines()
            .find(|line| line.starts_with("Max core file size"))
            .expect("a core limit");
        let mut inherited = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: `getrlimit` writes only the struct it is given.
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut inherited) },
            0
        );
        let hard = match inherited.rlim_max {
            0 => return,
            libc::RLIM_INFINITY => "unlimited".to_owned(),
            bytes => bytes.to_string(),
        };
        assert_eq!(
            core.split_whitespace().skip(4).collect::<Vec<_>>(),
            ["1", hard.as_str(), "bytes"],
            "{core}"
        );
    }
}
