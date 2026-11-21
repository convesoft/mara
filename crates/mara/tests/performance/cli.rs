//! Runs the real `mara` binary with `--debug-stats`, and lists the operation cases that the
//! counter and scaling tests run.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;

use mara_engine::work::WorkCounts;
use serde_json::{Map, Value};

use crate::corpus::{CANDIDATE, Corpus, ITEM_VALIDATION_RUNS, RESULTS, VALIDATION_RUNS};
use crate::cpu_time;

/// One invocation of an operation on a corpus: a case of DES-PERFORMANCE-HARNESS.
pub struct Case {
    pub args: Vec<String>,
    /// The exit status on a generated corpus.
    pub status: i32,
    /// The inputs it reads, which set its cold work counts.
    pub reads: Reads,
    /// The code files it reads.
    pub code: Code,
}

/// The code files an operation reads (DES-CODE-READERS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    None,
    /// Every one, each read and hashed once and each distinct content scanned once.
    All,
    /// One, read and hashed, not scanned at the basic level: `get` of a code location.
    One,
}

/// What an operation reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reads {
    /// The configuration and the schema.
    Schema,
    /// The configuration, the schema, the `.gitignore` files, every document and every program,
    /// which it parses into a snapshot and checks, and `runs` program runs.
    Checked { runs: u64 },
    /// The same inputs into a snapshot, without checks or program runs.
    Documents,
    /// The same inputs into a snapshot, then one run of a program of the corpus, and the files
    /// its `--input` arguments name.
    Evaluates { input_files: u64 },
    /// The same inputs, then a write: the files it writes are restored after each run
    /// (`restore`).
    Writes,
}

/// Each operation's case on `corpus`. An operation adds its case here, and both tests run it.
pub fn cases(corpus: &Corpus) -> Vec<Case> {
    let with_code = |args: &[&str], reads, code| Case {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        status: 0,
        reads,
        code,
    };
    let case = |args: &[&str], reads| with_code(args, reads, Code::None);
    let renamed = format!("{}-RENAMED", corpus.first_id);
    // Each single-file write changes a document of the first two clusters, the same at every
    // size, and no two of them the same document.
    let id = |index: usize| corpus.ids[index].0.as_str();
    let path = |index: usize| corpus.ids[index].1.as_str();
    let (second, untargeted) = (25, corpus.untargeted);
    let candidate = format!("candidate={CANDIDATE}");
    let results = format!("verification_result=@{RESULTS}");
    let line = format!("{}:2", corpus.code_files[0]);
    vec![
        with_code(
            &["project", "validate"],
            Reads::Checked {
                runs: VALIDATION_RUNS,
            },
            Code::All,
        ),
        // The corpus is valid, so fix writes nothing.
        case(&["project", "fix"], Reads::Documents),
        case(
            &["item", "validate", &corpus.first_id],
            Reads::Checked {
                runs: ITEM_VALIDATION_RUNS,
            },
        ),
        case(&["schema", "validate"], Reads::Schema),
        case(&["search", "cache"], Reads::Documents),
        case(&["get", &corpus.first_id], Reads::Documents),
        with_code(
            &["relation", "list", &corpus.first_id],
            Reads::Documents,
            Code::All,
        ),
        with_code(&["get", &line], Reads::Documents, Code::One),
        case(&["item", "list"], Reads::Documents),
        case(&["schema", "list"], Reads::Schema),
        case(&["schema", "get", "requirement"], Reads::Schema),
        case(&["query", "list"], Reads::Schema),
        // `readiness.dl` uses code links and `knowledge.dl` does not (DES-TEMPLATE-QUERIES), so
        // only the first reads code (DES-CODE-READERS).
        with_code(
            &["query", "execute", "relevant-risks"],
            Reads::Evaluates { input_files: 0 },
            Code::All,
        ),
        case(
            &["query", "execute", "requirement-basis"],
            Reads::Evaluates { input_files: 0 },
        ),
        with_code(
            &["query", "view", "risk-treatment"],
            Reads::Evaluates { input_files: 0 },
            Code::All,
        ),
        // Passes: the requirement is implemented and its verification passes.
        with_code(
            &[
                "query",
                "gate",
                "verified",
                "--id",
                &corpus.gate_requirement,
                "--input",
                &candidate,
                "--input",
                &results,
                "--complete",
                "verification_result",
            ],
            Reads::Evaluates { input_files: 1 },
            Code::All,
        ),
        // Explains the failing row of a retired requirement, `not_accepted`: 1 run, in
        // provenance mode. `readiness.dl` uses code links, so the report reads code.
        with_code(
            &[
                "query",
                "report",
                "implementation-ready",
                "--id",
                &corpus.retired_requirement,
            ],
            Reads::Evaluates { input_files: 0 },
            Code::All,
        ),
        // An invariant of `knowledge.dl`, which uses no code link: the retired requirement passes
        // it, so the report explains no row, reads no code and exits 0.
        case(
            &[
                "query",
                "report",
                "requirement-has-basis",
                "--id",
                &corpus.retired_requirement,
            ],
            Reads::Evaluates { input_files: 0 },
        ),
        case(
            &["item", "move", &corpus.first_id, "docs/moved/first.mara.md"],
            Reads::Writes,
        ),
        with_code(
            &["item", "rename", &corpus.first_id, &renamed],
            Reads::Writes,
            Code::All,
        ),
        case(
            &[
                "item",
                "create",
                "requirement",
                path(second),
                "--title",
                "Created by the harness",
                "--field",
                "status=draft",
                "--body",
                "Text.",
            ],
            Reads::Writes,
        ),
        case(
            &["item", "update", id(0), "--title", "Updated by the harness"],
            Reads::Writes,
        ),
        with_code(
            &["item", "delete", id(untargeted)],
            Reads::Writes,
            Code::All,
        ),
        case(
            &[
                "relation",
                "add",
                id(second + 13),
                "satisfies",
                id(second + 7),
            ],
            Reads::Writes,
        ),
        case(
            &["relation", "remove", id(13), "satisfies", id(5)],
            Reads::Writes,
        ),
    ]
}

/// The counters of the stats line (DES-PERFORMANCE-HARNESS), written here independently of
/// `WorkCounts`, so a dropped or renamed counter fails.
const COUNTERS: [&str; 8] = [
    "file_reads",
    "hashes",
    "parses",
    "snapshot_builds",
    "code_scans",
    "subprocess_spawns",
    "check_steps",
    "program_runs",
];

/// One finished `mara` process and the work it reported.
pub struct Run {
    pub output: Output,
    pub work: WorkCounts,
}

/// Runs `mara --debug-stats <args>` in `directory` with an empty environment but for
/// `MARA_NO_DAEMON=1`, so it reaches no user configuration or daemon and starts none. Requires exit status `status`, then reads the stats line that
/// the process writes last on stderr. The process's CPU time counts in [`cpu_time::now`].
pub fn run(directory: &Path, args: &[impl AsRef<std::ffi::OsStr>], status: i32) -> Run {
    run_with(directory, args, &[], status)
}

/// [`run`] with the environment holding only `env`, and `MARA_NO_DAEMON=1` unless `env` names a
/// runtime directory of the test as `XDG_RUNTIME_DIR` (DES-PERFORMANCE-HARNESS).
pub fn run_with(
    directory: &Path,
    args: &[impl AsRef<std::ffi::OsStr>],
    env: &[(&str, &Path)],
    status: i32,
) -> Run {
    let run = run_any(directory, args, env);
    assert_eq!(
        run.output.status.code(),
        Some(status),
        "exit status of mara {:?}; {:?}",
        shown(args),
        run.output
    );
    run
}

/// [`run_with`] whatever the exit status.
pub fn run_any(
    directory: &Path,
    args: &[impl AsRef<std::ffi::OsStr>],
    env: &[(&str, &Path)],
) -> Run {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mara"))
        .arg("--debug-stats")
        .args(args)
        .current_dir(directory)
        .env_clear()
        .envs(
            (!env.iter().any(|(name, _)| *name == "XDG_RUNTIME_DIR"))
                .then_some(("MARA_NO_DAEMON", Path::new("1"))),
        )
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start mara");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let errors = thread::spawn(move || {
        let mut buffer = Vec::new();
        stderr.read_to_end(&mut buffer).map(|_| buffer)
    });
    let mut stdout = Vec::new();
    let mut stdout_pipe = child.stdout.take().expect("piped stdout");
    stdout_pipe
        .read_to_end(&mut stdout)
        .expect("read the stdout of mara");
    let stderr = errors
        .join()
        .expect("read the stderr of mara")
        .expect("read the stderr of mara");
    let output = Output {
        status: cpu_time::wait(child),
        stdout,
        stderr,
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stderr
        .lines()
        .last()
        .unwrap_or_else(|| panic!("mara {:?} wrote no stats line; {output:?}", shown(args)));
    let work = parse(line).unwrap_or_else(|problem| panic!("stats line {line:?}: {problem}"));
    Run { output, work }
}

fn shown(args: &[impl AsRef<std::ffi::OsStr>]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.as_ref().to_string_lossy().into_owned())
        .collect()
}

/// Parses a stats line: a JSON object of exactly `format_version` 1 and the [`COUNTERS`].
pub fn parse(line: &str) -> Result<WorkCounts, String> {
    let value: Value = serde_json::from_str(line).map_err(|error| format!("not JSON: {error}"))?;
    let object = value.as_object().ok_or("not a JSON object")?;
    if object.get("format_version") != Some(&Value::from(1)) {
        return Err("format_version is not 1".into());
    }
    let mut expected: Vec<&str> = COUNTERS.to_vec();
    expected.push("format_version");
    expected.sort_unstable();
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    if keys != expected {
        return Err(format!("keys {keys:?} instead of {expected:?}"));
    }
    Ok(WorkCounts {
        file_reads: count(object, "file_reads")?,
        hashes: count(object, "hashes")?,
        parses: count(object, "parses")?,
        snapshot_builds: count(object, "snapshot_builds")?,
        code_scans: count(object, "code_scans")?,
        subprocess_spawns: count(object, "subprocess_spawns")?,
        check_steps: count(object, "check_steps")?,
        program_runs: count(object, "program_runs")?,
    })
}

fn count(object: &Map<String, Value>, name: &str) -> Result<u64, String> {
    object[name]
        .as_u64()
        .ok_or(format!("{name} is not a count"))
}
