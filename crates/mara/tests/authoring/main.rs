//! The single-file authoring operations through the real `mara` binary, on disposable projects
//! in the temporary directory: `item create`, `item update`, `item delete`, `relation add` and
//! `relation remove` (docs/verification-authoring.mara.md), with their cases of
//! docs/verification-writes.mara.md and the alpha.2 rows of docs/verification-documents.mara.md.

#![cfg(unix)]

mod create;
mod delete;
mod documents;
mod pauses;
mod relations;
mod update;
mod writes;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use serde_json::Value;
use tempfile::TempDir;

/// The configuration: every `*.mara.md` file below `docs/` is a document.
pub const CONFIG: &str = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\n\
                          format_version = 1\ninclude = [\"docs/**/*.mara.md\"]\n";

/// Requirements with a single-valued enum field, a required repeatable text field and an
/// optional repeatable field; needs, designs and goals; `satisfies` from requirements to needs
/// and from designs to requirements, and nothing that leads to goals.
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

[types.requirement.fields.tags]
kind = "text"
repeatable = true

[types.need]
id_prefix = "NEED-"
description = "A need."

[types.design]
id_prefix = "DES-"
description = "A design."

[types.design.fields.rationale]
kind = "text"
required = true

[types.goal]
id_prefix = "GOAL-"
description = "A goal."

[relations.satisfies]
inverse = "satisfied_by"
description = "The source fulfils the target."
targets = { requirement = ["need"], design = ["requirement"] }

[relations.refines]
inverse = "refined_by"
description = "The source details the target."
targets = { requirement = ["requirement"] }
same_type = true
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
            .prefix("mara-authoring-")
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

    pub fn exists(&self, relative: &str) -> bool {
        fs::symlink_metadata(self.path(relative)).is_ok()
    }

    /// Runs `mara --debug-stats <args>` in the project root.
    pub fn run(&self, args: &[&str]) -> Run {
        run_in(&self.root(), args)
    }

    /// Every file below the root, by path, with its bytes, mtime and mode.
    pub fn tree(&self) -> Tree {
        fn walk(root: &Path, directory: &Path, out: &mut Tree) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                let metadata = fs::symlink_metadata(&path).unwrap();
                let relative: String = path.strip_prefix(root).unwrap().to_string_lossy().into();
                if metadata.is_dir() {
                    out.insert(format!("{relative}/"), File::default());
                    walk(root, &path, out);
                } else {
                    out.insert(
                        relative,
                        File {
                            bytes: fs::read(&path).unwrap_or_default(),
                            modified: metadata.modified().ok(),
                            mode: metadata.permissions().mode() & 0o7777,
                        },
                    );
                }
            }
        }
        let mut out = Tree::new();
        walk(&self.root(), &self.root(), &mut out);
        out
    }

    /// Requires that no file or directory below the root was created, deleted or modified since
    /// `before`: names, bytes, mtimes and modes.
    pub fn unchanged(&self, before: &Tree, what: &str) {
        let after = self.tree();
        if *before != after {
            let changed: Vec<&String> = before
                .keys()
                .chain(after.keys())
                .filter(|path| before.get(*path) != after.get(*path))
                .collect();
            panic!("{what} changed {changed:?}");
        }
    }

    /// Runs `project validate` and requires a valid project.
    pub fn valid(&self) {
        let run = self.run(&["project", "validate"]);
        assert_eq!(
            (run.status, &run.json()["diagnostics"]),
            (0, &Value::Array(Vec::new())),
            "the project is not valid: {run:#?}"
        );
    }
}

/// A file as [`Project::tree`] records it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct File {
    pub bytes: Vec<u8>,
    pub modified: Option<SystemTime>,
    pub mode: u32,
}

pub type Tree = BTreeMap<String, File>;

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

    /// Requires exit status 1 and the failure code `code` in the JSON form.
    pub fn refused(self, code: &str) -> Run {
        assert_eq!(
            (self.status, self.json()["error"]["code"].as_str()),
            (1, Some(code)),
            "mara {:?}: {self:#?}",
            self.args
        );
        self
    }

    /// Requires exit status 2 and `invalid-argument`.
    pub fn rejected(self) -> Run {
        assert_eq!(
            (self.status, self.json()["error"]["code"].as_str()),
            (2, Some("invalid-argument")),
            "mara {:?}: {self:#?}",
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

    /// The failure's message.
    pub fn message(&self) -> String {
        self.json()["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }
}

/// Runs `mara --debug-stats --format json <args>` in `directory` with an empty environment, so
/// that it reaches no user configuration.
pub fn run_in(directory: &Path, args: &[&str]) -> Run {
    finish(args, command(directory, args).output().expect("start mara"))
}

/// The command of [`run_in`].
pub fn command(directory: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mara"));
    command
        .args(["--debug-stats", "--format", "json"])
        .args(args)
        .current_dir(directory)
        .env_clear()
        .stdin(Stdio::null());
    command
}

/// The [`Run`] of a finished `mara --debug-stats` process.
pub fn finish(args: &[&str], output: std::process::Output) -> Run {
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

/// An item: anchor, `##` heading, metadata table with `Type`, `ID` and `Status` rows, then
/// `rows`, then `body`, with LF line endings and a final one.
pub fn item(
    mid: &str,
    kind: &str,
    id: &str,
    title: &str,
    rows: &[(&str, &str)],
    body: &str,
) -> String {
    let mut text = format!(
        "<a id=\"{mid}\"></a>\n\n## {title}\n\n| Type | {kind} |\n| --- | --- |\n| ID | {id} |\n\
         | Status | Draft |\n"
    );
    for (key, value) in rows {
        text.push_str(&format!("| {key} | {value} |\n"));
    }
    text.push_str(&format!("\n{body}\n"));
    text
}

/// A title paragraph and items, joined by thematic breaks.
pub fn document(title: &str, items: &[String]) -> String {
    let mut regions = vec![format!("# {title}\n")];
    regions.extend(items.iter().cloned());
    regions.join("\n---\n\n")
}

/// A relation link or mention: `[text](path#mid)`.
pub fn link(text: &str, path: &str, mid: &str) -> String {
    format!("[{text}]({path}#{mid})")
}

/// The 1-based line of `text` that starts with `start`.
pub fn line_of(text: &str, start: &str) -> usize {
    let index = text.lines().position(|line| line.starts_with(start));
    index.unwrap_or_else(|| panic!("no line starts with {start:?} in {text}")) + 1
}

/// Whether `text` holds no `.mara-tmp-*` file name: a temporary file left behind.
pub fn no_temporary_file(tree: &Tree) -> bool {
    tree.keys().all(|path| {
        !path
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .starts_with(".mara-tmp-")
    })
}
