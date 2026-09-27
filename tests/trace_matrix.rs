use serde_json::{Value, json};
use std::{fs, path::Path};
use tempfile::TempDir;
mod support;
use support::*;

fn validation_with_parity(current_directory: &Path, paths: &[&str]) -> Value {
    let mut args = vec!["--format", "json", "project", "validate"];
    for path in paths {
        args.extend(["--path", path]);
    }
    let output = mara(current_directory, &args);
    let result: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", stderr(&output)));
    assert_eq!(output.status.success(), result["valid"].as_bool().unwrap());
    let responses = mcp_exchange(
        current_directory,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "project_validate", json!({"paths":paths})),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        result
    );
    result
}

fn legacy_engineering_project(root: &Path) {
    let init = mara(root, &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        root.join(".mara/schema.yaml"),
        include_str!("fixtures/engineering-legacy-schema.yaml"),
    )
    .unwrap();
}

fn rule_fixture() -> TempDir {
    let fixture = fixture();
    let root = fixture.path();
    legacy_engineering_project(root);
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    for flavour in ["requirement", "verification", "design", "risk"] {
        schema["flavours"][flavour]["fields"] = json!({"status":{"type":"enum","values":["draft","approved","accepted","mitigated"]},"owner":{"type":"string"},"score":{"type":"number"}});
    }
    schema["relations"]["verifies"]["inverse"] = json!("verified_by");
    fs::write(schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    for (flavour, id, status) in [
        ("requirement", "REQ-A", "approved"),
        ("verification", "VER-DRAFT", "draft"),
        ("verification", "VER-APPROVED", "approved"),
        ("design", "DES-A", "accepted"),
        ("risk", "RISK-A", "mitigated"),
    ] {
        let out = mara(
            root,
            &[
                "item",
                "create",
                flavour,
                id,
                "items.mara.md",
                "--title",
                id,
                "--body",
                "A real item.",
                "--field",
                &format!("status={status}"),
                "--field",
                "owner=Alice",
            ],
        );
        assert!(out.status.success(), "{}", stderr(&out));
    }
    let config_path = root.join(".mara/project.toml");
    let config = fs::read_to_string(&config_path).unwrap().replacen(
        "format_version = 1",
        "format_version = 2",
        1,
    );
    fs::write(
        config_path,
        format!("{config}\n[rules]\nformat_version = 1\nfiles = [\"rules.yaml\"]\n"),
    )
    .unwrap();
    fs::write(
        root.join("rules.yaml"),
        include_str!("fixtures/validation-rules.yaml"),
    )
    .unwrap();
    fixture
}

fn relation_tool(root: &Path, name: &str, params: Value) -> Value {
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, name, params),
        ],
    );
    let response = &mcp_response(&responses, 2)["result"];
    assert!(
        response.get("structuredContent").is_some(),
        "{responses:#?}"
    );
    let value = response["structuredContent"].clone();
    assert_eq!(response["isError"], value.get("error").is_some());
    value
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_reports_rule_states_edges_and_cli_mcp_parity() {
    let fixture = rule_fixture();
    let root = fixture.path();
    fs::write(
        root.join("rules.yaml"),
        "\
- id: rule:coverage
  targetClass: requirement
  whenShape: rule:approved
  property:
    - id: rule:verification_step
      path: {inversePath: verifies}
      qualifiedValueShape: rule:approved_verification
      qualifiedMinCount: 1
- id: rule:approved
  property: [{path: status, hasValue: approved}]
- id: rule:approved_verification
  class: verification
  node: rule:approved
",
    )
    .unwrap();
    for (id, status) in [("REQ-MISSING", "approved"), ("REQ-DRAFT", "draft")] {
        let output = mara(
            root,
            &[
                "item",
                "create",
                "requirement",
                id,
                "items.mara.md",
                "--title",
                id,
                "--body",
                "A real requirement.",
                "--field",
                &format!("status={status}"),
            ],
        );
        assert!(output.status.success(), "{}", stderr(&output));
    }
    for verification in ["VER-DRAFT", "VER-APPROVED"] {
        let output = mara(
            root,
            &["relation", "add", verification, "verifies", "REQ-A"],
        );
        assert!(output.status.success(), "{}", stderr(&output));
    }
    let args = [
        "--format",
        "json",
        "trace",
        "matrix",
        "--flavour",
        "requirement",
        "--rule",
        "urn:mara:rule:coverage",
        "--limit",
        "100",
    ];
    let output = mara(root, &args);
    assert!(output.status.success(), "{}", stderr(&output));
    let result: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(result["evaluation_complete"], true, "{result:#}");
    assert_eq!(result["summaries"][0]["selected"], 3);
    assert_eq!(result["summaries"][0]["passed"], 1);
    assert_eq!(result["summaries"][0]["failed"], 1);
    assert_eq!(result["summaries"][0]["not_applicable"], 1);
    let records = result["records"].as_array().unwrap();
    assert!(records.iter().any(|r| r["kind"] == "check"
        && r["obligation"]["shape"] == "urn:mara:rule:coverage"
        && r["obligation"]["component"]
            == json!(["http://www.w3.org/ns/shacl#PropertyConstraintComponent"])));
    assert!(records.iter().any(|r| {
        r["kind"] == "check"
            && r["reported"]["obligation"]["component"].is_string()
            && r["obligation"]["component"]
                .as_array()
                .unwrap()
                .contains(&r["reported"]["obligation"]["component"])
    }));
    assert!(
        records.iter().any(|r| r["kind"] == "edge"
            && r["endpoint"]["id"] == "VER-DRAFT"
            && r["qualification"] == "failed"
            && r["outside_selection"] == true),
        "{result:#}"
    );
    assert!(
        records.iter().any(|r| r["kind"] == "check"
            && r["obligation"]["shape"] == "urn:mara:rule:approved_verification"
            && r["state"] == "failed"
            && r["obligation"]["component"]
                == json!([
                    "http://www.w3.org/ns/shacl#ClassConstraintComponent",
                    "http://www.w3.org/ns/shacl#NodeConstraintComponent"
                ])
            && r["inspection"].is_null()),
        "{result:#}"
    );
    assert!(
        records.iter().any(|r| r["kind"] == "check"
            && r["state"] == "failed"
            && r["condition"]["path"] == "status"
            && r["inspection"]["item_id"] == "VER-DRAFT"
            && r["inspection"]["item"].is_string()
            && r["inspection"]["value"] == "draft"
            && r["inspection"]["value_count"] == 1
            && r["obligation"]["component"]
                == json!(["http://www.w3.org/ns/shacl#HasValueConstraintComponent"])
            && r["inspection"]["source"]["line"].is_number()),
        "{result:#}"
    );
    assert!(
        records.iter().any(|r| r["kind"] == "edge"
            && r["endpoint"]["id"] == "VER-APPROVED"
            && r["qualification"] == "passed"),
        "{result:#}"
    );
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "trace_matrix",
                json!({"flavours":["requirement"],
            "rules":["urn:mara:rule:coverage"],"limit":100}),
            ),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        result
    );
    let text = mara(
        root,
        &[
            "trace",
            "matrix",
            "--flavour",
            "requirement",
            "--rule",
            "urn:mara:rule:coverage",
            "--limit",
            "100",
        ],
    );
    assert!(text.status.success(), "{}", stderr(&text));
    assert!(stdout(&text).contains("line "));
    assert!(stdout(&text).contains("qualifying"));
    assert!(stdout(&text).contains("outside root selection"));
    assert!(stdout(&text).contains("status = \"draft\""));
    let all = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--all",
            "--rule",
            "urn:mara:rule:coverage",
            "--limit",
            "100",
        ],
    );
    assert!(all.status.success(), "{}", stderr(&all));
    let all: Value = serde_json::from_str(&stdout(&all)).unwrap();
    assert!(
        all["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "edge"
                && r["endpoint"]["id"] == "VER-DRAFT"
                && r["outside_selection"] == false),
        "{all:#}"
    );
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[cfg(unix)]
#[test]
fn trace_matrix_reports_schema_read_failure_as_io_error_on_cli_and_mcp() {
    let fixture = rule_fixture();
    let root = fixture.path();
    fs::remove_file(root.join(".mara/schema.yaml")).unwrap();
    let cli = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--all",
            "--rule",
            "urn:mara:rule:coverage",
        ],
    );
    assert!(!cli.status.success());
    let result: Value = serde_json::from_str(&stdout(&cli)).unwrap();
    assert_eq!(result["error"]["code"], "io_error");
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "trace_matrix",
                json!({"all":true,"rules":["urn:mara:rule:coverage"]}),
            ),
        ],
    );
    let mcp = &mcp_response(&responses, 2)["result"];
    assert_eq!(mcp["structuredContent"]["error"]["code"], "io_error");
    assert_eq!(mcp["isError"], true);
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_uses_native_qualified_literal_counts() {
    let fixture = rule_fixture();
    let root = fixture.path();
    fs::write(root.join("rules.yaml"),
        "id: rule:owner_a\ntargetClass: requirement\nproperty: [{path: owner, qualifiedValueShape: {pattern: '^A'}, qualifiedMinCount: 1}]\n").unwrap();
    let result = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--rule",
            "urn:mara:rule:owner_a",
        ],
    );
    assert!(result.status.success(), "{}", stderr(&result));
    let result: Value = serde_json::from_str(&stdout(&result)).unwrap();
    assert_eq!(result["summaries"][0]["passed"], 1, "{result:#}");
    assert!(
        result["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "check"
                && r["condition"]["path"] == "owner"
                && r["counts"]["selected"] == 1
                && r["counts"]["qualifying"] == 1
                && r["obligation"]["component"]
                    == json!(["http://www.w3.org/ns/shacl#QualifiedMinCountConstraintComponent"])),
        "{result:#}"
    );
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_does_not_infer_every_for_literal_field_nodes() {
    let fixture = rule_fixture();
    let root = fixture.path();
    fs::write(
        root.join("rules.yaml"),
        "id: rule:owner_a\ntargetClass: requirement\nproperty: [{path: owner, node: {pattern: '^A'}}]\n",
    ).unwrap();
    let path = root.join("items.mara.md");
    let source = fs::read_to_string(&path).unwrap();
    fs::write(&path, source.replacen(":owner: Alice", ":owner: Bob", 1)).unwrap();
    let result = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--rule",
            "urn:mara:rule:owner_a",
        ],
    );
    assert!(result.status.success(), "{}", stderr(&result));
    let result: Value = serde_json::from_str(&stdout(&result)).unwrap();
    assert_eq!(result["summaries"][0]["failed"], 1, "{result:#}");
    assert!(
        result["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "check"
                && r["condition"]["path"] == "owner"
                && r["inspection"]["value"] == "Bob"
                && r["every"].is_null()),
        "{result:#}"
    );
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_reports_external_qualifier_outcome() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["relations"]["tracked_by"] = json!({
        "description":"Local or external verification.","source":["requirement"],
        "target":["verification"],"external":true
    });
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    fs::write(root.join("rules.yaml"),
        "id: rule:tracked\ntargetClass: requirement\nproperty: [{path: tracked_by, qualifiedValueShape: {class: verification}, qualifiedMinCount: 1}]\n").unwrap();
    assert!(
        mara(
            root,
            &["relation", "add", "REQ-A", "tracked_by", "VER-APPROVED"]
        )
        .status
        .success()
    );
    assert!(
        mara(
            root,
            &[
                "relation",
                "add",
                "REQ-A",
                "tracked_by",
                "external:https://example.com/ticket/1"
            ]
        )
        .status
        .success()
    );
    let result = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--rule",
            "urn:mara:rule:tracked",
        ],
    );
    assert!(result.status.success(), "{}", stderr(&result));
    let result: Value = serde_json::from_str(&stdout(&result)).unwrap();
    assert_eq!(result["summaries"][0]["passed"], 1, "{result:#}");
    assert!(
        result["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "edge"
                && r["endpoint"]["kind"] == "external"
                && r["qualification"] == "failed"),
        "{result:#}"
    );
    assert!(
        result["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "edge"
                && r["endpoint"]["id"] == "VER-APPROVED"
                && r["qualification"] == "passed"),
        "{result:#}"
    );
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_reports_code_endpoint_predicate_states() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["relations"]["code_implements"] = json!({
        "description":"Code implements a requirement.", "source":[],
        "target":["requirement"], "code_source":true,
        "inverse":"implemented_by_code"
    });
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/check.rs"), "fn check() {}\n").unwrap();
    let added = mara(
        root,
        &[
            "relation",
            "add",
            "REQ-A",
            "implemented_by_code",
            "code:src/check.rs",
        ],
    );
    assert!(added.status.success(), "{}", stderr(&added));
    fs::write(
        root.join("rules.yaml"),
        "id: rule:code_state\ntargetClass: requirement\nproperty:\n  - path: {inversePath: code_implements}\n    qualifiedValueShape: {pattern: 'urn:mara:code:'}\n    qualifiedMinCount: 1\n    node: {pattern: 'urn:mara:code:'}\n",
    )
    .unwrap();
    let output = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--rule",
            "urn:mara:rule:code_state",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let result: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(result["summaries"][0]["passed"], 1, "{result:#}");
    assert!(
        result["records"].as_array().unwrap().iter().any(|record| {
            record["kind"] == "edge"
                && record["endpoint"]["reference"] == "code:src/check.rs"
                && record["qualification"] == "passed"
                && record["every"] == "passed"
        }),
        "{result:#}"
    );
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_explains_skipped_evaluation_for_invalid_field() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let path = root.join("items.mara.md");
    let source = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        source.replacen(":status: approved", ":status: invalid", 1),
    )
    .unwrap();
    let output = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--rule",
            "urn:mara:rule:approved_requirement",
        ],
    );
    assert!(!output.status.success());
    let result: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(result["evaluation_complete"], false);
    assert!(
        result["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "issue" && r["diagnostic"]["code"] == "field_invalid"),
        "{result:#}"
    );
    assert!(
        result["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "issue"
                && r["diagnostic"]["code"] == "evaluation_unavailable"
                && r["diagnostic"]["scope"] == "project"),
        "{result:#}"
    );
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_cursor_detects_excluded_source_changes() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let args = [
        "--format",
        "json",
        "trace",
        "matrix",
        "--id",
        "REQ-A",
        "--rule",
        "urn:mara:rule:approved_requirement",
        "--limit",
        "1",
    ];
    let first = mara(root, &args);
    assert!(first.status.success(), "{}", stderr(&first));
    let first: Value = serde_json::from_str(&stdout(&first)).unwrap();
    let cursor = first["next_cursor"].as_str().unwrap();
    let excluded = root.join("excluded.mara.md");
    fs::write(&excluded, [0xff]).unwrap();
    let mut continued = args.to_vec();
    continued.extend(["--cursor", cursor]);
    let stale = mara(root, &continued);
    assert!(!stale.status.success());
    let stale: Value = serde_json::from_str(&stdout(&stale)).unwrap();
    assert_eq!(stale["error"]["code"], "stale_cursor");

    let invalid = mara(root, &args);
    assert!(!invalid.status.success());
    let invalid: Value = serde_json::from_str(&stdout(&invalid)).unwrap();
    let cursor = invalid["next_cursor"].as_str().unwrap();
    fs::write(&excluded, [0xfe]).unwrap();
    let mut continued = args.to_vec();
    continued.extend(["--cursor", cursor]);
    let stale = mara(root, &continued);
    assert!(!stale.status.success());
    let stale: Value = serde_json::from_str(&stdout(&stale)).unwrap();
    assert_eq!(stale["error"]["code"], "stale_cursor");
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_explains_second_hop_and_continues_without_changing_counts() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["flavours"]["evidence"]["fields"] =
        json!({"status":{"type":"enum","values":["draft","approved"]}});
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    fs::write(
        root.join("rules.yaml"),
        "\
- id: rule:evidenced_requirement
  targetClass: requirement
  property:
    - id: rule:verification_step
      path: {inversePath: verifies}
      qualifiedValueShape: rule:evidenced_verification
      qualifiedMinCount: 1
- id: rule:evidenced_verification
  class: verification
  property:
    - id: rule:evidence_step
      path: {inversePath: evidences}
      qualifiedValueShape: rule:approved_evidence
      qualifiedMinCount: 1
- id: rule:approved_evidence
  class: evidence
  property: [{path: status, hasValue: approved}]
",
    )
    .unwrap();
    assert!(
        mara(
            root,
            &["relation", "add", "VER-APPROVED", "verifies", "REQ-A"]
        )
        .status
        .success()
    );
    let created = mara(
        root,
        &[
            "item",
            "create",
            "evidence",
            "EVD-A",
            "items.mara.md",
            "--title",
            "Draft evidence",
            "--body",
            "A result.",
            "--field",
            "status=draft",
        ],
    );
    assert!(created.status.success(), "{}", stderr(&created));
    assert!(
        mara(
            root,
            &["relation", "add", "EVD-A", "evidences", "VER-APPROVED"]
        )
        .status
        .success()
    );
    let first = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--rule",
            "urn:mara:rule:evidenced_requirement",
            "--limit",
            "100",
        ],
    );
    assert!(first.status.success(), "{}", stderr(&first));
    let result: Value = serde_json::from_str(&stdout(&first)).unwrap();
    assert_eq!(result["summaries"][0]["failed"], 1, "{result:#}");
    assert!(
        result["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "check"
                && r["obligation"]["shape"] == "urn:mara:rule:evidence_step"
                && r["counts"]["selected"] == 1
                && r["counts"]["qualifying"] == 0),
        "{result:#}"
    );
    assert!(
        result["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "edge"
                && r["endpoint"]["id"] == "EVD-A"
                && r["qualification"] == "failed"),
        "{result:#}"
    );
    let mut cursor = None;
    let mut gathered = Vec::new();
    loop {
        let mut args = vec![
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--rule",
            "urn:mara:rule:evidenced_requirement",
            "--limit",
            "2",
        ];
        if let Some(value) = cursor.as_deref() {
            args.extend(["--cursor", value]);
        }
        let page = mara(root, &args);
        assert!(page.status.success(), "{}", stderr(&page));
        let page: Value = serde_json::from_str(&stdout(&page)).unwrap();
        assert_eq!(page["summaries"], result["summaries"]);
        gathered.extend(page["records"].as_array().unwrap().iter().cloned());
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(gathered, result["records"].as_array().unwrap().clone());
    let repeat = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--rule",
            "urn:mara:rule:evidenced_requirement",
            "--limit",
            "100",
        ],
    );
    assert_eq!(stdout(&repeat), stdout(&first));
    let initial = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--rule",
            "urn:mara:rule:evidenced_requirement",
            "--limit",
            "2",
        ],
    );
    let initial: Value = serde_json::from_str(&stdout(&initial)).unwrap();
    let cursor = initial["next_cursor"].as_str().unwrap();
    let path = root.join("items.mara.md");
    let mut source = fs::read_to_string(&path).unwrap();
    source.push('\n');
    fs::write(&path, source).unwrap();
    let stale = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--rule",
            "urn:mara:rule:evidenced_requirement",
            "--limit",
            "2",
            "--cursor",
            cursor,
        ],
    );
    assert!(!stale.status.success());
    let error: Value = serde_json::from_str(&stdout(&stale)).unwrap();
    assert_eq!(error["error"]["code"], "stale_cursor");
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_request_check_preserves_external_terminal_and_incompleteness() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["relations"]["tracked_by"] = json!({
        "description":"Local delivery ticket.","source":["requirement"],
        "target":[],"external":true
    });
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    fs::write(
        root.join("check.yaml"),
        "id: rule:ticket_check\nproperty: [{path: tracked_by, minCount: 1}]\n",
    )
    .unwrap();
    let target = "external:https://example.com/ticket/1";
    assert!(
        mara(root, &["relation", "add", "REQ-A", "tracked_by", target])
            .status
            .success()
    );
    let args = [
        "--format",
        "json",
        "trace",
        "matrix",
        "--id",
        "REQ-A",
        "--check-file",
        "check.yaml",
        "--shape",
        "urn:mara:rule:ticket_check",
    ];
    let output = mara(root, &args);
    assert!(output.status.success(), "{}", stderr(&output));
    let result: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(result["summaries"][0]["passed"], 1, "{result:#}");
    let edge = result["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "edge")
        .unwrap();
    assert_eq!(
        edge["endpoint"],
        json!({"kind":"external","address":"https://example.com/ticket/1"})
    );
    assert_eq!(edge["qualification"], Value::Null);
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "trace_matrix",
                json!({"ids":["REQ-A"],"check":{
            "files":["check.yaml"],"shape":"urn:mara:rule:ticket_check"}}),
            ),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        result
    );

    let path = root.join("items.mara.md");
    let source = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        source.replace(
            ":tracked_by: external:https://example.com/ticket/1",
            ":tracked_by: external:not-a-url",
        ),
    )
    .unwrap();
    let invalid = mara(root, &args);
    assert!(!invalid.status.success());
    let incomplete: Value = serde_json::from_str(&stdout(&invalid)).unwrap();
    assert_eq!(incomplete["evaluation_complete"], false, "{incomplete:#}");
    assert_eq!(incomplete["summaries"][0]["counts_exact"], false);
    assert!(
        incomplete["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "issue")
    );
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_binds_revision_evidence_through_cli_and_mcp() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["flavours"]["evidence"]["fields"] = json!({
        "status":{"type":"enum","values":["failed","passed"]},
        "subject_revision":{"type":"string"}
    });
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    let check = "\
- id: rule:revision_evidence
  property:
    - path: {inversePath: verifies}
      qualifiedValueShape: rule:verified_revision
      qualifiedMinCount: 1
- id: rule:verified_revision
  class: verification
  property:
    - path: {inversePath: evidences}
      qualifiedValueShape: rule:passing_revision
      qualifiedMinCount: 1
- id: rule:passing_revision
  class: evidence
  property:
    - path: status
      hasValue: passed
    - path: subject_revision
      hasValue: {parameter: subject_revision}
";
    fs::write(root.join("check.yaml"), check).unwrap();
    assert!(
        mara(
            root,
            &["relation", "add", "VER-APPROVED", "verifies", "REQ-A"]
        )
        .status
        .success()
    );
    let create_evidence = |id: &str, revision: &str| {
        let out = mara(
            root,
            &[
                "item",
                "create",
                "evidence",
                id,
                "items.mara.md",
                "--title",
                id,
                "--body",
                "A recorded test result.",
                "--field",
                "status=passed",
                "--field",
                &format!("subject_revision={revision}"),
            ],
        );
        assert!(out.status.success(), "{}", stderr(&out));
        let out = mara(root, &["relation", "add", id, "evidences", "VER-APPROVED"]);
        assert!(out.status.success(), "{}", stderr(&out));
    };
    create_evidence("EVD-OLD", "old123");
    let args = [
        "--format",
        "json",
        "trace",
        "matrix",
        "--id",
        "REQ-A",
        "--check-file",
        "check.yaml",
        "--shape",
        "urn:mara:rule:revision_evidence",
        "--param",
        "subject_revision=new456",
        "--limit",
        "100",
    ];
    let failed = mara(root, &args);
    assert!(failed.status.success(), "{}", stderr(&failed));
    let failed: Value = serde_json::from_str(&stdout(&failed)).unwrap();
    assert_eq!(failed["summaries"][0]["failed"], 1, "{failed:#}");
    assert!(
        failed["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|record| record["condition"]["components"]["hasValue"] == "new456")
    );
    let request = json!({"ids":["REQ-A"], "check":{
        "files":["check.yaml"], "shape":"urn:mara:rule:revision_evidence",
        "parameters":{"subject_revision":"new456"}}, "limit":100});
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "trace_matrix", request.clone()),
            mcp_request(3, "tools/list", json!({})),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        failed
    );
    let tool = mcp_response(&responses, 3)["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "trace_matrix")
        .unwrap();
    assert!(
        tool["description"]
            .as_str()
            .unwrap()
            .contains("check.parameters")
    );
    assert_eq!(
        tool["inputSchema"]["$defs"]["TraceCheck"]["properties"]["parameters"]["additionalProperties"]
            ["type"],
        "string"
    );
    let help = mara(root, &["trace", "matrix", "--help"]);
    assert!(help.status.success());
    assert!(stdout(&help).contains("--param subject_revision=abc123"));

    create_evidence("EVD-NEW", "new456");
    let passed = mara(root, &args);
    assert!(passed.status.success(), "{}", stderr(&passed));
    let passed: Value = serde_json::from_str(&stdout(&passed)).unwrap();
    assert_eq!(passed["summaries"][0]["passed"], 1, "{passed:#}");
    assert_eq!(fs::read_to_string(root.join("check.yaml")).unwrap(), check);
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "trace_matrix", request),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        passed
    );

    let first = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--check-file",
            "check.yaml",
            "--shape",
            "urn:mara:rule:revision_evidence",
            "--param",
            "subject_revision=new456",
            "--limit",
            "1",
        ],
    );
    let first: Value = serde_json::from_str(&stdout(&first)).unwrap();
    let cursor = first["next_cursor"].as_str().unwrap();
    let stale = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--check-file",
            "check.yaml",
            "--shape",
            "urn:mara:rule:revision_evidence",
            "--param",
            "subject_revision=old123",
            "--limit",
            "1",
            "--cursor",
            cursor,
        ],
    );
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&stale)).unwrap()["error"]["code"],
        "stale_cursor"
    );
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_rejects_invalid_bindings_and_supports_in_parameters() {
    let fixture = rule_fixture();
    let root = fixture.path();
    fs::write(
        root.join("check.yaml"),
        "id: rule:owner_check\nproperty: [{path: owner, in: [{parameter: expected_owner}, Bob]}]\n",
    )
    .unwrap();
    let base = [
        "--format",
        "json",
        "trace",
        "matrix",
        "--id",
        "REQ-A",
        "--check-file",
        "check.yaml",
        "--shape",
        "urn:mara:rule:owner_check",
    ];
    let run = |extra: &[&str]| {
        let mut args = base.to_vec();
        args.extend_from_slice(extra);
        let output = mara(root, &args);
        serde_json::from_str::<Value>(&stdout(&output)).unwrap()
    };
    assert_eq!(
        run(&["--param", "expected_owner=Alice"])["summaries"][0]["passed"],
        1
    );
    assert_eq!(
        run(&["--param", "expected_owner=Carol"])["summaries"][0]["failed"],
        1
    );
    for extra in [
        vec![],
        vec![
            "--param",
            "expected_owner=Alice",
            "--param",
            "expected_owner=Bob",
        ],
        vec!["--param", "expected_owner=Alice", "--param", "unused=x"],
        vec!["--param", "bad-name=x"],
        vec!["--param", "expected_owner"],
    ] {
        assert_eq!(
            run(&extra)["error"]["code"],
            "invalid_argument",
            "{extra:?}"
        );
    }
    let without_check = mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--param",
            "expected_owner=Alice",
        ],
    );
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&without_check)).unwrap()["error"]["code"],
        "invalid_argument"
    );
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "trace_matrix",
                json!({"ids":["REQ-A"], "check":{
            "files":["check.yaml"], "shape":"urn:mara:rule:owner_check",
            "parameters":{"expected_owner":42}}}),
            ),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"]["error"]["code"],
        "invalid_argument"
    );
    fs::write(
        root.join("check.yaml"),
        "id: rule:owner_check\nproperty: [{path: owner, hasValue: {parameter: 3}}]\n",
    )
    .unwrap();
    assert_eq!(
        run(&["--param", "expected_owner=Alice"])["error"]["code"],
        "invalid_argument"
    );
}

// @mara implements VER-TRACE-MATRIX
// @mara checks REQ-TRACE-MATRIX
#[test]
fn engineering_profile_gates_acceptance_and_separates_coverage_from_results() {
    let fixture = fixture();
    let root = fixture.path();
    let run = |args: &[&str]| {
        let output = mara(root, args);
        assert!(
            output.status.success(),
            "{args:?}: {} {}",
            stderr(&output),
            stdout(&output)
        );
    };
    run(&["project", "init", "--template", "engineering"]);
    for (flavour, id) in [
        ("goal", "GOAL-EXPORT"),
        ("scenario", "SCN-EXPORT"),
        ("requirement", "REQ-EXPORT"),
        ("design", "DES-EXPORT"),
        ("verification", "VER-EXPORT"),
        ("evidence", "EVD-EXPORT"),
        ("risk", "RISK-EXPORT"),
        ("decision", "ADR-EXPORT"),
    ] {
        run(&[
            "item",
            "create",
            flavour,
            id,
            "export.mara.md",
            "--title",
            id,
            "--body",
            "Export contract or its supporting knowledge.",
            "--field",
            "status=draft",
        ]);
    }
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    run(&["item", "update", "REQ-EXPORT", "--field", "status=accepted"]);
    let incomplete = validation_with_parity(root, &[]);
    assert_eq!(incomplete["valid"], false);
    assert!(
        incomplete["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "rule_failed")
    );

    for (id, fields) in [
        ("GOAL-EXPORT", vec!["status=accepted"]),
        ("SCN-EXPORT", vec!["status=accepted"]),
        ("REQ-EXPORT", vec!["kind=functional"]),
        ("DES-EXPORT", vec!["status=accepted", "kind=interface"]),
        (
            "VER-EXPORT",
            vec!["status=accepted", "method=test", "level=system"],
        ),
    ] {
        let mut args = vec!["item", "update", id];
        for field in fields {
            args.extend(["--field", field]);
        }
        run(&args);
    }
    for (source, relation, target) in [
        ("SCN-EXPORT", "contributes_to", "GOAL-EXPORT"),
        ("REQ-EXPORT", "derives_from", "SCN-EXPORT"),
        ("DES-EXPORT", "satisfies", "REQ-EXPORT"),
        ("VER-EXPORT", "verifies", "REQ-EXPORT"),
    ] {
        run(&["relation", "add", source, relation, target]);
    }
    assert_eq!(validation_with_parity(root, &[])["valid"], true);

    let check = |id: &str, shape: &str, revision: Option<&str>, passed: bool| {
        let file = if shape == "execution" {
            ".mara/engineering-execution.yaml"
        } else {
            ".mara/engineering-checks.yaml"
        };
        let shape = format!("urn:mara:rule:{shape}");
        let mut args = vec![
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            id,
            "--check-file",
            file,
            "--shape",
            &shape,
        ];
        let mut request = json!({"files":[file],"shape":shape});
        let binding;
        if let Some(revision) = revision {
            binding = format!("subject_revision={revision}");
            args.extend(["--param", &binding]);
            request["parameters"] = json!({"subject_revision":revision});
        }
        let output = mara(root, &args);
        assert!(
            output.status.success(),
            "{} {}",
            stderr(&output),
            stdout(&output)
        );
        let cli: Value = serde_json::from_slice(&output.stdout).unwrap();
        let mcp = relation_tool(root, "trace_matrix", json!({"ids":[id],"check":request}));
        assert_eq!(cli, mcp);
        assert_eq!(cli["evaluation_complete"], true, "{cli:#}");
        assert_eq!(
            cli["summaries"][0][if passed { "passed" } else { "failed" }],
            1,
            "{cli:#}"
        );
    };
    check("REQ-EXPORT", "intent", None, true);
    check("REQ-EXPORT", "verification", None, true);
    check("REQ-EXPORT", "realization", None, false);
    check("GOAL-EXPORT", "validation", None, false);
    fs::write(root.join("export.txt"), "Concrete implementation fixture.").unwrap();
    fs::write(
        root.join("export-test.txt"),
        "Concrete check definition fixture.",
    )
    .unwrap();
    run(&[
        "relation",
        "add",
        "DES-EXPORT",
        "implemented_by",
        "code:export.txt",
    ]);
    check("REQ-EXPORT", "realization", None, true);
    run(&["relation", "add", "VER-EXPORT", "validates", "GOAL-EXPORT"]);
    check("GOAL-EXPORT", "validation", None, true);

    // Retired methods stop qualifying; a direct code check can define verification.
    run(&["item", "update", "VER-EXPORT", "--field", "status=retired"]);
    check("REQ-EXPORT", "verification", None, false);
    run(&[
        "relation",
        "add",
        "REQ-EXPORT",
        "checked_by",
        "code:export-test.txt",
    ]);
    check("REQ-EXPORT", "verification", None, true);
    run(&["item", "update", "VER-EXPORT", "--field", "status=accepted"]);

    // Accepted evidence may honestly record failure, with concrete provenance.
    run(&["relation", "add", "EVD-EXPORT", "evidences", "VER-EXPORT"]);
    run(&[
        "item",
        "update",
        "EVD-EXPORT",
        "--field",
        "status=accepted",
        "--field",
        "result=failed",
    ]);
    assert_eq!(validation_with_parity(root, &[])["valid"], false);
    run(&[
        "item",
        "update",
        "EVD-EXPORT",
        "--field",
        "captured_at=2026-09-26T20:00:00Z",
        "--field",
        "subject_revision=def456",
    ]);
    assert_eq!(validation_with_parity(root, &[])["valid"], true);

    // Definition coverage and an accepted failed run do not satisfy execution.
    let execution_file = root.join(".mara/engineering-execution.yaml");
    let execution_source = fs::read(&execution_file).unwrap();
    check("VER-EXPORT", "execution", Some("def456"), false);
    run(&["item", "update", "EVD-EXPORT", "--field", "result=passed"]);
    check("VER-EXPORT", "execution", Some("def456"), true);
    check("VER-EXPORT", "execution", Some("new789"), false);
    run(&["item", "update", "EVD-EXPORT", "--field", "status=retired"]);
    check("VER-EXPORT", "execution", Some("def456"), false);
    run(&[
        "item",
        "create",
        "evidence",
        "EVD-EXPORT-NEW",
        "export.mara.md",
        "--title",
        "Export check on the new candidate",
        "--body",
        "The export check passed against the new candidate.",
        "--field",
        "status=accepted",
        "--field",
        "result=passed",
        "--field",
        "captured_at=2026-09-26T21:00:00Z",
        "--field",
        "subject_revision=new789",
        "--relation",
        "evidences=VER-EXPORT",
    ]);
    check("VER-EXPORT", "execution", Some("new789"), true);
    assert_eq!(execution_source, fs::read(&execution_file).unwrap());
    assert_eq!(validation_with_parity(root, &[])["valid"], true);

    run(&["relation", "add", "RISK-EXPORT", "affects", "REQ-EXPORT"]);
    run(&[
        "item",
        "update",
        "RISK-EXPORT",
        "--field",
        "status=accepted",
        "--field",
        "treatment=tolerated",
    ]);
    assert_eq!(validation_with_parity(root, &[])["valid"], false);
    run(&["relation", "add", "ADR-EXPORT", "justifies", "RISK-EXPORT"]);
    run(&["item", "update", "ADR-EXPORT", "--field", "status=accepted"]);
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_markdown_preserves_file_and_scip_descriptor_links() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["relations"]["code_implements"] = json!({"description":"Code implements a requirement.","source":[],"target":["requirement"],"code_source":true,"inverse":"implemented_by_code"});
    fs::write(schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    support::code_index::configure(root, "typescript", &["ts"], false);
    fs::write(root.join("service.ts"), "export function run() {}\n").unwrap();
    support::code_index::write_single(
        root,
        "typescript",
        "service.ts",
        "run",
        "`service.ts`/run().",
    );
    for reference in [
        "code:service.ts",
        "code:service.ts::typescript::`service.ts`/run().",
    ] {
        let output = mara(
            root,
            &["relation", "add", "REQ-A", "implemented_by_code", reference],
        );
        assert!(output.status.success(), "{}", stderr(&output));
    }
    fs::write(root.join("rules.yaml"), "id: rule:code_state\ntargetClass: requirement\nproperty:\n  - path: {inversePath: code_implements}\n    minCount: 2\n").unwrap();
    let output = mara(
        root,
        &[
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--rule",
            "urn:mara:rule:code_state",
            "--limit",
            "100",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let result = relation_tool(
        root,
        "trace_matrix",
        json!({"ids":["REQ-A"],"rules":["urn:mara:rule:code_state"],"limit":100,"render":"markdown"}),
    );
    let markdown = stdout(&output);
    assert_eq!(result["markdown"], markdown);
    assert!(
        markdown.contains("[code:service.ts](<service.ts>)"),
        "{markdown}"
    );
    assert!(
        markdown.contains("[code:service.ts::typescript::\\`service.ts\\`/run().](<service.ts>)"),
        "{markdown}"
    );
    assert!(!markdown.contains("[?](<>)"), "{markdown}");
}

// @mara implements VER-TRACE-MATRIX
// @mara checks REQ-TRACE-MATRIX
#[test]
fn trace_matrix_selects_roots_by_intersection_and_rejects_ambiguous_requests() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let base = json!({"ids":["REQ-A","VER-DRAFT"],"flavours":["requirement"],"fields":[{"key":"status","value":"approved"}],"paths":["items.mara.md"],"rules":["urn:mara:rule:approved_requirement"]});
    let selected = relation_tool(root, "trace_matrix", base.clone());
    assert_eq!(selected["summaries"][0]["selected"], 1, "{selected:#}");
    let mut empty = base.clone();
    empty["fields"][0]["value"] = json!("draft");
    let result = relation_tool(root, "trace_matrix", empty);
    assert_eq!(result["summaries"][0]["selected"], 0, "{result:#}");
    assert_eq!(result["evaluation_complete"], true);
    for (key, value) in [
        ("all", json!(true)),
        ("limit", json!(0)),
        ("limit", json!(101)),
        ("paths", json!(["../items.mara.md"])),
        ("cursor", json!("")),
    ] {
        let mut invalid = base.clone();
        invalid[key] = value;
        let result = relation_tool(root, "trace_matrix", invalid);
        assert_eq!(result["error"]["code"], "invalid_argument", "{result:#}");
    }
}

// @mara implements VER-TRACE-MATRIX
// @mara checks DES-TRACE-VIEW-INTERFACES
#[test]
fn trace_matrix_byte_budget_pages_without_losing_results() {
    let fixture = rule_fixture();
    let root = fixture.path();
    for number in 0..6 {
        let output = mara(
            root,
            &[
                "item",
                "create",
                "requirement",
                &format!("REQ-LONG-{number}-{}", "X".repeat(5_000)),
                "items.mara.md",
                "--title",
                "Long identity",
                "--body",
                "Requirement.",
                "--field",
                "status=approved",
            ],
        );
        assert!(output.status.success(), "{}", stderr(&output));
    }
    let mut request = json!({"flavours":["requirement"],"rules":["urn:mara:rule:approved_requirement"],"limit":100,"render":"markdown"});
    let mut ids = std::collections::BTreeSet::new();
    let mut pages = 0;
    loop {
        pages += 1;
        assert!(pages < 30);
        let result = relation_tool(root, "trace_matrix", request.clone());
        assert!(result.get("error").is_none(), "{result:#}");
        assert!(serde_json::to_vec(&result).unwrap().len() <= 65_536);
        assert_eq!(result["summaries"][0]["selected"], 7);
        for record in result["records"].as_array().unwrap() {
            if record["kind"] == "result" {
                assert!(ids.insert(record["root"]["id"].as_str().unwrap().to_owned()));
            }
        }
        if result["has_more"] == false {
            break;
        }
        assert!(result["records"].as_array().unwrap().len() < 100);
        request["cursor"] = result["next_cursor"].clone();
    }
    assert!(pages > 1);
    assert_eq!(ids.len(), 7);
    let large_id = format!("REQ-{}", "X".repeat(70_000));
    let output = mara(
        root,
        &[
            "item",
            "create",
            "requirement",
            &large_id,
            "items.mara.md",
            "--title",
            "Large identity",
            "--body",
            "Requirement.",
            "--field",
            "status=approved",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let result = relation_tool(
        root,
        "trace_matrix",
        json!({"ids":[large_id],"rules":["urn:mara:rule:approved_requirement"],"render":"markdown"}),
    );
    assert_eq!(result["error"]["code"], "output_limit", "{result:#}");
}
