use std::fs;

use mara_core::{
    DiagnosticCode, DiagnosticSeverity, DiagnosticValue, ReferenceDiagnosticCode,
    RelationDiagnosticCode, RuleDiagnosticCode, SourceDocument, SourceText, ValidationPhase,
    ValidationPhaseState,
};
use mara_engine::{
    check_project, check_schema, project::load_from_root, schema::load_schema, validate_documents,
};
use mara_markdown::{ParsedDocument, parse_document};
use mara_test_support::{ProjectSandbox, ProjectSandboxMode};

const VALIDATION_SCHEMA: &str = r#"format_version: 1
schema:
  name: validation-fixture
  version: 1.0.0
identity:
  mid:
    format: ulid
    prefix: m_
flavours:
  alpha:
    label: Alpha
    description: Arbitrary source item.
    guidance:
      use_when: [Use for fixture source nodes.]
      avoid_when: [Use another configured flavour.]
    id: {}
    title: {}
    body: {}
    fields:
      state:
        type: enum
        values: [active, inactive]
      tag:
        type: string
      note:
        type: string
  beta:
    label: Beta
    description: Arbitrary target item.
    guidance:
      use_when: [Use for fixture target nodes.]
      avoid_when: [Use another configured flavour.]
    id: {}
    title: {}
    body: {}
relations:
  connects:
    source:
      flavours: [alpha]
    target:
      flavours: [beta]
    cardinality:
      outgoing: {min: 1, max: 1}
      incoming: {min: 0, max: 1}
  acyclic_link:
    source:
      flavours: [alpha]
    target:
      flavours: [alpha]
    acyclic: true
  free_link:
    source:
      flavours: [alpha]
    target:
      flavours: [alpha]
  links:
    source:
      flavours: [alpha]
    target:
      external: [https]
rules:
  - name: alpha_has_tag
    kind: requires_field
    severity: error
    applies_to: {flavours: [alpha]}
    field: tag
    min: 1
    max: 1
  - name: active_alpha_has_details
    kind: requires_field
    severity: warning
    applies_to: {flavours: [alpha]}
    when: {field: state, in: [active]}
    field_any_of: [note, tag]
    min: 2
    max: 2
  - name: alpha_has_direct_link
    kind: requires_relation
    severity: warning
    applies_to: {flavours: [alpha]}
    relation: acyclic_link
    direction: outgoing
    min: 1
    max: 1
  - name: alpha_has_incoming_link
    kind: requires_relation
    severity: error
    applies_to: {flavours: [alpha]}
    relation_any_of: [free_link, acyclic_link]
    direction: incoming
    min: 1
  - name: beta_is_connected
    kind: orphan
    severity: info
    applies_to: {flavours: [beta]}
    relations: [connects]
"#;

const ITEMS_ONE: &str = r#":::alpha m_00000000000000000000000001
:id: ALPHA-A
:state: active
:tag: one
:connects: BETA-D
:acyclic_link: ALPHA-B
:free_link: ALPHA-B
:links: https://example.test/a

:::

:::alpha m_00000000000000000000000002
:id: ALPHA-B
:state: active
:tag: one
:note: two
:acyclic_link: ALPHA-A
:free_link: ALPHA-A
:links: ftp://example.test/b

:::
"#;

const ITEMS_TWO: &str = r#":::alpha m_00000000000000000000000003
:id: ALPHA-C
:state: inactive

:::

:::beta m_00000000000000000000000004
:id: BETA-D

:::

:::beta m_00000000000000000000000005
:id: BETA-E

:::
"#;

struct Fixture {
    _sandbox: ProjectSandbox,
    root: std::path::PathBuf,
}

impl Fixture {
    fn new(schema: impl AsRef<[u8]>, warnings_as_errors: bool) -> Self {
        let sandbox = ProjectSandbox::new(ProjectSandboxMode::Configured)
            .expect("create isolated project sandbox");
        let root = sandbox.path().to_path_buf();
        fs::write(root.join(".mara/schema.yaml"), schema).unwrap();
        fs::write(
            root.join(".mara/project.toml"),
            project_config(warnings_as_errors),
        )
        .unwrap();
        Self {
            _sandbox: sandbox,
            root,
        }
    }

    fn write(&self, path: &str, source: impl AsRef<[u8]>) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }

    fn schema(&self) -> mara_core::SchemaDocument {
        load_schema(&load_from_root(&self.root).unwrap()).unwrap()
    }
}

fn project_config(warnings_as_errors: bool) -> String {
    format!(
        r#"format_version = 1
[project]
name = "validation-test"
schema = ".mara/schema.yaml"
[content]
include = ["docs/**/*.mara.md"]
exclude = []
respect_gitignore = false
follow_directory_symlinks = false
allow_internal_file_symlinks = false
[index]
path = ".mara/index.json"
[validation]
warnings_as_errors = {warnings_as_errors}
[git]
require_clean_worktree_for_writes = true
"#
    )
}

fn parsed(schema: &mara_core::SchemaDocument, path: &str, source: &str) -> ParsedDocument {
    parse_document(
        SourceDocument::try_new(path, SourceText::new(source.to_owned())).unwrap(),
        schema.identity().value().mid().value(),
    )
}

fn codes(result: &mara_engine::ValidationResult) -> Vec<&str> {
    result
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code().as_str())
        .collect()
}

#[test]
fn mixed_pipeline_completes_independent_content_and_skips_schema_dependents() {
    let fixture = Fixture::new("format_version: [\n", false);
    fixture.write("docs/bad.mara.md", [0xff, 0xfe]);

    let result = check_project(&fixture.root).unwrap();

    assert!(matches!(
        result.phase(ValidationPhase::Schema),
        Some(ValidationPhaseState::Completed)
    ));
    assert!(matches!(
        result.phase(ValidationPhase::Content),
        Some(ValidationPhaseState::Completed)
    ));
    assert!(matches!(
        result.phase(ValidationPhase::Parse),
        Some(ValidationPhaseState::Skipped {
            prerequisite: Some(ValidationPhase::Schema),
            ..
        })
    ));
    assert_eq!(codes(&result), ["schema.syntax", "content.invalid_utf8"]);
    assert_eq!(result.severity_counts().errors(), 2);
    assert!(result.documents().is_empty());
    assert!(result.semantic().is_none());
    assert!(result.graph().is_none());
}

#[test]
fn schema_only_check_is_independent_of_bad_content() {
    let fixture = Fixture::new(VALIDATION_SCHEMA, false);
    fixture.write("docs/bad.mara.md", [0xff, 0xfe]);

    let result = check_schema(&fixture.root).unwrap();

    assert!(result.is_valid());
    assert!(result.diagnostics().is_empty());
    assert!(matches!(
        result.phase(ValidationPhase::Content),
        Some(ValidationPhaseState::Skipped {
            prerequisite: None,
            ..
        })
    ));
    assert!(result.documents().is_empty());
}

#[test]
fn cardinality_cycles_and_generic_rules_are_deterministic_and_schema_driven() {
    let fixture = Fixture::new(VALIDATION_SCHEMA, false);
    let schema = fixture.schema();
    let first = parsed(&schema, "docs/z.mara.md", ITEMS_ONE);
    let second = parsed(&schema, "docs/a.mara.md", ITEMS_TWO);

    let result = validate_documents(&schema, &[first.clone(), second.clone()], false);
    let reversed = validate_documents(&schema, &[second, first], false);

    assert_eq!(result.diagnostics(), reversed.diagnostics());
    assert_eq!(result.severity_counts().errors(), 6);
    assert_eq!(result.severity_counts().warnings(), 2);
    assert_eq!(result.severity_counts().info(), 1);
    assert!(!result.is_valid());
    assert_eq!(
        codes(&result),
        [
            "relation.cardinality",
            "rule.failed",
            "rule.failed",
            "rule.failed",
            "rule.failed",
            "relation.cycle",
            "rule.failed",
            "relation.cardinality",
            "reference.external_scheme",
        ]
    );

    let cardinality = result
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code() == DiagnosticCode::Relation(RelationDiagnosticCode::Cardinality)
        })
        .collect::<Vec<_>>();
    assert_eq!(cardinality.len(), 2, "zero-edge alpha items are checked");
    assert!(cardinality.iter().all(|diagnostic| {
        diagnostic.details().get("actual") == Some(&DiagnosticValue::Unsigned(0))
    }));

    let cycle = result
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.code() == DiagnosticCode::Relation(RelationDiagnosticCode::Cycle)
        })
        .unwrap();
    assert_eq!(
        cycle.details().get("cycle_path"),
        Some(&DiagnosticValue::Array(vec![
            DiagnosticValue::String("m_00000000000000000000000001".to_owned()),
            DiagnosticValue::String("m_00000000000000000000000002".to_owned()),
            DiagnosticValue::String("m_00000000000000000000000001".to_owned()),
        ]))
    );
    let rule_severities = result
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code() == RuleDiagnosticCode::Failed.into())
        .fold([0; 3], |mut counts, diagnostic| {
            counts[match diagnostic.severity() {
                DiagnosticSeverity::Error => 0,
                DiagnosticSeverity::Warning => 1,
                DiagnosticSeverity::Info => 2,
            }] += 1;
            counts
        });
    assert_eq!(rule_severities, [2, 2, 1]);
    let orphan_failures = result
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code() == RuleDiagnosticCode::Failed.into()
                && diagnostic.details().get("kind")
                    == Some(&DiagnosticValue::String("orphan".to_owned()))
        })
        .collect::<Vec<_>>();
    assert_eq!(orphan_failures.len(), 1);
    assert_eq!(orphan_failures[0].item().unwrap().id(), Some("BETA-E"));
    assert!(
        result
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code().as_str() != "relation.cycle"
                || diagnostic.context().relation() == Some("acyclic_link"))
    );
    assert!(result.graph().unwrap().edges().iter().any(|edge| {
        edge.relation() == "links" && edge.target().uri() == Some("https://example.test/a")
    }));
}

#[test]
fn cardinality_uses_deduplicated_endpoints_and_enforces_upper_bounds() {
    let fixture = Fixture::new(VALIDATION_SCHEMA, false);
    let schema = fixture.schema();
    let document = parsed(
        &schema,
        "docs/bounds.mara.md",
        r#":::alpha m_00000000000000000000000001
:id: ALPHA-A
:state: inactive
:tag: present
:connects: BETA-D
:connects: BETA-D

:::

:::alpha m_00000000000000000000000002
:id: ALPHA-B
:state: inactive
:tag: present
:connects: BETA-D
:connects: BETA-E

:::

:::beta m_00000000000000000000000004
:id: BETA-D

:::

:::beta m_00000000000000000000000005
:id: BETA-E

:::
"#,
    );

    let result = validate_documents(&schema, &[document], false);
    let cardinality = result
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code() == DiagnosticCode::Relation(RelationDiagnosticCode::Cardinality)
        })
        .collect::<Vec<_>>();

    assert_eq!(cardinality.len(), 2);
    assert!(cardinality.iter().all(|diagnostic| {
        diagnostic.details().get("actual") == Some(&DiagnosticValue::Unsigned(2))
    }));
    assert!(
        cardinality
            .iter()
            .all(|diagnostic| { diagnostic.item().unwrap().id() != Some("ALPHA-A") })
    );
}

#[test]
fn globally_ambiguous_identity_skips_graph_rules_but_continues_field_rules() {
    let fixture = Fixture::new(VALIDATION_SCHEMA, false);
    let schema = fixture.schema();
    let first = parsed(
        &schema,
        "docs/first.mara.md",
        r#":::alpha m_00000000000000000000000001
:id: ALPHA-A
:state: unknown
:tag: present
:note: present

:::
"#,
    );
    let second = parsed(
        &schema,
        "docs/second.mara.md",
        r#":::alpha m_00000000000000000000000001
:id: ALPHA-B
:state: active
:tag: present

:::
"#,
    );

    let result = validate_documents(&schema, &[first, second], false);

    assert!(matches!(
        result.phase(ValidationPhase::Graph),
        Some(ValidationPhaseState::Skipped {
            prerequisite: Some(ValidationPhase::Semantic),
            ..
        })
    ));
    assert!(result.graph().is_none());
    let detail_rule = DiagnosticValue::String("active_alpha_has_details".to_owned());
    let detail_diagnostics = result
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.details().get("rule") == Some(&detail_rule))
        .collect::<Vec<_>>();
    assert_eq!(detail_diagnostics.len(), 2);
    assert!(detail_diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == RuleDiagnosticCode::Skipped.into()
            && diagnostic.item().unwrap().id() == Some("ALPHA-A")
    }));
    assert!(detail_diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == RuleDiagnosticCode::Failed.into()
            && diagnostic.item().unwrap().id() == Some("ALPHA-B")
    }));
}

#[test]
fn warning_escalation_changes_validity_without_rewriting_severity() {
    let schema_source = VALIDATION_SCHEMA
        .replace("outgoing: {min: 1, max: 1}", "outgoing: {min: 0, max: 1}")
        .replace("    acyclic: true\n", "")
        .replace(
            "    severity: error\n    applies_to: {flavours: [alpha]}\n    field: tag",
            "    severity: warning\n    applies_to: {flavours: [alpha]}\n    field: tag",
        )
        .replace(
            "    severity: error\n    applies_to: {flavours: [alpha]}\n    relation_any_of",
            "    severity: warning\n    applies_to: {flavours: [alpha]}\n    relation_any_of",
        )
        .replace("    severity: info\n", "    severity: warning\n");
    let fixture = Fixture::new(schema_source, false);
    let schema = fixture.schema();
    let document = parsed(
        &schema,
        "docs/warning.mara.md",
        r#":::alpha m_00000000000000000000000003
:id: ALPHA-C
:state: inactive

:::
"#,
    );

    let ordinary = validate_documents(&schema, std::slice::from_ref(&document), false);
    let escalated = validate_documents(&schema, &[document], true);

    assert!(ordinary.is_valid());
    assert!(!escalated.is_valid());
    assert_eq!(ordinary.diagnostics(), escalated.diagnostics());
    assert!(
        ordinary
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.severity() == DiagnosticSeverity::Warning)
    );
}

#[test]
fn unavailable_relation_rules_are_skipped_without_cascading_failures() {
    let fixture = Fixture::new(VALIDATION_SCHEMA, false);
    let schema = fixture.schema();
    let document = parsed(
        &schema,
        "docs/unresolved.mara.md",
        r#":::alpha m_00000000000000000000000003
:id: ALPHA-C
:state: inactive
:tag: present
:acyclic_link: MISSING

:::

:::alpha m_00000000000000000000000002
:id: ALPHA-B
:state: inactive
:tag: present
:free_link: ALPHA-C

:::
"#,
    );

    let result = validate_documents(&schema, &[document], false);

    assert!(result.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::Reference(ReferenceDiagnosticCode::Unresolved)
    }));
    let item_rule_diagnostics = result
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .item()
                .is_some_and(|item| item.id() == Some("ALPHA-C"))
        })
        .filter(|diagnostic| {
            matches!(
                diagnostic.code(),
                DiagnosticCode::Rule(RuleDiagnosticCode::Failed | RuleDiagnosticCode::Skipped)
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        item_rule_diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.details().get("rule")))
            .collect::<Vec<_>>(),
        [(
            DiagnosticCode::Rule(RuleDiagnosticCode::Skipped),
            Some(&DiagnosticValue::String("alpha_has_direct_link".to_owned())),
        )]
    );
    let unresolved = result
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.code() == DiagnosticCode::Reference(ReferenceDiagnosticCode::Unresolved)
        })
        .unwrap();
    assert_eq!(
        unresolved.details().get("canonical_direction"),
        Some(&DiagnosticValue::String("outgoing".to_owned()))
    );
}

#[test]
fn external_duplicate_occurrences_warn_before_projection_and_honor_escalation() {
    let schema_source =
        VALIDATION_SCHEMA.replace("outgoing: {min: 1, max: 1}", "outgoing: {min: 0, max: 1}");
    let schema_source = schema_source[..schema_source.find("rules:").unwrap()].to_owned();
    let fixture = Fixture::new(schema_source, false);
    let schema = fixture.schema();
    let document = parsed(
        &schema,
        "docs/external-duplicate.mara.md",
        r#":::alpha m_00000000000000000000000001
:id: ALPHA-A
:links: https://example.test/same
:links: https://example.test/same

:::
"#,
    );

    let ordinary = validate_documents(&schema, std::slice::from_ref(&document), false);
    let escalated = validate_documents(&schema, &[document], true);

    assert_eq!(
        ordinary.semantic().unwrap().items()[0]
            .authored_references()
            .len(),
        2
    );
    assert_eq!(ordinary.graph().unwrap().edge_count(), 1);
    let duplicate = ordinary
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == RelationDiagnosticCode::Duplicate.into())
        .unwrap();
    assert_eq!(duplicate.severity(), DiagnosticSeverity::Warning);
    assert_eq!(duplicate.context().relation(), Some("links"));
    assert_eq!(
        duplicate.details().get("target_uri"),
        Some(&DiagnosticValue::String(
            "https://example.test/same".to_owned()
        ))
    );
    assert!(ordinary.is_valid());
    assert!(!escalated.is_valid());
    assert_eq!(ordinary.diagnostics(), escalated.diagnostics());
}

#[test]
fn external_scheme_and_target_constraints_use_closed_diagnostics() {
    let fixture = Fixture::new(VALIDATION_SCHEMA, false);
    let schema = fixture.schema();
    let document = parsed(
        &schema,
        "docs/external.mara.md",
        r#":::alpha m_00000000000000000000000001
:id: ALPHA-A
:state: inactive
:tag: present
:links: ftp://example.test/not-allowed
:acyclic_link: https://example.test/not-an-item

:::

[[ftp://example.test/bare]]
"#,
    );

    let result = validate_documents(&schema, &[document], false);

    assert_eq!(
        result
            .diagnostics()
            .iter()
            .filter(|diagnostic| {
                diagnostic.code()
                    == DiagnosticCode::Reference(ReferenceDiagnosticCode::ExternalScheme)
            })
            .count(),
        2
    );
    assert!(result.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::Relation(RelationDiagnosticCode::InvalidTargetFlavour)
    }));
}
