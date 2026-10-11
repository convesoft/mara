//! Native SYSTEM acceptance: real lifecycle process, Rust client, CLI and stdio MCP.
#![cfg(any(target_os = "linux", target_os = "macos"))]
mod support;

use mara::{ClientError, ClientOptions, ExecutionMode, NativeClient, Template, ipc};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
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
        .prefix("ns-")
        .tempdir_in(base)
        .unwrap();
    fs::set_permissions(d.path(), fs::Permissions::from_mode(0o700)).unwrap();
    d
}
struct Host {
    runtime: tempfile::TempDir,
    cwd: tempfile::TempDir,
    child: Option<Child>,
}
impl Host {
    fn new() -> Self {
        let mut host = Self {
            runtime: private_directory(),
            cwd: support::fixture(),
            child: None,
        };
        host.child = Some(
            host.command(host.cwd.path(), "direct")
                .args(["daemon", "run"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while ipc::connect(host.runtime.path()).is_err() {
            assert!(
                host.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "daemon exited"
            );
            assert!(Instant::now() < deadline, "daemon not ready");
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            host.cli(host.cwd.path(), "direct", &["daemon", "status"])["state"],
            "running"
        );
        host
    }
    fn command(&self, cwd: &Path, mode: &str) -> Command {
        let mut command = support::command(cwd);
        command
            .args(["--execution-mode", mode, "--daemon-directory"])
            .arg(self.runtime.path())
            .args(["--format", "json"]);
        command
    }
    fn cli(&self, cwd: &Path, mode: &str, args: &[&str]) -> Value {
        let out = self.command(cwd, mode).args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {} {}",
            support::stdout(&out),
            support::stderr(&out)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn client(&self, root: &Path, mode: ExecutionMode) -> NativeClient {
        NativeClient::new(
            Some(root.into()),
            root.into(),
            ClientOptions {
                mode: Some(mode),
                daemon_directory: Some(self.runtime.path().into()),
            },
        )
        .unwrap()
    }
    fn mcp(&self, root: &Path, mode: &str, tool: &str, params: Value) -> Value {
        let responses = support::mcp_exchange_with_arguments(
            root,
            &[
                "--execution-mode",
                mode,
                "--daemon-directory",
                self.runtime.path().to_str().unwrap(),
                "mcp",
            ],
            &[
                support::mcp_initialize(1),
                support::mcp_call(2, tool, params),
            ],
        );
        let response = &support::mcp_response(&responses, 2)["result"];
        assert_eq!(response["isError"], false, "{response}");
        response["structuredContent"].clone()
    }
    fn stop(&mut self) {
        let out = self
            .command(self.cwd.path(), "direct")
            .args(["daemon", "stop"])
            .output()
            .unwrap();
        if !out.status.success() {
            if let Some(mut child) = self.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            panic!(
                "daemon stop failed: {} {}",
                support::stdout(&out),
                support::stderr(&out)
            );
        }
        if let Some(mut child) = self.child.take() {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(exit) = child.try_wait().unwrap() {
                    assert!(exit.success());
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("daemon did not exit");
                }
                thread::sleep(Duration::from_millis(20));
            }
            assert!(!ipc::endpoint(self.runtime.path()).exists());
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop();
    }
}
fn document(id: &str, mid: &str, title: &str) -> String {
    format!(
        ":::mara requirement {id}\n:mid: {mid}\n:title: {title}\n:status: draft\n\nBody.\n:::\n"
    )
}
fn seeded(root: &Path) {
    mara::initialize_project(root, Template::EngineeringLegacy).unwrap();
    fs::write(
        root.join("a.mara.md"),
        document("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "A"),
    )
    .unwrap();
    fs::write(
        root.join("b.mara.md"),
        document("REQ-B", "01ARZ3NDEKTSV4RRFFQ69G5F01", "B"),
    )
    .unwrap();
}
fn get(client: &NativeClient, reference: &str) -> Value {
    value(client.get(decode(json!({"reference":reference}))).unwrap())
}

// @mara implements VER-NATIVE-EXECUTION-BACKENDS
// @mara checks REQ-FRESH-PROJECT-SESSIONS
// @mara checks DES-NATIVE-PROJECT-SESSIONS
// @mara checks DES-NATIVE-CLIENT-ROUTING
#[test]
fn lifecycle_clients_reconcile_current_inputs_and_preserve_domain_continuations() {
    let host = Host::new();
    let f = support::fixture();
    let root = f.path();
    seeded(root);
    let direct = host.client(root, ExecutionMode::Direct);
    let daemon = host.client(root, ExecutionMode::DaemonRequired);
    let cold = Instant::now();
    let read = get(&daemon, "REQ-A");
    let cold = cold.elapsed();
    let warm = Instant::now();
    assert_eq!(get(&daemon, "REQ-A"), read);
    let warm = warm.elapsed();
    println!(
        "lifecycle native daemon document cold={cold:?} warm={warm:?}; observation, not a speedup gate"
    );
    assert_eq!(get(&direct, "REQ-A"), read);
    assert_eq!(host.cli(root, "daemon-required", &["get", "REQ-A"]), read);
    assert_eq!(
        host.mcp(root, "daemon-required", "get", json!({"reference":"REQ-A"})),
        read
    );
    let page = direct.item_list(decode(json!({"limit":1}))).unwrap();
    let next = json!({"limit":1,"cursor":page.next_cursor});
    assert_eq!(
        value(direct.item_list(decode(next.clone())).unwrap()),
        value(daemon.item_list(decode(next.clone())).unwrap())
    );
    fs::write(
        root.join("a.mara.md"),
        document("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "External"),
    )
    .unwrap();
    assert_eq!(get(&daemon, "REQ-A")["node"]["title"], "External");
    assert_eq!(get(&direct, "REQ-A"), get(&daemon, "REQ-A"));
    assert!(
        daemon.item_list(decode(next)).is_err(),
        "stale continuation accepted"
    );
    fs::write(
        root.join("added.mara.md"),
        document("REQ-C", "01ARZ3NDEKTSV4RRFFQ69G5F02", "Added"),
    )
    .unwrap();
    assert_eq!(daemon.item_list(Default::default()).unwrap().items.len(), 3);
    fs::write(root.join(".gitignore"), "added.mara.md\n").unwrap();
    assert_eq!(daemon.item_list(Default::default()).unwrap().items.len(), 2);
    fs::remove_file(root.join(".gitignore")).unwrap();
    fs::remove_file(root.join("added.mara.md")).unwrap();
    assert_eq!(daemon.item_list(Default::default()).unwrap().items.len(), 2);

    let trace = json!({"ids":["REQ-A"],"check":{"files":[".mara/engineering-checks.yaml"],"shape":"urn:mara:rule:intent"}});
    assert_eq!(
        value(direct.trace_matrix(&decode(trace.clone())).unwrap()),
        value(daemon.trace_matrix(&decode(trace.clone())).unwrap())
    );
    let request_rule = root.join(".mara/engineering-checks.yaml");
    let before = fs::read(&request_rule).unwrap();
    fs::write(&request_rule, "not: [valid yaml\n").unwrap();
    let invalid_check = daemon.trace_matrix(&decode(trace.clone())).unwrap();
    assert!(!invalid_check.evaluation_complete);
    assert_eq!(
        value(&invalid_check),
        value(direct.trace_matrix(&decode(trace.clone())).unwrap())
    );
    fs::write(&request_rule, before).unwrap();
    assert!(
        daemon
            .trace_matrix(&decode(trace))
            .unwrap()
            .evaluation_complete
    );

    for path in [
        "a.mara.md",
        ".mara/schema.yaml",
        ".mara/engineering-rules.yaml",
        ".mara/project.toml",
    ] {
        let target = root.join(path);
        let before = fs::read(&target).unwrap();
        fs::write(&target, "invalid: [\n:::mara unknown BAD\n").unwrap();
        let current = value(daemon.project_validate(&[]).unwrap());
        assert_eq!(current["valid"], false, "stale success for {path}");
        assert_eq!(current, value(direct.project_validate(&[]).unwrap()));
        assert_eq!(
            host.mcp(root, "daemon-required", "project_validate", json!({})),
            current
        );
        fs::write(&target, before).unwrap();
        assert!(
            daemon.project_validate(&[]).unwrap().valid,
            "did not recover after {path}"
        );
    }
    assert_eq!(
        host.cli(root, "direct", &["daemon", "refresh"]),
        json!({"refreshed":true})
    );
    assert_eq!(get(&direct, "REQ-A"), get(&daemon, "REQ-A"));
}

// @mara implements VER-NATIVE-EXECUTION-BACKENDS
// @mara checks REQ-USER-DAEMON-LIFECYCLE
// @mara checks DES-NATIVE-PROJECT-SESSIONS
#[test]
fn lifecycle_daemon_isolates_real_worktrees_and_restart_preserves_sources() {
    let mut host = Host::new();
    let parent = support::fixture();
    let left = parent.path().join("left");
    let right = parent.path().join("right");
    fs::create_dir(&left).unwrap();
    let git = |args: &[&str]| {
        let output = support::isolated_command("git", &left)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", support::stderr(&output));
    };
    git(&["init", "--quiet"]);
    seeded(&left);
    git(&["add", "."]);
    git(&[
        "-c",
        "user.name=Native System",
        "-c",
        "user.email=native@example.invalid",
        "commit",
        "--quiet",
        "-m",
        "fixture",
    ]);
    git(&[
        "worktree",
        "add",
        "--quiet",
        right.to_str().unwrap(),
        "-b",
        "other",
    ]);
    fs::write(
        right.join("a.mara.md"),
        document("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "Right"),
    )
    .unwrap();
    let a = host.client(&left, ExecutionMode::DaemonRequired);
    let b = host.client(&right, ExecutionMode::DaemonRequired);
    assert_eq!(get(&a, "REQ-A")["node"]["title"], "A");
    assert_eq!(get(&b, "REQ-A")["node"]["title"], "Right");
    b.item_update(decode(json!({"reference":"REQ-A","title":"Changed right"})))
        .unwrap();
    b.force_refresh().unwrap();
    assert_eq!(get(&a, "REQ-A")["node"]["title"], "A");
    assert_eq!(get(&b, "REQ-A")["node"]["title"], "Changed right");
    let before = [
        fs::read(left.join("a.mara.md")).unwrap(),
        fs::read(right.join("a.mara.md")).unwrap(),
    ];
    host.stop();
    assert!(matches!(
        a.get(decode(json!({"reference":"REQ-A"}))),
        Err(ClientError::Unavailable(_))
    ));
    assert_eq!(
        host.cli(host.cwd.path(), "direct", &["daemon", "start"])["state"],
        "running"
    );
    // Detached lifecycle ownership is controlled only through this private endpoint.
    assert_eq!(get(&a, "REQ-A")["node"]["title"], "A");
    assert_eq!(get(&b, "REQ-A")["node"]["title"], "Changed right");
    assert_eq!(
        before,
        [
            fs::read(left.join("a.mara.md")).unwrap(),
            fs::read(right.join("a.mara.md")).unwrap()
        ]
    );
    assert_eq!(
        host.cli(host.cwd.path(), "direct", &["daemon", "stop"])["state"],
        "stopped"
    );
    assert!(!ipc::endpoint(host.runtime.path()).exists());
}

// @mara implements VER-NATIVE-EXECUTION-BACKENDS
// @mara checks DES-MUTATION-RECOVERY
// @mara checks DES-MUTATION-TRANSACTION
#[test]
fn guarded_publication_and_explicit_recovery_preserve_bytes_and_modes_on_both_backends() {
    let host = Host::new();
    let mut outcomes = Vec::new();
    for mode in [ExecutionMode::Direct, ExecutionMode::DaemonRequired] {
        let f = support::fixture();
        let root = f.path();
        seeded(root);
        let client = host.client(root, mode);
        let path = root.join("a.mara.md");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let original = fs::read_to_string(&path).unwrap();
        let lock = fs::File::create(root.join(".mara/mutation.lock")).unwrap();
        lock.lock().unwrap();
        assert!(
            client
                .item_update(decode(json!({"reference":"REQ-A","title":"Blocked"})))
                .is_err()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        lock.unlock().unwrap();
        client
            .item_update(decode(json!({"reference":"REQ-A","title":"Updated"})))
            .unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        // Rename is a real multi-file journaled publisher: metadata and incoming mentions change.
        fs::write(
            root.join("b.mara.md"),
            document("REQ-B", "01ARZ3NDEKTSV4RRFFQ69G5F01", "B").replace("Body.", "See [[REQ-A]]."),
        )
        .unwrap();
        fs::set_permissions(root.join("b.mara.md"), fs::Permissions::from_mode(0o600)).unwrap();
        client.item_rename("REQ-A", "REQ-RENAMED").unwrap();
        assert!(!root.join(".mara/transaction.json").exists());
        assert!(
            fs::read_to_string(root.join("b.mara.md"))
                .unwrap()
                .contains("[[REQ-RENAMED]]")
        );
        assert_eq!(
            fs::metadata(root.join("b.mara.md"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        outcomes.push((
            fs::read(&path).unwrap(),
            fs::read(root.join("b.mara.md")).unwrap(),
        ));
        // Published-format recovery fixture, not a claim of crashing this daemon.
        // The existing transaction interruption subprocess gate owns that boundary.
        let candidate = fs::read_to_string(&path).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        let journal = json!({"format_version":1,"changes":[
            {"path":"a.mara.md","before":original,"after":candidate,"mode":{"readonly":false,"unix_mode":mode}},
            {"path":"new.mara.md","before":null,"after":"new destination","mode":null}
        ]});
        fs::write(root.join("new.mara.md"), "new destination").unwrap();
        fs::set_permissions(root.join("new.mara.md"), fs::Permissions::from_mode(0o640)).unwrap();
        fs::write(root.join(".mara/transaction.json"), journal.to_string()).unwrap();
        assert!(
            client
                .item_update(decode(json!({"reference":"REQ-RENAMED","title":"Blocked"})))
                .is_err()
        );
        fs::write(&path, "later manual edit").unwrap();
        assert!(client.project_transaction_rollback().is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "later manual edit");
        assert!(root.join("new.mara.md").exists());
        assert!(root.join(".mara/transaction.json").exists());
        fs::write(&path, &candidate).unwrap();
        // Recovery remains available with invalid current schema and corpus.
        fs::write(root.join(".mara/schema.yaml"), "invalid: [\n").unwrap();
        let restored = client.project_transaction_rollback().unwrap();
        assert_eq!(
            restored.restored,
            [PathBuf::from("a.mara.md"), PathBuf::from("new.mara.md")]
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode(), mode);
        assert!(!root.join("new.mara.md").exists());
        assert!(!root.join(".mara/transaction.json").exists());
        assert!(
            client
                .project_transaction_rollback()
                .unwrap()
                .restored
                .is_empty()
        );
    }
    assert_eq!(
        outcomes[0], outcomes[1],
        "direct/daemon publication bytes differ"
    );
}

// @mara implements VER-NATIVE-EXECUTION-BACKENDS
// @mara checks REQ-OPTIONAL-DAEMON-EXECUTION
// @mara checks DES-NATIVE-CLIENT-ROUTING
#[test]
fn lost_response_after_real_journaled_mutation_reports_unknown_without_replay() {
    let host = Host::new();
    let f = support::fixture();
    let root = f.path();
    seeded(root);
    fs::write(
        root.join("b.mara.md"),
        document("REQ-B", "01ARZ3NDEKTSV4RRFFQ69G5F01", "B").replace("Body.", "See [[REQ-A]]."),
    )
    .unwrap();
    let proxy = private_directory();
    let listener = UnixListener::bind(ipc::endpoint(proxy.path())).unwrap();
    fs::set_permissions(
        ipc::endpoint(proxy.path()),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let directory = host.runtime.path().to_path_buf();
    // Only transport fault injection: every result comes from the real daemon.
    // Read the actual publication result, then lose it before delivery to caller.
    let worker = thread::spawn(move || {
        let (downstream, _) = listener.accept().unwrap();
        downstream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut caller = BufReader::new(downstream);
        let upstream = ipc::connect(&directory).unwrap();
        upstream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        let mut server = BufReader::new(upstream);
        for dispatch in 0..2 {
            let mut request = String::new();
            assert!(caller.read_line(&mut request).unwrap() > 0);
            let parsed: Value = serde_json::from_str(&request).unwrap();
            assert_eq!(
                parsed["method"],
                if dispatch == 0 {
                    "mara.negotiate"
                } else {
                    "item_rename"
                }
            );
            server.get_mut().write_all(request.as_bytes()).unwrap();
            let mut response = String::new();
            assert!(server.read_line(&mut response).unwrap() > 0);
            let parsed: Value = serde_json::from_str(&response).unwrap();
            assert!(parsed.get("result").is_some(), "{parsed}");
            if dispatch == 0 {
                caller.get_mut().write_all(response.as_bytes()).unwrap();
            } else {
                return parsed["result"].clone();
            }
        }
        unreachable!()
    });
    let client = NativeClient::new(
        Some(root.into()),
        root.into(),
        ClientOptions {
            mode: Some(ExecutionMode::Auto),
            daemon_directory: Some(proxy.path().into()),
        },
    )
    .unwrap();
    let error = client.item_rename("REQ-A", "REQ-RENAMED").unwrap_err();
    assert!(
        matches!(error, ClientError::UnknownOutcome { .. }),
        "{error}"
    );
    assert_eq!(error.envelope()["error"]["code"], "unknown_outcome");
    let actual = worker.join().unwrap();
    assert_eq!(actual["new_id"], "REQ-RENAMED");
    // Replaying this rename would fail (old ID is gone), not produce UnknownOutcome.
    let daemon = host.client(root, ExecutionMode::DaemonRequired);
    assert_eq!(get(&daemon, "REQ-RENAMED")["node"]["id"], "REQ-RENAMED");
    assert!(daemon.get(decode(json!({"reference":"REQ-A"}))).is_err());
    assert!(
        fs::read_to_string(root.join("b.mara.md"))
            .unwrap()
            .contains("[[REQ-RENAMED]]")
    );
    assert!(!root.join(".mara/transaction.json").exists());
    assert!(daemon.project_validate(&[]).unwrap().valid);
}
