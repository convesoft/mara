#[path = "fixtures/document-portability/src/document_rules.rs"]
mod document_rules;

mod support;
use mara_engine::*;
use serde_json::{Value, json};
use std::fs;

fn normalized(mut value: Value) -> Value {
    value.as_object_mut().unwrap().remove("project");
    value.as_object_mut().unwrap().remove("path");
    value["next_cursor"] = Value::Null;
    value
}
// @mara checks DES-SEMANTIC-CHANGE-PLANS
// @mara checks DES-EXPLICIT-SOURCE-ENGINE
#[test]
fn applied_plan_invalidates_rule_outcomes_and_trace_cursors() {
    let mut engine = SourceEngine::new(document_rules::snapshot()).unwrap();
    let target = ValidationTarget {
        kind: ValidationTargetKind::Project,
        id: None,
    };
    let options = ValidationOptions::default();
    let old_checks = engine.rule_checks(target.clone()).unwrap();
    let before = engine
        .project_validate(&[], &options, Some(&old_checks))
        .unwrap();
    assert!(before.evaluation_complete);
    assert!(!before.valid); // Accepted REQ-B has no qualifying verification.
    let mut old_page = document_rules::params();
    old_page.cursor = Some(engine.trace_matrix(&old_page).unwrap().next_cursor.unwrap());

    let update = serde_json::from_value(json!({
        "reference": "REQ-B", "fields": [{"key": "status", "value": "draft"}]
    }))
    .unwrap();
    let plan = engine.planner().unwrap().plan_update(update).unwrap();
    engine.apply_plan(plan).unwrap();

    assert!(
        engine
            .project_validate(&[], &options, Some(&old_checks))
            .is_err()
    );
    assert!(engine.trace_matrix(&old_page).is_err());
    let new_checks = engine.rule_checks(target).unwrap();
    let after = engine
        .project_validate(&[], &options, Some(&new_checks))
        .unwrap();
    assert!(after.evaluation_complete && after.valid);
    let matrix = engine.trace_matrix(&document_rules::params()).unwrap();
    assert!(matrix.evaluation_complete);
    assert!(
        matrix
            .summaries
            .iter()
            .all(|summary| summary.failed == 0 && summary.unavailable == 0)
    );
}

// @mara checks DES-EXPLICIT-SOURCE-ENGINE
// @mara checks DES-TRACE-CHECK-BINDING
#[test]
fn explicit_rules_match_real_native_cli_and_target_bound_bridge() {
    let fixture = support::fixture();
    let root = fixture.path();
    let snapshot = document_rules::snapshot();
    for file in &snapshot.files {
        let path = root.join(&file.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, &file.content).unwrap();
    }
    let engine = SourceEngine::new(snapshot).unwrap();
    let project = mara::resolve_project(Some(root), root).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let (corpus, diagnostics) = mara::load_corpus_for_validation(&project, &schema).unwrap();
    let native_engine = mara::load_source_engine(&project, &corpus.documents).unwrap();
    for (kind, id, args) in [
        (
            ValidationTargetKind::Project,
            None,
            vec!["project", "validate"],
        ),
        (
            ValidationTargetKind::Schema,
            None,
            vec!["schema", "validate"],
        ),
        (
            ValidationTargetKind::Item,
            Some("REQ-A".to_owned()),
            vec!["item", "validate", "REQ-A"],
        ),
        (
            ValidationTargetKind::Item,
            Some("REQ-B".to_owned()),
            vec!["item", "validate", "REQ-B"],
        ),
    ] {
        let target = ValidationTarget {
            kind,
            id: id.clone(),
        };
        let native = mara::load_rule_checks(
            &project,
            &schema,
            &native_engine,
            target.clone(),
            (kind != ValidationTargetKind::Schema).then_some((&corpus, diagnostics.as_slice())),
        )
        .unwrap();
        let portable = engine.rule_checks(target).unwrap();
        assert_eq!(
            serde_json::to_value(&native.definitions.diagnostics).unwrap(),
            json!(portable.definitions.diagnostics)
        );
        if let (Some((_, a)), Some((_, b))) = (&native.evaluation, &portable.evaluation) {
            assert_eq!(a.evaluation_complete, b.evaluation_complete);
            assert_eq!(json!(a.diagnostics), json!(b.diagnostics));
        }
        let result = match kind {
            ValidationTargetKind::Schema => {
                engine.schema_validate(&ValidationOptions::default(), Some(&portable))
            }
            ValidationTargetKind::Project => {
                engine.project_validate(&[], &ValidationOptions::default(), Some(&portable))
            }
            ValidationTargetKind::Item => engine.item_validate(
                id.as_deref().unwrap(),
                &ValidationOptions::default(),
                Some(&portable),
            ),
        }
        .unwrap();
        let mut cli_args = vec!["--format", "json"];
        cli_args.extend(args);
        let cli = support::mara(root, &cli_args);
        let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
        assert_eq!(normalized(cli), normalized(json!(result)));
    }
    let params = document_rules::params();
    let portable = engine.trace_matrix(&params).unwrap();
    let output = support::mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--all",
            "--rule",
            "urn:mara:rule:coverage",
            "--rule",
            "urn:mara:rule:literals",
            "--limit",
            "2",
        ],
    );
    // CLI text rendering is selected by --format text; JSON is the portable envelope.
    let cli: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("{}", support::stderr(&output)));
    assert_eq!(cli["summaries"], json!(portable.summaries));
    assert_eq!(cli["evaluation_complete"], portable.evaluation_complete);
    for source in ["[]", &document_rules::snapshot().files[3].content] {
        let mut aliases = document_rules::snapshot();
        aliases.files[0].content = aliases.files[0].content.replace(
            "files = ['rules.yaml']",
            "files = ['./rules.yaml', 'rules.yaml']",
        );
        aliases.files[3].content = source.into();
        fs::write(root.join(PROJECT_FILE), &aliases.files[0].content).unwrap();
        fs::write(root.join("rules.yaml"), source).unwrap();
        let engine = SourceEngine::new(aliases).unwrap();
        let checks = engine
            .rule_checks(ValidationTarget {
                kind: ValidationTargetKind::Project,
                id: None,
            })
            .unwrap();
        let result = engine
            .project_validate(&[], &ValidationOptions::default(), Some(&checks))
            .unwrap();
        let cli = support::mara(root, &["--format", "json", "project", "validate"]);
        assert!(!cli.status.success());
        assert_eq!(
            normalized(serde_json::from_slice(&cli.stdout).unwrap()),
            normalized(json!(result))
        );
    }
}
