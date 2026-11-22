//! Tests of the fact relations and the checks of supplied inputs. VER-FACT-ROWS and
//! VER-REQUEST-INPUTS run through Soufflé in `crates/mara/tests/facts`.

use std::sync::Arc;

use serde_json::json;

use super::*;
use crate::document::DocumentKey;
use crate::hash::Hash;
use crate::schema::parse_schema;
use crate::snapshot::{DocumentInput, ParsedFile, SnapshotInputs};
use crate::work::WorkCounts;

const A: &str = "01M7D8DZ2663WP3A037SR2HJH2";
const B: &str = "01M7D8DZ2RBHGQBYAV7V3789H7";

const SCHEMA: &str = "format_version = 1\nstatuses = [\"draft\", \"in_review\"]\n\n\
    [types.requirement]\nid_prefix = \"REQ-\"\ndescription = \"R.\"\n\n\
    [types.requirement.fields.priority]\nkind = \"enum\"\nvalues = [\"very_high\", \"low\"]\n\n\
    [types.requirement.fields.owner]\nkind = \"text\"\nrepeatable = true\n\n\
    [relations.satisfies]\ninverse = \"satisfied_by\"\ndescription = \"S.\"\n\
    targets = { requirement = [\"requirement\"] }\n";

fn snapshot(text: &str) -> Snapshot {
    let (schema, diagnostics) = parse_schema(".mara/schema.toml", SCHEMA.as_bytes());
    assert_eq!(diagnostics, []);
    let mut work = WorkCounts::default();
    let input = DocumentInput {
        path: "docs/a.mara.md".into(),
        key: DocumentKey::new(Hash::of_file(text.as_bytes(), &mut work)),
        file: Arc::new(ParsedFile::parse(text.as_bytes().to_vec(), &mut work)),
    };
    SnapshotInputs::new(vec![input], schema.map(Arc::new), None, None).build(&mut work)
}

fn s(text: &str) -> Value {
    Value::Symbol(text.into())
}

fn rows<'f>(facts: &'f Facts, name: &str) -> &'f [Vec<Value>] {
    let relation = facts.relations.iter().find(|r| r.name == name).unwrap();
    &relation.rows
}

#[test]
fn derives_each_relation_with_schema_names() {
    let text = format!(
        "<a id=\"{A}\"></a>\n\n### Alpha\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | REQ-A |\n| Status | In Review |\n| Priority | Very High |\n| Owner | Ann |\n\
         | Owner | Bob |\n| Unknown | x |\n| Satisfies | [B](#{B}) |\n\n\
         Mentions [B](#{B}).\n\n---\n\n<a id=\"{B}\"></a>\n\n### Beta\n\n\
         | Type | Requirement |\n| --- | --- |\n"
    );
    let snapshot = snapshot(&text);
    let facts = snapshot.facts();
    let path = s("docs/a.mara.md");
    assert_eq!(
        rows(facts, "item"),
        [
            vec![
                s(A),
                s("requirement"),
                s("REQ-A"),
                s("in_review"),
                path.clone(),
                Value::Number(1)
            ],
            vec![
                s(B),
                s("requirement"),
                s(""),
                s(""),
                path.clone(),
                Value::Number(19)
            ],
        ]
    );
    assert_eq!(
        rows(facts, "title"),
        [vec![s(A), s("Alpha")], vec![s(B), s("Beta")]]
    );
    assert_eq!(
        rows(facts, "revision"),
        [
            vec![s(A), s(&snapshot.revision(0))],
            vec![s(B), s(&snapshot.revision(1))]
        ]
    );
    assert_eq!(
        rows(facts, "field"),
        [
            vec![s(A), s("priority"), s("very_high")],
            vec![s(A), s("owner"), s("Ann")],
            vec![s(A), s("owner"), s("Bob")],
        ]
    );
    let line = Value::Number(13);
    assert_eq!(
        rows(facts, "edge"),
        [vec![s("satisfies"), s(A), s(B), path, line]],
        "a mention is no edge"
    );
    assert_eq!(rows(facts, "folded")[0], vec![s("Alpha"), s("alpha")]);
    assert_eq!(rows(facts, "folded").len(), 5, "each text once");
    assert!(
        std::ptr::eq(snapshot.facts(), facts),
        "derived once per snapshot"
    );
    let item = &facts.relations[0].json;
    assert!(item.starts_with(&format!("[[\"{A}\",\"requirement\",\"REQ-A\",")));
}

#[test]
fn folds_to_nfc_lowercase() {
    assert_eq!(fold("E\u{301}COLE"), "\u{e9}cole");
    assert_eq!(fold("Straße ÄB"), "straße äb");
}

#[test]
fn writes_symbols_escaped_and_numbers_as_numbers() {
    let relation = Relation::new(
        "r",
        vec![
            vec![s("a\tb\n\"\\é"), Value::Number(i32::MIN)],
            vec![s(""), Value::Number(7)],
        ],
    );
    assert_eq!(relation.json, r#"[["a\tb\n\"\\é",-2147483648],["",7]]"#);
}

fn listed() -> Vec<InputDecl> {
    vec![
        InputDecl {
            name: "candidate".into(),
            columns: vec![Column::Symbol],
        },
        InputDecl {
            name: "result".into(),
            columns: vec![Column::Symbol, Column::Symbol, Column::Number],
        },
    ]
}

fn check(result: serde_json::Value) -> Result<Inputs, InputError> {
    let supplied = vec![
        ("candidate".to_owned(), Supplied::Inline("abc".into())),
        ("result".to_owned(), Supplied::Rows(result)),
    ];
    check_inputs(&listed(), supplied, &[])
}

#[test]
fn keeps_supplied_rows_as_given() {
    let inputs = check(json!([
        ["a", "pass", -2147483648],
        ["b", "fail\n\t", 2147483647]
    ]))
    .unwrap();
    assert_eq!(inputs.relations[0].rows, [vec![s("abc")]]);
    assert_eq!(
        inputs.relations[1].rows,
        [
            vec![s("a"), s("pass"), Value::Number(i32::MIN)],
            vec![s("b"), s("fail\n\t"), Value::Number(i32::MAX)],
        ]
    );
    assert_eq!(inputs.source_complete.rows, Vec::<Vec<Value>>::new());
    assert_eq!(inputs.source_complete.json, "[]");
}

#[test]
fn refuses_a_wrong_row_naming_its_row_and_column() {
    let cases = [
        (
            json!([["a", "pass"]]),
            "row 1: 2 columns where `result` has 3",
        ),
        (json!([["a", "b", 1], ["a", "b", 1, 2]]), "row 2: 4 columns"),
        (
            json!([[1, "pass", 1]]),
            "row 1, column 1: the number `1` where the column takes a string",
        ),
        (
            json!([["a", "b", "1"]]),
            "row 1, column 3: a string where the column takes a number",
        ),
        (
            json!([["a", "b", 1.5]]),
            "row 1, column 3: `1.5` is not an integer",
        ),
        (
            json!([["a", "b", 2147483648_i64]]),
            "column 3: `2147483648` is not",
        ),
        (
            json!([["a", "b", -2147483649_i64]]),
            "column 3: `-2147483649` is not",
        ),
        (json!([["a", "b", null]]), "column 3: null where"),
        (
            json!([["a", "b", 1], "x"]),
            "row 2: a string where a row is a list",
        ),
        (
            json!({"rows": []}),
            "`result` is not a JSON list of rows: it is an object",
        ),
    ];
    for (rows, expected) in cases {
        let error = check(rows.clone()).unwrap_err();
        assert_eq!(error.code(), "invalid-input", "{rows}");
        assert!(error.to_string().contains(expected), "{rows}: {error}");
    }
}

#[test]
fn refuses_a_missing_unlisted_or_repeated_input() {
    let only = |name: &str| vec![(name.to_owned(), Supplied::Inline("x".into()))];
    let error = check_inputs(&listed(), only("candidate"), &[]).unwrap_err();
    assert_eq!(error, InputError::Missing("result".into()));
    let mut supplied = only("candidate");
    supplied.push(("other".into(), Supplied::Rows(json!([]))));
    let error = check_inputs(&listed(), supplied, &[]).unwrap_err();
    assert_eq!(error, InputError::Unlisted("other".into()));
    let mut supplied = only("candidate");
    supplied.extend(only("candidate"));
    let error = check_inputs(&listed(), supplied, &[]).unwrap_err();
    assert_eq!(error, InputError::Repeated("candidate".into()));
    let error = check_inputs(&listed(), Vec::new(), &["other".into()]).unwrap_err();
    assert_eq!(error, InputError::Unlisted("other".into()));
}

#[test]
fn reads_an_inline_value_as_one_row_of_one_column() {
    let number = InputDecl {
        name: "n".into(),
        columns: vec![Column::Number],
    };
    let inline = |text: &str| vec![("n".to_owned(), Supplied::Inline(text.into()))];
    let inputs = check_inputs(std::slice::from_ref(&number), inline("-2147483648"), &[]).unwrap();
    assert_eq!(inputs.relations[0].rows, [vec![Value::Number(i32::MIN)]]);
    for text in ["2147483648", "1.5", "+1", "x"] {
        let error = check_inputs(std::slice::from_ref(&number), inline(text), &[]).unwrap_err();
        assert!(error.to_string().contains("row 1, column 1"), "{error}");
    }
    let error = check_inputs(
        &listed()[1..],
        vec![("result".into(), Supplied::Inline("a".into()))],
        &[],
    );
    assert!(
        error
            .unwrap_err()
            .to_string()
            .contains("1 column where `result` has 3")
    );
}

#[test]
fn marks_each_complete_input_once() {
    let complete = ["result".to_owned(), "result".to_owned()];
    let supplied = vec![
        ("candidate".to_owned(), Supplied::Inline("abc".into())),
        ("result".to_owned(), Supplied::Rows(json!([]))),
    ];
    let inputs = check_inputs(&listed(), supplied, &complete).unwrap();
    assert_eq!(inputs.source_complete.rows, [vec![s("result")]]);
    assert_eq!(inputs.source_complete.name, "source_complete");
}
