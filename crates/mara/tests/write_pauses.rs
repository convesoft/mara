//! The pause points of the write procedure in a debug build (DES-WRITE-PROCEDURE and
//! ADR-DEBUG-PAUSE-POINT in `docs/authoring/writes.mara.md`), below the CLI: the checks of step 2 and of
//! step 3 against a file changed while the writer waits, and a rename that fails. The CLI cases
//! of VER-REFUSED-EDITS and VER-INTERRUPTED-EDITS run with each write operation.
//!
//! The environment variable that turns the pauses on acts on the whole process, so this binary
//! holds one test, which sets it before any other thread starts.

#![cfg(all(unix, debug_assertions))]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use mara::inputs::Inputs;
use mara::write::{self, Basis, FileWrite, WriteError, WriteLock};
use mara_engine::work::WorkCounts;

const CONFIG: &str = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\nformat_version = 1\ninclude = [\"**/*.mara.md\"]\n";

const SCHEMA: &str = "format_version = 1\nstatuses = [\"draft\"]\n\n[types.requirement]\nid_prefix = \"REQ-\"\ndescription = \"A requirement.\"\n";

const FILES: [&str; 3] = ["one/a.mara.md", "two/b.mara.md", "three/c.mara.md"];

type Tree = BTreeMap<String, (Vec<u8>, SystemTime)>;

/// A project with the three documents of [`FILES`], each holding its own path, read by a
/// request that settles them.
struct Project {
    _dir: tempfile::TempDir,
    root: PathBuf,
    inputs: Inputs,
}

impl Project {
    fn new() -> Project {
        let dir = tempfile::Builder::new()
            .prefix("mara-pauses-")
            .tempdir()
            .unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        for (path, contents) in [(".mara/config.toml", CONFIG), (".mara/schema.toml", SCHEMA)]
            .into_iter()
            .chain(FILES.map(|path| (path, path)))
        {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        let mut inputs = Inputs::new(root.clone());
        let settled = SystemTime::now() + Duration::from_secs(3);
        inputs.snapshot(settled, &mut WorkCounts::default());
        Project {
            _dir: dir,
            root,
            inputs,
        }
    }

    /// The edit that writes `new <path>` into each of `paths`.
    fn edit(&self, paths: &[&str]) -> Vec<FileWrite> {
        paths
            .iter()
            .map(|path| FileWrite {
                path: (*path).to_owned(),
                basis: self.inputs.basis(path).unwrap_or(Basis::Absent),
                contents: format!("new {path}").into_bytes(),
            })
            .collect()
    }

    fn read(&self, path: &str) -> Option<String> {
        fs::read_to_string(self.root.join(path)).ok()
    }

    /// Every file below the root with its bytes and mtime.
    fn tree(&self) -> Tree {
        fn walk(root: &Path, directory: &Path, out: &mut Tree) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                let metadata = fs::symlink_metadata(&path).unwrap();
                if metadata.is_dir() {
                    walk(root, &path, out);
                } else {
                    let relative = path.strip_prefix(root).unwrap().to_string_lossy().into();
                    let bytes = fs::read(&path).unwrap_or_default();
                    out.insert(relative, (bytes, metadata.modified().unwrap()));
                }
            }
        }
        let mut out = Tree::new();
        walk(&self.root, &self.root, &mut out);
        out
    }

    /// Runs the edit of `paths` in another thread. At each pause, calls `at` with the contents
    /// of `paused` and the number of earlier pauses, then removes `paused`. Returns the result
    /// and the contents of every pause, in order.
    fn run(
        &self,
        pauses: &Path,
        paths: &[&str],
        mut at: impl FnMut(&str, usize),
    ) -> (Result<(), WriteError>, Vec<String>) {
        let edit = self.edit(paths);
        let root = self.root.clone();
        let writer = thread::spawn(move || {
            let lock = WriteLock::take(&root).unwrap();
            write::apply(&lock, &edit, &mut WorkCounts::default())
        });
        let paused = pauses.join("paused");
        let mut seen = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(60);
        while !writer.is_finished() {
            assert!(
                Instant::now() < deadline,
                "the writer neither pauses nor ends"
            );
            match fs::read_to_string(&paused) {
                Ok(contents) => {
                    at(&contents, seen.len());
                    seen.push(contents);
                    fs::remove_file(&paused).unwrap();
                }
                Err(_) => thread::sleep(Duration::from_millis(2)),
            }
        }
        (writer.join().unwrap(), seen)
    }
}

/// Paths of the tree that differ between `before` and `after`, or are in only one of them.
fn changed(before: &Tree, after: &Tree) -> Vec<String> {
    let mut paths: Vec<String> = before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

#[test]
fn pauses_where_the_write_procedure_says_and_its_checks_hold() {
    let pauses = tempfile::Builder::new()
        .prefix("mara-pause-")
        .tempdir()
        .unwrap();
    // SAFETY: this binary's only test sets the variable before it starts any thread, and no
    // other code of the process reads or writes the environment at the same time.
    unsafe { std::env::set_var("MARA_TEST_PAUSE", pauses.path()) };
    let other = |project: &Project, path: &str| {
        fs::write(project.root.join(path), "the other program's contents").unwrap();
    };

    // Uninterrupted: an empty `paused` before step 2, then each file's path in the edit's order.
    let project = Project::new();
    let (result, seen) = project.run(pauses.path(), &FILES, |_, _| {});
    result.unwrap();
    assert_eq!(seen, ["", FILES[0], FILES[1], FILES[2]]);
    for path in FILES {
        assert_eq!(project.read(path), Some(format!("new {path}")));
    }

    // Step 2: the first, a later or the only file changed before writing began. Nothing is
    // replaced, and no temporary file is left.
    for (paths, changed_file) in [
        (&FILES[..], FILES[2]),
        (&FILES[..], FILES[0]),
        (&FILES[1..2], FILES[1]),
        (&["new/dir/d.mara.md"][..], "new/dir/d.mara.md"),
    ] {
        let project = Project::new();
        let before = project.tree();
        let (result, seen) = project.run(pauses.path(), paths, |_, index| {
            if index == 0 {
                let path = project.root.join(changed_file);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                other(&project, changed_file);
            }
        });
        let error = result.expect_err("refused");
        assert_eq!(
            (error.code, error.path.as_str()),
            ("changed-file", changed_file)
        );
        assert!(error.replaced.is_empty(), "{error:?}");
        assert_eq!(seen, [""], "no file was written");
        assert_eq!(changed(&before, &project.tree()), [changed_file]);
    }

    // Step 3, before the first or only file's rename: refused, nothing replaced, the other
    // program's contents kept. A new document's directories are created by then.
    for paths in [&FILES[..], &FILES[2..], &["new/dir/d.mara.md"][..]] {
        let project = Project::new();
        let before = project.tree();
        let first = paths[0];
        let (result, seen) = project.run(pauses.path(), paths, |paused, _| {
            if paused == first {
                other(&project, first);
            }
        });
        let error = result.expect_err("refused");
        assert_eq!((error.code, error.path.as_str()), ("changed-file", first));
        assert!(error.replaced.is_empty(), "{error:?}");
        assert!(
            error.message.contains("nothing was written"),
            "{}",
            error.message
        );
        assert_eq!(seen, ["", first]);
        assert_eq!(
            project.read(first).as_deref(),
            Some("the other program's contents")
        );
        assert_eq!(changed(&before, &project.tree()), [first]);
    }

    // Step 3, before a later file's rename: the edit stops there. The files before it hold
    // their new contents, it the other program's, the later ones their former contents.
    let project = Project::new();
    let before = project.tree();
    let (result, seen) = project.run(pauses.path(), &FILES, |paused, _| {
        if paused == FILES[1] {
            other(&project, FILES[1]);
        }
    });
    let error = result.expect_err("stopped");
    assert_eq!(
        (error.code, error.path.as_str()),
        ("changed-file", FILES[1])
    );
    assert_eq!(error.replaced, [FILES[0]]);
    assert!(
        error
            .message
            .contains(&format!("stopped after replacing {}", FILES[0])),
        "{}",
        error.message
    );
    assert_eq!(seen, ["", FILES[0], FILES[1]]);
    assert_eq!(project.read(FILES[0]), Some(format!("new {}", FILES[0])));
    assert_eq!(
        project.read(FILES[1]).as_deref(),
        Some("the other program's contents")
    );
    assert_eq!(project.read(FILES[2]).as_deref(), Some(FILES[2]));
    assert_eq!(changed(&before, &project.tree()), [FILES[0], FILES[1]]);

    // A rename that fails stops the edit with the files before it replaced.
    // SAFETY: `geteuid` has no preconditions.
    assert_ne!(
        unsafe { libc::geteuid() },
        0,
        "a failed rename needs a user other than root"
    );
    let project = Project::new();
    let before = project.tree();
    let directory = project.root.join("two");
    let (result, _) = project.run(pauses.path(), &FILES, |paused, _| {
        if paused == FILES[1] {
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o555)).unwrap();
        }
    });
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
    let error = result.expect_err("stopped");
    assert_eq!((error.code, error.path.as_str()), ("io-error", FILES[1]));
    assert_eq!(error.replaced, [FILES[0]]);
    assert_eq!(project.read(FILES[1]).as_deref(), Some(FILES[1]));
    assert_eq!(project.read(FILES[2]).as_deref(), Some(FILES[2]));
    // The temporary file stays, as after a kill: the directory refused its removal too, and the
    // message names it.
    let left: Vec<String> = changed(&before, &project.tree());
    assert_eq!(left.len(), 2, "{left:?}");
    assert_eq!(left[0], FILES[0]);
    assert!(left[1].starts_with("two/.mara-tmp-"), "{left:?}");
    assert_eq!(error.temporary.as_ref(), Some(&left[1]));
    assert!(error.message.contains(&left[1]), "{}", error.message);

    // Review round 1, finding 1: when Mara cannot confirm that it removed the temporary file of
    // a refused first file, the message does not say that nothing was written. Its removal
    // fails, its directory is gone, or its directory became a symbolic link, here to a
    // directory that holds a file of the same name (ADR-WRITE-PATHS-AS-USED).
    let outside = tempfile::Builder::new()
        .prefix("mara-outside-")
        .tempdir()
        .unwrap();
    for how in ["removal refused", "directory gone", "directory linked"] {
        let project = Project::new();
        let directory = project.root.join("one");
        let mut temporary = None;
        let (result, _) = project.run(pauses.path(), &FILES[..1], |paused, _| {
            if paused != FILES[0] {
                return;
            }
            let name = fs::read_dir(&directory)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .find(|name| name.starts_with(".mara-tmp-"))
                .expect("the temporary file is written before the pause");
            match how {
                "removal refused" => {
                    other(&project, FILES[0]);
                    fs::set_permissions(&directory, fs::Permissions::from_mode(0o555)).unwrap();
                }
                "directory gone" => fs::rename(&directory, project.root.join("moved")).unwrap(),
                _ => {
                    fs::rename(&directory, project.root.join("moved")).unwrap();
                    fs::write(outside.path().join(&name), "unrelated").unwrap();
                    std::os::unix::fs::symlink(outside.path(), &directory).unwrap();
                }
            }
            temporary = Some(format!("one/{name}"));
        });
        if how == "removal refused" {
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let error = result.expect_err("refused");
        assert!(error.replaced.is_empty(), "{how}: {error:?}");
        assert_eq!(error.temporary, temporary, "{how}: {error:?}");
        assert!(
            !error.message.contains("nothing was written"),
            "{how}: {}",
            error.message
        );
        assert!(
            error.message.contains("no file was replaced"),
            "{how}: {}",
            error.message
        );
        assert!(
            error.message.contains(&format!(
                "could not confirm that it removed its temporary file {}",
                temporary.as_deref().unwrap()
            )),
            "{how}: {}",
            error.message
        );
    }
}
