//! VER-INTERRUPTED-EDITS for move and rename: each run stops the operation at one file F it
//! writes, in one of four ways, for every F; then `project validate` and, where the item says
//! so, a rerun.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};

use crate::{Project, documents, paused, temporary_left};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    /// `RLIMIT_FSIZE` below F's new size, `SIGXFSZ` at its default action.
    Killed,
    /// The same limit with `SIGXFSZ` ignored: the write fails with `EFBIG`.
    TemporaryFails,
    /// F changed at its pause before its second check.
    Changed,
    /// F's directory made read-only at that pause.
    RenameFails,
}

struct Operation {
    args: &'static [&'static str],
    /// The order of DES-WRITE-PROCEDURE.
    order: [&'static str; 3],
}

const MOVE: Operation = Operation {
    args: &["item", "move", "REQ-MOVED", "two/b.mara.md"],
    order: ["two/b.mara.md", "one/a.mara.md", "three/c.mara.md"],
};

const RENAME: Operation = Operation {
    args: &["item", "rename", "REQ-MOVED", "REQ-RENAMED"],
    order: ["three/c.mara.md", "one/a.mara.md", "two/b.mara.md"],
};

/// The project of [`documents`] with narrative at the top of `padded`, so that its new contents
/// are larger than those of every other file the operation writes.
fn padded(padded: &str) -> Project {
    let project = Project::three_directories();
    let padding = "Narrative that makes this file the largest that the edit writes.\n".repeat(300);
    let text = project
        .read(padded)
        .replacen("\n\n", &format!("\n\n{padding}\n"), 1);
    project.write(padded, &text);
    project
}

type Contents = BTreeMap<String, Vec<u8>>;

fn contents(project: &Project) -> Contents {
    let tree = project.tree();
    tree.into_iter()
        .filter(|(path, _)| path.ends_with(".mara.md"))
        .map(|(path, (bytes, _))| (path, bytes))
        .collect()
}

#[test]
fn stops_a_move_at_each_file_in_each_way() {
    stop_everywhere(&MOVE);
}

#[test]
fn stops_a_rename_at_each_file_in_each_way() {
    stop_everywhere(&RENAME);
}

fn stop_everywhere(operation: &Operation) {
    assert_ne!(
        // SAFETY: `geteuid` has no preconditions.
        unsafe { libc::geteuid() },
        0,
        "the rename-fails case needs a user without CAP_DAC_OVERRIDE; run the tests as another \
         user than root"
    );
    for (index, file) in operation.order.iter().enumerate() {
        // An uninterrupted run gives the order and every file's new contents.
        let learned = padded(file);
        let (output, pauses) = paused(&learned, operation.args, |_, _| {});
        assert!(output.status.success(), "{output:?}");
        assert_eq!(pauses[0], "", "the pause before step 2 holds nothing");
        assert_eq!(pauses[1..], operation.order, "the write order");
        let new = contents(&learned);
        let largest = (operation.order.iter())
            .filter(|path| *path != file)
            .map(|path| new[*path].len())
            .max()
            .unwrap();
        assert!(largest < new[*file].len(), "{file} is the largest");
        for stop in [
            Stop::Killed,
            Stop::TemporaryFails,
            Stop::Changed,
            Stop::RenameFails,
        ] {
            // The first file changed at its pause is VER-REFUSED-EDITS'.
            if stop == Stop::Changed && index == 0 {
                continue;
            }
            stopped(
                operation,
                index,
                stop,
                (largest + new[*file].len()) / 2,
                &new,
            );
        }
    }
}

fn stopped(operation: &Operation, index: usize, stop: Stop, limit: usize, new: &Contents) {
    let file = operation.order[index];
    let case = format!("{:?} stopped at {file} ({stop:?})", operation.args);
    let project = padded(file);
    let old = contents(&project);
    let mut changed = None;
    let output = match stop {
        Stop::Killed | Stop::TemporaryFails => {
            let mut command = project.mara(operation.args, &[]);
            let ignore = stop == Stop::TemporaryFails;
            let limit = limit as libc::rlim_t;
            // SAFETY: the closure runs in the child before `exec` and calls only `setrlimit` and
            // `signal`, which are async-signal-safe.
            unsafe {
                command.pre_exec(move || {
                    let size = libc::rlimit {
                        rlim_cur: limit,
                        rlim_max: limit,
                    };
                    if libc::setrlimit(libc::RLIMIT_FSIZE, &size) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    let action = if ignore { libc::SIG_IGN } else { libc::SIG_DFL };
                    libc::signal(libc::SIGXFSZ, action);
                    Ok(())
                });
            }
            command.output().expect("run mara")
        }
        Stop::Changed => {
            let text =
                String::from_utf8(old[file].clone()).unwrap() + "\nChanged by another program.\n";
            changed = Some(text.clone());
            paused(&project, operation.args, |held, _| {
                if held == file {
                    project.write(file, &text);
                }
            })
            .0
        }
        Stop::RenameFails => {
            let directory = project.path(file).parent().unwrap().to_owned();
            let set =
                |mode| fs::set_permissions(&directory, fs::Permissions::from_mode(mode)).unwrap();
            let output = paused(&project, operation.args, |held, _| {
                if held == file {
                    set(0o555);
                }
            })
            .0;
            set(0o755);
            output
        }
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    match stop {
        Stop::Killed => assert_eq!(
            output.status.signal(),
            Some(libc::SIGXFSZ),
            "{case}: {output:?}"
        ),
        _ => {
            assert_eq!(output.status.code(), Some(1), "{case}: {output:?}");
            assert!(
                stderr.contains(file),
                "{case}: the diagnostic names F: {stderr}"
            );
        }
    }
    if stop == Stop::TemporaryFails {
        assert!(!temporary_left(&project.tree()), "{case}");
    }

    // The files before F hold their new contents, F its former ones or the test's, every later
    // file its former contents.
    let after = contents(&project);
    for (at, path) in operation.order.iter().enumerate() {
        let expected = match at {
            _ if at < index => new[*path].clone(),
            _ if at == index && changed.is_some() => changed.clone().unwrap().into_bytes(),
            _ => old[*path].clone(),
        };
        assert!(after[*path] == expected, "{case}: {path}");
    }
    // Every item is still in some file with its MID and body.
    for (_, text) in documents() {
        for (at, _) in text.match_indices("<a id=\"") {
            let mid = &text[at + 7..at + 33];
            let id_at = text[at..].find("| ID | ").unwrap() + at + 7;
            let id = &text[id_at..id_at + text[id_at..].find(' ').unwrap()];
            let found = after.values().any(|bytes| {
                let text = String::from_utf8_lossy(bytes);
                text.contains(&format!("<a id=\"{mid}\"></a>"))
                    && text.contains(&format!("Body of {id}."))
            });
            assert!(found, "{case}: {id} is lost");
        }
    }

    // `project validate` reports what the stop left, and nothing else.
    let problems = project.any_problems();
    let codes: Vec<&str> = problems.iter().map(|(code, _)| code.as_str()).collect();
    // A file with mention text is written before the `ID` row with its relation link text too.
    let expected: &[&str] = match (operation.args[1], index) {
        (_, 0) => &[],
        ("move", 1) => &["duplicate-id", "duplicate-mid"],
        ("move", _) => &["stale-link-path"],
        _ => &["stale-link-text"],
    };
    let mut kinds = codes.clone();
    kinds.dedup();
    assert_eq!(kinds, expected, "{case}: {problems:?}");

    match (operation.args[1], index) {
        // Stopped while the item is in both files: a rerun completes the move.
        ("move", 1) => {
            project.run(operation.args, 0);
            assert_eq!(project.problems(), [], "{case}: after the rerun");
            if changed.is_none() {
                assert!(
                    contents(&project) == *new,
                    "{case}: the rerun writes what one run does"
                );
            }
        }
        // Stopped before the `ID` row: a rerun completes the rename.
        ("rename", 0 | 1) => {
            project.run(operation.args, 0);
            assert_eq!(project.problems(), [], "{case}: after the rerun");
        }
        // Stopped after it: mentions carry the new ID; only relation link text is stale.
        ("rename", _) => {
            for path in ["one/a.mara.md", "three/c.mara.md"] {
                let text = project.read(path);
                assert!(!text.contains("[REQ-MOVED]("), "{case}: {path}");
            }
            assert!(
                problems.iter().all(|(_, path)| path == "two/b.mara.md"),
                "{case}"
            );
        }
        _ => {}
    }
}
