use mara_engine::{SourceEngine, SourceSnapshot, named_query as q};
use serde_json::{Value, json};

fn snapshot(select: &str, columns: Value) -> SourceSnapshot {
    let mut snapshot: SourceSnapshot = serde_json::from_str(include_str!(
        "../../../tests/fixtures/named-query/snapshot.json"
    ))
    .unwrap();
    snapshot
        .files
        .iter_mut()
        .find(|file| file.path == "queries.json")
        .unwrap()
        .content = json!({
        "format_version": 3,
        "definitions": [{
            "id": "review",
            "required_fact_sources": ["documents"],
            "root_flavours": ["subject"],
            "columns": columns,
            "select": select
        }]
    })
    .to_string();
    snapshot
}

fn request() -> q::Request {
    q::Request {
        definition_files: vec!["queries.json".into()],
        definition: "review".into(),
        selection: q::Selection {
            ids: vec!["SUB-LINKED".into()],
            ..Default::default()
        },
        parameters: vec![],
        max_rows: 100,
        limit: 100,
        cursor: None,
        fact_snapshots: vec![],
        mappings: vec![],
    }
}

fn set_definition(snapshot: &mut SourceSnapshot, definition: Value) {
    let file = snapshot
        .files
        .iter_mut()
        .find(|file| file.path == "queries.json")
        .unwrap();
    file.content = json!({
        "format_version": 3,
        "definitions": [definition]
    })
    .to_string();
}

fn update_schema(snapshot: &mut SourceSnapshot, change: impl FnOnce(&mut Value)) {
    let file = snapshot
        .files
        .iter_mut()
        .find(|file| file.path == ".mara/schema.yaml")
        .unwrap();
    let mut schema: Value = serde_json::from_str(&file.content).unwrap();
    change(&mut schema);
    file.content = schema.to_string();
}

#[test]
fn format_three_definitions_reject_legacy_format_two() {
    let mut fixture_snapshot = snapshot(
        "SELECT ?root ?title WHERE { ?root <urn:mara:item:title> ?title }",
        json!({"title":{"type":"text"}}),
    );
    let file = fixture_snapshot
        .files
        .iter_mut()
        .find(|file| file.path == "queries.json")
        .unwrap();
    let mut definitions: Value = serde_json::from_str(&file.content).unwrap();
    definitions["format_version"] = json!(2);
    file.content = definitions.to_string();
    let error = SourceEngine::new(fixture_snapshot)
        .unwrap()
        .query_execute(request())
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
}

#[test]
fn unknown_schema_predicates_are_rejected_even_with_empty_roots() {
    for predicate in ["urn:mara:field:statuz", "urn:unknown:predicate"] {
        let mut req = request();
        req.selection = q::Selection {
            flavours: vec!["subject".into()],
            fields: vec![q::FieldSelection {
                key: "status".into(),
                value: "never".into(),
            }],
            ..Default::default()
        };
        let error = SourceEngine::new(snapshot(
            &format!("SELECT ?root ?value WHERE {{ ?root <{predicate}> ?value }}"),
            json!({"value":{"type":"text"}}),
        ))
        .unwrap()
        .query_execute(req)
        .unwrap_err();
        assert_eq!(error.code, "invalid_definition", "{predicate}");
        assert!(error.message.contains("unknown schema"), "{error:?}");
    }
}

#[test]
fn returned_types_and_references_are_checked_without_preflighting_empty_results() {
    let empty = SourceEngine::new(snapshot(
        "SELECT ?root ?value WHERE { ?root <urn:mara:field:amount> ?value }",
        json!({"value":{"type":"text"}}),
    ))
    .unwrap()
    .query_execute(request())
    .unwrap();
    assert!(empty.evaluation_complete, "{empty:#?}");
    assert_eq!(empty.summary.binding_count, Some(0));

    let mut actual_value = snapshot(
        "SELECT ?root ?value WHERE { ?root <urn:mara:field:amount> ?value }",
        json!({"value":{"type":"text"}}),
    );
    let file = actual_value
        .files
        .iter_mut()
        .find(|file| file.path == "queries.json")
        .unwrap();
    let mut set: Value = serde_json::from_str(&file.content).unwrap();
    set["definitions"][0]["root_flavours"] = json!(["support"]);
    file.content = set.to_string();
    let mut req = request();
    req.selection.ids = vec!["SUP-A".into()];
    let error = SourceEngine::new(actual_value)
        .unwrap()
        .query_execute(req.clone())
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(error.message.contains("incompatible type"));
    let mut wrong_reference = snapshot(
        "SELECT ?root ?title ?source WHERE { ?root <urn:mara:item:title> ?title . BIND(<https://example.invalid/source> AS ?source) }",
        json!({"title":{"type":"text"}}),
    );
    let file = wrong_reference
        .files
        .iter_mut()
        .find(|file| file.path == "queries.json")
        .unwrap();
    let mut definition: Value = serde_json::from_str(&file.content).unwrap();
    definition["definitions"][0]["source_columns"] = json!(["source"]);
    file.content = definition.to_string();
    req.selection.ids = vec!["SUB-LINKED".into()];
    let error = SourceEngine::new(wrong_reference)
        .unwrap()
        .query_execute(req)
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(error.message.contains("canonical item"));
}

#[test]
fn declared_required_sources_not_query_inference_control_availability() {
    let mut source = snapshot(
        "SELECT ?root ?code WHERE { ?root ^<urn:mara:relation:code_support> ?code }",
        json!({"code":{"type":"code"}}),
    );
    update_schema(&mut source, |schema| {
        schema["relations"]["code_support"] = json!({
            "description":"Fixture code support",
            "source":[],
            "target":["subject"],
            "code_source":true,
            "inverse":"code_supported_by"
        });
    });
    let document_only = SourceEngine::new(source.clone())
        .unwrap()
        .query_execute(request())
        .unwrap();
    assert!(document_only.evaluation_complete, "{document_only:#?}");
    assert_eq!(document_only.summary.binding_count, Some(0));

    let mut definition = json!({
        "id":"review",
        "required_fact_sources":["documents", "code"],
        "root_flavours":["subject"],
        "columns":{"code":{"type":"code"}},
        "select":"SELECT ?root ?code WHERE { ?root ^<urn:mara:relation:code_support> ?code }"
    });
    set_definition(&mut source, definition.clone());
    let mut req = request();
    req.selection = q::Selection {
        flavours: vec!["subject".into()],
        fields: vec![q::FieldSelection {
            key: "status".into(),
            value: "not-present".into(),
        }],
        ..Default::default()
    };
    let unavailable = SourceEngine::new(source.clone())
        .unwrap()
        .query_execute(req.clone())
        .unwrap();
    assert!(!unavailable.evaluation_complete, "{unavailable:#?}");
    assert_eq!(unavailable.summary.selected_roots, 0);
    assert_eq!(unavailable.summary.binding_count, None);
    assert!(
        unavailable
            .unavailable_reason
            .unwrap()
            .contains("required code facts")
    );

    definition["required_fact_sources"] = json!(["documents"]);
    set_definition(&mut source, definition);
    let complete_empty = SourceEngine::new(source)
        .unwrap()
        .query_execute(req)
        .unwrap();
    assert!(complete_empty.evaluation_complete, "{complete_empty:#?}");
    assert_eq!(complete_empty.summary.binding_count, Some(0));

    let mut undeclared = snapshot(
        "SELECT ?root ?title WHERE { ?root <urn:mara:item:title> ?title }",
        json!({"title":{"type":"text"}}),
    );
    let file = undeclared
        .files
        .iter_mut()
        .find(|file| file.path == "queries.json")
        .unwrap();
    let mut set: Value = serde_json::from_str(&file.content).unwrap();
    set["definitions"][0]
        .as_object_mut()
        .unwrap()
        .remove("required_fact_sources");
    file.content = set.to_string();
    let error = SourceEngine::new(undeclared)
        .unwrap()
        .query_execute(request())
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
}

#[test]
fn only_explicitly_projected_support_is_resolved_and_combined() {
    let select = "SELECT ?root ?title ?source_ref ?edge WHERE { ?root <urn:mara:item:title> ?title ; <urn:mara:relation:supported_by> ?support . ?edge <urn:mara:query:source> ?support ; <urn:mara:query:target> ?root ; <urn:mara:query:relation> <urn:mara:relation:supports> . BIND(?support AS ?source_ref) }";
    let mut fixture_snapshot = snapshot(select, json!({"title":{"type":"text"}}));
    let file = fixture_snapshot
        .files
        .iter_mut()
        .find(|file| file.path == "queries.json")
        .unwrap();
    let mut set: Value = serde_json::from_str(&file.content).unwrap();
    set["definitions"][0]["source_columns"] = json!(["source_ref"]);
    set["definitions"][0]["support_columns"] = json!(["edge"]);
    file.content = set.to_string();
    let page = SourceEngine::new(fixture_snapshot)
        .unwrap()
        .query_execute(request())
        .unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    let binding = page
        .records
        .iter()
        .find_map(|record| match record {
            q::Record::Binding { binding, .. } => Some(binding),
            _ => None,
        })
        .unwrap();
    assert_eq!(binding.support.len(), 3);
    assert_eq!(binding.contributors.len(), 3);
    assert!(binding.contributors.iter().any(|value| matches!(
        value,
        q::Value::Item {
            endpoint: q::Endpoint::Item { id, .. },
            ..
        } if id == "SUP-A"
    )));
    assert!(
        binding
            .support
            .iter()
            .all(|support| !support.occurrences.is_empty())
    );

    let hidden = SourceEngine::new(snapshot(
        "SELECT ?root ?title WHERE { ?root <urn:mara:item:title> ?title . ?root <urn:mara:relation:supported_by> ?support }",
        json!({"title":{"type":"text"}}),
    ))
    .unwrap()
    .query_execute(request())
    .unwrap();
    let binding = hidden
        .records
        .iter()
        .find_map(|record| match record {
            q::Record::Binding { binding, .. } => Some(binding),
            _ => None,
        })
        .unwrap();
    assert!(binding.support.is_empty());
    assert!(binding.contributors.is_empty());
}

#[test]
fn exact_item_aliases_are_canonicalized_without_static_flavour_proofs() {
    for item_ref in [
        "<urn:mara:id:SUP-A>",
        "<urn:mara:mid:01ARZ3NDEKTSV4RRFFQ69G5F02>",
    ] {
        let page = SourceEngine::new(snapshot(
            &format!("SELECT ?root ?value WHERE {{ BIND({item_ref} AS ?value) }}"),
            json!({"value":{"type":"item","flavours":["support"]}}),
        ))
        .unwrap()
        .query_execute(request())
        .unwrap();
        assert!(page.evaluation_complete, "{page:#?}");
        let binding = page
            .records
            .iter()
            .find_map(|record| match record {
                q::Record::Binding { binding, .. } => Some(binding),
                _ => None,
            })
            .unwrap();
        assert!(matches!(
            &binding.values["value"],
            Some(q::Value::Item {
                endpoint: q::Endpoint::Item { id, .. },
                ..
            }) if id == "SUP-A"
        ));
    }
}
