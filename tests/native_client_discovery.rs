#![cfg(any(target_os = "linux", target_os = "macos"))]
mod support;
use mara::{ClientOptions, ExecutionMode, NativeClient, RelationParams};
use serde_json::{Value, json};

// @mara checks DES-OPERATION-PROJECT-CONTEXT
// @mara checks DES-NATIVE-CLIENT-ROUTING
// @mara checks REQ-SURFACE-PARITY
#[test]
fn missing_caller_project_preserves_owning_errors_across_routing_modes() {
    let fixture = support::fixture();
    let root = fixture.path();
    let directory = root.join("absent-endpoint");
    let client = |mode| {
        NativeClient::new(
            None,
            root.into(),
            ClientOptions {
                mode: Some(mode),
                daemon_directory: Some(directory.clone()),
            },
        )
        .unwrap()
    };
    let edge = || RelationParams {
        source: "REQ-A".into(),
        relation: "depends_on".into(),
        target: "REQ-B".into(),
    };
    let direct = client(ExecutionMode::Direct);
    for mode in [ExecutionMode::Auto, ExecutionMode::DaemonRequired] {
        let c = client(mode);
        assert_eq!(
            direct.schema_validate().unwrap_err().envelope(),
            c.schema_validate().unwrap_err().envelope()
        );
        assert_eq!(
            direct.project_validate(&[]).unwrap_err().envelope(),
            c.project_validate(&[]).unwrap_err().envelope()
        );
        assert_eq!(
            direct.item_validate("REQ-A").unwrap_err().envelope(),
            c.item_validate("REQ-A").unwrap_err().envelope()
        );
        assert_eq!(
            direct
                .relation_get(edge(), None, None)
                .unwrap_err()
                .envelope(),
            c.relation_get(edge(), None, None).unwrap_err().envelope()
        );
        assert_eq!(
            direct.relation_add(edge()).unwrap_err().envelope(),
            c.relation_add(edge()).unwrap_err().envelope()
        );
        assert_eq!(
            direct.relation_remove(edge()).unwrap_err().envelope(),
            c.relation_remove(edge()).unwrap_err().envelope()
        );
    }
    let calls = [
        ("schema_validate", json!({}), vec!["schema", "validate"]),
        ("project_validate", json!({}), vec!["project", "validate"]),
        (
            "item_validate",
            json!({"id":"REQ-A"}),
            vec!["item", "validate", "REQ-A"],
        ),
        (
            "relation_get",
            json!({"source":"REQ-A","relation":"depends_on","target":"REQ-B"}),
            vec!["relation", "get", "REQ-A", "depends_on", "REQ-B"],
        ),
    ];
    let mut expected_cli = Vec::new();
    let mut expected_mcp = Vec::new();
    for mode in ["direct", "auto", "daemon-required"] {
        for (i, (method, params, args)) in calls.iter().enumerate() {
            let out = support::command(root)
                .args([
                    "--execution-mode",
                    mode,
                    "--daemon-directory",
                    directory.to_str().unwrap(),
                    "--format",
                    "json",
                ])
                .args(args)
                .output()
                .unwrap();
            assert!(!out.status.success());
            let result: Value = serde_json::from_slice(&out.stdout).unwrap();
            let responses = support::mcp_exchange_with_arguments(
                root,
                &[
                    "--execution-mode",
                    mode,
                    "--daemon-directory",
                    directory.to_str().unwrap(),
                    "mcp",
                ],
                &[
                    support::mcp_initialize(1),
                    json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                    support::mcp_call(2, method, params.clone()),
                ],
            );
            let mcp = support::mcp_response(&responses, 2)["result"].clone();
            assert_eq!(mcp["isError"], true);
            if mode == "direct" {
                expected_cli.push(result);
                expected_mcp.push(mcp);
            } else {
                assert_eq!(expected_cli[i], result);
                assert_eq!(expected_mcp[i], mcp);
            }
        }
    }
}
