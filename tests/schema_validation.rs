mod support;
use serde_json::{Value, json};
use std::{fs, path::Path};
use support::*;
use tempfile::TempDir;

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

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn schema_validation_rejects_unknown_relation_endpoints() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace("target: [scenario, requirement]", "target: [missing]"),
    )
    .unwrap();

    let validate = mara(fixture.path(), &["schema", "validate"]);

    assert!(!validate.status.success());
    assert!(
        stderr(&validate)
            .contains("relation 'derives_from' target references unknown flavour 'missing'"),
        "{}",
        stderr(&validate)
    );
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn schema_validation_rejects_an_id_prefix_with_an_empty_segment() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace("id_prefix: REQ-", "id_prefix: REQ--"),
    )
    .unwrap();

    let validate = mara(fixture.path(), &["schema", "validate"]);

    assert!(!validate.status.success());
    assert!(
        stderr(&validate).contains("flavour 'requirement' has invalid ID prefix 'REQ--'"),
        "{}",
        stderr(&validate)
    );
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn schema_validation_rejects_structural_names_as_custom_fields() {
    for field in ["mid", "flavour", "id", "title", "body"] {
        let fixture = fixture();
        let init = mara(fixture.path(), &["project", "init"]);
        assert!(init.status.success(), "{}", stderr(&init));
        let schema_file = fixture.path().join(".mara/schema.yaml");
        let schema = fs::read_to_string(&schema_file).unwrap();
        fs::write(
            &schema_file,
            schema.replace(
                "    id_prefix: REQ-\n    body: required\n    fields: {}",
                &format!(
                    "    id_prefix: REQ-\n    body: required\n    fields:\n      {field}:\n        type: string"
                ),
            ),
        )
        .unwrap();

        let validate = mara(fixture.path(), &["schema", "validate"]);

        assert!(!validate.status.success(), "field '{field}' was accepted");
        assert!(
            stderr(&validate).contains(&format!(
                "flavour 'requirement' field '{field}' is reserved for item structure"
            )),
            "{}",
            stderr(&validate)
        );
    }
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn schema_validation_rejects_enum_values_with_surrounding_whitespace() {
    for value in [" draft ", "   "] {
        let fixture = fixture();
        let init = mara(fixture.path(), &["project", "init"]);
        assert!(init.status.success(), "{}", stderr(&init));
        let schema_file = fixture.path().join(".mara/schema.yaml");
        let schema = fs::read_to_string(&schema_file).unwrap();
        fs::write(
            &schema_file,
            schema.replace(
                "    id_prefix: REQ-\n    body: required\n    fields: {}",
                &format!(
                    "    id_prefix: REQ-\n    body: required\n    fields:\n      status:\n        type: enum\n        values: [\"{value}\"]"
                ),
            ),
        )
        .unwrap();

        let validate = mara(fixture.path(), &["schema", "validate"]);

        assert!(
            !validate.status.success(),
            "enum value '{value}' was accepted"
        );
        assert!(
            stderr(&validate).contains(
                "flavour 'requirement' enum field 'status' values must not have surrounding whitespace"
            ),
            "{}",
            stderr(&validate)
        );
    }
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn schema_validation_rejects_structural_names_as_relations() {
    for relation in ["mid", "flavour", "id", "title", "body"] {
        let fixture = fixture();
        let init = mara(fixture.path(), &["project", "init"]);
        assert!(init.status.success(), "{}", stderr(&init));
        let schema_file = fixture.path().join(".mara/schema.yaml");
        let schema = fs::read_to_string(&schema_file).unwrap();
        fs::write(
            &schema_file,
            schema.replace("  derives_from:\n", &format!("  {relation}:\n")),
        )
        .unwrap();

        let validate = mara(fixture.path(), &["schema", "validate"]);

        assert!(
            !validate.status.success(),
            "relation '{relation}' was accepted"
        );
        assert!(
            stderr(&validate).contains(&format!(
                "relation '{relation}' is reserved for item structure"
            )),
            "{}",
            stderr(&validate)
        );
    }
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn schema_validation_rejects_relation_and_source_field_name_collisions() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace(
            "    id_prefix: SCN-\n    body: required\n    fields: {}",
            "    id_prefix: SCN-\n    body: required\n    fields:\n      depends_on:\n        type: string",
        ),
    )
    .unwrap();

    let validate = mara(fixture.path(), &["schema", "validate"]);

    assert!(!validate.status.success());
    assert!(
        stderr(&validate).contains(
            "relation 'depends_on' conflicts with field 'depends_on' on source flavour 'scenario'"
        ),
        "{}",
        stderr(&validate)
    );
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn diagnostic_configuration_failures_keep_typed_locations_and_schema_envelope() {
    let fixture = fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let schema = fixture.path().join(".mara/schema.yaml");
    let original = fs::read_to_string(&schema).unwrap();
    fs::write(
        &schema,
        original.replacen("format_version: 3", "format_version: 2", 1),
    )
    .unwrap();
    let unsupported = diagnostic_parity(
        fixture.path(),
        &["schema", "validate"],
        "schema_validate",
        json!({}),
    );
    assert_eq!(unsupported["target"]["kind"], "schema");
    assert_eq!(unsupported["diagnostics"][0]["code"], "format_unsupported");
    assert_eq!(
        unsupported["diagnostics"][0]["location"]["pointer"],
        "/format_version"
    );
    assert!(unsupported["flavours"].is_null());
    fs::write(&schema, "format_version: 3\nflavours: [\n").unwrap();
    let malformed = diagnostic_parity(
        fixture.path(),
        &["schema", "validate"],
        "schema_validate",
        json!({}),
    );
    assert_eq!(malformed["diagnostics"][0]["code"], "schema_invalid");
    assert!(
        malformed["diagnostics"][0]["location"]["line"]
            .as_u64()
            .is_some()
    );
    assert_eq!(malformed["evaluation_complete"], false);
    assert_eq!(malformed["summary"]["counts_exact"], false);
    fs::write(&schema, &original).unwrap();
    let config = fixture.path().join(".mara/project.toml");
    fs::write(config, "format_version = [\n").unwrap();
    let malformed = diagnostic_parity(
        fixture.path(),
        &["schema", "validate"],
        "schema_validate",
        json!({}),
    );
    assert_eq!(malformed["diagnostics"][0]["code"], "project_invalid");
    assert!(
        malformed["diagnostics"][0]["location"]["line"]
            .as_u64()
            .is_some()
    );
    assert_eq!(malformed["evaluation_complete"], false);
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn rejected_rule_sources_do_not_affect_validation_cursors() {
    let fixture = fixture();
    let root = fixture.path().join("project");
    fs::create_dir(&root).unwrap();
    assert!(mara(&root, &["project", "init"]).status.success());
    let outside = fixture.path().join("outside.yaml");
    fs::write(&outside, "id: rule:outside\n").unwrap();
    let config_path = root.join(".mara/project.toml");
    let config = fs::read_to_string(&config_path).unwrap().replacen(
        "format_version = 1",
        "format_version = 2",
        1,
    );
    fs::write(&config_path,
        format!("{config}\n[rules]\nformat_version = 1\nfiles = [\"../outside.yaml\", \"missing.yaml\"]\n")
    ).unwrap();
    let first = diagnostic_parity(
        &root,
        &["schema", "validate", "--limit", "1"],
        "schema_validate",
        json!({"limit":1}),
    );
    assert_eq!(first["valid"], false, "{first:#}");
    assert_eq!(first["has_more"], true, "{first:#}");
    assert_eq!(first["diagnostics"][0]["code"], "rule_invalid");
    let cursor = first["next_cursor"].as_str().unwrap();
    fs::write(
        &outside,
        "id: rule:changed\nmessage: unrelated external bytes\n",
    )
    .unwrap();
    let continued = diagnostic_parity(
        &root,
        &["schema", "validate", "--limit", "1", "--cursor", cursor],
        "schema_validate",
        json!({"limit":1,"cursor":cursor}),
    );
    assert!(continued.get("error").is_none(), "{continued:#}");
    assert_eq!(continued["diagnostics"][0]["code"], "rule_invalid");
    assert_eq!(continued["has_more"], false);
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn unused_class_scoped_shapes_validate_field_compatibility() {
    let fixture = rule_fixture();
    let root = fixture.path();
    fs::write(
        root.join("rules.yaml"),
        "id: rule:unused\nclass: requirement\nproperty: [{path: owner, datatype: boolean}]\n",
    )
    .unwrap();
    let invalid = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(invalid["valid"], false, "{invalid:#}");
    assert_eq!(invalid["diagnostics"][0]["code"], "rule_invalid");
    assert_eq!(
        invalid["diagnostics"][0]["location"]["pointer"],
        "/property/0/datatype"
    );
    fs::write(
        root.join("rules.yaml"),
        "id: rule:unused\nclass: requirement\nproperty: [{path: owner, datatype: string}]\n",
    )
    .unwrap();
    let valid = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(valid["valid"], true, "{valid:#}");
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn current_state_rules_reject_disjoint_relation_endpoint_classes() {
    let fixture = rule_fixture();
    let root = fixture.path();
    for (target, path, class) in [
        ("design", json!("satisfies"), "design"),
        (
            "requirement",
            json!({"inversePath":"satisfies"}),
            "requirement",
        ),
    ] {
        for key in ["class", "node", "qualifiedValueShape"] {
            let mut property = json!({"path":path});
            property[key] = if key == "class" {
                json!(class)
            } else {
                json!({"class":class})
            };
            if key == "qualifiedValueShape" {
                property["qualifiedMinCount"] = json!(1);
            }
            let rule = json!({"id":"rule:disjoint", "targetClass":target, "property":[property]});
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
    // A qualifier may select a proper subset of a relation's endpoint flavours.
    fs::write(root.join("rules.yaml"),
        "id: rule:subset\ntargetClass: verification\nproperty: [{path: verifies, qualifiedValueShape: {class: requirement}, qualifiedMinCount: 0}]\n"
    ).unwrap();
    let valid = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(valid["valid"], true, "{valid:#}");
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn current_state_rules_reject_literal_constraints_on_relation_endpoints() {
    let fixture = rule_fixture();
    let root = fixture.path();
    for (target, path) in [
        ("design", json!("satisfies")),
        ("requirement", json!({"inversePath":"satisfies"})),
    ] {
        for key in ["hasValue", "in"] {
            for nested in [false, true] {
                let mut constraint = json!({});
                constraint[key] = if key == "in" {
                    json!(["REQ-A"])
                } else {
                    json!("REQ-A")
                };
                let mut property = if nested {
                    json!({"node":constraint})
                } else {
                    constraint
                };
                property["path"] = path.clone();
                let rule = json!({"id":"rule:literal_relation", "targetClass":target, "property":[property]});
                fs::write(
                    root.join("rules.yaml"),
                    serde_saphyr::to_string(&rule).unwrap(),
                )
                .unwrap();
                let invalid =
                    diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
                assert_eq!(
                    invalid["valid"], false,
                    "{target}/{key}/{nested}: {invalid:#}"
                );
                assert_eq!(
                    invalid["diagnostics"][0]["code"], "rule_invalid",
                    "{invalid:#}"
                );
                let pointer = if nested {
                    format!("/property/0/node/{key}")
                } else {
                    format!("/property/0/{key}")
                };
                assert_eq!(invalid["diagnostics"][0]["location"]["pointer"], pointer);
            }
        }
    }
    // The same constraints remain valid on literal fields of related items.
    fs::write(root.join("rules.yaml"),
        "id: rule:related_value\ntargetClass: design\nproperty:\n  - path: satisfies\n    minCount: 1\n    node:\n      property: [{path: owner, hasValue: Alice, in: [Alice, Bob]}]\n"
    ).unwrap();
    let valid = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(valid["valid"], true, "{valid:#}");
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn current_state_rules_reject_node_constraints_on_literal_values() {
    let fixture = rule_fixture();
    let root = fixture.path();
    for (property, pointer) in [
        (
            json!({"path":"owner", "class":"requirement"}),
            "/property/0/class",
        ),
        (
            json!({"path":"score", "class":"requirement"}),
            "/property/0/class",
        ),
        (
            json!({"path":"owner", "node":{"class":"requirement"}}),
            "/property/0/node/class",
        ),
        (
            json!({"path":"owner", "qualifiedValueShape":{"class":"requirement"}, "qualifiedMinCount":0}),
            "/property/0/qualifiedValueShape/class",
        ),
        (
            json!({"path":"owner", "property":[{"path":"owner", "minCount":1}]}),
            "/property/0/property/0/path",
        ),
        (
            json!({"path":"owner", "node":{"property":[{"path":"satisfies", "minCount":1}]}}),
            "/property/0/node/property/0/path",
        ),
        (
            json!({"path":"owner", "node":{"property":[{"path":{"inversePath":"satisfies"}, "minCount":1}]}}),
            "/property/0/node/property/0/path",
        ),
        (
            json!({"path":"owner", "node":{"and":[{"class":"requirement"}]}}),
            "/property/0/node/and/0/class",
        ),
    ] {
        let rule = json!({"id":"rule:literal_endpoint", "targetClass":"requirement",
            "property":[property]});
        fs::write(
            root.join("rules.yaml"),
            serde_saphyr::to_string(&rule).unwrap(),
        )
        .unwrap();
        let result = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
        assert_eq!(result["valid"], false, "{pointer}: {result:#}");
        assert_eq!(
            result["diagnostics"][0]["code"], "rule_invalid",
            "{result:#}"
        );
        assert_eq!(result["diagnostics"][0]["location"]["pointer"], pointer);
    }
    // Literal constraints remain valid, including behind logical and node shapes.
    fs::write(root.join("rules.yaml"),
        "id: rule:literal_endpoint\ntargetClass: requirement\nproperty: [{path: owner, node: {and: [{datatype: string}, {hasValue: Alice}]}}]\n"
    ).unwrap();
    let valid = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(valid["valid"], true, "{valid:#}");
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn current_state_rules_reject_datatypes_on_relation_endpoints() {
    let fixture = rule_fixture();
    let root = fixture.path();
    for (target, path) in [
        ("design", json!("satisfies")),
        ("requirement", json!({"inversePath":"satisfies"})),
    ] {
        for key in ["datatype", "node", "qualifiedValueShape"] {
            let mut property = json!({"path":path});
            property[key] = if key == "datatype" {
                json!("string")
            } else {
                json!({"datatype":"string"})
            };
            if key == "qualifiedValueShape" {
                property["qualifiedMinCount"] = json!(0);
            }
            let rule = json!({"id":"rule:relation_type", "targetClass":target,
                "property":[property]});
            fs::write(
                root.join("rules.yaml"),
                serde_saphyr::to_string(&rule).unwrap(),
            )
            .unwrap();
            let result =
                diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
            assert_eq!(result["valid"], false, "{target}/{key}: {result:#}");
            assert_eq!(
                result["diagnostics"][0]["code"], "rule_invalid",
                "{result:#}"
            );
            let pointer = if key == "datatype" {
                "/property/0/datatype".into()
            } else {
                format!("/property/0/{key}/datatype")
            };
            assert_eq!(result["diagnostics"][0]["location"]["pointer"], pointer);
        }
    }
    // Reusing a valid literal constraint on a relation must still be rejected.
    let rules = json!([
        {"id":"rule:shared_type", "targetClass":"design", "property":[
            {"path":"owner", "node":"rule:text"},
            {"path":"satisfies", "node":"rule:text"}
        ]},
        {"id":"rule:text", "and":[{"datatype":"string"}]}
    ]);
    fs::write(
        root.join("rules.yaml"),
        serde_saphyr::to_string(&rules).unwrap(),
    )
    .unwrap();
    let invalid = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(invalid["valid"], false, "{invalid:#}");
    assert_eq!(
        invalid["diagnostics"][0]["location"]["pointer"],
        "/1/and/0/datatype"
    );
    // A relation's endpoint can instead constrain one of its literal fields.
    fs::write(root.join("rules.yaml"),
        "id: rule:related_owner\ntargetClass: design\nproperty:\n  - path: satisfies\n    minCount: 1\n    node:\n      class: requirement\n      property: [{path: owner, datatype: string, minCount: 1}]\n"
    ).unwrap();
    let valid = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(valid["valid"], true, "{valid:#}");
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn current_state_rules_preserve_field_types_in_nested_value_shapes() {
    let fixture = rule_fixture();
    let root = fixture.path();
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
                    assert_eq!(
                        diagnostic_parity(
                            root,
                            &["schema", "validate"],
                            "schema_validate",
                            json!({})
                        )["valid"],
                        true
                    );
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
}

fn rule_fixture() -> TempDir {
    let fixture = fixture();
    let root = fixture.path();
    assert!(
        mara(root, &["project", "init", "--template", "engineering"])
            .status
            .success()
    );
    let path = root.join(".mara/schema.yaml");
    let mut schema: Value = serde_saphyr::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    for flavour in ["requirement", "verification", "design", "risk"] {
        schema["flavours"][flavour]["fields"] = json!({"status":{"type":"enum","values":["draft","approved","accepted","mitigated"]},"owner":{"type":"string"},"score":{"type":"number"}});
    }
    fs::write(path, serde_saphyr::to_string(&schema).unwrap()).unwrap();
    let path = root.join(".mara/project.toml");
    let config = fs::read_to_string(&path)
        .unwrap()
        .replace(".mara/engineering-rules.yaml", "rules.yaml");
    fs::write(path, config).unwrap();
    fs::write(root.join("rules.yaml"), "[]\n").unwrap();
    fixture
}

// @mara checks REQ-FLAVOUR-AUTHORING-GUIDANCE
#[test]
fn schema_validation_reports_guidance_errors_with_stable_codes() {
    let fixture = fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let path = fixture.path().join(".mara/schema.yaml");
    let valid = "format_version: 3\nflavours:\n  note:\n    description: A project note.\n    use_when: [Record useful context.]\n    avoid_when: []\n    distinguish_from: {}\n    id_prefix: NOTE-\n    body: optional\nrelations: {}\n";
    fs::write(&path, valid).unwrap();
    let accepted = mara(fixture.path(), &["schema", "validate"]);
    assert!(accepted.status.success(), "{}", stderr(&accepted));
    let cases = [
        ("format_version: 3", "format_version: 1", "expected 3"),
        ("    description: A project note.\n", "", "description"),
        (
            "description: A project note.",
            "description: '  '",
            "description",
        ),
        (
            "description: A project note.",
            "description: 42",
            "description",
        ),
        ("    use_when: [Record useful context.]\n", "", "use_when"),
        (
            "use_when: [Record useful context.]",
            "use_when: []",
            "use_when",
        ),
        (
            "use_when: [Record useful context.]",
            "use_when: ['  ']",
            "use_when",
        ),
        (
            "use_when: [Record useful context.]",
            "use_when: context",
            "use_when",
        ),
        (
            "use_when: [Record useful context.]",
            "use_when: [true]",
            "use_when",
        ),
        ("    avoid_when: []\n", "", "avoid_when"),
        ("avoid_when: []", "avoid_when: ['  ']", "avoid_when"),
        ("avoid_when: []", "avoid_when: {}", "avoid_when"),
        ("avoid_when: []", "avoid_when: [42]", "avoid_when"),
        ("    distinguish_from: {}\n", "", "distinguish_from"),
        (
            "distinguish_from: {}",
            "distinguish_from: []",
            "distinguish_from",
        ),
        (
            "distinguish_from: {}",
            "distinguish_from: {note: '  '}",
            "distinguish_from",
        ),
        (
            "distinguish_from: {}",
            "distinguish_from: {note: true}",
            "distinguish_from",
        ),
        (
            "distinguish_from: {}",
            "distinguish_from: {note: Same flavour.}",
            "itself",
        ),
        (
            "distinguish_from: {}",
            "distinguish_from: {missing: Unknown flavour.}",
            "unknown flavour 'missing'",
        ),
        (
            "avoid_when: []",
            "avoid_when: []\n    guidance: {}",
            "unknown configuration key 'guidance'",
        ),
    ];
    for (from, to, _) in cases {
        let source = valid.replace(from, to);
        fs::write(&path, &source).unwrap();
        let cli = mara(fixture.path(), &["--format", "json", "schema", "validate"]);
        assert!(!cli.status.success(), "accepted {to}");
        let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
        let expected_code = if from == "format_version: 3" {
            "format_unsupported"
        } else {
            "schema_invalid"
        };
        assert_eq!(cli["valid"], false);
        assert!(
            cli["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == expected_code && d["severity"] == "error"),
            "{cli}"
        );
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "schema_validate", json!({})),
            ],
        );
        let result = &mcp_response(&responses, 2)["result"];
        assert_eq!(result["isError"], false, "{result}");
        assert_eq!(result["structuredContent"]["valid"], false);
        assert_eq!(result["structuredContent"], cli);
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
}

// @mara implements VER-SCHEMA-DEFINITIONS
// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn schema_validation_preserves_source_and_does_not_read_the_corpus() {
    for template in ["minimal", "empty", "engineering"] {
        let fixture = fixture();
        let root = fixture.path();
        assert!(
            mara(root, &["project", "init", "--template", template])
                .status
                .success()
        );
        let schema = fs::read(root.join(".mara/schema.yaml")).unwrap();
        fs::write(root.join("broken.mara.md"), [0xff]).unwrap();
        let value = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
        assert_eq!(value["valid"], true, "{value}");
        assert_eq!(value["evaluation_complete"], true);
        let human = mara(root, &["schema", "validate"]);
        assert!(stdout(&human).contains("valid schema"));
        assert_eq!(schema, fs::read(root.join(".mara/schema.yaml")).unwrap());
        assert_eq!(fs::read(root.join("broken.mara.md")).unwrap(), [0xff]);
    }
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn schema_diagnostics_continue_with_full_counts_and_reject_stale_inputs() {
    let fixture = rule_fixture();
    let root = fixture.path();
    let path = root.join("rules.yaml");
    let source = "- {id: 'rule:a', unknown: true}\n- {id: 'rule:b', unknown: true}\n";
    fs::write(&path, source).unwrap();
    let first = diagnostic_parity(
        root,
        &["schema", "validate", "--limit", "1"],
        "schema_validate",
        json!({"limit":1}),
    );
    assert_eq!(first["has_more"], true);
    assert_eq!(first["summary"]["errors"], 2);
    assert_eq!(first["evaluation_complete"], false);
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = diagnostic_parity(
        root,
        &["schema", "validate", "--limit", "1", "--cursor", cursor],
        "schema_validate",
        json!({"limit":1,"cursor":cursor}),
    );
    assert_eq!(second["has_more"], false);
    assert_eq!(second["summary"], first["summary"]);
    assert_ne!(second["diagnostics"], first["diagnostics"]);
    for (limit, change) in [(2, false), (1, true)] {
        if change {
            fs::write(&path, format!("{source}# changed\n")).unwrap();
        }
        let value = diagnostic_parity(
            root,
            &[
                "schema",
                "validate",
                "--limit",
                &limit.to_string(),
                "--cursor",
                cursor,
            ],
            "schema_validate",
            json!({"limit":limit,"cursor":cursor}),
        );
        assert_eq!(value["error"]["code"], "stale_cursor");
    }
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn schema_operation_failures_keep_their_error_envelope() {
    let fixture = fixture();
    let root = fixture.path();
    assert!(mara(root, &["project", "init"]).status.success());
    for (flag, value, options) in [
        ("--limit", "0", json!({"limit":0})),
        ("--limit", "101", json!({"limit":101})),
        ("--cursor", "", json!({"cursor":""})),
    ] {
        let result = diagnostic_parity(
            root,
            &["schema", "validate", flag, value],
            "schema_validate",
            options,
        );
        assert_eq!(result["error"]["code"], "invalid_argument");
    }
    let path = root.join(".mara/schema.yaml");
    fs::write(&path, [0xff]).unwrap();
    let result = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(result["error"]["code"], "io_error");
    assert_eq!(fs::read(&path).unwrap(), [0xff]);
    let huge = "x".repeat(70_000);
    fs::write(
        &path,
        include_str!("../templates/minimal-schema.yaml")
            .replace("id_prefix: REQ-", &format!("id_prefix: {huge}")),
    )
    .unwrap();
    let result = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
    assert_eq!(result["error"]["code"], "output_limit");
    assert!(serde_json::to_vec(&result).unwrap().len() <= 65_536);
}

// @mara checks REQ-SCHEMA-DISCOVERY
#[test]
fn rule_definitions_reject_unsupported_syntax_and_recursive_dependencies() {
    let fixture = rule_fixture();
    let root = fixture.path();
    for source in [
        "id: rule:a\nid: rule:b\n",
        "id: rule:a\n'@context': https://invalid.example/context\n",
        "id: rule:a\nnode: rule:a\n",
        "id: rule:a\nnode: rule:missing\n",
        "id: rule:a\nproperty: [{path: owner, qualifiedMinCount: 1}]\n",
        "id: rule:a\ntargetClass: requirement\nproperty: [{path: owner, pattern: '['}]\n",
        "id: rule:a\ntargetClass: requirement\nproperty: [{path: owner, hasValue: {parameter: revision}}]\n",
        "id: rule:a\ntargetClass: requirement\nproperty: [{path: owner, hasValue: {value: '1', datatype: integer}}]\n",
    ] {
        fs::write(root.join("rules.yaml"), source).unwrap();
        let result = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
        assert_eq!(result["valid"], false, "{source}: {result}");
        assert!(
            result["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .all(|d| d["code"] == "rule_invalid" && d["severity"] == "error")
        );
        assert_eq!(fs::read_to_string(root.join("rules.yaml")).unwrap(), source);
    }
    for (depth, valid) in [(32, true), (33, false)] {
        let source = (0..depth)
            .map(|i| {
                if i + 1 < depth {
                    format!("- {{id: 'rule:n{i}', node: 'rule:n{}'}}\n", i + 1)
                } else {
                    format!("- {{id: 'rule:n{i}', class: requirement}}\n")
                }
            })
            .collect::<String>();
        fs::write(root.join("rules.yaml"), source).unwrap();
        let result = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
        assert_eq!(result["valid"], valid, "{depth}: {result}");
    }
}

// @mara checks DES-SCHEMA-RULE-DEFINITIONS
#[test]
fn rule_relationship_depth_and_definition_merges_preserve_their_limits() {
    let fixture = rule_fixture();
    let root = fixture.path();
    for (depth, valid) in [(8, true), (9, false)] {
        let mut shape = json!({"class":"requirement"});
        for _ in 0..depth {
            shape = json!({"path":"depends_on","node":shape});
        }
        let source = json!({"id":"rule:chain", "targetClass":"requirement", "property":[shape]});
        fs::write(
            root.join("rules.yaml"),
            serde_saphyr::to_string(&source).unwrap(),
        )
        .unwrap();
        let result = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
        assert_eq!(result["valid"], valid, "{depth}: {result}");
    }
    for (extra, valid) in [
        ("description: Same", true),
        ("description: Different", false),
    ] {
        let source = format!(
            "- {{id: 'rule:shared', description: Same}}\n- {{id: 'rule:shared', {extra}}}\n"
        );
        fs::write(root.join("rules.yaml"), source).unwrap();
        let result = diagnostic_parity(root, &["schema", "validate"], "schema_validate", json!({}));
        assert_eq!(result["valid"], valid, "{result}");
    }
}
