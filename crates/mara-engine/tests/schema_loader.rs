use std::{fs, path::Path};

use mara_core::{Diagnostic, DiagnosticCode, DiagnosticValue, MidFormat, SchemaDiagnosticCode};
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
relations: {}
rules: []
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
        "requirement:\n    label: Requirement\n"
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
fn rejects_multiple_documents_at_the_second_document_marker() {
    let source = format!("{VALID_SCHEMA}---\n{VALID_SCHEMA}");
    let error = assert_invalid(source, SchemaDiagnosticCode::Syntax);
    let diagnostic = only_diagnostic(&error);

    assert_eq!(
        detail_string(diagnostic, "feature"),
        Some("multiple_documents")
    );
    assert_eq!(diagnostic.primary().unwrap().start_line(), 15);
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
        ("%TAG !e! tag:example.com,2026:\n---\n", "custom_tag"),
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
    let fixture = Fixture::new(source);
    let error = load_schema(&fixture.loaded_project()).unwrap_err();

    let features = error
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| detail_string(diagnostic, "feature"))
        .collect::<Vec<_>>();
    assert_eq!(features, ["anchor", "alias"]);
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
        "  !!str version: !!str # !\n    1.0.0",
    );
    let fixture = Fixture::new(&commented_tag_source);
    let document = load_schema(&fixture.loaded_project()).unwrap();
    assert_eq!(
        source_slice(
            &commented_tag_source,
            document.schema().value().version().value_source()
        ),
        "!!str # !\n    1.0.0"
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
    let cases = [
        ("root", format!("{VALID_SCHEMA}imports: other.yaml\n"), 15),
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
            "flavours:\n  requirement:\n    label: Requirement",
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
fn rejects_canonical_schema_paths_that_are_not_wire_safe_without_panicking() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new(VALID_SCHEMA);
    fs::remove_file(fixture.schema_path()).unwrap();
    let target = fixture.root.join(".mara/target\\schema.yaml");
    fs::write(&target, VALID_SCHEMA).unwrap();
    symlink("target\\schema.yaml", fixture.schema_path()).unwrap();
    let project = fixture.loaded_project();

    let error = load_schema(&project).unwrap_err();

    assert_code(only_diagnostic(&error), SchemaDiagnosticCode::Io);
    assert_eq!(error.path(), Some(project.schema_path.as_path()));
    assert!(error.io_source().is_some());
}
