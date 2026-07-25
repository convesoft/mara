use std::{fs, path::Path, process::Command};

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

    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_COMMON_DIR")
            .env_remove("GIT_INDEX_FILE")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .status()
            .expect("run Git for isolated fixture");
        assert!(status.success(), "Git command failed: {args:?}");
        if args.first() == Some(&"init") {
            let excludes = self.root.join(".git/fixture-global-excludes");
            fs::write(&excludes, "").unwrap();
            let status = Command::new("git")
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_COMMON_DIR")
                .env_remove("GIT_INDEX_FILE")
                .arg("-C")
                .arg(&self.root)
                .args(["config", "core.excludesFile", excludes.to_str().unwrap()])
                .status()
                .expect("isolate fixture from ambient Git excludes");
            assert!(status.success());
        }
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
            "literal/[{]draft[}].md",
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
        "literal/{draft}.md",
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
            "literal/{draft}.md",
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
    fixture.git(&["init", "--quiet"]);
    fixture.write(
        ".gitignore",
        "ignored.mara.md\nignored-dir/\ntracked-dir/\ntracked-ignored.mara.md\n",
    );
    fixture.write("ignored.mara.md", "ignored");
    fixture.write("ignored-dir/nested.mara.md", "ignored nested");
    fixture.write("tracked-dir/nested.mara.md", "tracked nested source");
    fixture.write("tracked-ignored.mara.md", "tracked source");
    fixture.write("untracked.mara.md", "new source");
    fixture.git(&[
        "add",
        "--force",
        "tracked-ignored.mara.md",
        "tracked-dir/nested.mara.md",
    ]);

    let discovery = discover_content(&fixture.load());

    assert_eq!(
        document_paths(&discovery),
        [
            "tracked-dir/nested.mara.md",
            "tracked-ignored.mara.md",
            "untracked.mara.md"
        ]
    );
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
            "tracked-dir/nested.mara.md",
            "tracked-ignored.mara.md",
            "untracked.mara.md"
        ]
    );
}

#[test]
fn gitignore_has_no_effect_outside_a_git_worktree() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], true, false, false);
    fs::create_dir(fixture.root.join(".git")).unwrap();
    fixture.write(".gitignore", "selected.mara.md\n");
    fixture.write("selected.mara.md", "selected");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["selected.mara.md"]);
    assert!(discovery.diagnostics().is_empty());
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
fn gitignore_pattern_semantics_are_delegated_to_git() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], true, false, false);
    fixture.git(&["init", "--quiet"]);
    fixture.write(".gitignore", "[z-a]\n");
    fixture.write("nested/good.mara.md", "good source");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["nested/good.mara.md"]);
    assert!(discovery.diagnostics().is_empty());
}

#[cfg(unix)]
#[test]
fn gitignore_files_preserve_git_byte_semantics() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], true, false, false);
    fixture.git(&["init", "--quiet"]);
    fixture.write(
        ".gitignore",
        [b'i', b'g', b'n', b'o', b'r', b'e', b'd', b'-', 0xff, b'\n'],
    );
    fixture.write("good.mara.md", "good source");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["good.mara.md"]);
    assert!(discovery.diagnostics().is_empty());
}

#[test]
fn ignore_query_failures_preserve_documents_and_report_evidence() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], true, false, false);
    fixture.git(&["init", "--quiet"]);
    fs::create_dir(fixture.root.join(".git/index")).unwrap();
    fixture.write(".gitignore", "ignored.mara.md\n");
    fixture.write("ignored.mara.md", "ignored");
    fixture.write("good.mara.md", "good");

    let discovery = discover_content(&fixture.load());

    assert_eq!(
        document_paths(&discovery),
        ["good.mara.md", "ignored.mara.md"]
    );
    assert!(discovery.diagnostics().iter().any(|diagnostic| {
        diagnostic.details().get("reason")
            == Some(&mara_core::DiagnosticValue::from("ignore_query_failed"))
    }));
}

#[test]
fn unreadable_ignore_rules_are_reported_without_hiding_independent_content() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], true, false, false);
    fixture.git(&["init", "--quiet"]);
    fs::create_dir(fixture.root.join(".gitignore")).unwrap();
    fixture.write("good.mara.md", "good");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["good.mara.md"]);
    assert!(
        discovery.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .primary()
                .is_some_and(|span| span.path() == ".gitignore")
                && diagnostic.details().get("reason")
                    == Some(&mara_core::DiagnosticValue::from("ignore_rule_io"))
        }),
        "diagnostics: {:#?}",
        discovery.diagnostics()
    );
}

#[test]
fn unreadable_external_ignore_rules_are_reported_without_host_path_provenance() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], true, false, false);
    fixture.git(&["init", "--quiet"]);
    let external_ignore = fixture._temp.path().join("external-global-ignore");
    fs::create_dir(&external_ignore).unwrap();
    fixture.git(&[
        "config",
        "core.excludesFile",
        external_ignore.to_str().unwrap(),
    ]);
    fixture.write("good.mara.md", "good");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["good.mara.md"]);
    assert!(discovery.diagnostics().iter().any(|diagnostic| {
        diagnostic.primary().is_none()
            && diagnostic.details().get("reason")
                == Some(&mara_core::DiagnosticValue::from("ignore_rule_io"))
    }));
}

#[test]
fn repository_configured_excludes_are_applied_to_untracked_content() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], true, false, false);
    fixture.git(&["init", "--quiet"]);
    let external_ignore = fixture._temp.path().join("effective-excludes");
    fs::write(&external_ignore, "ignored.mara.md\n").unwrap();
    fixture.git(&[
        "config",
        "core.excludesFile",
        external_ignore.to_str().unwrap(),
    ]);
    fixture.write("ignored.mara.md", "ignored");
    fixture.write("good.mara.md", "good");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["good.mara.md"]);
    assert!(discovery.diagnostics().is_empty());
}

#[cfg(unix)]
#[test]
fn readable_special_files_are_valid_configured_excludes() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], true, false, false);
    fixture.git(&["init", "--quiet"]);
    fixture.git(&["config", "core.excludesFile", "/dev/null"]);
    fixture.write("good.mara.md", "good");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["good.mara.md"]);
    assert!(discovery.diagnostics().is_empty());
}

#[cfg(unix)]
#[test]
fn symlinked_worktree_ignore_files_are_not_followed() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], true, false, false);
    fixture.git(&["init", "--quiet"]);
    let outside_ignore = fixture._temp.path().join("outside-ignore");
    fs::write(&outside_ignore, "selected.mara.md\n").unwrap();
    symlink_file(&outside_ignore, fixture.root.join(".gitignore"));
    fixture.write("selected.mara.md", "selected");

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["selected.mara.md"]);
    assert!(discovery.diagnostics().is_empty());
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

#[cfg(unix)]
#[test]
fn write_only_index_alias_is_diagnosed_before_content_open() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new(&["**/*.mara.md"], &[], false, false, false);
    fixture.write("index.mara.md", "derived output");
    let config = fs::read_to_string(fixture.root.join(".mara/project.toml"))
        .unwrap()
        .replace(".mara/index.json", "index.mara.md");
    fs::write(fixture.root.join(".mara/project.toml"), config).unwrap();
    fs::set_permissions(
        fixture.root.join("index.mara.md"),
        fs::Permissions::from_mode(0o200),
    )
    .unwrap();

    let discovery = discover_content(&fixture.load());

    assert!(discovery.documents().is_empty());
    assert_eq!(discovery.diagnostics().len(), 1);
    assert_eq!(
        discovery.diagnostics()[0].code(),
        DiagnosticCode::Project(ProjectDiagnosticCode::DuplicateFile)
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
fn unselected_dangling_symlinks_do_not_invalidate_discovery() {
    let fixture = Fixture::new(&["selected.mara.md"], &[], false, false, false);
    fixture.write("selected.mara.md", "selected");
    symlink_file("missing.txt", fixture.root.join("unselected.txt"));

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["selected.mara.md"]);
    assert!(discovery.diagnostics().is_empty());
}

#[cfg(any(unix, windows))]
#[test]
fn external_file_symlink_targets_are_rejected() {
    let fixture = Fixture::new(&["*.mara.md"], &[], false, false, true);
    let outside = fixture._temp.path().join("outside-source.md");
    fs::write(&outside, "outside").unwrap();
    symlink_file(&outside, fixture.root.join("external.mara.md"));

    let discovery = discover_content(&fixture.load());

    assert!(discovery.documents().is_empty());
    assert_eq!(discovery.diagnostics().len(), 1);
    assert_eq!(
        discovery.diagnostics()[0].code(),
        DiagnosticCode::Project(ProjectDiagnosticCode::SymlinkRejected)
    );
    assert_eq!(
        discovery.diagnostics()[0].primary().unwrap().path(),
        "external.mara.md"
    );
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
fn sibling_directory_symlink_cycles_are_bounded_by_filesystem_identity() {
    let fixture = Fixture::new(&["**/*.mara.md"], &[], false, true, false);
    fs::create_dir_all(fixture.root.join("a")).unwrap();
    fs::create_dir_all(fixture.root.join("b")).unwrap();
    symlink_directory("../b", fixture.root.join("a/to-b"));
    symlink_directory("../a", fixture.root.join("b/to-a"));

    let discovery = discover_content(&fixture.load());

    let cycle_paths = discovery
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code() == DiagnosticCode::Content(ContentDiagnosticCode::Io)
                && diagnostic.details().get("reason")
                    == Some(&mara_core::DiagnosticValue::from("directory_cycle"))
        })
        .filter_map(|diagnostic| diagnostic.primary().map(|span| span.path()))
        .collect::<Vec<_>>();
    assert!(cycle_paths.contains(&"a/to-b/to-a"));
    assert!(cycle_paths.contains(&"b/to-a/to-b"));
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
#[test]
fn tracked_directory_symlinks_remain_skipped_when_following_is_disabled() {
    let fixture = Fixture::new(&["alias"], &[], true, false, true);
    fixture.write("real/inside.mara.md", "inside");
    symlink_directory("real", fixture.root.join("alias"));
    fixture.git(&["init", "--quiet"]);
    fixture.git(&["add", "alias"]);

    let discovery = discover_content(&fixture.load());

    assert!(discovery.documents().is_empty());
    assert!(discovery.diagnostics().is_empty());
}

#[cfg(any(unix, windows))]
#[test]
fn full_include_shape_prunes_unreachable_symlink_trees_before_policy_checks() {
    let fixture = Fixture::new(&["docs/*/selected/*.mara.md"], &[], false, true, false);
    fixture.write("docs/group/selected/source.mara.md", "selected");
    let outside = fixture._temp.path().join("outside-include");
    fs::create_dir(&outside).unwrap();
    fs::create_dir_all(fixture.root.join("docs/group")).unwrap();
    symlink_directory(&outside, fixture.root.join("docs/group/unreachable"));

    let discovery = discover_content(&fixture.load());

    assert_eq!(
        document_paths(&discovery),
        ["docs/group/selected/source.mara.md"]
    );
    assert!(discovery.diagnostics().is_empty());
}

#[cfg(any(unix, windows))]
#[test]
fn fully_excluded_symlink_trees_are_pruned_before_policy_checks() {
    let fixture = Fixture::new(&["**/*.mara.md"], &["external/**"], false, true, false);
    fixture.write("ordinary.mara.md", "ordinary");
    let outside = fixture._temp.path().join("outside-exclude");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("outside.mara.md"), "outside").unwrap();
    symlink_directory(&outside, fixture.root.join("external"));

    let discovery = discover_content(&fixture.load());

    assert_eq!(document_paths(&discovery), ["ordinary.mara.md"]);
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
