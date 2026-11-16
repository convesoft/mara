//! `project convert` through the real CLI on disposable copies of this repository's 0.3.0 project
//! (`docs/` and `.mara/` at tag `v0.3.0`) and of fixtures: VER-CONVERSION in
//! `docs/verification.mara.md`, and the `project convert` cases of VER-PRESERVED-BYTES,
//! VER-REFUSED-EDITS and VER-INTERRUPTED-EDITS in `docs/authoring/verification.mara.md`.

#![cfg(unix)]

mod corpus;
mod mapping;
mod report;
mod stops;
mod writes;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use mara_engine::document::{self, Document};
use mara_engine::work::WorkCounts;
use serde_json::Value;

/// A project in its own temporary directory, removed on drop.
pub struct Project {
    _dir: tempfile::TempDir,
    pub root: PathBuf,
}

/// Each file below the root, temporary files of the write procedure left out, with its bytes.
pub type Tree = BTreeMap<String, Vec<u8>>;

/// Each file with its bytes, mtime and mode.
pub type Stamped = BTreeMap<String, (Vec<u8>, SystemTime, u32)>;

/// A finished `mara --format json` process.
#[derive(Debug)]
pub struct Run {
    pub status: i32,
    pub json: Value,
    pub stderr: String,
}

impl Run {
    /// The entries of a conversion's result.
    pub fn entries(&self) -> Vec<(String, u64, String)> {
        entries(&self.json["entries"])
    }

    /// The code of a failure.
    pub fn code(&self) -> &str {
        self.json["error"]["code"].as_str().unwrap_or_default()
    }

    /// The paths and lines of a failure's diagnostics.
    pub fn diagnostics(&self) -> Vec<(String, u64, String)> {
        let diagnostics = self.json["error"]["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        diagnostics
            .iter()
            .map(|d| {
                (
                    d["path"].as_str().unwrap().to_owned(),
                    d["line"].as_u64().unwrap(),
                    d["code"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    }
}

/// What a test does at a pause point of the write procedure.
pub enum Step {
    Go,
    Kill,
}

impl Project {
    pub fn empty() -> Project {
        let dir = tempfile::Builder::new()
            .prefix("mara-convert-")
            .tempdir()
            .expect("create a temporary directory");
        let root = fs::canonicalize(dir.path()).unwrap();
        Project { _dir: dir, root }
    }

    /// A copy of this repository's 0.3.0 project, read from the tag `v0.3.0`.
    pub fn corpus() -> Project {
        let project = Project::empty();
        let archive = Command::new("git")
            .arg("-C")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .args(["archive", "--format=tar", "v0.3.0", "docs", ".mara"])
            .output()
            .expect("run git");
        assert!(
            archive.status.success(),
            "the 0.3.0 corpus is read from the tag v0.3.0 of this repository; fetch the tag: {}",
            String::from_utf8_lossy(&archive.stderr)
        );
        let mut tar = Command::new("tar")
            .arg("-x")
            .arg("-C")
            .arg(&project.root)
            .stdin(Stdio::piped())
            .spawn()
            .expect("run tar");
        tar.stdin
            .take()
            .unwrap()
            .write_all(&archive.stdout)
            .unwrap();
        assert!(tar.wait().unwrap().success());
        project
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn write(&self, relative: &str, contents: impl AsRef<[u8]>) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative))
            .unwrap_or_else(|error| panic!("{relative}: {error}"))
    }

    pub fn exists(&self, relative: &str) -> bool {
        fs::symlink_metadata(self.path(relative)).is_ok()
    }

    /// A new project with the same files.
    pub fn copy(&self) -> Project {
        let copy = Project::empty();
        for (path, bytes) in self.tree() {
            copy.write(&path, bytes);
        }
        copy
    }

    pub fn tree(&self) -> Tree {
        self.stamped()
            .into_iter()
            .map(|(path, (bytes, _, _))| (path, bytes))
            .collect()
    }

    pub fn stamped(&self) -> Stamped {
        fn walk(root: &Path, directory: &Path, out: &mut Stamped) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                let metadata = fs::symlink_metadata(&path).unwrap();
                let name = path.file_name().unwrap().to_string_lossy();
                if metadata.is_dir() {
                    walk(root, &path, out);
                } else if !name.starts_with(".mara-tmp-") {
                    let relative = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    let bytes = fs::read(&path).unwrap_or_default();
                    let mode = metadata.permissions().mode() & 0o7777;
                    out.insert(relative, (bytes, metadata.modified().unwrap(), mode));
                }
            }
        }
        let mut out = Stamped::new();
        walk(&self.root, &self.root, &mut out);
        out
    }

    /// The temporary files of the write procedure left below the root.
    pub fn temporary_files(&self) -> Vec<String> {
        fn walk(directory: &Path, out: &mut Vec<String>) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if fs::symlink_metadata(&path).unwrap().is_dir() {
                    walk(&path, out);
                } else if path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".mara-tmp-")
                {
                    out.push(path.to_string_lossy().into_owned());
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        out
    }

    /// `mara --format json <args>` in the root, with an empty environment.
    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mara"));
        command
            .arg("--format")
            .arg("json")
            .args(args)
            .current_dir(&self.root)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    pub fn mara(&self, args: &[&str]) -> Run {
        finish(self.command(args).spawn().expect("start mara"))
    }

    /// `project convert`, which must succeed.
    pub fn convert(&self) -> Run {
        let run = self.mara(&["project", "convert"]);
        assert_eq!(run.status, 0, "{run:?}");
        run
    }

    /// Runs `command` with the pause points on. At each pause, calls `at` with the contents of
    /// `paused` and the number of earlier pauses, then goes on or kills the process. Returns the
    /// finished run, or `None` when it was killed.
    pub fn paused(
        &self,
        mut command: Command,
        mut at: impl FnMut(&str, usize) -> Step,
    ) -> Option<Run> {
        let pauses = tempfile::Builder::new()
            .prefix("mara-pauses-")
            .tempdir()
            .unwrap();
        command.env("MARA_TEST_PAUSE", pauses.path());
        let mut child = command.spawn().expect("start mara");
        let paused = pauses.path().join("paused");
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut seen = 0;
        loop {
            assert!(Instant::now() < deadline, "mara neither pauses nor ends");
            if child.try_wait().unwrap().is_some() {
                return Some(finish(child));
            }
            let Ok(contents) = fs::read_to_string(&paused) else {
                thread::sleep(Duration::from_millis(2));
                continue;
            };
            let step = at(&contents, seen);
            seen += 1;
            match step {
                Step::Go => fs::remove_file(&paused).unwrap(),
                Step::Kill => {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    return None;
                }
            }
        }
    }

    /// The paths that the conversion replaces, in its order, from the pauses of an
    /// uninterrupted run on a copy.
    pub fn order(&self) -> Vec<String> {
        let copy = self.copy();
        let mut order = Vec::new();
        let run = copy.paused(copy.command(&["project", "convert"]), |path, _| {
            if !path.is_empty() {
                order.push(path.to_owned());
            }
            Step::Go
        });
        assert_eq!(run.unwrap().status, 0);
        order
    }

    /// The items of the document at `path`.
    pub fn items(&self, path: &str) -> Document {
        document::parse(self.read(path).as_bytes(), &mut WorkCounts::default())
    }

    /// Whether the project is a 0.3.0 project that other operations refuse.
    pub fn is_legacy(&self) -> bool {
        let run = self.mara(&["project", "validate"]);
        run.status == 1 && run.code() == "legacy-project"
    }
}

fn finish(child: Child) -> Run {
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    Run {
        status: output.status.code().unwrap_or(-1),
        json: serde_json::from_str(&stdout).unwrap_or(Value::Null),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The entries of a report or result, as `(path, line, message)`.
pub fn entries(value: &Value) -> Vec<(String, u64, String)> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("no entries in {value}"))
        .iter()
        .map(|entry| {
            (
                entry["path"].as_str().unwrap().to_owned(),
                entry["line"].as_u64().unwrap(),
                entry["message"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// The entries of the report file of `project`.
pub fn report(project: &Project) -> Vec<(String, u64, String)> {
    let value: Value = serde_json::from_str(&project.read(".mara/conversion-report.json")).unwrap();
    assert_eq!(value["format_version"], 1);
    entries(&value["entries"])
}

/// The paths of `before` and `after` whose bytes differ, or that are in only one of them.
pub fn changed(before: &Tree, after: &Tree) -> Vec<String> {
    let mut paths: Vec<String> = before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect();
    paths.sort();
    paths.dedup();
    paths
}
