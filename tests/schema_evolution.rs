mod support;

use serde_json::{Value, json};
use std::{fs, path::Path};
use support::*;

fn validation_with_parity(root: &Path) -> Value {
    let output = mara(root, &["--format", "json", "project", "validate"]);
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.success(), result["valid"].as_bool().unwrap());
    assert_eq!(result["evaluation_complete"], true);
    assert_eq!(result["has_more"], false);
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "project_validate", json!({})),
        ],
    );
    let mcp = &mcp_response(&responses, 2)["result"];
    assert_eq!(mcp["isError"], false);
    assert_eq!(mcp["structuredContent"], result);
    result
}

// @mara implements VER-SCHEMA-EVOLUTION
// @mara checks REQ-SCHEMA-EVOLUTION
// @mara checks DES-SCHEMA-MIGRATION-WORKFLOW
#[test]
fn schema_alias_edits_preserve_custom_knowledge_and_reject_unrewritten_assertions() {
    let fixture = fixture();
    let root = fixture.path();
    let init = mara(root, &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        root.join(".mara/schema.yaml"),
        include_str!("fixtures/engineering-legacy-schema.yaml"),
    )
    .unwrap();
    let schema_path = root.join(".mara/schema.yaml");
    let original_schema = fs::read_to_string(&schema_path).unwrap().replace(
        "    id_prefix: VER-\n    body: required\n    fields: {}",
        "    id_prefix: VER-\n    body: required\n    fields:\n      status: {type: enum, values: [draft, approved]}",
    );
    assert!(original_schema.contains("status: {type: enum"));
    fs::write(&schema_path, &original_schema).unwrap();
    fs::write(root.join("notes.mara.md"), "# Policy\n\nKeep this prose.\n").unwrap();
    for args in [
        vec![
            "item",
            "create",
            "requirement",
            "REQ-A",
            "req.mara.md",
            "--title",
            "REQ-A",
            "--body",
            "See [policy](notes.mara.md#policy).",
        ],
        vec![
            "item",
            "create",
            "verification",
            "VER-A",
            "verify.mara.md",
            "--title",
            "VER-A",
            "--body",
            "Checks [[REQ-A]].",
            "--field",
            "status=approved",
            "--relation",
            "verifies=REQ-A",
        ],
    ] {
        let output = mara(root, &args);
        assert!(output.status.success(), "{}", stderr(&output));
    }
    let initial = mara(root, &["--format", "json", "project", "validate"]);
    assert!(initial.status.success(), "{}", stderr(&initial));
    let original_requirement = fs::read_to_string(root.join("req.mara.md")).unwrap();
    let original_verification = fs::read(root.join("verify.mara.md")).unwrap();
    let original_notes = fs::read(root.join("notes.mara.md")).unwrap();
    let original_config = fs::read(root.join(".mara/project.toml")).unwrap();
    let mids = ["REQ-A", "VER-A"].map(|id| {
        let output = mara(root, &["--format", "json", "get", id]);
        assert!(output.status.success(), "{}", stderr(&output));
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["node"]["mid"]
            .as_str()
            .unwrap()
            .to_owned()
    });

    fs::write(
        &schema_path,
        original_schema.replacen("format_version: 3", "format_version: 2", 1),
    )
    .unwrap();
    let old = mara(root, &["--format", "json", "schema", "validate"]);
    assert!(!old.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&old.stdout).unwrap()["diagnostics"][0]["code"],
        "format_unsupported"
    );

    let aliased_schema =
        original_schema.replace("  verifies:\n", "  verifies:\n    inverse: verified_by\n");
    assert_ne!(aliased_schema, original_schema);
    fs::write(&schema_path, &aliased_schema).unwrap();
    let aliased_requirement = original_requirement
        .replace(":title: REQ-A\n", ":title: REQ-A\n:verified_by: VER-A\n")
        .replace(
            "See [policy](notes.mara.md#policy).",
            "See [policy](notes.mara.md#policy).\n\nAlso [[verified_by:VER-A]].",
        );
    assert_ne!(aliased_requirement, original_requirement);
    fs::write(root.join("req.mara.md"), &aliased_requirement).unwrap();
    assert_eq!(validation_with_parity(root)["valid"], true);

    // Changing only the declaration would strand both authored inverse forms.
    let renamed_schema = aliased_schema.replace("inverse: verified_by", "inverse: checked_by");
    fs::write(&schema_path, &renamed_schema).unwrap();
    let invalid = validation_with_parity(root);
    assert_eq!(invalid["valid"], false, "{invalid:#}");
    assert_eq!(
        fs::read_to_string(root.join("req.mara.md")).unwrap(),
        aliased_requirement
    );

    let renamed_requirement = aliased_requirement.replace("verified_by", "checked_by");
    fs::write(root.join("req.mara.md"), &renamed_requirement).unwrap();
    let schema = mara(root, &["--format", "json", "schema", "validate"]);
    assert!(schema.status.success(), "{}", stderr(&schema));
    assert_eq!(validation_with_parity(root)["valid"], true);
    let edge = mara(
        root,
        &[
            "--format", "json", "relation", "get", "VER-A", "verifies", "REQ-A",
        ],
    );
    assert!(edge.status.success(), "{}", stderr(&edge));
    let edge: Value = serde_json::from_slice(&edge.stdout).unwrap();
    assert_eq!(edge["occurrence_count"], 3);
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "relation_get",
                json!({"source":"VER-A","relation":"verifies","target":"REQ-A"}),
            ),
        ],
    );
    assert_eq!(
        edge,
        mcp_response(&responses, 2)["result"]["structuredContent"]
    );
    for (id, mid) in ["REQ-A", "VER-A"].into_iter().zip(mids) {
        let output = mara(root, &["--format", "json", "get", id]);
        assert!(output.status.success(), "{}", stderr(&output));
        let item: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(item["node"]["mid"], mid);
    }
    assert_eq!(
        fs::read_to_string(root.join("req.mara.md")).unwrap(),
        renamed_requirement
    );
    assert_eq!(
        fs::read(root.join("verify.mara.md")).unwrap(),
        original_verification
    );
    assert_eq!(
        fs::read(root.join("notes.mara.md")).unwrap(),
        original_notes
    );
    assert_eq!(
        fs::read(root.join(".mara/project.toml")).unwrap(),
        original_config
    );
    assert!(renamed_schema.contains("status: {type: enum, values: [draft, approved]}"));
}

// @mara implements VER-SCHEMA-EVOLUTION
// @mara checks REQ-SCHEMA-EVOLUTION
// @mara checks DES-SCHEMA-MIGRATION-WORKFLOW
#[test]
fn manual_alias_removal_keeps_canonical_direction_when_both_endpoints_are_eligible() {
    let fixture = fixture();
    let root = fixture.path();
    let init = mara(root, &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let path = root.join(".mara/schema.yaml");
    let schema = fs::read_to_string(&path).unwrap()
        + "\n  follows:\n    description: Dependency\n    source: [requirement]\n    target: [requirement]\n    inverse: followed_by\n";
    fs::write(path, schema).unwrap();
    for (id, file) in [("REQ-A", "a.mara.md"), ("REQ-B", "b.mara.md")] {
        let output = mara(
            root,
            &[
                "item",
                "create",
                "requirement",
                id,
                file,
                "--title",
                id,
                "--body",
                "Preserved prose.",
            ],
        );
        assert!(output.status.success(), "{}", stderr(&output));
    }
    let schema_path = root.join(".mara/schema.yaml");
    let original_schema = fs::read_to_string(&schema_path).unwrap();
    let a_path = root.join("a.mara.md");
    let b_path = root.join("b.mara.md");
    let original_a = fs::read_to_string(&a_path).unwrap();
    let original_b = fs::read_to_string(&b_path).unwrap();
    let inverse_b = original_b
        .replace(":title: REQ-B\n", ":title: REQ-B\n:followed_by: REQ-A\n")
        .replace(
            "Preserved prose.",
            "Preserved prose. [[followed_by:REQ-A]].",
        );
    fs::write(&b_path, &inverse_b).unwrap();
    assert_eq!(validation_with_parity(root)["valid"], true);
    let original_edge = mara(root, &["relation", "get", "REQ-A", "follows", "REQ-B"]);
    assert!(original_edge.status.success(), "{}", stderr(&original_edge));

    let no_alias_schema = original_schema.replace("    inverse: followed_by\n", "");
    assert_ne!(no_alias_schema, original_schema);
    fs::write(&schema_path, &no_alias_schema).unwrap();
    // This tempting rewrite validates, but asserts the opposite edge.
    fs::write(&b_path, inverse_b.replace("followed_by", "follows")).unwrap();
    assert_eq!(validation_with_parity(root)["valid"], true);
    assert!(
        mara(root, &["relation", "get", "REQ-B", "follows", "REQ-A"])
            .status
            .success()
    );
    assert!(
        !mara(root, &["relation", "get", "REQ-A", "follows", "REQ-B"])
            .status
            .success()
    );

    // Restore the valid input, then move the assertion to its canonical
    // source before dropping the inverse declaration.
    fs::write(&schema_path, &original_schema).unwrap();
    fs::write(&b_path, &inverse_b).unwrap();
    let canonical_a = original_a.replace(":title: REQ-A\n", ":title: REQ-A\n:follows: REQ-B\n");
    let canonical_b = inverse_b
        .replace(":followed_by: REQ-A\n", "")
        .replace("[[followed_by:REQ-A]]", "[[REQ-A]]");
    fs::write(&a_path, &canonical_a).unwrap();
    fs::write(&b_path, &canonical_b).unwrap();
    assert_eq!(validation_with_parity(root)["valid"], true);
    fs::write(&schema_path, &no_alias_schema).unwrap();
    assert_eq!(validation_with_parity(root)["valid"], true);
    for (source, target, exists) in [("REQ-A", "REQ-B", true), ("REQ-B", "REQ-A", false)] {
        let cli = mara(
            root,
            &[
                "--format", "json", "relation", "get", source, "follows", target,
            ],
        );
        assert_eq!(cli.status.success(), exists);
        let responses = mcp_exchange(
            root,
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(
                    2,
                    "relation_get",
                    json!({"source":source,"relation":"follows","target":target}),
                ),
            ],
        );
        let result = &mcp_response(&responses, 2)["result"];
        assert_eq!(result["isError"], !exists, "{result}");
        if exists {
            assert_eq!(
                result["structuredContent"],
                serde_json::from_slice::<Value>(&cli.stdout).unwrap()
            );
        }
    }
    assert_eq!(fs::read_to_string(&a_path).unwrap(), canonical_a);
    assert_eq!(fs::read_to_string(&b_path).unwrap(), canonical_b);
    assert!(canonical_b.contains("Preserved prose. [[REQ-A]]."));
}
