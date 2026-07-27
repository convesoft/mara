use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use mara_test_support::{ProjectSandbox, ProjectSandboxMode};

const PROJECT_CONFIG: &str = r#"format_version = 1
[project]
name = "fixture"
schema = ".mara/schema.yaml"
[content]
include = ["**/*.mara.md"]
exclude = []
respect_gitignore = true
follow_directory_symlinks = false
allow_internal_file_symlinks = true
[index]
path = ".mara/index.json"
[validation]
warnings_as_errors = false
[git]
require_clean_worktree_for_writes = true
"#;

const SCHEMA: &str = r#"format_version: 1
schema:
  name: fixture
  version: 1.0.0
identity:
  mid:
    format: ulid
    prefix: m_
flavours:
  alpha:
    label: Alpha
    description: Fixture source.
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
  beta:
    label: Beta
    description: Fixture target.
    guidance:
      use_when: [Testing target items.]
      avoid_when: [Testing source items.]
    id: {}
    title: {}
    body: {}
relations:
  connects:
    source:
      flavours: [alpha]
    target:
      flavours: [beta]
rules: []
"#;

const VALID_ITEMS: &str = r#":::alpha m_00000000000000000000000001
:id: ALPHA-A
:title: First
:state: active
:tag: red
:connects: BETA-B

Alpha body.
:::

:::beta m_00000000000000000000000002
:id: BETA-B
:title: Second

Beta body.
:::
"#;

const DUPLICATE_DISPLAY_ID_ITEMS: &str = r#":::alpha m_00000000000000000000000003
:id: DUPLICATE
:title: First duplicate

First body.
:::

:::alpha m_00000000000000000000000004
:id: DUPLICATE
:title: Second duplicate

Second body.
:::
"#;

fn mara() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mara"))
}

fn project(content: &str) -> ProjectSandbox {
    project_with_schema(SCHEMA, content)
}

fn project_with_schema(schema: &str, content: &str) -> ProjectSandbox {
    let sandbox = ProjectSandbox::new(ProjectSandboxMode::Configured)
        .expect("create isolated CLI project sandbox");
    fs::create_dir_all(sandbox.path().join("docs")).unwrap();
    fs::write(sandbox.path().join(".mara/project.toml"), PROJECT_CONFIG).unwrap();
    fs::write(sandbox.path().join(".mara/schema.yaml"), schema).unwrap();
    fs::write(sandbox.path().join("docs/items.mara.md"), content).unwrap();
    sandbox
}

fn run(sandbox: &ProjectSandbox, args: &[&str]) -> std::process::Output {
    let mut command = mara();
    sandbox.configure_command(&mut command).args(args);
    command.output().expect("run mara command")
}

fn run_at_path(root: &Path, args: &[&str]) -> std::process::Output {
    mara()
        .current_dir(root)
        .args(args)
        .output()
        .expect("run mara command")
}

fn git(sandbox: &ProjectSandbox, args: &[&str]) -> std::process::Output {
    let mut command = Command::new("git");
    sandbox.configure_command(&mut command).args(args);
    command.output().expect("run Git command")
}

fn initialize_git_repository(sandbox: &ProjectSandbox) {
    for arguments in [
        &["init", "--quiet"][..],
        &["config", "user.email", "mara-cli@example.invalid"],
        &["config", "user.name", "Mara CLI Test"],
        &["add", "."],
        &["commit", "--quiet", "-m", "test: initialize fixture"],
    ] {
        let output = git(sandbox, arguments);
        assert!(
            output.status.success(),
            "Git command {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn create_incomplete_transaction(root: &Path) -> &'static str {
    let transaction = "tx_00000000000000000000000000";
    let directory = root.join(".mara/transactions").join(transaction);
    fs::create_dir_all(&directory).unwrap();
    let prefix = format!("docs/.mara-{transaction}-000000");
    let journal = serde_json::json!({
        "format": "mara.transaction",
        "version": 1,
        "id": transaction,
        "operation": "display_id_rename",
        "phase": "preparing",
        "outcome": null,
        "allow_dirty": false,
        "source_mid": "m_00000000000000000000000002",
        "old_id": "BETA-B",
        "new_id": "BETA-RENAMED",
        "files": [{
            "ordinal": 0,
            "path": "docs/items.mara.md",
            "file_identity": "fixture-file",
            "original_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "replacement_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "original_size": 0,
            "replacement_size": 0,
            "readonly": false,
            "unix_mode": null,
            "stage_path": format!("{prefix}.stage"),
            "stage_identity": null,
            "backup_path": format!("{prefix}.backup"),
            "backup_identity": null,
            "state": "declared"
        }]
    });
    fs::write(
        directory.join("journal.json"),
        serde_json::to_vec_pretty(&journal).unwrap(),
    )
    .unwrap();
    transaction
}

fn json(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("command emits one JSON envelope")
}

fn repository_mara_documents(root: &Path) -> Vec<PathBuf> {
    let mut directories = vec![root.join("docs")];
    let mut documents = Vec::new();

    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("read repository docs entry").path();
            if path.is_dir() {
                directories.push(path);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".mara.md"))
            {
                documents.push(path);
            }
        }
    }

    documents.sort();
    documents
}

#[test]
fn self_hosting_acceptance_validates_the_repository_deterministically() {
    // TEST-VERIFICATION-STRATEGY and REQ-SELF-HOSTING-GATE.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve repository root");
    let documents = repository_mara_documents(&root);
    assert!(!documents.is_empty(), "repository Mara corpus is empty");

    let first = run_at_path(&root, &["check", "--format", "json"]);
    let second = run_at_path(&root, &["check", "--format", "json"]);

    assert_eq!(first.status.code(), Some(0));
    assert_eq!(second.status.code(), Some(0));
    assert!(first.stderr.is_empty());
    assert!(second.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);

    let output = json(&first);
    assert_eq!(output["status"], "ok");
    assert_eq!(output["diagnostics"], serde_json::json!([]));
    assert_eq!(
        output["data"]["summary"]["documents"],
        serde_json::json!(documents.len()),
        "repository Mara corpus: {documents:#?}"
    );
    assert_eq!(output["data"]["summary"]["errors"], 0);
    assert_eq!(output["data"]["summary"]["warnings"], 0);
}

#[test]
fn self_hosting_negative_fixture_rejects_duplicate_display_ids() {
    // TEST-VERIFICATION-STRATEGY proves the acceptance gate detects a known defect.
    let temp = project(DUPLICATE_DISPLAY_ID_ITEMS);
    let output = run(&temp, &["check", "--format", "json"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let output = json(&output);
    assert_eq!(output["status"], "invalid");
    assert_eq!(
        output["diagnostics"][0]["code"],
        "identity.duplicate_display_id"
    );
}

#[test]
fn init_creates_a_valid_process_neutral_project_that_check_accepts() {
    let temp = ProjectSandbox::new(ProjectSandboxMode::Empty)
        .expect("create isolated empty CLI project sandbox");
    let root = temp.path().to_path_buf();

    let init = run(
        &temp,
        &["init", root.to_str().unwrap(), "--name", "fixture"],
    );

    assert_eq!(init.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(init.stdout).unwrap(),
        "created .mara/project.toml\ncreated .mara/schema.yaml\n"
    );
    assert!(init.stderr.is_empty());
    assert!(root.join(".mara/project.toml").is_file());
    assert!(root.join(".mara/schema.yaml").is_file());
    assert!(
        fs::read_to_string(root.join(".mara/schema.yaml"))
            .unwrap()
            .contains("flavours: {}\nrelations: {}\nrules: []\n")
    );

    let check = run(&temp, &["check", "--format", "json"]);

    assert_eq!(check.status.code(), Some(0));
    assert!(check.stderr.is_empty());
    let envelope: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(envelope["format"], "mara.command");
    assert_eq!(envelope["version"], 1);
    assert_eq!(envelope["command"], "check");
    assert_eq!(envelope["status"], "ok");
    assert_eq!(envelope["data"]["summary"]["items"], 0);
}

#[test]
fn mid_uses_project_identity_and_fails_without_a_project() {
    let fixture = project(VALID_ITEMS);
    let generated = run(&fixture, &["mid"]);
    assert_eq!(generated.status.code(), Some(0));
    assert!(generated.stderr.is_empty());
    let mid = String::from_utf8(generated.stdout).unwrap();
    assert_eq!(mid.len(), 29);
    assert!(mid.starts_with("m_"));
    assert!(mid.ends_with('\n'));

    let outside = ProjectSandbox::new(ProjectSandboxMode::Empty).unwrap();
    let failed = run(&outside, &["mid"]);
    assert_eq!(failed.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(failed.stdout).unwrap(),
        "error[project.unavailable]: a valid Mara project and schema are required to generate a MID\n"
    );
}

#[test]
fn check_reports_validation_findings_with_actionable_human_and_json_locations() {
    let invalid = VALID_ITEMS.replace(":connects: BETA-B", ":connects: MISSING");
    let fixture = project(&invalid);

    let human = run(&fixture, &["check"]);
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stderr.is_empty());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains(
        "docs/items.mara.md:6:1: error[reference.unresolved]: internal reference does not resolve to an active item"
    ), "actual human diagnostics:\n{human}");

    let machine = run(&fixture, &["check", "--format", "json"]);
    assert_eq!(machine.status.code(), Some(1));
    assert!(machine.stderr.is_empty());
    let envelope = json(&machine);
    assert_eq!(envelope["status"], "invalid");
    assert!(envelope["data"].is_null());
    assert!(envelope["error"].is_null());
    assert_eq!(envelope["diagnostics"][0]["code"], "reference.unresolved");
    assert_eq!(
        envelope["diagnostics"][0]["primary"]["path"],
        "docs/items.mara.md"
    );
}

#[test]
fn malformed_configuration_is_invalid_but_missing_project_is_operational() {
    let malformed = ProjectSandbox::new(ProjectSandboxMode::Configured).unwrap();
    fs::write(
        malformed.path().join(".mara/project.toml"),
        format!("{PROJECT_CONFIG}\nunknown = true\n"),
    )
    .unwrap();
    fs::write(malformed.path().join(".mara/schema.yaml"), SCHEMA).unwrap();

    let invalid = run(&malformed, &["schema", "check", "--format", "json"]);
    assert_eq!(invalid.status.code(), Some(1));
    let envelope = json(&invalid);
    assert_eq!(envelope["status"], "invalid");
    assert_eq!(envelope["diagnostics"][0]["code"], "config.unknown_key");
    assert_eq!(
        envelope["diagnostics"][0]["primary"]["path"],
        ".mara/project.toml"
    );

    let missing = ProjectSandbox::new(ProjectSandboxMode::Empty).unwrap();
    let failed = run(&missing, &["check", "--format", "json"]);
    assert_eq!(failed.status.code(), Some(2));
    assert!(failed.stderr.is_empty());
    let envelope = json(&failed);
    assert_eq!(envelope["status"], "failed");
    assert!(envelope["data"].is_null());
    assert_eq!(envelope["error"]["code"], "project.unavailable");
    assert!(
        !String::from_utf8(failed.stdout)
            .unwrap()
            .contains(missing.path().to_str().unwrap())
    );
}

#[test]
fn list_show_and_trace_use_the_shared_deterministic_model() {
    let fixture = project(VALID_ITEMS);

    let listed = run(&fixture, &["list"]);
    assert_eq!(listed.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(listed.stdout).unwrap(),
        include_str!("golden/list.txt")
    );

    let filtered = run(
        &fixture,
        &[
            "list",
            "--format",
            "json",
            "--flavour",
            "alpha",
            "--field",
            "state=active",
        ],
    );
    assert_eq!(filtered.status.code(), Some(0));
    let envelope = json(&filtered);
    assert_eq!(
        envelope["data"]["filters"]["flavours"],
        serde_json::json!(["alpha"])
    );
    assert_eq!(envelope["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        envelope["data"]["items"][0]["mid"],
        "m_00000000000000000000000001"
    );

    let shown = run(&fixture, &["show", "ALPHA-A", "--format", "json"]);
    assert_eq!(shown.status.code(), Some(0));
    let raw_show = String::from_utf8(shown.stdout.clone()).unwrap();
    let envelope = json(&shown);
    assert_eq!(
        envelope["data"]["item"]["mid"],
        "m_00000000000000000000000001"
    );
    assert_eq!(envelope["data"]["item"]["body_markdown"], "Alpha body.\n");
    assert_eq!(
        envelope["data"]["item"]["outgoing"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(raw_show.find("\"mid\"").unwrap() < raw_show.find("\"body_markdown\"").unwrap());
    assert!(raw_show.find("\"body_markdown\"").unwrap() < raw_show.find("\"metadata\"").unwrap());

    let traced = run(
        &fixture,
        &[
            "trace",
            "ALPHA-A",
            "--direction",
            "outgoing",
            "--depth",
            "1",
            "--format",
            "json",
        ],
    );
    assert_eq!(traced.status.code(), Some(0));
    let envelope = json(&traced);
    assert_eq!(envelope["data"]["paths"].as_array().unwrap().len(), 1);
    assert_eq!(
        envelope["data"]["paths"][0]["edges"][0]["relation"],
        "connects"
    );
    assert_eq!(
        envelope["data"]["paths"][0]["edges"][0]["traversal"],
        "outgoing"
    );
}

#[test]
fn init_refuses_overwrite_and_trace_rejects_zero_depth_with_exit_two() {
    let temp = ProjectSandbox::new(ProjectSandboxMode::Empty).unwrap();
    let first = run(&temp, &["init", "--name", "fixture"]);
    assert_eq!(first.status.code(), Some(0));
    let second = run(&temp, &["init", "--name", "fixture"]);
    assert_eq!(second.status.code(), Some(2));

    let fixture = project(VALID_ITEMS);
    let trace = run(&fixture, &["trace", "ALPHA-A", "--depth", "0"]);
    assert_eq!(trace.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(trace.stdout).unwrap(),
        "error[cli.invalid_arguments]: trace depth must be positive\n"
    );
}

#[test]
fn query_input_findings_are_invalid_and_json_is_repeatable() {
    let fixture = project(VALID_ITEMS);

    for command in [
        vec!["show", "UNKNOWN", "--format", "json"],
        vec!["trace", "UNKNOWN", "--format", "json"],
    ] {
        let output = run(&fixture, &command);
        assert_eq!(output.status.code(), Some(1));
        let envelope = json(&output);
        assert_eq!(envelope["status"], "invalid");
        assert_eq!(envelope["diagnostics"][0]["code"], "reference.unresolved");
        assert!(envelope["data"].is_null());
        assert!(envelope["error"].is_null());
    }

    let invalid_filter = run(
        &fixture,
        &["list", "--field", "state=unknown", "--format", "json"],
    );
    assert_eq!(invalid_filter.status.code(), Some(1));
    assert_eq!(
        json(&invalid_filter)["diagnostics"][0]["code"],
        "field.invalid_scalar"
    );

    let unknown_flavour = run(
        &fixture,
        &["list", "--flavour", "unknown", "--format", "json"],
    );
    assert_eq!(unknown_flavour.status.code(), Some(1));
    assert_eq!(
        json(&unknown_flavour)["diagnostics"][0]["code"],
        "item.unknown_flavour"
    );

    let first = run(&fixture, &["check", "--format", "json"]);
    let second = run(&fixture, &["check", "--format", "json"]);
    assert_eq!(first.stdout, second.stdout);
}

#[test]
fn every_repeated_field_filter_value_must_convert() {
    let fixture = project(VALID_ITEMS);
    let output = run(
        &fixture,
        &[
            "list",
            "--field",
            "state=active",
            "--field",
            "state=typo",
            "--format",
            "json",
        ],
    );

    assert_eq!(output.status.code(), Some(1));
    let envelope = json(&output);
    assert_eq!(envelope["status"], "invalid");
    assert_eq!(envelope["diagnostics"][0]["code"], "field.invalid_scalar");
    assert_eq!(envelope["diagnostics"][0]["context"]["field"], "state");
    assert_eq!(envelope["diagnostics"][0]["details"]["value"], "typo");
}

#[test]
fn non_failing_warnings_do_not_suppress_successful_human_payloads() {
    let content = VALID_ITEMS.replace(":connects: BETA-B", ":connects: BETA-B\n:connects: BETA-B");
    let fixture = project(&content);
    let cases = [
        (vec!["check"], "ok: 1 documents, 2 items, 1 edges"),
        (
            vec!["list"],
            "m_00000000000000000000000001\tALPHA-A\talpha\tFirst",
        ),
        (vec!["show", "ALPHA-A"], "Alpha body."),
        (
            vec!["trace", "ALPHA-A"],
            "focus: m_00000000000000000000000001",
        ),
    ];

    for (arguments, requested_data) in cases {
        let output = run(&fixture, &arguments);
        assert_eq!(output.status.code(), Some(0), "arguments: {arguments:?}");
        let human = String::from_utf8(output.stdout).unwrap();
        assert!(human.contains(requested_data), "actual output:\n{human}");
        assert!(
            human.contains("warning[relation.duplicate_occurrence]"),
            "actual output:\n{human}"
        );
    }
}

#[test]
fn duplicated_display_ids_are_command_specific_ambiguous_query_findings() {
    let content = VALID_ITEMS
        .replace(
            ":connects: BETA-B",
            ":connects: m_00000000000000000000000002",
        )
        .replace(":id: BETA-B", ":id: ALPHA-A");
    let fixture = project(&content);

    for arguments in [
        vec!["show", "ALPHA-A", "--format", "json"],
        vec!["trace", "ALPHA-A", "--format", "json"],
    ] {
        let output = run(&fixture, &arguments);
        assert_eq!(output.status.code(), Some(1));
        let envelope = json(&output);
        assert_eq!(envelope["diagnostics"].as_array().unwrap().len(), 1);
        assert_eq!(envelope["diagnostics"][0]["code"], "reference.ambiguous");
        assert_eq!(
            envelope["diagnostics"][0]["details"]["candidate_mids"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            envelope["diagnostics"][0]["related"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
}

#[test]
fn incoming_human_trace_renders_the_traversed_source_endpoint() {
    let fixture = project(VALID_ITEMS);
    let output = run(&fixture, &["trace", "BETA-B", "--direction", "incoming"]);

    assert_eq!(output.status.code(), Some(0));
    let human = String::from_utf8(output.stdout).unwrap();
    assert!(human.contains("--connects:incoming--> m_00000000000000000000000001"));
}

#[test]
fn summary_counts_bare_external_mentions_and_external_nodes() {
    let schema = SCHEMA.replace(
        "rules: []",
        "  external_link:\n    source:\n      flavours: [alpha]\n    target:\n      external: [https]\nrules: []",
    );
    let content = VALID_ITEMS.replace(
        "Alpha body.",
        "Alpha body with [[https://example.test/work]].",
    );
    let fixture = project_with_schema(&schema, &content);
    let output = run(&fixture, &["check", "--format", "json"]);

    assert_eq!(output.status.code(), Some(0));
    let envelope = json(&output);
    assert_eq!(envelope["data"]["summary"]["mentions"], 1);
    assert_eq!(envelope["data"]["summary"]["external_nodes"], 1);
}

#[test]
fn index_writes_the_configured_projection_and_reports_stable_evidence() {
    let fixture = project(VALID_ITEMS);
    let first = run(&fixture, &["index", "--format", "json"]);

    assert_eq!(first.status.code(), Some(0));
    let first_envelope = json(&first);
    assert_eq!(first_envelope["command"], "index");
    assert_eq!(first_envelope["status"], "ok");
    assert_eq!(first_envelope["data"]["path"], ".mara/index.json");
    assert_eq!(first_envelope["data"]["sha256"].as_str().unwrap().len(), 64);
    assert_eq!(first_envelope["data"]["summary"]["documents"], 1);
    assert_eq!(first_envelope["data"]["summary"]["items"], 2);
    assert_eq!(first_envelope["data"]["summary"]["edges"], 1);
    assert!(first_envelope["error"].is_null());

    let path = fixture.path().join(".mara/index.json");
    let first_index = fs::read(&path).unwrap();
    let projection: serde_json::Value = serde_json::from_slice(&first_index).unwrap();
    assert_eq!(projection["format"], "mara.index");
    assert_eq!(projection["version"], 1);
    assert_eq!(projection["items"].as_array().unwrap().len(), 2);

    let second = run(&fixture, &["index", "--format", "json"]);
    assert_eq!(second.status.code(), Some(0));
    assert_eq!(
        json(&second)["data"]["sha256"],
        first_envelope["data"]["sha256"]
    );
    assert_eq!(fs::read(&path).unwrap(), first_index);

    let human = run(&fixture, &["index"]);
    assert_eq!(human.status.code(), Some(0));
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .starts_with("wrote .mara/index.json (")
    );
}

#[test]
fn index_preserves_the_previous_file_when_validation_policy_fails() {
    let warning_content =
        VALID_ITEMS.replace(":connects: BETA-B", ":connects: BETA-B\n:connects: BETA-B");
    let warning_fixture = project(&warning_content);
    let warning = run(&warning_fixture, &["index", "--format", "json"]);
    assert_eq!(warning.status.code(), Some(0));
    let warning_envelope = json(&warning);
    assert_eq!(
        warning_envelope["diagnostics"][0]["code"],
        "relation.duplicate_occurrence"
    );
    let warning_projection: serde_json::Value =
        serde_json::from_slice(&fs::read(warning_fixture.path().join(".mara/index.json")).unwrap())
            .unwrap();
    assert_eq!(
        warning_projection["diagnostics"][0]["code"],
        "relation.duplicate_occurrence"
    );

    let escalated_fixture = project(&warning_content);
    fs::write(
        escalated_fixture.path().join(".mara/project.toml"),
        PROJECT_CONFIG.replace("warnings_as_errors = false", "warnings_as_errors = true"),
    )
    .unwrap();
    let escalated_path = escalated_fixture.path().join(".mara/index.json");
    fs::write(&escalated_path, b"previous index\n").unwrap();
    let escalated = run(&escalated_fixture, &["index", "--format", "json"]);
    assert_eq!(escalated.status.code(), Some(1));
    let escalated_envelope = json(&escalated);
    assert_eq!(escalated_envelope["status"], "invalid");
    assert!(escalated_envelope["data"].is_null());
    assert!(escalated_envelope["error"].is_null());
    assert_eq!(fs::read(&escalated_path).unwrap(), b"previous index\n");

    fs::write(
        escalated_fixture.path().join("docs/items.mara.md"),
        VALID_ITEMS,
    )
    .unwrap();
    let rebuilt = run(&escalated_fixture, &["index", "--format", "json"]);
    assert_eq!(rebuilt.status.code(), Some(0));
    assert_eq!(json(&rebuilt)["status"], "ok");
    let rebuilt_index = fs::read(&escalated_path).unwrap();
    assert_ne!(rebuilt_index, b"previous index\n");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&rebuilt_index).unwrap()["format"],
        "mara.index"
    );

    let invalid_fixture = project(DUPLICATE_DISPLAY_ID_ITEMS);
    let invalid_path = invalid_fixture.path().join(".mara/index.json");
    fs::write(&invalid_path, b"previous index\n").unwrap();
    let invalid = run(&invalid_fixture, &["index", "--format", "json"]);
    assert_eq!(invalid.status.code(), Some(1));
    assert_eq!(json(&invalid)["status"], "invalid");
    assert_eq!(fs::read(&invalid_path).unwrap(), b"previous index\n");
}

#[test]
fn transaction_recovery_requires_exactly_one_explicit_mode() {
    let fixture = project(VALID_ITEMS);
    for arguments in [
        vec!["transaction", "recover"],
        vec!["transaction", "recover", "--rollback", "--complete"],
    ] {
        let output = run(&fixture, &arguments);
        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("--rollback") || stderr.contains("--complete"));
    }
}

#[test]
fn display_id_rename_reports_success_and_rewrites_the_project() {
    // TEST-DISPLAY-ID and TEST-EDIT-PREFLIGHT.
    let fixture = project(VALID_ITEMS);
    let output = run(&fixture, &["id", "rename", "BETA-B", "BETA-RENAMED"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let human = String::from_utf8(output.stdout).unwrap();
    assert!(human.starts_with("renamed display ID BETA-B to BETA-RENAMED\ntransaction: tx_"));
    assert!(human.ends_with("\nchanged: docs/items.mara.md\n"));
    let source = fs::read_to_string(fixture.path().join("docs/items.mara.md")).unwrap();
    assert!(!source.contains("BETA-B"));
    assert_eq!(source.matches("BETA-RENAMED").count(), 2);
}

#[test]
fn display_id_rename_rejects_an_invalid_replacement_without_writing() {
    // TEST-DISPLAY-ID and TEST-EDIT-PREFLIGHT.
    let schema = SCHEMA.replacen(
        "  beta:\n    label: Beta\n    description: Fixture target.\n    guidance:\n      use_when: [Testing target items.]\n      avoid_when: [Testing source items.]\n    id: {}",
        "  beta:\n    label: Beta\n    description: Fixture target.\n    guidance:\n      use_when: [Testing target items.]\n      avoid_when: [Testing source items.]\n    id:\n      pattern: BETA-[A-Z]+",
        1,
    );
    let fixture = project_with_schema(&schema, VALID_ITEMS);
    let before = fs::read(fixture.path().join("docs/items.mara.md")).unwrap();
    let output = run(&fixture, &["id", "rename", "BETA-B", "invalid"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "project validation failed: the complete renamed project is invalid\n"
    );
    assert_eq!(
        fs::read(fixture.path().join("docs/items.mara.md")).unwrap(),
        before
    );
}

#[test]
fn display_id_rename_rejects_a_duplicate_id_without_writing() {
    // TEST-DISPLAY-ID and REQ-DISPLAY-ID-RENAME.
    let fixture = project(VALID_ITEMS);
    let before = fs::read(fixture.path().join("docs/items.mara.md")).unwrap();
    let output = run(&fixture, &["id", "rename", "BETA-B", "ALPHA-A"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "invalid display-ID rename: display ID \"ALPHA-A\" is already in use\n"
    );
    assert_eq!(
        fs::read(fixture.path().join("docs/items.mara.md")).unwrap(),
        before
    );
}

#[test]
fn display_id_rename_rejects_a_dirty_worktree_by_default() {
    // TEST-EDIT-PREFLIGHT and REQ-EDIT-WORKTREE-POLICY.
    let fixture = project(VALID_ITEMS);
    initialize_git_repository(&fixture);
    fs::write(fixture.path().join("notes.txt"), "uncommitted\n").unwrap();
    let before = fs::read(fixture.path().join("docs/items.mara.md")).unwrap();
    let output = run(&fixture, &["id", "rename", "BETA-B", "BETA-RENAMED"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "Git worktree is not clean\n"
    );
    assert_eq!(
        fs::read(fixture.path().join("docs/items.mara.md")).unwrap(),
        before
    );
}

#[test]
fn display_id_rename_allows_a_dirty_worktree_only_when_explicit() {
    // TEST-EDIT-PREFLIGHT, REQ-EDIT-WORKTREE-POLICY, and REQ-EDIT-NO-COMMIT.
    let fixture = project(VALID_ITEMS);
    initialize_git_repository(&fixture);
    let head = git(&fixture, &["rev-parse", "HEAD"]).stdout;
    fs::write(fixture.path().join("notes.txt"), "uncommitted\n").unwrap();
    let output = run(
        &fixture,
        &["id", "rename", "BETA-B", "BETA-RENAMED", "--allow-dirty"],
    );

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(git(&fixture, &["rev-parse", "HEAD"]).stdout, head);
    assert_eq!(
        fs::read_to_string(fixture.path().join("notes.txt")).unwrap(),
        "uncommitted\n"
    );
    assert!(
        fs::read_to_string(fixture.path().join("docs/items.mara.md"))
            .unwrap()
            .contains("BETA-RENAMED")
    );
}

#[test]
fn display_id_rename_blocks_on_an_incomplete_transaction() {
    // TEST-EDIT-RECOVERY and REQ-EDIT-RECOVERY.
    let fixture = project(VALID_ITEMS);
    let transaction = create_incomplete_transaction(fixture.path());
    let before = fs::read(fixture.path().join("docs/items.mara.md")).unwrap();
    let output = run(&fixture, &["id", "rename", "BETA-B", "BETA-RENAMED"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!("transaction {transaction} requires explicit recovery\n")
    );
    assert_eq!(
        fs::read(fixture.path().join("docs/items.mara.md")).unwrap(),
        before
    );
}
