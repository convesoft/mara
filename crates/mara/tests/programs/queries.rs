//! The query, view, gate and `query list` cases of VER-PROGRAM-CONSUMERS (DES-RULE-CONSUMERS,
//! DES-GATE-OUTCOME, DES-QUERY-RESULTS).

use serde_json::{Value, json};

use crate::{Run, Sandbox, invariant, item, mid};

/// A query and a view on the same relation, an invariant, and a gate.
const PROGRAM: &str = r#".include "mara.dl"
.decl accepted(m: Mid, id: Id)
accepted(m, i) :- item(m, _, i, "accepted", _, _).
.decl retired(m: Mid)
retired(m) :- item(m, _, _, "retired", _, _).
.decl not_listed(m: Mid)
not_listed(m) :- accepted(m, _).
.decl candidate(c: symbol)
.decl subject(m: symbol)
subject(m) :- accepted(m, _), candidate(_).
.decl condition(m: symbol)
condition(m) :- retired(m).
"#;

fn entries() -> String {
    let entry = |table: &str, name: &str, keys: &str| {
        format!("\n[{table}.{name}]\nprogram = \".mara/rules/p.dl\"\n{keys}\n")
    };
    entry(
        "queries",
        "accepted",
        "relation = \"accepted\"\ndescription = \"Accepted items\"",
    ) + &entry(
        "views",
        "accepted-table",
        "relation = \"accepted\"\ndescription = \"Accepted items as a table\"",
    ) + &invariant("retired-items", "p.dl", "retired", "error")
        + &entry(
            "gates",
            "release",
            "description = \"Named items are accepted\"\nsubjects = [\"subject\"]\n\
             conditions = [\"condition\"]\ninputs = [\"candidate\"]",
        )
}

fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    sandbox.program("p.dl", PROGRAM, &entries());
    sandbox
}

/// The values of each row of a `query execute` or `query view` result.
fn values(run: &Run) -> Vec<Value> {
    assert_eq!(run.status, 0, "{run:#?}");
    let rows = run.json["rows"].as_array().unwrap().iter();
    rows.map(|row| row["values"].clone()).collect()
}

#[test]
fn query_list_shows_every_entry_and_nothing_else() {
    let sandbox = sandbox();
    let run = sandbox.mara(&["query", "list"]);
    assert_eq!(run.status, 0, "{run:#?}");
    assert_eq!(
        run.json["entries"],
        json!([
            {"name": "accepted", "kind": "query", "description": "Accepted items", "inputs": []},
            {"name": "accepted-table", "kind": "view", "description": "Accepted items as a table",
             "inputs": []},
            {"name": "retired-items", "kind": "invariant", "description": "retired-items failed",
             "inputs": []},
            {"name": "release", "kind": "gate", "description": "Named items are accepted",
             "inputs": ["candidate"]},
        ])
    );
    assert_eq!((run.runs(), run.spawns()), (0, 0));
    assert_eq!(run.stats["parses"], 0, "no document is read");
}

#[test]
fn a_query_and_a_view_of_the_same_relation_give_the_same_rows() {
    let sandbox = sandbox();
    let query = sandbox.mara(&["query", "execute", "accepted"]);
    let view = sandbox.mara(&["query", "view", "accepted-table"]);
    let rows = values(&query);
    assert_eq!(
        rows,
        [json!([mid(1), "REQ-A"]), json!([mid(3), "NEED-ONE"])],
        "{query:#?}"
    );
    assert_eq!(values(&view), rows);
    assert_eq!(query.json["query"]["relation"], "accepted");
    assert_eq!(
        query.json["query"]["columns"],
        json!([{"name": "m", "type": "symbol"}, {"name": "id", "type": "symbol"}])
    );
    assert_eq!(query.json["rows"][0]["item"]["id"], "REQ-A");
    for run in [&query, &view] {
        assert_eq!((run.runs(), run.spawns()), (1, 1));
    }
    // The human form of the view is a table under the column names.
    let output = sandbox
        .command(&["query", "view", "accepted-table"])
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.starts_with(&format!(
            "| m | id |\n| --- | --- |\n| {} | REQ-A |\n",
            mid(1)
        )),
        "{text}"
    );
}

#[test]
fn a_query_naming_items_gets_only_their_rows() {
    let sandbox = sandbox();
    let run = sandbox.mara(&[
        "query",
        "execute",
        "accepted",
        "--id",
        "NEED-ONE",
        "--id",
        &mid(2),
    ]);
    assert_eq!(values(&run), [json!([mid(3), "NEED-ONE"])]);
    let run = sandbox.mara(&[
        "query", "execute", "accepted", "--id", "REQ-A", "--id", "NEED-ONE",
    ]);
    assert_eq!(values(&run).len(), 2);
    // An item that no item has refuses the request before any run.
    let run = sandbox.mara(&["query", "execute", "accepted", "--id", "REQ-NONE"]);
    assert_eq!(run.json["error"]["code"], "unknown-item", "{run:#?}");
    assert_eq!(run.runs(), 0);
}

#[test]
fn a_name_that_no_entry_has_is_refused_before_any_run() {
    let sandbox = sandbox();
    for (args, named) in [
        (&["query", "execute", "not_listed"][..], "`not_listed`"),
        (
            &["query", "execute", "accepted-table"],
            "`accepted-table` is a view",
        ),
        (
            &["query", "view", "retired-items"],
            "`retired-items` is an invariant",
        ),
        (
            &["query", "gate", "accepted", "--id", "REQ-A"],
            "`accepted` is a query",
        ),
    ] {
        let run = sandbox.mara(args);
        assert_eq!(run.status, 1, "{args:?}: {run:#?}");
        assert_eq!(run.json["error"]["code"], "unknown-entry", "{args:?}");
        let message = run.json["error"]["message"].as_str().unwrap();
        assert!(message.contains(named), "{args:?}: {message}");
        assert_eq!((run.runs(), run.spawns()), (0, 0), "{args:?}");
    }
}

#[test]
fn a_rule_added_between_queries_counts_in_the_second() {
    let sandbox = sandbox();
    assert_eq!(
        values(&sandbox.mara(&["query", "execute", "accepted"])).len(),
        2
    );
    let added = format!("{PROGRAM}accepted(m, i) :- item(m, _, i, \"draft\", _, _).\n");
    sandbox.write(".mara/rules/p.dl", &added);
    assert_eq!(
        values(&sandbox.mara(&["query", "execute", "accepted"])).len(),
        3
    );
}

/// Four named items: `REQ-A` without a subject row; `REQ-B` with a condition and an unavailable
/// row; `NEED-ONE` with only an unavailable row; `REQ-D` with nothing else.
const GATE: &str = r#".decl subject(m: symbol)
subject("M2"). subject("M3"). subject("M4").
.decl condition(m: symbol, why: symbol)
condition("M2", "broken").
.decl unknown(m: symbol)
unknown("M2"). unknown("M3").
"#;

fn gate_sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    let program = (2..=4).fold(GATE.to_owned(), |text, n| {
        text.replace(&format!("\"M{n}\""), &format!("\"{}\"", mid(n)))
    });
    let entry = "\n[gates.four]\nprogram = \".mara/rules/gate.dl\"\ndescription = \"d\"\n\
                 subjects = [\"subject\"]\nconditions = [\"condition\"]\n\
                 unavailable = [\"unknown\"]\n";
    sandbox.program("gate.dl", &program, entry);
    let d = item(4, "Requirement", "REQ-D", "Draft", "Delta", &[]);
    sandbox.write("docs/b.mara.md", &format!("# B\n\n---\n\n{d}"));
    sandbox
}

fn gate(sandbox: &Sandbox, ids: &[&str]) -> Run {
    let mut args = vec!["query", "gate", "four"];
    for id in ids {
        args.extend(["--id", id]);
    }
    sandbox.mara(&args)
}

#[test]
fn a_gate_judges_each_named_item_by_its_rows() {
    let sandbox = gate_sandbox();
    let run = gate(&sandbox, &["REQ-A", "REQ-B", "NEED-ONE", "REQ-D"]);
    assert_eq!(run.status, 1, "{run:#?}");
    assert_eq!(
        run.json["gate"],
        json!({"name": "four", "outcome": "failed"})
    );
    let items = run.json["items"].as_array().unwrap();
    let judged: Vec<(&Value, &Value, &Value)> = items
        .iter()
        .map(|item| (&item["item"]["id"], &item["outcome"], &item["rows"]))
        .collect();
    let row = |relation: &str, role: &str, values: Value| json!({"relation": relation, "role": role, "values": values});
    assert_eq!(
        judged,
        [
            (&json!("REQ-A"), &json!("not-applicable"), &json!([])),
            (
                &json!("REQ-B"),
                &json!("failed"),
                &json!([row("condition", "condition", json!([mid(2), "broken"]))])
            ),
            (
                &json!("NEED-ONE"),
                &json!("unavailable"),
                &json!([row("unknown", "unavailable", json!([mid(3)]))])
            ),
            (
                &json!("REQ-D"),
                &json!("passed"),
                &json!([row("subject", "subject", json!([mid(4)]))])
            ),
        ]
    );
    assert_eq!((run.runs(), run.spawns()), (1, 1));
}

#[test]
fn a_gate_passes_only_when_every_named_item_passed() {
    let sandbox = gate_sandbox();
    let run = gate(&sandbox, &["NEED-ONE", "REQ-D"]);
    assert_eq!(
        (run.status, &run.json["gate"]["outcome"]),
        (1, &json!("unavailable")),
        "{run:#?}"
    );
    let run = gate(&sandbox, &["REQ-D"]);
    assert_eq!(
        (run.status, &run.json["gate"]["outcome"]),
        (0, &json!("passed")),
        "{run:#?}"
    );
    // An item named twice is judged once.
    let run = gate(&sandbox, &["REQ-D", &mid(4)]);
    assert_eq!(run.json["items"].as_array().unwrap().len(), 1, "{run:#?}");
}

#[test]
fn a_gate_request_that_names_no_item_is_refused_before_any_run() {
    let sandbox = gate_sandbox();
    let run = gate(&sandbox, &[]);
    assert_eq!(run.status, 2, "{run:#?}");
    assert_eq!(run.json["error"]["code"], "invalid-argument");
    assert_eq!((run.runs(), run.spawns()), (0, 0));
}

#[test]
fn a_failed_gate_does_not_change_validation() {
    let sandbox = gate_sandbox();
    let before = sandbox.validate();
    let files = sandbox.files();
    assert_eq!(
        gate(&sandbox, &["REQ-A", "REQ-B"]).json["gate"]["outcome"],
        "failed"
    );
    let after = sandbox.validate();
    assert_eq!(after.status, before.status);
    assert_eq!(after.json, before.json);
    assert!(sandbox.files() == files, "a file changed");
}

/// An entry that names a relation its program does not declare fails the request that
/// evaluates it, as a compile error does; a gate and its items are then unavailable. Other
/// operations do not refuse the project (DES-RULE-CONSUMERS).
#[test]
fn an_entry_that_does_not_fit_its_program_fails_the_request_that_evaluates_it() {
    let sandbox = sandbox();
    let entries = entries()
        + "\n[queries.wrong]\nprogram = \".mara/rules/p.dl\"\nrelation = \"absent\"\n\
           description = \"d\"\n\n[gates.wrong-gate]\nprogram = \".mara/rules/p.dl\"\n\
           description = \"d\"\nsubjects = [\"accepted\"]\nconditions = [\"absent\"]\n";
    sandbox.program("p.dl", PROGRAM, &entries);
    let run = sandbox.mara(&["query", "execute", "wrong"]);
    assert_eq!(run.json["error"]["code"], "program-failed", "{run:#?}");
    let diagnostics = run.json["error"]["diagnostics"].as_array().unwrap();
    assert_eq!(
        diagnostics.len(),
        1,
        "only the entry's own problem: {run:#?}"
    );
    assert_eq!(diagnostics[0]["code"], "undeclared-relation");
    assert_eq!(run.runs(), 0);
    let run = sandbox.mara(&["query", "gate", "wrong-gate", "--id", "REQ-A"]);
    assert_eq!(run.status, 1, "{run:#?}");
    assert_eq!(run.json["gate"]["outcome"], "unavailable");
    assert_eq!(run.json["items"][0]["outcome"], "unavailable");
    assert_eq!(run.json["diagnostics"][0]["code"], "undeclared-relation");
    // The query that fits still runs, and reads still work.
    assert_eq!(
        values(&sandbox.mara(&["query", "execute", "accepted"])).len(),
        2
    );
    assert_eq!(sandbox.mara(&["item", "list"]).status, 0);
    assert_eq!(sandbox.mara(&["query", "list"]).status, 0);
}

/// A configuration with an error refuses the query operations, as every read.
#[test]
fn a_configuration_error_refuses_the_query_operations() {
    let sandbox = sandbox();
    let broken = entries() + "\n[queries.Bad]\nprogram = \".mara/rules/p.dl\"\n";
    sandbox.program("p.dl", PROGRAM, &broken);
    for args in [
        &["query", "list"][..],
        &["query", "execute", "accepted"],
        &[
            "query",
            "gate",
            "release",
            "--id",
            "REQ-A",
            "--input",
            "candidate=x",
        ],
    ] {
        let run = sandbox.mara(args);
        assert_eq!(run.status, 1, "{args:?}: {run:#?}");
        assert_eq!(
            run.json["error"]["code"], "invalid-project",
            "{args:?}: {run:#?}"
        );
        let diagnostics = run.json["error"]["diagnostics"].as_array().unwrap();
        assert!(!diagnostics.is_empty(), "{args:?}: {run:#?}");
        assert!(
            diagnostics.iter().all(|d| d["path"] == ".mara/config.toml"),
            "{args:?}: {run:#?}"
        );
        // A refusal, before any outcome: no gate, no item, no row.
        for member in ["gate", "items", "rows", "entries"] {
            assert!(run.json.get(member).is_none(), "{args:?}: {run:#?}");
        }
        assert_eq!((run.runs(), run.spawns()), (0, 0));
    }
}

/// A fault diagnostic larger than a page's budget for the inventory's diagnostics stays in the
/// gate's result, as in the failure of a query (DES-QUERY-RESULTS).
#[test]
fn a_gate_keeps_a_fault_diagnostic_of_any_size() {
    let sandbox = sandbox();
    let comment = "x".repeat(9000);
    let program = format!("{PROGRAM}condition(m) :- absent(m). //{comment}\n");
    let line = program.lines().count() as u64;
    sandbox.program("p.dl", &program, &entries());
    let args = [
        "query",
        "gate",
        "release",
        "--id",
        "REQ-A",
        "--input",
        "candidate=x",
    ];
    let run = sandbox.mara(&args);
    assert_eq!(run.status, 1, "{run:#?}");
    assert_eq!(run.json["gate"]["outcome"], "unavailable");
    assert_eq!(run.json["items"][0]["outcome"], "unavailable");
    assert_eq!(run.json["diagnostics_total"], 1, "{run:#?}");
    let shown = run.diagnostics();
    assert_eq!(shown.len(), 1, "{run:#?}");
    assert_eq!(
        (&shown[0]["code"], &shown[0]["path"], &shown[0]["line"]),
        (
            &json!("program-error"),
            &json!(".mara/rules/p.dl"),
            &json!(line)
        )
    );
    assert!(shown[0]["message"].as_str().unwrap().contains(&comment));
    // The human form shows it too.
    let output = sandbox.command(&args).output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("error[program-error]") && text.contains(&comment),
        "{text}"
    );
    // The query keeps it in its failure.
    let run = sandbox.mara(&["query", "execute", "accepted"]);
    assert_eq!(run.json["error"]["code"], "program-failed", "{run:#?}");
    let message = run.json["error"]["diagnostics"][0]["message"]
        .as_str()
        .unwrap();
    assert!(message.contains(&comment));
}

/// Rows are paged like every read: following the cursors gives each row once.
#[test]
fn query_rows_are_paged() {
    let sandbox = sandbox();
    let first = sandbox.mara(&["query", "execute", "accepted", "--limit", "1"]);
    assert_eq!(values(&first), [json!([mid(1), "REQ-A"])]);
    let cursor = first.json["next_cursor"].as_str().unwrap();
    let second = sandbox.mara(&[
        "query", "execute", "accepted", "--limit", "1", "--cursor", cursor,
    ]);
    assert_eq!(values(&second), [json!([mid(3), "NEED-ONE"])]);
    assert_eq!(second.json["next_cursor"], Value::Null);
    let other = sandbox.mara(&["query", "view", "accepted-table", "--cursor", cursor]);
    assert_eq!(other.json["error"]["code"], "stale-cursor", "{other:#?}");
}
