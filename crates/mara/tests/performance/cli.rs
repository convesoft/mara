//! Runs the real `mara` binary with `--debug-stats`, and lists the operation cases that the
//! counter and scaling tests run.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;

use mara_engine::work::WorkCounts;
use serde_json::{Map, Value};

use crate::corpus::Corpus;
use crate::cpu_time;

/// One invocation of an operation on a corpus: a case of DES-PERFORMANCE-HARNESS.
pub struct Case {
    pub args: Vec<String>,
    /// The exit status on a generated corpus.
    pub status: i32,
    /// The inputs it reads, which set its cold work counts.
    pub reads: Reads,
}

/// What an operation reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reads {
    /// The configuration and the schema.
    Schema,
    /// The configuration, the schema, the `.gitignore` files and every document, which it
    /// parses into a snapshot and checks.
    Checked,
    /// The same inputs into a snapshot, without checks.
    Documents,
    /// The same inputs, then a write: the files it writes are restored after each run
    /// (`restore`).
    Writes,
}

/// Each operation's case on `corpus`. An operation adds its case here, and both tests run it.
pub fn cases(corpus: &Corpus) -> Vec<Case> {
    let case = |args: &[&str], reads| Case {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        status: 0,
        reads,
    };
    let renamed = format!("{}-RENAMED", corpus.first_id);
    // Each single-file write changes a document of the first two clusters, the same at every
    // size, and no two of them the same document.
    let id = |index: usize| corpus.ids[index].0.as_str();
    let path = |index: usize| corpus.ids[index].1.as_str();
    let (second, untargeted) = (25, corpus.untargeted);
    vec![
        case(&["project", "validate"], Reads::Checked),
        // The corpus is valid, so fix writes nothing.
        case(&["project", "fix"], Reads::Documents),
        case(&["item", "validate", &corpus.first_id], Reads::Checked),
        case(&["schema", "validate"], Reads::Schema),
        case(&["search", "cache"], Reads::Documents),
        case(&["get", &corpus.first_id], Reads::Documents),
        case(&["relation", "list", &corpus.first_id], Reads::Documents),
        case(&["item", "list"], Reads::Documents),
        case(&["schema", "list"], Reads::Schema),
        case(&["schema", "get", "requirement"], Reads::Schema),
        case(
            &["item", "move", &corpus.first_id, "docs/moved/first.mara.md"],
            Reads::Writes,
        ),
        case(
            &["item", "rename", &corpus.first_id, &renamed],
            Reads::Writes,
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
        case(&["item", "delete", id(untargeted)], Reads::Writes),
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

/// Runs `mara --debug-stats <args>` in `directory` with an empty environment, so it reaches no
/// user configuration or daemon. Requires exit status `status`, then reads the stats line that
/// the process writes last on stderr. The process's CPU time counts in [`cpu_time::now`].
pub fn run(directory: &Path, args: &[impl AsRef<std::ffi::OsStr>], status: i32) -> Run {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mara"))
        .arg("--debug-stats")
        .args(args)
        .current_dir(directory)
        .env_clear()
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
    assert_eq!(
        output.status.code(),
        Some(status),
        "exit status of mara {:?}; {output:?}",
        shown(args)
    );
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
fn parse(line: &str) -> Result<WorkCounts, String> {
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
