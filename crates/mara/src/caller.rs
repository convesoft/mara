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
#[cfg(not(target_arch = "wasm32"))]
use std::fs::{self, Permissions};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
#[cfg(not(target_arch = "wasm32"))]
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
    /// The values of this process.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn of_this_process() -> Caller {
        Caller {
            umask: this_umask(),
            temp_dir: std::env::temp_dir(),
        }
    }
}

/// This process's umask, read without changing it from `Umask:` of `/proc/self/status` (Linux
/// 4.7 and later).
#[cfg(target_os = "linux")]
fn this_umask() -> u32 {
    let status = fs::read("/proc/self/status").unwrap_or_default();
    // An older kernel, or no `/proc`.
    umask_in(&status).unwrap_or_else(umask_by_setting)
}

/// The umask in the `Umask:` line of a `/proc/<pid>/status`. The file is read as bytes: other
/// lines, such as the task's name, need not be UTF-8.
#[cfg(target_os = "linux")]
fn umask_in(status: &[u8]) -> Option<u32> {
    let field =
        (status.split(|&byte| byte == b'\n')).find_map(|line| line.strip_prefix(b"Umask:"))?;
    u32::from_str_radix(std::str::from_utf8(field).ok()?.trim(), 8).ok()
}

#[cfg(not(any(target_os = "linux", target_arch = "wasm32")))]
fn this_umask() -> u32 {
    umask_by_setting()
}

/// This process's umask, read where the system offers no way to read it but setting it, as on
/// macOS: set and set back. Race: a file another thread of this process creates between the two
/// calls gets mode bits less 0o077 instead of less the umask, and two threads that read it so at
/// once can leave 0o077 set.
#[cfg(not(target_arch = "wasm32"))]
fn umask_by_setting() -> u32 {
    // SAFETY: umask takes no pointer and cannot fail.
    let umask = unsafe {
        let umask = libc::umask(0o077);
        libc::umask(umask);
        umask
    };
    // `mode_t` is `u16` on macOS.
    #[allow(clippy::useless_conversion)]
    u32::from(umask)
}

thread_local! {
    /// The client of the request that this thread serves in the daemon.
    static SERVED: RefCell<Option<Caller>> = const { RefCell::new(None) };
}

/// Runs `serve` on this thread as a request of `caller`, then restores the caller this thread
/// served before, if any.
pub fn serve<T>(caller: Caller, serve: impl FnOnce() -> T) -> T {
    struct Restore(Option<Caller>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SERVED.with(|served| *served.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(SERVED.with(|served| served.borrow_mut().replace(caller)));
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
#[cfg(not(target_arch = "wasm32"))]
fn restore(path: &Path, requested: u32, umask: Option<u32>) -> io::Result<()> {
    match umask {
        Some(umask) => fs::set_permissions(path, Permissions::from_mode(requested & !umask)),
        None => Ok(()),
    }
}

/// Creates the directory `path` with the mode direct mode would give it: 0777 less the umask.
#[cfg(not(target_arch = "wasm32"))]
pub fn create_dir(path: &Path) -> io::Result<()> {
    let umask = umask();
    let mode = 0o777 & !umask.unwrap_or(0);
    fs::DirBuilder::new().mode(mode).create(path)?;
    restore(path, 0o777, umask)
}

/// Creates the file `path`, which must not exist, for writing, with the mode direct mode would
/// give it: 0666 less the umask.
#[cfg(not(target_arch = "wasm32"))]
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
/// Whether it is created is the open's own answer, so a file another process creates or removes
/// meanwhile gets no wrong mode: one removed between the two opens is a `NotFound` error, as is a
/// link to nothing, which `fs::write` would follow.
pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let mut file = match create_new(path) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            OpenOptions::new().write(true).truncate(true).open(path)?
        }
        created => created?,
    };
    file.write_all(contents.as_ref())
}

/// A new private directory, named with `prefix`, in the client's temporary directory, removed
/// when the value is dropped: mode 0700 less the umask, as in direct mode.
#[cfg(not(target_arch = "wasm32"))]
pub fn tempdir(prefix: &str) -> io::Result<TempDir> {
    let umask = umask();
    let dir = tempfile::Builder::new()
        .prefix(prefix)
        .permissions(Permissions::from_mode(0o700 & !umask.unwrap_or(0)))
        .tempdir_in(temp_dir())?;
    restore(dir.path(), 0o700, umask)?;
    Ok(dir)
}

/// A private directory removed when dropped ([`tempdir`]).
#[cfg(not(target_arch = "wasm32"))]
pub type TempDir = tempfile::TempDir;

// A browser workspace has no operating system to create files on: its writes go through
// `crate::system`, and the functions below, which only an evaluation calls, report that.

/// The directory `path`, which a browser workspace cannot create.
#[cfg(target_arch = "wasm32")]
pub fn create_dir(path: &Path) -> io::Result<()> {
    std::fs::create_dir(path)
}

/// The file `path`, which a browser workspace cannot create.
#[cfg(target_arch = "wasm32")]
pub fn create_new(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

/// A private directory, which a browser workspace cannot create.
#[cfg(target_arch = "wasm32")]
pub fn tempdir(prefix: &str) -> io::Result<TempDir> {
    let message = format!("a browser workspace has no temporary directory for {prefix}");
    Err(io::Error::new(io::ErrorKind::Unsupported, message))
}

/// A private directory, of which a browser workspace has none.
#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
pub struct TempDir(std::convert::Infallible);

#[cfg(target_arch = "wasm32")]
impl TempDir {
    pub fn path(&self) -> &Path {
        match self.0 {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caller(umask: u32) -> Caller {
        Caller {
            umask,
            temp_dir: std::env::temp_dir(),
        }
    }

    #[test]
    fn this_umask_is_the_one_a_child_inherits() {
        let output = std::process::Command::new("sh")
            .args(["-c", "umask"])
            .output()
            .unwrap();
        let printed = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            u32::from_str_radix(printed.trim(), 8).unwrap(),
            Caller::of_this_process().umask
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_umask_is_read_from_a_status_whose_name_is_no_utf8() {
        let status = b"Name:\tmara\xff\xfe\nUmask:\t0027\nState:\tR (running)\n";
        assert_eq!(umask_in(status), Some(0o027));
        assert_eq!(umask_in(b"Name:\tmara\nState:\tR\n"), None);
    }

    #[test]
    fn a_nested_serve_restores_the_outer_caller() {
        serve(caller(0o022), || {
            serve(caller(0o077), || assert_eq!(umask(), Some(0o077)));
            assert_eq!(umask(), Some(0o022));
        });
        assert_eq!(umask(), None);
    }

    #[test]
    fn write_gives_a_created_file_the_callers_mode_and_keeps_an_existing_ones() {
        let dir = tempfile::tempdir().unwrap();
        let (created, existing) = (dir.path().join("created"), dir.path().join("existing"));
        fs::write(&existing, "old").unwrap();
        fs::set_permissions(&existing, Permissions::from_mode(0o640)).unwrap();
        serve(caller(0o002), || {
            write(&created, "new").unwrap();
            write(&existing, "new").unwrap();
        });
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&created), 0o664);
        assert_eq!(mode(&existing), 0o640);
        assert_eq!(fs::read_to_string(&existing).unwrap(), "new");
    }
}
