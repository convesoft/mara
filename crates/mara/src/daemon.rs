//! Where the daemon of a build lives, and the frames its clients exchange with it
//! (DES-DAEMON-LIFECYCLE and ADR-DAEMON-PER-VERSION in
//! `docs/daemon/index.mara.md`).
//!
//! A [`Build`] names the running executable. [`Runtime::find`] places its lock and socket in the
//! runtime directory, or says why every call runs in direct mode. The process itself, its
//! requests and `mara daemon` are the CLI's (`cli/daemon.rs`).

use std::fs::{self, DirBuilder, File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use mara_engine::hash::Hash;

/// The bytes of `sun_path`, which holds a socket path and its terminating NUL.
#[cfg(target_os = "macos")]
const SUN_PATH: usize = 104;
#[cfg(not(target_os = "macos"))]
const SUN_PATH: usize = 108;

/// The hexadecimal digits of a build's key that name its lock and socket.
const NAME_DIGITS: usize = 16;

/// The largest frame either side accepts.
pub const MAX_FRAME: usize = 64 << 20;

/// How long a client tries to connect while the daemon's queue of connections is full: past it,
/// the client serves its request in-process.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// How often the daemon tells a client that its request still runs, while it runs.
pub const HEARTBEAT: Duration = Duration::from_secs(1);

/// The version of the frames' JSON messages. Only one build talks to its own daemon, so a
/// change needs no compatibility; the field marks the format.
pub const PROTOCOL_VERSION: u64 = 1;

/// The running executable: its version with the path, size and modification time of its file
/// (ADR-DAEMON-PER-VERSION).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Build {
    pub version: &'static str,
    pub executable: PathBuf,
    pub size: u64,
    /// Seconds and nanoseconds since the epoch.
    pub modified: (i64, i64),
}

impl Build {
    /// The build of this process. On Linux the size and time are those of the image that runs,
    /// even when another file has since replaced it at its path.
    pub fn current() -> io::Result<Build> {
        let executable = std::env::current_exe()?;
        let executable = fs::canonicalize(&executable).unwrap_or(executable);
        #[cfg(target_os = "linux")]
        let metadata = fs::metadata("/proc/self/exe")?;
        #[cfg(not(target_os = "linux"))]
        let metadata = fs::metadata(&executable)?;
        Ok(Build {
            version: env!("CARGO_PKG_VERSION"),
            executable,
            size: metadata.size(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
        })
    }

    /// The lowercase hexadecimal BLAKE3 hash of every part of the build.
    pub fn key(&self) -> String {
        let mut bytes = Vec::new();
        for part in [
            self.version.as_bytes(),
            self.executable.as_os_str().as_encoded_bytes(),
            self.size.to_string().as_bytes(),
            format!("{}.{:09}", self.modified.0, self.modified.1).as_bytes(),
        ] {
            bytes.extend_from_slice(&(part.len() as u64).to_le_bytes());
            bytes.extend_from_slice(part);
        }
        Hash::of_bytes(&bytes).to_string()
    }
}

/// The runtime directory of one build: `mara/` in the base, with the build's lock and socket.
#[derive(Clone, Debug)]
pub struct Runtime {
    pub directory: PathBuf,
    pub lock: PathBuf,
    pub socket: PathBuf,
}

/// Why no daemon can serve this build: every call runs in direct mode.
#[derive(Debug)]
pub struct Unavailable(pub String);

impl Runtime {
    /// The runtime directory of `build`, without creating anything. The base must be an
    /// absolute directory, no symbolic link, owned by this user with mode exactly 0700, and the socket
    /// path must fit `sun_path`.
    pub fn find(build: &Build) -> Result<Runtime, Unavailable> {
        let base = base()?;
        private_directory(&base).map_err(|reason| {
            Unavailable(format!(
                "the runtime directory's base `{}` {reason}",
                base.display()
            ))
        })?;
        let directory = base.join("mara");
        let name = &build.key()[..NAME_DIGITS];
        let socket = directory.join(format!("{name}.sock"));
        let length = socket.as_os_str().len();
        if length + 1 > SUN_PATH {
            return Err(Unavailable(format!(
                "the socket path `{}` has {length} bytes, more than a socket path can hold \
                 ({} bytes)",
                socket.display(),
                SUN_PATH - 1
            )));
        }
        Ok(Runtime {
            lock: directory.join(format!("{name}.lock")),
            socket,
            directory,
        })
    }

    /// Creates the directory with mode 0700 if it is missing, and checks it as
    /// [`Runtime::check`] does.
    pub fn create(&self) -> io::Result<()> {
        match DirBuilder::new().mode(0o700).create(&self.directory) {
            // The umask may have taken bits away; nobody else can write in the base.
            Ok(()) => fs::set_permissions(&self.directory, fs::Permissions::from_mode(0o700))?,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(annotate(error, "cannot create", &self.directory));
            }
        }
        self.check().map(drop)
    }

    /// Whether the directory exists, and an error unless it is a directory, no symbolic link,
    /// owned by this user with mode exactly 0700: then its socket can only be ours.
    pub fn check(&self) -> io::Result<bool> {
        match private_directory(&self.directory) {
            Ok(()) => Ok(true),
            Err(_)
                if fs::symlink_metadata(&self.directory).is_err_and(|error| not_found(&error)) =>
            {
                Ok(false)
            }
            Err(reason) => Err(io::Error::other(format!(
                "the runtime directory `{}` {reason}; remove it, and Mara creates it again",
                self.directory.display()
            ))),
        }
    }

    /// Opens the lock file without following a symbolic link. A missing lock is created under
    /// another name, given mode 0600 whatever the umask, and linked into place, so that no
    /// process sees it with another mode. A lock of another mode or owner is refused.
    pub fn open_lock(&self) -> io::Result<File> {
        loop {
            let opened = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&self.lock);
            let metadata = match &opened {
                Ok(file) => file.metadata()?,
                Err(error) if not_found(error) => {
                    self.create_lock()?;
                    continue;
                }
                // One that cannot be opened for writing, such as a lock of mode 0400.
                Err(error) => match fs::symlink_metadata(&self.lock) {
                    Ok(metadata) if error.kind() == io::ErrorKind::PermissionDenied => metadata,
                    _ => {
                        let error = io::Error::new(error.kind(), error.to_string());
                        return Err(annotate(error, "cannot open the daemon lock", &self.lock));
                    }
                },
            };
            let mode = metadata.mode() & 0o7777;
            if !metadata.is_file() || metadata.uid() != user() || mode != 0o600 {
                return Err(io::Error::other(format!(
                    "the daemon lock `{}` is not a file of this user with mode 0600 (mode \
                     {mode:04o}); remove it, and Mara creates it again",
                    self.lock.display()
                )));
            }
            return opened;
        }
    }

    /// Writes an empty lock of mode 0600 and links it into place, unless another process did.
    fn create_lock(&self) -> io::Result<()> {
        let staged = self.staged_lock();
        // One that an earlier process of this ID left.
        let _ = fs::remove_file(&staged);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&staged)
            .map_err(|error| annotate(error, "cannot create the daemon lock", &staged))?;
        let linked = file
            .set_permissions(fs::Permissions::from_mode(0o600))
            .and_then(|()| match fs::hard_link(&staged, &self.lock) {
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
                linked => linked,
            });
        let _ = fs::remove_file(&staged);
        linked.map_err(|error| annotate(error, "cannot create the daemon lock", &self.lock))
    }

    /// A name for a new lock before it is linked into place, which no other call of any live
    /// process uses: the lock's name, the process ID and a number this process never gives
    /// twice, so that two threads of one process starting the daemon at once never collide.
    fn staged_lock(&self) -> PathBuf {
        static STAGED: AtomicU64 = AtomicU64::new(0);
        let number = STAGED.fetch_add(1, Ordering::Relaxed);
        let mut staged = self.lock.as_os_str().to_owned();
        staged.push(format!(".{}.{number}", std::process::id()));
        PathBuf::from(staged)
    }

    /// Binds the socket with mode 0600 whatever the umask, and checks it. The process must run
    /// no other thread, since the umask is the process's.
    pub fn bind(&self) -> io::Result<UnixListener> {
        // SAFETY: umask takes no pointer and cannot fail.
        let umask = unsafe { libc::umask(0o177) };
        let bound = UnixListener::bind(&self.socket);
        // SAFETY: as above.
        unsafe { libc::umask(umask) };
        let listener = bound.map_err(|error| annotate(error, "cannot listen on", &self.socket))?;
        let checked = fs::set_permissions(&self.socket, fs::Permissions::from_mode(0o600))
            .and_then(|()| fs::symlink_metadata(&self.socket))
            .and_then(|metadata| private_socket(&metadata).map_err(io::Error::other));
        if let Err(error) = checked {
            let _ = fs::remove_file(&self.socket);
            return Err(annotate(error, "cannot restrict", &self.socket));
        }
        Ok(listener)
    }

    /// The process that last took the lock, as it recorded itself in the lock file.
    pub fn lock_holder(&self) -> Option<u32> {
        let mut text = String::new();
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&self.lock)
            .ok()?;
        file.read_to_string(&mut text).ok()?;
        text.trim().parse().ok()
    }

    /// Connects to the daemon of this build: `None` when none listens. The peer must run as
    /// this user.
    pub fn connect(&self) -> io::Result<Option<UnixStream>> {
        if !self.check()? {
            return Ok(None);
        }
        match fs::symlink_metadata(&self.socket) {
            Ok(metadata) => private_socket(&metadata).map_err(|reason| {
                io::Error::other(format!("`{}` {reason}; remove it", self.socket.display()))
            })?,
            Err(error) if not_found(&error) => return Ok(None),
            Err(error) => return Err(annotate(error, "cannot examine", &self.socket)),
        }
        match connect_within(&self.socket, CONNECT_TIMEOUT) {
            Ok(stream) => {
                if peer_user(&stream)? != user() {
                    return Err(io::Error::other(format!(
                        "the daemon socket `{}` is served by another user",
                        self.socket.display()
                    )));
                }
                Ok(Some(stream))
            }
            Err(error) if not_found(&error) || error.kind() == io::ErrorKind::ConnectionRefused => {
                Ok(None)
            }
            Err(error) => Err(annotate(error, "cannot connect to", &self.socket)),
        }
    }
}

/// Connects to the socket at `path`, trying again while the queue of connections the listener
/// has not accepted is full, for at most `timeout`: a blocking connection would wait for as long
/// as the listener does not accept. Fails with `TimedOut` then.
fn connect_within(path: &Path, timeout: Duration) -> io::Result<UnixStream> {
    use std::os::fd::{FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;

    let deadline = Instant::now() + timeout;
    // SAFETY: an all-zero sockaddr_un is valid.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let bytes = path.as_os_str().as_bytes();
    if bytes.len() >= address.sun_path.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the socket path is too long",
        ));
    }
    for (slot, byte) in address.sun_path.iter_mut().zip(bytes) {
        *slot = *byte as libc::c_char;
    }
    let length = (std::mem::size_of::<libc::sa_family_t>() + bytes.len() + 1) as libc::socklen_t;
    loop {
        // SAFETY: socket takes no pointer.
        let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the descriptor was just opened and nothing else owns it.
        let socket = unsafe { OwnedFd::from_raw_fd(fd) };
        // SAFETY: fcntl on an open descriptor.
        unsafe {
            libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
        // SAFETY: the address is initialized and `length` does not pass it.
        let connected = unsafe { libc::connect(fd, (&raw const address).cast(), length) };
        let error = io::Error::last_os_error();
        let pending = connected != 0 && error.raw_os_error() == Some(libc::EINPROGRESS);
        if connected == 0 || pending {
            if pending {
                wait_writable(fd, deadline)?;
            }
            // SAFETY: fcntl on an open descriptor.
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFL);
                libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK);
            }
            return Ok(UnixStream::from(socket));
        }
        if error.raw_os_error() != Some(libc::EAGAIN) {
            return Err(error);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "the daemon accepted no connection within {} s",
                    timeout.as_secs()
                ),
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Waits until a connection in progress on `fd` is made, at most until `deadline`.
fn wait_writable(fd: std::os::fd::RawFd, deadline: Instant) -> io::Result<()> {
    let left = deadline.saturating_duration_since(Instant::now());
    let mut poll = libc::pollfd {
        fd,
        events: libc::POLLOUT,
        revents: 0,
    };
    let millis = i32::try_from(left.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: one pollfd that lives across the call.
    if unsafe { libc::poll(&mut poll, 1, millis) } <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the daemon accepted no connection in time",
        ));
    }
    let mut status: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    // SAFETY: getsockopt writes at most `size` bytes into `status`.
    unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_ERROR,
            (&raw mut status).cast(),
            &mut size,
        );
    }
    match status {
        0 => Ok(()),
        error => Err(io::Error::from_raw_os_error(error)),
    }
}

/// Takes the lock without waiting: `false` when another process holds it.
pub fn try_lock(file: &File) -> io::Result<bool> {
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(TryLockError::WouldBlock) => Ok(false),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

/// [`write_frame`] on a connection, which ends for writing when the frame fails: a frame cut
/// after its length would make the peer read whatever comes next as its payload.
pub fn send_frame(stream: &UnixStream, payload: &[u8]) -> io::Result<()> {
    let mut writer = stream;
    let sent = write_frame(&mut writer, payload);
    if sent.is_err() {
        let _ = stream.shutdown(Shutdown::Write);
    }
    sent
}

/// Writes one frame: the payload's length as 4 bytes, big-endian, then the payload.
pub fn write_frame(stream: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    stream.write_all(&frame_length(payload)?)?;
    stream.write_all(payload)?;
    stream.flush()
}

/// The 4 bytes that start the frame of `payload`.
pub fn frame_length(payload: &[u8]) -> io::Result<[u8; 4]> {
    if payload.len() > MAX_FRAME {
        return Err(io::Error::other("the message is too large for the daemon"));
    }
    let length = u32::try_from(payload.len()).expect("MAX_FRAME fits in 32 bits");
    Ok(length.to_be_bytes())
}

/// Reads one frame: `None` when the stream ends before it starts.
pub fn read_frame(stream: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut length = [0; 4];
    let mut filled = 0;
    while filled < length.len() {
        match stream.read(&mut length[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(count) => filled += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    let length = u32::from_be_bytes(length) as usize;
    if length > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "a frame is too large",
        ));
    }
    let mut payload = vec![0; length];
    stream.read_exact(&mut payload)?;
    Ok(Some(payload))
}

/// The effective user ID of this process.
pub fn user() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

/// The effective user ID of the process at the other end of `stream`.
pub fn peer_user(stream: &UnixStream) -> io::Result<u32> {
    #[cfg(target_os = "linux")]
    {
        let mut credentials = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: the descriptor is open, and the buffer and its length match `ucred`.
        let status = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&raw mut credentials).cast(),
                &mut size,
            )
        };
        if status != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(credentials.uid)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let (mut uid, mut gid) = (0, 0);
        // SAFETY: the descriptor is open, and both pointers are writable.
        let status = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
        if status != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(uid)
    }
}

/// The base of the runtime directory: `XDG_RUNTIME_DIR`, or `/run/user/<uid>` when it is unset
/// or empty, on Linux; `TMPDIR` on macOS.
fn base() -> Result<PathBuf, Unavailable> {
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("TMPDIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| Unavailable("`TMPDIR` is not set".to_owned()))?;
    #[cfg(not(target_os = "macos"))]
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|value| !value.is_empty())
        .map_or_else(
            || PathBuf::from(format!("/run/user/{}", user())),
            PathBuf::from,
        );
    if !base.is_absolute() {
        return Err(Unavailable(format!(
            "the runtime directory's base `{}` is not absolute",
            base.display()
        )));
    }
    // Without a trailing `/`, which would make the last component's link followed.
    Ok(base.components().collect())
}

/// Why `path` is no private directory of this user: a directory, no symbolic link, owned by
/// this user, mode exactly 0700, special bits included. Symbolic links above it are followed.
fn private_directory(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if not_found(&error) => return Err("does not exist".to_owned()),
        Err(error) => return Err(format!("cannot be examined: {error}")),
    };
    if metadata.file_type().is_symlink() {
        Err("is a symbolic link".to_owned())
    } else if !metadata.is_dir() {
        Err("is no directory".to_owned())
    } else if metadata.uid() != user() {
        Err(format!(
            "belongs to user {}, not to this user",
            metadata.uid()
        ))
    } else if metadata.mode() & 0o7777 != 0o700 {
        Err(format!(
            "has mode {:04o}, not 0700",
            metadata.mode() & 0o7777
        ))
    } else {
        Ok(())
    }
}

/// Why a file is no socket of this user with mode 0600.
fn private_socket(metadata: &fs::Metadata) -> Result<(), String> {
    let mode = metadata.mode() & 0o7777;
    if !metadata.file_type().is_socket() {
        Err("is no socket".to_owned())
    } else if metadata.uid() != user() {
        Err(format!(
            "belongs to user {}, not to this user",
            metadata.uid()
        ))
    } else if mode != 0o600 {
        Err(format!("has mode {mode:04o}, not 0600"))
    } else {
        Ok(())
    }
}

fn not_found(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound
}

/// `error` with what was done and the path it concerns.
fn annotate(error: io::Error, what: &str, path: &Path) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("{what} `{}`: {error}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(path: &str, size: u64, modified: (i64, i64)) -> Build {
        Build {
            version: "1.0.0",
            executable: PathBuf::from(path),
            size,
            modified,
        }
    }

    #[test]
    fn every_part_of_a_build_changes_its_key() {
        let one = build("/bin/mara", 10, (5, 6));
        let keys = [
            one.clone(),
            Build {
                version: "1.0.1",
                ..one.clone()
            },
            build("/bin/mara2", 10, (5, 6)),
            build("/bin/mara", 11, (5, 6)),
            build("/bin/mara", 10, (5, 7)),
            build("/bin/mara", 10, (6, 6)),
        ]
        .map(|build| build.key());
        for (index, key) in keys.iter().enumerate() {
            assert_eq!(key.len(), 64);
            assert!(!keys[index + 1..].contains(key), "{key}");
        }
        assert_eq!(one.key(), build("/bin/mara", 10, (5, 6)).key());
    }

    #[test]
    fn threads_of_one_process_create_the_lock_at_once() {
        let base = tempfile::tempdir().unwrap();
        let directory = base.path().join("mara");
        let runtime = Runtime {
            lock: directory.join("key.lock"),
            socket: directory.join("key.sock"),
            directory,
        };
        runtime.create().unwrap();
        // Each staged name is new, so no call removes the file another thread is linking.
        assert_ne!(runtime.staged_lock(), runtime.staged_lock());
        let threads = 16;
        let barrier = std::sync::Barrier::new(threads);
        std::thread::scope(|scope| {
            let opened: Vec<_> = (0..threads)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        runtime.open_lock().map(drop)
                    })
                })
                .collect();
            for thread in opened {
                thread.join().unwrap().unwrap();
            }
        });
        // Only the lock is left, of mode 0600.
        let names: Vec<_> = fs::read_dir(&runtime.directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["key.lock"]);
        let mode = fs::metadata(&runtime.lock).unwrap().mode() & 0o7777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn a_frame_cut_after_its_length_ends_the_connection_for_writing() {
        let (writer, mut reader) = UnixStream::pair().unwrap();
        writer
            .set_write_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        // Larger than any socket buffer, and never read while it is written: it times out
        // after its length and part of its payload.
        let payload = vec![b' '; 16 << 20];
        assert!(send_frame(&writer, &payload).is_err());
        // Nothing more is written: not a frame that the reader would take for the payload.
        let next = send_frame(&writer, b"{}").unwrap_err();
        assert_eq!(next.kind(), io::ErrorKind::BrokenPipe);
        // The reader finds the frame cut short by the end of the stream.
        let cut = read_frame(&mut reader).unwrap_err();
        assert_eq!(cut.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn frames_carry_their_payload_and_end_cleanly() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, b"{}").unwrap();
        write_frame(&mut bytes, b"").unwrap();
        let mut reader = bytes.as_slice();
        assert_eq!(read_frame(&mut reader).unwrap(), Some(b"{}".to_vec()));
        assert_eq!(read_frame(&mut reader).unwrap(), Some(Vec::new()));
        assert_eq!(read_frame(&mut reader).unwrap(), None);
        let mut cut = &bytes[..3];
        assert!(read_frame(&mut cut).is_err());
        let huge = ((MAX_FRAME + 1) as u32).to_be_bytes();
        assert!(read_frame(&mut huge.as_slice()).is_err());
    }
}
