//! The read operations through the real `mara` binary, on disposable projects in the temporary
//! directory, each with its own configuration (docs/retrieval/verification.mara.md and the read
//! cases of docs/verification.mara.md).

mod items;
mod nodes;
mod paging;
mod relations;
mod schema;
mod search;
mod selection;
mod unchanged;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

/// The configuration: every `*.mara.md` file below the root is a document.
pub const CONFIG: &str = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\n\
                          format_version = 1\ninclude = [\"**/*.mara.md\"]\n";

/// Requirements, needs and designs; `satisfies` and the same-type, acyclic `refines`.
pub const SCHEMA: &str = r#"format_version = 1
statuses = ["draft", "accepted"]

[types.requirement]
id_prefix = "REQ-"
description = "An obligation."

[types.requirement.fields.priority]
kind = "enum"
values = ["high", "low"]

[types.requirement.fields.owner]
kind = "text"
repeatable = true

[types.need]
id_prefix = "NEED-"
description = "A need."

[types.design]
id_prefix = "DES-"
description = "A design."

[relations.satisfies]
inverse = "satisfied_by"
description = "The source fulfils the target."
targets = { requirement = ["need"], design = ["requirement"] }

[relations.refines]
inverse = "refined_by"
description = "The source details the target."
targets = { requirement = ["requirement"] }
same_type = true
acyclic = true
"#;

/// A disposable project, removed on drop.
pub struct Project {
    dir: TempDir,
}

impl Project {
    /// A project with [`CONFIG`] and [`SCHEMA`].
    #[allow(
        clippy::new_without_default,
        reason = "each test project is a new directory"
    )]
    pub fn new() -> Project {
        let project = Project::empty();
        project.write(".mara/config.toml", CONFIG);
        project.write(".mara/schema.toml", SCHEMA);
        project
    }

    /// An empty directory.
    pub fn empty() -> Project {
        let dir = tempfile::Builder::new()
            .prefix("mara-reads-")
            .tempdir()
            .expect("create a temporary directory");
        Project { dir }
    }

    pub fn root(&self) -> PathBuf {
        fs::canonicalize(self.dir.path()).expect("the project root exists")
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root().join(relative)
    }

    pub fn write(&self, relative: &str, contents: &str) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).unwrap()
    }

    /// Runs `mara --debug-stats <args>` in the project root.
    pub fn run(&self, args: &[&str]) -> Run {
        run_in(&self.root(), args)
    }

    /// Runs `mara --debug-stats <args>` in `relative`, below the project root.
    pub fn run_in(&self, relative: &str, args: &[&str]) -> Run {
        run_in(&self.path(relative), args)
    }
}

/// One finished `mara` process.
#[derive(Debug)]
pub struct Run {
    pub args: Vec<String>,
    pub status: i32,
    pub stdout: String,
    /// Without the stats line.
    pub stderr: String,
    pub stats: Value,
}

impl Run {
    /// Requires exit status 0.
    pub fn ok(self) -> Run {
        assert_eq!(self.status, 0, "mara {:?} failed: {self:#?}", self.args);
        self
    }

    /// Requires exit status `status`.
    pub fn status(self, status: i32) -> Run {
        assert_eq!(
            self.status, status,
            "exit status of mara {:?}: {self:#?}",
            self.args
        );
        self
    }

    /// Stdout as JSON.
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|error| {
            panic!(
                "mara {:?}: stdout is not JSON ({error}): {self:#?}",
                self.args
            )
        })
    }

    /// The count of the counter `name` of the stats line.
    pub fn count(&self, name: &str) -> u64 {
        self.stats[name]
            .as_u64()
            .unwrap_or_else(|| panic!("no counter {name}: {self:#?}"))
    }
}

/// Runs `mara --debug-stats <args>` in `directory` with an empty environment, so that it reaches
/// no user configuration.
pub fn run_in(directory: &Path, args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_mara"))
        .arg("--debug-stats")
        .args(args)
        .current_dir(directory)
        .env_clear()
        .output()
        .expect("start mara");
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    let (stderr, stats) = match stderr.trim_end_matches('\n').rsplit_once('\n') {
        Some((before, last)) => (format!("{before}\n"), last.to_owned()),
        None => (String::new(), stderr.trim_end().to_owned()),
    };
    let stats: Value = serde_json::from_str(&stats)
        .unwrap_or_else(|error| panic!("mara {args:?}: no stats line ({error}): {stderr}"));
    Run {
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        status: output.status.code().expect("mara exits"),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr,
        stats,
    }
}

/// The `n`th test MID: a canonical uppercase ULID.
pub fn mid(n: u32) -> String {
    const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut text = String::from("01M7D8DZ2663WP3A03");
    for shift in (0..8).rev() {
        text.push(CROCKFORD[((n >> (shift * 4)) & 0xF) as usize] as char);
    }
    text
}

/// An item in the item format: anchor, heading, metadata table with `Type`, `ID` and `Status`
/// rows, then `rows`, then `body`.
pub fn item(
    mid: &str,
    kind: &str,
    id: &str,
    title: &str,
    rows: &[(&str, &str)],
    body: &str,
) -> String {
    let mut text = format!(
        "<a id=\"{mid}\"></a>\n\n### {title}\n\n| Type | {kind} |\n| --- | --- |\n| ID | {id} |\n\
         | Status | Draft |\n"
    );
    for (key, value) in rows {
        text.push_str(&format!("| {key} | {value} |\n"));
    }
    text.push_str(&format!("\n{body}\n"));
    text
}

/// Regions joined by thematic breaks.
pub fn document(regions: &[String]) -> String {
    regions.join("\n---\n\n")
}

/// A relation link or mention: `[text](path#mid)`.
pub fn link(text: &str, path: &str, mid: &str) -> String {
    format!("[{text}]({path}#{mid})")
}

/// The `id` of each record of `records`, or `<path>:<line>` for a record that is no item.
pub fn names(records: &Value) -> Vec<String> {
    records
        .as_array()
        .expect("an array of records")
        .iter()
        .map(|record| match record["id"].as_str() {
            Some(id) => id.to_owned(),
            None => format!("{}:{}", record["path"].as_str().unwrap(), record["line"]),
        })
        .collect()
}

/// The line of `path` in `project` that starts with `start`.
pub fn line_of(project: &Project, path: &str, start: &str) -> usize {
    let text = project.read(path);
    let index = text.lines().position(|line| line.starts_with(start));
    index.unwrap_or_else(|| panic!("no line of {path} starts with {start:?}")) + 1
}
