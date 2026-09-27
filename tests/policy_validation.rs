use serde_json::{Value, json};
use std::{fs, path::Path};
use tempfile::TempDir;
mod support;
use support::*;

fn diagnostic_parity(root: &Path, args: &[&str], tool: &str, params: Value) -> Value {
    let mut cli_args = vec!["--format", "json"];
    cli_args.extend_from_slice(args);
    let cli = mara(root, &cli_args);
    let value: Value = serde_json::from_slice(&cli.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", stderr(&cli)));
    let replies = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, tool, params),
        ],
    );
    let mcp = &mcp_response(&replies, 2)["result"];
    assert_eq!(value, mcp["structuredContent"]);
    assert_eq!(mcp["isError"], value.get("error").is_some());
    assert_eq!(cli.status.success(), value["valid"] == true);
    assert_eq!(value["format_version"], 1);
    value
}

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

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn current_state_rules_require_all_classes_on_relation_endpoints() {
    let fixture = rule_fixture();
    let root = fixture.path();
    for (target, path) in [
        ("design", json!("satisfies")),
        ("verification", json!("verifies")),
        ("requirement", json!({"inversePath":"satisfies"})),
    ] {
        for key in ["class", "node", "qualifiedValueShape"] {
            let classes = json!(["requirement", "design"]);
            let mut property = json!({"path":path});
            property[key] = if key == "class" {
                classes
            } else {
                json!({"class":classes})
            };
            if key == "qualifiedValueShape" {
                property["qualifiedMinCount"] = json!(1);
            }
            let rule =
                json!({"id":"rule:all_classes", "targetClass":target, "property":[property]});
            fs::write(
                root.join("rules.yaml"),
                serde_saphyr::to_string(&rule).unwrap(),
            )
            .unwrap();
            let invalid =
                diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
            assert_eq!(invalid["valid"], false, "{target}/{key}: {invalid:#}");
            assert_eq!(
                invalid["diagnostics"][0]["code"], "rule_invalid",
                "{invalid:#}"
            );
            let pointer = if key == "class" {
                "/property/0/class".into()
            } else {
                format!("/property/0/{key}/class")
            };
            assert_eq!(invalid["diagnostics"][0]["location"]["pointer"], pointer);
        }
    }
    assert!(
        mara(root, &["relation", "add", "DES-A", "satisfies", "REQ-A"])
            .status
            .success()
    );
    // Repeating one class remains satisfiable after RDF deduplication.
    fs::write(root.join("rules.yaml"),
        "id: rule:repeated_class\ntargetClass: design\nproperty: [{path: satisfies, class: [requirement, requirement], minCount: 1}]\n"
    ).unwrap();
    let valid = validation_with_parity(root, &[]);
    assert_eq!(valid["valid"], true, "{valid:#}");
    for target in ["REQ-A", "DES-A"] {
        assert!(
            mara(
                root,
                &["relation", "add", "VER-APPROVED", "verifies", target]
            )
            .status
            .success()
        );
    }
    // Alternative classes use explicit OR; multiple targetClass values select either flavour.
    fs::write(root.join("rules.yaml"),
        "- id: rule:alternatives\n  targetClass: verification\n  property:\n    - path: verifies\n      node:\n        or: [{class: requirement}, {class: design}]\n- id: rule:targets\n  targetClass: [requirement, design]\n  property: [{path: owner, minCount: 1}]\n"
    ).unwrap();
    let valid = validation_with_parity(root, &[]);
    assert_eq!(valid["valid"], true, "{valid:#}");
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn current_state_rules_preserve_field_types_in_nested_value_shapes() {
    let fixture = rule_fixture();
    let root = fixture.path();
    assert!(
        mara(root, &["item", "update", "REQ-A", "--field", "score=1"])
            .status
            .success()
    );
    for (field, datatype) in [
        ("owner", "string"),
        ("status", "string"),
        ("score", "double"),
    ] {
        for key in ["node", "qualifiedValueShape"] {
            for (constraint_type, valid) in [("integer", false), (datatype, true)] {
                let mut property = json!({"path":field});
                property[key] = json!({"datatype":constraint_type});
                if key == "qualifiedValueShape" {
                    property["qualifiedMinCount"] = json!(0);
                }
                let rule = json!({"id":"rule:nested_type", "targetClass":"requirement",
                    "property":[property]});
                fs::write(
                    root.join("rules.yaml"),
                    serde_saphyr::to_string(&rule).unwrap(),
                )
                .unwrap();
                let result =
                    diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
                assert_eq!(result["valid"], valid, "{field}/{key}: {result:#}");
                if valid {
                    assert_eq!(validation_with_parity(root, &[])["valid"], true);
                } else {
                    assert_eq!(
                        result["diagnostics"][0]["code"], "rule_invalid",
                        "{result:#}"
                    );
                    assert_eq!(
                        result["diagnostics"][0]["location"]["pointer"],
                        format!("/property/0/{key}/datatype")
                    );
                }
            }
        }
    }
    // The same reusable shape must be checked separately for each field type,
    // even through an additional logical/nested shape layer.
    for (second_field, valid) in [("status", true), ("score", false)] {
        let rules = json!([
            {"id":"rule:shared", "targetClass":"requirement", "property":[
                {"path":"owner", "node":"rule:text"},
                {"path":second_field, "node":"rule:text"}
            ]},
            {"id":"rule:text", "and":[{"node":{"datatype":"string"}}]}
        ]);
        fs::write(
            root.join("rules.yaml"),
            serde_saphyr::to_string(&rules).unwrap(),
        )
        .unwrap();
        let result = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
        assert_eq!(result["valid"], valid, "{second_field}: {result:#}");
        if !valid {
            assert_eq!(
                result["diagnostics"][0]["code"], "rule_invalid",
                "{result:#}"
            );
            assert_eq!(
                result["diagnostics"][0]["location"]["pointer"],
                "/1/and/0/node/datatype"
            );
        }
    }
    fs::write(root.join("rules.yaml"),
        "id: rule:value\ntargetClass: requirement\nproperty: [{path: owner, node: {datatype: string, hasValue: Bob}}]\n"
    ).unwrap();
    let failed = validation_with_parity(root, &[]);
    assert_eq!(failed["evaluation_complete"], true, "{failed:#}");
    assert_eq!(
        failed["diagnostics"][0]["code"], "rule_failed",
        "{failed:#}"
    );
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn current_state_rules_report_authored_messages_with_a_generated_fallback() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let message = "Assign an owner before approval — see the team policy.";
    for property in [false, true] {
        for authored in [true, false] {
            let mut obligation = if property {
                json!({"path":"owner", "maxCount":0})
            } else {
                json!({"class":"verification"})
            };
            if authored {
                obligation["message"] = json!(message);
            }
            let mut rule = if property {
                json!({"property":[obligation]})
            } else {
                obligation
            };
            rule["id"] = json!("rule:message");
            rule["targetClass"] = json!("requirement");
            fs::write(
                root.join("rules.yaml"),
                serde_saphyr::to_string(&rule).unwrap(),
            )
            .unwrap();
            let result = validation_with_parity(root, &[]);
            assert_eq!(result["evaluation_complete"], true, "{result:#}");
            assert_eq!(result["summary"]["errors"], 1, "{result:#}");
            let diagnostic = &result["diagnostics"][0];
            assert_eq!(diagnostic["code"], "rule_failed");
            let key = if property { "maxCount" } else { "class" };
            let fallback = format!("rule urn:mara:rule:message failed: {key}");
            assert_eq!(
                diagnostic["message"],
                if authored { message } else { &fallback }
            );
            let human = mara(root, &["project", "validate"]);
            assert!(stderr(&human).contains(diagnostic["message"].as_str().unwrap()));
        }
    }
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn current_state_rules_apply_property_classes_after_path_selection() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["flavours"]["design"]["fields"]["design_only"] = json!({"type":"string"});
    schema["relations"]["associated_with"] = json!({
        "description": "An association.", "source": ["requirement"],
        "target": ["design", "risk"]
    });
    fs::write(schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    assert!(
        mara(
            root,
            &["item", "update", "DES-A", "--field", "design_only=ready"]
        )
        .status
        .success()
    );
    for (source, relation, target) in [
        ("DES-A", "satisfies", "REQ-A"),
        ("REQ-A", "associated_with", "DES-A"),
    ] {
        assert!(
            mara(root, &["relation", "add", source, relation, target])
                .status
                .success()
        );
    }
    for (target, path, class) in [
        ("requirement", "satisfies", "design"),
        ("design", "{inversePath: satisfies}", "requirement"),
        ("requirement", "design_only", "design"),
    ] {
        fs::write(root.join("rules.yaml"), format!(
            "id: rule:invalid_path\ntargetClass: {target}\nproperty: [{{path: {path}, class: {class}, minCount: 1}}]\n"
        )).unwrap();
        let invalid =
            diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
        assert_eq!(invalid["valid"], false, "{path}: {invalid:#}");
        assert_eq!(
            invalid["diagnostics"][0]["code"], "rule_invalid",
            "{invalid:#}"
        );
        assert_eq!(
            invalid["diagnostics"][0]["location"]["pointer"],
            "/property/0/path"
        );
    }
    // PropertyShape class narrows selected endpoints for nested field checks.
    for path in ["{inversePath: satisfies}", "associated_with"] {
        fs::write(root.join("rules.yaml"), format!(
            "id: rule:design_field\ntargetClass: requirement\nproperty:\n  - path: {path}\n    class: design\n    minCount: 1\n    property: [{{path: design_only, hasValue: ready}}]\n"
        )).unwrap();
        let valid = validation_with_parity(root, &[]);
        assert_eq!(valid["valid"], true, "{path}: {valid:#}");
        assert!(
            mara(
                root,
                &["item", "update", "DES-A", "--field", "design_only=draft"]
            )
            .status
            .success()
        );
        let failed = validation_with_parity(root, &[]);
        assert_eq!(failed["evaluation_complete"], true, "{failed:#}");
        assert_eq!(
            failed["diagnostics"][0]["code"], "rule_failed",
            "{failed:#}"
        );
        assert!(
            mara(
                root,
                &["item", "update", "DES-A", "--field", "design_only=ready"]
            )
            .status
            .success()
        );
    }
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn current_state_rules_resolve_reserved_prefix_names_to_projected_values() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["flavours"]["requirement"]["fields"]["field"] = json!({"type":"string"});
    schema["relations"]["schema"] = json!({
        "description": "A relation with a reserved prefix name.",
        "source": ["requirement"], "target": ["design"]
    });
    fs::write(schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    assert!(
        mara(
            root,
            &["item", "update", "REQ-A", "--field", "field=present"]
        )
        .status
        .success()
    );
    assert!(
        mara(root, &["relation", "add", "REQ-A", "schema", "DES-A"])
            .status
            .success()
    );
    for (target, path) in [
        ("requirement", "field"),
        ("requirement", "field:field"),
        ("requirement", "schema"),
        ("requirement", "schema:schema"),
        ("design", "{inversePath: schema}"),
        ("design", "{inversePath: 'schema:schema'}"),
    ] {
        for (minimum, expected) in [(1, true), (2, false)] {
            fs::write(root.join("rules.yaml"), format!(
                "id: rule:prefix\ntargetClass: {target}\nproperty: [{{path: {path}, minCount: {minimum}}}]\n"
            )).unwrap();
            let result = validation_with_parity(root, &[]);
            assert_eq!(result["evaluation_complete"], true, "{path}: {result:#}");
            assert_eq!(result["valid"], expected, "{path}: {result:#}");
            if !expected {
                assert_eq!(
                    result["diagnostics"][0]["code"], "rule_failed",
                    "{result:#}"
                );
            }
        }
    }
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn current_state_rules_narrow_same_flavour_paths_in_both_directions() {
    let fixture = rule_fixture();
    let root = fixture.path();
    assert!(
        mara(
            root,
            &["item", "update", "DES-A", "--clear-field", "status"]
        )
        .status
        .success()
    );
    assert!(
        mara(
            root,
            &[
                "item",
                "create",
                "requirement",
                "REQ-B",
                "items.mara.md",
                "--title",
                "Second requirement",
                "--body",
                "A related requirement.",
                "--field",
                "status=approved"
            ]
        )
        .status
        .success()
    );
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["flavours"]["design"]["fields"]
        .as_object_mut()
        .unwrap()
        .remove("status");
    schema["relations"]["associated_with"] = json!({
        "description": "An association between items of the same flavour.",
        "source": ["requirement", "design"], "target": ["requirement", "design"],
        "same_flavour": true
    });
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    assert!(
        mara(
            root,
            &["relation", "add", "REQ-A", "associated_with", "REQ-B"]
        )
        .status
        .success()
    );
    for (path, endpoint) in [
        ("associated_with", "REQ-B"),
        ("{inversePath: associated_with}", "REQ-A"),
    ] {
        fs::write(root.join("rules.yaml"), format!(
            "id: rule:related_status\ntargetClass: requirement\nproperty:\n  - path: {path}\n    node:\n      property: [{{path: status, hasValue: approved}}]\n"
        )).unwrap();
        let valid = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
        assert_eq!(valid["valid"], true, "{valid:#}");
        let valid = validation_with_parity(root, &[]);
        assert_eq!(valid["valid"], true, "{valid:#}");
        assert!(
            mara(
                root,
                &["item", "update", endpoint, "--field", "status=draft"]
            )
            .status
            .success()
        );
        let failed = validation_with_parity(root, &[]);
        assert_eq!(failed["evaluation_complete"], true, "{failed:#}");
        assert_eq!(failed["summary"]["errors"], 1, "{failed:#}");
        assert_eq!(
            failed["diagnostics"][0]["code"], "rule_failed",
            "{failed:#}"
        );
        assert!(
            mara(
                root,
                &["item", "update", endpoint, "--field", "status=approved"]
            )
            .status
            .success()
        );
        // Without the restriction, design endpoints are reachable and need status too.
        schema["relations"]["associated_with"]["same_flavour"] = json!(false);
        fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
        let invalid =
            diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
        assert_eq!(invalid["valid"], false, "{invalid:#}");
        assert_eq!(
            invalid["diagnostics"][0]["code"], "rule_invalid",
            "{invalid:#}"
        );
        schema["relations"]["associated_with"]["same_flavour"] = json!(true);
        fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    }
}

// @mara checks DES-CURRENT-STATE-EVALUATION
// @mara implements VER-POLICY-VALIDATION
// @mara checks REQ-CURRENT-STATE-RULES
#[test]
fn current_state_rules_run_real_lifecycle_and_coverage_through_cli_and_mcp() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let failed = validation_with_parity(root, &[]);
    assert_eq!(failed["evaluation_complete"], true, "{failed:#}");
    assert_eq!(failed["summary"]["errors"], 3, "{failed:#}");
    assert!(
        failed["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["code"] == "rule_failed")
    );
    assert!(
        mara(root, &["relation", "add", "VER-DRAFT", "verifies", "REQ-A"])
            .status
            .success()
    );
    let draft = validation_with_parity(root, &[]);
    let d = draft["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["item"]["id"] == "REQ-A")
        .unwrap();
    assert_eq!(d["details"]["selected_count"], 1, "{d:#}");
    assert_eq!(d["details"]["qualifying_count"], 0);
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "relation_add",
                json!({"source":"REQ-A","relation":"verified_by","target":"VER-APPROVED"}),
            ),
        ],
    );
    assert_ne!(
        mcp_response(&responses, 2)["result"]["isError"],
        true,
        "{responses:#?}"
    );
    for (source, relation, target) in [
        ("DES-A", "satisfies", "REQ-A"),
        ("DES-A", "mitigates", "RISK-A"),
    ] {
        assert!(
            mara(root, &["relation", "add", source, relation, target])
                .status
                .success()
        );
    }
    // A direct Markdown duplicate has the same canonical edge as inverse metadata.
    let file = root.join("items.mara.md");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replace(
            ":title: VER-APPROVED",
            ":title: VER-APPROVED\n:verifies: REQ-A",
        ),
    )
    .unwrap();
    let pass = validation_with_parity(root, &[]);
    assert_eq!(pass["valid"], true, "{pass:#}");
    let rules = fs::read_to_string(root.join("rules.yaml")).unwrap();
    fs::write(
        root.join("rules.yaml"),
        rules.replace(
            "qualifiedMinCount: 1",
            "qualifiedMinCount: 1\n      qualifiedMaxCount: 1",
        ),
    )
    .unwrap();
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    fs::write(
        root.join("rules.yaml"),
        rules.replace(
            "id: rule:verification_count",
            "id: rule:verification_count\n      node: rule:approved_verification",
        ),
    )
    .unwrap();
    let every = validation_with_parity(root, &[]);
    assert_eq!(every["summary"]["errors"], 1, "{every:#}");
    assert_eq!(every["diagnostics"][0]["details"]["kind"], "every");
    assert_eq!(every["diagnostics"][0]["details"]["direction"], "incoming");
    assert_eq!(every["diagnostics"][0]["details"]["relation"], "verifies");
    assert_eq!(
        every["diagnostics"][0]["obligation"]["source"]["path"],
        "rules.yaml"
    );
    fs::write(root.join("rules.yaml"), rules).unwrap();
    // Structured edits do not acquire a policy gate; explicit validation evaluates current state.
    assert!(
        mara(root, &["item", "update", "REQ-A", "--clear-field", "owner"])
            .status
            .success()
    );
    assert_eq!(validation_with_parity(root, &[])["summary"]["errors"], 1);
    assert!(
        mara(
            root,
            &["item", "update", "REQ-A", "--field", "status=draft"]
        )
        .status
        .success()
    );
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn bounded_trace_chains_distinguish_downstream_coverage_and_relation_kinds() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["flavours"]["evidence"]["fields"] =
        json!({"status":{"type":"enum","values":["draft","approved"]}});
    schema["relations"]["reviews"] = json!({
        "description":"Reviews a requirement without verifying it.",
        "source":["verification"], "target":["requirement"]
    });
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    fs::write(
        root.join("rules.yaml"),
        "\
- id: rule:evidenced_requirement
  targetClass: requirement
  whenShape: rule:approved_status
  property:
    - id: rule:verification_step
      path: {inversePath: verifies}
      qualifiedValueShape: rule:evidenced_verification
      qualifiedMinCount: 1
- id: rule:approved_status
  property: [{path: status, hasValue: approved}]
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

    // A different canonical kind between the same endpoints cannot satisfy the first step.
    assert!(
        mara(
            root,
            &["relation", "add", "VER-APPROVED", "reviews", "REQ-A"]
        )
        .status
        .success()
    );
    let wrong_kind = diagnostic_parity(
        root,
        &["item", "validate", "REQ-A"],
        "item_validate",
        json!({"id":"REQ-A"}),
    );
    assert_eq!(
        wrong_kind["diagnostics"][0]["details"]["selected_count"], 0,
        "{wrong_kind:#}"
    );
    assert_eq!(
        wrong_kind["diagnostics"][0]["details"]["relation"],
        "verifies"
    );
    assert_eq!(
        wrong_kind["diagnostics"][0]["details"]["direction"],
        "incoming"
    );
    assert_eq!(
        wrong_kind["diagnostics"][0]["obligation"]["source"]["path"],
        "rules.yaml"
    );

    assert!(
        mara(
            root,
            &["relation", "add", "VER-APPROVED", "verifies", "REQ-A"]
        )
        .status
        .success()
    );
    let second_hop = diagnostic_parity(
        root,
        &["item", "validate", "REQ-A"],
        "item_validate",
        json!({"id":"REQ-A"}),
    );
    assert_eq!(second_hop["evaluation_complete"], true, "{second_hop:#}");
    assert_eq!(second_hop["valid"], false);
    assert_eq!(second_hop["diagnostics"][0]["details"]["selected_count"], 1);
    assert_eq!(
        second_hop["diagnostics"][0]["details"]["qualifying_count"],
        0
    );
    for relation in ["reviews", "verifies"] {
        let edge = relation_tool(
            root,
            "relation_get",
            json!({"source":"VER-APPROVED","relation":relation,"target":"REQ-A"}),
        );
        assert_eq!(edge["edge"]["relation"], relation);
        assert_eq!(edge["occurrence_count"], 1);
        assert_eq!(edge["occurrences"][0]["source"]["path"], "items.mara.md");
    }

    let created = mara(
        root,
        &[
            "item",
            "create",
            "evidence",
            "EVD-A",
            "items.mara.md",
            "--title",
            "Execution evidence",
            "--body",
            "A recorded result.",
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
    let draft = diagnostic_parity(
        root,
        &["item", "validate", "REQ-A"],
        "item_validate",
        json!({"id":"REQ-A"}),
    );
    assert_eq!(draft["valid"], false);
    assert_eq!(draft["diagnostics"][0]["details"]["qualifying_count"], 0);
    assert!(
        mara(
            root,
            &["item", "update", "EVD-A", "--field", "status=approved"]
        )
        .status
        .success()
    );
    let passed = diagnostic_parity(
        root,
        &["item", "validate", "REQ-A"],
        "item_validate",
        json!({"id":"REQ-A"}),
    );
    assert_eq!(passed["valid"], true, "{passed:#}");
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn bounded_trace_chains_reject_excess_depth_and_finish_on_cycles() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let created = mara(
        root,
        &[
            "item",
            "create",
            "requirement",
            "REQ-B",
            "items.mara.md",
            "--title",
            "B",
            "--body",
            "Another requirement.",
        ],
    );
    assert!(created.status.success(), "{}", stderr(&created));

    let chain = |steps| {
        let mut shape = json!({"class":"requirement"});
        for _ in 0..steps {
            shape = json!({"property":[{"path":"depends_on","minCount":1,"node":shape}]});
        }
        shape["id"] = json!("rule:bounded_chain");
        shape["targetClass"] = json!("requirement");
        serde_saphyr::to_string(&shape).unwrap()
    };
    fs::write(root.join("rules.yaml"), chain(8)).unwrap();
    let accepted = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(accepted["valid"], true, "{accepted:#}");

    let first = diagnostic_parity(
        root,
        &["project", "validate", "--limit", "1"],
        "project_validate",
        json!({"limit":1}),
    );
    assert_eq!(first["evaluation_complete"], true, "{first:#}");
    assert_eq!(first["valid"], false);
    assert_eq!(first["summary"]["errors"], 2);
    assert_eq!(first["diagnostics"].as_array().unwrap().len(), 1);
    assert_eq!(first["has_more"], true);
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = diagnostic_parity(
        root,
        &["project", "validate", "--limit", "1", "--cursor", cursor],
        "project_validate",
        json!({"limit":1,"cursor":cursor}),
    );
    assert_eq!(second["evaluation_complete"], true);
    assert_eq!(second["valid"], false);
    assert_eq!(second["summary"], first["summary"]);
    assert_eq!(second["has_more"], false);
    assert_ne!(
        first["diagnostics"][0]["item"],
        second["diagnostics"][0]["item"]
    );

    for (source, target) in [("REQ-A", "REQ-B"), ("REQ-B", "REQ-A")] {
        assert!(
            mara(root, &["relation", "add", source, "depends_on", target])
                .status
                .success()
        );
    }
    let cyclic = diagnostic_parity(
        root,
        &["project", "validate"],
        "project_validate",
        json!({}),
    );
    assert_eq!(cyclic["valid"], true, "{cyclic:#}");
    assert_eq!(cyclic["evaluation_complete"], true);
    let stale = diagnostic_parity(
        root,
        &["project", "validate", "--limit", "1", "--cursor", cursor],
        "project_validate",
        json!({"limit":1,"cursor":cursor}),
    );
    assert_eq!(stale["error"]["code"], "stale_cursor");

    fs::write(root.join("rules.yaml"), chain(9)).unwrap();
    let excessive = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(excessive["valid"], false);
    assert_eq!(excessive["diagnostics"][0]["code"], "rule_invalid");
    assert!(
        excessive["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("relationship depth")
    );
    assert_eq!(
        excessive["diagnostics"][0]["location"]["path"],
        "rules.yaml"
    );
    assert!(
        excessive["diagnostics"][0]["location"]["pointer"]
            .as_str()
            .unwrap()
            .ends_with("/path")
    );
    let incomplete = diagnostic_parity(
        root,
        &["project", "validate"],
        "project_validate",
        json!({}),
    );
    assert_eq!(incomplete["evaluation_complete"], false);
    assert_eq!(incomplete["valid"], false);

    let mut reusable: Value = serde_saphyr::from_str(&chain(9)).unwrap();
    reusable["id"] = json!("rule:shared_path");
    reusable.as_object_mut().unwrap().remove("targetClass");
    let named = serde_saphyr::to_string(&json!([
        {"id":"rule:root","targetClass":"requirement","node":"rule:shared_path"},
        reusable
    ]))
    .unwrap();
    fs::write(root.join("rules.yaml"), named).unwrap();
    let named_failure =
        diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(named_failure["diagnostics"].as_array().unwrap().len(), 1);
    assert_eq!(
        named_failure["diagnostics"][0]["rule"],
        "urn:mara:rule:root"
    );
    assert!(
        named_failure["diagnostics"][0]["location"]["pointer"]
            .as_str()
            .unwrap()
            .starts_with("/1/")
    );
    assert!(
        named_failure["diagnostics"][0]["location"]["pointer"]
            .as_str()
            .unwrap()
            .ends_with("/path")
    );

    let mut branched: Value = serde_saphyr::from_str(&chain(9)).unwrap();
    let branch = branched["property"][0].clone();
    branched["property"] = json!([branch.clone(), branch]);
    fs::write(
        root.join("rules.yaml"),
        serde_saphyr::to_string(&branched).unwrap(),
    )
    .unwrap();
    let branched_failure =
        diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    let diagnostics = branched_failure["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 2, "{branched_failure:#}");
    assert!(
        diagnostics
            .iter()
            .all(|d| d["rule"] == "urn:mara:rule:bounded_chain")
    );
    assert_ne!(
        diagnostics[0]["location"]["pointer"],
        diagnostics[1]["location"]["pointer"]
    );

    branched["id"] = json!("rule:shared_branches");
    branched.as_object_mut().unwrap().remove("targetClass");
    let named_branches = json!([
        {"id":"rule:branch_root","targetClass":"requirement","node":"rule:shared_branches"},
        branched
    ]);
    fs::write(
        root.join("rules.yaml"),
        serde_saphyr::to_string(&named_branches).unwrap(),
    )
    .unwrap();
    let named_branches_failure =
        diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    let diagnostics = named_branches_failure["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 2, "{named_branches_failure:#}");
    assert!(
        diagnostics
            .iter()
            .all(|d| d["rule"] == "urn:mara:rule:branch_root")
    );

    let references = |wrappers| {
        let mut shape = json!({"class":"requirement"});
        for _ in 0..wrappers {
            shape = json!({"node":shape});
        }
        shape["id"] = json!("rule:bounded_shapes");
        shape["targetClass"] = json!("requirement");
        serde_saphyr::to_string(&shape).unwrap()
    };
    fs::write(root.join("rules.yaml"), references(31)).unwrap();
    let allowed = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(allowed["valid"], true, "{allowed:#}");
    fs::write(root.join("rules.yaml"), references(32)).unwrap();
    let too_deep = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(too_deep["diagnostics"][0]["code"], "rule_invalid");
    assert!(
        too_deep["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("depth above 32")
    );
}

// @mara checks DES-TRACE-GRAPH-CONSTRAINTS
#[test]
fn structural_relation_policies_validate_normalized_edges_through_cli_and_mcp() {
    let fixture = fixture();
    let root = fixture.path();
    legacy_engineering_project(root);
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["relations"]["verifies"]["inverse"] = json!("verified_by");
    schema["relations"]["verifies"]["cardinality"] = json!({"outgoing":{"minimum":1,"maximum":1}});
    schema["relations"]["depends_on"]["inverse"] = json!("depended_on_by");
    schema["relations"]["depends_on"]["acyclic"] = json!({"severity":"warning"});
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    let items = r#":::mara requirement REQ-A
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: A

A.
:::

:::mara requirement REQ-B
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: B

B.
:::

:::mara requirement REQ-C
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: C

C.
:::

:::mara verification VER-A
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F03
:title: Verification

Checks the requirements.
:::
"#;
    let item_path = root.join("items.mara.md");
    fs::write(&item_path, items).unwrap();
    let schema_result =
        diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(schema_result["valid"], true, "{schema_result:#}");
    let missing = validation_with_parity(root, &[]);
    assert_eq!(missing["summary"]["errors"], 1, "{missing:#}");
    assert_eq!(missing["diagnostics"][0]["code"], "relation_cardinality");
    assert_eq!(missing["diagnostics"][0]["details"]["actual"], 0);

    let one = items
        .replace(":title: A", ":title: A\n:verified_by: VER-A")
        .replace(
            ":title: Verification",
            ":title: Verification\n:verifies: REQ-A\n:verifies: REQ-A",
        );
    fs::write(&item_path, &one).unwrap();
    let pass = validation_with_parity(root, &[]);
    assert_eq!(pass["valid"], true, "{pass:#}");
    let selected = diagnostic_parity(
        root,
        &["item", "validate", "VER-A"],
        "item_validate",
        json!({"id":"VER-A"}),
    );
    assert_eq!(selected["valid"], true, "{selected:#}");

    let too_many = one.replace(
        ":verifies: REQ-A\n:verifies: REQ-A",
        ":verifies: REQ-A\n:verifies: REQ-A\n:verifies: REQ-B",
    );
    fs::write(&item_path, &too_many).unwrap();
    let excess = validation_with_parity(root, &[]);
    assert_eq!(excess["summary"]["errors"], 1, "{excess:#}");
    assert_eq!(excess["diagnostics"][0]["details"]["actual"], 2);
    let hidden = validation_with_parity(root, &["absent/"]);
    assert_eq!(hidden["diagnostics"], json!([]));
    assert_eq!(hidden["summary"], excess["summary"]);
    assert_eq!(hidden["selection"]["omitted_diagnostics"], 1);

    let chain = one
        .replace(":title: A", ":title: A\n:depends_on: REQ-B")
        .replace(":title: B", ":title: B\n:depends_on: REQ-C");
    fs::write(&item_path, &chain).unwrap();
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    let cycle = chain.replace(":title: A", ":title: A\n:depended_on_by: REQ-C");
    fs::write(&item_path, &cycle).unwrap();
    let warned = validation_with_parity(root, &[]);
    assert_eq!(warned["valid"], true, "{warned:#}");
    assert_eq!(warned["summary"]["warnings"], 1);
    let diagnostic = &warned["diagnostics"][0];
    assert_eq!(diagnostic["code"], "relation_cycle");
    assert_eq!(diagnostic["severity"], "warning");
    let witness = diagnostic["details"]["witness"].as_array().unwrap();
    assert_eq!(witness.len(), 3, "{diagnostic:#}");
    assert_eq!(witness[2]["edge"]["source"]["id"], "REQ-C");
    assert!(
        witness
            .iter()
            .all(|edge| edge["reference"].as_str().unwrap().starts_with("occ-1-"))
    );
    let item_cycle = diagnostic_parity(
        root,
        &["item", "validate", "REQ-B"],
        "item_validate",
        json!({"id":"REQ-B"}),
    );
    assert_eq!(item_cycle["diagnostics"][0]["code"], "relation_cycle");
    assert_eq!(item_cycle["diagnostics"][0]["item"]["id"], "REQ-B");

    let two_components = cycle.replace(
        ":title: Verification",
        ":title: Verification\n:depends_on: VER-A",
    );
    fs::write(&item_path, two_components).unwrap();
    assert_eq!(validation_with_parity(root, &[])["summary"]["warnings"], 2);

    let self_loop = one.replace(":title: C", ":title: C\n:depends_on: REQ-C");
    fs::write(&item_path, &self_loop).unwrap();
    let self_result = validation_with_parity(root, &[]);
    assert_eq!(self_result["summary"]["warnings"], 1);
    assert_eq!(
        self_result["diagnostics"][0]["details"]["witness"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let unconstrained = one
        .replace(":title: A", ":title: A\n:supersedes: REQ-B")
        .replace(":title: B", ":title: B\n:supersedes: REQ-A");
    fs::write(&item_path, &unconstrained).unwrap();
    assert_eq!(validation_with_parity(root, &[])["valid"], true);

    fs::write(
        &item_path,
        one.replace(
            ":verifies: REQ-A\n:verifies: REQ-A",
            ":verifies: REQ-A\n:verifies: REQ-MISSING",
        ),
    )
    .unwrap();
    let unavailable = validation_with_parity(root, &[]);
    assert_eq!(unavailable["evaluation_complete"], false);
    assert!(
        unavailable["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "reference_unresolved")
    );
    assert!(
        unavailable["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "evaluation_unavailable")
    );
    assert!(
        !unavailable["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "relation_cardinality")
    );
    fs::write(&item_path, &one).unwrap();

    schema["relations"]["verifies"]["cardinality"] = json!({"outgoing":{"minimum":2,"maximum":1}});
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    let invalid = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(invalid["valid"], false);
    assert_eq!(invalid["diagnostics"][0]["code"], "schema_invalid");
    assert_eq!(
        invalid["diagnostics"][0]["location"]["pointer"],
        "/relations/verifies/cardinality/outgoing"
    );
    for invalid_policy in [json!(null), json!({"outgoing":null})] {
        schema["relations"]["verifies"]["cardinality"] = invalid_policy;
        fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
        let invalid =
            diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
        assert_eq!(
            invalid["diagnostics"][0]["code"], "schema_invalid",
            "{invalid:#}"
        );
    }
    schema["relations"]["verifies"]
        .as_object_mut()
        .unwrap()
        .remove("cardinality");
    schema["relations"]["depends_on"]["acyclic"] = json!(null);
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    let invalid = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(
        invalid["diagnostics"][0]["code"], "schema_invalid",
        "{invalid:#}"
    );
}

// @mara checks DES-TRACE-GRAPH-CONSTRAINTS
#[test]
fn structural_counts_cover_incoming_symmetric_and_external_targets() {
    let fixture = fixture();
    let root = fixture.path();
    legacy_engineering_project(root);
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["relations"]["verifies"]["cardinality"] = json!({"incoming":{"maximum":0}});
    schema["relations"]["associated_with"] = json!({
        "description":"Undirected association.", "source":["requirement"], "target":["requirement"],
        "symmetric":true, "cardinality":{"symmetric":{"maximum":1}}
    });
    schema["relations"]["tracked_by"] = json!({
        "description":"External ticket.", "source":["requirement"], "target":[],
        "external":true, "cardinality":{"outgoing":{"minimum":1,"maximum":1}}
    });
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    let items = r#":::mara requirement REQ-A
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: A
:associated_with: REQ-B
:tracked_by: external:https://example.com/ticket/1

A.
:::

:::mara requirement REQ-B
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: B
:associated_with: REQ-A
:tracked_by: external:https://example.com/ticket/2

B.
:::

:::mara verification VER-A
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: Verification
:verifies: REQ-A

Checks A.
:::
"#;
    let item_path = root.join("items.mara.md");
    fs::write(&item_path, items).unwrap();
    let incoming = validation_with_parity(root, &[]);
    assert_eq!(incoming["summary"]["errors"], 1, "{incoming:#}");
    assert_eq!(
        incoming["diagnostics"][0]["details"]["direction"],
        "incoming"
    );
    assert_eq!(incoming["diagnostics"][0]["item"]["id"], "REQ-A");

    schema["relations"]["verifies"]
        .as_object_mut()
        .unwrap()
        .remove("cardinality");
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    schema["relations"]["associated_with"]["cardinality"] = json!({"symmetric":{"maximum":0}});
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    let symmetric = validation_with_parity(root, &[]);
    assert_eq!(symmetric["summary"]["errors"], 2, "{symmetric:#}");
    assert!(
        symmetric["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["details"]["actual"] == 1)
    );

    schema["relations"]["associated_with"]["cardinality"] = json!({"symmetric":{"maximum":1}});
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    fs::write(
        &item_path,
        items.replace(":tracked_by: external:https://example.com/ticket/2\n", ""),
    )
    .unwrap();
    let external = validation_with_parity(root, &[]);
    assert_eq!(external["summary"]["errors"], 1, "{external:#}");
    assert_eq!(external["diagnostics"][0]["item"]["id"], "REQ-B");
    assert_eq!(
        external["diagnostics"][0]["details"]["direction"],
        "outgoing"
    );
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn current_state_rules_preserve_warnings_prerequisites_and_continuations() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let rules = fs::read_to_string(root.join("rules.yaml")).unwrap();
    fs::write(
        root.join("rules.yaml"),
        rules.replace("  targetClass:", "  severity: Warning\n  targetClass:"),
    )
    .unwrap();
    let warning = validation_with_parity(root, &[]);
    assert_eq!(warning["valid"], true, "{warning:#}");
    assert_eq!(warning["summary"]["warnings"], 3);
    let hidden = validation_with_parity(root, &["absent/"]);
    assert_eq!(hidden["valid"], true);
    assert_eq!(hidden["summary"], warning["summary"]);
    assert_eq!(hidden["diagnostics"], json!([]));
    let first = mara(
        root,
        &["--format", "json", "project", "validate", "--limit", "1"],
    );
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(first["has_more"], true);
    let cursor = first["next_cursor"].as_str().unwrap();
    fs::write(
        root.join("rules.yaml"),
        format!("{rules}\n# changed snapshot\n"),
    )
    .unwrap();
    let stale = mara(
        root,
        &[
            "--format", "json", "project", "validate", "--limit", "1", "--cursor", cursor,
        ],
    );
    let stale: Value = serde_json::from_slice(&stale.stdout).unwrap();
    assert_eq!(stale["error"]["code"], "stale_cursor");
    // Invalid status cannot turn applicability into false or a missing field.
    let file = root.join("items.mara.md");
    let source = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        source.replace(":status: approved", ":status: invalid"),
    )
    .unwrap();
    let invalid = validation_with_parity(root, &[]);
    assert_eq!(invalid["evaluation_complete"], false, "{invalid:#}");
    assert!(
        invalid["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "evaluation_unavailable" && d["scope"] == "project")
    );
    fs::write(&file, source).unwrap();
    // A corrupt qualifying endpoint remains unavailable even with another qualifying endpoint.
    for id in ["VER-DRAFT", "VER-APPROVED"] {
        assert!(
            mara(root, &["relation", "add", id, "verifies", "REQ-A"])
                .status
                .success()
        );
    }
    let source = fs::read_to_string(&file).unwrap();
    fs::write(&file, source.replace(":status: draft", ":status: invalid")).unwrap();
    let invalid = validation_with_parity(root, &[]);
    assert_eq!(invalid["evaluation_complete"], false);
    assert!(
        invalid["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "evaluation_unavailable" && d["scope"] == "project"),
        "{invalid:#}"
    );
    // Schema validation loads definitions but never evaluates item conformance.
    let schema = mara(root, &["--format", "json", "schema", "validate"]);
    let schema: Value = serde_json::from_slice(&schema.stdout).unwrap();
    assert_eq!(schema["valid"], true, "{schema:#}");
    fs::write(root.join("rules.yaml"),"id: rule:oops\ntype: NodeShape\ntargetClass: requirement\nproperty:\n  - path: owner\n    minCont: 1\n").unwrap();
    let bad = validation_with_parity(root, &[]);
    let d = bad["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "rule_invalid")
        .unwrap();
    assert_eq!(d["location"]["pointer"], "/property/0/minCont");
    assert_eq!(d["rule"], "urn:mara:rule:oops");
    assert_eq!(d["location"]["line"], 6);
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn current_state_rules_literal_semantics_definition_errors_and_patterns() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let numeric = "- id: rule:score_requires_owner\n  type: NodeShape\n  targetClass: requirement\n  whenShape: rule:score_one\n  property: [{path: owner, minCount: 1}]\n- id: rule:score_one\n  type: NodeShape\n  property: [{path: score, hasValue: {value: 1, datatype: double}}]\n";
    fs::write(root.join("rules.yaml"), numeric).unwrap();
    for score in ["1", "1.0"] {
        assert!(
            mara(
                root,
                &[
                    "item",
                    "update",
                    "REQ-A",
                    "--field",
                    &format!("score={score}"),
                    "--clear-field",
                    "owner"
                ]
            )
            .status
            .success()
        );
        let v = validation_with_parity(root, &[]);
        assert_eq!(v["summary"]["errors"], 1, "{v:#}");
        assert!(
            mara(root, &["item", "update", "REQ-A", "--field", "owner=Alice"])
                .status
                .success()
        );
        assert_eq!(validation_with_parity(root, &[])["valid"], true);
    }
    fs::write(
        root.join("rules.yaml"),
        numeric.replace("{value: 1, datatype: double}", "1.0"),
    )
    .unwrap();
    assert!(
        mara(root, &["item", "update", "REQ-A", "--clear-field", "owner"])
            .status
            .success()
    );
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    for bad in [
        "id: rule:a\nid: rule:b\n",
        "id: rule:a\n@context: {}\n",
        "id: rule:a\nnode: rule:missing\n",
        "id: rule:a\nnode: rule:a\n",
        "id: rule:a\ntargetNode: REQ-A\n",
        "id: rule:a\nproperty: [{path: owner, minCount: null}]\n",
        "id: rule:a\nproperty: [{path: owner, hasValue: {value: '1', datatype: double}}]\n",
        "id: rule:a\n<<: {type: NodeShape}\n",
    ] {
        fs::write(root.join("rules.yaml"), bad).unwrap();
        let v = validation_with_parity(root, &[]);
        assert_eq!(v["valid"], false, "{bad}: {v:#}");
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "rule_invalid"),
            "{v:#}"
        );
    }
    fs::write(root.join("rules.yaml"),"id: rule:pattern\ntype: NodeShape\ntargetClass: requirement\nproperty: [{path: owner, pattern: 'a{100}'}]\n").unwrap();
    assert!(
        mara(
            root,
            &[
                "item",
                "update",
                "REQ-A",
                "--field",
                &format!("owner={}", "a".repeat(2000))
            ]
        )
        .status
        .success()
    );
    let complete = validation_with_parity(root, &[]);
    assert_eq!(complete["valid"], true, "{complete:#}");
    assert_eq!(complete["evaluation_complete"], true);
    assert!(complete.get("work").is_none());
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn current_state_rules_scope_empty_sets_composition_and_schema_sources() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let every = "id: rule:every\ntargetClass: requirement\nproperty:\n  - id: rule:edges\n    path: {inversePath: verifies}\n    node: {class: verification, property: [{path: status, hasValue: approved}]}\n";
    fs::write(root.join("rules.yaml"), every).unwrap();
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    let item = diagnostic_parity(
        root,
        &["item", "validate", "REQ-A"],
        "item_validate",
        json!({"id":"REQ-A"}),
    );
    assert_eq!(item["valid"], true);
    // Named PropertyShape descriptions merge across files without redefining path/type.
    let config = root.join(".mara/project.toml");
    let original = fs::read_to_string(&config).unwrap();
    fs::write(
        &config,
        original.replace("[\"rules.yaml\"]", "[\"rules.yaml\", \"extra.yml\"]"),
    )
    .unwrap();
    fs::write(root.join("extra.yml"), "id: rule:edges\nminCount: 1\n").unwrap();
    let failed = validation_with_parity(root, &[]);
    assert_eq!(failed["summary"]["errors"], 1, "{failed:#}");
    assert_eq!(
        failed["diagnostics"][0]["obligation"]["source"]["path"],
        "extra.yml"
    );
    assert_eq!(
        diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}))["valid"],
        true
    );
    fs::write(&config, &original).unwrap();
    // Invalid corpus prerequisites skip policy, regardless of a passing OR alternative.
    let logical = "id: rule:logic\ntargetClass: requirement\nor:\n  - class: requirement\n  - property:\n      - path: {inversePath: verifies}\n        qualifiedValueShape: {class: verification}\n        qualifiedMinCount: 1\n";
    fs::write(root.join("rules.yaml"), logical).unwrap();
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    assert!(
        mara(root, &["relation", "add", "VER-DRAFT", "verifies", "REQ-A"])
            .status
            .success()
    );
    let file = root.join("items.mara.md");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replace(":status: draft", ":status: invalid")).unwrap();
    let incomplete = diagnostic_parity(
        root,
        &["item", "validate", "REQ-A"],
        "item_validate",
        json!({"id":"REQ-A"}),
    );
    assert_eq!(incomplete["evaluation_complete"], false, "{incomplete:#}");
    assert!(
        incomplete["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "field_invalid" && d["item"]["id"] == "VER-DRAFT"),
        "{incomplete:#}"
    );
    assert!(
        incomplete["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["code"] != "rule_failed")
    );
    fs::write(&file, &text).unwrap();
    // Only the root path scope selects items, and paths are project relative.
    fs::write(root.join("rules.yaml"),"id: rule:scope\ntargetClass: requirement\npaths: [other/]\nproperty: [{path: owner, maxCount: 0}]\n").unwrap();
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    fs::write(root.join("rules.yaml"),"id: rule:scope\ntargetClass: requirement\npaths: [items.mara.md]\nproperty: [{path: owner, maxCount: 0}]\n").unwrap();
    assert_eq!(validation_with_parity(root, &[])["summary"]["errors"], 1);
    // Deliberate configuration adoption: existing format 1 cannot enable sources.
    fs::write(
        &config,
        original.replace("format_version = 2", "format_version = 1"),
    )
    .unwrap();
    assert_eq!(
        diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}))["valid"],
        false
    );
    fs::write(&config, original.replace("[\"rules.yaml\"]", "[]")).unwrap();
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn current_state_rules_deduplicate_values_and_reject_unsupported_definitions() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["flavours"]["requirement"]["fields"]["owner"]["repeatable"] = json!(true);
    schema["flavours"]["requirement"]["fields"]["class"] = json!({"type":"string"});
    fs::write(schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    let file = root.join("items.mara.md");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replacen(
            ":owner: Alice",
            ":owner: Alice\n:owner: Alice\n:class: requirement",
            1,
        ),
    )
    .unwrap();
    fs::write(root.join("rules.yaml"),"id: rule:fields\ntargetClass: requirement\npaths: ['./items.mara.md']\nand:\n  - property: [{path: owner, minCount: 1, maxCount: 1, in: [Alice, Bob]}]\n  - property: [{path: class, hasValue: requirement}]\nnot: {property: [{path: status, hasValue: draft}]}\n").unwrap();
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    let fields = fs::read_to_string(root.join("rules.yaml")).unwrap();
    fs::write(
        root.join("rules.yaml"),
        fields.replace("minCount: 1", "minCount: 2"),
    )
    .unwrap();
    assert_eq!(validation_with_parity(root, &[])["summary"]["errors"], 1);
    for bad in [
        "- rule:reference\n",
        "id: rule:x\ntargetClass: [requirement, 3]\n",
        "id: rule:x\ntargetClass: requirement\nproperty: [{path: unknown, minCount: 1}]\n",
        "id: rule:x\ntargetClass: requirement\nproperty: [{path: owner, datatype: boolean}]\n",
        "id: rule:x\ntargetClass: requirement\nproperty: [{path: owner, pattern: '['}]\n",
        "id: rule:x\ntargetClass: requirement\nproperty: [{path: owner, severity: Warning}]\n",
        "id: rule:x\ntargetClass: requirement\nproperty: [{path: owner, qualifiedMinCount: 1}]\n",
        "id: rule:x\n---\nid: rule:y\n",
        "id: rule:x\nnode: !custom {class: requirement}\n",
    ] {
        fs::write(root.join("rules.yaml"), bad).unwrap();
        let v = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
        assert_eq!(v["valid"], false, "{bad}: {v:#}");
    }
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn non_finite_number_fields_report_the_authored_field_before_rule_projection() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let file = root.join("items.mara.md");
    let source = fs::read_to_string(&file).unwrap();
    for value in ["NaN", "inf", "-inf", "1e999"] {
        fs::write(
            &file,
            source.replacen(
                ":owner: Alice",
                &format!(":score: {value}\n:owner: Alice"),
                1,
            ),
        )
        .unwrap();
        for selection in [None, Some("REQ-A")] {
            let result = if let Some(id) = selection {
                diagnostic_parity(
                    root,
                    &["item", "validate", id],
                    "item_validate",
                    json!({"id":id}),
                )
            } else {
                validation_with_parity(root, &[])
            };
            assert_eq!(result["evaluation_complete"], false, "{value}: {result:#}");
            let diagnostics = result["diagnostics"].as_array().unwrap();
            let field = diagnostics
                .iter()
                .find(|d| d["code"] == "field_invalid" && d["item"]["id"] == "REQ-A")
                .unwrap_or_else(|| panic!("missing field diagnostic for {value}: {result:#}"));
            assert_eq!(field["location"]["path"], "items.mara.md");
            assert!(field["location"]["line"].as_u64().is_some());
            assert!(field["message"].as_str().unwrap().contains(value));
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d["code"] == "evaluation_unavailable")
            );
        }
    }
    fs::write(
        &file,
        source.replacen(":owner: Alice", ":score: 1e308\n:owner: Alice", 1),
    )
    .unwrap();
    let finite = validation_with_parity(root, &[]);
    assert_eq!(finite["evaluation_complete"], true, "{finite:#}");
    assert!(
        finite["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| { d["code"] != "field_invalid" && d["code"] != "evaluation_unavailable" })
    );
}

// @mara checks DES-CURRENT-STATE-EVALUATION
#[test]
fn external_ticket_obligation_and_navigation_have_cli_mcp_parity() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema: Value =
        serde_saphyr::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["relations"]["tracked_by"] = json!({
        "description":"Local delivery ticket reference.", "source":["requirement"],
        "target":[], "external":true
    });
    fs::write(&schema_path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    fs::write(root.join("rules.yaml"),
        "- id: rule:approved_ticket\n  targetClass: requirement\n  whenShape: rule:approved\n  property: [{path: tracked_by, minCount: 1}]\n- id: rule:approved\n  property: [{path: status, hasValue: approved}]\n"
    ).unwrap();
    let missing = validation_with_parity(root, &[]);
    assert_eq!(missing["valid"], false, "{missing:#}");
    assert!(
        missing["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "rule_failed")
    );
    assert!(
        mara(
            root,
            &["item", "update", "REQ-A", "--field", "status=draft"]
        )
        .status
        .success()
    );
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    assert!(
        mara(
            root,
            &["item", "update", "REQ-A", "--field", "status=approved"]
        )
        .status
        .success()
    );
    let target = "external:https://linear.example.invalid/ticket/ENG-7?view=full#notes";
    let add = mara(
        root,
        &[
            "--format",
            "json",
            "relation",
            "add",
            "REQ-A",
            "tracked_by",
            target,
        ],
    );
    assert!(add.status.success(), "{}", stderr(&add));
    let added: Value = serde_json::from_slice(&add.stdout).unwrap();
    assert_eq!(
        added["edge"]["target"],
        json!({"kind":"external","address":"https://linear.example.invalid/ticket/ENG-7?view=full#notes"})
    );
    let inspected = relation_tool(
        root,
        "relation_get",
        json!({"source":"REQ-A","relation":"tracked_by","target":target}),
    );
    assert_eq!(inspected["edge"], added["edge"]);
    assert_eq!(inspected["occurrence_count"], 1);
    let related = related_cli_mcp(root, "REQ-A", &[("--relation", "tracked_by")]);
    assert_eq!(
        related["connections"][0]["neighbour"],
        added["edge"]["target"]
    );
    assert_eq!(related["connections"][0]["occurrence_count"], 1);
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    let remove = mara(
        root,
        &[
            "--format",
            "json",
            "relation",
            "remove",
            "REQ-A",
            "tracked_by",
            target,
        ],
    );
    assert!(remove.status.success(), "{}", stderr(&remove));
    assert_eq!(validation_with_parity(root, &[])["valid"], false);
    let mcp_added = relation_tool(
        root,
        "relation_add",
        json!({"source":"REQ-A","relation":"tracked_by","target":target}),
    );
    assert_eq!(mcp_added["edge"], added["edge"]);
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    let mcp_removed = relation_tool(
        root,
        "relation_remove",
        json!({"source":"REQ-A","relation":"tracked_by","target":target}),
    );
    assert_eq!(mcp_removed["edge"], added["edge"]);
    assert_eq!(validation_with_parity(root, &[])["valid"], false);
    fs::write(root.join("rules.yaml"),
        "id: rule:remote_status\ntargetClass: requirement\nproperty: [{path: tracked_by, node: {property: [{path: status, hasValue: approved}]}}]\n"
    ).unwrap();
    let invalid = validation_with_parity(root, &[]);
    assert_eq!(invalid["valid"], false);
    assert!(
        invalid["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "rule_invalid"),
        "{invalid:#}"
    );
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

fn related_cli_mcp(root: &Path, reference: &str, filters: &[(&str, &str)]) -> Value {
    let mut args = vec!["--format", "json", "related", reference];
    let mut params = json!({"reference":reference});
    for (flag, value) in filters {
        args.extend([*flag, *value]);
        match *flag {
            "--limit" => params["limit"] = json!(value.parse::<usize>().unwrap()),
            "--relation" => {
                if params.get("relations").is_none() {
                    params["relations"] = json!([]);
                }
                params["relations"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!(value));
            }
            "--flavour" => params["flavours"] = json!([value]),
            flag => params[flag.trim_start_matches("--")] = json!(value),
        }
    }
    let output = mara(root, &args);
    assert!(output.status.success(), "{}", stdout(&output));
    let page: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(serde_json::to_vec(&page).unwrap().len() <= 65_536);
    assert_eq!(page["format_version"], 2);
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "related", params),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        page
    );
    page
}
