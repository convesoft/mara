mod support;
use mara::{named_query as q, *};
use serde_json::{Value, json};
use std::{fs, ops::Deref};

fn initialized(template: Template) -> SourceEngine {
    let mut engine = SourceEngine::new(SourceSnapshot {
        format_version: 1,
        files: vec![],
        selection: SelectionInputs {
            document_paths: vec![],
            ignores: vec![],
        },
        base_identity: None,
    })
    .unwrap();
    let plan = engine.plan_initialize("Engineering", template).unwrap();
    engine.apply_plan(plan).unwrap();
    engine
}

struct EngineeringProject {
    runtime: NativeRuntime,
    _fixture: tempfile::TempDir,
}
impl Deref for EngineeringProject {
    type Target = NativeRuntime;
    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}
fn documents(source: &str) -> EngineeringProject {
    let fixture = support::fixture();
    let root = fixture.path();
    let runtime = NativeRuntime::new(None, root.to_owned()).unwrap();
    runtime
        .project_initialize(Some(root.to_owned()), Template::Engineering)
        .unwrap();
    fs::write(root.join("knowledge.mara.md"), source).unwrap();
    EngineeringProject {
        runtime,
        _fixture: fixture,
    }
}

fn item(index: u32, flavour: &str, id: &str, status: &str, metadata: &str) -> String {
    format!(
        ":::mara {flavour} {id}\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F{index:02}\n:title: {id}\n:status: {status}\n{metadata}\nFixture engineering meaning.\n:::\n\n"
    )
}

fn knowledge() -> String {
    [
        item(0, "goal", "GOAL-A", "accepted", ""),
        item(1, "need", "NEED-A", "accepted", ":contributes_to: GOAL-A\n"),
        item(
            2,
            "requirement",
            "REQ-PARENT",
            "accepted",
            ":satisfies: NEED-A\n",
        ),
        item(
            3,
            "requirement",
            "REQ-A",
            "accepted",
            ":decomposes: REQ-PARENT\n",
        ),
        item(
            4,
            "requirement",
            "REQ-SIBLING",
            "draft",
            ":decomposes: REQ-PARENT\n",
        ),
        item(
            5,
            "verification",
            "VER-PARENT",
            "accepted",
            ":methods: inspection\n:verifies: REQ-PARENT\n",
        ),
        item(
            6,
            "verification",
            "VER-A",
            "accepted",
            ":methods: test\n:verifies: REQ-A\n",
        ),
        item(
            7,
            "verification",
            "VER-B",
            "accepted",
            ":methods: analysis\n:verifies: REQ-A\n",
        ),
        item(8, "design", "DES-A", "accepted", ":satisfies: REQ-A\n"),
        item(
            9,
            "validation",
            "VAL-A",
            "accepted",
            ":methods: demonstration\n:validates: NEED-A\n",
        ),
    ]
    .concat()
}

fn request(definition: &str, id: &str) -> q::Request {
    serde_json::from_value(json!({
        "definition_files":[".mara/engineering-queries.json", ".mara/engineering-views.json"],
        "definition":definition,"selection":{"ids":[id]},
        "parameters":[],"max_rows":10000,"limit":100
    }))
    .unwrap()
}

fn bindings(engine: &EngineeringProject, definition: &str, id: &str) -> Value {
    let page = engine.query_execute(request(definition, id)).unwrap();
    assert!(page.evaluation_complete, "{page:?}");
    serde_json::to_value(page.records).unwrap()
}

fn gate(
    engine: &EngineeringProject,
    name: &str,
    id: &str,
    facts: Vec<Value>,
) -> query_consumers::QueryGateResult {
    engine.query_gate(serde_json::from_value(json!({
        "definition_files":[".mara/engineering-queries.json"],
        "gate_file":".mara/engineering-gates.yaml",
        "shape_files":[".mara/engineering-shapes.ttl"],
        "gate":name,"scope":{"ids":[id]},
        "context":if name == "verified" || name == "release_ready" {
            json!([{"name":"candidate","value":{"type":"text","value":"C"}}])
        } else if name == "required_validation" {
            json!([{"name":"context","value":{"type":"text","value":"staging"}}])
        } else { json!([]) },
        "fact_snapshots":facts,
        "mappings":[serde_json::from_str::<Value>(include_str!("../templates/engineering-risk-matrix.json")).unwrap()],
        "max_rows":10000
    })).unwrap()).unwrap()
}

fn facts(second: Option<&str>, candidate: &str) -> Value {
    let key = |id: &str| json!({"verification":{"type":"item","value":id},"candidate":{"type":"text","value":candidate}});
    let record = |id: &str, result: &str| {
        json!({"keys":key(id),"fields":{
            "result":{"type":"text","value":result},
            "definition_revision":{"type":"text","value":"definition-1"},
            "applicable":{"type":"boolean","value":true}
        }})
    };
    let mut records = vec![record("VER-A", "PASS")];
    if let Some(result) = second {
        records.push(record("VER-B", result));
    }
    json!({"input":"verification_results","identity":"run-1", "coverage":[key("VER-A"),key("VER-B")],"records":records})
}

// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn installs_both_profiles_and_validates_every_new_definition() {
    let legacy = initialized(Template::EngineeringLegacy);
    assert_eq!(
        legacy.files()[".mara/schema.yaml"],
        include_str!("../templates/engineering-legacy-schema.yaml")
    );
    assert_eq!(
        legacy.files()[".mara/engineering-rules.yaml"],
        include_str!("../templates/engineering-legacy-rules.yaml")
    );
    assert_eq!(
        legacy.files()[".mara/engineering-checks.yaml"],
        include_str!("../templates/engineering-legacy-checks.yaml")
    );
    assert_eq!(
        legacy.files()[".mara/engineering-execution.yaml"],
        include_str!("../templates/engineering-legacy-execution.yaml")
    );
    assert_eq!(legacy.files().len(), 5);
    let engine = initialized(Template::Engineering);
    assert_eq!(engine.files().len(), 9);
    let validation = documents("").project_validate(&[]).unwrap();
    assert!(
        validation.valid && validation.evaluation_complete,
        "{validation:?}"
    );
    assert!(
        engine
            .clone()
            .with_native_query_code(|| panic!("portable input must not acquire code"))
            .is_err()
    );
    let (_, schema, _) = engine.corpus().unwrap();
    assert_eq!(schema.flavours().len(), 11);
    for name in ["need", "validation"] {
        assert!(schema.flavours().contains_key(name));
    }
    for name in ["actor", "artifact"] {
        assert!(!schema.flavours().contains_key(name));
    }
    let matrix: Value =
        serde_json::from_str(include_str!("../templates/engineering-risk-matrix.json")).unwrap();
    assert_eq!(matrix["entries"].as_array().unwrap().len(), 25);
    // Loading both sets validates every definition, even with no matching roots.
    engine.query_execute(serde_json::from_value(json!({
        "definition_files":[".mara/engineering-queries.json", ".mara/engineering-views.json"],
        "definition":"requirement_basis","selection":{"flavours":["requirement"]},
        "max_rows":10000,"limit":100
    })).unwrap()).unwrap();
}

// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn accepted_basis_and_verification_are_separate_from_implementation_and_runs() {
    let source = knowledge();
    let engine = documents(&source);
    let valid = engine.project_validate(&[]).unwrap();
    assert!(valid.valid, "{valid:?}");
    assert!(
        bindings(&engine, "requirement_basis", "REQ-A")
            .to_string()
            .contains("REQ-PARENT")
    );
    let implementation = engine
        .query_execute(request("implemented", "REQ-A"))
        .unwrap();
    assert!(implementation.evaluation_complete, "{implementation:?}");
    assert_eq!(implementation.summary.binding_count, Some(0));
    let invalid = documents(
        &source
            .replace(
                ":status: accepted\n:methods: test",
                ":status: draft\n:methods: test",
            )
            .replace(
                ":status: accepted\n:methods: analysis",
                ":status: draft\n:methods: analysis",
            ),
    );
    assert!(!invalid.project_validate(&[]).unwrap().valid);
    let draft = documents(&item(0, "requirement", "REQ-DRAFT", "draft", ""));
    assert!(draft.project_validate(&[]).unwrap().valid);
    assert_eq!(
        gate(&draft, "implementation_ready", "REQ-DRAFT", vec![]).state,
        query_consumers::ConsumerState::Failed
    );
    assert_eq!(
        gate(&engine, "implementation_ready", "NEED-A", vec![]).state,
        query_consumers::ConsumerState::Failed
    );
}

// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn verification_requires_every_accepted_definition_and_applicable_results() {
    let engine = documents(&knowledge());
    for (second, expected) in [
        (Some("PASS"), query_consumers::ConsumerState::Passed),
        (Some("FAIL"), query_consumers::ConsumerState::Failed),
        (None, query_consumers::ConsumerState::Failed),
    ] {
        let result = gate(&engine, "verified", "REQ-A", vec![facts(second, "C")]);
        assert!(result.evaluation_complete, "{result:?}");
        assert_eq!(
            result.state,
            query_consumers::ConsumerState::Failed,
            "{result:?}"
        );
        let conformity = result.roots[0]
            .conditions
            .iter()
            .find(|c| c.id == "every-verification")
            .unwrap();
        assert_eq!(conformity.state, expected, "{result:?}");
    }
    let with_draft = documents(
        &(knowledge()
            + &item(
                10,
                "verification",
                "VER-DRAFT",
                "draft",
                ":verifies: REQ-A\n",
            )),
    );
    let result = gate(
        &with_draft,
        "verified",
        "REQ-A",
        vec![facts(Some("PASS"), "C")],
    );
    let conformity = result.roots[0]
        .conditions
        .iter()
        .find(|c| c.id == "every-verification")
        .unwrap();
    assert_eq!(conformity.state, query_consumers::ConsumerState::Passed);
    let stale = gate(
        &engine,
        "verified",
        "REQ-A",
        vec![facts(Some("PASS"), "OLD")],
    );
    assert_eq!(stale.state, query_consumers::ConsumerState::Unavailable);
    let mut wrong_revision = facts(Some("PASS"), "C");
    wrong_revision["records"][1]["fields"]["applicable"]["value"] = json!(false);
    let stale = gate(&engine, "verified", "REQ-A", vec![wrong_revision]);
    assert_eq!(
        stale.roots[0]
            .conditions
            .iter()
            .find(|c| c.id == "every-verification")
            .unwrap()
            .state,
        query_consumers::ConsumerState::Failed
    );
    assert_eq!(
        gate(&engine, "verified", "REQ-A", vec![]).state,
        query_consumers::ConsumerState::Unavailable
    );
}

// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn risk_context_does_not_expand_sibling_work_and_mitigation_does_not_lower_risk() {
    let risk = item(
        10,
        "risk",
        "RISK-A",
        "accepted",
        ":likelihood: likely\n:consequence: severe\n:threatens: GOAL-A\n",
    );
    let source = knowledge() + &risk;
    let engine = documents(&source);
    let scope = bindings(&engine, "risk_scope", "REQ-A").to_string();
    assert!(scope.contains("REQ-PARENT") && scope.contains("NEED-A") && scope.contains("GOAL-A"));
    assert!(!scope.contains("REQ-SIBLING"), "{scope}");
    assert_eq!(
        gate(&engine, "implementation_ready", "REQ-A", vec![]).state,
        query_consumers::ConsumerState::Failed
    );
    let mitigated = documents(&source.replace(
        ":satisfies: REQ-A\n",
        ":satisfies: REQ-A\n:mitigates: RISK-A\n",
    ));
    assert_eq!(
        gate(&mitigated, "implementation_ready", "REQ-A", vec![]).state,
        query_consumers::ConsumerState::Failed
    );
    let accepted =
        documents(&(source + &item(11, "decision", "ADR-RISK", "accepted", ":accepts: RISK-A\n")));
    assert_eq!(
        gate(&accepted, "implementation_ready", "REQ-A", vec![]).state,
        query_consumers::ConsumerState::Passed
    );
}

// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn terminology_and_historical_supersession_preserve_authority_boundaries() {
    let terms = item(0, "term", "TERM-A", "accepted", ":aliases: Customer\n")
        + &item(1, "term", "TERM-B", "accepted", ":aliases: CUSTOMER\n");
    assert!(!documents(&terms).project_validate(&[]).unwrap().valid);
    let history = item(0, "goal", "GOAL-A", "retired", "")
        + &item(1, "goal", "GOAL-B", "retired", ":supersedes: GOAL-A\n")
        + &item(2, "goal", "GOAL-C", "accepted", ":supersedes: GOAL-B\n");
    let result = documents(&history).project_validate(&[]).unwrap();
    assert!(result.valid, "{result:?}");
    let all_retired = documents(&history.replace(":status: accepted", ":status: retired"));
    assert!(all_retired.project_validate(&[]).unwrap().valid);
}

// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn current_profile_acceptance_rejects_invalid_strict_claims_and_endpoint_pairs() {
    let source = knowledge() + &item(10, "need", "NEED-DRAFT", "draft", "");
    let invalid_lifecycle = documents(&source.replace(
        ":decomposes: REQ-PARENT\n",
        ":decomposes: REQ-PARENT\n:satisfies: NEED-DRAFT\n",
    ));
    assert!(!invalid_lifecycle.project_validate(&[]).unwrap().valid);
    assert_eq!(
        gate(&invalid_lifecycle, "implementation_ready", "REQ-A", vec![]).state,
        query_consumers::ConsumerState::Failed
    );
    let invalid_pair = documents(&knowledge().replace(
        ":decomposes: REQ-PARENT\n",
        ":decomposes: REQ-PARENT\n:satisfies: REQ-PARENT\n",
    ));
    assert!(!invalid_pair.project_validate(&[]).unwrap().valid);
    assert_eq!(
        gate(&invalid_pair, "implementation_ready", "REQ-A", vec![]).state,
        query_consumers::ConsumerState::Failed
    );
    let invalid_basis = documents(&knowledge().replace(
        ":title: REQ-PARENT\n:status: accepted",
        ":title: REQ-PARENT\n:status: draft",
    ));
    assert_eq!(
        gate(&invalid_basis, "implementation_ready", "REQ-A", vec![]).state,
        query_consumers::ConsumerState::Failed
    );
}

// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn explicitly_required_validation_preserves_context_and_knowledge_completeness() {
    let engine = documents(&knowledge());
    let key = json!({"validation":{"type":"item","value":"VAL-A"},"context":{"type":"text","value":"staging"}});
    let snapshot = json!({"input":"validation_results","identity":"validation-run",
        "coverage":[key.clone()], "records":[{"keys":key,"fields":{
            "result":{"type":"text","value":"PASS"},
            "definition_revision":{"type":"text","value":"validation-definition-1"},
            "applicable":{"type":"boolean","value":true}
        }}]
    });
    assert_eq!(
        gate(
            &engine,
            "required_validation",
            "VAL-A",
            vec![snapshot.clone()]
        )
        .state,
        query_consumers::ConsumerState::Passed
    );
    let missing_method = documents(&knowledge().replace(":methods: demonstration\n", ""));
    assert_eq!(
        gate(
            &missing_method,
            "required_validation",
            "VAL-A",
            vec![snapshot.clone()]
        )
        .state,
        query_consumers::ConsumerState::Failed
    );
    let mut pending = snapshot.clone();
    pending["records"] = json!([]);
    assert_eq!(
        gate(&engine, "required_validation", "VAL-A", vec![pending]).state,
        query_consumers::ConsumerState::Failed
    );
    let mut old_context = snapshot;
    old_context["coverage"][0]["context"]["value"] = json!("other-context");
    old_context["records"][0]["keys"]["context"]["value"] = json!("other-context");
    assert_eq!(
        gate(&engine, "required_validation", "VAL-A", vec![old_context]).state,
        query_consumers::ConsumerState::Unavailable
    );
    assert_eq!(
        gate(&engine, "required_validation", "VAL-A", vec![]).state,
        query_consumers::ConsumerState::Unavailable
    );
    let mut raw = request("validation_status", "NEED-A");
    raw.parameters = vec![q::BindingParameter {
        name: "context".into(),
        value: q::ParameterValue::Text("staging".into()),
    }];
    raw.fact_snapshots = vec![];
    assert!(!engine.query_execute(raw).unwrap().evaluation_complete);
}

// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn current_profile_authoring_and_queries_execute_through_real_cli_and_mcp() {
    let project = documents("");
    let root = project._fixture.path();
    let flavours = [
        ("goal", "GOAL-DRAFT"),
        ("need", "NEED-DRAFT"),
        ("scenario", "SCN-DRAFT"),
        ("requirement", "REQ-DRAFT"),
        ("design", "DES-DRAFT"),
        ("decision", "ADR-DRAFT"),
        ("risk", "RISK-DRAFT"),
        ("term", "TERM-DRAFT"),
        ("evidence", "EVD-DRAFT"),
        ("verification", "VER-DRAFT"),
        ("validation", "VAL-DRAFT"),
    ];
    let mut calls = vec![
        support::mcp_initialize(1),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    ];
    for (index, (flavour, id)) in flavours.iter().enumerate() {
        calls.push(support::mcp_call(
            index as u64 + 2,
            "item_create",
            json!({
                "project":root,"flavour":flavour,"id":id,"file":"drafts.mara.md",
                "title":id,"body":"A draft with independently maintained engineering meaning.",
                "fields":[{"key":"status","value":"draft"}]
            }),
        ));
    }
    let responses = support::mcp_exchange(root, &calls);
    for index in 0..flavours.len() {
        assert_ne!(
            support::mcp_response(&responses, index as u64 + 2)["result"]["isError"],
            true
        );
    }
    let link = support::mara(
        root,
        &["relation", "add", "REQ-DRAFT", "satisfies", "NEED-DRAFT"],
    );
    assert!(link.status.success(), "{}", support::stderr(&link));
    let before = fs::read(root.join("drafts.mara.md")).unwrap();
    let rejected = support::mara(
        root,
        &["relation", "add", "VER-DRAFT", "verifies", "DES-DRAFT"],
    );
    assert!(!rejected.status.success());
    assert_eq!(before, fs::read(root.join("drafts.mara.md")).unwrap());
    assert!(project.project_validate(&[]).unwrap().valid);
    let expected = serde_json::to_value(
        project
            .query_execute(request("requirement_basis", "REQ-DRAFT"))
            .unwrap(),
    )
    .unwrap();
    let cli = support::mara(
        root,
        &[
            "--format",
            "json",
            "query",
            "execute",
            "requirement_basis",
            "--id",
            "REQ-DRAFT",
            "--definition-file",
            ".mara/engineering-queries.json",
            "--definition-file",
            ".mara/engineering-views.json",
            "--max-rows",
            "10000",
            "--limit",
            "100",
        ],
    );
    assert!(cli.status.success(), "{}", support::stderr(&cli));
    assert_eq!(
        serde_json::from_slice::<Value>(&cli.stdout).unwrap(),
        expected
    );
    let mut params = serde_json::to_value(request("requirement_basis", "REQ-DRAFT")).unwrap();
    params["project"] = json!(root);
    let responses = support::mcp_exchange(
        root,
        &[
            support::mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            support::mcp_call(2, "query_execute", params),
        ],
    );
    assert_eq!(
        support::mcp_response(&responses, 2)["result"]["structuredContent"],
        expected
    );
}

// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn factual_views_preserve_missing_roots_and_mixed_flavours_without_false_readiness() {
    let engine = documents(&knowledge());
    for (view, id) in [
        ("view_engineering_basis_and_rationale", "REQ-A"),
        ("view_engineering_basis_and_rationale", "DES-A"),
        ("view_knowledge_validity", "REQ-A"),
        ("view_knowledge_validity", "NEED-A"),
        ("view_change_impact", "REQ-A"),
    ] {
        let page = engine.query_execute(request(view, id)).unwrap();
        assert!(page.evaluation_complete, "{page:?}");
        assert_eq!(page.summary.selected_roots, 1);
    }
    let risk = documents(
        &(knowledge()
            + &item(
                10,
                "risk",
                "RISK-A",
                "accepted",
                ":likelihood: likely\n:consequence: severe\n:threatens: GOAL-A\n",
            )),
    );
    let mut risk_view = request("view_risk_relevance_and_treatment", "REQ-A");
    risk_view.mappings = vec![
        serde_json::from_str(include_str!("../templates/engineering-risk-matrix.json")).unwrap(),
    ];
    let page = risk.query_execute(risk_view).unwrap();
    assert!(page.evaluation_complete, "{page:?}");
    assert_eq!(page.summary.selected_roots, 1);
    let mut outcome = request("view_outcome_coverage_and_validation", "NEED-A");
    outcome.parameters = vec![q::BindingParameter {
        name: "context".into(),
        value: q::ParameterValue::Text("staging".into()),
    }];
    let key = json!({"validation":{"type":"item","value":"VAL-A"},"context":{"type":"text","value":"staging"}});
    outcome.fact_snapshots = vec![serde_json::from_value(json!({"input":"validation_results","identity":"view-run","coverage":[key.clone()],"records":[{"keys":key,"fields":{
        "result":{"type":"text","value":"PASS"},"definition_revision":{"type":"text","value":"definition-1"},"applicable":{"type":"boolean","value":true}
    }}]})).unwrap()];
    let page = engine.query_execute(outcome).unwrap();
    assert!(page.evaluation_complete, "{page:?}");
    assert_eq!(page.summary.selected_roots, 1);
    let mut implementation_view =
        request("view_requirement_implementation_and_verification", "REQ-A");
    implementation_view.parameters = vec![q::BindingParameter {
        name: "candidate".into(),
        value: q::ParameterValue::Text("C".into()),
    }];
    implementation_view.fact_snapshots =
        vec![serde_json::from_value(facts(Some("PASS"), "C")).unwrap()];
    assert!(
        engine
            .query_execute(implementation_view)
            .unwrap()
            .evaluation_complete
    );
    let draft = documents(&item(0, "requirement", "REQ-DRAFT", "draft", ""));
    let page = draft
        .query_execute(request("view_engineering_basis_and_rationale", "REQ-DRAFT"))
        .unwrap();
    assert!(page.evaluation_complete, "{page:?}");
    assert_eq!(page.summary.selected_roots, 1);
    assert!(!page.records.is_empty());
    let mut view = request("view_delivery_readiness", "REQ-A");
    view.parameters = vec![q::BindingParameter {
        name: "candidate".into(),
        value: q::ParameterValue::Text("C".into()),
    }];
    view.fact_snapshots = vec![serde_json::from_value(facts(Some("PASS"), "C")).unwrap()];
    view.mappings = vec![
        serde_json::from_str(include_str!("../templates/engineering-risk-matrix.json")).unwrap(),
    ];
    assert!(engine.query_execute(view).unwrap().evaluation_complete);
}

// @mara checks REQ-NAMED-QUERY-FACTS
// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn native_code_coverage_unblocks_gates_preserves_provenance_and_invalidates_cursors() {
    for target in ["REQ-A", "DES-A"] {
        let source = knowledge().replace(
            &format!(":title: {target}\n"),
            &format!(":title: {target}\n:implemented_by: code:implementation.rs\n"),
        );
        let engine = documents(&source);
        let root = engine._fixture.path();
        fs::write(
            root.join("implementation.rs"),
            "pub fn implementation() -> u32 { 1 }\n",
        )
        .unwrap();
        let page = engine
            .query_execute(request("implemented", "REQ-A"))
            .unwrap();
        assert!(page.evaluation_complete, "{page:?}");
        assert_eq!(page.summary.binding_count, Some(1));
        assert!(
            serde_json::to_string(&page.records)
                .unwrap()
                .contains("implementation.rs")
        );
        for name in ["verified", "release_ready"] {
            let result = gate(&engine, name, "REQ-A", vec![facts(Some("PASS"), "C")]);
            assert!(result.evaluation_complete, "{result:?}");
            assert_eq!(
                result.state,
                query_consumers::ConsumerState::Passed,
                "{result:?}"
            );
        }
        let mut req = request("implemented", "REQ-A");
        req.limit = 1;
        let before = engine.query_execute(req.clone()).unwrap();
        assert!(before.has_more);
        req.cursor = before.next_cursor;
        fs::write(
            root.join("implementation.rs"),
            "pub fn implementation() -> u32 { 2 }\n",
        )
        .unwrap();
        assert_eq!(
            engine.query_execute(req).unwrap_err().code,
            "invalid_argument"
        );

        let args = [
            "--execution-mode",
            "direct",
            "--format",
            "json",
            "query",
            "execute",
            "implemented",
            "--definition-file",
            ".mara/engineering-queries.json",
            "--definition-file",
            ".mara/engineering-views.json",
            "--max-rows",
            "10000",
            "--limit",
            "100",
            "--id",
            "REQ-A",
        ];
        let cli = support::mara(root, &args);
        assert!(
            cli.status.success(),
            "stderr={} stdout={}",
            support::stderr(&cli),
            String::from_utf8_lossy(&cli.stdout)
        );
        let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
        let responses = support::mcp_exchange(
            root,
            &[
                support::mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                support::mcp_call(
                    2,
                    "query_execute",
                    serde_json::to_value(request("implemented", "REQ-A")).unwrap(),
                ),
            ],
        );
        let mcp = support::mcp_response(&responses, 2);
        assert_ne!(mcp["result"]["isError"], true);
        assert!(mcp["result"]["structuredContent"]["evaluation_complete"] == true);
        assert_eq!(
            mcp["result"]["structuredContent"]["records"],
            cli["records"]
        );
    }

    let engine = documents(&knowledge());
    let root = engine._fixture.path();
    fs::write(root.join("unavailable.rs"), "pub fn unavailable() {}").unwrap();
    let config = root.join(".mara/project.toml");
    let original = fs::read_to_string(&config).unwrap();
    fs::write(config, format!("{original}\n[[code.languages]]\nname = \"rust\"\nextensions = [\"rs\"]\ncommand = [\"mara-test-missing-indexer\", \"{{output}}\"]\n")).unwrap();
    assert!(
        engine
            .query_execute(request("requirement_basis", "REQ-A"))
            .unwrap()
            .evaluation_complete
    );
    let result = gate(&engine, "verified", "REQ-A", vec![facts(Some("PASS"), "C")]);
    assert!(!result.evaluation_complete);
    assert_eq!(result.state, query_consumers::ConsumerState::Unavailable);
}

// @mara checks REQ-NAMED-QUERY-FACTS
// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn indexed_code_comments_are_shared_native_query_facts() {
    let engine = documents(&knowledge());
    let root = engine._fixture.path();
    let source = "// @mara implements REQ-A\npub fn implementation() {}\n";
    fs::write(root.join("implementation.rs"), source).unwrap();
    support::code_index::configure(root, "rust", &["rs"], true);
    support::code_index::write_single(
        root,
        "rust",
        "implementation.rs",
        "implementation",
        "implementation().",
    );
    let page = engine
        .query_execute(request("implemented", "REQ-A"))
        .unwrap();
    assert!(page.evaluation_complete, "{page:?}");
    assert_eq!(page.summary.binding_count, Some(1));
    let rendered = serde_json::to_string(&page.records).unwrap();
    assert!(rendered.contains("code_comment"));
    assert!(rendered.contains("implementation()."));
}

// @mara checks REQ-ENGINEERING-TEMPLATE
#[test]
fn validation_status_retains_individual_results_and_an_all_accepted_summary() {
    let engine = documents(
        &(knowledge()
            + &item(
                10,
                "validation",
                "VAL-B",
                "accepted",
                ":methods: test\n:validates: NEED-A\n",
            )),
    );
    let key = |id: &str| json!({"validation":{"type":"item","value":id},"context":{"type":"text","value":"staging"}});
    let record = |id: &str, result: &str| {
        json!({"keys":key(id),"fields":{
            "result":{"type":"text","value":result},
            "definition_revision":{"type":"text","value":"definition-1"},
            "applicable":{"type":"boolean","value":true}
        }})
    };
    for (second, expected) in [
        (Some("PASS"), "PASS"),
        (Some("FAIL"), "FAIL"),
        (None, "PENDING"),
    ] {
        let mut records = vec![record("VAL-A", "PASS")];
        if let Some(result) = second {
            records.push(record("VAL-B", result));
        }
        let mut req = request("validation_status", "NEED-A");
        req.parameters = vec![q::BindingParameter {
            name: "context".into(),
            value: q::ParameterValue::Text("staging".into()),
        }];
        req.fact_snapshots = vec![
            serde_json::from_value(
                json!({"input":"validation_results","identity":"validation-run",
                    "coverage":[key("VAL-A"),key("VAL-B")],"records":records
                }),
            )
            .unwrap(),
        ];
        let page = engine.query_execute(req).unwrap();
        assert!(page.evaluation_complete, "{page:?}");
        assert_eq!(page.summary.binding_count, Some(2));
        for record in page.records {
            if let q::Record::Binding { binding, .. } = record {
                assert!(
                    matches!(&binding.values["summary"], Some(q::Value::Text {value, ..}) if value == expected)
                );
                assert!(matches!(
                    &binding.values["validation_count"],
                    Some(q::Value::Integer { value: 2 })
                ));
                assert!(binding.values["validation"].is_some());
            }
        }
    }
    let empty = documents(
        &(item(0, "need", "NEED-EMPTY", "accepted", "")
            + &item(
                1,
                "validation",
                "VAL-DRAFT",
                "draft",
                ":validates: NEED-EMPTY\n",
            )),
    );
    let mut req = request("validation_summary", "NEED-EMPTY");
    req.parameters = vec![q::BindingParameter {
        name: "context".into(),
        value: q::ParameterValue::Text("staging".into()),
    }];
    req.fact_snapshots = vec![
        serde_json::from_value(
            json!({"input":"validation_results","identity":"empty","coverage":[],"records":[]}),
        )
        .unwrap(),
    ];
    let page = empty.query_execute(req).unwrap();
    assert!(page.evaluation_complete, "{page:?}");
    assert!(page.records.iter().any(|record| matches!(record, q::Record::Binding {binding, ..} if matches!(&binding.values["state"], Some(q::Value::Text {value, ..}) if value == "PENDING") && matches!(&binding.values["count"], Some(q::Value::Integer {value:0})))));
}
