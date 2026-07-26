use std::{fs, process::Command};

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

fn mara() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mara"))
}

fn project(content: &str) -> tempfile::TempDir {
    project_with_schema(SCHEMA, content)
}

fn project_with_schema(schema: &str, content: &str) -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("create isolated CLI fixture");
    fs::create_dir_all(temp.path().join(".mara")).unwrap();
    fs::create_dir_all(temp.path().join("docs")).unwrap();
    fs::write(temp.path().join(".mara/project.toml"), PROJECT_CONFIG).unwrap();
    fs::write(temp.path().join(".mara/schema.yaml"), schema).unwrap();
    fs::write(temp.path().join("docs/items.mara.md"), content).unwrap();
    temp
}

fn run(root: &std::path::Path, args: &[&str]) -> std::process::Output {
    mara()
        .current_dir(root)
        .args(args)
        .output()
        .expect("run mara command")
}

fn json(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("command emits one JSON envelope")
}

#[test]
fn init_creates_a_valid_process_neutral_project_that_check_accepts() {
    let temp = tempfile::tempdir().expect("create isolated CLI fixture");
    let root = temp.path().join("project");

    let init = mara()
        .args(["init", root.to_str().unwrap(), "--name", "fixture"])
        .output()
        .expect("run mara init");

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

    let check = mara()
        .current_dir(&root)
        .args(["check", "--format", "json"])
        .output()
        .expect("run mara check");

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
    let generated = run(fixture.path(), &["mid"]);
    assert_eq!(generated.status.code(), Some(0));
    assert!(generated.stderr.is_empty());
    let mid = String::from_utf8(generated.stdout).unwrap();
    assert_eq!(mid.len(), 29);
    assert!(mid.starts_with("m_"));
    assert!(mid.ends_with('\n'));

    let outside = tempfile::tempdir().unwrap();
    let failed = run(outside.path(), &["mid"]);
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

    let human = run(fixture.path(), &["check"]);
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stderr.is_empty());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains(
        "docs/items.mara.md:6:1: error[reference.unresolved]: internal reference does not resolve to an active item"
    ), "actual human diagnostics:\n{human}");

    let machine = run(fixture.path(), &["check", "--format", "json"]);
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
    let malformed = tempfile::tempdir().unwrap();
    fs::create_dir_all(malformed.path().join(".mara")).unwrap();
    fs::write(
        malformed.path().join(".mara/project.toml"),
        format!("{PROJECT_CONFIG}\nunknown = true\n"),
    )
    .unwrap();
    fs::write(malformed.path().join(".mara/schema.yaml"), SCHEMA).unwrap();

    let invalid = run(malformed.path(), &["schema", "check", "--format", "json"]);
    assert_eq!(invalid.status.code(), Some(1));
    let envelope = json(&invalid);
    assert_eq!(envelope["status"], "invalid");
    assert_eq!(envelope["diagnostics"][0]["code"], "config.unknown_key");
    assert_eq!(
        envelope["diagnostics"][0]["primary"]["path"],
        ".mara/project.toml"
    );

    let missing = tempfile::tempdir().unwrap();
    let failed = run(missing.path(), &["check", "--format", "json"]);
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

    let listed = run(fixture.path(), &["list"]);
    assert_eq!(listed.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(listed.stdout).unwrap(),
        include_str!("golden/list.txt")
    );

    let filtered = run(
        fixture.path(),
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

    let shown = run(fixture.path(), &["show", "ALPHA-A", "--format", "json"]);
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
        fixture.path(),
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
    let temp = tempfile::tempdir().unwrap();
    let first = run(temp.path(), &["init", "--name", "fixture"]);
    assert_eq!(first.status.code(), Some(0));
    let second = run(temp.path(), &["init", "--name", "fixture"]);
    assert_eq!(second.status.code(), Some(2));

    let fixture = project(VALID_ITEMS);
    let trace = run(fixture.path(), &["trace", "ALPHA-A", "--depth", "0"]);
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
        let output = run(fixture.path(), &command);
        assert_eq!(output.status.code(), Some(1));
        let envelope = json(&output);
        assert_eq!(envelope["status"], "invalid");
        assert_eq!(envelope["diagnostics"][0]["code"], "reference.unresolved");
        assert!(envelope["data"].is_null());
        assert!(envelope["error"].is_null());
    }

    let invalid_filter = run(
        fixture.path(),
        &["list", "--field", "state=unknown", "--format", "json"],
    );
    assert_eq!(invalid_filter.status.code(), Some(1));
    assert_eq!(
        json(&invalid_filter)["diagnostics"][0]["code"],
        "field.invalid_scalar"
    );

    let unknown_flavour = run(
        fixture.path(),
        &["list", "--flavour", "unknown", "--format", "json"],
    );
    assert_eq!(unknown_flavour.status.code(), Some(1));
    assert_eq!(
        json(&unknown_flavour)["diagnostics"][0]["code"],
        "item.unknown_flavour"
    );

    let first = run(fixture.path(), &["check", "--format", "json"]);
    let second = run(fixture.path(), &["check", "--format", "json"]);
    assert_eq!(first.stdout, second.stdout);
}

#[test]
fn every_repeated_field_filter_value_must_convert() {
    let fixture = project(VALID_ITEMS);
    let output = run(
        fixture.path(),
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
        let output = run(fixture.path(), &arguments);
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
        let output = run(fixture.path(), &arguments);
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
    let output = run(
        fixture.path(),
        &["trace", "BETA-B", "--direction", "incoming"],
    );

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
    let output = run(fixture.path(), &["check", "--format", "json"]);

    assert_eq!(output.status.code(), Some(0));
    let envelope = json(&output);
    assert_eq!(envelope["data"]["summary"]["mentions"], 1);
    assert_eq!(envelope["data"]["summary"]["external_nodes"], 1);
}

#[test]
fn transaction_recovery_requires_exactly_one_explicit_mode() {
    let fixture = project(VALID_ITEMS);
    for arguments in [
        vec!["transaction", "recover"],
        vec!["transaction", "recover", "--rollback", "--complete"],
    ] {
        let output = run(fixture.path(), &arguments);
        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("--rollback") || stderr.contains("--complete"));
    }
}
