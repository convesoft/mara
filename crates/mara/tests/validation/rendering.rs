//! VER-GITHUB-RENDERING: documents checked against their GitHub rendering.
//!
//! Analysis of GitHub's handling of the links of `links_resolve_from_the_linking_file`: every
//! relation link and mention there is `[text](<relative path>#<MID>)` or `[text](#<MID>)`, with
//! the MID in uppercase, in a table cell or in a paragraph. EVD-GITHUB-FRAGMENT-MATCHING found
//! that GitHub keeps such hrefs as written, resolves the path from the linking file, and
//! navigates to the anchor `user-content-<lowercase MID>` by matching the fragment
//! case-insensitively, for same-file, cross-file and table-cell links. The test checks that
//! each path, resolved from the linking file as GitHub resolves it, names the file that holds
//! the MID's anchor; so each link navigates to its target.
//!
//! Inspection of the CLI's operations: `project validate`, `item validate` and `schema
//! validate` write no file (VER-UNCHANGED-SOURCES checks it), so none writes documents in
//! another format.

use std::collections::HashMap;

use crate::{Dir, SCHEMA, at, document, item, json, line_of};

const A: &str = "01M7DM833ZEHAAPSXERFMWWVM3";
const B: &str = "01M7DM8345CRE7FD0J3XKXC3WT";
const C: &str = "01M7DM834C8XKF1AG88N5AZMFV";
const D: &str = "01M7DM834KEVD5V9YJVG4N2SB7";
const MISSING: &str = "01M7DM834TF3ZRQPDPH79W9414";

/// Item validation shows which item holds a mention: a mention of a MID that no item has is a
/// `broken-link` of the item it is in.
#[test]
fn regions_break_at_star_and_underscore_breaks_and_not_inside_code() {
    let dir = Dir::project(SCHEMA);
    let first = item(
        A,
        "Holds a fenced break",
        "Requirement",
        "| ID | REQ-A |\n| Status | Draft |\n",
        &format!("Code:\n\n```text\nbefore\n---\nafter\n```\n\nStill [in A](#{MISSING})."),
    );
    let second = item(
        B,
        "After stars",
        "Requirement",
        "| ID | REQ-B |\n| Status | Draft |\n",
        "B.",
    );
    let third = item(
        C,
        "After underscores",
        "Requirement",
        "| ID | REQ-C |\n| Status | Draft |\n",
        &format!("In [C](#{MISSING})."),
    );
    let text = format!("# Breaks\n\n---\n\n{first}\n***\n\n{second}\n___\n\n{third}");
    dir.write("docs/breaks.mara.md", &text);
    let path = "docs/breaks.mara.md";
    let in_a = at("broken-link", path, line_of(&text, "Still [in A]"));
    let in_c = at("broken-link", path, line_of(&text, "In [C]"));
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!(run.diagnostics(), [in_a.clone(), in_c.clone()], "{run:?}");
    // The `***` and `___` lines end items: each item is read, holds its own mentions, and no
    // anchor is out of place. The `---` inside the fence does not end A.
    assert_eq!(
        json(&dir.root(), &["item", "validate", "REQ-A"]).diagnostics(),
        [in_a]
    );
    assert_eq!(
        json(&dir.root(), &["item", "validate", "REQ-B"]).diagnostics(),
        []
    );
    assert_eq!(
        json(&dir.root(), &["item", "validate", "REQ-C"]).diagnostics(),
        [in_c]
    );
}

#[test]
fn a_document_that_starts_with_front_matter_has_no_diagnostic() {
    let dir = Dir::project(SCHEMA);
    let body = item(
        A,
        "After front matter",
        "Requirement",
        "| ID | REQ-A |\n| Status | Draft |\n",
        "A.",
    );
    dir.write(
        "docs/front.mara.md",
        format!("---\ntitle: Front matter\ntags: [a, b]\n---\n\n# Front\n\n---\n\n{body}"),
    );
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!((run.status, run.diagnostics()), (0, vec![]), "{run:?}");
}

#[test]
fn links_resolve_from_the_linking_file() {
    let dir = Dir::project(SCHEMA);
    let files = [
        (
            "docs/a.mara.md",
            document(
                "A",
                &[
                    item(
                        A,
                        "Root requirement",
                        "Requirement",
                        &format!(
                            "| ID | REQ-A |\n| Status | Draft |\n| Refines | [REQ-B](#{B}) |\n"
                        ),
                        &format!("Same file: [B](#{B}); other directory: [C](area/c.mara.md#{C})."),
                    ),
                    item(
                        B,
                        "Same-file target",
                        "Requirement",
                        "| ID | REQ-B |\n| Status | Draft |\n",
                        "B.",
                    ),
                ],
            ),
        ),
        (
            "docs/area/c.mara.md",
            document(
                "C",
                &[item(
                    C,
                    "Nested requirement",
                    "Requirement",
                    &format!(
                        "| ID | REQ-C |\n| Status | Draft |\n| Refines | [REQ-A](../a.mara.md#{A}) |\n"
                    ),
                    &format!(
                        "| Mentioned | In a table cell |\n| --- | --- |\n| [D](../other/d.mara.md#{D}) | [A](../a.mara.md#{A}) |"
                    ),
                )],
            ),
        ),
        (
            "docs/other/d.mara.md",
            format!(
                "# D\n\nNarrative mentions [C](../area/c.mara.md#{C}).\n\n---\n\n{}",
                item(
                    D,
                    "A note",
                    "Note",
                    &format!(
                        "| ID | NOTE-D |\n| Status | Draft |\n| Explains | [REQ-C](../area/c.mara.md#{C}) |\n"
                    ),
                    "D.",
                )
            ),
        ),
    ];
    for (path, text) in &files {
        dir.write(path, text);
    }
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!((run.status, run.diagnostics()), (0, vec![]), "{run:?}");

    // Independently of Mara: each link's path, resolved from the linking file, names the file
    // that holds the anchor of its MID.
    let mut holder: HashMap<String, &str> = HashMap::new();
    for (path, text) in &files {
        for (start, _) in text.match_indices("<a id=\"") {
            holder.insert(text[start + 7..start + 33].to_owned(), path);
        }
    }
    let mut links = 0;
    for (path, text) in &files {
        for (start, _) in text.match_indices("](") {
            let rest = &text[start + 2..];
            let destination = &rest[..rest.find(')').unwrap()];
            let (relative, mid) = destination.split_once('#').unwrap();
            let directory = path.rsplit_once('/').unwrap().0;
            let mut parts: Vec<&str> = directory.split('/').collect();
            if relative.is_empty() {
                parts = path.split('/').collect();
            } else {
                for part in relative.split('/') {
                    match part {
                        ".." => {
                            parts.pop();
                        }
                        part => parts.push(part),
                    }
                }
            }
            assert_eq!(parts.join("/"), holder[mid], "{destination} in {path}");
            links += 1;
        }
    }
    assert_eq!(links, 8);
}
