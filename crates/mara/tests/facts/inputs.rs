//! VER-REQUEST-INPUTS, below the CLI: a gate program with the inputs `candidate` and `result`,
//! supplied as `--input` arguments give them and read back through Soufflé. The gate outcome
//! per item, MCP, `--debug-stats` and a later request are mara-bx82k0's.

use std::fs;
use std::path::Path;

use mara::rules::supplied_input;
use mara_engine::facts::{Column, InputDecl, InputError, Supplied, check_inputs};
use serde_json::{Value, json};

use crate::{Project, evaluate, mid, sorted};

const PROGRAM: &str = r#".include "mara.dl"
.decl candidate(text: symbol)
.decl result(mid: Mid, outcome: symbol, run: number)

.decl read_candidate(mid: Mid, text: symbol)
read_candidate(m, c) :- item(m, _, _, _, _, _), candidate(c).
.decl read_result(mid: Mid, outcome: symbol, run: number)
read_result(m, o, r) :- result(m, o, r).

.decl subject(mid: Mid)
subject(m) :- item(m, "requirement", _, _, _, _).
.decl has_result(mid: Mid)
has_result(m) :- result(m, _, _).
.decl condition(mid: Mid)
condition(m) :- subject(m), result(m, "fail", _).
condition(m) :- subject(m), !has_result(m), source_complete("result").
.decl unavailable(mid: Mid)
unavailable(m) :- subject(m), !has_result(m), !source_complete("result").
"#;

const OUTPUTS: [&str; 4] = ["read_candidate", "read_result", "condition", "unavailable"];

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

/// REQ-A fails, REQ-B passes at the limits of a number column, REQ-C has no result.
fn results() -> Value {
    json!([
        [mid(1), "fail", -2147483648_i64],
        [mid(2), "pass", 2147483647],
    ])
}

/// Checks `--input` arguments, run in `dir`, and `--complete` names.
fn check(
    dir: &Path,
    arguments: &[&str],
    complete: &[&str],
) -> Result<mara_engine::facts::Inputs, InputError> {
    let supplied = arguments
        .iter()
        .map(|argument| supplied_input(argument, dir))
        .collect::<Result<Vec<_>, _>>()?;
    let complete: Vec<String> = complete.iter().map(|name| (*name).to_owned()).collect();
    check_inputs(&listed(), supplied, &complete)
}

fn ids(rows: &[Vec<Value>]) -> Vec<String> {
    let mut mids: Vec<String> = rows
        .iter()
        .map(|row| row[0].as_str().unwrap().to_owned())
        .collect();
    mids.sort();
    mids
}

#[test]
fn the_program_reads_exactly_the_supplied_rows() {
    let project = Project::new();
    let dir = tempfile::Builder::new()
        .prefix("mara-inputs-")
        .tempdir()
        .unwrap();
    fs::write(dir.path().join("results.json"), results().to_string()).unwrap();
    let arguments = ["candidate=abc", "result=@results.json"];

    let inputs = check(dir.path(), &arguments, &[]).unwrap();
    let got = evaluate(&project.facts(), &inputs, PROGRAM, &OUTPUTS).unwrap();
    let candidates: Vec<Vec<Value>> = (1..=4).map(|n| vec![json!(mid(n)), json!("abc")]).collect();
    assert_eq!(sorted(got["read_candidate"].clone()), sorted(candidates));
    let supplied: Vec<Vec<Value>> = serde_json::from_value(results()).unwrap();
    assert_eq!(
        sorted(got["read_result"].clone()),
        sorted(supplied),
        "read as given"
    );
    assert_eq!(
        ids(&got["condition"]),
        [mid(1)],
        "the `fail` row fails its item"
    );
    assert_eq!(
        ids(&got["unavailable"]),
        [mid(4)],
        "no result, source incomplete"
    );

    let inputs = check(dir.path(), &arguments, &["result"]).unwrap();
    let got = evaluate(&project.facts(), &inputs, PROGRAM, &OUTPUTS).unwrap();
    assert_eq!(
        ids(&got["condition"]),
        [mid(1), mid(4)],
        "no result, source complete"
    );
    assert_eq!(got["unavailable"], Vec::<Vec<Value>>::new());
}

#[test]
fn the_same_rows_passed_as_json_values_give_the_same_inputs() {
    let dir = tempfile::Builder::new()
        .prefix("mara-inputs-")
        .tempdir()
        .unwrap();
    fs::write(dir.path().join("results.json"), results().to_string()).unwrap();
    let from_file = check(dir.path(), &["candidate=abc", "result=@results.json"], &[]).unwrap();
    let passed = vec![
        ("candidate".to_owned(), Supplied::Inline("abc".into())),
        ("result".to_owned(), Supplied::Rows(results())),
    ];
    assert_eq!(check_inputs(&listed(), passed, &[]).unwrap(), from_file);
}

#[test]
fn an_omitted_unlisted_or_unreadable_input_is_refused_naming_it() {
    let dir = tempfile::Builder::new()
        .prefix("mara-inputs-")
        .tempdir()
        .unwrap();
    let error = check(dir.path(), &["candidate=abc"], &[]).unwrap_err();
    assert_eq!(
        (error.code(), error.to_string().contains("`result`")),
        ("missing-input", true)
    );
    let error = check(dir.path(), &["candidate=abc", "other=x"], &[]).unwrap_err();
    assert_eq!(
        (error.code(), error.to_string().contains("`other`")),
        ("unlisted-input", true)
    );

    fs::write(dir.path().join("broken.json"), "[[\"a\", ").unwrap();
    fs::write(dir.path().join("object.json"), "{\"rows\": []}").unwrap();
    for (file, expected) in [
        ("broken.json", "broken.json is not JSON"),
        ("missing.json", "missing.json cannot be read"),
        ("object.json", "not a JSON list of rows"),
    ] {
        let argument = format!("result=@{file}");
        let error = check(dir.path(), &["candidate=abc", &argument], &[]).unwrap_err();
        assert_eq!(error.code(), "invalid-input");
        assert!(error.to_string().contains(expected), "{file}: {error}");
    }
    let error = check(dir.path(), &["candidate"], &[]).unwrap_err();
    assert!(error.to_string().contains("names no input"), "{error}");
}

#[test]
fn a_wrong_row_is_refused_naming_its_row_and_column() {
    let dir = tempfile::Builder::new()
        .prefix("mara-inputs-")
        .tempdir()
        .unwrap();
    let a = mid(1);
    let cases = [
        (json!([[a, "fail"]]), "row 1: 2 columns"),
        (
            json!([[a, "fail", 1], [a, "fail", 1, 2]]),
            "row 2: 4 columns",
        ),
        (json!([[1, "fail", 1]]), "row 1, column 1"),
        (json!([[a, "fail", "1"]]), "row 1, column 3"),
        (json!([[a, "fail", 1.5]]), "row 1, column 3"),
        (json!([[a, "fail", 2147483648_i64]]), "row 1, column 3"),
        (json!([[a, "fail", -2147483649_i64]]), "row 1, column 3"),
        (json!([[a, "fail", 1], [a]]), "row 2: 1 column"),
    ];
    for (rows, expected) in cases {
        fs::write(dir.path().join("results.json"), rows.to_string()).unwrap();
        let error = check(dir.path(), &["candidate=abc", "result=@results.json"], &[]).unwrap_err();
        assert_eq!(error.code(), "invalid-input", "{rows}");
        let message = error.to_string();
        assert!(
            message.contains("`result`") && message.contains(expected),
            "{rows}: {message}"
        );
    }
}
