//! Running programs: the files a program run reads, the fact relations of a snapshot and the
//! supplied inputs, one JSON file per relation (DES-FACT-BRIDGE in `docs/rules/index.mara.md`),
//! the inputs a command line supplies (DES-REQUEST-INPUTS in `docs/rules/index.mara.md`), and the
//! evaluation by the pinned Soufflé (DES-RULE-EXECUTION in `docs/rules/evaluation.mara.md`).

mod consumer;
mod invariants;

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use mara_engine::evaluation::{self, Exit, Fault, Limit, Limits, Rows};
use mara_engine::facts::{Facts, InputError, Inputs, MARA_DL, Relation, Supplied, file_name};
use mara_engine::hash::Hash;
use mara_engine::program::Program;
use mara_engine::work::WorkCounts;

/// Writes `mara.dl`, every fact relation of `facts` and every relation of `inputs` into `dir`,
/// the evaluation's private directory, each relation to the file [`file_name`] names. The JSON
/// text was rendered once with the snapshot's facts and is written as is.
pub fn write_facts(dir: &Path, facts: &Facts, inputs: &Inputs) -> io::Result<()> {
    fs::write(dir.join("mara.dl"), MARA_DL)?;
    let relations = facts.relations.iter().chain(&inputs.relations);
    for relation in relations.chain([&inputs.source_complete]) {
        fs::write(dir.join(file_name(&relation.name)), &relation.json)?;
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

pub use consumer::{Consumed, consume};
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
    let host = |what: &str, error: &dyn std::fmt::Display| Fault::Host(format!("{what}: {error}"));
    // Declared before the directory, so it ends Mara for a deferred signal only after the
    // directory is removed.
    let _interrupts = interrupt::Deferred::begin();
    let dir = tempfile::Builder::new()
        .prefix("mara-evaluation-")
        .tempdir()
        .map_err(|error| host("cannot create the evaluation's private directory", &error))?;
    let read = evaluation::read_relations(run.program, run.relations);
    let inputs: Vec<&str> = read.iter().map(|relation| relation.name.as_str()).collect();
    prepare(dir.path(), run, &read, &inputs)
        .map_err(|error| host("cannot write the evaluation's private directory", &error))?;
    let exit = spawn_and_wait(run.souffle, dir.path(), run.limits, work)
        .map_err(|error| host("cannot run Soufflé", &error))?;
    let printed = |name: &str| bounded_read(&dir.path().join(name));
    evaluation::interpret(
        run.program,
        exit,
        &printed(evaluation::STDOUT),
        &printed(evaluation::STDERR),
        run.limits,
    )?;
    let mut rows = Rows::default();
    for relation in run.outputs {
        let path = dir.path().join(evaluation::output_file(relation));
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
        fs::write(dir.join(name), text)?;
    }
    for relation in read {
        fs::write(dir.join(file_name(&relation.name)), &relation.json)?;
    }
    fs::create_dir(dir.join(evaluation::OUTPUT_DIR))?;
    let wrapper = evaluation::wrapper(run.program, inputs, run.outputs);
    fs::write(dir.join(evaluation::WRAPPER), wrapper)
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
/// place where Mara starts a process. Its standard output and error go to files in `dir`.
fn spawn_and_wait(
    souffle: &Path,
    dir: &Path,
    limits: Limits,
    work: &mut WorkCounts,
) -> io::Result<Exit> {
    let mut command = Command::new(souffle);
    command
        .args(["--no-preprocessor", "-l", "", evaluation::WRAPPER])
        .current_dir(dir)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(fs::File::create(dir.join(evaluation::STDOUT))?)
        .stderr(fs::File::create(dir.join(evaluation::STDERR))?);
    limit(&mut command, limits);
    let mut child = command.spawn()?;
    work.subprocess_spawns += 1;
    work.program_runs += 1;
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
        if started.elapsed() >= deadline {
            child.kill()?;
            child.wait()?;
            return Ok(Exit {
                success: false,
                limit: Some(Limit::Time),
            });
        }
        std::thread::sleep(pause.min(deadline.saturating_sub(started.elapsed())));
        pause = (pause * 2).min(Duration::from_millis(2));
    };
    Ok(Exit {
        success: status.success(),
        limit: output_limit(&status),
    })
}

/// Bounds the child's address space and the size of each file it writes, and keeps it from
/// writing a core file. Past the file size it receives `SIGXFSZ`, whose default ends it.
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
            if unsafe { libc::getppid() } as u32 != parent {
                return Err(io::Error::other("Mara ended before Soufflé started"));
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = parent;
        set(libc::RLIMIT_AS, limits.memory_bytes)?;
        set(libc::RLIMIT_FSIZE, limits.output_bytes)?;
        set(libc::RLIMIT_CORE, 0)?;
        // SAFETY: restoring a signal's default action is async-signal-safe.
        unsafe { libc::signal(libc::SIGXFSZ, libc::SIG_DFL) };
        Ok(())
    };
    // SAFETY: the closure calls only async-signal-safe functions and allocates nothing.
    unsafe { command.pre_exec(pre_exec) };
}

#[cfg(not(unix))]
fn limit(_: &mut Command, _: Limits) {}

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
