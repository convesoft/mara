mod support;

use serde_json::{Value, json};
use std::{collections::BTreeSet, fs};
use support::*;

// @mara implements VER-INTERFACE-PARITY
// @mara checks REQ-SURFACE-PARITY
#[test]
fn parse_failures_use_the_selected_transport_and_help_remains_readable() {
    let fixture = fixture();
    for args in [
        &["--format", "json", "get"][..],
        &["get", "--format=json"][..],
    ] {
        let result = mara(fixture.path(), args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stderr.is_empty(), "{}", stderr(&result));
        let error: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert!(
            error["error"]["message"]
                .as_str()
                .unwrap()
                .contains("<REFERENCE>")
        );
    }
    let result = mara(fixture.path(), &["get"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(stderr(&result).contains("<REFERENCE>"));
    for option in ["--help", "--version"] {
        let result = mara(fixture.path(), &["--format", "json", option]);
        assert!(result.status.success(), "{}", stderr(&result));
        assert!(result.stderr.is_empty());
        assert!(stdout(&result).contains("mara"));
        assert!(serde_json::from_slice::<Value>(&result.stdout).is_err());
    }
}

// @mara implements VER-INTERFACE-PARITY
// @mara checks REQ-SURFACE-PARITY
// @mara checks REQ-MID-BACKFILL
#[test]
fn undeclared_mcp_arguments_are_rejected_before_backfill_or_other_operations() {
    let fixture = fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let source = ":::mara requirement REQ-INPUT\n:title: Validate arguments first\n\nDo not write source for an undeclared argument.\n:::\n";
    let path = fixture.path().join("items.mara.md");
    fs::write(&path, source).unwrap();
    let cli = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "project",
            "mid",
            "backfill",
            "--workspace",
            "other",
        ],
    );
    assert_eq!(cli.status.code(), Some(2));
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_request(2, "tools/list", json!({})),
            mcp_call(3, "project_mid_backfill", json!({"workspace":"other"})),
            mcp_call(
                4,
                "project_transaction_rollback",
                json!({"workspace":"other"}),
            ),
            mcp_call(5, "project_validate", json!({"workspace":"other"})),
            mcp_call(
                6,
                "item_create",
                json!({"flavour":"requirement","id":"REQ-UNDECLARED","file":"new.mara.md","title":"Rejected input","body":"Do not create this file.","workspace":"other"}),
            ),
        ],
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        source,
        "undeclared MCP arguments changed source"
    );
    assert!(!fixture.path().join("new.mara.md").exists());
    for id in 3..=6 {
        let result = mcp_response(&responses, id);
        assert!(
            result.get("error").is_some() || result["result"]["isError"] == true,
            "{result:#}"
        );
        assert!(result.to_string().contains("unknown field"), "{result:#}");
    }
    for tool in mcp_response(&responses, 2)["result"]["tools"]
        .as_array()
        .unwrap()
    {
        assert_eq!(
            tool["inputSchema"]["additionalProperties"], false,
            "{} accepts undeclared arguments",
            tool["name"]
        );
    }
}

// @mara implements VER-INTERFACE-PARITY
// @mara checks REQ-SURFACE-PARITY
#[test]
fn every_cli_command_argument_and_option_has_rendered_guidance() {
    let fixture = fixture();
    let mut pending = vec![Vec::<String>::new()];
    let mut visited = BTreeSet::new();
    while let Some(command) = pending.pop() {
        assert!(visited.insert(command.clone()));
        let mut arguments = command.iter().map(String::as_str).collect::<Vec<_>>();
        arguments.push("--help");
        let result = mara(fixture.path(), &arguments);
        assert!(result.status.success(), "{command:?}: {}", stderr(&result));
        assert!(result.stderr.is_empty());
        let help = stdout(&result);
        let (purpose, _) = help.split_once("Usage:").expect("help has usage");
        assert!(!purpose.trim().is_empty(), "{command:?}");
        let mut section = "";
        let mut lines = help.lines().peekable();
        while let Some(line) = lines.next() {
            if matches!(line, "Commands:" | "Arguments:" | "Options:") {
                section = line;
            } else if line.starts_with("  ")
                && !line.starts_with("          ")
                && !section.is_empty()
            {
                let (name, description) = line
                    .trim()
                    .split_once("  ")
                    .or_else(|| {
                        lines
                            .peek()
                            .filter(|next| next.starts_with("          "))
                            .map(|next| (line.trim(), next.trim()))
                    })
                    .unwrap_or_else(|| panic!("{command:?} has undocumented entry: {line}"));
                assert!(!description.trim().is_empty(), "{command:?}: {line}");
                if name.starts_with("--cursor") {
                    for convention in ["next_cursor", "unchanged", "has_more"] {
                        assert!(description.contains(convention), "{command:?}: {line}");
                    }
                }
                if name.starts_with("--limit") {
                    for convention in ["1", "100", "20", "byte budget"] {
                        assert!(description.contains(convention), "{command:?}: {line}");
                    }
                }
                if section == "Commands:" && name != "help" {
                    let mut child = command.clone();
                    child.push(name.to_owned());
                    pending.push(child);
                }
            }
        }
    }
    // Root, seven groups, 21 project operations, and the MCP server.
    assert_eq!(visited.len(), 30);
}

// @mara implements VER-INTERFACE-PARITY
// @mara checks REQ-SURFACE-PARITY
#[test]
fn mcp_input_schemas_describe_nested_parameters_and_current_invocation_conventions() {
    fn inspect(schema: &Value) {
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (name, property) in properties {
                assert!(
                    property["description"]
                        .as_str()
                        .is_some_and(|value| !value.trim().is_empty()),
                    "missing guidance for {name}: {property:#}"
                );
            }
        }
        match schema {
            Value::Object(map) => map.values().for_each(inspect),
            Value::Array(array) => array.iter().for_each(inspect),
            _ => {}
        }
    }
    let fixture = fixture();
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_request(2, "tools/list", json!({})),
        ],
    );
    let tools = mcp_response(&responses, 2)["result"]["tools"]
        .as_array()
        .unwrap();
    for tool in tools {
        assert!(
            tool["description"]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty()),
            "{tool:#}"
        );
        inspect(&tool["inputSchema"]);
        let project = &tool["inputSchema"]["properties"]["project"];
        assert!(project["description"].as_str().unwrap().contains("project"));
    }
    let get = tools.iter().find(|tool| tool["name"] == "get").unwrap();
    assert!(get["inputSchema"]["properties"].get("limit").is_none());
    let help = mara(fixture.path(), &["get", "--help"]);
    assert!(!stdout(&help).contains("--limit"));
    for name in ["item_create", "item_update"] {
        let tool = tools.iter().find(|tool| tool["name"] == name).unwrap();
        let description = tool["inputSchema"]["properties"]["body"]["description"]
            .as_str()
            .unwrap();
        assert!(
            description.contains("literal") && description.contains("required"),
            "{description}"
        );
    }
    for name in ["item_list", "search", "project_validate"] {
        let tool = tools.iter().find(|tool| tool["name"] == name).unwrap();
        let description = tool["inputSchema"]["properties"]["paths"]["description"]
            .as_str()
            .unwrap();
        assert!(
            description.contains("whole project") && description.contains("[]"),
            "{description}"
        );
    }
}
