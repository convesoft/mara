//! Running programs: the files a program run reads, the fact relations of a snapshot and the
//! supplied inputs, one JSON file per relation (DES-FACT-BRIDGE in `docs/rules/index.mara.md`),
//! the inputs a command line supplies (DES-REQUEST-INPUTS in `docs/rules/index.mara.md`), and the
//! evaluation by the pinned Soufflé (DES-RULE-EXECUTION in `docs/rules/evaluation.mara.md`).

#[cfg(target_arch = "wasm32")]
pub mod browser;
mod consumer;
mod explain;
mod invariants;
#[cfg(target_os = "linux")]
mod sandbox;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
use std::process::{Command, Stdio};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(not(target_arch = "wasm32"))]
use mara_engine::evaluation::Limit;
use mara_engine::evaluation::{self, Exit, Fault, Limits, Rows};
use mara_engine::facts::{
    CodeFacts, Facts, InputError, Inputs, MARA_DL, Relation, Supplied, file_name,
};
use mara_engine::hash::Hash;
use mara_engine::program::Program;
use mara_engine::work::WorkCounts;

use crate::caller;
#[cfg(not(target_arch = "wasm32"))]
use crate::hooks::{self, Hook};

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

// @mara implements REQ-EXTERNAL-RECORDS-AS-SUPPLIED
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
#[cfg(not(target_arch = "wasm32"))]
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

/// Soufflé in a browser workspace: the package's module, which its Soufflé worker loads
/// ([`browser`]); the name only identifies it.
#[cfg(target_arch = "wasm32")]
pub fn souffle() -> Result<PathBuf, String> {
    Ok(PathBuf::from(browser::MODULE))
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

/// Soufflé's arguments: `--no-preprocessor -l ""`, `-t explain` in provenance mode, and the
/// wrapper, which it reads from its working directory, the private directory.
pub fn arguments(provenance: bool) -> Vec<&'static str> {
    let mut arguments = vec!["--no-preprocessor", "-l", ""];
    if provenance {
        arguments.extend(["-t", "explain"]);
    }
    arguments.push(evaluation::WRAPPER);
    arguments
}

/// What a run of Soufflé printed and wrote: natively the files of its private directory, in a
/// browser workspace what the package's worker gave back ([`browser`]).
pub(crate) trait Left {
    /// At most the first 64 KiB of what it printed on the stream whose file is `name`,
    /// [`evaluation::STDERR`] or [`evaluation::STDOUT`]; nothing when it printed nothing.
    fn printed(&self, name: &str) -> Vec<u8>;
    /// The file at `path` relative to the private directory, such as an output relation's.
    fn read(&self, path: &str) -> io::Result<Vec<u8>>;
}

impl Left for &Path {
    fn printed(&self, name: &str) -> Vec<u8> {
        bounded_read(&self.join(name))
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        fs::read(self.join(path))
    }
}

// @mara implements REQ-PROJECT-AUTHORED-LOGIC
/// Evaluates `run.program` with Soufflé as a child process, in a new private directory under
/// the system's temporary directory that is removed when it ends: the parsed copies of the
/// program's files, the facts and inputs it reads and the generated wrapper, run as
/// `souffle --no-preprocessor -l "" wrapper.dl` there, with an empty environment and within
/// the limits (DES-RULE-EXECUTION). Returns the rows of every output relation, or the fault
/// that makes the outcome unavailable. Counts one subprocess spawn and one program run. In a
/// browser workspace the package's Soufflé worker runs it ([`browser::evaluate`]).
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
struct Started {
    _interrupts: interrupt::Deferred,
    _running: cancel::Running,
}

#[cfg(not(target_arch = "wasm32"))]
impl Started {
    /// Begins an evaluation, or refuses it once the daemon stops. The check follows the count,
    /// so a stopping daemon either sees the evaluation counted and waits for it, or the
    /// evaluation sees the daemon stopping and creates no directory.
    fn begin() -> Result<Started, Fault> {
        let started = Started {
            _running: cancel::Running::begin(),
            _interrupts: interrupt::Deferred::begin(),
        };
        // A pause point for tests (ADR-DEBUG-PAUSE-POINT): before the check of whether the
        // daemon stops.
        let _ = hooks::pause(Hook::PauseEvaluation, "");
        if cancel::cancelled() {
            return Err(host("cannot run Soufflé", &cancel::error()));
        }
        Ok(started)
    }
}

/// The outcome of an evaluation in a browser workspace ([`browser::evaluate`]).
#[cfg(target_arch = "wasm32")]
pub fn evaluate(run: &Run, work: &mut WorkCounts) -> Result<Rows, Fault> {
    browser::evaluate(run, work)
}

/// Runs Soufflé on the prepared private directory `dir` and reads its outcome.
#[cfg(not(target_arch = "wasm32"))]
fn run_in(run: &Run, dir: &Path, work: &mut WorkCounts) -> Result<Rows, Fault> {
    let exit = spawn_and_wait(run.souffle, dir, run.limits, None, work)
        .map_err(|error| host("cannot run Soufflé", &error))?;
    outcome(run, exit, &dir)
}

/// The rows of a run of Soufflé that ended as `exit` and left `left`, or its fault.
fn outcome(run: &Run, exit: Exit, left: &dyn Left) -> Result<Rows, Fault> {
    let stderr = left.printed(evaluation::STDERR);
    #[cfg(target_os = "linux")]
    if !exit.success
        && let Some(problem) = sandbox::library_fault(run.souffle, &stderr)
    {
        return Err(Fault::Host(problem));
    }
    evaluation::interpret(
        run.program,
        exit,
        &left.printed(evaluation::STDOUT),
        &stderr,
        run.limits,
    )?;
    read_rows(left, run)
}

/// A fault of the host, from `error` while doing `what`.
#[cfg(not(target_arch = "wasm32"))]
fn host(what: &str, error: &dyn std::fmt::Display) -> Fault {
    Fault::Host(format!("{what}: {error}"))
}

/// A new private directory, prepared for a run of `run.program`.
#[cfg(not(target_arch = "wasm32"))]
fn private_dir(run: &Run) -> Result<caller::TempDir, Fault> {
    let dir = caller::tempdir("mara-evaluation-")
        .map_err(|error| host("cannot create the evaluation's private directory", &error))?;
    prepare(dir.path(), run)
        .map_err(|error| host("cannot write the evaluation's private directory", &error))?;
    Ok(dir)
}

/// The rows of each output relation of `run`, from the files Soufflé wrote ([`Left`]).
fn read_rows(left: &dyn Left, run: &Run) -> Result<Rows, Fault> {
    let mut rows = Rows::default();
    for relation in run.outputs {
        let bytes = left
            .read(&evaluation::output_file(relation))
            .map_err(|error| Fault::Output {
                relation: (*relation).to_owned(),
                problem: format!("Soufflé wrote no readable file: {error}"),
            })?;
        let read = evaluation::read_output(run.program, relation, &bytes, run.limits)?;
        rows.relations.insert((*relation).to_owned(), read);
    }
    Ok(rows)
}

/// The files of a run's private directory, each by its path there: the private copies, the
/// relations the wrapper reads and the wrapper. The output directory is [`evaluation::OUTPUT_DIR`].
fn private_files<'r>(run: &Run<'r>) -> Vec<(String, Cow<'r, [u8]>)> {
    let read = evaluation::read_relations(run.program, run.relations);
    let inputs: Vec<&str> = read.iter().map(|relation| relation.name.as_str()).collect();
    let mut files: Vec<(String, Cow<[u8]>)> = evaluation::copies(run.program)
        .into_iter()
        .map(|(name, text)| (name, Cow::Owned(text.into_bytes())))
        .collect();
    for relation in &read {
        files.push((
            file_name(&relation.name),
            Cow::Borrowed(relation.json.as_bytes()),
        ));
    }
    let wrapper = evaluation::wrapper(run.program, &inputs, run.outputs);
    files.push((
        evaluation::WRAPPER.to_owned(),
        Cow::Owned(wrapper.into_bytes()),
    ));
    files
}

/// Writes the files of the private directory ([`private_files`]) and its output directory into
/// `dir`.
#[cfg(not(target_arch = "wasm32"))]
fn prepare(dir: &Path, run: &Run) -> io::Result<()> {
    for (name, bytes) in private_files(run) {
        caller::write(&dir.join(name), bytes)?;
    }
    caller::create_dir(&dir.join(evaluation::OUTPUT_DIR))
}

/// The first 64 KiB of the file at `path`; nothing when it cannot be read.
fn bounded_read(path: &Path) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Ok(file) = fs::File::open(path) {
        let _ = file.take(64 << 10).read_to_end(&mut bytes);
    }
    bytes
}

// @mara implements REQ-UNAVAILABLE-NOT-PARTIAL
/// Starts Soufflé on the wrapper in `dir` and waits for it, at most the time limit: the one
/// place where Mara starts a process. Its standard error goes to a file in `dir`. Without a
/// console, so does its standard output, and its standard input is empty; with one, Soufflé
/// runs in provenance mode, the console's commands on its standard input and its replies read
/// from its standard output, both bounded ([`explain::Exchange`]). On Linux it runs in its
/// sandbox; where that cannot be built, it does not start.
#[cfg(not(target_arch = "wasm32"))]
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
    command
        .args(arguments(console.is_some()))
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

#[cfg(not(any(unix, target_arch = "wasm32")))]
fn limit(_: &mut Command, _: Limits) {}

/// The child's soft core limit. For a piped `core_pattern` the kernel treats 1 byte as "start no
/// core handler"; with 0 it still runs the handler, which records the crash without its core. A
/// file pattern writes nothing below a page. A socket pattern ignores the limit.
#[cfg(unix)]
const CORE_LIMIT_BYTES: u64 = 1;

/// Ending the evaluations of a daemon that stops (DES-DAEMON-LIFECYCLE): the daemon blocks the
/// signals that [`interrupt`] takes, so it ends them through this flag instead.
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(any(unix, target_arch = "wasm32")))]
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

#[cfg(not(any(unix, target_arch = "wasm32")))]
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
