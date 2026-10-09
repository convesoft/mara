//! Supplied external facts, typed mappings and definition-selected derived computations.
use mara_engine::{
    SourceEngine, SourceSnapshot,
    named_query::{self as q, Record, Value},
};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

fn snapshot() -> SourceSnapshot {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/query-facts/snapshot.json"
    ))
    .unwrap()
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
fn reported(
    records: Vec<q::FactRecord>,
    coverage: Vec<BTreeMap<String, q::ParameterValue>>,
) -> q::FactSnapshot {
    q::FactSnapshot {
        input: "reported".into(),
        identity: "reported-run".into(),
        coverage,
        records,
    }
}
fn request(
    definition: &str,
    ids: &[&str],
    parameters: Vec<q::BindingParameter>,
    facts: Vec<q::FactSnapshot>,
    mappings: Vec<q::TypedMapping>,
) -> q::Request {
    q::Request {
        definition_files: vec!["facts.json".into()],
        definition: definition.into(),
        selection: q::Selection {
            ids: ids.iter().map(|id| (*id).into()).collect(),
            ..Default::default()
        },
        parameters,
        max_rows: 100,
        limit: 100,
        cursor: None,
        fact_snapshots: facts,
        mappings,
    }
}
fn verification_request(
    ids: &[&str],
    candidate: &str,
    context: &str,
    facts: Vec<q::FactSnapshot>,
) -> q::Request {
    request(
        "urn:fixture:facts:verifications",
        ids,
        vec![
            q::BindingParameter {
                name: "candidate".into(),
                value: text(candidate),
            },
            q::BindingParameter {
                name: "context".into(),
                value: text(context),
            },
        ],
        facts,
        vec![],
    )
}
fn validation_request(
    ids: &[&str],
    candidate: &str,
    context: &str,
    facts: Vec<q::FactSnapshot>,
) -> q::Request {
    request(
        "urn:fixture:facts:validation",
        ids,
        vec![
            q::BindingParameter {
                name: "candidate".into(),
                value: text(candidate),
            },
            q::BindingParameter {
                name: "context".into(),
                value: text(context),
            },
        ],
        facts,
        vec![],
    )
}
fn risk_request(ids: &[&str], mappings: Vec<q::TypedMapping>) -> q::Request {
    request("urn:fixture:facts:risk", ids, vec![], vec![], mappings)
}
fn risk_mapping() -> q::TypedMapping {
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
    }
}
fn band_mapping(fallback: Option<q::ParameterValue>) -> q::TypedMapping {
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
        fallback,
    }
}
fn root_id(endpoint: &q::Endpoint) -> String {
    match endpoint {
        q::Endpoint::Item { id, .. } => id.clone(),
        other => format!("{other:?}"),
    }
}
fn bindings(page: &q::Page) -> Vec<(String, &q::Binding)> {
    page.records
        .iter()
        .filter_map(|record| match record {
            Record::Binding { root, binding } => Some((root_id(root), binding)),
            Record::Root { .. } => None,
        })
        .collect()
}
fn text_value(value: Option<&Option<Value>>) -> Option<String> {
    match value {
        Some(Some(Value::Text { value, .. })) => Some(value.clone()),
        _ => None,
    }
}
fn integer_value(value: Option<&Option<Value>>) -> Option<i64> {
    match value {
        Some(Some(Value::Integer { value })) => Some(*value),
        _ => None,
    }
}
fn states(page: &q::Page) -> Vec<String> {
    bindings(page)
        .iter()
        .filter_map(|(_, binding)| text_value(binding.values.get("state")))
        .collect()
}
fn mutate_definition(s: &mut SourceSnapshot, f: impl FnOnce(&mut Json)) {
    let file = s.files.iter_mut().find(|f| f.path == "facts.json").unwrap();
    let mut definitions: Json = serde_json::from_str(&file.content).unwrap();
    f(&mut definitions);
    file.content = definitions.to_string();
}

// @mara checks REQ-EXPLICIT-QUERY-INPUTS
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn complete_coverage_missing_and_unavailable_are_distinct() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let coverage = vec![
        key("VER-V1", "CAND-C", "release"),
        key("VER-V2", "CAND-C", "release"),
        key("VER-V3", "CAND-C", "release"),
    ];
    let complete = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V2", "CAND-C", "release", "pass"),
            record("VER-V3", "CAND-C", "release", "pass"),
        ],
        coverage.clone(),
    );
    let page = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![complete],
        ))
        .unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    assert_eq!(page.summary.binding_count, Some(3));
    assert!(
        states(&page).iter().all(|state| state == "pass"),
        "{page:#?}"
    );
    for (_, binding) in bindings(&page) {
        assert_eq!(binding.facts.len(), 1);
        assert_eq!(binding.facts[0].identity, "reported-run");
        assert!(binding.facts[0].record.is_some());
    }

    // A complete snapshot with no record for a covered key is known missing, never a pass.
    let missing = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V3", "CAND-C", "release", "pass"),
        ],
        coverage.clone(),
    );
    let page = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![missing],
        ))
        .unwrap();
    assert!(page.evaluation_complete);
    assert_eq!(states(&page).iter().filter(|s| *s == "missing").count(), 1);
    for (_, binding) in bindings(&page) {
        if text_value(binding.values.get("state")).as_deref() == Some("missing") {
            assert_eq!(binding.facts[0].record, None);
        }
    }

    // Partial coverage (no claim for V2/V3) is unavailable, not known missing.
    let partial = reported(
        vec![record("VER-V1", "CAND-C", "release", "pass")],
        vec![key("VER-V1", "CAND-C", "release")],
    );
    let page = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![partial],
        ))
        .unwrap();
    assert!(!page.evaluation_complete);
    assert!(page.summary.binding_count.is_none());
    assert!(
        page.unavailable_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("does not cover"))
    );

    // A snapshot claiming another candidate's keys does not cover CAND-E.
    let wrong = reported(
        vec![record("VER-V1", "CAND-C", "release", "pass")],
        vec![key("VER-V1", "CAND-C", "release")],
    );
    let page = engine
        .query_execute(verification_request(
            &["CAND-E"],
            "CAND-E",
            "release",
            vec![wrong],
        ))
        .unwrap();
    assert!(!page.evaluation_complete);

    // A missing required snapshot is unavailable input, not an empty successful result.
    let page = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![],
        ))
        .unwrap();
    assert!(!page.evaluation_complete);
    assert!(
        page.unavailable_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("was not supplied"))
    );
}

// @mara checks REQ-COMPOSABLE-QUERY-PROJECTIONS
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
#[test]
fn definition_precedence_selects_members_and_retains_their_support() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let coverage = vec![
        key("VER-V1", "CAND-C", "release"),
        key("VER-V2", "CAND-C", "release"),
        key("VER-V3", "CAND-C", "release"),
    ];
    let mixed = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V2", "CAND-C", "release", "fail"),
            record("VER-V3", "CAND-C", "release", "inconclusive"),
        ],
        coverage.clone(),
    );
    let page = engine
        .query_execute(validation_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![mixed.clone()],
        ))
        .unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    assert_eq!(page.summary.binding_count, Some(1));
    let (_, binding) = &bindings(&page)[0];
    assert_eq!(
        text_value(binding.values.get("state")).as_deref(),
        Some("fail")
    );
    assert_eq!(integer_value(binding.values.get("members")), Some(3));
    // The selected member's retained binding and its explicit support survive the aggregate.
    assert!(matches!(
        binding.values.get("verification"),
        Some(Some(Value::Item { .. }))
    ));
    assert!(!binding.support.is_empty());

    // Records in another validation context are ignored, not treated as matching evidence.
    let other_context = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V2", "CAND-C", "release", "fail"),
            record("VER-V3", "CAND-C", "release", "inconclusive"),
            record("VER-V1", "CAND-C", "nightly", "error"),
        ],
        coverage
            .iter()
            .cloned()
            .chain([key("VER-V1", "CAND-C", "nightly")])
            .collect(),
    );
    let page = engine
        .query_execute(validation_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![other_context],
        ))
        .unwrap();
    assert!(page.evaluation_complete);
    assert_eq!(
        text_value(bindings(&page)[0].1.values.get("state")).as_deref(),
        Some("fail")
    );

    // Replacing the declared precedence changes the aggregate without any Rust default.
    let mut replaced = snapshot();
    mutate_definition(&mut replaced, |definitions| {
        let definition = definitions["definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|definition| definition["id"] == "urn:fixture:facts:validation")
            .unwrap();
        definition["aggregate"]["precedence"] = json!([
            {"type": "text", "value": "pass"},
            {"type": "text", "value": "inconclusive"},
            {"type": "text", "value": "fail"}
        ]);
    });
    let engine = SourceEngine::new(replaced).unwrap();
    let page = engine
        .query_execute(validation_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![mixed],
        ))
        .unwrap();
    assert_eq!(
        text_value(bindings(&page)[0].1.values.get("state")).as_deref(),
        Some("pass")
    );

    // No grouped members returns the definition's declared empty outcome, not readiness.
    let empty = engine
        .query_execute(validation_request(
            &["CAND-EMPTY"],
            "CAND-EMPTY",
            "release",
            vec![reported(vec![], vec![])],
        ))
        .unwrap();
    assert!(empty.evaluation_complete);
    let (_, binding) = &bindings(&empty)[0];
    assert_eq!(
        text_value(binding.values.get("state")).as_deref(),
        Some("missing")
    );
    assert_eq!(integer_value(binding.values.get("members")), Some(0));
}

// @mara checks REQ-EXPLICIT-QUERY-INPUTS
#[test]
fn typed_lookup_mappings_are_replaceable_and_never_silently_defaulted() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let page = engine
        .query_execute(risk_request(
            &["CAND-C", "CAND-E"],
            vec![risk_mapping(), band_mapping(None)],
        ))
        .unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    let mut scored = vec![];
    for (root, binding) in bindings(&page) {
        scored.push((
            root,
            integer_value(binding.values.get("score")),
            text_value(binding.values.get("band")),
        ));
    }
    scored.sort();
    assert_eq!(
        scored,
        vec![
            ("CAND-C".to_owned(), Some(6), Some("above".to_owned())),
            ("CAND-E".to_owned(), Some(3), Some("below".to_owned())),
        ]
    );

    // A matrix that lacks a required key is unavailable, never silently zero or safe.
    let mut reduced = risk_mapping();
    reduced.entries.retain(|entry| {
        !matches!(
            entry.keys.get("likelihood"),
            Some(q::ParameterValue::Integer(1))
        )
    });
    let page = engine
        .query_execute(risk_request(
            &["CAND-E"],
            vec![reduced.clone(), band_mapping(Some(text("below")))],
        ))
        .unwrap();
    assert!(!page.evaluation_complete);
    assert!(
        page.unavailable_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("no entry"))
    );

    // An explicit fallback is the only permitted substitution, and it applies per mapping.
    let mut with_fallback = reduced;
    with_fallback.fallback = Some(integer(0));
    let page = engine
        .query_execute(risk_request(
            &["CAND-E"],
            vec![with_fallback, band_mapping(Some(text("below")))],
        ))
        .unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    let (_, binding) = &bindings(&page)[0];
    assert_eq!(integer_value(binding.values.get("score")), Some(0));
    assert_eq!(
        text_value(binding.values.get("band")).as_deref(),
        Some("below")
    );
}

// @mara checks REQ-EXPLICIT-QUERY-INPUTS
#[test]
fn malformed_conflicting_and_unknown_inputs_are_rejected() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let coverage = |verification: &str| vec![key(verification, "CAND-C", "release")];
    // Conflicting duplicate record keys.
    let conflicting = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V1", "CAND-C", "release", "fail"),
        ],
        coverage("VER-V1"),
    );
    let error = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![conflicting],
        ))
        .unwrap_err();
    assert_eq!(error.code, "invalid_argument");
    assert!(error.message.contains("conflicting records"));

    // A record that does not declare exactly the declared key columns.
    let mut malformed = record("VER-V1", "CAND-C", "release", "pass");
    malformed.keys.remove("context");
    let error = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![reported(vec![malformed], coverage("VER-V1"))],
        ))
        .unwrap_err();
    assert_eq!(error.code, "invalid_argument");

    // A supplied snapshot that no loaded definition declares.
    let mut unknown = reported(vec![], coverage("VER-V1"));
    unknown.input = "other".into();
    let error = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![unknown],
        ))
        .unwrap_err();
    assert_eq!(error.code, "invalid_argument");

    // Conflicting duplicate mapping keys.
    let mut mapping = band_mapping(None);
    let duplicate = mapping.entries[0].clone();
    mapping.entries.push(duplicate);
    let error = engine
        .query_execute(risk_request(&["CAND-C"], vec![risk_mapping(), mapping]))
        .unwrap_err();
    assert_eq!(error.code, "invalid_argument");
    assert!(error.message.contains("conflicting duplicate keys"));

    // A missing required named definition is a definition error, not an empty result.
    let mut missing = verification_request(
        &["CAND-C"],
        "CAND-C",
        "release",
        vec![reported(vec![], vec![])],
    );
    missing.definition = "urn:fixture:facts:unknown".into();
    let error = engine.query_execute(missing).unwrap_err();
    assert_eq!(error.code, "invalid_definition");
}

// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
#[test]
fn fact_mapping_and_context_changes_invalidate_continuation() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let coverage = vec![
        key("VER-V1", "CAND-C", "release"),
        key("VER-V2", "CAND-C", "release"),
        key("VER-V3", "CAND-C", "release"),
    ];
    let facts = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V2", "CAND-C", "release", "pass"),
            record("VER-V3", "CAND-C", "release", "fail"),
        ],
        coverage,
    );
    let mut request = verification_request(&["CAND-C"], "CAND-C", "release", vec![facts]);
    request.limit = 1;
    let first = engine.query_execute(request.clone()).unwrap();
    let cursor = first.next_cursor.clone().expect("paged cursor");
    request.cursor = Some(cursor.clone());
    engine.query_execute(request.clone()).unwrap();

    // Changed fact content with an unchanged opaque label still rejects the cursor.
    let mut changed = request.clone();
    changed.fact_snapshots[0].records[0]
        .fields
        .insert("state".into(), text("fail"));
    let error = engine.query_execute(changed).unwrap_err();
    assert_eq!(error.code, "invalid_argument");
    assert!(error.message.contains("cursor"));

    // Changed context is a different question and rejects the cursor too.
    let mut changed = request.clone();
    changed.parameters[1].value = text("nightly");
    assert!(engine.query_execute(changed).is_err());
}

/// Retained native report for the real browser worker parity check.
#[test]
fn native_query_facts_report() {
    let Ok(path) = std::env::var("MARA_QUERY_FACTS_REPORT") else {
        return;
    };
    let engine = SourceEngine::new(snapshot()).unwrap();
    let coverage = vec![
        key("VER-V1", "CAND-C", "release"),
        key("VER-V2", "CAND-C", "release"),
        key("VER-V3", "CAND-C", "release"),
    ];
    let complete = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V2", "CAND-C", "release", "pass"),
            record("VER-V3", "CAND-C", "release", "fail"),
        ],
        coverage.clone(),
    );
    let missing = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V3", "CAND-C", "release", "pass"),
        ],
        coverage.clone(),
    );
    let partial = reported(
        vec![record("VER-V1", "CAND-C", "release", "pass")],
        vec![key("VER-V1", "CAND-C", "release")],
    );
    let cases: Vec<(&str, q::Request)> = vec![
        (
            "verifications_complete",
            verification_request(&["CAND-C"], "CAND-C", "release", vec![complete.clone()]),
        ),
        (
            "verifications_missing",
            verification_request(&["CAND-C"], "CAND-C", "release", vec![missing]),
        ),
        (
            "verifications_partial",
            verification_request(&["CAND-C"], "CAND-C", "release", vec![partial]),
        ),
        (
            "verifications_absent",
            verification_request(&["CAND-C"], "CAND-C", "release", vec![]),
        ),
        (
            "validation_aggregate",
            validation_request(&["CAND-C"], "CAND-C", "release", vec![complete.clone()]),
        ),
        (
            "validation_empty",
            validation_request(
                &["CAND-EMPTY"],
                "CAND-EMPTY",
                "release",
                vec![reported(vec![], vec![])],
            ),
        ),
        (
            "risk_matrix",
            risk_request(
                &["CAND-C", "CAND-E"],
                vec![risk_mapping(), band_mapping(None)],
            ),
        ),
        (
            "composed_provenance",
            q::Request {
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
                fact_snapshots: vec![complete],
                mappings: vec![],
            },
        ),
    ];
    let mut report = serde_json::Map::new();
    for (name, request) in cases {
        let outcome = match engine.query_execute(request) {
            Ok(page) => json!({"page": page}),
            Err(error) => json!({"error": {"code": error.code, "message": error.message}}),
        };
        report.insert(name.into(), outcome);
    }
    std::fs::write(path, serde_json::to_vec(&report).unwrap()).unwrap();
}

// @mara checks DES-QUERY-FACT-COMPUTATION
#[test]
fn derived_values_validate_unselected_records_flavours_and_iris() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    // A record whose key no returned tuple selects is still validated.
    let mut malformed = record("VER-V1", "CAND-E", "release", "pass");
    malformed.fields.insert("state".into(), integer(1));
    let error = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![reported(vec![malformed], vec![])],
        ))
        .unwrap_err();
    assert_eq!(error.code, "invalid_argument");
    assert!(error.message.contains("wrong type"), "{error:?}");

    let short = q::FactRecord {
        keys: key("VER-V1", "CAND-E", "release"),
        fields: BTreeMap::new(),
    };
    let error = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![reported(vec![short], vec![])],
        ))
        .unwrap_err();
    assert_eq!(error.code, "invalid_argument");
    assert!(error.message.contains("declared fields"), "{error:?}");

    // A definition-supplied missing value must satisfy the declared item flavour. Only this
    // definition is loaded so a shared input name cannot mask the intended error.
    let mut flavoured = snapshot();
    mutate_definition(&mut flavoured, |definitions| {
        definitions["definitions"] = serde_json::Value::Array(
            definitions["definitions"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|definition| definition["id"] == "urn:fixture:facts:verifications")
                .cloned()
                .collect(),
        );
        let definition = &mut definitions["definitions"][0];
        definition["columns"]["state"] = json!({"type": "item", "flavours": ["verification"]});
        definition["external_facts"][0]["missing"]["state"] =
            json!({"type": "item", "value": "CAND-C"});
    });
    let engine = SourceEngine::new(flavoured).unwrap();
    let mut v1 = record("VER-V1", "CAND-C", "release", "pass");
    v1.fields.insert("state".into(), item("VER-V1"));
    let mut v3 = record("VER-V3", "CAND-C", "release", "pass");
    v3.fields.insert("state".into(), item("VER-V3"));
    let coverage = vec![
        key("VER-V1", "CAND-C", "release"),
        key("VER-V2", "CAND-C", "release"),
        key("VER-V3", "CAND-C", "release"),
    ];
    let error = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![reported(vec![v1, v3], coverage)],
        ))
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(error.message.contains("flavour"), "{error:?}");

    // A supplied derived value declared as an IRI must be a valid absolute IRI.
    let mut iri_typed = snapshot();
    mutate_definition(&mut iri_typed, |definitions| {
        let definition = definitions["definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|definition| definition["id"] == "urn:fixture:facts:risk")
            .unwrap();
        definition["columns"]["band"] = json!({"type": "iri"});
    });
    let engine = SourceEngine::new(iri_typed).unwrap();
    let mut band = band_mapping(None);
    band.entries[0].value = q::ParameterValue::Iri("not an iri".into());
    let error = engine
        .query_execute(risk_request(&["CAND-C"], vec![risk_mapping(), band]))
        .unwrap_err();
    assert_eq!(error.code, "invalid_argument");
    assert!(error.message.contains("invalid IRI"), "{error:?}");
}

// @mara checks DES-QUERY-FACT-COMPUTATION
#[test]
fn declarations_and_supplied_content_are_checked_before_root_enumeration() {
    // The external source belongs to format 5 and cannot be declared in older formats.
    let mut older = snapshot();
    older
        .files
        .iter_mut()
        .find(|file| file.path == "facts.json")
        .unwrap()
        .content = json!({
        "format_version": 3,
        "definitions": [{
            "id": "urn:fixture:facts:external-declaration",
            "required_fact_sources": ["documents", "external"],
            "root_flavours": ["candidate"],
            "columns": {"candidate": {"type": "text"}},
            "select": "SELECT ?root ?candidate WHERE { ?root <urn:mara:relation:requires_verification> ?candidate }"
        }]
    })
        .to_string();
    let engine = SourceEngine::new(older).unwrap();
    let request = q::Request {
        definition_files: vec!["facts.json".into()],
        definition: "urn:fixture:facts:external-declaration".into(),
        selection: q::Selection {
            ids: vec!["CAND-C".into()],
            ..Default::default()
        },
        parameters: vec![],
        max_rows: 100,
        limit: 100,
        cursor: None,
        fact_snapshots: vec![],
        mappings: vec![],
    };
    let error = engine.query_execute(request).unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(error.message.contains("format 5"), "{error:?}");

    // Malformed supplied content is rejected even when the root filter selects nothing.
    let engine = SourceEngine::new(snapshot()).unwrap();
    let empty_selection = q::Selection {
        flavours: vec!["candidate".into()],
        fields: vec![q::FieldSelection {
            key: "status".into(),
            value: "retired".into(),
        }],
        ..Default::default()
    };
    let mut request = verification_request(
        &["CAND-C"],
        "CAND-C",
        "release",
        vec![reported(vec![], vec![])],
    );
    request.selection = empty_selection.clone();
    let complete = engine.query_execute(request.clone()).unwrap();
    assert!(complete.evaluation_complete);
    assert_eq!(complete.summary.selected_roots, 0);

    // An unbound required input is still unavailable for a complete empty selection.
    let mut request = verification_request(&["CAND-C"], "CAND-C", "release", vec![]);
    request.selection = empty_selection.clone();
    let absent = engine.query_execute(request).unwrap();
    assert!(!absent.evaluation_complete);
    assert_eq!(absent.summary.selected_roots, 0);
    assert!(
        absent
            .unavailable_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("was not supplied"))
    );

    let mut malformed = record("VER-V1", "CAND-C", "release", "pass");
    malformed.fields.insert("state".into(), integer(1));
    let mut request = verification_request(
        &["CAND-C"],
        "CAND-C",
        "release",
        vec![reported(vec![malformed], vec![])],
    );
    request.selection = empty_selection.clone();
    let error = engine.query_execute(request).unwrap_err();
    assert_eq!(error.code, "invalid_argument");
    assert!(error.message.contains("wrong type"), "{error:?}");

    let conflicting = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V1", "CAND-C", "release", "fail"),
        ],
        vec![key("VER-V1", "CAND-C", "release")],
    );
    let mut request = verification_request(&["CAND-C"], "CAND-C", "release", vec![conflicting]);
    request.selection = empty_selection;
    let error = engine.query_execute(request).unwrap_err();
    assert_eq!(error.code, "invalid_argument");
    assert!(error.message.contains("conflicting records"), "{error:?}");
}

// @mara checks DES-QUERY-FACT-COMPUTATION
#[test]
fn projected_parameter_reads_and_item_keys_are_checked() {
    // A caller-supplied projected column may be read by FILTER; only rebinding is rejected.
    let mut filtered = snapshot();
    mutate_definition(&mut filtered, |definitions| {
        let definition = definitions["definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|definition| definition["id"] == "urn:fixture:facts:verifications")
            .unwrap();
        let select = definition["select"].as_str().unwrap().to_owned();
        definition["select"] = json!(select.replace(
            "FILTER(sameTerm(?vstatus, \"accepted\"))",
            "FILTER(sameTerm(?vstatus, \"accepted\") && sameTerm(?context, \"release\"))"
        ));
    });
    let engine = SourceEngine::new(filtered).unwrap();
    let facts = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V2", "CAND-C", "release", "pass"),
            record("VER-V3", "CAND-C", "release", "pass"),
        ],
        vec![
            key("VER-V1", "CAND-C", "release"),
            key("VER-V2", "CAND-C", "release"),
            key("VER-V3", "CAND-C", "release"),
        ],
    );
    let page = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![facts.clone()],
        ))
        .unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    assert_eq!(page.summary.binding_count, Some(3));
    let other = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "nightly",
            vec![facts],
        ))
        .unwrap();
    assert!(other.evaluation_complete);
    assert_eq!(other.summary.binding_count, Some(0));

    // A supplied key item must satisfy a declared key-column flavour, not just its type.
    let engine = SourceEngine::new(snapshot()).unwrap();
    let mut wrong_key = record("VER-V1", "CAND-C", "release", "pass");
    wrong_key.keys.insert("verification".into(), item("CAND-C"));
    let error = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![reported(vec![wrong_key], vec![])],
        ))
        .unwrap_err();
    assert_eq!(error.code, "invalid_argument");
    assert!(error.message.contains("flavour"), "{error:?}");
}

// @mara checks DES-QUERY-FACT-COMPUTATION
#[test]
fn unavailable_inputs_and_exists_keep_available_bindings() {
    // One root's missing mapping key must not discard another root's known bindings.
    let engine = SourceEngine::new(snapshot()).unwrap();
    let mut only_e = risk_mapping();
    only_e.entries.retain(|entry| {
        !matches!(
            entry.keys.get("likelihood"),
            Some(q::ParameterValue::Integer(2))
        )
    });
    let page = engine
        .query_execute(risk_request(
            &["CAND-C", "CAND-E"],
            vec![only_e, band_mapping(None)],
        ))
        .unwrap();
    assert!(!page.evaluation_complete);
    assert!(page.summary.binding_count.is_none());
    let known = bindings(&page);
    assert_eq!(known.len(), 2, "both roots keep a binding: {page:#?}");
    let e = known
        .iter()
        .find(|(root, _)| root == "CAND-E")
        .expect("CAND-E binding retained");
    assert_eq!(integer_value(e.1.values.get("score")), Some(3));
    assert_eq!(text_value(e.1.values.get("band")).as_deref(), Some("below"));

    // A projected parameter may be read inside EXISTS without counting as a rebinding.
    let mut exists = snapshot();
    mutate_definition(&mut exists, |definitions| {
        let definition = definitions["definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|definition| definition["id"] == "urn:fixture:facts:verifications")
            .unwrap();
        let select = definition["select"].as_str().unwrap().to_owned();
        definition["select"] = json!(select.replace(
            "FILTER(sameTerm(?vstatus, \"accepted\"))",
            "FILTER(sameTerm(?vstatus, \"accepted\") && EXISTS { ?verification <urn:mara:field:status> ?context })"
        ));
    });
    let engine = SourceEngine::new(exists).unwrap();
    let facts = reported(
        vec![
            record("VER-V1", "CAND-C", "accepted", "pass"),
            record("VER-V2", "CAND-C", "accepted", "pass"),
            record("VER-V3", "CAND-C", "accepted", "pass"),
        ],
        vec![
            key("VER-V1", "CAND-C", "accepted"),
            key("VER-V2", "CAND-C", "accepted"),
            key("VER-V3", "CAND-C", "accepted"),
        ],
    );
    let page = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "accepted",
            vec![facts],
        ))
        .unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    assert_eq!(page.summary.binding_count, Some(3));
    let empty = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![reported(vec![], vec![])],
        ))
        .unwrap();
    assert!(empty.evaluation_complete);
    assert_eq!(empty.summary.binding_count, Some(0));
}

// @mara checks DES-QUERY-FACT-COMPUTATION
#[test]
fn resolved_precedence_duplicates_are_definition_errors() {
    let mut duplicated = snapshot();
    mutate_definition(&mut duplicated, |definitions| {
        let definition = definitions["definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|definition| definition["id"] == "urn:fixture:facts:validation")
            .unwrap();
        definition["columns"]["state"] = json!({"type": "item", "flavours": ["verification"]});
        definition["external_facts"][0]["missing"]["state"] =
            json!({"type": "item", "value": "VER-V1"});
        definition["aggregate"]["empty"] = json!({"type": "item", "value": "VER-V1"});
        definition["aggregate"]["precedence"] = json!([
            {"type": "item", "value": "VER-V1"},
            {"type": "item", "value": "01JQFACT000000000000000004"}
        ]);
    });
    let engine = SourceEngine::new(duplicated).unwrap();
    let error = engine
        .query_execute(validation_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![reported(vec![], vec![])],
        ))
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(error.message.contains("precedence"), "{error:?}");
}

// @mara checks DES-QUERY-FACT-COMPUTATION
#[test]
fn truncated_aggregate_counts_are_not_asserted() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let coverage = vec![
        key("VER-V1", "CAND-C", "release"),
        key("VER-V2", "CAND-C", "release"),
        key("VER-V3", "CAND-C", "release"),
    ];
    let facts = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V2", "CAND-C", "release", "fail"),
            record("VER-V3", "CAND-C", "release", "error"),
        ],
        coverage,
    );
    let mut request = validation_request(&["CAND-C"], "CAND-C", "release", vec![facts]);
    request.max_rows = 1;
    let page = engine.query_execute(request).unwrap();
    assert!(!page.evaluation_complete);
    let (_, binding) = &bindings(&page)[0];
    // The observed member and its state are retained, but no exact truncated count is asserted.
    assert!(
        text_value(binding.values.get("state")).is_some(),
        "{page:#?}"
    );
    assert!(
        binding
            .values
            .get("members")
            .and_then(|value| value.as_ref())
            .is_none(),
        "{page:#?}"
    );
}

// @mara checks DES-QUERY-FACT-COMPUTATION
#[test]
fn required_code_source_suppresses_aggregate_counts() {
    let mut code = snapshot();
    mutate_definition(&mut code, |definitions| {
        let definition = definitions["definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|definition| definition["id"] == "urn:fixture:facts:validation")
            .unwrap();
        definition["required_fact_sources"] = json!(["documents", "external", "code"]);
    });
    let engine = SourceEngine::new(code).unwrap();
    let coverage = vec![
        key("VER-V1", "CAND-C", "release"),
        key("VER-V2", "CAND-C", "release"),
        key("VER-V3", "CAND-C", "release"),
    ];
    let facts = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V2", "CAND-C", "release", "pass"),
            record("VER-V3", "CAND-C", "release", "pass"),
        ],
        coverage,
    );
    let page = engine
        .query_execute(validation_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![facts],
        ))
        .unwrap();
    assert!(!page.evaluation_complete);
    let (_, binding) = &bindings(&page)[0];
    assert_eq!(
        text_value(binding.values.get("state")).as_deref(),
        Some("pass")
    );
    assert!(
        binding
            .values
            .get("members")
            .and_then(|value| value.as_ref())
            .is_none(),
        "{page:#?}"
    );
}

// @mara checks DES-QUERY-FACT-COMPUTATION
#[test]
fn coverage_and_truncation_do_not_assert_missing_or_empty() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    // A supplied record outside declared coverage stays known evidence but is unavailable.
    let uncovered = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V2", "CAND-C", "release", "pass"),
            record("VER-V3", "CAND-C", "release", "pass"),
        ],
        vec![key("VER-V1", "CAND-C", "release")],
    );
    let page = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![uncovered],
        ))
        .unwrap();
    assert!(!page.evaluation_complete);
    assert_eq!(states(&page).len(), 3);
    assert!(
        page.unavailable_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("does not cover"))
    );

    // A truncated later root must not receive the declared empty aggregate outcome.
    let facts = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V2", "CAND-C", "release", "pass"),
            record("VER-V3", "CAND-C", "release", "fail"),
            record("VER-V1", "CAND-E", "release", "pass"),
        ],
        vec![
            key("VER-V1", "CAND-C", "release"),
            key("VER-V2", "CAND-C", "release"),
            key("VER-V3", "CAND-C", "release"),
            key("VER-V1", "CAND-E", "release"),
        ],
    );
    let mut request = validation_request(&["CAND-C", "CAND-E"], "CAND-C", "release", vec![facts]);
    request.max_rows = 2;
    let page = engine.query_execute(request).unwrap();
    assert!(!page.evaluation_complete);
    let known = bindings(&page);
    assert!(
        known.iter().all(|(root, _)| root != "CAND-E"),
        "truncated root must not assert an empty outcome: {page:#?}"
    );
    assert!(
        known.iter().all(|(_, binding)| {
            text_value(binding.values.get("state")).as_deref() != Some("missing")
        }),
        "{page:#?}"
    );
}

// @mara checks DES-QUERY-FACT-COMPUTATION
#[test]
fn derived_keys_and_lookup_self_outputs_are_definition_errors() {
    // A fact key that the same fact derives can never be projected or populated.
    let mut derived_key = snapshot();
    mutate_definition(&mut derived_key, |definitions| {
        let definition = definitions["definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|definition| definition["id"] == "urn:fixture:facts:verifications")
            .unwrap();
        definition["external_facts"][0]["fields"] = json!({"state": "candidate"});
        definition["external_facts"][0]["missing"] =
            json!({"candidate": {"type": "text", "value": "missing"}});
    });
    let engine = SourceEngine::new(derived_key).unwrap();
    let error = engine
        .query_execute(verification_request(
            &["CAND-C"],
            "CAND-C",
            "release",
            vec![reported(vec![], vec![])],
        ))
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(error.message.contains("key column"), "{error:?}");

    // A lookup key may not be its own output.
    let mut self_keyed = snapshot();
    mutate_definition(&mut self_keyed, |definitions| {
        let definition = definitions["definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|definition| definition["id"] == "urn:fixture:facts:risk")
            .unwrap();
        definition["lookups"][0]["keys"] = json!(["score"]);
    });
    let engine = SourceEngine::new(self_keyed).unwrap();
    let error = engine
        .query_execute(risk_request(
            &["CAND-C"],
            vec![risk_mapping(), band_mapping(None)],
        ))
        .unwrap_err();
    assert_eq!(error.code, "invalid_definition");
    assert!(error.message.contains("output"), "{error:?}");
}

// @mara checks DES-QUERY-FACT-COMPUTATION
#[test]
fn unreachable_declarations_still_validate_supplied_inputs() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    // A malformed snapshot declared only by an unreachable definition is still rejected.
    let mut malformed = record("VER-V1", "CAND-C", "release", "pass");
    malformed.fields.insert("state".into(), integer(1));
    let mut request = risk_request(&["CAND-C"], vec![risk_mapping(), band_mapping(None)]);
    request.fact_snapshots = vec![reported(vec![malformed], vec![])];
    let error = engine.query_execute(request).unwrap_err();
    assert_eq!(error.code, "invalid_argument");
    assert!(error.message.contains("wrong type"), "{error:?}");

    // An absent input of an unreachable definition stays irrelevant and does not make the
    // reachable risk computation unavailable.
    let page = engine
        .query_execute(risk_request(
            &["CAND-C"],
            vec![risk_mapping(), band_mapping(None)],
        ))
        .unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
}

// @mara checks DES-QUERY-FACT-COMPUTATION
#[test]
fn dependency_projection_carries_explicit_fact_provenance() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let coverage = vec![
        key("VER-V1", "CAND-C", "release"),
        key("VER-V2", "CAND-C", "release"),
        key("VER-V3", "CAND-C", "release"),
    ];
    let facts = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V2", "CAND-C", "release", "pass"),
            record("VER-V3", "CAND-C", "release", "fail"),
        ],
        coverage,
    );
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
        fact_snapshots: vec![facts],
        mappings: vec![],
    };
    let page = engine.query_execute(request).unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    let known = bindings(&page);
    assert_eq!(known.len(), 3, "{page:#?}");
    for (_, binding) in &known {
        // The parent explicitly projected child fact provenance, so it is retained.
        assert_eq!(binding.facts.len(), 1, "{page:#?}");
        assert_eq!(binding.facts[0].input, "reported");
        assert_eq!(binding.facts[0].identity, "reported-run");
        assert!(binding.facts[0].record.is_some());
        assert_eq!(binding.facts[0].key.len(), 3);
    }
}

// @mara checks DES-QUERY-FACT-COMPUTATION
#[test]
fn merged_tuples_do_not_duplicate_fact_provenance() {
    let mut duplicated = snapshot();
    mutate_definition(&mut duplicated, |definitions| {
        let definition = definitions["definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|definition| definition["id"] == "urn:fixture:facts:composed")
            .unwrap();
        let select = definition["select"].as_str().unwrap().to_owned();
        definition["select"] =
            json!(select.replace("?child_facts .\n}", "?child_facts .\n  { } UNION { }\n}"));
    });
    let engine = SourceEngine::new(duplicated).unwrap();
    let coverage = vec![
        key("VER-V1", "CAND-C", "release"),
        key("VER-V2", "CAND-C", "release"),
        key("VER-V3", "CAND-C", "release"),
    ];
    let facts = reported(
        vec![
            record("VER-V1", "CAND-C", "release", "pass"),
            record("VER-V2", "CAND-C", "release", "pass"),
            record("VER-V3", "CAND-C", "release", "pass"),
        ],
        coverage,
    );
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
        fact_snapshots: vec![facts],
        mappings: vec![],
    };
    let page = engine.query_execute(request).unwrap();
    assert!(page.evaluation_complete, "{page:#?}");
    let known = bindings(&page);
    assert_eq!(known.len(), 3, "{page:#?}");
    for (_, binding) in &known {
        assert_eq!(
            binding.facts.len(),
            1,
            "one canonical provenance: {page:#?}"
        );
    }
}
