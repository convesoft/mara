//! The MCP case of VER-CONCURRENT-WRITERS: an MCP write takes the same write lock as a CLI write.

use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use mara::write::WriteLock;
use serde_json::{Value, json};

use crate::{cli, engineering_project};

fn create(title: &str) -> Value {
    json!({
        "type": "requirement", "path": "docs/reqs.mara.md", "title": title,
        "field": ["status=draft"], "body": "Text.",
    })
}

/// A CLI write and an MCP `item_create` call at once, as for two CLI writes: every item reported
/// as created is in the document, every other write reports a refusal, and the project gains
/// no diagnostic.
#[test]
fn a_cli_write_and_an_mcp_create_at_once() {
    let dir = engineering_project();
    let mut server = crate::Server::start(&dir.root(), &[]);
    let mut created = 0;
    for round in 0..3 {
        let title = format!("By the CLI {round}");
        let child = Command::new(env!("CARGO_BIN_EXE_mara"))
            .args([
                "--format",
                "json",
                "item",
                "create",
                "requirement",
                "docs/reqs.mara.md",
            ])
            .args([
                "--title",
                &title,
                "--field",
                "status=draft",
                "--body",
                "Text.",
            ])
            .current_dir(dir.root())
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start mara");
        let call = server.call("item_create", create(&format!("By MCP {round}")));
        let output = child.wait_with_output().unwrap();
        let cli_json: Value = serde_json::from_slice(&output.stdout).unwrap();
        let document = dir.read("docs/reqs.mara.md");
        for (ok, json) in [
            (output.status.success(), cli_json),
            (!call.is_error, call.json()),
        ] {
            if ok {
                created += 1;
                let mid = json["item"]["mid"].as_str().unwrap();
                assert!(
                    document.contains(&format!("<a id=\"{mid}\"></a>")),
                    "{json}"
                );
            } else {
                assert!(json["error"]["code"].is_string(), "{json}");
            }
        }
    }
    assert!(created > 0);
    let items = cli(&dir.root(), &["item", "list", "--type", "requirement"]);
    assert_eq!(items.json()["items"].as_array().unwrap().len(), 1 + created);
    let validation = cli(&dir.root(), &["project", "validate"]);
    assert_eq!(
        validation.json()["diagnostics"],
        json!([]),
        "{validation:#?}"
    );
    assert!(server.finish().0.success());
}

/// An MCP write started while the test holds the write lock as a Mara writer does changes
/// nothing until the lock is released, then applies to the contents the test left.
#[test]
fn an_mcp_write_waits_for_the_write_lock() {
    let dir = engineering_project();
    let mut server = crate::Server::start(&dir.root(), &[]);
    let lock = WriteLock::take(&dir.root()).unwrap();
    let before = dir.files();
    let request = json!({
        "jsonrpc": "2.0", "id": "waits", "method": "tools/call",
        "params": {"name": "item_create", "arguments": create("Waited")},
    });
    server.send_line(&request.to_string());
    thread::sleep(Duration::from_millis(500));
    assert_eq!(dir.files(), before, "a write while the lock is held");
    let changed = format!(
        "{}\nChanged while Mara waited.\n",
        dir.read("docs/reqs.mara.md")
    );
    dir.write("docs/reqs.mara.md", &changed);
    drop(lock);
    let response = server.receive();
    assert_eq!(response["id"], "waits");
    assert_eq!(response["result"]["isError"], false, "{response}");
    let after = dir.read("docs/reqs.mara.md");
    assert!(after.starts_with(&changed), "{after}");
    assert!(after.contains("## Waited"), "{after}");
    assert!(server.finish().0.success());
}
