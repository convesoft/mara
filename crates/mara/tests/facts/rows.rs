//! VER-FACT-ROWS: each fact relation copied into a relation that starts with an item's MID,
//! compared with what `get` and `relation list` give.

use mara::rules::supplied_input;
use mara_engine::facts::{Column, InputDecl, Inputs, check_inputs};
use serde_json::{Value, json};

use crate::{ECOLE, ODD_VALUE, Outputs, Project, evaluate, mid, sorted};

const PROGRAM: &str = r#".include "mara.dl"
.decl note(text: symbol)

.decl r_item(mid: Mid, type: Type, id: Id, status: Status, path: Path, line: number)
r_item(m, t, i, s, p, l) :- item(m, t, i, s, p, l).
.decl r_title(mid: Mid, text: symbol)
r_title(m, t) :- title(m, t).
.decl r_revision(mid: Mid, hash: symbol)
r_revision(m, h) :- revision(m, h).
.decl r_field(mid: Mid, key: symbol, value: symbol)
r_field(m, k, v) :- field(m, k, v).
.decl r_edge(src: Mid, rel: Rel, dst: Mid, path: Path, line: number)
r_edge(s, r, d, p, l) :- edge(r, s, d, p, l).
.decl r_folded(mid: Mid, text: symbol, folded: symbol)
r_folded(m, t, f) :- title(m, t), folded(t, f).
r_folded(m, v, f) :- field(m, _, v), folded(v, f).
.decl r_complete(mid: Mid, source: symbol)
r_complete(m, s) :- item(m, _, _, _, _, _), source_complete(s).
.decl r_note(mid: Mid, text: symbol)
r_note(m, t) :- item(m, _, _, _, _, _), note(t).
"#;

const OUTPUTS: [&str; 8] = [
    "r_item",
    "r_title",
    "r_revision",
    "r_field",
    "r_edge",
    "r_folded",
    "r_complete",
    "r_note",
];

const NOTE: &str = "line\nbreak\ttab";

fn inputs(complete: &[String]) -> Inputs {
    let dir = tempfile::Builder::new()
        .prefix("mara-facts-note-")
        .tempdir()
        .unwrap();
    std::fs::write(dir.path().join("note.json"), json!([[NOTE]]).to_string()).unwrap();
    let note = supplied_input("note=@note.json", dir.path()).unwrap();
    let listed = [InputDecl {
        name: "note".into(),
        columns: vec![Column::Symbol],
    }];
    check_inputs(&listed, vec![note], complete).unwrap()
}

fn run(project: &Project, complete: &[String]) -> Outputs {
    let facts = project.facts();
    evaluate(&facts, &inputs(complete), PROGRAM, &OUTPUTS).unwrap()
}

fn s(text: &str) -> Value {
    Value::String(text.to_owned())
}

/// The schema name of a label.
fn name(label: &str) -> String {
    label.to_lowercase().replace(' ', "_")
}

/// The rows each relation should have, from `get` and `relation list` of every item.
fn expected(project: &Project) -> Outputs {
    let mut out = Outputs::new();
    let mut add = |relation: &str, row: Vec<Value>| {
        out.entry(relation.to_owned()).or_default().push(row);
    };
    for n in 1..=4 {
        let mid = mid(n);
        let page = project.mara(&["get", &mid]);
        let node = &page["node"];
        let rows = page["rows"].as_array().unwrap();
        let value = |key: &str| {
            let row = rows.iter().find(|row| row["key"] == key).unwrap();
            row["value"].as_str().unwrap().to_owned()
        };
        let title = node["title"].as_str().unwrap();
        add(
            "r_item",
            vec![
                s(&mid),
                node["type"].clone(),
                node["id"].clone(),
                s(&name(&value("Status"))),
                node["path"].clone(),
                node["line"].clone(),
            ],
        );
        add("r_title", vec![s(&mid), s(title)]);
        add("r_revision", vec![s(&mid), node["revision"].clone()]);
        add(
            "r_folded",
            vec![s(&mid), s(title), s(&mara_engine::facts::fold(title))],
        );
        for row in rows {
            let (key, value) = (row["key"].as_str().unwrap(), row["value"].as_str().unwrap());
            let value = match key {
                "Priority" => name(value),
                "Owner" => value.to_owned(),
                _ => continue,
            };
            add("r_field", vec![s(&mid), s(&name(key)), s(&value)]);
            add(
                "r_folded",
                vec![s(&mid), s(&value), s(&mara_engine::facts::fold(&value))],
            );
        }
        let connections = project.mara(&["relation", "list", &mid]);
        for connection in connections["connections"].as_array().unwrap() {
            if connection["kind"] != "relation" || connection["direction"] != "outgoing" {
                continue;
            }
            add(
                "r_edge",
                vec![
                    s(&mid),
                    connection["relation"].clone(),
                    connection["neighbour"]["mid"].clone(),
                    connection["source"]["path"].clone(),
                    connection["source"]["line"].clone(),
                ],
            );
        }
        add("r_note", vec![s(&mid), s(NOTE)]);
    }
    out
}

#[test]
fn item_title_revision_field_and_edge_rows_are_what_get_and_relation_list_give() {
    let project = Project::new();
    let got = run(&project, &[]);
    let expected = expected(&project);
    for relation in [
        "r_item",
        "r_title",
        "r_revision",
        "r_field",
        "r_edge",
        "r_folded",
        "r_note",
    ] {
        assert_eq!(
            sorted(got[relation].clone()),
            sorted(expected[relation].clone()),
            "{relation}"
        );
    }
    assert_eq!(got["r_item"].len(), 4);
    assert_eq!(
        got["r_edge"].len(),
        2,
        "a relation row of two items each; mentions are no edges"
    );
    assert_eq!(got["r_field"].len(), 3);
    let statuses: Vec<&Value> = got["r_item"].iter().map(|row| &row[3]).collect();
    assert!(statuses.contains(&&s("accepted")) && statuses.contains(&&s("draft")));
    assert!(got["r_field"].contains(&vec![s(&mid(1)), s("priority"), s("very_high")]));
}

#[test]
fn folded_gives_the_nfc_lowercase_title() {
    let project = Project::new();
    let got = run(&project, &[]);
    assert!(
        got["r_folded"].contains(&vec![s(&mid(1)), s(ECOLE), s("\u{e9}cole")]),
        "{:?}",
        got["r_folded"]
    );
}

#[test]
fn odd_titles_and_values_come_back_as_get_gives_them() {
    let project = Project::new();
    let got = run(&project, &[]);
    let title = &got["r_title"]
        .iter()
        .find(|row| row[0] == s(&mid(2)))
        .unwrap()[1];
    let title = title.as_str().unwrap();
    assert_eq!(title, "Tab\there \"q\" back\\slash \u{fc}", "one row");
    assert!(got["r_field"].contains(&vec![s(&mid(1)), s("owner"), s(ODD_VALUE)]));
}

#[test]
fn source_complete_holds_only_what_the_request_marks() {
    let project = Project::new();
    assert_eq!(run(&project, &[])["r_complete"], Vec::<Vec<Value>>::new());
    let got = run(&project, &["note".into()]);
    let expected: Vec<Vec<Value>> = (1..=4).map(|n| vec![s(&mid(n)), s("note")]).collect();
    assert_eq!(sorted(got["r_complete"].clone()), sorted(expected));
}

#[test]
fn a_rule_that_compares_a_mid_with_an_id_does_not_compile() {
    let project = Project::new();
    let program =
        ".include \"mara.dl\"\n.decl bad(m: Mid)\nbad(m) :- item(m, _, i, _, _, _), m = i.\n";
    let inputs = check_inputs(&[], Vec::new(), &[]).unwrap();
    let error = evaluate(&project.facts(), &inputs, program, &["bad"]).unwrap_err();
    assert!(error.contains("in file program.dl at line 3"), "{error}");
}
