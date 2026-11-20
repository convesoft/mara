#![cfg(unix)]

//! The `engineering` template's programs through the real `mara` binary, on disposable projects
//! that `project init` creates: VER-TEMPLATE-CASES in `docs/template/index.mara.md`. The cases
//! that need code links wait for mara-hk2arx; those that need explanations wait for `query
//! report`.
//!
//! The binary runs in direct mode (`MARA_NO_DAEMON=1`) with an environment of only `HOME` and
//! `TMPDIR`, both inside the test's temporary directory; it finds the pinned `souffle` beside its
//! own `target/` directory.

mod invariants;
mod readiness;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

/// One item of a test document, its type given by its ID's prefix.
#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub status: String,
    pub title: String,
    /// Metadata rows after `Status`: a text value, or the ID of an item to link to.
    rows: Vec<(String, Cell)>,
}

#[derive(Clone, Debug)]
enum Cell {
    Text(String),
    Link(String),
}

impl Item {
    pub fn title(mut self, title: &str) -> Item {
        self.title = title.to_owned();
        self
    }

    /// A field row, such as `Likelihood`.
    pub fn field(mut self, key: &str, value: &str) -> Item {
        self.rows
            .push((key.to_owned(), Cell::Text(value.to_owned())));
        self
    }

    /// A relation row to the item with ID `target`, such as `Satisfies`.
    pub fn link(mut self, key: &str, target: &str) -> Item {
        self.rows
            .push((key.to_owned(), Cell::Link(target.to_owned())));
        self
    }

    /// The same item with status `Draft`.
    pub fn as_draft(&self) -> Item {
        Item {
            status: "Draft".to_owned(),
            ..self.clone()
        }
    }

    fn kind(&self) -> &'static str {
        let prefixes = [
            ("GOAL-", "Goal"),
            ("NEED-", "Need"),
            ("SCN-", "Scenario"),
            ("REQ-", "Requirement"),
            ("DES-", "Design"),
            ("ADR-", "Decision"),
            ("RISK-", "Risk"),
            ("TERM-", "Term"),
            ("EVD-", "Evidence"),
            ("VER-", "Verification"),
            ("VAL-", "Validation"),
        ];
        prefixes
            .iter()
            .find(|(prefix, _)| self.id.starts_with(prefix))
            .map(|(_, kind)| *kind)
            .unwrap_or_else(|| panic!("no type for {}", self.id))
    }
}

fn with_status(id: &str, status: &str) -> Item {
    Item {
        id: id.to_owned(),
        status: status.to_owned(),
        title: format!("Title of {id}"),
        rows: Vec::new(),
    }
}

pub fn accepted(id: &str) -> Item {
    with_status(id, "Accepted")
}

pub fn draft(id: &str) -> Item {
    with_status(id, "Draft")
}

pub fn retired(id: &str) -> Item {
    with_status(id, "Retired")
}

/// The MID of the `n`th item of a document.
fn mid(n: usize) -> String {
    format!("01M7D8DZ2663WP3A0300000{n:03}")
}

/// A project that `mara project init` created, with `home/` and `tmp/` beside it.
pub struct Project {
    dir: TempDir,
    /// The ID of each MID of the current document.
    ids: BTreeMap<String, String>,
}

impl Default for Project {
    fn default() -> Project {
        Project::new()
    }
}

/// What one run of the binary printed.
#[derive(Debug)]
pub struct Run {
    pub status: i32,
    pub json: Value,
    pub stderr: String,
}

impl Project {
    pub fn new() -> Project {
        let dir = tempfile::Builder::new()
            .prefix("mara-template-")
            .tempdir()
            .expect("create a temporary directory");
        let project = Project {
            dir,
            ids: BTreeMap::new(),
        };
        for directory in ["home", "tmp", "project"] {
            fs::create_dir_all(project.root().join(directory)).unwrap();
        }
        let init = project.mara(&["project", "init"]);
        assert_eq!(init.status, 0, "{init:#?}");
        project
    }

    fn root(&self) -> PathBuf {
        fs::canonicalize(self.dir.path()).unwrap()
    }

    pub fn path(&self) -> PathBuf {
        self.root().join("project")
    }

    /// `mara <args> --format json` in the project, in direct mode, with only `HOME` and `TMPDIR`
    /// set.
    pub fn mara(&self, args: &[&str]) -> Run {
        let output = Command::new(env!("CARGO_BIN_EXE_mara"))
            .args(args)
            .args(["--format", "json"])
            .current_dir(self.path())
            .env_clear()
            .env("MARA_NO_DAEMON", "1")
            .env("HOME", self.root().join("home"))
            .env("TMPDIR", self.root().join("tmp"))
            .output()
            .expect("start mara");
        Run {
            status: output.status.code().unwrap_or(-1),
            json: serde_json::from_slice(&output.stdout).unwrap_or(Value::Null),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// Replaces the project's one document with `items`, each with the MID of its position.
    pub fn write(&mut self, items: &[Item]) {
        let mids: BTreeMap<&str, String> = items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id.as_str(), mid(index + 1)))
            .collect();
        let mut sections = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let mut text = format!(
                "<a id=\"{}\"></a>\n\n### {}\n\n| Type | {} |\n| --- | --- |\n| ID | {} |\n\
                 | Status | {} |\n",
                mid(index + 1),
                item.title,
                item.kind(),
                item.id,
                item.status
            );
            for (key, cell) in &item.rows {
                let value = match cell {
                    Cell::Text(text) => text.clone(),
                    Cell::Link(target) => {
                        let target_mid = mids
                            .get(target.as_str())
                            .unwrap_or_else(|| panic!("{} links to unknown {target}", item.id));
                        format!("[{target}](#{target_mid})")
                    }
                };
                text.push_str(&format!("| {key} | {value} |\n"));
            }
            text.push_str("\nThe body.\n");
            sections.push(text);
        }
        let document = format!("# Items\n\n---\n\n{}", sections.join("\n---\n\n"));
        let path = self.path().join("docs/items.mara.md");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, document).unwrap();
        self.ids = mids
            .into_iter()
            .map(|(id, mid)| (mid, id.to_owned()))
            .collect();
    }

    /// `value` with each MID of the document replaced by its ID, and numbers as text.
    pub fn named(&self, value: &Value) -> String {
        match value {
            Value::String(text) => self.ids.get(text).cloned().unwrap_or_else(|| text.clone()),
            other => other.to_string(),
        }
    }

    /// `project validate`: each diagnostic as (code, ID). Requires the outcome and the exit
    /// status to agree with the diagnostics.
    #[track_caller]
    pub fn validate(&self) -> BTreeSet<(String, String)> {
        let run = self.mara(&["project", "validate"]);
        let diagnostics = run.json["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let found: BTreeSet<(String, String)> = diagnostics
            .iter()
            .map(|d| {
                let id = d["id"].as_str().unwrap_or_default().to_owned();
                (d["code"].as_str().unwrap().to_owned(), id)
            })
            .collect();
        assert_eq!(
            found.len(),
            diagnostics.len(),
            "a repeated diagnostic: {run:#?}"
        );
        let errors = diagnostics.iter().any(|d| d["severity"] == "error");
        assert_eq!(run.json["valid"], !errors, "{run:#?}");
        assert_eq!(run.status, i32::from(errors), "{run:#?}");
        found
    }

    /// Writes `rows` as the JSON file of the input `name` and returns its `--input` value.
    pub fn input(&self, name: &str, rows: &Value) -> String {
        let path = self.root().join(format!("{name}.json"));
        fs::write(&path, rows.to_string()).unwrap();
        format!("{name}=@{}", path.display())
    }

    /// The rows of `query execute <name>` (or `query view`) with `args`, each as its values
    /// with MIDs named by ID.
    #[track_caller]
    pub fn rows(&self, operation: &str, name: &str, args: &[&str]) -> BTreeSet<Vec<String>> {
        let mut command = vec!["query", operation, name, "--limit", "100"];
        command.extend_from_slice(args);
        let run = self.mara(&command);
        assert_eq!(run.status, 0, "{run:#?}");
        assert_eq!(run.json["next_cursor"], Value::Null, "{run:#?}");
        run.json["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let values = row["values"].as_array().unwrap();
                values.iter().map(|value| self.named(value)).collect()
            })
            .collect()
    }

    /// `query gate <name>` with `args`: the gate's outcome, and each item's ID, outcome and rows
    /// as `role relation(values)` with MIDs named by ID.
    #[track_caller]
    pub fn gate(&self, name: &str, args: &[&str]) -> Gate {
        let mut command = vec!["query", "gate", name];
        command.extend_from_slice(args);
        let run = self.mara(&command);
        let outcome = run.json["gate"]["outcome"]
            .as_str()
            .unwrap_or_else(|| panic!("no gate outcome: {run:#?}"))
            .to_owned();
        assert_eq!(run.status, i32::from(outcome != "passed"), "{run:#?}");
        let items = run.json["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                let rows = item["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| {
                        let values: Vec<String> = row["values"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|value| self.named(value))
                            .collect();
                        format!(
                            "{} {}({})",
                            row["role"].as_str().unwrap(),
                            row["relation"].as_str().unwrap(),
                            values.join(", ")
                        )
                    })
                    .collect();
                GateItem {
                    id: item["item"]["id"].as_str().unwrap().to_owned(),
                    outcome: item["outcome"].as_str().unwrap().to_owned(),
                    rows,
                }
            })
            .collect();
        Gate { outcome, items }
    }
}

/// A gate's result.
#[derive(Debug)]
pub struct Gate {
    pub outcome: String,
    pub items: Vec<GateItem>,
}

#[derive(Debug)]
pub struct GateItem {
    pub id: String,
    pub outcome: String,
    pub rows: BTreeSet<String>,
}

impl Gate {
    /// The one named item.
    #[track_caller]
    pub fn only(&self) -> &GateItem {
        assert_eq!(self.items.len(), 1, "{self:#?}");
        &self.items[0]
    }
}

/// Rows of strings, for comparing with [`Project::rows`].
pub fn rows<const N: usize>(rows: &[[&str; N]]) -> BTreeSet<Vec<String>> {
    rows.iter()
        .map(|row| row.iter().map(|value| (*value).to_owned()).collect())
        .collect()
}
