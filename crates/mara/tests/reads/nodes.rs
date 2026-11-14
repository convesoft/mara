//! VER-NODE-READS, the `get` case of VER-INVALID-ROWS and of VER-MID-RESOLUTION.

use std::fs;

use serde_json::Value;

use crate::{Project, document, item, line_of, link, mid};

fn get(project: &Project, node: &str) -> Value {
    project.run(&["get", node, "--format", "json"]).ok().json()
}

/// `docs/a.mara.md`: narrative, then REQ-ALPHA, which refines REQ-LINKED in `docs/b.mara.md`.
fn example() -> Project {
    let project = Project::new();
    project.write(
        "docs/a.mara.md",
        &document(&[
            "# Overview\n\nNarrative text.\n".to_owned(),
            item(
                &mid(1),
                "Requirement",
                "REQ-ALPHA",
                "Alpha",
                &[
                    ("Priority", "High"),
                    ("Refines", &link("REQ-LINKED", "b.mara.md", &mid(2))),
                ],
                "Alpha body, first line.\nSecond line.",
            ),
            "Closing narrative.\n".to_owned(),
        ]),
    );
    project.write(
        "docs/b.mara.md",
        &item(
            &mid(2),
            "Requirement",
            "REQ-LINKED",
            "Linked title",
            &[],
            "Linked body text.",
        ),
    );
    project
}

#[test]
fn by_id_and_by_mid_alike_without_neighbours() {
    let project = example();
    let by_id = get(&project, "REQ-ALPHA");
    assert_eq!(get(&project, &mid(1)), by_id);
    let node = &by_id["node"];
    assert_eq!(node["kind"], "item");
    assert_eq!(
        (
            node["path"].as_str(),
            node["id"].as_str(),
            node["title"].as_str()
        ),
        (Some("docs/a.mara.md"), Some("REQ-ALPHA"), Some("Alpha"))
    );
    assert_eq!(
        node["line"],
        line_of(&project, "docs/a.mara.md", "<a id=") as u64
    );
    assert_eq!(node["revision"].as_str().map(str::len), Some(64));
    let rows: Vec<(&str, &str)> = by_id["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| (row["key"].as_str().unwrap(), row["value"].as_str().unwrap()))
        .collect();
    let refines = link("REQ-LINKED", "b.mara.md", &mid(2));
    assert_eq!(
        rows,
        [
            ("Type", "Requirement"),
            ("ID", "REQ-ALPHA"),
            ("Status", "Draft"),
            ("Priority", "High"),
            ("Refines", refines.as_str())
        ]
    );
    assert_eq!(by_id["content"], "Alpha body, first line.\nSecond line.");
    assert_eq!(by_id["next_cursor"], Value::Null);
    let text = by_id.to_string();
    assert!(
        !text.contains("Linked title") && !text.contains("Linked body"),
        "{text}"
    );
}

#[test]
fn a_line_names_the_item_or_unit_that_holds_it_and_a_path_the_document() {
    let project = example();
    let body = line_of(&project, "docs/a.mara.md", "Second line.");
    assert_eq!(
        get(&project, &format!("docs/a.mara.md:{body}"))["node"]["id"],
        "REQ-ALPHA"
    );
    let narrative = get(&project, "docs/a.mara.md:3");
    assert_eq!(narrative["node"]["kind"], "narrative");
    assert_eq!(narrative["node"]["line"], 1);
    assert_eq!(narrative["content"], "# Overview\n\nNarrative text.");
    assert_eq!(narrative["rows"], Value::Array(Vec::new()));
    let whole = get(&project, "docs/a.mara.md");
    assert_eq!(whole["node"]["kind"], "document");
    assert_eq!(
        whole["content"].as_str(),
        Some(project.read("docs/a.mara.md").as_str())
    );
    // A line between nodes, past the end, or of another file names nothing.
    let separator = line_of(&project, "docs/a.mara.md", "---");
    for (missing, code) in [
        (format!("docs/a.mara.md:{separator}"), "unknown-node"),
        ("docs/a.mara.md:0".to_owned(), "unknown-node"),
        ("docs/a.mara.md:999".to_owned(), "unknown-node"),
        ("docs/none.mara.md".to_owned(), "unknown-node"),
        ("REQ-NONE".to_owned(), "unknown-item"),
    ] {
        let run = project.run(&["get", &missing]).status(1);
        assert!(
            run.stderr.contains(&format!("error[{code}]")),
            "{missing}: {}",
            run.stderr
        );
        assert_eq!(run.stdout, "");
        let json = project
            .run(&["get", &missing, "--format", "json"])
            .status(1)
            .json();
        assert_eq!(json["error"]["code"], code, "{missing}");
    }
}

#[test]
fn the_revision_changes_only_with_the_item() {
    let project = example();
    let revision = |project: &Project| get(project, "REQ-ALPHA")["node"]["revision"].clone();
    let before = revision(&project);
    let text = project.read("docs/a.mara.md");
    project.write(
        "docs/a.mara.md",
        &text.replace("Closing narrative.", "Closing narrative, edited."),
    );
    assert_eq!(revision(&project), before, "an edit elsewhere in the file");
    let text = project.read("docs/a.mara.md");
    project.write(
        "docs/a.mara.md",
        &text.replace("Second line.", "Second line, edited."),
    );
    assert_ne!(revision(&project), before, "an edit inside the item");
}

#[test]
fn an_item_with_invalid_rows_reads_with_all_of_them() {
    let project = Project::new();
    project.write(
        "docs/a.mara.md",
        &item(
            &mid(1),
            "Requirement",
            "REQ-ROWS",
            "Rows",
            &[
                ("Colour", "Red"),
                ("Priority", "Urgent"),
                ("Owner", "Ann"),
                ("Owner", "Ann"),
            ],
            "Body.",
        ),
    );
    let read = get(&project, "REQ-ROWS");
    let rows: Vec<String> = read["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            format!(
                "{}={}@{}",
                row["key"].as_str().unwrap(),
                row["value"].as_str().unwrap(),
                row["line"]
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            "Type=Requirement@5",
            "ID=REQ-ROWS@7",
            "Status=Draft@8",
            "Colour=Red@9",
            "Priority=Urgent@10",
            "Owner=Ann@11",
            "Owner=Ann@12"
        ]
    );
}

#[test]
fn a_hand_edited_item_is_found_by_its_mid() {
    let project = Project::new();
    project.write(
        "docs/a.mara.md",
        &item(&mid(1), "Requirement", "REQ-OLD", "Old title", &[], "Body."),
    );
    project.write(
        "docs/b.mara.md",
        &item(
            &mid(2),
            "Requirement",
            "REQ-LINKER",
            "Linker",
            &[("Refines", &link("REQ-OLD", "a.mara.md", &mid(1)))],
            &format!("Mentions {}.", link("the old one", "a.mara.md", &mid(1))),
        ),
    );
    let before = project
        .run(&["relation", "list", "REQ-OLD", "--format", "json"])
        .ok()
        .json();
    // Retitle, rename and move the item by hand, leaving the links as they are.
    fs::remove_file(project.path("docs/a.mara.md")).unwrap();
    project.write(
        "docs/moved/c.mara.md",
        &format!(
            "# Moved\n\n---\n\n{}",
            item(&mid(1), "Requirement", "REQ-NEW", "New title", &[], "Body.")
        ),
    );
    let read = get(&project, &mid(1));
    let node = &read["node"];
    assert_eq!(
        (
            node["id"].as_str(),
            node["title"].as_str(),
            node["path"].as_str(),
            node["line"].as_u64()
        ),
        (
            Some("REQ-NEW"),
            Some("New title"),
            Some("docs/moved/c.mara.md"),
            Some(5)
        )
    );
    let after = project
        .run(&["relation", "list", &mid(1), "--format", "json"])
        .ok()
        .json();
    assert_eq!(
        after["connections"], before["connections"],
        "the same links at the same places"
    );
    let sources: Vec<String> = after["connections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            format!(
                "{} {}:{}",
                c["kind"].as_str().unwrap(),
                c["source"]["path"].as_str().unwrap(),
                c["source"]["line"]
            )
        })
        .collect();
    assert_eq!(
        sources,
        ["relation docs/b.mara.md:9", "mention docs/b.mara.md:11"]
    );
}
