//! An evaluation never outlives the `mara` process that started it (DES-RULE-EXECUTION): a
//! signal that ends `mara`, through the CLI or `mara mcp`, also ends Soufflé, and SIGINT,
//! SIGTERM and SIGHUP remove the private directory before `mara` ends by that signal.

use std::fs;
use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use crate::Sandbox;
use crate::time_limit::{LONG, with_limit};

/// The processes whose working directory is below `dir`.
fn processes_in(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for entry in fs::read_dir("/proc").unwrap().flatten() {
        if let Ok(cwd) = fs::read_link(entry.path().join("cwd"))
            && cwd.starts_with(dir)
        {
            found.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    found
}

/// Waits at most 10 s for `condition`.
fn wait_for(what: &str, condition: impl Fn() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(started.elapsed() < Duration::from_secs(10), "{what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Sends `signal` to `mara` once Soufflé evaluates, then checks how both end.
fn interrupt(sandbox: &Sandbox, mut mara: Child, signal: i32) {
    let tmp = sandbox.tmp();
    wait_for("Soufflé never started", || !processes_in(&tmp).is_empty());
    // SAFETY: `kill` sends a signal to the test's own child.
    assert_eq!(unsafe { libc::kill(mara.id() as i32, signal) }, 0);
    let status = mara.wait().unwrap();
    assert_eq!(status.signal(), Some(signal), "{status:?}");
    wait_for("Soufflé outlived mara", || {
        processes_in(&sandbox.root()).is_empty()
    });
    if signal == libc::SIGKILL {
        // Nothing could remove it.
        assert_eq!(sandbox.tmp_entries().len(), 1);
    } else {
        assert_eq!(
            sandbox.tmp_entries(),
            Vec::<String>::new(),
            "signal {signal}"
        );
    }
}

fn long_project() -> Sandbox {
    let sandbox = Sandbox::new();
    with_limit(&sandbox, LONG, Some(30));
    sandbox
}

#[test]
fn a_signal_to_the_cli_ends_the_evaluation() {
    for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP, libc::SIGKILL] {
        let sandbox = long_project();
        let mara = sandbox
            .command(&["project", "validate"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        interrupt(&sandbox, mara, signal);
    }
}

#[test]
fn a_signal_to_the_mcp_server_ends_the_evaluation() {
    for signal in [libc::SIGTERM, libc::SIGKILL] {
        let sandbox = long_project();
        let mut mara = sandbox
            .command(&["mcp"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let requests = [
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"project_validate","arguments":{}}}"#,
        ];
        let mut stdin = mara.stdin.take().unwrap();
        for request in requests {
            writeln!(stdin, "{request}").unwrap();
        }
        interrupt(&sandbox, mara, signal);
        drop(stdin);
    }
}
