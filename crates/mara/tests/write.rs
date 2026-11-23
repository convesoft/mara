//! The write procedure below the CLI, on disposable projects in the temporary directory
//! (DES-WRITE-PROCEDURE in `docs/authoring/writes.mara.md`): the lock, the checks before writing, the
//! temporary files and renames, and the work the checks count. The pause points have their own
//! test binary, `write_pauses`, since they act on the whole process.

#![cfg(unix)]

#[path = "support/stat.rs"]
mod stat;

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use mara::inputs::Inputs;
use mara::write::{self, Basis, FileWrite, WriteError, WriteLock};
use mara_engine::document::is_mid;
use mara_engine::snapshot::ParsedFile;
use mara_engine::validation;
use mara_engine::work::WorkCounts;
use mara_engine::write::{
    ReadBack, cell, insert, item, label, line_ending, link, remove, row, set_value,
};
use stat::visibly;

const CONFIG: &str = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\nformat_version = 1\ninclude = [\"docs/**/*.mara.md\"]\n";

const SCHEMA: &str = "format_version = 1\nstatuses = [\"draft\"]\n\n[types.requirement]\nid_prefix = \"REQ-\"\ndescription = \"A requirement.\"\n\n[relations.refines]\ninverse = \"refined_by\"\ndescription = \"Refines.\"\ntargets = { requirement = [\"requirement\"] }\n";

/// A project in its own temporary directory, removed on drop.
struct Project {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Project {
    fn new() -> Project {
        let dir = tempfile::Builder::new()
            .prefix("mara-write-")
            .tempdir()
            .expect("create a temporary directory");
        let root = fs::canonicalize(dir.path()).unwrap();
        let project = Project { _dir: dir, root };
        project.write(".mara/config.toml", CONFIG);
        project.write(".mara/schema.toml", SCHEMA);
        project
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).unwrap()
    }

    fn mode(&self, relative: &str) -> u32 {
        fs::metadata(self.path(relative))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777
    }

    fn set_mode(&self, relative: &str, mode: u32) {
        fs::set_permissions(self.path(relative), fs::Permissions::from_mode(mode)).unwrap();
    }

    /// Every entry below the root: a directory as `None`, a file with its bytes and mtime.
    fn tree(&self) -> BTreeMap<String, Option<(Vec<u8>, SystemTime)>> {
        fn walk(
            root: &Path,
            directory: &Path,
            out: &mut BTreeMap<String, Option<(Vec<u8>, SystemTime)>>,
        ) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                let metadata = fs::symlink_metadata(&path).unwrap();
                if metadata.is_dir() {
                    out.insert(relative, None);
                    walk(root, &path, out);
                } else {
                    let bytes = fs::read(&path).unwrap_or_default();
                    out.insert(relative, Some((bytes, metadata.modified().unwrap())));
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(&self.root, &self.root, &mut out);
        out
    }

    /// The inputs as a request at `now` reads them.
    fn inputs(&self, now: SystemTime) -> Inputs {
        let mut inputs = Inputs::new(self.root.clone());
        inputs.snapshot(now, &mut WorkCounts::default());
        inputs
    }
}

/// A request late enough that it settles every file written so far (DES-INPUTS-AND-KEYS).
fn settled() -> SystemTime {
    SystemTime::now() + Duration::from_secs(3)
}

fn document(id: &str) -> String {
    format!(
        "# {id}\n\n---\n\n<a id=\"01M7D8DZ2663WP3A037SR2H{}\"></a>\n\n### {id}\n\n\
         | Type | Requirement |\n| --- | --- |\n| ID | {id} |\n| Status | Draft |\n\nBody.\n",
        &id[id.len() - 3..]
    )
}

fn change(inputs: &Inputs, path: &str, contents: &str) -> FileWrite {
    FileWrite {
        path: path.to_owned(),
        basis: inputs.basis(path).expect("a document of the snapshot"),
        contents: contents.as_bytes().to_vec(),
    }
}

fn create(path: &str, contents: &str) -> FileWrite {
    FileWrite {
        path: path.to_owned(),
        basis: Basis::Absent,
        contents: contents.as_bytes().to_vec(),
    }
}

fn apply(project: &Project, files: &[FileWrite]) -> (Result<(), WriteError>, WorkCounts) {
    let lock = WriteLock::take(&project.root).unwrap();
    let mut work = WorkCounts::default();
    (write::apply(&lock, files, &mut work), work)
}

fn refused(result: Result<(), WriteError>) -> WriteError {
    let error = result.expect_err("the edit is refused");
    assert_eq!(error.replaced, Vec::<String>::new(), "{error:?}");
    error
}

#[test]
fn replaces_each_file_and_keeps_its_mode_and_every_other_file() {
    let project = Project::new();
    project.write("docs/a.mara.md", &document("REQ-AAA"));
    project.write("docs/b/b.mara.md", &document("REQ-BBB"));
    project.write("docs/c.mara.md", &document("REQ-CCC"));
    project.set_mode("docs/a.mara.md", 0o600);
    project.set_mode("docs/b/b.mara.md", 0o644);
    let inputs = project.inputs(settled());
    let before = project.tree();

    let files = [
        change(&inputs, "docs/a.mara.md", "new a\n"),
        change(&inputs, "docs/b/b.mara.md", "new b\r\n"),
        create("docs/new/deeper/d.mara.md", "new d\n"),
    ];
    let (result, work) = apply(&project, &files);
    result.unwrap();

    assert_eq!(project.read("docs/a.mara.md"), "new a\n");
    assert_eq!(project.read("docs/b/b.mara.md"), "new b\r\n");
    assert_eq!(project.read("docs/new/deeper/d.mara.md"), "new d\n");
    assert_eq!(project.mode("docs/a.mara.md"), 0o600);
    assert_eq!(project.mode("docs/b/b.mara.md"), 0o644);
    // Every other file keeps its name, bytes and mtime, and no temporary file is left.
    let after = project.tree();
    let written = [
        "docs/a.mara.md",
        "docs/b/b.mara.md",
        "docs/new",
        "docs/new/deeper",
        "docs/new/deeper/d.mara.md",
    ];
    let changed: Vec<&str> = after
        .keys()
        .filter(|path| before.get(*path) != after.get(*path))
        .map(String::as_str)
        .collect();
    assert_eq!(changed, written);
    assert!(before.keys().all(|path| after.contains_key(path)));
    // Settled files are neither read nor hashed again, and nothing is re-derived.
    assert_eq!(work, WorkCounts::default());
}

#[test]
fn refuses_an_edit_whose_file_changed_and_writes_nothing() {
    let project = Project::new();
    project.write("docs/a.mara.md", &document("REQ-AAA"));
    project.write("docs/b.mara.md", &document("REQ-BBB"));
    let inputs = project.inputs(settled());
    let edit = |inputs: &Inputs| {
        vec![
            change(inputs, "docs/a.mara.md", "new a\n"),
            change(inputs, "docs/b.mara.md", "new b\n"),
            create("docs/c.mara.md", "new c\n"),
        ]
    };
    let files = edit(&inputs);

    // A later file changed: nothing is replaced, the first file included.
    project.write("docs/b.mara.md", "changed by another program\n");
    let before = project.tree();
    let error = refused(apply(&project, &files).0);
    assert_eq!(
        (error.code, error.path.as_str()),
        ("changed-file", "docs/b.mara.md")
    );
    assert!(
        error.message.contains("nothing was written"),
        "{}",
        error.message
    );
    assert_eq!(error.diagnostic().line, 1);
    assert_eq!(project.tree(), before);

    // A file the edit creates appeared, or a file it replaces was removed.
    project.write("docs/c.mara.md", "created by another program\n");
    let before = project.tree();
    let error = refused(apply(&project, &edit(&project.inputs(settled()))).0);
    assert_eq!(
        (error.code, error.path.as_str()),
        ("changed-file", "docs/c.mara.md")
    );
    assert_eq!(project.tree(), before);
    fs::remove_file(project.path("docs/c.mara.md")).unwrap();
    let files = edit(&project.inputs(settled()));
    fs::remove_file(project.path("docs/a.mara.md")).unwrap();
    let before = project.tree();
    let error = refused(apply(&project, &files).0);
    assert_eq!(
        (error.code, error.path.as_str()),
        ("changed-file", "docs/a.mara.md")
    );
    assert_eq!(project.tree(), before);
}

#[test]
fn checks_read_a_file_again_only_when_the_two_second_rule_says_so() {
    let project = Project::new();
    project.write("docs/a.mara.md", &document("REQ-AAA"));

    // Read by a request right after it changed: each check reads and hashes it again.
    let inputs = project.inputs(SystemTime::now());
    let (result, work) = apply(&project, &[change(&inputs, "docs/a.mara.md", "x\n")]);
    result.unwrap();
    let again = WorkCounts {
        file_reads: 2,
        hashes: 2,
        ..WorkCounts::default()
    };
    assert_eq!(work, again);

    // A save of the same bytes changes the stat but not the contents: the edit goes ahead.
    let inputs = project.inputs(settled());
    let files = [change(&inputs, "docs/a.mara.md", "y\n")];
    thread::sleep(Duration::from_millis(20));
    project.write("docs/a.mara.md", "x\n");
    let (result, work) = apply(&project, &files);
    result.unwrap();
    assert_eq!(project.read("docs/a.mara.md"), "y\n");
    assert_eq!(work, again);
}

#[test]
fn reads_a_basis_for_a_file_outside_the_snapshot() {
    let project = Project::new();
    project.write(".mara/report.json", "{}\n");
    let lock = WriteLock::take(&project.root).unwrap();
    let mut work = WorkCounts::default();
    let (contents, basis) = Basis::read(&lock, ".mara/report.json", &mut work).unwrap();
    assert_eq!(contents.as_deref(), Some(&b"{}\n"[..]));
    assert_eq!((work.file_reads, work.hashes), (1, 1));
    let (contents, absent) = Basis::read(&lock, ".mara/none.json", &mut work).unwrap();
    assert!(contents.is_none() && matches!(absent, Basis::Absent));
    let files = [
        FileWrite {
            path: ".mara/report.json".into(),
            basis,
            contents: b"[]\n".to_vec(),
        },
        FileWrite {
            path: ".mara/none.json".into(),
            basis: absent,
            contents: b"new\n".to_vec(),
        },
    ];
    write::apply(&lock, &files, &mut work).unwrap();
    assert_eq!(project.read(".mara/report.json"), "[]\n");
    assert_eq!(project.read(".mara/none.json"), "new\n");
}

#[test]
fn follows_no_symbolic_link() {
    let project = Project::new();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("target.mara.md"), "outside\n").unwrap();
    symlink(
        outside.path().join("target.mara.md"),
        project.path("docs-link.mara.md"),
    )
    .unwrap();
    fs::create_dir_all(project.path("docs")).unwrap();
    symlink(outside.path(), project.path("docs/linked")).unwrap();
    let before = project.tree();

    for (path, link) in [
        ("docs-link.mara.md", "docs-link.mara.md"),
        ("docs/linked/new.mara.md", "docs/linked"),
    ] {
        let error = refused(apply(&project, &[create(path, "new\n")]).0);
        assert_eq!((error.code, error.path.as_str()), ("symbolic-link", path));
        assert!(
            error
                .message
                .starts_with(&format!("{link} is a symbolic link")),
            "{}",
            error.message
        );
    }
    assert_eq!(project.tree(), before);
    let outside_files: Vec<_> = fs::read_dir(outside.path()).unwrap().collect();
    assert_eq!(outside_files.len(), 1);
    assert_eq!(
        fs::read_to_string(outside.path().join("target.mara.md")).unwrap(),
        "outside\n"
    );

    // The lock refuses a `.mara` that is a symbolic link, at line 1 of the configuration.
    let real = project.path("real-mara");
    fs::rename(project.path(".mara"), &real).unwrap();
    symlink(&real, project.path(".mara")).unwrap();
    let error = WriteLock::take(&project.root).unwrap_err();
    assert_eq!(
        (error.code, error.path.as_str()),
        ("symbolic-link", ".mara/config.toml")
    );
}

#[test]
fn a_writer_waits_while_another_holds_the_lock() {
    let project = Project::new();
    // The lock as every Mara writer takes it: an exclusive advisory lock on `.mara`.
    let held = File::open(project.path(".mara")).unwrap();
    held.lock().unwrap();
    let taken = Arc::new(AtomicBool::new(false));
    let waiter = {
        let (root, taken) = (project.root.clone(), Arc::clone(&taken));
        thread::spawn(move || {
            let lock = WriteLock::take(&root).unwrap();
            taken.store(true, Ordering::SeqCst);
            drop(lock);
        })
    };
    thread::sleep(Duration::from_millis(300));
    assert!(
        !taken.load(Ordering::SeqCst),
        "the lock was taken while held"
    );
    held.unlock().unwrap();
    waiter.join().unwrap();
    assert!(taken.load(Ordering::SeqCst));
    // Dropping a lock releases it.
    drop(WriteLock::take(&project.root).unwrap());
    let lock = WriteLock::take(&project.root).unwrap();
    assert!(
        File::open(project.path(".mara"))
            .unwrap()
            .try_lock()
            .is_err()
    );
    drop(lock);
}

/// The writer and the procedure together: an item with a relation, created in a new document,
/// loads in the next snapshot and the project stays valid.
#[test]
fn a_written_item_loads_and_the_project_stays_valid() {
    let project = Project::new();
    project.write("docs/a.mara.md", &document("REQ-AAA"));
    let mut inputs = Inputs::new(project.root.clone());
    let lock = WriteLock::take(&project.root).unwrap();
    let generation = inputs.snapshot(settled(), &mut WorkCounts::default());
    let target = generation.snapshot.item(0).mid.clone();
    let mid = write::new_mid().unwrap();
    assert!(is_mid(&mid) && mid != write::new_mid().unwrap());

    let path = "docs/sub dir/b.mara.md";
    let rows = [
        row("type", &label("requirement")),
        row("id", &cell("REQ-NEW").unwrap()),
        row("status", &label("draft")),
        row("refines", &link("REQ-AAA", &target, path, "docs/a.mara.md")),
    ];
    let contents = "";
    let new = item(
        &mid,
        3,
        "New",
        &rows,
        "Created by the writer.",
        line_ending(contents),
    )
    .unwrap();
    let splice = insert(contents, 0, &new);
    let mut work = WorkCounts::default();
    let written = ReadBack::default()
        .edit(&ParsedFile::default(), &[splice], &mut work)
        .unwrap();
    assert_eq!(work.parses, 1);
    let files = [FileWrite {
        path: path.to_owned(),
        basis: Basis::Absent,
        contents: written.contents.into_vec(),
    }];
    visibly(&project.path("docs"), || {
        write::apply(&lock, &files, &mut WorkCounts::default()).unwrap();
    });
    drop(lock);

    let generation = inputs.snapshot(
        settled() + Duration::from_secs(10),
        &mut WorkCounts::default(),
    );
    assert_eq!(generation.snapshot.items().len(), 2);
    let mut work = WorkCounts::default();
    let findings = validation::check(&generation.snapshot, &mut work);
    let diagnostics: Vec<_> = findings
        .into_iter()
        .map(|finding| finding.diagnostic)
        .collect();
    assert_eq!(diagnostics, []);
}

/// Review round 1, finding 6: writing clears the set-user-ID and set-group-ID bits, so the mode
/// is set after the contents.
#[test]
fn keeps_special_permission_bits() {
    let project = Project::new();
    let modes = [0o6755, 0o4700, 0o2750, 0o1644];
    let paths: Vec<String> = (0..modes.len())
        .map(|n| format!("docs/m{n}.mara.md"))
        .collect();
    for (path, mode) in paths.iter().zip(modes) {
        project.write(path, &document(&format!("REQ-M{mode:03o}")));
        project.set_mode(path, mode);
        assert_eq!(project.mode(path), mode, "the test sets {mode:o}");
    }
    let inputs = project.inputs(settled());
    let files: Vec<FileWrite> = paths
        .iter()
        .map(|path| change(&inputs, path, "new\n"))
        .collect();
    apply(&project, &files).0.unwrap();
    for (path, mode) in paths.iter().zip(modes) {
        assert_eq!(project.read(path), "new\n");
        assert_eq!(project.mode(path), mode, "{path}");
    }
}

/// Review round 1, finding 8: a temporary file's name does not grow with the target's, so a
/// file whose name is as long as names go is written.
#[test]
fn writes_a_file_whose_name_is_as_long_as_names_go() {
    let project = Project::new();
    let name = format!("{}.mara.md", "n".repeat(255 - ".mara.md".len()));
    let path = format!("docs/{name}");
    project.write(&path, &document("REQ-LONG"));
    let inputs = project.inputs(settled());
    let created = format!("docs/new-{}", &name[4..]);
    apply(
        &project,
        &[
            change(&inputs, &path, "new\n"),
            create(&created, "created\n"),
        ],
    )
    .0
    .unwrap();
    assert_eq!(project.read(&path), "new\n");
    assert_eq!(project.read(&created), "created\n");
}

/// The cold write row of VER-WORK-COUNTERS below the CLI: a write whose files last changed at
/// least 2 s before it counts what a cold request counts plus 1 parse per distinct content
/// written, made by the read-back before the renames, and nothing after them.
#[test]
fn a_cold_write_counts_one_more_parse_per_distinct_content_written() {
    let project = Project::new();
    let page = |id: &str| {
        format!(
            "# Shared page\n\n---\n\n{}",
            &document(id)[format!("# {id}\n\n---\n\n").len()..]
        )
    };
    for (path, id) in [
        ("docs/a.mara.md", "REQ-AAA"),
        ("docs/b.mara.md", "REQ-BBB"),
        ("docs/c.mara.md", "REQ-CCC"),
    ] {
        project.write(path, &page(id));
    }
    let mut work = WorkCounts::default();
    let lock = WriteLock::take(&project.root).unwrap();
    let mut inputs = Inputs::new(project.root.clone());
    let generation = inputs.snapshot(settled(), &mut work);
    let cold = work;
    assert_eq!(
        (cold.file_reads, cold.parses, cold.snapshot_builds),
        (5, 3, 1)
    );

    // `a` and `b` lose their item and become the same narrative; `c` gets a new ID.
    let mut read_back = ReadBack::default();
    let mut files = Vec::new();
    for document in generation.snapshot.documents() {
        let old = &document.file;
        let contents = std::str::from_utf8(&old.contents).unwrap();
        let item = &old.document.items[0];
        let splice = match document.path.as_str() {
            "docs/c.mara.md" => set_value(&item.rows[1], "REQ-RENAMED"),
            _ => remove(contents, item.span.clone()),
        };
        let new = read_back.edit(old, &[splice], &mut work).unwrap();
        files.push(FileWrite {
            path: document.path.clone(),
            basis: inputs.basis(&document.path).unwrap(),
            contents: new.contents.into_vec(),
        });
    }
    let before_renames = work;
    write::apply(&lock, &files, &mut work).unwrap();
    assert_eq!(
        project.read("docs/a.mara.md"),
        project.read("docs/b.mara.md")
    );
    assert_eq!(work, before_renames, "nothing after the read-back");
    let expected = WorkCounts {
        parses: cold.parses + 2,
        ..cold
    };
    assert_eq!(work, expected);
}
