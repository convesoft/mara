//! The facts and supplied inputs that a program reads (VER-FACT-ROWS and VER-REQUEST-INPUTS in
//! docs/rules/verification.mara.md). VER-FACT-ROWS runs through a configured query and
//! `query execute` (`rows.rs`). The checks of supplied inputs run below the CLI (`inputs.rs`):
//! the snapshot comes from the host's inputs, the files from `mara::rules`, and the wrapper and
//! the `souffle` invocation are this test's, as DES-RULE-EXECUTION describes them; their CLI,
//! gate and MCP cases are in the `programs` and `mcp` tests.

mod inputs;
mod rows;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use mara::rules::write_facts;
use mara_engine::facts::{Facts, Inputs, SNAPSHOT_RELATIONS, SOURCE_COMPLETE, file_name};
use mara_engine::work::WorkCounts;
use serde_json::Value;
use tempfile::TempDir;

const CONFIG: &str = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\n\
                      format_version = 1\ninclude = [\"**/*.mara.md\"]\n";

const SCHEMA: &str = r#"format_version = 1
statuses = ["draft", "accepted"]

[types.requirement]
id_prefix = "REQ-"
description = "An obligation."

[types.requirement.fields.priority]
kind = "enum"
values = ["very_high", "low"]

[types.requirement.fields.owner]
kind = "text"
repeatable = true

[types.need]
id_prefix = "NEED-"
description = "A need."

[relations.satisfies]
inverse = "satisfied_by"
description = "The source fulfils the target."
targets = { requirement = ["need"] }

[relations.refines]
inverse = "refined_by"
description = "The source details the target."
targets = { requirement = ["requirement"] }
"#;

/// The `n`th test MID.
pub fn mid(n: u32) -> String {
    format!("01M7D8DZ2663WP3A03000000{n:02}")
}

/// A title in capitals with a combining accent.
pub const ECOLE: &str = "E\u{301}COLE";
/// A title and a field value that hold a tab, a quote, a backslash and non-ASCII text. In the
/// heading, `\\` is one backslash.
pub const ODD_TITLE: &str = "Tab\there \"q\" back\\\\slash \u{fc}";
pub const ODD_VALUE: &str = "a\tb \"q\" back\\slash \u{fc}";

fn item(
    n: u32,
    kind: &str,
    id: &str,
    status: &str,
    title: &str,
    rows: &[(&str, String)],
) -> String {
    let mut text = format!(
        "<a id=\"{}\"></a>\n\n### {title}\n\n| Type | {kind} |\n| --- | --- |\n| ID | {id} |\n\
         | Status | {status} |\n",
        mid(n)
    );
    for (key, value) in rows {
        text.push_str(&format!("| {key} | {value} |\n"));
    }
    text.push_str(&format!("\nMentions [one](b.mara.md#{}).\n", mid(3)));
    text
}

fn link(path: &str, n: u32) -> String {
    format!("[x]({path}#{})", mid(n))
}

/// Two requirements with fields and relation rows in `docs/a.mara.md`; a need and a
/// requirement without fields in `docs/b.mara.md`.
pub struct Project {
    dir: TempDir,
}

impl Project {
    #[allow(
        clippy::new_without_default,
        reason = "each test project is a new directory"
    )]
    pub fn new() -> Project {
        let dir = tempfile::Builder::new()
            .prefix("mara-facts-")
            .tempdir()
            .expect("create a temporary directory");
        let project = Project { dir };
        project.write(".mara/config.toml", CONFIG);
        project.write(".mara/schema.toml", SCHEMA);
        let a = [
            item(
                1,
                "Requirement",
                "REQ-A",
                "Accepted",
                ECOLE,
                &[
                    ("Priority", "Very High".into()),
                    ("Owner", ODD_VALUE.into()),
                    ("Satisfies", link("b.mara.md", 3)),
                ],
            ),
            item(
                2,
                "Requirement",
                "REQ-B",
                "Draft",
                ODD_TITLE,
                &[("Owner", "Ann".into()), ("Refines", link("", 1))],
            ),
        ];
        let b = [
            item(3, "Need", "NEED-ONE", "Accepted", "One", &[]),
            item(4, "Requirement", "REQ-C", "Draft", "Without result", &[]),
        ];
        project.write("docs/a.mara.md", &a.join("\n---\n\n"));
        project.write("docs/b.mara.md", &b.join("\n---\n\n"));
        project
    }

    pub fn root(&self) -> PathBuf {
        fs::canonicalize(self.dir.path()).unwrap()
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.dir.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// The snapshot's facts, as the host derives them for a request.
    pub fn facts(&self) -> Facts {
        let mut inputs = mara::inputs::Inputs::new(self.root());
        let generation = inputs.snapshot(SystemTime::now(), &mut WorkCounts::default());
        assert_eq!(generation.diagnostics, []);
        let snapshot = generation.snapshot;
        Facts {
            relations: snapshot.facts().relations.clone(),
        }
    }

    /// `mara <args> --format json`, run in the project root with an environment of only
    /// `TMPDIR`; it must succeed.
    pub fn mara(&self, args: &[&str]) -> Value {
        let (status, json) = self.mara_any(args);
        assert_eq!(status, 0, "mara {args:?}: {json}");
        json
    }

    /// `mara <args> --format json`, with its exit status.
    pub fn mara_any(&self, args: &[&str]) -> (i32, Value) {
        let output = Command::new(env!("CARGO_BIN_EXE_mara"))
            .args(args)
            .args(["--format", "json"])
            .current_dir(self.root())
            .env_clear()
            .env("MARA_NO_DAEMON", "1")
            .env("TMPDIR", std::env::temp_dir())
            .output()
            .expect("start mara");
        let json = serde_json::from_slice(&output.stdout).expect("mara prints JSON");
        (output.status.code().unwrap_or(-1), json)
    }
}

/// The pinned Soufflé that `scripts/build-souffle.sh` builds (DES-SOUFFLE-BUILD).
fn souffle() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/souffle-2.5/souffle");
    assert!(
        path.is_file(),
        "{} is missing: run scripts/build-souffle.sh first",
        path.display()
    );
    path
}

/// The rows of each relation of `outputs`, as Soufflé wrote them.
pub type Outputs = BTreeMap<String, Vec<Vec<Value>>>;

/// Runs `program`, which includes `mara.dl`, on `facts` and `inputs` in a private directory, as
/// `souffle --no-preprocessor -l "" wrapper.dl` with JSON files and an empty environment.
/// Returns Soufflé's stderr when it fails or prints anything there.
pub fn evaluate(
    facts: &Facts,
    inputs: &Inputs,
    program: &str,
    outputs: &[&str],
) -> Result<Outputs, String> {
    let dir = tempfile::Builder::new()
        .prefix("mara-facts-run-")
        .tempdir()
        .unwrap();
    write_facts(dir.path(), facts, inputs).unwrap();
    fs::write(dir.path().join("program.dl"), program).unwrap();
    let mut wrapper = String::from(".include \"program.dl\"\n");
    let read = SNAPSHOT_RELATIONS.iter().copied().chain([SOURCE_COMPLETE]);
    for relation in read.chain(inputs.relations.iter().map(|r| r.name.as_str())) {
        wrapper.push_str(&directive("input", relation));
    }
    for relation in outputs {
        wrapper.push_str(&directive("output", relation));
    }
    fs::write(dir.path().join("wrapper.dl"), wrapper).unwrap();
    let output = Command::new(souffle())
        .args(["--no-preprocessor", "-l", "", "wrapper.dl"])
        .current_dir(dir.path())
        .env_clear()
        .output()
        .expect("start souffle");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() || !stderr.is_empty() {
        return Err(stderr);
    }
    let read = |relation: &&str| {
        let text = fs::read(dir.path().join(file_name(relation))).unwrap();
        let rows = serde_json::from_slice(&text).unwrap();
        (relation.to_string(), rows)
    };
    Ok(outputs.iter().map(read).collect())
}

fn directive(kind: &str, relation: &str) -> String {
    let file = file_name(relation);
    format!(".{kind} {relation}(IO=jsonfile, format=list, filename=\"{file}\")\n")
}

/// `rows` sorted, for comparing sets of rows.
pub fn sorted(mut rows: Vec<Vec<Value>>) -> Vec<Vec<Value>> {
    rows.sort_by_key(|row| row.iter().map(Value::to_string).collect::<Vec<_>>());
    rows
}
