use std::{fs, path::Path};

use mara_core::{
    Diagnostic, DiagnosticCode, DiagnosticValue, FieldType, Mid, MidFormat, SchemaDiagnosticCode,
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

fn detail_string<'a>(diagnostic: &'a Diagnostic, key: &str) -> Option<&'a str> {
    match diagnostic.details().get(key) {
        Some(DiagnosticValue::String(value)) => Some(value),
        _ => None,
    }
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
fn validates_and_drops_deep_profile_values_iteratively() {
    const DEPTH: usize = 20_000;
    let nested = format!("{}leaf", "- ".repeat(DEPTH));
    let source = format!(
        "format_version: 1\nschema:\n  name: deep-schema\n  version: 1.0.0\nidentity:\n  mid:\n    format: ulid\n    prefix: deep_\nflavours: {{}}\nrelations:\n  deep:\n    {nested}\n"
    );

    let fixture = Fixture::new(source);
    let document = load_schema(&fixture.loaded_project()).unwrap();

    assert_eq!(document.relations().unwrap().len(), 1);
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
