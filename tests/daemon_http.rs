#![cfg(any(target_os = "linux", target_os = "macos"))]
//! Real daemon processes, native socket handoff, HTTP sockets and disposable disk projects.
mod support;
use mara::{NativeRuntime, ipc, protocol::APPLICATION_VERSION};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    os::unix::{
        fs::{PermissionsExt, symlink},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
const ORIGIN: &str = "http://localhost:3210";
struct Host {
    runtime: tempfile::TempDir,
    cwd: tempfile::TempDir,
    child: Option<Child>,
    background: bool,
}
impl Host {
    fn new() -> Self {
        let base = std::env::var_os("MARA_TEST_RUNTIME_DIR")
            .or_else(|| std::env::var_os("XDG_RUNTIME_DIR"))
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let runtime = tempfile::Builder::new()
            .prefix("dh-")
            .tempdir_in(base)
            .unwrap();
        fs::set_permissions(runtime.path(), fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            runtime,
            cwd: support::fixture(),
            child: None,
            background: false,
        }
    }
    fn directory(&self) -> PathBuf {
        self.runtime.path().join("mara")
    }
    fn command(&self) -> Command {
        let mut c = support::command(self.cwd.path());
        c.args(["--format", "json", "--daemon-directory"])
            .arg(self.directory());
        c
    }
    fn cli(&self, args: &[&str]) -> Value {
        let out = self.command().args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{} {}",
            support::stdout(&out),
            support::stderr(&out)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn spawn(&mut self, projects: &[&Path], destinations: &[&Path], enabled: bool) -> Value {
        self.spawn_with_limit(projects, destinations, enabled, None)
    }
    fn spawn_with_limit(
        &mut self,
        projects: &[&Path],
        destinations: &[&Path],
        enabled: bool,
        descriptor_limit: Option<libc::rlim_t>,
    ) -> Value {
        let mut c = self.command();
        if let Some(limit) = descriptor_limit {
            // Only the fixture child receives this limit; setrlimit is async-signal-safe.
            unsafe {
                c.pre_exec(move || {
                    let limit = libc::rlimit {
                        rlim_cur: limit,
                        rlim_max: limit,
                    };
                    if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) == 0 {
                        Ok(())
                    } else {
                        Err(std::io::Error::last_os_error())
                    }
                });
            }
        }
        c.args(["daemon", "run"]);
        if enabled {
            c.args(["--http-bind", "127.0.0.1:0", "--http-origin", ORIGIN]);
        }
        for p in projects {
            c.arg("--http-project").arg(p);
        }
        for p in destinations {
            c.arg("--http-init-destination").arg(p);
        }
        self.child = Some(
            c.stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let out = self
                .command()
                .args(["daemon", "connection-info"])
                .output()
                .unwrap();
            if out.status.success() {
                return serde_json::from_slice(&out.stdout).unwrap();
            }
            assert!(
                self.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "daemon exited"
            );
            assert!(Instant::now() < deadline, "daemon not ready");
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn ipc(&self, method: &str, params: Value) -> Value {
        let stream = ipc::connect(&self.directory()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(120)))
            .unwrap();
        let mut reader = BufReader::new(stream);
        let mut result = Value::Null;
        for (method, params) in [("mara.negotiate", negotiate()), (method, params)] {
            writeln!(reader.get_mut(), "{}", request(method, params)).unwrap();
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            result = serde_json::from_str(&line).unwrap();
            assert!(result.get("error").is_none(), "{result}");
        }
        result["result"].clone()
    }
    fn stop(&mut self) {
        self.cli(&["daemon", "stop"]);
        self.background = false;
        if let Some(mut c) = self.child.take() {
            assert!(c.wait().unwrap().success());
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        // A detached child is owned through this private fixture endpoint.
        if self.background {
            let _ = self.command().args(["daemon", "stop"]).output();
        }
        // Owned foreground process handle only; panic cleanup cannot block on RPC.
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
fn negotiate() -> Value {
    json!({"protocol_version":1,"application_version":APPLICATION_VERSION})
}
fn request(method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","id":7,"method":method,"params":params})
}
fn exchange(
    info: &Value,
    method: &str,
    path: &str,
    body: &Value,
    headers: &[(&str, &str)],
) -> (u16, String, Value) {
    let address = info["endpoint"]
        .as_str()
        .unwrap()
        .strip_prefix("http://")
        .unwrap()
        .strip_suffix("/rpc")
        .unwrap();
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(120)))
        .unwrap();
    let body = body.to_string();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nContent-Length: {}\r\n",
        body.len()
    )
    .unwrap();
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n").unwrap();
    }
    write!(stream, "\r\n{body}").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    (
        status,
        head.to_owned(),
        if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(body).unwrap()
        },
    )
}
fn headers(token: &str) -> Vec<(String, String)> {
    vec![
        ("Origin".into(), ORIGIN.into()),
        ("Authorization".into(), format!("Bearer {token}")),
        ("Content-Type".into(), "application/json".into()),
        ("X-Mara-Protocol-Version".into(), "1".into()),
        (
            "X-Mara-Application-Version".into(),
            APPLICATION_VERSION.into(),
        ),
    ]
}
fn http_with(info: &Value, req: Value, h: &[(String, String)]) -> (u16, String, Value) {
    exchange(
        info,
        "POST",
        "/rpc",
        &req,
        &h.iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect::<Vec<_>>(),
    )
}
fn http(info: &Value, method: &str, params: Value) -> (u16, String, Value) {
    http_with(
        info,
        request(method, params),
        &headers(info["token"].as_str().unwrap()),
    )
}
fn ok(info: &Value, method: &str, params: Value) -> Value {
    let (status, _, body) = http(info, method, params);
    assert_eq!(status, 200, "{body}");
    assert!(body.get("error").is_none(), "{body}");
    body["result"].clone()
}
fn project() -> tempfile::TempDir {
    let f = support::fixture();
    mara::initialize_project(f.path(), mara::Template::Minimal).unwrap();
    f
}

fn query_facts_project(root: &Path) -> mara::named_query::FactSnapshot {
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
    fs::write(root.join("facts-present.ttl"), r#"@prefix sh: <http://www.w3.org/ns/shacl#>.
<urn:fixture:shape:has-fact-row> a sh:NodeShape; sh:property [ sh:path <urn:mara:query:match>; sh:minCount 1 ].
"#).unwrap();
    let keys = |verification: &str| {
        json!({
            "verification":{"type":"item","value":verification},
            "candidate":{"type":"text","value":"CAND-C"},
            "context":{"type":"text","value":"release"}
        })
    };
    let coverage = ["VER-V1", "VER-V2", "VER-V3"]
        .map(keys)
        .into_iter()
        .collect::<Vec<_>>();
    let records = [("VER-V1", "pass"), ("VER-V2", "pass"), ("VER-V3", "fail")]
        .map(|(verification, state)| {
            json!({
                "keys":keys(verification),
                "fields":{"state":{"type":"text","value":state}}
            })
        })
        .into_iter()
        .collect::<Vec<_>>();
    serde_json::from_value(json!({
        "input":"reported", "identity":"reported-run", "coverage":coverage, "records":records
    }))
    .unwrap()
}

// @mara checks REQ-LOCAL-DAEMON-HTTP
// @mara checks DES-QUERY-CONSUMER-CONTRACT
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn supplied_fact_consumers_round_trip_through_authorized_daemon_http() {
    let fixture = support::fixture();
    let root = fixture.path();
    let facts = query_facts_project(root);
    let mut host = Host::new();
    let info = host.spawn(&[root], &[], true);
    let runtime = NativeRuntime::new(None, root.to_path_buf()).unwrap();
    let call = |method: &str, mut params: Value| {
        params["project"] = json!(root);
        ok(&info, method, params)
    };
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
    assert_eq!(
        call("query_execute", serde_json::to_value(&request).unwrap()),
        serde_json::to_value(runtime.query_execute(request.clone()).unwrap()).unwrap()
    );
    let view_params = mara::query_views::ViewParams {
        request: request.clone(),
        render: None,
    };
    assert_eq!(
        call(
            "query_view",
            serde_json::to_value(view_params.clone()).unwrap()
        ),
        serde_json::to_value(runtime.query_view(view_params).unwrap()).unwrap()
    );
    let options = mara::ValidationOptions {
        fact_snapshots: vec![facts.clone()],
        ..Default::default()
    };
    assert_eq!(
        call(
            "item_validate",
            json!({"id":"CAND-C","fact_snapshots":[facts.clone()]})
        ),
        serde_json::to_value(
            runtime
                .item_validate_with_options("CAND-C", &options)
                .unwrap()
        )
        .unwrap()
    );
    let gate = mara::query_consumers::QueryGateRequest {
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
    assert_eq!(
        call("query_gate", serde_json::to_value(&gate).unwrap()),
        serde_json::to_value(runtime.query_gate(gate.clone()).unwrap()).unwrap()
    );
    for subject in [
        mara::query_views::ReportRequest::Gate(gate),
        mara::query_views::ReportRequest::DocumentationRules {
            target: mara::ValidationTarget {
                kind: mara::ValidationTargetKind::Item,
                id: Some("CAND-C".into()),
            },
            fact_snapshots: vec![facts],
            mappings: vec![],
        },
    ] {
        let expected = runtime.query_report(subject.clone(), None).unwrap();
        assert_eq!(
            call("query_report", json!({"subject":subject,"render":null})),
            serde_json::to_value(expected).unwrap()
        );
    }

    let invalid_file = "invalid-facts.json";
    fs::write(root.join(invalid_file), "not valid JSON").unwrap();
    let mut invalid_request = request.clone();
    invalid_request
        .definition_files
        .push(invalid_file.to_owned());
    assert_eq!(
        runtime
            .query_execute(invalid_request.clone())
            .unwrap_err()
            .source
            .as_deref(),
        Some(invalid_file)
    );
    let mut params = serde_json::to_value(&invalid_request).unwrap();
    params["project"] = json!(root);
    let (status, _, response) = http(&info, "query_execute", params);
    assert_eq!(status, 200, "{response}");
    assert_eq!(response["error"]["data"]["error"]["source"], invalid_file);

    let view_params = mara::query_views::ViewParams {
        request: invalid_request,
        render: None,
    };
    let (status, _, response) = http(
        &info,
        "query_view",
        json!({"project":root,"request":view_params.request,"render":null}),
    );
    assert_eq!(status, 200, "{response}");
    assert_eq!(response["error"]["data"]["error"]["source"], invalid_file);
}

// @mara checks REQ-LOCAL-DAEMON-HTTP
// @mara checks DES-LOCAL-DAEMON-PROTOCOL
#[test]
fn unauthenticated_connection_pressure_preserves_http_and_native_service() {
    for limit in [64, 32] {
        let project = project();
        let mut host = Host::new();
        let info = host.spawn_with_limit(&[project.path()], &[], true, Some(limit));
        ok(&info, "mara.negotiate", negotiate());
        let address = info["endpoint"]
            .as_str()
            .unwrap()
            .strip_prefix("http://")
            .unwrap()
            .strip_suffix("/rpc")
            .unwrap()
            .parse()
            .unwrap();
        let mut peers = Vec::new();
        for _ in 0..80 {
            match TcpStream::connect_timeout(&address, Duration::from_secs(1)) {
                Ok(stream) => peers.push(stream),
                Err(_) => break,
            }
            thread::sleep(Duration::from_millis(2));
        }
        assert!(
            peers.len() >= 8,
            "pressure fixture did not establish enough peers"
        );
        thread::sleep(Duration::from_millis(300));
        drop(peers);
        assert_eq!(host.cli(&["daemon", "status"])["state"], "running");
        ok(&info, "mara.negotiate", negotiate());
        let listed = ok(&info, "item_list", json!({"project": project.path()}));
        assert_eq!(
            listed,
            host.ipc("item_list", json!({"project": project.path()}))
        );
        host.stop();
    }
}

// @mara checks REQ-LOCAL-DAEMON-HTTP
// @mara checks DES-LOCAL-DAEMON-PROTOCOL
// @mara checks DES-DAEMON-LIFECYCLE
#[test]
fn authorized_http_initializes_mutates_reads_rolls_back_and_matches_native_results() {
    let parent = support::fixture();
    let destination = parent.path().join("new-project");
    let mut host = Host::new();
    let info = host.spawn(&[&destination], &[&destination], true);
    assert_eq!(info["available"], true);
    assert_eq!(info["token"].as_str().unwrap().len(), 64);
    assert!(
        !info["endpoint"]
            .as_str()
            .unwrap()
            .contains(info["token"].as_str().unwrap())
    );
    let capabilities = ok(&info, "mara.negotiate", negotiate());
    assert_eq!(capabilities["capabilities"]["local_methods"], json!([]));
    ok(
        &info,
        "project_init",
        json!({"project":destination,"template":"minimal"}),
    );
    assert!(destination.join(".mara/project.toml").is_file());
    ok(
        &info,
        "item_create",
        json!({"project":destination,"flavour":"requirement","id":"REQ-A","file":"a.mara.md","title":"HTTP created","body":"Persisted body."}),
    );
    assert!(
        fs::read_to_string(destination.join("a.mara.md"))
            .unwrap()
            .contains("Persisted body.")
    );
    ok(
        &info,
        "item_update",
        json!({"project":destination,"reference":"REQ-A","title":"HTTP updated"}),
    );
    let read = ok(
        &info,
        "get",
        json!({"project":destination,"reference":"REQ-A"}),
    );
    assert_eq!(read["node"]["title"], "HTTP updated");
    assert_eq!(
        read,
        host.ipc("get", json!({"project":destination,"reference":"REQ-A"}))
    );
    assert_eq!(
        ok(&info, "project_validate", json!({"project":destination}))["valid"],
        true
    );
    let source = fs::read_to_string(destination.join("a.mara.md")).unwrap();
    fs::write(destination.join("a.mara.md"), "").unwrap();
    fs::write(destination.join(".mara/transaction.json"),json!({"format_version":1,"changes":[{"path":"a.mara.md","before":source,"after":"","mode":{"readonly":false}}]}).to_string()).unwrap();
    ok(
        &info,
        "project_transaction_rollback",
        json!({"project":destination}),
    );
    assert_eq!(
        fs::read_to_string(destination.join("a.mara.md")).unwrap(),
        source
    );
    assert!(!destination.join(".mara/transaction.json").exists());
    assert_eq!(
        ok(&info, "project_refresh", json!({"project":destination})),
        json!({"refreshed":true})
    );
    let status = host.cli(&["daemon", "status"]).to_string();
    assert!(!status.contains(info["token"].as_str().unwrap()));
    assert!(
        !fs::read_to_string(destination.join(".mara/project.toml"))
            .unwrap()
            .contains(info["token"].as_str().unwrap())
    );
    host.stop();
    let address = info["endpoint"]
        .as_str()
        .unwrap()
        .trim_start_matches("http://")
        .trim_end_matches("/rpc");
    assert!(TcpStream::connect(address).is_err());
    assert!(
        !host
            .command()
            .args(["daemon", "connection-info"])
            .output()
            .unwrap()
            .status
            .success()
    );
}

// @mara checks REQ-LOCAL-DAEMON-HTTP
// @mara checks DES-LOCAL-DAEMON-PROTOCOL
#[test]
fn refuses_credentials_origins_versions_admin_and_exact_root_escapes_before_commands() {
    let allowed = project();
    let other = project();
    // An actual forbidden configured command makes dispatch observable.
    let marker = other.path().join("command-ran");
    fs::write(other.path().join("a.rs"), "pub fn forbidden() {}\n").unwrap();
    let config = fs::read_to_string(other.path().join(".mara/project.toml"))
        .unwrap()
        .replacen("format_version = 1", "format_version = 3", 1);
    fs::write(other.path().join(".mara/project.toml"),format!("{config}\n[[code.languages]]\nname = 'rust'\nextensions = ['rs']\ncommand = ['sh', '-c', 'touch command-ran; exit 1', 'sentinel', '{{output}}']\nposition_encoding = 'utf8'\n")).unwrap();
    let nested = allowed.path().join("nested");
    fs::create_dir(&nested).unwrap();
    mara::initialize_project(&nested, mara::Template::Minimal).unwrap();
    let alias = allowed.path().join("alias");
    symlink(other.path(), &alias).unwrap();
    let self_alias = other.path().join("allowed-alias");
    symlink(allowed.path(), &self_alias).unwrap();
    let dest = other.path().join("new");
    let mut host = Host::new();
    let info = host.spawn(&[&self_alias], &[&dest], true);
    let params = json!({"project":other.path()});
    for method in [
        "item_list",
        "project_refresh",
        "project_transaction_rollback",
    ] {
        assert_eq!(http(&info, method, params.clone()).0, 403);
    }
    for root in [&nested, &alias, &other.path().to_path_buf()] {
        assert_eq!(http(&info, "item_list", json!({"project":root})).0, 403);
    }
    // Canonical aliases of the exact authorized identity are accepted.
    ok(&info, "item_list", json!({"project":self_alias.join(".")}));
    ok(
        &info,
        "item_list",
        json!({"project":allowed.path().join("nested/..")}),
    );
    assert_eq!(
        http(&info, "item_list", json!({"project":alias.join("..")})).0,
        403
    );
    fs::remove_file(&self_alias).unwrap();
    symlink(other.path(), &self_alias).unwrap();
    assert_eq!(
        http(&info, "item_list", json!({"project":self_alias})).0,
        403
    );
    for token in ["", "wrong"] {
        assert_eq!(
            http_with(&info, request("item_list", params.clone()), &headers(token)).0,
            401
        );
    }
    let mut h = headers(info["token"].as_str().unwrap());
    h.remove(1);
    assert_eq!(
        http_with(&info, request("item_list", params.clone()), &h).0,
        401
    );
    let mut h = headers(info["token"].as_str().unwrap());
    h[0].1 = "http://localhost:3211".into();
    assert_eq!(
        http_with(&info, request("item_list", params.clone()), &h).0,
        403
    );
    h.remove(0);
    assert_eq!(
        http_with(&info, request("item_list", params.clone()), &h).0,
        403
    );
    let mut h = headers(info["token"].as_str().unwrap());
    h[3].1 = "99".into();
    let incompatible = http_with(&info, request("item_list", params.clone()), &h);
    assert_eq!(
        incompatible.2["error"]["data"]["error"]["code"],
        "incompatible_version"
    );
    h[3].1 = "1".into();
    h[4].1 = "0.0.0".into();
    assert_eq!(
        http_with(&info, request("item_list", params.clone()), &h).2["error"]["code"],
        -32002
    );
    h.truncate(3);
    assert_eq!(
        http_with(&info, request("item_list", params.clone()), &h).2["error"]["code"],
        -32001
    );
    assert!(!marker.exists(), "forbidden indexer executed");
    let unapproved_dest = other.path().join("not-authorized");
    assert_eq!(
        http(&info, "project_init", json!({"project":unapproved_dest})).0,
        403
    );
    assert!(!unapproved_dest.exists());
    // A symlink introduced after startup cannot retarget initialization authority.
    symlink(allowed.path(), &dest).unwrap();
    assert_eq!(http(&info, "project_init", json!({"project":dest})).0, 403);
    fs::remove_file(&dest).unwrap();
    for method in [
        "daemon_status",
        "daemon_stop",
        "daemon_connection_info",
        "daemon_set_policy",
        "daemon_set_token",
    ] {
        assert_eq!(http(&info, method, json!({})).2["error"]["code"], -32601);
    }
    assert_eq!(host.cli(&["daemon", "status"])["state"], "running");
    // HTTP authority does not narrow same-user native IPC selection.
    assert_eq!(
        host.ipc("schema_get", json!({"project":other.path()}))["kind"],
        "schema"
    );
    // A real FIFO proves refusal precedes reading forbidden project configuration.
    // Opening this file for a read would block, not produce a normal RPC error.
    let config_path = other.path().join(".mara/project.toml");
    fs::remove_file(&config_path).unwrap();
    let fifo = support::isolated_command("mkfifo", other.path())
        .arg(&config_path)
        .output()
        .unwrap();
    assert!(fifo.status.success());
    assert_eq!(http(&info, "schema_get", params.clone()).0, 403);
    fs::remove_file(&config_path).unwrap();
    fs::write(&config_path, &config).unwrap();
    let preflight = exchange(
        &info,
        "OPTIONS",
        "/rpc",
        &Value::Null,
        &[
            ("Origin", ORIGIN),
            ("Access-Control-Request-Method", "POST"),
            (
                "Access-Control-Request-Headers",
                "authorization, content-type, x-mara-protocol-version, x-mara-application-version",
            ),
        ],
    );
    assert_eq!(preflight.0, 204);
    assert!(
        preflight
            .1
            .contains(&format!("Access-Control-Allow-Origin: {ORIGIN}"))
    );
    assert!(!preflight.1.contains('*'));
    let h = headers(info["token"].as_str().unwrap());
    assert_eq!(
        exchange(
            &info,
            "POST",
            "/rpc?token=wrong",
            &request("mara.negotiate", negotiate()),
            &h.iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect::<Vec<_>>()
        )
        .0,
        404
    );
    host.stop();
}

// @mara checks REQ-LOCAL-DAEMON-HTTP
// @mara checks DES-LOCAL-DAEMON-PROTOCOL
#[test]
fn non_utf8_resolved_roots_return_errors_before_project_access() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let fixture = support::fixture();
    let root = fixture
        .path()
        .join(OsString::from_vec(b"non-utf8-\xff".to_vec()));
    fs::create_dir(&root).unwrap();
    mara::initialize_project(&root, mara::Template::Minimal).unwrap();
    let alias = fixture.path().join("utf8-alias");
    symlink(&root, &alias).unwrap();
    let ordinary = project();
    let mut host = Host::new();
    let info = host.spawn(&[&alias, ordinary.path()], &[&alias], true);
    assert_eq!(
        host.ipc("schema_get", json!({"project":alias}))["kind"],
        "schema"
    );
    // Opening the real configuration would hang. HTTP must fail while encoding
    // the authorized resolved target, before entering the project runtime.
    let config = root.join(".mara/project.toml");
    fs::remove_file(&config).unwrap();
    assert!(
        support::isolated_command("mkfifo", fixture.path())
            .arg(&config)
            .status()
            .unwrap()
            .success()
    );
    for method in ["schema_get", "project_init"] {
        let (status, _, response) = http(&info, method, json!({"project":alias}));
        assert_eq!(status, 200);
        assert_eq!(response["error"]["code"], -32602);
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains("UTF-8")
        );
    }
    ok(&info, "schema_get", json!({"project":ordinary.path()}));
    assert_eq!(host.cli(&["daemon", "status"])["state"], "running");
    host.stop();
}

// @mara checks REQ-LOCAL-DAEMON-HTTP
// @mara checks DES-DAEMON-LIFECYCLE
#[test]
fn disabled_by_default_start_policy_and_connection_info_are_local_only() {
    let mut host = Host::new();
    assert!(
        !host
            .command()
            .args(["daemon", "connection-info"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(
        host.spawn(&[], &[], false),
        json!({"available":false,"reason":"http_disabled"})
    );
    host.stop();
    for args in [
        vec!["--http-bind", "0.0.0.0:0", "--http-origin", ORIGIN],
        vec!["--http-bind", "127.0.0.1:0", "--http-origin", "*"],
        vec!["--http-bind", "127.0.0.1:0"],
        vec!["--http-origin", ORIGIN],
    ] {
        assert!(
            !host
                .command()
                .args(["daemon", "run"])
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    host.cli(&[
        "daemon",
        "start",
        "--http-bind",
        "127.0.0.1:0",
        "--http-origin",
        ORIGIN,
    ]);
    host.background = true;
    let info = host.cli(&["daemon", "connection-info"]);
    assert_eq!(info["available"], true);
    let tested = std::panic::catch_unwind(|| {
        assert_eq!(
            http(&info, "item_list", json!({"project":host.cwd.path()})).0,
            403,
            "token and origin alone grant no project authority"
        );
        let result = ok(&info, "mara.negotiate", negotiate());
        assert_eq!(result["protocol_version"], 1);
    });
    host.stop();
    if let Err(e) = tested {
        std::panic::resume_unwind(e);
    }
}

// @mara checks REQ-LOCAL-DAEMON-HTTP
// @mara checks DES-LOCAL-DAEMON-PROTOCOL
#[test]
fn authorized_http_runs_real_indexer_and_refresh_rebuilds_without_authorizing_worktrees() {
    let f = project();
    let root = f.path();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='http-fixture'\nversion='0.1.0'\nedition='2024'\n",
    )
    .unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub fn actual() -> bool { true }\n",
    )
    .unwrap();
    let audit = tempfile::tempdir().unwrap();
    let marker = audit.path().join("indexer.log");
    let script = audit.path().join("indexer.sh");
    fs::write(&script,format!("#!/bin/sh\nset -eu\nprintf '%s\\n' \"$PWD\" >> '{}'\nexec rust-analyzer scip . --output \"$1\"\n",marker.display())).unwrap();
    let config = fs::read_to_string(root.join(".mara/project.toml"))
        .unwrap()
        .replacen("format_version = 1", "format_version = 3", 1);
    fs::write(root.join(".mara/project.toml"),format!("{config}\n[[code.languages]]\nname='rust'\nextensions=['rs']\ncommand=['sh','{}','{{output}}']\nposition_encoding='utf8'\n",script.display())).unwrap();
    // A real second Git worktree is not authorized by the root's Git common directory.
    let git = support::isolated_command("git", root)
        .args(["add", "."])
        .output()
        .unwrap();
    assert!(git.status.success());
    let git = support::isolated_command("git", root)
        .args([
            "-c",
            "user.name=HTTP fixture",
            "-c",
            "user.email=http@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ])
        .output()
        .unwrap();
    assert!(git.status.success(), "{}", support::stderr(&git));
    let worktree = audit.path().join("worktree");
    let git = support::isolated_command("git", root)
        .args(["worktree", "add", "--detach"])
        .arg(&worktree)
        .output()
        .unwrap();
    assert!(git.status.success(), "{}", support::stderr(&git));
    let mut host = Host::new();
    let info = host.spawn(&[root], &[], true);
    assert_eq!(http(&info, "item_list", json!({"project":worktree})).0, 403);
    assert!(!marker.exists());
    ok(&info, "item_list", json!({"project":root}));
    ok(&info, "item_list", json!({"project":root})); // let real Cargo.lock creation settle
    let before = fs::read_to_string(&marker).unwrap();
    assert!(before.lines().all(|l| l == root.to_str().unwrap()));
    ok(&info, "item_list", json!({"project":root}));
    assert_eq!(fs::read_to_string(&marker).unwrap(), before);
    ok(&info, "project_refresh", json!({"project":root}));
    assert_eq!(fs::read_to_string(&marker).unwrap(), before);
    ok(&info, "item_list", json!({"project":root}));
    assert_eq!(
        fs::read_to_string(&marker).unwrap().lines().count(),
        before.lines().count() + 1
    );
    assert_eq!(
        ok(&info, "project_validate", json!({"project":root}))["valid"],
        true
    );
    host.stop();
}
