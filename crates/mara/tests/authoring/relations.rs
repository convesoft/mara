//! VER-EDIT-RELATIONS: `relation add` and `relation remove` write and delete one row.

use serde_json::Value;

use crate::{Project, document, item, link, mid};

const REQS: &str = "docs/reqs.mara.md";
const NEEDS: &str = "docs/area/sub/needs.mara.md";

/// A requirement, a need and a goal in one file, and a need in another directory.
fn project() -> (Project, String, String) {
    let project = Project::new();
    let reqs = document(
        "Requirements",
        &[
            item(
                &mid(1),
                "Requirement",
                "REQ-SOURCE",
                "Source",
                &[("Priority", "High")],
                "Source.",
            ),
            item(&mid(2), "Need", "NEED-HERE", "Here", &[], "Here."),
            item(&mid(3), "Goal", "GOAL-G", "Goal", &[], "Goal."),
        ],
    );
    let needs = document(
        "Needs",
        &[item(&mid(4), "Need", "NEED-THERE", "There", &[], "There.")],
    );
    project.write(REQS, &reqs);
    project.write(NEEDS, &needs);
    (project, reqs, needs)
}

/// The connections of `relation list <node>` that target it under `relation`.
fn incoming(project: &Project, node: &str, relation: &str) -> Vec<Value> {
    let run = project.run(&["relation", "list", node]).ok();
    run.json()["connections"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["direction"] == "incoming" && c["relation"] == relation)
        .cloned()
        .collect()
}

#[test]
fn adds_one_canonical_row_to_an_item_in_the_same_file_and_in_another_directory() {
    let (project, reqs, needs) = project();
    let run = project
        .run(&["relation", "add", "REQ-SOURCE", "satisfies", "NEED-HERE"])
        .ok();
    let same = format!("| Satisfies | {} |", link("NEED-HERE", "", &mid(2)));
    let expected = reqs.replacen(
        "| Priority | High |",
        &format!("| Priority | High |\n{same}"),
        1,
    );
    assert_eq!(project.read(REQS), expected);
    assert_eq!(run.json()["relation"], "satisfies");
    assert_eq!(run.json()["target"]["id"], "NEED-HERE");
    assert_eq!(run.json()["line"], 14);
    // The row comes before the need, which moves down one line.
    assert_eq!(run.json()["target"]["line"], 20);
    let run = project
        .run(&["relation", "add", "REQ-SOURCE", "Satisfies", "NEED-THERE"])
        .ok();
    let other = format!(
        "| Satisfies | {} |",
        link("NEED-THERE", "area/sub/needs.mara.md", &mid(4))
    );
    let expected = expected.replacen(&same, &format!("{same}\n{other}"), 1);
    assert_eq!(project.read(REQS), expected);
    assert_eq!(project.read(NEEDS), needs);
    assert_eq!(run.json()["line"], 15);
    for (need, path) in [("NEED-HERE", REQS), ("NEED-THERE", REQS)] {
        let found = incoming(&project, need, "satisfied_by");
        assert_eq!(found.len(), 1, "{need}: {found:?}");
        assert_eq!(found[0]["neighbour"]["id"], "REQ-SOURCE");
        assert_eq!(found[0]["source"]["path"], path);
    }
    project.valid();
}

#[test]
fn refuses_a_relation_the_schema_does_not_allow() {
    let (project, ..) = project();
    let tree = project.tree();
    for (source, relation, target, code) in [
        ("REQ-SOURCE", "satisfies", "GOAL-G", "invalid-item"),
        ("NEED-HERE", "satisfies", "REQ-SOURCE", "invalid-item"),
        ("REQ-SOURCE", "refines", "NEED-HERE", "invalid-item"),
        (
            "REQ-SOURCE",
            "threatens",
            "NEED-HERE",
            "unknown-declaration",
        ),
        ("REQ-NONE", "satisfies", "NEED-HERE", "unknown-item"),
    ] {
        project
            .run(&["relation", "add", source, relation, target])
            .refused(code);
        project.unchanged(&tree, &format!("relation add {source} {relation} {target}"));
    }
    project
        .run(&["relation", "add", "REQ-SOURCE", "satisfies", "NEED-HERE"])
        .ok();
    let tree = project.tree();
    project
        .run(&["relation", "add", "REQ-SOURCE", "satisfies", "NEED-HERE"])
        .refused("invalid-item");
    project.unchanged(&tree, "a relation added twice");
}

#[test]
fn refuses_an_inverse_name_and_names_the_forward_request() {
    let (project, ..) = project();
    project
        .run(&["relation", "add", "REQ-SOURCE", "satisfies", "NEED-HERE"])
        .ok();
    let tree = project.tree();
    for verb in ["add", "remove"] {
        let run = project
            .run(&["relation", verb, "NEED-HERE", "satisfied_by", "REQ-SOURCE"])
            .refused("inverse-relation");
        let forward = format!("mara relation {verb} REQ-SOURCE satisfies NEED-HERE");
        assert!(run.message().contains(&forward), "{}", run.message());
        project.unchanged(&tree, verb);
        project
            .run(&["relation", verb, "NEED-HERE", "Satisfied by", "REQ-SOURCE"])
            .refused("inverse-relation");
        project.unchanged(&tree, verb);
    }
}

#[test]
fn removes_that_row_and_nothing_else() {
    let (project, reqs, needs) = project();
    let here = format!("| Satisfies | {} |\n", link("NEED-HERE", "", &mid(2)));
    let there = format!(
        "|Satisfies|{}|\n",
        link("NEED-THERE", "area/sub/needs.mara.md", &mid(4))
    );
    let with = reqs.replacen(
        "| Priority | High |\n",
        &format!("{here}| Priority | High |\n{there}"),
        1,
    );
    project.write(REQS, &with);
    let run = project
        .run(&[
            "relation",
            "remove",
            "REQ-SOURCE",
            "satisfies",
            "NEED-THERE",
        ])
        .ok();
    assert_eq!(run.json()["line"], 15);
    let expected = with.replacen(&there, "", 1);
    assert_eq!(project.read(REQS), expected);
    project
        .run(&["relation", "remove", "REQ-SOURCE", "satisfies", &mid(2)])
        .ok();
    assert_eq!(project.read(REQS), expected.replacen(&here, "", 1));
    assert_eq!(project.read(REQS), reqs);
    assert_eq!(project.read(NEEDS), needs);
    let tree = project.tree();
    project
        .run(&["relation", "remove", "REQ-SOURCE", "satisfies", "NEED-HERE"])
        .refused("missing-relation");
    project.unchanged(&tree, "a relation removed twice");
    project.valid();
}

#[test]
fn leaves_a_field_alone_that_shares_a_relation_name() {
    let project = Project::empty();
    project.write(".mara/config.toml", crate::CONFIG);
    project.write(
        ".mara/schema.toml",
        r#"format_version = 1
statuses = ["draft"]

[types.note]
id_prefix = "NOTE-"
description = "A note."

[types.note.fields.refines]
kind = "text"
required = true

[types.requirement]
id_prefix = "REQ-"
description = "A requirement."

[relations.refines]
inverse = "refined_by"
description = "Refines."
targets = { requirement = ["requirement"] }
"#,
    );
    let field = link("REQ-TARGET", "", &mid(1));
    project.write(
        REQS,
        &document(
            "Notes",
            &[
                item(
                    &mid(1),
                    "Requirement",
                    "REQ-TARGET",
                    "Target",
                    &[],
                    "Target.",
                ),
                item(
                    &mid(2),
                    "Note",
                    "NOTE-SOURCE",
                    "Source",
                    &[("Refines", &field)],
                    "A text field.",
                ),
            ],
        ),
    );
    project.valid();
    let tree = project.tree();
    for verb in ["remove", "add"] {
        project
            .run(&["relation", verb, "NOTE-SOURCE", "refines", "REQ-TARGET"])
            .refused("invalid-item");
        project.unchanged(&tree, verb);
    }
}
