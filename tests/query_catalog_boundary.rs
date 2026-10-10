//! Boundary regressions for the bounded catalog extension, not new analysis guarantees.
use mara::{SourceEngine, SourceSnapshot, named_query as q};
use serde_json::{Value, json};
fn snapshot() -> SourceSnapshot {
    serde_json::from_str(include_str!("fixtures/query-catalog/snapshot.json")).unwrap()
}
fn request() -> q::Request {
    serde_json::from_value(json!({"definition_files":["queries.json"],"definition":"urn:fixture:catalog:risk-score",
        "selection":{"ids":["CAND-C"]},"parameters":[{"name":"threshold","value":{"type":"integer","value":6}}],
        "max_rows":100,"limit":100})).unwrap()
}
// @mara checks DES-QUERY-NAMED-ADAPTER
// @mara checks DES-QUERY-BACKEND-PROOF
#[test]
fn proof_profile_stays_bounded_and_named_operands_keep_all_existing_checks() {
    let mut s = snapshot();
    s.files.push(mara::SourceFile {
        path: "proof.json".into(),
        content: json!({
            "format_version":1,"id":"multiply","select":"SELECT ?root ?value WHERE { BIND(2 * 3 AS ?value) }",
            "columns":["value"],"support_columns":[]
        })
        .to_string(),
    });
    let proof = mara::query_backend::Request {
        definition_file: "proof.json".into(),
        roots: vec!["CAND-C".into()],
        max_rows: 100,
    };
    let error = SourceEngine::new(s.clone())
        .unwrap()
        .query_backend_execute(proof.clone())
        .unwrap_err();
    assert!(
        error.contains("expression outside the exact typed proof profile"),
        "{error}"
    );
    let mut admitted = s.clone();
    let file = admitted
        .files
        .iter_mut()
        .find(|f| f.path == "proof.json")
        .unwrap();
    *file = mara::SourceFile {
        path: "proof.json".into(),
        content: file.content.replace("2 * 3", "2"),
    };
    let facts = SourceEngine::new(admitted)
        .unwrap()
        .query_backend_execute(proof)
        .unwrap();
    assert_eq!(facts.roots[0].bindings.len(), 1);
    // Each admitted operand must still validate nested vocabulary, dependency aliases,
    // item constants and signature boundaries before selecting roots.
    for operand in [
        "EXISTS { ?root <urn:mara:field:unknown> ?x }",
        "EXISTS { ?root <urn:mara:query:input:undeclared> ?x }",
        "<urn:mara:id:ABSENT>",
        "EXISTS { SELECT ?root WHERE { ?root <urn:mara:item:title> ?x } }",
    ] {
        let mut s = s.clone();
        let file = s
            .files
            .iter_mut()
            .find(|f| f.path == "queries.json")
            .unwrap();
        let mut set: Value = serde_json::from_str(&file.content).unwrap();
        let d = set["definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|d| d["id"] == "urn:fixture:catalog:risk-score")
            .unwrap();
        d["select"] = json!(format!(
            "SELECT ?root ?likelihood ?consequence ?score ?above WHERE {{ BIND(2 AS ?likelihood) BIND(3 AS ?consequence) BIND({operand} * 3 AS ?score) BIND(?score >= ?threshold AS ?above) }}"
        ));
        file.content = set.to_string();
        let mut r = request();
        r.selection.ids.clear();
        r.selection.fields.push(q::FieldSelection {
            key: "status".into(),
            value: "no-match".into(),
        });
        assert_eq!(
            SourceEngine::new(s)
                .unwrap()
                .query_execute(r)
                .unwrap_err()
                .code,
            "invalid_definition",
            "{operand}"
        );
    }
}
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn missing_source_inventory_and_missing_definition_source_are_not_empty_success() {
    let mut s = snapshot();
    s.selection
        .document_paths
        .push("unavailable.mara.md".into());
    let mut r = request();
    r.definition = "urn:fixture:catalog:inner-empty".into();
    r.parameters.clear();
    r.selection.ids.clear();
    r.selection.fields.push(q::FieldSelection {
        key: "status".into(),
        value: "no-match".into(),
    });
    let page = SourceEngine::new(s)
        .unwrap()
        .query_execute(r.clone())
        .unwrap();
    assert!(!page.evaluation_complete);
    assert!(page.summary.binding_count.is_none());
    let mut s = snapshot();
    s.files.retain(|f| f.path != "queries.json");
    let page = SourceEngine::new(s).unwrap().query_execute(r).unwrap();
    assert!(!page.evaluation_complete);
    assert!(
        page.unavailable_reason
            .unwrap()
            .contains("missing definition source")
    );
}
