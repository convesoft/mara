#![cfg(any(target_os = "linux", target_os = "macos"))]
mod support;
use mara::{
    NativeRuntime, NativeSessions,
    ipc::{self, IpcServer},
    protocol::APPLICATION_VERSION,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

struct Server {
    directory: tempfile::TempDir,
    sessions: NativeSessions,
    shutdown: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<std::io::Result<()>>>,
}
impl Server {
    fn new() -> Self {
        // Only sockets/locks live here. Documents and builds stay in disk TMPDIR.
        let base = std::env::var_os("MARA_TEST_RUNTIME_DIR")
            .or_else(|| std::env::var_os("XDG_RUNTIME_DIR"))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let directory = tempfile::Builder::new()
            .prefix("ipc-")
            .tempdir_in(base)
            .unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let sessions = NativeSessions::default();
        let server = IpcServer::bind_in(directory.path(), sessions.clone()).unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let stop = shutdown.clone();
        let worker = thread::spawn(move || server.serve_until(&stop));
        Self {
            directory,
            sessions,
            shutdown,
            worker: Some(worker),
        }
    }
    fn client(&self) -> Client {
        Client::new(self.directory.path())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap().unwrap();
        assert!(!ipc::endpoint(self.directory.path()).exists());
    }
}
struct Client {
    stream: BufReader<UnixStream>,
    next_id: u64,
}
impl Client {
    fn new(directory: &Path) -> Self {
        let stream = ipc::connect(directory).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        Self {
            stream: BufReader::new(stream),
            next_id: 0,
        }
    }
    fn send(&mut self, method: &str, params: Value) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        writeln!(
            self.stream.get_mut(),
            "{}",
            json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        )
        .unwrap();
        id
    }
    fn read(&mut self) -> Value {
        let mut line = String::new();
        assert!(self.stream.read_line(&mut line).unwrap() > 0);
        serde_json::from_str(&line).unwrap()
    }
    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.send(method, params);
        let result = self.read();
        assert_eq!(result["jsonrpc"], "2.0");
        assert_eq!(result["id"], id);
        result
    }
    fn negotiate(&mut self) -> Value {
        let response = self.call(
            "mara.negotiate",
            json!({"protocol_version":1,"application_version":APPLICATION_VERSION}),
        );
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }
    fn project(&mut self, root: &Path, method: &str, mut params: Value) -> Value {
        params["project"] = json!(root);
        self.call(method, params)
    }
    fn ok(&mut self, root: &Path, method: &str, params: Value) -> Value {
        let response = self.project(root, method, params);
        assert!(response.get("error").is_none(), "{method}: {response}");
        response["result"].clone()
    }
}
fn create(client: &mut Client, root: &Path, id: &str, file: &str) -> Value {
    client.ok(root, "item_create", json!({"flavour":"requirement","id":id,"file":file,"title":id,"body":"Real native source."}))
}

fn daemon_cli(server: &Server, root: &Path, args: &[&str]) -> Value {
    let output = support::mara_with_daemon_directory(root, server.directory.path(), args);
    assert!(
        output.status.success(),
        "{} {}",
        support::stdout(&output),
        support::stderr(&output)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn query_facts_fixture(root: &Path) -> mara::named_query::FactSnapshot {
    let snapshot: mara::SourceSnapshot =
        serde_json::from_str(include_str!("fixtures/query-facts/snapshot.json")).unwrap();
    for file in snapshot.files {
        let path = root.join(file.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, file.content).unwrap();
    }
    let project_file = root.join(".mara/project.toml");
    let mut project = fs::read_to_string(&project_file).unwrap();
    project.push_str(
        r#"
[query_policy]
format_version = 1
files = ["facts-invariants.yaml"]
definition_files = ["facts.json"]
shape_files = ["facts-present.ttl"]
max_rows = 100
"#,
    );
    fs::write(project_file, project).unwrap();
    fs::write(
        root.join("facts-invariants.yaml"),
        r#"format_version: 1
invariants:
  - id: supplied-composed-facts
    definition: urn:fixture:facts:composed
    shape: urn:fixture:shape:has-fact-row
    flavours: [candidate]
    severity: error
    parameters:
      - name: candidate
        value: {type: text, value: CAND-C}
      - name: context
        value: {type: text, value: release}
"#,
    )
    .unwrap();
    fs::write(
        root.join("facts-gates.yaml"),
        r#"format_version: 1
gates:
  - id: require-composed-facts
    empty_roots: not_applicable
    empty_required: failed
    conditions:
      - id: fact-row
        definition: urn:fixture:facts:composed
        shape_file: facts-present.ttl
        shape: urn:fixture:shape:has-fact-row
        severity: error
        context_parameters: [candidate, context]
"#,
    )
    .unwrap();
    fs::write(
        root.join("facts-present.ttl"),
        r#"@prefix sh: <http://www.w3.org/ns/shacl#>.
<urn:fixture:shape:has-fact-row> a sh:NodeShape; sh:property [ sh:path <urn:mara:query:match>; sh:minCount 1 ].
"#,
    )
    .unwrap();
    serde_json::from_value(json!({
        "input":"reported",
        "identity":"reported-run",
        "coverage":[
            {"verification":{"type":"item","value":"VER-V1"},"candidate":{"type":"text","value":"CAND-C"},"context":{"type":"text","value":"release"}},
            {"verification":{"type":"item","value":"VER-V2"},"candidate":{"type":"text","value":"CAND-C"},"context":{"type":"text","value":"release"}},
            {"verification":{"type":"item","value":"VER-V3"},"candidate":{"type":"text","value":"CAND-C"},"context":{"type":"text","value":"release"}}
        ],
        "records":[
            {"keys":{"verification":{"type":"item","value":"VER-V1"},"candidate":{"type":"text","value":"CAND-C"},"context":{"type":"text","value":"release"}},"fields":{"state":{"type":"text","value":"pass"}}},
            {"keys":{"verification":{"type":"item","value":"VER-V2"},"candidate":{"type":"text","value":"CAND-C"},"context":{"type":"text","value":"release"}},"fields":{"state":{"type":"text","value":"pass"}}},
            {"keys":{"verification":{"type":"item","value":"VER-V3"},"candidate":{"type":"text","value":"CAND-C"},"context":{"type":"text","value":"release"}},"fields":{"state":{"type":"text","value":"fail"}}}
        ]
    })).unwrap()
}

// @mara checks DES-LOCAL-DAEMON-PROTOCOL
#[test]
fn negotiation_errors_and_notifications_never_dispatch_without_compatibility() {
    let server = Server::new();
    let fixture = support::fixture();
    let root = fixture.path();
    let mut c = server.client();
    assert_eq!(
        c.project(root, "project_init", json!({}))["error"]["code"],
        -32001
    );
    assert!(!root.join(".mara").exists());
    assert_eq!(
        c.call(
            "mara.negotiate",
            json!({"protocol_version":2,"application_version":APPLICATION_VERSION})
        )["error"]["data"]["error"]["code"],
        "incompatible_version"
    );
    assert_eq!(
        c.call(
            "mara.negotiate",
            json!({"protocol_version":1,"application_version":"other"})
        )["error"]["code"],
        -32002
    );
    let capabilities = c.negotiate();
    assert_eq!(capabilities["result_formats"]["discovery"], 2);
    assert_eq!(capabilities["capabilities"]["persistence"], "filesystem");
    assert_eq!(
        capabilities["capabilities"]["operations"],
        json!(mara::protocol::OPERATIONS)
    );
    writeln!(
        c.stream.get_mut(),
        "{}",
        json!({"jsonrpc":"2.0","method":"project_init","params":{"project":root}})
    )
    .unwrap();
    assert_eq!(c.call("unknown", json!({}))["error"]["code"], -32601);
    assert!(!root.join(".mara").exists());
    assert_eq!(
        c.call("project_init", json!({"project":"relative"}))["error"]["code"],
        -32602
    );
    assert_eq!(c.call("project_init", json!({}))["error"]["code"], -32602);
    assert_eq!(
        c.project(root, "project_init", json!({"unexpected":true}))["error"]["code"],
        -32602
    );
    writeln!(c.stream.get_mut(), "not-json").unwrap();
    assert_eq!(c.read()["error"]["code"], -32700);
    writeln!(c.stream.get_mut(), "[]").unwrap();
    assert_eq!(c.read()["error"]["code"], -32600);
    c.ok(root, "project_init", json!({}));
    assert_eq!(
        c.call(
            "mara.negotiate",
            json!({"protocol_version":1,"application_version":"wrong"})
        )["error"]["code"],
        -32002
    );
    assert_eq!(
        c.project(root, "schema_get", json!({}))["error"]["code"],
        -32001
    );
    let mut second = server.client();
    assert_eq!(
        second.project(root, "schema_get", json!({}))["error"]["code"],
        -32001
    );
}

// @mara checks DES-QUERY-CONSUMER-CONTRACT
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
// @mara checks DES-LOCAL-DAEMON-PROTOCOL
#[test]
fn supplied_fact_consumers_round_trip_through_native_daemon_ipc() {
    let server = Server::new();
    let fixture = support::fixture();
    let root = fixture.path();
    let facts = query_facts_fixture(root);
    fs::write(
        root.join("facts-input.json"),
        serde_json::to_vec(&facts).unwrap(),
    )
    .unwrap();
    let mut client = server.client();
    let capabilities = client.negotiate();
    for operation in ["query_execute", "query_gate", "query_view", "query_report"] {
        assert!(
            capabilities["capabilities"]["operations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry == operation)
        );
    }
    let runtime = NativeRuntime::new(None, root.to_path_buf()).unwrap();
    let request = mara::named_query::Request {
        definition_files: vec!["facts.json".into()],
        definition: "urn:fixture:facts:composed".into(),
        selection: mara::named_query::Selection {
            ids: vec!["CAND-C".into()],
            ..Default::default()
        },
        parameters: vec![
            mara::named_query::BindingParameter {
                name: "candidate".into(),
                value: mara::named_query::ParameterValue::Text("CAND-C".into()),
            },
            mara::named_query::BindingParameter {
                name: "context".into(),
                value: mara::named_query::ParameterValue::Text("release".into()),
            },
        ],
        max_rows: 100,
        limit: 100,
        cursor: None,
        fact_snapshots: vec![facts.clone()],
        mappings: vec![],
    };
    let expected_page = runtime.query_execute(request.clone()).unwrap();
    assert_eq!(
        client.ok(
            root,
            "query_execute",
            serde_json::to_value(&request).unwrap()
        ),
        serde_json::to_value(&expected_page).unwrap()
    );
    let view_params = mara::query_views::ViewParams {
        request: request.clone(),
        render: None,
    };
    let expected_view = runtime.query_view(view_params.clone()).unwrap();
    assert_eq!(
        client.ok(
            root,
            "query_view",
            serde_json::to_value(view_params).unwrap()
        ),
        serde_json::to_value(&expected_view).unwrap()
    );
    let schema_options = mara::ValidationOptions::default();
    assert_eq!(
        client.ok(
            root,
            "schema_validate",
            serde_json::to_value(&schema_options).unwrap()
        ),
        serde_json::to_value(
            runtime
                .schema_validate_with_options(&schema_options)
                .unwrap()
        )
        .unwrap()
    );
    let options = mara::ValidationOptions {
        fact_snapshots: vec![facts.clone()],
        ..Default::default()
    };
    let expected_validation = runtime
        .item_validate_with_options("CAND-C", &options)
        .unwrap();
    assert_eq!(
        client.ok(
            root,
            "item_validate",
            json!({"id":"CAND-C","fact_snapshots":[facts.clone()]}),
        ),
        serde_json::to_value(&expected_validation).unwrap()
    );
    let gate_request = mara::query_consumers::QueryGateRequest {
        definition_files: vec!["facts.json".into()],
        gate_file: "facts-gates.yaml".into(),
        shape_files: vec!["facts-present.ttl".into()],
        gate: "require-composed-facts".into(),
        scope: request.selection.clone(),
        context: request.parameters.clone(),
        fact_snapshots: vec![facts.clone()],
        mappings: vec![],
        max_rows: 100,
    };
    let expected_gate = runtime.query_gate(gate_request.clone()).unwrap();
    assert_eq!(
        client.ok(
            root,
            "query_gate",
            serde_json::to_value(&gate_request).unwrap()
        ),
        serde_json::to_value(expected_gate).unwrap()
    );
    for subject in [
        mara::query_views::ReportRequest::Gate(gate_request.clone()),
        mara::query_views::ReportRequest::DocumentationRules {
            target: mara::ValidationTarget {
                kind: mara::ValidationTargetKind::Item,
                id: Some("CAND-C".into()),
            },
            fact_snapshots: vec![facts.clone()],
            mappings: vec![],
        },
    ] {
        let expected = runtime.query_report(subject.clone(), None).unwrap();
        assert_eq!(
            client.ok(
                root,
                "query_report",
                json!({"subject":subject,"render":null})
            ),
            serde_json::to_value(expected).unwrap()
        );
    }

    let common = [
        "--format",
        "json",
        "query",
        "execute",
        "urn:fixture:facts:composed",
        "--definition-file",
        "facts.json",
        "--id",
        "CAND-C",
        "--param",
        r#"candidate={"type":"text","value":"CAND-C"}"#,
        "--param",
        r#"context={"type":"text","value":"release"}"#,
        "--fact-file",
        "facts-input.json",
        "--max-rows",
        "100",
        "--limit",
        "100",
    ];
    let cli_page = daemon_cli(&server, root, &common);
    assert_eq!(cli_page, serde_json::to_value(&expected_page).unwrap());

    let view_args = [
        "--format",
        "json",
        "query",
        "view",
        "urn:fixture:facts:composed",
        "--definition-file",
        "facts.json",
        "--id",
        "CAND-C",
        "--param",
        r#"candidate={"type":"text","value":"CAND-C"}"#,
        "--param",
        r#"context={"type":"text","value":"release"}"#,
        "--fact-file",
        "facts-input.json",
        "--max-rows",
        "100",
        "--limit",
        "100",
    ];
    let cli_view = daemon_cli(&server, root, &view_args);
    assert_eq!(cli_view, serde_json::to_value(&expected_view).unwrap());

    let cli_validation = daemon_cli(
        &server,
        root,
        &[
            "--format",
            "json",
            "item",
            "validate",
            "CAND-C",
            "--fact-file",
            "facts-input.json",
        ],
    );
    assert_eq!(
        cli_validation,
        serde_json::to_value(&expected_validation).unwrap()
    );

    let gate_args = [
        "--format",
        "json",
        "query",
        "gate",
        "require-composed-facts",
        "--gate-file",
        "facts-gates.yaml",
        "--definition-file",
        "facts.json",
        "--shape-file",
        "facts-present.ttl",
        "--id",
        "CAND-C",
        "--param",
        r#"candidate={"type":"text","value":"CAND-C"}"#,
        "--param",
        r#"context={"type":"text","value":"release"}"#,
        "--fact-file",
        "facts-input.json",
        "--max-rows",
        "100",
    ];
    let cli_gate = daemon_cli(&server, root, &gate_args);
    let expected_gate = runtime.query_gate(gate_request.clone()).unwrap();
    assert_eq!(cli_gate, serde_json::to_value(expected_gate).unwrap());

    let gate_report_args = [
        "--format",
        "json",
        "query",
        "report",
        "gate",
        "require-composed-facts",
        "--gate-file",
        "facts-gates.yaml",
        "--definition-file",
        "facts.json",
        "--shape-file",
        "facts-present.ttl",
        "--id",
        "CAND-C",
        "--param",
        r#"candidate={"type":"text","value":"CAND-C"}"#,
        "--param",
        r#"context={"type":"text","value":"release"}"#,
        "--fact-file",
        "facts-input.json",
        "--max-rows",
        "100",
    ];
    let cli_gate_report = daemon_cli(&server, root, &gate_report_args);
    let expected_gate_report = runtime
        .query_report(
            mara::query_views::ReportRequest::Gate(gate_request.clone()),
            None,
        )
        .unwrap();
    assert_eq!(
        cli_gate_report,
        serde_json::to_value(&expected_gate_report).unwrap()
    );

    let rules_report_args = [
        "--format",
        "json",
        "query",
        "report",
        "rules",
        "--id",
        "CAND-C",
        "--fact-file",
        "facts-input.json",
    ];
    let cli_rules_report = daemon_cli(&server, root, &rules_report_args);
    let rule_subject = mara::query_views::ReportRequest::DocumentationRules {
        target: mara::ValidationTarget {
            kind: mara::ValidationTargetKind::Item,
            id: Some("CAND-C".into()),
        },
        fact_snapshots: vec![facts.clone()],
        mappings: vec![],
    };
    let expected_rules_report = runtime.query_report(rule_subject.clone(), None).unwrap();
    assert_eq!(
        cli_rules_report,
        serde_json::to_value(&expected_rules_report).unwrap()
    );

    let mut view_params = serde_json::to_value(&request).unwrap();
    view_params["render"] = Value::Null;
    let mcp_requests = [
        support::mcp_initialize(1),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        support::mcp_call(2, "query_execute", serde_json::to_value(&request).unwrap()),
        support::mcp_call(3, "query_view", view_params),
        support::mcp_call(
            4,
            "item_validate",
            json!({"id":"CAND-C","fact_snapshots":[facts.clone()]}),
        ),
        support::mcp_call(
            5,
            "query_gate",
            serde_json::to_value(&gate_request).unwrap(),
        ),
        support::mcp_call(
            6,
            "query_report",
            json!({"subject":mara::query_views::ReportRequest::Gate(gate_request.clone()),"render":null}),
        ),
        support::mcp_call(
            7,
            "query_report",
            json!({"subject":rule_subject,"render":null}),
        ),
    ];
    let responses = support::mcp_exchange_with_daemon_directory(
        root,
        server.directory.path(),
        &["mcp"],
        &mcp_requests,
    );
    for (id, expected) in [
        (2, serde_json::to_value(&expected_page).unwrap()),
        (3, serde_json::to_value(&expected_view).unwrap()),
        (4, serde_json::to_value(&expected_validation).unwrap()),
        (
            5,
            serde_json::to_value(runtime.query_gate(gate_request.clone()).unwrap()).unwrap(),
        ),
        (6, serde_json::to_value(&expected_gate_report).unwrap()),
        (7, serde_json::to_value(&expected_rules_report).unwrap()),
    ] {
        let response = support::mcp_response(&responses, id);
        assert_ne!(response["result"]["isError"], true, "{response:#}");
        assert_eq!(
            response["result"]["structuredContent"], expected,
            "{response:#}"
        );
    }

    let invalid_file = "invalid-facts.json";
    fs::write(root.join(invalid_file), "not valid JSON").unwrap();
    let mut invalid_request = request.clone();
    invalid_request
        .definition_files
        .push(invalid_file.to_owned());
    let direct_error = runtime.query_execute(invalid_request.clone()).unwrap_err();
    assert_eq!(direct_error.source.as_deref(), Some(invalid_file));
    let daemon_error = client.project(
        root,
        "query_execute",
        serde_json::to_value(&invalid_request).unwrap(),
    );
    assert_eq!(
        daemon_error["error"]["data"]["error"]["source"],
        invalid_file
    );
    assert_eq!(
        daemon_error["error"]["data"]["error"]["code"],
        direct_error.code
    );

    let invalid_view = mara::query_views::ViewParams {
        request: invalid_request,
        render: None,
    };
    let direct_error = runtime.query_view(invalid_view.clone()).unwrap_err();
    assert_eq!(direct_error.source.as_deref(), Some(invalid_file));
    let daemon_error = client.project(
        root,
        "query_view",
        serde_json::to_value(&invalid_view).unwrap(),
    );
    assert_eq!(
        daemon_error["error"]["data"]["error"]["source"],
        invalid_file
    );
    assert_eq!(
        daemon_error["error"]["data"]["error"]["code"],
        direct_error.code
    );
}

// @mara checks DES-LOCAL-DAEMON-PROTOCOL
// @mara checks DES-EXECUTION-BOUNDARIES
#[test]
fn real_socket_dispatches_complete_surface_and_guarded_filesystem_mutations() {
    let server = Server::new();
    let fixture = support::fixture();
    let root = fixture.path();
    let mut c = server.client();
    c.negotiate();
    c.ok(root, "project_init", json!({}));
    c.ok(root, "schema_get", json!({}));
    c.ok(root, "schema_list", json!({"kind":"flavour"}));
    assert_eq!(c.ok(root, "schema_validate", json!({}))["valid"], true);
    let a = create(&mut c, root, "REQ-A", "a.mara.md");
    create(&mut c, root, "REQ-B", "b.mara.md");
    let edge = json!({"source":"REQ-A","relation":"depends_on","target":"REQ-B"});
    c.ok(root, "relation_add", edge.clone());
    let inspected = c.ok(root, "relation_get", edge.clone());
    assert_eq!(inspected["format_version"], 1);
    let mut remove = edge.clone();
    remove["occurrence"] = inspected["occurrences"][0]["reference"].clone();
    c.ok(root, "relation_remove", remove);
    c.ok(root, "related", json!({"reference":"REQ-A"}));
    let before = fs::read(root.join("a.mara.md")).unwrap();
    fs::write(root.join(".mara/transaction.json"), "pending").unwrap();
    let denied = c.project(
        root,
        "item_update",
        json!({"reference":"REQ-A","title":"must not publish"}),
    );
    assert_eq!(denied["error"]["data"]["error"]["code"], "operation_failed");
    assert_eq!(fs::read(root.join("a.mara.md")).unwrap(), before);
    fs::remove_file(root.join(".mara/transaction.json")).unwrap();
    fs::set_permissions(root.join("a.mara.md"), fs::Permissions::from_mode(0o640)).unwrap();
    c.ok(
        root,
        "item_update",
        json!({"reference":"REQ-A","title":"Updated"}),
    );
    assert_eq!(
        fs::metadata(root.join("a.mara.md"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    c.ok(
        root,
        "item_move",
        json!({"reference":"REQ-A","file":"moved.mara.md"}),
    );
    c.ok(
        root,
        "item_rename",
        json!({"reference":"REQ-A","new_id":"REQ-RENAMED"}),
    );
    assert_eq!(
        c.ok(root, "get", json!({"reference":a["mid"]}))["node"]["title"],
        "Updated"
    );
    c.ok(root, "search", json!({"query":"Updated"}));
    let first = c.ok(root, "item_list", json!({"limit":1}));
    assert_eq!(first["has_more"], true);
    let last = c.ok(
        root,
        "item_list",
        json!({"limit":1,"cursor":first["next_cursor"]}),
    );
    assert_eq!(last["has_more"], false);
    assert_ne!(first["items"][0]["id"], last["items"][0]["id"]);
    assert_eq!(c.ok(root, "project_validate", json!({}))["valid"], true);
    assert_eq!(
        c.ok(root, "item_validate", json!({"id":"REQ-RENAMED"}))["valid"],
        true
    );
    c.ok(root, "project_mid_backfill", json!({}));
    c.ok(root, "project_transaction_rollback", json!({}));
    c.ok(root, "project_refresh", json!({}));
    c.ok(root, "item_delete", json!({"reference":"REQ-RENAMED"}));
    assert_eq!(fs::read(root.join("moved.mara.md")).unwrap(), b"");
    assert_eq!(
        c.project(
            root,
            "item_list",
            json!({"limit":1,"cursor":first["next_cursor"]})
        )["error"]["code"],
        -32000
    );
    // Current invalid source is a domain result, never an RPC error or cached success.
    fs::write(root.join("b.mara.md"), ":::mara unknown BAD\n").unwrap();
    let invalid = c.ok(root, "project_validate", json!({"limit":1}));
    assert_eq!(invalid["valid"], false);
    assert_eq!(invalid["format_version"], 1);
    assert!(invalid["summary"]["errors"].as_u64().unwrap() > 0);
    let error = c.project(root, "get", json!({"reference":"REQ-B"}));
    assert_eq!(error["error"]["code"], -32000);
    assert!(
        !error["error"]["message"]
            .as_str()
            .unwrap()
            .chars()
            .any(char::is_control)
    );
    let other = support::fixture();
    c.ok(
        other.path(),
        "project_init",
        json!({"template":"engineering-legacy"}),
    );
    c.ok(other.path(), "item_create", json!({"flavour":"requirement","id":"REQ-A","file":"a.mara.md","title":"Trace root","fields":[{"key":"status","value":"draft"}],"body":"Draft."}));
    let trace = c.ok(other.path(), "trace_matrix", json!({"ids":["REQ-A"],"check":{"files":[".mara/engineering-checks.yaml"],"shape":"urn:mara:rule:intent"}}));
    assert_eq!(trace["evaluation_complete"], true);
    assert_eq!(trace["format_version"], 1);
}

// @mara checks DES-NATIVE-PROJECT-SESSIONS
// @mara checks REQ-USER-DAEMON-LIFECYCLE
#[test]
fn shared_roots_real_worktrees_reconciliation_and_consumer_disconnect_independence() {
    let server = Server::new();
    let fixture = support::fixture();
    let root = fixture.path();
    let mut c = server.client();
    c.negotiate();
    c.ok(root, "project_init", json!({}));
    create(&mut c, root, "REQ-A", "a.mara.md");
    c.ok(root, "get", json!({"reference":"REQ-A"}));
    let runtime = NativeRuntime::new(Some(root.to_owned()), root.to_owned())
        .unwrap()
        .with_sessions(server.sessions.clone());
    let counts = runtime.session_activity().unwrap();
    let alias_dir = tempfile::tempdir().unwrap();
    let alias = alias_dir.path().join("alias");
    std::os::unix::fs::symlink(root, &alias).unwrap();
    let mut observer = server.client();
    observer.negotiate();
    observer.ok(&alias, "get", json!({"reference":"REQ-A"}));
    assert_eq!(runtime.session_activity().unwrap(), counts);
    for args in [
        vec!["add", "."],
        vec![
            "-c",
            "user.name=IPC test",
            "-c",
            "user.email=ipc@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
    ] {
        let output = support::isolated_command("git", root)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", support::stderr(&output));
    }
    let worktree_parent = tempfile::tempdir().unwrap();
    let worktree = worktree_parent.path().join("worktree");
    let output = support::isolated_command("git", root)
        .args(["worktree", "add", "--quiet", "--detach"])
        .arg(&worktree)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", support::stderr(&output));
    observer.ok(
        &worktree,
        "item_update",
        json!({"reference":"REQ-A","title":"Other worktree"}),
    );
    assert_eq!(
        observer.ok(root, "get", json!({"reference":"REQ-A"}))["node"]["title"],
        "REQ-A"
    );
    assert_eq!(
        observer.ok(&worktree, "get", json!({"reference":"REQ-A"}))["node"]["title"],
        "Other worktree"
    );
    let source = fs::read_to_string(root.join("a.mara.md")).unwrap();
    fs::write(
        root.join("a.mara.md"),
        source.replace(":title: REQ-A", ":title: External edit"),
    )
    .unwrap();
    assert_eq!(
        observer.ok(root, "get", json!({"reference":"REQ-A"}))["node"]["title"],
        "External edit"
    );
    // Block the real publisher, send a complete mutation, then lose its response.
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(".mara/mutation.lock"))
        .unwrap();
    lock.lock().unwrap();
    c.send(
        "item_update",
        json!({"project":root,"reference":"REQ-A","title":"Disconnected mutation"}),
    );
    drop(c);
    lock.unlock().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let title = observer.ok(root, "get", json!({"reference":"REQ-A"}))["node"]["title"].clone();
        if title == "Disconnected mutation" {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "disconnected operation did not finish"
        );
        thread::sleep(Duration::from_millis(10));
    }
    drop(observer);
    let mut later = server.client();
    later.negotiate();
    assert_eq!(later.ok(root, "project_validate", json!({}))["valid"], true);
    // Shutdown closes idle consumers but never owns their project data.
    assert!(root.join("a.mara.md").exists());
}

// @mara checks DES-LOCAL-DAEMON-PROTOCOL
#[test]
fn private_endpoint_ownership_and_framing_preserve_existing_state() {
    let server = Server::new();
    let dir = server.directory.path();
    assert_eq!(
        fs::metadata(dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(ipc::endpoint(dir))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(IpcServer::bind_in(dir, NativeSessions::default()).is_err());
    let mut c = server.client();
    c.stream
        .get_mut()
        .write_all(&vec![b'x'; ipc::MAX_REQUEST_BYTES as usize + 1])
        .unwrap();
    assert_eq!(c.read()["error"]["code"], -32600);
    let mut later = server.client();
    later.negotiate();
    let base = dir.parent().unwrap();
    let private = tempfile::tempdir_in(base).unwrap();
    fs::set_permissions(private.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let sentinel = ipc::endpoint(private.path());
    fs::write(&sentinel, b"never unlink stale state").unwrap();
    assert!(IpcServer::bind_in(private.path(), NativeSessions::default()).is_err());
    assert_eq!(fs::read(&sentinel).unwrap(), b"never unlink stale state");
    let link = private.path().join("link");
    std::os::unix::fs::symlink(dir, &link).unwrap();
    assert!(ipc::connect(&link).is_err());
    assert!(ipc::connect(&link.join("")).is_err());
    assert!(ipc::connect(&link.join(".")).is_err());
    fs::set_permissions(private.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(IpcServer::bind_in(private.path(), NativeSessions::default()).is_err());
    assert!(ipc::connect(private.path()).is_err());
    // An unterminated frame is not a project request, even after negotiation.
    let fixture = support::fixture();
    let mut partial = server.client();
    partial.negotiate();
    write!(
        partial.stream.get_mut(),
        "{}",
        json!({"jsonrpc":"2.0","id":2,"method":"project_init","params":{"project":fixture.path()}})
    )
    .unwrap();
    drop(partial);
    assert_eq!(later.call("unknown", json!({}))["error"]["code"], -32601);
    assert!(!fixture.path().join(".mara").exists());
}
