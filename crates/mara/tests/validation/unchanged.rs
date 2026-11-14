//! VER-UNCHANGED-SOURCES for the validation operations: every run, failing ones included,
//! leaves the project tree as it was.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use crate::{Dir, SCHEMA, mara};

const A: &str = "01M7DM8369HSQMCFREMB2NZFN3";
const B: &str = "01M7DM836HPYNHAMQD27QGZCDM";

/// Names, bytes and modification times of every entry below `root`.
fn tree(root: &Path) -> BTreeMap<String, (Option<Vec<u8>>, SystemTime)> {
    let mut entries = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let metadata = fs::symlink_metadata(entry.path()).unwrap();
            let name = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .display()
                .to_string();
            let bytes = if metadata.is_dir() {
                pending.push(entry.path());
                None
            } else {
                Some(fs::read(entry.path()).unwrap())
            };
            entries.insert(name, (bytes, metadata.modified().unwrap()));
        }
    }
    entries
}

#[test]
fn no_validation_run_changes_the_project() {
    let dir = Dir::project(SCHEMA);
    // A missing anchor, a bare-ID relation value, a stale link path and text, invalid rows and
    // CRLF line endings.
    let text = format!(
        "# Broken\n\n---\n\n### No anchor\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-NONE |\n\
         | Status | Draft |\n\nBody.\n\n---\n\n<a id=\"{A}\"></a>\n\n### Links\n\n\
         | Type | Requirement |\n| --- | --- |\n| ID | REQ-A |\n| Status | Draft |\n\
         | Refines | REQ-B |\n| Refines | [OLD](other.mara.md#{B}) |\n| Owner | me |\n\
         | Priority | Urgent |\n| Tags | x |\n| Tags | x |\n\nBody.\n\n---\n\n<a id=\"{B}\"></a>\n\n\
         ### Target\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-B |\n| Status | Draft |\n\nBody.\n"
    )
    .replace('\n', "\r\n");
    dir.write("docs/broken.mara.md", text);
    let root = dir.root();
    let before = tree(&root);

    let first = mara(
        &root,
        &["--format", "json", "project", "validate", "--limit", "1"],
    );
    assert_eq!(first.status, 1);
    let codes: Vec<String> = mara(&root, &["--format", "json", "project", "validate"])
        .diagnostics()
        .into_iter()
        .map(|d| d.0)
        .collect();
    assert_eq!(
        codes,
        [
            "incomplete-item",
            "bare-id-value",
            "stale-link-path",
            "stale-link-text",
            "undeclared-key",
            "invalid-value",
            "duplicate-row"
        ]
    );
    let cursor = first.json()["next_cursor"].as_str().unwrap().to_owned();
    let runs: [(&[&str], i32); 11] = [
        (&["project", "validate"], 1),
        (&["project", "validate", "--cursor", &cursor], 1),
        (&["project", "validate", "--path", "docs/broken.mara.md"], 1),
        (
            &["--format", "json", "project", "validate", "--limit", "2"],
            1,
        ),
        (&["item", "validate", "REQ-A"], 1),
        (&["item", "validate", B], 0),
        (&["item", "validate", "REQ-NONE"], 1),
        (&["item", "validate", "REQ-A", "--cursor", &cursor], 1),
        (&["schema", "validate"], 0),
        (&["project", "validate", "--no-such-option"], 2),
        (&[], 0),
    ];
    for (args, status) in runs {
        let run = mara(&root, args);
        assert_eq!(run.status, status, "{args:?}: {run:?}");
        assert!(
            tree(&root) == before,
            "mara {args:?} changed the project tree"
        );
    }
}
