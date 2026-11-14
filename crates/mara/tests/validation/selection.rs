//! Project selection and loading through the validation operations, with the codes of
//! DES-PROJECT-CONFIGURATION. Configuration and schema errors never refuse validation: each is
//! a diagnostic of a result.

use std::fs;

use crate::{CONFIG, Dir, SCHEMA, at, document, item, json, line_of};

const A: &str = "01M7DM835SD82QFD800BHHCXZE";
const B: &str = "01M7DM835ZTGS6Z3CWNNHS42R1";

/// A document with one problem that needs no schema: an item without `Status`.
fn without_status() -> String {
    document(
        "Docs",
        &[item(
            A,
            "No status",
            "Requirement",
            "| ID | REQ-A |\n",
            "Body.",
        )],
    )
}

/// A document whose item's `Status` is outside the schema's statuses.
fn with_bad_status() -> String {
    document(
        "Docs",
        &[item(
            B,
            "Bad status",
            "Requirement",
            "| ID | REQ-B |\n| Status | Done |\n",
            "Body.",
        )],
    )
}

#[test]
fn selects_from_a_subdirectory_and_with_the_project_option() {
    let dir = Dir::project(SCHEMA);
    dir.write("proj/.mara/config.toml", CONFIG);
    dir.write("proj/.mara/schema.toml", SCHEMA);
    dir.write("proj/docs/deep/er/a.mara.md", without_status());
    let missing = at("missing-row", "docs/deep/er/a.mara.md", 5);
    let from = |directory: &str, args: &[&str]| {
        let run = json(&dir.path(directory), args);
        assert_eq!(
            run.diagnostics(),
            std::slice::from_ref(&missing),
            "{args:?} from {directory}"
        );
    };
    from("proj/docs/deep/er", &["project", "validate"]);
    from("", &["--project", "proj", "project", "validate"]);
    let absolute = dir.path("proj").display().to_string();
    from(
        "proj/docs",
        &["project", "validate", "--project", &absolute],
    );

    // `--project` names the root itself; it is not searched from. The problem lies in no
    // project, so it has no location.
    let run = json(
        &dir.root(),
        &["--project", "proj/docs", "project", "validate"],
    );
    assert_eq!(run.status, 1);
    assert_eq!(run.json()["error"]["code"], "not-a-project");
    assert_eq!(run.json()["error"]["diagnostics"], serde_json::json!([]));
    assert_eq!(run.file_reads(), 0);
}

/// A directory whose `.mara` cannot be examined stops the search, located like loading
/// locates an unreadable configuration.
#[cfg(unix)]
#[test]
fn an_unreadable_mara_directory_is_located_at_its_configuration() {
    use std::os::unix::fs::PermissionsExt;
    let dir = Dir::project(SCHEMA);
    fs::create_dir_all(dir.path("docs")).unwrap();
    let mara = dir.path(".mara");
    fs::set_permissions(&mara, fs::Permissions::from_mode(0o000)).unwrap();
    let readable = fs::read_dir(&mara).is_ok();
    let run = json(&dir.path("docs"), &["project", "validate"]);
    fs::set_permissions(&mara, fs::Permissions::from_mode(0o755)).unwrap();
    if readable {
        // Permissions do not apply, as for root.
        return;
    }
    let error = &run.json()["error"];
    assert_eq!((run.status, error["code"].clone()), (1, "io-error".into()));
    assert_eq!(
        crate::diagnostics(&error["diagnostics"]),
        [at("io-error", ".mara/config.toml", 1)]
    );
}

#[test]
fn reports_configuration_and_schema_problems_with_the_documents() {
    let dir = Dir::project(&SCHEMA.replace("[types.note]\n", "[types.note]\ncolour = 1\n"));
    dir.write(
        ".mara/config.toml",
        CONFIG.replace("[documents]\n", "name = \"x\"\n\n[documents]\n"),
    );
    let text = with_bad_status();
    dir.write("docs/a.mara.md", &text);
    let schema = dir.read(".mara/schema.toml");
    let config = dir.read(".mara/config.toml");
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!(run.status, 1);
    assert_eq!(
        run.diagnostics(),
        [
            at(
                "unknown-key",
                ".mara/config.toml",
                line_of(&config, "name = ")
            ),
            at(
                "unknown-key",
                ".mara/schema.toml",
                line_of(&schema, "colour = 1")
            ),
            at(
                "invalid-value",
                "docs/a.mara.md",
                line_of(&text, "| Status")
            ),
        ]
    );
}

#[test]
fn a_missing_schema_is_reported_at_the_line_that_names_it() {
    let dir = Dir::new();
    dir.write(
        ".mara/config.toml",
        CONFIG.replace(".mara/schema.toml", "schemas/main.toml"),
    );
    dir.write("docs/a.mara.md", without_status());
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!(run.status, 1);
    // Without a schema, the checks that need none still run.
    assert_eq!(
        run.diagnostics(),
        [
            at("io-error", ".mara/config.toml", 2),
            at("missing-row", "docs/a.mara.md", 5)
        ]
    );
    let run = json(&dir.root(), &["schema", "validate"]);
    assert_eq!(run.diagnostics(), [at("io-error", ".mara/config.toml", 2)]);
}

#[test]
fn another_format_version_stops_reading_its_file() {
    let dir = Dir::project(SCHEMA);
    dir.write("docs/a.mara.md", without_status());
    dir.write(
        ".mara/config.toml",
        CONFIG.replacen("format_version = 1", "format_version = 2", 1),
    );
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!(
        run.diagnostics(),
        [at("unsupported-format-version", ".mara/config.toml", 1)]
    );
    assert_eq!((run.status, run.file_reads()), (1, 1));

    dir.write(".mara/config.toml", CONFIG);
    dir.write(
        ".mara/schema.toml",
        SCHEMA.replacen("format_version = 1", "format_version = 2", 1),
    );
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!(
        run.diagnostics(),
        [
            at("unsupported-format-version", ".mara/schema.toml", 1),
            at("missing-row", "docs/a.mara.md", 5),
        ]
    );
}

#[test]
fn a_schema_path_into_another_project_is_not_read() {
    let dir = Dir::project(SCHEMA);
    dir.write("nested/.mara/config.toml", CONFIG);
    dir.write("nested/.mara/schema.toml", SCHEMA);
    dir.write("legacy/.mara/project.toml", "version = 1\n");
    dir.write("legacy/schema.toml", SCHEMA);
    for schema in ["nested/.mara/schema.toml", "legacy/schema.toml"] {
        dir.write(
            ".mara/config.toml",
            CONFIG.replace(".mara/schema.toml", schema),
        );
        for operation in [&["project", "validate"][..], &["schema", "validate"]] {
            let run = json(&dir.root(), operation);
            assert_eq!(run.status, 1);
            assert_eq!(
                run.diagnostics(),
                [at("outside-project", ".mara/config.toml", 2)],
                "{schema}"
            );
            assert_eq!(
                run.file_reads(),
                1,
                "only the configuration is read: {schema}"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_in_the_project_files_is_refused_and_not_followed() {
    use std::os::unix::fs::symlink;

    // `.mara` itself: no project is selected.
    let dir = Dir::new();
    dir.write("real/config.toml", CONFIG);
    dir.write("real/schema.toml", SCHEMA);
    symlink("real", dir.path(".mara")).unwrap();
    fs::create_dir_all(dir.path("docs")).unwrap();
    // By the ancestor search and by `--project`, at line 1 of the configuration.
    let root = dir.root().display().to_string();
    for (directory, args) in [
        ("docs", &["project", "validate"][..]),
        ("", &["--project", &root, "schema", "validate"]),
    ] {
        let run = json(&dir.path(directory), args);
        let error = &run.json()["error"];
        assert_eq!(
            (run.status, error["code"].clone()),
            (1, "symbolic-link".into())
        );
        assert_eq!(
            crate::diagnostics(&error["diagnostics"]),
            [at("symbolic-link", ".mara/config.toml", 1)],
            "{args:?}"
        );
        assert_eq!(run.file_reads(), 0);
    }
    let run = crate::mara(&dir.root(), &["project", "validate"]);
    let lines: Vec<&str> = run.stderr.lines().collect();
    assert!(
        lines[0].starts_with("mara: error[symbolic-link]: "),
        "{}",
        run.stderr
    );
    assert!(
        lines[1].starts_with(".mara/config.toml:1: error[symbolic-link]: "),
        "{}",
        run.stderr
    );

    // `.mara/config.toml`: reported at its line 1, nothing behind it read.
    let dir = Dir::new();
    dir.write("real.toml", CONFIG);
    dir.write(".mara/schema.toml", SCHEMA);
    symlink("../real.toml", dir.path(".mara/config.toml")).unwrap();
    for operation in [&["project", "validate"][..], &["schema", "validate"]] {
        let run = json(&dir.root(), operation);
        assert_eq!(
            run.diagnostics(),
            [at("symbolic-link", ".mara/config.toml", 1)]
        );
        assert_eq!((run.status, run.file_reads()), (1, 0));
    }

    // A component of the schema path: reported at the line that names the schema.
    let dir = Dir::project(SCHEMA);
    dir.write("real/main.toml", SCHEMA);
    symlink("real", dir.path("schemas")).unwrap();
    dir.write(
        ".mara/config.toml",
        CONFIG.replace(".mara/schema.toml", "schemas/main.toml"),
    );
    for operation in [&["project", "validate"][..], &["schema", "validate"]] {
        let run = json(&dir.root(), operation);
        assert_eq!(
            run.diagnostics(),
            [at("symbolic-link", ".mara/config.toml", 2)]
        );
        assert_eq!((run.status, run.file_reads()), (1, 1));
    }
    fs::remove_file(dir.path("schemas")).unwrap();
}
