//! Tests of move and rename below the host: the edits they compute read back, leave a valid
//! project and change only the bytes they must.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::*;
use crate::document::DocumentKey;
use crate::hash::Hash;
use crate::schema::parse_schema;
use crate::snapshot::{DocumentInput, ParsedFile, SnapshotInputs};
use crate::validation;
use crate::work::WorkCounts;
use crate::write::ReadBack;

const X: &str = "01M7D8DZ2663WP3A037SR2HJH2";
const Y: &str = "01M7D8DZ2RBHGQBYAV7V3789H7";
const Z: &str = "01M7D8DZ39AMW9NPW9NA17D4S4";
const W: &str = "01M7D8DZ3MXVBN7QW52PEJCX4N";

const SCHEMA: &str = "format_version = 1\nstatuses = [\"draft\"]\n\n\
    [types.requirement]\nid_prefix = \"REQ-\"\ndescription = \"A requirement.\"\n\n\
    [relations.refines]\ninverse = \"refined_by\"\ndescription = \"Refines.\"\n\
    targets = { requirement = [\"requirement\"] }\n";

type Files = BTreeMap<String, String>;

fn snapshot(files: &Files) -> Snapshot {
    let (schema, diagnostics) = parse_schema(".mara/schema.toml", SCHEMA.as_bytes());
    assert_eq!(diagnostics, []);
    let documents = files
        .iter()
        .map(|(path, text)| {
            let mut work = WorkCounts::default();
            DocumentInput {
                path: path.clone(),
                key: DocumentKey::new(Hash::of_file(text.as_bytes(), &mut work)),
                file: Arc::new(ParsedFile::parse(text.as_bytes().to_vec(), &mut work)),
            }
        })
        .collect();
    SnapshotInputs::new(documents, schema.map(Arc::new), None, None)
        .build(&mut WorkCounts::default())
}

/// The diagnostics of the project of `files`, as `(path, line, code)`.
fn problems(files: &Files) -> Vec<(String, usize, &'static str)> {
    let snapshot = snapshot(files);
    let findings = validation::check(&snapshot, &mut WorkCounts::default());
    findings
        .into_iter()
        .map(|finding| {
            let diagnostic = finding.diagnostic;
            (diagnostic.path, diagnostic.line, diagnostic.code)
        })
        .collect()
}

/// `files` with `edits` read back and applied, and the paths written, in order.
fn applied(files: &Files, edits: &[DocumentEdit]) -> (Files, Vec<String>) {
    let snapshot = snapshot(files);
    let mut read_back = ReadBack::default();
    let mut out = files.clone();
    for edit in edits {
        let old = match snapshot.document(&edit.path) {
            Some(index) => ParsedFile {
                contents: snapshot.documents()[index].file.contents.clone(),
                document: snapshot.documents()[index].file.document.clone(),
            },
            None => ParsedFile::default(),
        };
        let new = read_back
            .edit(&old, &edit.splices, &mut WorkCounts::default())
            .unwrap_or_else(|misread| panic!("{} does not read back: {misread:?}", edit.path));
        out.insert(
            edit.path.clone(),
            String::from_utf8(new.contents.into_vec()).unwrap(),
        );
    }
    let order = edits.iter().map(|edit| edit.path.clone()).collect();
    (out, order)
}

/// An item of type `Requirement` with `rows` after its `Status` row and `body`.
fn item(mid: &str, id: &str, rows: &str, body: &str) -> String {
    format!(
        "<a id=\"{mid}\"></a>\n\n### Item {id}\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | {id} |\n| Status | Draft |\n{rows}\n{body}"
    )
}

/// X in `one/a.mara.md` refines Y beside it and Z in `two/z.mara.md`, and mentions itself; Y,
/// Z and W in `three/d.mara.md` link to X, and narrative in `two/z.mara.md` mentions it through
/// a definition that two links share.
fn project() -> Files {
    let x = item(
        X,
        "REQ-X",
        &format!("| Refines | [REQ-Y](#{Y}) |\n| Refines | [REQ-Z](../two/z.mara.md#{Z}) |\n"),
        &format!(
            "X mentions [itself](#{X}), [REQ-Y](#{Y}) and [Z][z].\n\n[z]: ../two/z.mara.md#{Z}\n\n\
             End of X.\n"
        ),
    );
    let y = item(
        Y,
        "REQ-Y",
        &format!("| Refines | [REQ-X](#{X}) |\n"),
        &format!("Y mentions [REQ-X](#{X}).\n"),
    );
    let z = item(
        Z,
        "REQ-Z",
        "",
        &format!("Z mentions [it](../one/a.mara.md#{X}).\n"),
    );
    let w = item(
        W,
        "REQ-W",
        &format!("| Refines | [REQ-X](../one/a.mara.md#{X}) |\n"),
        "W.\n",
    );
    [
        ("one/a.mara.md", format!("# A\n\n---\n\n{x}\n---\n\n{y}")),
        (
            "two/z.mara.md",
            format!("# Z\n\nSee [x][r] and [REQ-X][r].\n\n[r]: ../one/a.mara.md#{X}\n\n---\n\n{z}"),
        ),
        ("three/d.mara.md", format!("# D\n\n---\n\n{w}")),
    ]
    .into_iter()
    .map(|(path, text)| (path.to_owned(), text))
    .collect()
}

#[test]
fn moves_an_item_to_another_directory_with_every_link() {
    let files = project();
    assert_eq!(problems(&files), []);
    let moved = move_item(&snapshot(&files), "REQ-X", "three/d.mara.md", None).unwrap();
    let (after, order) = applied(&files, &moved.edits);
    assert_eq!(order, ["three/d.mara.md", "one/a.mara.md", "two/z.mara.md"]);
    assert_eq!(problems(&after), []);
    let destination = &after["three/d.mara.md"];
    // The item's bytes, with its own links naming the same files from the destination.
    let expected = item(
        X,
        "REQ-X",
        &format!(
            "| Refines | [REQ-Y](../one/a.mara.md#{Y}) |\n| Refines | [REQ-Z](../two/z.mara.md#{Z}) |\n"
        ),
        &format!(
            "X mentions [itself](#{X}), [REQ-Y](../one/a.mara.md#{Y}) and [Z][z].\n\n\
             [z]: ../two/z.mara.md#{Z}\n\nEnd of X."
        ),
    );
    assert!(destination.contains(&expected), "{destination}");
    assert!(destination.contains(&format!("| Refines | [REQ-X](#{X}) |")));
    assert!(
        after["one/a.mara.md"].contains(&format!("| Refines | [REQ-X](../three/d.mara.md#{X}) |"))
    );
    assert!(
        after["one/a.mara.md"].contains(&format!("Y mentions [REQ-X](../three/d.mara.md#{X})."))
    );
    assert!(!after["one/a.mara.md"].contains(&format!("<a id=\"{X}\">")));
    let narrative = &after["two/z.mara.md"];
    assert!(narrative.contains(&format!(
        "See [x][r] and [REQ-X][r].\n\n[r]: ../three/d.mara.md#{X}\n"
    )));
    assert!(narrative.contains(&format!("Z mentions [it](../three/d.mara.md#{X}).")));
}

#[test]
fn moves_an_item_into_a_new_document_and_within_its_own() {
    let files = project();
    let moved = move_item(&snapshot(&files), X, "four/new.mara.md", None).unwrap();
    let (after, order) = applied(&files, &moved.edits);
    assert_eq!(order[0], "four/new.mara.md");
    assert_eq!(problems(&after), []);
    assert!(after["four/new.mara.md"].starts_with(&format!("<a id=\"{X}\"></a>\n")));

    // Before the heading of A, then back after Y.
    let moved = move_item(&snapshot(&files), "REQ-X", "one/a.mara.md", Some(1)).unwrap();
    let (first, order) = applied(&files, &moved.edits);
    assert_eq!(order, ["one/a.mara.md"]);
    assert_eq!(problems(&first), []);
    let text = &first["one/a.mara.md"];
    assert!(text.starts_with(&format!("<a id=\"{X}\"></a>\n")), "{text}");
    assert!(text.find(&format!("<a id=\"{Y}\">")) > text.find("# A"));
    let moved = move_item(&snapshot(&first), "REQ-X", "one/a.mara.md", None).unwrap();
    let (second, _) = applied(&first, &moved.edits);
    assert_eq!(problems(&second), []);
    let text = &second["one/a.mara.md"];
    assert!(text.starts_with("# A\n\n"), "{text}");
    assert!(text.find(&format!("<a id=\"{X}\">")) > text.find(&format!("<a id=\"{Y}\">")));
}

#[test]
fn writes_nothing_to_move_an_item_where_it_is() {
    let files = project();
    // Lines 2 to 4 hold the blank lines and separator before X, whose anchor starts line 5.
    for line in [2, 3, 4, 5] {
        let moved = move_item(&snapshot(&files), "REQ-X", "one/a.mara.md", Some(line)).unwrap();
        assert_eq!(moved.edits, [], "line {line}");
    }
    let moved = move_item(&snapshot(&files), "REQ-Y", "one/a.mara.md", None).unwrap();
    assert_eq!(moved.edits, []);
}

#[test]
fn completes_a_move_that_left_the_item_in_both_documents() {
    let files = project();
    let moved = move_item(&snapshot(&files), "REQ-X", "three/d.mara.md", None).unwrap();
    let complete = applied(&files, &moved.edits).0;
    // Stopped after the destination: X is in both documents.
    let (stopped, _) = applied(&files, &moved.edits[..1]);
    assert!(
        problems(&stopped)
            .iter()
            .any(|(_, _, code)| *code == "duplicate-mid")
    );
    let rerun = move_item(&snapshot(&stopped), "REQ-X", "three/d.mara.md", None).unwrap();
    let (after, order) = applied(&stopped, &rerun.edits);
    assert_eq!(order, ["one/a.mara.md", "two/z.mara.md"]);
    assert_eq!(after, complete);
    // A copy that differs from what the move writes is no interrupted move's.
    let mut changed = stopped.clone();
    let text = changed["three/d.mara.md"].replace("X mentions", "X notes");
    changed.insert("three/d.mara.md".to_owned(), text);
    let error = move_item(&snapshot(&changed), X, "three/d.mara.md", None).unwrap_err();
    assert_eq!(error.code, "ambiguous-item");
}

#[test]
fn refuses_a_line_inside_an_item_or_past_the_end() {
    let files = project();
    let lines = files["three/d.mara.md"].lines().count();
    for (line, expected) in [(7, "inside the item"), (lines + 2, "past its end")] {
        let error =
            move_item(&snapshot(&files), "REQ-X", "three/d.mara.md", Some(line)).unwrap_err();
        assert_eq!(error.code, INVALID_POSITION);
        assert!(error.message.contains(expected), "{}", error.message);
    }
    let moved = move_item(
        &snapshot(&files),
        "REQ-X",
        "three/d.mara.md",
        Some(lines + 1),
    )
    .unwrap();
    assert_eq!(problems(&applied(&files, &moved.edits).0), []);
    let error = move_item(&snapshot(&files), "REQ-NONE", "three/d.mara.md", None).unwrap_err();
    assert_eq!(error.code, "unknown-item");
}

#[test]
fn renames_the_id_row_mention_text_and_relation_link_text() {
    let mut files = project();
    let z = files["two/z.mara.md"].replace("Z mentions [it]", "Z mentions [**REQ-X**]");
    files.insert("two/z.mara.md".to_owned(), z);
    let renamed = rename(&snapshot(&files), "REQ-X", "REQ-RENAMED").unwrap();
    assert_eq!(renamed.old_id, "REQ-X");
    let (after, order) = applied(&files, &renamed.edits);
    // Mention text first, then the `ID` row, then relation link text only.
    assert_eq!(order, ["two/z.mara.md", "one/a.mara.md", "three/d.mara.md"]);
    assert_eq!(problems(&after), []);
    let z = &after["two/z.mara.md"];
    assert!(z.contains("See [x][r] and [REQ-RENAMED][r]."), "{z}");
    assert!(z.contains("Z mentions [**REQ-RENAMED**]"));
    let a = &after["one/a.mara.md"];
    assert!(a.contains("| ID | REQ-RENAMED |"));
    assert!(a.contains(&format!("| Refines | [REQ-RENAMED](#{X}) |")));
    assert!(a.contains(&format!("Y mentions [REQ-RENAMED](#{X}).")));
    // Other mention text stays.
    assert!(a.contains(&format!("X mentions [itself](#{X})")));
    assert!(after["three/d.mara.md"].contains(&format!("[REQ-RENAMED](../one/a.mara.md#{X})")));
    // Nothing else changed.
    let undone: Files = after
        .iter()
        .map(|(path, text)| (path.clone(), text.replace("REQ-RENAMED", "REQ-X")))
        .collect();
    assert_eq!(undone, files);
}

#[test]
fn renames_a_shortcut_reference_through_its_label() {
    let mut files = project();
    let z = files["two/z.mara.md"].replace("See [x][r]", "See [REQ-X]");
    let z = z.replace(
        &format!("[r]: ../one/a.mara.md#{X}"),
        &format!("[r]: ../one/a.mara.md#{X}\n[REQ-X]: ../one/a.mara.md#{X}"),
    );
    files.insert("two/z.mara.md".to_owned(), z);
    assert_eq!(problems(&files), []);
    let renamed = rename(&snapshot(&files), X, "REQ-NEW").unwrap();
    let (after, _) = applied(&files, &renamed.edits);
    assert!(after["two/z.mara.md"].contains("See [REQ-NEW][REQ-X] and [REQ-NEW][r]."));
    assert_eq!(problems(&after), []);
}

#[test]
fn refuses_a_new_id_that_would_not_be_valid() {
    let files = project();
    for (new, code) in [
        ("req-lower", "invalid-argument"),
        ("NOT-REQ", "wrong-id-prefix"),
        ("REQ-Y", "duplicate-id"),
    ] {
        let error = rename(&snapshot(&files), "REQ-X", new).unwrap_err();
        assert_eq!(error.code, code, "{new}");
    }
    // The same ID rewrites only relation link text that is not already it.
    let mut stale = files.clone();
    let d = stale["three/d.mara.md"].replace("[REQ-X](../one", "[REQ-OLD](../one");
    stale.insert("three/d.mara.md".to_owned(), d);
    let renamed = rename(&snapshot(&stale), "REQ-X", "REQ-X").unwrap();
    let (after, order) = applied(&stale, &renamed.edits);
    assert_eq!(order, ["three/d.mara.md"]);
    assert_eq!(after, files);
}

#[test]
fn refuses_to_move_an_item_whose_links_use_a_definition_outside_it() {
    let files = project();
    // After the item's last paragraph, and at the end of the page, a definition is outside it.
    let trailing = files["one/a.mara.md"].replace("\n\nEnd of X.\n", "\n");
    let at_end = files["one/a.mara.md"].replace(&format!("\n\n[z]: ../two/z.mara.md#{Z}\n"), "\n")
        + &format!("\n[z]: ../two/z.mara.md#{Z}\n");
    for text in [trailing, at_end] {
        let mut changed = files.clone();
        changed.insert("one/a.mara.md".to_owned(), text);
        assert_eq!(problems(&changed), []);
        for destination in ["three/d.mara.md", "one/a.mara.md"] {
            let error = move_item(&snapshot(&changed), "REQ-X", destination, Some(1)).unwrap_err();
            assert_eq!(error.code, UNREADABLE_EDIT);
            assert!(
                error.message.contains("`[Z][z]` in the item `Item REQ-X`"),
                "{}",
                error.message
            );
            assert_eq!(error.diagnostics[0].line, 16);
        }
    }
}
