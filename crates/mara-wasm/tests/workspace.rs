use mara_engine::{SelectionInputs, SourceSnapshot};
use mara_wasm::{MemoryWorkspace, Request};
use serde_json::{Value, json};

fn empty() -> MemoryWorkspace {
    MemoryWorkspace::new(SourceSnapshot {
        format_version: 1,
        files: vec![],
        base_identity: Some("opaque base".into()),
        selection: SelectionInputs {
            document_paths: vec![],
            ignores: vec![],
        },
    })
    .unwrap()
}
fn call(w: &mut MemoryWorkspace, operation: &str, params: Value) -> Result<Value, Value> {
    let mut n = 0;
    w.execute(
        Request {
            operation: operation.into(),
            params,
        },
        || {
            n += 1;
            Ok(format!("01ARZ3NDEKTSV4RRFFQ69G5F{n:02}"))
        },
    )
}
fn init(w: &mut MemoryWorkspace, template: &str) {
    call(
        w,
        "project_init",
        json!({"name":"café 👩🏽‍💻", "template":template}),
    )
    .unwrap();
}
fn create(w: &mut MemoryWorkspace, id: &str) -> Value {
    // Distinct predictable host identities, not engine-generated entropy.
    let suffix = if id == "REQ-A" { "00" } else { "01" };
    w.execute(
        Request {
            operation: "item_create".into(),
            params: json!({
                "flavour":"requirement", "id":id, "file":"café/知识.mara.md", "title":id,
                "fields":[{"key":"status","value":"draft"}], "body":"Unicode café 👩🏽‍💻 中文."
            }),
        },
        || Ok(format!("01ARZ3NDEKTSV4RRFFQ69G5F{suffix}")),
    )
    .unwrap()
}
fn sources(w: &mut MemoryWorkspace) -> Value {
    call(w, "workspace_sources", json!({})).unwrap()
}

// @mara checks REQ-BROWSER-LOCAL-WORKSPACE
#[test]
fn dispatcher_applies_every_semantic_mutation_and_retains_base() {
    let mut w = empty();
    init(&mut w, "engineering");
    let a = create(&mut w, "REQ-A");
    create(&mut w, "REQ-B");
    assert!(a["complete"] == true);
    assert_eq!(sources(&mut w)["base"]["files"], json!([]));
    let edge = json!({"source":"REQ-A","relation":"refines","target":"REQ-B"});
    call(&mut w, "relation_add", edge.clone()).unwrap();
    let inspection = call(&mut w, "relation_get", edge.clone()).unwrap();
    let mut remove = edge.clone();
    remove["occurrence"] = inspection["occurrences"][0]["reference"].clone();
    call(&mut w, "relation_remove", remove).unwrap();
    call(
        &mut w,
        "item_update",
        json!({"reference":"REQ-A","title":"Changed café"}),
    )
    .unwrap();
    call(
        &mut w,
        "item_rename",
        json!({"reference":"REQ-A","new_id":"REQ-RENAMED"}),
    )
    .unwrap();
    call(
        &mut w,
        "item_move",
        json!({"reference":"REQ-RENAMED","file":"moved.mara.md"}),
    )
    .unwrap();
    call(&mut w, "item_delete", json!({"reference":"REQ-RENAMED"})).unwrap();
    assert!(call(&mut w, "project_validate", json!({})).unwrap()["valid"] == true);
    assert!(call(&mut w, "schema_validate", json!({})).unwrap()["valid"] == true);
    assert_eq!(sources(&mut w)["base"]["files"], json!([]));
    assert_eq!(sources(&mut w)["current"]["base_identity"], "opaque base");
    assert!(
        sources(&mut w)["current"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == "moved.mara.md")
    );
}

// @mara checks DES-BROWSER-PROJECT-SNAPSHOT
#[test]
fn replacement_normalizes_complete_inventory_and_refuses_missing_inputs_atomically() {
    let mut w = empty();
    init(&mut w, "engineering");
    create(&mut w, "REQ-A");
    let mut snapshot = w.current_snapshot();
    snapshot.files[0].path = format!("./{}", snapshot.files[0].path);
    snapshot.base_identity = Some("new provenance".into());
    call(&mut w, "load_snapshot", json!(snapshot)).unwrap();
    let before = sources(&mut w);
    assert_eq!(before["base"], before["current"]);
    for path in [
        ".mara/project.toml",
        ".mara/schema.yaml",
        ".mara/engineering-rules.yaml",
        "café/知识.mara.md",
    ] {
        let mut incomplete = w.current_snapshot();
        incomplete.files.retain(|f| f.path != path);
        assert!(
            call(&mut w, "load_snapshot", json!(incomplete)).is_err(),
            "{path}"
        );
        assert_eq!(sources(&mut w), before);
    }
    for path in ["../escape", "/absolute", "C:/drive", "bad\\path"] {
        let mut invalid = w.current_snapshot();
        invalid.files[0].path = path.into();
        assert!(call(&mut w, "load_snapshot", json!(invalid)).is_err());
        assert_eq!(sources(&mut w), before);
    }
    let mut excluded = w.current_snapshot();
    excluded.selection.ignores.push(mara_engine::IgnoreInput {
        directory: String::new(),
        root_prefix: vec![],
        content: "café/\n".into(),
    });
    assert!(call(&mut w, "load_snapshot", json!(excluded)).is_err());
    assert_eq!(sources(&mut w), before);

    let mut duplicate = w.current_snapshot();
    duplicate.files.push(duplicate.files[0].clone());
    assert!(call(&mut w, "load_snapshot", json!(duplicate)).is_err());
    assert_eq!(sources(&mut w), before);
}

// @mara checks DES-BROWSER-WORKER-BINDING
#[test]
fn custom_schema_invalid_configuration_keeps_recovering_validation_available() {
    let mut w = empty();
    init(&mut w, "minimal");
    let mut snapshot = w.current_snapshot();
    snapshot
        .files
        .iter_mut()
        .find(|f| f.path == ".mara/schema.yaml")
        .unwrap()
        .path = "custom.yaml".into();
    let config = snapshot
        .files
        .iter_mut()
        .find(|f| f.path == ".mara/project.toml")
        .unwrap();
    config.content = config.content.replace(".mara/schema.yaml", "custom.yaml");
    call(&mut w, "load_snapshot", json!(snapshot.clone())).unwrap();
    // Invalid TOML cannot decode a configured path. It must not demand the
    // parser's fallback .mara/schema.yaml, which this project has never used.
    snapshot
        .files
        .iter_mut()
        .find(|f| f.path == ".mara/project.toml")
        .unwrap()
        .content
        .push_str("\ninvalid = [");
    call(&mut w, "load_snapshot", json!(snapshot)).unwrap();
    let validation = call(&mut w, "project_validate", json!({})).unwrap();
    assert!(validation["valid"] == false);
    assert!(validation["evaluation_complete"] == false);
    assert!(
        validation["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "project_invalid")
    );
    let before = sources(&mut w);
    assert!(call(&mut w, "project_mid_backfill", json!({})).is_err());
    assert_eq!(sources(&mut w), before);
}

// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
#[test]
fn unavailable_code_refuses_all_mutations_but_not_reads_or_schema_checks() {
    let mut w = empty();
    init(&mut w, "engineering");
    create(&mut w, "REQ-A");
    let mut snapshot = w.current_snapshot();
    let config = snapshot
        .files
        .iter_mut()
        .find(|f| f.path == ".mara/project.toml")
        .unwrap();
    config.content = config
        .content
        .replace("format_version = 2", "format_version = 3");
    config.content.push_str("\n[[code.languages]]\nname = 'rust'\nextensions = ['rs']\ncommand = ['never-run', '{output}']\n");
    call(&mut w, "load_snapshot", json!(snapshot)).unwrap();
    let before = sources(&mut w);
    for (operation, params) in [
        ("project_init", json!({"name":"Refused"})),
        ("project_mid_backfill", json!({})),
        (
            "item_create",
            json!({"id":"REQ-NEW","flavour":"requirement","file":"new.mara.md","title":"New","fields":[{"key":"status","value":"draft"}]}),
        ),
        (
            "item_update",
            json!({"reference":"REQ-A","title":"Unchanged"}),
        ),
        ("item_delete", json!({"reference":"REQ-A"})),
        (
            "item_move",
            json!({"reference":"REQ-A","file":"new.mara.md"}),
        ),
        ("item_rename", json!({"reference":"REQ-A","new_id":"REQ-A"})),
        (
            "relation_add",
            json!({"source":"REQ-A","relation":"refines","target":"REQ-A"}),
        ),
        (
            "relation_remove",
            json!({"source":"REQ-A","relation":"refines","target":"REQ-A"}),
        ),
    ] {
        assert!(call(&mut w, operation, params).is_err(), "{operation}");
        assert_eq!(sources(&mut w), before);
    }
    for (operation, params) in [
        ("schema_get", json!({})),
        ("schema_list", json!({"kind":"flavour"})),
        ("item_list", json!({})),
        ("search", json!({"query":"Unicode"})),
        ("get", json!({"reference":"REQ-A"})),
        ("schema_validate", json!({})),
    ] {
        call(&mut w, operation, params).unwrap();
    }
    let validation = call(&mut w, "project_validate", json!({})).unwrap();
    assert!(validation["evaluation_complete"] == false);
    assert!(
        validation["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "evaluation_unavailable")
    );
    assert_eq!(
        call(&mut w, "capabilities", json!({})).unwrap()["mutation_persistence"],
        "memory"
    );
    assert!(call(&mut w, "project_transaction_rollback", json!({})).is_err());
}

// @mara checks DES-BROWSER-PROJECT-SNAPSHOT
#[test]
fn selection_ignores_and_rejected_edits_preserve_state_and_continuation() {
    let mut w = empty();
    init(&mut w, "engineering");
    create(&mut w, "REQ-A");
    create(&mut w, "REQ-B");
    let mut snapshot = w.current_snapshot();
    snapshot.selection.ignores.push(mara_engine::IgnoreInput {
        directory: String::new(),
        root_prefix: vec![],
        content: "ignored/\n".into(),
    });
    call(&mut w, "load_snapshot", json!(snapshot)).unwrap();
    let page = call(&mut w, "item_list", json!({"limit":1})).unwrap();
    let before = sources(&mut w);
    for params in [
        json!({"reference":"REQ-A","file":"ignored/new.mara.md"}),
        json!({"reference":"REQ-A","file":"wrong.txt"}),
    ] {
        assert!(call(&mut w, "item_move", params).is_err());
        assert_eq!(sources(&mut w), before);
    }
    assert!(
        w.execute(
            Request {
                operation: "item_create".into(),
                params: json!({
                    "flavour":"requirement", "id":"REQ-NEW", "file":"ignored/new.mara.md",
                    "title":"Refuse", "body":"Body.", "fields":[{"key":"status","value":"draft"}]
                })
            },
            || Ok("01ARZ3NDEKTSV4RRFFQ69G5F99".into())
        )
        .is_err()
    );
    assert_eq!(sources(&mut w), before);
    assert!(
        call(
            &mut w,
            "item_update",
            json!({"reference":"REQ-A","body":"[[REQ-MISSING]]"})
        )
        .is_err()
    );
    assert_eq!(sources(&mut w), before);
    call(
        &mut w,
        "item_list",
        json!({"limit":1,"cursor":page["next_cursor"]}),
    )
    .unwrap();
    call(
        &mut w,
        "item_update",
        json!({"reference":"REQ-A","title":"New"}),
    )
    .unwrap();
    assert!(
        call(
            &mut w,
            "item_list",
            json!({"limit":1,"cursor":page["next_cursor"]})
        )
        .is_err()
    );
}

// @mara checks DES-MID-BACKFILL
#[test]
fn backfill_hosts_entropy_and_no_op_keeps_fingerprint() {
    let mut w = empty();
    init(&mut w, "minimal");
    let mut snapshot = w.current_snapshot();
    snapshot
        .selection
        .document_paths
        .push("legacy.mara.md".into());
    snapshot.files.push(mara_engine::SourceFile {
        path: "legacy.mara.md".into(),
        content: ":::mara requirement REQ-OLD\r\n:title: café\r\n\r\nBody.\r\n:::\r\n".into(),
    });
    call(&mut w, "load_snapshot", json!(snapshot)).unwrap();
    let result = call(&mut w, "project_mid_backfill", json!({})).unwrap();
    assert_eq!(result["changed"].as_array().unwrap().len(), 1);
    let before = sources(&mut w);
    assert_eq!(
        call(&mut w, "project_mid_backfill", json!({})).unwrap()["changed"],
        json!([])
    );
    assert_eq!(sources(&mut w), before);
}
