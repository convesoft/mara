//! VER-ONE-HOP.

use serde_json::Value;

use crate::{CONFIG, Project, SCHEMA, document, item, line_of, link, mid};

/// One connection as `<kind> <relation> <direction> <neighbour> @<source path>:<line>`.
fn describe(connection: &Value) -> String {
    let neighbour = &connection["neighbour"];
    let neighbour = match neighbour["id"].as_str() {
        Some(id) => id.to_owned(),
        None => format!(
            "{}:{}",
            neighbour["path"].as_str().unwrap_or("?"),
            neighbour["line"]
        ),
    };
    format!(
        "{} {} {} {} @{}:{}",
        connection["kind"].as_str().unwrap(),
        connection["relation"].as_str().unwrap_or("-"),
        connection["direction"].as_str().unwrap(),
        neighbour,
        connection["source"]["path"].as_str().unwrap(),
        connection["source"]["line"]
    )
}

fn connections(project: &Project, args: &[&str]) -> Vec<String> {
    let mut all = vec!["relation", "list"];
    all.extend_from_slice(args);
    all.extend_from_slice(&["--format", "json"]);
    let page = project.run(&all).ok().json();
    page["connections"]
        .as_array()
        .unwrap()
        .iter()
        .map(describe)
        .collect()
}

/// A has two outgoing relations, an incoming one from B, mentions to and from it and a code link;
/// B has a relation of its own.
fn example() -> Project {
    let project = Project::new();
    let code = "\n[code]\nformat_version = 1\n\n[code.languages.rust]\n\
                include = [\"src/**/*.rs\"]\ncomment_prefixes = [\"//\"]\n";
    project.write(".mara/config.toml", &format!("{CONFIG}{code}"));
    let implements = "\n[relations.implements]\ninverse = \"implemented_by\"\n\
                      description = \"Code realizes the target.\"\n\
                      targets = { code = [\"requirement\"] }\n";
    project.write(".mara/schema.toml", &format!("{SCHEMA}{implements}"));
    project.write(
        "src/a.rs",
        "fn a() {}\n// @mara implements REQ-A\nfn b() {}\n",
    );
    project.write(
        "docs/a.mara.md",
        &document(&[
            item(&mid(1), "Need", "NEED-ONE", "One", &[], "Need."),
            item(&mid(2), "Need", "NEED-TWO", "Two", &[], "Need."),
            item(
                &mid(3),
                "Requirement",
                "REQ-A",
                "A",
                &[
                    ("Satisfies", &link("NEED-ONE", "", &mid(1))),
                    ("Satisfies", &link("NEED-TWO", "", &mid(2))),
                ],
                &format!("A mentions {}.", link("C", "b.mara.md", &mid(5))),
            ),
            "## Narrative\n\nThe narrative mentions [A](#MID).\n".replace("MID", &mid(3)),
        ]),
    );
    project.write(
        "docs/b.mara.md",
        &document(&[
            item(
                &mid(4),
                "Requirement",
                "REQ-B",
                "B",
                &[
                    ("Refines", &link("REQ-A", "a.mara.md", &mid(3))),
                    ("Satisfies", &link("NEED-THREE", "", &mid(6))),
                ],
                "B body.",
            ),
            item(
                &mid(5),
                "Requirement",
                "REQ-C",
                "C",
                &[],
                &format!("C mentions {}.", link("A", "a.mara.md", &mid(3))),
            ),
            item(&mid(6), "Need", "NEED-THREE", "Three", &[], "Need."),
        ]),
    );
    project
}

#[test]
fn lists_one_hop_with_source_locations() {
    let project = example();
    let at = |path: &str, start: &str| format!("{path}:{}", line_of(&project, path, start));
    let narrative = line_of(&project, "docs/a.mara.md", "## Narrative");
    assert_eq!(
        connections(&project, &["REQ-A"]),
        [
            format!(
                "relation satisfies outgoing NEED-ONE @{}",
                at("docs/a.mara.md", "| Satisfies | [NEED-ONE]")
            ),
            format!(
                "relation satisfies outgoing NEED-TWO @{}",
                at("docs/a.mara.md", "| Satisfies | [NEED-TWO]")
            ),
            format!(
                "mention - outgoing REQ-C @{}",
                at("docs/a.mara.md", "A mentions")
            ),
            format!(
                "mention - incoming docs/a.mara.md:{narrative} @{}",
                at("docs/a.mara.md", "The narrative mentions")
            ),
            format!(
                "relation refined_by incoming REQ-B @{}",
                at("docs/b.mara.md", "| Refines")
            ),
            format!(
                "mention - incoming REQ-C @{}",
                at("docs/b.mara.md", "C mentions")
            ),
            "relation implemented_by incoming src/a.rs:2 @src/a.rs:2".to_owned(),
        ]
    );
    // B's other relations are not A's neighbours'.
    assert_eq!(connections(&project, &["src/a.rs:2"]).len(), 1);
}

#[test]
fn narrows_by_relation_name() {
    let project = example();
    let only = connections(&project, &["REQ-A", "--relation", "satisfies"]);
    assert_eq!(only.len(), 2);
    assert!(
        only.iter()
            .all(|c| c.starts_with("relation satisfies outgoing")),
        "{only:?}"
    );
    let inverse = connections(&project, &["REQ-A", "--relation", "Refined by"]);
    assert_eq!(inverse.len(), 1);
    assert!(inverse[0].starts_with("relation refined_by incoming REQ-B"));
    assert_eq!(
        connections(&project, &["REQ-A", "--relation", "refines"]),
        [] as [&str; 0]
    );
}

#[test]
fn a_narrative_unit_lists_its_mentions() {
    let project = example();
    let unit = line_of(&project, "docs/a.mara.md", "## Narrative");
    let mention = line_of(&project, "docs/a.mara.md", "The narrative mentions");
    assert_eq!(
        connections(&project, &[&format!("docs/a.mara.md:{mention}")]),
        [format!(
            "mention - outgoing REQ-A @docs/a.mara.md:{mention}"
        )]
    );
    let page = project
        .run(&[
            "relation",
            "list",
            &format!("docs/a.mara.md:{unit}"),
            "--format",
            "json",
        ])
        .ok()
        .json();
    assert_eq!(page["node"]["kind"], "narrative");
}

#[test]
fn every_kind_of_node_that_get_reads_lists_its_connections() {
    let project = example();
    // A whole document: every relation and mention written in it, in source order.
    let at = |start: &str| line_of(&project, "docs/b.mara.md", start);
    assert_eq!(
        connections(&project, &["docs/b.mara.md"]),
        [
            format!(
                "relation refines outgoing REQ-A @docs/b.mara.md:{}",
                at("| Refines")
            ),
            format!(
                "relation satisfies outgoing NEED-THREE @docs/b.mara.md:{}",
                at("| Satisfies")
            ),
            format!(
                "mention - outgoing REQ-A @docs/b.mara.md:{}",
                at("C mentions")
            ),
        ]
    );
    let page = project
        .run(&["relation", "list", "docs/b.mara.md", "--format", "json"])
        .ok()
        .json();
    assert_eq!(
        page["node"],
        serde_json::json!({"kind": "document", "path": "docs/b.mara.md"})
    );
    // `get` and `relation list` take the same nodes.
    let unit = format!(
        "docs/a.mara.md:{}",
        line_of(&project, "docs/a.mara.md", "## Narrative")
    );
    for node in ["REQ-A", unit.as_str(), "docs/a.mara.md"] {
        let kind =
            project.run(&["get", node, "--format", "json"]).ok().json()["node"]["kind"].clone();
        let listed = project
            .run(&["relation", "list", node, "--format", "json"])
            .ok()
            .json();
        assert_eq!(listed["node"]["kind"], kind, "{node}");
    }
    assert_eq!(
        connections(&project, &["docs/b.mara.md", "--relation", "satisfies"]).len(),
        1
    );
}
