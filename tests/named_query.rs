mod support;
use mara::{NativeRuntime, SourceEngine, SourceSnapshot, named_query as q};
use serde_json::{Value, json};
use std::fs;
use support::*;
fn snapshot() -> SourceSnapshot {
    serde_json::from_str(include_str!("fixtures/named-query/snapshot.json")).unwrap()
}
fn project() -> tempfile::TempDir {
    let fixture = fixture();
    for file in snapshot().files {
        let path = fixture.path().join(file.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, file.content).unwrap();
    }
    fixture
}
fn write_queries(root: &std::path::Path, mut definitions: Value) {
    definitions["format_version"] = json!(1);
    for definition in definitions["definitions"]
        .as_array_mut()
        .expect("definition array")
    {
        definition
            .as_object_mut()
            .expect("definition object")
            .entry("required_fact_sources")
            .or_insert_with(|| json!(["documents"]));
    }
    fs::write(root.join("queries.json"), definitions.to_string()).unwrap();
}

fn request() -> q::Request {
    q::Request {
        definition_files: vec!["queries.json".into()],
        definition: "urn:fixture:query:supports".into(),
        selection: q::Selection {
            ids: vec!["SUB-LINKED".into(), "SUB-EMPTY".into()],
            ..Default::default()
        },
        parameters: vec![q::BindingParameter {
            name: "state".into(),
            value: q::ParameterValue::Text("".into()),
        }],
        max_rows: 100,
        limit: 1,
        cursor: None,
        fact_snapshots: vec![],
        mappings: vec![],
    }
}
fn cli(root: &std::path::Path, req: &q::Request) -> std::process::Output {
    let mut args = vec![
        "--execution-mode".to_owned(),
        "direct".into(),
        "--format".into(),
        "json".into(),
        "query".into(),
        "execute".into(),
        req.definition.clone(),
        "--definition-file".into(),
        "queries.json".into(),
        "--id".into(),
        "SUB-LINKED".into(),
        "--id".into(),
        "SUB-EMPTY".into(),
        "--param".into(),
        format!(
            "state={}",
            serde_json::to_string(&req.parameters[0].value).unwrap()
        ),
        "--max-rows".into(),
        req.max_rows.to_string(),
        "--limit".into(),
        req.limit.to_string(),
    ];
    if let Some(cursor) = &req.cursor {
        args.extend(["--cursor".into(), cursor.clone()]);
    }
    mara(root, &args.iter().map(String::as_str).collect::<Vec<_>>())
}
fn composition_cli(root: &std::path::Path, req: &q::Request) -> std::process::Output {
    let mut args = vec![
        "--execution-mode".to_owned(),
        "direct".into(),
        "--format".into(),
        "json".into(),
        "query".into(),
        "execute".into(),
        req.definition.clone(),
    ];
    for file in &req.definition_files {
        args.extend(["--definition-file".into(), file.clone()]);
    }
    for id in &req.selection.ids {
        args.extend(["--id".into(), id.clone()]);
    }
    args.extend([
        "--max-rows".into(),
        req.max_rows.to_string(),
        "--limit".into(),
        req.limit.to_string(),
    ]);
    mara(root, &args.iter().map(String::as_str).collect::<Vec<_>>())
}
fn mcp(root: &std::path::Path, req: &q::Request) -> Value {
    let responses = mcp_exchange_with_arguments(
        root,
        &["--execution-mode", "direct", "mcp"],
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "query_execute", serde_json::to_value(req).unwrap()),
        ],
    );
    let response = mcp_response(&responses, 2);
    assert_ne!(response["result"]["isError"], true, "{response:#}");
    response["result"]["structuredContent"].clone()
}
fn install_query_consumers(root: &std::path::Path) {
    let config = root.join(".mara/project.toml");
    let mut project = fs::read_to_string(&config).unwrap();
    project.push_str("\n[query_policy]\nformat_version = 1\nfiles = [\"invariants.yaml\"]\ndefinition_files = [\"queries.json\"]\nshape_files = [\"presence.ttl\"]\nmax_rows = 100\n");
    fs::write(config, project).unwrap();
    fs::write(
        root.join("invariants.yaml"),
        r#"format_version: 1
invariants:
  - id: require-query-match
    definition: urn:fixture:query:supports
    shape: urn:fixture:shape:has-match
    flavours: [subject]
    severity: error
    parameters:
      - name: state
        value: {type: text, value: accepted}
"#,
    )
    .unwrap();
    fs::write(
        root.join("gate.yaml"),
        r#"format_version: 1
gates:
  - id: require-support
    empty_roots: not_applicable
    empty_required: not_applicable
    conditions:
      - id: support-condition
        definition: urn:fixture:query:supports
        shape_file: presence.ttl
        shape: urn:fixture:shape:has-match
        severity: error
        context_parameters: [state]
  - id: empty-required
    empty_roots: passed
    empty_required: failed
    conditions: []
  - id: empty-roots
    empty_roots: not_applicable
    empty_required: failed
    conditions: []
"#,
    )
    .unwrap();
    fs::write(
        root.join("presence.ttl"),
        r#"@prefix sh: <http://www.w3.org/ns/shacl#>.
<urn:fixture:shape:has-match> a sh:NodeShape;
  sh:property [ sh:path <urn:mara:query:match>; sh:minCount 1 ].
"#,
    )
    .unwrap();
}
fn consumer_snapshot() -> SourceSnapshot {
    let mut snapshot = snapshot();
    let project = snapshot
        .files
        .iter_mut()
        .find(|file| file.path == ".mara/project.toml")
        .unwrap();
    project.content.push_str("\n[query_policy]\nformat_version = 1\nfiles = [\"invariants.yaml\"]\ndefinition_files = [\"queries.json\"]\nshape_files = [\"presence.ttl\"]\nmax_rows = 100\n");
    snapshot.files.extend([
        mara::SourceFile { path: "invariants.yaml".into(), content: r#"format_version: 1
invariants:
  - id: require-query-match
    definition: urn:fixture:query:supports
    shape: urn:fixture:shape:has-match
    flavours: [subject]
    severity: error
    parameters:
      - name: state
        value: {type: text, value: accepted}
"#.into() },
        mara::SourceFile { path: "gate.yaml".into(), content: r#"format_version: 1
gates:
  - id: require-support
    empty_roots: not_applicable
    empty_required: not_applicable
    conditions:
      - id: support-condition
        definition: urn:fixture:query:supports
        shape_file: presence.ttl
        shape: urn:fixture:shape:has-match
        severity: error
        context_parameters: [state]
  - id: empty-required
    empty_roots: passed
    empty_required: failed
    conditions: []
  - id: empty-roots
    empty_roots: not_applicable
    empty_required: failed
    conditions: []
"#.into() },
        mara::SourceFile { path: "presence.ttl".into(), content: "@prefix sh: <http://www.w3.org/ns/shacl#>.\n<urn:fixture:shape:has-match> a sh:NodeShape; sh:property [ sh:path <urn:mara:query:match>; sh:minCount 1 ].\n".into() },
    ]);
    snapshot
}
fn empty_facts_request() -> q::Request {
    q::Request {
        definition_files: vec!["queries.json".into()],
        definition: "urn:fixture:query:supports".into(),
        selection: q::Selection {
            ids: vec!["SUB-EMPTY".into()],
            ..Default::default()
        },
        parameters: vec![q::BindingParameter {
            name: "state".into(),
            value: q::ParameterValue::Text("accepted".into()),
        }],
        max_rows: 100,
        limit: 100,
        cursor: None,
        fact_snapshots: vec![],
        mappings: vec![],
    }
}
fn consumer_mcp(root: &std::path::Path, name: &str, params: Value) -> Value {
    let responses = mcp_exchange_with_arguments(
        root,
        &["--execution-mode", "direct", "mcp"],
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, name, params),
        ],
    );
    let response = mcp_response(&responses, 2);
    assert_ne!(response["result"]["isError"], true, "{response:#}");
    response["result"]["structuredContent"].clone()
}

// @mara checks REQ-QUERY-POLICY-CONSUMERS
// @mara checks REQ-NAMED-QUERY-FACTS
#[test]
fn shared_query_facts_become_invariant_and_gate_failures_across_rust_cli_and_mcp() {
    let fixture = project();
    install_query_consumers(fixture.path());
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let portable = SourceEngine::new(consumer_snapshot()).unwrap();
    let request = empty_facts_request();

    let facts = runtime.query_execute(request.clone()).unwrap();
    assert!(facts.evaluation_complete);
    assert_eq!(facts.summary.binding_count, Some(0));
    assert!(matches!(
        facts.records.as_slice(),
        [q::Record::Root {
            binding_count: Some(0),
            ..
        }]
    ));
    let direct_facts = portable.query_execute(request.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&direct_facts.records).unwrap(),
        serde_json::to_value(&facts.records).unwrap()
    );
    let cli_facts = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "urn:fixture:query:supports",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-EMPTY",
            "--param",
            "state={\"type\":\"text\",\"value\":\"accepted\"}",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(
        cli_facts.status.success(),
        "{} {}",
        stdout(&cli_facts),
        stderr(&cli_facts)
    );
    let cli_facts: Value = serde_json::from_slice(&cli_facts.stdout).unwrap();
    assert_eq!(cli_facts, serde_json::to_value(&facts).unwrap());
    assert_eq!(
        consumer_mcp(
            fixture.path(),
            "query_execute",
            serde_json::to_value(&request).unwrap()
        ),
        cli_facts
    );

    let (portable_policy, portable_diagnostics) =
        portable.query_policy_validation(mara::ValidationTarget {
            kind: mara::ValidationTargetKind::Project,
            id: None,
        });
    let portable_policy = portable_policy.unwrap();
    assert!(
        portable_policy.evaluation_complete,
        "{portable_policy:#?} {portable_diagnostics:#?}"
    );
    let portable_failure = portable_policy
        .checks
        .iter()
        .find(
            |check| matches!(&check.root, Some(q::Endpoint::Item { id, .. }) if id == "SUB-EMPTY"),
        )
        .unwrap();
    assert_eq!(
        portable_failure.state,
        mara::query_consumers::ConsumerState::Failed
    );
    assert_eq!(
        portable_failure.applicability,
        mara::query_consumers::ConsumerState::Passed
    );
    assert_eq!(portable_failure.severity, mara::Severity::Error);
    assert_eq!(portable_failure.binding_count, Some(0));
    assert_eq!(
        portable_failure.source.path.as_deref(),
        Some(std::path::Path::new("invariants.yaml"))
    );
    assert!(!portable_failure.components.is_empty());
    let schema_validation = runtime.schema_validate().unwrap();
    assert!(schema_validation.valid && schema_validation.evaluation_complete);
    assert!(
        schema_validation
            .query_policies
            .as_ref()
            .unwrap()
            .checks
            .iter()
            .all(|check| check.state == mara::query_consumers::ConsumerState::NotApplicable)
    );
    let validation = runtime.project_validate(&[]).unwrap();
    assert!(!validation.valid && validation.evaluation_complete);
    let expected_policy_checks = validation.query_policies.as_ref().unwrap().checks.len();
    let mut cursor = None;
    let mut paged_policy_checks = Vec::new();
    loop {
        let page = runtime
            .project_validate_with_options(
                &[],
                &mara::ValidationOptions {
                    limit: Some(1),
                    cursor,
                    fact_snapshots: vec![],
                    mappings: vec![],
                },
            )
            .unwrap();
        let policy_checks = page.query_policies.as_ref().unwrap().checks.clone();
        assert!(page.diagnostics.len() + policy_checks.len() <= 1);
        assert_eq!(page.valid, validation.valid);
        paged_policy_checks.extend(policy_checks);
        if !page.has_more {
            break;
        }
        cursor = page.next_cursor.clone();
        assert!(cursor.is_some());
    }
    assert_eq!(paged_policy_checks.len(), expected_policy_checks);
    let cli_page = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "project",
            "validate",
            "--limit",
            "1",
        ],
    );
    assert!(
        !cli_page.status.success(),
        "invalid validation must retain its CLI failure status"
    );
    let cli_page: Value = serde_json::from_slice(&cli_page.stdout).unwrap();
    assert_eq!(
        cli_page.get("has_more"),
        Some(&Value::Bool(true)),
        "{cli_page}"
    );
    assert_eq!(
        cli_page["diagnostics"].as_array().unwrap().len()
            + cli_page["query_policies"]["checks"]
                .as_array()
                .unwrap()
                .len(),
        1
    );
    assert_eq!(
        consumer_mcp(fixture.path(), "project_validate", json!({"limit":1})),
        cli_page
    );
    assert!(
        validation
            .diagnostics
            .iter()
            .any(|d| d.rule.as_deref() == Some("require-query-match")
                && d.severity == mara::Severity::Error)
    );
    let native_failure = validation
        .query_policies
        .as_ref()
        .unwrap()
        .checks
        .iter()
        .find(
            |check| matches!(&check.root, Some(q::Endpoint::Item { id, .. }) if id == "SUB-EMPTY"),
        )
        .unwrap();
    assert_eq!(
        native_failure.state,
        mara::query_consumers::ConsumerState::Failed
    );
    assert_eq!(
        serde_json::to_value(&native_failure.bindings).unwrap(),
        serde_json::to_value(
            facts
                .records
                .iter()
                .filter_map(|record| match record {
                    q::Record::Binding { binding, .. } => Some(binding),
                    _ => None,
                })
                .collect::<Vec<_>>()
        )
        .unwrap()
    );
    let item_validation = runtime.item_validate("SUB-EMPTY").unwrap();
    assert!(!item_validation.valid && item_validation.evaluation_complete);
    assert!(
        item_validation
            .diagnostics
            .iter()
            .any(|d| d.rule.as_deref() == Some("require-query-match"))
    );
    let unrelated_item = runtime.item_validate("SUP-A").unwrap();
    assert!(
        unrelated_item
            .query_policies
            .as_ref()
            .unwrap()
            .checks
            .iter()
            .all(|check| check.state == mara::query_consumers::ConsumerState::NotApplicable)
    );
    let cli_validation = mara(fixture.path(), &["--format", "json", "project", "validate"]);
    let cli_validation: Value = serde_json::from_slice(&cli_validation.stdout).unwrap();
    assert_eq!(
        cli_validation["query_policies"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["root"]["id"] == "SUB-EMPTY")
            .unwrap()["state"],
        "failed"
    );
    let mcp_validation = consumer_mcp(fixture.path(), "project_validate", json!({}));
    assert_eq!(
        mcp_validation["query_policies"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["root"]["id"] == "SUB-EMPTY")
            .unwrap()["state"],
        "failed"
    );
    let invariant_path = fixture.path().join("invariants.yaml");
    let invariant_text = fs::read_to_string(&invariant_path).unwrap();
    fs::write(
        &invariant_path,
        invariant_text.replace("severity: error", "severity: warning"),
    )
    .unwrap();
    let warning_validation = runtime.project_validate(&[]).unwrap();
    assert!(warning_validation.valid && warning_validation.evaluation_complete);
    let warning_failure = warning_validation
        .query_policies
        .as_ref()
        .unwrap()
        .checks
        .iter()
        .find(
            |check| matches!(&check.root, Some(q::Endpoint::Item { id, .. }) if id == "SUB-EMPTY"),
        )
        .unwrap();
    assert_eq!(
        warning_failure.state,
        mara::query_consumers::ConsumerState::Failed
    );
    assert_eq!(warning_failure.severity, mara::Severity::Warning);
    assert!(
        warning_validation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == mara::Severity::Warning
                && diagnostic.rule.as_deref() == Some("require-query-match"))
    );
    fs::write(&invariant_path, invariant_text).unwrap();
    let document_path = fixture.path().join("fixture.mara.md");
    let document_text = fs::read_to_string(&document_path).unwrap();
    fs::write(
        &document_path,
        document_text.replacen(":status: draft", ":status: invalid", 1),
    )
    .unwrap();
    let invalid_source = runtime.project_validate(&[]).unwrap();
    assert!(
        invalid_source
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == mara::DiagnosticCode::FieldInvalid)
    );
    assert!(
        !invalid_source
            .query_policies
            .as_ref()
            .unwrap()
            .evaluation_complete
    );
    assert!(
        invalid_source
            .query_policies
            .as_ref()
            .unwrap()
            .checks
            .iter()
            .all(|check| check.state != mara::query_consumers::ConsumerState::Passed)
    );
    fs::write(
        fixture.path().join("empty-root-pass.yaml"),
        r#"format_version: 1
gates:
  - id: empty-root-passes
    empty_roots: passed
    empty_required: passed
    conditions:
      - id: support
        definition: urn:fixture:query:supports
        shape_file: presence.ttl
        shape: urn:fixture:shape:has-match
        severity: error
        context_parameters: [state]
"#,
    )
    .unwrap();
    let invalid_scope_gate = runtime
        .query_gate(mara::query_consumers::QueryGateRequest {
            definition_files: vec!["queries.json".into()],
            gate_file: "empty-root-pass.yaml".into(),
            shape_files: vec!["presence.ttl".into()],
            gate: "empty-root-passes".into(),
            scope: q::Selection {
                fields: vec![q::FieldSelection {
                    key: "status".into(),
                    value: "no-match".into(),
                }],
                ..Default::default()
            },
            context: vec![],
            fact_snapshots: vec![],
            mappings: vec![],
            max_rows: 100,
        })
        .unwrap();
    assert_eq!(
        invalid_scope_gate.state,
        mara::query_consumers::ConsumerState::Unavailable
    );
    assert!(!invalid_scope_gate.evaluation_complete);
    assert!(invalid_scope_gate.roots.is_empty());
    assert!(invalid_scope_gate.empty_reason.is_none());
    fs::write(&document_path, document_text).unwrap();

    let gate = mara::query_consumers::QueryGateRequest {
        definition_files: vec!["queries.json".into()],
        gate_file: "gate.yaml".into(),
        shape_files: vec!["presence.ttl".into()],
        gate: "require-support".into(),
        scope: request.selection.clone(),
        context: request.parameters.clone(),
        fact_snapshots: vec![],
        mappings: vec![],
        max_rows: 100,
    };
    let gate_result = runtime.query_gate(gate.clone()).unwrap();
    let portable_gate = portable.query_gate(gate.clone()).unwrap();
    let condition = &gate_result.roots[0].conditions[0];
    assert_eq!(
        condition.state,
        mara::query_consumers::ConsumerState::Failed
    );
    assert_eq!(
        condition.applicability,
        mara::query_consumers::ConsumerState::Passed
    );
    assert_eq!(condition.severity, mara::Severity::Error);
    assert_eq!(condition.binding_count, Some(0));
    assert_eq!(
        gate_result.source.path.as_deref(),
        Some(std::path::Path::new("gate.yaml"))
    );
    assert_eq!(
        condition.source.path.as_deref(),
        Some(std::path::Path::new("gate.yaml"))
    );
    assert!(gate_result.evaluation_complete);
    let gate_cli = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "gate",
            "require-support",
            "--gate-file",
            "gate.yaml",
            "--definition-file",
            "queries.json",
            "--shape-file",
            "presence.ttl",
            "--id",
            "SUB-EMPTY",
            "--param",
            "state={\"type\":\"text\",\"value\":\"accepted\"}",
            "--max-rows",
            "100",
        ],
    );
    assert!(
        gate_cli.status.success(),
        "{} {}",
        stdout(&gate_cli),
        stderr(&gate_cli)
    );
    let gate_cli: Value = serde_json::from_slice(&gate_cli.stdout).unwrap();
    assert_eq!(gate_cli, serde_json::to_value(&gate_result).unwrap());
    let gate_mcp = consumer_mcp(
        fixture.path(),
        "query_gate",
        serde_json::to_value(&gate).unwrap(),
    );
    assert_eq!(gate_mcp, gate_cli);
    assert_eq!(portable_gate.roots[0].conditions[0].state, condition.state);

    fs::write(
        fixture.path().join("scope-one.ttl"),
        "@prefix sh: <http://www.w3.org/ns/shacl#>.\n<urn:fixture:shape:shared> a sh:NodeShape; sh:property _:property.\n_:property sh:path <urn:mara:query:match>; sh:minCount 1.\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("scope-two.ttl"),
        "@prefix sh: <http://www.w3.org/ns/shacl#>.\n<urn:fixture:shape:shared> a sh:NodeShape; sh:property _:property.\n_:property sh:path <urn:mara:query:match>; sh:maxCount 0.\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("scoped-gate.yaml"),
        r#"format_version: 1
gates:
  - id: scoped-blank-nodes
    empty_roots: not_applicable
    empty_required: not_applicable
    conditions:
      - id: require-one-match
        definition: urn:fixture:query:supports
        shape_file: scope-one.ttl
        shape: urn:fixture:shape:shared
        severity: error
        context_parameters: [state]
      - id: allow-zero-matches
        definition: urn:fixture:query:supports
        shape_file: scope-two.ttl
        shape: urn:fixture:shape:shared
        severity: error
        context_parameters: [state]
"#,
    )
    .unwrap();
    let scoped_shapes = runtime
        .query_gate(mara::query_consumers::QueryGateRequest {
            definition_files: vec!["queries.json".into()],
            gate_file: "scoped-gate.yaml".into(),
            shape_files: vec!["scope-one.ttl".into(), "scope-two.ttl".into()],
            gate: "scoped-blank-nodes".into(),
            scope: q::Selection {
                ids: vec!["SUB-LINKED".into()],
                ..Default::default()
            },
            context: vec![mara::named_query::BindingParameter {
                name: "state".into(),
                value: mara::named_query::ParameterValue::Text("accepted".into()),
            }],
            fact_snapshots: vec![],
            mappings: vec![],
            max_rows: 100,
        })
        .unwrap();
    assert_eq!(
        scoped_shapes.roots[0].conditions[0].state,
        mara::query_consumers::ConsumerState::Passed
    );
    assert_eq!(
        scoped_shapes.roots[0].conditions[1].state,
        mara::query_consumers::ConsumerState::Failed
    );

    let inactive_gate = runtime
        .query_gate(mara::query_consumers::QueryGateRequest {
            definition_files: vec!["queries.json".into()],
            gate_file: "gate.yaml".into(),
            shape_files: vec!["presence.ttl".into()],
            gate: "require-support".into(),
            scope: q::Selection {
                ids: vec!["SUP-A".into()],
                ..Default::default()
            },
            context: vec![],
            fact_snapshots: vec![],
            mappings: vec![],
            max_rows: 100,
        })
        .unwrap();
    assert_eq!(
        inactive_gate.roots[0].conditions[0].applicability,
        mara::query_consumers::ConsumerState::NotApplicable
    );
    assert_eq!(
        inactive_gate.roots[0].conditions[0].state,
        mara::query_consumers::ConsumerState::NotApplicable
    );
    assert_eq!(
        inactive_gate.state,
        mara::query_consumers::ConsumerState::NotApplicable
    );

    let gate_only = project();
    install_query_consumers(gate_only.path());
    let config = gate_only.path().join(".mara/project.toml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(&config, text.split("\n[query_policy]\n").next().unwrap()).unwrap();
    let gate_only_runtime =
        NativeRuntime::new(Some(gate_only.path().into()), gate_only.path().into()).unwrap();
    assert!(gate_only_runtime.project_validate(&[]).unwrap().valid);
    let gate_only_failure = gate_only_runtime.query_gate(gate.clone()).unwrap();
    assert_eq!(
        gate_only_failure.state,
        mara::query_consumers::ConsumerState::Failed
    );
    assert!(
        gate_only_runtime.project_validate(&[]).unwrap().valid,
        "gate evaluation must not alter documentation validity"
    );

    let empty_required = runtime
        .query_gate(mara::query_consumers::QueryGateRequest {
            definition_files: vec![],
            gate_file: "gate.yaml".into(),
            shape_files: vec![],
            gate: "empty-required".into(),
            scope: request.selection.clone(),
            context: vec![],
            fact_snapshots: vec![],
            mappings: vec![],
            max_rows: 100,
        })
        .unwrap();
    assert_eq!(
        empty_required.state,
        mara::query_consumers::ConsumerState::Failed
    );
    assert_eq!(
        empty_required.empty_reason.as_deref(),
        Some("required_conditions")
    );
    assert_eq!(
        empty_required.roots[0].state,
        mara::query_consumers::ConsumerState::Failed
    );
    let empty_roots = runtime
        .query_gate(mara::query_consumers::QueryGateRequest {
            definition_files: vec![],
            gate_file: "gate.yaml".into(),
            shape_files: vec![],
            gate: "empty-roots".into(),
            scope: q::Selection {
                fields: vec![q::FieldSelection {
                    key: "status".into(),
                    value: "no-match".into(),
                }],
                ..Default::default()
            },
            context: vec![],
            fact_snapshots: vec![],
            mappings: vec![],
            max_rows: 100,
        })
        .unwrap();
    assert!(empty_roots.roots.is_empty());
    assert_eq!(
        empty_roots.state,
        mara::query_consumers::ConsumerState::NotApplicable
    );
    assert_eq!(empty_roots.empty_reason.as_deref(), Some("roots"));

    let unavailable_gate = r#"  - id: retain-known-failure
    empty_roots: passed
    empty_required: passed
    conditions:
      - id: unavailable-code-condition
        definition: urn:fixture:query:needs-code
        shape_file: presence.ttl
        shape: urn:fixture:shape:has-match
        severity: warning
      - id: known-failure
        definition: urn:fixture:query:supports
        shape_file: presence.ttl
        shape: urn:fixture:shape:has-match
        severity: error
        context_parameters: [state]
"#;
    let mut definitions: Value =
        serde_json::from_slice(&fs::read(fixture.path().join("queries.json")).unwrap()).unwrap();
    let mut needs_code = definitions["definitions"][0].clone();
    needs_code["id"] = json!("urn:fixture:query:needs-code");
    needs_code["required_fact_sources"] = json!(["documents", "code"]);
    needs_code["parameters"] = json!({});
    needs_code["columns"] = json!({"title":{"type":"text"}});
    needs_code["source_columns"] = json!([]);
    needs_code["support_columns"] = json!([]);
    needs_code["select"] =
        json!("SELECT ?root ?title WHERE { ?root <urn:mara:item:title> ?title }");
    definitions["definitions"]
        .as_array_mut()
        .unwrap()
        .push(needs_code);
    let definitions = serde_json::to_string(&definitions).unwrap();
    fs::write(fixture.path().join("queries.json"), &definitions).unwrap();
    let gate_file = fixture.path().join("gate.yaml");
    let mut gate_text = fs::read_to_string(&gate_file).unwrap();
    gate_text.push_str(unavailable_gate);
    fs::write(&gate_file, &gate_text).unwrap();
    // Native absence is now known; use an actual unavailable indexer to retain this case.
    fs::write(
        fixture.path().join("unavailable.rs"),
        "pub fn unavailable() {}",
    )
    .unwrap();
    let config = fixture.path().join(".mara/project.toml");
    let original_config = fs::read_to_string(&config).unwrap();
    let (_, rest) = original_config.split_once('\n').unwrap();
    let config_text = format!("format_version = 3\n{rest}");
    fs::write(&config, format!("{config_text}\n[[code.languages]]\nname = \"rust\"\nextensions = [\"rs\"]\ncommand = [\"mara-test-missing-indexer\", \"{{output}}\"]\n")).unwrap();

    let mut incomplete_snapshot = consumer_snapshot();
    let query_source = incomplete_snapshot
        .files
        .iter_mut()
        .find(|file| file.path == "queries.json")
        .unwrap();
    query_source.content = definitions;
    let gate_source = incomplete_snapshot
        .files
        .iter_mut()
        .find(|file| file.path == "gate.yaml")
        .unwrap();
    gate_source.content.push_str(unavailable_gate);
    let incomplete_engine = SourceEngine::new(incomplete_snapshot).unwrap();
    let retained = runtime
        .query_gate(mara::query_consumers::QueryGateRequest {
            definition_files: vec!["queries.json".into()],
            gate_file: "gate.yaml".into(),
            shape_files: vec!["presence.ttl".into()],
            gate: "retain-known-failure".into(),
            scope: request.selection,
            context: vec![mara::named_query::BindingParameter {
                name: "state".into(),
                value: mara::named_query::ParameterValue::Text("accepted".into()),
            }],
            fact_snapshots: vec![],
            mappings: vec![],
            max_rows: 100,
        })
        .unwrap();
    let portable_retained = incomplete_engine
        .query_gate(mara::query_consumers::QueryGateRequest {
            definition_files: vec!["queries.json".into()],
            gate_file: "gate.yaml".into(),
            shape_files: vec!["presence.ttl".into()],
            gate: "retain-known-failure".into(),
            scope: q::Selection {
                ids: vec!["SUB-EMPTY".into()],
                ..Default::default()
            },
            context: vec![q::BindingParameter {
                name: "state".into(),
                value: q::ParameterValue::Text("accepted".into()),
            }],
            fact_snapshots: vec![],
            mappings: vec![],
            max_rows: 100,
        })
        .unwrap();
    assert_eq!(
        retained.state,
        mara::query_consumers::ConsumerState::Unavailable
    );
    assert!(!retained.evaluation_complete);
    assert_eq!(
        retained.roots[0].state,
        mara::query_consumers::ConsumerState::Unavailable
    );
    assert_eq!(
        retained.roots[0].conditions[0].state,
        mara::query_consumers::ConsumerState::Unavailable
    );
    assert_eq!(
        retained.roots[0].conditions[0].applicability,
        mara::query_consumers::ConsumerState::Unavailable
    );
    assert_eq!(
        retained.roots[0].conditions[1].state,
        mara::query_consumers::ConsumerState::Failed
    );
    assert_eq!(
        retained.roots[0].conditions[1].severity,
        mara::Severity::Error
    );
    assert_eq!(
        portable_retained.roots[0].conditions[1].state,
        mara::query_consumers::ConsumerState::Failed
    );
    let empty_scope_missing_code = runtime
        .query_gate(mara::query_consumers::QueryGateRequest {
            definition_files: vec!["queries.json".into()],
            gate_file: "gate.yaml".into(),
            shape_files: vec!["presence.ttl".into()],
            gate: "retain-known-failure".into(),
            scope: q::Selection {
                fields: vec![q::FieldSelection {
                    key: "status".into(),
                    value: "no-match".into(),
                }],
                ..Default::default()
            },
            context: vec![],
            fact_snapshots: vec![],
            mappings: vec![],
            max_rows: 100,
        })
        .unwrap();
    assert_eq!(
        empty_scope_missing_code.state,
        mara::query_consumers::ConsumerState::Unavailable
    );
    assert!(!empty_scope_missing_code.evaluation_complete);
    assert!(empty_scope_missing_code.roots.is_empty());
    assert!(empty_scope_missing_code.empty_reason.is_none());
    fs::write(config, original_config).unwrap();
    fs::remove_file(fixture.path().join("unavailable.rs")).unwrap();

    let schema_path = fixture.path().join(".mara/schema.yaml");
    let mut schema: Value = serde_json::from_slice(&fs::read(&schema_path).unwrap()).unwrap();
    schema["relations"]["parent"]["acyclic"] = json!({"severity":"error"});
    fs::write(&schema_path, serde_json::to_vec(&schema).unwrap()).unwrap();
    let structural_validation = runtime.project_validate(&[]).unwrap();
    assert!(structural_validation.evaluation_complete);
    assert!(
        structural_validation
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == mara::DiagnosticCode::RelationCycle })
    );
    assert!(structural_validation.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == mara::DiagnosticCode::RuleFailed
            && diagnostic.rule.as_deref() == Some("require-query-match")
    }));
}

// @mara checks REQ-QUERY-POLICY-CONSUMERS
#[test]
fn configured_query_policy_paths_are_normalized() {
    let fixture = project();
    install_query_consumers(fixture.path());
    let config_path = fixture.path().join(".mara/project.toml");
    let config = fs::read_to_string(&config_path)
        .unwrap()
        .replace(
            "files = [\"invariants.yaml\"]",
            "files = [\"./invariants.yaml\"]",
        )
        .replace(
            "shape_files = [\"presence.ttl\"]",
            "shape_files = [\"./presence.ttl\"]",
        );
    fs::write(&config_path, config).unwrap();
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();

    let schema_validation = runtime.schema_validate().unwrap();
    assert!(schema_validation.valid && schema_validation.evaluation_complete);
    let validation = runtime.project_validate(&[]).unwrap();
    assert!(!validation.valid && validation.evaluation_complete);
    assert!(
        validation
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.rule.as_deref() == Some("require-query-match") })
    );
}

// @mara checks REQ-QUERY-POLICY-CONSUMERS
// @mara checks REQ-NAMED-QUERY-FACTS
// @mara checks DES-EXECUTION-BOUNDARIES
// @mara implements VER-REUSABLE-QUERY-ENGINE
#[test]
fn real_portable_rust_embedded_rust_cli_mcp_fixture_and_paged_results() {
    let fixture = project();
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let portable = SourceEngine::new(snapshot()).unwrap();
    let mut req = request();
    let mut records = vec![];
    loop {
        let rust = runtime.query_execute(req.clone()).unwrap();
        assert_eq!(rust.format_version, 3);
        let engine = portable.query_execute(req.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(&rust).unwrap(),
            serde_json::to_value(&engine).unwrap()
        );
        let output = cli(fixture.path(), &req);
        assert!(
            output.status.success(),
            "{} {}",
            stdout(&output),
            stderr(&output)
        );
        let cli: Value = serde_json::from_slice(&output.stdout).unwrap();
        let mcp = mcp(fixture.path(), &req);
        assert_eq!(cli, serde_json::to_value(&rust).unwrap());
        assert_eq!(mcp, cli);
        assert!(rust.evaluation_complete);
        assert_eq!(rust.summary.binding_count, Some(3));
        records.extend(rust.records);
        req.cursor = rust.next_cursor;
        if !rust.has_more {
            break;
        }
    }
    assert_eq!(records.len(), 5);
    // Retain real fixture/output artifacts when requested by the acceptance run.
    if let Ok(path) = std::env::var("MARA_NAMED_QUERY_REPORT") {
        fs::write(
            path,
            serde_json::to_vec_pretty(&json!({"records":records,"snapshot":snapshot()})).unwrap(),
        )
        .unwrap();
    }
}
// @mara checks REQ-COMPOSABLE-QUERY-PROJECTIONS
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
#[test]
fn named_query_composition_matches_portable_embedded_cli_and_mcp_results() {
    let fixture = project();
    let query_file = include_str!("fixtures/named-query/composition.json");
    fs::write(fixture.path().join("composition.json"), query_file).unwrap();
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let mut source = snapshot();
    source.files.retain(|file| file.path != "queries.json");
    source.files.push(mara::SourceFile {
        path: "composition.json".into(),
        content: query_file.into(),
    });
    let portable = SourceEngine::new(source).unwrap();
    let req = q::Request {
        definition_files: vec!["composition.json".into()],
        definition: "urn:fixture:query:composed-support-status".into(),
        selection: q::Selection {
            ids: vec!["SUB-LINKED".into(), "SUB-EMPTY".into()],
            ..Default::default()
        },
        parameters: vec![],
        max_rows: 100,
        limit: 100,
        cursor: None,
        fact_snapshots: vec![],
        mappings: vec![],
    };
    let native = runtime.query_execute(req.clone()).unwrap();
    let engine = portable.query_execute(req.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&native).unwrap(),
        serde_json::to_value(&engine).unwrap()
    );
    assert!(native.evaluation_complete);
    assert_eq!(native.summary.binding_count, Some(1));
    let cli = composition_cli(fixture.path(), &req);
    assert!(cli.status.success(), "{} {}", stdout(&cli), stderr(&cli));
    assert_eq!(
        serde_json::from_slice::<Value>(&cli.stdout).unwrap(),
        serde_json::to_value(&native).unwrap()
    );
    assert_eq!(
        mcp(fixture.path(), &req),
        serde_json::to_value(&native).unwrap()
    );
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn real_native_content_changes_reconcile_and_cli_mcp_reject_stale_cursor() {
    let fixture = project();
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    for file in ["queries.json", "fixture.mara.md", ".mara/schema.yaml"] {
        let mut req = request();
        req.cursor = runtime.query_execute(req.clone()).unwrap().next_cursor;
        let path = fixture.path().join(file);
        let original = fs::read_to_string(&path).unwrap();
        fs::write(&path, format!("{original}\n")).unwrap();
        assert_eq!(
            runtime.query_execute(req.clone()).unwrap_err().code,
            "invalid_argument"
        );
        let output = cli(fixture.path(), &req);
        assert!(!output.status.success());
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["error"]["code"], "invalid_argument");
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "query_execute", serde_json::to_value(&req).unwrap()),
            ],
        );
        let response = mcp_response(&responses, 2);
        assert_eq!(response["result"]["isError"], true);
        assert!(response.to_string().contains("invalid_argument"));
        fs::write(&path, original).unwrap();
    }
    let mut req = request();
    req.cursor = runtime.query_execute(req.clone()).unwrap().next_cursor;
    req.parameters[0].value = q::ParameterValue::Text("draft".into());
    assert!(runtime.query_execute(req.clone()).is_err());
    assert!(!cli(fixture.path(), &req).status.success());
    req.cursor = None;
    req.limit = 100;
    let updated = runtime.query_execute(req).unwrap();
    assert_eq!(updated.summary.binding_count, Some(1));
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn embedded_document_queries_ignore_unavailable_code_assertions() {
    let fixture = project();
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let mut schema: Value =
        serde_json::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["relations"]["code_support"] = json!({"description":"Code support","source":[],"target":["subject"],"code_source":true,"inverse":"code_supported_by"});
    fs::write(schema_path, schema.to_string()).unwrap();
    let document = fixture.path().join("fixture.mara.md");
    let content = fs::read_to_string(&document).unwrap();
    fs::write(
        &document,
        content.replacen(
            ":status: draft\n",
            ":status: draft\n:code_supported_by: code:existing.txt\n",
            1,
        ),
    )
    .unwrap();
    let definition = json!({"definitions":[{"id":"document","required_fact_sources":["documents"],"root_flavours":["subject"],"columns":{"title":{"type":"text"}},"select":"SELECT ?root ?title WHERE { ?root <urn:mara:item:title> ?title }"}]});
    write_queries(fixture.path(), definition.clone());
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let mut req = request();
    req.definition = "document".into();
    req.selection.ids = vec!["SUB-LINKED".into()];
    req.parameters.clear();
    req.limit = 100;
    let rust = runtime.query_execute(req.clone()).unwrap();
    assert!(rust.evaluation_complete, "{rust:#?}");
    assert_eq!(rust.summary.binding_count, Some(1));
    let cli = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "document",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-LINKED",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(cli.status.success(), "{}", stderr(&cli));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&cli)).unwrap(),
        serde_json::to_value(&rust).unwrap()
    );
    assert_eq!(
        mcp(fixture.path(), &req),
        serde_json::to_value(&rust).unwrap()
    );
    let mut definitions = definition;
    definitions["definitions"].as_array_mut().unwrap().push(json!({"id":"wrong","required_fact_sources":["documents"],"root_flavours":["subject"],"columns":{"value":{"type":"item","flavours":["subject"]}},"select":"SELECT ?root ?value WHERE { ?root <urn:mara:relation:missing> ?value }"}));
    write_queries(fixture.path(), definitions);
    assert_eq!(
        runtime.query_execute(req.clone()).unwrap_err().code,
        "invalid_definition"
    );
    let cli = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "document",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-LINKED",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(!cli.status.success());
    let responses = mcp_exchange_with_arguments(
        fixture.path(),
        &["--execution-mode", "direct", "mcp"],
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "query_execute", serde_json::to_value(&req).unwrap()),
        ],
    );
    let response = mcp_response(&responses, 2);
    assert_eq!(response["result"]["isError"], true);
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn embedded_required_code_source_is_unavailable_even_for_empty_roots() {
    let fixture = project();
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let mut schema: Value =
        serde_json::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["relations"]["code_support"] = json!({"description":"Code support","source":[],"target":["subject"],"code_source":true,"inverse":"code_supported_by"});
    fs::write(schema_path, schema.to_string()).unwrap();
    let document = fixture.path().join("fixture.mara.md");
    let content = fs::read_to_string(&document).unwrap();
    fs::write(
        &document,
        content.replacen(
            ":status: draft\n",
            ":status: draft\n:code_supported_by: code:existing.txt\n",
            1,
        ),
    )
    .unwrap();
    write_queries(
        fixture.path(),
        json!({"definitions":[{"id":"code-required","required_fact_sources":["documents","code"],"root_flavours":["subject"],"columns":{"title":{"type":"text"}},"select":"SELECT ?root ?title WHERE { ?root <urn:mara:item:title> ?title }"}]}),
    );
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let mut req = request();
    req.definition = "code-required".into();
    req.selection.ids = vec!["SUB-EMPTY".into()];
    req.selection.fields = vec![q::FieldSelection {
        key: "status".into(),
        value: "never".into(),
    }];
    req.parameters.clear();
    req.limit = 100;
    let rust = runtime.query_execute(req.clone()).unwrap();
    assert!(!rust.evaluation_complete, "{rust:#?}");
    assert_eq!(rust.summary.binding_count, None);
    assert!(rust.records.is_empty());
    let cli = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "code-required",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-EMPTY",
            "--field",
            "status=never",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(
        !cli.status.success(),
        "incomplete CLI result must be unsuccessful"
    );
    let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(cli, serde_json::to_value(&rust).unwrap());
    let responses = mcp_exchange_with_arguments(
        fixture.path(),
        &["--execution-mode", "direct", "mcp"],
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "query_execute", serde_json::to_value(&req).unwrap()),
        ],
    );
    let response = mcp_response(&responses, 2);
    assert_ne!(response["result"]["isError"], true, "{response:#}");
    assert_eq!(response["result"]["structuredContent"], cli, "{response:#}");
}

// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn embedded_unknown_schema_predicates_fail_before_empty_results() {
    let fixture = project();
    write_queries(
        fixture.path(),
        json!({"definitions":[{"id":"unknown-field","root_flavours":["subject"],"columns":{"value":{"type":"text"}},"select":"SELECT ?root ?value WHERE { ?root <urn:mara:field:statuz> ?value }"}]}),
    );
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let mut req = request();
    req.definition = "unknown-field".into();
    req.selection = q::Selection {
        flavours: vec!["subject".into()],
        fields: vec![q::FieldSelection {
            key: "status".into(),
            value: "never".into(),
        }],
        ..Default::default()
    };
    req.parameters.clear();
    let error = runtime.query_execute(req.clone()).unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    let cli = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "unknown-field",
            "--definition-file",
            "queries.json",
            "--flavour",
            "subject",
            "--field",
            "status=never",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(!cli.status.success());
    let cli_error: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(cli_error["error"]["code"], "invalid_definition");
    let responses = mcp_exchange_with_arguments(
        fixture.path(),
        &["--execution-mode", "direct", "mcp"],
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "query_execute", serde_json::to_value(&req).unwrap()),
        ],
    );
    assert_eq!(mcp_response(&responses, 2)["result"]["isError"], true);
}

// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn embedded_multiple_relation_parameters_are_bound_as_one_analysis_row() {
    let fixture = project();
    write_queries(
        fixture.path(),
        json!({"definitions":[{"id":"two-params","root_flavours":["subject"],"parameters":{"first":{"type":"iri","required":true},"second":{"type":"iri","required":true}},"columns":{"title":{"type":"text"}},"select":"SELECT ?root ?title WHERE { ?root <urn:mara:item:title> ?title . FILTER(!sameTerm(?first,?second)) }"}]}),
    );
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let mut req = request();
    req.definition = "two-params".into();
    req.selection.ids = vec!["SUB-LINKED".into()];
    req.parameters = vec![
        q::BindingParameter {
            name: "first".into(),
            value: q::ParameterValue::Iri("urn:mara:relation:supports".into()),
        },
        q::BindingParameter {
            name: "second".into(),
            value: q::ParameterValue::Iri("urn:mara:relation:parent".into()),
        },
    ];
    req.limit = 100;
    let rust = runtime.query_execute(req.clone()).unwrap();
    assert!(rust.evaluation_complete, "{rust:#?}");
    assert_eq!(rust.summary.binding_count, Some(1));
    let cli = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "two-params",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-LINKED",
            "--param",
            "first={\"type\":\"iri\",\"value\":\"urn:mara:relation:supports\"}",
            "--param",
            "second={\"type\":\"iri\",\"value\":\"urn:mara:relation:parent\"}",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(cli.status.success(), "{}", stderr(&cli));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&cli)).unwrap(),
        serde_json::to_value(&rust).unwrap()
    );
    assert_eq!(
        mcp(fixture.path(), &req),
        serde_json::to_value(&rust).unwrap()
    );
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn embedded_bound_relation_parameters_and_empty_code_selection_are_complete() {
    let fixture = project();
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let mut schema: Value =
        serde_json::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["relations"]["code_support"] = json!({"description":"Code support","source":[],"target":["subject"],"code_source":true,"inverse":"code_supported_by"});
    fs::write(schema_path, schema.to_string()).unwrap();
    let queries = json!({"definitions":[
        {"id":"parameter-relation","root_flavours":["subject"],"parameters":{"wanted":{"type":"iri","required":true}},"columns":{"title":{"type":"text"}},"select":"SELECT ?root ?title WHERE { ?edge <urn:mara:query:source> ?source ; <urn:mara:query:target> ?root ; <urn:mara:query:relation> ?kind . ?root <urn:mara:item:title> ?title . FILTER(sameTerm(?kind,?wanted)) }"},
        {"id":"empty-code","root_flavours":["subject"],"columns":{"title":{"type":"text"}},"select":"SELECT ?root ?title WHERE { ?root ^<urn:mara:relation:code_support> ?code . ?root <urn:mara:item:title> ?title }"}
    ]});
    write_queries(fixture.path(), queries);
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let mut req = request();
    req.definition = "parameter-relation".into();
    req.selection.ids = vec!["SUB-LINKED".into()];
    req.parameters = vec![q::BindingParameter {
        name: "wanted".into(),
        value: q::ParameterValue::Iri("urn:mara:relation:supports".into()),
    }];
    req.limit = 100;
    let rust = runtime.query_execute(req.clone()).unwrap();
    assert!(rust.evaluation_complete, "{rust:#?}");
    assert_eq!(rust.summary.binding_count, Some(1));
    let b = rust
        .records
        .iter()
        .find_map(|r| match r {
            q::Record::Binding { binding, .. } => Some(binding),
            _ => None,
        })
        .unwrap();
    assert!(b.support.is_empty());
    assert!(b.contributors.is_empty());
    let cli = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "parameter-relation",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-LINKED",
            "--param",
            "wanted={\"type\":\"iri\",\"value\":\"urn:mara:relation:supports\"}",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(cli.status.success(), "{}", stderr(&cli));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&cli)).unwrap(),
        serde_json::to_value(&rust).unwrap()
    );
    assert_eq!(
        mcp(fixture.path(), &req),
        serde_json::to_value(&rust).unwrap()
    );
    req.definition = "empty-code".into();
    req.parameters.clear();
    req.selection.ids.clear();
    req.selection.fields = vec![q::FieldSelection {
        key: "status".into(),
        value: "never".into(),
    }];
    let empty = runtime.query_execute(req.clone()).unwrap();
    assert!(empty.evaluation_complete, "{empty:#?}");
    assert_eq!(empty.summary.selected_roots, 0);
    assert_eq!(empty.summary.binding_count, Some(0));
    assert!(empty.records.is_empty());
    let cli = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "empty-code",
            "--definition-file",
            "queries.json",
            "--field",
            "status=never",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(cli.status.success(), "{}", stderr(&cli));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&cli)).unwrap(),
        serde_json::to_value(&empty).unwrap()
    );
    assert_eq!(
        mcp(fixture.path(), &req),
        serde_json::to_value(&empty).unwrap()
    );
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn embedded_id_constants_bind_the_canonical_mid_item() {
    let fixture = project();
    write_queries(
        fixture.path(),
        json!({"definitions":[{"id":"id-constant","root_flavours":["subject"],"columns":{"support":{"type":"item","flavours":["support"]}},"select":"SELECT ?root ?support WHERE { ?root <urn:mara:relation:supported_by> <urn:mara:id:SUP-A> . BIND(<urn:mara:id:SUP-A> AS ?support) }"}]}),
    );
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let mut req = request();
    req.definition = "id-constant".into();
    req.selection.ids = vec!["SUB-LINKED".into()];
    req.parameters.clear();
    req.limit = 100;
    let rust = runtime.query_execute(req.clone()).unwrap();
    assert!(rust.evaluation_complete);
    assert_eq!(rust.summary.binding_count, Some(1));
    let b = rust
        .records
        .iter()
        .find_map(|r| match r {
            q::Record::Binding { binding, .. } => Some(binding),
            _ => None,
        })
        .unwrap();
    assert!(b.support.is_empty());
    assert!(
        matches!(&b.values["support"],Some(q::Value::Item{endpoint:q::Endpoint::Item{id,mid:Some(mid)},..}) if id=="SUP-A" && mid=="01ARZ3NDEKTSV4RRFFQ69G5F02")
    );
    let output = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "id-constant",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-LINKED",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&output)).unwrap(),
        serde_json::to_value(&rust).unwrap()
    );
    assert_eq!(
        mcp(fixture.path(), &req),
        serde_json::to_value(&rust).unwrap()
    );
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn embedded_optional_absence_is_preserved_without_hidden_support() {
    let fixture = project();
    write_queries(
        fixture.path(),
        json!({"definitions":[{"id":"wanted","root_flavours":["subject"],"parameters":{"wanted":{"type":"item","required":true}},"columns":{"support":{"type":"item","flavours":["support"],"optional":true}},"select":"SELECT ?root ?support WHERE { ?root <urn:mara:item:title> ?title . OPTIONAL { ?root <urn:mara:relation:supported_by> ?support FILTER(sameTerm(?support,?wanted)) } }"}]}),
    );
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let mut req = request();
    req.definition = "wanted".into();
    req.selection.ids = vec!["SUB-EMPTY".into()];
    req.limit = 100;
    req.parameters = vec![q::BindingParameter {
        name: "wanted".into(),
        value: q::ParameterValue::Item("SUP-A".into()),
    }];
    let rust = runtime.query_execute(req.clone()).unwrap();
    assert!(rust.evaluation_complete);
    assert_eq!(rust.summary.binding_count, Some(1));
    assert_eq!(rust.records.len(), 2);
    let binding = rust
        .records
        .iter()
        .find_map(|record| match record {
            q::Record::Binding { binding, .. } => Some(binding),
            _ => None,
        })
        .unwrap();
    assert!(binding.values["support"].is_none());
    assert!(binding.support.is_empty());
    assert!(binding.contributors.is_empty());
    let output = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "wanted",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-EMPTY",
            "--param",
            "wanted={\"type\":\"item\",\"value\":\"SUP-A\"}",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&output)).unwrap(),
        serde_json::to_value(&rust).unwrap()
    );
    assert_eq!(
        mcp(fixture.path(), &req),
        serde_json::to_value(&rust).unwrap()
    );
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn embedded_constant_edges_retain_authored_support() {
    use sha2::{Digest, Sha256};
    let fixture = project();
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let mut initial = request();
    initial.limit = 100;
    let base = runtime.query_execute(initial).unwrap();
    let edge = base
        .records
        .iter()
        .find_map(|r| match r {
            q::Record::Binding { binding, .. } => binding
                .support
                .iter()
                .find(|s| matches!(&s.edge.source,q::Endpoint::Item{id,..} if id=="SUP-A"))
                .map(|s| s.edge.clone()),
            _ => None,
        })
        .unwrap();
    let iri = format!(
        "urn:mara:query:edge:{:x}",
        Sha256::digest(serde_json::to_vec(&edge).unwrap())
    );
    write_queries(
        fixture.path(),
        json!({"definitions":[{"id":"constant-edge","root_flavours":["subject"],"columns":{"title":{"type":"text"}},"select":format!("SELECT ?root ?title ?edge WHERE {{ ?edge <urn:mara:query:target> ?root ; <urn:mara:query:relation> <urn:mara:relation:supports> . FILTER(sameTerm(?edge, <{iri}>)) . ?root <urn:mara:item:title> ?title }}") ,"support_columns":["edge"]}]}),
    );
    let mut req = request();
    req.definition = "constant-edge".into();
    req.parameters.clear();
    req.limit = 100;
    let rust = runtime.query_execute(req.clone()).unwrap();
    assert!(rust.evaluation_complete);
    assert_eq!(rust.summary.binding_count, Some(1));
    let b = rust
        .records
        .iter()
        .find_map(|r| match r {
            q::Record::Binding { binding, .. } => Some(binding),
            _ => None,
        })
        .unwrap();
    assert_eq!(b.support.len(), 1);
    assert_eq!(b.support[0].edge, edge);
    assert_eq!(b.support[0].occurrences.len(), 3);
    let output = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "constant-edge",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-LINKED",
            "--id",
            "SUB-EMPTY",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&output)).unwrap(),
        serde_json::to_value(&rust).unwrap()
    );
    assert_eq!(
        mcp(fixture.path(), &req),
        serde_json::to_value(&rust).unwrap()
    );
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn embedded_sparql_queries_do_not_gain_hidden_support() {
    let fixture = project();
    let path = fixture.path().join(".mara/schema.yaml");
    let mut schema: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    schema["relations"]["code_support"] = json!({"description":"Code support","source":[],"target":["subject"],"code_source":true,"inverse":"code_supported_by"});
    fs::write(path, schema.to_string()).unwrap();
    write_queries(
        fixture.path(),
        json!({"definitions":[
            {"id":"expression","root_flavours":["subject"],"columns":{"value":{"type":"boolean"}},"select":"SELECT ?root ?value WHERE { BIND(EXISTS { ?root <urn:mara:relation:supported_by> ?s } AS ?value) }"},
            {"id":"edges","root_flavours":["subject"],"columns":{"title":{"type":"text"}},"select":"SELECT ?root ?title WHERE { ?edge <urn:mara:query:source> ?source ; <urn:mara:query:target> ?root ; <urn:mara:query:relation> <urn:mara:relation:supports> . ?root <urn:mara:item:title> ?title }"},
            {"id":"edge-filter","root_flavours":["subject"],"columns":{"title":{"type":"text"}},"select":"SELECT ?root ?title WHERE { ?edge <urn:mara:query:source> ?source ; <urn:mara:query:target> ?root ; <urn:mara:query:relation> ?kind . ?root <urn:mara:item:title> ?title . FILTER(sameTerm(?kind,<urn:mara:relation:supports>)) }"},
            {"id":"edge-values","root_flavours":["subject"],"columns":{"title":{"type":"text"}},"select":"SELECT ?root ?title WHERE { ?edge <urn:mara:query:source> ?source ; <urn:mara:query:target> ?root ; <urn:mara:query:relation> ?kind . ?root <urn:mara:item:title> ?title . VALUES ?kind { <urn:mara:relation:supports> } }"},
            {"id":"edge-bind","root_flavours":["subject"],"columns":{"title":{"type":"text"}},"select":"SELECT ?root ?title WHERE { BIND(<urn:mara:relation:supports> AS ?kind) ?edge <urn:mara:query:source> ?source ; <urn:mara:query:target> ?root ; <urn:mara:query:relation> ?kind . ?root <urn:mara:item:title> ?title }"},
            {"id":"edge-alias","root_flavours":["subject"],"columns":{"title":{"type":"text"}},"select":"SELECT ?root ?title WHERE { ?edge <urn:mara:query:source> ?source ; <urn:mara:query:target> ?root ; <urn:mara:query:relation> ?kind . BIND(?kind AS ?alias) ?root <urn:mara:item:title> ?title . FILTER(sameTerm(?alias,<urn:mara:relation:supports>)) }"},
            {"id":"twohop","root_flavours":["subject"],"columns":{"ancestor":{"type":"item","flavours":["subject"]}},"select":"SELECT ?root ?ancestor WHERE { ?root <urn:mara:relation:parent>/<urn:mara:relation:parent> ?ancestor . FILTER(sameTerm(?ancestor,<urn:mara:mid:01ARZ3NDEKTSV4RRFFQ69G5F07>)) }"},
            {"id":"mixed","root_flavours":["subject"],"columns":{"ancestor":{"type":"item","flavours":["subject"]}},"select":"SELECT ?root ?ancestor WHERE { ?root <urn:mara:relation:parent>/<urn:mara:relation:parent>+ ?ancestor . FILTER(sameTerm(?ancestor,<urn:mara:mid:01ARZ3NDEKTSV4RRFFQ69G5F07>)) }"}
        ]}),
    );
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    for (definition, id) in [
        ("expression", "SUB-EMPTY"),
        ("edges", "SUB-LINKED"),
        ("edge-filter", "SUB-LINKED"),
        ("edge-values", "SUB-LINKED"),
        ("edge-bind", "SUB-LINKED"),
        ("edge-alias", "SUB-LINKED"),
        ("twohop", "SUB-LINKED"),
        ("mixed", "SUB-LINKED"),
    ] {
        let mut req = request();
        req.definition = definition.into();
        req.selection.ids = vec![id.into()];
        req.parameters.clear();
        req.limit = 100;
        let rust = runtime.query_execute(req.clone()).unwrap();
        assert!(rust.evaluation_complete, "{rust:#?}");
        assert_eq!(rust.summary.binding_count, Some(1));
        let binding = rust
            .records
            .iter()
            .find_map(|r| match r {
                q::Record::Binding { binding, .. } => Some(binding),
                _ => None,
            })
            .unwrap();
        assert!(binding.support.is_empty());
        if definition == "expression" {
            assert!(matches!(
                binding.values["value"],
                Some(q::Value::Boolean { value: false })
            ));
        }
        let output = mara(
            fixture.path(),
            &[
                "--execution-mode",
                "direct",
                "--format",
                "json",
                "query",
                "execute",
                definition,
                "--definition-file",
                "queries.json",
                "--id",
                id,
                "--max-rows",
                "100",
                "--limit",
                "100",
            ],
        );
        assert!(
            output.status.success(),
            "{} {}",
            stdout(&output),
            stderr(&output)
        );
        let cli: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(cli, serde_json::to_value(rust).unwrap());
        assert_eq!(mcp(fixture.path(), &req), cli);
    }
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn embedded_negative_parameter_and_constant_expression_bindings() {
    let fixture = project();
    write_queries(
        fixture.path(),
        json!({"definitions":[
            {"id":"parameter","root_flavours":["subject"],"columns":{"value":{"type":"text"}},"parameters":{"state":{"type":"text","required":true}},"select":"SELECT ?root ?value WHERE { FILTER(BOUND(?state)) BIND(?state AS ?value) }"},
            {"id":"root","root_flavours":["subject"],"columns":{"value":{"type":"item"}},"select":"SELECT ?root ?value WHERE { BIND(?root AS ?value) }"},
            {"id":"constant","root_flavours":["subject"],"columns":{"value":{"type":"boolean"}},"select":"SELECT ?root ?value WHERE { BIND(EXISTS { VALUES ?w { \"x\" } } AS ?value) }"},
            {"id":"unbound","root_flavours":["subject"],"columns":{"value":{"type":"boolean"}},"select":"SELECT ?root ?value WHERE { BIND(?missing AS ?alias) BIND(BOUND(?alias) AS ?value) }"},
            {"id":"negative","root_flavours":["subject"],"columns":{"support":{"type":"item","flavours":["support"],"optional":true}},"select":"SELECT ?root ?support WHERE { OPTIONAL { ?root <urn:mara:relation:supported_by> ?support } FILTER(!BOUND(?support)) }"},
            {"id":"minus","root_flavours":["subject"],"columns":{"support":{"type":"item","flavours":["support"],"optional":true}},"select":"SELECT ?root ?support WHERE { MINUS { ?root <urn:mara:relation:supported_by> ?support } }"}
        ]}),
    );
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    for definition in [
        "parameter",
        "root",
        "constant",
        "unbound",
        "negative",
        "minus",
    ] {
        let id = if matches!(definition, "negative" | "minus") {
            "SUB-EMPTY"
        } else {
            "SUB-LINKED"
        };
        let mut req = request();
        req.definition = definition.into();
        req.selection.ids = vec![id.into()];
        req.parameters.clear();
        req.limit = 100;
        if definition == "parameter" {
            req.parameters.push(q::BindingParameter {
                name: "state".into(),
                value: q::ParameterValue::Text("supplied".into()),
            });
        }
        let rust = runtime.query_execute(req.clone()).unwrap();
        assert!(rust.evaluation_complete, "{rust:#?}");
        assert_eq!(rust.summary.binding_count, Some(1));
        let b = rust
            .records
            .iter()
            .find_map(|r| {
                if let q::Record::Binding { binding, .. } = r {
                    Some(binding)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(b.support.is_empty());
        match definition {
            "parameter" => assert!(
                matches!(&b.values["value"],Some(q::Value::Text {value,..}) if value=="supplied")
            ),
            "root" => assert!(
                matches!(&b.values["value"],Some(q::Value::Item {endpoint:q::Endpoint::Item {id,..},..}) if id=="SUB-LINKED")
            ),
            "constant" => assert!(matches!(
                b.values["value"],
                Some(q::Value::Boolean { value: true })
            )),
            "unbound" => assert!(matches!(
                b.values["value"],
                Some(q::Value::Boolean { value: false })
            )),
            "negative" | "minus" => assert!(b.values["support"].is_none()),
            _ => unreachable!(),
        }
        let mut args = vec![
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            definition,
            "--definition-file",
            "queries.json",
            "--id",
            id,
            "--max-rows",
            "100",
            "--limit",
            "100",
        ];
        if definition == "parameter" {
            args.extend(["--param", r#"state={"type":"text","value":"supplied"}"#]);
        }
        let output = mara(fixture.path(), &args);
        assert!(
            output.status.success(),
            "{} {}",
            stdout(&output),
            stderr(&output)
        );
        let cli: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(cli, serde_json::to_value(rust).unwrap());
        assert_eq!(mcp(fixture.path(), &req), cli);
    }
}
// @mara checks DES-QUERY-CONSUMER-CONTRACT
#[test]
fn raw_facts_ignore_failing_documentation_policy() {
    let fixture = project();
    let config = fixture.path().join(".mara/project.toml");
    let source = fs::read_to_string(&config).unwrap();
    fs::write(
        config,
        format!("{source}\n[rules]\nformat_version = 1\nfiles = [\".mara/fail.yaml\"]\n"),
    )
    .unwrap();
    fs::write(fixture.path().join(".mara/fail.yaml"),"id: rule:impossible\ntargetClass: subject\nproperty:\n  - path: status\n    hasValue: impossible\n").unwrap();
    let validation = mara(fixture.path(), &["--format", "json", "project", "validate"]);
    assert!(!validation.status.success());
    let validation: Value = serde_json::from_slice(&validation.stdout).unwrap();
    assert_eq!(validation["evaluation_complete"], true, "{validation:#}");
    assert!(
        validation["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "rule_failed"),
        "{validation:#}"
    );
    let facts = cli(fixture.path(), &request());
    assert!(
        facts.status.success(),
        "{} {}",
        stdout(&facts),
        stderr(&facts)
    );
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn embedded_returned_types_and_exact_item_constants_are_validated() {
    let fixture = project();
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let definitions = json!({"definitions":[
        {"id":"filtered","root_flavours":["subject"],"columns":{"value":{"type":"text"}},"select":"SELECT ?root ?value WHERE { VALUES ?value { \"x\" \"y\" 1 } FILTER(sameTerm(?value, \"x\") || sameTerm(?value, \"y\")) }"}
    ]});
    write_queries(fixture.path(), definitions);
    let mut req = request();
    req.definition = "filtered".into();
    req.selection.ids = vec!["SUB-LINKED".into()];
    req.parameters.clear();
    req.limit = 100;
    let rust = runtime.query_execute(req.clone()).unwrap();
    assert!(rust.evaluation_complete, "{rust:#?}");
    assert_eq!(rust.summary.binding_count, Some(2));
    let cli = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "filtered",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-LINKED",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(cli.status.success(), "{}", stderr(&cli));
    let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(cli, serde_json::to_value(&rust).unwrap());
    assert_eq!(mcp(fixture.path(), &req), cli);
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let mut schema: Value =
        serde_json::from_str(&fs::read_to_string(&schema_path).unwrap()).unwrap();
    schema["relations"]["parent"]["target"] = json!(["subject", "support"]);
    fs::write(&schema_path, schema.to_string()).unwrap();
    let bad = json!({"definitions":[
        {"id":"bad","root_flavours":["subject"],"columns":{"value":{"type":"item","flavours":["support"]}},"select":"SELECT ?root ?value WHERE { ?root <urn:mara:relation:parent> ?value }"},
        {"id":"selected","root_flavours":["subject"],"columns":{"title":{"type":"text"}},"select":"SELECT ?root ?title WHERE { ?root <urn:mara:item:title> ?title }"}
    ]});
    write_queries(fixture.path(), bad);
    req.definition = "bad".into();
    req.selection.ids = vec!["SUB-LINKED".into()];
    let error = runtime.query_execute(req.clone()).unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(error.message.contains("incompatible flavour"));
    let cli = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "bad",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-LINKED",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(!cli.status.success());
    assert!(stdout(&cli).contains("invalid_definition"));
    let responses = mcp_exchange_with_arguments(
        fixture.path(),
        &["--execution-mode", "direct", "mcp"],
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "query_execute", serde_json::to_value(&req).unwrap()),
        ],
    );
    assert_eq!(mcp_response(&responses, 2)["result"]["isError"], true);
    req.selection.ids = vec!["SUB-EMPTY".into()];
    let no_rows = runtime.query_execute(req.clone()).unwrap();
    assert!(no_rows.evaluation_complete, "{no_rows:#?}");
    assert_eq!(no_rows.summary.binding_count, Some(0));

    let endpoint = json!({"definitions":[{"id":"endpoint","root_flavours":["subject"],"columns":{"title":{"type":"text"}},"select":"SELECT ?root ?title WHERE { ?root <urn:mara:item:title> ?title . ?root <urn:mara:relation:parent> <urn:mara:id:SUB-UNKNOWN> }"}]});
    write_queries(fixture.path(), endpoint);
    req.definition = "endpoint".into();
    req.selection.ids = vec!["SUB-LINKED".into()];
    assert_eq!(
        runtime.query_execute(req.clone()).unwrap_err().code,
        "invalid_definition"
    );
    let cli = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "endpoint",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-LINKED",
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(!cli.status.success());
    assert!(stdout(&cli).contains("invalid_definition"));
    let responses = mcp_exchange_with_arguments(
        fixture.path(),
        &["--execution-mode", "direct", "mcp"],
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "query_execute", serde_json::to_value(&req).unwrap()),
        ],
    );
    assert_eq!(mcp_response(&responses, 2)["result"]["isError"], true);
}

fn view_cli_args(req: &q::Request, format: &str) -> Vec<String> {
    let mut args = vec![
        "--execution-mode".into(),
        "direct".into(),
        "--format".into(),
        format.into(),
        "query".into(),
        "view".into(),
        req.definition.clone(),
    ];
    for file in &req.definition_files {
        args.extend(["--definition-file".into(), file.clone()]);
    }
    for id in &req.selection.ids {
        args.extend(["--id".into(), id.clone()]);
    }
    for parameter in &req.parameters {
        args.extend([
            "--param".into(),
            format!(
                "{}={}",
                parameter.name,
                serde_json::to_string(&parameter.value).unwrap()
            ),
        ]);
    }
    args.extend([
        "--max-rows".into(),
        req.max_rows.to_string(),
        "--limit".into(),
        req.limit.to_string(),
    ]);
    if let Some(cursor) = &req.cursor {
        args.extend(["--cursor".into(), cursor.clone()]);
    }
    args
}

fn gate_request() -> mara::query_consumers::QueryGateRequest {
    mara::query_consumers::QueryGateRequest {
        definition_files: vec!["queries.json".into()],
        gate_file: "gate.yaml".into(),
        shape_files: vec!["presence.ttl".into()],
        gate: "require-support".into(),
        scope: q::Selection {
            ids: vec!["SUB-LINKED".into(), "SUB-EMPTY".into()],
            ..Default::default()
        },
        context: vec![q::BindingParameter {
            name: "state".into(),
            value: q::ParameterValue::Text("accepted".into()),
        }],
        fact_snapshots: vec![],
        mappings: vec![],
        max_rows: 100,
    }
}

fn gate_report_cli_args(format: &str) -> Vec<String> {
    vec![
        "--execution-mode".into(),
        "direct".into(),
        "--format".into(),
        format.into(),
        "query".into(),
        "report".into(),
        "gate".into(),
        "require-support".into(),
        "--gate-file".into(),
        "gate.yaml".into(),
        "--definition-file".into(),
        "queries.json".into(),
        "--shape-file".into(),
        "presence.ttl".into(),
        "--id".into(),
        "SUB-LINKED".into(),
        "--id".into(),
        "SUB-EMPTY".into(),
        "--param".into(),
        "state={\"type\":\"text\",\"value\":\"accepted\"}".into(),
        "--max-rows".into(),
        "100".into(),
    ]
}

fn is_sub_empty(root: &Option<q::Endpoint>) -> bool {
    matches!(root, Some(q::Endpoint::Item { id, .. }) if id == "SUB-EMPTY")
}

// @mara checks REQ-QUERY-VIEWS-AND-REPORTS
#[test]
fn factual_view_preserves_facts_and_empty_roots_across_rust_cli_and_mcp() {
    let fixture = project();
    install_query_consumers(fixture.path());
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let mut request = request();
    request.limit = 100;

    let page = runtime.query_execute(request.clone()).unwrap();
    let view = runtime
        .query_view(mara::query_views::ViewParams {
            request: request.clone(),
            render: None,
        })
        .unwrap();
    assert_eq!(view.format_version, 1);
    assert_eq!(view.kind, "query_view");
    assert_eq!(view.definition, page.definition);
    assert_eq!(view.definition_file, page.definition_file);
    assert_eq!(view.input_identity, page.input_identity);
    assert_eq!(view.evaluation_complete, page.evaluation_complete);
    assert_eq!(view.summary.selected_roots, page.summary.selected_roots);
    assert_eq!(view.summary.binding_count, page.summary.binding_count);
    assert_eq!(view.summary.known_bindings, page.summary.known_bindings);
    assert_eq!(view.has_more, page.has_more);
    assert_eq!(view.next_cursor, page.next_cursor);
    assert_eq!(view.markdown, None);
    let names = view
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>();
    assert!(names.contains(&"support"));
    assert!(names.contains(&"amount"));
    assert!(names.contains(&"absent"));
    let linked = view
        .roots
        .iter()
        .find(|root| matches!(&root.root, q::Endpoint::Item { id, .. } if id == "SUB-LINKED"))
        .unwrap();
    let empty = view
        .roots
        .iter()
        .find(|root| matches!(&root.root, q::Endpoint::Item { id, .. } if id == "SUB-EMPTY"))
        .unwrap();
    assert!(linked.source.is_some());
    assert_eq!(linked.binding_count, Some(3));
    assert_eq!(linked.rows.len(), 3);
    assert!(empty.source.is_some());
    assert_eq!(empty.binding_count, Some(0));
    assert!(empty.rows.is_empty());

    // Rendering does not change returned typed facts.
    let page_rows = page
        .records
        .iter()
        .filter_map(|record| match record {
            q::Record::Binding { binding, .. } => Some(serde_json::to_value(binding).unwrap()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let view_rows = view
        .roots
        .iter()
        .flat_map(|root| root.rows.iter())
        .map(|row| {
            serde_json::to_value(q::Binding {
                values: row.values.clone(),
                contributors: row.contributors.clone(),
                support: row.support.clone(),
                facts: row.facts.clone(),
            })
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(page_rows, view_rows);

    let rendered = runtime
        .query_view(mara::query_views::ViewParams {
            request: request.clone(),
            render: Some("markdown".into()),
        })
        .unwrap();
    let markdown = rendered.markdown.as_deref().unwrap();
    assert!(markdown.contains("# Factual query view"));
    assert!(
        markdown.contains("No related matches were returned for this root in the complete result.")
    );
    let empty_section = markdown.split("## [SUB-EMPTY]").nth(1).unwrap();
    assert!(
        !empty_section.to_lowercase().contains("fail"),
        "{empty_section}"
    );

    let cli_json = mara(
        fixture.path(),
        &view_cli_args(&request, "json")
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    assert!(cli_json.status.success(), "{}", stderr(&cli_json));
    let cli_json: Value = serde_json::from_slice(&cli_json.stdout).unwrap();
    assert_eq!(cli_json, serde_json::to_value(&view).unwrap());
    let cli_human = mara(
        fixture.path(),
        &view_cli_args(&request, "human")
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    assert!(cli_human.status.success(), "{}", stderr(&cli_human));
    assert!(stdout(&cli_human).contains("# Factual query view"));

    let params = serde_json::to_value(&request).unwrap();
    assert_eq!(
        consumer_mcp(fixture.path(), "query_view", params.clone()),
        cli_json
    );
    let mut rendered_params = params;
    rendered_params["render"] = json!("markdown");
    let mcp_rendered = consumer_mcp(fixture.path(), "query_view", rendered_params);
    assert_eq!(
        mcp_rendered["markdown"].as_str().unwrap(),
        rendered.markdown.as_deref().unwrap()
    );

    // Continuation preserves every binding and root without repeats.
    let mut cursor = None;
    let mut total_rows = 0usize;
    let mut roots = std::collections::BTreeSet::new();
    loop {
        let mut page_request = self::request();
        page_request.cursor = cursor.take();
        let page_view = runtime
            .query_view(mara::query_views::ViewParams {
                request: page_request,
                render: None,
            })
            .unwrap();
        total_rows += page_view
            .roots
            .iter()
            .map(|root| root.rows.len())
            .sum::<usize>();
        for root in &page_view.roots {
            roots.insert(root.root.clone());
        }
        if !page_view.has_more {
            break;
        }
        cursor = page_view.next_cursor.clone();
        assert!(cursor.is_some());
    }
    assert_eq!(total_rows, 3);
    assert_eq!(roots.len(), 2);
}

// @mara checks REQ-QUERY-VIEWS-AND-REPORTS
#[test]
fn evaluation_reports_expose_gate_and_rule_outcomes_across_rust_cli_and_mcp() {
    let fixture = project();
    install_query_consumers(fixture.path());
    let runtime = NativeRuntime::new(Some(fixture.path().into()), fixture.path().into()).unwrap();
    let portable = SourceEngine::new(consumer_snapshot()).unwrap();
    let target = mara::ValidationTarget {
        kind: mara::ValidationTargetKind::Project,
        id: None,
    };
    let gate = gate_request();
    let gate_result = runtime.query_gate(gate.clone()).unwrap();
    let report = runtime
        .query_report(mara::query_views::ReportRequest::Gate(gate.clone()), None)
        .unwrap();
    assert_eq!(report.format_version, 1);
    assert_eq!(report.input_identity, gate_result.input_identity);
    assert_eq!(report.evaluation_complete, gate_result.evaluation_complete);
    assert_eq!(report.state, Some(gate_result.state));
    assert!(report.roots.iter().any(|root| {
        matches!(&root.root, Some(q::Endpoint::Item { id, .. }) if id == "SUB-EMPTY")
            && root.state == Some(mara::query_consumers::ConsumerState::Failed)
    }));
    assert!(matches!(
        &report.subject,
        mara::query_views::ReportSubject::Gate(subject) if subject.gate == "require-support"
    ));
    let failed = report
        .checks
        .iter()
        .find(|check| check.obligation == "support-condition" && is_sub_empty(&check.root))
        .unwrap();
    assert_eq!(failed.kind, "gate_condition");
    assert_eq!(failed.state, mara::query_consumers::ConsumerState::Failed);
    assert_eq!(
        failed.support_state,
        mara::query_views::SupportState::Missing
    );
    assert_eq!(failed.binding_count, Some(0));
    assert!(!failed.components.is_empty());
    assert_eq!(failed.severity, mara::Severity::Error);
    assert_eq!(
        failed.source.path.as_deref(),
        Some(std::path::Path::new("gate.yaml"))
    );
    assert_eq!(failed.shape, "urn:fixture:shape:has-match");
    let qualified = report
        .checks
        .iter()
        .find(|check| {
            check.obligation == "support-condition"
                && matches!(&check.root, Some(q::Endpoint::Item { id, .. }) if id == "SUB-LINKED")
        })
        .unwrap();
    assert_eq!(
        qualified.state,
        mara::query_consumers::ConsumerState::Passed
    );
    assert_eq!(
        qualified.support_state,
        mara::query_views::SupportState::Qualifying
    );
    assert_eq!(qualified.binding_count, Some(1));
    assert!(!qualified.bindings.is_empty());
    assert_eq!(report.summary.passed, 1);
    assert_eq!(report.summary.failed, 1);
    let gate_markdown = runtime
        .query_report(
            mara::query_views::ReportRequest::Gate(gate.clone()),
            Some("markdown".into()),
        )
        .unwrap()
        .markdown
        .unwrap();
    assert!(gate_markdown.contains("# Gate evaluation report: require-support"));
    assert!(gate_markdown.contains("support **missing**"));
    assert!(gate_markdown.contains("support **qualifying**"));
    assert!(gate_markdown.contains("gate.yaml"));
    assert!(gate_markdown.contains("Scope: ids SUB-LINKED, SUB-EMPTY."));
    assert!(gate_markdown.contains("Context: state = text accepted."));
    assert!(gate_markdown.contains("Root: item SUB-EMPTY."));
    assert!(gate_markdown.contains("| item SUB-EMPTY | failed |"));

    // A gate with no condition records still preserves every evaluated root.
    let mut conditionless = gate_request();
    conditionless.gate = "empty-required".into();
    conditionless.context.clear();
    let conditionless_report = runtime
        .query_report(
            mara::query_views::ReportRequest::Gate(conditionless),
            Some("markdown".into()),
        )
        .unwrap();
    assert!(conditionless_report.checks.is_empty());
    assert_eq!(conditionless_report.roots.len(), 2);
    assert!(
        conditionless_report
            .roots
            .iter()
            .all(|root| root.state == Some(mara::query_consumers::ConsumerState::Failed))
    );
    let conditionless_markdown = conditionless_report.markdown.as_deref().unwrap();
    assert!(conditionless_markdown.contains("| Root | Outcome |"));
    assert!(conditionless_markdown.contains("| item SUB-EMPTY | failed |"));
    assert!(conditionless_markdown.contains("| item SUB-LINKED | failed |"));

    let (policy, _policy_diagnostics) = portable.query_policy_validation(target.clone());
    let policy = policy.unwrap();
    let rules = runtime
        .query_report(
            mara::query_views::ReportRequest::DocumentationRules {
                target: target.clone(),
                fact_snapshots: vec![],
                mappings: vec![],
            },
            None,
        )
        .unwrap();
    assert!(rules.state.is_none());
    assert!(!rules.input_identity.is_empty());
    assert!(rules.evaluation_complete);
    assert_eq!(rules.evaluation_complete, policy.evaluation_complete);
    assert!(rules.diagnostics.iter().any(|diagnostic| {
        diagnostic.rule.as_deref() == Some("require-query-match")
            && diagnostic.code == mara::DiagnosticCode::RuleFailed
    }));
    let rule_failed = rules
        .checks
        .iter()
        .find(|check| check.obligation == "require-query-match" && is_sub_empty(&check.root))
        .unwrap();
    assert_eq!(rule_failed.kind, "documentation_rule");
    assert_eq!(
        rule_failed.state,
        mara::query_consumers::ConsumerState::Failed
    );
    assert_eq!(
        rule_failed.support_state,
        mara::query_views::SupportState::Missing
    );
    assert_eq!(rule_failed.binding_count, Some(0));
    assert!(!rule_failed.components.is_empty());
    assert_eq!(
        rule_failed.source.path.as_deref(),
        Some(std::path::Path::new("invariants.yaml"))
    );
    let policy_failed = policy
        .checks
        .iter()
        .find(|check| {
            check.rule == "require-query-match"
                && check.root.as_ref().is_some_and(
                    |root| matches!(root, q::Endpoint::Item { id, .. } if id == "SUB-EMPTY"),
                )
        })
        .unwrap();
    assert_eq!(
        serde_json::to_value(&rule_failed.bindings).unwrap(),
        serde_json::to_value(&policy_failed.bindings).unwrap()
    );
    let rule_markdown = runtime
        .query_report(
            mara::query_views::ReportRequest::DocumentationRules {
                target: target.clone(),
                fact_snapshots: vec![],
                mappings: vec![],
            },
            Some("markdown".into()),
        )
        .unwrap()
        .markdown
        .unwrap();
    assert!(rule_markdown.contains("# Documentation rule evaluation report"));
    assert!(rule_markdown.contains("support **missing**"));
    assert!(rule_markdown.contains("Root: item SUB-EMPTY."));

    let cli_gate = mara(
        fixture.path(),
        &gate_report_cli_args("json")
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    assert!(cli_gate.status.success(), "{}", stderr(&cli_gate));
    let cli_gate: Value = serde_json::from_slice(&cli_gate.stdout).unwrap();
    assert_eq!(cli_gate, serde_json::to_value(&report).unwrap());
    let cli_gate_human = mara(
        fixture.path(),
        &gate_report_cli_args("human")
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    assert!(
        cli_gate_human.status.success(),
        "{}",
        stderr(&cli_gate_human)
    );
    assert!(stdout(&cli_gate_human).contains("# Gate evaluation report: require-support"));

    let gate_subject = serde_json::to_value(mara::query_views::ReportRequest::Gate(gate)).unwrap();
    assert_eq!(
        consumer_mcp(
            fixture.path(),
            "query_report",
            json!({ "subject": gate_subject })
        ),
        cli_gate
    );

    let cli_rules = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "report",
            "rules",
        ],
    );
    assert!(cli_rules.status.success(), "{}", stderr(&cli_rules));
    let cli_rules: Value = serde_json::from_slice(&cli_rules.stdout).unwrap();
    assert_eq!(cli_rules, serde_json::to_value(&rules).unwrap());
    assert_eq!(
        consumer_mcp(
            fixture.path(),
            "query_report",
            json!({"subject": {"kind": "documentation_rules", "target": {"kind": "project"}}})
        ),
        cli_rules
    );
    let cli_rules_human = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "human",
            "query",
            "report",
            "rules",
        ],
    );
    assert!(
        cli_rules_human.status.success(),
        "{}",
        stderr(&cli_rules_human)
    );
    assert!(stdout(&cli_rules_human).contains("# Documentation rule evaluation report"));
}
