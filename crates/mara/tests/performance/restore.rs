//! What a case that writes changed, so that the next run starts from the same files: restored
//! outside the timed span (DES-PERFORMANCE-HARNESS).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The files and directories that one run of a write case changed below a project root, `.git`
/// aside.
pub struct Written {
    root: PathBuf,
    /// Each changed file with its former contents, `None` for one the run created.
    files: Vec<(PathBuf, Option<Vec<u8>>)>,
    /// Directories the run created, deepest first.
    directories: Vec<PathBuf>,
    /// The new contents of each file the run wrote.
    pub contents: Vec<Vec<u8>>,
}

impl Written {
    /// Runs `run` once and records what it changed below `root`.
    pub fn learn(root: &Path, run: impl FnOnce()) -> Written {
        let (files_before, directories_before) = files(root);
        run();
        let (files_after, directories_after) = files(root);
        let mut changed = Vec::new();
        let mut contents = Vec::new();
        for (path, bytes) in &files_after {
            if files_before.get(path) != Some(bytes) {
                changed.push((path.clone(), files_before.get(path).cloned()));
                contents.push(bytes.clone());
            }
        }
        assert!(
            files_before
                .keys()
                .all(|path| files_after.contains_key(path)),
            "a write case deleted a file"
        );
        let mut directories: Vec<PathBuf> = directories_after
            .difference(&directories_before)
            .cloned()
            .collect();
        directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        Written {
            root: root.to_owned(),
            files: changed,
            directories,
            contents,
        }
    }

    /// Puts back what the run changed.
    pub fn restore(&self) {
        for (path, former) in &self.files {
            let absolute = self.root.join(path);
            match former {
                Some(bytes) => fs::write(absolute, bytes).expect("restore a file"),
                None => fs::remove_file(absolute).expect("remove a created file"),
            }
        }
        for directory in &self.directories {
            fs::remove_dir(self.root.join(directory)).expect("remove a created directory");
        }
    }

    /// The number of distinct contents the run wrote into documents, which it parses.
    pub fn distinct_documents(&self) -> u64 {
        let written = self.files.iter().zip(&self.contents);
        let documents =
            written.filter(|((path, _), _)| path.to_string_lossy().ends_with(".mara.md"));
        documents
            .map(|(_, bytes)| bytes)
            .collect::<BTreeSet<_>>()
            .len() as u64
    }

    /// The number of code files the run wrote.
    pub fn code_files(&self) -> u64 {
        let paths = self.files.iter().map(|(path, _)| path.to_string_lossy());
        paths.filter(|path| path.ends_with(".rs")).count() as u64
    }
}

/// Every file below `root` with its contents, and every directory, `.git` aside.
fn files(root: &Path) -> (BTreeMap<PathBuf, Vec<u8>>, BTreeSet<PathBuf>) {
    let mut files = BTreeMap::new();
    let mut directories = BTreeSet::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(root.join(&directory)).expect("list a directory") {
            let entry = entry.expect("read a directory entry");
            let relative = directory.join(entry.file_name());
            if entry.file_name() == ".git" {
                continue;
            }
            if entry.file_type().expect("an entry's type").is_dir() {
                directories.insert(relative.clone());
                pending.push(relative);
            } else {
                files.insert(relative, fs::read(entry.path()).expect("read a file"));
            }
        }
    }
    (files, directories)
}

/// Waits until a write started now finds every file changed at or before `last` settled: at
/// least 2 s after its last change, so its checks read it again only when DES-INPUTS-AND-KEYS
/// would (VER-WORK-COUNTERS).
pub fn settle(last: SystemTime) {
    let settled = last + Duration::from_millis(2_100);
    if let Ok(wait) = settled.duration_since(SystemTime::now()) {
        std::thread::sleep(wait);
    }
}
