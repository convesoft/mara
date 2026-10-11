//! Real embedded/native CLI/MCP parity and per-operation loading. No daemon substitute.
mod support;
use mara::*;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}
fn json_result(value: impl serde::Serialize) -> Value {
    serde_json::to_value(value).unwrap()
}

// @mara checks DES-EXECUTION-BOUNDARIES
// @mara checks DES-OPERATION-PROJECT-CONTEXT
#[test]
fn relative_embedded_selection_uses_caller_directory_for_initialization_and_reads() {
    let fixture = support::fixture();
    let caller = fixture.path().join("caller");
    let process_directory = fixture.path().join("process");
    fs::create_dir(&caller).unwrap();
    fs::create_dir(&process_directory).unwrap();
    let executable = std::env::current_exe().unwrap();
    let output = support::isolated_command(executable.to_str().unwrap(), &process_directory)
        .args([
            "--ignored",
            "--exact",
            "relative_embedded_selection_child",
            "--nocapture",
        ])
        .env("MARA_NATIVE_RUNTIME_TEST_CALLER", &caller)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        support::stdout(&output),
        support::stderr(&output)
    );
    assert!(caller.join("selected/.mara/project.toml").is_file());
    assert!(!process_directory.join("selected").exists());
}

#[test]
#[ignore = "subprocess helper with a disposable process CWD"]
fn relative_embedded_selection_child() {
    let caller = PathBuf::from(std::env::var_os("MARA_NATIVE_RUNTIME_TEST_CALLER").unwrap());
    let runtime = NativeRuntime::new(Some("selected".into()), caller.clone()).unwrap();
    let result = runtime.project_initialize(None, Template::Minimal).unwrap();
    assert_eq!(result.project.root, caller.join("selected"));
    assert!(runtime.schema_validate().unwrap().valid);
    assert!(runtime.project_validate(&[]).unwrap().valid);
    assert!(!std::env::current_dir().unwrap().join("selected").exists());
}

// @mara checks DES-EXPLICIT-SOURCE-ENGINE
#[test]
fn loaded_validation_requires_current_target_bound_rule_outcomes() {
    let fixture = support::fixture();
    let original = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let configuration = fs::read_to_string(original.root().join(PROJECT_FILE))
        .unwrap()
        .replacen("format_version = 1", "format_version = 2", 1)
        + "\n[rules]\nformat_version = 1\nfiles = [\"rules.yaml\"]\n";
    fs::write(original.root().join(PROJECT_FILE), configuration).unwrap();
    fs::write(original.root().join("rules.yaml"), "[]\n").unwrap();
    let project = resolve_project(Some(fixture.path()), fixture.path()).unwrap();
    let schema = load_schema(&project).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let engine = load_native_source_engine(&project, &corpus.documents).unwrap();
    let runtime =
        NativeRuntime::new(Some(fixture.path().to_owned()), fixture.path().to_owned()).unwrap();
    let baseline = runtime.project_validate(&[]).unwrap();
    assert!(baseline.valid);
    let checks = load_rule_checks(
        &project,
        &schema,
        &engine,
        baseline.target.clone(),
        Some((&corpus, &[])),
    )
    .unwrap();
    for stale_target in [false, true] {
        let mut stale = checks.clone();
        if stale_target {
            stale.evaluation.as_mut().unwrap().0.kind = ValidationTargetKind::Schema;
        } else {
            stale.source_fingerprint.push_str("-stale");
        }
        assert!(
            mara_engine::validation::evaluate_loaded(
                &project,
                Some(&schema),
                Some((&corpus, vec![])),
                Some(&stale),
                engine.source_fingerprint(),
                &mut baseline.clone()
            )
            .is_err()
        );
    }
    for definitions_only in [false, true] {
        let mut result = baseline.clone();
        let mut checks = checks.clone();
        checks.evaluation = None;
        mara_engine::validation::evaluate_loaded(
            &project,
            Some(&schema),
            Some((&corpus, vec![])),
            definitions_only.then_some(&checks),
            engine.source_fingerprint(),
            &mut result,
        )
        .unwrap();
        result.summarize();
        assert!(!result.valid);
        assert!(!result.evaluation_complete);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::EvaluationUnavailable)
        );
    }
}

// @mara checks DES-EXECUTION-BOUNDARIES
// @mara checks REQ-SURFACE-PARITY
#[test]
fn embedded_runtime_covers_document_operations_without_environment_or_routing() {
    let fixture = support::fixture();
    let root = fixture.path();
    let runtime = NativeRuntime::new(Some(root.to_owned()), root.to_owned()).unwrap();
    runtime.project_initialize(None, Template::Minimal).unwrap();
    runtime.schema_get(None, None).unwrap();
    runtime.schema_list(SchemaKind::Flavour).unwrap();
    let created = runtime
        .item_create(decode(json!({
            "flavour":"requirement", "id":"REQ-A", "file":"a.mara.md",
            "title":"A", "body":"Preserve native behavior."
        })))
        .unwrap();
    runtime
        .item_create(decode(json!({
            "flavour":"requirement", "id":"REQ-B", "file":"b.mara.md",
            "title":"B", "body":"Second item."
        })))
        .unwrap();
    let edge = || RelationParams {
        source: "REQ-A".into(),
        relation: "depends_on".into(),
        target: "REQ-B".into(),
    };
    runtime.relation_add(edge()).unwrap();
    runtime.relation_get(edge(), None, None).unwrap();
    runtime
        .related(decode(json!({"reference":"REQ-B"})))
        .unwrap();
    runtime.relation_remove_occurrence(edge(), None).unwrap();
    runtime
        .item_update(decode(json!({"reference":"REQ-A","title":"Changed"})))
        .unwrap();
    runtime
        .item_move(decode(json!({"reference":"REQ-A","file":"moved.mara.md"})))
        .unwrap();
    runtime.item_rename("REQ-A", "REQ-RENAMED").unwrap();
    let read = runtime
        .get(decode(json!({"reference":created.mid})))
        .unwrap();
    assert!(json_result(&read).to_string().contains("Changed"));
    let cli = support::mara(root, &["--format", "json", "get", &created.mid]);
    assert!(cli.status.success(), "{}", support::stderr(&cli));
    assert_eq!(
        decode::<Value>(serde_json::from_slice(&cli.stdout).unwrap()),
        json_result(&read)
    );
    let responses = support::mcp_exchange(
        root,
        &[
            support::mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            support::mcp_call(2, "get", json!({"reference":created.mid})),
        ],
    );
    assert_eq!(
        support::mcp_response(&responses, 2)["result"]["structuredContent"],
        json_result(read)
    );
    runtime.item_list(decode(json!({}))).unwrap();
    runtime.search(decode(json!({"query":"Changed"}))).unwrap();
    assert!(runtime.item_validate("REQ-RENAMED").unwrap().valid);
    assert!(runtime.project_validate(&[]).unwrap().valid);
    assert!(runtime.schema_validate().unwrap().valid);
    assert!(runtime.project_mid_backfill().unwrap().changed.is_empty());
    runtime.item_delete("REQ-RENAMED").unwrap();
    assert_eq!(fs::read(root.join("moved.mara.md")).unwrap(), b"");
    assert!(
        runtime
            .project_transaction_rollback()
            .unwrap()
            .restored
            .is_empty()
    );
    // Reusing the runtime must not mask current invalid source, then must see repair.
    let source = fs::read(root.join("b.mara.md")).unwrap();
    fs::write(root.join("b.mara.md"), ":::mara unknown BAD\n").unwrap();
    assert!(!runtime.project_validate(&[]).unwrap().valid);
    assert!(runtime.get(decode(json!({"reference":"REQ-B"}))).is_err());
    fs::write(root.join("b.mara.md"), source).unwrap();
    assert!(runtime.project_validate(&[]).unwrap().valid);
}

// @mara checks DES-EXECUTION-BOUNDARIES
#[test]
fn embedded_rules_trace_and_selection_keep_native_inputs_and_results() {
    let fixture = support::fixture();
    let root = fixture.path();
    let runtime = NativeRuntime::new(None, root.to_owned()).unwrap();
    runtime
        .project_initialize(Some(root.to_owned()), Template::EngineeringLegacy)
        .unwrap();
    assert!(runtime.schema_validate().unwrap().valid);
    assert!(runtime.project_validate(&[]).unwrap().valid);
    runtime
        .item_create(decode(json!({
            "flavour":"requirement", "id":"REQ-A", "file":"a.mara.md",
            "title":"A", "body":"A real draft.", "fields":[{"key":"status","value":"draft"}]
        })))
        .unwrap();
    let params: TraceMatrixParams = decode(json!({"ids":["REQ-A"], "check":{
        "files":[".mara/engineering-checks.yaml"], "shape":"urn:mara:rule:intent"
    }}));
    let matrix = runtime.trace_matrix(&params).unwrap();
    assert!(matrix.evaluation_complete);
    let cli = support::mara(
        root,
        &[
            "--format",
            "json",
            "trace",
            "matrix",
            "--id",
            "REQ-A",
            "--check-file",
            ".mara/engineering-checks.yaml",
            "--shape",
            "urn:mara:rule:intent",
        ],
    );
    assert!(cli.status.success(), "{}", support::stderr(&cli));
    assert_eq!(
        serde_json::from_slice::<Value>(&cli.stdout).unwrap(),
        json_result(matrix)
    );
    assert!(
        runtime
            .for_project(Some(PathBuf::from("relative")))
            .is_err()
    );
    assert!(NativeRuntime::new(None, "relative".into()).is_err());
    let bound = NativeRuntime::new(Some(root.to_owned()), root.to_owned()).unwrap();
    assert!(bound.for_project(Some(root.to_owned())).is_err());
    fs::write(
        root.join(".mara/engineering-rules.yaml"),
        "shapes: not-a-sequence",
    )
    .unwrap();
    assert!(!runtime.schema_validate().unwrap().valid);
    assert!(!runtime.project_validate(&[]).unwrap().evaluation_complete);
}

// @mara checks DES-SEMANTIC-CHANGE-PLANS
#[cfg(unix)]
#[test]
fn trusted_native_engine_preserves_host_only_filenames_and_rejects_unknown_ignore_bytes() {
    let fixture = support::fixture();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    create_item(
        &project,
        &schema,
        ItemCreationRequest {
            flavour: "requirement".into(),
            id: "REQ-A".into(),
            file: "native:literal\\name.mara.md".into(),
            title: "A".into(),
            fields: vec![],
            relations: vec![],
            body: Some("Body.".into()),
            line: None,
        },
    )
    .unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let engine = load_native_source_engine(&project, &corpus.documents).unwrap();
    assert_eq!(engine.source_fingerprint(), corpus.source_fingerprint);
    assert!(
        engine
            .planner_with_code(corpus.code())
            .unwrap()
            .plan_rename("REQ-A", "REQ-B")
            .is_ok()
    );
    // The same trusted bridge must preserve native configuration/rule filenames too.
    let schema_name = ".mara/schema:literal\\name.yaml";
    let rule_name = ".mara/rules:literal\\name.yaml";
    fs::rename(project.schema_path(), fixture.path().join(schema_name)).unwrap();
    fs::write(fixture.path().join(rule_name), "[]\n").unwrap();
    let configuration = fs::read_to_string(fixture.path().join(PROJECT_FILE))
        .unwrap()
        .replacen("format_version = 1", "format_version = 2", 1)
        .replace(
            "schema = \".mara/schema.yaml\"",
            &format!("schema = {schema_name:?}"),
        )
        + &format!("\n[rules]\nformat_version = 1\nfiles = [{rule_name:?}]\n");
    fs::write(fixture.path().join(PROJECT_FILE), configuration).unwrap();
    let project = resolve_project(Some(fixture.path()), fixture.path()).unwrap();
    let schema = load_schema(&project).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let engine = load_native_source_engine(&project, &corpus.documents).unwrap();
    let checks = load_rule_checks(
        &project,
        &schema,
        &engine,
        ValidationTarget {
            kind: ValidationTargetKind::Project,
            id: None,
        },
        Some((&corpus, &[])),
    )
    .unwrap();
    assert!(checks.definitions.evaluation_complete);
    assert!(
        NativeRuntime::new(Some(fixture.path().to_owned()), fixture.path().to_owned())
            .unwrap()
            .project_validate(&[])
            .unwrap()
            .valid
    );
    fs::write(fixture.path().join(".gitignore"), [0xff]).unwrap();
    assert!(load_native_source_engine(&project, &corpus.documents).is_err());
}
