//! The production explicit-source APIs, executed unchanged on native and browser WASM.
use mara_engine::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
#[cfg(target_family = "wasm")]
use wasm_bindgen_test::*;

pub fn snapshot() -> SourceSnapshot {
    let fields = json!({"status":{"type":"enum","values":["draft","accepted","retired"]},
        "owner":{"type":"string"},"integer":{"type":"integer"},"double":{"type":"number"},
        "boolean":{"type":"boolean"},"revision":{"type":"string"}});
    let mut flavours = serde_json::Map::new();
    for (name, prefix) in [
        ("requirement", "REQ-"),
        ("verification", "VER-"),
        ("evidence", "EVD-"),
    ] {
        flavours.insert(
            name.into(),
            json!({"description":name,"use_when":[name],"avoid_when":[],
            "distinguish_from":{},"id_prefix":prefix,"body":"required","fields":fields}),
        );
    }
    let schema = json!({"format_version":3,"flavours":flavours,"relations":{
        "verifies":{"description":"Checks","source":["verification"],"target":["requirement"],"inverse":"verified_by"},
        "evidences":{"description":"Records","source":["evidence"],"target":["verification"],"inverse":"evidenced_by"},
        "implements":{"description":"Code","source":[],"target":["requirement"],"code_source":true,"inverse":"implemented_by"}
    }});
    let mut document = String::from("# Unicode café 👩🏽‍💻\n\n");
    for (n, flavour, id, status, relations) in [
        (0, "requirement", "REQ-A", "accepted", ""),
        (1, "requirement", "REQ-B", "accepted", ""),
        (2, "requirement", "REQ-DRAFT", "draft", ""),
        (
            3,
            "verification",
            "VER-A",
            "accepted",
            ":verifies: REQ-A\n:verifies: REQ-A\n",
        ),
        (
            4,
            "verification",
            "VER-DRAFT",
            "draft",
            ":verifies: REQ-A\n",
        ),
        (5, "evidence", "EVD-A", "accepted", ":evidences: VER-A\n"),
    ] {
        document.push_str(&format!(":::mara {flavour} {id}\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F0{n}\n:title: {id}\n:status: {status}\n:owner: Unicode café\n:integer: 7\n:double: 1.5\n:boolean: true\n:revision: abc123\n{relations}\nReal source.\n:::\n\n"));
    }
    SourceSnapshot { format_version:1, base_identity:None,
        selection:SelectionInputs { document_paths:vec!["items.mara.md".into()], ignores:vec![] },
        files:vec![
            SourceFile { path: PROJECT_FILE.into(), content: "format_version = 2\n[project]\nname = 'Rules parity'\nschema = '.mara/schema.yaml'\n[content]\ninclude = ['**/*.mara.md']\n[rules]\nformat_version = 1\nfiles = ['rules.yaml']\n".into() },
            SourceFile { path: SCHEMA_FILE.into(), content:serde_saphyr::to_string(&schema).unwrap() },
            SourceFile { path:"items.mara.md".into(), content:document },
            SourceFile { path:"rules.yaml".into(), content:RULES.into() },
            SourceFile { path:"checks.yaml".into(), content:CHECKS.into() },
        ] }
}
const RULES: &str = r#"
- id: rule:coverage
  targetClass: requirement
  whenShape: rule:accepted
  paths: [items.mara.md]
  property:
    - id: rule:verification_step
      path: {inversePath: verifies}
      qualifiedValueShape: rule:qualified
      qualifiedMinCount: 1
      qualifiedMaxCount: 1
- {id: rule:accepted, property: [{path: status, hasValue: accepted}]}
- id: rule:qualified
  class: verification
  node: rule:accepted
  property:
    - path: {inversePath: evidences}
      qualifiedValueShape: {class: evidence, node: rule:accepted}
      qualifiedMinCount: 1
- id: rule:literals
  targetClass: requirement
  and: [rule:literal_fields]
  or: [rule:accepted, rule:draft]
  not: rule:retired
- {id: rule:draft, property: [{path: status, hasValue: draft}]}
- {id: rule:retired, property: [{path: status, hasValue: retired}]}
- id: rule:literal_fields
  property:
    - {path: status, datatype: string, minCount: 1, maxCount: 1, in: [accepted, draft]}
    - {path: owner, pattern: '^Unicode café', node: {datatype: string}}
    - {path: integer, datatype: integer, hasValue: {value: 7, datatype: integer}}
    - {path: double, datatype: double, hasValue: {value: 1.5, datatype: double}}
    - {path: boolean, datatype: boolean, hasValue: {value: true, datatype: boolean}}
"#;
const CHECKS: &str = r#"
- id: rule:execution
  class: requirement
  property:
    - path: {inversePath: verifies}
      qualifiedValueShape:
        class: verification
        property:
          - path: {inversePath: evidences}
            qualifiedValueShape:
              class: evidence
              property:
                - path: revision
                  hasValue: {parameter: subject_revision}
                  in: [{parameter: subject_revision}]
            qualifiedMinCount: 1
      qualifiedMinCount: 1
- id: rule:every
  class: requirement
  property: [{path: {inversePath: verifies}, node: rule:accepted, minCount: 1, maxCount: 2}]
- {id: rule:accepted, property: [{path: status, hasValue: accepted}]}
# A targeted shape is not enabled by a request check.
- {id: rule:other_policy, targetClass: requirement, property: [{path: status, hasValue: retired}]}
"#;
pub fn params() -> TraceMatrixParams {
    serde_json::from_value(json!({"all":true,"rules":["urn:mara:rule:coverage","urn:mara:rule:literals"],"limit":2,"render":"markdown"})).unwrap()
}
fn target(kind: ValidationTargetKind) -> ValidationTarget {
    ValidationTarget { kind, id: None }
}
fn checks(engine: &SourceEngine) -> RuleChecks {
    engine
        .rule_checks(target(ValidationTargetKind::Project))
        .unwrap()
}
fn complete_trace(engine: &SourceEngine, mut params: TraceMatrixParams) -> Value {
    let mut records = vec![];
    let mut first: Option<Value> = None;
    loop {
        let mut page = serde_json::to_value(engine.trace_matrix(&params).unwrap()).unwrap();
        records.extend(page["records"].as_array().unwrap().iter().cloned());
        if let Some(first) = &first {
            assert_eq!(page["summaries"], first["summaries"]);
        }
        params.cursor = page["next_cursor"].as_str().map(str::to_owned);
        let more = page["has_more"] == true;
        page["records"] = json!([]);
        page["markdown"] = Value::Null;
        page["next_cursor"] = Value::Null;
        page["has_more"] = json!(false);
        if first.is_none() {
            first = Some(page);
        }
        if !more {
            break;
        }
    }
    let mut result = first.unwrap();
    result["records"] = json!(records);
    result
}
fn parity_digest(label: &str, value: &Value, expected: &str) {
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()));
    println!("{label}: {digest}");
    assert_eq!(digest, expected, "native/browser full-result parity");
}
// @mara checks REQ-PORTABLE-DOCUMENT-ENGINE
// @mara checks DES-TRACE-CHECK-BINDING
#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), test)]
fn supplied_rules_and_trace_preserve_complete_outcomes() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let checks = checks(&engine);
    assert!(
        checks.definitions.diagnostics.is_empty(),
        "{:?}",
        checks.definitions.diagnostics
    );
    let validation = engine
        .project_validate(
            &[],
            &ValidationOptions {
                limit: Some(1),
                cursor: None,
                fact_snapshots: vec![],
                mappings: vec![],
            },
            Some(&checks),
        )
        .unwrap();
    assert!(validation.evaluation_complete);
    assert!(!validation.valid);
    assert_eq!(validation.summary.errors, 1);
    assert!(validation.summary.counts_exact);
    assert_eq!(validation.diagnostics[0].code, DiagnosticCode::RuleFailed);
    assert!(
        validation.diagnostics[0]
            .obligation
            .as_ref()
            .unwrap()
            .source
            .line
            .is_some()
    );
    let matrix = complete_trace(&engine, params());
    assert!(matrix["evaluation_complete"] == true);
    assert_eq!(matrix["summaries"][0]["passed"], 1);
    assert_eq!(matrix["summaries"][0]["failed"], 1);
    assert_eq!(matrix["summaries"][0]["not_applicable"], 4);
    assert!(
        matrix["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "edge" && r["occurrence_count"] == 2)
    );
    assert!(
        matrix["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "check" && r["counts"]["qualifying"] == 1)
    );
    parity_digest(
        "rules",
        &json!({"validation":validation,"trace":matrix}),
        "e00f6be66aab4b1f1abf22db6087cbf3cefb8212acc0ba429fb656397c06c6be",
    );
    let item_checks = engine
        .rule_checks(ValidationTarget {
            kind: ValidationTargetKind::Item,
            id: Some("REQ-A".into()),
        })
        .unwrap();
    assert!(
        engine
            .project_validate(&[], &ValidationOptions::default(), Some(&item_checks))
            .is_err()
    );
    let mut failing = snapshot();
    failing.files[3].content = RULES.replace("in: [accepted, draft]", "in: [retired]");
    let failing = SourceEngine::new(failing).unwrap();
    let failing_checks = failing
        .rule_checks(target(ValidationTargetKind::Project))
        .unwrap();
    let mut options = ValidationOptions {
        limit: Some(1),
        cursor: None,
        fact_snapshots: vec![],
        mappings: vec![],
    };
    let mut seen = 0;
    loop {
        let page = failing
            .project_validate(&[], &options, Some(&failing_checks))
            .unwrap();
        assert_eq!(page.summary.errors, 4);
        assert!(page.evaluation_complete && page.summary.counts_exact);
        seen += page.diagnostics.len();
        options.cursor = page.next_cursor;
        if !page.has_more {
            break;
        }
    }
    assert_eq!(seen, 4);
    let first = engine.trace_matrix(&params()).unwrap();
    let mut changed = snapshot();
    changed.files[3].content.push_str("\n# changed\n");
    let engine = SourceEngine::new(changed).unwrap();
    let mut next = params();
    next.cursor = first.next_cursor;
    assert_eq!(engine.trace_matrix(&next).unwrap_err().code, "stale_cursor");
    assert!(
        engine
            .project_validate(&[], &ValidationOptions::default(), Some(&checks))
            .is_err()
    );
}
// @mara checks DES-TRACE-CHECK-BINDING
#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), test)]
fn literal_parameters_every_and_refusals_are_portable() {
    let engine = SourceEngine::new(snapshot()).unwrap();
    let mut outputs = vec![];
    for (text, passed) in [
        ("abc123", true),
        ("other", false),
        ("", false),
        ("rule:abc123", false),
    ] {
        let params: TraceMatrixParams = serde_json::from_value(json!({"ids":["REQ-A"], "check":{"files":["checks.yaml"],"shape":"urn:mara:rule:execution","parameters":{"subject_revision":text}},"limit":1})).unwrap();
        let trace = complete_trace(&engine, params.clone());
        assert_eq!(trace["summaries"][0]["passed"] == 1, passed);
        outputs.push(trace);
        let first = engine.trace_matrix(&params).unwrap();
        let mut changed = params;
        changed.cursor = first.next_cursor;
        changed
            .check
            .as_mut()
            .unwrap()
            .parameters
            .insert("subject_revision".into(), json!("different"));
        assert_eq!(
            engine.trace_matrix(&changed).unwrap_err().code,
            "stale_cursor"
        );
    }
    for bindings in [
        json!({}),
        json!({"subject_revision":7}),
        json!({"subject_revision":"abc123","unused":"x"}),
        json!({"subject_revision":"abc123","bad-name":"x"}),
    ] {
        let params = serde_json::from_value(json!({"ids":["REQ-A"],"check":{"files":["checks.yaml"],"shape":"urn:mara:rule:execution","parameters":bindings}})).unwrap();
        assert_eq!(
            engine.trace_matrix(&params).unwrap_err().code,
            "invalid_argument"
        );
    }
    // All placeholders in the supplied files need values, even for another root;
    // bindings not reachable from the designated root are refused.
    let params = serde_json::from_value(json!({"ids":["REQ-A"],"check":{"files":["checks.yaml"],"shape":"urn:mara:rule:every","parameters":{"subject_revision":"abc123"}}})).unwrap();
    assert_eq!(
        engine.trace_matrix(&params).unwrap_err().code,
        "invalid_argument"
    );
    let mut every = snapshot();
    every.files[4].content = CHECKS[CHECKS.find("- id: rule:every").unwrap()..].into();
    let every_engine = SourceEngine::new(every).unwrap();
    let params = serde_json::from_value(
        json!({"ids":["REQ-A"],"check":{"files":["checks.yaml"],"shape":"urn:mara:rule:every"}}),
    )
    .unwrap();
    let trace = complete_trace(&every_engine, params);
    assert_eq!(trace["summaries"][0]["failed"], 1);
    outputs.push(trace);
    parity_digest(
        "parameters",
        &json!(outputs),
        "0c52dd5d33c9ae1c752cd980b628eb37bfa2cbef81888f3dd11ca4b2b0885def",
    );
}
// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), test)]
fn invalid_sources_and_unavailable_code_never_pass() {
    let mut outputs = vec![];
    for source in [
        "- {id: rule:unsupported, targetClass: requirement, minLength: 1}",
        "- {id: rule:cycle, targetClass: requirement, node: rule:cycle}",
        "- {id: rule:param, targetClass: requirement, property: [{path: revision, hasValue: {parameter: revision}}]}",
        "- {id: rule:bad, targetClass: requirement, property: [{path: owner, pattern: '['}]}",
        "- id: [not-a-shape\n",
    ] {
        let mut changed = snapshot();
        changed.files[3].content = source.into();
        let engine = SourceEngine::new(changed).unwrap();
        let checks = checks(&engine);
        let result = engine
            .project_validate(&[], &ValidationOptions::default(), Some(&checks))
            .unwrap();
        assert!(!result.valid && !result.evaluation_complete);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::RuleInvalid)
        );
        outputs.push(serde_json::to_value(result).unwrap());
    }
    for source in ["[]", RULES] {
        let mut aliases = snapshot();
        aliases.files[0].content = aliases.files[0].content.replace(
            "files = ['rules.yaml']",
            "files = ['./rules.yaml', 'rules.yaml']",
        );
        aliases.files[3].content = source.into();
        let engine = SourceEngine::new(aliases).unwrap();
        let checks = checks(&engine);
        assert_eq!(checks.definitions.diagnostics.len(), 1);
        assert_eq!(
            checks.definitions.diagnostics[0].message,
            "duplicate rule source path"
        );
        let result = engine
            .project_validate(&[], &ValidationOptions::default(), Some(&checks))
            .unwrap();
        assert!(!result.valid && !result.evaluation_complete);
    }
    let mut missing = snapshot();
    missing.files.remove(3);
    let engine = SourceEngine::new(missing).unwrap();
    let result = engine
        .project_validate(&[], &ValidationOptions::default(), Some(&checks(&engine)))
        .unwrap();
    assert!(!result.valid && !result.evaluation_complete);
    outputs.push(json!(result));
    let mut code = snapshot();
    code.files[2].content = code.files[2].content.replacen(
        ":status: accepted",
        ":implemented_by: code:missing.rs\n:status: accepted",
        1,
    );
    let engine = SourceEngine::new(code).unwrap();
    let result = engine
        .project_validate(&[], &ValidationOptions::default(), Some(&checks(&engine)))
        .unwrap();
    assert!(!result.valid && !result.evaluation_complete);
    outputs.push(json!(result));
    let matrix = complete_trace(&engine, params());
    assert_eq!(matrix["evaluation_complete"], false);
    assert_eq!(matrix["summaries"][0]["passed"], 0);
    assert_eq!(matrix["summaries"][0]["counts_exact"], false);
    outputs.push(matrix);
    parity_digest(
        "diagnostics",
        &json!(outputs),
        "88a6a47cdc830b3bcf47c286cc31dfa5668c8290d5f8f254167ca211463f08d4",
    );
}
