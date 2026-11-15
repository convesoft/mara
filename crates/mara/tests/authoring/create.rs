//! VER-CREATE-ITEM: `item create` at each position, with an ID given or proposed.

use mara_engine::document::is_mid;

use crate::{Project, document, item, line_of, mid};

/// `item create` of a requirement into `path` with every part a request supplies, then
/// `extra`.
fn create<'a>(path: &'a str, title: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec![
        "item",
        "create",
        "requirement",
        path,
        "--title",
        title,
        "--field",
        "status=accepted",
        "--field",
        "priority=high",
        "--body",
        "The system *shall* do it.",
    ];
    args.extend(extra);
    args
}

fn two_items() -> String {
    document(
        "Requirements",
        &[
            item(&mid(1), "Requirement", "REQ-ONE", "One", &[], "First."),
            item(&mid(2), "Requirement", "REQ-TWO", "Two", &[], "Second."),
        ],
    ) + "\n---\n\nClosing narrative.\n"
}

#[test]
fn creates_a_complete_item_that_get_returns() {
    let project = Project::new();
    project.write("docs/reqs.mara.md", &two_items());
    for id in [Some("REQ-GIVEN"), None] {
        let mut extra = vec![];
        if let Some(id) = id {
            extra.extend(["--id", id]);
        }
        let title = match id {
            Some(_) => "Given an ID",
            None => "Phishing-resistant auth",
        };
        let run = project
            .run(&create("docs/reqs.mara.md", title, &extra))
            .ok();
        let created = &run.json()["item"];
        let expected = id.unwrap_or("REQ-PHISHING-RESISTANT-AUTH");
        assert_eq!(created["id"], expected, "{run:#?}");
        assert_eq!(created["type"], "requirement");
        assert_eq!(created["title"], title);
        let got = project.run(&["get", expected]).ok().json();
        assert_eq!(got["node"]["mid"], created["mid"]);
        assert_eq!(got["node"]["line"], created["line"]);
        let rows: Vec<(String, String)> = got["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let text = |key: &str| row[key].as_str().unwrap().to_owned();
                (text("key"), text("value"))
            })
            .collect();
        let row = |key: &str, value: &str| (key.to_owned(), value.to_owned());
        assert_eq!(
            rows,
            [
                row("Type", "Requirement"),
                row("ID", expected),
                row("Status", "Accepted"),
                row("Priority", "High")
            ]
        );
        assert!(
            got["content"]
                .as_str()
                .unwrap()
                .contains("The system *shall* do it.")
        );
    }
    project.valid();
}

#[test]
fn ends_the_document_without_a_position() {
    let project = Project::new();
    let before = two_items();
    project.write("docs/reqs.mara.md", &before);
    let run = project.run(&create("docs/reqs.mara.md", "Last", &[])).ok();
    let after = project.read("docs/reqs.mara.md");
    let created = run.json()["item"].clone();
    let mid = created["mid"].as_str().unwrap();
    let expected = format!(
        "{before}\n---\n\n<a id=\"{mid}\"></a>\n\n## Last\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | REQ-LAST |\n| Status | Accepted |\n| Priority | High |\n\nThe system *shall* do it.\n"
    );
    assert_eq!(after, expected);
    let anchor = format!("<a id=\"{mid}\"></a>");
    assert_eq!(created["line"], line_of(&after, &anchor) as u64);
    project.valid();
}

#[test]
fn starts_at_the_line_given_and_merges_with_no_neighbour() {
    let before = two_items();
    let separator = before
        .lines()
        .enumerate()
        .filter(|(_, line)| *line == "---")
        .nth(1)
        .map(|(index, _)| index + 1)
        .unwrap();
    let narrative = line_of(&before, "Closing narrative.");
    for line in [1, narrative, separator] {
        let project = Project::new();
        project.write("docs/reqs.mara.md", &before);
        let at = line.to_string();
        let run = project
            .run(&create("docs/reqs.mara.md", "Inserted", &["--line", &at]))
            .ok();
        let after = project.read("docs/reqs.mara.md");
        let created = run.json()["item"].clone();
        let mid = created["mid"].as_str().unwrap();
        // What was before the line stays before the item, and what was at the line follows it.
        let start: usize = before.lines().take(line - 1).map(|l| l.len() + 1).sum();
        let (head, tail) = before.split_at(start);
        assert!(after.starts_with(head), "line {line}: {after}");
        assert!(after.ends_with(tail), "line {line}: {after}");
        let rest = &after[head.len()..];
        let anchor = format!("<a id=\"{mid}\"></a>");
        // The item starts at the line, after the separator that it needs there when none comes
        // before the line (DES-ITEM-WRITER).
        let anchor_line = match rest.starts_with(&anchor) {
            true => line,
            false => {
                assert!(
                    rest.starts_with(&format!("---\n\n{anchor}")),
                    "line {line}: {rest:?}"
                );
                line + 2
            }
        };
        let expected = if line == separator { line + 2 } else { line };
        assert_eq!(anchor_line, expected, "line {line}");
        assert_eq!(created["line"], anchor_line as u64, "line {line}");
        assert_eq!(line_of(&after, &anchor), anchor_line);
        // Each neighbour is still its own region with its own body.
        for id in ["REQ-ONE", "REQ-TWO", "REQ-INSERTED"] {
            project.run(&["get", id]).ok();
        }
        assert_eq!(
            project.run(&["item", "list"]).ok().json()["items"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        project.valid();
    }
}

#[test]
fn refuses_a_line_inside_an_item_or_past_the_end() {
    let project = Project::new();
    let before = two_items();
    project.write("docs/reqs.mara.md", &before);
    let inside = line_of(&before, "| ID | REQ-ONE |").to_string();
    let past = (before.lines().count() + 2).to_string();
    for line in [inside, past] {
        let tree = project.tree();
        project
            .run(&create("docs/reqs.mara.md", "Nowhere", &["--line", &line]))
            .refused("invalid-position");
        project.unchanged(&tree, &format!("a create at line {line}"));
    }
}

#[test]
fn creates_a_new_document_that_the_patterns_match() {
    let project = Project::new();
    project.write("docs/reqs.mara.md", &two_items());
    let run = project
        .run(&create(
            "docs/new/area/first.mara.md",
            "First of its file",
            &[],
        ))
        .ok();
    let created = project.read("docs/new/area/first.mara.md");
    assert!(created.starts_with("<a id=\""), "{created}");
    assert_eq!(run.json()["item"]["line"], 1);
    assert_eq!(run.json()["files"][0], "docs/new/area/first.mara.md");
    project.run(&["get", "REQ-FIRST-OF-ITS-FILE"]).ok();
    project.valid();
}

#[test]
fn refuses_a_new_document_that_would_be_no_input() {
    let project = Project::new();
    project.write("docs/reqs.mara.md", &two_items());
    project.write(".gitignore", "docs/ignored/\ndocs/skipped.mara.md\n");
    project.write("docs/inner/.mara/config.toml", crate::CONFIG);
    project.write("docs/inner/.mara/schema.toml", crate::SCHEMA);
    for path in [
        "notes/outside.mara.md",
        "docs/plain.md",
        "docs/ignored/new.mara.md",
        "docs/skipped.mara.md",
        "docs/inner/docs/new.mara.md",
        "docs/inner/new.mara.md",
    ] {
        let tree = project.tree();
        project
            .run(&create(path, "Nowhere", &[]))
            .refused("not-an-input");
        project.unchanged(&tree, path);
    }
}

#[test]
fn gives_each_item_a_new_mid() {
    let project = Project::new();
    project.write("docs/reqs.mara.md", &two_items());
    let first = project
        .run(&create("docs/reqs.mara.md", "First new", &[]))
        .ok();
    let second = project
        .run(&create("docs/reqs.mara.md", "Second new", &[]))
        .ok();
    let mids: Vec<String> = [first, second]
        .iter()
        .map(|run| run.json()["item"]["mid"].as_str().unwrap().to_owned())
        .collect();
    let text = project.read("docs/reqs.mara.md");
    for mid in &mids {
        assert!(is_mid(mid), "{mid}");
        assert!(text.contains(&format!("<a id=\"{mid}\"></a>")));
        assert_eq!(project.run(&["get", mid]).ok().json()["node"]["mid"], *mid);
    }
    assert_ne!(mids[0], mids[1]);
    assert!(!mids.contains(&mid(1)) && !mids.contains(&mid(2)));
    project.valid();
}

#[test]
fn rejects_a_mid_option() {
    let project = Project::new();
    project.write("docs/reqs.mara.md", &two_items());
    let tree = project.tree();
    let unused = mid(9);
    project
        .run(&create(
            "docs/reqs.mara.md",
            "With a MID",
            &["--mid", &unused],
        ))
        .rejected();
    project.unchanged(&tree, "a create with a MID");
}

#[test]
fn refuses_what_the_schema_or_the_identity_rules_do_not_allow() {
    let project = Project::new();
    project.write("docs/reqs.mara.md", &two_items());
    let tree = project.tree();
    let base = |kind: &'static str, title: &'static str| {
        vec![
            "item",
            "create",
            kind,
            "docs/reqs.mara.md",
            "--title",
            title,
            "--body",
            "Text.",
        ]
    };
    for (extra, code, kind) in [
        (
            vec!["--field", "status=draft", "--id", "REQ-ONE"],
            "invalid-item",
            "requirement",
        ),
        (
            vec!["--field", "status=draft", "--id", "NEED-X"],
            "invalid-item",
            "requirement",
        ),
        (
            vec!["--field", "status=draft", "--id", "req-lower"],
            "invalid-item",
            "requirement",
        ),
        (
            vec!["--field", "status=done"],
            "invalid-item",
            "requirement",
        ),
        (
            vec!["--field", "status=draft", "--field", "priority=urgent"],
            "invalid-item",
            "requirement",
        ),
        (
            vec![
                "--field",
                "status=draft",
                "--field",
                "priority=high",
                "--field",
                "priority=low",
            ],
            "invalid-item",
            "requirement",
        ),
        (
            vec!["--field", "status=draft", "--field", "colour=red"],
            "invalid-item",
            "requirement",
        ),
        (
            vec!["--field", "status=draft", "--field", "satisfies=NEED-X"],
            "invalid-item",
            "requirement",
        ),
        (vec![], "invalid-item", "requirement"),
        (vec!["--field", "status=draft"], "invalid-item", "design"),
        (
            vec!["--field", "status=draft"],
            "unknown-declaration",
            "risk",
        ),
    ] {
        let mut args = base(kind, "One");
        args.extend(extra.iter().copied());
        project.run(&args).refused(code);
        project.unchanged(&tree, &format!("{args:?}"));
    }
    // A title the writer cannot write, a missing title or body, and a key that is no field.
    for args in [
        vec![
            "item",
            "create",
            "requirement",
            "docs/reqs.mara.md",
            "--title",
            " x",
            "--body",
            "B.",
        ],
        vec![
            "item",
            "create",
            "requirement",
            "docs/reqs.mara.md",
            "--body",
            "B.",
        ],
        vec![
            "item",
            "create",
            "requirement",
            "docs/reqs.mara.md",
            "--title",
            "T",
        ],
        vec![
            "item",
            "create",
            "requirement",
            "docs/reqs.mara.md",
            "--title",
            "T",
            "--body",
            "B.",
            "--field",
            "type=need",
        ],
        vec![
            "item",
            "create",
            "requirement",
            "../outside.mara.md",
            "--title",
            "T",
            "--body",
            "B.",
        ],
    ] {
        project.run(&args).rejected();
        project.unchanged(&tree, &format!("{args:?}"));
    }
}
