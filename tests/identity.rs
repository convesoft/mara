mod support;

use serde_json::{Value, json};
use std::{fs, path::Path};
use support::*;

// @mara checks REQ-DURABLE-ITEM-IDENTITY
#[test]
fn project_validation_treats_mid_as_structural_metadata() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("valid.mara.md"),
        r#":::mara requirement REQ-VALID
:mid: 01JQZ4W7G5H8K2M3N6P9R0STVX
:title: Valid

A complete requirement.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(validate.status.success(), "{}", stderr(&validate));
    assert!(stdout(&validate).contains("valid project"));
}

// @mara checks REQ-DURABLE-ITEM-IDENTITY
#[test]
fn project_validation_reports_missing_malformed_duplicate_and_misplaced_mids() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement REQ-MISSING
:title: Missing MID

Body.
:::

:::mara requirement REQ-MALFORMED
:mid: not-a-mid
:title: Malformed MID

Body.
:::

:::mara requirement REQ-OVERFLOW
:mid: ZZZZZZZZZZZZZZZZZZZZZZZZZZ
:title: Overflow MID

Body.
:::

:::mara requirement REQ-DUPLICATE-ONE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Duplicate one

Body.
:::

:::mara requirement REQ-DUPLICATE-TWO
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Duplicate two

Body.
:::

:::mara requirement REQ-MISPLACED
:title: Misplaced MID
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01

Body.
:::
"#,
    )
    .unwrap();

    let diagnostics = invalid_identities(fixture.path());
    let lines: Vec<_> = diagnostics
        .iter()
        .map(|d| d["line"].as_u64().unwrap())
        .collect();
    assert_eq!(lines, [1, 8, 15, 22, 29, 37]);
    assert!(diagnostics.iter().all(|d| d["path"] == "invalid.mara.md"));
}

// @mara checks REQ-DURABLE-ITEM-IDENTITY
#[test]
fn project_validation_rejects_non_bijective_item_identities() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement REQ-SHARED-ID
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Shared ID one

Body.
:::

:::mara requirement REQ-SHARED-ID
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Shared ID two

Body.
:::

:::mara requirement REQ-SHARED-MID-ONE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: Shared MID one

Body.
:::

:::mara requirement REQ-SHARED-MID-TWO
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: Shared MID two

Body.
:::
"#,
    )
    .unwrap();

    let diagnostics = invalid_identities(fixture.path());
    let lines: Vec<_> = diagnostics
        .iter()
        .map(|d| d["line"].as_u64().unwrap())
        .collect();
    assert_eq!(lines, [1, 8, 16, 23]);
    assert!(diagnostics.iter().all(|d| d["path"] == "invalid.mara.md"));
    for reference in ["REQ-SHARED-ID", "01ARZ3NDEKTSV4RRFFQ69G5F02"] {
        let get = mara(fixture.path(), &["--format", "json", "get", reference]);
        assert!(!get.status.success(), "{}", stdout(&get));
    }
}

// @mara checks REQ-DURABLE-ITEM-IDENTITY
#[test]
fn project_validation_reports_the_duplicated_secondary_mid_entry() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement REQ-FIRST
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: First

Body.
:::

:::mara requirement REQ-SECOND
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Second

Body.
:::
"#,
    )
    .unwrap();

    let diagnostics = invalid_identities(fixture.path());
    let mut lines: Vec<_> = diagnostics
        .iter()
        .map(|d| d["line"].as_u64().unwrap())
        .collect();
    lines.sort_unstable();
    assert_eq!(lines, [3, 3, 10]);
}

// @mara checks REQ-MID-BACKFILL
#[test]
fn project_mid_backfill_is_deliberate_preflighted_and_idempotent() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let path = fixture.path().join("legacy.mara.md");
    fs::write(
        &path,
        r#":::mara scenario SCN-LEGACY
:title: Legacy scenario

Legacy.
:::

:::mara requirement REQ-LEGACY
:title: Legacy requirement
:derives_from: SCN-LEGACY

Legacy body.
:::
"#,
    )
    .unwrap();

    let established_path = fixture.path().join("established.mara.md");
    let established = ":::mara requirement REQ-EXISTING\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Existing\n\nKeep these bytes.\n:::\n";
    fs::write(&established_path, established).unwrap();
    let original = fs::read_to_string(&path).unwrap();
    let read = mara(fixture.path(), &["--format", "json", "get", "REQ-LEGACY"]);
    assert!(read.status.success(), "{}", stderr(&read));
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    let validate = mara(fixture.path(), &["project", "validate"]);
    assert!(!validate.status.success());
    assert_eq!(invalid_identities(fixture.path()).len(), 2);

    let backfill = mara(
        fixture.path(),
        &["--format", "json", "project", "mid", "backfill"],
    );
    assert!(backfill.status.success(), "{}", stderr(&backfill));
    let backfill: Value = serde_json::from_str(&stdout(&backfill)).unwrap();
    let changed = backfill["changed"].as_array().unwrap();
    assert_eq!(changed.len(), 2);
    assert_eq!(changed[0]["id"], "SCN-LEGACY");
    assert_eq!(changed[0]["line"], 2);
    assert_eq!(changed[1]["id"], "REQ-LEGACY");
    assert_eq!(changed[1]["line"], 9);
    assert!(
        changed
            .iter()
            .all(|entry| is_mid(entry["mid"].as_str().unwrap()))
    );

    assert_eq!(fs::read_to_string(&established_path).unwrap(), established);
    let source = fs::read_to_string(&path).unwrap();
    let without_mids: String = source
        .split_inclusive('\n')
        .filter(|line| !line.starts_with(":mid: "))
        .collect();
    assert_eq!(without_mids, original);
    assert!(source.contains(":::mara scenario SCN-LEGACY\n:mid: "));
    assert!(source.contains(":::mara requirement REQ-LEGACY\n:mid: "));
    assert!(source.contains(":derives_from: SCN-LEGACY"));
    let validate = mara(fixture.path(), &["project", "validate"]);
    assert!(validate.status.success(), "{}", stderr(&validate));

    let again = mara(
        fixture.path(),
        &["--format", "json", "project", "mid", "backfill"],
    );
    assert!(again.status.success(), "{}", stderr(&again));
    let again: Value = serde_json::from_str(&stdout(&again)).unwrap();
    assert!(again["changed"].as_array().unwrap().is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), source);

    fs::write(
        &path,
        r#":::mara requirement REQ-BROKEN
:mid: invalid
:title: Broken

Body.
:::
"#,
    )
    .unwrap();
    let original = fs::read_to_string(&path).unwrap();
    let rejected = mara(fixture.path(), &["project", "mid", "backfill"]);
    assert!(!rejected.status.success());

    assert_eq!(fs::read_to_string(&path).unwrap(), original);
}

// @mara checks REQ-MID-BACKFILL
#[test]
fn project_mid_backfill_preflight_does_not_match_user_text_as_missing_mid() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace(
            "    id_prefix: REQ-\n    body: required\n    fields: {}",
            "    id_prefix: REQ-\n    body: required\n    fields:\n      blocked:\n        type: boolean",
        ),
    )
    .unwrap();
    let path = fixture.path().join("legacy.mara.md");
    fs::write(
        &path,
        r#":::mara requirement REQ-LEGACY
:title: Legacy requirement
:blocked: bad is missing its MID

Legacy body.
:::
"#,
    )
    .unwrap();
    let original = fs::read_to_string(&path).unwrap();

    let validation = mara(fixture.path(), &["--format", "json", "project", "validate"]);
    let validation: Value = serde_json::from_slice(&validation.stdout).unwrap();
    assert!(
        validation["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "field_invalid" && d["severity"] == "error")
    );
    let rejected = mara(fixture.path(), &["project", "mid", "backfill"]);

    assert!(!rejected.status.success());

    assert_eq!(fs::read_to_string(&path).unwrap(), original);
}

// @mara checks REQ-MID-BACKFILL
#[test]
fn mcp_project_mid_backfill_backfills_a_selected_project() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("legacy.mara.md"),
        ":::mara requirement REQ-LEGACY\n:title: Legacy\n\nBody.\n:::\n",
    )
    .unwrap();

    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "project_mid_backfill", json!({})),
            mcp_call(3, "project_validate", json!({})),
        ],
    );

    let backfill = &mcp_response(&responses, 2)["result"]["structuredContent"];
    assert_eq!(backfill["changed"].as_array().unwrap().len(), 1);
    assert!(is_mid(backfill["changed"][0]["mid"].as_str().unwrap()));
    assert_eq!(
        mcp_response(&responses, 3)["result"]["structuredContent"]["valid"],
        true
    );
}

// @mara checks REQ-DURABLE-ITEM-IDENTITY
// @mara checks DES-DURABLE-ITEM-IDENTITIES
#[test]
fn item_taking_operations_resolve_mids_but_author_relations_as_human_ids() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("items.mara.md"),
        r#":::mara scenario SCN-TARGET
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Target

Target.
:::

:::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Source

Source.
:::
"#,
    )
    .unwrap();

    let get = mara(
        fixture.path(),
        &["--format", "json", "get", "01ARZ3NDEKTSV4RRFFQ69G5F01"],
    );
    assert!(get.status.success(), "{}", stderr(&get));
    let item: Value = serde_json::from_str(&stdout(&get)).unwrap();
    assert_eq!(item["node"]["id"], "REQ-SOURCE");
    assert_eq!(item["node"]["mid"], "01ARZ3NDEKTSV4RRFFQ69G5F01");

    let list = mara(fixture.path(), &["--format", "json", "item", "list"]);
    assert!(list.status.success(), "{}", stderr(&list));
    let list: Value = serde_json::from_str(&stdout(&list)).unwrap();
    assert_eq!(list["items"][0]["id"], "SCN-TARGET");
    assert_eq!(list["items"][0]["mid"], "01ARZ3NDEKTSV4RRFFQ69G5F00");

    let search = mara(fixture.path(), &["--format", "json", "search", "source"]);
    assert!(search.status.success(), "{}", stderr(&search));
    let search: Value = serde_json::from_str(&stdout(&search)).unwrap();
    assert_eq!(search["results"][0]["node"]["id"], "REQ-SOURCE");
    assert_eq!(
        search["results"][0]["node"]["mid"],
        "01ARZ3NDEKTSV4RRFFQ69G5F01"
    );

    let add = mara(
        fixture.path(),
        &[
            "relation",
            "add",
            "01ARZ3NDEKTSV4RRFFQ69G5F01",
            "derives_from",
            "01ARZ3NDEKTSV4RRFFQ69G5F00",
        ],
    );
    assert!(add.status.success(), "{}", stderr(&add));
    let source = fs::read_to_string(fixture.path().join("items.mara.md")).unwrap();
    assert!(source.contains(":derives_from: SCN-TARGET"));
    assert!(!source.contains(":derives_from: 01ARZ3NDEKTSV4RRFFQ69G5F00"));
}

// @mara checks REQ-DURABLE-ITEM-IDENTITY
// @mara checks DES-DURABLE-ITEM-IDENTITIES
#[test]
fn relation_traversal_resolves_authored_mids_as_item_identity() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("items.mara.md"),
        r#":::mara scenario SCN-TARGET
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Target

Target.
:::

:::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Source
:derives_from: 01ARZ3NDEKTSV4RRFFQ69G5F00

Source.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);
    assert!(validate.status.success(), "{}", stderr(&validate));

    let get = mara(
        fixture.path(),
        &["--format", "json", "get", "01ARZ3NDEKTSV4RRFFQ69G5F00"],
    );
    assert!(get.status.success(), "{}", stderr(&get));
    let item: Value = serde_json::from_str(&stdout(&get)).unwrap();
    assert!(item.get("incoming_relations").is_none());

    let related = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "related",
            "01ARZ3NDEKTSV4RRFFQ69G5F00",
            "--relation",
            "derives_from",
        ],
    );
    assert!(related.status.success(), "{}", stderr(&related));
    let related: Value = serde_json::from_str(&stdout(&related)).unwrap();
    assert_eq!(related["connections"][0]["neighbour"]["id"], "REQ-SOURCE");
    assert_eq!(related["connections"][0]["direction"], "incoming");
}

fn invalid_identities(root: &Path) -> Vec<Value> {
    let output = mara(
        root,
        &["--format", "json", "project", "validate", "--limit", "100"],
    );
    assert!(!output.status.success());
    let cli: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(cli["valid"], false);
    assert_eq!(cli["evaluation_complete"], true);
    assert_eq!(cli["has_more"], false);
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
            mcp_call(2, "project_validate", json!({"project":root,"limit":100})),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        cli
    );
    let diagnostics = cli["diagnostics"].as_array().unwrap().clone();
    assert!(
        diagnostics
            .iter()
            .all(|d| d["code"] == "identity_invalid" && d["severity"] == "error"),
        "{diagnostics:?}"
    );
    diagnostics
}
