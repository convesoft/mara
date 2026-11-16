//! VER-REQUEST-INPUTS through the CLI: a gate lists the inputs `candidate` (one symbol) and
//! `result` (two symbols and a number), supplied with `--input` and `--complete`. A query on
//! the same program shows what it read. The MCP rows are in the `mcp` test's parity case.

use serde_json::{Value, json};

use crate::{Run, Sandbox, mid};

const PROGRAM: &str = r#".include "mara.dl"
.decl candidate(c: symbol)
.decl result(m: Mid, outcome: symbol, run: number)
.decl read_result(m: Mid, outcome: symbol, run: number, c: symbol)
read_result(m, o, r, c) :- result(m, o, r), candidate(c).
.decl subject(m: Mid)
subject(m) :- item(m, "requirement", _, _, _, _).
.decl has(m: Mid)
has(m) :- result(m, _, _).
.decl failed(m: Mid, why: symbol)
failed(m, "fail") :- subject(m), result(m, "fail", _).
failed(m, "missing") :- subject(m), !has(m), source_complete("result").
.decl missing(m: Mid)
missing(m) :- subject(m), !has(m), !source_complete("result").
"#;

const ENTRIES: &str = r#"
[gates.verified]
program = ".mara/rules/gate.dl"
description = "Named requirements pass"
subjects = ["subject"]
conditions = ["failed"]
unavailable = ["missing"]
inputs = ["candidate", "result"]

[queries.read]
program = ".mara/rules/gate.dl"
relation = "read_result"
description = "The results as read, with the candidate"
inputs = ["candidate", "result"]
"#;

/// `REQ-A` fails, `NEED-ONE` (no subject) passes at the limits of a number column, `REQ-B` has
/// no result.
fn results() -> Value {
    json!([
        [mid(1), "fail", -2147483648_i64],
        [mid(3), "pass", 2147483647],
    ])
}

fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    sandbox.program("gate.dl", PROGRAM, ENTRIES);
    sandbox.write("results.json", &results().to_string());
    sandbox
}

const SUPPLIED: [&str; 4] = [
    "--input",
    "candidate=abc",
    "--input",
    "result=@results.json",
];

fn gate(sandbox: &Sandbox, extra: &[&str]) -> Run {
    let mut args = vec![
        "query", "gate", "verified", "--id", "REQ-A", "--id", "REQ-B",
    ];
    args.extend(extra);
    let run = sandbox.mara(&args);
    assert_eq!(run.spawns(), run.runs(), "Mara starts no other process");
    run
}

fn outcomes(run: &Run) -> Vec<(String, String)> {
    let items = run.json["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{run:#?}"));
    (items.iter())
        .map(|item| {
            let id = item["item"]["id"].as_str().unwrap().to_owned();
            (id, item["outcome"].as_str().unwrap().to_owned())
        })
        .collect()
}

#[test]
fn the_program_reads_exactly_the_supplied_rows() {
    let sandbox = sandbox();
    let mut args = vec!["query", "execute", "read"];
    args.extend(SUPPLIED);
    let run = sandbox.mara(&args);
    assert_eq!(run.status, 0, "{run:#?}");
    let rows: Vec<&Value> = (run.json["rows"].as_array().unwrap().iter())
        .map(|row| &row["values"])
        .collect();
    assert_eq!(
        rows,
        [
            &json!([mid(1), "fail", -2147483648_i64, "abc"]),
            &json!([mid(3), "pass", 2147483647, "abc"]),
        ],
        "read as given"
    );
    assert_eq!((run.runs(), run.spawns()), (1, 1));
}

#[test]
fn a_fail_row_fails_its_item_and_a_missing_result_is_unavailable_until_complete() {
    let sandbox = sandbox();
    let run = gate(&sandbox, &SUPPLIED);
    assert_eq!(run.status, 1, "{run:#?}");
    let expected = |b: &str| {
        vec![
            ("REQ-A".into(), "failed".into()),
            ("REQ-B".into(), b.into()),
        ]
    };
    assert_eq!(outcomes(&run), expected("unavailable"));
    assert_eq!(
        run.json["items"][0]["rows"],
        json!([{"relation": "failed", "role": "condition", "values": [mid(1), "fail"]}])
    );
    let mut complete = SUPPLIED.to_vec();
    complete.extend(["--complete", "result"]);
    let run = gate(&sandbox, &complete);
    assert_eq!(outcomes(&run), expected("failed"));
    assert_eq!(
        run.json["items"][1]["rows"],
        json!([{"relation": "failed", "role": "condition", "values": [mid(2), "missing"]}])
    );
    assert_eq!((run.runs(), run.spawns()), (1, 1));
}

/// Refused, naming the input, row or column, before any run.
#[track_caller]
fn refused(sandbox: &Sandbox, extra: &[&str], code: &str, named: &[&str]) {
    let run = gate(sandbox, extra);
    assert_eq!(run.status, 1, "{extra:?}: {run:#?}");
    assert_eq!(run.json["error"]["code"], code, "{extra:?}: {run:#?}");
    let message = run.json["error"]["message"].as_str().unwrap();
    for part in named {
        assert!(message.contains(part), "{extra:?}: {message}");
    }
    assert_eq!((run.runs(), run.spawns()), (0, 0), "{extra:?}");
}

#[test]
fn an_omitted_unlisted_or_repeated_input_is_refused_before_any_run() {
    let sandbox = sandbox();
    refused(
        &sandbox,
        &["--input", "candidate=abc"],
        "missing-input",
        &["`result`"],
    );
    let mut extra = SUPPLIED.to_vec();
    extra.extend(["--input", "other=1"]);
    refused(&sandbox, &extra, "unlisted-input", &["`other`"]);
    let mut extra = SUPPLIED.to_vec();
    extra.extend(["--complete", "other"]);
    refused(&sandbox, &extra, "unlisted-input", &["`other`"]);
    let mut extra = SUPPLIED.to_vec();
    extra.extend(["--input", "candidate=def"]);
    refused(&sandbox, &extra, "repeated-input", &["`candidate`"]);
}

#[test]
fn a_wrong_row_is_refused_naming_its_row_and_column() {
    let sandbox = sandbox();
    let a = mid(1);
    let cases = [
        (json!([[a, "fail"]]), "row 1: 2 columns"),
        (json!([[a, "fail", 1, 2]]), "row 1: 4 columns"),
        (json!([[1, "fail", 1]]), "row 1, column 1"),
        (json!([[a, "fail", "1"]]), "row 1, column 3"),
        (json!([[a, "fail", 1.5]]), "row 1, column 3"),
        (json!([[a, "fail", 2147483648_i64]]), "row 1, column 3"),
        (json!([[a, "fail", -2147483649_i64]]), "row 1, column 3"),
        (json!({"rows": []}), "not a JSON list of rows"),
        (json!([[a, "fail", 1], "x"]), "row 2"),
    ];
    for (rows, expected) in cases {
        sandbox.write("wrong.json", &rows.to_string());
        let extra = ["--input", "candidate=abc", "--input", "result=@wrong.json"];
        refused(&sandbox, &extra, "invalid-input", &["`result`", expected]);
    }
    sandbox.write("broken.json", "[[\"a\", ");
    let extra = ["--input", "candidate=abc", "--input", "result=@broken.json"];
    refused(&sandbox, &extra, "invalid-input", &["is not JSON"]);
    let extra = [
        "--input",
        "candidate=abc",
        "--input",
        "result=@missing.json",
    ];
    refused(&sandbox, &extra, "invalid-input", &["cannot be read"]);
}

/// Nothing supplied is stored: a later request without the inputs is refused as omitting them,
/// and the project tree is as it was.
#[test]
fn a_later_request_without_inputs_is_refused_with_the_project_tree_unchanged() {
    let sandbox = sandbox();
    let before = sandbox.files();
    let tmp = sandbox.tmp_entries();
    let mut complete = SUPPLIED.to_vec();
    complete.extend(["--complete", "result"]);
    assert_eq!(gate(&sandbox, &complete).runs(), 1);
    refused(&sandbox, &[], "missing-input", &["`candidate`"]);
    refused(
        &sandbox,
        &["--input", "candidate=abc"],
        "missing-input",
        &["`result`"],
    );
    assert!(sandbox.files() == before, "a file changed");
    assert_eq!(sandbox.tmp_entries(), tmp);
}
