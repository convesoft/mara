use mara_engine::*;
use serde_json::json;
use std::path::Path;

fn decode<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).unwrap()
}
fn empty() -> SourceEngine {
    SourceEngine::new(SourceSnapshot {
        format_version: 1,
        files: vec![],
        selection: SelectionInputs {
            document_paths: vec![],
            ignores: vec![],
        },
        base_identity: Some("caller-base".into()),
    })
    .unwrap()
}
fn initialized() -> SourceEngine {
    let mut engine = empty();
    let plan = engine
        .plan_initialize("Workspace", Template::Minimal)
        .unwrap();
    engine.apply_plan(plan).unwrap();
    engine
}
fn request(id: &str, file: &str, body: &str) -> ItemCreationRequest {
    ItemCreationRequest {
        flavour: "requirement".into(),
        id: id.into(),
        file: file.into(),
        title: id.into(),
        fields: vec![],
        relations: vec![],
        body: Some(body.into()),
        line: None,
    }
}
fn create(engine: &mut SourceEngine, id: &str, file: &str, body: &str, mid: &str) {
    let plan = engine
        .planner()
        .unwrap()
        .plan_create(request(id, file, body), mid)
        .unwrap();
    engine.apply_plan(plan).unwrap();
}
const MID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5F00";
const MID_B: &str = "01ARZ3NDEKTSV4RRFFQ69G5F01";
fn with_file(engine: &SourceEngine, path: &str, content: String, selected: bool) -> SourceEngine {
    let mut snapshot = engine.snapshot();
    if let Some(file) = snapshot.files.iter_mut().find(|f| f.path == path) {
        file.content = content;
    } else {
        snapshot.files.push(SourceFile {
            path: path.into(),
            content,
        });
    }
    if selected && !snapshot.selection.document_paths.iter().any(|p| p == path) {
        snapshot.selection.document_paths.push(path.into());
    }
    SourceEngine::new(snapshot).unwrap()
}

// @mara checks DES-SEMANTIC-CHANGE-PLANS
// @mara checks REQ-PORTABLE-DOCUMENT-ENGINE
#[test]
fn real_document_only_planner_executes_all_operations_without_publication() {
    let mut engine = initialized();
    create(&mut engine, "REQ-A", "a.mara.md", "Self [[REQ-A]].", MID_A);
    create(&mut engine, "REQ-B", "b.mara.md", "Other.", MID_B);
    let original_b = engine.files()["b.mara.md"].clone();
    let relation = decode(json!({"source":"REQ-A","relation":"depends_on","target":"REQ-B"}));
    let plan = engine
        .planner()
        .unwrap()
        .plan_relation(&relation, true, None)
        .unwrap();
    assert_eq!(plan.result().changed_occurrences, 1);
    assert_eq!(
        plan.changes()[0].before.as_deref(),
        Some(engine.files()["a.mara.md"].as_str())
    );
    engine.apply_plan(plan).unwrap();
    let plan = engine
        .planner()
        .unwrap()
        .plan_update(decode(
            json!({"reference":"REQ-A","title":"A changed title"}),
        ))
        .unwrap();
    assert_eq!(plan.result().changed_fields, vec!["title"]);
    engine.apply_plan(plan).unwrap();
    let inspection = engine.relation_get(relation.clone(), None, None).unwrap();
    let plan = engine
        .planner()
        .unwrap()
        .plan_relation(&relation, false, Some(&inspection.occurrences[0].reference))
        .unwrap();
    assert!(!plan.result().edge_exists);
    engine.apply_plan(plan).unwrap();
    let plan = engine
        .planner()
        .unwrap()
        .plan_rename("REQ-A", "REQ-RENAMED")
        .unwrap();
    assert_eq!(plan.result().mid, MID_A);
    engine.apply_plan(plan).unwrap();
    let plan = engine
        .planner()
        .unwrap()
        .plan_move("REQ-RENAMED", Path::new("new.mara.md"), None)
        .unwrap();
    assert_eq!(
        plan.changes()
            .iter()
            .find(|c| c.path == "new.mara.md")
            .unwrap()
            .before,
        None
    );
    engine.apply_plan(plan).unwrap();
    assert!(engine.files()["a.mara.md"].trim().is_empty());
    let plan = engine
        .planner()
        .unwrap()
        .plan_delete("REQ-RENAMED")
        .unwrap();
    engine.apply_plan(plan).unwrap();
    assert!(engine.files().contains_key("new.mara.md"));
    assert_eq!(engine.files()["b.mara.md"], original_b);
    assert_eq!(engine.base_identity(), Some("caller-base"));
    let validation = engine
        .project_validate(&[], &ValidationOptions::default(), None)
        .unwrap();
    assert!(validation.valid);
}

// @mara checks DES-SEMANTIC-CHANGE-PLANS
#[test]
fn templates_and_initialization_preserve_sources_and_refuse_overwrite() {
    for template in [
        Template::Minimal,
        Template::Empty,
        Template::Engineering,
        Template::EngineeringLegacy,
    ] {
        let mut engine = empty();
        let plan = engine
            .plan_initialize("Name \"with quotes\"", template)
            .unwrap();
        assert!(engine.files().is_empty());
        assert!(
            plan.changes()
                .iter()
                .all(|c| c.before.is_none() && c.after.is_some())
        );
        assert_eq!(
            plan.changes().len(),
            match template {
                Template::Engineering => 9,
                Template::EngineeringLegacy => 5,
                Template::Minimal | Template::Empty => 2,
            }
        );
        engine.apply_plan(plan).unwrap();
        assert_eq!(engine.corpus().unwrap().0.name(), "Name \"with quotes\"");
        assert!(engine.plan_initialize("Again", template).is_err());
    }
    let engine = with_file(&empty(), SCHEMA_FILE, "manual source".into(), false);
    assert!(engine.plan_initialize("Refuse", Template::Minimal).is_err());
    assert_eq!(engine.files()[SCHEMA_FILE], "manual source");
    // Initialization retains its existing prerequisite: pre-existing documents need not conform.
    let mut engine = with_file(
        &empty(),
        "existing.mara.md",
        ":::mara unknown REQ-X\n".into(),
        true,
    );
    let plan = engine
        .plan_initialize("Existing", Template::Minimal)
        .unwrap();
    engine.apply_plan(plan).unwrap();
    assert_eq!(
        engine.files()["existing.mara.md"],
        ":::mara unknown REQ-X\n"
    );
}

// @mara checks DES-MID-BACKFILL
// @mara checks DES-SEMANTIC-CHANGE-PLANS
#[test]
fn supplied_mids_backfill_exact_crlf_sources_and_reject_missing_or_duplicate_entropy() {
    let source = "# Intro\r\n\r\n:::mara requirement REQ-A\r\n:title: A\r\n\r\nBody.\r\n:::\r\n\r\n:::mara requirement REQ-B\r\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\r\n:title: B\r\n\r\nBody.\r\n:::\r\n";
    let mut engine = with_file(&initialized(), "a.mara.md", source.into(), true);
    assert!(engine.planner().unwrap().plan_backfill(&[]).is_err());
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_backfill(&[MID_B.into()])
            .is_err()
    );
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_backfill(&["not-a-MID".into()])
            .is_err()
    );
    let plan = engine
        .planner()
        .unwrap()
        .plan_backfill(&[MID_A.into()])
        .unwrap();
    assert_eq!(plan.result().entries()[0].line(), 4);
    assert_eq!(engine.files()["a.mara.md"], source);
    engine.apply_plan(plan).unwrap();
    assert_eq!(
        engine.files()["a.mara.md"],
        source.replacen(
            ":::mara requirement REQ-A\r\n",
            &format!(":::mara requirement REQ-A\r\n:mid: {MID_A}\r\n"),
            1
        )
    );
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_backfill(&[])
            .unwrap()
            .changes()
            .is_empty()
    );
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_create(request("REQ-C", "c.mara.md", "Body"), MID_A)
            .is_err()
    );
}

// @mara checks DES-SEMANTIC-CHANGE-PLANS
#[test]
fn every_original_input_binds_plans_and_memory_acceptance_is_atomic() {
    let mut engine = initialized();
    create(&mut engine, "REQ-A", "a.mara.md", "Body", MID_A);
    create(&mut engine, "REQ-B", "b.mara.md", "Body", MID_B);
    engine = with_file(&engine, "rules.yaml", "original rule source".into(), false);
    let plan = engine
        .planner()
        .unwrap()
        .plan_update(decode(json!({"reference":"REQ-A","title":"Changed"})))
        .unwrap();
    assert_eq!(plan.changes().len(), 1);
    assert!(plan.original().files.iter().any(|f| f.path == "rules.yaml"));
    for path in [
        PROJECT_FILE,
        SCHEMA_FILE,
        "a.mara.md",
        "b.mara.md",
        "rules.yaml",
    ] {
        let mut changed = with_file(&engine, path, format!("{}\n", engine.files()[path]), false);
        let fingerprint = changed.source_fingerprint().to_owned();
        assert!(changed.apply_plan(plan.clone()).is_err(), "{path}");
        assert_eq!(changed.source_fingerprint(), fingerprint);
    }
    let mut snapshot = engine.snapshot();
    snapshot.selection.ignores.push(IgnoreInput {
        directory: "".into(),
        root_prefix: vec![],
        content: "unrelated/\n".into(),
    });
    assert!(
        SourceEngine::new(snapshot)
            .unwrap()
            .apply_plan(plan.clone())
            .is_err()
    );
    let mut snapshot = engine.snapshot();
    snapshot.base_identity = Some("another-base".into());
    assert!(
        SourceEngine::new(snapshot)
            .unwrap()
            .apply_plan(plan.clone())
            .is_err()
    );
    let mut snapshot = engine.snapshot();
    snapshot.files.push(SourceFile {
        path: "extra.txt".into(),
        content: "new inventory".into(),
    });
    assert!(
        SourceEngine::new(snapshot)
            .unwrap()
            .apply_plan(plan.clone())
            .is_err()
    );
    engine.apply_plan(plan.clone()).unwrap();
    assert!(engine.apply_plan(plan).is_err());
}

// @mara checks DES-SEMANTIC-CHANGE-PLANS
#[test]
fn selection_ignores_and_stale_occurrences_cursors_and_handles_remain_enforced() {
    let mut engine = with_file(&initialized(), "a.mara.md", "Narrative.\n\n".into(), true);
    create(&mut engine, "REQ-A", "a.mara.md", "Body", MID_A);
    create(&mut engine, "REQ-B", "b.mara.md", "Body", MID_B);
    let relation = decode(json!({"source":"REQ-A","relation":"depends_on","target":"REQ-B"}));
    let plan = engine
        .planner()
        .unwrap()
        .plan_relation(&relation, true, None)
        .unwrap();
    engine.apply_plan(plan).unwrap();
    let occurrence = engine
        .relation_get(relation.clone(), None, None)
        .unwrap()
        .occurrences[0]
        .reference
        .clone();
    let cursor = engine
        .item_list(decode(json!({"limit":1})))
        .unwrap()
        .next_cursor
        .unwrap();
    let handle = engine
        .search(decode(json!({"query":"Narrative"})))
        .unwrap()
        .results
        .iter()
        .find(|r| r.node.kind == DiscoveryKind::Block)
        .unwrap()
        .node
        .reference
        .clone();
    let plan = engine
        .planner()
        .unwrap()
        .plan_update(decode(json!({"reference":"REQ-A","title":"Changed"})))
        .unwrap();
    engine.apply_plan(plan).unwrap();
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_relation(&relation, false, Some(&occurrence))
            .is_err()
    );
    assert!(
        engine
            .item_list(decode(json!({"limit":1,"cursor":cursor})))
            .is_err()
    );
    assert!(engine.get(decode(json!({"reference":handle}))).is_err());
    let mut snapshot = engine.snapshot();
    snapshot.selection.ignores.push(IgnoreInput {
        directory: "".into(),
        root_prefix: vec![],
        content: "blocked/\n".into(),
    });
    engine = SourceEngine::new(snapshot).unwrap();
    let before = engine.source_fingerprint().to_owned();
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_move("REQ-A", Path::new("blocked/new.mara.md"), None)
            .is_err()
    );
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_create(
                request("REQ-C", "blocked/new.mara.md", "Body"),
                "01ARZ3NDEKTSV4RRFFQ69G5F02"
            )
            .is_err()
    );
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_move("REQ-A", Path::new("../escape.mara.md"), None)
            .is_err()
    );
    assert_eq!(engine.source_fingerprint(), before);
    let config = engine.files()[PROJECT_FILE].replace("**/*.mara.md", "a.mara.md");
    let mut snapshot = engine.snapshot();
    snapshot.files.retain(|f| f.path != "b.mara.md");
    snapshot
        .selection
        .document_paths
        .retain(|p| p != "b.mara.md");
    for file in &mut snapshot.files {
        if file.path == PROJECT_FILE {
            file.content = config.clone();
        }
    }
    let engine = SourceEngine::new(snapshot).unwrap();
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_create(
                request("REQ-C", "new.mara.md", "Body"),
                "01ARZ3NDEKTSV4RRFFQ69G5F02"
            )
            .is_err()
    );
}

// @mara checks DES-SEMANTIC-CHANGE-PLANS
// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
#[test]
fn unavailable_required_code_blocks_every_mutation_but_unused_declarations_do_not() {
    let mut engine = initialized();
    create(&mut engine, "REQ-A", "a.mara.md", "Body", MID_A);
    let schema = format!(
        "{}\n  implements:\n    description: Code realizes a requirement\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: implemented_by\n",
        engine.files()[SCHEMA_FILE]
    );
    engine = with_file(&engine, SCHEMA_FILE, schema, false);
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_update(decode(json!({"reference":"REQ-A","title":"Allowed"})))
            .is_ok()
    );
    // An authored endpoint needs actual code information even without configured languages.
    let source =
        engine.files()["a.mara.md"].replacen(":title:", ":implemented_by: code:lib.rs\n:title:", 1);
    let authored = with_file(&engine, "a.mara.md", source, true);
    assert!(authored.planner().is_err());
    let config = engine.files()[PROJECT_FILE].replacen(
        "format_version = 1",
        "format_version = 3",
        1,
    ) + "\n[[code.languages]]\nname = 'rust'\nextensions = ['rs']\ncommand = ['never-executed', '{output}']\n";
    engine = with_file(&engine, PROJECT_FILE, config, false);
    let before = engine.source_fingerprint().to_owned();
    // All public mutation methods require the same planner gate, including no-ops and backfill.
    assert!(engine.planner().is_err());
    assert_eq!(engine.source_fingerprint(), before);
    assert!(engine.get(decode(json!({"reference":"REQ-A"}))).is_ok());
    assert!(engine.schema_get(None, None).is_ok());
}

// @mara checks DES-SEMANTIC-CHANGE-PLANS
#[test]
fn surviving_structural_and_item_references_reject_candidates_without_changing_inputs() {
    let mut engine = initialized();
    let source = format!(
        "# Same\n\n:::mara requirement REQ-A\n:mid: {MID_A}\n:title: A\n\n## Same\n\nBlock text.\n:::\n\n[Second](#same-1)\n[[REQ-A]]\n"
    );
    engine = with_file(&engine, "a.mara.md", source.clone(), true);
    let before = engine.source_fingerprint().to_owned();
    assert!(engine.planner().unwrap().plan_delete("REQ-A").is_err());
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_move("REQ-A", Path::new("new.mara.md"), None)
            .is_err()
    );
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_update(decode(json!({"reference":"REQ-A","body":"No heading."})))
            .is_err()
    );
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_create(
                ItemCreationRequest {
                    line: Some(1),
                    ..request("REQ-B", "a.mara.md", "# Same")
                },
                MID_B
            )
            .is_err()
    );
    assert_eq!(engine.source_fingerprint(), before);
    assert_eq!(engine.files()["a.mara.md"], source);
    let plan = engine
        .planner()
        .unwrap()
        .plan_rename("REQ-A", "REQ-NEW")
        .unwrap();
    engine.apply_plan(plan).unwrap();
    assert!(engine.files()["a.mara.md"].contains("[[REQ-NEW]]"));
}

// @mara checks DES-SEMANTIC-CHANGE-PLANS
#[test]
fn scaffold_exceptions_and_inline_occurrence_removal_keep_owning_profiles() {
    let mut engine = initialized();
    let plan = engine
        .planner()
        .unwrap()
        .plan_create(request("REQ-A", "a.mara.md", ""), MID_A)
        .unwrap();
    assert!(!plan.result().is_complete());
    engine.apply_plan(plan).unwrap();
    let plan = engine
        .planner()
        .unwrap()
        .plan_update(decode(json!({"reference":"REQ-A","title":"Draft"})))
        .unwrap();
    assert_eq!(plan.result().warnings.len(), 1);
    engine.apply_plan(plan).unwrap();
    assert!(
        engine
            .planner()
            .unwrap()
            .plan_update(decode(json!({"reference":"REQ-A","body":""})))
            .is_err()
    );
    assert!(engine.planner().unwrap().plan_delete("REQ-A").is_err());
    let plan = engine
        .planner()
        .unwrap()
        .plan_update(decode(json!({"reference":"REQ-A","body":"Body."})))
        .unwrap();
    engine.apply_plan(plan).unwrap();
    create(
        &mut engine,
        "REQ-B",
        "b.mara.md",
        "[[depends_on:REQ-A]] [[depends_on:REQ-A]]",
        MID_B,
    );
    let relation: RelationParams =
        decode(json!({"source":"REQ-B","relation":"depends_on","target":"REQ-A"}));
    let inspection = engine.relation_get(relation.clone(), None, None).unwrap();
    let plan = engine
        .planner()
        .unwrap()
        .plan_relation(&relation, false, Some(&inspection.occurrences[0].reference))
        .unwrap();
    assert_eq!(plan.result().remaining_occurrences, 1);
    engine.apply_plan(plan).unwrap();
    let plan = engine
        .planner()
        .unwrap()
        .plan_relation(&relation, false, None)
        .unwrap();
    engine.apply_plan(plan).unwrap();
    assert!(engine.files()["b.mara.md"].contains("[[REQ-A]] [[REQ-A]]"));
    assert!(engine.planner().unwrap().plan_delete("REQ-A").is_err());
}

// @mara checks DES-MID-BACKFILL
// @mara checks DES-SEMANTIC-CHANGE-PLANS
#[test]
fn backfill_preserves_incoming_self_and_contained_structural_destinations() {
    let source = format!(
        ":::mara requirement REQ-A\n:title: A\n\n## Inside\n\nSelf [[REQ-A]].\n:::\n\n:::mara requirement REQ-B\n:mid: {MID_B}\n:title: B\n\n[[depends_on:REQ-A]] [Heading](#inside).\n:::\n\nOutside [[REQ-A]] and [Heading](#inside).\n"
    );
    let mut engine = with_file(&initialized(), "a.mara.md", source.clone(), true);
    let plan = engine
        .planner()
        .unwrap()
        .plan_backfill(&[MID_A.into()])
        .unwrap();
    engine.apply_plan(plan).unwrap();
    assert_eq!(
        engine.files()["a.mara.md"],
        source.replacen(
            ":::mara requirement REQ-A\n",
            &format!(":::mara requirement REQ-A\n:mid: {MID_A}\n"),
            1
        )
    );
    assert!(
        engine
            .project_validate(&[], &ValidationOptions::default(), None)
            .unwrap()
            .valid
    );
}
