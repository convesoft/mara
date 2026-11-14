//! Tests of the built-in checks and of validation results.

use std::sync::Arc;

use super::*;
use crate::document::DocumentKey;
use crate::schema::{Schema, parse_schema};
use crate::snapshot::{DocumentInput, ParsedFile, SnapshotInputs};

const SCHEMA: &str = r#"format_version = 1
statuses = ["draft", "accepted"]

[types.requirement]
id_prefix = "REQ-"
description = "A requirement."

[types.requirement.fields.priority]
kind = "enum"
values = ["low", "high"]
required = true

[types.requirement.fields.tags]
kind = "text"
repeatable = true

[types.note]
id_prefix = "NOTE-"
description = "A note."

[relations.refines]
inverse = "refined_by"
description = "Refines."
targets = { requirement = ["requirement"] }
same_type = true
acyclic = true

[relations.explains]
inverse = "explained_by"
description = "Explains."
targets = { note = ["requirement"] }
"#;

const A: &str = "01M7D8DZ2663WP3A037SR2HJH2";
const B: &str = "01M7D8DZ2RBHGQBYAV7V3789H7";
const C: &str = "01M7D8DZ39AMW9NPW9NA17D4S4";
const N: &str = "01M7D8DZ3MXVBN7QW52PEJCX4N";
const MISSING: &str = "01M7D8DZ3ZQ6V9M3K8E2X4TJRC";

fn schema() -> Arc<Schema> {
    let (schema, diagnostics) = parse_schema(".mara/schema.toml", SCHEMA.as_bytes());
    assert_eq!(diagnostics, []);
    Arc::new(schema.unwrap())
}

/// An item with `rows` after its `Type` row.
fn item(mid: &str, kind: &str, rows: &str) -> String {
    format!(
        "<a id=\"{mid}\"></a>\n\n### Item {mid}\n\n| Type | {kind} |\n| --- | --- |\n{rows}\nBody.\n"
    )
}

fn snapshot(documents: &[(&str, String)], schema: Option<Arc<Schema>>) -> Snapshot {
    let mut work = WorkCounts::default();
    let inputs = documents
        .iter()
        .map(|(path, text)| DocumentInput {
            path: (*path).to_owned(),
            key: DocumentKey::new(crate::hash::Hash::of_file(text.as_bytes(), &mut work)),
            file: Arc::new(ParsedFile::parse(text.as_bytes().to_vec(), &mut work)),
        })
        .collect();
    SnapshotInputs::new(inputs, schema, None, None).build(&mut work)
}

/// The primary findings as (path, line, code), and the item MID of each.
fn found(snapshot: &Snapshot) -> Vec<(String, usize, &'static str, Option<String>)> {
    check(snapshot, &mut WorkCounts::default())
        .into_iter()
        .filter(|finding| finding.primary)
        .map(|finding| {
            let d = finding.diagnostic;
            (d.path, d.line, d.code, d.mid)
        })
        .collect()
}

fn codes(snapshot: &Snapshot) -> Vec<(String, usize, &'static str)> {
    let mut codes: Vec<_> = found(snapshot)
        .into_iter()
        .map(|(path, line, code, _)| (path, line, code))
        .collect();
    codes.sort();
    codes
}

fn at(path: &str, line: usize, code: &'static str) -> (String, usize, &'static str) {
    (path.to_owned(), line, code)
}

#[test]
fn a_conforming_project_has_no_finding() {
    let a = item(
        A,
        "Requirement",
        "| ID | REQ-A |\n| Status | Accepted |\n| Priority | High |\n| Tags | x |\n| Tags | y |\n",
    );
    let b = item(
        B,
        "requirement",
        &format!(
            "| ID | REQ-B |\n| status | draft |\n| PRIORITY | low |\n| Refines | [REQ-A](../a.mara.md#{A}) |\n"
        ),
    );
    let note = item(
        N,
        "Note",
        &format!(
            "| ID | NOTE-N |\n| Status | Draft |\n| Explains | [REQ-B](../b/b.mara.md#{B}) |\n"
        ),
    ) + &format!("See [the first](../a.mara.md#{A}) and [this](#{N}).\n");
    let snapshot = snapshot(
        &[
            ("docs/a.mara.md", a),
            ("docs/b/b.mara.md", b),
            (
                "docs/n/n.mara.md",
                format!("Intro [link](../a.mara.md#{A}).\n\n---\n\n{note}"),
            ),
        ],
        Some(schema()),
    );
    assert_eq!(codes(&snapshot), []);
}

#[test]
fn reports_metadata_problems_once_each() {
    let rows = "| ID | REQ-A |\n| Status | Accepted |\n| Status | Draft |\n| Priority | Medium |\n\
                | Tags | x |\n| Tags | x |\n| Owner | me |\n| Explains | x |\n";
    let snapshot = snapshot(
        &[
            ("a.mara.md", item(A, "Requirement", rows)),
            ("b.mara.md", item(B, "Requirement", "| Priority | Low |\n")),
            (
                "c.mara.md",
                item(
                    C,
                    "Story",
                    "| ID | STORY-C |\n| Status | Done |\n| Anything | x |\n",
                ),
            ),
        ],
        Some(schema()),
    );
    assert_eq!(
        codes(&snapshot),
        [
            at("a.mara.md", 9, "repeated-key"),
            at("a.mara.md", 10, "invalid-value"),
            at("a.mara.md", 12, "duplicate-row"),
            at("a.mara.md", 13, "undeclared-key"),
            // A relation that does not lead from the type: an undeclared key, no relation row.
            at("a.mara.md", 14, "undeclared-key"),
            at("b.mara.md", 1, "missing-row"),
            at("b.mara.md", 1, "missing-row"),
            at("c.mara.md", 5, "undeclared-type"),
            at("c.mara.md", 8, "invalid-value"),
        ]
    );
}

#[test]
fn a_required_field_must_have_a_row() {
    let snapshot = snapshot(
        &[(
            "a.mara.md",
            item(A, "Requirement", "| ID | REQ-A |\n| Status | Draft |\n"),
        )],
        Some(schema()),
    );
    let findings = check(&snapshot, &mut WorkCounts::default());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].diagnostic.code, "missing-row");
    assert!(findings[0].diagnostic.message.contains("`Priority`"));
}

#[test]
fn reports_identity_problems_at_the_later_item_and_to_each_item() {
    let rows = |id: &str| format!("| ID | {id} |\n| Status | Draft |\n| Priority | Low |\n");
    let snapshot = snapshot(
        &[
            ("a.mara.md", item(A, "Requirement", &rows("REQ-A"))),
            ("b.mara.md", item(A, "Requirement", &rows("REQ-A"))),
            ("c.mara.md", item(B, "Requirement", &rows("NOTE-B"))),
            ("d.mara.md", item(C, "Requirement", &rows("req-c"))),
        ],
        Some(schema()),
    );
    assert_eq!(
        codes(&snapshot),
        [
            at("b.mara.md", 1, "duplicate-mid"),
            at("b.mara.md", 7, "duplicate-id"),
            at("c.mara.md", 7, "wrong-id-prefix"),
            at("d.mara.md", 7, "invalid-id"),
            at("d.mara.md", 7, "wrong-id-prefix"),
        ]
    );
    // Each item that shares a MID or an ID has the problem in its own findings.
    let findings = check(&snapshot, &mut WorkCounts::default());
    let first: Vec<_> = findings
        .iter()
        .filter(|finding| finding.item == Some(0))
        .map(|finding| (finding.diagnostic.code, finding.primary))
        .collect();
    assert_eq!(first, [("duplicate-mid", false), ("duplicate-id", false)]);
    assert!(findings[..].iter().all(|f| f.diagnostic.mid.is_some()));
}

#[test]
fn reports_link_problems() {
    let rows =
        |extra: &str| format!("| ID | REQ-A |\n| Status | Draft |\n| Priority | Low |\n{extra}");
    let a = item(
        A,
        "Requirement",
        &rows(&format!(
            "| Refines | [REQ-B](wrong.mara.md#{B}) |\n| Refines | [OLD](b.mara.md#{B}) |\n\
             | Refines | [REQ-X](b.mara.md#{MISSING}) |\n| Refines | REQ-B |\n"
        )),
    ) + &format!("A mention of [b](#{B}) and [none](#{MISSING}).\n");
    let b = item(
        B,
        "Requirement",
        "| ID | REQ-B |\n| Status | Draft |\n| Priority | Low |\n",
    );
    let snapshot = snapshot(
        &[
            ("docs/a.mara.md", a),
            (
                "docs/b.mara.md",
                format!("[Narrative](a.mara.md#{MISSING}).\n\n---\n\n{b}"),
            ),
        ],
        Some(schema()),
    );
    assert_eq!(
        codes(&snapshot),
        [
            at("docs/a.mara.md", 10, "stale-link-path"),
            at("docs/a.mara.md", 11, "stale-link-text"),
            at("docs/a.mara.md", 12, "broken-link"),
            at("docs/a.mara.md", 13, "bare-id-value"),
            at("docs/a.mara.md", 16, "broken-link"),
            at("docs/a.mara.md", 16, "stale-link-path"),
            at("docs/b.mara.md", 1, "broken-link"),
        ]
    );
    let bare = check(&snapshot, &mut WorkCounts::default())
        .into_iter()
        .find(|finding| finding.diagnostic.code == "bare-id-value")
        .unwrap();
    assert!(
        bare.diagnostic
            .message
            .contains(&format!("`[REQ-B](b.mara.md#{B})`")),
        "{}",
        bare.diagnostic.message
    );
    let narrative = found(&snapshot).pop().unwrap();
    assert_eq!(narrative.3, None, "a narrative link concerns no item");
}

#[test]
fn reports_disallowed_targets_and_each_cycle_once() {
    let requirement = |mid: &str, id: &str, refines: &[(&str, &str)]| {
        let mut rows = format!("| ID | {id} |\n| Status | Draft |\n| Priority | Low |\n");
        for (target_id, target) in refines {
            rows.push_str(&format!("| Refines | [{target_id}](#{target}) |\n"));
        }
        item(mid, "Requirement", &rows)
    };
    let note = item(
        N,
        "Note",
        &format!("| ID | NOTE-N |\n| Status | Draft |\n| Explains | [NOTE-N](#{N}) |\n"),
    );
    let document = [
        requirement(A, "REQ-A", &[("REQ-B", B)]),
        requirement(B, "REQ-B", &[("REQ-C", C)]),
        requirement(C, "REQ-C", &[("REQ-A", A), ("REQ-C", C), ("NOTE-N", N)]),
        note,
    ]
    .join("\n---\n\n");
    let snapshot = snapshot(&[("a.mara.md", document)], Some(schema()));
    let findings = check(&snapshot, &mut WorkCounts::default());
    let mut reported: Vec<_> = findings
        .iter()
        .map(|f| (f.diagnostic.line, f.diagnostic.code, f.primary))
        .collect();
    reported.sort();
    // A → B → C → A and C → C are one cycle; C → NOTE-N and NOTE-N → NOTE-N leave their types.
    assert_eq!(
        reported,
        [
            (10, "relation-cycle", true),
            (25, "relation-cycle", false),
            (40, "relation-cycle", false),
            (42, "disallowed-target", true),
            (56, "disallowed-target", true),
        ]
    );
    let cycle = &findings
        .iter()
        .find(|f| f.primary && f.diagnostic.code == "relation-cycle")
        .unwrap();
    assert!(
        cycle
            .diagnostic
            .message
            .contains("`REQ-A`, `REQ-B`, `REQ-C`")
    );
}

#[test]
fn assigns_parse_problems_to_the_item_that_holds_their_line() {
    let a = item(
        A,
        "Requirement",
        "| ID | REQ-A |\n| Status | Draft |\n| Priority | Low |\n| Tags |\n",
    );
    let document = format!("Narrative\n===\n\n---\n\n{a}");
    let snapshot = snapshot(&[("a.mara.md", document)], Some(schema()));
    let found = found(&snapshot);
    let parse: Vec<_> = found
        .iter()
        .map(|(_, line, code, mid)| (*line, *code, mid.as_deref()))
        .collect();
    assert_eq!(
        parse,
        [(2, "setext-heading", None), (15, "malformed-row", Some(A))]
    );
}

#[test]
fn without_a_schema_checks_identity_and_links_only() {
    let a = item(
        A,
        "Anything",
        &format!("| Whatever | x |\n| Refines | [X](#{MISSING}) |\n"),
    );
    let snapshot = snapshot(&[("a.mara.md", a)], None);
    // No schema: no relation edges, no type or key checks; the missing ID and Status remain.
    assert_eq!(
        codes(&snapshot),
        [
            at("a.mara.md", 1, "missing-row"),
            at("a.mara.md", 1, "missing-row")
        ]
    );
}

#[test]
fn counts_two_steps_per_item_one_per_edge_and_more_for_acyclic_relations() {
    let rows = |id: &str, extra: &str| {
        format!("| ID | {id} |\n| Status | Draft |\n| Priority | Low |\n{extra}")
    };
    let a = item(
        A,
        "Requirement",
        &rows("REQ-A", &format!("| Refines | [REQ-B](#{B}) |\n")),
    );
    let b = item(B, "Requirement", &rows("REQ-B", "")) + &format!("See [a](#{A}).\n");
    let snapshot = snapshot(&[("a.mara.md", format!("{a}\n---\n\n{b}"))], Some(schema()));
    let mut work = WorkCounts::default();
    check(&snapshot, &mut work);
    // 2 items visited by the facts pass and the item pass, 2 edges, and the cycle pass over 1
    // acyclic edge between 2 items.
    assert_eq!(
        work,
        WorkCounts {
            check_steps: 9,
            ..WorkCounts::default()
        }
    );
}

#[test]
fn lists_ten_names_then_counts_the_rest() {
    let names = |count: usize| (0..count).map(|n| format!("`N{n}`"));
    let ten: Vec<String> = names(10).collect();
    assert_eq!(list(names(10), 10), ten.join(", "));
    assert_eq!(
        list(names(11), 11),
        format!("{}, and 1 more", ten.join(", "))
    );
    assert_eq!(
        list(names(25), 25),
        format!("{}, and 15 more", ten.join(", "))
    );
    assert_eq!(list(names(0), 0), "");
}

#[test]
fn pages_follow_cursors_and_keep_the_whole_validity() {
    let diagnostic = |path: &str, line: usize| {
        Diagnostic::error("broken-link", path, line, "message".to_owned())
    };
    let key = result_key(&["project validate"], &[None], &[]);
    let report = Report::new(
        vec![
            diagnostic("docs/b.mara.md", 1),
            diagnostic("docs/a.mara.md", 9),
            diagnostic("docs/a.mara.md", 2),
            diagnostic("other.mara.md", 1),
        ],
        key,
    );
    assert!(!report.valid());
    assert_eq!((report.errors(), report.warnings()), (4, 0));
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = report.page(&["docs"], cursor.as_ref(), 1).unwrap();
        seen.extend(page.diagnostics.iter().map(|d| (d.path.clone(), d.line)));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(
        seen,
        [
            ("docs/a.mara.md".to_owned(), 2),
            ("docs/a.mara.md".to_owned(), 9),
            ("docs/b.mara.md".to_owned(), 1)
        ]
    );
    let page = report.page(&["docs/a.mara.md"], None, 20).unwrap();
    assert_eq!(page.diagnostics.len(), 2);
    assert_eq!(
        report.page(&["doc"], None, 20).unwrap().diagnostics.len(),
        0
    );
    let json = report.page(&[], None, 1).unwrap().to_json();
    assert!(
        json.starts_with(r#"{"format_version":1,"valid":false,"errors":4,"warnings":0,"diagnostics":[{"code":"broken-link","severity":"error","message":"message","path":"docs/a.mara.md","line":2,"id":null,"mid":null}],"next_cursor":""#),
        "{json}"
    );
    let other = result_key(&["project validate", "docs"], &[None], &[]);
    let stale = Cursor {
        key: other,
        offset: 1,
    };
    assert_eq!(
        report.page(&[], Some(&stale), 1).unwrap_err(),
        PageError::StaleCursor
    );
}
