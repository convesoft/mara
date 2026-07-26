use std::{fs, path::Path, process::Command};

use mara_engine::{
    IndexProjection, check_project,
    command::{OutputFormat, run_show},
    write_index,
};

const SCHEMA: &str = r#"format_version: 1
schema:
  name: index-fixture
  version: 1.0.0
identity:
  mid:
    format: ulid
    prefix: m_
flavours:
  alpha:
    label: Alpha
    description: Index source.
    guidance:
      use_when: [Testing source items.]
      avoid_when: [Testing target items.]
    id: {}
    title: {}
    body: {}
    fields:
      state:
        type: enum
        values: [active, inactive]
      tag:
        type: string
        repeatable: true
  beta:
    label: Beta
    description: Index target.
    guidance:
      use_when: [Testing target items.]
      avoid_when: [Testing source items.]
    id: {}
    title: {}
    body: {}
    fields: {}
relations:
  connects:
    source:
      flavours: [alpha]
    target:
      flavours: [beta]
    inverse: connected_by
    inverse_authoring: true
  external_link:
    source:
      flavours: [alpha]
    target:
      external: [https]
rules: []
"#;

const CONTENT: &str = r#"Preamble [[BETA-B|target]] and [[https://example.test/context]].

# Top

:::beta m_00000000000000000000000002
:id: BETA-B
:title: Second

Beta body.
:::

## Child

:::alpha m_00000000000000000000000001
:id: ALPHA-A
:title: First
:state: active
:tag: red
:tag: blue
:connects: BETA-B
:external_link: https://example.test/artifact

Alpha body with [[BETA-B]] and [[https://example.test/body]].
:::
"#;

fn write_project(root: &Path, warnings_as_errors: bool) {
    fs::create_dir_all(root.join(".mara")).unwrap();
    fs::create_dir_all(root.join("docs")).unwrap();
    let project = format!(
        r#"format_version = 1
[project]
name = "index-fixture"
schema = ".mara/schema.yaml"
[content]
include = ["docs/**/*.mara.md"]
exclude = []
respect_gitignore = true
follow_directory_symlinks = false
allow_internal_file_symlinks = true
[index]
path = ".mara/index.json"
[validation]
warnings_as_errors = {warnings_as_errors}
[git]
require_clean_worktree_for_writes = true
"#
    );
    fs::write(root.join(".mara/project.toml"), project).unwrap();
    fs::write(root.join(".mara/schema.yaml"), SCHEMA).unwrap();
    fs::write(root.join("docs/project.mara.md"), CONTENT).unwrap();
}

fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).expect("canonical index is JSON")
}

fn git(root: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .expect("run git fixture command");
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn complete_unversioned_projection_is_canonical_and_repeatable() {
    let fixture = tempfile::tempdir().unwrap();
    write_project(fixture.path(), false);
    let result = check_project(fixture.path()).unwrap();
    assert!(result.is_valid());

    let first = IndexProjection::from_validation(&result)
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let second = IndexProjection::from_validation(&result)
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(first, second);
    assert!(first.ends_with(b"\n"));
    assert!(!first.ends_with(b"\n\n"));
    assert!(!first.contains(&b'\r'));

    let raw = String::from_utf8(first.clone()).unwrap();
    assert!(raw.starts_with("{\n  \"format\": \"mara.index\",\n  \"version\": 1,\n"));
    let mut previous = 0;
    for key in [
        "\"project\"",
        "\"git\"",
        "\"documents\"",
        "\"items\"",
        "\"source_nodes\"",
        "\"edges\"",
        "\"mentions\"",
        "\"external_nodes\"",
        "\"diagnostics\"",
    ] {
        let marker = format!("\n  {key}:");
        assert_eq!(raw.matches(&marker).count(), 1, "top-level key {key}");
        let position = raw.find(&marker).unwrap();
        assert!(position > previous, "top-level key order at {key}");
        previous = position;
    }
    assert!(!raw.contains(fixture.path().to_str().unwrap()));
    assert!(!raw.contains("timestamp"));
    assert!(!raw.contains("generated_at"));

    let value = json(&first);
    assert_eq!(value["format"], "mara.index");
    assert_eq!(value["version"], 1);
    assert_eq!(value["git"]["available"], false);
    assert!(value["git"]["commit"].is_null());
    assert!(value["git"]["branch"].is_null());
    assert!(value["git"]["project_path"].is_null());
    assert!(value["git"]["dirty"].is_null());
    assert_eq!(value["project"]["name"], "index-fixture");
    assert_eq!(value["project"]["schema"]["format_version"], 1);
    assert_eq!(
        value["project"]["schema"]["sha256"].as_str().unwrap().len(),
        64
    );
    assert_eq!(value["documents"].as_array().unwrap().len(), 1);
    assert_eq!(value["documents"][0]["preamble"][0]["kind"], "narrative");
    assert_eq!(value["documents"][0]["sections"][0]["title"], "Top");
    assert_eq!(
        value["documents"][0]["sections"][0]["children"][0]["title"],
        "Child"
    );
    assert_eq!(
        value["documents"][0]["item_mids"],
        serde_json::json!([
            "m_00000000000000000000000002",
            "m_00000000000000000000000001"
        ])
    );
    assert_eq!(value["items"][0]["mid"], "m_00000000000000000000000001");
    assert_eq!(value["items"][1]["mid"], "m_00000000000000000000000002");
    assert_eq!(value["items"][0]["fields"][1]["name"], "tag");
    assert_eq!(
        value["items"][0]["fields"][1]["values"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(value["edges"].as_array().unwrap().len(), 2);
    assert_eq!(value["items"][0]["outgoing"].as_array().unwrap().len(), 2);
    assert_eq!(value["items"][1]["incoming"].as_array().unwrap().len(), 1);
    assert_eq!(value["mentions"].as_array().unwrap().len(), 4);
    assert_eq!(
        value["documents"][0]["preamble"][0]["block"]["mentions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(value["external_nodes"].as_array().unwrap().len(), 3);

    let written = write_index(&result).unwrap();
    assert_eq!(written.path(), ".mara/index.json");
    assert_eq!(written.sha256().len(), 64);
    assert_eq!(
        fs::read(fixture.path().join(written.path())).unwrap(),
        first
    );
    let repeated = write_index(&result).unwrap();
    assert_eq!(repeated.sha256(), written.sha256());
    assert_eq!(
        fs::read(fixture.path().join(repeated.path())).unwrap(),
        first
    );
}

#[test]
fn projection_hashes_the_schema_snapshot_that_validation_consumed() {
    let fixture = tempfile::tempdir().unwrap();
    write_project(fixture.path(), false);
    let result = check_project(fixture.path()).unwrap();
    let before = IndexProjection::from_validation(&result)
        .unwrap()
        .to_canonical_json()
        .unwrap();

    fs::write(
        fixture.path().join(".mara/schema.yaml"),
        format!("{SCHEMA}# changed after validation\n"),
    )
    .unwrap();
    let same_snapshot = IndexProjection::from_validation(&result)
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let fresh_snapshot = IndexProjection::from_validation(&check_project(fixture.path()).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();

    assert_eq!(
        json(&same_snapshot)["project"]["schema"]["sha256"],
        json(&before)["project"]["schema"]["sha256"]
    );
    assert_ne!(
        json(&fresh_snapshot)["project"]["schema"]["sha256"],
        json(&before)["project"]["schema"]["sha256"]
    );
}

#[test]
fn index_and_show_share_inverse_inline_occurrence_provenance() {
    let fixture = tempfile::tempdir().unwrap();
    write_project(fixture.path(), false);
    fs::write(
        fixture.path().join("docs/project.mara.md"),
        r#":::alpha m_00000000000000000000000001
:id: ALPHA-A
:title: Alpha

Alpha body.
:::

:::beta m_00000000000000000000000002
:id: BETA-B
:title: Beta

Beta body with [[connected_by:ALPHA-A]].
:::
"#,
    )
    .unwrap();
    let result = check_project(fixture.path()).unwrap();
    assert!(result.is_valid(), "diagnostics: {:?}", result.diagnostics());
    let index = IndexProjection::from_validation(&result)
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let index = json(&index);
    let show = run_show(fixture.path(), "ALPHA-A");
    let show: serde_json::Value = serde_json::from_str(&show.render(OutputFormat::Json)).unwrap();

    assert_eq!(
        index["edges"][0]["occurrences"][0]["origin"],
        "inverse_metadata"
    );
    assert_eq!(
        show["data"]["item"]["outgoing"][0]["occurrences"][0]["origin"],
        index["edges"][0]["occurrences"][0]["origin"]
    );
}

#[test]
fn symmetric_edges_use_the_canonical_name_for_inverse_presentation() {
    let fixture = tempfile::tempdir().unwrap();
    write_project(fixture.path(), false);
    let schema = SCHEMA.replacen(
        "relations:\n",
        r#"relations:
  related_to:
    source:
      flavours: [alpha]
    target:
      flavours: [alpha]
    symmetric: true
"#,
        1,
    );
    fs::write(fixture.path().join(".mara/schema.yaml"), schema).unwrap();
    fs::write(
        fixture.path().join("docs/project.mara.md"),
        r#":::alpha m_00000000000000000000000001
:id: ALPHA-A
:title: Alpha A
:related_to: ALPHA-B

Alpha A body.
:::

:::alpha m_00000000000000000000000003
:id: ALPHA-B
:title: Alpha B

Alpha B body.
:::
"#,
    )
    .unwrap();
    let result = check_project(fixture.path()).unwrap();
    assert!(result.is_valid(), "diagnostics: {:?}", result.diagnostics());
    let index = json(
        &IndexProjection::from_validation(&result)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
    );
    let edge = index["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|edge| edge["relation"] == "related_to")
        .unwrap();
    let show: serde_json::Value =
        serde_json::from_str(&run_show(fixture.path(), "ALPHA-A").render(OutputFormat::Json))
            .unwrap();

    assert_eq!(edge["inverse_name"], "related_to");
    assert_eq!(
        show["data"]["item"]["outgoing"][0]["inverse_name"],
        edge["inverse_name"]
    );
}

#[test]
fn filesystem_creation_order_does_not_change_projection_bytes() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    write_project(first.path(), false);
    write_project(second.path(), false);
    fs::write(first.path().join("docs/z.mara.md"), "Z narrative.\n").unwrap();
    fs::write(first.path().join("docs/a.mara.md"), "A narrative.\n").unwrap();
    fs::write(second.path().join("docs/a.mara.md"), "A narrative.\n").unwrap();
    fs::write(second.path().join("docs/z.mara.md"), "Z narrative.\n").unwrap();

    let first = IndexProjection::from_validation(&check_project(first.path()).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let second = IndexProjection::from_validation(&check_project(second.path()).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(
        json(&first)["documents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|document| document["path"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["docs/a.mara.md", "docs/project.mara.md", "docs/z.mara.md"]
    );
}

#[test]
fn writer_creates_a_missing_contained_parent_before_atomic_replacement() {
    let fixture = tempfile::tempdir().unwrap();
    write_project(fixture.path(), false);
    let config_path = fixture.path().join(".mara/project.toml");
    let config = fs::read_to_string(&config_path).unwrap().replace(
        "path = \".mara/index.json\"",
        "path = \"generated/mara/index.json\"",
    );
    fs::write(config_path, config).unwrap();

    let result = check_project(fixture.path()).unwrap();
    let written = write_index(&result).unwrap();
    assert_eq!(written.path(), "generated/mara/index.json");
    let bytes = fs::read(fixture.path().join(written.path())).unwrap();
    assert_eq!(json(&bytes)["format"], "mara.index");
}

#[test]
fn git_provenance_distinguishes_clean_modified_and_untracked_project_inputs() {
    let fixture = tempfile::tempdir().unwrap();
    let project_root = fixture.path().join("nested");
    git(fixture.path(), &["init", "-b", "main"]);
    git(fixture.path(), &["config", "user.name", "Mara Test"]);
    git(
        fixture.path(),
        &["config", "user.email", "mara@example.test"],
    );
    write_project(&project_root, false);
    fs::write(project_root.join("notes.txt"), "unselected fixture\n").unwrap();
    git(fixture.path(), &["add", "."]);
    git(fixture.path(), &["commit", "-m", "fixture"]);
    let commit = git(fixture.path(), &["rev-parse", "HEAD"]);

    let clean_result = check_project(&project_root).unwrap();
    let clean_bytes = IndexProjection::from_validation(&clean_result)
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let clean = json(&clean_bytes);
    assert_eq!(clean["git"]["available"], true);
    assert_eq!(clean["git"]["commit"], commit);
    assert_eq!(clean["git"]["branch"], "main");
    assert_eq!(clean["git"]["project_path"], "nested");
    assert_eq!(clean["git"]["dirty"], false);
    write_index(&clean_result).unwrap();
    let repeated_clean = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(repeated_clean, clean_bytes);

    fs::remove_file(project_root.join("notes.txt")).unwrap();
    let unrelated_deletion =
        IndexProjection::from_validation(&check_project(&project_root).unwrap())
            .unwrap()
            .to_canonical_json()
            .unwrap();
    assert_eq!(json(&unrelated_deletion)["git"]["dirty"], false);
    fs::write(project_root.join("notes.txt"), "unselected fixture\n").unwrap();

    fs::remove_file(project_root.join("docs/project.mara.md")).unwrap();
    let deleted = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(json(&deleted)["git"]["dirty"], true);
    assert_eq!(json(&deleted)["documents"].as_array().unwrap().len(), 0);
    fs::write(project_root.join("docs/project.mara.md"), CONTENT).unwrap();

    fs::write(
        project_root.join("docs/project.mara.md"),
        format!("{CONTENT}\n"),
    )
    .unwrap();
    let modified = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(json(&modified)["git"]["dirty"], true);

    fs::write(project_root.join("docs/project.mara.md"), CONTENT).unwrap();
    fs::write(
        project_root.join("docs/untracked.mara.md"),
        "Untracked narrative.\n",
    )
    .unwrap();
    let untracked = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let untracked = json(&untracked);
    assert_eq!(untracked["git"]["dirty"], true);
    assert_eq!(untracked["documents"].as_array().unwrap().len(), 2);

    fs::remove_file(project_root.join("docs/untracked.mara.md")).unwrap();
    git(fixture.path(), &["checkout", "--detach"]);
    let detached = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let detached = json(&detached);
    assert!(detached["git"]["branch"].is_null());
    assert_eq!(detached["git"]["dirty"], false);
}

#[test]
fn git_provenance_counts_selected_ignored_content_when_ignore_handling_is_disabled() {
    let fixture = tempfile::tempdir().unwrap();
    let project_root = fixture.path().join("nested");
    git(fixture.path(), &["init", "-b", "main"]);
    git(fixture.path(), &["config", "user.name", "Mara Test"]);
    git(
        fixture.path(),
        &["config", "user.email", "mara@example.test"],
    );
    write_project(&project_root, false);
    let config_path = project_root.join(".mara/project.toml");
    let config = fs::read_to_string(&config_path)
        .unwrap()
        .replace("respect_gitignore = true", "respect_gitignore = false");
    fs::write(config_path, config).unwrap();
    fs::write(
        fixture.path().join(".gitignore"),
        "nested/docs/ignored.mara.md\n",
    )
    .unwrap();
    fs::write(
        project_root.join("docs/ignored.mara.md"),
        "Ignored but selected narrative.\n",
    )
    .unwrap();
    git(fixture.path(), &["add", "."]);
    git(fixture.path(), &["commit", "-m", "fixture"]);

    let projection = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let projection = json(&projection);

    assert_eq!(projection["documents"].as_array().unwrap().len(), 2);
    assert_eq!(projection["git"]["dirty"], true);
}

#[test]
fn git_provenance_detects_selected_content_renamed_out_of_selection() {
    let fixture = tempfile::tempdir().unwrap();
    let project_root = fixture.path().join("nested");
    git(fixture.path(), &["init", "-b", "main"]);
    git(fixture.path(), &["config", "user.name", "Mara Test"]);
    git(
        fixture.path(),
        &["config", "user.email", "mara@example.test"],
    );
    git(fixture.path(), &["config", "diff.renames", "true"]);
    write_project(&project_root, false);
    git(fixture.path(), &["add", "."]);
    git(fixture.path(), &["commit", "-m", "fixture"]);
    git(
        fixture.path(),
        &["mv", "nested/docs/project.mara.md", "nested/renamed.txt"],
    );

    let projection = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let projection = json(&projection);

    assert!(projection["documents"].as_array().unwrap().is_empty());
    assert_eq!(projection["git"]["dirty"], true);
}

#[cfg(unix)]
#[test]
fn git_provenance_tracks_resolved_internal_symlink_targets() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().unwrap();
    let project_root = fixture.path().join("nested");
    git(fixture.path(), &["init", "-b", "main"]);
    git(fixture.path(), &["config", "user.name", "Mara Test"]);
    git(
        fixture.path(),
        &["config", "user.email", "mara@example.test"],
    );
    write_project(&project_root, false);
    fs::remove_file(project_root.join("docs/project.mara.md")).unwrap();
    fs::create_dir_all(project_root.join("sources")).unwrap();
    let target = project_root.join("sources/actual.md");
    fs::write(&target, CONTENT).unwrap();
    symlink(
        "../sources/actual.md",
        project_root.join("docs/link.mara.md"),
    )
    .unwrap();
    git(fixture.path(), &["add", "."]);
    git(fixture.path(), &["commit", "-m", "fixture"]);

    let clean = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(json(&clean)["git"]["dirty"], false);

    fs::write(&target, format!("{CONTENT}\n")).unwrap();
    let modified = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let modified = json(&modified);
    assert_eq!(modified["documents"][0]["path"], "docs/link.mara.md");
    assert_eq!(modified["git"]["dirty"], true);
}

#[cfg(unix)]
#[test]
fn git_provenance_tracks_the_resolved_project_config_target() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().unwrap();
    let project_root = fixture.path().join("nested");
    git(fixture.path(), &["init", "-b", "main"]);
    git(fixture.path(), &["config", "user.name", "Mara Test"]);
    git(
        fixture.path(),
        &["config", "user.email", "mara@example.test"],
    );
    write_project(&project_root, false);
    fs::create_dir(project_root.join("config")).unwrap();
    let target = project_root.join("config/project.toml");
    fs::rename(project_root.join(".mara/project.toml"), &target).unwrap();
    symlink(
        "../config/project.toml",
        project_root.join(".mara/project.toml"),
    )
    .unwrap();
    git(fixture.path(), &["add", "."]);
    git(fixture.path(), &["commit", "-m", "fixture"]);

    let clean = check_project(&project_root).unwrap();
    let clean_projection = IndexProjection::from_validation(&clean)
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(json(&clean_projection)["git"]["dirty"], false);

    let changed = format!("{}\n", fs::read_to_string(&target).unwrap());
    fs::write(&target, changed).unwrap();
    let modified = check_project(&project_root).unwrap();
    let modified_projection = IndexProjection::from_validation(&modified)
        .unwrap()
        .to_canonical_json()
        .unwrap();

    assert_eq!(json(&modified_projection)["git"]["dirty"], true);
    assert!(matches!(
        write_index(&modified),
        Err(mara_engine::IndexError::DirtyWorktree)
    ));
}

#[test]
fn git_provenance_rejects_head_changes_after_validation_started() {
    let fixture = tempfile::tempdir().unwrap();
    let project_root = fixture.path().join("nested");
    git(fixture.path(), &["init", "-b", "main"]);
    git(fixture.path(), &["config", "user.name", "Mara Test"]);
    git(
        fixture.path(),
        &["config", "user.email", "mara@example.test"],
    );
    write_project(&project_root, false);
    git(fixture.path(), &["add", "."]);
    git(fixture.path(), &["commit", "-m", "first"]);
    let validated = check_project(&project_root).unwrap();

    fs::write(
        project_root.join("docs/project.mara.md"),
        format!("{CONTENT}\n"),
    )
    .unwrap();
    git(fixture.path(), &["add", "."]);
    git(fixture.path(), &["commit", "-m", "second"]);

    let error = IndexProjection::from_validation(&validated).unwrap_err();
    assert!(matches!(&error, mara_engine::IndexError::GitStateChanged));
    assert_eq!(error.command_code(), "git.precondition");
}

#[test]
fn git_provenance_detects_changes_hidden_by_index_flags() {
    let fixture = tempfile::tempdir().unwrap();
    let project_root = fixture.path().join("nested");
    let content_path = "nested/docs/project.mara.md";
    git(fixture.path(), &["init", "-b", "main"]);
    git(fixture.path(), &["config", "user.name", "Mara Test"]);
    git(
        fixture.path(),
        &["config", "user.email", "mara@example.test"],
    );
    write_project(&project_root, false);
    git(fixture.path(), &["add", "."]);
    git(fixture.path(), &["commit", "-m", "fixture"]);

    git(
        fixture.path(),
        &["update-index", "--assume-unchanged", content_path],
    );
    let assumed_clean = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(json(&assumed_clean)["git"]["dirty"], false);
    fs::write(
        project_root.join("docs/project.mara.md"),
        format!("{CONTENT}\n"),
    )
    .unwrap();
    let assumed = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(json(&assumed)["git"]["dirty"], true);
    fs::write(project_root.join("docs/project.mara.md"), CONTENT).unwrap();
    git(
        fixture.path(),
        &["update-index", "--no-assume-unchanged", content_path],
    );

    git(
        fixture.path(),
        &["update-index", "--skip-worktree", content_path],
    );
    let skipped_clean = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(json(&skipped_clean)["git"]["dirty"], false);
    fs::write(
        project_root.join("docs/project.mara.md"),
        format!("{CONTENT}\n"),
    )
    .unwrap();
    let skipped = IndexProjection::from_validation(&check_project(&project_root).unwrap())
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(json(&skipped)["git"]["dirty"], true);
}

#[test]
fn writer_enforces_the_configured_clean_relevant_inputs_policy() {
    let fixture = tempfile::tempdir().unwrap();
    let project_root = fixture.path().join("nested");
    git(fixture.path(), &["init", "-b", "main"]);
    git(fixture.path(), &["config", "user.name", "Mara Test"]);
    git(
        fixture.path(),
        &["config", "user.email", "mara@example.test"],
    );
    write_project(&project_root, false);
    git(fixture.path(), &["add", "."]);
    git(fixture.path(), &["commit", "-m", "fixture"]);

    let destination = project_root.join(".mara/index.json");
    let previous = b"previous complete index\n";
    fs::write(&destination, previous).unwrap();
    fs::write(
        project_root.join("docs/project.mara.md"),
        format!("{CONTENT}\n"),
    )
    .unwrap();
    let dirty = check_project(&project_root).unwrap();

    let error = write_index(&dirty).unwrap_err();

    assert!(matches!(error, mara_engine::IndexError::DirtyWorktree));
    assert_eq!(error.command_code(), "git.precondition");
    assert_eq!(fs::read(&destination).unwrap(), previous);

    let config_path = project_root.join(".mara/project.toml");
    let config = fs::read_to_string(&config_path).unwrap().replace(
        "require_clean_worktree_for_writes = true",
        "require_clean_worktree_for_writes = false",
    );
    fs::write(config_path, config).unwrap();
    let allowed = check_project(&project_root).unwrap();
    let written = write_index(&allowed).unwrap();
    assert_ne!(
        fs::read(project_root.join(written.path())).unwrap(),
        previous
    );
}

#[test]
fn writer_rejects_stale_validation_preimages_outside_git() {
    let fixture = tempfile::tempdir().unwrap();
    write_project(fixture.path(), false);
    let validated = check_project(fixture.path()).unwrap();
    let destination = fixture.path().join(".mara/index.json");
    let previous = b"previous complete index\n";
    fs::write(&destination, previous).unwrap();
    fs::write(
        fixture.path().join("docs/project.mara.md"),
        format!("{CONTENT}\n"),
    )
    .unwrap();

    let error = write_index(&validated).unwrap_err();

    assert!(matches!(
        &error,
        mara_engine::IndexError::InputStateChanged { .. }
    ));
    assert_eq!(error.command_code(), "io.failed");
    assert_eq!(
        error
            .path()
            .unwrap()
            .strip_prefix(fixture.path().canonicalize().unwrap())
            .unwrap(),
        Path::new("docs/project.mara.md")
    );
    assert_eq!(fs::read(&destination).unwrap(), previous);
    assert!(
        fs::read_dir(fixture.path().join(".mara"))
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".mara-"))
    );
}

#[test]
fn writer_rejects_a_changed_selected_content_set() {
    let fixture = tempfile::tempdir().unwrap();
    write_project(fixture.path(), false);
    let validated = check_project(fixture.path()).unwrap();
    let destination = fixture.path().join(".mara/index.json");
    let previous = b"previous complete index\n";
    fs::write(&destination, previous).unwrap();
    fs::write(
        fixture.path().join("docs/z-added.mara.md"),
        "New narrative input.\n",
    )
    .unwrap();

    let error = write_index(&validated).unwrap_err();

    assert!(matches!(
        &error,
        mara_engine::IndexError::InputStateChanged { .. }
    ));
    assert_eq!(fs::read(&destination).unwrap(), previous);
}
