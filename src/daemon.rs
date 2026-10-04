//! Explicit CLI lifecycle adapter. This is not the general native routing client.
use clap::{Args, Subcommand};
use mara::{
    ClientError, NativeSessions,
    ipc::{self, IpcServer},
    protocol::{APPLICATION_VERSION, PROTOCOL_VERSION},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, BufRead, BufReader, Write},
    os::unix::{fs::MetadataExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::AtomicBool,
    thread,
    time::{Duration, Instant},
};

static SIGNAL_STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop_signal(_: libc::c_int) {
    SIGNAL_STOP.store(true, std::sync::atomic::Ordering::Release);
}

#[derive(Debug, Subcommand)]
pub enum DaemonCommand {
    /// Run in the foreground until local stop, SIGINT or SIGTERM. Never auto-started.
    Run(HttpStartup),
    /// Explicitly start a detached daemon, or report the compatible existing instance.
    Start(HttpStartup),
    /// Stop through authenticated local IPC and wait for this endpoint owner to exit.
    Stop,
    /// Report running/stopping, stopped (no endpoint), or stale (refused connection).
    Status,
    /// Discard selected-project derived state, even with unchanged files. Rebuild on next use.
    Refresh,
    /// Deliberate private HTTP handoff; returns unavailable while HTTP is disabled.
    ConnectionInfo,
}

#[derive(Debug, Default, Args)]
pub struct HttpStartup {
    /// Enable HTTP at this numeric loopback address (port 0 selects a free port).
    #[arg(long)]
    http_bind: Option<std::net::SocketAddr>,
    /// Exact serialized HTTP(S) browser origin; repeat for multiple origins.
    #[arg(long, requires = "http_bind")]
    http_origin: Vec<String>,
    /// Exact existing project root authorized for HTTP operations (absolute).
    #[arg(long, requires = "http_bind")]
    http_project: Vec<PathBuf>,
    /// Exact initialization destination authorized for HTTP project_init (absolute).
    #[arg(long, requires = "http_bind")]
    http_init_destination: Vec<PathBuf>,
}
impl HttpStartup {
    fn options(self) -> Option<mara::http::HttpOptions> {
        self.http_bind.map(|bind| mara::http::HttpOptions {
            bind,
            origins: self.http_origin,
            projects: self.http_project,
            initialization_destinations: self.http_init_destination,
        })
    }
    fn append(&self, command: &mut Command) {
        if let Some(bind) = self.http_bind {
            command.arg("--http-bind").arg(bind.to_string());
            for origin in &self.http_origin {
                command.arg("--http-origin").arg(origin);
            }
            for project in &self.http_project {
                command.arg("--http-project").arg(project);
            }
            for destination in &self.http_init_destination {
                command.arg("--http-init-destination").arg(destination);
            }
        }
    }
}

// @mara implements REQ-USER-DAEMON-LIFECYCLE
// @mara implements DES-DAEMON-LIFECYCLE
pub fn execute(
    command: DaemonCommand,
    project: Option<PathBuf>,
    directory: Option<PathBuf>,
) -> Result<Value, ClientError> {
    if project.is_some() && !matches!(command, DaemonCommand::Refresh) {
        return Err("--project is only used by daemon refresh; lifecycle is per user".into());
    }
    let directory = directory
        .or_else(|| std::env::var_os("MARA_DAEMON_DIRECTORY").map(PathBuf::from))
        .map(Ok)
        .unwrap_or_else(ipc::runtime_directory)
        .map_err(|e| e.to_string())?;
    match command {
        DaemonCommand::Run(http) => run(&directory, http),
        DaemonCommand::Start(http) => start(&directory, http),
        DaemonCommand::Status => status(&directory),
        DaemonCommand::Stop => stop(&directory),
        DaemonCommand::ConnectionInfo => call(&directory, "daemon_connection_info", json!({})),
        DaemonCommand::Refresh => {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            let root = match project {
                Some(root) if root.is_absolute() => root,
                Some(root) => cwd.join(root),
                None => mara::resolve_project(None, &cwd)
                    .map_err(|e| e.to_string())?
                    .root()
                    .to_path_buf(),
            };
            let root = serde_json::to_value(root).map_err(|_| {
                ClientError::invalid_argument(
                    "Project path must be UTF-8 for native IPC; no refresh was dispatched",
                )
            })?;
            call(&directory, "project_refresh", json!({"project":root}))
        }
    }
    .map_err(ClientError::from)
}

fn run(directory: &Path, http: HttpStartup) -> Result<Value, String> {
    // Install before binding so there is no visible endpoint without shutdown handling.
    // SAFETY: handlers use only a lock-free atomic flag; no allocation, I/O or pointers.
    unsafe {
        if libc::signal(
            libc::SIGTERM,
            stop_signal as *const () as libc::sighandler_t,
        ) == libc::SIG_ERR
            || libc::signal(libc::SIGINT, stop_signal as *const () as libc::sighandler_t)
                == libc::SIG_ERR
        {
            return Err(io::Error::last_os_error().to_string());
        }
    }
    IpcServer::bind_recovering(directory, NativeSessions::default())
        .and_then(|server| match http.options() {
            Some(options) => server.with_http(options),
            None => Ok(server),
        })
        .and_then(|server| server.serve_until(&SIGNAL_STOP))
        .map_err(|e| e.to_string())?;
    Ok(json!({"state":"stopped"}))
}

fn status(directory: &Path) -> Result<Value, String> {
    status_with_timeout(directory, Duration::from_secs(30))
}
fn status_with_timeout(directory: &Path, timeout: Duration) -> Result<Value, String> {
    match ipc::connect(directory) {
        Ok(stream) => request(stream, "daemon_status", json!({}), timeout),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(json!({"state":"stopped"})),
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => Ok(json!({"state":"stale"})),
        Err(e) => Err(e.to_string()),
    }
}

fn start(directory: &Path, http: HttpStartup) -> Result<Value, String> {
    let current = status(directory)?;
    if matches!(current["state"].as_str(), Some("running" | "stopping")) {
        return Ok(json!({"state":current["state"],"pid":current["pid"],"already_running":true}));
    }
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    // The server executes NativeRuntime directly, independently of client routing.
    // Do not let inherited client preference override an accepted parent invocation.
    command
        .args(["--execution-mode", "direct", "--daemon-directory"])
        .arg(directory)
        .args(["daemon", "run"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    http.append(&mut command);
    // SAFETY: setsid is async-signal-safe and does not touch Rust state after fork.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot launch daemon: {e}"))?;
    wait_for_start(directory, &mut child)
}
fn wait_for_start(directory: &Path, child: &mut Child) -> Result<Value, String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // A concurrent start may win. Report that live owner without replacing it.
        let exited = match child.try_wait() {
            Ok(exit) => exit,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        };
        match status_with_timeout(
            directory,
            deadline
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(1)),
        ) {
            Ok(current) if matches!(current["state"].as_str(), Some("running" | "stopping")) => {
                return finish_start(current, child);
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            if let Some(exit) = exited {
                return Err(format!(
                    "Daemon failed to start ({exit}); no compatible owner became ready within five seconds. Run 'mara daemon run' for the ownership/permission diagnostic"
                ));
            }
            // This handle belongs to the child we launched, never a PID from disk.
            let _ = child.kill();
            let _ = child.wait();
            return Err("Daemon startup timed out; run 'mara daemon run' for diagnostics".into());
        }
        // An exited child may have lost the lock to a not-yet-published owner.
        // Use the remaining readiness budget rather than failing on that race.
        thread::sleep(Duration::from_millis(20));
    }
}

fn finish_start(current: Value, child: &mut Child) -> Result<Value, String> {
    let already_running = current["pid"] != json!(child.id());
    if already_running {
        // A delayed losing child could bind after the reported owner stops.
        // Terminate/reap our child before returning another owner's readiness.
        if child.try_wait().map_err(|e| e.to_string())?.is_none() {
            child.kill().map_err(|e| e.to_string())?;
        }
        child.wait().map_err(|e| e.to_string())?;
    }
    Ok(json!({"state":current["state"],"pid":current["pid"],"already_running":already_running}))
}

fn stop(directory: &Path) -> Result<Value, String> {
    let stream = match ipc::connect(directory) {
        Ok(stream) => stream,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(json!({"state":"stopped"})),
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
            return Ok(json!({"state":"stale"}));
        }
        Err(e) => return Err(e.to_string()),
    };
    stop_connected(directory, stream)
}
fn stop_connected(
    directory: &Path,
    stream: std::os::unix::net::UnixStream,
) -> Result<Value, String> {
    // Retain this authenticated connection before reading the endpoint identity.
    // Never reconnect for shutdown: a replacement owner must not receive a stop
    // whose wait is bound to the previous owner's inode.
    let socket = ipc::endpoint(directory);
    let metadata = fs::symlink_metadata(&socket).map_err(|e| e.to_string())?;
    let identity = (metadata.dev(), metadata.ino());
    request(stream, "daemon_stop", json!({}), Duration::from_secs(30))?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match fs::symlink_metadata(&socket) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => break,
            Ok(m) if (m.dev(), m.ino()) != identity => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
        if Instant::now() >= deadline {
            return Err("Stop requested; still waiting for dispatched operations to finish. Check daemon status; no process was forcibly signaled".into());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok(json!({"state":"stopped"}))
}

fn call(directory: &Path, method: &str, params: Value) -> Result<Value, String> {
    request(
        ipc::connect(directory).map_err(|e| e.to_string())?,
        method,
        params,
        Duration::from_secs(30),
    )
}
// Narrow lifecycle/refresh exchange only. No fallback, replay, discovery or routing.
fn request(
    stream: std::os::unix::net::UnixStream,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    let mut stream = BufReader::new(stream);
    for (id, method, params) in [
        (
            1,
            "mara.negotiate",
            json!({"protocol_version":PROTOCOL_VERSION,"application_version":APPLICATION_VERSION}),
        ),
        (2, method, params),
    ] {
        // Refresh must wait for the selected-root gate, not fail on a health-check
        // deadline while a legitimate long-running indexer still owns that gate.
        if id == 2 && method == "project_refresh" {
            stream
                .get_mut()
                .set_read_timeout(None)
                .map_err(|e| e.to_string())?;
        }
        writeln!(
            stream.get_mut(),
            "{}",
            json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        )
        .map_err(|e| e.to_string())?;
        let mut line = String::new();
        if stream.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err("Daemon disconnected; a dispatched request may have completed. Do not replay automatically".into());
        }
        let response: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if response["jsonrpc"] != "2.0" || response["id"] != id {
            return Err("Invalid daemon response; request outcome is unknown".into());
        }
        if let Some(error) = response.get("error") {
            return Err(format!("Daemon request failed: {error}"));
        }
        let result = response
            .get("result")
            .ok_or("Daemon response missing result")?;
        if id == 2 {
            return Ok(result.clone());
        }
    }
    unreachable!("two requests")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // A concurrent fork can briefly inherit another test's ownership lock before
    // exec closes CLOEXEC descriptors. Keep these process/socket cases isolated.
    static TEST_GATE: Mutex<()> = Mutex::new(());

    // @mara checks DES-DAEMON-LIFECYCLE
    #[test]
    fn exited_start_child_still_waits_for_delayed_owner_publication() {
        let _gate = TEST_GATE.lock().unwrap();
        let base = std::env::var_os("MARA_TEST_RUNTIME_DIR")
            .or_else(|| std::env::var_os("XDG_RUNTIME_DIR"))
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let directory = tempfile::Builder::new()
            .prefix("dw-")
            .tempdir_in(base)
            .unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().to_path_buf();
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = shutdown.clone();
        let worker = thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            IpcServer::bind_in(&path, NativeSessions::default())
                .unwrap()
                .serve_until(&flag)
        });
        // Real exited child and real delayed IPC owner: no synthetic response.
        let mut loser = Command::new("sh")
            .args(["-c", "exit 1"])
            .current_dir(directory.path())
            .spawn()
            .unwrap();
        let result = wait_for_start(directory.path(), &mut loser).unwrap();
        assert_eq!(result["state"], "running");
        assert_eq!(result["already_running"], true);
        assert!(loser.try_wait().unwrap().is_some());
        shutdown.store(true, std::sync::atomic::Ordering::Release);
        worker.join().unwrap().unwrap();
    }

    // @mara checks DES-DAEMON-LIFECYCLE
    #[test]
    fn reporting_another_owner_reaps_the_delayed_start_child() {
        let _gate = TEST_GATE.lock().unwrap();
        let base = std::env::var_os("MARA_TEST_RUNTIME_DIR")
            .or_else(|| std::env::var_os("XDG_RUNTIME_DIR"))
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let directory = tempfile::Builder::new()
            .prefix("dc-")
            .tempdir_in(base)
            .unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let owner = IpcServer::bind_in(directory.path(), NativeSessions::default()).unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = shutdown.clone();
        let worker = thread::spawn(move || owner.serve_until(&flag));
        // A real not-yet-bound child remains live until finish_start retires it.
        let mut delayed = Command::new("sleep")
            .arg("60")
            .current_dir(directory.path())
            .spawn()
            .unwrap();
        let current = status(directory.path()).unwrap();
        let result = finish_start(current, &mut delayed).unwrap();
        assert_eq!(result["already_running"], true);
        assert!(delayed.try_wait().unwrap().is_some());
        assert_eq!(status(directory.path()).unwrap()["state"], "running");
        shutdown.store(true, std::sync::atomic::Ordering::Release);
        worker.join().unwrap().unwrap();
    }

    // @mara checks DES-DAEMON-LIFECYCLE
    #[test]
    fn shutdown_does_not_reconnect_to_a_replacement_endpoint_owner() {
        let _gate = TEST_GATE.lock().unwrap();
        let base = std::env::var_os("MARA_TEST_RUNTIME_DIR")
            .or_else(|| std::env::var_os("XDG_RUNTIME_DIR"))
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let directory = tempfile::Builder::new()
            .prefix("ds-")
            .tempdir_in(base)
            .unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let original = IpcServer::bind_in(directory.path(), NativeSessions::default()).unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = shutdown.clone();
        let worker = thread::spawn(move || original.serve_until(&flag));
        let original_connection = ipc::connect(directory.path()).unwrap();
        shutdown.store(true, std::sync::atomic::Ordering::Release);
        worker.join().unwrap().unwrap();

        let replacement = IpcServer::bind_in(directory.path(), NativeSessions::default()).unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = shutdown.clone();
        let worker = thread::spawn(move || replacement.serve_until(&flag));
        assert!(stop_connected(directory.path(), original_connection).is_err());
        assert_eq!(status(directory.path()).unwrap()["state"], "running");
        shutdown.store(true, std::sync::atomic::Ordering::Release);
        worker.join().unwrap().unwrap();
        assert!(!ipc::endpoint(directory.path()).exists());
    }
}
