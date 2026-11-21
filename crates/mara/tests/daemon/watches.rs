//! The daemon's watches (DES-DAEMON-SESSIONS), as the kernel lists them in the `fdinfo` of the
//! daemon's inotify descriptor: they exist once `daemon start` opened a session, before any
//! request, and they follow a directory that is moved away and replaced, or deleted and created
//! again, as a branch checkout does. A sync of a session's watches holds no lock across its
//! system calls: the watcher reads events and derives meanwhile.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::{Test, WAIT, finish};

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

/// A request's sync of its session's watches, paused by `MARA_TEST_PAUSE_WATCH`
/// (ADR-DEBUG-PAUSE-POINT) where it looks up directories and adds watches, keeps neither the
/// thread that reads events nor a background derivation waiting: an edit made meanwhile is
/// derived, as `MARA_TEST_DERIVED` reports, before the sync goes on.
#[cfg(debug_assertions)]
#[test]
fn a_sync_paused_at_its_system_calls_keeps_the_watcher_working() {
    let test = Test::new();
    let project = test.project();
    let pauses = test.path().join("pauses");
    fs::create_dir(&pauses).unwrap();
    let paused = pauses.join("paused");
    let derived = test.path().join("derived");
    let derivations = || {
        fs::read_to_string(&derived)
            .unwrap_or_default()
            .lines()
            .count()
    };
    let wait_for = |what: &str, done: &dyn Fn() -> bool| {
        let deadline = Instant::now() + WAIT;
        while !done() {
            assert!(Instant::now() < deadline, "{what}");
            thread::sleep(Duration::from_millis(5));
        }
    };
    let env = [
        ("MARA_TEST_PAUSE_WATCH", pauses.to_str().unwrap()),
        ("MARA_TEST_DERIVED", derived.to_str().unwrap()),
    ];
    // `daemon start` derives its project, then syncs its watches.
    let pid = test.start_in(&project, &env);
    wait_for("no sync after the first derivation", &|| paused.exists());
    assert_eq!(derivations(), 1);
    fs::remove_file(&paused).unwrap();
    wait_for_watches(pid, &[inode(&project.join("docs"))], &[]);
    // A read's sync stops before its system calls.
    let read = ["--format", "json", "item", "list"];
    let mut client = test.spawn_in(&project, &read);
    wait_for("the read did not sync", &|| paused.exists());
    let document = project.join("docs/reqs.mara.md");
    let text = fs::read_to_string(&document).unwrap();
    fs::write(&document, text.replace("## Two", "## Deux")).unwrap();
    wait_for("the edit was not derived while a sync was paused", &|| {
        derivations() == 2
    });
    // The read and the derivation's own sync go on.
    while client.try_wait().unwrap().is_none() {
        let _ = fs::remove_file(&paused);
        thread::sleep(Duration::from_millis(5));
    }
    finish(client).succeeded();
    let _ = fs::remove_file(&paused);
    test.run(&["daemon", "stop"]).succeeded();
}
