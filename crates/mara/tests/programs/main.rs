//! Programs evaluated by the real `mara` binary, on disposable projects (Verification items in
//! `docs/rules/verification.mara.md` and `docs/rules/verification.mara.md`): the invariant
//! side and the query, view and gate cases of VER-PROGRAM-CONSUMERS, VER-REQUEST-INPUTS through
//! the CLI, VER-PROGRAM-FAULTS, VER-EVALUATION-TIME-LIMIT and VER-PROGRAM-REACH-CASES for
//! validation, `query execute`, `query gate` and `query report`, VER-EXPLANATIONS, and
//! VER-ADMITTED-LANGUAGE's compile check of the fixtures.
//!
//! The binary runs with an environment of only `HOME` and `TMPDIR`, both inside the test's
//! temporary directory; it finds the pinned `souffle` beside its own `target/` directory.

mod consumers;
mod explain;
mod faults;
mod fixtures;
mod inputs;
mod interrupt;
mod queries;
mod reach;
mod time_limit;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;
use tempfile::TempDir;

pub const SCHEMA: &str = r#"format_version = 1
statuses = ["draft", "accepted"]

[types.requirement]
id_prefix = "REQ-"
description = "An obligation."

[types.requirement.fields.owner]
kind = "text"

[types.need]
id_prefix = "NEED-"
description = "A need."

[relations.satisfies]
inverse = "satisfied_by"
description = "The source fulfils the target."
targets = { requirement = ["need"] }
"#;

/// The `n`th test MID.
pub fn mid(n: u32) -> String {
    format!("01M7D8DZ2663WP3A03000000{n:02}")
}

fn item(n: u32, kind: &str, id: &str, status: &str, title: &str, rows: &[(&str, &str)]) -> String {
    let mut text = format!(
        "<a id=\"{}\"></a>\n\n### {title}\n\n| Type | {kind} |\n| --- | --- |\n| ID | {id} |\n\
         | Status | {status} |\n",
        mid(n)
    );
    for (key, value) in rows {
        text.push_str(&format!("| {key} | {value} |\n"));
    }
    text.push_str("\nThe body.\n");
    text
}

/// The configuration: documents below `docs/`, programs below `.mara/rules/`, then `entries`.
pub fn config(entries: &str) -> String {
    format!(
        "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\nformat_version = 1\n\
         include = [\"docs/**/*.mara.md\"]\n\n[rules]\nprograms = [\".mara/rules/**/*.dl\"]\n\
         {entries}"
    )
}

/// An invariant entry of `severity` on `relation` of `program`, a path below `.mara/rules/`.
pub fn invariant(name: &str, program: &str, relation: &str, severity: &str) -> String {
    format!(
        "\n[invariants.{name}]\nprogram = \".mara/rules/{program}\"\nrelation = \"{relation}\"\n\
         severity = \"{severity}\"\nmessage = \"{name} failed\"\n"
    )
}

/// An error and a warning invariant on `relation` of `program`, the query `as-query` on it, and
/// the gate `as-gate` whose subject and condition it is.
pub fn both_severities(program: &str, relation: &str) -> String {
    invariant("as-error", program, relation, "error")
        + &invariant("as-warning", program, relation, "warning")
        + &format!(
            "\n[queries.as-query]\nprogram = \".mara/rules/{program}\"\nrelation = \"{relation}\"\n\
             description = \"d\"\n\n[gates.as-gate]\nprogram = \".mara/rules/{program}\"\n\
             description = \"d\"\nsubjects = [\"{relation}\"]\nconditions = [\"{relation}\"]\n"
        )
}

/// `query execute as-query` of [`both_severities`].
pub const QUERY: [&str; 3] = ["query", "execute", "as-query"];
/// `query gate as-gate` of [`both_severities`], naming two items.
pub const GATE: [&str; 7] = [
    "query", "gate", "as-gate", "--id", "REQ-A", "--id", "NEED-ONE",
];
/// `query report as-error` of [`both_severities`], about one item.
pub const REPORT: [&str; 5] = ["query", "report", "as-error", "--id", "REQ-A"];

/// A sandbox: `home/`, `tmp/` (the binary's `TMPDIR`) and a project in `project/` with three
/// items in `docs/a.mara.md`: `REQ-A` (MID 1, accepted, owner `abc`), `REQ-B` (MID 2, draft,
/// title `Héllo`) and `NEED-ONE` (MID 3, accepted).
pub struct Sandbox {
    dir: TempDir,
}

/// What one run of the binary printed and did.
#[derive(Debug)]
pub struct Run {
    pub status: i32,
    pub json: Value,
    pub stderr: String,
    pub stats: Value,
    pub elapsed: Duration,
}

impl Run {
    pub fn diagnostics(&self) -> &[Value] {
        self.json["diagnostics"]
            .as_array()
            .map_or(&[], Vec::as_slice)
    }

    /// The diagnostics with `code`.
    pub fn with_code(&self, code: &str) -> Vec<&Value> {
        let diagnostics = self.diagnostics().iter();
        diagnostics.filter(|d| d["code"] == code).collect()
    }

    pub fn spawns(&self) -> u64 {
        self.stats["subprocess_spawns"].as_u64().unwrap()
    }

    pub fn runs(&self) -> u64 {
        self.stats["program_runs"].as_u64().unwrap()
    }
}

impl Default for Sandbox {
    fn default() -> Sandbox {
        Sandbox::new()
    }
}

impl Sandbox {
    pub fn new() -> Sandbox {
        let dir = tempfile::Builder::new()
            .prefix("mara-programs-")
            .tempdir()
            .expect("create a temporary directory");
        let sandbox = Sandbox { dir };
        fs::create_dir_all(sandbox.root().join("home")).unwrap();
        fs::create_dir_all(sandbox.tmp()).unwrap();
        sandbox.write(".mara/schema.toml", SCHEMA);
        sandbox.write(".mara/config.toml", &config(""));
        let items = [
            item(
                1,
                "Requirement",
                "REQ-A",
                "Accepted",
                "Alpha",
                &[
                    ("Owner", "abc"),
                    ("Satisfies", &format!("[NEED-ONE](#{})", mid(3))),
                ],
            ),
            item(2, "Requirement", "REQ-B", "Draft", "Héllo", &[]),
            item(3, "Need", "NEED-ONE", "Accepted", "One", &[]),
        ];
        sandbox.write(
            "docs/a.mara.md",
            &format!("# A\n\n---\n\n{}", items.join("\n---\n\n")),
        );
        sandbox
    }

    /// The sandbox's root, canonical.
    pub fn root(&self) -> PathBuf {
        fs::canonicalize(self.dir.path()).unwrap()
    }

    pub fn project(&self) -> PathBuf {
        self.root().join("project")
    }

    pub fn tmp(&self) -> PathBuf {
        self.root().join("tmp")
    }

    /// Writes `contents` to `relative` in the project.
    pub fn write(&self, relative: &str, contents: &str) {
        let path = self.project().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// Writes the program `name` below `.mara/rules/` and a configuration with `entries`.
    pub fn program(&self, name: &str, text: &str, entries: &str) {
        self.write(&format!(".mara/rules/{name}"), text);
        self.write(".mara/config.toml", &config(entries));
    }

    /// `mara <args>` in the project, with only `HOME` and `TMPDIR` set.
    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mara"));
        command
            .args(args)
            .current_dir(self.project())
            .env_clear()
            .env("MARA_NO_DAEMON", "1")
            .env("HOME", self.root().join("home"))
            .env("TMPDIR", self.tmp());
        command
    }

    /// `mara <args> --format json --debug-stats` in the project, with only `HOME` and `TMPDIR`
    /// set, and `extra` environment variables.
    pub fn mara_with(&self, args: &[&str], extra: &[(&str, &Path)]) -> Run {
        let started = Instant::now();
        let output = self
            .command(args)
            .args(["--format", "json", "--debug-stats"])
            .envs(extra.iter().map(|(name, value)| (name, value)))
            .output()
            .expect("start mara");
        let elapsed = started.elapsed();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let stats = stderr.lines().last().unwrap_or_default();
        let stats = serde_json::from_str(stats).unwrap_or_else(|_| panic!("no stats: {stderr}"));
        let json = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
        Run {
            status: output.status.code().unwrap_or(-1),
            json,
            stderr,
            stats,
            elapsed,
        }
    }

    pub fn mara(&self, args: &[&str]) -> Run {
        self.mara_with(args, &[])
    }

    pub fn validate(&self) -> Run {
        self.mara(&["project", "validate"])
    }

    /// Every file below the sandbox's root, with its bytes and modification time.
    pub fn files(&self) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
        let mut files = BTreeMap::new();
        let mut pending = vec![self.root()];
        while let Some(dir) = pending.pop() {
            for entry in fs::read_dir(&dir).unwrap() {
                let entry = entry.unwrap();
                let metadata = fs::symlink_metadata(entry.path()).unwrap();
                if metadata.is_dir() {
                    pending.push(entry.path());
                } else {
                    let bytes = fs::read(entry.path()).unwrap_or_default();
                    files.insert(entry.path(), (bytes, metadata.modified().unwrap()));
                }
            }
        }
        files
    }

    /// The entries of the binary's `TMPDIR`.
    pub fn tmp_entries(&self) -> Vec<String> {
        let mut entries: Vec<String> = fs::read_dir(self.tmp())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        entries.sort();
        entries
    }
}

/// Requires `run` to be an invalid `project validate` whose program fault is reported as an
/// error with `code` at `path` and `line`, whose message contains each of `parts`.
#[track_caller]
pub fn assert_fault(run: &Run, code: &str, path: &str, line: u64, parts: &[&str]) {
    assert_eq!(run.status, 1, "{run:#?}");
    assert_eq!(run.json["valid"], false, "{run:#?}");
    assert_diagnostic(run.diagnostics(), code, path, line, parts);
}

/// Requires `query execute as-query` to fail, and `query gate as-gate` and `query report
/// as-error` to be unavailable, the gate with both items and the report without rows, each with
/// the program's fault reported as [`assert_fault`] requires: never a row, never a passing
/// outcome, never an explanation. Returns the three runs.
#[track_caller]
pub fn assert_unavailable(
    sandbox: &Sandbox,
    code: &str,
    path: &str,
    line: u64,
    parts: &[&str],
) -> [Run; 3] {
    let query = sandbox.mara(&QUERY);
    assert_eq!(query.status, 1, "{query:#?}");
    assert_eq!(query.json["error"]["code"], "program-failed", "{query:#?}");
    let diagnostics = query.json["error"]["diagnostics"].as_array().unwrap();
    assert_diagnostic(diagnostics, code, path, line, parts);
    let gate = sandbox.mara(&GATE);
    assert_eq!(gate.status, 1, "{gate:#?}");
    assert_eq!(gate.json["gate"]["outcome"], "unavailable", "{gate:#?}");
    let items = gate.json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{gate:#?}");
    for item in items {
        assert_eq!(item["outcome"], "unavailable", "{gate:#?}");
        assert_eq!(item["rows"], serde_json::json!([]), "{gate:#?}");
    }
    assert_diagnostic(gate.diagnostics(), code, path, line, parts);
    let report = sandbox.mara(&REPORT);
    assert_eq!(report.status, 1, "{report:#?}");
    assert_eq!(
        report.json["report"]["outcome"], "unavailable",
        "{report:#?}"
    );
    assert_eq!(report.json["rows"], serde_json::json!([]), "{report:#?}");
    assert_diagnostic(report.diagnostics(), code, path, line, parts);
    [query, gate, report]
}

/// Requires an error with `code` at `path` and `line`, any line for 0, whose message contains
/// each of `parts` and never names the private copy.
#[track_caller]
pub fn assert_diagnostic(diagnostics: &[Value], code: &str, path: &str, line: u64, parts: &[&str]) {
    let found = diagnostics.iter().find(|d| {
        d["code"] == code
            && d["path"] == path
            && (line == 0 || d["line"] == line)
            && d["severity"] == "error"
    });
    let Some(found) = found else {
        panic!("no {code} at {path}:{line}: {diagnostics:#?}");
    };
    let message = found["message"].as_str().unwrap();
    for part in parts {
        assert!(message.contains(part), "{message:?} lacks {part:?}");
    }
    assert!(
        !message.contains("mara-evaluation-") && !message.contains("p0.dl"),
        "{message}"
    );
}

/// The configuration line that declares the programs.
pub const PROGRAMS_LINE: u64 = 9;
