//! The MCP lifecycle, as clients with the `initialize` handshake speak it, and the tool list.

use serde_json::json;

use crate::{Dir, Server, usage};

/// The document operations of DES-OPERATIONS, as tools.
const TOOLS: [&str; 19] = [
    "project_init",
    "project_validate",
    "project_fix",
    "project_convert",
    "schema_get",
    "schema_list",
    "schema_validate",
    "item_create",
    "item_update",
    "item_move",
    "item_rename",
    "item_delete",
    "item_list",
    "item_validate",
    "relation_add",
    "relation_remove",
    "relation_list",
    "search",
    "get",
];

#[test]
fn initialize_answers_with_the_requested_version_or_the_latest() {
    let dir = Dir::new();
    for (requested, answered) in [
        ("2025-11-25", "2025-11-25"),
        ("2025-06-18", "2025-06-18"),
        ("2025-03-26", "2025-03-26"),
        ("2024-11-05", "2024-11-05"),
        ("2099-01-01", "2025-11-25"),
    ] {
        let mut server = Server::spawn(&dir.root(), &[]);
        let response = server.request(
            "initialize",
            json!({"protocolVersion": requested, "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}),
        );
        let result = &response["result"];
        assert_eq!(result["protocolVersion"], answered, "{response}");
        assert_eq!(result["capabilities"], json!({"tools": {}}), "{response}");
        assert_eq!(result["serverInfo"]["name"], "mara");
        assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
        let (status, _) = server.finish();
        assert!(status.success());
    }
}

/// A client of the 2026-07-28 revision opens no session: it discovers the server and sends its
/// version and capabilities with every request. It gets the same tools and results.
#[test]
fn a_client_without_the_initialize_handshake_gets_the_same_tools_and_results() {
    let dir = crate::engineering_project();
    let meta = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {"name": "t", "version": "1"},
    });
    let mut modern = Server::spawn(&dir.root(), &[]);
    let discovered = modern.request("server/discover", json!({"_meta": meta}));
    let versions = &discovered["result"]["supportedVersions"];
    for version in ["2026-07-28", "2025-11-25", "2024-11-05"] {
        assert!(
            versions.as_array().unwrap().contains(&json!(version)),
            "{discovered}"
        );
    }
    let listed = modern.request("tools/list", json!({"_meta": meta}));
    let tools = listed["result"]["tools"].as_array().unwrap().clone();
    let mut legacy = Server::start(&dir.root(), &[]);
    assert_eq!(tools, legacy.tools());
    for (tool, arguments) in [
        ("item_list", json!({})),
        ("get", json!({"node": "REQ-COLD-READ-TIME"})),
        ("get", json!({"node": "REQ-NOPE"})),
    ] {
        let params = json!({"name": tool, "arguments": arguments, "_meta": meta});
        let response = modern.request("tools/call", params);
        let call = legacy.call(tool, arguments);
        let content = &response["result"]["content"][0];
        assert_eq!(content["text"], call.text.as_str(), "{response}");
        assert_eq!(response["result"]["isError"], call.is_error, "{response}");
    }
    assert!(modern.finish().0.success());
    assert!(legacy.finish().0.success());
}

/// The protocol errors this server makes: an unknown tool, and `ping` at any time.
#[test]
fn an_unknown_tool_is_a_protocol_error() {
    let dir = Dir::new();
    let mut server = Server::start(&dir.root(), &[]);
    let unknown = server.request(
        "tools/call",
        json!({"name": "no_such_tool", "arguments": {}}),
    );
    assert_eq!(unknown["error"]["code"], -32602, "{unknown}");
    assert!(
        unknown["error"]["message"]
            .as_str()
            .unwrap()
            .contains("no_such_tool")
    );
    let malformed = server.request("tools/call", json!({"name": "item_list", "arguments": []}));
    assert!(malformed["error"]["code"].is_i64(), "{malformed}");
    assert_eq!(server.request("ping", json!({}))["result"], json!({}));
    // The session goes on.
    assert_eq!(server.tools().len(), TOOLS.len());
    let (status, stats) = server.finish();
    assert!(status.success());
    assert_eq!(stats, crate::no_work());
}

/// One tool per document operation, and each tool's parameters are the arguments and options of
/// its CLI operation, as the CLI's usage lists them.
#[test]
fn every_document_operation_is_a_tool_with_its_arguments_and_options() {
    let dir = Dir::new();
    let mut server = Server::start(&dir.root(), &[]);
    let tools = server.tools();
    let mut names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    let mut expected = TOOLS.to_vec();
    expected.sort_unstable();
    assert_eq!(names, expected);
    let usages = usage();
    let served: Vec<String> = usages
        .iter()
        .filter(|usage| usage.words != ["mcp"])
        .map(|usage| usage.words.join("_"))
        .collect();
    assert_eq!(served.len(), TOOLS.len(), "{served:?}");
    for usage in usages.iter().filter(|usage| usage.words != ["mcp"]) {
        let name = usage.words.join("_");
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == name.as_str())
            .unwrap();
        assert!(!tool["description"].as_str().unwrap().is_empty(), "{tool}");
        let schema = &tool["inputSchema"];
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false, "{tool}");
        let properties = schema["properties"].as_object().unwrap();
        let mut keys: Vec<&str> = properties.keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut declared: Vec<&str> = (usage.arguments.iter())
            .chain(usage.options.iter().map(|(name, _)| name))
            .map(String::as_str)
            .chain(["project"])
            .collect();
        declared.sort_unstable();
        assert_eq!(keys, declared, "{name}");
        let required: Vec<&str> = schema.get("required").map_or(Vec::new(), |required| {
            required
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r.as_str().unwrap())
                .collect()
        });
        for argument in &usage.arguments {
            assert_eq!(properties[argument]["type"], "string", "{name} {argument}");
            assert!(required.contains(&argument.as_str()), "{name} {argument}");
        }
        for (option, repeatable) in &usage.options {
            let property = &properties[option];
            assert!(property["description"].is_string(), "{name} {option}");
            let kind = match *repeatable {
                true => {
                    assert_eq!(property["type"], "array", "{name} {option}");
                    &property["items"]["type"]
                }
                false => &property["type"],
            };
            let integer = option == "limit" || option == "line";
            assert_eq!(
                kind,
                if integer { "integer" } else { "string" },
                "{name} {option}"
            );
        }
        let options_required: Vec<&&str> = required
            .iter()
            .filter(|name| !usage.arguments.iter().any(|argument| argument == *name))
            .collect();
        let expected: Vec<&&str> = match name.as_str() {
            "item_create" => vec![&"title", &"body"],
            _ => Vec::new(),
        };
        assert_eq!(options_required, expected, "{name}");
    }
    assert!(server.finish().0.success());
    // Bound to a project, no tool takes one.
    let mut bound = Server::start(&dir.root(), &["--project", "."]);
    for tool in bound.tools() {
        assert!(
            tool["inputSchema"]["properties"].get("project").is_none(),
            "{tool}"
        );
    }
    assert!(bound.finish().0.success());
}
