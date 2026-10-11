use mara_engine::{IgnoreInput, SelectionInputs, SourceFile, SourceSnapshot};
use mara_wasm::{FileChange, MemoryWorkspace, Request};
use serde_json::{Value, json};

fn call(w: &mut MemoryWorkspace, operation: &str, params: Value) -> Result<Value, Value> {
    w.execute(
        Request {
            operation: operation.into(),
            params,
        },
        || Ok("01ARZ3NDEKTSV4RRFFQ69G5FAV".into()),
    )
}
fn workspace() -> MemoryWorkspace {
    let mut w = MemoryWorkspace::new(SourceSnapshot {
        format_version: 1,
        files: vec![],
        base_identity: Some("opaque revision".into()),
        selection: SelectionInputs {
            document_paths: vec![],
            ignores: vec![],
        },
    })
    .unwrap();
    call(
        &mut w,
        "project_init",
        json!({"name":"Export", "template":"engineering-legacy"}),
    )
    .unwrap();
    call(
        &mut w,
        "item_create",
        json!({"flavour":"requirement","id":"REQ-A","file":"a.mara.md",
        "title":"Original", "body":"café\r\n中文 👩🏽‍💻", "fields":[{"key":"status","value":"draft"}]}),
    )
    .unwrap();
    let mut base = w.current_snapshot();
    base.files.push(SourceFile {
        path: "untouched.mara.md".into(),
        content: "# Untouched\n".into(),
    });
    base.selection
        .document_paths
        .push("untouched.mara.md".into());
    base.selection.ignores.push(IgnoreInput {
        directory: String::new(),
        root_prefix: vec![],
        content: "ignored/\n".into(),
    });
    call(&mut w, "load_snapshot", json!(base)).unwrap();
    w
}

// @mara checks REQ-BROWSER-CHANGE-EXPORT
// @mara checks DES-BROWSER-FILE-CHANGESET
#[test]
fn export_consolidates_real_edits_without_advancing_base_and_replacement_resets_it() {
    let mut w = workspace();
    let original = call(&mut w, "workspace_state", json!({})).unwrap();
    let page = call(&mut w, "item_list", json!({"limit":1})).unwrap();
    assert!(w.export_changes().changes.is_empty());
    for title in ["Intermediate", "Final", "Original"] {
        call(
            &mut w,
            "item_update",
            json!({"reference":"REQ-A","title":title}),
        )
        .unwrap();
    }
    assert!(w.export_changes().changes.is_empty());
    call(
        &mut w,
        "item_update",
        json!({"reference":"REQ-A","title":"Final"}),
    )
    .unwrap();
    let export = w.export_changes();
    assert_eq!(export.changes.len(), 1);
    assert_eq!(export.changes[0].path, "a.mara.md");
    assert!(
        export.changes[0]
            .before
            .as_ref()
            .unwrap()
            .contains(":title: Original\n")
    );
    assert!(
        export.changes[0]
            .after
            .as_ref()
            .unwrap()
            .contains(":title: Final\n")
    );
    assert_eq!(
        export.base_source_fingerprint,
        original["base_source_fingerprint"]
    );
    assert_eq!(export.base_identity.as_deref(), Some("opaque revision"));
    let sources = call(&mut w, "workspace_sources", json!({})).unwrap();
    assert_eq!(
        call(&mut w, "export_changes", json!({})).unwrap(),
        json!(export)
    );
    assert_eq!(w.export_changes(), export);
    assert_eq!(
        call(&mut w, "workspace_sources", json!({})).unwrap(),
        sources
    );
    assert!(!page["has_more"].as_bool().unwrap());
    assert!(
        call(
            &mut w,
            "item_update",
            json!({"reference":"REQ-A","body":"[[REQ-MISSING]]"})
        )
        .is_err()
    );
    let mut invalid = w.current_snapshot();
    invalid.files[0].path = "../escape".into();
    assert!(call(&mut w, "load_snapshot", json!(invalid)).is_err());
    assert_eq!(w.export_changes(), export);
    assert_eq!(
        call(&mut w, "workspace_sources", json!({})).unwrap(),
        sources
    );
    let replacement = w.current_snapshot();
    call(&mut w, "load_snapshot", json!(replacement)).unwrap();
    assert!(w.export_changes().changes.is_empty());
    assert_ne!(
        w.export_changes().base_source_fingerprint,
        export.base_source_fingerprint
    );
}

// @mara checks DES-BROWSER-FILE-CHANGESET
#[test]
fn whole_base_and_each_preimage_are_required_without_memory_writes() {
    let mut w = workspace();
    call(
        &mut w,
        "item_update",
        json!({"reference":"REQ-A","title":"Final"}),
    )
    .unwrap();
    let export = w.export_changes();
    let base = w.base_snapshot().clone();
    let before = call(&mut w, "workspace_sources", json!({})).unwrap();
    let mut equivalent = base.clone();
    equivalent.base_identity = Some("different opaque label is not different source".into());
    equivalent.files.reverse();
    equivalent.files[0].path = format!("./{}", equivalent.files[0].path);
    export.check_preconditions(equivalent).unwrap();
    let matched = call(
        &mut w,
        "check_change_set_preconditions",
        json!({"change_set":export,"snapshot":base}),
    )
    .unwrap();
    assert_eq!(matched["source_preconditions_match"], true);
    assert_eq!(matched["external_context_verified"], false);
    for path in [
        "untouched.mara.md",
        ".mara/project.toml",
        ".mara/schema.yaml",
        ".mara/engineering-rules.yaml",
        "a.mara.md",
    ] {
        let mut divergent = base.clone();
        divergent
            .files
            .iter_mut()
            .find(|f| f.path == path)
            .unwrap()
            .content
            .push('\n');
        assert!(
            export.check_preconditions(divergent.clone()).is_err(),
            "{path}"
        );
        assert!(
            call(
                &mut w,
                "check_change_set_preconditions",
                json!({"change_set":export,"snapshot":divergent})
            )
            .is_err()
        );
    }
    for kind in 0..4 {
        let mut divergent = base.clone();
        match kind {
            0 => {
                divergent.selection.document_paths.clear();
            }
            1 => {
                divergent.selection.ignores[0].content.push_str("other/\n");
            }
            2 => {
                divergent.files.push(SourceFile {
                    path: "extra.mara.md".into(),
                    content: "# Extra\n".into(),
                });
                divergent
                    .selection
                    .document_paths
                    .push("extra.mara.md".into());
            }
            _ => {
                divergent.files.retain(|f| f.path != "untouched.mara.md");
                divergent
                    .selection
                    .document_paths
                    .retain(|p| p != "untouched.mara.md");
            }
        }
        assert!(export.check_preconditions(divergent).is_err());
    }
    for kind in 0..8 {
        let mut malformed = export.clone();
        match kind {
            0 => malformed.format_version = 2,
            1 => malformed.changes[0].before = Some("wrong".into()),
            2 => malformed.changes[0].before = None,
            3 => malformed.changes.push(malformed.changes[0].clone()),
            4 => malformed.changes[0].path = "./a.mara.md".into(),
            5 => malformed.changes[0].path = "../escape".into(),
            6 => malformed.changes[0].after = malformed.changes[0].before.clone(),
            _ => malformed.changes.insert(
                0,
                FileChange {
                    path: "z/new".into(),
                    before: None,
                    after: Some("new".into()),
                },
            ),
        }
        assert!(
            malformed.check_preconditions(base.clone()).is_err(),
            "case {kind}"
        );
    }
    assert_eq!(
        call(&mut w, "workspace_sources", json!({})).unwrap(),
        before
    );
    assert_eq!(w.export_changes(), export);
}
