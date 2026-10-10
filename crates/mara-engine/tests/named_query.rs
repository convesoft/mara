use mara_engine::{
    SourceEngine, SourceSnapshot,
    named_query::{self as q, Record, Value},
};
use serde_json::json;
use std::collections::BTreeSet;
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
        fact_snapshots: vec![],
        mappings: vec![],
    }
}
fn mutate_file(s: &mut SourceSnapshot, path: &str, f: impl FnOnce(&mut serde_json::Value)) {
    let file = s.files.iter_mut().find(|f| f.path == path).unwrap();
    let mut definitions: serde_json::Value = serde_json::from_str(&file.content).unwrap();
    f(&mut definitions);
    file.content = definitions.to_string();
}
fn mutate_definition(s: &mut SourceSnapshot, f: impl FnOnce(&mut serde_json::Value)) {
    mutate_file(s, "queries.json", f);
}
fn add_definition_file(s: &mut SourceSnapshot, path: &str, content: &str) {
    s.files.push(mara_engine::SourceFile {
        path: path.into(),
        content: content.into(),
    });
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
    let mut non_text_normalization = base.clone();
    mutate_definition(&mut non_text_normalization, |defs| {
        let select = defs["definitions"][0]["select"].as_str().unwrap();
        defs["definitions"][0]["select"] = json!(select.replace(
            "f:text ?text",
            "<urn:mara:query:nfc-casefold:field:amount> ?text"
        ));
    });
    let error = SourceEngine::new(non_text_normalization)
        .unwrap()
        .query_execute(request())
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(error.message.contains("string or enum"), "{error:?}");

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

// @mara checks REQ-COMPOSABLE-QUERY-PROJECTIONS
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn named_composition_joins_typed_child_bindings_and_retains_projected_support() {
    let mut s = snapshot();
    add_definition_file(
        &mut s,
        "composition.json",
        include_str!("../../../tests/fixtures/named-query/composition.json"),
    );
    let engine = SourceEngine::new(s.clone()).unwrap();
    let mut req = request();
    req.definition_files = vec!["composition.json".into()];
    req.definition = "urn:fixture:query:composed-support-status".into();
    req.parameters.clear();
    let full = engine.query_execute(req.clone()).unwrap();
    assert!(full.evaluation_complete, "{full:#?}");
    assert_eq!(full.summary.selected_roots, 2);
    assert_eq!(full.summary.binding_count, Some(1));
    let mut matched_support = false;
    assert!(full.records.iter().any(|record| matches!(record,
        Record::Root { root: q::Endpoint::Item { id, .. }, binding_count: Some(0), .. }
        if id == "SUB-EMPTY")));
    for record in &full.records {
        let Record::Binding { binding, .. } = record else {
            continue;
        };
        assert!(matches!(&binding.values["support"], Some(Value::Item {
            endpoint: q::Endpoint::Item { id, .. }, ..
        }) if id == "SUP-A"));
        assert!(
            matches!(&binding.values["lifecycle"], Some(Value::Text { value, .. }) if value == "draft")
        );
        assert_eq!(binding.support.len(), 1);
        assert!(!binding.support[0].occurrences.is_empty());
        matched_support = true;
    }
    assert!(matched_support);

    let mut row_identity = s.clone();
    mutate_file(&mut row_identity, "composition.json", |defs| {
        defs["definitions"][2]["select"] = json!(
            "SELECT ?root ?support ?lifecycle ?edge WHERE { ?root <urn:mara:query:input:supports> ?support_row ; <urn:mara:query:input:status> ?status_row . FILTER(sameTerm(?support_row, ?status_row)) ?support_row <urn:mara:query:column:supports:support> ?support ; <urn:mara:query:column:supports:edge> ?edge . ?status_row <urn:mara:query:column:status:state> ?lifecycle }"
        );
    });
    let mut row_identity_request = request();
    row_identity_request.definition_files = vec!["composition.json".into()];
    row_identity_request.definition = "urn:fixture:query:composed-support-status".into();
    row_identity_request.parameters.clear();
    row_identity_request.selection.ids = vec!["SUB-LINKED".into()];
    let row_identity_result = SourceEngine::new(row_identity)
        .unwrap()
        .query_execute(row_identity_request)
        .unwrap();
    assert!(row_identity_result.evaluation_complete);
    assert_eq!(row_identity_result.summary.binding_count, Some(0));

    req.limit = 1;
    let first = engine.query_execute(req.clone()).unwrap();
    assert!(first.has_more);
    let cursor = first.next_cursor.clone().unwrap();
    let mut records = first.records;
    req.cursor = Some(cursor.clone());
    loop {
        let page = engine.query_execute(req.clone()).unwrap();
        assert_eq!(page.summary.binding_count, Some(1));
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

    let mut changed = s;
    changed
        .files
        .iter_mut()
        .find(|file| file.path == "composition.json")
        .unwrap()
        .content
        .push(' ');
    let mut stale = request();
    stale.definition_files = vec!["composition.json".into()];
    stale.definition = "urn:fixture:query:composed-support-status".into();
    stale.parameters.clear();
    stale.limit = 1;
    stale.cursor = Some(cursor);
    assert_eq!(
        SourceEngine::new(changed)
            .unwrap()
            .query_execute(stale)
            .unwrap_err()
            .code,
        "invalid_argument"
    );
}

// @mara checks REQ-COMPOSABLE-QUERY-PROJECTIONS
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
#[test]
fn explicit_left_join_keeps_an_absent_optional_binding_for_a_matched_left_row() {
    let mut s = snapshot();
    add_definition_file(
        &mut s,
        "composition.json",
        include_str!("../../../tests/fixtures/named-query/composition.json"),
    );
    let engine = SourceEngine::new(s).unwrap();
    let mut req = request();
    req.definition_files = vec!["composition.json".into()];
    req.definition = "urn:fixture:query:left-join-absence".into();
    req.parameters.clear();
    req.selection.ids = vec!["SUB-EMPTY".into()];
    let page = engine.query_execute(req).unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    assert_eq!(page.summary.binding_count, Some(1));
    assert!(page.records.iter().any(|record| matches!(record,
        Record::Binding { binding, .. }
        if binding.values["support"].is_none() && binding.support.is_empty())));
}

// @mara checks REQ-COMPOSABLE-QUERY-PROJECTIONS
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
#[test]
fn direct_or_design_mediated_coverage_is_distinct_and_keeps_both_support_edges() {
    let mut s = snapshot();
    add_definition_file(
        &mut s,
        "composition.json",
        include_str!("../../../tests/fixtures/named-query/composition.json"),
    );
    let engine = SourceEngine::new(s).unwrap();
    let mut req = request();
    req.definition_files = vec!["composition.json".into()];
    req.definition = "urn:fixture:query:direct-or-design-coverage".into();
    req.parameters.clear();
    req.selection.ids = vec!["SUB-LINKED".into()];
    let page = engine.query_execute(req).unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    assert_eq!(page.summary.binding_count, Some(3));
    let mut support_b_edges = 0;
    let mut lifecycles = vec![];
    for record in page.records {
        let Record::Binding { binding, .. } = record else {
            continue;
        };
        let Some(Value::Item {
            endpoint: q::Endpoint::Item { id, .. },
            ..
        }) = &binding.values["support"]
        else {
            panic!("typed support")
        };
        if id == "SUP-B" {
            support_b_edges = binding.support.len();
        }
        let Some(Value::Text { value, .. }) = &binding.values["lifecycle"] else {
            panic!("lifecycle-bearing alternative")
        };
        lifecycles.push(value.clone());
        assert!(!binding.support.is_empty());
        assert!(
            binding
                .support
                .iter()
                .all(|support| !support.occurrences.is_empty())
        );
    }
    assert_eq!(
        support_b_edges, 2,
        "direct and design-mediated canonical edges survive distinct projection"
    );
    lifecycles.sort();
    assert_eq!(lifecycles, ["accepted", "draft", "retired"]);
}

// @mara checks REQ-COMPOSABLE-QUERY-PROJECTIONS
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
#[test]
fn transitive_decomposition_supersession_and_ancestry_terminate_and_exclude_seed_siblings() {
    let mut s = snapshot();
    add_definition_file(
        &mut s,
        "transitive.json",
        include_str!("../../../tests/fixtures/named-query/transitive.json"),
    );
    let engine = SourceEngine::new(s).unwrap();
    let mut req = request();
    req.definition_files = vec!["transitive.json".into()];
    req.definition = "urn:fixture:query:decomposition-or-supersession".into();
    req.parameters.clear();
    req.selection.ids = vec!["SUB-LINKED".into()];
    let composed = engine.query_execute(req).unwrap();
    assert!(composed.evaluation_complete, "{composed:#?}");
    assert_eq!(composed.summary.binding_count, Some(3));
    let mut members = composed
        .records
        .iter()
        .filter_map(|record| match record {
            Record::Binding { binding, .. } => match &binding.values["member"] {
                Some(Value::Item {
                    endpoint: q::Endpoint::Item { id, .. },
                    ..
                }) => Some(id.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    members.sort();
    assert_eq!(members, ["SUB-A", "SUB-B", "SUB-G"]);

    let mut ancestry = request();
    ancestry.definition = "urn:fixture:query:ancestry".into();
    ancestry.parameters.clear();
    ancestry.selection.ids = vec!["SUB-LINKED".into()];
    let page = engine.query_execute(ancestry.clone()).unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    assert_eq!(page.summary.binding_count, Some(3));
    let ancestors = page
        .records
        .iter()
        .filter_map(|record| match record {
            Record::Binding { binding, .. } => match &binding.values["ancestor"] {
                Some(Value::Item {
                    endpoint: q::Endpoint::Item { id, .. },
                    ..
                }) => Some(id.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(ancestors, ["SUB-A", "SUB-B", "SUB-G"].into_iter().collect());
    ancestry.max_rows = 1;
    let limited = engine.query_execute(ancestry).unwrap();
    assert!(!limited.evaluation_complete);
    assert_eq!(limited.summary.binding_count, None);
    assert_eq!(limited.summary.known_bindings, 1);
    assert!(
        limited
            .records
            .iter()
            .any(|record| matches!(record, Record::Binding { .. }))
    );
}

// @mara checks REQ-COMPOSABLE-QUERY-PROJECTIONS
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
#[test]
fn normalized_title_alias_collisions_are_exact_not_fuzzy() {
    let mut s = snapshot();
    add_definition_file(
        &mut s,
        "normalized.json",
        include_str!("../../../tests/fixtures/named-query/normalized.json"),
    );
    let engine = SourceEngine::new(s.clone()).unwrap();
    let mut req = request();
    req.definition_files = vec!["normalized.json".into()];
    req.definition = "urn:fixture:query:title-alias-collisions".into();
    req.parameters.clear();
    req.selection.ids = vec!["SUP-A".into(), "SUP-C".into()];
    let page = engine.query_execute(req.clone()).unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    assert_eq!(page.summary.binding_count, Some(2));
    let matches = page
        .records
        .iter()
        .filter_map(|record| match record {
            Record::Binding {
                root: q::Endpoint::Item { id: root, .. },
                binding,
            } => {
                let Some(Value::Item {
                    endpoint: q::Endpoint::Item { id, .. },
                    ..
                }) = &binding.values["collision"]
                else {
                    panic!("typed collision")
                };
                Some((root.as_str(), id.as_str()))
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        matches,
        [("SUP-A", "SUP-X"), ("SUP-C", "SUP-Y")]
            .into_iter()
            .collect()
    );

    let docs = s
        .files
        .iter_mut()
        .find(|file| file.path == "fixture.mara.md")
        .unwrap();
    docs.content = docs.content.replace(":alias: Straße", ":alias: Strase");
    let mut fuzzy = req;
    fuzzy.selection.ids = vec!["SUP-A".into()];
    let exact = SourceEngine::new(s).unwrap().query_execute(fuzzy).unwrap();
    assert!(exact.evaluation_complete);
    assert_eq!(exact.summary.binding_count, Some(0));
}

// @mara checks REQ-COMPOSABLE-QUERY-PROJECTIONS
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
// @mara checks REQ-COMPOSABLE-QUERY-PROJECTIONS
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
#[test]
fn parent_parameter_can_be_consumed_only_by_a_mapped_child() {
    let mut source = snapshot();
    add_definition_file(
        &mut source,
        "composition.json",
        include_str!("../../../tests/fixtures/named-query/composition.json"),
    );
    let mut req = request();
    req.definition_files = vec!["composition.json".into()];
    req.definition = "urn:fixture:query:forwarded-status".into();
    req.selection.ids = vec!["SUB-LINKED".into(), "SUB-EMPTY".into()];
    req.parameters = vec![q::BindingParameter {
        name: "filter_state".into(),
        value: q::ParameterValue::Text("draft".into()),
    }];
    let page = SourceEngine::new(source)
        .unwrap()
        .query_execute(req)
        .unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    assert_eq!(page.summary.binding_count, Some(1));
    assert!(page.records.iter().any(|record| matches!(record,
        Record::Binding { root: q::Endpoint::Item { id, .. }, binding }
        if id == "SUB-LINKED"
            && matches!(&binding.values["state"], Some(Value::Text { value, .. }) if value == "draft")
    )));
}

#[test]
fn definition_dependencies_are_resolved_and_acyclic() {
    let mut base = snapshot();
    add_definition_file(
        &mut base,
        "composition.json",
        include_str!("../../../tests/fixtures/named-query/composition.json"),
    );
    let mut req = request();
    req.definition_files = vec!["composition.json".into()];
    req.definition = "urn:fixture:query:composed-support-status".into();
    req.parameters.clear();

    let mut unknown = base.clone();
    mutate_file(&mut unknown, "composition.json", |defs| {
        defs["definitions"][2]["dependencies"][0]["definition"] = json!("urn:missing:query");
    });
    let error = SourceEngine::new(unknown)
        .unwrap()
        .query_execute(req.clone())
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert_eq!(error.source.as_deref(), Some("composition.json"));

    let mut unused_unknown = base.clone();
    mutate_file(&mut unused_unknown, "composition.json", |defs| {
        defs["definitions"][1]["dependencies"] = json!([{
            "name":"unused", "definition":"urn:missing:query", "root":"root", "projections":{}
        }]);
    });
    let mut unused_request = req.clone();
    unused_request.definition = "urn:fixture:query:direct-coverage".into();
    let error = SourceEngine::new(unused_unknown)
        .unwrap()
        .query_execute(unused_request.clone())
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(
        error.message.contains("unknown named dependency"),
        "{error:?}"
    );

    let mut unused_projection = base.clone();
    mutate_file(&mut unused_projection, "composition.json", |defs| {
        defs["definitions"][1]["dependencies"] = json!([{
            "name":"unused", "definition":"urn:fixture:query:support-links", "root":"root",
            "projections":{"columns":{"missing":"value"}}
        }]);
    });
    let error = SourceEngine::new(unused_projection)
        .unwrap()
        .query_execute(unused_request.clone())
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(error.message.contains("no column missing"), "{error:?}");

    let mut cyclic = base.clone();
    mutate_file(&mut cyclic, "composition.json", |defs| {
        defs["definitions"][1]["dependencies"] = json!([{
            "name":"loop", "definition":"urn:fixture:query:root-status", "root":"root", "projections":{}
        }]);
    });
    let error = SourceEngine::new(cyclic)
        .unwrap()
        .query_execute(req)
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(error.message.contains("recursive"), "{error:?}");
    assert_eq!(error.source.as_deref(), Some("composition.json"));

    let mut unused_cycle = base;
    mutate_file(&mut unused_cycle, "composition.json", |defs| {
        defs["definitions"][1]["dependencies"] = json!([{
            "name":"loop", "definition":"urn:fixture:query:root-status", "root":"root", "projections":{}
        }]);
    });
    let error = SourceEngine::new(unused_cycle)
        .unwrap()
        .query_execute(unused_request)
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(error.message.contains("recursive"), "{error:?}");
}
