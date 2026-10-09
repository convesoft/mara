use mara_engine::{
    SourceEngine, SourceSnapshot,
    named_query::{self as q, Record, Value},
};
use serde_json::json;
fn snapshot() -> SourceSnapshot {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/named-query/snapshot.json"
    ))
    .unwrap()
}
fn request() -> q::Request {
    q::Request {
        definition_files: vec!["queries.json".into()],
        definition: "urn:fixture:query:supports".into(),
        selection: q::Selection {
            ids: vec![
                "SUB-LINKED".into(),
                "SUB-EMPTY".into(),
                "01ARZ3NDEKTSV4RRFFQ69G5F00".into(),
            ],
            ..Default::default()
        },
        parameters: vec![q::BindingParameter {
            name: "state".into(),
            value: q::ParameterValue::Text("".into()),
        }],
        max_rows: 100,
        limit: 100,
        cursor: None,
    }
}
fn mutate_definition(s: &mut SourceSnapshot, f: impl FnOnce(&mut serde_json::Value)) {
    let file = s
        .files
        .iter_mut()
        .find(|f| f.path == "queries.json")
        .unwrap();
    let mut definitions: serde_json::Value = serde_json::from_str(&file.content).unwrap();
    f(&mut definitions);
    file.content = definitions.to_string();
}
// @mara checks REQ-NAMED-QUERY-FACTS
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn typed_lifecycle_distinct_support_absence_empty_roots_and_continuation() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let full = engine.query_execute(request()).unwrap();
    assert!(full.evaluation_complete && !full.has_more, "{full:#?}");
    assert_eq!(full.summary.selected_roots, 2);
    assert_eq!(full.summary.binding_count, Some(3));
    let mut lifecycle = vec![];
    let mut occurrences = 0;
    for record in &full.records {
        if let Record::Binding { binding, .. } = record {
            let Some(Value::Text { value, .. }) = &binding.values["lifecycle"] else {
                panic!("typed lifecycle")
            };
            lifecycle.push(value.clone());
            assert!(matches!(
                binding.values["amount"],
                Some(Value::Integer { value: 1 })
            ));
            assert!(
                matches!(&binding.values["text"], Some(Value::Text { value, .. }) if value=="1")
            );
            assert!(matches!(
                binding.values["flag"],
                Some(Value::Boolean { value: true })
            ));
            assert!(
                matches!(&binding.values["score"], Some(Value::Number { value }) if value=="1.5")
            );
            assert!(binding.values["absent"].is_none());
            assert_eq!(binding.support.len(), 1);
            occurrences += binding.support[0].occurrences.len();
            assert!(
                binding.support[0]
                    .occurrences
                    .iter()
                    .all(|o| o.source.path().to_str() == Some("fixture.mara.md"))
            );
            assert_eq!(binding.contributors.len(), 1);
        }
    }
    lifecycle.sort();
    assert_eq!(lifecycle, ["accepted", "draft", "retired"]);
    assert_eq!(occurrences, 5);
    assert!(full.records.iter().any(|r| matches!(r,Record::Root { root:q::Endpoint::Item { id,.. },binding_count:Some(0),.. } if id=="SUB-EMPTY")));
    let mut req = request();
    req.limit = 1;
    let mut records = vec![];
    let mut identity = None;
    loop {
        let page = engine.query_execute(req.clone()).unwrap();
        assert_eq!(page.summary.binding_count, Some(3));
        assert!(page.evaluation_complete);
        if let Some(id) = &identity {
            assert_eq!(id, &page.input_identity);
        } else {
            identity = Some(page.input_identity.clone());
        }
        records.extend(page.records);
        req.cursor = page.next_cursor;
        if !page.has_more {
            break;
        }
    }
    assert_eq!(
        serde_json::to_value(records).unwrap(),
        serde_json::to_value(full.records).unwrap()
    );
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn strict_definition_schema_arguments_and_unavailable_inputs() {
    let base = snapshot();
    for mutation in [0, 1, 2, 4] {
        let mut s = base.clone();
        mutate_definition(&mut s, |defs| match mutation {
            0 => {
                let d = defs["definitions"][0].clone();
                defs["definitions"].as_array_mut().unwrap().push(d);
            }
            1 => {
                defs["definitions"][0]["select"] = json!(
                    "SELECT ?root ?support ?lifecycle ?amount ?text ?flag ?score ?absent WHERE { ?root <urn:mara:relation:typo> ?support }"
                )
            }
            2 => defs["definitions"][0]["root_flavours"] = json!(["unknown"]),
            _ => {
                defs["definitions"][0]["select"] = json!(
                    "SELECT ?root ?support ?lifecycle ?amount ?text ?flag ?score ?absent WHERE { SERVICE SILENT <https://example.invalid/> { ?root ?support ?lifecycle } }"
                )
            }
        });
        let engine = SourceEngine::new(s).unwrap();
        let mut req = request();
        req.selection = q::Selection {
            flavours: vec!["subject".into()],
            fields: vec![q::FieldSelection {
                key: "status".into(),
                value: "never".into(),
            }],
            ..Default::default()
        };
        let err = engine.query_execute(req).unwrap_err();
        assert_eq!(err.code, "invalid_definition", "{mutation}: {err:?}");
        assert_eq!(err.source.as_deref(), Some("queries.json"));
    }
    let engine = SourceEngine::new(base.clone()).unwrap();
    for bad in [0, 1, 2, 3, 4, 5] {
        let mut req = request();
        match bad {
            0 => req.selection = q::Selection::default(),
            1 => req.selection.all = true,
            2 => req.selection.ids.push("SUB-NOPE".into()),
            3 => req.parameters.clear(),
            4 => req.parameters[0].value = q::ParameterValue::Integer(1),
            _ => req.parameters.push(req.parameters[0].clone()),
        }
        assert_eq!(
            engine.query_execute(req).unwrap_err().code,
            "invalid_argument",
            "invalid request case {bad}"
        );
    }
    let mut req = request();
    req.selection = q::Selection {
        flavours: vec!["subject".into()],
        fields: vec![q::FieldSelection {
            key: "status".into(),
            value: "never".into(),
        }],
        ..Default::default()
    };
    let empty = engine.query_execute(req).unwrap();
    assert!(empty.evaluation_complete && empty.records.is_empty());
    for missing in ["queries.json", "fixture.mara.md", ".mara/schema.yaml"] {
        let mut s = base.clone();
        s.files.retain(|f| f.path != missing);
        let page = SourceEngine::new(s)
            .unwrap()
            .query_execute(request())
            .unwrap();
        assert!(!page.evaluation_complete);
        assert!(page.unavailable_reason.is_some());
        assert_eq!(page.summary.binding_count, None);
    }
    let mut req = request();
    req.max_rows = 1;
    let limited = engine.query_execute(req).unwrap();
    assert!(!limited.evaluation_complete);
    assert_eq!(limited.summary.binding_count, None);
    assert_eq!(limited.summary.selected_roots, 2);
    assert_eq!(
        limited
            .records
            .iter()
            .filter(|r| matches!(r, Record::Root { .. }))
            .count(),
        2
    );
}
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
#[test]
fn cyclic_diamond_ancestry_uses_sparql_results_without_hidden_support() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let mut req = request();
    req.definition = "urn:fixture:query:ancestry".into();
    req.parameters.clear();
    let page = engine.query_execute(req).unwrap();
    assert!(page.evaluation_complete);
    assert_eq!(page.summary.binding_count, Some(3));
    let mut endpoints = vec![];
    for record in page.records {
        if let Record::Binding { binding, .. } = record {
            let Some(Value::Item {
                endpoint: q::Endpoint::Item { id, .. },
                ..
            }) = &binding.values["ancestor"]
            else {
                panic!()
            };
            endpoints.push(id.clone());
            assert!(binding.support.is_empty());
            assert!(
                binding
                    .support
                    .iter()
                    .all(|s| !matches!(&s.edge.source,q::Endpoint::Item { id,.. } if id=="SUB-S"))
            );
        }
    }
    endpoints.sort();
    assert_eq!(endpoints, ["SUB-A", "SUB-B", "SUB-G"]);
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn content_parameters_and_controls_invalidate_continuation_and_runtime_reuse() {
    let base = snapshot();
    let engine = SourceEngine::new(base.clone()).unwrap();
    let mut req = request();
    req.limit = 1;
    req.cursor = engine.query_execute(req.clone()).unwrap().next_cursor;
    assert!(req.cursor.is_some());
    for kind in [0, 1, 2] {
        let mut s = base.clone();
        let file = s
            .files
            .iter_mut()
            .find(|f| {
                f.path
                    == match kind {
                        0 => "queries.json",
                        1 => "fixture.mara.md",
                        _ => ".mara/schema.yaml",
                    }
            })
            .unwrap();
        file.content.push('\n');
        assert_eq!(
            SourceEngine::new(s)
                .unwrap()
                .query_execute(req.clone())
                .unwrap_err()
                .code,
            "invalid_argument"
        );
    }
    let mut changed = req.clone();
    changed.parameters[0].value = q::ParameterValue::Text("draft".into());
    assert_eq!(
        engine.query_execute(changed).unwrap_err().code,
        "invalid_argument"
    );
    let mut changed = req.clone();
    changed.max_rows = 99;
    assert!(engine.query_execute(changed).is_err());
    let mut changed = req;
    changed.limit = 2;
    assert!(engine.query_execute(changed).is_err());
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn id_fallback_and_forward_queries_do_not_manufacture_mids_or_optional_matches() {
    let mut s = snapshot();
    let file = s
        .files
        .iter_mut()
        .find(|f| f.path == "fixture.mara.md")
        .unwrap();
    file.content = file
        .content
        .replace(":mid: 01ARZ3NDEKTSV4RRFFQ69G5F02\n", "");
    let engine = SourceEngine::new(s).unwrap();
    let full = engine.query_execute(request()).unwrap();
    assert!(full.evaluation_complete, "{full:#?}");
    assert_eq!(full.summary.binding_count, Some(3));
    assert!(full.records.iter().any(|r| matches!(r,Record::Binding { binding,.. } if matches!(&binding.values["support"],Some(Value::Item { endpoint:q::Endpoint::Item { id,mid:None },.. }) if id=="SUP-A"))));
    let mut req = request();
    req.definition = "urn:fixture:query:forward".into();
    req.parameters.clear();
    req.selection = q::Selection {
        ids: vec!["SUP-A".into()],
        ..Default::default()
    };
    let forward = engine.query_execute(req).unwrap();
    assert_eq!(forward.summary.binding_count, Some(1));
    let mut req = request();
    req.definition = "urn:fixture:query:optional".into();
    req.parameters.clear();
    req.selection.ids = vec!["SUB-EMPTY".into()];
    let optional = engine.query_execute(req).unwrap();
    assert_eq!(optional.summary.binding_count, Some(0));
}

// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
#[test]
fn explicit_projected_support_combines_even_when_optional_values_are_absent() {
    let mut s = snapshot();
    mutate_definition(&mut s, |defs| {
        defs["definitions"][0]["parameters"] = json!({});
        defs["definitions"][0]["columns"] = json!({"title":{"type":"text"}});
        defs["definitions"][0]["support_columns"] = json!(["edge"]);
        defs["definitions"][0]["source_columns"] = json!(["source_ref"]);
        defs["definitions"][0]["select"] = json!(
            "SELECT ?root ?title ?source_ref ?edge WHERE { ?root <urn:mara:item:title> ?title ; <urn:mara:relation:supported_by> ?support . ?support <urn:mara:field:status> \"accepted\" . ?edge <urn:mara:query:source> ?support ; <urn:mara:query:target> ?root ; <urn:mara:query:relation> <urn:mara:relation:supports> . BIND(?support AS ?source_ref) }"
        );
    });
    let mut req = request();
    req.parameters.clear();
    let page = SourceEngine::new(s.clone())
        .unwrap()
        .query_execute(req.clone())
        .unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    assert_eq!(page.summary.binding_count, Some(1));
    let binding = page
        .records
        .iter()
        .find_map(|r| match r {
            Record::Binding { binding, .. } => Some(binding),
            _ => None,
        })
        .unwrap();
    assert_eq!(binding.support.len(), 1);
    assert!(
        matches!(&binding.support[0].edge.source, q::Endpoint::Item { id, .. } if id == "SUP-B")
    );
    mutate_definition(&mut s, |defs| {
        defs["definitions"][0]["columns"] = json!({"absent":{"type":"text","optional":true}});
        defs["definitions"][0]["select"] = json!(
            "SELECT ?root ?absent ?source_ref ?edge WHERE { ?root <urn:mara:relation:supported_by> ?support . ?edge <urn:mara:query:source> ?support ; <urn:mara:query:target> ?root ; <urn:mara:query:relation> <urn:mara:relation:supports> . BIND(?support AS ?source_ref) OPTIONAL { ?support <urn:mara:field:absent> ?absent } }"
        );
    });
    let page = SourceEngine::new(s).unwrap().query_execute(req).unwrap();
    assert_eq!(page.summary.binding_count, Some(1));
    let binding = page
        .records
        .iter()
        .find_map(|r| match r {
            Record::Binding { binding, .. } => Some(binding),
            _ => None,
        })
        .unwrap();
    assert!(binding.values["absent"].is_none());
    assert_eq!(binding.support.len(), 3);
    assert_eq!(binding.contributors.len(), 3);
}

// @mara checks DES-QUERY-CONSUMER-CONTRACT
#[test]
fn unavailable_code_is_not_empty_and_cycle_policy_does_not_hide_facts() {
    let mut s = snapshot();
    let file = s
        .files
        .iter_mut()
        .find(|f| f.path == ".mara/schema.yaml")
        .unwrap();
    let mut schema: serde_json::Value = serde_json::from_str(&file.content).unwrap();
    schema["relations"]["code_support"] = json!({"description":"Fixture code support","source":[],"target":["subject"],"code_source":true,"inverse":"code_supported_by"});
    schema["relations"]["parent"]["acyclic"] = json!({"severity":"error"});
    file.content = schema.to_string();
    mutate_definition(&mut s, |defs| {
        defs["definitions"][0]["columns"] = json!({"code":{"type":"code"}});
        defs["definitions"][0]["required_fact_sources"] = json!(["documents", "code"]);
        defs["definitions"][0]["support_columns"] = json!([]);
        defs["definitions"][0]["source_columns"] = json!([]);
        defs["definitions"][0]["parameters"] = json!({});
        defs["definitions"][0]["select"] =
            json!("SELECT ?root ?code WHERE { ?root ^<urn:mara:relation:code_support> ?code }");
    });
    let engine = SourceEngine::new(s).unwrap();
    let mut req = request();
    req.parameters.clear();
    let code = engine.query_execute(req.clone()).unwrap();
    assert!(!code.evaluation_complete);
    assert_eq!(code.summary.selected_roots, 2, "{code:#?}");
    assert_eq!(code.summary.binding_count, None);
    assert!(
        code.unavailable_reason
            .unwrap()
            .contains("required code facts")
    );
    req.definition = "urn:fixture:query:ancestry".into();
    let ancestry = engine.query_execute(req).unwrap();
    assert!(ancestry.evaluation_complete);
    assert_eq!(ancestry.summary.binding_count, Some(3));
    let validation = engine
        .project_validate(&[], &Default::default(), None)
        .unwrap();
    assert!(!validation.valid);
    assert!(
        validation
            .diagnostics
            .iter()
            .any(|d| d.code == mara_engine::DiagnosticCode::RelationCycle)
    );
}
