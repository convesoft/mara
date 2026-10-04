#![cfg(any(target_os = "linux", target_os = "macos"))]
mod support;
use mara::{ipc, protocol::APPLICATION_VERSION};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Host {
    runtime: tempfile::TempDir,
    project: tempfile::TempDir,
    foreground: Option<Child>,
}
impl Host {
    fn new() -> Self {
        let base = std::env::var_os("MARA_TEST_RUNTIME_DIR")
            .or_else(|| std::env::var_os("XDG_RUNTIME_DIR"))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let runtime = tempfile::Builder::new()
            .prefix("dl-")
            .tempdir_in(base)
            .unwrap();
        fs::set_permissions(runtime.path(), fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            runtime,
            project: support::fixture(),
            foreground: None,
        }
    }
    fn directory(&self) -> std::path::PathBuf {
        self.runtime.path().join("mara")
    }
    fn command(&self) -> Command {
        let mut c = support::command(self.project.path());
        c.env("XDG_RUNTIME_DIR", self.runtime.path());
        #[cfg(target_os = "macos")]
        c.env("TMPDIR", self.runtime.path());
        c.args(["--format", "json"]);
        c
    }
    fn cli(&self, args: &[&str]) -> Value {
        let output = self.command().args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {} {}",
            support::stdout(&output),
            support::stderr(&output)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn spawn(&mut self) {
        self.foreground = Some(
            self.command()
                .args(["daemon", "run"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self
                .command()
                .args(["daemon", "status"])
                .output()
                .is_ok_and(|o| {
                    o.status.success()
                        && serde_json::from_slice::<Value>(&o.stdout)
                            .is_ok_and(|v| v["state"] == "running")
                })
            {
                break;
            }
            assert!(
                self.foreground
                    .as_mut()
                    .unwrap()
                    .try_wait()
                    .unwrap()
                    .is_none(),
                "foreground exited"
            );
            assert!(Instant::now() < deadline, "foreground not ready");
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn rpc(&self, method: &str, mut params: Value) -> Value {
        let mut c = Rpc::new(&self.directory());
        params["project"] = json!(self.project.path());
        c.ok(method, params)
    }
    fn wait(&mut self) {
        if let Some(mut child) = self.foreground.take() {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(exit) = child.try_wait().unwrap() {
                    assert!(exit.success());
                    break;
                }
                if Instant::now() >= deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("foreground did not exit");
                }
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        // The endpoint is private to this fixture. Never use the real user endpoint.
        let _ = self.command().args(["daemon", "stop"]).output();
        if let Some(child) = self.foreground.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
struct Rpc(BufReader<UnixStream>);
impl Rpc {
    fn new(directory: &Path) -> Self {
        let stream = ipc::connect(directory).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(120)))
            .unwrap();
        let mut c = Self(BufReader::new(stream));
        c.ok(
            "mara.negotiate",
            json!({"protocol_version":1,"application_version":APPLICATION_VERSION}),
        );
        c
    }
    fn call(&mut self, method: &str, params: Value) -> Value {
        writeln!(
            self.0.get_mut(),
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})
        )
        .unwrap();
        let mut line = String::new();
        assert!(self.0.read_line(&mut line).unwrap() > 0);
        serde_json::from_str(&line).unwrap()
    }
    fn ok(&mut self, method: &str, params: Value) -> Value {
        let response = self.call(method, params);
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }
}

// @mara checks DES-DAEMON-LIFECYCLE
// @mara checks DES-NATIVE-CLIENT-ROUTING
// @mara checks REQ-SURFACE-PARITY
#[test]
fn lifecycle_and_routing_share_explicit_and_environment_endpoint_selection() {
    let host = Host::new();
    let selected = host.runtime.path().join("explicit");
    let environment = host.runtime.path().join("environment");
    let explicit = |args: &[&str]| {
        let output = host
            .command()
            .env("MARA_DAEMON_DIRECTORY", &environment)
            .args(["--daemon-directory", selected.to_str().unwrap()])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            support::stdout(&output),
            support::stderr(&output)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    explicit(&["daemon", "start"]);
    // Stop before asserting so even a pre-fix wrong-endpoint reproduction is cleaned.
    let selected_published = ipc::endpoint(&selected).exists();
    let default_published = ipc::endpoint(&host.directory()).exists();
    explicit(&["daemon", "stop"]);
    assert!(selected_published, "explicit endpoint was ignored");
    assert!(
        !default_published,
        "lifecycle targeted the default endpoint"
    );
    assert!(!ipc::endpoint(&environment).exists());
    explicit(&["daemon", "start"]);
    let result = std::panic::catch_unwind(|| {
        explicit(&[
            "--execution-mode",
            "daemon-required",
            "project",
            "init",
            "--template",
            "engineering",
        ]);
        explicit(&[
            "--execution-mode",
            "daemon-required",
            "item",
            "create",
            "requirement",
            "REQ-A",
            "a.mara.md",
            "--title",
            "A",
            "--body",
            "First.",
            "--field",
            "status=draft",
        ]);
        explicit(&[
            "--execution-mode",
            "daemon-required",
            "item",
            "create",
            "requirement",
            "REQ-B",
            "b.mara.md",
            "--title",
            "B",
            "--body",
            "Second.",
            "--field",
            "status=draft",
        ]);
        explicit(&[
            "--execution-mode",
            "daemon-required",
            "item",
            "update",
            "REQ-A",
            "--title",
            "Updated",
        ]);
        let daemon = explicit(&["--execution-mode", "daemon-required", "get", "REQ-A"]);
        assert_eq!(
            daemon,
            explicit(&["--execution-mode", "direct", "get", "REQ-A"])
        );
        assert_eq!(
            daemon,
            explicit(&["--execution-mode", "auto", "get", "REQ-A"])
        );
        let first = explicit(&[
            "--execution-mode",
            "daemon-required",
            "item",
            "list",
            "--limit",
            "1",
        ]);
        let cursor = first["next_cursor"].as_str().unwrap();
        assert_eq!(
            explicit(&[
                "--execution-mode",
                "direct",
                "item",
                "list",
                "--limit",
                "1",
                "--cursor",
                cursor
            ])["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(
            explicit(&["--execution-mode", "daemon-required", "project", "validate"])["valid"]
                .as_bool()
                .unwrap()
        );
        assert!(
            explicit(&[
                "--execution-mode",
                "daemon-required",
                "trace",
                "matrix",
                "--id",
                "REQ-A",
                "--check-file",
                ".mara/engineering-checks.yaml",
                "--shape",
                "urn:mara:rule:intent"
            ])["evaluation_complete"]
                .as_bool()
                .unwrap()
        );
        let requests = [
            support::mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            support::mcp_call(2, "get", json!({"reference":"REQ-A"})),
        ];
        let replies = support::mcp_exchange_with_arguments(
            host.project.path(),
            &[
                "--daemon-directory",
                selected.to_str().unwrap(),
                "--execution-mode",
                "daemon-required",
                "mcp",
            ],
            &requests,
        );
        assert_eq!(
            support::mcp_response(&replies, 2)["result"]["structuredContent"],
            daemon
        );
        assert_eq!(explicit(&["daemon", "status"])["state"], "running");
        assert_eq!(explicit(&["daemon", "refresh"])["refreshed"], true);
        assert_eq!(
            explicit(&["daemon", "connection-info"])["reason"],
            "http_disabled"
        );
    });
    explicit(&["daemon", "stop"]);
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
    let env_call = |args: &[&str]| {
        let output = host
            .command()
            .env("MARA_DAEMON_DIRECTORY", &environment)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            support::stdout(&output),
            support::stderr(&output)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    env_call(&["daemon", "start"]);
    let environment_published = ipc::endpoint(&environment).exists();
    let default_published = ipc::endpoint(&host.directory()).exists();
    env_call(&["daemon", "stop"]);
    assert!(environment_published, "environment endpoint was ignored");
    assert!(!default_published);
}

// @mara checks DES-DAEMON-LIFECYCLE
// @mara checks DES-NATIVE-CLIENT-ROUTING
#[test]
fn detached_start_accepts_explicit_mode_over_invalid_environment() {
    let host = Host::new();
    for mode in ["direct", "auto", "daemon-required"] {
        let output = host
            .command()
            .env("MARA_EXECUTION_MODE", "invalid")
            .args(["--execution-mode", mode, "daemon", "start"])
            .output()
            .unwrap();
        let status = host.cli(&["daemon", "status"]);
        host.cli(&["daemon", "stop"]);
        assert!(
            output.status.success(),
            "{mode}: {} {}",
            support::stdout(&output),
            support::stderr(&output)
        );
        assert_eq!(status["state"], "running");
        assert_eq!(host.cli(&["daemon", "status"])["state"], "stopped");
    }
}

// @mara checks DES-DAEMON-LIFECYCLE
// @mara checks DES-OPERATION-PROJECT-CONTEXT
#[test]
fn refresh_rejects_non_utf8_selected_or_discovered_project_without_panicking() {
    use std::os::unix::ffi::OsStringExt;
    let host = Host::new();
    let root = host
        .project
        .path()
        .join(std::ffi::OsString::from_vec(b"native-\xff".to_vec()));
    fs::create_dir(&root).unwrap();
    mara::initialize_project(&root, mara::Template::Minimal).unwrap();
    for explicit in [true, false] {
        let mut command = host.command();
        if explicit {
            command.arg("--project").arg(&root);
        } else {
            command.current_dir(&root);
        }
        let output = command.args(["daemon", "refresh"]).output().unwrap();
        assert!(!output.status.success());
        assert_ne!(
            output.status.code(),
            Some(101),
            "refresh panicked: {}",
            support::stderr(&output)
        );
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["error"]["code"], "invalid_argument");
        assert!(!ipc::endpoint(&host.directory()).exists());
    }
}

// @mara checks REQ-USER-DAEMON-LIFECYCLE
// @mara checks DES-DAEMON-LIFECYCLE
#[test]
fn foreground_detached_single_owner_disconnect_and_local_handoff() {
    let mut host = Host::new();
    assert_eq!(host.cli(&["daemon", "status"])["state"], "stopped");
    host.spawn();
    let first = host.cli(&["daemon", "status"]);
    assert_eq!(first["pid"], host.foreground.as_ref().unwrap().id());
    assert_eq!(host.cli(&["daemon", "start"])["already_running"], true);
    let duplicate = host.command().args(["daemon", "run"]).output().unwrap();
    assert!(!duplicate.status.success());
    assert!(support::stdout(&duplicate).contains("Another daemon owns"));
    drop(Rpc::new(&host.directory()));
    assert_eq!(host.cli(&["daemon", "status"]), first);
    assert_eq!(
        host.cli(&["daemon", "connection-info"]),
        json!({"available":false,"reason":"http_disabled"})
    );
    assert!(!first.to_string().contains("token"));
    host.rpc("project_init", json!({}));
    host.rpc("item_create", json!({"flavour":"requirement","id":"REQ-LIVE","file":"live.mara.md","title":"Live","body":"Source survives stop."}));
    let source = fs::read(host.project.path().join("live.mara.md")).unwrap();
    // A valid interrupted publication journal must survive and remain recoverable.
    let recovered_source = ":::mara requirement REQ-RECOVER\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Recovered\n\nDurable preimage.\n:::\n";
    let recovery = host.project.path().join(".mara/transaction.json");
    fs::write(host.project.path().join("recover.mara.md"), "").unwrap();
    let journal = json!({"format_version":1,"changes":[{"path":"recover.mara.md","before":recovered_source,"after":"","mode":{"readonly":false}}]}).to_string();
    fs::write(&recovery, &journal).unwrap();
    assert_eq!(host.cli(&["daemon", "stop"])["state"], "stopped");
    host.wait();
    assert!(!ipc::endpoint(&host.directory()).exists());
    let start = host.cli(&["daemon", "start"]);
    assert_eq!(start["state"], "running");
    assert_eq!(start["already_running"], false);
    assert_eq!(host.cli(&["daemon", "start"])["pid"], start["pid"]);
    #[cfg(target_os = "linux")]
    {
        let pid = start["pid"].as_i64().unwrap() as libc::pid_t;
        // SAFETY: read-only process/session queries for this fixture's daemon.
        assert_eq!(unsafe { libc::getsid(pid) }, pid);
        assert_eq!(
            fs::read_link(format!("/proc/{pid}/fd/0")).unwrap(),
            Path::new("/dev/null")
        );
    }
    assert_eq!(
        host.rpc("get", json!({"reference":"REQ-LIVE"}))["node"]["title"],
        "Live"
    );
    assert_eq!(
        fs::read(host.project.path().join("live.mara.md")).unwrap(),
        source
    );
    assert_eq!(fs::read_to_string(&recovery).unwrap(), journal);
    assert_eq!(
        host.rpc("project_transaction_rollback", json!({}))["restored"],
        json!(["recover.mara.md"])
    );
    assert_eq!(
        fs::read_to_string(host.project.path().join("recover.mara.md")).unwrap(),
        recovered_source
    );
    assert!(!recovery.exists());
    host.cli(&["daemon", "stop"]);
    assert_eq!(host.cli(&["daemon", "stop"])["state"], "stopped");
}

// @mara checks DES-DAEMON-LIFECYCLE
#[test]
fn concurrent_cold_starts_report_one_published_private_owner() {
    use std::os::unix::process::CommandExt;
    let host = Host::new();
    for _ in 0..2 {
        let mut starts = Vec::new();
        for _ in 0..12 {
            let mut command = host.command();
            // SAFETY: umask is async-signal-safe; this affects only the forked
            // fixture command, not the test process or other fixtures.
            unsafe {
                command.pre_exec(|| {
                    libc::umask(0o022);
                    Ok(())
                });
            }
            starts.push(
                command
                    .args(["daemon", "start"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap(),
            );
        }
        let outputs: Vec<_> = starts
            .into_iter()
            .map(|child| child.wait_with_output().unwrap())
            .collect();
        let mut results = Vec::new();
        for output in outputs {
            assert!(
                output.status.success(),
                "{} {}",
                support::stdout(&output),
                support::stderr(&output)
            );
            results.push(serde_json::from_slice::<Value>(&output.stdout).unwrap());
        }
        let pid = &results[0]["pid"];
        assert!(
            results
                .iter()
                .all(|result| result["state"] == "running" && result["pid"] == *pid)
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| result["already_running"] == false)
                .count(),
            1
        );
        assert_eq!(
            fs::metadata(ipc::endpoint(&host.directory()))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        host.cli(&["daemon", "stop"]);
        assert_eq!(host.cli(&["daemon", "status"])["state"], "stopped");
        thread::sleep(Duration::from_millis(50));
        assert_eq!(host.cli(&["daemon", "status"])["state"], "stopped");
    }
}

// @mara checks DES-DAEMON-LIFECYCLE
// @mara checks DES-LOCAL-DAEMON-PROTOCOL
#[test]
fn stale_socket_recovery_never_signals_or_replaces_unrelated_state() {
    let mut host = Host::new();
    host.spawn();
    host.foreground.as_mut().unwrap().kill().unwrap();
    host.foreground.take().unwrap().wait().unwrap();
    assert_eq!(host.cli(&["daemon", "status"])["state"], "stale");
    assert_eq!(host.cli(&["daemon", "stop"])["state"], "stale");
    host.spawn();
    host.cli(&["daemon", "stop"]);
    host.wait();
    // PID-looking lock contents have no authority to stop any process.
    let mut unrelated = support::isolated_command("sleep", host.project.path())
        .arg("60")
        .spawn()
        .unwrap();
    fs::write(
        host.directory().join("native.lock"),
        unrelated.id().to_string(),
    )
    .unwrap();
    host.spawn();
    host.cli(&["daemon", "stop"]);
    host.wait();
    assert!(unrelated.try_wait().unwrap().is_none());
    unrelated.kill().unwrap();
    unrelated.wait().unwrap();
    let endpoint = ipc::endpoint(&host.directory());
    fs::write(&endpoint, b"not a socket").unwrap();
    let run = host.command().args(["daemon", "run"]).output().unwrap();
    assert!(!run.status.success());
    assert_eq!(fs::read(&endpoint).unwrap(), b"not a socket");
    fs::remove_file(&endpoint).unwrap();
    let listener = UnixListener::bind(&endpoint).unwrap();
    fs::set_permissions(&endpoint, fs::Permissions::from_mode(0o600)).unwrap();
    let ino = fs::metadata(&endpoint).unwrap().ino();
    let run = host.command().args(["daemon", "run"]).output().unwrap();
    assert!(!run.status.success());
    assert_eq!(fs::metadata(&endpoint).unwrap().ino(), ino);
    drop(listener);
    fs::remove_file(&endpoint).unwrap();
    fs::set_permissions(host.directory(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        !host
            .command()
            .args(["daemon", "run"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(
        fs::metadata(host.directory()).unwrap().permissions().mode() & 0o777,
        0o755
    );
    // Test-owned restoration only; production code never repairs permissions.
    fs::set_permissions(host.directory(), fs::Permissions::from_mode(0o700)).unwrap();
}

// @mara checks REQ-USER-DAEMON-LIFECYCLE
// @mara checks DES-NATIVE-PROJECT-SESSIONS
#[test]
fn real_indexer_reuses_startup_environment_until_refresh_and_restart() {
    let mut host = Host::new();
    mara::initialize_project(host.project.path(), mara::Template::Minimal).unwrap();
    let root = host.project.path().to_path_buf();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = 'lifecycle-fixture'\nversion = '0.1.0'\nedition = '2024'\n",
    )
    .unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn live() -> bool { true }\n").unwrap();
    fs::write(root.join("a.mara.md"), ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Actual indexer\n\nBody.\n:::\n").unwrap();
    let audit = tempfile::tempdir().unwrap();
    let log = audit.path().join("indexer.log");
    let script = audit.path().join("indexer.sh");
    let delay = audit.path().join("delay");
    let entered = audit.path().join("entered");
    fs::write(&script, format!("#!/bin/sh\nset -eu\nprintf '%s|%s\\n' \"$PWD\" \"$MARA_LIFECYCLE_ENV\" >> '{}'\nif [ -e '{}' ]; then touch '{}'; sleep 34; fi\nexport TMPDIR=\"$MARA_INDEXER_TMPDIR\"\nexec rust-analyzer scip . --output \"$1\"\n", log.display(), delay.display(), entered.display())).unwrap();
    let config = fs::read_to_string(root.join(".mara/project.toml"))
        .unwrap()
        .replacen("format_version = 1", "format_version = 3", 1);
    fs::write(root.join(".mara/project.toml"), format!("{config}\n[[code.languages]]\nname = 'rust'\nextensions = ['rs']\ncommand = ['sh', '{}', '{{output}}']\nposition_encoding = 'utf8'\n", script.display())).unwrap();
    let mut command = host.command();
    host.foreground = Some(
        command
            .env("MARA_LIFECYCLE_ENV", "startup")
            .env("MARA_INDEXER_TMPDIR", std::env::temp_dir())
            .args(["daemon", "run"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while ipc::connect(&host.directory()).is_err() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(20));
    }
    host.rpc("get", json!({"reference":"REQ-A"}));
    // Cargo.lock is generated by the real first indexing run; let that input settle.
    host.rpc("get", json!({"reference":"REQ-A"}));
    let before = fs::read_to_string(&log).unwrap();
    host.rpc("get", json!({"reference":"REQ-A"}));
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        before,
        "warm request invoked indexer"
    );
    assert!(
        before
            .lines()
            .all(|l| l == format!("{}|startup", root.display()))
    );
    let output = host
        .command()
        .env("MARA_LIFECYCLE_ENV", "changed")
        .args(["--project", root.to_str().unwrap(), "daemon", "refresh"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", support::stdout(&output));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"refreshed":true})
    );
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        before,
        "refresh itself must not run indexer"
    );
    host.rpc("get", json!({"reference":"REQ-A"}));
    let refreshed = fs::read_to_string(&log).unwrap();
    assert_eq!(refreshed.lines().count(), before.lines().count() + 1);
    assert!(refreshed.ends_with("|startup\n"));

    // A genuinely active indexer (delayed outside the recorded root) holds the
    // root gate beyond the lifecycle health timeout. Refresh must keep waiting.
    fs::write(&delay, "delay this invocation").unwrap();
    host.cli(&["daemon", "refresh"]);
    let mut client = Rpc::new(&host.directory());
    let selected = root.to_path_buf();
    let work =
        thread::spawn(move || client.ok("get", json!({"project":selected,"reference":"REQ-A"})));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !entered.exists() {
        assert!(Instant::now() < deadline, "delayed indexer did not enter");
        thread::sleep(Duration::from_millis(20));
    }
    let mut refresh = host
        .command()
        .args(["daemon", "refresh"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_secs(31));
    assert!(
        refresh.try_wait().unwrap().is_none(),
        "refresh must still wait past 30 seconds"
    );
    let stop = host
        .command()
        .args(["daemon", "stop"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while host.cli(&["daemon", "status"])["state"] != "stopping" {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(20));
    }
    let existing = host.cli(&["daemon", "start"]);
    assert_eq!(existing["state"], "stopping");
    assert_eq!(existing["already_running"], true);
    let mut observer = Rpc::new(&host.directory());
    assert_eq!(
        observer.call("project_refresh", json!({"project":root}))["error"]["code"],
        -32000
    );
    drop(observer);
    assert_eq!(work.join().unwrap()["node"]["title"], "Actual indexer");
    let output = refresh.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        support::stdout(&output),
        support::stderr(&output)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"refreshed":true})
    );
    let output = stop.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        support::stdout(&output),
        support::stderr(&output)
    );
    fs::remove_file(&delay).unwrap();
    host.wait();
    let output = host
        .command()
        .env("MARA_LIFECYCLE_ENV", "changed")
        .env("MARA_INDEXER_TMPDIR", std::env::temp_dir())
        .args(["daemon", "start"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", support::stdout(&output));
    host.rpc("get", json!({"reference":"REQ-A"}));
    assert!(fs::read_to_string(&log).unwrap().ends_with("|changed\n"));
    host.cli(&["daemon", "stop"]);
    if let Some(evidence) = std::env::var_os("MARA_RUST_PROBE_EVIDENCE") {
        let evidence = std::path::PathBuf::from(evidence);
        fs::create_dir_all(&evidence).unwrap();
        for (source, destination) in [
            (root.join("Cargo.toml"), "Cargo.toml"),
            (root.join("src/lib.rs"), "lib.rs"),
            (root.join("a.mara.md"), "a.mara.md"),
            (root.join(".mara/project.toml"), "project.toml"),
            (root.join(".mara/schema.yaml"), "schema.yaml"),
            (script, "indexer.sh"),
            (log, "indexer.log"),
        ] {
            fs::copy(&source, evidence.join(destination))
                .unwrap_or_else(|error| panic!("capture {}: {error}", source.display()));
        }
    }
}

// @mara checks DES-DAEMON-LIFECYCLE
#[test]
fn foreground_signals_release_owned_endpoint() {
    let mut host = Host::new();
    for signal in [libc::SIGINT, libc::SIGTERM] {
        host.spawn();
        let pid = host.foreground.as_ref().unwrap().id() as libc::pid_t;
        // SAFETY: the live child handle identifies this test's foreground server.
        assert_eq!(unsafe { libc::kill(pid, signal) }, 0);
        host.wait();
        assert_eq!(host.cli(&["daemon", "status"])["state"], "stopped");
    }
}

// @mara checks DES-DAEMON-LIFECYCLE
#[test]
fn non_reading_consumer_cannot_pin_stop_or_signal_shutdown() {
    let mut host = Host::new();
    for through_signal in [false, true] {
        host.spawn();
        let mut peer = ipc::connect(&host.directory()).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let frame = format!(
            "{}\n",
            json!({"jsonrpc":"2.0","id":1,"method":"mara.negotiate","params":{"protocol_version":1,"application_version":APPLICATION_VERSION}})
        );
        peer.write_all(frame.repeat(136).as_bytes()).unwrap();
        // Leave the peer alive and unread, including throughout shutdown/wait.
        thread::sleep(Duration::from_millis(50));
        if through_signal {
            let pid = host.foreground.as_ref().unwrap().id() as libc::pid_t;
            // SAFETY: this test still owns the live foreground child.
            assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
        } else {
            assert_eq!(host.cli(&["daemon", "stop"])["state"], "stopped");
        }
        host.wait();
        assert!(!ipc::endpoint(&host.directory()).exists());
        drop(peer);
    }
}

// @mara checks DES-LOCAL-DAEMON-PROTOCOL
#[test]
fn administrative_methods_are_not_part_of_transport_independent_rpc() {
    let mut rpc = mara::protocol::RpcSession::new(mara::NativeSessions::default());
    rpc.handle(json!({"jsonrpc":"2.0","id":1,"method":"mara.negotiate","params":{"protocol_version":1,"application_version":APPLICATION_VERSION}}));
    for method in mara::protocol::LOCAL_METHODS {
        let response = rpc
            .handle(json!({"jsonrpc":"2.0","id":2,"method":method}))
            .unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }
}
