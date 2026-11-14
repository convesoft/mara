//! Runs the real `mara` binary with `--debug-stats`.

use std::path::Path;
use std::process::{Command, Output};

use mara_engine::work::WorkCounts;
use serde_json::{Map, Value};

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
/// the process writes last on stderr.
pub fn run(directory: &Path, args: &[&str], status: i32) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_mara"))
        .arg("--debug-stats")
        .args(args)
        .current_dir(directory)
        .env_clear()
        .output()
        .expect("start mara");
    assert_eq!(
        output.status.code(),
        Some(status),
        "exit status of mara {args:?}; {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stderr
        .lines()
        .last()
        .unwrap_or_else(|| panic!("mara {args:?} wrote no stats line; {output:?}"));
    let work = parse(line).unwrap_or_else(|problem| panic!("stats line {line:?}: {problem}"));
    Run { output, work }
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
