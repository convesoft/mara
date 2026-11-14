//! VER-NESTED-PROJECTS for the validation operations: a project that holds a second project
//! in a subdirectory, both including every `*.mara.md` below their roots.

use std::fs;
use std::path::Path;

use crate::{Dir, SCHEMA, at, document, item, json, line_of};

const OUTER: &str = "01M7DM835184W9ZBKQK7HVQEHK";
const INNER: &str = "01M7DM835A9D71RGM7GKAF2GCN";
const PLAIN: &str = "01M7DM835JG3WN5WWESB7HE3YM";

const CONFIG: &str = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\nformat_version = 1\ninclude = [\"**/*.mara.md\"]\n";

/// A document whose one item, `REQ-SHARED`, has the invalid status `status`.
fn shared(mid: &str, status: &str) -> String {
    document(
        "Shared",
        &[item(
            mid,
            "Shared",
            "Requirement",
            &format!("| ID | REQ-SHARED |\n| Status | {status} |\n"),
            "Body.",
        )],
    )
}

/// The outer project at the root, the inner one at `sub/inner`.
fn nested() -> Dir {
    let dir = Dir::new();
    for root in ["", "sub/inner/"] {
        dir.write(&format!("{root}.mara/config.toml"), CONFIG);
        dir.write(&format!("{root}.mara/schema.toml"), SCHEMA);
    }
    dir.write("docs/outer.mara.md", shared(OUTER, "Outer"));
    dir.write("sub/inner/docs/inner.mara.md", shared(INNER, "Inner"));
    dir
}

/// The validation operations, with the files each reads besides the documents.
const OPERATIONS: [(&[&str], bool); 3] = [
    (&["project", "validate"], true),
    (&["item", "validate", "REQ-SHARED"], true),
    (&["schema", "validate"], false),
];

/// Runs each validation operation from `directory` with `extra` arguments, and checks that it
/// reports only the problem of `expected`, a document of the selected project relative to its
/// root, and reads only that project's configuration, schema and `documents` documents.
fn uses_only(dir: &Dir, directory: &str, extra: &[&str], expected: &str, documents: u64) {
    let text = fs::read_to_string(selected_root(dir, directory, extra).join(expected)).unwrap();
    for (operation, reads_documents) in OPERATIONS {
        let mut args = extra.to_vec();
        args.extend(operation);
        let run = json(&dir.path(directory), &args);
        let (diagnostics, reads) = match reads_documents {
            true => (
                vec![at("invalid-value", expected, line_of(&text, "| Status"))],
                2 + documents,
            ),
            false => (vec![], 2),
        };
        assert_eq!(run.diagnostics(), diagnostics, "{args:?} in {directory}");
        assert_eq!(run.file_reads(), reads, "{args:?} in {directory}");
    }
}

/// The root that `extra` (`--project`) names from `directory`, or the nearest project.
fn selected_root(dir: &Dir, directory: &str, extra: &[&str]) -> std::path::PathBuf {
    match extra {
        ["--project", project] => fs::canonicalize(dir.path(directory).join(project)).unwrap(),
        _ => {
            let start = dir.path(directory);
            start
                .ancestors()
                .find(|ancestor| ancestor.join(".mara/config.toml").exists())
                .map(Path::to_path_buf)
                .unwrap()
        }
    }
}

#[test]
fn each_operation_uses_only_the_project_it_selects() {
    let dir = nested();
    fs::create_dir_all(dir.path("sub/inner/docs/deep")).unwrap();
    // From inside the inner project: none of the outer project's files is read.
    uses_only(&dir, "sub/inner/docs/deep", &[], "docs/inner.mara.md", 1);
    // From the outer project, outside the inner one: none of the inner project's files.
    uses_only(&dir, "docs", &[], "docs/outer.mara.md", 1);
    // `--project` naming one project, from inside the other.
    uses_only(
        &dir,
        "docs",
        &["--project", "../sub/inner"],
        "docs/inner.mara.md",
        1,
    );
    uses_only(
        &dir,
        "sub/inner/docs",
        &["--project", "../../.."],
        "docs/outer.mara.md",
        1,
    );
}

#[test]
fn a_converted_or_plain_subdirectory_is_handled_as_its_configuration_says() {
    let dir = nested();
    // The inner project as a 0.3.0 project: still another project's root.
    fs::remove_file(dir.path("sub/inner/.mara/config.toml")).unwrap();
    dir.write("sub/inner/.mara/project.toml", "version = 1\n");
    uses_only(&dir, "docs", &[], "docs/outer.mara.md", 1);

    // A `config.toml` outside any `.mara/` makes no project: its documents are inputs.
    dir.write("plain/config.toml", CONFIG);
    let text = shared(PLAIN, "Plain").replace("REQ-SHARED", "REQ-PLAIN");
    dir.write("plain/docs/plain.mara.md", &text);
    let run = json(&dir.path("docs"), &["project", "validate"]);
    let outer = dir.read("docs/outer.mara.md");
    assert_eq!(
        run.diagnostics(),
        [
            at(
                "invalid-value",
                "docs/outer.mara.md",
                line_of(&outer, "| Status")
            ),
            at(
                "invalid-value",
                "plain/docs/plain.mara.md",
                line_of(&text, "| Status")
            ),
        ]
    );
    assert_eq!(run.file_reads(), 4);

    // The 0.3.0 project itself is refused, with the conversion named.
    let run = json(&dir.path("sub/inner/docs"), &["project", "validate"]);
    assert_eq!(run.status, 1);
    let error = &run.json()["error"];
    assert_eq!(error["code"], "legacy-project");
    assert_eq!(error["diagnostics"], serde_json::json!([]));
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("mara project convert"),
        "{error}"
    );
    assert_eq!((run.file_reads(), run.parses()), (0, 0));
}

#[test]
fn outside_every_project_fails_and_parses_nothing() {
    let dir = Dir::new();
    if let Some(above) = dir.root().ancestors().find(|ancestor| {
        ancestor.join(".mara/config.toml").exists() || ancestor.join(".mara/project.toml").exists()
    }) {
        panic!(
            "the temporary directory is inside the project at {}: set TMPDIR outside every Mara project",
            above.display()
        );
    }
    dir.write("docs/stray.mara.md", shared(OUTER, "Draft"));
    for (operation, _) in OPERATIONS {
        let run = json(&dir.path("docs"), operation);
        assert_eq!(run.status, 1, "{operation:?}");
        assert_eq!(run.json()["error"]["code"], "no-project", "{operation:?}");
        assert_eq!(run.json()["error"]["diagnostics"], serde_json::json!([]));
        assert_eq!((run.file_reads(), run.parses()), (0, 0), "{operation:?}");
        let run = crate::mara(&dir.path("docs"), operation);
        assert!(
            run.stderr
                .starts_with("mara: error[no-project]: no Mara project in "),
            "{}",
            run.stderr
        );
        assert_eq!(run.stdout, "");
    }
}
