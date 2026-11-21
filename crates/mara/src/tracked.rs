//! Change detection by stat: a file is read and hashed again, and a directory listed again, only
//! when its stat changed or its last check came too soon after it changed (DES-INPUTS-AND-KEYS
//! in `docs/engine.mara.md`).

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use mara_engine::hash::Hash;
use mara_engine::work::WorkCounts;

/// A check less than this long after a file's last change does not settle its contents: an edit
/// in the same timestamp tick would leave its stat unchanged.
const SETTLE: Duration = Duration::from_secs(2);

/// What a stat, without following a symbolic link, tells of a file or directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Stat {
    size: u64,
    modified: Option<SystemTime>,
    changed: Option<SystemTime>,
    /// Device and inode.
    inode: (u64, u64),
}

impl Stat {
    pub(crate) fn of(metadata: &Metadata) -> Stat {
        #[cfg(unix)]
        let (changed, inode) = {
            use std::os::unix::fs::MetadataExt;
            let changed = u64::try_from(metadata.ctime()).ok().map(|seconds| {
                let nanos = u32::try_from(metadata.ctime_nsec()).unwrap_or(0);
                SystemTime::UNIX_EPOCH + Duration::new(seconds, nanos)
            });
            (changed, (metadata.dev(), metadata.ino()))
        };
        #[cfg(not(unix))]
        let (changed, inode) = (None, (0, 0));
        Stat {
            size: metadata.len(),
            modified: metadata.modified().ok(),
            changed,
            inode,
        }
    }

    /// Whether what a check at `checked` found still holds while the stat stays this one: the
    /// check came at least [`SETTLE`] after the last modification and status change.
    fn settled_at(&self, checked: SystemTime) -> bool {
        let last = match (self.modified, self.changed) {
            (Some(modified), Some(changed)) => modified.max(changed),
            (Some(modified), None) if cfg!(not(unix)) => modified,
            _ => return false,
        };
        last.checked_add(SETTLE)
            .is_some_and(|settled| settled <= checked)
    }
}

/// Whether a check of the file or directory whose metadata is `metadata` keeps what a check at
/// `checked` found with the stat `stat`: the stat is unchanged, and that check settled it.
fn kept_by(metadata: &Metadata, stat: Stat, checked: SystemTime) -> bool {
    Stat::of(metadata) == stat && stat.settled_at(checked)
}

/// A file's contents as of a check, with the value derived from them.
#[derive(Clone, Debug)]
pub(crate) struct Tracked<T> {
    stat: Stat,
    /// When the check that read the contents began.
    checked: SystemTime,
    pub(crate) hash: Hash,
    pub(crate) value: T,
}

/// The outcome of checking a tracked file.
pub(crate) enum Check<T> {
    /// The contents are those of the previous check, with its value.
    Kept(Tracked<T>),
    /// The contents changed or were never read: their value is to be derived.
    Read(Read),
}

/// Contents read and hashed by a check.
pub(crate) struct Read {
    stat: Stat,
    checked: SystemTime,
    pub(crate) hash: Hash,
    pub(crate) contents: Vec<u8>,
}

impl<T> Tracked<T> {
    /// Whether the file at `path` still has the stat it had when these contents were read: a
    /// result derived from them may be kept for the next check, which compares that stat.
    pub(crate) fn still_as_read(&self, path: &Path) -> bool {
        fs::symlink_metadata(path).is_ok_and(|metadata| Stat::of(&metadata) == self.stat)
    }

    /// Whether `other` records the same check of the same contents: the same stat, time and
    /// hash.
    pub(crate) fn same_check<U>(&self, other: &Tracked<U>) -> bool {
        (self.stat, self.checked, self.hash) == (other.stat, other.checked, other.hash)
    }

    /// Whether a check of the file whose metadata, without following a link, is `metadata`
    /// keeps these contents without reading it: the file has the stat they were read with, and
    /// their check settled them. Of several records of one file, this tells the one that
    /// answers, whatever their check times. The caller takes the metadata only where the check
    /// would, after the walk admitted the file's directories.
    pub(crate) fn holds(&self, metadata: &Metadata) -> bool {
        kept_by(metadata, self.stat, self.checked)
    }

    /// The same check of the same contents, with another value.
    pub(crate) fn with_value<U>(&self, value: U) -> Tracked<U> {
        Tracked {
            stat: self.stat,
            checked: self.checked,
            hash: self.hash,
            value,
        }
    }
}

impl Read {
    pub(crate) fn with<T>(self, value: T) -> Tracked<T> {
        self.with_value(value)
    }

    /// This check of these contents, with `value`, keeping the contents.
    pub(crate) fn with_value<T>(&self, value: T) -> Tracked<T> {
        Tracked {
            stat: self.stat,
            checked: self.checked,
            hash: self.hash,
            value,
        }
    }
}

/// The project files read so far in one request, by path, so that a file with another role as
/// well, such as a schema that a pattern also matches as a document, is read and hashed once
/// per request (DES-INPUTS-AND-KEYS); and which unsettled files a derivation that serves no
/// request keeps without reading ([`Keep`]).
#[derive(Debug, Default)]
pub(crate) struct Reads {
    files: HashMap<PathBuf, (Stat, Hash, Arc<[u8]>)>,
    pub(crate) keep: Keep,
}

/// Which files whose stat is that of their last check, but which that check did not settle, a
/// derivation keeps without reading them again. Only a derivation that answers no request
/// keeps any (DES-DAEMON-SESSIONS): the next request's own check reads them, since their record
/// stays unsettled (REQ-NEVER-STALE).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Keep {
    /// None: a request's check.
    #[default]
    None,
    /// Those that a check now would still leave unsettled, which reading could not settle: the
    /// daemon's background derivation.
    Unsettleable,
    /// Every one: the derivation after a write, which re-derives only the files it wrote.
    Unchanged,
}

impl Reads {
    /// Whether an unsettled file of stat `stat`, unchanged since its last check, is kept
    /// without reading at `now`.
    fn keeps(&self, stat: &Stat, now: SystemTime) -> bool {
        match self.keep {
            Keep::None => false,
            Keep::Unsettleable => !stat.settled_at(now),
            Keep::Unchanged => true,
        }
    }

    /// Reads of a request with the policy `keep`.
    pub(crate) fn new(keep: Keep) -> Reads {
        Reads {
            files: HashMap::new(),
            keep,
        }
    }

    /// Records `contents`, with hash `hash`, as those of the file at `path` while it has the
    /// stat `stat`, as a read of this request would.
    pub(crate) fn insert(&mut self, path: PathBuf, stat: Stat, hash: Hash, contents: Arc<[u8]>) {
        self.files.insert(path, (stat, hash, contents));
    }
}

// @mara implements REQ-NEVER-STALE
/// Checks the regular file at `path`, whose metadata without following links is `metadata`, in
/// a request that began at `now`. Reads and hashes it, counting one file read and one hash in
/// `work`, unless the stat is unchanged and `previous` was settled or `reads` keeps it
/// ([`Keep`]), or `reads` holds the same file read in this request. With `share`, what it reads joins `reads`. Contents that hash as
/// before keep the previous value.
pub(crate) fn check<T>(
    path: &Path,
    metadata: &Metadata,
    previous: Option<Tracked<T>>,
    now: SystemTime,
    work: &mut WorkCounts,
    reads: &mut Reads,
    share: bool,
) -> io::Result<Check<T>> {
    let stat = Stat::of(metadata);
    let previous = match previous {
        Some(previous)
            if previous.stat == stat
                && (stat.settled_at(previous.checked) || reads.keeps(&stat, now)) =>
        {
            return Ok(Check::Kept(previous));
        }
        previous => previous,
    };
    if !metadata.is_file() {
        return Err(io::Error::other("it is not a regular file"));
    }
    let (hash, contents) = match reads.files.get(path) {
        Some((read, hash, contents)) if *read == stat => (*hash, contents.to_vec()),
        _ => {
            let contents = fs::read(path)?;
            work.file_reads += 1;
            let hash = Hash::of_file(&contents, work);
            if share {
                let shared = (stat, hash, Arc::from(contents.as_slice()));
                reads.files.insert(path.to_owned(), shared);
            }
            (hash, contents)
        }
    };
    let read = Read {
        stat,
        checked: now,
        hash,
        contents,
    };
    Ok(match previous {
        Some(previous) if previous.hash == hash => Check::Kept(read.with(previous.value)),
        _ => Check::Read(read),
    })
}

/// A directory's entries as of a check.
#[derive(Clone, Debug)]
pub(crate) struct Listing {
    stat: Stat,
    checked: SystemTime,
    /// By name.
    pub(crate) entries: Arc<[Entry]>,
}

#[derive(Debug)]
pub(crate) struct Entry {
    /// With each byte sequence that is not UTF-8 replaced by U+FFFD.
    pub(crate) name: String,
    /// The name is UTF-8 as it is.
    pub(crate) utf8: bool,
    pub(crate) kind: Kind,
}

/// An entry's type, as the directory lists it, without following a symbolic link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Directory,
    File,
    SymbolicLink,
    /// A socket, pipe or device, or a type that cannot be determined.
    Other,
}

impl Listing {
    /// [`Tracked::holds`] for a directory: a check keeps this listing.
    pub(crate) fn holds(&self, metadata: &Metadata) -> bool {
        kept_by(metadata, self.stat, self.checked)
    }
}

/// Lists the directory at `path`, whose metadata is `metadata`, in a request that began at
/// `now`, unless `previous` was settled and the stat is unchanged.
pub(crate) fn list(
    path: &Path,
    metadata: &Metadata,
    previous: Option<Listing>,
    now: SystemTime,
) -> io::Result<Listing> {
    let stat = Stat::of(metadata);
    if let Some(previous) = previous
        && previous.stat == stat
        && stat.settled_at(previous.checked)
    {
        return Ok(previous);
    }
    if !metadata.is_dir() {
        return Err(io::Error::other("it is not a directory"));
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let kind = match entry.file_type() {
            Ok(kind) if kind.is_symlink() => Kind::SymbolicLink,
            Ok(kind) if kind.is_dir() => Kind::Directory,
            Ok(kind) if kind.is_file() => Kind::File,
            _ => Kind::Other,
        };
        let name: OsString = entry.file_name();
        let (name, utf8) = match name.into_string() {
            Ok(name) => (name, true),
            Err(name) => (name.to_string_lossy().into_owned(), false),
        };
        entries.push(Entry { name, utf8, kind });
    }
    entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    Ok(Listing {
        stat,
        checked: now,
        entries: entries.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The case the 2 s rule exists for: an edit that keeps the size and every timestamp of a
    /// file, within the timestamp tick of the check that hashed it, is found by the next check,
    /// however late it comes. Here the stale hash stands for what that check read before the
    /// edit.
    #[test]
    fn an_edit_in_the_tick_of_the_hashing_check_is_found_later() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mara.md");
        fs::write(&path, "after").unwrap();
        let metadata = fs::symlink_metadata(&path).unwrap();
        let stat = Stat::of(&metadata);
        let changed = stat.modified.unwrap().max(stat.changed.unwrap());
        let mut work = WorkCounts::default();
        let before = Hash::of_file(b"befor", &mut work);
        let previous = |checked| Tracked {
            stat,
            checked,
            hash: before,
            value: (),
        };
        let later = changed + Duration::from_secs(3600);
        let check = |checked, work: &mut WorkCounts| {
            let previous = Some(previous(checked));
            check(
                &path,
                &metadata,
                previous,
                later,
                work,
                &mut Reads::default(),
                false,
            )
            .unwrap()
        };

        let mut work = WorkCounts::default();
        let found = check(changed + Duration::from_millis(1), &mut work);
        let Check::Read(read) = found else {
            panic!("the stale hash was kept");
        };
        assert_eq!(read.contents, b"after");
        assert_eq!((work.file_reads, work.hashes), (1, 1));

        // A hash taken 2 s or more after the last change is kept while the stat stays.
        let mut work = WorkCounts::default();
        let kept = check(changed + SETTLE, &mut work);
        assert!(matches!(kept, Check::Kept(_)));
        assert_eq!(work, WorkCounts::default());
    }
}
