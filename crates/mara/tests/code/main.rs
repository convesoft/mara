//! Code markers through the real `mara` binary and `mara mcp`, on disposable projects
//! initialized from `engineering`: VER-CODE-MARKERS, VER-GRAMMAR-MARKERS, VER-CODE-EDITS and
//! VER-CODE-FACTS in `docs/code/verification.mara.md`, and the code rows of VER-DELETE-ITEM and
//! VER-INTERRUPTED-EDITS.

#![cfg(unix)]

mod edits;
mod facts;
mod grammar;
mod markers;

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::SystemTime;

use serde_json::{Value, json};
use tempfile::TempDir;

/// Rust, Python and TypeScript at the basic level (VER-CODE-MARKERS).
pub const CODE: &str = r##"
[code]
format_version = 1

[code.languages.rust]
include = ["src/**/*.rs"]
comment_prefixes = ["//", "///", "*"]

[code.languages.python]
include = ["scripts/**/*.py"]
comment_prefixes = ["#"]

[code.languages.typescript]
include = ["web/**/*.ts"]
comment_prefixes = ["//", "*"]
"##;

/// The MID of item `n`.
pub fn mid(n: u32) -> String {
    format!("01M7JQ{n:020}")
}

/// An item of `kind` with `rows` after its `Status` row and `body`.
pub fn item(n: u32, kind: &str, id: &str, rows: &str, body: &str) -> String {
    format!(
        "---\n\n<a id=\"{}\"></a>\n\n### Item {id}\n\n| Type | {kind} |\n| --- | --- |\n\
         | ID | {id} |\n| Status | Draft |\n{rows}\n{body}\n\n",
        mid(n)
    )
}

/// Files with their bytes and mtimes.
pub type Tree = BTreeMap<String, (Vec<u8>, SystemTime)>;

/// The official release files of the grammars that the tests install (VER-GRAMMAR-MARKERS):
/// language, version, and the `.wasm` file's name in its fixture directory.
pub const GRAMMARS: [(&str, &str, &str); 2] = [
    ("rust", "0.24.2", "tree-sitter-rust.wasm"),
    ("python", "0.25.0", "tree-sitter-python.wasm"),
];

/// A disposable project, removed on drop.
pub struct Project {
    dir: TempDir,
    /// The `HOME` of its runs, which holds the per-user grammar cache, once a grammar is
    /// installed.
    home: Option<TempDir>,
}

impl Project {
    /// A project initialized from `engineering`, its configuration declaring [`CODE`], with
    /// REQ-LOGIN, REQ-AUDIT, REQ-LOGIN-AUDIT and NEED-ACCESS in `docs/req.mara.md`.
    #[allow(
        clippy::new_without_default,
        reason = "each test project is a new directory"
    )]
    pub fn new() -> Project {
        let project = Project::bare();
        project.run(&["project", "init"], 0);
        let config = project.read(".mara/config.toml");
        project.write(".mara/config.toml", format!("{config}{CODE}"));
        let items = [
            item(1, "Requirement", "REQ-LOGIN", "", "Users log in."),
            item(2, "Requirement", "REQ-AUDIT", "", "Log-ins are audited."),
            item(3, "Requirement", "REQ-LOGIN-AUDIT", "", "Audits log in."),
            item(4, "Need", "NEED-ACCESS", "", "Users need access."),
        ];
        project.write(
            "docs/req.mara.md",
            format!("# Requirements\n\n{}", items.concat()),
        );
        project
    }

    /// An empty directory.
    pub fn bare() -> Project {
        let dir = tempfile::Builder::new()
            .prefix("mara-code-")
            .tempdir()
            .expect("create a temporary directory");
        Project { dir, home: None }
    }

    /// The `HOME` of the runs: the per-user grammar cache is `.cache/mara/grammars` in it.
    pub fn home(&mut self) -> PathBuf {
        let home = self.home.get_or_insert_with(|| {
            tempfile::Builder::new()
                .prefix("mara-home-")
                .tempdir()
                .expect("create a temporary directory")
        });
        home.path().to_owned()
    }

    /// Installs the fixture grammar of `language` in the per-user cache, as `code add` stores it,
    /// and pins it in the configuration's `[code.languages.<language>]` (DES-GRAMMAR-ASSETS).
    /// Returns the cache directory and the configuration line of the pin.
    pub fn install_grammar(&mut self, language: &str) -> (PathBuf, u64) {
        let (_, version, wasm) = GRAMMARS
            .iter()
            .find(|(name, ..)| *name == language)
            .expect("a fixture grammar");
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grammars");
        let wasm = fs::read(fixtures.join(language).join(wasm)).unwrap();
        let tags = fs::read(fixtures.join(language).join("tags.scm")).unwrap();
        let checksum = mara::grammar::checksum(&wasm, &tags);
        let directory = self
            .home()
            .join(".cache/mara/grammars")
            .join(language)
            .join(version)
            .join(checksum.to_string());
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(mara::grammar::WASM_FILE), wasm).unwrap();
        fs::write(directory.join(mara::grammar::TAGS_FILE), tags).unwrap();
        let config = self.read(".mara/config.toml");
        let header = format!("[code.languages.{language}]\n");
        let at = config.find(&header).expect("the language is declared") + header.len();
        let pin =
            format!("grammar = {{ version = \"{version}\", checksum = \"blake3:{checksum}\" }}\n");
        self.write(
            ".mara/config.toml",
            format!("{}{pin}{}", &config[..at], &config[at..]),
        );
        let line = config[..at].lines().count() as u64 + 1;
        (directory, line)
    }

    pub fn root(&self) -> PathBuf {
        fs::canonicalize(self.dir.path()).expect("the project root exists")
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

    pub fn bytes(&self, relative: &str) -> Vec<u8> {
        fs::read(self.path(relative)).unwrap()
    }

    /// Runs `mara <args>` in the project root with an environment of only `MARA_NO_DAEMON`, the
    /// test's `TMPDIR`, which receives the private directories of program runs, and `env`.
    pub fn mara(&self, args: &[&str], env: &[(&str, &Path)]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mara"));
        command
            .args(args)
            .current_dir(self.root())
            .env_clear()
            .env("MARA_NO_DAEMON", "1")
            .env("TMPDIR", std::env::temp_dir());
        if let Some(home) = &self.home {
            command.env("HOME", home.path());
        }
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

    /// The code, path and line of each diagnostic of `project validate`, sorted.
    pub fn problems(&self) -> Vec<(String, String, u64)> {
        let output = self
            .mara(
                &["--format", "json", "project", "validate", "--limit", "100"],
                &[],
            )
            .output()
            .expect("run mara");
        let result: Value = serde_json::from_slice(&output.stdout).expect("a JSON result");
        let mut problems: Vec<(String, String, u64)> = result["diagnostics"]
            .as_array()
            .unwrap_or_else(|| panic!("no diagnostics in {result}"))
            .iter()
            .map(|d| {
                let field = |name: &str| d[name].as_str().unwrap().to_owned();
                (field("code"), field("path"), d["line"].as_u64().unwrap())
            })
            .collect();
        problems.sort();
        problems
    }

    /// The code locations of the `implemented_by` connections of `node`, as `<path>:<line>`,
    /// after requiring that each one's source is its location.
    pub fn code_links(&self, node: &str) -> Vec<String> {
        let args = ["relation", "list", node, "--relation", "implemented_by"];
        let result = self.json(&[&args[..], &["--limit", "100"]].concat(), 0);
        let connections = result["connections"].as_array().unwrap();
        connections
            .iter()
            .map(|connection| {
                let neighbour = &connection["neighbour"];
                assert_eq!(neighbour["kind"], "code", "{connection}");
                assert_eq!(neighbour["declaration"], Value::Null, "{connection}");
                assert_eq!(connection["direction"], "incoming");
                assert_eq!(connection["kind"], "relation");
                assert_eq!(connection["source"]["path"], neighbour["path"]);
                assert_eq!(connection["source"]["line"], neighbour["line"]);
                format!(
                    "{}:{}",
                    neighbour["path"].as_str().unwrap(),
                    neighbour["line"]
                )
            })
            .collect()
    }

    /// Every file below the root with its bytes and mtime; a symbolic link as itself.
    pub fn tree(&self) -> Tree {
        fn walk(root: &Path, directory: &Path, out: &mut Tree) {
            // A directory a test made unreadable is compared as an empty one.
            let Ok(entries) = fs::read_dir(directory) else {
                return;
            };
            for entry in entries {
                let path = entry.unwrap().path();
                let metadata = fs::symlink_metadata(&path).unwrap();
                if metadata.is_dir() {
                    walk(root, &path, out);
                } else {
                    let relative = path.strip_prefix(root).unwrap().to_string_lossy().into();
                    let bytes = match metadata.is_symlink() {
                        true => fs::read_link(&path)
                            .unwrap()
                            .into_os_string()
                            .into_encoded_bytes(),
                        false => fs::read(&path).unwrap_or_default(),
                    };
                    out.insert(relative, (bytes, metadata.modified().unwrap()));
                }
            }
        }
        let mut out = Tree::new();
        walk(&self.root(), &self.root(), &mut out);
        out
    }

    /// A running `mara mcp` in the project root.
    pub fn mcp(&self) -> Mcp {
        self.mcp_with(&[])
    }

    /// A running `mara mcp` in the project root, its environment holding `env` too.
    pub fn mcp_with(&self, env: &[(&str, &Path)]) -> Mcp {
        Mcp::start(&self.root(), self.home.as_ref().map(TempDir::path), env)
    }
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

/// The documents of `tree`.
pub fn documents(tree: &Tree) -> Tree {
    let documents = tree.iter().filter(|(path, _)| path.ends_with(".mara.md"));
    documents.map(|(p, v)| (p.clone(), v.clone())).collect()
}

/// Runs `mara <args>` with the pause points of the debug binary on (DES-WRITE-PROCEDURE): at
/// each pause, calls `at` with what `paused` holds, empty before step 2 and otherwise a file's
/// path, then removes `paused`. Returns the output and what each pause held.
pub fn paused(project: &Project, args: &[&str], mut at: impl FnMut(&str)) -> (Output, Vec<String>) {
    let pauses = tempfile::Builder::new()
        .prefix("mara-pauses-")
        .tempdir()
        .expect("create a temporary directory");
    let mut child = project
        .mara(args, &[("MARA_TEST_PAUSE", pauses.path())])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start mara");
    let paused = pauses.path().join("paused");
    let mut seen = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while child.try_wait().expect("wait for mara").is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "mara neither pauses nor ends"
        );
        match fs::read_to_string(&paused) {
            Ok(held) => {
                at(&held);
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

/// A running `mara mcp`, initialized.
pub struct Mcp {
    child: std::process::Child,
    stdout: BufReader<std::process::ChildStdout>,
    next: u64,
}

impl Mcp {
    fn start(directory: &Path, home: Option<&Path>, env: &[(&str, &Path)]) -> Mcp {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mara"));
        command
            .arg("mcp")
            .current_dir(directory)
            .env_clear()
            .env("MARA_NO_DAEMON", "1");
        if let Some(home) = home {
            command.env("HOME", home);
        }
        command.envs(env.iter().copied());
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start mara mcp");
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut mcp = Mcp {
            child,
            stdout,
            next: 1,
        };
        mcp.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "mara-tests", "version": "1"},
            }),
        );
        mcp.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        mcp
    }

    fn send(&mut self, message: Value) {
        let stdin = self.child.stdin.as_mut().expect("stdin is open");
        writeln!(stdin, "{message}").expect("write to mara mcp");
        stdin.flush().expect("flush mara mcp's stdin");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("read from mara mcp");
        let response: Value = serde_json::from_str(&line).expect("one JSON line");
        assert_eq!(response["id"], id, "{response}");
        response
    }

    /// The process ID of the server.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// The JSON result of `tool` with `arguments`.
    pub fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let response = self.request("tools/call", json!({"name": tool, "arguments": arguments}));
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("text");
        serde_json::from_str(text).expect("the tool's text is JSON")
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}
