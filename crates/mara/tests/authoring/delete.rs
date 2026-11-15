//! VER-DELETE-ITEM: `item delete` removes only an item that nothing else targets.

use serde_json::Value;

use crate::{Project, document, item, line_of, link, mid};

#[test]
fn removes_an_item_that_only_it_targets() {
    let project = Project::new();
    let need = link("NEED-X", "needs.mara.md", &mid(3));
    let deleted = item(
        &mid(1),
        "Requirement",
        "REQ-GONE",
        "Gone",
        &[("Satisfies", &need)],
        &format!("It mentions {} itself.", link("this item", "", &mid(1))),
    );
    let kept = item(&mid(2), "Requirement", "REQ-KEPT", "Kept", &[], "Stays.");
    project.write(
        "docs/reqs.mara.md",
        &document("Requirements", &[deleted, kept.clone()]),
    );
    let needs = document(
        "Needs",
        &[item(&mid(3), "Need", "NEED-X", "Need", &[], "Need.")],
    );
    project.write("docs/needs.mara.md", &needs);
    let run = project.run(&["item", "delete", "REQ-GONE"]).ok();
    assert_eq!(run.json()["item"]["id"], "REQ-GONE");
    assert_eq!(run.json()["item"]["line"], 5);
    assert_eq!(
        project.read("docs/reqs.mara.md"),
        document("Requirements", &[kept])
    );
    assert_eq!(project.read("docs/needs.mara.md"), needs);
    for name in ["REQ-GONE".to_owned(), mid(1)] {
        let run = project.run(&["get", &name]);
        assert_eq!(run.json()["error"]["code"], "unknown-item", "{run:#?}");
    }
    project.valid();
}

#[test]
fn refuses_an_item_that_other_text_targets_and_lists_each_place() {
    let project = Project::new();
    let target = item(
        &mid(1),
        "Requirement",
        "REQ-TARGET",
        "Target",
        &[],
        "Targeted.",
    );
    project.write("docs/reqs.mara.md", &document("Requirements", &[target]));
    let stale = link("REQ-TARGET", "old/place.mara.md", &mid(1));
    let mention = link("the target", "../reqs.mara.md", &mid(1));
    let other = item(
        &mid(2),
        "Requirement",
        "REQ-OTHER",
        "Other",
        &[("Refines", &stale)],
        &format!("See {mention}."),
    );
    let sub = document("Sub", &[other]);
    project.write("docs/sub/other.mara.md", &sub);
    let notes = format!(
        "# Notes\n\nNarrative that mentions {}.\n",
        link("it", "reqs.mara.md", &mid(1))
    );
    project.write("docs/notes.mara.md", &notes);
    let tree = project.tree();
    let run = project
        .run(&["item", "delete", "REQ-TARGET"])
        .refused("targeted-item");
    project.unchanged(&tree, "a refused delete");
    let places: Vec<(String, u64)> = run.json()["error"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|diagnostic: &Value| {
            (
                diagnostic["path"].as_str().unwrap().to_owned(),
                diagnostic["line"].as_u64().unwrap(),
            )
        })
        .collect();
    let at = |path: &str, text: &str, start: &str| (path.to_owned(), line_of(text, start) as u64);
    assert_eq!(
        places,
        [
            at("docs/notes.mara.md", &notes, "Narrative"),
            at("docs/sub/other.mara.md", &sub, "| Refines"),
            at("docs/sub/other.mara.md", &sub, "See "),
        ]
    );
    let message = run.message();
    for (path, line) in &places {
        assert!(message.contains(&format!("{path}:{line}")), "{message}");
    }
}
