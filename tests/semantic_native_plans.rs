mod support;
use mara::*;
use serde_json::json;
use std::fs;

// @mara checks DES-SEMANTIC-CHANGE-PLANS
// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
#[test]
fn real_native_code_inputs_bind_plans_and_cannot_be_accepted_by_memory_hosts() {
    let directory = support::fixture();
    let project = initialize_project(directory.path(), Template::Minimal).unwrap();
    let schema_path = project.schema_path();
    let source = fs::read_to_string(schema_path).unwrap()
        + "\n  implements:\n    description: Code realizes an obligation\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: implemented_by\n";
    fs::write(schema_path, source).unwrap();
    fs::write(directory.path().join("lib.rs"), "pub fn original() {}\n").unwrap();
    let schema = load_schema(&project).unwrap();
    create_item(
        &project,
        &schema,
        ItemCreationRequest {
            flavour: "requirement".into(),
            id: "REQ-A".into(),
            file: "a.mara.md".into(),
            title: "A".into(),
            fields: vec![],
            relations: vec![],
            body: Some("Body".into()),
            line: None,
        },
    )
    .unwrap();
    let context = OperationContext::from_environment(Some(directory.path().to_owned())).unwrap();
    context
        .relation_add(RelationParams {
            source: "REQ-A".into(),
            relation: "implemented_by".into(),
            target: "code:lib.rs".into(),
        })
        .unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let mut engine = load_source_engine(&project, &corpus.documents).unwrap();
    assert!(engine.planner().is_err());
    let plan = engine
        .planner_with_code(corpus.code())
        .unwrap()
        .plan_update(
            serde_json::from_value(json!({"reference":"REQ-A","title":"Changed"})).unwrap(),
        )
        .unwrap();
    plan.check_preconditions(&engine, Some(corpus.code()))
        .unwrap();
    assert!(engine.apply_plan(plan.clone()).is_err());
    assert!(
        fs::read_to_string(directory.path().join("a.mara.md"))
            .unwrap()
            .contains(":title: A\n")
    );
    fs::write(directory.path().join("lib.rs"), "pub fn changed() {}\n").unwrap();
    let current = load_corpus(&project, &schema).unwrap();
    let current_engine = load_source_engine(&project, &current.documents).unwrap();
    assert_eq!(
        engine.source_fingerprint(),
        current_engine.source_fingerprint()
    );
    assert!(
        plan.check_preconditions(&current_engine, Some(current.code()))
            .is_err()
    );
    fs::write(directory.path().join("lib.rs"), "pub fn original() {}\n").unwrap();
    let current = load_corpus(&project, &schema).unwrap();
    plan.check_preconditions(&current_engine, Some(current.code()))
        .unwrap();
}

// @mara checks DES-SEMANTIC-CHANGE-PLANS
#[test]
fn native_plans_bind_new_file_only_dependencies_before_publication() {
    let directory = support::fixture();
    let project = initialize_project(directory.path(), Template::Minimal).unwrap();
    let source = fs::read_to_string(project.schema_path()).unwrap()
        + "\n  implements:\n    description: Code realizes an obligation\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: implemented_by\n";
    fs::write(project.schema_path(), source).unwrap();
    fs::write(directory.path().join("lib.rs"), "pub fn original() {}\n").unwrap();
    let schema = load_schema(&project).unwrap();
    create_item(
        &project,
        &schema,
        ItemCreationRequest {
            flavour: "requirement".into(),
            id: "REQ-A".into(),
            file: "a.mara.md".into(),
            title: "A".into(),
            fields: vec![],
            relations: vec![],
            body: Some("Body".into()),
            line: None,
        },
    )
    .unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let engine = load_source_engine(&project, &corpus.documents).unwrap();
    let planner = engine.planner_with_code(corpus.code()).unwrap();
    let relation = RelationParams {
        source: "REQ-A".into(),
        relation: "implemented_by".into(),
        target: "code:lib.rs".into(),
    };
    let added = planner.plan_relation(&relation, true, None).unwrap();
    let updated = planner
        .plan_update(
            serde_json::from_value(
                json!({"reference":"REQ-A","body":"[[implemented_by:code:lib.rs]]"}),
            )
            .unwrap(),
        )
        .unwrap();
    let created = planner
        .plan_create(
            ItemCreationRequest {
                flavour: "requirement".into(),
                id: "REQ-B".into(),
                file: "b.mara.md".into(),
                title: "B".into(),
                fields: vec![],
                relations: vec![],
                body: Some("[[implemented_by:code:lib.rs]]".into()),
                line: None,
            },
            "01ARZ3NDEKTSV4RRFFQ69G5F01",
        )
        .unwrap();
    for bytes in [Some("pub fn changed() {}\n"), None] {
        if let Some(bytes) = bytes {
            fs::write(directory.path().join("lib.rs"), bytes).unwrap();
        } else {
            fs::remove_file(directory.path().join("lib.rs")).unwrap();
        }
        let current = load_corpus(&project, &schema).unwrap();
        assert!(
            added
                .check_preconditions(&engine, Some(current.code()))
                .is_err()
        );
        assert!(
            updated
                .check_preconditions(&engine, Some(current.code()))
                .is_err()
        );
        assert!(
            created
                .check_preconditions(&engine, Some(current.code()))
                .is_err()
        );
    }
    assert!(!directory.path().join("b.mara.md").exists());
    assert!(
        !fs::read_to_string(directory.path().join("a.mara.md"))
            .unwrap()
            .contains("implemented_by")
    );
}

// @mara checks DES-SEMANTIC-CHANGE-PLANS
#[cfg(unix)]
#[test]
fn native_cli_mutations_preserve_host_filenames_outside_portable_snapshot_paths() {
    let directory = support::fixture();
    let project = initialize_project(directory.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    for (id, path) in [
        ("REQ-COLON", "notes:2026.mara.md"),
        ("REQ-BACKSLASH", "notes\\2026.mara.md"),
    ] {
        create_item(
            &project,
            &schema,
            ItemCreationRequest {
                flavour: "requirement".into(),
                id: id.into(),
                file: path.into(),
                title: id.into(),
                fields: vec![],
                relations: vec![],
                body: Some("Body".into()),
                line: None,
            },
        )
        .unwrap();
        let output = support::mara(
            directory.path(),
            &["item", "update", id, "--title", "Changed"],
        );
        assert!(output.status.success(), "{}", support::stderr(&output));
        assert!(
            fs::read_to_string(directory.path().join(path))
                .unwrap()
                .contains(":title: Changed\n")
        );
    }
    let output = support::mara(
        directory.path(),
        &["item", "move", "REQ-COLON", "new:2026.mara.md"],
    );
    assert!(output.status.success(), "{}", support::stderr(&output));
    let output = support::mara(
        directory.path(),
        &["item", "rename", "REQ-BACKSLASH", "REQ-RENAMED"],
    );
    assert!(output.status.success(), "{}", support::stderr(&output));
    let output = support::mara(directory.path(), &["item", "delete", "REQ-RENAMED"]);
    assert!(output.status.success(), "{}", support::stderr(&output));
    assert!(directory.path().join("notes\\2026.mara.md").exists());
    // Portable import remains unchanged and rejects these host-specific spellings.
    let docs = load_documents(&project, &schema).unwrap();
    assert!(load_source_engine(&project, &docs).is_err());
}

// @mara checks DES-MID-BACKFILL
#[test]
fn native_backfill_keeps_legacy_incoming_references() {
    let directory = support::fixture();
    initialize_project(directory.path(), Template::Minimal).unwrap();
    fs::write(
        directory.path().join("a.mara.md"),
        ":::mara requirement REQ-A\n:title: A\n\nSelf [[REQ-A]].\n:::\n\nNarrative [[REQ-A]].\n",
    )
    .unwrap();
    let output = support::mara(directory.path(), &["project", "mid", "backfill"]);
    assert!(output.status.success(), "{}", support::stderr(&output));
    assert!(
        fs::read_to_string(directory.path().join("a.mara.md"))
            .unwrap()
            .contains("Narrative [[REQ-A]].")
    );
}
