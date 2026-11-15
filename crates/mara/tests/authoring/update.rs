//! VER-UPDATE-ITEM: `item update` replaces one part of an item at a time.

use crate::{Project, document, item, link, mid};

const PATH: &str = "docs/reqs.mara.md";

/// An item with a relation, a single-valued field, a repeatable field and a body, and another
/// item in its file.
fn before() -> String {
    let satisfies = link("NEED-X", "", &mid(2));
    document(
        "Requirements",
        &[
            item(
                &mid(1),
                "Requirement",
                "REQ-UPDATED",
                "Original title",
                &[
                    ("Priority", "High"),
                    ("Owner", "Ann"),
                    ("Owner", "Bob"),
                    ("Satisfies", &satisfies),
                ],
                "Original body.\n\nMore *body*.",
            ),
            item(&mid(2), "Need", "NEED-X", "Need", &[], "Need body."),
        ],
    )
}

/// Runs `item update REQ-UPDATED <args>` on a new project and requires the document to be
/// `before` with each `(old, new)` replaced once.
fn updates(args: &[&str], replacements: &[(&str, &str)]) {
    let project = Project::new();
    let before = before();
    project.write(PATH, &before);
    project.write("docs/other.mara.md", "# Other\n\nUntouched.\n");
    let tree = project.tree();
    let mut command = vec!["item", "update", "REQ-UPDATED"];
    command.extend(args);
    let run = project.run(&command).ok();
    let mut expected = before.clone();
    for (old, new) in replacements {
        assert!(expected.contains(old), "{old:?}");
        expected = expected.replacen(old, new, 1);
    }
    assert_eq!(project.read(PATH), expected, "{args:?}");
    assert_eq!(run.json()["item"]["mid"], mid(1));
    assert_eq!(run.json()["files"][0], PATH);
    let mut after = project.tree();
    after.remove(PATH);
    let mut unchanged = tree.clone();
    unchanged.remove(PATH);
    assert_eq!(after, unchanged, "{args:?} changed another file");
    project.valid();
    let got = project.run(&["get", &mid(1)]).ok().json();
    assert_eq!(got["node"]["id"], "REQ-UPDATED");
}

#[test]
fn replaces_only_the_title() {
    updates(
        &["--title", "New *title*"],
        &[("## Original title", "## New *title*")],
    );
}

#[test]
fn replaces_only_the_rows_of_the_fields_given() {
    updates(
        &[
            "--field",
            "status=accepted",
            "--field",
            "Priority=low",
            "--field",
            "owner=Cat",
            "--field",
            "owner=Dan",
            "--field",
            "owner=Eve",
        ],
        &[
            ("| Status | Draft |", "| Status | Accepted |"),
            ("| Priority | High |", "| Priority | Low |"),
            ("| Owner | Ann |", "| Owner | Cat |"),
            ("| Owner | Bob |", "| Owner | Dan |"),
            (
                &format!("| Satisfies | {} |", link("NEED-X", "", &mid(2))),
                &format!(
                    "| Satisfies | {} |\n| Owner | Eve |",
                    link("NEED-X", "", &mid(2))
                ),
            ),
        ],
    );
    updates(
        &["--field", "owner=Cat"],
        &[
            ("| Owner | Ann |", "| Owner | Cat |"),
            ("| Owner | Bob |\n", ""),
        ],
    );
    updates(
        &["--field", "tags=one", "--field", "tags=two"],
        &[(
            &format!("| Satisfies | {} |", link("NEED-X", "", &mid(2))),
            &format!(
                "| Satisfies | {} |\n| Tags | one |\n| Tags | two |",
                link("NEED-X", "", &mid(2))
            ),
        )],
    );
}

#[test]
fn replaces_only_the_body() {
    updates(
        &["--body", "New body\nover two lines."],
        &[(
            "Original body.\n\nMore *body*.",
            "New body\nover two lines.",
        )],
    );
}

#[test]
fn replaces_title_field_and_body_in_one_request() {
    updates(
        &[
            "--title",
            "All three",
            "--field",
            "status=accepted",
            "--body",
            "All new.",
        ],
        &[
            ("## Original title", "## All three"),
            ("| Status | Draft |", "| Status | Accepted |"),
            ("Original body.\n\nMore *body*.", "All new."),
        ],
    );
}

#[test]
fn refuses_a_new_type_id_mid_or_relation() {
    let project = Project::new();
    project.write(PATH, &before());
    let tree = project.tree();
    let unused = mid(9);
    for args in [
        vec!["--field", "type=need"],
        vec!["--field", "ID=REQ-OTHER"],
        vec!["--type", "need"],
        vec!["--id", "REQ-OTHER"],
        vec!["--mid", &unused],
        vec!["--relation", "satisfies"],
        vec![],
    ] {
        let mut command = vec!["item", "update", "REQ-UPDATED"];
        command.extend(&args);
        project.run(&command).rejected();
        project.unchanged(&tree, &format!("{args:?}"));
    }
    for args in [
        vec!["--field", "satisfies=NEED-X"],
        vec!["--field", "satisfied by=NEED-X"],
        vec!["--field", "priority=urgent"],
        vec!["--field", "priority=high", "--field", "priority=low"],
        vec!["--field", "status=done"],
        vec!["--field", "colour=red"],
        vec!["--body", " \n "],
        vec!["--body", "Split\n\n---\n\nin two."],
        vec!["--body", "Text\n---"],
    ] {
        let mut command = vec!["item", "update", "REQ-UPDATED"];
        command.extend(&args);
        let run = project.run(&command);
        assert_eq!(run.status, 1, "{args:?}: {run:#?}");
        project.unchanged(&tree, &format!("{args:?}"));
    }
}

#[test]
fn adds_rows_after_the_last_row_that_stays() {
    let project = Project::new();
    let before = document(
        "Requirements",
        &[item(
            &mid(1),
            "Requirement",
            "REQ-LAST",
            "Last rows",
            &[("Owner", "Ann"), ("Owner", "Bob")],
            "Body.",
        )],
    );
    project.write(PATH, &before);
    project
        .run(&[
            "item",
            "update",
            "REQ-LAST",
            "--field",
            "owner=Ann",
            "--field",
            "tags=one",
        ])
        .ok();
    let expected = before.replacen(
        "| Owner | Ann |\n| Owner | Bob |\n",
        "| Owner | Ann |\n| Tags | one |\n",
        1,
    );
    assert_eq!(project.read(PATH), expected);
    project.valid();
}

#[test]
fn clears_every_row_of_an_optional_field_and_nothing_else() {
    updates(
        &["--clear-field", "Owner"],
        &[("| Owner | Ann |\n| Owner | Bob |\n", "")],
    );
    updates(
        &["--clear-field", "priority", "--field", "owner=Cat"],
        &[
            ("| Priority | High |\n", ""),
            ("| Owner | Ann |", "| Owner | Cat |"),
            ("| Owner | Bob |\n", ""),
        ],
    );
    // A field the item does not have: nothing to write.
    let project = Project::new();
    project.write(PATH, &before());
    let tree = project.tree();
    let run = project
        .run(&["item", "update", "REQ-UPDATED", "--clear-field", "tags"])
        .ok();
    assert_eq!(run.json()["files"], serde_json::json!([]));
    project.unchanged(&tree, "clearing a field the item does not have");
}

#[test]
fn refuses_to_clear_what_every_item_or_its_type_needs() {
    let project = Project::new();
    let design = item(
        &mid(3),
        "Design",
        "DES-D",
        "Design",
        &[("Rationale", "Why.")],
        "Design.",
    );
    project.write(PATH, &format!("{}\n---\n\n{design}", before()));
    let tree = project.tree();
    for args in [
        vec!["REQ-UPDATED", "--clear-field", "status"],
        vec!["REQ-UPDATED", "--clear-field", "Type"],
        vec!["REQ-UPDATED", "--clear-field", "id"],
        vec![
            "REQ-UPDATED",
            "--clear-field",
            "owner",
            "--field",
            "Owner=Cat",
        ],
        vec!["REQ-UPDATED", "--field", "owner="],
    ] {
        let mut command = vec!["item", "update"];
        command.extend(&args);
        project.run(&command).rejected();
        project.unchanged(&tree, &format!("{args:?}"));
    }
    for args in [
        vec!["DES-D", "--clear-field", "rationale"],
        vec!["REQ-UPDATED", "--clear-field", "satisfies"],
        vec!["REQ-UPDATED", "--clear-field", "colour"],
    ] {
        let mut command = vec!["item", "update"];
        command.extend(&args);
        project.run(&command).refused("invalid-item");
        project.unchanged(&tree, &format!("{args:?}"));
    }
}
