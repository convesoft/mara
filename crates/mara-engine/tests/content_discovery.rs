use std::{fs, path::Path};

use mara_core::{ContentDiagnosticCode, DiagnosticCode, LineEnding, ProjectDiagnosticCode};
use mara_engine::{content::discover_content, project::load_from_root};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    root: std::path::PathBuf,
}

impl Fixture {
    fn new(
        include: &[&str],
        exclude: &[&str],
        respect_gitignore: bool,
        follow_directory_symlinks: bool,
        allow_internal_file_symlinks: bool,
    ) -> Self {
        let temp = tempfile::tempdir().expect("create isolated fixture");
        let root = temp.path().join("project");
        fs::create_dir_all(root.join(".mara")).unwrap();
        fs::write(root.join(".mara/schema.yaml"), "format_version: 1\n").unwrap();
        fs::write(
            root.join(".mara/project.toml"),
            config(
                include,
                exclude,
                respect_gitignore,
                follow_directory_symlinks,
                allow_internal_file_symlinks,
            ),
        )
        .unwrap();
        Self {
            _temp: temp,
            root: root.canonicalize().unwrap(),
        }
    }

    fn write(&self, path: &str, source: impl AsRef<[u8]>) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }

    fn load(&self) -> mara_engine::project::LoadedProject {
        load_from_root(&self.root).unwrap()
    }
}

fn config(
    include: &[&str],
    exclude: &[&str],
    respect_gitignore: bool,
    follow_directory_symlinks: bool,
    allow_internal_file_symlinks: bool,
) -> String {
    format!(
        r#"format_version = 1
[project]
name = "content-test"
schema = ".mara/schema.yaml"
[content]
include = {include:?}
exclude = {exclude:?}
respect_gitignore = {respect_gitignore}
follow_directory_symlinks = {follow_directory_symlinks}
allow_internal_file_symlinks = {allow_internal_file_symlinks}
[index]
path = ".mara/index.json"
[validation]
warnings_as_errors = false
[git]
require_clean_worktree_for_writes = true
"#
    )
}

fn document_paths(discovery: &mara_engine::content::ContentDiscovery) -> Vec<&str> {
    discovery
        .documents()
        .iter()
        .map(|document| document.path())
        .collect()
}

#[test]
fn configured_globs_select_files_with_deterministic_precedence_and_order() {
    let fixture = Fixture::new(
        &[
            "**/*.mara.md",
            "notes/file?.md",
            "classes/[a-c].md",
            "literal/[?].md",
            "negative/[!a].md",
            "unicode/?.md",
            "extensions/$cash.md",
            "extensions/(flag).md",
            "extensions/<repeat>.md",
            "extensions/name,part.md",
            "classes/[a-b-c].txt",
        ],
        &["docs/excluded*.mara.md", "classes/b.md"],
        false,
        false,
        false,
    );
    for path in [
        "z.mara.md",
        "a.mara.md",
        "docs/kept.mara.md",
        "docs/excluded-one.mara.md",
        "notes/file1.md",
        "notes/file12.md",
        "classes/a.md",
        "classes/b.md",
        "classes/d.md",
        "literal/?.md",
        "negative/a.md",
        "negative/b.md",
        "unicode/é.md",
        "unicode/éé.md",
        "extensions/$cash.md",
        "extensions/(flag).md",
        "extensions/<repeat>.md",
        "extensions/name,part.md",
        "classes/c.txt",
        "classes/d.txt",
        "ordinary.md",
    ] {
        fixture.write(path, path);
    }

    let project = fixture.load();
    let first = discover_content(&project);
    let second = discover_content(&project);

    assert_eq!(first, second, "repeated discovery must be deterministic");
    assert_eq!(
        document_paths(&first),
        [
            "a.mara.md",
            "classes/a.md",
            "classes/c.txt",
            "docs/kept.mara.md",
            "extensions/$cash.md",
            "extensions/(flag).md",
            "extensions/<repeat>.md",
            "extensions/name,part.md",
            "literal/?.md",
            "negative/b.md",
            "notes/file1.md",
            "unicode/é.md",
            "z.mara.md",
        ]
    );
    assert!(first.diagnostics().is_empty());
}

#[test]
fn gitignore_overrides_includes_but_matching_untracked_files_remain_eligible() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], true, false, false);
    fs::create_dir(fixture.root.join(".git")).unwrap();
    fixture.write(".gitignore", "ignored.mara.md\nignored-dir/\n");
    fixture.write("ignored.mara.md", "ignored");
    fixture.write("ignored-dir/nested.mara.md", "ignored nested");
    fixture.write("untracked.mara.md", "new source");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["untracked.mara.md"]);
    assert!(discovery.diagnostics().is_empty());

    fs::write(
        fixture.root.join(".mara/project.toml"),
        config(&["**/*.mara.md"], &[], false, false, false),
    )
    .unwrap();
    let without_ignore = discover_content(&fixture.load());
    assert_eq!(
        document_paths(&without_ignore),
        [
            "ignored-dir/nested.mara.md",
            "ignored.mara.md",
            "untracked.mara.md"
        ]
    );
}

#[test]
fn invalid_utf8_does_not_erase_independently_loaded_documents() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], false, false, false);
    fixture.write("a-bad.mara.md", [b'a', b'\n', 0xff, b'b']);
    fs::hard_link(
        fixture.root.join("a-bad.mara.md"),
        fixture.root.join("b-bad-alias.mara.md"),
    )
    .unwrap();
    fixture.write("good.mara.md", "complete\nsource\n");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["good.mara.md"]);
    assert_eq!(discovery.diagnostics().len(), 2);
    let invalid_utf8 = discovery
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.code() == DiagnosticCode::Content(ContentDiagnosticCode::InvalidUtf8)
        })
        .unwrap();
    let primary = invalid_utf8.primary().unwrap();
    assert_eq!(primary.path(), "a-bad.mara.md");
    assert_eq!(primary.start_byte(), 2);
    assert_eq!((primary.start_line(), primary.start_column()), (2, 1));
    assert!(discovery.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::Project(ProjectDiagnosticCode::DuplicateFile)
            && diagnostic
                .primary()
                .is_some_and(|span| span.path() == "b-bad-alias.mara.md")
    }));
}

#[cfg(unix)]
#[test]
fn unsupported_source_paths_are_diagnosed_without_panicking_or_losing_other_files() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], false, false, false);
    fixture.write("a-invalid.mara.md", [0xff]);
    fixture.write("scheme:bad.mara.md", "unsupported path");
    fixture.write("valid.mara.md", "valid source");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["valid.mara.md"]);
    assert_eq!(discovery.diagnostics().len(), 2);
    assert_eq!(
        discovery.diagnostics()[0].code(),
        DiagnosticCode::Content(ContentDiagnosticCode::InvalidUtf8)
    );
    assert!(discovery.diagnostics()[0].primary().is_some());
    assert!(discovery.diagnostics()[1].primary().is_none());
}

#[test]
fn gitignore_parse_errors_are_reported_without_hiding_independent_content() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], true, false, false);
    fs::create_dir(fixture.root.join(".git")).unwrap();
    fixture.write(".gitignore", "[z-a]\n");
    fixture.write("nested/good.mara.md", "good source");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["nested/good.mara.md"]);
    assert!(discovery.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::Content(ContentDiagnosticCode::Io)
    }));
}

#[test]
fn complete_text_provenance_and_source_boundaries_are_retained() {
    let fixture = Fixture::new(&["docs/*.mara.md"], &[], false, false, false);
    let source = "é\r\nline\r\n";
    fixture.write("docs/unicode.mara.md", source);

    let discovery = discover_content(&fixture.load());

    assert!(discovery.is_valid());
    let [document] = discovery.documents() else {
        panic!("expected exactly one source document")
    };
    assert_eq!(document.path(), "docs/unicode.mara.md");
    assert_eq!(document.source().as_str(), source);
    assert_eq!(document.source().line_ending(), LineEnding::CrLf);
    assert_eq!(document.span().start_byte(), 0);
    assert_eq!(document.span().end_byte(), source.len() as u64);
    assert_eq!(
        (document.span().start_line(), document.span().start_column()),
        (1, 1)
    );
    assert_eq!(
        (document.span().end_line(), document.span().end_column()),
        (3, 1)
    );
    assert_eq!(document.source_index().coordinates_at(4), Ok((2, 1)));
}

#[test]
fn matching_is_case_sensitive_even_when_the_filesystem_is_not() {
    let fixture = Fixture::new(&["case.mara.md"], &[], false, false, false);
    fixture.write("Case.mara.md", "case-sensitive");

    let discovery = discover_content(&fixture.load());

    assert!(discovery.documents().is_empty());
    assert!(discovery.diagnostics().is_empty());
}

#[test]
fn duplicate_file_identities_are_diagnosed_without_losing_other_documents() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], false, false, false);
    fixture.write("a.mara.md", "shared");
    fs::hard_link(
        fixture.root.join("a.mara.md"),
        fixture.root.join("b.mara.md"),
    )
    .unwrap();
    fixture.write("c.mara.md", "independent");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["a.mara.md", "c.mara.md"]);
    assert_eq!(discovery.diagnostics().len(), 1);
    assert_eq!(
        discovery.diagnostics()[0].code(),
        DiagnosticCode::Project(ProjectDiagnosticCode::DuplicateFile)
    );
    assert_eq!(
        discovery.diagnostics()[0].primary().unwrap().path(),
        "b.mara.md"
    );
}

#[test]
fn configured_index_destination_cannot_also_be_selected_content() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], false, false, false);
    fixture.write("index.mara.md", "derived output");
    let config = fs::read_to_string(fixture.root.join(".mara/project.toml"))
        .unwrap()
        .replace(".mara/index.json", "index.mara.md");
    fs::write(fixture.root.join(".mara/project.toml"), config).unwrap();

    let discovery = discover_content(&fixture.load());

    assert!(discovery.documents().is_empty());
    assert_eq!(discovery.diagnostics().len(), 1);
    assert_eq!(
        discovery.diagnostics()[0].code(),
        DiagnosticCode::Project(ProjectDiagnosticCode::DuplicateFile)
    );
    assert_eq!(
        discovery.diagnostics()[0].primary().unwrap().path(),
        "index.mara.md"
    );
}

#[cfg(any(unix, windows))]
#[test]
fn internal_file_symlinks_follow_explicit_policy() {
    let disabled = Fixture::new(&["*.mara.md"], &[], false, false, false);
    disabled.write("target.txt", "linked source");
    symlink_file("target.txt", disabled.root.join("link.mara.md"));

    let rejected = discover_content(&disabled.load());
    assert!(rejected.documents().is_empty());
    assert_eq!(
        rejected.diagnostics()[0].code(),
        DiagnosticCode::Project(ProjectDiagnosticCode::SymlinkRejected)
    );

    let allowed = Fixture::new(&["*.mara.md"], &[], false, false, true);
    allowed.write("target.txt", "linked source");
    symlink_file("target.txt", allowed.root.join("link.mara.md"));

    let accepted = discover_content(&allowed.load());
    assert!(accepted.is_valid());
    assert_eq!(document_paths(&accepted), ["link.mara.md"]);
    assert_eq!(accepted.documents()[0].source().as_str(), "linked source");

    symlink_file("missing.txt", allowed.root.join("broken.mara.md"));
    let with_broken_link = discover_content(&allowed.load());
    assert_eq!(document_paths(&with_broken_link), ["link.mara.md"]);
    assert!(with_broken_link.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::Content(ContentDiagnosticCode::Io)
            && diagnostic
                .primary()
                .is_some_and(|span| span.path() == "broken.mara.md")
    }));
}

#[cfg(any(unix, windows))]
#[test]
fn directory_symlink_policy_contains_targets_and_recovers_from_cycles() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], false, true, false);
    fixture.write("ordinary.mara.md", "ordinary");
    fixture.write("real/inside.mara.md", "inside");
    symlink_directory("real", fixture.root.join("alias"));
    symlink_directory("..", fixture.root.join("real/cycle"));
    let outside = fixture._temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("outside.mara.md"), "outside").unwrap();
    symlink_directory(&outside, fixture.root.join("external"));

    let discovery = discover_content(&fixture.load());

    assert!(document_paths(&discovery).contains(&"ordinary.mara.md"));
    assert!(document_paths(&discovery).contains(&"alias/inside.mara.md"));
    assert!(discovery.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::Project(ProjectDiagnosticCode::SymlinkRejected)
            && diagnostic
                .primary()
                .is_some_and(|span| span.path() == "external")
    }));
    assert!(discovery.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::Content(ContentDiagnosticCode::Io)
            && diagnostic
                .primary()
                .is_some_and(|span| span.path() == "real/cycle")
    }));
}

#[cfg(any(unix, windows))]
#[test]
fn directory_symlinks_are_skipped_when_following_is_disabled() {
    let fixture = Fixture::new(&["alias/*.mara.md"], &[], false, false, false);
    fixture.write("real/inside.mara.md", "inside");
    symlink_directory("real", fixture.root.join("alias"));

    let discovery = discover_content(&fixture.load());

    assert!(discovery.documents().is_empty());
    assert!(discovery.diagnostics().is_empty());
}

#[cfg(unix)]
fn symlink_file(original: impl AsRef<Path>, link: impl AsRef<Path>) {
    std::os::unix::fs::symlink(original, link).unwrap();
}

#[cfg(windows)]
fn symlink_file(original: impl AsRef<Path>, link: impl AsRef<Path>) {
    std::os::windows::fs::symlink_file(original, link).unwrap();
}

#[cfg(unix)]
fn symlink_directory(original: impl AsRef<Path>, link: impl AsRef<Path>) {
    std::os::unix::fs::symlink(original, link).unwrap();
}

#[cfg(windows)]
fn symlink_directory(original: impl AsRef<Path>, link: impl AsRef<Path>) {
    std::os::windows::fs::symlink_dir(original, link).unwrap();
}
