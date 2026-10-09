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
    let fixture = fixture();
    for file in snapshot().files {
        let path = fixture.path().join(file.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, file.content).unwrap();
    }
    fixture
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
    let responses = mcp_exchange_with_arguments(
        root,
        &["--execution-mode", "direct", "mcp"],
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "query_execute", serde_json::to_value(request).unwrap()),
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
