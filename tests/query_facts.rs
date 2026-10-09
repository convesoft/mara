//! Real native CLI/MCP execution of supplied external facts and derived computations.
mod support;
use mara::{NativeRuntime, SourceEngine, SourceSnapshot, named_query as q};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use support::*;

fn snapshot() -> SourceSnapshot {
    serde_json::from_str(include_str!("fixtures/query-facts/snapshot.json")).unwrap()
}
fn project() -> tempfile::TempDir {
    materialize(snapshot())
}
fn materialize(snapshot: SourceSnapshot) -> tempfile::TempDir {
    let fixture = fixture();
    for file in snapshot.files {
        let path = fixture.path().join(file.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, file.content).unwrap();
    }
    fixture
}
fn consumer_snapshot() -> SourceSnapshot {
    let mut snapshot = snapshot();
    let project = snapshot
        .files
        .iter_mut()
        .find(|file| file.path == ".mara/project.toml")
        .unwrap();
    project.content.push_str(
        r#"
[query_policy]
format_version = 1
files = ["facts-invariants.yaml"]
definition_files = ["facts.json"]
shape_files = ["facts-present.ttl"]
max_rows = 100
"#,
    );
    snapshot.files.extend([
        mara::SourceFile {
            path: "facts-invariants.yaml".into(),
            content: r#"format_version: 1
invariants:
  - id: supplied-composed-facts
    definition: urn:fixture:facts:composed
    shape: urn:fixture:shape:has-fact-row
    flavours: [candidate]
    severity: error
    parameters:
      - name: candidate
        value: {type: text, value: CAND-C}
      - name: context
        value: {type: text, value: release}
"#
            .into(),
        },
        mara::SourceFile {
            path: "facts-gates.yaml".into(),
            content: r#"format_version: 1
gates:
  - id: require-composed-facts
    empty_roots: not_applicable
    empty_required: failed
    conditions:
      - id: fact-row
        definition: urn:fixture:facts:composed
        shape_file: facts-present.ttl
        shape: urn:fixture:shape:has-fact-row
        severity: error
        context_parameters: [candidate, context]
"#
            .into(),
        },
        mara::SourceFile {
            path: "facts-present.ttl".into(),
            content: r#"@prefix sh: <http://www.w3.org/ns/shacl#>.
<urn:fixture:shape:has-fact-row> a sh:NodeShape; sh:property [ sh:path <urn:mara:query:match>; sh:minCount 1 ].
"#.into(),
        },
    ]);
    snapshot
}
fn text(value: &str) -> q::ParameterValue {
    q::ParameterValue::Text(value.into())
}
fn integer(value: i64) -> q::ParameterValue {
    q::ParameterValue::Integer(value)
}
fn item(value: &str) -> q::ParameterValue {
    q::ParameterValue::Item(value.into())
}
fn key(verification: &str, candidate: &str, context: &str) -> BTreeMap<String, q::ParameterValue> {
    BTreeMap::from([
        ("verification".to_owned(), item(verification)),
        ("candidate".to_owned(), text(candidate)),
        ("context".to_owned(), text(context)),
    ])
}
fn record(verification: &str, candidate: &str, context: &str, state: &str) -> q::FactRecord {
    q::FactRecord {
        keys: key(verification, candidate, context),
        fields: BTreeMap::from([("state".to_owned(), text(state))]),
    }
}
fn coverage(candidate: &str) -> Vec<BTreeMap<String, q::ParameterValue>> {
    vec![
        key("VER-V1", candidate, "release"),
        key("VER-V2", candidate, "release"),
        key("VER-V3", candidate, "release"),
    ]
}
fn reported(candidate: &str, states: &[(&str, &str)]) -> q::FactSnapshot {
    q::FactSnapshot {
        input: "reported".into(),
        identity: "reported-run".into(),
        coverage: coverage(candidate),
        records: states
            .iter()
            .map(|(verification, state)| record(verification, candidate, "release", state))
            .collect(),
    }
}
fn validation_request(candidate: &str, facts: Vec<q::FactSnapshot>) -> q::Request {
    q::Request {
        definition_files: vec!["facts.json".into()],
        definition: "urn:fixture:facts:validation".into(),
        selection: q::Selection {
            ids: vec![candidate.into()],
            ..Default::default()
        },
        parameters: vec![
            q::BindingParameter {
                name: "candidate".into(),
                value: text(candidate),
            },
            q::BindingParameter {
                name: "context".into(),
                value: text("release"),
            },
        ],
        max_rows: 100,
        limit: 100,
        cursor: None,
        fact_snapshots: facts,
        mappings: vec![],
    }
}
fn risk_request() -> q::Request {
    q::Request {
        definition_files: vec!["facts.json".into()],
        definition: "urn:fixture:facts:risk".into(),
        selection: q::Selection {
            ids: vec!["CAND-C".into(), "CAND-E".into()],
            ..Default::default()
        },
        parameters: vec![],
        max_rows: 100,
        limit: 100,
        cursor: None,
        fact_snapshots: vec![],
        mappings: vec![
            q::TypedMapping {
                mapping: "risk_matrix".into(),
                entries: vec![
                    q::MappingEntry {
                        keys: BTreeMap::from([
                            ("likelihood".to_owned(), integer(1)),
                            ("consequence".to_owned(), integer(3)),
                        ]),
                        value: integer(3),
                    },
                    q::MappingEntry {
                        keys: BTreeMap::from([
                            ("likelihood".to_owned(), integer(2)),
                            ("consequence".to_owned(), integer(3)),
                        ]),
                        value: integer(6),
                    },
                ],
                fallback: None,
            },
            q::TypedMapping {
                mapping: "risk_band".into(),
                entries: vec![
                    q::MappingEntry {
                        keys: BTreeMap::from([("score".to_owned(), integer(3))]),
                        value: text("below"),
                    },
                    q::MappingEntry {
                        keys: BTreeMap::from([("score".to_owned(), integer(6))]),
                        value: text("above"),
                    },
                ],
                fallback: None,
            },
        ],
    }
}
fn write_json(root: &std::path::Path, name: &str, value: &impl serde::Serialize) -> String {
    fs::write(root.join(name), serde_json::to_vec(value).unwrap()).unwrap();
    name.to_owned()
}
fn cli(
    root: &std::path::Path,
    request: &q::Request,
    fact_files: &[String],
    mapping_files: &[String],
) -> std::process::Output {
    let mut args = vec![
        "--execution-mode".to_owned(),
        "direct".into(),
        "--format".into(),
        "json".into(),
        "query".into(),
        "execute".into(),
        request.definition.clone(),
    ];
    for file in &request.definition_files {
        args.extend(["--definition-file".into(), file.clone()]);
    }
    for id in &request.selection.ids {
        args.extend(["--id".into(), id.clone()]);
    }
    for parameter in &request.parameters {
        args.extend([
            "--param".into(),
            format!(
                "{}={}",
                parameter.name,
                serde_json::to_string(&parameter.value).unwrap()
            ),
        ]);
    }
    for file in fact_files {
        args.extend(["--fact-file".into(), file.clone()]);
    }
    for file in mapping_files {
        args.extend(["--mapping-file".into(), file.clone()]);
    }
    args.extend([
        "--max-rows".into(),
        request.max_rows.to_string(),
        "--limit".into(),
        request.limit.to_string(),
    ]);
    mara(root, &args.iter().map(String::as_str).collect::<Vec<_>>())
}
fn mcp(root: &std::path::Path, request: &q::Request) -> Value {
    mcp_operation(
        root,
        "query_execute",
        serde_json::to_value(request).unwrap(),
    )
}
fn mcp_operation(root: &std::path::Path, name: &str, params: Value) -> Value {
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

// @mara checks REQ-EXPLICIT-QUERY-INPUTS
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn supplied_facts_and_mappings_match_portable_embedded_cli_and_mcp_results() {
    let root = project();
    let runtime = NativeRuntime::new(None, root.path().to_path_buf()).unwrap();
    let portable = SourceEngine::new(snapshot()).unwrap();

    // Complete coverage, one known-missing verification and a definition-selected aggregate.
    let facts = reported("CAND-C", &[("VER-V1", "pass"), ("VER-V2", "pass")]);
    let fact_file = write_json(root.path(), "reported.json", &facts);
    let request = validation_request("CAND-C", vec![facts]);
    let embedded = portable.query_execute(request.clone()).unwrap();
    assert!(embedded.evaluation_complete, "{embedded:#?}");
    assert_eq!(embedded.summary.binding_count, Some(1));

    let native = runtime.query_execute(request.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&native).unwrap(),
        serde_json::to_value(&embedded).unwrap()
    );

    let cli_page = cli(root.path(), &request, &[fact_file], &[]);
    assert!(
        cli_page.status.success(),
        "{} {}",
        stdout(&cli_page),
        stderr(&cli_page)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&cli_page.stdout).unwrap(),
        serde_json::to_value(&embedded).unwrap()
    );

    let mcp_page = mcp(root.path(), &request);
    assert_eq!(mcp_page, serde_json::to_value(&embedded).unwrap());

    // Replaceable risk mappings through the same CLI/MCP paths.
    let risk = risk_request();
    let matrix = write_json(root.path(), "matrix.json", &risk.mappings[0]);
    let band = write_json(root.path(), "band.json", &risk.mappings[1]);
    let embedded = portable.query_execute(risk.clone()).unwrap();
    assert!(embedded.evaluation_complete, "{embedded:#?}");
    let cli_page = cli(root.path(), &risk, &[], &[matrix.clone(), band.clone()]);
    assert!(
        cli_page.status.success(),
        "{} {}",
        stdout(&cli_page),
        stderr(&cli_page)
    );
    let risk_page: Value = serde_json::from_slice(&cli_page.stdout).unwrap();
    assert_eq!(risk_page, serde_json::to_value(&embedded).unwrap());
    assert_eq!(mcp(root.path(), &risk), risk_page);

    // A missing required snapshot is unavailable in every real path, not empty success.
    let absent = validation_request("CAND-C", vec![]);
    let embedded = portable.query_execute(absent.clone()).unwrap();
    assert!(!embedded.evaluation_complete);
    assert!(!cli(root.path(), &absent, &[], &[]).status.success());
    let absent_page = mcp(root.path(), &absent);
    assert_eq!(absent_page["evaluation_complete"], json!(false));
    assert!(
        absent_page["unavailable_reason"]
            .as_str()
            .unwrap()
            .contains("was not supplied")
    );

    // Partial coverage (V1 only) is unavailable, not a known-missing pass.
    let partial = q::FactSnapshot {
        input: "reported".into(),
        identity: "reported-run".into(),
        coverage: vec![key("VER-V1", "CAND-C", "release")],
        records: vec![record("VER-V1", "CAND-C", "release", "pass")],
    };
    let partial_file = write_json(root.path(), "partial.json", &partial);
    let request = validation_request("CAND-C", vec![partial]);
    let embedded = portable.query_execute(request.clone()).unwrap();
    assert!(!embedded.evaluation_complete);
    let cli_page = cli(root.path(), &request, &[partial_file], &[]);
    assert!(!cli_page.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&cli_page.stdout).unwrap()["evaluation_complete"],
        json!(false)
    );
}

// @mara checks REQ-QUERY-ENGINE-SURFACES
// @mara checks DES-QUERY-CONSUMER-CONTRACT
#[test]
fn composed_supplied_facts_flow_through_views_policy_gates_and_reports() {
    // The same fixture is used by browser and native hosts; no source state is persisted from this input.
    let facts = reported(
        "CAND-C",
        &[("VER-V1", "pass"), ("VER-V2", "pass"), ("VER-V3", "fail")],
    );
    let mut fixture_snapshot = consumer_snapshot();
    fixture_snapshot.files.push(mara::SourceFile {
        path: "facts-input.json".into(),
        content: serde_json::to_string(&facts).unwrap(),
    });
    let engine = SourceEngine::new(fixture_snapshot).unwrap();
    let request = q::Request {
        definition_files: vec!["facts.json".into()],
        definition: "urn:fixture:facts:composed".into(),
        selection: q::Selection {
            ids: vec!["CAND-C".into()],
            ..Default::default()
        },
        parameters: vec![
            q::BindingParameter {
                name: "candidate".into(),
                value: text("CAND-C"),
            },
            q::BindingParameter {
                name: "context".into(),
                value: text("release"),
            },
        ],
        max_rows: 100,
        limit: 100,
        cursor: None,
        fact_snapshots: vec![facts.clone()],
        mappings: vec![],
    };
    let raw = engine.query_execute(request.clone()).unwrap();
    assert!(raw.evaluation_complete, "{raw:#?}");
    let raw_bindings = raw
        .records
        .iter()
        .filter_map(|record| match record {
            q::Record::Binding { binding, .. } => Some(binding),
            q::Record::Root { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(raw_bindings.len(), 3);
    assert!(raw_bindings.iter().all(|binding| !binding.facts.is_empty()));

    let view = engine
        .query_view(mara::query_views::ViewParams {
            request: request.clone(),
            render: None,
        })
        .unwrap();
    assert_eq!(view.input_identity, raw.input_identity);
    assert!(view.evaluation_complete);
    assert_eq!(view.summary.binding_count, Some(3));
    assert!(
        view.roots
            .iter()
            .flat_map(|root| &root.rows)
            .all(|row| !row.facts.is_empty())
    );

    let options = mara::ValidationOptions {
        limit: Some(100),
        cursor: None,
        fact_snapshots: vec![facts.clone()],
        mappings: vec![],
    };
    let validation = engine.item_validate("CAND-C", &options, None).unwrap();
    assert!(validation.valid && validation.evaluation_complete);
    assert_eq!(
        validation.query_policies.as_ref().unwrap().checks[0].state,
        mara::query_consumers::ConsumerState::Passed
    );

    let gate_request = mara::query_consumers::QueryGateRequest {
        definition_files: vec!["facts.json".into()],
        gate_file: "facts-gates.yaml".into(),
        shape_files: vec!["facts-present.ttl".into()],
        gate: "require-composed-facts".into(),
        scope: request.selection.clone(),
        context: request.parameters.clone(),
        fact_snapshots: vec![facts.clone()],
        mappings: vec![],
        max_rows: 100,
    };
    let gate = engine.query_gate(gate_request.clone()).unwrap();
    assert_eq!(gate.state, mara::query_consumers::ConsumerState::Passed);
    let gate_report = engine
        .query_report(
            mara::query_views::ReportRequest::Gate(gate_request.clone()),
            None,
        )
        .unwrap();
    assert!(gate_report.evaluation_complete);
    assert_eq!(
        gate_report.checks[0].state,
        mara::query_consumers::ConsumerState::Passed
    );

    let rules_report = engine
        .query_report(
            mara::query_views::ReportRequest::DocumentationRules {
                target: mara::ValidationTarget {
                    kind: mara::ValidationTargetKind::Item,
                    id: Some("CAND-C".into()),
                },
                fact_snapshots: vec![facts.clone()],
                mappings: vec![],
            },
            None,
        )
        .unwrap();
    assert!(rules_report.evaluation_complete);
    assert_eq!(
        rules_report.checks[0].state,
        mara::query_consumers::ConsumerState::Passed
    );

    // Exercise every supplied-input consumer through real embedded CLI and MCP calls.
    let root = materialize(consumer_snapshot());
    let facts_file = write_json(root.path(), "facts-input.json", &facts);
    let runtime = NativeRuntime::new(None, root.path().to_path_buf()).unwrap();
    let runtime_view = runtime
        .query_view(mara::query_views::ViewParams {
            request: request.clone(),
            render: None,
        })
        .unwrap();
    let runtime_validation = runtime
        .item_validate_with_options(
            "CAND-C",
            &mara::ValidationOptions {
                fact_snapshots: vec![facts.clone()],
                ..Default::default()
            },
        )
        .unwrap();
    let runtime_gate = runtime.query_gate(gate_request.clone()).unwrap();
    let runtime_gate_report = runtime
        .query_report(
            mara::query_views::ReportRequest::Gate(gate_request.clone()),
            None,
        )
        .unwrap();
    let runtime_rules_report = runtime
        .query_report(
            mara::query_views::ReportRequest::DocumentationRules {
                target: mara::ValidationTarget {
                    kind: mara::ValidationTargetKind::Item,
                    id: Some("CAND-C".into()),
                },
                fact_snapshots: vec![facts.clone()],
                mappings: vec![],
            },
            None,
        )
        .unwrap();
    let view_cli = mara(
        root.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "view",
            "urn:fixture:facts:composed",
            "--definition-file",
            "facts.json",
            "--id",
            "CAND-C",
            "--param",
            r#"candidate={"type":"text","value":"CAND-C"}"#,
            "--param",
            r#"context={"type":"text","value":"release"}"#,
            "--fact-file",
            facts_file.as_str(),
            "--max-rows",
            "100",
            "--limit",
            "100",
        ],
    );
    assert!(view_cli.status.success(), "{}", stderr(&view_cli));
    assert_eq!(
        serde_json::from_slice::<Value>(&view_cli.stdout).unwrap(),
        serde_json::to_value(&runtime_view).unwrap()
    );

    let validation_cli = mara(
        root.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "item",
            "validate",
            "CAND-C",
            "--fact-file",
            facts_file.as_str(),
        ],
    );
    assert!(
        validation_cli.status.success(),
        "{}",
        stderr(&validation_cli)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&validation_cli.stdout).unwrap(),
        serde_json::to_value(&runtime_validation).unwrap()
    );

    let gate_args = [
        "--execution-mode",
        "direct",
        "--format",
        "json",
        "query",
        "gate",
        "require-composed-facts",
        "--gate-file",
        "facts-gates.yaml",
        "--definition-file",
        "facts.json",
        "--shape-file",
        "facts-present.ttl",
        "--id",
        "CAND-C",
        "--param",
        r#"candidate={"type":"text","value":"CAND-C"}"#,
        "--param",
        r#"context={"type":"text","value":"release"}"#,
        "--fact-file",
        facts_file.as_str(),
        "--max-rows",
        "100",
    ];
    let gate_cli = mara(root.path(), &gate_args);
    assert!(gate_cli.status.success(), "{}", stderr(&gate_cli));
    assert_eq!(
        serde_json::from_slice::<Value>(&gate_cli.stdout).unwrap(),
        serde_json::to_value(&runtime_gate).unwrap()
    );

    let gate_report_cli = mara(
        root.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "report",
            "gate",
            "require-composed-facts",
            "--gate-file",
            "facts-gates.yaml",
            "--definition-file",
            "facts.json",
            "--shape-file",
            "facts-present.ttl",
            "--id",
            "CAND-C",
            "--param",
            r#"candidate={"type":"text","value":"CAND-C"}"#,
            "--param",
            r#"context={"type":"text","value":"release"}"#,
            "--fact-file",
            facts_file.as_str(),
            "--max-rows",
            "100",
        ],
    );
    assert!(
        gate_report_cli.status.success(),
        "{}",
        stderr(&gate_report_cli)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&gate_report_cli.stdout).unwrap(),
        serde_json::to_value(&runtime_gate_report).unwrap()
    );

    let rules_report_cli = mara(
        root.path(),
        &[
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "report",
            "rules",
            "--id",
            "CAND-C",
            "--fact-file",
            facts_file.as_str(),
        ],
    );
    assert!(
        rules_report_cli.status.success(),
        "{}",
        stderr(&rules_report_cli)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&rules_report_cli.stdout).unwrap(),
        serde_json::to_value(&runtime_rules_report).unwrap()
    );

    let facts_mcp = json!([facts.clone()]);
    let mut view_params = serde_json::to_value(&request).unwrap();
    view_params["render"] = Value::Null;
    assert_eq!(
        mcp_operation(root.path(), "query_view", view_params),
        serde_json::to_value(&runtime_view).unwrap()
    );
    assert_eq!(
        mcp_operation(
            root.path(),
            "item_validate",
            json!({"id":"CAND-C","fact_snapshots":facts_mcp})
        ),
        serde_json::to_value(&runtime_validation).unwrap()
    );
    assert_eq!(
        mcp_operation(
            root.path(),
            "query_gate",
            serde_json::to_value(&gate_request).unwrap()
        ),
        serde_json::to_value(&runtime_gate).unwrap()
    );
    assert_eq!(
        mcp_operation(
            root.path(),
            "query_report",
            json!({"subject":mara::query_views::ReportRequest::Gate(gate_request.clone()),"render":null}),
        ),
        serde_json::to_value(&runtime_gate_report).unwrap()
    );
    assert_eq!(
        mcp_operation(
            root.path(),
            "query_report",
            json!({
                "subject":mara::query_views::ReportRequest::DocumentationRules {
                    target:mara::ValidationTarget {
                        kind:mara::ValidationTargetKind::Item,
                        id:Some("CAND-C".into()),
                    },
                    fact_snapshots:vec![facts],
                    mappings:vec![],
                },
                "render":null,
            }),
        ),
        serde_json::to_value(&runtime_rules_report).unwrap()
    );

    // Required external input remains unavailable for an explicitly empty scope.
    let mut empty_gate = gate_request;
    empty_gate.scope = q::Selection {
        fields: vec![q::FieldSelection {
            key: "status".into(),
            value: "no-match".into(),
        }],
        ..Default::default()
    };
    empty_gate.fact_snapshots.clear();
    empty_gate.context.clear();
    let empty = engine.query_gate(empty_gate).unwrap();
    assert_eq!(
        empty.state,
        mara::query_consumers::ConsumerState::Unavailable
    );
    assert!(!empty.evaluation_complete);
}
