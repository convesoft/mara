//! VER-UNCHANGED-SOURCES for the read operations: on a broken project, no read, failing ones
//! included, creates, deletes or modifies a file.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use crate::{Project, item, link, mid};

/// Every file and directory below `root`, with its bytes and mtime.
fn tree(root: &Path) -> BTreeMap<String, (Vec<u8>, SystemTime)> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            let bytes = if metadata.is_file() {
                fs::read(&path).unwrap()
            } else {
                Vec::new()
            };
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            found.insert(name, (bytes, metadata.modified().unwrap()));
            if metadata.is_dir() {
                pending.push(path);
            }
        }
    }
    found
}

#[test]
fn no_read_changes_a_file_of_a_broken_project() {
    let project = Project::new();
    // A missing anchor, a bare-ID relation value, stale link paths and text, invalid rows.
    let missing_anchor =
        "### No anchor\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-NONE |\n\nBody.\n";
    let broken = item(
        &mid(1),
        "Requirement",
        "REQ-BROKEN",
        "Broken",
        &[
            ("Satisfies", "NEED-BARE"),
            ("Refines", &link("REQ-OLD-NAME", "wrong.mara.md", &mid(2))),
            ("Colour", "Red"),
            ("Priority", "Urgent"),
            ("Priority", "Urgent"),
        ],
        &format!("Mentions {}.", link("gone", "elsewhere.mara.md", &mid(9))),
    );
    let target = item(
        &mid(2),
        "Requirement",
        "REQ-TARGET",
        "Target",
        &[],
        "Holds word.",
    );
    // CRLF line endings throughout.
    let text = format!(
        "# Broken\n\nNarrative word.\n\n---\n\n{missing_anchor}\n---\n\n{broken}\n---\n\n{target}"
    )
    .replace('\n', "\r\n");
    project.write("docs/a.mara.md", &text);
    let page = project
        .run(&["item", "list", "--limit", "1", "--format", "json"])
        .ok()
        .json();
    let cursor = page["next_cursor"].as_str().unwrap().to_owned();
    let before = tree(&project.root());
    let reads: Vec<Vec<&str>> = vec![
        vec!["search", "word"],
        vec!["search", "word", "--format", "json"],
        vec!["get", "REQ-BROKEN"],
        vec!["get", "docs/a.mara.md:3", "--format", "json"],
        vec!["get", "docs/a.mara.md"],
        vec!["relation", "list", "REQ-BROKEN"],
        vec!["item", "list", "--cursor", &cursor],
        vec!["schema", "list"],
        vec!["schema", "get", "requirement"],
        // Failing requests.
        vec!["get", "REQ-MISSING"],
        vec!["relation", "list", "docs/a.mara.md"],
        vec!["item", "list", "--type", "requirement", "--cursor", &cursor],
        vec!["schema", "get", "missing"],
        vec!["search", "word", "--limit", "0"],
        vec!["item", "list", "--no-such-option"],
    ];
    for read in reads {
        let run = project.run(&read);
        assert!(run.status <= 2, "{read:?}: {run:#?}");
        assert!(
            tree(&project.root()) == before,
            "{read:?} changed the project tree"
        );
    }
}
