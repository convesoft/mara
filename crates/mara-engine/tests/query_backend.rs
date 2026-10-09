use mara_engine::{
    SourceEngine, SourceSnapshot,
    query_backend::{PolicyRequest, Request, TypedValue},
};
use serde_json::{Value, json};

fn snapshot() -> SourceSnapshot {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/query-backend/snapshot.json"
    ))
    .unwrap()
}
fn request() -> Request {
    Request {
        definition_file: "supports.query.json".into(),
        roots: vec![
            "SUB-LINKED".into(),
            "SUB-EMPTY".into(),
            "01ARZ3NDEKTSV4RRFFQ69G5F00".into(),
        ],
        max_rows: 100,
    }
}
fn policy(query: Request) -> PolicyRequest {
    PolicyRequest {
        query,
        shape_file: "coverage.ttl".into(),
        shape: "urn:fixture:policy:coverage".into(),
    }
}

// @mara checks DES-QUERY-POLICY-ENGINE
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
#[test]
fn shared_definition_returns_facts_and_real_shacl_qualification() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let raw = engine.query_backend_execute(request()).unwrap();
    let evaluation = engine.query_backend_policy(policy(request())).unwrap();
    assert_eq!(
        serde_json::to_value(&raw).unwrap(),
        serde_json::to_value(&evaluation.facts).unwrap()
    );
    assert!(raw.evaluation_complete && evaluation.evaluation_complete);
    assert_eq!(raw.roots.len(), 2); // ID/MID aliases select one root.
    assert_eq!(raw.roots[0].bindings.len(), 3);
    assert!(raw.roots[1].bindings.is_empty());
    let mut lifecycle = vec![];
    for binding in &raw.roots[0].bindings {
        let v = serde_json::to_value(&binding.values).unwrap();
        lifecycle.push(v["lifecycle"]["value"].as_str().unwrap().to_owned());
        assert_eq!(
            v["amount"]["datatype"],
            "http://www.w3.org/2001/XMLSchema#integer"
        );
        assert_eq!(
            v["text"]["datatype"],
            "http://www.w3.org/2001/XMLSchema#string"
        );
        assert_eq!(
            v["flag"]["datatype"],
            "http://www.w3.org/2001/XMLSchema#boolean"
        );
        assert_eq!(binding.support.len(), 1);
        assert!(
            binding.support[0]
                .occurrences
                .iter()
                .all(|o| o.source.path().to_str() == Some("fixture.mara.md"))
        );
    }
    lifecycle.sort();
    assert_eq!(lifecycle, ["accepted", "draft", "retired"]);
    assert_eq!(
        raw.roots[0]
            .bindings
            .iter()
            .map(|b| b.support[0].occurrences.len())
            .sum::<usize>(),
        5
    );
    assert_eq!(evaluation.checks[0].state, "passed");
    assert_eq!(evaluation.checks[1].state, "failed");
    assert!(
        evaluation.checks[0]
            .counts
            .iter()
            .any(|c| c.total == 3 && c.qualifying == Some(1))
    );
    assert!(
        evaluation.checks[1]
            .counts
            .iter()
            .any(|c| c.total == 0 && c.qualifying == Some(0))
    );
    assert!(
        evaluation.checks[1]
            .components
            .iter()
            .any(|c| c.ends_with("QualifiedMinCountConstraintComponent"))
    );
    assert_eq!(evaluation.checks[1].shape_file, "coverage.ttl");
    // Standard Rust test harness retains the actual native result for browser parity.
    if let Ok(path) = std::env::var("MARA_QUERY_PROOF_REPORT") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&json!({"raw":raw,"policy":evaluation,
                "limited": engine.query_backend_policy(policy(Request { max_rows: 1, ..request() })).unwrap(),
                "regressions": regression_report()
            })).unwrap(),
        )
        .unwrap();
    }
}

// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
// @mara checks DES-QUERY-CONSUMER-CONTRACT
#[test]
fn explicit_empty_unknown_roots_and_incomplete_budget_are_not_policy_success() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let mut empty = request();
    empty.roots.clear();
    let facts = engine.query_backend_execute(empty).unwrap();
    assert!(facts.evaluation_complete && facts.roots.is_empty());
    let mut missing_shape = policy(Request {
        roots: vec![],
        ..request()
    });
    missing_shape.shape = "urn:fixture:policy:missing".into();
    assert!(
        engine
            .query_backend_policy(missing_shape)
            .unwrap_err()
            .contains("unknown SHACL shape")
    );
    let mut unknown = request();
    unknown.roots = vec!["SUB-NOPE".into()];
    assert!(engine.query_backend_execute(unknown).is_err());
    let mut limited = request();
    limited.max_rows = 1;
    let result = engine.query_backend_policy(policy(limited)).unwrap();
    assert!(!result.evaluation_complete);
    assert!(result.facts.unavailable_reason.is_some());
    assert!(
        result
            .checks
            .iter()
            .all(|c| c.state == "unavailable" && c.counts.is_empty())
    );
    assert_eq!(result.facts.roots.len(), 2);
}

// @mara checks DES-QUERY-POLICY-ENGINE
#[test]
fn unsupported_service_and_coercing_equality_are_rejected() {
    for select in [
        "SELECT ?root ?support ?lifecycle ?amount ?text ?flag ?edge WHERE { SERVICE SILENT <http://127.0.0.1:1/> { ?root ?p ?support } }",
        "SELECT ?root ?support ?lifecycle ?amount ?text ?flag ?edge WHERE { FILTER(1 = 1.0) }",
    ] {
        let mut fixture = snapshot();
        let file = fixture
            .files
            .iter_mut()
            .find(|f| f.path == "supports.query.json")
            .unwrap();
        let mut definition: Value = serde_json::from_str(&file.content).unwrap();
        definition["select"] = json!(select);
        file.content = definition.to_string();
        let engine = SourceEngine::new(fixture).unwrap();
        assert!(engine.query_backend_execute(request()).is_err());
    }
}

fn regression_report() -> Value {
    let original = snapshot();
    let cases: Value = serde_json::from_str(
        &original
            .files
            .iter()
            .find(|f| f.path == "regressions.json")
            .unwrap()
            .content,
    )
    .unwrap();
    let mut report = serde_json::Map::new();
    for variant in cases.as_array().unwrap() {
        let mut fixture = original.clone();
        fixture
            .files
            .iter_mut()
            .find(|f| f.path == "supports.query.json")
            .unwrap()
            .content = variant["definition_text"].as_str().unwrap().into();
        if let Some(shape) = variant["shape_text"].as_str() {
            fixture
                .files
                .iter_mut()
                .find(|f| f.path == "coverage.ttl")
                .unwrap()
                .content = shape.into();
        }
        let engine = SourceEngine::new(fixture).unwrap();
        let outcome = if variant["reject"] == true {
            let outcomes: Vec<_> = [vec!["SUB-LINKED".into()], vec![]]
                .into_iter()
                .map(|roots| {
                    engine
                        .query_backend_execute(Request { roots, ..request() })
                        .is_err()
                })
                .collect();
            assert_eq!(outcomes, [true, true]);
            json!({"rejected":outcomes})
        } else {
            let evaluation = engine.query_backend_policy(policy(request())).unwrap();
            let repeated = engine.query_backend_policy(policy(request())).unwrap();
            let value = serde_json::to_value(&evaluation).unwrap();
            assert_eq!(value, serde_json::to_value(&repeated).unwrap());
            assert!(evaluation.evaluation_complete);
            if variant["name"] == "supported_optional_absence" {
                assert_eq!(evaluation.facts.roots[0].bindings.len(), 1);
                assert!(evaluation.facts.roots[0].bindings[0].values["absent"].is_none());
                assert_eq!(evaluation.facts.roots[0].bindings[0].support.len(), 3);
                assert_eq!(evaluation.checks[0].state, "passed");
                assert_eq!(evaluation.checks[1].state, "failed");
                assert!(evaluation.checks[0].counts.iter().any(|c| c.total == 1));
            } else if variant["name"] == "row_identity_collision" {
                assert_eq!(evaluation.facts.roots[0].bindings.len(), 2);
                assert!(
                    evaluation
                        .checks
                        .iter()
                        .all(|c| c.state == "failed" && c.counts.iter().any(|n| n.total == 0))
                );
            }
            if variant["name"] == "canonical_literal_identity" {
                assert_eq!(evaluation.facts.roots[0].bindings.len(), 4);
                let datatypes: std::collections::BTreeSet<_> = evaluation.facts.roots[0]
                    .bindings
                    .iter()
                    .map(|b| {
                        let Some(TypedValue::Literal { datatype, .. }) = &b.values["value"] else {
                            panic!("literal expected")
                        };
                        datatype.as_str()
                    })
                    .collect();
                assert_eq!(datatypes.len(), 4);
                assert!(evaluation.checks.iter().all(|c| c.state == "passed"));
            }
            if variant["name"] == "canonical_same_term" {
                assert_eq!(evaluation.facts.roots[0].bindings.len(), 1);
                assert!(
                    matches!(&evaluation.facts.roots[0].bindings[0].values["value"], Some(TypedValue::Literal { datatype, value, .. }) if datatype.ends_with("#integer") && value == "1")
                );
                assert!(evaluation.checks.iter().all(|c| c.state == "passed"));
            }
            json!({"evaluation":evaluation, "repeated_identical":true})
        };
        report.insert(variant["name"].as_str().unwrap().into(), outcome);
    }
    let engine = SourceEngine::new(original).unwrap();
    let budget_max = engine
        .query_backend_execute(Request {
            max_rows: u32::MAX,
            ..request()
        })
        .unwrap();
    assert!(budget_max.evaluation_complete);
    let mut oversized = serde_json::to_value(request()).unwrap();
    oversized["max_rows"] = json!(u64::from(u32::MAX) + 1);
    let rejected = serde_json::from_value::<Request>(oversized).is_err();
    assert!(rejected);
    report.insert(
        "portable_budget".into(),
        json!({"at_max":budget_max,"above_max_rejected":rejected}),
    );
    Value::Object(report)
}

// @mara checks DES-QUERY-BACKEND-PROOF
#[test]
fn admitted_nullable_projections_and_anonymous_policy_counts_are_stable() {
    regression_report();
}
