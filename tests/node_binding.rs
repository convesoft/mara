//! Real Node addon execution, with a real IPC host (no CLI subprocess).
#[cfg(any(target_os = "linux", target_os = "macos"))]
// @mara checks REQ-NODE-NATIVE-BINDING
// @mara checks DES-JAVASCRIPT-CLIENT-ADAPTERS
#[test]
#[ignore = "requires built MARA_NODE_ADDON, Node 24 NODE24, private MARA_NODE_RUNTIME"]
fn real_node_addon() {
    run_node_tests(false, false);
}

// @mara checks REQ-SHARED-TYPESCRIPT-CLIENT
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "requires MARA_NODE_CLIENT package entry, built addon, Node 24 and private runtime"]
fn real_node_facade() {
    run_node_tests(true, false);
}

// @mara checks REQ-NODE-NATIVE-BINDING
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "requires installed MARA_NODE_CLIENT/MARA_NODE_TYPED_CONSUMER, Node 24 and private runtime"]
fn installed_node_client() {
    run_node_tests(true, true);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_node_tests(public: bool, installed: bool) {
    use std::{
        path::PathBuf,
        process::Command,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    };
    let addon = PathBuf::from(std::env::var_os("MARA_NODE_ADDON").expect("MARA_NODE_ADDON"));
    assert!(addon.is_absolute() && addon.is_file());
    let node = PathBuf::from(std::env::var_os("NODE24").expect("NODE24"));
    assert!(node.is_absolute() && node.is_file());
    let runtime = PathBuf::from(std::env::var_os("MARA_NODE_RUNTIME").expect("MARA_NODE_RUNTIME"));
    assert!(runtime.is_absolute() && runtime.is_dir());
    let directory = tempfile::Builder::new()
        .prefix("n")
        .tempdir_in(runtime)
        .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let server =
        mara::ipc::IpcServer::bind_in(directory.path(), mara::NativeSessions::default()).unwrap();
    struct Host {
        shutdown: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<std::io::Result<()>>>,
    }
    impl Drop for Host {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::Release);
            self.thread.take().unwrap().join().unwrap().unwrap();
        }
    }
    let shutdown = Arc::new(AtomicBool::new(false));
    let stopping = shutdown.clone();
    let host = Host {
        shutdown,
        thread: Some(thread::spawn(move || server.serve_until(&stopping))),
    };
    let fixtures = tempfile::tempdir().unwrap();
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/node/addon.test.mjs");
    let mut command = if let Some(trace) = std::env::var_os("MARA_NODE_PROCESS_TRACE") {
        let mut command = Command::new("strace");
        command
            .args(["-f", "-e", "trace=process,connect", "-o"])
            .arg(trace)
            .arg(&node);
        command
    } else {
        Command::new(&node)
    };
    if public {
        command.env(
            "MARA_NODE_CLIENT",
            std::env::var_os("MARA_NODE_CLIENT").expect("MARA_NODE_CLIENT"),
        );
    } else {
        command.env_remove("MARA_NODE_CLIENT");
    }
    if installed {
        command.env("MARA_NODE_INSTALLED", "1");
    } else {
        command.env_remove("MARA_NODE_INSTALLED");
    }
    command.args(["--test", "--test-concurrency=1"]).arg(script);
    if public {
        command.arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/node/client.test.mjs"));
        command.arg(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/node/query-catalog.test.mjs"),
        );
    }
    command
        .current_dir(fixtures.path())
        .env("MARA_NODE_ADDON", addon)
        .env("MARA_NODE_FIXTURES", fixtures.path())
        .env("MARA_NODE_DAEMON", directory.path())
        .env(
            "MARA_NODE_LOADER",
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("crates/mara-node/loader.mjs"),
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            fixtures.path().join("absent-git-config"),
        )
        .env("MARA_EXECUTION_MODE", "deliberately-invalid-ignored")
        .env("MARA_DAEMON_DIRECTORY", "deliberately-invalid-ignored");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_")
            && !["GIT_CONFIG_NOSYSTEM", "GIT_CONFIG_GLOBAL"]
                .contains(&key.to_string_lossy().as_ref())
        {
            command.env_remove(key);
        }
    }
    if installed {
        let node_only = fixtures.path().join("node-only");
        std::fs::create_dir(&node_only).unwrap();
        std::os::unix::fs::symlink(&node, node_only.join("node")).unwrap();
        command.env("PATH", node_only);
    }
    let status = command.status().unwrap();
    drop(host);
    assert!(status.success(), "real Node tests failed: {status}");
}
