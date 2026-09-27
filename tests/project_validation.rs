use serde_json::{Value, json};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{fs, path::Path};
use tempfile::TempDir;
mod support;
use support::*;

fn diagnostic_parity(root: &Path, args: &[&str], tool: &str, params: Value) -> Value {
    let mut cli_args = vec!["--format", "json"];
    cli_args.extend_from_slice(args);
    let cli = mara(root, &cli_args);
    let value: Value = serde_json::from_slice(&cli.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", stderr(&cli)));
    let replies = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, tool, params),
        ],
    );
    let mcp = &mcp_response(&replies, 2)["result"];
    assert_eq!(value, mcp["structuredContent"]);
    assert_eq!(mcp["isError"], value.get("error").is_some());
    assert_eq!(cli.status.success(), value["valid"] == true);
    assert_eq!(value["format_version"], 1);
    value
}

fn validation_with_parity(current_directory: &Path, paths: &[&str]) -> Value {
    let mut args = vec!["--format", "json", "project", "validate"];
    for path in paths {
        args.extend(["--path", path]);
    }
    let output = mara(current_directory, &args);
    let result: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", stderr(&output)));
    assert_eq!(output.status.success(), result["valid"].as_bool().unwrap());
    let responses = mcp_exchange(
        current_directory,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "project_validate", json!({"paths":paths})),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        result
    );
    result
}

fn directory_validation_fixture() -> TempDir {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    for (path, content) in [
        (
            "packages/dicom-viewer/docs/viewer.mara.md",
            ":::mara requirement REQ-VIEWER\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: Viewer\n:derives_from: REQ-CORE\n\nUses [[REQ-CORE]].\n:::\n",
        ),
        (
            "packages/core/core.mara.md",
            ":::mara requirement REQ-CORE\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02\n:title: Core\n\nCore behavior.\n:::\n",
        ),
        (
            "packages/dicom-viewer-extra/extra.mara.md",
            ":::mara requirement REQ-EXTRA\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F03\n:title: Extra\n\nExtra behavior.\n:::\n",
        ),
    ] {
        let file = fixture.path().join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, content).unwrap();
    }
    fixture
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
// @mara implements VER-PROJECT-VALIDATION
#[test]
fn directory_validation_filters_reporting_with_full_project_status_and_context() {
    let fixture = directory_validation_fixture();
    let package = fixture.path().join("packages/dicom-viewer");
    let path = "packages/dicom-viewer/docs/viewer.mara.md";
    let valid = validation_with_parity(&package, &["packages/dicom-viewer/"]);
    assert_eq!(valid["valid"], true);
    assert_eq!(valid["diagnostics"], json!([]));
    assert_eq!(valid["selection"]["omitted_diagnostics"], 0);
    assert_eq!(valid["project"], json!(fixture.path()));

    // Keep the external target valid while breaking a local relation and mention.
    let viewer = fixture.path().join(path);
    let original = fs::read_to_string(&viewer).unwrap();
    fs::write(
        &viewer,
        original
            .replace(":derives_from: REQ-CORE", ":derives_from: REQ-MISSING")
            .replace(
                "Uses [[REQ-CORE]].",
                "Uses [[REQ-CORE]] and [[REQ-MISSING]].",
            ),
    )
    .unwrap();
    for path in [
        "packages/core/core.mara.md",
        "packages/dicom-viewer-extra/extra.mara.md",
    ] {
        let file = fixture.path().join(path);
        let content = fs::read_to_string(&file).unwrap();
        fs::write(
            file,
            content.replace("behavior.", "behavior with [[REQ-ABSENT]]."),
        )
        .unwrap();
    }
    let full = validation_with_parity(&package, &[]);
    assert!(full["selection"].is_null());
    assert_eq!(full["diagnostics"].as_array().unwrap().len(), 4);
    let expected: Vec<_> = full["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|diagnostic| diagnostic["path"] == path)
        .cloned()
        .collect();
    assert_eq!(expected.len(), 2);
    assert_eq!(expected[0]["line"], 4);
    assert_eq!(expected[1]["line"], 6);
    for paths in [
        vec!["packages/dicom-viewer"],
        vec!["./packages//dicom-viewer/./"],
        vec!["packages/dicom-viewer", "packages/dicom-viewer/docs"],
        vec![path],
    ] {
        let result = validation_with_parity(&package, &paths);
        assert_eq!(result["valid"], false);
        assert_eq!(result["diagnostics"], json!(expected));
        assert_eq!(result["selection"]["omitted_diagnostics"], 2);
        if paths.len() == 1 && paths[0] != path {
            assert_eq!(
                result["selection"]["paths"],
                json!(["packages/dicom-viewer"])
            );
        }
    }
    let combined = validation_with_parity(&package, &["packages/dicom-viewer", "packages/core"]);
    assert_eq!(combined["diagnostics"].as_array().unwrap().len(), 3);
    assert_eq!(combined["selection"]["omitted_diagnostics"], 1);

    // An empty displayed list cannot turn an invalid project into a valid one.
    fs::write(viewer, original).unwrap();
    for path in ["packages/dicom-viewer", "packages/absent"] {
        let result = validation_with_parity(&package, &[path]);
        assert_eq!(result["valid"], false);
        assert_eq!(result["diagnostics"], json!([]));
        assert_eq!(result["selection"]["omitted_diagnostics"], 2);
        let human = mara(&package, &["project", "validate", "--path", path]);
        assert!(!human.status.success());
        assert!(
            stderr(&human).contains("2 diagnostics outside the selection omitted"),
            "{}",
            stderr(&human)
        );
        assert!(stderr(&human).contains("validation failed with 2 diagnostics"));
    }

    // Identity uniqueness also needs documents outside the selected package.
    let core = fixture.path().join("packages/core/core.mara.md");
    let content = fs::read_to_string(&core).unwrap();
    fs::write(core, format!("{content}\n:::mara requirement REQ-VIEWER\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F04\n:title: Duplicate\n\nBody.\n:::\n")).unwrap();
    let result = validation_with_parity(&package, &["packages/dicom-viewer"]);
    assert_eq!(result["valid"], false);
    let diagnostics = result["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]["message"]
            .as_str()
            .unwrap()
            .contains("duplicate item ID 'REQ-VIEWER'")
    );
    assert_eq!(diagnostics[0]["path"], path);
    assert_eq!(diagnostics[0]["line"], 1);
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn directory_validation_preserves_configuration_and_incomplete_context_failures() {
    let fixture = directory_validation_fixture();
    let config = fixture.path().join(".mara/project.toml");
    let original_config = fs::read_to_string(&config).unwrap();
    fs::write(&config, format!("unexpected = true\n{original_config}")).unwrap();
    let schema = fixture.path().join(".mara/schema.yaml");
    let original_schema = fs::read_to_string(&schema).unwrap();
    fs::write(&schema, format!("unexpected: true\n{original_schema}")).unwrap();
    let result = validation_with_parity(fixture.path(), &["packages/absent"]);
    assert_eq!(result["valid"], false);
    assert_eq!(result["selection"]["omitted_diagnostics"], 0);
    let diagnostics = result["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0]["scope"], "project");
    assert_eq!(diagnostics[1]["scope"], "schema");
    assert_eq!(diagnostics[0]["path"], ".mara/project.toml");
    assert_eq!(diagnostics[1]["path"], ".mara/schema.yaml");

    fs::write(config, original_config).unwrap();
    fs::write(schema, original_schema).unwrap();
    fs::write(fixture.path().join("packages/core/core.mara.md"), [0xff]).unwrap();
    let result = validation_with_parity(fixture.path(), &["packages/dicom-viewer"]);
    assert_eq!(result["valid"], false);
    assert_eq!(result["diagnostics"], json!([]));
    assert_eq!(result["selection"]["omitted_diagnostics"], 1);
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn directory_validation_rejects_invalid_paths_on_both_surfaces() {
    let fixture = directory_validation_fixture();
    for path in ["", ".", "./", "packages/../core", "/packages/core"] {
        let output = mara(fixture.path(), &["project", "validate", "--path", path]);
        assert!(!output.status.success(), "{path}");
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "project_validate", json!({"paths":[path]})),
            ],
        );
        let response = &mcp_response(&responses, 2)["result"];
        assert_eq!(response["isError"], true, "{path}");
        assert!(
            response["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("path filter must be a project-relative path")
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_treats_mid_as_structural_metadata() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("valid.mara.md"),
        r#":::mara requirement REQ-VALID
:mid: 01JQZ4W7G5H8K2M3N6P9R0STVX
:title: Valid

A complete requirement.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(validate.status.success(), "{}", stderr(&validate));
    assert!(stdout(&validate).contains("valid project"));
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_reports_missing_malformed_duplicate_and_misplaced_mids() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement REQ-MISSING
:title: Missing MID

Body.
:::

:::mara requirement REQ-MALFORMED
:mid: not-a-mid
:title: Malformed MID

Body.
:::

:::mara requirement REQ-OVERFLOW
:mid: ZZZZZZZZZZZZZZZZZZZZZZZZZZ
:title: Overflow MID

Body.
:::

:::mara requirement REQ-DUPLICATE-ONE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Duplicate one

Body.
:::

:::mara requirement REQ-DUPLICATE-TWO
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Duplicate two

Body.
:::

:::mara requirement REQ-MISPLACED
:title: Misplaced MID
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "item 'REQ-MISSING' is missing its MID",
        "invalid item MID 'not-a-mid'",
        "invalid item MID 'ZZZZZZZZZZZZZZZZZZZZZZZZZZ'",
        "duplicate item MID '01ARZ3NDEKTSV4RRFFQ69G5F00'",
        "item 'REQ-MISPLACED' MID must immediately follow its opener",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_rejects_non_bijective_item_identities() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement REQ-SHARED-ID
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Shared ID one

Body.
:::

:::mara requirement REQ-SHARED-ID
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Shared ID two

Body.
:::

:::mara requirement REQ-SHARED-MID-ONE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: Shared MID one

Body.
:::

:::mara requirement REQ-SHARED-MID-TWO
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: Shared MID two

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert_eq!(
        errors.matches("duplicate item ID 'REQ-SHARED-ID'").count(),
        2,
        "{errors}"
    );
    assert_eq!(
        errors
            .matches("duplicate item MID '01ARZ3NDEKTSV4RRFFQ69G5F02'")
            .count(),
        2,
        "{errors}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_reports_the_duplicated_secondary_mid_entry() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement REQ-FIRST
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: First

Body.
:::

:::mara requirement REQ-SECOND
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Second

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["--format", "json", "project", "validate"]);

    assert!(!validate.status.success());
    assert!(stderr(&validate).is_empty(), "{}", stderr(&validate));
    let validate: Value = serde_json::from_str(&stdout(&validate)).unwrap();
    let duplicate_mids = validate["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|diagnostic| {
            diagnostic["message"] == "duplicate item MID '01ARZ3NDEKTSV4RRFFQ69G5F01'"
        })
        .collect::<Vec<_>>();
    assert_eq!(duplicate_mids.len(), 2, "{validate:#}");
    assert!(
        duplicate_mids
            .iter()
            .any(|diagnostic| diagnostic["line"] == 3),
        "{validate:#}"
    );
    assert!(
        duplicate_mids
            .iter()
            .any(|diagnostic| diagnostic["line"] == 10),
        "{validate:#}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_reports_all_independently_available_diagnostics() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:unknown: value
:derives_from: MISSING-RELATION

Mentions [[MISSING-MENTION]].
:::

:::mara mystery MYS-UNKNOWN
:title: Unknown

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "unknown metadata field 'unknown'",
        "references missing item 'MISSING-RELATION'",
        "mention references missing item 'MISSING-MENTION'",
        "unknown flavour 'mystery'",
        "validation failed with 7 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }

    let item = mara(fixture.path(), &["item", "validate", "WRONG-ID"]);
    assert!(!item.status.success());
    assert!(stderr(&item).contains("validation failed with 5 diagnostics"));
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn item_validation_reports_ambiguous_relation_and_mention_targets() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("source.mara.md"),
        r#":::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Source
:derives_from: REQ-TARGET

Mentions [[REQ-TARGET]].
:::
"#,
    )
    .unwrap();
    for name in ["first", "second"] {
        fs::write(
            fixture.path().join(format!("{name}.mara.md")),
            format!(":::mara requirement REQ-TARGET\n:title: {name}\n\nTarget body.\n:::\n"),
        )
        .unwrap();
    }

    let validate = mara(fixture.path(), &["item", "validate", "REQ-SOURCE"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "relation 'derives_from' references ambiguous item 'REQ-TARGET'",
        "mention references ambiguous item 'REQ-TARGET'",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn item_validation_retains_recovered_syntax_diagnostics() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("nested.mara.md"),
        r#":::mara requirement REQ-OUTER
:title: Outer

:::mara requirement REQ-INNER
:title: Inner

Inner body.
:::
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-INNER"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("nested.mara.md:4: error: items cannot nest"),
        "{errors}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn item_validation_rejects_nested_opener_after_an_early_outer_error() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("nested.mara.md"),
        r#":::mara requirement REQ-OUTER

:::mara requirement REQ-INNER
:title: Inner

Inner body.
:::
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-INNER"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("nested.mara.md:3: error: items cannot nest"),
        "{errors}"
    );
    assert!(
        !stdout(&validate).contains("valid item"),
        "{}",
        stdout(&validate)
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_reports_schema_and_independent_syntax_diagnostics() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace("id_prefix: REQ-", "id_prefix: REQ--"),
    )
    .unwrap();
    fs::write(
        fixture.path().join("broken.mara.md"),
        ":::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "flavour 'requirement' has invalid ID prefix 'REQ--'",
        "item opener must be ':::mara <flavour> <id>' with no other tokens",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn item_validation_reports_syntax_for_an_identifiable_malformed_item() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("broken.mara.md"),
        ":::mara requirement REQ-BROKEN\n\nBody without a title.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-BROKEN"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors
            .contains("broken.mara.md:1: error: item must have exactly one non-empty title entry"),
        "{errors}"
    );
    assert!(
        !errors.contains("item 'REQ-BROKEN' was not found"),
        "{errors}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn item_validation_associates_a_malformed_opener_with_its_id() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("broken.mara.md"),
        ":::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-BROKEN"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(errors.contains("item opener must be"), "{errors}");
    assert!(
        !errors.contains("item 'REQ-BROKEN' was not found"),
        "{errors}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_continues_after_invalid_utf8() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(fixture.path().join("first.mara.md"), [0xff]).unwrap();
    fs::write(
        fixture.path().join("second.mara.md"),
        ":::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "first.mara.md: error: could not read Mara document",
        "second.mara.md:1: error: item opener must be",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn item_validation_fails_when_an_included_document_is_unreadable() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("valid.mara.md"),
        ":::mara requirement REQ-VALID\n:title: Valid\n\nBody.\n:::\n",
    )
    .unwrap();
    fs::write(fixture.path().join("bad.mara.md"), [0xff]).unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-VALID"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("bad.mara.md: error: could not read Mara document"),
        "{errors}"
    );
    assert!(
        !stdout(&validate).contains("valid item"),
        "{}",
        stdout(&validate)
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn item_validation_associates_a_missing_close_with_the_outer_item() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("missing-close.mara.md"),
        r#":::mara requirement REQ-OUTER
:title: Outer

Outer body.

:::mara requirement REQ-INNER
:title: Inner

Inner body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-OUTER"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(errors.contains("items cannot nest"), "{errors}");
    assert!(
        !errors.contains("item 'REQ-OUTER' was not found"),
        "{errors}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_accumulates_independent_schema_diagnostics() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema
            .replace("id_prefix: REQ-", "id_prefix: REQ--")
            .replace("id_prefix: SCN-", "id_prefix: SCN--"),
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "flavour 'requirement' has invalid ID prefix 'REQ--'",
        "flavour 'scenario' has invalid ID prefix 'SCN--'",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_uses_unaffected_schema_declarations() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace("id_prefix: SCN-", "id_prefix: SCN--"),
    )
    .unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:unknown: value
:derives_from: MISSING-TARGET

:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "flavour 'scenario' has invalid ID prefix 'SCN--'",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "required body is empty",
        "unknown metadata field 'unknown'",
        "relation 'derives_from' references missing item 'MISSING-TARGET'",
        "validation failed with 6 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_runs_schema_independent_checks_after_schema_errors() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace("id_prefix: REQ-", "id_prefix: REQ--"),
    )
    .unwrap();
    fs::write(
        fixture.path().join("first.mara.md"),
        ":::mara requirement REQ-DUPLICATE\n:title: First\n\nMentions [[MISSING-MENTION]].\n:::\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("second.mara.md"),
        ":::mara requirement REQ-DUPLICATE\n:title: Second\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "flavour 'requirement' has invalid ID prefix 'REQ--'",
        "duplicate item ID 'REQ-DUPLICATE'",
        "mention references missing item 'MISSING-MENTION'",
        "validation failed with 6 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_accumulates_independent_configuration_errors() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join(".mara/project.toml"),
        r#"format_version = 1

[project]
name = ""
schema = ".mara/schema.yaml"

[content]
include = ["../**/*.mara.md"]
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "project.name must not be empty",
        "content.include entries must be project-relative patterns",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_retains_valid_include_entries_after_a_type_error() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project.replace(
            "include = [\"**/*.mara.md\"]",
            "include = [\"recovered.mara.md\", 42]",
        ),
    )
    .unwrap();
    fs::write(
        fixture.path().join("recovered.mara.md"),
        ":::mara requirement WRONG-ID\n:title: Recovered\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "invalid project configuration value 'content.include[1]'",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "validation failed with 3 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_retains_item_context_after_title_errors() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("first.mara.md"),
        ":::mara requirement REQ-DUPLICATE\n:title: First\n\nBody.\n:::\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("second.mara.md"),
        r#":::mara requirement REQ-DUPLICATE
:title: Second
:title: Duplicate title
:unknown: value
:derives_from: MISSING-TARGET

:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "item must have exactly one non-empty title entry",
        "duplicate item ID 'REQ-DUPLICATE'",
        "required body is empty",
        "unknown metadata field 'unknown'",
        "relation 'derives_from' references missing item 'MISSING-TARGET'",
        "validation failed with 8 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_uses_declarations_unaffected_by_schema_decode_errors() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace(
            "    body: required\n    fields: {}\n  requirement:",
            "    body: invalid\n    fields: {}\n  requirement:",
        ),
    )
    .unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:unknown: value
:derives_from: MISSING-TARGET

:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "flavour 'scenario' is invalid",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "required body is empty",
        "unknown metadata field 'unknown'",
        "relation 'derives_from' references missing item 'MISSING-TARGET'",
        "validation failed with 6 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_uses_properties_unaffected_by_a_flavour_decode_error() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join(".mara/schema.yaml"),
        r#"format_version: 3
flavours:
  requirement:
    description: An independently verifiable obligation.
    use_when: [Record project knowledge.]
    avoid_when: []
    distinguish_from: {}
    id_prefix: REQ-
    body: invalid
    fields:
      count:
        type: integer
relations: {}
"#,
    )
    .unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:count: nope

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "flavour 'requirement' is invalid",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "invalid integer value 'nope' for field 'count'",
        "validation failed with 4 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_uses_flavours_when_the_relations_section_is_malformed() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join(".mara/schema.yaml"),
        r#"format_version: 3
flavours:
  requirement:
    description: An independently verifiable obligation.
    use_when: [Record project knowledge.]
    avoid_when: []
    distinguish_from: {}
    id_prefix: REQ-
    body: required
    fields:
      count:
        type: integer
relations: invalid
"#,
    )
    .unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:count: nope

:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "invalid schema configuration value 'relations'",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "required body is empty",
        "invalid integer value 'nope' for field 'count'",
        "validation failed with 5 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_retains_known_configuration_after_unknown_keys() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project.replace("[project]\n", "[project]\nunexpected = true\n"),
    )
    .unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:unknown: value

:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "unknown project configuration key 'project.unexpected'",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "required body is empty",
        "unknown metadata field 'unknown'",
        "validation failed with 5 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_retains_schema_after_unknown_root_keys() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(&schema_file, format!("unexpected: true\n{schema}")).unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:unknown: value

:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "unknown schema configuration key 'unexpected'",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "required body is empty",
        "unknown metadata field 'unknown'",
        "validation failed with 5 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_uses_fields_unaffected_by_configuration_type_errors() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    let configured_name = fixture.path().file_name().unwrap().to_str().unwrap();
    fs::write(
        &project_file,
        project.replace(&format!("name = \"{configured_name}\""), "name = 42"),
    )
    .unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        ":::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "invalid project configuration value 'project.name'",
        "item opener must be ':::mara <flavour> <id>' with no other tokens",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_retains_item_identity_after_metadata_errors() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("first.mara.md"),
        ":::mara requirement REQ-DUPLICATE\n:title: First\n\nBody.\n:::\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("second.mara.md"),
        r#":::mara requirement REQ-DUPLICATE
:title: Second
:malformed

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "invalid metadata entry",
        "duplicate item ID 'REQ-DUPLICATE'",
        "validation failed with 5 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_checks_metadata_recovered_before_an_error() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("partial.mara.md"),
        r#":::mara requirement REQ-PARTIAL
:title: Partial
:unknown: value
:derives_from: REQ-MISSING
:malformed

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "invalid metadata entry",
        "unknown metadata field 'unknown'",
        "relation 'derives_from' references missing item 'REQ-MISSING'",
        "validation failed with 4 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_checks_title_errors_proven_before_malformed_metadata() {
    for metadata in [
        ":title:\n:malformed",
        ":title: First\n:title: Second\n:malformed",
    ] {
        let fixture = fixture();
        let init = mara(fixture.path(), &["project", "init"]);
        assert!(init.status.success(), "{}", stderr(&init));
        fs::write(
            fixture.path().join("partial.mara.md"),
            format!(":::mara requirement REQ-PARTIAL\n{metadata}\n\nBody.\n:::\n"),
        )
        .unwrap();

        let validate = mara(fixture.path(), &["project", "validate"]);

        assert!(!validate.status.success());
        let errors = stderr(&validate);
        for expected in [
            "invalid metadata entry",
            "item must have exactly one non-empty title entry",
            "validation failed with 3 diagnostics",
        ] {
            assert!(
                errors.contains(expected),
                "missing {expected:?} in {errors}"
            );
        }
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_does_not_infer_missing_targets_after_item_parse_failures() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("source.mara.md"),
        r#":::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Source
:derives_from: REQ-TARGET

Mentions [[REQ-TARGET]].
:::

:::mara requirement REQ-TARGET trailing
:title: Target

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("item opener must be ':::mara <flavour> <id>' with no other tokens"),
        "{errors}"
    );
    assert!(
        !errors.contains("references missing item 'REQ-TARGET'"),
        "{errors}"
    );
    assert!(
        errors.contains("validation failed with 1 diagnostic"),
        "{errors}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_retains_opener_semantics_after_a_missing_close() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("complete.mara.md"),
        ":::mara requirement WRONG-ID\n:title: Complete\n\nBody.\n:::\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("partial.mara.md"),
        ":::mara requirement WRONG-ID\n:title: Partial\n\nBody without a close.\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("item is missing its closing delimiter"),
        "{errors}"
    );
    assert_eq!(
        errors.matches("duplicate item ID 'WRONG-ID'").count(),
        2,
        "{errors}"
    );
    assert_eq!(
        errors
            .matches("item ID 'WRONG-ID' must start with 'REQ-'")
            .count(),
        2,
        "{errors}"
    );
    assert!(
        errors.contains("validation failed with 7 diagnostics"),
        "{errors}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_does_not_infer_missing_targets_after_include_recovery() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project.replace(
            "include = [\"**/*.mara.md\"]",
            "include = [\"source.mara.md\", \"[\"]",
        ),
    )
    .unwrap();
    fs::write(
        fixture.path().join("source.mara.md"),
        r#":::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Source
:derives_from: REQ-TARGET

Mentions [[REQ-TARGET]].
:::
"#,
    )
    .unwrap();
    fs::write(
        fixture.path().join("target.mara.md"),
        r#":::mara requirement REQ-TARGET
:title: Target

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("invalid content.include pattern '['"),
        "{errors}"
    );
    assert!(
        !errors.contains("references missing item 'REQ-TARGET'"),
        "{errors}"
    );
    assert!(
        errors.contains("validation failed with 1 diagnostic"),
        "{errors}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn item_validation_fails_when_incomplete_corpus_recovery_skips_context_checks() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("source.mara.md"),
        r#":::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Source
:derives_from: REQ-TARGET

Mentions [[REQ-TARGET]].
:::

:::mara requirement REQ-TARGET trailing
:title: Target

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-SOURCE"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains(
            "item 'REQ-SOURCE' could not be fully validated because the project corpus is incomplete"
        ),
        "{errors}"
    );
    assert!(
        errors.contains("validation failed with 1 diagnostic"),
        "{errors}"
    );
    assert!(!stdout(&validate).contains("valid item"));
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[cfg(unix)]
#[test]
fn project_validation_continues_after_directory_walk_errors() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        ":::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBody.\n:::\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("valid.mara.md"),
        ":::mara requirement REQ-VALID\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Valid\n\nMentions [[MISSING-TARGET]].\n:::\n",
    )
    .unwrap();
    let unreadable = fixture.path().join("unreadable");
    fs::create_dir(&unreadable).unwrap();
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "could not discover Mara documents",
        "item opener must be ':::mara <flavour> <id>' with no other tokens",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
    assert!(
        !errors.contains("mention references missing item 'MISSING-TARGET'"),
        "{errors}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn item_validation_reports_configuration_that_prevents_reliable_discovery() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project.replace("**/*.mara.md", "../**/*.mara.md"),
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-MISSING"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("content.include entries must be project-relative patterns"),
        "{errors}"
    );
    assert!(
        !errors.contains("item 'REQ-MISSING' was not found"),
        "{errors}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn item_validation_reports_context_errors_and_a_proven_missing_item() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    let configured_name = fixture.path().file_name().unwrap().to_str().unwrap();
    fs::write(
        &project_file,
        project.replace(&format!("name = \"{configured_name}\""), "name = \"\""),
    )
    .unwrap();
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace("id_prefix: REQ-", "id_prefix: REQ--"),
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-MISSING"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "project.name must not be empty",
        "flavour 'requirement' has invalid ID prefix 'REQ--'",
        "item 'REQ-MISSING' was not found",
        "validation failed with 3 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_validation_does_not_invent_a_schema_version_after_decode_failure() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(&schema_file, schema.replacen("format_version: 3\n", "", 1)).unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("schema configuration key 'format_version' is required"),
        "{errors}"
    );
    assert!(
        !errors.contains("unsupported schema format version 0"),
        "{errors}"
    );
    assert!(
        errors.contains("validation failed with 1 diagnostic"),
        "{errors}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn diagnostic_completeness_tracks_unavailable_item_source_checks() {
    let fixture = fixture();
    let root = fixture.path();
    assert!(mara(root, &["project", "init"]).status.success());
    let malformed = ":::mara requirement REQ-BAD\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Bad metadata\n:bad metadata\n\n[[REQ-UNREADABLE]]\n:::\n";
    fs::write(root.join("bad.mara.md"), malformed).unwrap();
    fs::write(root.join("other.mara.md"), ":::mara requirement REQ-OTHER\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: Other\n\n[[REQ-MISSING]]\n:::\n").unwrap();

    let project = diagnostic_parity(
        root,
        &["project", "validate"],
        "project_validate",
        json!({}),
    );
    assert_eq!(project["valid"], false);
    assert_eq!(project["evaluation_complete"], false);
    assert_eq!(project["summary"]["counts_exact"], false);
    // Known item identities still permit independent missing-reference checks.
    assert_eq!(project["summary"]["errors"], 2);
    assert!(
        project["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "reference_unresolved")
    );

    for handle in ["REQ-BAD", "01ARZ3NDEKTSV4RRFFQ69G5F00"] {
        let item = diagnostic_parity(
            root,
            &["item", "validate", handle],
            "item_validate",
            json!({"id":handle}),
        );
        assert_eq!(item["evaluation_complete"], false);
        assert_eq!(item["summary"]["counts_exact"], false);
        assert_eq!(item["summary"]["errors"], 1);
        assert_eq!(item["diagnostics"][0]["code"], "source_invalid");
    }
    let hidden = diagnostic_parity(
        root,
        &["project", "validate", "--path", "absent/"],
        "project_validate",
        json!({"paths":["absent/"]}),
    );
    assert_eq!(hidden["diagnostics"], json!([]));
    assert_eq!(hidden["evaluation_complete"], false);
    assert_eq!(hidden["summary"], project["summary"]);

    // A different item's fully evaluated failure is still exact.
    let other = diagnostic_parity(
        root,
        &["item", "validate", "REQ-OTHER"],
        "item_validate",
        json!({"id":"REQ-OTHER"}),
    );
    assert_eq!(other["valid"], false);
    assert_eq!(other["evaluation_complete"], true);
    assert_eq!(other["summary"]["counts_exact"], true);
    assert_eq!(other["diagnostics"][0]["code"], "reference_unresolved");
    assert_eq!(
        fs::read_to_string(root.join("bad.mara.md")).unwrap(),
        malformed
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn diagnostic_completeness_accounts_for_invalid_titles() {
    let fixture = fixture();
    let root = fixture.path();
    assert!(mara(root, &["project", "init"]).status.success());
    let file = root.join("title.mara.md");
    for title in ["", ":title: \n", ":title: First\n:title: Second\n"] {
        let source = format!(
            ":::mara requirement REQ-TITLE\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n{title}\n[missing](absent.mara.md)\n:::\n"
        );
        fs::write(&file, &source).unwrap();
        let project = diagnostic_parity(
            root,
            &["project", "validate"],
            "project_validate",
            json!({}),
        );
        assert_eq!(project["evaluation_complete"], false, "title: {title:?}");
        assert_eq!(project["summary"]["counts_exact"], false);
        assert_eq!(project["summary"]["errors"], 1);
        assert_eq!(project["diagnostics"][0]["code"], "field_invalid");
        for id in ["REQ-TITLE", "01ARZ3NDEKTSV4RRFFQ69G5F00"] {
            let item = diagnostic_parity(
                root,
                &["item", "validate", id],
                "item_validate",
                json!({"id":id}),
            );
            assert_eq!(item["evaluation_complete"], false);
            assert_eq!(item["summary"], project["summary"]);
        }
        let hidden = diagnostic_parity(
            root,
            &["project", "validate", "--path", "other/"],
            "project_validate",
            json!({"paths":["other/"]}),
        );
        assert_eq!(hidden["diagnostics"], json!([]));
        assert_eq!(hidden["evaluation_complete"], false);
        assert_eq!(hidden["summary"], project["summary"]);
        assert_eq!(fs::read_to_string(&file).unwrap(), source);
    }
    // Repairing the prerequisite enables the previously skipped reference check.
    fs::write(&file, ":::mara requirement REQ-TITLE\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Repaired\n\n[missing](absent.mara.md)\n:::\n").unwrap();
    let repaired = diagnostic_parity(
        root,
        &["project", "validate"],
        "project_validate",
        json!({}),
    );
    assert_eq!(repaired["evaluation_complete"], true);
    assert_eq!(repaired["summary"]["counts_exact"], true);
    assert_eq!(repaired["summary"]["errors"], 1);
    assert_eq!(repaired["diagnostics"][0]["code"], "reference_unresolved");
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn diagnostic_codes_locations_and_hidden_failures_have_surface_parity() {
    let fixture = fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let source = ":::mara requirement WRONG-PREFIX\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Café\n:unknown: value\n:justifies: REQ-MISSING\n\n[[missing_relation:REQ-MISSING]] [[REQ-MISSING]]\n:::\n";
    fs::write(fixture.path().join("bad.mara.md"), source).unwrap();
    let result = diagnostic_parity(
        fixture.path(),
        &["project", "validate"],
        "project_validate",
        json!({}),
    );
    let diagnostics = result["diagnostics"].as_array().unwrap();
    for code in [
        "identity_invalid",
        "field_invalid",
        "relation_invalid",
        "reference_unresolved",
    ] {
        assert!(
            diagnostics.iter().any(|d| d["code"] == code),
            "missing {code}: {result}"
        );
    }
    for diagnostic in diagnostics {
        assert_eq!(diagnostic["severity"], "error");
        assert_eq!(diagnostic["path"], diagnostic["location"]["path"]);
        assert_eq!(diagnostic["line"], diagnostic["location"]["line"]);
        assert_eq!(diagnostic["item"]["id"], "WRONG-PREFIX");
        let start = diagnostic["location"]["start_byte"].as_u64().unwrap() as usize;
        let end = diagnostic["location"]["end_byte"].as_u64().unwrap() as usize;
        assert!(source.is_char_boundary(start) && source.is_char_boundary(end));
        assert_eq!(
            diagnostic["line"].as_u64().unwrap() as usize,
            source[..start].bytes().filter(|b| *b == b'\n').count() + 1
        );
    }
    let hidden = diagnostic_parity(
        fixture.path(),
        &["project", "validate", "--path", "unrelated/"],
        "project_validate",
        json!({"paths":["unrelated/"]}),
    );
    assert_eq!(hidden["diagnostics"], json!([]));
    assert_eq!(hidden["valid"], false);
    assert_eq!(hidden["summary"], result["summary"]);
    assert_eq!(
        hidden["selection"]["omitted_diagnostics"],
        diagnostics.len()
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("bad.mara.md")).unwrap(),
        source
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn diagnostic_pages_preserve_summary_and_reject_changed_snapshots_or_options() {
    let fixture = fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let source = ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n:extra: x\n\n[[REQ-MISSING]]\n:::\n";
    let file = fixture.path().join("a.mara.md");
    fs::write(&file, source).unwrap();
    let first = diagnostic_parity(
        fixture.path(),
        &["project", "validate", "--limit", "1"],
        "project_validate",
        json!({"limit":1}),
    );
    assert_eq!(first["has_more"], true);
    assert_eq!(first["summary"]["errors"], 2);
    let cursor = first["next_cursor"].as_str().unwrap();
    let next = diagnostic_parity(
        fixture.path(),
        &["project", "validate", "--limit", "1", "--cursor", cursor],
        "project_validate",
        json!({"limit":1,"cursor":cursor}),
    );
    assert_eq!(next["has_more"], false);
    assert_eq!(next["summary"], first["summary"]);
    assert!(first.get("work").is_none());
    assert_ne!(next["diagnostics"], first["diagnostics"]);
    let item = diagnostic_parity(
        fixture.path(),
        &["item", "validate", "REQ-A", "--limit", "1"],
        "item_validate",
        json!({"id":"REQ-A","limit":1}),
    );
    assert_eq!(item["summary"], first["summary"]);
    assert_eq!(item["has_more"], true);
    assert_eq!(
        diagnostic_parity(
            fixture.path(),
            &["project", "validate", "--limit", "2", "--cursor", cursor],
            "project_validate",
            json!({"limit":2,"cursor":cursor}),
        )["error"]["code"],
        "stale_cursor"
    );
    // Even a semantically irrelevant edit invalidates continuation.
    fs::write(&file, format!("{source}\n<!-- changed -->\n")).unwrap();
    assert_eq!(
        diagnostic_parity(
            fixture.path(),
            &["project", "validate", "--limit", "1", "--cursor", cursor],
            "project_validate",
            json!({"limit":1,"cursor":cursor})
        )["error"]["code"],
        "stale_cursor"
    );
    fs::write(&file, source).unwrap();
    let schema = fixture.path().join(".mara/schema.yaml");
    let schema_source = fs::read_to_string(&schema).unwrap();
    fs::write(schema, format!("{schema_source}\n# changed\n")).unwrap();
    assert_eq!(
        diagnostic_parity(
            fixture.path(),
            &["project", "validate", "--limit", "1", "--cursor", cursor],
            "project_validate",
            json!({"limit":1,"cursor":cursor})
        )["error"]["code"],
        "stale_cursor"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn diagnostic_operation_errors_are_distinct_from_policy_failure() {
    let fixture = fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let tools = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        ],
    );
    let tools = &mcp_response(&tools, 2)["result"]["tools"];
    for name in ["project_validate", "item_validate", "schema_validate"] {
        let tool = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap();
        assert!(tool["inputSchema"]["properties"].get("max_work").is_none());
    }
    for (command, tool) in [
        ("project", "project_validate"),
        ("schema", "schema_validate"),
    ] {
        let complete = diagnostic_parity(fixture.path(), &[command, "validate"], tool, json!({}));
        assert_eq!(complete["valid"], true);
        assert_eq!(complete["evaluation_complete"], true);
        assert!(complete.get("work").is_none());
        assert!(
            !stdout(&mara(fixture.path(), &[command, "validate", "--help"])).contains("--max-work")
        );
        for (flag, value, params) in [
            ("--limit", "0", json!({"limit":0})),
            ("--cursor", "", json!({"cursor":""})),
        ] {
            assert_eq!(
                diagnostic_parity(
                    fixture.path(),
                    &[command, "validate", flag, value],
                    tool,
                    params
                )["error"]["code"],
                "invalid_argument"
            );
        }
    }
    fs::write(fixture.path().join("bad.mara.md"), [0xff]).unwrap();
    let unreadable = diagnostic_parity(
        fixture.path(),
        &["project", "validate"],
        "project_validate",
        json!({}),
    );
    assert_eq!(unreadable["diagnostics"][0]["code"], "source_invalid");
    assert!(
        unreadable["diagnostics"][0]["location"]
            .get("line")
            .is_none()
    );
    assert_eq!(unreadable["evaluation_complete"], false);
    fs::remove_file(fixture.path().join(".mara/project.toml")).unwrap();
    let root = fixture.path().to_str().unwrap();
    assert_eq!(
        diagnostic_parity(
            fixture.path(),
            &["--project", root, "project", "validate"],
            "project_validate",
            json!({"project":root})
        )["error"]["code"],
        "io_error"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn diagnostic_repeated_enum_values_complete_without_a_work_budget() {
    let fixture = fixture();
    let root = fixture.path();
    assert!(mara(root, &["project", "init"]).status.success());
    let schema_path = root.join(".mara/schema.yaml");
    let values = (0..200).map(|n| format!("value{n:04}")).collect::<Vec<_>>();
    let schema = fs::read_to_string(&schema_path).unwrap().replace(
        "    id_prefix: REQ-\n    body: required\n    fields: {}",
        &format!("    id_prefix: REQ-\n    body: required\n    fields:\n      status:\n        type: enum\n        repeatable: true\n        values: [{}]", values.join(", ")),
    );
    fs::write(&schema_path, schema).unwrap();
    // Each occurrence matches only the final allowed value.
    let source = format!(
        ":::mara requirement REQ-ENUM\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Enum\n{}\nBody.\n:::\n",
        ":status: value0199\n".repeat(100)
    );
    fs::write(root.join("enum.mara.md"), &source).unwrap();
    for (args, tool, params) in [
        (vec!["project", "validate"], "project_validate", json!({})),
        (
            vec!["item", "validate", "REQ-ENUM"],
            "item_validate",
            json!({"id":"REQ-ENUM"}),
        ),
    ] {
        let complete = diagnostic_parity(root, &args, tool, params);
        assert_eq!(complete["valid"], true);
        assert_eq!(complete["evaluation_complete"], true);
        assert_eq!(complete["summary"]["counts_exact"], true);
        assert!(complete.get("work").is_none());
    }
    assert_eq!(
        fs::read_to_string(root.join("enum.mara.md")).unwrap(),
        source
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn diagnostic_output_budget_never_silently_discards_an_oversized_record() {
    let fixture = fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let huge = "x".repeat(70_000);
    fs::write(fixture.path().join("huge.mara.md"), format!(":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n:{huge}: value\n\nBody.\n:::\n")).unwrap();
    let result = diagnostic_parity(
        fixture.path(),
        &["project", "validate"],
        "project_validate",
        json!({}),
    );
    assert_eq!(result["error"]["code"], "output_limit");
    assert!(serde_json::to_vec(&result).unwrap().len() <= 65_536);
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn project_and_item_validation_run_through_the_real_cli() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("valid.mara.md"),
        ":::mara requirement REQ-VALID\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Valid\n\nA complete requirement.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);
    assert!(validate.status.success(), "{}", stderr(&validate));
    assert!(stdout(&validate).contains("valid project"));

    let item = mara(fixture.path(), &["item", "validate", "REQ-VALID"]);
    assert!(item.status.success(), "{}", stderr(&item));
    assert!(stdout(&item).contains("valid item 'REQ-VALID'"));
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[test]
fn cli_json_and_mcp_return_the_same_structured_validation_diagnostics() {
    let fixture = fixture();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        ":::mara requirement WRONG-ID\n:title: Invalid\n\n\n:::\n",
    )
    .unwrap();

    let cli = mara(fixture.path(), &["--format", "json", "project", "validate"]);
    assert!(!cli.status.success());
    assert!(stderr(&cli).is_empty(), "{}", stderr(&cli));
    let cli_result: Value = serde_json::from_str(&stdout(&cli)).unwrap();
    assert_eq!(cli_result["valid"], false);
    assert_eq!(cli_result["diagnostics"].as_array().unwrap().len(), 3);

    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "project_validate", json!({})),
        ],
    );
    let result = &mcp_response(&responses, 2)["result"];
    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"], cli_result);
}

fn legacy_engineering_project(root: &Path) {
    let init = mara(root, &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        root.join(".mara/schema.yaml"),
        include_str!("fixtures/engineering-legacy-schema.yaml"),
    )
    .unwrap();
}

fn rust_code_fixture() -> TempDir {
    let fixture = fixture();
    let root = fixture.path();
    legacy_engineering_project(root);
    code_index::configure(root, "rust", &["rs"], true);
    let schema_path = root.join(".mara/schema.yaml");
    let mut schema = fs::read_to_string(&schema_path).unwrap();
    schema.push_str("  code_implements:\n    description: Code implements a requirement.\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: implemented_by_code\n");
    fs::write(&schema_path, schema).unwrap();
    fixture
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[cfg(unix)]
#[test]
fn code_discovery_walk_errors_make_validation_incomplete() {
    let fixture = rust_code_fixture();
    let root = fixture.path();
    let unreadable = root.join("unreadable");
    fs::create_dir(&unreadable).unwrap();
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();
    let output = mara(root, &["--format", "json", "project", "validate"]);
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o755)).unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["evaluation_complete"], false, "{result:#}");
    assert!(
        result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic["code"] == "source_invalid"
                    && diagnostic["message"].as_str().is_some_and(|message| {
                        message.contains("could not snapshot indexer inputs")
                    })
            }),
        "{result:#}"
    );
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[cfg(unix)]
#[test]
fn item_validation_reports_invalid_code_markers_and_internal_symlinks_resolve() {
    let fixture = rust_code_fixture();
    let root = fixture.path();
    let item_path = root.join("req.mara.md");
    let item =
        ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n\nA.\n:::\n";
    fs::write(&item_path, item).unwrap();
    let code_path = root.join("implementation.rs");
    fs::write(&code_path, "// @mara unknown_relation REQ-A\nfn run() {}\n").unwrap();
    code_index::write_single(root, "rust", "implementation.rs", "run", "run().");
    let project = validation_with_parity(root, &[]);
    assert_eq!(project["valid"], false);
    let selected: Value = serde_json::from_slice(
        &mara(root, &["--format", "json", "item", "validate", "REQ-A"]).stdout,
    )
    .unwrap();
    assert_eq!(selected["valid"], false, "{selected:#}");
    assert!(
        selected["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic["code"] == "relation_invalid"
                    && diagnostic["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("unknown code relation"))
            }),
        "{selected:#}"
    );

    code_index::write_index(root, "rust", &[]);
    fs::write(
        &code_path,
        "const VALUE: () = {\n    // @mara code_implements REQ-A\n};\n",
    )
    .unwrap();
    let project = validation_with_parity(root, &[]);
    assert_eq!(project["valid"], false);
    let selected: Value = serde_json::from_slice(
        &mara(root, &["--format", "json", "item", "validate", "REQ-A"]).stdout,
    )
    .unwrap();
    assert_eq!(selected["valid"], false, "{selected:#}");
    assert!(
        selected["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "code_unsupported"),
        "{selected:#}"
    );
    code_index::write_index(root, "rust", &[]);
    fs::write(
        &code_path,
        "const VALUE: () = {\n    // @mara code_implements 01ARZ3NDEKTSV4RRFFQ69G5F00\n};\n",
    )
    .unwrap();
    let selected: Value = serde_json::from_slice(
        &mara(root, &["--format", "json", "item", "validate", "REQ-A"]).stdout,
    )
    .unwrap();
    assert_eq!(selected["valid"], false, "{selected:#}");
    assert!(
        selected["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "code_unsupported"),
        "{selected:#}"
    );

    fs::write(&code_path, "fn run() {}\n").unwrap();
    std::os::unix::fs::symlink("implementation.rs", root.join("linked.rs")).unwrap();
    code_index::write_single(root, "rust", "linked.rs", "run", "run().");
    fs::write(
        &item_path,
        item.replace(
            ":title: A\n",
            ":title: A\n:implemented_by_code: code:linked.rs::rust::run().\n",
        ),
    )
    .unwrap();
    assert_eq!(validation_with_parity(root, &[])["valid"], true);
    let get: Value = serde_json::from_slice(
        &mara(
            root,
            &["--format", "json", "get", "code:linked.rs::rust::run()."],
        )
        .stdout,
    )
    .unwrap();
    assert_eq!(get["content"], "fn run() {}");
    assert_eq!(get["node"]["source"]["path"], "linked.rs");
    let related: Value = serde_json::from_slice(
        &mara(
            root,
            &[
                "--format",
                "json",
                "related",
                "code:linked.rs::rust::run().",
            ],
        )
        .stdout,
    )
    .unwrap();
    assert_eq!(related["connections"][0]["neighbour"]["id"], "REQ-A");
}

// @mara checks REQ-PROJECT-VALIDATION
// @mara checks DES-TRACE-DIAGNOSTIC-INTERFACE
#[cfg(unix)]
#[test]
fn invalid_language_pack_does_not_return_partial_code_relations() {
    let fixture = rust_code_fixture();
    let root = fixture.path();
    fs::write(
        root.join("req.mara.md"),
        ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n\nA.\n:::\n",
    )
    .unwrap();
    fs::write(
        root.join("implementation.rs"),
        "// @mara code_implements REQ-A\nfn run() {}\n",
    )
    .unwrap();
    code_index::write_single(root, "rust", "implementation.rs", "run", "run().");
    let related = mara(root, &["--format", "json", "related", "REQ-A"]);
    assert!(related.status.success(), "{}", stderr(&related));
    let related: Value = serde_json::from_slice(&related.stdout).unwrap();
    assert!(
        related["connections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| {
                entry["neighbour"]["reference"] == "code:implementation.rs::rust::run()."
            })
    );

    fs::write(root.join(".mara/code/rust.scm"), "(").unwrap();
    let validation = validation_with_parity(root, &[]);
    assert_eq!(validation["valid"], false);
    assert!(
        validation["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| { diagnostic["code"] == "code_unsupported" })
    );
    let related = mara(root, &["related", "REQ-A"]);
    assert!(!related.status.success());
    assert!(
        stderr(&related).contains("rust.scm"),
        "{}",
        stderr(&related)
    );

    fs::write(
        root.join(".mara/code/rust.scm"),
        "(function_item) @symbol\n(function_item name: (_) @name)\n(line_comment) @comment\n",
    )
    .unwrap();
    let validation = validation_with_parity(root, &[]);
    assert_eq!(validation["valid"], false);
    assert!(
        validation["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "code_unsupported"
                && diagnostic["message"]
                    .as_str()
                    .unwrap()
                    .contains("pair @name with @symbol"))
    );
    let related = mara(root, &["related", "REQ-A"]);
    assert!(!related.status.success());
    assert!(stderr(&related).contains("pair @name with @symbol"));
}
