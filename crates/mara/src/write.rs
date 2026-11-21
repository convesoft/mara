//! Putting an edit on disk (DES-WRITE-PROCEDURE in `docs/authoring/writes.mara.md`): the project's write
//! lock, the checks that refuse an edit whose files changed after it was computed, and each file
//! replaced through a temporary file, in the order the edit gives. Nothing is read, hashed,
//! parsed or built after the renames (ADR-NO-REDERIVATION-IN-CLI).
//!
//! An operation takes the [`WriteLock`], loads its inputs, computes the edit as [`FileWrite`]s
//! with the [`Basis`] of each file, and passes them to [`apply`].

use std::fmt::Write as _;
use std::fs::{self, File, Metadata, Permissions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mara_engine::config::CONFIG_PATH;
use mara_engine::diagnostic::Diagnostic;
use mara_engine::work::WorkCounts;

use crate::caller;
use crate::hooks::{self, Hook};
use crate::project::SYMBOLIC_LINK;
use crate::tracked::{self, Check, Reads, Tracked};

/// How the name of a temporary file of a write starts. The inventory never takes such a file
/// for a code file, and its name never ends in `.mara.md`, so it is no input.
pub const TEMPORARY_PREFIX: &str = ".mara-tmp-";

/// The project's write lock: an exclusive advisory lock on its `.mara` directory, held until it
/// is dropped. Every Mara writer takes it before it loads what it edits.
#[derive(Debug)]
pub struct WriteLock {
    root: PathBuf,
    _directory: File,
}

impl WriteLock {
    /// Takes the write lock of the project at `root`, the canonical root that selection
    /// returns, waiting while another process holds it. A `.mara` that is a symbolic link is
    /// refused, never followed (DES-PROJECT-CONFIGURATION).
    pub fn take(root: &Path) -> Result<WriteLock, WriteError> {
        let mara = root.join(".mara");
        let refuse = |code, message| WriteError {
            code,
            message,
            path: CONFIG_PATH.to_owned(),
            replaced: Vec::new(),
            temporary: None,
        };
        let cannot_lock = |error: io::Error| {
            refuse(
                "io-error",
                format!("cannot lock .mara, so Mara cannot write: {error}"),
            )
        };
        let linked = || {
            refuse(
                SYMBOLIC_LINK,
                ".mara is a symbolic link, and Mara follows none: put the directory itself there"
                    .to_owned(),
            )
        };
        let before = fs::symlink_metadata(&mara).map_err(cannot_lock)?;
        if before.file_type().is_symlink() {
            return Err(linked());
        }
        let directory = File::open(&mara).map_err(cannot_lock)?;
        // `.mara` replaced by a link between the two looks would be another directory.
        let opened = directory.metadata().map_err(cannot_lock)?;
        if !opened.is_dir() || identity(&opened) != identity(&before) {
            return Err(linked());
        }
        directory.lock().map_err(cannot_lock)?;
        Ok(WriteLock {
            root: root.to_owned(),
            _directory: directory,
        })
    }
}

/// Device and inode.
fn identity(metadata: &Metadata) -> (u64, u64) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        (metadata.dev(), metadata.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        (0, 0)
    }
}

/// What a file that an edit writes held when the edit was computed.
#[derive(Clone, Debug)]
pub enum Basis {
    /// No file was there.
    Absent,
    /// The contents read, which the checks compare by hash.
    Contents(Version),
}

/// Contents as a check read them: their hash, and the file's stat and the time of that check, so
/// that a later check reads and hashes the file again only when DES-INPUTS-AND-KEYS would.
#[derive(Clone, Debug)]
pub struct Version(Tracked<()>);

impl Version {
    pub(crate) fn of<T>(tracked: &Tracked<T>) -> Version {
        Version(tracked.with_value(()))
    }
}

impl Basis {
    /// Reads the file at `path`, relative to the root of the locked project, for an edit: its
    /// contents, or `None` when no file is there, and its basis. Counts one file read and one
    /// hash when it reads. Use [`crate::inputs::Inputs::basis`] for a document of a snapshot.
    pub fn read(
        lock: &WriteLock,
        path: &str,
        work: &mut WorkCounts,
    ) -> Result<(Option<Vec<u8>>, Basis), WriteError> {
        let target = Target::new(&lock.root, path);
        let refused = |error: Refusal| error.into_error(path, &[], None);
        let Some(metadata) = target.metadata().map_err(refused)? else {
            return Ok((None, Basis::Absent));
        };
        let checked = tracked::check::<()>(
            &target.absolute,
            &metadata,
            None,
            SystemTime::now(),
            work,
            &mut Reads::default(),
            false,
        );
        match checked.map_err(|error| refused(Refusal::Unreadable(error)))? {
            Check::Read(read) => {
                let version = Version(read.with_value(()));
                Ok((Some(read.contents), Basis::Contents(version)))
            }
            Check::Kept(_) => unreachable!("a check without a previous value reads"),
        }
    }
}

/// One file of an edit: where it is, what the edit was computed against, and its new contents.
#[derive(Clone, Debug)]
pub struct FileWrite {
    /// Relative to the project root, with `/` between its parts, none of them empty, `.` or
    /// `..`.
    pub path: String,
    pub basis: Basis,
    pub contents: Vec<u8>,
}

/// Why an edit was refused or stopped.
#[derive(Debug)]
pub struct WriteError {
    /// `changed-file`, `symbolic-link` or `io-error`.
    pub code: &'static str,
    pub message: String,
    /// The file concerned, relative to the project root.
    pub path: String,
    /// The files replaced before the edit stopped, in order: empty when it was refused and
    /// nothing changed.
    pub replaced: Vec<String>,
    /// A temporary file that Mara wrote and could not confirm it removed, relative to the
    /// project root.
    pub temporary: Option<String>,
}

impl WriteError {
    /// The problem as a diagnostic at line 1 of the file concerned.
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::error(self.code, self.path.clone(), 1, self.message.clone())
    }
}

/// A file that changed after the edit was computed, or that cannot be checked or written.
enum Refusal {
    Changed(&'static str),
    Linked(String),
    Unreadable(io::Error),
    Io(io::Error),
}

/// A temporary file whose removal Mara could not confirm: its path relative to the project root
/// and why.
struct Leftover {
    path: String,
    reason: String,
}

impl Refusal {
    fn into_error(self, path: &str, replaced: &[String], left: Option<Leftover>) -> WriteError {
        let (code, mut message) = match self {
            Refusal::Changed(how) => (
                "changed-file",
                format!("{path} {how} after Mara read the project for this edit"),
            ),
            Refusal::Linked(link) => (
                SYMBOLIC_LINK,
                format!(
                    "{link} is a symbolic link, and Mara writes through none: put the file or \
                     directory itself there"
                ),
            ),
            Refusal::Unreadable(error) => ("io-error", format!("cannot read {path}: {error}")),
            Refusal::Io(error) => ("io-error", format!("cannot write {path}: {error}")),
        };
        match (replaced, &left) {
            ([], None) => message.push_str(", so nothing was written; run the operation again"),
            ([], Some(_)) => message.push_str(", so no file was replaced"),
            (replaced, _) => {
                let _ = write!(
                    message,
                    ", so the edit stopped after replacing {}; `mara project validate` reports \
                     what is left to repair",
                    replaced.join(", ")
                );
            }
        }
        if let Some(Leftover { path, reason }) = &left {
            let _ = write!(
                message,
                "; Mara could not confirm that it removed its temporary file {path} ({reason}): \
                 delete that file if it is there"
            );
        }
        WriteError {
            code,
            message,
            path: path.to_owned(),
            replaced: replaced.to_vec(),
            temporary: left.map(|left| left.path),
        }
    }
}

/// Writes the edit `files` into the locked project (DES-WRITE-PROCEDURE): checks that every
/// file still has its basis, which refuses the whole edit otherwise, then replaces each file in
/// order through a temporary file in its directory, checked once more just before its rename.
/// A change found after the first rename stops the edit there.
pub fn apply(
    lock: &WriteLock,
    files: &[FileWrite],
    work: &mut WorkCounts,
) -> Result<(), WriteError> {
    for (index, file) in files.iter().enumerate() {
        assert!(
            file.path
                .split('/')
                .all(|part| !matches!(part, "" | "." | "..")),
            "an edit's path is relative to the project root: {}",
            file.path
        );
        assert!(
            files[..index].iter().all(|other| other.path != file.path),
            "an edit writes {} once",
            file.path
        );
    }
    let first = files.first().map_or("", |file| file.path.as_str());
    hooks::pause(Hook::Pause, "")
        .map_err(|error| Refusal::Io(error).into_error(first, &[], None))?;
    let mut checked = Vec::with_capacity(files.len());
    for file in files {
        let target = Target::new(&lock.root, &file.path);
        let current = target
            .check(&file.basis, work)
            .map_err(|refusal| refusal.into_error(&file.path, &[], None))?;
        checked.push((target, current));
    }
    let mut replaced = Vec::with_capacity(files.len());
    for (file, (target, current)) in files.iter().zip(checked) {
        target
            .replace(file, current, work)
            .map_err(|(refusal, left)| refusal.into_error(&file.path, &replaced, left))?;
        replaced.push(file.path.clone());
        sync_directory(target.directory())
            .map_err(|error| Refusal::Io(error).into_error(&file.path, &replaced, None))?;
    }
    Ok(())
}

/// A file an edit writes.
struct Target<'r> {
    root: &'r Path,
    path: &'r str,
    absolute: PathBuf,
}

/// What a check found: no file, or the file with its basis as of that check and its
/// permissions.
enum Current {
    Absent,
    File(Basis, Permissions),
}

impl<'r> Target<'r> {
    fn new(root: &'r Path, path: &'r str) -> Target<'r> {
        Target {
            root,
            path,
            absolute: root.join(path),
        }
    }

    /// The file's metadata, `None` when it is absent. A symbolic link at it or at a directory
    /// above it, below the root, is refused.
    fn metadata(&self) -> Result<Option<Metadata>, Refusal> {
        let mut path = self.root.to_owned();
        let parts: Vec<&str> = self.path.split('/').collect();
        for (index, part) in parts.iter().enumerate() {
            path.push(part);
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(Refusal::Unreadable(error)),
            };
            if metadata.file_type().is_symlink() {
                return Err(Refusal::Linked(parts[..=index].join("/")));
            }
            if index + 1 == parts.len() {
                return Ok(Some(metadata));
            }
            if !metadata.is_dir() {
                let shown = parts[..=index].join("/");
                return Err(Refusal::Unreadable(io::Error::other(format!(
                    "{shown} is not a directory"
                ))));
            }
        }
        unreachable!("a path has a part")
    }

    /// Whether the file still has the contents of `basis`: still absent, or with the same hash,
    /// read again only when its stat changed or the last check that read it came less than 2 s
    /// after it changed (DES-INPUTS-AND-KEYS).
    fn check(&self, basis: &Basis, work: &mut WorkCounts) -> Result<Current, Refusal> {
        let metadata = self.metadata()?;
        match (basis, metadata) {
            (Basis::Absent, None) => Ok(Current::Absent),
            (Basis::Absent, Some(_)) => Err(Refusal::Changed("was created")),
            (Basis::Contents(_), None) => Err(Refusal::Changed("was removed")),
            (Basis::Contents(Version(version)), Some(metadata)) => {
                let checked = tracked::check(
                    &self.absolute,
                    &metadata,
                    Some(version.clone()),
                    SystemTime::now(),
                    work,
                    &mut Reads::default(),
                    false,
                );
                match checked.map_err(Refusal::Unreadable)? {
                    Check::Kept(kept) => Ok(Current::File(
                        Basis::Contents(Version(kept)),
                        metadata.permissions(),
                    )),
                    Check::Read(_) => Err(Refusal::Changed("changed")),
                }
            }
        }
    }

    /// Step 3 for this file: its missing directories created, its temporary file written,
    /// checked once more and renamed into place. A failure after the temporary file exists
    /// removes it and says when that removal is not confirmed.
    fn replace(
        &self,
        file: &FileWrite,
        current: Current,
        work: &mut WorkCounts,
    ) -> Result<(), (Refusal, Option<Leftover>)> {
        let (basis, mode) = match current {
            Current::Absent => {
                self.create_directories()
                    .map_err(|refusal| (refusal, None))?;
                (Basis::Absent, None)
            }
            Current::File(basis, mode) => (basis, Some(mode)),
        };
        let directory = self.directory();
        let (temporary, mut handle) =
            temporary_file(directory).map_err(|refusal| (refusal, None))?;
        let removed = |refusal: Refusal| {
            // A directory of the target replaced by a link redirects the removal too
            // (ADR-WRITE-PATHS-AS-USED), so it confirms nothing.
            let redirected = matches!(&refusal, Refusal::Linked(link) if link != self.path);
            let reason = match fs::remove_file(&temporary) {
                Err(error) => Some(error.to_string()),
                Ok(()) if redirected => Some(
                    "a directory on its path became a symbolic link, so the file removed there \
                     may be another"
                        .to_owned(),
                ),
                Ok(()) => None,
            };
            let name = temporary.file_name().unwrap_or_default().to_string_lossy();
            let path = match self.path.rsplit_once('/') {
                Some((parent, _)) => format!("{parent}/{name}"),
                None => name.into_owned(),
            };
            (refusal, reason.map(|reason| Leftover { path, reason }))
        };
        // The mode is set after the contents, whose writing clears set-user-ID and
        // set-group-ID bits.
        let written = (|| {
            handle.write_all(&file.contents)?;
            if let Some(mode) = mode {
                handle.set_permissions(mode)?;
            }
            handle.sync_all()?;
            hooks::pause(Hook::Pause, self.path)
        })();
        drop(handle);
        written.map_err(|error| removed(Refusal::Io(error)))?;
        self.check(&basis, work).map_err(removed)?;
        fs::rename(&temporary, &self.absolute).map_err(|error| removed(Refusal::Io(error)))
    }

    fn directory(&self) -> &Path {
        self.absolute
            .parent()
            .expect("a target lies below the root")
    }

    /// Creates each directory above the file that is missing, from the root down. One that
    /// exists is checked as [`Target::metadata`] checks it before anything goes into it.
    fn create_directories(&self) -> Result<(), Refusal> {
        let parts: Vec<&str> = self.path.split('/').collect();
        let mut path = self.root.to_owned();
        for (index, part) in parts[..parts.len() - 1].iter().enumerate() {
            let parent = path.clone();
            path.push(part);
            match caller::create_dir(&path) {
                Ok(()) => sync_directory(&parent).map_err(Refusal::Io)?,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let shown = || parts[..=index].join("/");
                    let metadata = fs::symlink_metadata(&path).map_err(Refusal::Unreadable)?;
                    if metadata.file_type().is_symlink() {
                        return Err(Refusal::Linked(shown()));
                    }
                    if !metadata.is_dir() {
                        let message = format!("{} is not a directory", shown());
                        return Err(Refusal::Unreadable(io::Error::other(message)));
                    }
                }
                Err(error) => return Err(Refusal::Io(error)),
            }
        }
        Ok(())
    }
}

/// A new temporary file in `directory`. Its name is short whatever the target's, hidden, and
/// does not end in `.mara.md`, so it is never read as an input.
fn temporary_file(directory: &Path) -> Result<(PathBuf, File), Refusal> {
    let process = std::process::id();
    let mut attempt = 0;
    loop {
        let path = directory.join(format!("{TEMPORARY_PREFIX}{process}-{attempt}"));
        match caller::create_new(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists && attempt < 100 => {
                attempt += 1;
            }
            Err(error) => return Err(Refusal::Io(error)),
        }
    }
}

/// Flushes a directory's entries, so that a rename survives a power loss in the edit's order.
fn sync_directory(directory: &Path) -> io::Result<()> {
    File::open(directory)?.sync_all()
}

/// A new MID from the current time and 80 bits of the operating system's random source
/// (DES-ENGINE-BOUNDARY).
pub fn new_mid() -> io::Result<String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64);
    let mut random = [0; 10];
    File::open("/dev/urandom")?.read_exact(&mut random)?;
    Ok(mara_engine::write::mid(millis, random))
}
