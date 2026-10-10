#![cfg(any(target_os = "linux", target_os = "macos"))]
mod support;
use mara::{
    ClientError, ClientOptions, ExecutionMode, NativeClient, NativeSessions, Template,
    ipc::{self, IpcServer},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

fn decode<T: DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}
fn value(v: impl Serialize) -> Value {
    serde_json::to_value(v).unwrap()
}
fn private_directory() -> tempfile::TempDir {
    let base = std::env::var_os("MARA_TEST_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = tempfile::Builder::new()
        .prefix("route-")
        .tempdir_in(base)
        .unwrap();
    fs::set_permissions(d.path(), fs::Permissions::from_mode(0o700)).unwrap();
    d
}
struct Server {
    directory: tempfile::TempDir,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<std::io::Result<()>>>,
}
impl Server {
    fn new() -> Self {
        let directory = private_directory();
        let server = IpcServer::bind_in(directory.path(), NativeSessions::default()).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let shutdown = stop.clone();
        let worker = Some(thread::spawn(move || server.serve_until(&shutdown)));
        Self {
            directory,
            stop,
            worker,
        }
    }
    fn client(&self, root: &Path, mode: ExecutionMode) -> NativeClient {
        NativeClient::new(
            Some(root.into()),
            root.into(),
            ClientOptions {
                mode: Some(mode),
                daemon_directory: Some(self.directory.path().into()),
            },
        )
        .unwrap()
    }
    fn cli(&self, cwd: &Path, mode: &str, args: &[&str]) -> Value {
        let out = support::command(cwd)
            .args([
                "--execution-mode",
                mode,
                "--daemon-directory",
                self.directory.path().to_str().unwrap(),
                "--format",
                "json",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{} {}",
            support::stdout(&out),
            support::stderr(&out)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn mcp(
        &self,
        cwd: &Path,
        mode: &str,
        project: Option<&Path>,
        requests: &[Value],
    ) -> Vec<Value> {
        let mut args = vec![
            "--execution-mode",
            mode,
            "--daemon-directory",
            self.directory.path().to_str().unwrap(),
            "mcp",
        ];
        if let Some(project) = project {
            args.extend(["--project", project.to_str().unwrap()]);
        }
        support::mcp_exchange_with_arguments(cwd, &args, requests)
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap().unwrap();
        assert!(!ipc::endpoint(self.directory.path()).exists());
    }
}
fn request(id: u64, method: &str, params: Value) -> Value {
    support::mcp_call(id, method, params)
}
fn init() -> Vec<Value> {
    vec![
        support::mcp_initialize(1),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    ]
}

// @mara checks REQ-OPTIONAL-DAEMON-EXECUTION
// @mara checks DES-NATIVE-CLIENT-ROUTING
// @mara checks REQ-SURFACE-PARITY
#[test]
fn real_clients_preserve_document_results_and_cross_backend_continuation() {
    let server = Server::new();
    let fixture = support::fixture();
    let root = fixture.path();
    let direct = server.client(root, ExecutionMode::Direct);
    let daemon = server.client(root, ExecutionMode::DaemonRequired);
    daemon
        .project_initialize(None, Template::Engineering)
        .unwrap();
    let a=daemon.item_create(decode(json!({"flavour":"requirement","id":"REQ-A","title":"A","file":"a.mara.md","fields":[{"key":"status","value":"draft"}],"body":"First."}))).unwrap();
    direct.item_create(decode(json!({"flavour":"requirement","id":"REQ-B","title":"B","file":"b.mara.md","fields":[{"key":"status","value":"draft"}],"body":"Second."}))).unwrap();
    assert_eq!(
        value(direct.schema_get(None, None).unwrap()),
        value(daemon.schema_get(None, None).unwrap())
    );
    assert_eq!(
        value(direct.schema_list(mara::SchemaKind::Relation).unwrap()),
        value(daemon.schema_list(mara::SchemaKind::Relation).unwrap())
    );
    let page = direct.item_list(decode(json!({"limit":1}))).unwrap();
    assert!(page.has_more);
    assert_eq!(
        server.cli(root, "daemon-required", &["item", "list", "--limit", "1"]),
        value(&page)
    );
    let cursor = page.next_cursor.as_deref().unwrap();
    assert_eq!(
        server.cli(
            root,
            "direct",
            &["item", "list", "--limit", "1", "--cursor", cursor]
        ),
        server.cli(
            root,
            "daemon-required",
            &["item", "list", "--limit", "1", "--cursor", cursor]
        )
    );
    let params = json!({"limit":1,"cursor":page.next_cursor});
    assert_eq!(
        value(direct.item_list(decode(params.clone())).unwrap()),
        value(daemon.item_list(decode(params)).unwrap())
    );
    assert_eq!(
        value(direct.search(decode(json!({"query":"First"}))).unwrap()),
        value(daemon.search(decode(json!({"query":"First"}))).unwrap())
    );
    let read = value(daemon.get(decode(json!({"reference":a.mid}))).unwrap());
    assert_eq!(
        read,
        value(direct.get(decode(json!({"reference":a.mid}))).unwrap())
    );
    assert_eq!(server.cli(root, "daemon-required", &["get", &a.mid]), read);
    assert_eq!(server.cli(root, "direct", &["get", &a.mid]), read);
    let edge = || decode(json!({"source":"REQ-A","relation":"derives_from","target":"REQ-B"}));
    daemon.relation_add(edge()).unwrap();
    assert_eq!(
        value(direct.relation_get(edge(), None, None).unwrap()),
        value(daemon.relation_get(edge(), None, None).unwrap())
    );
    assert_eq!(
        value(
            direct
                .related(decode(json!({"reference":"REQ-B"})))
                .unwrap()
        ),
        value(
            daemon
                .related(decode(json!({"reference":"REQ-B"})))
                .unwrap()
        )
    );
    // Duplicate relation failures retain owning edge/count context.
    assert_eq!(
        direct.relation_add(edge()).unwrap_err().envelope(),
        daemon.relation_add(edge()).unwrap_err().envelope()
    );
    daemon.relation_remove(edge()).unwrap();
    let edge = || decode(json!({"source":"REQ-A","relation":"mitigates","target":"REQ-B"}));
    assert!(daemon.relation_add(edge()).is_err());
    daemon
        .item_update(decode(json!({"reference":"REQ-A","title":"Changed"})))
        .unwrap();
    direct
        .item_move(decode(json!({"reference":"REQ-A","file":"moved.mara.md"})))
        .unwrap();
    daemon.item_rename("REQ-A", "REQ-RENAMED").unwrap();
    assert_eq!(
        daemon
            .get(decode::<mara::GetParams>(json!({"reference":a.mid})))
            .unwrap()
            .node
            .id
            .as_deref(),
        Some("REQ-RENAMED")
    );
    for c in [&direct, &daemon] {
        assert!(c.project_validate(&[]).unwrap().valid);
        assert!(c.schema_validate().unwrap().valid);
        assert!(c.item_validate("REQ-RENAMED").unwrap().valid);
        assert!(c.project_mid_backfill().unwrap().changed.is_empty());
        assert!(
            c.project_transaction_rollback()
                .unwrap()
                .restored
                .is_empty()
        );
    }
    let trace = json!({"ids":["REQ-RENAMED"],"check":{"files":[".mara/engineering-checks.yaml"],"shape":"urn:mara:rule:intent"},"limit":1});
    let matrix = direct.trace_matrix(&decode(trace.clone())).unwrap();
    assert!(matrix.evaluation_complete);
    assert_eq!(
        value(&matrix),
        value(daemon.trace_matrix(&decode(trace.clone())).unwrap())
    );
    if matrix.has_more {
        let mut next = trace.clone();
        next["cursor"] = json!(matrix.next_cursor);
        assert_eq!(
            value(direct.trace_matrix(&decode(next.clone())).unwrap()),
            value(daemon.trace_matrix(&decode(next)).unwrap())
        );
    }
    for mode in ["direct", "auto", "daemon-required"] {
        let matrix = direct.trace_matrix(&decode(trace.clone())).unwrap();
        let mut requests = init();
        requests.extend([
            request(2, "get", json!({"reference":a.mid})),
            request(4, "project_validate", json!({})),
            request(5, "trace_matrix", trace.clone()),
            request(6, "item_update", json!({"reference":"REQ-B","title":mode})),
            request(3, "item_list", json!({"limit":1})),
        ]);
        let responses = server.mcp(root, mode, None, &requests);
        for id in 2..=6 {
            assert_eq!(
                support::mcp_response(&responses, id)["result"]["isError"],
                false
            );
        }
        assert_eq!(
            support::mcp_response(&responses, 5)["result"]["structuredContent"],
            value(&matrix)
        );
        let cursor =
            &support::mcp_response(&responses, 3)["result"]["structuredContent"]["next_cursor"];
        assert!(cursor.is_string());
        let mut requests = init();
        requests.push(request(2, "item_list", json!({"limit":1,"cursor":cursor})));
        let responses = server.mcp(root, mode, None, &requests);
        assert_eq!(
            support::mcp_response(&responses, 2)["result"]["isError"],
            false
        );
        server.cli(root, mode, &["item", "update", "REQ-B", "--title", "CLI"]);
        assert!(
            server.cli(root, mode, &["project", "validate"])["valid"]
                .as_bool()
                .unwrap()
        );
        assert!(
            server.cli(
                root,
                mode,
                &[
                    "trace",
                    "matrix",
                    "--id",
                    "REQ-RENAMED",
                    "--check-file",
                    ".mara/engineering-checks.yaml",
                    "--shape",
                    "urn:mara:rule:intent"
                ]
            )["evaluation_complete"]
                .as_bool()
                .unwrap()
        );
    }
    // External invalid input is current validation data, never fallback or stale success.
    fs::write(root.join("b.mara.md"), ":::mara unknown BAD\n").unwrap();
    assert_eq!(
        value(direct.project_validate(&[]).unwrap()),
        value(daemon.project_validate(&[]).unwrap())
    );
    assert!(!daemon.project_validate(&[]).unwrap().valid);
    daemon.item_delete("REQ-RENAMED").unwrap_err();
    fs::write(root.join(".mara/schema.yaml"), "flavours: [bad\n").unwrap();
    let invalid_schema = value(direct.schema_validate().unwrap());
    assert_eq!(invalid_schema["flavours"], Value::Null);
    assert!(invalid_schema.get("flavours").is_some());
    assert_eq!(invalid_schema, value(daemon.schema_validate().unwrap()));
}

// @mara checks DES-OPERATION-PROJECT-CONTEXT
#[test]
fn caller_selection_and_initialization_never_use_server_cwd() {
    let server = Server::new();
    let fixture = support::fixture();
    let root = fixture.path();
    let caller = root.join("nested");
    fs::create_dir(&caller).unwrap();
    let client = NativeClient::new(
        None,
        caller.clone(),
        ClientOptions {
            mode: Some(ExecutionMode::DaemonRequired),
            daemon_directory: Some(server.directory.path().into()),
        },
    )
    .unwrap();
    client
        .project_initialize(Some(root.into()), Template::Minimal)
        .unwrap();
    let root_json = json!(root);
    assert_eq!(client.project_validate(&[]).unwrap().project, root);
    assert_eq!(
        server.cli(&caller, "daemon-required", &["project", "validate"])["project"],
        root_json
    );
    assert!(client.for_project(Some("relative".into())).is_err());
    assert!(
        server
            .client(root, ExecutionMode::Direct)
            .for_project(Some(root.into()))
            .is_err()
    );
    let target = root.join("created-from-caller");
    server.cli(
        &caller,
        "daemon-required",
        &["project", "init", "../created-from-caller"],
    );
    assert!(target.join(".mara/project.toml").is_file());
    for mode in ["direct", "daemon-required"] {
        let mut requests = init();
        requests.push(request(2, "project_validate", json!({"project":target})));
        let responses = server.mcp(&caller, mode, None, &requests);
        assert_eq!(
            support::mcp_response(&responses, 2)["result"]["structuredContent"]["project"],
            json!(target)
        );
        let responses = server.mcp(&caller, mode, Some(root), &requests);
        assert_eq!(
            support::mcp_response(&responses, 2)["result"]["isError"],
            true
        );
        let mut requests = init();
        requests.push(request(
            2,
            "project_init",
            json!({"project":root.join(mode)}),
        ));
        let responses = server.mcp(&caller, mode, None, &requests);
        assert_eq!(
            support::mcp_response(&responses, 2)["result"]["isError"],
            false
        );
    }
}

// @mara checks REQ-OPTIONAL-DAEMON-EXECUTION
#[test]
fn mode_precedence_and_only_established_absence_falls_back() {
    let server = Server::new();
    let fixture = support::fixture();
    let root = fixture.path();
    server
        .client(root, ExecutionMode::Direct)
        .project_initialize(None, Template::Minimal)
        .unwrap();
    let absent = server.directory.path().join("absent");
    let client = |mode| {
        NativeClient::new(
            Some(root.into()),
            root.into(),
            ClientOptions {
                mode: Some(mode),
                daemon_directory: Some(absent.clone()),
            },
        )
        .unwrap()
    };
    assert!(client(ExecutionMode::Auto).schema_get(None, None).is_ok());
    assert!(matches!(
        client(ExecutionMode::DaemonRequired).schema_get(None, None),
        Err(ClientError::Unavailable(_))
    ));
    let out = support::command(root)
        .env("MARA_EXECUTION_MODE", "daemon-required")
        .env("MARA_DAEMON_DIRECTORY", &absent)
        .args([
            "--execution-mode",
            "auto",
            "--format",
            "json",
            "schema",
            "get",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let out = support::command(root)
        .env("MARA_EXECUTION_MODE", "bad")
        .env("MARA_DAEMON_DIRECTORY", &absent)
        .args([
            "--execution-mode",
            "daemon-required",
            "--daemon-directory",
            server.directory.path().to_str().unwrap(),
            "--format",
            "json",
            "schema",
            "get",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let out = support::command(root)
        .env("MARA_EXECUTION_MODE", "daemon-required")
        .env("MARA_DAEMON_DIRECTORY", &absent)
        .args(["--format", "json", "schema", "get"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"]["code"],
        "daemon_unavailable"
    );
    let out = support::command(root)
        .env("MARA_EXECUTION_MODE", "bad")
        .args(["--format", "json", "schema", "get"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    fs::create_dir(&absent).unwrap();
    fs::set_permissions(&absent, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        client(ExecutionMode::Auto).schema_get(None, None),
        Err(ClientError::Connection(_))
    ));
    fs::set_permissions(&absent, fs::Permissions::from_mode(0o700)).unwrap();
    let socket = UnixListener::bind(ipc::endpoint(&absent)).unwrap();
    fs::set_permissions(ipc::endpoint(&absent), fs::Permissions::from_mode(0o600)).unwrap();
    drop(socket);
    // A refused/stale occupied socket is an error, not absence or recovery authority.
    assert!(matches!(
        client(ExecutionMode::Auto).schema_get(None, None),
        Err(ClientError::Connection(_))
    ));
    assert!(ipc::endpoint(&absent).exists());
}

// @mara checks DES-NATIVE-CLIENT-ROUTING
#[test]
fn non_utf8_host_paths_never_panic_and_are_encoded_only_for_selected_daemon() {
    use std::os::unix::ffi::OsStringExt;
    let server = Server::new();
    let fixture = support::fixture();
    let root = fixture.path();
    let direct = server.client(root, ExecutionMode::Direct);
    direct.project_initialize(None, Template::Minimal).unwrap();
    direct.item_create(decode(json!({"flavour":"requirement","id":"REQ-A","title":"A","file":"a.mara.md","body":"Preserve source."}))).unwrap();
    let original = fs::read(root.join("a.mara.md")).unwrap();
    let destination = PathBuf::from(std::ffi::OsString::from_vec(b"host-\xff.mara.md".to_vec()));
    let params = || mara::ItemMoveParams {
        reference: "REQ-A".into(),
        file: destination.clone(),
        line: None,
    };
    let expected = mara::NativeRuntime::new(Some(root.into()), root.into())
        .unwrap()
        .item_move(params())
        .unwrap_err();
    let absent = server.directory.path().join("absent");
    let auto = NativeClient::new(
        Some(root.into()),
        root.into(),
        ClientOptions {
            mode: Some(ExecutionMode::Auto),
            daemon_directory: Some(absent.clone()),
        },
    )
    .unwrap();
    assert_eq!(
        direct.item_move(params()).unwrap_err().to_string(),
        expected
    );
    assert_eq!(auto.item_move(params()).unwrap_err().to_string(), expected);
    assert!(matches!(
        server
            .client(root, ExecutionMode::DaemonRequired)
            .item_move(params()),
        Err(ClientError::InvalidArgument(_))
    ));
    for mode in ["direct", "auto", "daemon-required"] {
        let directory = if mode == "daemon-required" {
            server.directory.path()
        } else {
            &absent
        };
        let out = support::command(root)
            .args([
                "--execution-mode",
                mode,
                "--daemon-directory",
                directory.to_str().unwrap(),
                "--format",
                "json",
                "item",
                "move",
                "REQ-A",
            ])
            .arg(&destination)
            .output()
            .unwrap();
        assert!(!out.status.success());
        let error: Value = serde_json::from_slice(&out.stdout).unwrap();
        if mode == "daemon-required" {
            assert_eq!(error["error"]["code"], "invalid_argument");
        } else {
            assert_eq!(error["error"]["message"], expected);
        }
    }
    assert_eq!(fs::read(root.join("a.mara.md")).unwrap(), original);
    let host_root = root.join(std::ffi::OsString::from_vec(b"root-\xff".to_vec()));
    // Only Linux can create this existing-root fixture. Keep IPC argument/root
    // encoding checks on both hosts, without requiring filesystem creation.
    #[cfg(target_os = "linux")]
    {
        fs::create_dir_all(host_root.join(".mara")).unwrap();
        for name in ["project.toml", "schema.yaml"] {
            fs::copy(
                root.join(".mara").join(name),
                host_root.join(".mara").join(name),
            )
            .unwrap();
        }
        let host_direct = server.client(&host_root, ExecutionMode::Direct);
        assert!(host_direct.schema_get(None, None).is_ok());
    }
    assert!(matches!(
        server
            .client(&host_root, ExecutionMode::DaemonRequired)
            .schema_get(None, None),
        Err(ClientError::InvalidArgument(_))
    ));
}

// A fault fixture uses the actual protocol dispatcher; it is not primary workflow evidence.
fn faulty_peer(
    directory: &Path,
    mutation: bool,
    negotiation: &'static str,
) -> thread::JoinHandle<()> {
    let socket = UnixListener::bind(ipc::endpoint(directory)).unwrap();
    fs::set_permissions(ipc::endpoint(directory), fs::Permissions::from_mode(0o600)).unwrap();
    thread::spawn(move || {
        let (stream, _) = socket.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut rpc = mara::protocol::RpcSession::new(NativeSessions::default());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let mut request: Value = serde_json::from_str(&line).unwrap();
        if negotiation == "incompatible" {
            request["params"]["application_version"] = json!("incompatible");
        }
        let mut response = rpc.handle(request).unwrap();
        match negotiation {
            "formats" => response["result"]["result_formats"]["discovery"] = json!(1),
            "authentication" => {
                response = json!({"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"authentication failed","data":{"format_version":1,"error":{"code":"authentication_failed","message":"Denied"}}}})
            }
            "malformed" => response["id"] = json!(99),
            "null" => response["result"] = Value::Null,
            "types" => response["result"]["capabilities"]["operations"] = json!([1]),
            "missing" => {
                response["result"]
                    .as_object_mut()
                    .unwrap()
                    .remove("capabilities");
            }
            _ => {}
        }
        writeln!(reader.get_mut(), "{response}").unwrap();
        if negotiation != "compatible" && negotiation != "unusable-error" {
            return;
        }
        line.clear();
        reader.read_line(&mut line).unwrap();
        let response = rpc.handle_bytes(line.as_bytes()).unwrap();
        if mutation {
            assert!(response.get("result").is_some(), "{response}");
        }
        if negotiation == "unusable-error" {
            writeln!(reader.get_mut(), "{}", json!({"jsonrpc":"2.0","id":2,"error":{"code":-32000,"message":"Outcome data lost"}})).unwrap();
        }
        // Deliberately lose the operation response after actual dispatch/publication.
    })
}

// @mara checks DES-NATIVE-CLIENT-ROUTING
#[test]
fn compatibility_is_checked_before_dispatch_and_lost_mutation_is_never_replayed() {
    let fixture = support::fixture();
    let root = fixture.path();
    mara::initialize_project(root, Template::Minimal).unwrap();
    for mode in [ExecutionMode::Auto, ExecutionMode::DaemonRequired] {
        let dir = private_directory();
        let peer = faulty_peer(dir.path(), false, "incompatible");
        let c = NativeClient::new(
            Some(root.into()),
            root.into(),
            ClientOptions {
                mode: Some(mode),
                daemon_directory: Some(dir.path().into()),
            },
        )
        .unwrap();
        let result = c.schema_get(None, None);
        peer.join().unwrap();
        if mode == ExecutionMode::Auto {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(ClientError::Incompatible(_))));
        }
    }
    for negotiation in [
        "formats",
        "authentication",
        "malformed",
        "null",
        "types",
        "missing",
    ] {
        let dir = private_directory();
        let peer = faulty_peer(dir.path(), false, negotiation);
        let c = NativeClient::new(
            Some(root.into()),
            root.into(),
            ClientOptions {
                mode: Some(ExecutionMode::Auto),
                daemon_directory: Some(dir.path().into()),
            },
        )
        .unwrap();
        let result = c.schema_get(None, None);
        peer.join().unwrap();
        if negotiation == "formats" {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(ClientError::Connection(_))));
        }
    }
    let dir = private_directory();
    let peer = faulty_peer(dir.path(), false, "compatible");
    let c = NativeClient::new(
        Some(root.into()),
        root.into(),
        ClientOptions {
            mode: Some(ExecutionMode::Auto),
            daemon_directory: Some(dir.path().into()),
        },
    )
    .unwrap();
    assert!(matches!(
        c.schema_get(None, None),
        Err(ClientError::Transport(_))
    ));
    peer.join().unwrap();
    for mode in [ExecutionMode::Auto, ExecutionMode::DaemonRequired] {
        let dir = private_directory();
        let peer = faulty_peer(dir.path(), true, "compatible");
        let c = NativeClient::new(
            Some(root.into()),
            root.into(),
            ClientOptions {
                mode: Some(mode),
                daemon_directory: Some(dir.path().into()),
            },
        )
        .unwrap();
        let id = if mode == ExecutionMode::Auto {
            "REQ-A"
        } else {
            "REQ-B"
        };
        let error=c.item_create(decode(json!({"flavour":"requirement","id":id,"title":id,"file":"lost.mara.md","body":"Published despite lost response."}))).unwrap_err();
        peer.join().unwrap();
        assert!(matches!(error, ClientError::UnknownOutcome { .. }));
        assert_eq!(error.envelope()["error"]["code"], "unknown_outcome");
        assert!(error.to_string().contains("Do not retry"));
        let content = fs::read_to_string(root.join("lost.mara.md")).unwrap();
        assert_eq!(
            content
                .matches(&format!(":::mara requirement {id}\n"))
                .count(),
            1
        );
    }
    let dir = private_directory();
    let peer = faulty_peer(dir.path(), true, "unusable-error");
    let out = support::command(root)
        .args([
            "--execution-mode",
            "auto",
            "--daemon-directory",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
            "item",
            "create",
            "requirement",
            "REQ-CLI",
            "lost.mara.md",
            "--title",
            "CLI",
            "--body",
            "Published.",
        ])
        .output()
        .unwrap();
    peer.join().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"]["code"],
        "unknown_outcome"
    );
    let dir = private_directory();
    let peer = faulty_peer(dir.path(), true, "unusable-error");
    let mut requests = init();
    requests.push(request(2,"item_create",json!({"flavour":"requirement","id":"REQ-MCP","file":"lost.mara.md","title":"MCP","body":"Published."})));
    let responses = support::mcp_exchange_with_arguments(
        root,
        &[
            "--execution-mode",
            "auto",
            "--daemon-directory",
            dir.path().to_str().unwrap(),
            "mcp",
        ],
        &requests,
    );
    peer.join().unwrap();
    let result = &support::mcp_response(&responses, 2)["result"];
    assert_eq!(result["isError"], true);
    assert_eq!(
        result["structuredContent"]["error"]["code"],
        "unknown_outcome"
    );
}
