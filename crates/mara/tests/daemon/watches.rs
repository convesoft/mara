//! The daemon's watches (DES-DAEMON-SESSIONS), as the kernel lists them in the `fdinfo` of the
//! daemon's inotify descriptor: they exist once `daemon start` opened a session, before any
//! request, and they follow a directory that is moved away and replaced, or deleted and created
//! again, as a branch checkout does.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::{Test, WAIT};

/// The inodes of the directories the daemon `pid` watches.
fn watched(pid: u32) -> Vec<u64> {
    let mut inodes = Vec::new();
    let Ok(descriptors) = fs::read_dir(format!("/proc/{pid}/fd")) else {
        return inodes;
    };
    for descriptor in descriptors.flatten() {
        let is_inotify = fs::read_link(descriptor.path())
            .is_ok_and(|target| target.as_os_str() == "anon_inode:inotify");
        if !is_inotify {
            continue;
        }
        let name = descriptor.file_name();
        let info = fs::read_to_string(format!("/proc/{pid}/fdinfo/{}", name.to_string_lossy()))
            .unwrap_or_default();
        for line in info.lines().filter(|line| line.starts_with("inotify wd:")) {
            let inode = line
                .split_whitespace()
                .find_map(|field| field.strip_prefix("ino:"))
                .and_then(|hex| u64::from_str_radix(hex, 16).ok());
            inodes.extend(inode);
        }
    }
    inodes
}

fn inode(path: &Path) -> u64 {
    fs::metadata(path).unwrap().ino()
}

/// Waits until the daemon `pid` watches each of `inodes` and none of `not`.
#[track_caller]
fn wait_for_watches(pid: u32, inodes: &[u64], not: &[u64]) {
    let deadline = Instant::now() + WAIT;
    loop {
        let now = watched(pid);
        if inodes.iter().all(|inode| now.contains(inode)) && !not.iter().any(|i| now.contains(i)) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "watched {now:?}, expected {inodes:?} and none of {not:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

/// Copies the files of `from`, a directory without subdirectories, into a new directory `to`.
fn copy_directory(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
    }
}

#[test]
fn daemon_start_watches_its_project_before_any_request() {
    let test = Test::new();
    let project = test.project();
    let pid = test.start_in(&project, &[]);
    let expected = [
        inode(&project),
        inode(&project.join(".mara")),
        inode(&project.join("docs")),
    ];
    wait_for_watches(pid, &expected, &[]);
}

#[test]
fn watches_follow_a_directory_moved_away_and_copied_back_or_created_again() {
    let test = Test::new();
    let project = test.project();
    let docs = project.join("docs");
    let pid = test.start_in(&project, &[]);
    let first = inode(&docs);
    wait_for_watches(pid, &[first], &[]);
    // Moved out of the project and copied back.
    let away = test.path().join("docs-away");
    fs::rename(&docs, &away).unwrap();
    copy_directory(&away, &docs);
    let second = inode(&docs);
    assert_ne!(first, second);
    wait_for_watches(pid, &[second], &[first]);
    // Deleted and created again.
    let copy = test.path().join("docs-copy");
    copy_directory(&docs, &copy);
    fs::remove_dir_all(&docs).unwrap();
    copy_directory(&copy, &docs);
    wait_for_watches(pid, &[inode(&docs)], &[]);
    // The new directory is watched: an edit there is seen, also by the background.
    let document = docs.join("reqs.mara.md");
    let text = fs::read_to_string(&document).unwrap();
    fs::write(&document, text.replace("## Two", "## Deux")).unwrap();
    let read = ["--format", "json", "item", "list"];
    assert_eq!(
        test.run_in(&project, &read, &[]),
        test.direct(&project, &read)
    );
}
