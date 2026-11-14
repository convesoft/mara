//! VER-ITEM-LIST: `item list` on a project with items in several documents.

use crate::search::{FILTER_CASES, filter_project};
use crate::{Project, document, item, mid, names};

fn listed(project: &Project, args: &[&str]) -> Vec<String> {
    let mut all = vec!["item", "list", "--format", "json"];
    all.extend_from_slice(args);
    names(&project.run(&all).ok().json()["items"])
}

#[test]
fn lists_every_item_once_in_path_and_source_order() {
    let project = filter_project();
    assert_eq!(
        listed(&project, &[]),
        ["NEED-ONE", "REQ-ONE", "REQ-TWO", "DES-ONE"]
    );
    let page = project
        .run(&["item", "list", "--format", "json"])
        .ok()
        .json();
    let first = &page["items"][0];
    let keys: Vec<&str> = first
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys.len(), 7, "{first}");
    assert_eq!(
        (
            first["kind"].as_str(),
            first["type"].as_str(),
            first["title"].as_str(),
            first["mid"].as_str()
        ),
        (
            Some("item"),
            Some("need"),
            Some("Need one"),
            Some(mid(1).as_str())
        )
    );
}

#[test]
fn a_summary_omits_the_body() {
    let project = Project::new();
    let body = format!("{} uniquelastword", "filler text ".repeat(20_000 / 12));
    assert!(body.len() >= 20_000);
    project.write(
        "docs/a.mara.md",
        &document(&[item(&mid(1), "Requirement", "REQ-BIG", "Big", &[], &body)]),
    );
    let run = project.run(&["item", "list", "--format", "json"]).ok();
    assert_eq!(names(&run.json()["items"]), ["REQ-BIG"]);
    assert!(!run.stdout.contains("uniquelastword"), "{}", run.stdout);
    let human = project.run(&["item", "list"]).ok();
    assert!(!human.stdout.contains("uniquelastword"));
}

#[test]
fn each_filter_lists_exactly_the_items_it_matches() {
    let project = filter_project();
    for (option, value, near, expected) in FILTER_CASES {
        assert_eq!(
            listed(&project, &[option, value]),
            *expected,
            "{option} {value}"
        );
        assert_eq!(
            listed(&project, &[option, near]),
            [] as [&str; 0],
            "{option} {near}"
        );
    }
}
