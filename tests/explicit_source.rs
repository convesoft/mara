//! Native process parity and explicit-input source-engine regressions.
mod support;
use mara_engine::*;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{fs, path::Path};

const SCHEMA: &str = r#"format_version: 3
flavours:
  requirement:
    description: An obligation
    use_when: [State an obligation]
    avoid_when: []
    distinguish_from: {}
    id_prefix: REQ-
    body: required
    fields:
      status: {type: string}
relations:
  depends_on:
    description: Depends on a requirement
    source: [requirement]
    target: [requirement]
    inverse: prerequisite_of
  implements:
    description: Native code realizes an obligation
    source: []
    target: [requirement]
    code_source: true
    inverse: implemented_by
"#;
const A: &str = "# Café context\r\n\r\n:::mara requirement REQ-A\r\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\r\n:title: Straße café\r\n:status: accepted\r\n:depends_on: REQ-B\r\n:depends_on: REQ-B\r\n\r\nUnicode 👩🏽‍💻 café. [[depends_on:REQ-B]] [Target](b.mara.md#target)\r\n:::\r\n";
const B: &str = "# Target\n\n:::mara requirement REQ-B\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: Target\n\nA requirement.\n:::\n";
fn decode<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}
fn fixture() -> tempfile::TempDir {
    let fixture = support::fixture();
    let root = fixture.path();
    let output = support::mara(root, &["project", "init"]);
    assert!(output.status.success(), "{}", support::stderr(&output));
    fs::write(root.join(SCHEMA_FILE), SCHEMA).unwrap();
    fs::write(root.join("a.mara.md"), A).unwrap();
    fs::write(root.join("b.mara.md"), B).unwrap();
    fixture
}
fn snapshot(root: &Path) -> SourceSnapshot {
    let mut files = vec![];
    for path in [
        PROJECT_FILE,
        SCHEMA_FILE,
        "a.mara.md",
        "b.mara.md",
        ".mara/rules.yaml",
    ] {
        if let Ok(content) = fs::read_to_string(root.join(path)) {
            files.push(SourceFile {
                path: path.into(),
                content,
            });
        }
    }
    SourceSnapshot {
        format_version: 1,
        files,
        selection: SelectionInputs {
            document_paths: vec!["a.mara.md".into(), "b.mara.md".into()],
            ignores: vec![],
        },
        base_identity: Some("opaque caller identity".into()),
    }
}
fn cli(root: &Path, args: &[&str]) -> Value {
    let mut arguments = vec!["--format", "json"];
    arguments.extend_from_slice(args);
    let output = support::mara(root, &arguments);
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("{}", support::stderr(&output)))
}
fn validation_value(result: ValidationResult) -> Value {
    let mut value = serde_json::to_value(result).unwrap();
    value.as_object_mut().unwrap().remove("project");
    value.as_object_mut().unwrap().remove("path");
    value["next_cursor"] = Value::Null;
    value
}
fn normalized_validation(mut value: Value) -> Value {
    let root = value["project"].as_str().unwrap_or("").to_owned();
    for diagnostic in value["diagnostics"].as_array_mut().unwrap() {
        if let Some(message) = diagnostic["message"].as_str() {
            diagnostic["message"] = Value::String(message.replace(&root, "."));
        }
    }
    value.as_object_mut().unwrap().remove("project");
    value.as_object_mut().unwrap().remove("path");
    value["next_cursor"] = Value::Null;
    value
}
// @mara checks REQ-PORTABLE-DOCUMENT-ENGINE
// @mara checks DES-EXECUTION-BOUNDARIES
// @mara checks DES-EXPLICIT-SOURCE-ENGINE
#[test]
fn native_cli_mcp_and_explicit_queries_preserve_outcomes_and_continuations() {
    let fixture = fixture();
    let root = fixture.path();
    let engine = SourceEngine::new(snapshot(root)).unwrap();
    let project = mara::resolve_project(Some(root), root).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let docs = mara::load_documents(&project, &schema).unwrap();
    let bridge = mara::load_source_engine(&project, &docs).unwrap();
    assert_eq!(bridge.source_fingerprint(), engine.source_fingerprint());
    assert_eq!(engine.base_identity(), Some("opaque caller identity"));
    let mut provenance = snapshot(root);
    provenance.base_identity = Some("different provenance".into());
    provenance.files.reverse();
    assert_eq!(
        SourceEngine::new(provenance).unwrap().source_fingerprint(),
        engine.source_fingerprint()
    );
    assert_eq!(
        serde_json::to_value(engine.schema_get(None, None).unwrap()).unwrap(),
        cli(root, &["schema", "get"])
    );
    assert_eq!(
        serde_json::to_value(engine.schema_list(SchemaKind::Relation).unwrap()).unwrap(),
        cli(root, &["schema", "list", "relation"])
    );
    assert_eq!(
        serde_json::to_value(
            engine
                .schema_get(Some(SchemaKind::Relation), Some("prerequisite_of".into()))
                .unwrap()
        )
        .unwrap(),
        cli(root, &["schema", "get", "relation", "prerequisite_of"])
    );
    let first = engine.item_list(decode(json!({"limit":1}))).unwrap();
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        cli(root, &["item", "list", "--limit", "1"])
    );
    let cursor = first.next_cursor.unwrap();
    let next = engine
        .item_list(decode(json!({"limit":1,"cursor":cursor})))
        .unwrap();
    assert!(!next.has_more);
    assert_eq!(next.items.len(), 1);
    let searched = engine
        .search(decode(json!({"query":"STRASSE café"})))
        .unwrap();
    assert_eq!(
        serde_json::to_value(&searched).unwrap(),
        cli(root, &["search", "STRASSE café"])
    );
    let reference = searched.results[0].node.reference.clone();
    let gotten = engine.get(decode(json!({"reference":reference}))).unwrap();
    assert_eq!(
        serde_json::to_value(&gotten).unwrap(),
        cli(root, &["get", &reference])
    );
    let related = engine
        .related(decode(json!({"reference":"REQ-A"})))
        .unwrap();
    assert_eq!(
        serde_json::to_value(&related).unwrap(),
        cli(root, &["related", "REQ-A"])
    );
    let relation = engine
        .relation_get(
            decode(json!({"source":"REQ-A","relation":"depends_on","target":"REQ-B"})),
            Some(1),
            None,
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(&relation).unwrap(),
        cli(
            root,
            &[
                "relation",
                "get",
                "REQ-A",
                "depends_on",
                "REQ-B",
                "--limit",
                "1"
            ]
        )
    );
    assert_eq!(relation.occurrence_count, 3);
    let mut cursor = relation.next_cursor;
    let mut count = relation.occurrences.len();
    while let Some(token) = cursor {
        let page = engine
            .relation_get(
                decode(json!({"source":"REQ-A","relation":"depends_on","target":"REQ-B"})),
                Some(1),
                Some(token),
            )
            .unwrap();
        count += page.occurrences.len();
        cursor = page.next_cursor;
    }
    assert_eq!(count, 3);
    let responses = support::mcp_exchange(
        root,
        &[
            support::mcp_initialize(1),
            support::mcp_call(2, "get", json!({"reference":"REQ-A"})),
            support::mcp_call(3, "schema_list", json!({"kind":"relation"})),
            support::mcp_call(4, "project_validate", json!({})),
        ],
    );
    assert_eq!(
        support::mcp_response(&responses, 2)["result"]["structuredContent"],
        serde_json::to_value(engine.get(decode(json!({"reference":"REQ-A"}))).unwrap()).unwrap()
    );
    assert_eq!(
        support::mcp_response(&responses, 3)["result"]["structuredContent"],
        serde_json::to_value(engine.schema_list(SchemaKind::Relation).unwrap()).unwrap()
    );
    let validated = engine
        .project_validate(&[], &ValidationOptions::default(), None)
        .unwrap();
    assert!(validated.valid && validated.evaluation_complete);
    assert_eq!(
        validation_value(validated),
        normalized_validation(
            support::mcp_response(&responses, 4)["result"]["structuredContent"].clone()
        )
    );
}
#[test]
fn source_conformance_recovery_graph_policies_and_missing_inputs_match_native() {
    let fixture = fixture();
    let root = fixture.path();
    let schema=SCHEMA.replace("    inverse: prerequisite_of","    inverse: prerequisite_of\n    cardinality: {outgoing: {minimum: 1}}\n    acyclic: {severity: warning}");
    fs::write(root.join(SCHEMA_FILE), &schema).unwrap();
    let b = B.replace("\n:title: Target", "\n:depends_on: REQ-A\n:title: Target");
    fs::write(root.join("b.mara.md"), &b).unwrap();
    let engine = SourceEngine::new(snapshot(root)).unwrap();
    let policy = engine
        .project_validate(&[], &ValidationOptions::default(), None)
        .unwrap();
    assert!(policy.valid);
    assert!(
        policy
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::RelationCycle)
    );
    assert_eq!(
        validation_value(policy),
        normalized_validation(cli(root, &["project", "validate"]))
    );
    fs::write(root.join("a.mara.md"), A.replace("REQ-B", "REQ-MISSING")).unwrap();
    let engine = SourceEngine::new(snapshot(root)).unwrap();
    let invalid = engine
        .item_validate("REQ-A", &ValidationOptions::default(), None)
        .unwrap();
    assert!(!invalid.valid && !invalid.evaluation_complete);
    assert!(
        invalid
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::ReferenceUnresolved)
    );
    assert_eq!(
        validation_value(invalid),
        normalized_validation(cli(root, &["item", "validate", "REQ-A"]))
    );
    fs::write(
        root.join(SCHEMA_FILE),
        "format_version: 3\nflavours: [bad]\nrelations: {}\n",
    )
    .unwrap();
    let engine = SourceEngine::new(snapshot(root)).unwrap();
    let invalid = engine
        .schema_validate(&ValidationOptions::default(), None)
        .unwrap();
    assert!(!invalid.valid && !invalid.evaluation_complete);
    assert_eq!(
        validation_value(invalid),
        normalized_validation(cli(root, &["schema", "validate"]))
    );
    let mut missing = snapshot(root);
    missing.files.retain(|f| f.path != SCHEMA_FILE);
    let engine = SourceEngine::new(missing).unwrap();
    let missing = engine
        .schema_validate(&ValidationOptions::default(), None)
        .unwrap();
    assert!(!missing.valid && !missing.evaluation_complete);
    assert!(
        missing
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::ProjectInvalid)
    );
    fs::remove_file(root.join(SCHEMA_FILE)).unwrap();
    assert_eq!(
        validation_value(missing),
        normalized_validation(cli(root, &["schema", "validate"]))
    );
    let mut missing = snapshot(root);
    missing.files.retain(|f| f.path != PROJECT_FILE);
    let engine = SourceEngine::new(missing).unwrap();
    assert!(
        engine
            .project_validate(&[], &ValidationOptions::default(), None)
            .is_err()
    );
}
// @mara checks DES-BROWSER-PROJECT-SNAPSHOT
#[test]
fn explicit_inventory_paths_ignore_context_and_stale_state_are_not_invented() {
    let fixture = fixture();
    let root = fixture.path();
    let original = snapshot(root);
    let engine = SourceEngine::new(original.clone()).unwrap();
    let list = engine.item_list(decode(json!({"limit":1}))).unwrap();
    let structure = engine
        .search(decode(json!({"query":"context"})))
        .unwrap()
        .results[0]
        .node
        .reference
        .clone();
    let mut changed = original.clone();
    changed.files[0]
        .content
        .push_str("\n# configuration comment\n");
    let changed = SourceEngine::new(changed).unwrap();
    assert_ne!(engine.source_fingerprint(), changed.source_fingerprint());
    assert!(
        changed
            .item_list(decode(json!({"limit":1,"cursor":list.next_cursor})))
            .is_err()
    );
    let mut changed = original.clone();
    changed
        .files
        .iter_mut()
        .find(|f| f.path == "a.mara.md")
        .unwrap()
        .content
        .push_str("\nchanged\n");
    let changed = SourceEngine::new(changed).unwrap();
    assert!(changed.get(decode(json!({"reference":structure}))).is_err());
    let mut missing = original.clone();
    missing.files.retain(|f| f.path != "b.mara.md");
    let engine = SourceEngine::new(missing).unwrap();
    assert!(engine.item_list(ItemFilterParams::default()).is_err());
    let result = engine
        .project_validate(&[], &ValidationOptions::default(), None)
        .unwrap();
    assert!(!result.valid && !result.evaluation_complete);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("missing required selected document"))
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::ReferenceUnresolved)
    );
    for path in [
        "../escape.mara.md",
        "/absolute.mara.md",
        "C:/host.mara.md",
        "docs\\windows.mara.md",
    ] {
        let mut bad = original.clone();
        bad.files.push(SourceFile {
            path: path.into(),
            content: String::new(),
        });
        assert!(SourceEngine::new(bad).is_err());
    }
    let mut bad = original.clone();
    bad.files.push(SourceFile {
        path: "./a.mara.md".into(),
        content: String::new(),
    });
    assert!(SourceEngine::new(bad).is_err());
    let mut missing = original.clone();
    missing.selection.document_paths.clear();
    assert!(
        SourceEngine::new(missing)
            .unwrap()
            .item_list(ItemFilterParams::default())
            .is_err()
    );
    let mut ignored = original.clone();
    ignored.selection.ignores.push(IgnoreInput {
        directory: "".into(),
        root_prefix: Vec::new(),
        content: "draft/\n*.secret.mara.md\n".into(),
    });
    let engine = SourceEngine::new(ignored).unwrap();
    let (project, _, _) = engine.corpus().unwrap();
    assert!(
        !engine
            .document_is_selected(&project, "draft/new.mara.md")
            .unwrap()
    );
    assert!(
        !engine
            .document_is_selected(&project, "new.secret.mara.md")
            .unwrap()
    );
    assert!(
        engine
            .document_is_selected(&project, "new.mara.md")
            .unwrap()
    );
    let mut child_ignore = original.clone();
    child_ignore.selection.ignores.extend([
        IgnoreInput {
            directory: String::new(),
            root_prefix: Vec::new(),
            content: "blocked/\n".into(),
        },
        IgnoreInput {
            directory: "blocked".into(),
            root_prefix: Vec::new(),
            content: "!resurrect.mara.md\n".into(),
        },
    ]);
    let engine = SourceEngine::new(child_ignore).unwrap();
    let (project, _, _) = engine.corpus().unwrap();
    assert!(
        !engine
            .document_is_selected(&project, "blocked/resurrect.mara.md")
            .unwrap()
    );
    let mut inconsistent = original.clone();
    inconsistent.selection.ignores.extend([
        IgnoreInput {
            directory: String::new(),
            root_prefix: vec!["one".into()],
            content: String::new(),
        },
        IgnoreInput {
            directory: String::new(),
            root_prefix: vec!["two".into()],
            content: String::new(),
        },
    ]);
    assert!(SourceEngine::new(inconsistent).is_err());
    let mut paged = original.clone();
    paged
        .files
        .iter_mut()
        .find(|f| f.path == "a.mara.md")
        .unwrap()
        .content = A.replace("REQ-B", "REQ-MISSING");
    let engine = SourceEngine::new(paged.clone()).unwrap();
    let options = ValidationOptions {
        limit: Some(1),
        cursor: None,
    };
    let first = engine.project_validate(&[], &options, None).unwrap();
    assert!(first.has_more);
    let full_summary = serde_json::to_value(&first.summary).unwrap();
    let mut cursor = first.next_cursor;
    while let Some(token) = cursor {
        let page = engine
            .project_validate(
                &[],
                &ValidationOptions {
                    limit: Some(1),
                    cursor: Some(token),
                },
                None,
            )
            .unwrap();
        assert_eq!(serde_json::to_value(&page.summary).unwrap(), full_summary);
        cursor = page.next_cursor;
    }
    let first = engine.project_validate(&[], &options, None).unwrap();
    paged.files[0].content.push_str("\n# change\n");
    assert!(
        SourceEngine::new(paged)
            .unwrap()
            .project_validate(
                &[],
                &ValidationOptions {
                    limit: Some(1),
                    cursor: first.next_cursor
                },
                None
            )
            .is_err()
    );
    let mut unicode = original;
    unicode
        .files
        .iter_mut()
        .find(|f| f.path == "a.mara.md")
        .unwrap()
        .content = A.replace("Unicode 👩🏽‍💻 café.", &"é👩🏽‍💻".repeat(20_000));
    let engine = SourceEngine::new(unicode).unwrap();
    let mut cursor = None;
    let mut body = String::new();
    let mut metadata = vec![];
    loop {
        let page = engine
            .get(GetParams {
                reference: "REQ-A".into(),
                cursor,
            })
            .unwrap();
        body.push_str(&page.content);
        metadata.extend(page.metadata);
        if !page.has_more {
            break;
        }
        cursor = page.next_cursor;
    }
    assert!(body.contains(&"é👩🏽‍💻".repeat(20_000)));
    assert_eq!(metadata.iter().filter(|m| m.key == "depends_on").count(), 2);
}
// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
#[test]
fn code_and_rule_absence_are_explicit_and_native_rule_outcomes_remain_real() {
    let fixture = fixture();
    let root = fixture.path();
    let mut configured = snapshot(root);
    let config = configured
        .files
        .iter_mut()
        .find(|f| f.path == PROJECT_FILE)
        .unwrap();
    config.content = config
        .content
        .replace("format_version = 1", "format_version = 3");
    config.content.push_str("\n[[code.languages]]\nname = 'rust'\nextensions = ['rs']\ncommand = ['sh', '-c', 'touch MUST_NOT_EXECUTE', '{output}']\n");
    let engine = SourceEngine::new(configured).unwrap();
    assert!(
        engine
            .schema_validate(&ValidationOptions::default(), None)
            .unwrap()
            .valid
    );
    assert_eq!(
        engine
            .item_list(ItemFilterParams::default())
            .unwrap()
            .items
            .len(),
        2
    );
    let unavailable = engine
        .project_validate(&[], &ValidationOptions::default(), None)
        .unwrap();
    assert!(!unavailable.valid && !unavailable.evaluation_complete);
    assert!(
        unavailable
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::EvaluationUnavailable)
    );
    assert!(!root.join("MUST_NOT_EXECUTE").exists());
    assert!(
        engine
            .related(decode(
                json!({"reference":"REQ-A","relations":["builtin:contains"]})
            ))
            .is_ok()
    );
    assert!(
        engine
            .related(decode(json!({"reference":"REQ-A"})))
            .is_err()
    );
    let mut code = snapshot(root);
    code.files
        .iter_mut()
        .find(|f| f.path == "a.mara.md")
        .unwrap()
        .content = A.replace(
        ":status: accepted",
        ":status: accepted\r\n:implemented_by: code:unconfigured.bin",
    );
    let engine = SourceEngine::new(code).unwrap();
    assert!(engine.get(decode(json!({"reference":"REQ-A"}))).is_ok());
    assert!(
        !engine
            .item_validate("REQ-B", &ValidationOptions::default(), None)
            .unwrap()
            .evaluation_complete
    );
    assert!(
        engine
            .related(decode(json!({"reference":"REQ-A"})))
            .is_err()
    );
    assert!(
        engine
            .get(decode(json!({"reference":"code:unconfigured.bin"})))
            .is_err()
    );
    let config = fs::read_to_string(root.join(PROJECT_FILE))
        .unwrap()
        .replace("format_version = 1", "format_version = 2")
        + "\n[rules]\nformat_version = 1\nfiles = ['.mara/rules.yaml']\n";
    fs::write(root.join(PROJECT_FILE), config).unwrap();
    fs::write(root.join(".mara/rules.yaml"),"id: rule:accepted\ntargetClass: requirement\nproperty:\n  - path: status\n    hasValue: accepted\n").unwrap();
    let engine = SourceEngine::new(snapshot(root)).unwrap();
    let absent = engine
        .schema_validate(&ValidationOptions::default(), None)
        .unwrap();
    assert!(!absent.valid && !absent.evaluation_complete);
    assert!(engine.schema_list(SchemaKind::Flavour).is_ok());
    let project = mara::resolve_project(Some(root), root).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let (corpus, diagnostics) = mara::load_corpus_for_validation(&project, &schema).unwrap();
    let target = ValidationTarget {
        kind: ValidationTargetKind::Project,
        id: None,
    };
    let checks = mara::load_rule_checks(
        &project,
        &schema,
        &engine,
        target,
        Some((&corpus, &diagnostics)),
    )
    .unwrap();
    let replacement = std::collections::BTreeMap::from([(
        std::path::PathBuf::from("b.mara.md"),
        B.replace(":title: Target", ":title: Target\n:status: accepted"),
    )]);
    let candidate = corpus.with_replacements(&replacement, &schema).unwrap();
    assert!(candidate.source_fingerprint.is_empty());
    assert!(
        mara::load_rule_checks(
            &project,
            &schema,
            &engine,
            ValidationTarget {
                kind: ValidationTargetKind::Project,
                id: None
            },
            Some((&candidate, &diagnostics))
        )
        .is_err()
    );
    let unchanged = corpus
        .with_replacements(&std::collections::BTreeMap::new(), &schema)
        .unwrap();
    assert!(
        mara::load_rule_checks(
            &project,
            &schema,
            &engine,
            ValidationTarget {
                kind: ValidationTargetKind::Project,
                id: None
            },
            Some((&unchanged, &diagnostics))
        )
        .is_ok()
    );
    let checked = engine
        .project_validate(&[], &ValidationOptions::default(), Some(&checks))
        .unwrap();
    assert!(
        !checked.valid && checked.evaluation_complete,
        "{checked:#?}"
    );
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::RuleFailed)
    );
    assert_eq!(
        validation_value(checked),
        normalized_validation(cli(root, &["project", "validate"]))
    );
    let definitions = mara::load_rule_checks(
        &project,
        &schema,
        &engine,
        ValidationTarget {
            kind: ValidationTargetKind::Schema,
            id: None,
        },
        None,
    )
    .unwrap();
    assert!(
        engine
            .schema_validate(&ValidationOptions::default(), Some(&definitions))
            .unwrap()
            .valid
    );
    let mut changed = snapshot(root);
    changed
        .files
        .iter_mut()
        .find(|f| f.path == "b.mara.md")
        .unwrap()
        .content
        .push_str("\nchanged\n");
    assert!(
        SourceEngine::new(changed)
            .unwrap()
            .project_validate(&[], &ValidationOptions::default(), Some(&checks))
            .is_err()
    );
}

// @mara checks DES-BROWSER-PROJECT-SNAPSHOT
#[test]
fn native_inherited_and_nested_ignores_match_explicit_membership() {
    let fixture = support::fixture();
    let parent = fixture.path();
    let output = support::mara(parent, &["project", "init", "logical-project"]);
    assert!(output.status.success(), "{}", support::stderr(&output));
    let root = parent.join("logical-project");
    fs::write(parent.join(".gitignore"), "/logical-project/draft/\n").unwrap();
    fs::write(root.join(".gitignore"), "docs/*\n!docs/keep.mara.md\n").unwrap();
    fs::write(root.join(SCHEMA_FILE), SCHEMA).unwrap();
    fs::create_dir_all(root.join("draft")).unwrap();
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(
        root.join("a.mara.md"),
        A.replace("[Target](b.mara.md#target)", ""),
    )
    .unwrap();
    fs::write(root.join("docs/keep.mara.md"), B).unwrap();
    fs::write(root.join("draft/hidden.mara.md"), "hidden").unwrap();
    fs::write(root.join("docs/hidden.mara.md"), "hidden").unwrap();
    let project = mara::resolve_project(Some(&root), &root).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let docs = mara::load_documents(&project, &schema).unwrap();
    let engine = mara::load_source_engine(&project, &docs).unwrap();
    assert_eq!(docs.documents().len(), 2);
    assert_eq!(
        serde_json::to_value(engine.item_list(ItemFilterParams::default()).unwrap()).unwrap(),
        cli(&root, &["item", "list"])
    );
    for path in ["draft/new.mara.md", "docs/new.mara.md"] {
        assert!(!engine.document_is_selected(&project, path).unwrap());
        let output = support::mara(
            &root,
            &[
                "item",
                "create",
                "requirement",
                "REQ-NEW",
                path,
                "--title",
                "New",
                "--body",
                "Body",
            ],
        );
        assert!(!output.status.success());
        assert!(!root.join(path).exists());
    }
    assert!(
        engine
            .document_is_selected(&project, "docs/keep.mara.md")
            .unwrap()
    );
    assert!(
        engine
            .document_is_selected(&project, "new.mara.md")
            .unwrap()
    );
}

#[cfg(unix)]
#[test]
fn native_nonportable_filenames_do_not_narrow_existing_reads() {
    let fixture = fixture();
    let root = fixture.path();
    fs::write(
        root.join("literal\\\\name.mara.md"),
        B.replace("REQ-B", "REQ-C")
            .replace("01ARZ3NDEKTSV4RRFFQ69G5F01", "01ARZ3NDEKTSV4RRFFQ69G5F02"),
    )
    .unwrap();
    let listed = cli(root, &["item", "list"]);
    assert_eq!(listed["items"].as_array().unwrap().len(), 3);
    assert_eq!(cli(root, &["get", "REQ-C"])["node"]["id"], "REQ-C");
}

// @mara checks DES-EXPLICIT-SOURCE-ENGINE
#[test]
fn incomplete_native_projection_and_invalid_selection_preserve_failure() {
    let fixture = fixture();
    let root = fixture.path();
    fs::write(
        root.join("bad.mara.md"),
        ":::mara requirement REQ-BAD\n:title: Bad\n\nmissing closing delimiter\n",
    )
    .unwrap();
    let project = mara::resolve_project(Some(root), root).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let (documents, diagnostics) = mara::load_documents_for_validation(&project, &schema).unwrap();
    assert!(!documents.is_complete());
    assert!(!diagnostics.is_empty());
    assert!(mara::load_source_engine(&project, &documents).is_err());
    let native = cli(root, &["project", "validate"]);
    assert_eq!(native["valid"], false);
    assert_eq!(native["evaluation_complete"], false);
    fs::remove_file(root.join("bad.mara.md")).unwrap();
    let original = snapshot(root);
    let mut invalid = original.clone();
    invalid
        .files
        .iter_mut()
        .find(|file| file.path == PROJECT_FILE)
        .unwrap()
        .content = fs::read_to_string(root.join(PROJECT_FILE))
        .unwrap()
        .replace("include = [\"**/*.mara.md\"]", "include = [17]");
    // The initialization template may include a different whitespace style.
    let config = &mut invalid
        .files
        .iter_mut()
        .find(|file| file.path == PROJECT_FILE)
        .unwrap()
        .content;
    assert!(config.contains("include = [17]"), "{config}");
    fs::write(root.join(PROJECT_FILE), &*config).unwrap();
    let result = SourceEngine::new(invalid)
        .unwrap()
        .project_validate(&[], &ValidationOptions::default(), None)
        .unwrap();
    assert!(!result.valid && !result.evaluation_complete);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::ProjectInvalid
                && d.location.pointer.as_deref() == Some("/content/include/0"))
    );
    assert_eq!(
        validation_value(result),
        normalized_validation(cli(root, &["project", "validate"]))
    );
    for path in [
        "./C:/host.mara.md",
        ".//./C:/host.mara.md",
        "./C:relative.mara.md",
    ] {
        assert!(normalize_source_path(path).is_err());
        let mut bad = original.clone();
        bad.files.push(SourceFile {
            path: path.into(),
            content: String::new(),
        });
        assert!(SourceEngine::new(bad).is_err());
    }
}

// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
// @mara checks DES-EXPLICIT-SOURCE-ENGINE
#[test]
fn related_capabilities_follow_node_direction_and_neighbour_filters() {
    let fixture = fixture();
    let root = fixture.path();
    fs::write(root.join("unconfigured.bin"), [0xff, 0x00]).unwrap();
    fs::write(
        root.join("b.mara.md"),
        B.replace(
            ":title: Target",
            ":title: Target\n:implemented_by: code:unconfigured.bin",
        ),
    )
    .unwrap();
    let source = snapshot(root);
    let engine = SourceEngine::new(source.clone()).unwrap();
    for (reference, arguments, options) in [
        (
            "REQ-A",
            vec!["related", "REQ-A"],
            json!({"reference":"REQ-A"}),
        ),
        (
            "REQ-A",
            vec!["related", "REQ-A", "--direction", "outgoing"],
            json!({"reference":"REQ-A","direction":"outgoing"}),
        ),
        (
            "REQ-B",
            vec!["related", "REQ-B", "--direction", "outgoing"],
            json!({"reference":"REQ-B","direction":"outgoing"}),
        ),
        (
            "REQ-B",
            vec![
                "related",
                "REQ-B",
                "--direction",
                "incoming",
                "--flavour",
                "requirement",
            ],
            json!({"reference":"REQ-B","direction":"incoming","flavours":["requirement"]}),
        ),
    ] {
        let result = engine
            .related(decode(options))
            .unwrap_or_else(|error| panic!("{reference}: {error}"));
        assert_eq!(serde_json::to_value(result).unwrap(), cli(root, &arguments));
    }
    assert!(
        engine
            .related(decode(json!({"reference":"REQ-B","direction":"incoming"})))
            .is_err()
    );
    let context = engine
        .search(decode(json!({"query":"context"})))
        .unwrap()
        .results[0]
        .node
        .reference
        .clone();
    let mut indexed = source;
    let config = &mut indexed
        .files
        .iter_mut()
        .find(|file| file.path == PROJECT_FILE)
        .unwrap()
        .content;
    *config = config.replace("format_version = 1", "format_version = 3");
    config.push_str("\n[[code.languages]]\nname = 'rust'\nextensions = ['rs']\ncommand = ['MUST_NOT_EXECUTE', '{output}']\n");
    let engine = SourceEngine::new(indexed).unwrap();
    assert!(
        engine
            .related(decode(json!({"reference":"REQ-A","direction":"outgoing"})))
            .is_ok()
    );
    assert!(
        engine
            .related(decode(json!({"reference":"REQ-A","direction":"symmetric"})))
            .is_ok()
    );
    assert!(
        engine
            .related(decode(
                json!({"reference":"REQ-A","direction":"incoming","flavours":["requirement"]})
            ))
            .is_ok()
    );
    assert!(engine.related(decode(json!({"reference":context}))).is_ok());
    assert!(
        engine
            .related(decode(json!({"reference":"REQ-A"})))
            .is_err()
    );
    assert!(
        !engine
            .project_validate(&[], &ValidationOptions::default(), None)
            .unwrap()
            .evaluation_complete
    );
}

// @mara checks DES-BROWSER-PROJECT-SNAPSHOT
#[test]
fn supplied_bom_ignore_text_matches_native_discovery() {
    let fixture = fixture();
    let root = fixture.path();
    let ignore = "\u{feff}\u{feff}a.mara.md\n";
    fs::write(root.join(".gitignore"), ignore).unwrap();
    let project = mara::resolve_project(Some(root), root).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let documents = mara::load_documents(&project, &schema).unwrap();
    assert_eq!(documents.documents().len(), 1);
    let bridge = mara::load_source_engine(&project, &documents).unwrap();
    assert!(!bridge.document_is_selected(&project, "a.mara.md").unwrap());
    assert_eq!(
        serde_json::to_value(bridge.item_list(ItemFilterParams::default()).unwrap()).unwrap(),
        cli(root, &["item", "list"])
    );
    let mut supplied = snapshot(root);
    supplied.selection.ignores.push(IgnoreInput {
        directory: String::new(),
        root_prefix: vec![],
        content: ignore.into(),
    });
    assert!(
        SourceEngine::new(supplied)
            .unwrap()
            .item_list(ItemFilterParams::default())
            .is_err()
    );
}
