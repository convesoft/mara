use std::{fs, path::Path};

use mara_engine::project::{
    ProjectLoadError, ProjectLoadErrorCode, ProjectLoadOperationalErrorCode, discover_and_load,
    discover_project, load_from_root,
};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    root: std::path::PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("create isolated fixture");
        let root = temp.path().join("project");
        fs::create_dir_all(root.join(".mara")).unwrap();
        let root = root.canonicalize().unwrap();
        fs::write(root.join(".mara/schema.yaml"), "format_version: 1\n").unwrap();
        fs::write(root.join(".mara/project.toml"), valid_config()).unwrap();
        Self { _temp: temp, root }
    }

    fn config_path(&self) -> std::path::PathBuf {
        self.root.join(".mara/project.toml")
    }

    fn write_config(&self, source: impl AsRef<[u8]>) {
        fs::write(self.config_path(), source).unwrap();
    }
}

fn valid_config() -> String {
    config_with(
        ".mara/schema.yaml",
        ".mara/index.json",
        &["**/*.mara.md"],
        &[],
    )
}

fn config_with(schema: &str, index: &str, include: &[&str], exclude: &[&str]) -> String {
    format!(
        r#"format_version = 1
[project]
name = "mara-test"
schema = {schema:?}
[content]
include = {include:?}
exclude = {exclude:?}
respect_gitignore = true
follow_directory_symlinks = false
allow_internal_file_symlinks = true
[index]
path = {index:?}
[validation]
warnings_as_errors = false
[git]
require_clean_worktree_for_writes = true
"#
    )
}

#[cfg(unix)]
fn symlink_file(original: impl AsRef<Path>, link: impl AsRef<Path>) {
    std::os::unix::fs::symlink(original, link).unwrap();
}

#[cfg(windows)]
fn symlink_file(original: impl AsRef<Path>, link: impl AsRef<Path>) {
    std::os::windows::fs::symlink_file(original, link).unwrap();
}

#[cfg(unix)]
fn symlink_directory(original: impl AsRef<Path>, link: impl AsRef<Path>) {
    std::os::unix::fs::symlink(original, link).unwrap();
}

#[cfg(windows)]
fn symlink_directory(original: impl AsRef<Path>, link: impl AsRef<Path>) {
    std::os::windows::fs::symlink_dir(original, link).unwrap();
}

fn assert_invalid_field(error: ProjectLoadError, expected_field: &str) {
    assert_eq!(
        error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ConfigInvalidValue)
    );
    match error {
        ProjectLoadError::InvalidConfiguration {
            field, location, ..
        } => {
            assert_eq!(field, Some(expected_field));
            assert!(location.is_some(), "semantic errors retain source location");
        }
        other => panic!("expected invalid {expected_field}, got {other}"),
    }
}

fn assert_unsafe_field(error: ProjectLoadError, expected_field: &str) {
    match error {
        ProjectLoadError::UnsafePath {
            field, location, ..
        } => {
            assert_eq!(field, expected_field);
            assert!(location.is_some(), "path errors retain source location");
        }
        other => panic!("expected unsafe {expected_field}, got {other}"),
    }
}

#[test]
fn loads_the_complete_normative_v1_shape() {
    let fixture = Fixture::new();

    let project = load_from_root(&fixture.root).unwrap();

    assert_eq!(project.root, fixture.root.canonicalize().unwrap());
    assert_eq!(project.config_path, fixture.config_path());
    assert_eq!(project.format_version, 1);
    assert_eq!(project.name, "mara-test");
    assert_eq!(project.schema_source_path, ".mara/schema.yaml");
    assert_eq!(
        project.schema_path,
        fixture
            .root
            .join(".mara/schema.yaml")
            .canonicalize()
            .unwrap()
    );
    assert_eq!(project.index_path, fixture.root.join(".mara/index.json"));
    assert_eq!(project.content.include, ["**/*.mara.md"]);
    assert!(project.content.exclude.is_empty());
    assert!(project.content.respect_gitignore);
    assert!(!project.content.follow_directory_symlinks);
    assert!(project.content.allow_internal_file_symlinks);
    assert!(!project.validation.warnings_as_errors);
    assert!(project.git.require_clean_worktree_for_writes);
}

#[test]
fn discovery_selects_the_nearest_root_from_a_directory_or_file() {
    let outer = Fixture::new();
    let inner_root = outer.root.join("nested");
    fs::create_dir_all(inner_root.join(".mara")).unwrap();
    fs::write(inner_root.join(".mara/schema.yaml"), "format_version: 1\n").unwrap();
    fs::write(inner_root.join(".mara/project.toml"), valid_config()).unwrap();
    let deep = inner_root.join("deep/path");
    fs::create_dir_all(&deep).unwrap();
    let file = deep.join("note.md");
    fs::write(&file, "note").unwrap();

    let from_directory = discover_project(&deep).unwrap();
    let from_file = discover_project(&file).unwrap();

    assert_eq!(from_directory.root, inner_root.canonicalize().unwrap());
    assert_eq!(from_file, from_directory);
}

#[test]
fn a_malformed_nearest_configuration_does_not_fall_back_to_an_outer_project() {
    let outer = Fixture::new();
    let inner_root = outer.root.join("nested");
    fs::create_dir_all(inner_root.join(".mara/deep")).unwrap();
    fs::write(inner_root.join(".mara/schema.yaml"), "format_version: 1\n").unwrap();
    let inner_config = valid_config().replace(
        "name = \"mara-test\"",
        "name = \"mara-test\"\nunknown = true",
    );
    fs::write(inner_root.join(".mara/project.toml"), inner_config).unwrap();

    let error = discover_and_load(inner_root.join(".mara/deep")).unwrap_err();
    assert_eq!(
        error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ConfigUnknownKey)
    );

    match error {
        ProjectLoadError::InvalidConfiguration { path, .. } => {
            assert_eq!(path, inner_root.join(".mara/project.toml"));
        }
        other => panic!("expected inner configuration failure, got {other}"),
    }
}

#[test]
fn reports_when_no_project_marker_exists() {
    let temp = tempfile::tempdir().unwrap();
    let error = discover_project(temp.path()).unwrap_err();
    assert_eq!(
        error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ProjectNotFound)
    );
    assert_eq!(error.class().as_str(), "project.not_found");
    assert!(matches!(error, ProjectLoadError::ProjectNotFound { .. }));
}

#[test]
fn rejects_unknown_keys_in_root_and_nested_tables() {
    for source in [
        valid_config().replace("format_version = 1", "format_version = 1\nunknown = true"),
        valid_config().replace(
            "name = \"mara-test\"",
            "name = \"mara-test\"\nunknown = true",
        ),
    ] {
        let fixture = Fixture::new();
        fixture.write_config(source);
        let error = load_from_root(&fixture.root).unwrap_err();
        assert_eq!(
            error.diagnostic_code(),
            Some(ProjectLoadErrorCode::ConfigUnknownKey)
        );
        match error {
            ProjectLoadError::InvalidConfiguration {
                field: None,
                location: Some(location),
                message,
                ..
            } => {
                assert!(location.line > 0);
                assert!(message.contains("unknown field"));
            }
            other => panic!("expected source-aware unknown-field error, got {other}"),
        }
    }
}

#[test]
fn rejects_duplicate_assignments_and_malformed_types_with_locations() {
    let cases = [
        (
            valid_config().replace(
                "format_version = 1",
                "format_version = 1\nformat_version = 1",
            ),
            ProjectLoadErrorCode::ConfigDuplicateKey,
        ),
        (
            valid_config().replace(
                "warnings_as_errors = false",
                "warnings_as_errors = \"false\"",
            ),
            ProjectLoadErrorCode::ConfigInvalidValue,
        ),
    ];
    for (source, expected_code) in cases {
        let fixture = Fixture::new();
        fixture.write_config(source);
        let error = load_from_root(&fixture.root).unwrap_err();
        assert_eq!(error.diagnostic_code(), Some(expected_code));
        match error {
            ProjectLoadError::InvalidConfiguration {
                location: Some(location),
                ..
            } => assert!(location.byte_offset > 0),
            other => panic!("expected located TOML error, got {other}"),
        }
    }
}

#[test]
fn rejects_malformed_toml_with_the_catalogue_syntax_code() {
    let fixture = Fixture::new();
    fixture.write_config(valid_config().replace("name = \"mara-test\"", "name = \"unterminated"));

    let error = load_from_root(&fixture.root).unwrap_err();

    assert_eq!(
        error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ConfigSyntax)
    );
    assert_eq!(error.class().as_str(), "config.syntax");
    assert!(matches!(
        error,
        ProjectLoadError::InvalidConfiguration {
            location: Some(_),
            ..
        }
    ));
}

#[test]
fn rejects_missing_required_fields_and_tables() {
    for source in [
        valid_config().replace("name = \"mara-test\"\n", ""),
        valid_config().replace("[git]\nrequire_clean_worktree_for_writes = true\n", ""),
    ] {
        let fixture = Fixture::new();
        fixture.write_config(source);
        let error = load_from_root(&fixture.root).unwrap_err();
        assert_eq!(
            error.diagnostic_code(),
            Some(ProjectLoadErrorCode::ConfigInvalidValue)
        );
        assert!(matches!(
            error,
            ProjectLoadError::InvalidConfiguration { .. }
        ));
    }
}

#[test]
fn rejects_unsupported_versions_and_invalid_project_names() {
    let fixture = Fixture::new();
    fixture.write_config(valid_config().replace("format_version = 1", "format_version = 2"));
    assert_invalid_field(load_from_root(&fixture.root).unwrap_err(), "format_version");

    let future = Fixture::new();
    future.write_config("format_version = 2\n[future]\nnew_shape = true\n");
    let future_error = load_from_root(&future.root).unwrap_err();
    assert!(
        future_error
            .to_string()
            .contains("unsupported format version 2")
    );
    assert_invalid_field(future_error, "format_version");

    for name in ["", "Mara", "1mara", "mara_kit", "mara--kit", "mara-"] {
        let fixture = Fixture::new();
        fixture.write_config(valid_config().replace("mara-test", name));
        assert_invalid_field(load_from_root(&fixture.root).unwrap_err(), "project.name");
    }
}

#[test]
fn rejects_missing_and_non_integer_format_versions_deterministically() {
    let missing = Fixture::new();
    missing.write_config(valid_config().replace("format_version = 1\n", ""));
    let first_missing = load_from_root(&missing.root).unwrap_err();
    let second_missing = load_from_root(&missing.root).unwrap_err();
    assert_eq!(
        first_missing.diagnostic_code(),
        Some(ProjectLoadErrorCode::ConfigInvalidValue)
    );
    assert_eq!(first_missing.to_string(), second_missing.to_string());
    assert!(first_missing.to_string().contains("missing field"));

    for replacement in [
        "format_version = \"1\"",
        "format_version = 1.0",
        "format_version = true",
    ] {
        let fixture = Fixture::new();
        fixture.write_config(valid_config().replace("format_version = 1", replacement));
        let error = load_from_root(&fixture.root).unwrap_err();
        assert_eq!(
            error.diagnostic_code(),
            Some(ProjectLoadErrorCode::ConfigInvalidValue)
        );
        assert!(matches!(
            error,
            ProjectLoadError::InvalidConfiguration {
                location: Some(_),
                ..
            }
        ));
    }
}

#[test]
fn rejects_utf8_bom_and_invalid_utf8() {
    let fixture = Fixture::new();
    let mut bom = vec![0xef, 0xbb, 0xbf];
    bom.extend(valid_config().into_bytes());
    fixture.write_config(bom);
    let bom_error = load_from_root(&fixture.root).unwrap_err();
    assert_eq!(
        bom_error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ConfigSyntax)
    );
    assert!(bom_error.to_string().contains("byte-order mark"));

    fixture.write_config([0xff, 0xfe, 0xfd]);
    let utf8_error = load_from_root(&fixture.root).unwrap_err();
    assert_eq!(
        utf8_error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ConfigSyntax)
    );
    assert!(utf8_error.to_string().contains("not valid UTF-8"));
}

#[test]
fn validates_every_supported_glob_form_without_discovering_content() {
    let fixture = Fixture::new();
    let globs = [
        "*",
        "?",
        "[abc]",
        "[a-z]",
        "[!abc]",
        "**",
        "docs/**/[?].mara.md",
        ".hidden/*.mara.md",
    ];
    fixture.write_config(config_with(
        ".mara/schema.yaml",
        ".mara/index.json",
        &globs,
        &[],
    ));

    let project = load_from_root(&fixture.root).unwrap();

    assert_eq!(project.content.include, globs);
}

#[test]
fn rejects_empty_duplicate_and_unsupported_globs() {
    let fixture = Fixture::new();
    fixture.write_config(config_with(
        ".mara/schema.yaml",
        ".mara/index.json",
        &[],
        &[],
    ));
    assert_invalid_field(
        load_from_root(&fixture.root).unwrap_err(),
        "content.include",
    );

    for patterns in [
        vec!["same", "same"],
        vec![""],
        vec!["docs/{one,two}"],
        vec![r"docs\*.md"],
        vec!["docs//*.md"],
        vec!["./docs/*.md"],
        vec!["../*.md"],
        vec!["docs/../*.md"],
        vec!["docs/**suffix.md"],
        vec!["docs/[abc.md"],
        vec!["docs/[].md"],
        vec!["docs/[z-a].md"],
        vec!["docs/a].md"],
    ] {
        let fixture = Fixture::new();
        fixture.write_config(config_with(
            ".mara/schema.yaml",
            ".mara/index.json",
            &patterns,
            &[],
        ));
        assert_invalid_field(
            load_from_root(&fixture.root).unwrap_err(),
            "content.include",
        );
    }

    let fixture = Fixture::new();
    fixture.write_config(config_with(
        ".mara/schema.yaml",
        ".mara/index.json",
        &["**/*.mara.md"],
        &["same", "same"],
    ));
    assert_invalid_field(
        load_from_root(&fixture.root).unwrap_err(),
        "content.exclude",
    );
}

#[test]
fn rejects_every_forbidden_path_form_before_filesystem_lookup() {
    let absolute = if cfg!(windows) {
        "C:/outside/schema.yaml"
    } else {
        "/outside/schema.yaml"
    };
    for configured in [
        "",
        absolute,
        r".mara\schema.yaml",
        ".mara//schema.yaml",
        ".mara/./schema.yaml",
        ".mara/../schema.yaml",
        "https://example.test/schema.yaml",
        "C:schema.yaml",
    ] {
        let fixture = Fixture::new();
        fixture.write_config(config_with(
            configured,
            ".mara/index.json",
            &["**/*.mara.md"],
            &[],
        ));
        assert_unsafe_field(load_from_root(&fixture.root).unwrap_err(), "project.schema");
    }

    let nul = Fixture::new();
    nul.write_config(
        valid_config().replace("schema = \".mara/schema.yaml\"", "schema = \"\\u0000\""),
    );
    assert!(matches!(
        load_from_root(&nul.root),
        Err(ProjectLoadError::InvalidConfiguration { .. })
            | Err(ProjectLoadError::UnsafePath { .. })
    ));
}

#[test]
fn environment_expressions_are_literal_and_are_never_expanded() {
    let fixture = Fixture::new();
    fixture.write_config(config_with(
        ".mara/schema.yaml",
        "$HOME/index.json",
        &["**/*.mara.md"],
        &[],
    ));

    let project = load_from_root(&fixture.root).unwrap();

    assert_eq!(project.index_path, fixture.root.join("$HOME/index.json"));
}

#[test]
fn schema_must_be_an_existing_readable_regular_file() {
    let missing = Fixture::new();
    missing.write_config(config_with(
        ".mara/missing.yaml",
        ".mara/index.json",
        &["**/*.mara.md"],
        &[],
    ));
    assert_invalid_field(load_from_root(&missing.root).unwrap_err(), "project.schema");

    let directory = Fixture::new();
    fs::create_dir(directory.root.join("schema-dir")).unwrap();
    directory.write_config(config_with(
        "schema-dir",
        ".mara/index.json",
        &["**/*.mara.md"],
        &[],
    ));
    assert_unsafe_field(
        load_from_root(&directory.root).unwrap_err(),
        "project.schema",
    );
}

#[test]
fn an_existing_index_destination_must_be_a_regular_file() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.root.join("index-dir")).unwrap();
    fixture.write_config(config_with(
        ".mara/schema.yaml",
        "index-dir",
        &["**/*.mara.md"],
        &[],
    ));

    assert_unsafe_field(load_from_root(&fixture.root).unwrap_err(), "index.path");
}

#[cfg(unix)]
#[test]
fn an_existing_write_only_index_does_not_require_read_access() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new();
    let index_path = fixture.root.join(".mara/index.json");
    fs::write(&index_path, "derived").unwrap();
    fs::set_permissions(&index_path, fs::Permissions::from_mode(0o200)).unwrap();

    let project = load_from_root(&fixture.root).unwrap();

    assert_eq!(project.index_path, index_path);
}

#[cfg(unix)]
#[test]
fn rejects_non_writable_existing_index_and_missing_index_parent() {
    use std::os::unix::fs::PermissionsExt;

    let existing = Fixture::new();
    let existing_index = existing.root.join(".mara/index.json");
    fs::write(&existing_index, "derived").unwrap();
    fs::set_permissions(&existing_index, fs::Permissions::from_mode(0o400)).unwrap();
    let existing_error = load_from_root(&existing.root).unwrap_err();
    assert_eq!(
        existing_error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ConfigInvalidValue)
    );
    assert!(existing_error.to_string().contains("not writable"));

    let missing = Fixture::new();
    let output_parent = missing.root.join("generated");
    fs::create_dir(&output_parent).unwrap();
    missing.write_config(config_with(
        ".mara/schema.yaml",
        "generated/index.json",
        &["**/*.mara.md"],
        &[],
    ));
    fs::set_permissions(&output_parent, fs::Permissions::from_mode(0o500)).unwrap();
    let missing_error = load_from_root(&missing.root).unwrap_err();
    fs::set_permissions(&output_parent, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        missing_error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ConfigInvalidValue)
    );
    assert!(missing_error.to_string().contains("not writable"));
}

#[cfg(windows)]
#[test]
fn rejects_a_read_only_existing_index_on_windows() {
    let fixture = Fixture::new();
    let index_path = fixture.root.join(".mara/index.json");
    fs::write(&index_path, "derived").unwrap();
    let mut permissions = fs::metadata(&index_path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&index_path, permissions).unwrap();

    let error = load_from_root(&fixture.root).unwrap_err();

    assert_eq!(
        error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ConfigInvalidValue)
    );
}

#[cfg(any(unix, windows))]
#[test]
fn rejects_a_dangling_output_symlink_with_source_context() {
    let fixture = Fixture::new();
    symlink_file(
        fixture.root.join(".mara/missing-index-target"),
        fixture.root.join(".mara/index.json"),
    );

    let error = load_from_root(&fixture.root).unwrap_err();

    assert_eq!(
        error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ProjectSymlinkRejected)
    );
    match error {
        ProjectLoadError::UnsafePath {
            field,
            location: Some(location),
            ..
        } => {
            assert_eq!(field, "index.path");
            assert_eq!(location.line, 12);
        }
        other => panic!("expected source-aware dangling symlink error, got {other}"),
    }

    let intermediate = Fixture::new();
    symlink_directory(
        intermediate.root.join("missing-output-target"),
        intermediate.root.join("output"),
    );
    intermediate.write_config(config_with(
        ".mara/schema.yaml",
        "output/generated/index.json",
        &["**/*.mara.md"],
        &[],
    ));

    let intermediate_error = load_from_root(&intermediate.root).unwrap_err();

    assert_eq!(
        intermediate_error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ProjectSymlinkRejected)
    );
    assert!(matches!(
        intermediate_error,
        ProjectLoadError::UnsafePath {
            field: "index.path",
            location: Some(_),
            ..
        }
    ));
}

#[cfg(any(unix, windows))]
#[test]
fn rejects_schema_and_index_paths_that_escape_through_symlinks() {
    let outside = tempfile::tempdir().unwrap();
    let outside_schema = outside.path().join("schema.yaml");
    fs::write(&outside_schema, "format_version: 1\n").unwrap();

    let schema_fixture = Fixture::new();
    fs::remove_file(schema_fixture.root.join(".mara/schema.yaml")).unwrap();
    symlink_file(
        &outside_schema,
        schema_fixture.root.join(".mara/schema.yaml"),
    );
    assert_unsafe_field(
        load_from_root(&schema_fixture.root).unwrap_err(),
        "project.schema",
    );

    let index_fixture = Fixture::new();
    symlink_directory(outside.path(), index_fixture.root.join("external"));
    index_fixture.write_config(config_with(
        ".mara/schema.yaml",
        "external/index.json",
        &["**/*.mara.md"],
        &[],
    ));
    assert_unsafe_field(
        load_from_root(&index_fixture.root).unwrap_err(),
        "index.path",
    );
}

#[cfg(any(unix, windows))]
#[test]
fn normalizes_internal_symlinks_and_rejects_index_aliases_to_inputs() {
    let schema_fixture = Fixture::new();
    let real_schema = schema_fixture.root.join(".mara/real-schema.yaml");
    fs::rename(schema_fixture.root.join(".mara/schema.yaml"), &real_schema).unwrap();
    symlink_file(&real_schema, schema_fixture.root.join(".mara/schema.yaml"));
    let loaded = load_from_root(&schema_fixture.root).unwrap();
    assert_eq!(loaded.schema_source_path, ".mara/schema.yaml");
    assert_eq!(loaded.schema_path, real_schema.canonicalize().unwrap());

    let alias_fixture = Fixture::new();
    symlink_file(
        alias_fixture.root.join(".mara/schema.yaml"),
        alias_fixture.root.join(".mara/index-alias"),
    );
    alias_fixture.write_config(config_with(
        ".mara/schema.yaml",
        ".mara/index-alias",
        &["**/*.mara.md"],
        &[],
    ));
    assert_unsafe_field(
        load_from_root(&alias_fixture.root).unwrap_err(),
        "index.path",
    );
}

#[test]
fn rejects_hard_linked_index_aliases_to_configuration_or_schema() {
    let config_alias = Fixture::new();
    let config_index = config_alias.root.join(".mara/config-index-alias");
    fs::hard_link(config_alias.config_path(), &config_index).unwrap();
    config_alias.write_config(config_with(
        ".mara/schema.yaml",
        ".mara/config-index-alias",
        &["**/*.mara.md"],
        &[],
    ));
    let config_alias_error = load_from_root(&config_alias.root).unwrap_err();
    assert_eq!(
        config_alias_error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ProjectDuplicateFile)
    );
    assert_unsafe_field(config_alias_error, "index.path");

    let schema_alias = Fixture::new();
    fs::hard_link(
        schema_alias.root.join(".mara/schema.yaml"),
        schema_alias.root.join(".mara/schema-index-alias"),
    )
    .unwrap();
    schema_alias.write_config(config_with(
        ".mara/schema.yaml",
        ".mara/schema-index-alias",
        &["**/*.mara.md"],
        &[],
    ));
    assert_unsafe_field(
        load_from_root(&schema_alias.root).unwrap_err(),
        "index.path",
    );
}

#[cfg(any(unix, windows))]
#[test]
fn normalizes_a_missing_output_beneath_an_internal_symlinked_directory() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.root.join("real-output")).unwrap();
    symlink_directory(
        fixture.root.join("real-output"),
        fixture.root.join("output"),
    );
    fixture.write_config(config_with(
        ".mara/schema.yaml",
        "output/generated/index.json",
        &["**/*.mara.md"],
        &[],
    ));

    let loaded = load_from_root(&fixture.root).unwrap();

    assert_eq!(
        loaded.index_path,
        fixture.root.join("real-output/generated/index.json")
    );
}

#[cfg(any(unix, windows))]
#[test]
fn rejects_a_project_configuration_marker_that_resolves_outside_the_root() {
    let fixture = Fixture::new();
    let outside = tempfile::tempdir().unwrap();
    let outside_config = outside.path().join("project.toml");
    fs::write(&outside_config, valid_config()).unwrap();
    fs::remove_file(fixture.config_path()).unwrap();
    symlink_file(&outside_config, fixture.config_path());

    let error = load_from_root(&fixture.root).unwrap_err();
    assert_eq!(
        error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ProjectPathOutsideRoot)
    );

    match error {
        ProjectLoadError::UnsafePath {
            field,
            location: None,
            ..
        } => assert_eq!(field, "project configuration"),
        other => panic!("expected escaped configuration error, got {other}"),
    }
}

#[test]
fn diagnostics_are_deterministic_and_actionable() {
    let fixture = Fixture::new();
    fixture.write_config(config_with(
        "../schema.yaml",
        ".mara/index.json",
        &["**/*.mara.md"],
        &[],
    ));

    let first_error = load_from_root(&fixture.root).unwrap_err();
    let second_error = load_from_root(&fixture.root).unwrap_err();
    assert_eq!(
        first_error.diagnostic_code(),
        Some(ProjectLoadErrorCode::ProjectPathOutsideRoot)
    );
    assert_eq!(first_error.class().as_str(), "project.path_outside_root");
    let first = first_error.to_string();
    let second = second_error.to_string();

    assert_eq!(first, second);
    assert!(first.contains("project.schema"));
    assert!(first.contains("../schema.yaml"));
    assert!(first.contains(&fixture.config_path().display().to_string()));
    match load_from_root(&fixture.root).unwrap_err() {
        ProjectLoadError::UnsafePath {
            location: Some(location),
            ..
        } => {
            assert_eq!(location.line, 4);
            assert_eq!(location.column, 10);
        }
        other => panic!("expected located path diagnostic, got {other}"),
    }
}

#[test]
fn an_explicit_root_must_be_a_directory() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("file");
    fs::write(&file, "not a root").unwrap();
    let error = load_from_root(&file).unwrap_err();
    assert_eq!(error.diagnostic_code(), None);
    assert_eq!(
        error.operational_code(),
        Some(ProjectLoadOperationalErrorCode::ProjectUnavailable)
    );
    assert_eq!(error.class().as_str(), "project.unavailable");
    assert!(matches!(
        error,
        ProjectLoadError::InvalidConfiguration { .. }
    ));
}

#[test]
fn config_path_is_independent_of_the_callers_current_directory() {
    let fixture = Fixture::new();
    let relative_spelling = fixture.root.join("nested/../nested/deep");
    fs::create_dir_all(fixture.root.join("nested/deep")).unwrap();

    let project = discover_and_load(relative_spelling).unwrap();

    assert_eq!(project.root, fixture.root.canonicalize().unwrap());
    assert_eq!(project.config_path, fixture.config_path());
    assert!(Path::new(&project.schema_path).is_absolute());
    assert!(Path::new(&project.index_path).is_absolute());
}
