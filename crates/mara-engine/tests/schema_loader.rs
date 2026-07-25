use std::{fs, path::Path};

use mara_core::{
    CardinalityMaximum, DerivedSourceKind, Diagnostic, DiagnosticCode, DiagnosticValue,
    FieldRuleSelection, FieldType, Mid, MidFormat, RelationRuleSelection, RuleConditionValue,
    RuleConfiguration, RuleDirection, RuleKind, RuleSeverity, SchemaDiagnosticCode, SchemaField,
};
use mara_engine::{
    project::{LoadedProject, load_from_root},
    schema::{SchemaLoadError, load_schema},
};
use tempfile::TempDir;

const VALID_SCHEMA: &str = r#"# strict v1 fixture
format_version: 1
schema:
  name: mara-schema
  version: 1.2.3-alpha.1+build.5
identity:
  mid:
    format: ulid
    prefix: m_
flavours:
  requirement:
    label: Requirement
    description: A verifiable obligation.
    guidance:
      use_when: [Document an externally visible obligation.]
      avoid_when: [The content explains an implementation choice.]
    id: {}
    title: {}
    body: {}
relations: {}
rules: []
"#;

const RICH_SCHEMA: &str = r#"format_version: 1
schema:
  name: rich-schema
  version: 1.0.0
identity:
  mid:
    format: ulid
    prefix: rich_
flavours:
  requirement:
    label: Requirement
    description: A verifiable obligation.
    guidance:
      use_when:
        - Define an externally visible obligation.
      avoid_when:
        - The content explains an implementation choice.
      distinguish_from:
        design: Describes the solution rather than the obligation.
    id:
      required: true
      pattern: REQ-[0-9]+
    title:
      required: true
    body: {}
    fields:
      summary:
        type: string
        required: true
        repeatable: true
        pattern: .+
      estimate:
        type: integer
      confidence:
        type: number
      automated:
        type: boolean
      status:
        type: enum
        required: true
        values:
          - draft
          - approved
  design:
    label: Design
    description: A chosen implementation structure.
    guidance:
      use_when: [Describe a solution structure.]
      avoid_when: [The content defines an obligation.]
    id: {}
    title:
      required: false
    body: {}
"#;

const COMPLETE_RELATIONS: &str = r#"  derives_from:
    source:
      flavours: [design]
      derived: [source_span]
    target:
      flavours: [requirement]
      external: [https, linear+v1]
    inverse: derived_by
    inverse_authoring: true
    symmetric: false
    self_reference: false
    cardinality:
      outgoing:
        min: 1
        max: many
      incoming:
        min: 0
        max: 3
  references:
    source:
      flavours: [requirement]
    target:
      external: [mailto, https]
  related_to:
    source:
      flavours: [requirement, design]
    target:
      flavours: [design, requirement]
    symmetric: true
    same_flavour: true
    acyclic: true
"#;

const COMPLETE_RULES: &str = r#"  - name: design_has_requirement
    kind: requires_relation
    severity: error
    applies_to:
      flavours: [design]
    relation: derives_from
    direction: outgoing
    min: 1
    max: many
  - name: requirement_has_estimate
    kind: requires_field
    severity: warning
    applies_to:
      flavours: [requirement]
    when:
      field: status
      in: [draft, approved]
    field_any_of: [estimate, confidence]
    min: 1
    max: 2
  - name: requirement_is_connected
    kind: orphan
    severity: info
    applies_to:
      flavours: [requirement]
    relations: [derives_from, related_to]
"#;

fn rich_schema_with_relations(relations: &str) -> String {
    format!("{RICH_SCHEMA}relations:\n{relations}")
}

fn rich_schema_with_relations_and_rules(relations: &str, rules: &str) -> String {
    format!("{RICH_SCHEMA}relations:\n{relations}rules:\n{rules}")
}

struct Fixture {
    _temp: TempDir,
    root: std::path::PathBuf,
    schema_relative: String,
}

impl Fixture {
    fn new(schema: impl AsRef<[u8]>) -> Self {
        Self::with_path(".mara/schema.yaml", schema)
    }

    fn with_path(schema_relative: &str, schema: impl AsRef<[u8]>) -> Self {
        let temp = tempfile::tempdir().expect("create isolated fixture");
        let root = temp.path().join("project");
        fs::create_dir_all(root.join(".mara")).unwrap();
        let root = root.canonicalize().unwrap();
        fs::write(root.join(schema_relative), schema).unwrap();
        fs::write(
            root.join(".mara/project.toml"),
            project_config(schema_relative),
        )
        .unwrap();
        Self {
            _temp: temp,
            root,
            schema_relative: schema_relative.to_owned(),
        }
    }

    fn loaded_project(&self) -> LoadedProject {
        load_from_root(&self.root).unwrap()
    }

    fn schema_path(&self) -> std::path::PathBuf {
        self.root.join(&self.schema_relative)
    }
}

fn project_config(schema: &str) -> String {
    format!(
        r#"format_version = 1
[project]
name = "schema-test"
schema = {schema:?}
[content]
include = ["**/*.mara.md"]
exclude = []
respect_gitignore = true
follow_directory_symlinks = false
allow_internal_file_symlinks = true
[index]
path = ".mara/index.json"
[validation]
warnings_as_errors = false
[git]
require_clean_worktree_for_writes = true
"#
    )
}

fn only_diagnostic(error: &SchemaLoadError) -> &Diagnostic {
    let diagnostics = error.diagnostics();
    assert_eq!(
        diagnostics.len(),
        1,
        "unexpected diagnostics: {diagnostics:#?}"
    );
    &diagnostics[0]
}

fn assert_code(diagnostic: &Diagnostic, code: SchemaDiagnosticCode) {
    assert_eq!(diagnostic.code(), DiagnosticCode::Schema(code));
    assert_eq!(diagnostic.severity().as_str(), "error");
}

fn assert_invalid(schema: impl AsRef<[u8]>, code: SchemaDiagnosticCode) -> SchemaLoadError {
    let fixture = Fixture::new(schema);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();
    assert_code(only_diagnostic(&error), code);
    error
}

fn source_slice<'a>(source: &'a str, span: &mara_core::SourceSpan) -> &'a str {
    &source[span.start_byte() as usize..span.end_byte() as usize]
}

fn assert_field_source<T>(source: &str, field: &SchemaField<T>, key: &str, value: &str) {
    assert_eq!(source_slice(source, field.key_source()), key);
    assert_eq!(source_slice(source, field.value_source()), value);
}

fn detail_string<'a>(diagnostic: &'a Diagnostic, key: &str) -> Option<&'a str> {
    match diagnostic.details().get(key) {
        Some(DiagnosticValue::String(value)) => Some(value),
        _ => None,
    }
}

fn condition_string(value: &RuleConditionValue) -> &str {
    let RuleConditionValue::String(value) = value else {
        panic!("expected a string condition value")
    };
    value
}

#[test]
fn loads_valid_v1_identity_and_preserves_every_decoded_key_and_value_span() {
    let fixture = Fixture::new(VALID_SCHEMA);
    let document = load_schema(&fixture.loaded_project()).unwrap();

    assert_eq!(document.source().path(), ".mara/schema.yaml");
    assert_eq!(source_slice(VALID_SCHEMA, document.source()), VALID_SCHEMA);
    assert_eq!(*document.format_version().value(), 1);
    assert_eq!(
        source_slice(VALID_SCHEMA, document.format_version().key_source()),
        "format_version"
    );
    assert_eq!(
        source_slice(VALID_SCHEMA, document.format_version().value_source()),
        "1"
    );

    let schema = document.schema();
    assert_eq!(source_slice(VALID_SCHEMA, schema.key_source()), "schema");
    assert_eq!(
        source_slice(VALID_SCHEMA, schema.value_source()),
        "name: mara-schema\n  version: 1.2.3-alpha.1+build.5\n"
    );
    assert_eq!(schema.value().name().value(), "mara-schema");
    assert_eq!(schema.value().version().value(), "1.2.3-alpha.1+build.5");
    assert_eq!(
        source_slice(VALID_SCHEMA, schema.value().name().key_source()),
        "name"
    );
    assert_eq!(
        source_slice(VALID_SCHEMA, schema.value().name().value_source()),
        "mara-schema"
    );
    assert_eq!(
        source_slice(VALID_SCHEMA, schema.value().version().key_source()),
        "version"
    );
    assert_eq!(
        source_slice(VALID_SCHEMA, schema.value().version().value_source()),
        "1.2.3-alpha.1+build.5"
    );

    let identity = document.identity();
    assert_eq!(
        source_slice(VALID_SCHEMA, identity.key_source()),
        "identity"
    );
    assert_eq!(
        source_slice(VALID_SCHEMA, identity.value_source()),
        "mid:\n    format: ulid\n    prefix: m_\n"
    );
    let mid = identity.value().mid();
    assert_eq!(source_slice(VALID_SCHEMA, mid.key_source()), "mid");
    assert_eq!(
        source_slice(VALID_SCHEMA, mid.value_source()),
        "format: ulid\n    prefix: m_\n"
    );
    assert_eq!(*mid.value().format().value(), MidFormat::Ulid);
    assert_eq!(mid.value().prefix().value(), "m_");
    let first_mid = Mid::from_ulid_value(mid.value(), 0);
    assert_eq!(first_mid.as_str(), "m_00000000000000000000000000");
    assert_eq!(
        Mid::parse(first_mid.as_str(), mid.value()).unwrap(),
        first_mid
    );
    assert_eq!(
        source_slice(VALID_SCHEMA, mid.value().format().key_source()),
        "format"
    );
    assert_eq!(
        source_slice(VALID_SCHEMA, mid.value().format().value_source()),
        "ulid"
    );
    assert_eq!(
        source_slice(VALID_SCHEMA, mid.value().prefix().key_source()),
        "prefix"
    );
    assert_eq!(
        source_slice(VALID_SCHEMA, mid.value().prefix().value_source()),
        "m_"
    );

    assert_eq!(
        source_slice(VALID_SCHEMA, document.flavours().key_source()),
        "flavours"
    );
    assert_eq!(
        source_slice(VALID_SCHEMA, document.flavours().value_source()),
        concat!(
            "requirement:\n",
            "    label: Requirement\n",
            "    description: A verifiable obligation.\n",
            "    guidance:\n",
            "      use_when: [Document an externally visible obligation.]\n",
            "      avoid_when: [The content explains an implementation choice.]\n",
            "    id: {}\n",
            "    title: {}\n",
            "    body: {}\n",
        )
    );
    assert_eq!(document.flavours().len(), 1);
    assert_eq!(
        source_slice(VALID_SCHEMA, document.relations().unwrap().key_source()),
        "relations"
    );
    assert_eq!(
        source_slice(VALID_SCHEMA, document.relations().unwrap().value_source()),
        "{}"
    );
    assert!(document.relations().unwrap().is_empty());
    assert!(document.external_mention_schemes().is_empty());
    assert_eq!(
        source_slice(VALID_SCHEMA, document.rules().unwrap().key_source()),
        "rules"
    );
    assert_eq!(
        source_slice(VALID_SCHEMA, document.rules().unwrap().value_source()),
        "[]"
    );
    assert!(document.rules().unwrap().is_empty());

    assert_eq!(document.format_version().key_source().start_line(), 2);
    assert_eq!(document.format_version().key_source().start_column(), 1);
    assert_eq!(mid.value().prefix().value_source().start_line(), 9);
    assert_eq!(mid.value().prefix().value_source().start_column(), 13);
}

#[test]
fn accepts_an_initial_utf8_bom_and_preserves_original_offsets() {
    let schema = VALID_SCHEMA
        .strip_prefix("# strict v1 fixture\n")
        .expect("the shared fixture starts with its descriptive comment");
    let source = format!("\u{feff}{schema}");
    let fixture = Fixture::new(&source);
    let document = load_schema(&fixture.loaded_project()).unwrap();

    assert_eq!(document.source().start_byte(), 0);
    assert_eq!(document.source().end_byte(), source.len() as u64);
    assert_eq!(document.format_version().key_source().start_byte(), 3);
    assert_eq!(document.format_version().key_source().start_column(), 2);
    assert_eq!(
        source_slice(&source, document.format_version().key_source()),
        "format_version"
    );
}

#[test]
fn accepts_empty_flavours_and_keeps_omitted_collections_absent() {
    let source = r#"format_version: 1
schema:
  name: empty-schema
  version: 0.1.0
identity:
  mid:
    format: ulid
    prefix: empty_
flavours: {}
"#;
    let fixture = Fixture::new(source);
    let document = load_schema(&fixture.loaded_project()).unwrap();

    assert!(document.flavours().is_empty());
    assert!(document.relations().is_none());
    assert!(document.external_mention_schemes().is_empty());
    assert!(document.rules().is_none());
}

#[test]
fn compiles_guidance_builtins_and_every_scalar_field_into_deterministic_domain_values() {
    let first_fixture = Fixture::new(RICH_SCHEMA);
    let first = load_schema(&first_fixture.loaded_project()).unwrap();
    let second_fixture = Fixture::new(RICH_SCHEMA);
    let second = load_schema(&second_fixture.loaded_project()).unwrap();

    assert_eq!(first.flavours(), second.flavours());
    assert_eq!(
        first
            .flavours()
            .definitions()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["design", "requirement"]
    );

    let requirement = first.flavours().get("requirement").unwrap();
    assert_eq!(requirement.name(), "requirement");
    assert_eq!(
        source_slice(RICH_SCHEMA, requirement.key_source()),
        "requirement"
    );
    assert_eq!(requirement.label().value(), "Requirement");
    assert_eq!(
        source_slice(RICH_SCHEMA, requirement.description().value_source()),
        "A verifiable obligation."
    );

    let guidance = requirement.guidance().value();
    assert_eq!(
        guidance
            .use_when()
            .value()
            .iter()
            .map(|value| value.value().as_str())
            .collect::<Vec<_>>(),
        ["Define an externally visible obligation."]
    );
    assert_eq!(
        source_slice(RICH_SCHEMA, guidance.use_when().value()[0].source()),
        "Define an externally visible obligation."
    );
    let distinction = &guidance.distinguish_from()["design"];
    assert_eq!(
        distinction.value(),
        "Describes the solution rather than the obligation."
    );
    assert_eq!(
        source_slice(RICH_SCHEMA, distinction.key_source()),
        "design"
    );
    assert_eq!(guidance.distinguish_from_source().unwrap().len(), 1);

    assert!(requirement.display_id().value().is_required());
    assert_eq!(
        requirement.display_id().value().pattern().unwrap().value(),
        "REQ-[0-9]+"
    );
    assert!(requirement.title().value().is_required());
    assert!(!requirement.body().value().is_required());
    assert!(requirement.body().value().required().is_none());

    let fields = requirement.fields();
    assert_eq!(
        fields.keys().map(String::as_str).collect::<Vec<_>>(),
        ["automated", "confidence", "estimate", "status", "summary"]
    );
    assert_eq!(*fields["summary"].field_type().value(), FieldType::String);
    assert!(fields["summary"].is_required());
    assert!(fields["summary"].is_repeatable());
    assert_eq!(fields["summary"].pattern().unwrap().value(), ".+");
    assert_eq!(*fields["estimate"].field_type().value(), FieldType::Integer);
    assert_eq!(
        *fields["confidence"].field_type().value(),
        FieldType::Number
    );
    assert_eq!(
        *fields["automated"].field_type().value(),
        FieldType::Boolean
    );
    assert_eq!(*fields["status"].field_type().value(), FieldType::Enum);
    assert_eq!(
        fields["status"]
            .values()
            .unwrap()
            .value()
            .iter()
            .map(|value| value.value().as_str())
            .collect::<Vec<_>>(),
        ["draft", "approved"]
    );
    assert_eq!(
        source_slice(
            RICH_SCHEMA,
            fields["status"].values().unwrap().value()[1].source()
        ),
        "approved"
    );
    assert!(fields["estimate"].required().is_none());
    assert!(!fields["estimate"].is_required());
    assert!(requirement.fields_source().is_some());

    let design = first.flavours().get("design").unwrap();
    assert!(!design.title().value().is_required());
    assert!(!*design.title().value().required().unwrap().value());
    assert!(design.fields_source().is_none());
    assert!(design.fields().is_empty());
}

#[test]
fn compiles_relation_endpoints_constraints_cardinality_and_external_allowlist_deterministically() {
    // TEST-SCHEMA-RELATIONS-RULES covers REQ-SCHEMA-RELATIONS and
    // REQ-SCHEMA-INVERSES for the relation-declaration slice delivered by CON-25.
    let source = rich_schema_with_relations(COMPLETE_RELATIONS);
    let first_fixture = Fixture::new(&source);
    let first = load_schema(&first_fixture.loaded_project()).unwrap();
    let second_fixture = Fixture::new(&source);
    let second = load_schema(&second_fixture.loaded_project()).unwrap();

    assert_eq!(first.relations(), second.relations());
    let relations = first.relations().unwrap();
    assert_eq!(source_slice(&source, relations.key_source()), "relations");
    let relations_source = source_slice(&source, relations.value_source());
    assert!(relations_source.starts_with("derives_from:\n"));
    assert!(relations_source.ends_with("    acyclic: true\n"));
    assert_eq!(relations.len(), 3);
    assert_eq!(
        relations
            .definitions()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["derives_from", "references", "related_to"]
    );
    assert_eq!(
        first
            .external_mention_schemes()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["https", "linear+v1", "mailto"]
    );

    let derives_from = relations.get("derives_from").unwrap();
    assert_eq!(derives_from.name(), "derives_from");
    assert_eq!(
        source_slice(&source, derives_from.key_source()),
        "derives_from"
    );
    let relation_source = source_slice(&source, derives_from.value_source());
    assert!(relation_source.starts_with("source:\n"));
    assert!(relation_source.ends_with("        max: 3\n  "));
    assert_field_source(
        &source,
        derives_from.source(),
        "source",
        "flavours: [design]\n      derived: [source_span]\n    ",
    );
    assert_field_source(
        &source,
        derives_from.source().value().flavours(),
        "flavours",
        "[design]",
    );
    assert_eq!(
        derives_from
            .source()
            .value()
            .flavours()
            .value()
            .iter()
            .map(|value| value.value().as_str())
            .collect::<Vec<_>>(),
        ["design"]
    );
    assert_eq!(
        source_slice(
            &source,
            derives_from.source().value().flavours().value()[0].source()
        ),
        "design"
    );
    let derived = derives_from.source().value().derived().unwrap();
    assert_field_source(&source, derived, "derived", "[source_span]");
    assert_eq!(derived.value().len(), 1);
    assert_eq!(*derived.value()[0].value(), DerivedSourceKind::SourceSpan);
    assert_eq!(
        source_slice(&source, derived.value()[0].source()),
        "source_span"
    );
    let target = derives_from.target().value();
    assert_field_source(
        &source,
        derives_from.target(),
        "target",
        "flavours: [requirement]\n      external: [https, linear+v1]\n    ",
    );
    assert_field_source(
        &source,
        target.flavours().unwrap(),
        "flavours",
        "[requirement]",
    );
    assert_field_source(
        &source,
        target.external().unwrap(),
        "external",
        "[https, linear+v1]",
    );
    assert_eq!(
        target
            .flavours()
            .unwrap()
            .value()
            .iter()
            .map(|value| value.value().as_str())
            .collect::<Vec<_>>(),
        ["requirement"]
    );
    assert_eq!(
        source_slice(&source, target.flavours().unwrap().value()[0].source()),
        "requirement"
    );
    assert_eq!(
        target
            .external()
            .unwrap()
            .value()
            .iter()
            .map(|value| value.value().as_str())
            .collect::<Vec<_>>(),
        ["https", "linear+v1"]
    );
    assert_eq!(
        target
            .external()
            .unwrap()
            .value()
            .iter()
            .map(|value| source_slice(&source, value.source()))
            .collect::<Vec<_>>(),
        ["https", "linear+v1"]
    );
    assert_eq!(derives_from.inverse().unwrap().value(), "derived_by");
    assert_field_source(
        &source,
        derives_from.inverse().unwrap(),
        "inverse",
        "derived_by",
    );
    assert_field_source(
        &source,
        derives_from.inverse_authoring().unwrap(),
        "inverse_authoring",
        "true",
    );
    assert_field_source(
        &source,
        derives_from.symmetric().unwrap(),
        "symmetric",
        "false",
    );
    assert_field_source(
        &source,
        derives_from.self_reference().unwrap(),
        "self_reference",
        "false",
    );
    assert!(derives_from.permits_inverse_authoring());
    assert!(!derives_from.is_symmetric());
    assert!(!derives_from.requires_same_flavour());
    assert!(!derives_from.permits_self_reference());
    assert!(!derives_from.is_acyclic());

    let cardinality = derives_from.cardinality().unwrap().value();
    assert_field_source(
        &source,
        derives_from.cardinality().unwrap(),
        "cardinality",
        concat!(
            "outgoing:\n",
            "        min: 1\n",
            "        max: many\n",
            "      incoming:\n",
            "        min: 0\n",
            "        max: 3\n  ",
        ),
    );
    let outgoing = cardinality.outgoing().unwrap().value();
    assert_field_source(
        &source,
        cardinality.outgoing().unwrap(),
        "outgoing",
        "min: 1\n        max: many\n      ",
    );
    assert_eq!(outgoing.minimum(), 1);
    assert_eq!(outgoing.maximum(), CardinalityMaximum::Many);
    assert_field_source(&source, outgoing.min().unwrap(), "min", "1");
    assert_field_source(&source, outgoing.max().unwrap(), "max", "many");
    let incoming = cardinality.incoming().unwrap().value();
    assert_field_source(
        &source,
        cardinality.incoming().unwrap(),
        "incoming",
        "min: 0\n        max: 3\n  ",
    );
    assert_eq!(incoming.minimum(), 0);
    assert_eq!(incoming.maximum(), CardinalityMaximum::Bounded(3));
    assert_field_source(&source, incoming.min().unwrap(), "min", "0");
    assert_field_source(&source, incoming.max().unwrap(), "max", "3");

    let related_to = relations.get("related_to").unwrap();
    assert!(related_to.is_symmetric());
    assert!(related_to.requires_same_flavour());
    assert!(related_to.permits_self_reference());
    assert!(related_to.is_acyclic());
    assert!(related_to.inverse().is_none());
    assert!(related_to.cardinality().is_none());
    assert_field_source(
        &source,
        related_to.same_flavour().unwrap(),
        "same_flavour",
        "true",
    );
    assert_field_source(&source, related_to.acyclic().unwrap(), "acyclic", "true");

    let references = relations.get("references").unwrap();
    assert!(references.target().value().flavours().is_none());
    assert_eq!(
        references
            .target()
            .value()
            .external()
            .unwrap()
            .value()
            .iter()
            .map(|value| value.value().as_str())
            .collect::<Vec<_>>(),
        ["mailto", "https"]
    );
}

#[test]
fn compiles_every_rule_shape_in_authored_order_with_complete_source_evidence() {
    // TEST-SCHEMA-RELATIONS-RULES covers REQ-SCHEMA-RULES and the compiled
    // rule slice of DES-SCHEMA-META-MODEL delivered by CON-26.
    let source = rich_schema_with_relations_and_rules(COMPLETE_RELATIONS, COMPLETE_RULES);
    let first_fixture = Fixture::new(&source);
    let first = load_schema(&first_fixture.loaded_project()).unwrap();
    let second_fixture = Fixture::new(&source);
    let second = load_schema(&second_fixture.loaded_project()).unwrap();

    assert_eq!(first, second);
    let rules = first.rules().unwrap();
    assert_eq!(source_slice(&source, rules.key_source()), "rules");
    assert_eq!(
        source_slice(&source, rules.value_source()),
        COMPLETE_RULES
            .strip_prefix("  ")
            .expect("the embedded rules are indented under the root key")
    );
    assert_eq!(rules.len(), 3);
    assert_eq!(
        rules
            .definitions()
            .iter()
            .map(|rule| rule.name().value().as_str())
            .collect::<Vec<_>>(),
        [
            "design_has_requirement",
            "requirement_has_estimate",
            "requirement_is_connected"
        ]
    );

    let relation = rules.get("design_has_requirement").unwrap();
    assert_eq!(*relation.kind().value(), RuleKind::RequiresRelation);
    assert_eq!(*relation.severity().value(), RuleSeverity::Error);
    assert_field_source(&source, relation.name(), "name", "design_has_requirement");
    assert_field_source(&source, relation.kind(), "kind", "requires_relation");
    assert_field_source(&source, relation.severity(), "severity", "error");
    assert_eq!(
        source_slice(&source, relation.source()),
        concat!(
            "name: design_has_requirement\n",
            "    kind: requires_relation\n",
            "    severity: error\n",
            "    applies_to:\n",
            "      flavours: [design]\n",
            "    relation: derives_from\n",
            "    direction: outgoing\n",
            "    min: 1\n",
            "    max: many\n  "
        )
    );
    assert_field_source(
        &source,
        relation.applies_to().value().flavours(),
        "flavours",
        "[design]",
    );
    let RuleConfiguration::RequiresRelation(configuration) = relation.configuration() else {
        panic!("expected requires_relation configuration")
    };
    let RelationRuleSelection::Relation(selected) = configuration.relations() else {
        panic!("expected singular relation selection")
    };
    assert_field_source(&source, selected, "relation", "derives_from");
    assert_eq!(*configuration.direction().value(), RuleDirection::Outgoing);
    assert_field_source(&source, configuration.direction(), "direction", "outgoing");
    assert_eq!(*configuration.count().min().value(), 1);
    assert_eq!(configuration.count().maximum(), CardinalityMaximum::Many);

    let required_field = rules.get("requirement_has_estimate").unwrap();
    assert_eq!(*required_field.kind().value(), RuleKind::RequiresField);
    assert_eq!(*required_field.severity().value(), RuleSeverity::Warning);
    let condition = required_field.condition().unwrap().value();
    assert_field_source(&source, condition.field(), "field", "status");
    assert_eq!(
        condition
            .values()
            .value()
            .iter()
            .map(|value| condition_string(value.value()))
            .collect::<Vec<_>>(),
        ["draft", "approved"]
    );
    assert_eq!(
        condition
            .values()
            .value()
            .iter()
            .map(|value| source_slice(&source, value.source()))
            .collect::<Vec<_>>(),
        ["draft", "approved"]
    );
    let RuleConfiguration::RequiresField(configuration) = required_field.configuration() else {
        panic!("expected requires_field configuration")
    };
    let FieldRuleSelection::AnyOf(selected) = configuration.fields() else {
        panic!("expected field_any_of selection")
    };
    assert_eq!(
        selected
            .value()
            .iter()
            .map(|value| value.value().as_str())
            .collect::<Vec<_>>(),
        ["estimate", "confidence"]
    );
    assert_eq!(
        configuration.count().maximum(),
        CardinalityMaximum::Bounded(2)
    );

    let orphan = rules.get("requirement_is_connected").unwrap();
    assert_eq!(*orphan.kind().value(), RuleKind::Orphan);
    assert_eq!(*orphan.severity().value(), RuleSeverity::Info);
    assert!(orphan.condition().is_none());
    let RuleConfiguration::Orphan(configuration) = orphan.configuration() else {
        panic!("expected orphan configuration")
    };
    assert_eq!(
        configuration
            .relations()
            .value()
            .iter()
            .map(|value| value.value().as_str())
            .collect::<Vec<_>>(),
        ["derives_from", "related_to"]
    );
}

#[test]
fn compiles_the_repository_schema_through_the_rooted_project_flow() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let project = load_from_root(root).unwrap();
    let first = load_schema(&project).unwrap();
    let second = load_schema(&project).unwrap();

    assert_eq!(first, second);
    assert!(first.flavours().get("req").is_some());
    assert!(first.relations().unwrap().get("verifies").is_some());
    let rules = first.rules().unwrap();
    assert!(rules.get("test_has_target").is_some());
    assert!(rules.get("semantic_item_is_not_orphaned").is_some());
}

#[test]
fn diagnoses_relation_endpoint_and_external_scheme_defects_at_their_sources() {
    let cases = [
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "missing",
            r#"  relation:
    source: {flavours: [missing]}
    target: {flavours: [requirement]}
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "missing",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [missing]}
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidName,
            "Requirement",
            r#"  relation:
    source: {flavours: [Requirement]}
    target: {flavours: [requirement]}
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidName,
            "HTTPS",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {external: [HTTPS]}
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidName,
            "'https://'",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {external: ['https://']}
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "source_file",
            r#"  relation:
    source: {flavours: [requirement], derived: [source_file]}
    target: {flavours: [requirement]}
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "[]",
            r#"  relation:
    source: {flavours: []}
    target: {flavours: [requirement]}
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "[]",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {external: []}
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "{}",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {}
"#,
        ),
    ];

    for (code, primary, relation) in cases {
        let source = rich_schema_with_relations(relation);
        let error = assert_invalid(&source, code);
        assert_eq!(
            source_slice(&source, only_diagnostic(&error).primary().unwrap()),
            primary
        );
    }
}

#[test]
fn diagnoses_inverse_symmetry_acyclicity_and_cardinality_contradictions() {
    let cases = [
        (
            "true",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [design]}
    inverse_authoring: true
"#,
        ),
        (
            "inverse_name",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [requirement]}
    inverse: inverse_name
    symmetric: true
"#,
        ),
        (
            "false",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [requirement]}
    inverse_authoring: false
    symmetric: true
"#,
        ),
        (
            "{flavours: [design]}",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [design]}
    symmetric: true
"#,
        ),
        (
            "true",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [requirement], external: [https]}
    acyclic: true
"#,
        ),
        (
            "1",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [requirement]}
    cardinality:
      outgoing: {min: 2, max: 1}
"#,
        ),
        (
            "{}",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [requirement]}
    cardinality: {}
"#,
        ),
        (
            "-1",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [requirement]}
    cardinality:
      incoming: {min: -1}
"#,
        ),
        (
            "some",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [requirement]}
    cardinality:
      incoming: {max: some}
"#,
        ),
    ];

    for (primary, relation) in cases {
        let source = rich_schema_with_relations(relation);
        let error = assert_invalid(&source, SchemaDiagnosticCode::InvalidDeclaration);
        assert_eq!(
            source_slice(&source, only_diagnostic(&error).primary().unwrap()),
            primary
        );
    }
}

#[test]
fn enforces_each_flavour_authoring_namespace_collision_class() {
    let cases = [
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "field",
            r#"  status:
    source: {flavours: [requirement]}
    target: {flavours: [design]}
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "field",
            r#"  reviewed_by:
    source: {flavours: [design]}
    target: {flavours: [requirement]}
    inverse: status
    inverse_authoring: true
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "canonical",
            r#"  blocks:
    source: {flavours: [requirement]}
    target: {flavours: [design]}
  depends_on:
    source: {flavours: [design]}
    target: {flavours: [requirement]}
    inverse: blocks
    inverse_authoring: true
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "inverse",
            r#"  one:
    source: {flavours: [design]}
    target: {flavours: [requirement]}
    inverse: backlinks
    inverse_authoring: true
  two:
    source: {flavours: [design]}
    target: {flavours: [requirement]}
    inverse: backlinks
    inverse_authoring: true
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidName,
            "built_in",
            r#"  title:
    source: {flavours: [design]}
    target: {flavours: [requirement]}
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidName,
            "reserved",
            r#"  mentions:
    source: {flavours: [design]}
    target: {flavours: [requirement]}
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidName,
            "built_in",
            r#"  reviewed_by:
    source: {flavours: [design]}
    target: {flavours: [requirement]}
    inverse: body
    inverse_authoring: true
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidName,
            "reserved",
            r#"  reviewed_by:
    source: {flavours: [design]}
    target: {flavours: [requirement]}
    inverse: source_location
    inverse_authoring: true
"#,
        ),
    ];

    for (code, collision, relation) in cases {
        let source = rich_schema_with_relations(relation);
        let error = assert_invalid(source, code);
        assert_eq!(
            detail_string(only_diagnostic(&error), "collision"),
            Some(collision)
        );
    }

    let scoped = rich_schema_with_relations(
        r#"  status:
    source: {flavours: [design]}
    target: {flavours: [requirement]}
    inverse: mentions
    inverse_authoring: false
"#,
    );
    let fixture = Fixture::new(scoped);
    let schema = load_schema(&fixture.loaded_project()).unwrap();
    assert!(schema.relations().unwrap().get("status").is_some());
}

#[test]
fn reports_relation_diagnostics_independently_of_other_declaration_failures() {
    let endpoint_source = rich_schema_with_relations(
        r#"  relation:
    source: {flavours: [missing]}
    target: {flavours: [requirement]}
"#,
    )
    .replacen("label: Requirement", "label: ''", 1);
    let fixture = Fixture::new(&endpoint_source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();
    assert_eq!(
        error
            .diagnostics()
            .iter()
            .map(|diagnostic| source_slice(&endpoint_source, diagnostic.primary().unwrap()))
            .collect::<Vec<_>>(),
        ["''", "missing"]
    );

    let symmetric_source = rich_schema_with_relations(
        r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [missing]}
    symmetric: true
"#,
    );
    let fixture = Fixture::new(&symmetric_source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();
    assert_eq!(
        error
            .diagnostics()
            .iter()
            .map(|diagnostic| source_slice(&symmetric_source, diagnostic.primary().unwrap()))
            .collect::<Vec<_>>(),
        ["{flavours: [missing]}", "missing"]
    );

    let acyclic_source = rich_schema_with_relations(
        r#"  relation:
    source: {flavours: [requirement]}
    target: {external: [HTTPS]}
    acyclic: true
"#,
    );
    let fixture = Fixture::new(&acyclic_source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();
    assert_eq!(
        error
            .diagnostics()
            .iter()
            .map(|diagnostic| source_slice(&acyclic_source, diagnostic.primary().unwrap()))
            .collect::<Vec<_>>(),
        ["HTTPS", "true"]
    );

    let namespace_source = rich_schema_with_relations(
        r#"  status:
    source: {flavours: [requirement]}
    target: {flavours: [design]}
    cardinality:
      outgoing: {min: -1}
"#,
    );
    let fixture = Fixture::new(&namespace_source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();
    assert_eq!(error.diagnostics().len(), 2, "{:#?}", error.diagnostics());
    assert_eq!(
        error
            .diagnostics()
            .iter()
            .map(|diagnostic| source_slice(&namespace_source, diagnostic.primary().unwrap()))
            .collect::<Vec<_>>(),
        ["status", "-1"]
    );
    assert_eq!(
        error
            .diagnostics()
            .iter()
            .filter_map(|diagnostic| detail_string(diagnostic, "collision"))
            .collect::<Vec<_>>(),
        ["field"]
    );
}

#[test]
fn rejects_unknown_keys_at_every_relation_declaration_boundary() {
    let cases = [
        (
            "relations.relation",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [design]}
    lifecycle: draft
"#,
        ),
        (
            "relations.relation.source",
            r#"  relation:
    source: {flavours: [requirement], scanner: source_span}
    target: {flavours: [design]}
"#,
        ),
        (
            "relations.relation.target",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [design], uri: https}
"#,
        ),
        (
            "relations.relation.cardinality",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [design]}
    cardinality: {outgoing: {}, total: {max: 1}}
"#,
        ),
        (
            "relations.relation.cardinality.outgoing",
            r#"  relation:
    source: {flavours: [requirement]}
    target: {flavours: [design]}
    cardinality: {outgoing: {minimum: 1}}
"#,
        ),
    ];

    for (mapping, relation) in cases {
        let source = rich_schema_with_relations(relation);
        let error = assert_invalid(source, SchemaDiagnosticCode::UnknownKey);
        assert_eq!(
            detail_string(only_diagnostic(&error), "mapping"),
            Some(mapping)
        );
    }
}

#[test]
fn diagnoses_unknown_rule_shapes_and_every_reference_class() {
    let cases = [
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "custom_rule",
            r#"  - name: unknown_kind
    kind: custom_rule
    severity: error
    applies_to: {flavours: [requirement]}
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "missing",
            r#"  - name: missing_flavour
    kind: orphan
    severity: warning
    applies_to: {flavours: [missing]}
    relations: [related_to]
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "missing_relation",
            r#"  - name: missing_relation
    kind: requires_relation
    severity: error
    applies_to: {flavours: [requirement]}
    relation: missing_relation
    direction: outgoing
    min: 1
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "missing_field",
            r#"  - name: missing_field
    kind: requires_field
    severity: error
    applies_to: {flavours: [requirement]}
    field: missing_field
    min: 1
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "missing_field",
            r#"  - name: missing_condition_field
    kind: requires_field
    severity: error
    applies_to: {flavours: [requirement]}
    when: {field: missing_field, in: [draft]}
    field: estimate
    min: 1
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "retired",
            r#"  - name: invalid_condition_value
    kind: requires_field
    severity: error
    applies_to: {flavours: [requirement]}
    when: {field: status, in: [retired]}
    field: estimate
    min: 1
"#,
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "1",
            r#"  - name: invalid_bounds
    kind: requires_relation
    severity: error
    applies_to: {flavours: [requirement]}
    relation: related_to
    direction: incoming
    min: 2
    max: 1
"#,
        ),
    ];

    for (code, primary, rule) in cases {
        let source = rich_schema_with_relations_and_rules(COMPLETE_RELATIONS, rule);
        let error = assert_invalid(&source, code);
        assert_eq!(
            source_slice(&source, only_diagnostic(&error).primary().unwrap()),
            primary,
            "{:#?}",
            error.diagnostics()
        );
    }
}

#[test]
fn rejects_unknown_keys_at_every_rule_mapping_boundary() {
    let cases = [
        (
            "rules[0]",
            r#"  - name: unknown_common_key
    kind: orphan
    severity: warning
    applies_to: {flavours: [requirement]}
    relations: [related_to]
    lifecycle: draft
"#,
        ),
        (
            "rules[0].applies_to",
            r#"  - name: unknown_applicability_key
    kind: orphan
    severity: warning
    applies_to: {flavours: [requirement], fields: [status]}
    relations: [related_to]
"#,
        ),
        (
            "rules[0].when",
            r#"  - name: unknown_condition_key
    kind: orphan
    severity: warning
    applies_to: {flavours: [requirement]}
    when: {field: status, in: [draft], mode: any}
    relations: [related_to]
"#,
        ),
        (
            "rules[0]",
            r#"  - name: wrong_kind_key
    kind: orphan
    severity: warning
    applies_to: {flavours: [requirement]}
    relations: [related_to]
    direction: incoming
"#,
        ),
    ];

    for (mapping, rule) in cases {
        let source = rich_schema_with_relations_and_rules(COMPLETE_RELATIONS, rule);
        let error = assert_invalid(&source, SchemaDiagnosticCode::UnknownKey);
        assert_eq!(
            detail_string(only_diagnostic(&error), "mapping"),
            Some(mapping)
        );
    }
}

#[test]
fn rejects_duplicate_rule_names_selectors_and_sequence_values() {
    let cases = [
        (
            "duplicate_name",
            r#"  - name: duplicate_name
    kind: orphan
    severity: warning
    applies_to: {flavours: [requirement]}
    relations: [related_to]
  - name: duplicate_name
    kind: orphan
    severity: info
    applies_to: {flavours: [design]}
    relations: [related_to]
"#,
        ),
        (
            "relation",
            r#"  - name: conflicting_selector
    kind: requires_relation
    severity: error
    applies_to: {flavours: [requirement]}
    relation: related_to
    relation_any_of: [related_to]
    direction: incoming
    min: 1
"#,
        ),
        (
            "related_to",
            r#"  - name: duplicate_relation
    kind: orphan
    severity: warning
    applies_to: {flavours: [requirement]}
    relations: [related_to, related_to]
"#,
        ),
        (
            "draft",
            r#"  - name: duplicate_condition_value
    kind: requires_field
    severity: error
    applies_to: {flavours: [requirement]}
    when: {field: status, in: [draft, draft]}
    field: estimate
    min: 1
"#,
        ),
    ];

    for (primary, rule) in cases {
        let source = rich_schema_with_relations_and_rules(COMPLETE_RELATIONS, rule);
        let error = assert_invalid(&source, SchemaDiagnosticCode::InvalidDeclaration);
        assert_eq!(
            source_slice(&source, only_diagnostic(&error).primary().unwrap()),
            primary,
            "{:#?}",
            error.diagnostics()
        );
    }
}

fn cross_flavour_condition_schema(values: &str, design_repeatable: bool) -> String {
    format!(
        r#"format_version: 1
schema:
  name: condition-schema
  version: 1.0.0
identity:
  mid:
    format: ulid
    prefix: condition_
flavours:
  requirement:
    label: Requirement
    description: A verifiable obligation.
    guidance:
      use_when: [Define an obligation.]
      avoid_when: [Describe a solution.]
    id: {{}}
    title: {{}}
    body: {{}}
    fields:
      status:
        type: enum
        values: [draft, approved]
  design:
    label: Design
    description: A chosen solution.
    guidance:
      use_when: [Describe a solution.]
      avoid_when: [Define an obligation.]
    id: {{}}
    title: {{}}
    body: {{}}
    fields:
      status:
        type: enum
        repeatable: {design_repeatable}
        values: [draft, accepted]
relations:
  related_to:
    source: {{flavours: [requirement, design]}}
    target: {{flavours: [requirement, design]}}
    symmetric: true
rules:
  - name: accepted_items_are_connected
    kind: orphan
    severity: warning
    applies_to: {{flavours: [requirement, design]}}
    when: {{field: status, in: [{values}]}}
    relations: [related_to]
"#
    )
}

#[test]
fn applies_cross_flavour_condition_union_semantics_and_rejects_repeatable_fields() {
    let union = cross_flavour_condition_schema("approved, accepted", false);
    let fixture = Fixture::new(&union);
    let document = load_schema(&fixture.loaded_project()).unwrap();
    let condition = document.rules().unwrap().definitions()[0]
        .condition()
        .unwrap()
        .value();
    assert_eq!(
        condition
            .values()
            .value()
            .iter()
            .map(|value| condition_string(value.value()))
            .collect::<Vec<_>>(),
        ["approved", "accepted"]
    );

    let outside_union = cross_flavour_condition_schema("approved, retired", false);
    let error = assert_invalid(&outside_union, SchemaDiagnosticCode::InvalidDeclaration);
    assert_eq!(
        source_slice(&outside_union, only_diagnostic(&error).primary().unwrap()),
        "retired"
    );

    let repeatable = cross_flavour_condition_schema("approved", true);
    let error = assert_invalid(&repeatable, SchemaDiagnosticCode::InvalidDeclaration);
    let diagnostic = only_diagnostic(&error);
    assert_eq!(
        source_slice(&repeatable, diagnostic.primary().unwrap()),
        "status"
    );
    assert_eq!(detail_string(diagnostic, "flavour"), Some("design"));
}

#[test]
fn compiles_typed_condition_values_against_each_scalar_field_domain() {
    let source = rich_schema_with_relations_and_rules(
        COMPLETE_RELATIONS,
        r#"  - name: estimate_condition
    kind: requires_field
    severity: error
    applies_to: {flavours: [requirement]}
    when: {field: estimate, in: [-2, 1]}
    field: estimate
    min: 1
  - name: confidence_condition
    kind: requires_field
    severity: warning
    applies_to: {flavours: [requirement]}
    when: {field: confidence, in: [1, 2.5]}
    field: confidence
    min: 1
  - name: automation_condition
    kind: requires_field
    severity: info
    applies_to: {flavours: [requirement]}
    when: {field: automated, in: [true, false]}
    field: automated
    min: 1
"#,
    );
    let fixture = Fixture::new(source);
    let document = load_schema(&fixture.loaded_project()).unwrap();
    let rules = document.rules().unwrap();

    assert!(matches!(
        rules
            .get("estimate_condition")
            .unwrap()
            .condition()
            .unwrap()
            .value()
            .values()
            .value()[0]
            .value(),
        RuleConditionValue::Integer(-2)
    ));
    assert!(matches!(
        rules
            .get("confidence_condition")
            .unwrap()
            .condition()
            .unwrap()
            .value()
            .values()
            .value()[0]
            .value(),
        RuleConditionValue::Integer(1)
    ));
    let RuleConditionValue::Number(number) = rules
        .get("confidence_condition")
        .unwrap()
        .condition()
        .unwrap()
        .value()
        .values()
        .value()[1]
        .value()
    else {
        panic!("expected a floating-point condition value")
    };
    assert_eq!(number.get(), 2.5);
    assert!(matches!(
        rules
            .get("automation_condition")
            .unwrap()
            .condition()
            .unwrap()
            .value()
            .values()
            .value()[0]
            .value(),
        RuleConditionValue::Boolean(true)
    ));
}

#[test]
fn preserves_independent_diagnostics_across_the_normative_compilation_stages() {
    let source = rich_schema_with_relations_and_rules(
        r#"  broken_relation:
    source: {flavours: [requirement]}
    target: {flavours: [missing_flavour]}
"#,
        r#"  - name: broken_rule
    kind: requires_relation
    severity: error
    applies_to: {flavours: [requirement]}
    relation: missing_relation
    direction: outgoing
    min: 1
"#,
    )
    .replacen("name: rich-schema", "name: Invalid_Name", 1)
    .replacen("label: Requirement", "label: ''", 1);
    let fixture = Fixture::new(&source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();

    assert_eq!(
        error
            .diagnostics()
            .iter()
            .map(|diagnostic| source_slice(&source, diagnostic.primary().unwrap()))
            .collect::<Vec<_>>(),
        ["Invalid_Name", "''", "missing_flavour", "missing_relation"]
    );

    let same_rule = rich_schema_with_relations_and_rules(
        COMPLETE_RELATIONS,
        r#"  - name: independently_broken_rule
    kind: requires_field
    severity: error
    applies_to: {flavours: [requirement, missing_flavour]}
    when: {field: missing_condition, in: [draft]}
    field: missing_field
    min: 1
"#,
    );
    let fixture = Fixture::new(&same_rule);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();
    assert_eq!(
        error
            .diagnostics()
            .iter()
            .map(|diagnostic| source_slice(&same_rule, diagnostic.primary().unwrap()))
            .collect::<Vec<_>>(),
        ["missing_flavour", "missing_condition", "missing_field"]
    );
}

#[test]
fn rejects_a_non_sequence_rule_root_without_producing_a_schema_model() {
    let source = VALID_SCHEMA.replace("rules: []", "rules: {}");
    let error = assert_invalid(&source, SchemaDiagnosticCode::InvalidDeclaration);
    assert_eq!(
        source_slice(&source, only_diagnostic(&error).primary().unwrap()),
        "{}"
    );
}

#[test]
fn accepts_unicode_rust_patterns_and_rejects_invalid_patterns_at_the_value() {
    let unicode = RICH_SCHEMA.replace("pattern: REQ-[0-9]+", "pattern: '\\p{Greek}+'");
    let fixture = Fixture::new(&unicode);
    let document = load_schema(&fixture.loaded_project()).unwrap();
    assert_eq!(
        document
            .flavours()
            .get("requirement")
            .unwrap()
            .display_id()
            .value()
            .pattern()
            .unwrap()
            .value(),
        r"\p{Greek}+"
    );

    let verbose_comment = RICH_SCHEMA.replace(
        "pattern: REQ-[0-9]+",
        "pattern: '(?x)REQ-[0-9]+ # trailing comment'",
    );
    let fixture = Fixture::new(&verbose_comment);
    let document = load_schema(&fixture.loaded_project()).unwrap();
    assert_eq!(
        document
            .flavours()
            .get("requirement")
            .unwrap()
            .display_id()
            .value()
            .pattern()
            .unwrap()
            .value(),
        "(?x)REQ-[0-9]+ # trailing comment"
    );

    for source in [
        RICH_SCHEMA.replace("pattern: REQ-[0-9]+", "pattern: '('"),
        RICH_SCHEMA.replace("pattern: .+", "pattern: '[unterminated'"),
    ] {
        let error = assert_invalid(source.clone(), SchemaDiagnosticCode::InvalidPattern);
        let diagnostic = only_diagnostic(&error);
        let invalid_pattern = source_slice(&source, diagnostic.primary().unwrap());
        assert!(invalid_pattern.contains('(') || invalid_pattern.contains('['));
    }
}

#[test]
fn rejects_forbidden_field_shapes_and_no_default_keys() {
    let cases = [
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            RICH_SCHEMA.replace(
                "type: integer",
                "type: integer\n        values: [small, large]",
            ),
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            RICH_SCHEMA.replace("type: integer", "type: integer\n        pattern: '[0-9]+'"),
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            RICH_SCHEMA.replace(
                "        values:\n          - draft\n          - approved\n",
                "",
            ),
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            RICH_SCHEMA.replace(
                "        values:\n          - draft\n          - approved",
                "        values: []",
            ),
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            RICH_SCHEMA.replace("          - approved", "          - draft"),
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            RICH_SCHEMA.replace("type: boolean", "type: object"),
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            RICH_SCHEMA.replace("          - approved", "          - 1"),
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            RICH_SCHEMA.replace("        type: boolean", "        required: true"),
        ),
        (
            SchemaDiagnosticCode::UnknownKey,
            RICH_SCHEMA.replace("type: boolean", "type: boolean\n        default: false"),
        ),
        (
            SchemaDiagnosticCode::UnknownKey,
            RICH_SCHEMA.replace("type: boolean", "type: boolean\n        nullable: true"),
        ),
    ];

    for (code, source) in cases {
        assert_invalid(source, code);
    }
}

#[test]
fn requires_complete_non_empty_flavour_guidance_and_builtin_declarations() {
    let missing_cases = [
        "    label: Requirement\n",
        "    description: A verifiable obligation.\n",
        concat!(
            "    guidance:\n",
            "      use_when: [Document an externally visible obligation.]\n",
            "      avoid_when: [The content explains an implementation choice.]\n",
        ),
        "    id: {}\n",
        "    title: {}\n",
        "    body: {}\n",
    ];
    for declaration in missing_cases {
        let source = VALID_SCHEMA.replacen(declaration, "", 1);
        assert_invalid(source, SchemaDiagnosticCode::InvalidDeclaration);
    }

    for (authored, replacement) in [
        ("label: Requirement", "label: ''"),
        ("description: A verifiable obligation.", "description: ''"),
        ("id: {}", "id: []"),
        ("title: {}", "title: []"),
        ("body: {}", "body: []"),
    ] {
        let source = VALID_SCHEMA.replacen(authored, replacement, 1);
        assert_invalid(source, SchemaDiagnosticCode::InvalidDeclaration);
    }

    let invalid_boolean = RICH_SCHEMA.replacen("required: true", "required: yes", 1);
    assert_invalid(invalid_boolean, SchemaDiagnosticCode::InvalidDeclaration);
    let invalid_fields =
        VALID_SCHEMA.replacen("    body: {}\n", "    body: {}\n    fields: []\n", 1);
    assert_invalid(invalid_fields, SchemaDiagnosticCode::InvalidDeclaration);
}

#[test]
fn rejects_invalid_and_reserved_declaration_names_at_their_keys() {
    let invalid_flavour = VALID_SCHEMA.replace("  requirement:", "  Requirement:");
    let error = assert_invalid(invalid_flavour.clone(), SchemaDiagnosticCode::InvalidName);
    assert_eq!(
        source_slice(&invalid_flavour, only_diagnostic(&error).primary().unwrap()),
        "Requirement"
    );

    let invalid_field = RICH_SCHEMA.replace("      summary:", "      bad-name:");
    let error = assert_invalid(invalid_field.clone(), SchemaDiagnosticCode::InvalidName);
    assert_eq!(
        source_slice(&invalid_field, only_diagnostic(&error).primary().unwrap()),
        "bad-name"
    );

    for reserved in [
        "mid",
        "flavour",
        "id",
        "title",
        "body",
        "source_location",
        "mentions",
    ] {
        let source = RICH_SCHEMA.replace("      summary:", &format!("      {reserved}:"));
        let error = assert_invalid(source.clone(), SchemaDiagnosticCode::InvalidName);
        assert_eq!(
            source_slice(&source, only_diagnostic(&error).primary().unwrap()),
            reserved
        );
    }
}

#[test]
fn rejects_missing_empty_duplicate_and_inconsistent_guidance_at_source() {
    let missing = VALID_SCHEMA.replace(
        "      use_when: [Document an externally visible obligation.]\n",
        "",
    );
    assert_invalid(missing, SchemaDiagnosticCode::InvalidDeclaration);

    let empty_sequence = VALID_SCHEMA.replace("[Document an externally visible obligation.]", "[]");
    let error = assert_invalid(
        empty_sequence.clone(),
        SchemaDiagnosticCode::InvalidDeclaration,
    );
    assert_eq!(
        source_slice(&empty_sequence, only_diagnostic(&error).primary().unwrap()),
        "[]"
    );

    let empty_entry = VALID_SCHEMA.replace("[Document an externally visible obligation.]", "['']");
    let error = assert_invalid(
        empty_entry.clone(),
        SchemaDiagnosticCode::InvalidDeclaration,
    );
    assert_eq!(
        source_slice(&empty_entry, only_diagnostic(&error).primary().unwrap()),
        "''"
    );

    let duplicate = VALID_SCHEMA.replace(
        "[Document an externally visible obligation.]",
        "[same, same]",
    );
    assert_invalid(duplicate, SchemaDiagnosticCode::InvalidDeclaration);

    let self_distinction = RICH_SCHEMA.replace(
        "        design: Describes the solution rather than the obligation.",
        "        requirement: Describes the solution rather than the obligation.",
    );
    let error = assert_invalid(
        self_distinction.clone(),
        SchemaDiagnosticCode::InvalidDeclaration,
    );
    assert_eq!(
        source_slice(
            &self_distinction,
            only_diagnostic(&error).primary().unwrap()
        ),
        "requirement"
    );

    let unknown_distinction = RICH_SCHEMA.replace(
        "        design: Describes the solution rather than the obligation.",
        "        risk: Describes the solution rather than the obligation.",
    );
    let error = assert_invalid(
        unknown_distinction.clone(),
        SchemaDiagnosticCode::InvalidDeclaration,
    );
    assert_eq!(
        source_slice(
            &unknown_distinction,
            only_diagnostic(&error).primary().unwrap()
        ),
        "risk"
    );
}

#[test]
fn rejects_unknown_keys_at_every_flavour_declaration_boundary() {
    let cases = [
        RICH_SCHEMA.replacen(
            "    description:",
            "    lifecycle: draft\n    description:",
            1,
        ),
        RICH_SCHEMA.replacen(
            "      use_when:",
            "      prompt: choose carefully\n      use_when:",
            1,
        ),
        RICH_SCHEMA.replace(
            "      pattern: REQ",
            "      generator: sequence\n      pattern: REQ",
        ),
        RICH_SCHEMA.replacen("    title:\n", "    title:\n      pattern: .+\n", 1),
        RICH_SCHEMA.replacen("    body: {}", "    body:\n      format: markdown", 1),
        RICH_SCHEMA.replace(
            "        type: boolean",
            "        type: boolean\n        default: false",
        ),
    ];

    for source in cases {
        assert_invalid(source, SchemaDiagnosticCode::UnknownKey);
    }
}

#[test]
fn reads_only_the_loaded_projects_configured_schema_path() {
    let fixture = Fixture::with_path(".mara/selected.yaml", VALID_SCHEMA);
    fs::write(
        fixture.root.join(".mara/schema.yaml"),
        b"not: the selected schema\n",
    )
    .unwrap();

    let document = load_schema(&fixture.loaded_project()).unwrap();

    assert_eq!(document.source().path(), ".mara/selected.yaml");
    assert_eq!(document.schema().value().name().value(), "mara-schema");
}

#[test]
fn reports_schema_io_with_the_affected_path_and_preserved_cause() {
    let fixture = Fixture::new(VALID_SCHEMA);
    let project = fixture.loaded_project();
    fs::remove_file(fixture.schema_path()).unwrap();

    let error = load_schema(&project).unwrap_err();

    assert_code(only_diagnostic(&error), SchemaDiagnosticCode::Io);
    assert_eq!(error.path(), Some(project.schema_path.as_path()));
    assert!(error.io_source().is_some());
    assert_eq!(
        detail_string(only_diagnostic(&error), "path"),
        Some(".mara/schema.yaml")
    );
}

#[test]
fn rejects_malformed_utf8_at_the_first_invalid_byte() {
    let fixture = Fixture::new([b'#', b' ', 0xc3, 0x28]);

    let error = load_schema(&fixture.loaded_project()).unwrap_err();
    let diagnostic = only_diagnostic(&error);

    assert_code(diagnostic, SchemaDiagnosticCode::Syntax);
    let primary = diagnostic.primary().unwrap();
    assert_eq!(primary.path(), ".mara/schema.yaml");
    assert_eq!(primary.start_byte(), 2);
    assert_eq!(primary.end_byte(), 2);
    assert_eq!((primary.start_line(), primary.start_column()), (1, 3));
    assert_eq!(detail_string(diagnostic, "feature"), Some("invalid_utf8"));
}

#[test]
fn rejects_nul_before_the_parser_can_truncate_the_document() {
    let source = format!("{VALID_SCHEMA}\0---\n{VALID_SCHEMA}");
    let error = assert_invalid(&source, SchemaDiagnosticCode::Syntax);
    let diagnostic = only_diagnostic(&error);
    let primary = diagnostic.primary().unwrap();

    assert_eq!(detail_string(diagnostic, "feature"), Some("nul_character"));
    assert_eq!(primary.start_byte(), VALID_SCHEMA.len() as u64);
    assert_eq!(source_slice(&source, primary), "\0");
}

#[test]
fn rejects_multiple_documents_at_the_second_document_marker() {
    let source = format!("{VALID_SCHEMA}---\n{VALID_SCHEMA}");
    let error = assert_invalid(source, SchemaDiagnosticCode::Syntax);
    let diagnostic = only_diagnostic(&error);

    assert_eq!(
        detail_string(diagnostic, "feature"),
        Some("multiple_documents")
    );
    assert_eq!(
        diagnostic.primary().unwrap().start_line(),
        (VALID_SCHEMA.lines().count() + 1) as u64
    );
}

#[test]
fn rejects_empty_documents_at_an_exact_eof_span() {
    for source in ["", "# comment only\r\n"] {
        let error = assert_invalid(source, SchemaDiagnosticCode::Syntax);
        let diagnostic = only_diagnostic(&error);
        let primary = diagnostic.primary().expect("empty input has an EOF span");

        assert_eq!(detail_string(diagnostic, "feature"), Some("empty_document"));
        assert_eq!(primary.start_byte(), source.len() as u64);
        assert_eq!(primary.end_byte(), source.len() as u64);
        if source.is_empty() {
            assert_eq!((primary.start_line(), primary.start_column()), (1, 1));
        } else {
            assert_eq!((primary.start_line(), primary.start_column()), (2, 1));
        }
    }
}

#[test]
fn accepts_yaml_1_2_directive_and_rejects_other_directives() {
    let explicit_v1 = format!("%YAML 1.2\n---\n{VALID_SCHEMA}");
    let fixture = Fixture::new(explicit_v1);
    load_schema(&fixture.loaded_project()).unwrap();

    let cases = [
        ("%YAML 1.1\n---\n", "unsupported_yaml_version"),
        ("%YAML1.2\n---\n", "unsupported_directive"),
        ("%TAG !e! tag:example.com,2026:\n---\n", "custom_tag"),
        ("%TAGGED value\n---\n", "unsupported_directive"),
        ("%FUTURE value\n---\n", "unsupported_directive"),
    ];
    for (directive, feature) in cases {
        let source = format!("{directive}{VALID_SCHEMA}");
        let error = assert_invalid(source, SchemaDiagnosticCode::Syntax);
        let diagnostic = only_diagnostic(&error);
        assert_eq!(detail_string(diagnostic, "feature"), Some(feature));
        assert_eq!(diagnostic.primary().unwrap().start_line(), 1);
        assert_eq!(diagnostic.primary().unwrap().start_column(), 1);
    }

    let bare_cr = format!("%YAML 1.2\r%TAG !e! tag:example.com,2026:\r---\r{VALID_SCHEMA}");
    let error = assert_invalid(bare_cr, SchemaDiagnosticCode::Syntax);
    let diagnostic = only_diagnostic(&error);
    assert_eq!(detail_string(diagnostic, "feature"), Some("custom_tag"));
    assert_eq!(diagnostic.primary().unwrap().start_line(), 2);
    assert_eq!(diagnostic.primary().unwrap().start_column(), 1);
}

#[test]
fn validates_and_drops_deep_invalid_rule_values_iteratively() {
    const DEPTH: usize = 20_000;
    let nested = format!("{}leaf", "- ".repeat(DEPTH));
    let source = format!(
        "format_version: 1\nschema:\n  name: deep-schema\n  version: 1.0.0\nidentity:\n  mid:\n    format: ulid\n    prefix: deep_\nflavours: {{}}\nrules:\n  {nested}\n"
    );

    let fixture = Fixture::new(source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();

    assert_eq!(error.diagnostics().len(), 1);
    assert_eq!(
        error.diagnostics()[0].message(),
        "rules[] must be a mapping"
    );
}

#[test]
fn rejects_semantically_duplicate_mapping_keys() {
    let source = VALID_SCHEMA.replace(
        "  name: mara-schema",
        "  name: mara-schema\n  \"name\": duplicate",
    );
    let error = assert_invalid(source, SchemaDiagnosticCode::DuplicateKey);
    let diagnostic = only_diagnostic(&error);

    assert_eq!(detail_string(diagnostic, "key"), Some("name"));
    assert_eq!(diagnostic.primary().unwrap().start_line(), 5);
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(diagnostic.related()[0].span().start_line(), 4);
}

#[test]
fn rejects_every_forbidden_yaml_profile_feature() {
    let cases = [
        (
            "custom_tag",
            VALID_SCHEMA.replace("name: mara-schema", "name: !custom mara-schema"),
        ),
        (
            "anchor",
            VALID_SCHEMA.replace("name: mara-schema", "name: &schema_name mara-schema"),
        ),
        (
            "merge_key",
            VALID_SCHEMA.replace("  requirement:", "  <<: {}\n  requirement:"),
        ),
        (
            "non_string_key",
            VALID_SCHEMA.replace("  requirement:", "  1: {}\n  requirement:"),
        ),
        (
            "null",
            VALID_SCHEMA.replace("relations: {}", "relations: null"),
        ),
    ];

    for (feature, source) in cases {
        let error = assert_invalid(source, SchemaDiagnosticCode::Syntax);
        assert_eq!(
            detail_string(only_diagnostic(&error), "feature"),
            Some(feature),
            "feature {feature}"
        );
    }
}

#[test]
fn rejects_aliases_in_addition_to_their_required_anchor() {
    let source = VALID_SCHEMA
        .replace("name: mara-schema", "name: &schema_name mara-schema")
        .replace("version: 1.2.3-alpha.1+build.5", "version: *schema_name");
    let fixture = Fixture::new(&source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();

    let features = error
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| detail_string(diagnostic, "feature"))
        .collect::<Vec<_>>();
    assert_eq!(features, ["anchor", "alias"]);
    let anchor = error
        .diagnostics()
        .iter()
        .find(|diagnostic| detail_string(diagnostic, "feature") == Some("anchor"))
        .unwrap();
    assert_eq!(
        source_slice(&source, anchor.primary().unwrap()),
        "&schema_name"
    );
}

#[test]
fn preserves_a_tag_span_when_the_tag_precedes_an_anchor() {
    let source = VALID_SCHEMA.replace(
        "name: mara-schema",
        "name: !custom &schema_name mara-schema",
    );
    let fixture = Fixture::new(&source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();

    let custom_tag = error
        .diagnostics()
        .iter()
        .find(|diagnostic| detail_string(diagnostic, "feature") == Some("custom_tag"))
        .unwrap();
    let anchor = error
        .diagnostics()
        .iter()
        .find(|diagnostic| detail_string(diagnostic, "feature") == Some("anchor"))
        .unwrap();
    assert_eq!(
        source_slice(&source, custom_tag.primary().unwrap()),
        "!custom &schema_name mara-schema"
    );
    assert_eq!(
        source_slice(&source, anchor.primary().unwrap()),
        "&schema_name"
    );
}

#[test]
fn accepts_yaml_1_2_core_tags_but_rejects_incompatible_core_tags() {
    let source = r#"!!str format_version: !!int "1"
!!str schema: !!map
  !!str name: !!str core-schema
  !!str version: ! 1.0.0
!!str identity: !!map
  !!str mid: !!map
    !!str format: !!str ulid
    !!str prefix: !!str core_
!!str flavours: !!map {}
!!str relations: !!map {}
!!str rules: !!seq []
"#;
    let fixture = Fixture::new(source);
    let document = load_schema(&fixture.loaded_project()).unwrap();
    assert_eq!(document.schema().value().name().value(), "core-schema");
    assert_eq!(
        source_slice(source, document.format_version().key_source()),
        "!!str format_version"
    );
    assert_eq!(
        source_slice(source, document.format_version().value_source()),
        "!!int \"1\""
    );
    assert!(source_slice(source, document.schema().value_source()).starts_with("!!map\n"));
    assert_eq!(
        source_slice(source, document.schema().value().name().value_source()),
        "!!str core-schema"
    );
    assert_eq!(
        source_slice(source, document.schema().value().version().value_source()),
        "! 1.0.0"
    );
    assert_eq!(
        source_slice(source, document.relations().unwrap().value_source()),
        "!!map {}"
    );
    assert_eq!(
        source_slice(source, document.rules().unwrap().value_source()),
        "!!seq []"
    );

    let verbatim_tag = "!<tag:yaml.org,2002:%73tr> core-schema";
    let verbatim_source = source.replace("!!str core-schema", verbatim_tag);
    let fixture = Fixture::new(&verbatim_source);
    let document = load_schema(&fixture.loaded_project()).unwrap();
    assert_eq!(
        source_slice(
            &verbatim_source,
            document.schema().value().name().value_source()
        ),
        verbatim_tag
    );

    let commented_tag_source = source.replace(
        "  !!str version: ! 1.0.0",
        "  !!str version: !!str\n    # !\n    1.0.0",
    );
    let fixture = Fixture::new(&commented_tag_source);
    let document = load_schema(&fixture.loaded_project()).unwrap();
    assert_eq!(
        source_slice(
            &commented_tag_source,
            document.schema().value().version().value_source()
        ),
        "!!str\n    # !\n    1.0.0"
    );

    let preceding_comment_source = source.replace(
        "  !!str name: !!str core-schema",
        "  !!str name:\n    # misleading !tag\n    !!str core-schema",
    );
    let fixture = Fixture::new(&preceding_comment_source);
    let document = load_schema(&fixture.loaded_project()).unwrap();
    assert_eq!(
        source_slice(
            &preceding_comment_source,
            document.schema().value().name().value_source()
        ),
        "!!str core-schema"
    );

    let quoted_hash_source = source.replace(
        "  !!str name: !!str core-schema",
        "  !!str name: [\"# quoted\", !custom\n    core-schema]",
    );
    let fixture = Fixture::new(&quoted_hash_source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();
    let custom_tag = error
        .diagnostics()
        .iter()
        .find(|diagnostic| detail_string(diagnostic, "feature") == Some("custom_tag"))
        .unwrap();
    assert_eq!(
        source_slice(&quoted_hash_source, custom_tag.primary().unwrap()),
        "!custom\n    core-schema"
    );

    let invalid = source.replace("!!str core-schema", "!!timestamp core-schema");
    let error = assert_invalid(invalid, SchemaDiagnosticCode::Syntax);
    assert_eq!(
        detail_string(only_diagnostic(&error), "feature"),
        Some("custom_tag")
    );
}

#[test]
fn preserves_the_complete_authored_block_scalar_span() {
    let source = VALID_SCHEMA.replace(
        "version: 1.2.3-alpha.1+build.5",
        "version: |- # |\n    1.2.3-alpha.1+build.5",
    );
    let fixture = Fixture::new(&source);
    let document = load_schema(&fixture.loaded_project()).unwrap();

    assert_eq!(
        source_slice(&source, document.schema().value().version().value_source()),
        "|- # |\n    1.2.3-alpha.1+build.5\n"
    );
}

#[test]
fn rejects_non_decimal_scalars_explicitly_tagged_as_floats() {
    for value in ["0x3A", "0o7"] {
        let source = VALID_SCHEMA.replace(
            "relations: {}",
            &format!("relations: {{invalid: !!float {value}}}"),
        );
        let error = assert_invalid(source, SchemaDiagnosticCode::Syntax);
        assert_eq!(
            detail_string(only_diagnostic(&error), "feature"),
            Some("custom_tag")
        );
    }
}

#[test]
fn unsupported_format_selection_precedes_v1_closed_mapping_checks() {
    let source = r#"format_version: 2
future_root: true
"#;
    let error = assert_invalid(source, SchemaDiagnosticCode::UnsupportedFormat);
    let diagnostic = only_diagnostic(&error);

    assert_eq!(diagnostic.primary().unwrap().start_line(), 1);
    assert_eq!(detail_string(diagnostic, "format_version"), Some("2"));
}

#[test]
fn rejects_unknown_keys_at_every_decoded_mapping_boundary() {
    let appended_line = (VALID_SCHEMA.lines().count() + 1) as u64;
    let cases = [
        (
            "root",
            format!("{VALID_SCHEMA}imports: other.yaml\n"),
            appended_line,
        ),
        (
            "schema",
            VALID_SCHEMA.replace("  version:", "  owner: team\n  version:"),
            5,
        ),
        (
            "identity",
            VALID_SCHEMA.replace("  mid:", "  generator: random\n  mid:"),
            7,
        ),
        (
            "identity.mid",
            VALID_SCHEMA.replace("    prefix:", "    length: 26\n    prefix:"),
            9,
        ),
    ];

    for (mapping, source, line) in cases {
        let error = assert_invalid(source, SchemaDiagnosticCode::UnknownKey);
        let diagnostic = only_diagnostic(&error);
        assert_eq!(detail_string(diagnostic, "mapping"), Some(mapping));
        assert_eq!(diagnostic.primary().unwrap().start_line(), line);
    }
}

#[test]
fn reports_all_independent_unknown_keys_in_source_order() {
    let source = VALID_SCHEMA
        .replace("  version:", "  owner: team\n  version:")
        .replace("  mid:", "  generator: random\n  mid:")
        .replace("    prefix:", "    length: 26\n    prefix:")
        + "imports: first.yaml\nplugins: [example]\n";
    let fixture = Fixture::new(&source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();

    let diagnostics = error.diagnostics();
    assert_eq!(diagnostics.len(), 5, "{diagnostics:#?}");
    for diagnostic in diagnostics {
        assert_code(diagnostic, SchemaDiagnosticCode::UnknownKey);
    }
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| detail_string(diagnostic, "key").unwrap())
            .collect::<Vec<_>>(),
        ["owner", "generator", "length", "imports", "plugins"]
    );
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| detail_string(diagnostic, "mapping").unwrap())
            .collect::<Vec<_>>(),
        ["schema", "identity", "identity.mid", "root", "root"]
    );
    assert!(diagnostics.windows(2).all(|pair| {
        pair[0].primary().unwrap().start_byte() < pair[1].primary().unwrap().start_byte()
    }));
}

#[test]
fn reports_unknown_keys_together_with_independent_declaration_defects() {
    let source = VALID_SCHEMA.replace("    label: Requirement", "    owner: team\n    label: ''");
    let fixture = Fixture::new(&source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();

    let diagnostics = error.diagnostics();
    assert_eq!(diagnostics.len(), 2, "{diagnostics:#?}");
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| (
                diagnostic.code().as_str(),
                source_slice(&source, diagnostic.primary().unwrap()),
            ))
            .collect::<Vec<_>>(),
        [
            ("schema.unknown_key", "owner"),
            ("schema.invalid_declaration", "''"),
        ]
    );
}

#[test]
fn reports_all_independent_flavour_and_field_defects_regardless_of_mapping_order() {
    let source = RICH_SCHEMA
        .replacen("label: Requirement", "label: ''", 1)
        .replacen("pattern: .+", "pattern: '['", 1)
        .replacen("type: integer", "type: integer\n        values: [small]", 1)
        .replacen(
            "description: A chosen implementation structure.",
            "description: ''",
            1,
        );
    let flavours_start = source.find("flavours:\n").unwrap() + "flavours:\n".len();
    let design_start = source.find("  design:\n").unwrap();
    let reordered = format!(
        "{}{}{}",
        &source[..flavours_start],
        &source[design_start..],
        &source[flavours_start..design_start]
    );

    let first_fixture = Fixture::new(&source);
    let first = load_schema(&first_fixture.loaded_project()).unwrap_err();
    let reordered_fixture = Fixture::new(&reordered);
    let reordered_error = load_schema(&reordered_fixture.loaded_project()).unwrap_err();

    assert_eq!(first.diagnostics().len(), 4, "{:#?}", first.diagnostics());
    assert_eq!(
        first
            .diagnostics()
            .iter()
            .map(|diagnostic| source_slice(&source, diagnostic.primary().unwrap()))
            .collect::<Vec<_>>(),
        ["''", "'['", "values", "''"]
    );

    let identities = |error: &SchemaLoadError| {
        let mut identities = error
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.code().as_str().to_owned(),
                    diagnostic.context().field().unwrap_or("").to_owned(),
                    diagnostic.message().to_owned(),
                )
            })
            .collect::<Vec<_>>();
        identities.sort_unstable();
        identities
    };
    assert_eq!(identities(&first), identities(&reordered_error));
}

#[test]
fn reports_root_flavour_and_sequence_defects_from_one_compilation() {
    let source = VALID_SCHEMA
        .replace("  requirement:", "  bad-name:")
        .replace("label: Requirement", "label: ''")
        .replace("[Document an externally visible obligation.]", "[1, '']")
        .replace("relations: {}", "relations: []")
        .replace("rules: []", "rules: {}");
    let fixture = Fixture::new(&source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();

    let diagnostics = error.diagnostics();
    assert_eq!(diagnostics.len(), 6, "{diagnostics:#?}");
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| source_slice(&source, diagnostic.primary().unwrap()))
            .collect::<Vec<_>>(),
        ["bad-name", "''", "1", "''", "[]", "{}"]
    );
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .collect::<Vec<_>>(),
        [
            "schema.invalid_name",
            "schema.invalid_declaration",
            "schema.invalid_declaration",
            "schema.invalid_declaration",
            "schema.invalid_declaration",
            "schema.invalid_declaration",
        ]
    );
}

#[test]
fn reports_all_schema_and_mid_identity_defects_from_one_compilation() {
    let source = VALID_SCHEMA
        .replace("name: mara-schema", "name: Mara_Schema")
        .replace("1.2.3-alpha.1+build.5", "\"1.2\"")
        .replace("format: ulid", "format: uuid")
        .replace("prefix: m_", "prefix: M-");
    let fixture = Fixture::new(&source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();

    let diagnostics = error.diagnostics();
    assert_eq!(diagnostics.len(), 4, "{diagnostics:#?}");
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| (
                diagnostic.code().as_str(),
                source_slice(&source, diagnostic.primary().unwrap()),
            ))
            .collect::<Vec<_>>(),
        [
            ("schema.invalid_name", "Mara_Schema"),
            ("schema.invalid_declaration", "\"1.2\""),
            ("schema.invalid_declaration", "uuid"),
            ("schema.invalid_name", "M-"),
        ]
    );
}

#[test]
fn rejects_unsupported_root_composition_constructs_as_unknown_v1_keys() {
    for key in [
        "imports",
        "extends",
        "defaults",
        "scripts",
        "plugins",
        "environment",
    ] {
        let source = format!("{VALID_SCHEMA}{key}: value\n");
        let error = assert_invalid(source, SchemaDiagnosticCode::UnknownKey);
        assert_eq!(detail_string(only_diagnostic(&error), "key"), Some(key));
    }
}

#[test]
fn rejects_invalid_schema_name_version_and_mid_settings() {
    let cases = [
        (
            SchemaDiagnosticCode::InvalidName,
            "schema.name",
            VALID_SCHEMA.replace("name: mara-schema", "name: Mara_Schema"),
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "schema.version",
            VALID_SCHEMA.replace("1.2.3-alpha.1+build.5", "\"1.2\""),
        ),
        (
            SchemaDiagnosticCode::InvalidDeclaration,
            "identity.mid.format",
            VALID_SCHEMA.replace("format: ulid", "format: uuid"),
        ),
        (
            SchemaDiagnosticCode::InvalidName,
            "identity.mid.prefix",
            VALID_SCHEMA.replace("prefix: m_", "prefix: M-"),
        ),
    ];

    for (code, field, source) in cases {
        let error = assert_invalid(source, code);
        assert_eq!(only_diagnostic(&error).context().field(), Some(field));
    }
}

#[test]
fn validates_semver_2_syntax_without_a_numeric_size_limit() {
    let oversized = "18446744073709551616.0.0";
    let valid = VALID_SCHEMA.replace("1.2.3-alpha.1+build.5", &format!("\"{oversized}\""));
    let fixture = Fixture::new(valid);
    let document = load_schema(&fixture.loaded_project()).unwrap();
    assert_eq!(document.schema().value().version().value(), oversized);

    for invalid in ["01.0.0", "1.0.0-01", "1.0.0-", "1.0.0+"] {
        let source = VALID_SCHEMA.replace("1.2.3-alpha.1+build.5", &format!("\"{invalid}\""));
        assert_invalid(source, SchemaDiagnosticCode::InvalidDeclaration);
    }
}

#[test]
fn rejects_malformed_yaml_and_non_mapping_document_roots() {
    let malformed = assert_invalid("schema: [\n", SchemaDiagnosticCode::Syntax);
    assert_eq!(
        detail_string(only_diagnostic(&malformed), "feature"),
        Some("yaml_syntax")
    );

    let root_sequence = assert_invalid(
        "- format_version\n- 1\n",
        SchemaDiagnosticCode::InvalidDeclaration,
    );
    assert_eq!(
        only_diagnostic(&root_sequence).context().field(),
        Some("root")
    );
}

#[test]
fn format_version_requires_unsigned_base_ten_integer_source() {
    for authored in ["+1", "0x1", "1.0", "\"1\""] {
        let source =
            VALID_SCHEMA.replace("format_version: 1", &format!("format_version: {authored}"));
        assert_invalid(source, SchemaDiagnosticCode::InvalidDeclaration);
    }

    let oversized = "9".repeat(80);
    let source = VALID_SCHEMA.replace("format_version: 1", &format!("format_version: {oversized}"));
    let error = assert_invalid(source, SchemaDiagnosticCode::UnsupportedFormat);
    assert_eq!(
        detail_string(only_diagnostic(&error), "format_version"),
        Some(oversized.as_str())
    );
}

#[test]
fn rejects_wrong_types_missing_keys_and_collection_shapes() {
    let cases = [
        VALID_SCHEMA.replace("format_version: 1", "format_version: \"1\""),
        VALID_SCHEMA.replace("  name: mara-schema\n", ""),
        VALID_SCHEMA.replace(
            concat!(
                "flavours:\n",
                "  requirement:\n",
                "    label: Requirement\n",
                "    description: A verifiable obligation.\n",
                "    guidance:\n",
                "      use_when: [Document an externally visible obligation.]\n",
                "      avoid_when: [The content explains an implementation choice.]\n",
                "    id: {}\n",
                "    title: {}\n",
                "    body: {}",
            ),
            "flavours: []",
        ),
        VALID_SCHEMA.replace("relations: {}", "relations: []"),
        VALID_SCHEMA.replace("rules: []", "rules: {}"),
    ];

    for source in cases {
        assert_invalid(source, SchemaDiagnosticCode::InvalidDeclaration);
    }
}

#[test]
fn reports_byte_offsets_and_unicode_columns_for_crlf_source() {
    let source = "# é\r\nformat_version: 1\r\nschema:\r\n  name: crlf-schema\r\n  version: 1.0.0\r\nidentity:\r\n  mid:\r\n    format: ulid\r\n    prefix: crlf_\r\nflavours: {}\r\n";
    let fixture = Fixture::new(source);
    let document = load_schema(&fixture.loaded_project()).unwrap();
    let prefix = document.identity().value().mid().value().prefix();

    assert_eq!(prefix.key_source().start_line(), 9);
    assert_eq!(prefix.key_source().start_column(), 5);
    assert_eq!(
        prefix.key_source().start_byte() as usize,
        source.find("prefix:").unwrap()
    );
    assert_eq!(prefix.value_source().start_column(), 13);
    assert_eq!(source_slice(source, prefix.value_source()), "crlf_");
    assert_eq!(document.source().end_line(), 11);
    assert_eq!(document.source().end_column(), 1);
}

#[test]
fn parser_library_details_do_not_escape_the_public_schema_result() {
    fn assert_mara_owned(_: &mara_core::SchemaDocument) {}

    let fixture = Fixture::new(VALID_SCHEMA);
    let document = load_schema(&fixture.loaded_project()).unwrap();
    assert_mara_owned(&document);
    assert!(Path::new(document.source().path()).is_relative());
}

#[cfg(unix)]
#[test]
fn preserves_the_logical_source_path_for_internal_schema_symlinks() {
    use std::os::unix::fs::symlink;

    for (target_relative, symlink_target) in [
        (".mara/target\\schema.yaml", "target\\schema.yaml"),
        ("C:schema.yaml", "../C:schema.yaml"),
        ("https:schema.yaml", "../https:schema.yaml"),
    ] {
        let fixture = Fixture::new(VALID_SCHEMA);
        fs::remove_file(fixture.schema_path()).unwrap();
        fs::write(fixture.root.join(target_relative), VALID_SCHEMA).unwrap();
        symlink(symlink_target, fixture.schema_path()).unwrap();
        let project = fixture.loaded_project();

        let document = load_schema(&project).unwrap();
        let prefix = document.identity().value().mid().value().prefix();

        assert_eq!(project.schema_source_path, ".mara/schema.yaml");
        assert_eq!(document.source().path(), ".mara/schema.yaml");
        assert_eq!(prefix.value_source().path(), ".mara/schema.yaml");
    }
}
