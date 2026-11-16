//! `item move` and `item rename` through the real `mara` binary, on disposable projects in the
//! temporary directory: docs/authoring/verification.mara.md (VER-MOVE-ITEM, VER-RENAME-ID), the
//! cases of docs/authoring/verification.mara.md for these operations, and their alpha.2 rows of
//! docs/verification.mara.md.

mod interrupted;
mod moving;
mod renaming;
mod safety;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

use serde_json::Value;
use tempfile::TempDir;

/// Every `*.mara.md` file below the root is a document.
pub const CONFIG: &str = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\n\
                          format_version = 1\ninclude = [\"**/*.mara.md\"]\n";

/// Requirements that refine one another.
pub const SCHEMA: &str = "format_version = 1\nstatuses = [\"draft\"]\n\n[types.requirement]\n\
    id_prefix = \"REQ-\"\ndescription = \"An obligation.\"\n\n[relations.refines]\n\
    inverse = \"refined_by\"\ndescription = \"Refines.\"\n\
    targets = { requirement = [\"requirement\"] }\n";

/// The item that moves or is renamed, REQ-MOVED in `one/a.mara.md`.
pub const X: &str = "01M7D8DZ2663WP3A037SR2HJH2";
/// REQ-NEIGHBOUR, beside X.
pub const Y: &str = "01M7D8DZ2RBHGQBYAV7V3789H7";
/// REQ-DEST in `two/b.mara.md`, the destination of the move.
pub const W: &str = "01M7D8DZ3MXVBN7QW52PEJCX4N";
/// REQ-THIRD in `three/c.mara.md`.
pub const Z: &str = "01M7D8DZ39AMW9NPW9NA17D4S4";

/// Files with their bytes and mtimes.
pub type Tree = BTreeMap<String, (Vec<u8>, SystemTime)>;

/// A disposable project, removed on drop.
pub struct Project {
    dir: TempDir,
}

impl Project {
    /// A project with [`CONFIG`] and [`SCHEMA`] and no document.
    #[allow(
        clippy::new_without_default,
        reason = "each test project is a new directory"
    )]
    pub fn new() -> Project {
        let dir = tempfile::Builder::new()
            .prefix("mara-relink-")
            .tempdir()
            .expect("create a temporary directory");
        let project = Project { dir };
        project.write(".mara/config.toml", CONFIG);
        project.write(".mara/schema.toml", SCHEMA);
        project
    }

    /// A project of [`documents`].
    pub fn three_directories() -> Project {
        let project = Project::new();
        for (path, text) in documents() {
            project.write(path, &text);
        }
        project
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

    /// Runs `mara <args>` in the project root with an empty environment and `env`.
    pub fn mara(&self, args: &[&str], env: &[(&str, &Path)]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mara"));
        command.args(args).current_dir(self.root()).env_clear();
        for (name, value) in env {
            command.env(name, value);
        }
        command
    }

    /// Runs `mara <args>` and requires exit status `status`.
    pub fn run(&self, args: &[&str], status: i32) -> Output {
        let output = self.mara(args, &[]).output().expect("run mara");
        assert_eq!(
            output.status.code(),
            Some(status),
            "mara {args:?}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    /// Runs `mara --format json <args>`, requires exit status `status` and returns its result.
    pub fn json(&self, args: &[&str], status: i32) -> Value {
        let mut all = vec!["--format", "json"];
        all.extend(args);
        let output = self.run(&all, status);
        serde_json::from_slice(&output.stdout).expect("a JSON result")
    }

    /// The codes of the diagnostics of `project validate`, with their paths, sorted.
    pub fn problems(&self) -> Vec<(String, String)> {
        let result = self
            .json(&["project", "validate", "--limit", "100"], 0)
            .clone();
        problems(&result)
    }

    /// Like [`Project::problems`] for a project that may be invalid.
    pub fn any_problems(&self) -> Vec<(String, String)> {
        let output = self
            .mara(
                &["--format", "json", "project", "validate", "--limit", "100"],
                &[],
            )
            .output()
            .expect("run mara");
        problems(&serde_json::from_slice(&output.stdout).expect("a JSON result"))
    }

    /// Every file below the root with its bytes and mtime.
    pub fn tree(&self) -> Tree {
        fn walk(root: &Path, directory: &Path, out: &mut Tree) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                let metadata = fs::symlink_metadata(&path).unwrap();
                if metadata.is_dir() {
                    walk(root, &path, out);
                } else {
                    let relative = path.strip_prefix(root).unwrap().to_string_lossy().into();
                    let bytes = fs::read(&path).unwrap_or_default();
                    out.insert(relative, (bytes, metadata.modified().unwrap()));
                }
            }
        }
        let mut out = Tree::new();
        walk(&self.root(), &self.root(), &mut out);
        out
    }
}

fn problems(result: &Value) -> Vec<(String, String)> {
    let mut problems: Vec<(String, String)> = result["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("no diagnostics in {result}"))
        .iter()
        .map(|diagnostic| {
            let field = |name: &str| diagnostic[name].as_str().unwrap().to_owned();
            (field("code"), field("path"))
        })
        .collect();
    problems.sort();
    problems
}

/// An item of type `Requirement` with `rows` after its `Status` row and the body
/// `Body of <id>.` after `body`.
pub fn item(mid: &str, id: &str, rows: &str, body: &str) -> String {
    format!(
        "<a id=\"{mid}\"></a>\n\n### Item {id}\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | {id} |\n| Status | Draft |\n{rows}\n{body}Body of {id}.\n"
    )
}

/// X (REQ-MOVED) in `one/a.mara.md` refines Y beside it and Z in another directory, and
/// mentions itself, Y and Z. Y beside it, W in `two/b.mara.md` and Z in `three/c.mara.md` refine
/// X; Y mentions it with its ID as the text, W and Z with other words; narrative in
/// `three/c.mara.md` mentions it with its ID as the text and holds its ID as plain text.
pub fn documents() -> [(&'static str, String); 3] {
    let x = item(
        X,
        "REQ-MOVED",
        &format!(
            "| Refines | [REQ-NEIGHBOUR](#{Y}) |\n| Refines | [REQ-THIRD](../three/c.mara.md#{Z}) |\n"
        ),
        &format!(
            "It mentions [itself](#{X}), [REQ-NEIGHBOUR](#{Y}) and [the third](../three/c.mara.md#{Z}).\n\n"
        ),
    );
    let y = item(
        Y,
        "REQ-NEIGHBOUR",
        &format!("| Refines | [REQ-MOVED](#{X}) |\n"),
        &format!("It mentions [REQ-MOVED](#{X}).\n\n"),
    );
    let w = item(
        W,
        "REQ-DEST",
        &format!("| Refines | [REQ-MOVED](../one/a.mara.md#{X}) |\n"),
        &format!("It mentions [the moved item](../one/a.mara.md#{X}).\n\n"),
    );
    let z = item(
        Z,
        "REQ-THIRD",
        &format!("| Refines | [REQ-MOVED](../one/a.mara.md#{X}) |\n"),
        &format!("It mentions [the moved one](../one/a.mara.md#{X}).\n\n"),
    );
    [
        ("one/a.mara.md", format!("# One\n\n---\n\n{x}\n---\n\n{y}")),
        ("two/b.mara.md", format!("# Two\n\n---\n\n{w}")),
        (
            "three/c.mara.md",
            format!(
                "# Three\n\nThe narrative mentions [REQ-MOVED](../one/a.mara.md#{X}).\n\n\
                 REQ-MOVED is plain text here.\n\n---\n\n{z}"
            ),
        ),
    ]
}

/// Each file of `before` whose bytes `after` changes, created or deleted.
pub fn changed(before: &Tree, after: &Tree) -> Vec<String> {
    let mut paths: Vec<&String> = before.keys().chain(after.keys()).collect();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter(|path| {
            before.get(*path).map(|(bytes, _)| bytes) != after.get(*path).map(|(bytes, _)| bytes)
        })
        .cloned()
        .collect()
}

/// Requires that `after` is `before`: names, bytes and mtimes.
pub fn unchanged(before: &Tree, after: &Tree, case: &str) {
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "{case}: files created or deleted"
    );
    for (path, (bytes, mtime)) in before {
        let (new_bytes, new_mtime) = &after[path];
        assert!(
            bytes == new_bytes && mtime == new_mtime,
            "{case}: {path} changed"
        );
    }
}

/// Runs `mara <args>` with the pause points of the debug binary on (DES-WRITE-PROCEDURE): at
/// each pause, calls `at` with what `paused` holds, empty before step 2 and otherwise a file's
/// path, and the number of earlier pauses, then removes `paused`. Returns the output and what
/// each pause held.
pub fn paused(
    project: &Project,
    args: &[&str],
    mut at: impl FnMut(&str, usize),
) -> (Output, Vec<String>) {
    let pauses = tempfile::Builder::new()
        .prefix("mara-pauses-")
        .tempdir()
        .expect("create a temporary directory");
    let child = project
        .mara(args, &[("MARA_TEST_PAUSE", pauses.path())])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("start mara");
    let paused = pauses.path().join("paused");
    let mut seen = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut child = child;
    while child.try_wait().expect("wait for mara").is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "mara neither pauses nor ends"
        );
        match fs::read_to_string(&paused) {
            Ok(held) => {
                at(&held, seen.len());
                seen.push(held);
                fs::remove_file(&paused).unwrap();
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(2)),
        }
    }
    (
        child.wait_with_output().expect("collect mara's output"),
        seen,
    )
}

/// Whether a temporary file of the write procedure is left in `tree`.
pub fn temporary_left(tree: &Tree) -> bool {
    tree.keys().any(|path| path.contains(".mara-tmp-"))
}
