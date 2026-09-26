#![allow(dead_code)]
// Ordinary shared process helpers; each test owns its fixture and assertions.
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

use serde_json::{Value, json};
use tempfile::TempDir;

pub fn mara(current_directory: &Path, arguments: &[&str]) -> std::process::Output {
    command(current_directory)
        .current_dir(current_directory)
        .args(arguments)
        .output()
        .expect("run Mara CLI")
}

pub fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

pub fn is_mid(value: &str) -> bool {
    value.len() == 26
        && value.chars().all(|character| {
            matches!(
                character,
                '0'..='9'
                    | 'A'..='H'
                    | 'J'..='K'
                    | 'M'..='N'
                    | 'P'..='T'
                    | 'V'..='Z'
            )
        })
}

pub fn mcp_exchange(current_directory: &Path, requests: &[Value]) -> Vec<Value> {
    mcp_exchange_with_arguments(current_directory, &["mcp"], requests)
}

pub fn mcp_exchange_with_arguments(
    current_directory: &Path,
    arguments: &[&str],
    requests: &[Value],
) -> Vec<Value> {
    let input = requests
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let expected = requests
        .iter()
        .filter(|request| request.get("id").is_some())
        .count();
    let mut child = command(current_directory)
        .current_dir(current_directory)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run Mara MCP server");
    let mut stdin = child.stdin.take().expect("capture Mara stdin");
    stdin.write_all(input.as_bytes()).expect("write Mara stdin");
    let stdout = child.stdout.take().expect("capture Mara stdout");
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line.expect("read MCP response")).is_err() {
                break;
            }
        }
    });
    let mut responses = Vec::new();
    while responses.len() < expected {
        match receiver.recv_timeout(Duration::from_secs(120)) {
            Ok(line) => responses.push(serde_json::from_str(&line).expect("MCP response is JSON")),
            Err(error) => {
                let _ = child.kill();
                let output = child.wait_with_output().expect("read Mara MCP output");
                reader.join().expect("read MCP stdout");
                panic!(
                    "received {} of {expected} MCP responses: {error}; {}",
                    responses.len(),
                    stderr(&output)
                );
            }
        }
    }
    drop(stdin);
    let output = child.wait_with_output().expect("read Mara MCP output");
    reader.join().expect("read MCP stdout");
    assert!(output.status.success(), "{}", stderr(&output));
    responses
}

pub fn mcp_request(id: u64, method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

pub fn mcp_initialize(id: u64) -> Value {
    mcp_request(
        id,
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "mara-test", "version": "1" },
        }),
    )
}

pub fn mcp_call(id: u64, name: &str, arguments: Value) -> Value {
    mcp_request(
        id,
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    )
}

pub fn fixture() -> TempDir {
    let fixture = TempDir::new().unwrap();
    let root = fixture.path();
    let init = isolated_command("git", root)
        .args(["-c", "init.defaultBranch=main", "init", "--quiet"])
        .output()
        .unwrap();
    assert!(init.status.success(), "{}", stderr(&init));
    let config = isolated_command("git", root)
        .args(["config", "core.excludesFile", "/dev/null"])
        .output()
        .unwrap();
    assert!(config.status.success(), "{}", stderr(&config));
    fixture
}

pub fn command(current_directory: &Path) -> Command {
    isolated_command(env!("CARGO_BIN_EXE_mara"), current_directory)
}

pub fn isolated_command(program: &str, current_directory: &Path) -> Command {
    let mut command = Command::new(program);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .current_dir(current_directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CEILING_DIRECTORIES", current_directory)
        .env("XDG_CONFIG_HOME", current_directory.join(".test-config"));
    command
}

pub fn mcp_response(responses: &[Value], id: u64) -> &Value {
    responses
        .iter()
        .find(|response| response["id"] == id)
        .unwrap_or_else(|| panic!("missing MCP response {id}"))
}

pub fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}
