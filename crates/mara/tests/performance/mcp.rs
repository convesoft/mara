//! The cases of `mara mcp`: a session of the real server, the handshake, `tools/list` and one
//! tool call, so that the session's stats line is that call's (DES-PERFORMANCE-HARNESS). The
//! server keeps nothing between calls, so a call's counts are those of a cold request.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{ChildStdin, ChildStdout, Command, Output, Stdio};
use std::thread;

use serde_json::{Value, json};

use crate::cli::{Code, Reads, Run, parse};
use crate::corpus::{Corpus, VALIDATION_RUNS};
use crate::cpu_time;

/// One tool call through `mara mcp`.
pub struct Case {
    pub tool: &'static str,
    pub arguments: Value,
    /// The inputs it reads, which set its cold work counts.
    pub reads: Reads,
    /// The code files it reads.
    pub code: Code,
}

/// A call of each kind of read, and a write.
pub fn cases(corpus: &Corpus) -> Vec<Case> {
    let case = |tool, arguments, reads| Case {
        tool,
        arguments,
        reads,
        code: Code::None,
    };
    vec![
        Case {
            code: Code::All,
            ..case(
                "project_validate",
                json!({}),
                Reads::Checked {
                    runs: VALIDATION_RUNS,
                },
            )
        },
        case("item_list", json!({}), Reads::Documents),
        case("schema_get", json!({"name": "requirement"}), Reads::Schema),
        case(
            "item_create",
            json!({
                "type": "requirement", "path": corpus.ids[25].1, "title": "Created through MCP",
                "field": ["status=draft"], "body": "Text.",
            }),
            Reads::Writes,
        ),
    ]
}

/// Runs `mara --debug-stats mcp` in `directory` with an empty environment, as a client does: the
/// handshake and `tools/list`, then each call, which must succeed, then stdin closed. Requires
/// exit status 0 and returns the session's stats line, the last line on stderr. The process's CPU
/// time counts in [`cpu_time::now`].
pub fn run(directory: &Path, calls: &[(&str, &Value)]) -> Run {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mara"))
        .args(["--debug-stats", "mcp"])
        .current_dir(directory)
        .env_clear()
        .env("MARA_NO_DAEMON", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start mara mcp");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let errors = thread::spawn(move || {
        let mut buffer = Vec::new();
        stderr.read_to_end(&mut buffer).map(|_| buffer)
    });
    let mut stdin = child.stdin.take().expect("piped stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
    let mut received = Vec::new();
    let initialize = json!({
        "protocolVersion": "2025-11-25", "capabilities": {},
        "clientInfo": {"name": "mara-performance", "version": "1"},
    });
    let mut session = Session {
        stdin: &mut stdin,
        stdout: &mut stdout,
        received: &mut received,
    };
    session.exchange(0, "initialize", initialize);
    session.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    let tools = session.exchange(1, "tools/list", json!({}));
    assert!(tools["result"]["tools"].is_array(), "{tools}");
    for (index, (tool, arguments)) in calls.iter().enumerate() {
        let params = json!({"name": tool, "arguments": arguments});
        let response = session.exchange(index + 2, "tools/call", params);
        assert_eq!(
            response["result"]["isError"], false,
            "{tool} {arguments}: {response}"
        );
    }
    drop(stdin);
    stdout
        .read_to_end(&mut received)
        .expect("read the rest of mara mcp's stdout");
    let stderr = errors
        .join()
        .expect("read the stderr of mara mcp")
        .expect("read the stderr of mara mcp");
    let output = Output {
        status: cpu_time::wait(child),
        stdout: received,
        stderr,
    };
    assert!(output.status.success(), "mara mcp {calls:?}: {output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stderr.lines().last().unwrap_or_default();
    let work = parse(line).unwrap_or_else(|problem| panic!("stats line {line:?}: {problem}"));
    Run { output, work }
}

/// The client's side of a session's stdio.
struct Session<'s> {
    stdin: &'s mut ChildStdin,
    stdout: &'s mut BufReader<ChildStdout>,
    /// Everything read from stdout.
    received: &'s mut Vec<u8>,
}

impl Session<'_> {
    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").expect("write to mara mcp");
    }

    /// Sends a request and returns its response, the next line on stdout.
    fn exchange(&mut self, id: usize, method: &str, params: Value) -> Value {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("read from mara mcp");
        self.received.extend_from_slice(line.as_bytes());
        let response: Value = serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("{method}: {error}: {line:?}"));
        assert_eq!(response["id"], id, "{response}");
        response
    }
}
