//! The `project convert` cases of VER-INTERRUPTED-EDITS: a conversion stopped at each file it
//! writes, in each of the four ways, then rerun (ADR-CONVERSION-RERUN).

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;

use crate::{Project, Step, Tree};

const MIDS: [&str; 3] = [
    "01M7FX0D000000000000000001",
    "01M7FX0D000000000000000002",
    "01M7FX0D000000000000000003",
];

/// Items with relations and mentions across three directories.
fn fixture() -> Project {
    let project = Project::empty();
    project.write(
        ".mara/project.toml",
        "format_version = 3\n\n[project]\nname = \"stops\"\nschema = \".mara/schema.yaml\"\n\n[content]\ninclude = [\"**/*.mara.md\"]\n",
    );
    project.write(
        ".mara/schema.yaml",
        "format_version: 3\nflavours:\n  requirement:\n    description: An obligation.\n    use_when: [x]\n    avoid_when: []\n    distinguish_from: {}\n    id_prefix: REQ-\n    body: required\n    fields:\n      status: {type: enum, values: [draft]}\nrelations:\n  depends_on:\n    description: Needs.\n    source: [requirement]\n    target: [requirement]\n    inverse: required_by\n  related_to:\n    description: Related.\n    source: [requirement]\n    target: [requirement]\n    symmetric: true\n",
    );
    let item = |id: &str, mid: &str, metadata: &str, body: &str| {
        format!(
            "# {id}\n\n:::mara requirement {id}\n:mid: {mid}\n:title: Item {id}\n:status: draft\n{metadata}\n{body}\n:::\n"
        )
    };
    project.write(
        "one/a.mara.md",
        item("REQ-1", MIDS[0], ":depends_on: REQ-2\n", "Names [[REQ-3]]."),
    );
    project.write(
        "two/b.mara.md",
        item(
            "REQ-2",
            MIDS[1],
            ":required_by: REQ-3\n:related_to: REQ-1\n",
            "Names [[REQ-1]].",
        ),
    );
    project.write(
        "three/c.mara.md",
        item("REQ-3", MIDS[2], "", "Names [[depends_on:REQ-1]]."),
    );
    project
}

/// The tree of an uninterrupted conversion of a copy of `project`.
fn uninterrupted(project: &Project) -> Tree {
    let copy = project.copy();
    copy.convert();
    copy.tree()
}

/// Pads the document at `path` with narrative, so that its new contents are larger than those of
/// every other file the conversion writes.
fn pad(project: &Project, path: &str) {
    let mut text = project.read(path);
    text.push_str(&format!("\n{}\n", "Padding narrative. ".repeat(1000)));
    project.write(path, text);
}

/// Runs the conversion with its file size limited to `limit` bytes, `SIGXFSZ` ignored or not.
fn limited(project: &Project, limit: u64, ignore: bool) -> crate::Run {
    let mut command = project.command(&["project", "convert"]);
    unsafe {
        command.pre_exec(move || {
            let rlimit = libc::rlimit {
                rlim_cur: limit,
                rlim_max: limit,
            };
            if libc::setrlimit(libc::RLIMIT_FSIZE, &rlimit) != 0 {
                return Err(io::Error::last_os_error());
            }
            // A soft core limit of 1 byte where the hard limit allows it: then a piped
            // `core_pattern` records no crash for SIGXFSZ (DES-RULE-EXECUTION).
            let mut core = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            if libc::getrlimit(libc::RLIMIT_CORE, &mut core) == 0 && core.rlim_max >= 1 {
                core.rlim_cur = 1;
                libc::setrlimit(libc::RLIMIT_CORE, &core);
            }
            if ignore {
                libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
            }
            Ok(())
        });
    }
    crate::finish(command.spawn().expect("start mara"))
}

/// Before F the new contents, F and after it the former ones.
fn assert_stopped_at(project: &Project, base: &Tree, new: &Tree, order: &[String], at: usize) {
    let tree = project.tree();
    for (index, path) in order.iter().enumerate() {
        let expected = if index < at {
            new.get(path)
        } else {
            base.get(path)
        };
        assert_eq!(tree.get(path), expected, "{path}, stopped at {}", order[at]);
    }
}

/// Killed by the kernel while it writes F's temporary file, and the same with the write failing
/// with `EFBIG`; F is the report file or a document.
#[test]
fn stopped_by_the_file_size_limit_at_each_file_then_rerun() {
    let order = fixture().order();
    for ignore in [false, true] {
        for at in 0..order.len() - 2 {
            let project = fixture();
            if at > 0 {
                pad(&project, &order[at]);
            }
            let base = project.tree();
            let new = uninterrupted(&project);
            let size = |path: &str| new[path].len() as u64;
            let limit = size(&order[at]) - 1;
            for (index, path) in order.iter().enumerate() {
                if index != at && at > 0 {
                    assert!(size(path) <= limit, "{path} is larger than {}", order[at]);
                }
            }
            let run = limited(&project, limit, ignore);
            match ignore {
                false => assert_eq!(run.status, -1, "killed at {}: {run:?}", order[at]),
                true => {
                    assert_eq!(run.status, 1, "{run:?}");
                    assert_eq!(run.diagnostics()[0].0, order[at]);
                    assert_eq!(project.temporary_files(), Vec::<String>::new());
                }
            }
            assert_stopped_at(&project, &base, &new, &order, at);
            assert!(project.is_legacy());
            project.convert();
            assert_eq!(project.tree(), new, "rerun after a stop at {}", order[at]);
        }
    }
}

/// F changed by another program while the writer waits at its pause point, F not the first
/// file.
#[test]
fn stopped_by_a_file_changed_at_its_pause_point_then_rerun() {
    let base_project = fixture();
    let order = base_project.order();
    let base = base_project.tree();
    let new = uninterrupted(&base_project);
    for at in 1..order.len() {
        let project = base_project.copy();
        let path = order[at].clone();
        let changed = match base.get(&path) {
            Some(bytes) => format!("{}\nChanged by the test.\n", String::from_utf8_lossy(bytes)),
            None => "changed by the test\n".to_owned(),
        };
        let run = project.paused(project.command(&["project", "convert"]), |paused, _| {
            if paused == path {
                project.write(&path, &changed);
            }
            Step::Go
        });
        let run = run.unwrap();
        assert_eq!((run.status, run.code()), (1, "changed-file"), "{run:?}");
        assert_eq!(run.diagnostics()[0].0, path);
        let tree = project.tree();
        for (index, other) in order.iter().enumerate() {
            let expected = match index {
                index if index < at => new.get(other).cloned(),
                index if index == at => Some(changed.clone().into_bytes()),
                _ => base.get(other).cloned(),
            };
            assert_eq!(
                tree.get(other),
                expected.as_ref(),
                "{other}, stopped at {path}"
            );
        }
        assert_eq!(project.temporary_files(), Vec::<String>::new());
        // The rerun converts the project as the other program left it.
        let expected = match base.contains_key(&path) {
            true => {
                let reference = base_project.copy();
                reference.write(&path, &changed);
                uninterrupted(&reference)
            }
            false => new.clone(),
        };
        if path == ".mara/config.toml" {
            fs::remove_file(project.path(&path)).unwrap();
        }
        assert!(project.is_legacy(), "stopped at {path}");
        project.convert();
        assert_eq!(project.tree(), expected, "rerun after a stop at {path}");
    }
}

/// F's rename fails: its directory is not writable.
#[test]
fn stopped_by_a_failed_rename_at_each_file_then_rerun() {
    assert_ne!(
        unsafe { libc::geteuid() },
        0,
        "a failed rename needs a user without CAP_DAC_OVERRIDE; run the test as another user"
    );
    let base_project = fixture();
    let order = base_project.order();
    let base = base_project.tree();
    let new = uninterrupted(&base_project);
    for at in 0..order.len() {
        let project = base_project.copy();
        let path = order[at].clone();
        let directory = project.path(&path).parent().unwrap().to_owned();
        let mode =
            |mode| fs::set_permissions(&directory, fs::Permissions::from_mode(mode)).unwrap();
        let run = project.paused(project.command(&["project", "convert"]), |paused, _| {
            if paused == path {
                mode(0o555);
            }
            Step::Go
        });
        mode(0o755);
        let run = run.unwrap();
        assert_eq!((run.status, run.code()), (1, "io-error"), "{run:?}");
        assert_eq!(run.diagnostics()[0].0, path);
        assert_stopped_at(&project, &base, &new, &order, at);
        assert!(project.is_legacy());
        project.convert();
        assert_eq!(project.tree(), new, "rerun after a stop at {path}");
    }
}
