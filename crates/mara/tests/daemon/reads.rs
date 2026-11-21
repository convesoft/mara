//! VER-READS-DURING-VALIDATION (`docs/daemon/verification.mara.md`, REQ-READS-DURING-VALIDATION):
//! a read of a project is answered while a long validation of the same project runs, from the
//! CLI and from one `mara mcp` session, and while a write of the project waits before its step 2
//! (DES-WRITE-PROCEDURE). The test's directory is the `TMPDIR` of the daemon; the validation's
//! `souffle` is a child of the daemon's process with its working directory in it. By order, each
//! read returns while that process still runs and the validation has not answered, and the
//! daemon's count of served requests rises by one with each read: the daemon answered it.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::fresh::{LONG, project, runs};
use crate::{RUN_WAIT, Test, WAIT, finish, mara};

/// The reads of the cases, as CLI calls and as tool calls.
const READS: [(&[&str], &str, &str); 3] = [
    (&["--format", "json", "item", "list"], "item_list", "{}"),
    (
        &["--format", "json", "get", "REQ-ONE"],
        "get",
        r#"{"node": "REQ-ONE"}"#,
    ),
    (
        &["--format", "json", "search", "apple"],
        "search",
        r#"{"query": "apple"}"#,
    ),
];

/// A project whose invariant uses the long program of VER-EVALUATION-TIME-LIMIT, with a time
/// limit of 60 s, and a started daemon serving it: the project's root and the daemon's process.
fn long_project(test: &Test) -> (PathBuf, u32) {
    let root = project(test, "project");
    fs::write(root.join(".mara/rules/flag.dl"), LONG).unwrap();
    let config = root.join(".mara/config.toml");
    let text = fs::read_to_string(&config).unwrap();
    assert!(text.contains("time_limit_seconds = 5"), "{text}");
    fs::write(
        &config,
        text.replace("time_limit_seconds = 5", "time_limit_seconds = 60"),
    )
    .unwrap();
    let pid = test.start_in(&root, &[]);
    test.wait_for_session(&root);
    (root, pid)
}

/// How many operation requests the daemon has answered, as `mara daemon status` reports.
fn served(test: &Test) -> u64 {
    test.status()["served"]
        .as_u64()
        .expect("a count of served requests")
}

/// The parent of the process `pid`.
fn parent(pid: u32) -> Option<u32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // After the command's name, in parentheses: the state, then the parent.
    let rest = &stat[stat.rfind(')')? + 2..];
    rest.split(' ').nth(1)?.parse().ok()
}

/// The `souffle` process that the daemon `daemon` runs with its working directory in
/// `temporary`, once one runs.
fn souffle_of(daemon: u32, temporary: &Path) -> u32 {
    let deadline = Instant::now() + WAIT;
    loop {
        for process in fs::read_dir("/proc").unwrap().flatten() {
            let Some(pid) = process.file_name().to_str().and_then(|n| n.parse().ok()) else {
                continue;
            };
            let (Ok(exe), Ok(cwd)) = (
                fs::read_link(process.path().join("exe")),
                fs::read_link(process.path().join("cwd")),
            ) else {
                continue;
            };
            let souffle = exe.file_name().is_some_and(|name| name == "souffle");
            if souffle && cwd.starts_with(temporary) {
                let mut ancestor = parent(pid);
                while let Some(up) = ancestor.filter(|&up| up > 1) {
                    if up == daemon {
                        return pid;
                    }
                    ancestor = parent(up);
                }
            }
        }
        assert!(Instant::now() < deadline, "the daemon ran no souffle");
        thread::sleep(Duration::from_millis(10));
    }
}

/// Stops the daemon, which ends the validation and its `souffle`.
fn stop(test: &Test, souffle: u32) {
    test.run(&["daemon", "stop"]).succeeded();
    let deadline = Instant::now() + WAIT;
    while runs(souffle) {
        assert!(Instant::now() < deadline, "souffle outlived the daemon");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn reads_from_the_cli_are_answered_while_a_validation_runs() {
    let test = Test::new();
    let (root, daemon) = long_project(&test);
    let direct: Vec<_> = (READS.iter())
        .map(|(args, ..)| test.direct(&root, args))
        .collect();
    let mut validation = test.spawn_in(&root, &["--format", "json", "project", "validate"]);
    let souffle = souffle_of(daemon, test.path());
    for ((args, ..), direct) in READS.iter().zip(&direct) {
        let before = served(&test);
        let read = test.run_in(&root, args, &[]);
        assert_eq!(
            served(&test),
            before + 1,
            "{args:?} was not served by the daemon"
        );
        assert!(runs(souffle), "souffle ended before {args:?} answered");
        let answered = validation.try_wait().unwrap();
        assert!(
            answered.is_none(),
            "the validation answered before {args:?}"
        );
        assert_eq!(&read, direct, "{args:?}");
    }
    stop(&test, souffle);
    finish(validation);
}

/// One `mara mcp` session of the test, whose calls are sent without waiting for earlier ones.
struct Session {
    child: Child,
    stdin: ChildStdin,
    lines: mpsc::Receiver<String>,
    /// Responses received for calls other than the one waited for, by ID.
    received: HashMap<u64, Value>,
    next: u64,
}

impl Session {
    fn start(test: &Test, directory: &Path) -> Session {
        let mut command = test.command_in(directory, &mara(), &["mcp"], &[]);
        let mut child = crate::spawn(command.stdin(Stdio::piped()));
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    return;
                }
            }
        });
        let mut session = Session {
            child,
            stdin,
            lines,
            received: HashMap::new(),
            next: 0,
        };
        let initialize = json!({
            "protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "mara-tests", "version": "1"},
        });
        let id = session.send("initialize", initialize);
        session.response(id);
        session.line(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        session
    }

    fn line(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Sends a request: its ID.
    fn send(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next;
        self.next += 1;
        self.line(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        id
    }

    fn call(&mut self, tool: &str, arguments: Value) -> u64 {
        self.send("tools/call", json!({"name": tool, "arguments": arguments}))
    }

    /// The response to the request `id`.
    fn response(&mut self, id: u64) -> Value {
        let deadline = Instant::now() + RUN_WAIT;
        loop {
            if let Some(response) = self.received.remove(&id) {
                return response;
            }
            let wait = deadline.saturating_duration_since(Instant::now());
            let line = (self.lines.recv_timeout(wait)).expect("a response from mara mcp");
            let response: Value = serde_json::from_str(&line).unwrap();
            if let Some(number) = response["id"].as_u64() {
                self.received.insert(number, response);
            }
        }
    }

    /// The text of a call's result, which must succeed.
    fn text(&mut self, id: u64) -> String {
        let response = self.response(id);
        let result = &response["result"];
        assert_eq!(result["isError"], false, "{response}");
        result["content"][0]["text"].as_str().unwrap().to_owned()
    }

    /// Whether the response to `id` has come.
    fn answered(&mut self, id: u64) -> bool {
        while let Ok(line) = self.lines.try_recv() {
            let response: Value = serde_json::from_str(&line).unwrap();
            if let Some(number) = response["id"].as_u64() {
                self.received.insert(number, response);
            }
        }
        self.received.contains_key(&id)
    }
}

#[test]
fn reads_from_one_mcp_session_are_answered_while_its_validation_runs() {
    let test = Test::new();
    let (root, daemon) = long_project(&test);
    let direct: Vec<String> = (READS.iter())
        .map(|(args, ..)| test.direct(&root, args).stdout.trim_end().to_owned())
        .collect();
    let mut session = Session::start(&test, &root);
    let validation = session.call("project_validate", json!({}));
    let souffle = souffle_of(daemon, test.path());
    for ((_, tool, arguments), direct) in READS.iter().zip(&direct) {
        let before = served(&test);
        let id = session.call(tool, serde_json::from_str(arguments).unwrap());
        let text = session.text(id);
        assert_eq!(
            served(&test),
            before + 1,
            "{tool} was not served by the daemon"
        );
        assert!(runs(souffle), "souffle ended before {tool} answered");
        assert!(
            !session.answered(validation),
            "the validation answered before {tool}"
        );
        assert_eq!(&text, direct, "{tool}");
    }
    stop(&test, souffle);
    drop(session.stdin);
    let _ = session.child.wait();
}

#[test]
fn a_read_is_answered_while_a_write_waits_before_its_step_2() {
    let test = Test::new();
    let root = project(&test, "project");
    let pauses = test.path().join("pauses");
    fs::create_dir(&pauses).unwrap();
    test.start_in(&root, &[("MARA_TEST_PAUSE", pauses.to_str().unwrap())]);
    test.wait_for_session(&root);
    let list: &[&str] = &["--format", "json", "item", "list"];
    let before = test.direct(&root, list);
    let create = [
        "item",
        "create",
        "requirement",
        "docs/reqs.mara.md",
        "--title",
        "Four kiwi",
        "--field",
        "status=draft",
        "--body",
        "Text.",
    ];
    let mut client = test.spawn_in(&root, &create);
    let paused = pauses.join("paused");
    let deadline = Instant::now() + WAIT;
    let at = loop {
        if let Ok(at) = fs::read_to_string(&paused) {
            break at;
        }
        assert!(client.try_wait().unwrap().is_none(), "the create ended");
        assert!(Instant::now() < deadline, "the writer did not pause");
        thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(at, "", "the create waits before step 2");
    // Only the daemon's writer pauses: the client's environment names no pauses.
    let served_before = served(&test);
    let read = test.run_in(&root, list, &[]);
    assert_eq!(
        served(&test),
        served_before + 1,
        "the read was not served by the daemon"
    );
    assert!(client.try_wait().unwrap().is_none(), "the create went on");
    assert_eq!(read, before);
    while client.try_wait().unwrap().is_none() {
        let _ = fs::remove_file(&paused);
        assert!(
            Instant::now() < deadline + RUN_WAIT,
            "the create does not end"
        );
        thread::sleep(Duration::from_millis(5));
    }
    finish(client).succeeded();
    let after = test.run_in(&root, list, &[]);
    assert!(after.stdout.contains("Four kiwi"), "{after:?}");
}
