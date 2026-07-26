use std::{fs, path::Path, process::Command};

use mara_engine::{IndexProjection, check_project, write_index};

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
