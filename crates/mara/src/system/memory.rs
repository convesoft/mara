//! The filesystem of a browser workspace (DES-BROWSER-WORKSPACE in `docs/hosts/index.mara.md`):
//! the workspace's files in memory, behind the part of `std::fs` that the host's code uses, so
//! that the operations, the inventory and the write procedure run unchanged on supplied contents.
//!
//! The module's instance holds one filesystem, which the worker's one workspace fills
//! ([`reset`]). It is rooted at `/`, holds files and directories and no symbolic link. Each
//! change of a file's contents or of a directory's entries gives it a new modification time from a
//! logical clock that starts at the Unix epoch, one nanosecond per change: a change is always seen
//! by its stat, and every stat lies long before any check, so a check always settles what it read
//! (DES-INPUTS-AND-KEYS). A rename keeps the file's inode, as POSIX's does. A directory goes
//! with its last entry, as a Git checkout holds no empty directory.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

thread_local! {
    static TREE: RefCell<Tree> = RefCell::new(Tree::new());
}

/// Every file and directory, by absolute path without `.` or `..`.
#[derive(Debug)]
struct Tree {
    nodes: BTreeMap<PathBuf, Node>,
    /// The last tick of the logical clock.
    tick: u64,
    /// The last inode given.
    inode: u64,
}

#[derive(Debug)]
struct Node {
    inode: u64,
    modified: u64,
    permissions: Permissions,
    kind: Contents,
}

#[derive(Debug)]
enum Contents {
    File(Vec<u8>),
    Directory(BTreeSet<OsString>),
}

impl Tree {
    fn new() -> Tree {
        let mut tree = Tree {
            nodes: BTreeMap::new(),
            tick: 0,
            inode: 0,
        };
        let root = tree.node(Contents::Directory(BTreeSet::new()));
        tree.nodes.insert(PathBuf::from("/"), root);
        tree
    }

    fn node(&mut self, kind: Contents) -> Node {
        self.inode += 1;
        self.tick += 1;
        Node {
            inode: self.inode,
            modified: self.tick,
            permissions: Permissions::default(),
            kind,
        }
    }

    /// Marks the node at `path` changed now.
    fn touch(&mut self, path: &Path) {
        self.tick += 1;
        let tick = self.tick;
        if let Some(node) = self.nodes.get_mut(path) {
            node.modified = tick;
        }
    }

    /// The node at `path`, or why there is none: a missing part, or a part above it that is a
    /// file.
    fn get(&self, path: &Path) -> io::Result<&Node> {
        self.parent_directory(path)?;
        self.nodes.get(path).ok_or_else(not_found)
    }

    /// Checks that every directory above `path` exists and is a directory.
    fn parent_directory(&self, path: &Path) -> io::Result<()> {
        let mut ancestors: Vec<&Path> = path.ancestors().skip(1).collect();
        ancestors.reverse();
        for ancestor in ancestors {
            match self.nodes.get(ancestor) {
                None => return Err(not_found()),
                Some(Node {
                    kind: Contents::File(_),
                    ..
                }) => return Err(io::ErrorKind::NotADirectory.into()),
                Some(_) => {}
            }
        }
        Ok(())
    }

    /// Adds `node` at `path`, whose parent must be a directory, as its entry.
    fn insert(&mut self, path: &Path, node: Node) -> io::Result<()> {
        self.parent_directory(path)?;
        let (parent, name) = split(path)?;
        let Some(Node {
            kind: Contents::Directory(entries),
            ..
        }) = self.nodes.get_mut(&parent)
        else {
            return Err(not_found());
        };
        entries.insert(name);
        self.touch(&parent);
        self.nodes.insert(path.to_owned(), node);
        Ok(())
    }

    /// Takes the node at `path` out of its directory.
    fn take(&mut self, path: &Path) -> io::Result<Node> {
        self.get(path)?;
        let (parent, name) = split(path)?;
        if let Some(Node {
            kind: Contents::Directory(entries),
            ..
        }) = self.nodes.get_mut(&parent)
        {
            entries.remove(&name);
        }
        self.touch(&parent);
        Ok(self.nodes.remove(path).expect("looked up"))
    }

    /// Removes `directory` if it is empty and not the root, then each emptied directory above.
    fn prune(&mut self, directory: &Path) {
        let mut directory = directory.to_owned();
        while directory != Path::new("/") {
            match self.nodes.get(&directory) {
                Some(Node {
                    kind: Contents::Directory(entries),
                    ..
                }) if entries.is_empty() => {}
                _ => return,
            }
            let _ = self.take(&directory);
            directory.pop();
        }
    }
}

/// The parent and the name of an absolute path below the root.
fn split(path: &Path) -> io::Result<(PathBuf, OsString)> {
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => Ok((parent.to_owned(), name.to_owned())),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the root has no parent",
        )),
    }
}

fn not_found() -> io::Error {
    io::ErrorKind::NotFound.into()
}

/// `path` as the tree keys it: absolute, without `.` or `..`.
fn normal(path: &Path) -> io::Result<PathBuf> {
    let mut normal = PathBuf::from("/");
    let mut components = path.components();
    if components.next() != Some(Component::RootDir) {
        let message = format!("{} is not absolute", path.display());
        return Err(io::Error::new(io::ErrorKind::InvalidInput, message));
    }
    for component in components {
        match component {
            Component::Normal(name) => normal.push(name),
            Component::ParentDir => {
                normal.pop();
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    Ok(normal)
}

fn with<T>(change: impl FnOnce(&mut Tree) -> io::Result<T>) -> io::Result<T> {
    TREE.with(|tree| change(&mut tree.borrow_mut()))
}

/// What a stat tells of a file or directory.
#[derive(Clone, Debug)]
pub struct Metadata {
    file_type: FileType,
    len: u64,
    modified: SystemTime,
    inode: u64,
    permissions: Permissions,
}

impl Metadata {
    fn of(node: &Node) -> Metadata {
        let (file_type, len) = match &node.kind {
            Contents::File(contents) => (FileType::File, contents.len() as u64),
            Contents::Directory(entries) => (FileType::Directory, entries.len() as u64),
        };
        Metadata {
            file_type,
            len,
            modified: SystemTime::UNIX_EPOCH + Duration::from_nanos(node.modified),
            inode: node.inode,
            permissions: node.permissions.clone(),
        }
    }

    pub fn file_type(&self) -> FileType {
        self.file_type
    }

    pub fn is_file(&self) -> bool {
        self.file_type.is_file()
    }

    pub fn is_dir(&self) -> bool {
        self.file_type.is_dir()
    }

    /// The size of a file; the number of entries of a directory. As `std::fs::Metadata`, it has
    /// no `is_empty`.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn modified(&self) -> io::Result<SystemTime> {
        Ok(self.modified)
    }

    pub fn permissions(&self) -> Permissions {
        self.permissions.clone()
    }

    pub fn ino(&self) -> u64 {
        self.inode
    }
}

/// A file or a directory: the memory holds nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
}

impl FileType {
    pub fn is_file(&self) -> bool {
        *self == FileType::File
    }

    pub fn is_dir(&self) -> bool {
        *self == FileType::Directory
    }

    pub fn is_symlink(&self) -> bool {
        false
    }
}

/// Whether a file is read-only, which nothing here sets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Permissions {
    readonly: bool,
}

impl Permissions {
    pub fn readonly(&self) -> bool {
        self.readonly
    }
}

/// An entry of a directory.
#[derive(Debug)]
pub struct DirEntry {
    directory: PathBuf,
    name: OsString,
    file_type: FileType,
}

impl DirEntry {
    pub fn file_name(&self) -> OsString {
        self.name.clone()
    }

    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(self.file_type)
    }

    pub fn path(&self) -> PathBuf {
        self.directory.join(&self.name)
    }
}

/// The entries of a directory, by name.
pub type ReadDir = std::vec::IntoIter<io::Result<DirEntry>>;

/// The metadata of the file or directory at `path`.
pub fn symlink_metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    let path = normal(path.as_ref())?;
    with(|tree| tree.get(&path).map(Metadata::of))
}

/// As [`symlink_metadata`]: there is no link to follow.
pub fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    symlink_metadata(path)
}

/// The contents of the file at `path`.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let path = normal(path.as_ref())?;
    with(|tree| match &tree.get(&path)?.kind {
        Contents::File(contents) => Ok(contents.clone()),
        Contents::Directory(_) => Err(io::ErrorKind::IsADirectory.into()),
    })
}

/// The entries of the directory at `path`.
pub fn read_dir(path: impl AsRef<Path>) -> io::Result<ReadDir> {
    let path = normal(path.as_ref())?;
    with(|tree| {
        let Contents::Directory(names) = &tree.get(&path)?.kind else {
            return Err(io::ErrorKind::NotADirectory.into());
        };
        let entries: Vec<io::Result<DirEntry>> = names
            .iter()
            .map(|name| {
                let file_type = Metadata::of(&tree.nodes[&path.join(name)]).file_type;
                Ok(DirEntry {
                    directory: path.clone(),
                    name: name.clone(),
                    file_type,
                })
            })
            .collect();
        Ok(entries.into_iter())
    })
}

/// `path` without `.` and `..`, which must name a file or directory.
pub fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = normal(path.as_ref())?;
    with(|tree| tree.get(&path).map(|_| path.clone()))
}

/// Creates the directory `path`, whose parent must exist.
pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
    let path = normal(path.as_ref())?;
    with(|tree| {
        if tree.get(&path).is_ok() {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        let node = tree.node(Contents::Directory(BTreeSet::new()));
        tree.insert(&path, node)
    })
}

/// Creates the directory `path` and every missing directory above it.
pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let path = normal(path.as_ref())?;
    let mut ancestors: Vec<&Path> = path.ancestors().collect();
    ancestors.reverse();
    for directory in ancestors {
        match symlink_metadata(directory) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Err(io::ErrorKind::NotADirectory.into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => create_dir(directory)?,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Gives the file at `path` the contents `contents`, creating it if it does not exist. Its
/// directory must exist.
pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let path = normal(path.as_ref())?;
    let contents = contents.as_ref().to_vec();
    with(|tree| match tree.get(&path) {
        Ok(Node {
            kind: Contents::Directory(_),
            ..
        }) => Err(io::ErrorKind::IsADirectory.into()),
        Ok(_) => {
            tree.nodes.get_mut(&path).expect("looked up").kind = Contents::File(contents);
            tree.touch(&path);
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let node = tree.node(Contents::File(contents));
            tree.insert(&path, node)
        }
        Err(error) => Err(error),
    })
}

/// Removes the file at `path`, and each directory above that it leaves empty.
pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    let path = normal(path.as_ref())?;
    with(|tree| match tree.get(&path)?.kind {
        Contents::File(_) => {
            tree.take(&path)?;
            tree.prune(path.parent().expect("a file lies below the root"));
            Ok(())
        }
        Contents::Directory(_) => Err(io::ErrorKind::IsADirectory.into()),
    })
}

/// Moves the file at `from` to `to`, replacing a file there, as POSIX's `rename` does, and
/// removes each directory above `from` that it leaves empty. A directory is never moved or
/// replaced here.
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let (from, to) = (normal(from.as_ref())?, normal(to.as_ref())?);
    with(|tree| {
        if let Contents::Directory(_) = tree.get(&from)?.kind {
            return Err(io::ErrorKind::IsADirectory.into());
        }
        if from == to {
            return Ok(());
        }
        match tree.get(&to) {
            Ok(Node {
                kind: Contents::Directory(_),
                ..
            }) => return Err(io::ErrorKind::IsADirectory.into()),
            Ok(_) => drop(tree.take(&to)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => tree.parent_directory(&to)?,
            Err(error) => return Err(error),
        }
        let node = tree.take(&from)?;
        tree.insert(&to, node)?;
        tree.prune(from.parent().expect("a file lies below the root"));
        Ok(())
    })
}

/// Sets the permissions of the file or directory at `path`.
pub fn set_permissions(path: impl AsRef<Path>, permissions: Permissions) -> io::Result<()> {
    let path = normal(path.as_ref())?;
    with(|tree| {
        tree.get(&path)?;
        tree.nodes.get_mut(&path).expect("looked up").permissions = permissions;
        Ok(())
    })
}

/// An open file or directory. Writing appends to the file's contents.
#[derive(Debug)]
pub struct File {
    path: PathBuf,
}

impl File {
    /// Opens the file or directory at `path`.
    pub fn open(path: impl AsRef<Path>) -> io::Result<File> {
        let path = canonicalize(path)?;
        Ok(File { path })
    }

    /// Creates the file `path`, which must not exist, empty.
    pub fn create_new(path: impl AsRef<Path>) -> io::Result<File> {
        let path = normal(path.as_ref())?;
        with(|tree| {
            if tree.get(&path).is_ok() {
                return Err(io::ErrorKind::AlreadyExists.into());
            }
            let node = tree.node(Contents::File(Vec::new()));
            tree.insert(&path, node)
        })?;
        Ok(File { path })
    }

    pub fn metadata(&self) -> io::Result<Metadata> {
        symlink_metadata(&self.path)
    }

    /// Locks nothing: one worker runs the module's requests one at a time.
    pub fn lock(&self) -> io::Result<()> {
        self.metadata().map(drop)
    }

    pub fn sync_all(&self) -> io::Result<()> {
        self.metadata().map(drop)
    }

    pub fn set_permissions(&self, permissions: Permissions) -> io::Result<()> {
        set_permissions(&self.path, permissions)
    }
}

impl io::Write for File {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        with(|tree| {
            let node = tree.nodes.get_mut(&self.path).ok_or_else(not_found)?;
            let Contents::File(contents) = &mut node.kind else {
                return Err(io::ErrorKind::IsADirectory.into());
            };
            contents.extend_from_slice(bytes);
            tree.touch(&self.path);
            Ok(bytes.len())
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Empties the filesystem and fills it with `files`, each a path relative to the root with its
/// contents, creating the directories above them.
pub fn reset(files: impl IntoIterator<Item = (String, Vec<u8>)>) -> io::Result<()> {
    TREE.with(|tree| *tree.borrow_mut() = Tree::new());
    for (path, contents) in files {
        let path = Path::new("/").join(path);
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }
        write(&path, contents)?;
    }
    Ok(())
}

/// Every file, by its path relative to the root, with its contents.
pub fn files() -> BTreeMap<String, Vec<u8>> {
    TREE.with(|tree| {
        (tree.borrow().nodes.iter())
            .filter_map(|(path, node)| match &node.kind {
                Contents::File(contents) => {
                    let relative = path.strip_prefix("/").expect("absolute");
                    Some((relative.to_string_lossy().into_owned(), contents.clone()))
                }
                Contents::Directory(_) => None,
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    fn filled() {
        reset([
            ("docs/a.mara.md".to_owned(), b"a".to_vec()),
            ("docs/sub/b.mara.md".to_owned(), b"bb".to_vec()),
        ])
        .unwrap();
    }

    #[test]
    fn lists_reads_and_stats_what_it_was_filled_with() {
        filled();
        let names: Vec<_> = read_dir("/docs")
            .unwrap()
            .map(|entry| entry.unwrap())
            .map(|entry| (entry.file_name(), entry.file_type().unwrap()))
            .collect();
        assert_eq!(
            names,
            [
                (OsString::from("a.mara.md"), FileType::File),
                (OsString::from("sub"), FileType::Directory)
            ]
        );
        assert_eq!(read("/docs/sub/b.mara.md").unwrap(), b"bb");
        assert_eq!(symlink_metadata("/docs/sub/b.mara.md").unwrap().len(), 2);
        assert_eq!(canonicalize("/docs/./sub/..").unwrap(), Path::new("/docs"));
        assert_eq!(
            files().into_keys().collect::<Vec<_>>(),
            ["docs/a.mara.md", "docs/sub/b.mara.md"]
        );
    }

    #[test]
    fn reports_missing_paths_and_files_in_the_way_as_std_does() {
        filled();
        let kind = |path: &str| symlink_metadata(path).unwrap_err().kind();
        assert_eq!(kind("/docs/c.mara.md"), io::ErrorKind::NotFound);
        assert_eq!(kind("/docs/a.mara.md/x"), io::ErrorKind::NotADirectory);
        assert_eq!(kind("/none/x"), io::ErrorKind::NotFound);
        assert_eq!(
            create_dir("/docs").unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            read("/docs").unwrap_err().kind(),
            io::ErrorKind::IsADirectory
        );
        assert_eq!(
            File::create_new("/docs/a.mara.md").unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert!(symlink_metadata("docs").is_err());
    }

    /// Every change shows in the stat of the file or directory it changes, and a rename keeps
    /// the file's inode.
    #[test]
    fn every_change_shows_in_a_stat() {
        filled();
        let stat = |path: &str| {
            let metadata = symlink_metadata(path).unwrap();
            (metadata.modified().unwrap(), metadata.ino(), metadata.len())
        };
        let (file, directory) = (stat("/docs/a.mara.md"), stat("/docs"));
        write("/docs/a.mara.md", "b").unwrap();
        assert_ne!(stat("/docs/a.mara.md"), file);
        assert_eq!(stat("/docs"), directory);
        let mut temporary = File::create_new("/docs/.t").unwrap();
        temporary.write_all(b"new").unwrap();
        let written = stat("/docs/.t");
        assert_ne!(stat("/docs"), directory);
        rename("/docs/.t", "/docs/a.mara.md").unwrap();
        assert_eq!(read("/docs/a.mara.md").unwrap(), b"new");
        assert_eq!(stat("/docs/a.mara.md").1, written.1);
        assert!(symlink_metadata("/docs/.t").is_err());
        remove_file("/docs/a.mara.md").unwrap();
        assert!(symlink_metadata("/docs/a.mara.md").is_err());
        assert!(SystemTime::UNIX_EPOCH + Duration::from_secs(1) > stat("/docs").0);
    }

    /// A directory goes with its last file, so that a file can take its path.
    #[test]
    fn a_directory_goes_with_its_last_file() {
        filled();
        remove_file("/docs/sub/b.mara.md").unwrap();
        assert!(symlink_metadata("/docs/sub").is_err());
        write("/docs/sub", "a file now").unwrap();
        rename("/docs/a.mara.md", "/moved.mara.md").unwrap();
        assert_eq!(
            files().into_iter().collect::<Vec<_>>(),
            [
                ("docs/sub".to_owned(), b"a file now".to_vec()),
                ("moved.mara.md".to_owned(), b"a".to_vec())
            ]
        );
        remove_file("/docs/sub").unwrap();
        assert!(symlink_metadata("/docs").is_err());
        assert!(symlink_metadata("/").unwrap().is_dir());
    }
}
