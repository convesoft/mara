//! Same-user Unix socket host for the reusable native operation protocol.
use crate::{
    NativeSessions,
    protocol::{RpcError, RpcSession},
};
use serde_json::Value;
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

/// Maximum encoded request bytes, including its newline delimiter.
pub const MAX_REQUEST_BYTES: u64 = 1024 * 1024;
const SOCKET_NAME: &str = "native-v1.sock";

/// Default discovery is independent of project selection and daemon working directory.
pub fn runtime_directory() -> io::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    let base = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", uid())));
    #[cfg(target_os = "macos")]
    let base = env::var_os("TMPDIR").map(PathBuf::from).ok_or_else(|| {
        io::Error::other("TMPDIR must identify the private macOS user runtime directory")
    })?;
    check_private_directory(&base)?;
    Ok(base.join("mara"))
}

pub fn endpoint(directory: &Path) -> PathBuf {
    directory.join(SOCKET_NAME)
}

// @mara implements DES-LOCAL-DAEMON-PROTOCOL
// @mara implements DES-DAEMON-LIFECYCLE
pub struct IpcServer {
    listener: UnixListener,
    socket: PathBuf,
    identity: (u64, u64),
    _lock: File,
    sessions: NativeSessions,
}
impl IpcServer {
    pub fn bind_default(sessions: NativeSessions) -> io::Result<Self> {
        Self::bind_in(&runtime_directory()?, sessions)
    }
    /// An explicit private host runtime directory (not a project or data directory).
    /// Never removes an existing endpoint, even if it appears stale.
    pub fn bind_in(directory: &Path, sessions: NativeSessions) -> io::Result<Self> {
        if !directory.is_absolute() {
            return Err(io::Error::other("IPC runtime directory must be absolute"));
        }
        match fs::DirBuilder::new().mode(0o700).create(directory) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => {
                return Err(io::Error::other(
                    "Cannot create private IPC runtime directory; check its parent and permissions",
                ));
            }
        }
        check_private_directory(directory)?;
        let directory = fs::canonicalize(directory)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(directory.join("native.lock"))?;
        let metadata = lock.metadata()?;
        if !metadata.is_file() || metadata.uid() != uid() || metadata.mode() & 0o777 != 0o600 {
            return Err(io::Error::other(
                "IPC ownership lock must be a same-user private regular file",
            ));
        }
        lock.try_lock().map_err(|_| {
            io::Error::other("Another daemon owns the IPC endpoint; use the existing instance")
        })?;
        let socket = endpoint(&directory);
        // macOS has the smaller sockaddr_un capacity. Leave room for the NUL.
        if socket.as_os_str().as_encoded_bytes().len() >= 104 {
            return Err(io::Error::other(
                "IPC socket path is too long; select a shorter private runtime directory",
            ));
        }
        let listener = UnixListener::bind(&socket).map_err(|_| io::Error::other("Cannot bind IPC endpoint; an existing endpoint is never replaced. Inspect it through local lifecycle management"))?;
        if let Err(e) = fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)) {
            let _ = fs::remove_file(&socket);
            return Err(e);
        }
        let metadata = fs::symlink_metadata(&socket)?;
        let identity = (metadata.dev(), metadata.ino());
        Ok(Self {
            listener,
            socket,
            identity,
            _lock: lock,
            sessions,
        })
    }
    pub fn endpoint(&self) -> &Path {
        &self.socket
    }

    /// Foreground foundation. Lifecycle policy and background launch are separate adapters.
    pub fn serve(self) -> io::Result<()> {
        self.serve_until(&AtomicBool::new(false))
    }

    /// Host-owned shutdown: consumer EOF never sets this flag. Already-dispatched
    /// operations finish even if their peer disconnects or the host stops listening.
    pub fn serve_until(self, shutdown: &AtomicBool) -> io::Result<()> {
        self.listener.set_nonblocking(true)?;
        let mut workers: Vec<(UnixStream, thread::JoinHandle<io::Result<()>>)> = Vec::new();
        let result = loop {
            // Retain neither completed threads nor their socket descriptors.
            let mut i = 0;
            while i < workers.len() {
                if workers[i].1.is_finished() {
                    let (_, h) = workers.swap_remove(i);
                    let _ = h.join();
                } else {
                    i += 1;
                }
            }
            self.sessions.prune_idle();
            if shutdown.load(Ordering::Acquire) {
                break Ok(());
            }
            match self.listener.accept() {
                Ok((stream, _)) => {
                    match same_user(&stream) {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(e) => break Err(e),
                    }
                    let control = match stream.try_clone() {
                        Ok(s) => s,
                        Err(e) => break Err(e),
                    };
                    let sessions = self.sessions.clone();
                    match thread::Builder::new()
                        .name("mara-ipc".into())
                        .spawn(move || serve_connection(stream, sessions))
                    {
                        Ok(worker) => workers.push((control, worker)),
                        Err(e) => break Err(e),
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => break Err(e),
            }
        };
        for (stream, _) in &workers {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        for (_, worker) in workers {
            let _ = worker.join();
        }
        result
    }
}
impl Drop for IpcServer {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.socket).is_ok_and(|m| (m.dev(), m.ino()) == self.identity) {
            let _ = fs::remove_file(&self.socket);
        }
    }
}

/// Check the endpoint and server identity before sending any project operation.
pub fn connect(directory: &Path) -> io::Result<UnixStream> {
    check_private_directory(directory)?;
    let socket = endpoint(directory);
    let metadata = fs::symlink_metadata(&socket)?;
    use std::os::unix::fs::FileTypeExt;
    if !metadata.file_type().is_socket()
        || metadata.uid() != uid()
        || metadata.mode() & 0o777 != 0o600
    {
        return Err(io::Error::other(
            "IPC endpoint is not a same-user private socket",
        ));
    }
    let stream = UnixStream::connect(socket)?;
    if !same_user(&stream)? {
        return Err(io::Error::other(
            "IPC server identity does not match the current OS user",
        ));
    }
    Ok(stream)
}

fn check_private_directory(directory: &Path) -> io::Result<()> {
    if !directory.is_absolute() {
        return Err(io::Error::other("IPC runtime directory must be absolute"));
    }
    // Normalize trailing slash/dot spelling before lstat so it cannot follow a leaf symlink.
    let leaf: PathBuf = directory.components().collect();
    let m = fs::symlink_metadata(leaf)?;
    if !m.is_dir() || m.uid() != uid() || m.mode() & 0o777 != 0o700 {
        return Err(io::Error::other(
            "IPC runtime directory must be a same-user directory with mode 0700, not a symlink",
        ));
    }
    Ok(())
}
fn uid() -> libc::uid_t {
    // SAFETY: geteuid takes no pointers and has no preconditions.
    unsafe { libc::geteuid() }
}
fn same_user(stream: &UnixStream) -> io::Result<bool> {
    #[cfg(target_os = "linux")]
    {
        let mut credentials = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: live descriptor and correctly-sized writable ucred/length buffers.
        let status = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut size,
            )
        };
        if status != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(size as usize == std::mem::size_of::<libc::ucred>() && credentials.uid == uid())
    }
    #[cfg(target_os = "macos")]
    {
        let (mut peer_uid, mut peer_gid) = (0, 0);
        // SAFETY: live descriptor and writable uid/gid buffers of the declared types.
        let status = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut peer_uid, &mut peer_gid) };
        if status != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(peer_uid == uid())
    }
}
fn serve_connection(mut stream: UnixStream, sessions: NativeSessions) -> io::Result<()> {
    // macOS accept inherits O_NONBLOCK from the polling listener; the framed worker
    // must wait for subsequent requests rather than treat WouldBlock as a disconnect.
    stream.set_nonblocking(false)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut rpc = RpcSession::new(sessions);
    loop {
        let mut bytes = Vec::new();
        let count = reader
            .by_ref()
            .take(MAX_REQUEST_BYTES + 1)
            .read_until(b'\n', &mut bytes)?;
        if count == 0 {
            return Ok(());
        }
        if count as u64 > MAX_REQUEST_BYTES {
            let response = RpcError::new(
                -32600,
                "Request exceeds 1 MiB; reduce its body and reconnect",
            )
            .response(Value::Null);
            write_response(&mut stream, &response)?;
            return Ok(());
        }
        // EOF is not a frame delimiter. Do not dispatch a partial write.
        if bytes.last() != Some(&b'\n') {
            return Ok(());
        }
        if let Some(response) = rpc.handle_bytes(&bytes) {
            write_response(&mut stream, &response)?;
        }
    }
}
fn write_response(stream: &mut UnixStream, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, value)?;
    stream.write_all(b"\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::APPLICATION_VERSION;
    use serde_json::json;

    // @mara checks DES-LOCAL-DAEMON-PROTOCOL
    #[test]
    fn framed_worker_restores_blocking_mode_for_inherited_nonblocking_socket() {
        let (server, mut client) = UnixStream::pair().unwrap();
        // Reproduce macOS accept inheritance without claiming macOS host execution.
        server.set_nonblocking(true).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let request = |id| {
            json!({"jsonrpc":"2.0","id":id,"method":"mara.negotiate",
            "params":{"protocol_version":1,"application_version":APPLICATION_VERSION}})
        };
        writeln!(client, "{}", request(1)).unwrap();
        let worker = thread::spawn(move || serve_connection(server, NativeSessions::default()));
        let mut reader = BufReader::new(client);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["id"], 1);
        thread::sleep(Duration::from_millis(20));
        assert!(
            !worker.is_finished(),
            "idle framed connection must wait for the next request"
        );
        writeln!(reader.get_mut(), "{}", request(2)).unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["id"], 2);
        drop(reader);
        worker.join().unwrap().unwrap();
    }
}
