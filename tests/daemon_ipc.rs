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
        json!({"template":"engineering"}),
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
