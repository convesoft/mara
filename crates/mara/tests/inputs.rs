//! The input inventory and the snapshot reused between requests, on disposable projects in the
//! temporary directory (DES-INPUTS-AND-KEYS and DES-SNAPSHOT in `docs/engine.mara.md`,
//! REQ-WORK-FOLLOWS-CHANGE in `docs/performance.mara.md`). Each request is a call with its own
//! `now`; later requests pass a later `now`, as time passing would.

#[path = "support/stat.rs"]
mod stat;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use mara::inputs::{Generation, Inputs};
use mara_engine::work::WorkCounts;
use stat::visibly;

const CONFIG: &str = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\nformat_version = 1\ninclude = [\"**/*.mara.md\"]\n";

const SCHEMA: &str = "format_version = 1\nstatuses = [\"draft\"]\n\n[types.requirement]\nid_prefix = \"REQ-\"\ndescription = \"A requirement.\"\n";

const RELATION: &str = "\n[relations.satisfies]\ninverse = \"satisfied_by\"\ndescription = \"Satisfies.\"\ntargets = { requirement = [\"requirement\"] }\n";

/// A project in its own temporary directory, removed on drop.
struct Project {
    dir: tempfile::TempDir,
    root: PathBuf,
}

impl Project {
    /// A project with the configuration and schema above and nothing else.
    fn new() -> Project {
        let dir = tempfile::Builder::new()
            .prefix("mara-inputs-")
            .tempdir()
            .expect("create a temporary directory");
        let root = fs::canonicalize(dir.path()).unwrap();
        let project = Project { dir, root };
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
        fs::write(&path, contents).unwrap();
    }

    fn remove(&self, relative: &str) {
        fs::remove_file(self.path(relative)).unwrap();
    }

    fn rename(&self, from: &str, to: &str) {
        fs::rename(self.path(from), self.path(to)).unwrap();
    }

    #[cfg(unix)]
    fn link(&self, target: &Path, relative: &str) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, path).unwrap();
    }
}

/// A document holding one item with a unique MID and ID made from `n`, and `extra` after its
/// body.
fn document(n: u32, extra: &str) -> String {
    format!(
        "# Page {n}\n\n---\n\n<a id=\"{}\"></a>\n\n### Item {n}\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | REQ-ITEM-{n} |\n| Status | Draft |\n\nBody {n}.\n{extra}",
        mid(n)
    )
}

fn mid(n: u32) -> String {
    format!("01M7D8DZ2663WP3A037SR{n:05}")
}

/// Time passing between requests: each starts at least 3 s after the last one and after every
/// file written before it, longer than the 2 s after which a check settles a file.
struct Clock(SystemTime);

impl Clock {
    fn new() -> Clock {
        Clock(SystemTime::UNIX_EPOCH)
    }

    fn next(&mut self) -> SystemTime {
        let gap = Duration::from_secs(3);
        let now = self.0.max(SystemTime::now() + gap);
        self.0 = now + gap;
        now
    }
}

/// One request: the snapshot of `inputs` at `now`, with the work it counted.
fn request(inputs: &mut Inputs, now: SystemTime) -> (Generation, WorkCounts) {
    let mut work = WorkCounts::default();
    let generation = inputs.snapshot(now, &mut work);
    (generation, work)
}

fn counts(file_reads: u64, hashes: u64, parses: u64, snapshot_builds: u64) -> WorkCounts {
    WorkCounts {
        file_reads,
        hashes,
        parses,
        snapshot_builds,
        ..WorkCounts::default()
    }
}

fn paths(generation: &Generation) -> Vec<&str> {
    let documents = generation.snapshot.documents();
    documents
        .iter()
        .map(|document| document.path.as_str())
        .collect()
}

fn printed(generation: &Generation) -> Vec<String> {
    generation
        .diagnostics
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// A project with two `.gitignore` files, a `.git` directory, and documents that are and are not
/// inputs.
fn mixed() -> Project {
    let project = Project::new();
    project.write(
        ".gitignore",
        "/target/\n*.draft.mara.md\n!keep.draft.mara.md\n",
    );
    project.write("docs/sub/.gitignore", "local.mara.md\n");
    project.write("docs/a.mara.md", &document(1, ""));
    project.write("docs/sub/b.mara.md", &document(2, ""));
    project.write("docs/keep.draft.mara.md", &document(3, ""));
    project.write("docs/local.mara.md", &document(4, ""));
    // Not inputs: ignored, not `*.mara.md`, inside `.git`.
    project.write("docs/x.draft.mara.md", &document(5, ""));
    project.write("docs/sub/local.mara.md", &document(6, ""));
    project.write("target/out.mara.md", &document(7, ""));
    project.write("docs/readme.md", &document(8, ""));
    project.write("README.md", "# Readme\n");
    project.write(".git/HEAD", "ref: refs/heads/main\n");
    project.write(".git/hooks/hook.mara.md", &document(9, ""));
    // Git's own exclude file is never read, so what it ignores stays an input.
    project.write(".git/info/exclude", "a.mara.md\n");
    project
}

const MIXED_DOCUMENTS: [&str; 4] = [
    "docs/a.mara.md",
    "docs/keep.draft.mara.md",
    "docs/local.mara.md",
    "docs/sub/b.mara.md",
];

/// Configuration, schema, two `.gitignore` files and four documents.
const MIXED_INPUTS: u64 = 8;

#[test]
fn a_cold_request_reads_and_hashes_each_input_once() {
    let project = mixed();
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, work) = request(&mut inputs, Clock::new().next());
    assert_eq!(paths(&generation), MIXED_DOCUMENTS);
    assert_eq!(printed(&generation), [] as [String; 0]);
    assert_eq!(generation.project.diagnostics, []);
    assert_eq!(work, counts(MIXED_INPUTS, MIXED_INPUTS, 4, 1));
    let snapshot = &generation.snapshot;
    assert_eq!(snapshot.items().len(), 4);
    assert_eq!(snapshot.items_with_id("REQ-ITEM-2").len(), 1);
}

#[test]
fn an_unchanged_project_is_neither_read_nor_rebuilt() {
    let project = mixed();
    let mut clock = Clock::new();
    let mut inputs = Inputs::new(project.root.clone());
    let (first, _) = request(&mut inputs, clock.next());
    let (second, work) = request(&mut inputs, clock.next());
    assert_eq!(work, WorkCounts::default());
    assert!(Arc::ptr_eq(&first.snapshot, &second.snapshot));
}

#[test]
fn an_edited_document_alone_is_read_hashed_and_parsed_again() {
    let project = mixed();
    let mut clock = Clock::new();
    let mut inputs = Inputs::new(project.root.clone());
    let (first, _) = request(&mut inputs, clock.next());
    project.write("docs/sub/b.mara.md", &document(2, "More text.\n"));
    let (second, work) = request(&mut inputs, clock.next());
    assert_eq!(work, counts(1, 1, 1, 1));
    assert_eq!(paths(&second), MIXED_DOCUMENTS);
    assert_ne!(first.snapshot.key(), second.snapshot.key());
    // The other documents' parses are shared, not copied.
    let file = |generation: &Generation, path: &str| {
        let snapshot = &generation.snapshot;
        Arc::clone(&snapshot.documents()[snapshot.document(path).unwrap()].file)
    };
    assert!(Arc::ptr_eq(
        &file(&first, "docs/a.mara.md"),
        &file(&second, "docs/a.mara.md")
    ));
    let (_, work) = request(&mut inputs, clock.next());
    assert_eq!(work, WorkCounts::default());
}

#[test]
fn undeclared_ignored_and_git_files_cause_no_work() {
    let project = mixed();
    let mut clock = Clock::new();
    let mut inputs = Inputs::new(project.root.clone());
    let (first, _) = request(&mut inputs, clock.next());
    project.write("README.md", "# Changed\n");
    project.write("notes/new.md", "New.\n");
    project.write("docs/x.draft.mara.md", &document(5, "Changed.\n"));
    project.write("docs/sub/local.mara.md", &document(6, "Changed.\n"));
    project.write("target/more.mara.md", &document(10, ""));
    project.write(".git/objects/ab/cdef", "object");
    project.write(".git/hooks/hook.mara.md", &document(9, "Changed.\n"));
    project.write("docs/readme.md", "Changed.\n");
    let (second, work) = request(&mut inputs, clock.next());
    assert_eq!(work, WorkCounts::default());
    assert!(Arc::ptr_eq(&first.snapshot, &second.snapshot));
}

/// The review's reproduction of the 2 s rule (DES-INPUTS-AND-KEYS, the warm rows of
/// VER-WORK-COUNTERS), with the clock injected instead of waited for: a request right after
/// the files were written hashes them within 2 s of their change, so the next one, 2.1 s
/// later, hashes each once more but parses and builds nothing; a third hashes nothing.
#[test]
fn an_input_hashed_within_2_s_of_its_change_is_hashed_once_more() {
    let project = mixed();
    let mut inputs = Inputs::new(project.root.clone());
    let now = SystemTime::now();
    let (first, work) = request(&mut inputs, now);
    assert_eq!(work, counts(MIXED_INPUTS, MIXED_INPUTS, 4, 1));
    let (second, work) = request(&mut inputs, now + Duration::from_millis(2100));
    assert_eq!(work, counts(MIXED_INPUTS, MIXED_INPUTS, 0, 0));
    assert!(Arc::ptr_eq(&first.snapshot, &second.snapshot));
    let (_, work) = request(&mut inputs, now + Duration::from_millis(4200));
    assert_eq!(work, WorkCounts::default());
}

/// A check that began before the files' last change settles nothing either.
#[test]
fn a_check_that_began_before_a_change_is_repeated_once() {
    let now = SystemTime::now();
    let project = mixed();
    let mut inputs = Inputs::new(project.root.clone());
    request(&mut inputs, now);
    let (_, work) = request(&mut inputs, now + Duration::from_secs(10));
    assert_eq!(work, counts(MIXED_INPUTS, MIXED_INPUTS, 0, 0));
    let (_, work) = request(&mut inputs, now + Duration::from_secs(20));
    assert_eq!(work, WorkCounts::default());
}

/// The cold row of VER-WORK-COUNTERS counts parses per distinct document content: byte-identical
/// documents are read and hashed once each, parsed once, and stay two documents with their own
/// paths, diagnostics and items.
#[test]
fn identical_documents_are_parsed_once_and_kept_per_path() {
    let project = Project::new();
    // An item without a body, so that each document has a diagnostic.
    let text = document(1, "").replace("\nBody 1.\n", "\n");
    project.write("a.mara.md", &text);
    project.write("b.mara.md", &text);
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, work) = request(&mut inputs, Clock::new().next());
    assert_eq!(work, counts(4, 4, 1, 1));
    let snapshot = &generation.snapshot;
    assert_eq!(paths(&generation), ["a.mara.md", "b.mara.md"]);
    let diagnostics: Vec<String> = snapshot
        .documents()
        .iter()
        .flat_map(|document| document.file.document.diagnostics(&document.path))
        .map(|diagnostic| {
            format!(
                "{}:{}:{}",
                diagnostic.path, diagnostic.line, diagnostic.code
            )
        })
        .collect();
    assert_eq!(
        diagnostics,
        ["a.mara.md:5:empty-body", "b.mara.md:5:empty-body"]
    );
    let items = snapshot.items_with_mid(&mid(1));
    assert_eq!(items, [0, 1]);
    let documents: Vec<usize> = items
        .iter()
        .map(|&item| snapshot.items()[item].document)
        .collect();
    assert_eq!(documents, [0, 1]);
    for document in snapshot.documents() {
        let item = &document.file.document.items[0];
        assert!(document.file.contents[item.span.clone()].starts_with(b"<a id="));
    }
}

#[test]
fn touched_files_are_hashed_but_nothing_is_derived_again() {
    let project = mixed();
    let mut clock = Clock::new();
    let mut inputs = Inputs::new(project.root.clone());
    let (first, _) = request(&mut inputs, clock.next());
    for path in ["docs/a.mara.md", ".mara/schema.toml"] {
        let file = fs::File::options()
            .write(true)
            .open(project.path(path))
            .unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(60))
            .unwrap();
    }
    let (second, work) = request(&mut inputs, clock.next());
    assert_eq!(work, counts(2, 2, 0, 0));
    let schema = |generation: &Generation| Arc::clone(generation.project.schema.as_ref().unwrap());
    assert!(Arc::ptr_eq(&schema(&first), &schema(&second)));
    assert!(Arc::ptr_eq(&first.snapshot, &second.snapshot));
}

#[test]
fn a_moved_document_is_read_but_not_parsed_again() {
    let project = mixed();
    let mut clock = Clock::new();
    let mut inputs = Inputs::new(project.root.clone());
    request(&mut inputs, clock.next());
    visibly(&project.path("docs"), || {
        visibly(&project.path("docs/sub"), || {
            project.rename("docs/a.mara.md", "docs/sub/moved.mara.md");
        });
    });
    let (generation, work) = request(&mut inputs, clock.next());
    assert_eq!(work, counts(1, 1, 0, 1));
    assert_eq!(
        paths(&generation),
        [
            "docs/keep.draft.mara.md",
            "docs/local.mara.md",
            "docs/sub/b.mara.md",
            "docs/sub/moved.mara.md"
        ]
    );
}

#[test]
fn an_added_document_is_parsed_and_a_removed_one_dropped() {
    let project = mixed();
    let mut clock = Clock::new();
    let mut inputs = Inputs::new(project.root.clone());
    request(&mut inputs, clock.next());
    visibly(&project.path("docs"), || {
        project.write("docs/new/c.mara.md", &document(11, ""));
    });
    let (generation, work) = request(&mut inputs, clock.next());
    assert_eq!(work, counts(1, 1, 1, 1));
    assert_eq!(generation.snapshot.items_with_mid(&mid(11)).len(), 1);
    visibly(&project.path("docs"), || {
        project.remove("docs/local.mara.md")
    });
    let (generation, work) = request(&mut inputs, clock.next());
    assert_eq!(work, counts(0, 0, 0, 1));
    assert_eq!(generation.snapshot.items_with_mid(&mid(4)).len(), 0);
    assert_eq!(generation.snapshot.items().len(), 4);
}

/// The schema is applied after parsing, so a schema edit re-parses no document
/// (DES-INPUTS-AND-KEYS).
#[test]
fn a_schema_edit_rebuilds_the_snapshot_without_parsing() {
    let project = Project::new();
    let link = format!("| Satisfies | [REQ-ITEM-2](b.mara.md#{}) |\n", mid(2));
    let linking = document(1, "").replace(
        "| Status | Draft |\n",
        &format!("| Status | Draft |\n{link}"),
    );
    project.write("a.mara.md", &linking);
    project.write("b.mara.md", &document(2, ""));
    let mut clock = Clock::new();
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, _) = request(&mut inputs, clock.next());
    assert_eq!(generation.snapshot.incoming(&mid(2)).count(), 0);
    project.write(".mara/schema.toml", &format!("{SCHEMA}{RELATION}"));
    let (generation, work) = request(&mut inputs, clock.next());
    assert_eq!(work, counts(1, 1, 0, 1));
    let edges: Vec<_> = generation.snapshot.incoming(&mid(2)).collect();
    assert_eq!(edges.len(), 1);
    assert_eq!(generation.snapshot.relation(edges[0]), Some("satisfies"));
}

#[test]
fn a_gitignore_edit_changes_the_inputs_without_parsing() {
    let project = mixed();
    let mut clock = Clock::new();
    let mut inputs = Inputs::new(project.root.clone());
    request(&mut inputs, clock.next());
    project.write(
        ".gitignore",
        "/target/\n*.draft.mara.md\n!keep.draft.mara.md\n/docs/local.mara.md\n",
    );
    let (generation, work) = request(&mut inputs, clock.next());
    assert_eq!(work, counts(1, 1, 0, 1));
    assert_eq!(
        paths(&generation),
        [
            "docs/a.mara.md",
            "docs/keep.draft.mara.md",
            "docs/sub/b.mara.md"
        ]
    );
}

#[test]
fn only_directories_that_can_hold_a_match_are_listed() {
    let project = Project::new();
    project.write(
        ".mara/config.toml",
        &CONFIG.replace("**/*.mara.md", "docs/*.mara.md"),
    );
    project.write("docs/a.mara.md", &document(1, ""));
    project.write("docs/sub/b.mara.md", &document(2, ""));
    project.write("src/c.mara.md", &document(3, ""));
    // A `.gitignore` where no document can be is no input.
    project.write("src/.gitignore", "*\n");
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, work) = request(&mut inputs, Clock::new().next());
    assert_eq!(paths(&generation), ["docs/a.mara.md"]);
    assert_eq!(work, counts(3, 3, 1, 1));
}

/// VER-NESTED-PROJECTS below the CLI: the outer project's pattern matches the inner projects'
/// documents, and none of their files is read.
#[test]
fn another_project_below_the_root_is_never_an_input() {
    let outer = Project::new();
    outer.write("docs/a.mara.md", &document(1, ""));
    outer.write("inner/.mara/config.toml", CONFIG);
    outer.write("inner/.mara/schema.toml", SCHEMA);
    outer.write("inner/.gitignore", "nothing\n");
    outer.write("inner/docs/b.mara.md", &document(2, ""));
    outer.write("old/.mara/project.toml", "format_version = 3\n");
    outer.write("old/docs/c.mara.md", &document(3, ""));
    // A `config.toml` outside a `.mara/` directory makes no project.
    outer.write("stray/config.toml", CONFIG);
    outer.write("stray/d.mara.md", &document(4, ""));
    let mut clock = Clock::new();
    let mut inputs = Inputs::new(outer.root.clone());
    let (generation, work) = request(&mut inputs, clock.next());
    assert_eq!(paths(&generation), ["docs/a.mara.md", "stray/d.mara.md"]);
    assert_eq!(printed(&generation), [] as [String; 0]);
    assert_eq!(work, counts(4, 4, 2, 1));

    let mut inputs = Inputs::new(outer.path("inner"));
    let (generation, work) = request(&mut inputs, clock.next());
    assert_eq!(paths(&generation), ["docs/b.mara.md"]);
    assert_eq!(work, counts(4, 4, 1, 1));
}

#[cfg(unix)]
#[test]
fn symbolic_links_are_never_followed() {
    let elsewhere = Project::new();
    elsewhere.write("docs/x.mara.md", &document(20, ""));
    let project = Project::new();
    project.write("docs/a.mara.md", &document(1, ""));
    project.write("inner/.mara/config.toml", CONFIG);
    project.write("inner/docs/b.mara.md", &document(2, ""));
    // A document that is a link: into the project, into the nested project, out of the root.
    project.link(&project.path("docs/a.mara.md"), "docs/same.mara.md");
    project.link(&project.path("inner/docs/b.mara.md"), "docs/inner.mara.md");
    project.link(&elsewhere.path("docs/x.mara.md"), "docs/outside.mara.md");
    // Directories that are links, into the nested project and out of the root.
    project.link(&project.path("inner/docs"), "docs/inner-docs");
    project.link(&elsewhere.path("docs"), "docs/outside-docs");
    // A link no pattern matches as a document is no input.
    project.link(&project.path("docs/a.mara.md"), "docs/alias.md");
    // A `.gitignore` that is a link, which keeps its whole directory out, and a directory whose
    // `.mara` is one.
    project.write("rules", "nothing\n");
    project.write("linked-rules/e.mara.md", &document(5, ""));
    project.link(&project.path("rules"), "linked-rules/.gitignore");
    project.write("sub/c.mara.md", &document(3, ""));
    project.link(&project.path("inner/.mara"), "sub/.mara");

    let mut inputs = Inputs::new(project.root.clone());
    let (generation, work) = request(&mut inputs, Clock::new().next());
    assert_eq!(paths(&generation), ["docs/a.mara.md"]);
    assert_eq!(work, counts(3, 3, 1, 1), "nothing behind a link is read");
    let found: Vec<(&str, &str)> = generation
        .diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.path.as_str(), &*diagnostic.code))
        .collect();
    assert_eq!(
        found,
        [
            ("docs/inner.mara.md", "symbolic-link"),
            ("docs/outside.mara.md", "symbolic-link"),
            ("docs/same.mara.md", "symbolic-link"),
            ("linked-rules/.gitignore", "symbolic-link"),
            ("sub/.mara", "symbolic-link"),
        ]
    );
    assert!(
        printed(&generation)[3].ends_with(
            "so Mara cannot apply its patterns and reads nothing in linked-rules or below: put \
             the file itself there"
        ),
        "{:?}",
        printed(&generation)
    );
    assert!(
        printed(&generation)[4].starts_with(
            "sub/.mara:1: error[symbolic-link]: sub/.mara is a symbolic link, so Mara cannot tell"
        ),
        "{:?}",
        printed(&generation)
    );
}

#[cfg(unix)]
#[test]
fn inputs_that_cannot_be_read_are_reported() {
    use std::os::unix::fs::PermissionsExt;
    let project = Project::new();
    project.write("docs/a.mara.md", &document(1, ""));
    project.write("docs/locked.mara.md", &document(2, ""));
    project.write("docs/closed/b.mara.md", &document(3, ""));
    let mode = |path: &str, mode| {
        fs::set_permissions(project.path(path), fs::Permissions::from_mode(mode)).unwrap();
    };
    mode("docs/locked.mara.md", 0o000);
    mode("docs/closed", 0o000);
    let readable = fs::read(project.path("docs/locked.mara.md")).is_ok();
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, work) = request(&mut inputs, Clock::new().next());
    mode("docs/locked.mara.md", 0o644);
    mode("docs/closed", 0o755);
    if readable {
        // Permissions do not apply, as for root.
        return;
    }
    assert_eq!(paths(&generation), ["docs/a.mara.md"]);
    assert_eq!(work, counts(3, 3, 1, 1), "a failed read is no file read");
    let found: Vec<(&str, &str)> = generation
        .diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.path.as_str(), &*diagnostic.code))
        .collect();
    assert_eq!(
        found,
        [
            ("docs/closed", "io-error"),
            ("docs/locked.mara.md", "io-error")
        ]
    );
    assert!(
        printed(&generation)[0]
            .contains("cannot tell whether docs/closed is another project's root"),
        "{:?}",
        printed(&generation)
    );
}

#[cfg(unix)]
#[test]
fn a_document_name_that_is_not_utf8_is_reported() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let project = Project::new();
    project.write("docs/a.mara.md", &document(1, ""));
    let name = OsStr::from_bytes(b"\xff.mara.md");
    if fs::write(project.path("docs").join(name), document(2, "")).is_err() {
        // The filesystem accepts only UTF-8 names, as on macOS.
        return;
    }
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, work) = request(&mut inputs, Clock::new().next());
    assert_eq!(paths(&generation), ["docs/a.mara.md"]);
    assert_eq!(work, counts(3, 3, 1, 1));
    assert_eq!(
        printed(&generation),
        [
            "docs/\u{fffd}.mara.md:1: error[io-error]: the name of docs/\u{fffd}.mara.md is not UTF-8, so Mara cannot read it: rename it"
        ]
    );
}

/// Validation reports on a project whose configuration or schema has errors, so the snapshot
/// is built; the other operations refuse such a project (DES-PROJECT-CONFIGURATION).
#[test]
fn a_project_with_configuration_errors_still_gets_a_snapshot() {
    let project = Project::new();
    project.write(".mara/config.toml", &format!("{CONFIG}unknown = 1\n"));
    project.write("a.mara.md", &document(1, ""));
    let mut clock = Clock::new();
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, work) = request(&mut inputs, clock.next());
    assert!(generation.project.has_errors());
    assert_eq!(paths(&generation), ["a.mara.md"]);
    assert_eq!(work, counts(3, 3, 1, 1));

    // Without include patterns, nothing is listed and the snapshot is empty.
    project.write(
        ".mara/config.toml",
        "format_version = 1\nschema = \".mara/schema.toml\"\n",
    );
    let (generation, work) = request(&mut inputs, clock.next());
    assert!(generation.project.has_errors());
    assert_eq!(paths(&generation), [] as [&str; 0]);
    assert_eq!(work, counts(1, 1, 0, 1));
}

#[test]
fn loading_twice_in_one_request_reads_once() {
    let project = mixed();
    let mut inputs = Inputs::new(project.root.clone());
    let now = SystemTime::now();
    let mut work = WorkCounts::default();
    let loaded = inputs.project(now, &mut work);
    assert_eq!(work, counts(2, 2, 0, 0));
    let generation = inputs.snapshot(now, &mut work);
    assert!(Arc::ptr_eq(&loaded, &generation.project));
    assert_eq!(work, counts(MIXED_INPUTS, MIXED_INPUTS, 4, 1));
}

/// The inventory filters by `.gitignore` files as Git does, and reads neither Git's own exclude
/// file nor its global one: Git, given none of either, lists the same documents.
#[test]
fn gitignore_files_filter_as_in_git() {
    let project = Project::new();
    project.write(
        ".gitignore",
        "# a comment\n/build/\n*.tmp.mara.md\n!keep.tmp.mara.md\ndocs/private/\n**/secret*.mara.md\n\\#hash.mara.md\ntrailing.mara.md   \n",
    );
    project.write(
        "docs/.gitignore",
        "/local.mara.md\nsub/*.mara.md\n!sub/kept.mara.md\n",
    );
    project.write("docs/deep/.gitignore", "!*.tmp.mara.md\n");
    for path in [
        "plain.mara.md",
        "build/a.mara.md",
        "src/build/b.mara.md",
        "a.tmp.mara.md",
        "keep.tmp.mara.md",
        "docs/private/p.mara.md",
        "docs/secret1.mara.md",
        "docs/x/secret2.mara.md",
        "#hash.mara.md",
        "trailing.mara.md",
        "docs/local.mara.md",
        "docs/x/local.mara.md",
        "docs/sub/s.mara.md",
        "docs/sub/kept.mara.md",
        "docs/sub/deeper/t.mara.md",
        "docs/deep/d.tmp.mara.md",
        "docs/ok.mara.md",
    ] {
        project.write(path, "# Narrative\n");
    }
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, _) = request(&mut inputs, Clock::new().next());
    let expected = git_documents(&project);
    assert_eq!(paths(&generation), expected);
    assert_eq!(
        expected,
        [
            "docs/deep/d.tmp.mara.md",
            "docs/ok.mara.md",
            "docs/sub/deeper/t.mara.md",
            "docs/sub/kept.mara.md",
            "docs/x/local.mara.md",
            "keep.tmp.mara.md",
            "plain.mara.md",
            "src/build/b.mara.md",
        ]
    );
}

/// The `*.mara.md` files that Git, in a new repository at the project root with no global or
/// system configuration, lists as untracked and not ignored, in byte order.
fn git_documents(project: &Project) -> Vec<String> {
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&project.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", project.dir.path())
            .env("XDG_CONFIG_HOME", project.dir.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("run git: the test compares the inventory with Git");
        assert!(output.status.success(), "git {args:?}: {output:?}");
        output.stdout
    };
    git(&["init", "--quiet"]);
    let listed = git(&["ls-files", "-z", "--others", "--exclude-standard"]);
    let mut documents: Vec<String> = listed
        .split(|&byte| byte == 0)
        .map(|path| String::from_utf8(path.to_vec()).expect("UTF-8 test names"))
        .filter(|path| path.ends_with(".mara.md"))
        .collect();
    documents.sort_unstable();
    documents
}

/// Escapes, trailing spaces, negation, classes, `?` and `**` in every place, CRLF and a byte
/// order mark filter as in Git.
#[test]
fn gitignore_escapes_and_double_stars_filter_as_in_git() {
    let project = Project::new();
    let mut root =
        b"\xEF\xBB\xBFspace.mara.md  \r\nesc\\ .mara.md\\ \r\nfoo**/bar.mara.md\n".to_vec();
    root.extend(
        b"[!x]y.mara.md\n*[]a-]z.mara.md\n\\!bang.mara.md\n\\#hash.mara.md\nd?/*.mara.md\n",
    );
    root.extend(b"s/**\n**/t/u*.mara.md\nv/**/w.mara.md\n**x**.mara.md\nm**n/o.mara.md\n/r*/\n");
    fs::write(project.path(".gitignore"), root).unwrap();
    for path in [
        "space.mara.md",
        "esc .mara.md ",
        "esc .mara.md",
        "foo/bar.mara.md",
        "foo/x/bar.mara.md",
        "foox/bar.mara.md",
        "foox/y/bar.mara.md",
        "zy.mara.md",
        "xy.mara.md",
        "-z.mara.md",
        "]z.mara.md",
        "bz.mara.md",
        "!bang.mara.md",
        "bang.mara.md",
        "#hash.mara.md",
        "db/c.mara.md",
        "d/c.mara.md",
        "dbb/c.mara.md",
        "s/in.mara.md",
        "s/deep/in.mara.md",
        "t/u1.mara.md",
        "k/t/u2.mara.md",
        "k/t/x/u3.mara.md",
        "v/w.mara.md",
        "v/a/b/w.mara.md",
        "vv/w.mara.md",
        "axb.mara.md",
        "a/x/b.mara.md",
        "mabn/o.mara.md",
        "m/n/o.mara.md",
        "r1/r.mara.md",
        "q/r2/r.mara.md",
        "kept.mara.md",
    ] {
        project.write(path, "# Narrative\n");
    }
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, _) = request(&mut inputs, Clock::new().next());
    assert_eq!(printed(&generation), [] as [String; 0]);
    assert_eq!(paths(&generation), git_documents(&project));
}

/// A project whose root `.gitignore` holds `line`, with a `*.mara.md` file at each of `files`:
/// the documents Mara lists, then those Git lists.
fn mara_and_git(line: &[u8], files: &[&str]) -> (Vec<String>, Vec<String>) {
    let project = Project::new();
    fs::write(project.path(".gitignore"), [line, b"\n"].concat()).unwrap();
    for path in files {
        project.write(path, "# Narrative\n");
    }
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, _) = request(&mut inputs, Clock::new().next());
    assert_eq!(printed(&generation), [] as [String; 0]);
    let mara = paths(&generation).into_iter().map(str::to_owned).collect();
    (mara, git_documents(&project))
}

/// Known limitation of the `ignore` crate (DES-INPUTS-AND-KEYS): it reads `{a,b}` as
/// alternatives, where Git reads the braces literally. A crate that changes this fails here.
#[test]
fn known_limitation_braces_alternate_unlike_git() {
    let (mara, git) = mara_and_git(b"{a,b}.mara.md", &["a.mara.md", "{a,b}.mara.md"]);
    assert_eq!(mara, ["{a,b}.mara.md"]);
    assert_eq!(git, ["a.mara.md"]);
}

/// Known limitation: the crate reads `[[:digit:]]` as the set `[:digit` followed by `]`, where
/// Git reads a POSIX class.
#[test]
fn known_limitation_posix_classes_are_no_classes_unlike_git() {
    let files = ["1x.mara.md", "d]x.mara.md", "dx.mara.md"];
    let (mara, git) = mara_and_git(b"[[:digit:]]x.mara.md", &files);
    assert_eq!(mara, ["1x.mara.md", "dx.mara.md"]);
    assert_eq!(git, ["d]x.mara.md", "dx.mara.md"]);
}

/// Known limitation: the crate trims trailing tabs, where Git trims only spaces.
#[test]
fn known_limitation_trailing_tabs_are_trimmed_unlike_git() {
    let (mara, git) = mara_and_git(b"t.mara.md\t", &["t.mara.md", "u.mara.md"]);
    assert_eq!(mara, ["u.mara.md"]);
    assert_eq!(git, ["t.mara.md", "u.mara.md"]);
}

/// Known limitation: the crate keeps a NUL byte in a pattern, which then matches no name, where
/// Git ends the pattern at it.
#[test]
fn known_limitation_a_nul_byte_stays_in_the_pattern_unlike_git() {
    let (mara, git) = mara_and_git(b"nul.mara.md\0rest", &["nul.mara.md", "u.mara.md"]);
    assert_eq!(mara, ["nul.mara.md", "u.mara.md"]);
    assert_eq!(git, ["u.mara.md"]);
}

/// The review's reproduction of a `.gitignore` the crate cannot compile, and a pattern it
/// refuses: each is reported at its line, with the crate's reason, and nothing in the
/// `.gitignore`'s directory or below is an input, whatever Git makes of it.
#[test]
fn a_gitignore_the_crate_cannot_compile_keeps_its_directory_out() {
    let project = Project::new();
    project.write("c.mara.md", &document(3, ""));
    project.write("docs/a.mara.md", &document(1, ""));
    project.write("docs/b.mara.md", &document(2, ""));
    project.write("docs/sub/d.mara.md", &document(4, ""));
    let mut contents = b"a.mara.md\n".to_vec();
    contents.extend(std::iter::repeat_n(b'?', 300_000));
    contents.extend(b"z\n");
    fs::write(project.path("docs/.gitignore"), &contents).unwrap();
    project.write("refused/.gitignore", "# a comment\n[z-a].mara.md\n");
    project.write("refused/e.mara.md", &document(5, ""));
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, work) = request(&mut inputs, Clock::new().next());
    assert_eq!(paths(&generation), ["c.mara.md"]);
    assert_eq!(
        work,
        counts(5, 5, 1, 1),
        "both files are read, nothing below them"
    );
    let found: Vec<(&str, usize, &str)> = generation
        .diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.path.as_str(), diagnostic.line, &*diagnostic.code))
        .collect();
    assert_eq!(
        found,
        [
            ("docs/.gitignore", 1, "unsupported-gitignore"),
            ("refused/.gitignore", 2, "unsupported-gitignore"),
        ]
    );
    let printed = printed(&generation);
    assert!(
        printed[0].contains("Mara cannot compile the patterns of this file (")
            && printed[0]
                .ends_with("so Mara cannot apply its patterns and reads nothing in docs or below"),
        "{printed:?}"
    );
    assert!(
        printed[1].contains("invalid range; 'z' > 'a'"),
        "the crate's reason: {printed:?}"
    );
    // Git ignores the one document the long file names and applies the other file.
    assert_eq!(
        git_documents(&project),
        [
            "c.mara.md",
            "docs/b.mara.md",
            "docs/sub/d.mara.md",
            "refused/e.mara.md"
        ]
    );
    // An unchanged file is not read again, and still reported.
    let (generation, work) = request(&mut inputs, Clock::new().next());
    assert_eq!(work, WorkCounts::default());
    assert_eq!(generation.diagnostics.len(), 2);
}

/// The review's reproduction: a pattern byte that is not UTF-8. The crate takes patterns only as
/// text, so Mara does not apply the file, never a lossily decoded pattern; Git applies it, and
/// the byte matches no UTF-8 name.
#[test]
fn a_gitignore_line_that_is_not_utf8_keeps_its_directory_out() {
    let project = Project::new();
    fs::write(
        project.path(".gitignore"),
        b"# caf\xE9 is fine in a comment\n\xFF.mara.md\n",
    )
    .unwrap();
    project.write("\u{fffd}.mara.md", &document(1, ""));
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, _) = request(&mut inputs, Clock::new().next());
    assert_eq!(paths(&generation), [] as [&str; 0]);
    assert_eq!(
        printed(&generation),
        [
            ".gitignore:2: error[unsupported-gitignore]: this line is not UTF-8, and Mara reads \
          `.gitignore` patterns only as UTF-8 text, so Mara cannot apply its patterns and reads \
          nothing in the project root or below"
        ]
    );
    assert_eq!(git_documents(&project), ["\u{fffd}.mara.md"]);
}

/// A `.gitignore` that cannot be applied might ignore any entry of its directory, so nothing
/// there or below is an input, and the inventory says so (DES-INPUTS-AND-KEYS).
#[cfg(unix)]
#[test]
fn a_gitignore_that_cannot_be_applied_keeps_its_directory_out() {
    use std::os::unix::fs::PermissionsExt;
    let project = Project::new();
    project.write("a.mara.md", &document(1, ""));
    for directory in ["pipe", "folder", "locked"] {
        project.write(&format!("{directory}/b.mara.md"), &document(2, ""));
        project.write(&format!("{directory}/sub/c.mara.md"), &document(3, ""));
    }
    // The review's reproduction is a pipe at the root; it is never opened, so nothing blocks.
    let status = Command::new("mkfifo")
        .arg(project.path("pipe/.gitignore"))
        .status()
        .expect("run mkfifo");
    assert!(status.success());
    fs::create_dir(project.path("folder/.gitignore")).unwrap();
    project.write("locked/.gitignore", "nothing\n");
    let lock = |mode| {
        let permissions = fs::Permissions::from_mode(mode);
        fs::set_permissions(project.path("locked/.gitignore"), permissions).unwrap();
    };
    lock(0o000);
    let readable = fs::read(project.path("locked/.gitignore")).is_ok();
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, work) = request(&mut inputs, Clock::new().next());
    lock(0o644);
    assert_eq!(paths(&generation), ["a.mara.md"]);
    assert_eq!(work, counts(3, 3, 1, 1), "nothing below is read");
    let mut expected = vec![
        "folder/.gitignore:1: error[io-error]: folder/.gitignore is not a regular file, so Mara \
         cannot apply its patterns and reads nothing in folder or below",
        "pipe/.gitignore:1: error[io-error]: pipe/.gitignore is not a regular file, so Mara \
         cannot apply its patterns and reads nothing in pipe or below",
    ];
    if !readable {
        expected.insert(
            1,
            "locked/.gitignore:1: error[io-error]: cannot read locked/.gitignore, so Mara cannot \
             apply its patterns and reads nothing in locked or below: Permission denied (os \
             error 13)",
        );
    }
    let found = printed(&generation);
    if readable {
        // Permissions do not apply, as for root.
        assert_eq!(found.len(), 2, "{found:?}");
        return;
    }
    assert_eq!(found, expected);

    // At the project root, no document is an input.
    let root = Project::new();
    root.write("a.mara.md", &document(1, ""));
    let status = Command::new("mkfifo")
        .arg(root.path(".gitignore"))
        .status()
        .expect("run mkfifo");
    assert!(status.success());
    let mut inputs = Inputs::new(root.root.clone());
    let (generation, work) = request(&mut inputs, Clock::new().next());
    assert_eq!(paths(&generation), [] as [&str; 0]);
    assert_eq!(work, counts(2, 2, 0, 1));
    assert_eq!(
        printed(&generation),
        [
            ".gitignore:1: error[io-error]: .gitignore is not a regular file, so Mara cannot apply \
          its patterns and reads nothing in the project root or below"
        ]
    );
}

/// The review's reproduction: a file that is both the schema and a document is read and hashed
/// once in a request; so is a schema that is the configuration or a `.gitignore`.
#[test]
fn a_file_with_two_roles_is_read_once_per_request() {
    let project = Project::new();
    project.write(
        ".mara/config.toml",
        &CONFIG.replace(".mara/schema.toml", ".mara/schema.mara.md"),
    );
    project.write(".mara/schema.mara.md", SCHEMA);
    let mut clock = Clock::new();
    let mut inputs = Inputs::new(project.root.clone());
    let (generation, work) = request(&mut inputs, clock.next());
    assert_eq!(paths(&generation), [".mara/schema.mara.md"]);
    assert_eq!(generation.project.diagnostics, []);
    assert_eq!(work, counts(2, 2, 1, 1));
    // Changed, it is read again once.
    project.write(".mara/schema.mara.md", &format!("{SCHEMA}{RELATION}"));
    let (_, work) = request(&mut inputs, clock.next());
    assert_eq!(work, counts(1, 1, 1, 1));

    for schema in [".mara/config.toml", ".gitignore"] {
        let project = Project::new();
        project.write(
            ".mara/config.toml",
            &CONFIG.replace(".mara/schema.toml", schema),
        );
        project.write(".gitignore", "nothing\n");
        project.write("a.mara.md", &document(1, ""));
        let mut inputs = Inputs::new(project.root.clone());
        let (generation, work) = request(&mut inputs, Clock::new().next());
        assert_eq!(paths(&generation), ["a.mara.md"], "{schema}");
        assert_eq!(work, counts(3, 3, 1, 1), "{schema}");
    }
}
