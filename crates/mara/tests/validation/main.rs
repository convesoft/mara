//! The validation operations through the real `mara` binary, on disposable projects under the
//! temporary directory (`docs/validation.mara.md`; Verification items in
//! `docs/verification.mara.md`).

mod nested;
mod rendering;
mod rows;
mod schema;
mod seeded;
mod selection;
mod unchanged;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

/// A configuration whose documents are every `*.mara.md` below `docs/`.
pub const CONFIG: &str = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\nformat_version = 1\ninclude = [\"docs/**/*.mara.md\"]\n";

/// A small schema: requirements with a priority and tags, and notes.
pub const SCHEMA: &str = r#"format_version = 1
statuses = ["draft", "accepted"]

[types.requirement]
id_prefix = "REQ-"
description = "An obligation."

[types.requirement.fields.priority]
kind = "enum"
values = ["low", "high"]

[types.requirement.fields.tags]
kind = "text"
repeatable = true

[types.note]
id_prefix = "NOTE-"
description = "A note."

[relations.refines]
inverse = "refined_by"
description = "The source refines the target."
targets = { requirement = ["requirement"] }
same_type = true
acyclic = true

[relations.explains]
inverse = "explained_by"
description = "The source explains the target."
targets = { note = ["requirement"] }
"#;

/// A directory under the temporary directory, removed on drop.
pub struct Dir(TempDir);

impl Default for Dir {
    fn default() -> Dir {
        Dir::new()
    }
}

impl Dir {
    pub fn new() -> Dir {
        let dir = tempfile::Builder::new()
            .prefix("mara-validation-")
            .tempdir()
            .expect("create a temporary directory");
        Dir(dir)
    }

    /// A project at the directory's root with `CONFIG` and `schema`.
    pub fn project(schema: &str) -> Dir {
        let dir = Dir::new();
        dir.write(".mara/config.toml", CONFIG);
        dir.write(".mara/schema.toml", schema);
        dir
    }

    /// The directory, with symbolic links resolved.
    pub fn root(&self) -> PathBuf {
        fs::canonicalize(self.0.path()).unwrap()
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root().join(relative)
    }

    pub fn write(&self, relative: &str, contents: impl AsRef<[u8]>) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).unwrap()
    }
}

/// One finished `mara` process.
#[derive(Debug)]
pub struct Run {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
    /// The stats line's counters.
    pub stats: Value,
}

impl Run {
    /// Stdout as JSON.
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|error| panic!("stdout is not JSON ({error}): {self:?}"))
    }

    pub fn file_reads(&self) -> u64 {
        self.stats["file_reads"].as_u64().unwrap()
    }

    pub fn parses(&self) -> u64 {
        self.stats["parses"].as_u64().unwrap()
    }

    /// The `(code, path, line)` of each diagnostic of a JSON result.
    pub fn diagnostics(&self) -> Vec<(String, String, u64)> {
        diagnostics(&self.json()["diagnostics"])
    }
}

/// The `(code, path, line)` of each diagnostic of a JSON array.
pub fn diagnostics(array: &Value) -> Vec<(String, String, u64)> {
    array
        .as_array()
        .unwrap_or_else(|| panic!("not an array: {array}"))
        .iter()
        .map(|diagnostic| {
            (
                diagnostic["code"].as_str().unwrap().to_owned(),
                diagnostic["path"].as_str().unwrap().to_owned(),
                diagnostic["line"].as_u64().unwrap(),
            )
        })
        .collect()
}

/// A diagnostic as [`diagnostics`] lists it.
pub fn at(code: &str, path: &str, line: usize) -> (String, String, u64) {
    (code.to_owned(), path.to_owned(), line as u64)
}

/// Runs `mara --debug-stats <args>` in `directory` with an empty environment, so it reaches no
/// user configuration or daemon.
pub fn mara(directory: &Path, args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_mara"))
        .arg("--debug-stats")
        .args(args)
        .current_dir(directory)
        .env_clear()
        .env("MARA_NO_DAEMON", "1")
        .output()
        .expect("start mara");
    let stderr = String::from_utf8(output.stderr).unwrap();
    let stats = stderr
        .lines()
        .last()
        .and_then(|line| serde_json::from_str(line).ok())
        .unwrap_or_else(|| panic!("no stats line on stderr: {stderr:?}"));
    Run {
        status: output.status.code().expect("mara exited"),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr,
        stats,
    }
}

/// [`mara`] with `--format json`.
pub fn json(directory: &Path, args: &[&str]) -> Run {
    let mut all = vec!["--format", "json"];
    all.extend(args);
    mara(directory, &all)
}

/// The 1-based line of `text` on which `marker` starts; it must occur once.
pub fn line_of(text: &str, marker: &str) -> usize {
    assert_eq!(
        text.matches(marker).count(),
        1,
        "{marker:?} must occur once"
    );
    let start = text.find(marker).unwrap();
    text[..start].matches('\n').count() + 1
}

/// An item of `kind` with `rows` after its `Type` row, and `body`.
pub fn item(mid: &str, title: &str, kind: &str, rows: &str, body: &str) -> String {
    format!(
        "<a id=\"{mid}\"></a>\n\n### {title}\n\n| Type | {kind} |\n| --- | --- |\n{rows}\n{body}\n"
    )
}

/// A document of a heading and `items`, separated by thematic breaks.
pub fn document(title: &str, items: &[String]) -> String {
    let mut text = format!("# {title}\n");
    for item in items {
        text.push_str("\n---\n\n");
        text.push_str(item);
    }
    text
}
