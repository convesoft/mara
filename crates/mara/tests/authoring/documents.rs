//! The alpha.2 rows of docs/verification-documents.mara.md for the single-file operations: the
//! writer row of VER-GITHUB-RENDERING, the `item update` row of VER-MID-RESOLUTION and the write
//! row of VER-NESTED-PROJECTS.

use std::collections::HashMap;

use crate::{Project, document, item, link, mid, run_in};

/// Independently of Mara: each link's path in `files`, resolved from the linking file as GitHub
/// resolves it, names the file that holds the anchor of its MID. Returns how many links there
/// are.
fn links_resolve(files: &[(String, String)]) -> usize {
    let mut holder: HashMap<String, &str> = HashMap::new();
    for (path, text) in files {
        for (start, _) in text.match_indices("<a id=\"") {
            holder.insert(text[start + 7..start + 33].to_owned(), path);
        }
    }
    let mut links = 0;
    for (path, text) in files {
        for (start, _) in text.match_indices("](") {
            let rest = &text[start + 2..];
            let destination = &rest[..rest.find(')').unwrap()];
            let (relative, mid) = destination.split_once('#').unwrap();
            let mut parts: Vec<&str> = path.split('/').collect();
            if !relative.is_empty() {
                parts.pop();
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
    links
}

#[test]
fn written_items_and_links_resolve_as_github_resolves_them() {
    let project = Project::new();
    let create = |path: &str, kind: &str, title: &str| {
        let mut args = vec![
            "item",
            "create",
            kind,
            path,
            "--title",
            title,
            "--field",
            "status=draft",
            "--body",
            "Text.",
        ];
        if kind == "design" {
            args.extend(["--field", "rationale=Why."]);
        }
        project.run(&args).ok();
    };
    create("docs/a.mara.md", "need", "Here");
    create("docs/a.mara.md", "requirement", "Same file");
    create("docs/area/sub/b.mara.md", "requirement", "Nested");
    create("docs/other/c.mara.md", "design", "Other");
    let relate = |source: &str, relation: &str, target: &str| {
        project
            .run(&["relation", "add", source, relation, target])
            .ok();
    };
    relate("REQ-SAME-FILE", "satisfies", "NEED-HERE");
    relate("REQ-NESTED", "satisfies", "NEED-HERE");
    relate("REQ-NESTED", "refines", "REQ-SAME-FILE");
    relate("DES-OTHER", "satisfies", "REQ-NESTED");
    let files: Vec<(String, String)> = [
        "docs/a.mara.md",
        "docs/area/sub/b.mara.md",
        "docs/other/c.mara.md",
    ]
    .iter()
    .map(|path| ((*path).to_owned(), project.read(path)))
    .collect();
    for (path, text) in &files {
        assert!(!text.starts_with("---"), "{path} starts with ---");
    }
    assert_eq!(links_resolve(&files), 4);
    project.valid();
}

#[test]
fn an_updated_item_resolves_by_its_mid() {
    let project = Project::new();
    let target = item(
        &mid(1),
        "Requirement",
        "REQ-OLD",
        "Old title",
        &[],
        "Target.",
    );
    project.write("docs/a.mara.md", &document("A", &[target]));
    let refines = link("REQ-OLD", "a.mara.md", &mid(1));
    let source = item(
        &mid(2),
        "Requirement",
        "REQ-SOURCE",
        "Source",
        &[("Refines", &refines)],
        &format!("Mentions {}.", link("the target", "a.mara.md", &mid(1))),
    );
    let other = document("B", &[source]);
    project.write("docs/b.mara.md", &other);
    project
        .run(&["item", "update", &mid(1), "--title", "New title"])
        .ok();
    let got = project.run(&["get", &mid(1)]).ok().json();
    assert_eq!(got["node"]["title"], "New title");
    assert_eq!(got["node"]["path"], "docs/a.mara.md");
    assert!(
        project
            .read("docs/a.mara.md")
            .contains(&format!("<a id=\"{}\"></a>", mid(1)))
    );
    let listed = project.run(&["relation", "list", &mid(1)]).ok().json();
    let incoming: Vec<(String, u64)> = listed["connections"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["direction"] == "incoming")
        .map(|c| {
            (
                c["kind"].as_str().unwrap().to_owned(),
                c["source"]["line"].as_u64().unwrap(),
            )
        })
        .collect();
    let line = |start: &str| crate::line_of(&other, start) as u64;
    assert_eq!(
        incoming,
        [
            ("relation".to_owned(), line("| Refines")),
            ("mention".to_owned(), line("Mentions"))
        ]
    );
    assert_eq!(project.read("docs/b.mara.md"), other);
}

#[test]
fn a_write_from_the_outer_project_into_the_inner_one_is_refused() {
    let project = Project::new();
    project.write("docs/outer.mara.md", &document("Outer", &[]));
    project.write("docs/inner/.mara/config.toml", crate::CONFIG);
    project.write("docs/inner/.mara/schema.toml", crate::SCHEMA);
    let inner = document(
        "Inner",
        &[item(
            &mid(1),
            "Requirement",
            "REQ-INNER",
            "Inner",
            &[],
            "Inner.",
        )],
    );
    project.write("docs/inner/docs/inner.mara.md", &inner);
    let tree = project.tree();
    for path in [
        "docs/inner/docs/inner.mara.md",
        "docs/inner/docs/new.mara.md",
    ] {
        let args = [
            "item",
            "create",
            "requirement",
            path,
            "--title",
            "Into inner",
            "--field",
            "status=draft",
            "--body",
            "Text.",
        ];
        run_in(&project.path("docs"), &args).refused("not-an-input");
        project.unchanged(&tree, path);
    }
    for args in [
        vec!["item", "update", "REQ-INNER", "--title", "X"],
        vec!["item", "delete", "REQ-INNER"],
    ] {
        project.run(&args).refused("unknown-item");
        project.unchanged(&tree, &format!("{args:?}"));
    }
}
