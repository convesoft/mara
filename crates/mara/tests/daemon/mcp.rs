//! `mara mcp` as a client of the daemon: the MCP cases of VER-DAEMON-START (DES-MCP-SERVER).

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

use crate::{STOPPED, Test, mara, signal};

/// The read that the cases send, as a CLI call and as a tool call.
const CLI_READ: &[&str] = &["--debug-stats", "--format", "json", "item", "list"];
const TOOL: &str = "item_list";

/// A `mara --debug-stats mcp` session of the test.
struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    /// Lines of stdout, read on a thread of their own, so that a test can see that a call has
    /// not been answered yet.
    lines: mpsc::Receiver<String>,
    next: u64,
}

impl Server {
    /// Starts the server in `directory` with `options`, and initializes the session.
    fn start(test: &Test, directory: &Path, options: &[&str], env: &[(&str, &str)]) -> Server {
        let args: Vec<&str> = ["--debug-stats", "mcp"]
            .iter()
            .chain(options)
            .copied()
            .collect();
        let mut child = crate::spawn(
            test.command_in(directory, &mara(), &args, env)
                .stdin(std::process::Stdio::piped()),
        );
        let stdin = child.stdin.take();
        let stdout: ChildStdout = child.stdout.take().unwrap();
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { return };
                if sender.send(line).is_err() {
                    return;
                }
            }
        });
        let mut server = Server {
            child,
            stdin,
            lines,
            next: 1,
        };
        let params = json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "mara-tests", "version": "1"},
        });
        server.send("initialize", params);
        server
            .receive(crate::RUN_WAIT)
            .expect("an answer to initialize");
        server.line(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        server
    }

    fn line(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    fn send(&mut self, method: &str, params: Value) {
        let id = self.next;
        self.next += 1;
        self.line(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
    }

    /// The next message, if one comes within `wait`.
    fn receive(&self, wait: Duration) -> Option<Value> {
        let line = self.lines.recv_timeout(wait).ok()?;
        Some(serde_json::from_str(&line).unwrap())
    }

    /// Sends the read as a tool call.
    fn call(&mut self) {
        self.send("tools/call", json!({"name": TOOL, "arguments": {}}));
    }

    /// The text of the call's result, which must succeed.
    fn result(&self, wait: Duration) -> Option<String> {
        let response = self.receive(wait)?;
        let result = &response["result"];
        assert_eq!(result["isError"], false, "{response}");
        Some(result["content"][0]["text"].as_str().unwrap().to_owned())
    }

    /// A call while no daemon serves it: answered at once.
    fn call_in_process(&mut self) -> String {
        self.call();
        self.result(crate::RUN_WAIT).expect("an answer")
    }

    /// A call while the daemon `pid` is stopped: not answered until it goes on.
    fn call_served_by(&mut self, pid: u32) -> String {
        signal(pid, libc::SIGSTOP);
        self.call();
        let early = self.result(STOPPED);
        signal(pid, libc::SIGCONT);
        assert_eq!(early, None, "answered while the daemon was stopped");
        self.result(crate::RUN_WAIT).expect("an answer")
    }

    /// Closes the session: its stats line, the sum of its calls' counts.
    fn finish(mut self) -> Value {
        drop(self.stdin.take());
        let output = self.child.wait_with_output().unwrap();
        assert!(output.status.success(), "{output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        serde_json::from_str(stderr.lines().last().expect("a stats line")).unwrap()
    }
}

/// The JSON result and the counts of the CLI read in direct mode.
fn direct(test: &Test, project: &Path) -> (String, Value) {
    let run = test.direct(project, CLI_READ);
    assert!(run.status.success(), "{run:?}");
    let stats = serde_json::from_str(run.stderr.lines().last().unwrap()).unwrap();
    (run.stdout.trim_end().to_owned(), stats)
}

/// The counts of `stats` multiplied by `factor`.
fn times(stats: &Value, factor: u64) -> Value {
    let mut out = stats.clone();
    for (name, value) in out.as_object_mut().unwrap() {
        if name != "format_version" {
            *value = json!(value.as_u64().unwrap() * factor);
        }
    }
    out
}

#[test]
fn a_server_sends_calls_to_the_daemon_that_the_cli_started() {
    let test = Test::new();
    let project = test.project();
    let (text, stats) = direct(&test, &project);
    test.run_in(&project, CLI_READ, &[]).succeeded();
    let pid = test.wait_for_daemon();
    let mut server = Server::start(&test, &project, &[], &[]);
    assert_eq!(server.call_served_by(pid), text);
    // Until sessions (mara-xqcnqw), the daemon reports a cold request for every request.
    assert_eq!(server.finish(), stats);
}

#[test]
fn a_server_serves_in_process_until_a_daemon_runs_and_then_through_it() {
    let test = Test::new();
    let project = test.project();
    let (text, stats) = direct(&test, &project);
    let mut server = Server::start(&test, &project, &[], &[]);
    let process = server.child.id();
    assert_eq!(server.call_in_process(), text);
    let first = test.wait_for_daemon_beside(&[process]);
    assert_eq!(server.call_served_by(first), text);
    test.run(&["daemon", "stop"]).succeeded();
    assert_eq!(test.daemons(), [process]);
    assert_eq!(server.call_in_process(), text);
    let second = test.wait_for_daemon_beside(&[process]);
    assert_ne!(first, second);
    assert_eq!(server.call_served_by(second), text);
    assert_eq!(server.finish(), times(&stats, 4));
}

#[test]
fn a_server_in_direct_mode_reaches_and_starts_no_daemon() {
    let test = Test::new();
    let project = test.project();
    let (text, stats) = direct(&test, &project);
    let modes: [crate::Mode; 3] = [
        (&["--no-daemon"], &[]),
        (&[], &[("CI", "1")]),
        (&[], &[("MARA_NO_DAEMON", "1")]),
    ];
    for (options, env) in modes {
        let mut server = Server::start(&test, &project, options, env);
        assert_eq!(server.call_in_process(), text);
        assert_eq!(test.daemons(), [server.child.id()], "{options:?} {env:?}");
        assert_eq!(server.finish(), stats);
        assert!(test.daemons().is_empty(), "{options:?} {env:?}");
    }
    let (_, pid) = test.start();
    for (options, env) in modes {
        let mut server = Server::start(&test, &project, options, env);
        signal(pid, libc::SIGSTOP);
        server.call();
        let answer = server.result(Duration::from_secs(10));
        signal(pid, libc::SIGCONT);
        assert_eq!(
            answer.as_deref(),
            Some(text.as_str()),
            "{options:?} {env:?}"
        );
        server.finish();
    }
    assert_eq!(test.daemons(), [pid]);
}
