//! What an operation takes from the process of its client, rather than from the process that
//! runs it (DES-DAEMON-LIFECYCLE in `docs/daemon/index.mara.md`): the umask, which sets the mode
//! of every file and directory it creates, and the temporary directory.
//!
//! In direct mode the operation runs in the client's process, and both are that process's own.
//! The daemon serves requests of many clients at once on threads that share one umask and one
//! environment, so it runs each request inside [`serve`] with the values its client sent. An
//! operation therefore creates files and directories only through [`create_dir`],
//! [`create_new`], [`write`] and [`tempdir`], and finds the temporary directory with
//! [`temp_dir`]. An operation that reads another value of the environment adds it to [`Caller`]
//! and to the request.

use std::cell::RefCell;
use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The values of a client's process that an operation uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caller {
    /// The client's umask.
    pub umask: u32,
    /// The client's temporary directory: `TMPDIR`, or `/tmp` when it is unset, made absolute.
    pub temp_dir: PathBuf,
}

impl Caller {
    /// The values of this process. The umask is read by setting it and setting it back, so
    /// this runs before the process starts a thread that creates files.
    pub fn of_this_process() -> Caller {
        // SAFETY: umask takes no pointer and cannot fail.
        let umask = unsafe {
            let umask = libc::umask(0o077);
            libc::umask(umask);
            umask
        };
        // `mode_t` is `u16` on macOS.
        #[allow(clippy::useless_conversion)]
        let umask = u32::from(umask);
        Caller {
            umask,
            temp_dir: std::env::temp_dir(),
        }
    }
}

thread_local! {
    /// The client of the request that this thread serves in the daemon.
    static SERVED: RefCell<Option<Caller>> = const { RefCell::new(None) };
}

/// Runs `serve` on this thread as a request of `caller`.
pub fn serve<T>(caller: Caller, serve: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            SERVED.with(|served| served.borrow_mut().take());
        }
    }
    SERVED.with(|served| *served.borrow_mut() = Some(caller));
    let _reset = Reset;
    serve()
}

/// The umask of the client this thread serves, if it serves one in the daemon. In direct mode
/// the process's own umask applies by itself.
pub fn umask() -> Option<u32> {
    SERVED.with(|served| served.borrow().as_ref().map(|caller| caller.umask))
}

/// The temporary directory of the request's client.
pub fn temp_dir() -> PathBuf {
    SERVED
        .with(|served| (served.borrow().as_ref()).map(|caller| caller.temp_dir.clone()))
        .unwrap_or_else(std::env::temp_dir)
}

/// Gives what was just created at `path` the mode `requested` less the client's umask, as
/// creating it in the client's process would have: the daemon's own umask may have taken
/// other bits.
fn restore(path: &Path, requested: u32, umask: Option<u32>) -> io::Result<()> {
    match umask {
        Some(umask) => fs::set_permissions(path, Permissions::from_mode(requested & !umask)),
        None => Ok(()),
    }
}

/// Creates the directory `path` with the mode direct mode would give it: 0777 less the umask.
pub fn create_dir(path: &Path) -> io::Result<()> {
    let umask = umask();
    let mode = 0o777 & !umask.unwrap_or(0);
    fs::DirBuilder::new().mode(mode).create(path)?;
    restore(path, 0o777, umask)
}

/// Creates the file `path`, which must not exist, for writing, with the mode direct mode would
/// give it: 0666 less the umask.
pub fn create_new(path: &Path) -> io::Result<File> {
    let umask = umask();
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o666 & !umask.unwrap_or(0))
        .open(path)?;
    if let Some(umask) = umask {
        file.set_permissions(Permissions::from_mode(0o666 & !umask))?;
    }
    Ok(file)
}

/// Creates or truncates the file `path` and writes `contents`, as `fs::write` does in the
/// client's process: a created file gets 0666 less the umask, an existing one keeps its mode.
pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let umask = umask();
    let existed = fs::symlink_metadata(path).is_ok();
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o666 & !umask.unwrap_or(0))
        .open(path)?;
    if !existed && let Some(umask) = umask {
        file.set_permissions(Permissions::from_mode(0o666 & !umask))?;
    }
    file.write_all(contents.as_ref())
}

/// A new private directory, named with `prefix`, in the client's temporary directory, removed
/// when the value is dropped: mode 0700 less the umask, as in direct mode.
pub fn tempdir(prefix: &str) -> io::Result<tempfile::TempDir> {
    let umask = umask();
    let dir = tempfile::Builder::new()
        .prefix(prefix)
        .permissions(Permissions::from_mode(0o700 & !umask.unwrap_or(0)))
        .tempdir_in(temp_dir())?;
    restore(dir.path(), 0o700, umask)?;
    Ok(dir)
}
