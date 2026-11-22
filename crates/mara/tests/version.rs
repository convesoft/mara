#![cfg(unix)]

//! `mara --version` through the real binary (DES-OPERATIONS in `docs/hosts/index.mara.md`):
//! alone it prints the version that `mara mcp` reports, beside another argument it is rejected,
//! and the `--version` option of `code add` (DES-GRAMMAR-ASSETS) keeps its meaning.

#[allow(dead_code, reason = "the tests here use part of the shared server")]
#[path = "support/grammar_server.rs"]
mod grammar_server;

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

use grammar_server::{GrammarServer, fixture};

/// A disposable directory under `$TMPDIR` with a project and a home of its own.
struct Dir {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Dir {
    fn new() -> Dir {
        let dir = tempfile::Builder::new()
            .prefix("mara-version-")
            .tempdir()
            .expect("create a temporary directory");
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::create_dir(root.join("home")).unwrap();
        std::fs::create_dir(root.join("project")).unwrap();
        Dir { _dir: dir, root }
    }

    /// `mara <args>` in the project directory, in direct mode, with `HOME`, `TMPDIR` and `MARA_NO_DAEMON=1`.
    fn mara(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mara"));
        command
            .args(args)
            .current_dir(self.root.join("project"))
            .env_clear()
            .env("TMPDIR", std::env::temp_dir())
            .env("HOME", self.root.join("home"))
            .env("MARA_NO_DAEMON", "1");
        command
    }

    fn run(&self, args: &[&str], status: i32) -> Output {
        let output = self.mara(args).output().expect("run mara");
        assert_eq!(
            output.status.code(),
            Some(status),
            "mara {args:?}: {output:?}"
        );
        output
    }
}

/// `serverInfo.version` of the `initialize` response of a real `mara mcp`.
fn mcp_version(dir: &Dir) -> String {
    let mut server = dir
        .mara(&["mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start mara mcp");
    let mut stdin = server.stdin.take().unwrap();
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "mara-tests", "version": "1"},
        },
    });
    writeln!(stdin, "{request}").unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    BufReader::new(server.stdout.take().unwrap())
        .read_line(&mut line)
        .expect("read from mara mcp");
    drop(stdin);
    let status = server.wait().expect("wait for mara mcp");
    assert!(status.success(), "mara mcp: {status:?}");
    let response: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line:?}"));
    response["result"]["serverInfo"]["version"]
        .as_str()
        .unwrap_or_else(|| panic!("no serverInfo.version: {response}"))
        .to_owned()
}

#[test]
fn prints_the_version_that_the_mcp_server_reports() {
    let dir = Dir::new();
    let version = mcp_version(&dir);
    let output = dir.run(&["--version"], 0);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("mara {version}\n")
    );
    assert_eq!(output.stderr, b"");
}

#[test]
fn rejects_any_other_argument_beside_the_version_flag() {
    let dir = Dir::new();
    for args in [
        &["--version", "--debug-stats"][..],
        &["--debug-stats", "--version"],
        &["--version", "--no-daemon"],
        &["--version=1"],
        &["--version", "project", "validate"],
        &["--format", "json", "--version"],
    ] {
        let output = dir.run(args, 2);
        assert_eq!(
            output.stdout.is_empty(),
            !args.contains(&"json"),
            "{args:?}"
        );
        let text =
            String::from_utf8_lossy(&output.stderr) + String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("invalid-argument"), "{args:?}: {text}");
        assert!(text.contains("`mara --version` takes"), "{args:?}: {text}");
    }
}

#[test]
fn bare_mara_names_the_version_flag_in_its_usage() {
    let output = Dir::new().run(&[], 0);
    let usage = String::from_utf8(output.stdout).unwrap();
    assert!(
        usage.lines().any(|line| line.trim() == "mara --version"),
        "{usage}"
    );
}

/// `code add <language> --version <version>` reaches the operation, which fetches that version
/// from a loopback repository of the test and pins it.
#[test]
fn leaves_the_version_option_of_code_add_to_the_operation() {
    let dir = Dir::new();
    dir.run(&["project", "init", "--template", "engineering"], 0);
    let server = GrammarServer::start();
    let (wasm, tags, version) = fixture("rust");
    server.serve_grammar("rust", version, wasm, tags);
    let url = server.url();
    let args = [
        "--format",
        "json",
        "code",
        "add",
        "rust",
        "--repository",
        &url,
        "--version",
        version,
    ];
    let output = dir.run(&args, 0);
    let result: Value = serde_json::from_slice(&output.stdout).expect("a JSON result");
    assert_eq!(result["version"], version, "{result}");
    assert_eq!(result["fetched"], true, "{result}");
    assert!(server.connections() > 0);
    let config = std::fs::read_to_string(dir.root.join("project/.mara/config.toml")).unwrap();
    assert!(
        config.contains(&format!("version = \"{version}\"")),
        "{config}"
    );
    assert!(cache_holds(&dir.root.join("home"), version));
}

/// Whether a cached grammar of `version` lies under `home`.
fn cache_holds(home: &Path, version: &str) -> bool {
    let mut pending = vec![home.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == version) {
                    return true;
                }
                pending.push(path);
            }
        }
    }
    false
}
