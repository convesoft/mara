//! The real stdio MCP server, `mara mcp`, spoken to over its stdin and stdout on disposable
//! projects: VER-CLI-MCP-PARITY in `docs/hosts/verification.mara.md`, the MCP side of
//! VER-AGENT-GUIDE in `docs/verification.mara.md`, and the MCP case of
//! VER-CONCURRENT-WRITERS in `docs/authoring/verification.mara.md`.

#![cfg(unix)]

mod guide;
mod parity;
mod protocol;
mod writers;

#[path = "../support/requests.rs"]
mod requests;

pub use requests::{Files, Usage, mcp_request, pair_mids, tool_result, usage};

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

/// A disposable directory, removed on drop.
pub struct Dir {
    dir: TempDir,
}

impl Dir {
    #[allow(
        clippy::new_without_default,
        reason = "each directory is a new temporary one"
    )]
    pub fn new() -> Dir {
        let dir = tempfile::Builder::new()
            .prefix("mara-mcp-")
            .tempdir()
            .expect("create a temporary directory");
        Dir { dir }
    }

    pub fn root(&self) -> PathBuf {
        fs::canonicalize(self.dir.path()).expect("the directory exists")
    }

    pub fn write(&self, relative: &str, contents: &str) {
        let path = self.root().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.root().join(relative)).unwrap()
    }

    /// Every file below the root with its bytes, and every directory with a trailing `/`.
    pub fn files(&self) -> Files {
        requests::files(&self.root())
    }

    /// Makes the directory hold exactly `files`, at the same path.
    pub fn restore(&self, files: &Files) {
        let root = self.root();
        for entry in fs::read_dir(&root).unwrap() {
            let path = entry.unwrap().path();
            if fs::symlink_metadata(&path).unwrap().is_dir() {
                fs::remove_dir_all(path).unwrap();
            } else {
                fs::remove_file(path).unwrap();
            }
        }
        for (path, bytes) in files {
            match path.strip_suffix('/') {
                Some(directory) => fs::create_dir_all(root.join(directory)).unwrap(),
                None => {
                    let path = root.join(path);
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(path, bytes).unwrap();
                }
            }
        }
    }
}

/// A finished `mara --debug-stats --format json` process.
#[derive(Debug)]
pub struct CliRun {
    pub status: i32,
    pub stdout: String,
    /// The stats line, the last on stderr.
    pub stats: Value,
}

impl CliRun {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|error| panic!("stdout is not JSON ({error}): {self:#?}"))
    }
}

/// Runs `mara --debug-stats --format json <args>` in `directory` in direct mode, its environment
/// holding only the test process's temporary directory as `TMPDIR`, so that no evaluation writes
/// under `/tmp` when the test runs elsewhere.
pub fn cli(directory: &Path, args: &[&str]) -> CliRun {
    let output = Command::new(env!("CARGO_BIN_EXE_mara"))
        .args(["--debug-stats", "--format", "json"])
        .args(args)
        .current_dir(directory)
        .env_clear()
        .env("MARA_NO_DAEMON", "1")
        .env("TMPDIR", std::env::temp_dir())
        .stdin(Stdio::null())
        .output()
        .expect("start mara");
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    CliRun {
        status: output.status.code().expect("mara exited"),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stats: stats(&stderr),
    }
}

/// The stats line, the last line of `stderr`.
pub fn stats(stderr: &str) -> Value {
    let line = stderr.lines().last().unwrap_or_default();
    serde_json::from_str(line).unwrap_or_else(|_| panic!("no stats line: {stderr:?}"))
}

/// The stats line of a request that did no work.
pub fn no_work() -> Value {
    json!({
        "format_version": 1, "file_reads": 0, "hashes": 0, "parses": 0, "snapshot_builds": 0,
        "code_scans": 0, "subprocess_spawns": 0, "check_steps": 0, "program_runs": 0,
    })
}

/// A running `mara --debug-stats mcp` and the client side of its stdio.
pub struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

/// A tool's result.
#[derive(Debug)]
pub struct Call {
    pub text: String,
    pub is_error: bool,
}

impl Call {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.text)
            .unwrap_or_else(|error| panic!("the tool's text is not JSON ({error}): {self:#?}"))
    }
}

impl Server {
    /// Starts `mara --debug-stats mcp <options>` in `directory` with the environment of [`cli`],
    /// without initializing it.
    pub fn spawn(directory: &Path, options: &[&str]) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mara"))
            .args(["--debug-stats", "mcp"])
            .args(options)
            .current_dir(directory)
            .env_clear()
            .env("MARA_NO_DAEMON", "1")
            .env("TMPDIR", std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start mara mcp");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        Server {
            child,
            stdin,
            stdout,
            next: 1,
        }
    }

    /// [`Server::spawn`], then the `initialize` handshake.
    pub fn start(directory: &Path, options: &[&str]) -> Server {
        let mut server = Server::spawn(directory, options);
        let response = server.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "mara-tests", "version": "1"},
            }),
        );
        assert_eq!(response["result"]["protocolVersion"], "2025-11-25");
        server.send_line(
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
        );
        server
    }

    pub fn send_line(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        writeln!(stdin, "{line}").expect("write to mara mcp");
        stdin.flush().expect("flush mara mcp's stdin");
    }

    /// The next line on stdout, which must be one JSON value.
    pub fn receive(&mut self) -> Value {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("read from mara mcp");
        assert!(line.ends_with('\n'), "mara mcp closed stdout: {line:?}");
        serde_json::from_str(&line).unwrap_or_else(|error| panic!("{error}: {line:?}"))
    }

    /// Sends a request with the next id and returns its response.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.send_line(&request.to_string());
        let response = self.receive();
        assert_eq!(response["jsonrpc"], "2.0", "{response}");
        assert_eq!(response["id"], id, "{response}");
        response
    }

    /// Calls `tool` with `arguments` and returns its result.
    pub fn call(&mut self, tool: &str, arguments: Value) -> Call {
        let response = self.request("tools/call", json!({"name": tool, "arguments": arguments}));
        let (text, is_error) = tool_result(&response);
        Call { text, is_error }
    }

    /// The tools of `tools/list`.
    pub fn tools(&mut self) -> Vec<Value> {
        let response = self.request("tools/list", json!({}));
        assert!(response["result"].get("nextCursor").is_none(), "{response}");
        response["result"]["tools"]
            .as_array()
            .expect("tools")
            .clone()
    }

    /// Closes stdin and waits: the exit status and the stats line of the whole session.
    pub fn finish(mut self) -> (ExitStatus, Value) {
        drop(self.stdin.take());
        let mut rest = String::new();
        std::io::Read::read_to_string(&mut self.stdout, &mut rest).unwrap();
        assert_eq!(rest, "", "mara mcp wrote after stdin closed");
        let output = self.child.wait_with_output().expect("wait for mara mcp");
        let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
        (output.status, stats(&stderr))
    }
}

/// A project of the engineering template with one need and one requirement that satisfies it.
pub fn engineering_project() -> Dir {
    let dir = Dir::new();
    assert_eq!(cli(&dir.root(), &["project", "init"]).status, 0);
    for args in [
        &[
            "item",
            "create",
            "need",
            "docs/needs.mara.md",
            "--title",
            "Fast answers",
            "--field",
            "status=draft",
            "--body",
            "Agents get answers quickly.",
        ][..],
        &[
            "item",
            "create",
            "requirement",
            "docs/reqs.mara.md",
            "--title",
            "Cold read time",
            "--field",
            "status=draft",
            "--body",
            "A cold read takes at most 200 ms.",
        ],
        &[
            "relation",
            "add",
            "REQ-COLD-READ-TIME",
            "satisfies",
            "NEED-FAST-ANSWERS",
        ],
    ] {
        let run = cli(&dir.root(), args);
        assert_eq!(run.status, 0, "{args:?}: {run:#?}");
    }
    dir
}
