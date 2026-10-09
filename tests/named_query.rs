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
    definitions["format_version"] = json!(3);
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
fn raw_facts_ignore_failing_documentation_policy_and_refuse_unimplemented_daemon_path() {
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
    let result = mara(
        fixture.path(),
        &[
            "--execution-mode",
            "daemon-required",
            "--format",
            "json",
            "query",
            "execute",
            "urn:fixture:query:supports",
            "--definition-file",
            "queries.json",
            "--id",
            "SUB-LINKED",
            "--max-rows",
            "100",
        ],
    );
    assert!(!result.status.success());
    assert!(stdout(&result).contains("unsupported_capability"));
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
