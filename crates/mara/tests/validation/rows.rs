//! How a row is read: as a field or a relation by the item's type (DES-SNAPSHOT,
//! DES-SCHEMA-CHECKS), and every row's value checked on its own, repeated ones included
//! (REQ-REPORT-EVERY-PROBLEM).

use crate::{Dir, SCHEMA, at, document, item, json, line_of};

const A: &str = "01M7DNVKSGZYXCQBADDTG4VP8G";
const B: &str = "01M7DNVKSP4MXP3QJ8AV833ZVE";
const N: &str = "01M7DNVKSWQBRNWB9D9VBJS4A6";
const GONE: &str = "01M7DNVKT2MDYT2D2QXZZAR8VH";

/// `SCHEMA` with a text field `refines` on notes: a name that is a relation for requirements,
/// which are its source, and a field for notes, which are not.
fn schema() -> String {
    SCHEMA.replace(
        "[relations.refines]",
        "[types.note.fields.refines]\nkind = \"text\"\nrepeatable = true\n\n[relations.refines]",
    )
}

#[test]
fn a_name_is_a_field_where_the_type_is_no_source_of_the_relation() {
    let dir = Dir::project(&schema());
    let text = document(
        "Notes",
        &[
            item(
                A,
                "Target",
                "Requirement",
                "| ID | REQ-A |\n| Status | Draft |\n",
                "A.",
            ),
            item(
                N,
                "A note",
                "Note",
                &format!(
                    "| ID | NOTE-N |\n| Status | Draft |\n| Refines | free text |\n\
                     | Refines | [a link](#{GONE}) |\n"
                ),
                "A text field, even one whose value is a link, is no relation.",
            ),
        ],
    );
    dir.write("docs/notes.mara.md", &text);
    let run = json(&dir.root(), &["schema", "validate"]);
    assert_eq!((run.status, run.diagnostics()), (0, vec![]));
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!((run.status, run.diagnostics()), (0, vec![]), "{run:?}");
}

#[test]
fn a_relation_that_does_not_lead_from_the_type_is_an_undeclared_key_and_no_edge() {
    let dir = Dir::project(&schema());
    let text = document(
        "Requirements",
        &[item(
            A,
            "Explains nothing",
            "Requirement",
            // `explains` leads from notes only; its target here is of the wrong type and gone.
            &format!("| ID | REQ-A |\n| Status | Draft |\n| Explains | [NOTE-X](#{GONE}) |\n"),
            "A.",
        )],
    );
    dir.write("docs/a.mara.md", &text);
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!(run.status, 1);
    // One problem, the key: the row is no edge, so its link is not checked as one.
    assert_eq!(
        run.diagnostics(),
        [at(
            "undeclared-key",
            "docs/a.mara.md",
            line_of(&text, "| Explains")
        )]
    );
}

#[test]
fn every_repeated_row_has_its_value_checked() {
    let dir = Dir::project(SCHEMA);
    let rows = format!(
        "| Type | Unknown |\n| ID | REQ-A |\n| ID | bad |\n| Status | Draft |\n| Status | Done |\n\
         | Priority | Low |\n| Priority | Medium |\n| Refines | [REQ-B](#{B}) |\n\
         | Refines | [REQ-GONE](#{GONE}) |\n| Tags | x |\n"
    );
    let text = document(
        "Repeated",
        &[
            item(A, "Repeats", "Requirement", &rows, "A."),
            item(
                B,
                "Duplicates",
                "Requirement",
                "| ID | REQ-B |\n| Status | Draft |\n| Priority | Urgent |\n| Priority | Urgent |\n",
                "B.",
            ),
        ],
    );
    dir.write("docs/a.mara.md", &text);
    let line = |marker: &str| line_of(&text, marker);
    let path = "docs/a.mara.md";
    let urgent = line("| Priority | Urgent |\n| Priority");
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!(run.status, 1);
    // The first `Type` stays the type: the rows are checked as a requirement's.
    assert_eq!(
        run.diagnostics(),
        [
            at("repeated-key", path, line("| Type | Unknown |")),
            at("undeclared-type", path, line("| Type | Unknown |")),
            at("repeated-key", path, line("| ID | bad |")),
            at("invalid-id", path, line("| ID | bad |")),
            at("wrong-id-prefix", path, line("| ID | bad |")),
            at("repeated-key", path, line("| Status | Done |")),
            at("invalid-value", path, line("| Status | Done |")),
            at("repeated-key", path, line("| Priority | Medium |")),
            at("invalid-value", path, line("| Priority | Medium |")),
            at("broken-link", path, line("| Refines | [REQ-GONE]")),
            at("invalid-value", path, urgent),
            at("duplicate-row", path, urgent + 1),
            at("invalid-value", path, urgent + 1),
        ]
    );
}

const X1: &str = "01M7DPMHDH29XXGWJHPSXJ6FJ4";
const X2: &str = "01M7DPMHDSTMCF4N1NV98FCJN7";
const C1: &str = "01M7DPMHE0YV6213XXP8NHYQFB";
const C2: &str = "01M7DPMHE629GKH2FDY3YJM185";

/// A problem that items share is reported once on each, whatever rows repeat; each
/// occurrence's own problem is reported at that occurrence.
#[test]
fn a_repeated_row_repeats_its_own_problems_and_not_the_shared_ones() {
    let dir = Dir::project(SCHEMA);
    let a = document(
        "A",
        &[item(
            X1,
            "First holder",
            "Requirement",
            "| ID | REQ-X |\n| Status | Draft |\n",
            "A.",
        )],
    );
    let b = document(
        "B",
        &[item(
            X2,
            "Second holder",
            "Requirement",
            "| ID | REQ-X |\n| Status | Draft |\n| ID | REQ-X |\n",
            "B.",
        )],
    );
    let refines = |id: &str, mid: &str| format!("| Refines | [{id}](#{mid}) |\n");
    let gone = format!("| Refines | [REQ-GONE](#{GONE}) |\n");
    let cycle = document(
        "Cycle",
        &[
            item(
                C1,
                "Refines twice",
                "Requirement",
                &format!(
                    "| ID | REQ-C1 |\n| Status | Draft |\n{}{}{gone}{gone}",
                    refines("REQ-C2", C2),
                    refines("REQ-C2", C2)
                ),
                "C1.",
            ),
            item(
                C2,
                "Closes the cycle",
                "Requirement",
                &format!(
                    "| ID | REQ-C2 |\n| Status | Draft |\n{}",
                    refines("REQ-C1", C1)
                ),
                "C2.",
            ),
        ],
    );
    dir.write("docs/a.mara.md", &a);
    dir.write("docs/b.mara.md", &b);
    dir.write("docs/cycle.mara.md", &cycle);
    let twice = refines("REQ-C2", C2).repeat(2);
    let first_c2 = line_of(&cycle, &twice);
    let first_gone = line_of(&cycle, &format!("{gone}{gone}"));
    let id_b = line_of(&b, "| ID | REQ-X |\n| Status");
    let (b_path, c_path) = ("docs/b.mara.md", "docs/cycle.mara.md");
    let run = json(&dir.root(), &["project", "validate"]);
    assert_eq!(
        run.diagnostics(),
        [
            at("duplicate-id", b_path, id_b),
            at("duplicate-row", b_path, id_b + 2),
            at("relation-cycle", c_path, first_c2),
            at("duplicate-row", c_path, first_c2 + 1),
            at("broken-link", c_path, first_gone),
            at("duplicate-row", c_path, first_gone + 1),
            at("broken-link", c_path, first_gone + 1),
        ]
    );
    // Each holder of the ID, and each item on the cycle, has its shared problem once.
    let validate = |mid: &str| json(&dir.root(), &["item", "validate", mid]).diagnostics();
    assert_eq!(
        validate(X1),
        [at("duplicate-id", "docs/a.mara.md", line_of(&a, "| ID"))]
    );
    assert_eq!(
        validate(X2),
        [
            at("duplicate-id", b_path, id_b),
            at("duplicate-row", b_path, id_b + 2)
        ]
    );
    assert_eq!(
        validate(C1)
            .iter()
            .filter(|d| d.0 == "relation-cycle")
            .count(),
        1
    );
    assert_eq!(
        validate(C2),
        [at(
            "relation-cycle",
            c_path,
            line_of(&cycle, "| Refines | [REQ-C1]")
        )]
    );
}

/// Every message that lists names lists at most 10 of them, then `and <N> more`
/// (DES-SCHEMA-CHECKS): statuses, enum values, allowed target types, the items of a cycle and
/// the holders of an ambiguous handle, each here with more than ten.
#[test]
fn every_list_in_a_message_shows_ten_names_and_counts_the_rest() {
    let quoted = |prefix: &str, count: usize| -> Vec<String> {
        (0..count).map(|n| format!("\"{prefix}{n}\"")).collect()
    };
    let mut schema = format!(
        "format_version = 1\nstatuses = [{}]\n\n[types.requirement]\nid_prefix = \"REQ-\"\n\
         description = \"An obligation.\"\n\n[types.requirement.fields.level]\nkind = \"enum\"\n\
         values = [{}]\n\n[types.note]\nid_prefix = \"NOTE-\"\ndescription = \"A note.\"\n",
        quoted("s", 20).join(", "),
        quoted("v", 11).join(", ")
    );
    for n in 0..11 {
        schema.push_str(&format!(
            "\n[types.t{n}]\nid_prefix = \"T{n}-\"\ndescription = \"Target {n}.\"\n"
        ));
    }
    schema.push_str(&format!(
        "\n[relations.refines]\ninverse = \"refined_by\"\ndescription = \"Refines.\"\n\
         targets = {{ requirement = [\"requirement\"] }}\nacyclic = true\n\n\
         [relations.explains]\ninverse = \"explained_by\"\ndescription = \"Explains.\"\n\
         targets = {{ note = [{}] }}\n",
        quoted("t", 11).join(", ")
    ));
    let dir = Dir::project(&schema);
    let mid = |n: usize| format!("01M7DQ0000000000000000{n:04}");
    dir.write(
        "docs/a.mara.md",
        document(
            "A",
            &[
                item(
                    &mid(0),
                    "Bad values",
                    "Requirement",
                    "| ID | REQ-S |\n| Status | Draft |\n| Level | Bad |\n",
                    "S.",
                ),
                item(
                    &mid(1),
                    "Wrong target",
                    "Note",
                    &format!(
                        "| ID | NOTE-N |\n| Status | S0 |\n| Explains | [REQ-S](#{}) |\n",
                        mid(0)
                    ),
                    "N.",
                ),
            ],
        ),
    );
    let cycle: Vec<String> = (0..11)
        .map(|n| {
            let next = (n + 1) % 11;
            let rows = format!(
                "| ID | REQ-C{n} |\n| Status | S0 |\n| Refines | [REQ-C{next}](#{}) |\n",
                mid(10 + next)
            );
            item(&mid(10 + n), "On the cycle", "Requirement", &rows, "C.")
        })
        .collect();
    dir.write("docs/cycle.mara.md", document("Cycle", &cycle));
    let same: Vec<String> = (0..11)
        .map(|n| {
            item(
                &mid(30 + n),
                "Same ID",
                "Requirement",
                "| ID | REQ-SAME |\n| Status | S0 |\n",
                "X.",
            )
        })
        .collect();
    dir.write("docs/same.mara.md", document("Same", &same));

    let ten = |names: &dyn Fn(usize) -> String| -> String {
        let listed: Vec<String> = (0..10).map(names).collect();
        listed.join(", ")
    };
    let run = json(&dir.root(), &["project", "validate", "--limit", "100"]);
    let messages: Vec<(String, String)> = run.json()["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["code"].as_str().unwrap().to_owned(),
                d["message"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let message = |code: &str, start: &str| -> String {
        let found = messages
            .iter()
            .find(|(c, m)| c == code && m.starts_with(start));
        found
            .unwrap_or_else(|| panic!("no {code} {start:?} in {messages:?}"))
            .1
            .clone()
    };
    assert_eq!(
        message("invalid-value", "`Draft`"),
        format!(
            "`Draft` is not a status of the schema: {}, and 10 more",
            ten(&|n| format!("`S{n}`"))
        )
    );
    assert_eq!(
        message("invalid-value", "`Bad`"),
        format!(
            "`Bad` is not a value of `Level`: {}, and 1 more",
            ten(&|n| format!("`V{n}`"))
        )
    );
    assert!(
        message("disallowed-target", "").starts_with(&format!(
            "this relation leads from type `note` only to {}, and 1 more, and ",
            ten(&|n| format!("`t{n}`"))
        )),
        "{messages:?}"
    );
    assert_eq!(
        message("relation-cycle", ""),
        format!(
            "the `Refines` links of {}, and 1 more form a cycle, which relation `refines` forbids",
            ten(&|n| format!("`REQ-C{n}`"))
        )
    );
    let run = json(&dir.root(), &["item", "validate", "REQ-SAME"]);
    let error = run.json()["error"]["message"].as_str().unwrap().to_owned();
    let text = dir.read("docs/same.mara.md");
    let lines: Vec<usize> = text
        .match_indices("<a id=")
        .map(|(at, _)| text[..at].matches('\n').count() + 1)
        .collect();
    assert_eq!(
        error,
        format!(
            "11 items have `REQ-SAME`, at {}, and 1 more: name one by its MID",
            ten(&|n| format!("`docs/same.mara.md:{}`", lines[n]))
        )
    );
}
