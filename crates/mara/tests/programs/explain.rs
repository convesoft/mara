//! VER-EXPLANATIONS (REQ-EXPLAIN-OUTCOMES, DES-RULE-EXECUTION, DES-QUERY-RESULTS): `query
//! report` of rows of invariants and gates, compared with what `get` and `relation list` give
//! for each leaf, and with the outcomes of `project validate` and `query gate`.

use std::fs;

use serde_json::{Value, json};

use crate::{Run, SCHEMA, Sandbox, invariant, item, mid};

/// Every node of a proof, depth first.
fn nodes(proof: &Value) -> Vec<&Value> {
    let mut found = vec![proof];
    for child in proof["children"].as_array().map_or(&[][..], Vec::as_slice) {
        found.extend(nodes(child));
    }
    found
}

/// The leaves of a proof: its facts, inputs and negated literals.
fn leaves(proof: &Value) -> Vec<&Value> {
    nodes(proof)
        .into_iter()
        .filter(|node| node.get("children").is_none())
        .collect()
}

/// `query report <name> --id <id>` with `extra` arguments; also requires the report to change
/// nothing that `project validate` returns.
fn report(sandbox: &Sandbox, name: &str, id: &str, extra: &[&str]) -> Run {
    let before = sandbox.validate();
    let mut args = vec!["query", "report", name, "--id", id];
    args.extend(extra);
    let run = sandbox.mara(&args);
    assert_eq!((run.runs(), run.spawns()), (1, 1), "{run:#?}");
    let after = sandbox.validate();
    assert_eq!(before.json, after.json, "the report changed validation");
    run
}

/// The only row of a report, with its proof.
#[track_caller]
fn only_proof(run: &Run) -> &Value {
    assert_eq!(run.status, 0, "{run:#?}");
    let rows = run.json["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{run:#?}");
    let proof = &rows[0]["proof"];
    assert!(proof.is_object(), "{run:#?}");
    proof
}

const JOIN: &str = r#".include "mara.dl"
.decl joined(m: Mid, n: Mid, o: symbol)
joined(m, n, o) :- item(m, "requirement", _, "accepted", _, _), field(m, "owner", o),
    edge("satisfies", m, n, _, _).
.decl result(m: Mid, r: symbol)
.decl subject(m: Mid)
subject(m) :- item(m, "requirement", _, _, _, _).
.decl condition(m: Mid, r: symbol)
condition(m, r) :- subject(m), result(m, r), r != "pass".
"#;

fn join_entries() -> String {
    invariant("joined", "p.dl", "joined", "error")
        + "\n[gates.release]\nprogram = \".mara/rules/p.dl\"\ndescription = \"d\"\n\
           subjects = [\"subject\"]\nconditions = [\"condition\"]\ninputs = [\"result\"]\n"
}

#[test]
fn an_invariant_row_is_explained_down_to_its_item_and_relation_rows() {
    let sandbox = Sandbox::new();
    sandbox.program("p.dl", JOIN, &join_entries());
    let run = report(&sandbox, "joined", "REQ-A", &[]);
    assert_eq!(run.json["report"]["kind"], "invariant");
    assert_eq!(run.json["report"]["outcome"], "failed");
    assert_eq!(run.json["report"]["item"]["id"], "REQ-A");
    assert_eq!(
        run.json["rows"][0]["values"],
        json!([mid(1), mid(3), "abc"])
    );
    assert!(run.json["rows"][0].get("role").is_none(), "{run:#?}");
    let proof = only_proof(&run);
    assert_eq!(proof["relation"], "joined");
    assert_eq!(
        proof["atom"],
        format!("joined(\"{}\", \"{}\", \"abc\")", mid(1), mid(3))
    );
    assert!(
        proof["rule"]
            .as_str()
            .unwrap()
            .starts_with("joined(m,n,o) :- ")
    );
    // Each leaf is at the path and line `get` and `relation list` give for its item or row.
    let get = sandbox.mara(&["get", "REQ-A"]);
    let item_at = (&get.json["node"]["path"], &get.json["node"]["line"]);
    let relations = sandbox.mara(&["relation", "list", "REQ-A"]);
    let source = &relations.json["connections"][0]["source"];
    let leaves = leaves(proof);
    let at = |leaf: &Value| (leaf["path"].clone(), leaf["line"].clone());
    assert_eq!(leaves.len(), 3, "{proof:#}");
    assert_eq!(leaves[0]["relation"], "item");
    assert_eq!(leaves[0]["values"][0], mid(1));
    assert_eq!(at(leaves[0]), (item_at.0.clone(), item_at.1.clone()));
    assert_eq!(leaves[1]["relation"], "field");
    assert_eq!(leaves[1]["values"], json!([mid(1), "owner", "abc"]));
    assert_eq!(at(leaves[1]), (item_at.0.clone(), item_at.1.clone()));
    assert_eq!(leaves[2]["relation"], "edge");
    assert_eq!(
        leaves[2]["values"],
        json!(["satisfies", mid(1), mid(3), source["path"], source["line"]])
    );
    assert_eq!(
        at(leaves[2]),
        (source["path"].clone(), source["line"].clone())
    );
    // An item the invariant does not fail: passed, nothing to explain.
    let run = report(&sandbox, "joined", "NEED-ONE", &[]);
    assert_eq!(run.status, 0, "{run:#?}");
    assert_eq!(run.json["report"]["outcome"], "passed");
    assert_eq!(run.json["rows"], json!([]));
    // The human form prints the proof one step per line.
    let output = sandbox
        .command(&["query", "report", "joined", "--id", "REQ-A"])
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("invariant joined: docs/a.mara.md:"), "{text}");
    assert!(text.contains(&format!(
        "    field({}, owner, abc) at docs/a.mara.md:",
        mid(1)
    )));
}

#[test]
fn a_gate_condition_row_names_the_supplied_input() {
    let sandbox = Sandbox::new();
    sandbox.program("p.dl", JOIN, &join_entries());
    sandbox.write("result.json", &json!([[mid(1), "fail"]]).to_string());
    let input = ["--input", "result=@result.json"];
    let gate = sandbox.mara(&[
        "query", "gate", "release", "--id", "REQ-A", input[0], input[1],
    ]);
    let run = report(&sandbox, "release", "REQ-A", &input);
    assert_eq!(run.json["report"]["kind"], "gate");
    assert_eq!(
        run.json["report"]["outcome"],
        gate.json["items"][0]["outcome"]
    );
    assert_eq!(run.json["report"]["outcome"], "failed");
    assert_eq!(run.json["rows"][0]["role"], "condition");
    assert_eq!(run.json["rows"][0]["relation"], "condition");
    let proof = only_proof(&run);
    let leaves = leaves(proof);
    assert_eq!(leaves.len(), 2, "{proof:#}");
    assert_eq!(leaves[0]["relation"], "item");
    assert_eq!(
        leaves[1],
        &json!({"input": "result", "values": [mid(1), "fail"]})
    );
    // The constraint `r != "pass"` is left out.
    assert_eq!(proof["children"].as_array().unwrap().len(), 2, "{proof:#}");
    // A passed item explains its subject rows; one without a subject row has none.
    sandbox.write("result.json", &json!([[mid(1), "pass"]]).to_string());
    let run = report(&sandbox, "release", "REQ-A", &input);
    assert_eq!(run.json["report"]["outcome"], "passed");
    assert_eq!(run.json["rows"][0]["role"], "subject");
    only_proof(&run);
    let run = report(&sandbox, "release", "NEED-ONE", &input);
    assert_eq!(
        (run.status, &run.json["report"]["outcome"]),
        (0, &json!("not-applicable"))
    );
    // The gate takes its inputs as `query gate` does.
    let run = sandbox.mara(&["query", "report", "release", "--id", "REQ-A"]);
    assert_eq!(run.json["error"]["code"], "missing-input", "{run:#?}");
    assert_eq!(run.runs(), 0);
}

#[test]
fn negation_records_components_and_eqrel_are_explained() {
    let program = r#".include "mara.dl"
.decl incoming(m: Mid)
incoming(m) :- edge(_, _, m, _, _).
.decl lonely(m: Mid)
lonely(m) :- item(m, "requirement", _, "accepted", _, _), !incoming(m).
.decl unlinked(m: Mid)
unlinked(m) :- item(m, "requirement", _, "draft", _, _), !edge(_, m, _, _, _).
.type At = [m: symbol, line: number]
.decl at(a: At)
at([m, l]) :- item(m, _, _, _, _, l).
.decl placed(m: Mid)
placed(m) :- at([m, _]), item(m, "need", _, _, _, _).
.comp Typed<T> {
    .decl of(m: T)
    of(m) :- item(m, "need", _, _, _, _).
}
.init needs = Typed<Mid>
.decl component(m: Mid)
component(m) :- needs.of(m).
.decl linked(a: Mid, b: Mid) eqrel
linked(a, b) :- edge("satisfies", a, b, _, _).
.decl connected(m: Mid)
connected(m) :- linked(m, x), x != m, item(x, "requirement", _, _, _, _).
"#;
    let entries = ["lonely", "unlinked", "placed", "component", "connected"]
        .map(|name| invariant(name, "p.dl", name, "warning"))
        .concat();
    let sandbox = Sandbox::new();
    sandbox.program("p.dl", program, &entries);
    for (name, id) in [
        ("lonely", "REQ-A"),
        ("unlinked", "REQ-B"),
        ("placed", "NEED-ONE"),
        ("component", "NEED-ONE"),
        ("connected", "NEED-ONE"),
    ] {
        let run = report(&sandbox, name, id, &[]);
        let proof = only_proof(&run);
        assert_eq!(proof["relation"], name, "{run:#?}");
        for leaf in leaves(proof) {
            assert!(
                leaf.get("path").is_some() || leaf.get("absent").is_some(),
                "{name}: {proof:#}"
            );
        }
    }
    // A negated literal without a wildcard as Soufflé prints it, one with wildcards as the rule
    // writes it: Soufflé prints arbitrary values for wildcards.
    let run = report(&sandbox, "lonely", "REQ-A", &[]);
    let absent: Vec<&Value> = leaves(only_proof(&run))
        .into_iter()
        .filter(|leaf| leaf.get("absent").is_some())
        .collect();
    assert_eq!(
        absent,
        [&json!({"absent": format!("incoming(\"{}\")", mid(1))})]
    );
    let run = report(&sandbox, "unlinked", "REQ-B", &[]);
    let absent: Vec<&Value> = leaves(only_proof(&run))
        .into_iter()
        .filter(|leaf| leaf.get("absent").is_some())
        .collect();
    assert_eq!(absent, [&json!({"absent": "edge(_,m,_,_,_)"})]);
    // The eqrel step derived by symmetry rests on the `Satisfies` row of REQ-A.
    let run = report(&sandbox, "connected", "NEED-ONE", &[]);
    let edges: Vec<&Value> = leaves(only_proof(&run))
        .into_iter()
        .filter(|leaf| leaf["relation"] == "edge")
        .collect();
    assert_eq!(edges.len(), 1, "{run:#?}");
    assert_eq!(edges[0]["values"][1], mid(1));
}

#[test]
fn an_ancestry_closure_over_eight_links_is_expanded() {
    let sandbox = Sandbox::new();
    let schema = format!(
        "{SCHEMA}\n[relations.decomposes]\ninverse = \"decomposed_by\"\n\
         description = \"The source refines the target.\"\n\
         targets = {{ requirement = [\"requirement\"] }}\n"
    );
    sandbox.write(".mara/schema.toml", &schema);
    let chain: Vec<String> = (0..9)
        .map(|n| {
            let rows = match n {
                8 => Vec::new(),
                _ => vec![(
                    "Decomposes".to_owned(),
                    format!("[C{}](#{})", n + 1, mid(20 + n + 1)),
                )],
            };
            let rows: Vec<(&str, &str)> =
                rows.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            item(
                20 + n,
                "Requirement",
                &format!("C{n}"),
                "Draft",
                &format!("Step {n}"),
                &rows,
            )
        })
        .collect();
    sandbox.write(
        "docs/chain.mara.md",
        &format!("# Chain\n\n---\n\n{}", chain.join("\n---\n\n")),
    );
    let program = r#".include "mara.dl"
.decl above(a: Mid, b: Mid)
above(a, b) :- edge("decomposes", a, b, _, _).
above(a, c) :- above(a, b), edge("decomposes", b, c, _, _).
.decl deep(a: Mid, b: Mid)
deep(a, b) :- above(a, b), item(a, _, "C0", _, _, _), item(b, _, "C8", _, _, _).
"#;
    sandbox.program(
        "p.dl",
        program,
        &invariant("deep", "p.dl", "deep", "warning"),
    );
    let run = report(&sandbox, "deep", "C0", &[]);
    let proof = only_proof(&run);
    assert!(!proof.to_string().contains("subproof"), "{proof:#}");
    let mut edges: Vec<(String, u64)> = leaves(proof)
        .into_iter()
        .filter(|leaf| leaf["relation"] == "edge")
        .map(|leaf| {
            assert_eq!(leaf["path"], "docs/chain.mara.md");
            (
                leaf["values"][1].as_str().unwrap().to_owned(),
                leaf["line"].as_u64().unwrap(),
            )
        })
        .collect();
    edges.sort();
    let sources: Vec<String> = edges.iter().map(|(source, _)| source.clone()).collect();
    assert_eq!(sources, (20..28).map(mid).collect::<Vec<_>>());
    // Each at the line `relation list` gives for its row.
    for (n, (_, line)) in edges.iter().enumerate() {
        let relations = sandbox.mara(&["relation", "list", &format!("C{n}")]);
        let outgoing = (relations.json["connections"].as_array().unwrap().iter())
            .find(|c| c["direction"] == "outgoing")
            .unwrap();
        assert_eq!(outgoing["source"]["line"], *line);
    }
}

#[test]
fn a_comparison_of_symbols_leaves_no_constraint_leaf() {
    let program = r#".include "mara.dl"
.decl early(m: Mid)
early(m) :- item(m, _, i, _, _, _), i < "Z".
"#;
    let sandbox = Sandbox::new();
    sandbox.program(
        "p.dl",
        program,
        &invariant("early", "p.dl", "early", "warning"),
    );
    let run = report(&sandbox, "early", "REQ-A", &[]);
    let proof = only_proof(&run);
    let children = proof["children"].as_array().unwrap();
    assert_eq!(children.len(), 1, "{proof:#}");
    assert_eq!(children[0]["relation"], "item");
}

#[test]
fn values_the_command_cannot_take_are_not_explained() {
    let program = r#".include "mara.dl"
.decl result(m: Mid, r: symbol)
.decl score(m: Mid, n: number)
.decl subject(m: Mid)
subject(m) :- item(m, "requirement", _, _, _, _).
.decl condition(m: Mid, r: symbol)
condition(m, r) :- subject(m), result(m, r).
.decl low(m: Mid, n: number)
low(m, n) :- subject(m), score(m, n).
.decl titled(m: Mid, t: symbol)
titled(m, t) :- title(m, t), item(m, "requirement", _, "draft", _, _).
"#;
    let entries = "\n[gates.release]\nprogram = \".mara/rules/p.dl\"\ndescription = \"d\"\n\
                   subjects = [\"subject\"]\nconditions = [\"condition\", \"low\"]\n\
                   inputs = [\"result\", \"score\"]\n"
        .to_owned()
        + &invariant("titled", "p.dl", "titled", "warning");
    let sandbox = Sandbox::new();
    sandbox.program("p.dl", program, &entries);
    // `a\nb` with a newline, and `a\\nb` with a backslash: the command cannot tell them apart.
    let results = json!([
        [mid(1), "a\nb"],
        [mid(1), "a\\nb"],
        [mid(1), "q\"x"],
        [mid(1), "plain"]
    ]);
    sandbox.write("result.json", &results.to_string());
    sandbox.write("score.json", &json!([[mid(1), -1]]).to_string());
    let input = [
        "--input",
        "result=@result.json",
        "--input",
        "score=@score.json",
    ];
    let mut gate = vec!["query", "gate", "release", "--id", "REQ-A"];
    gate.extend(input);
    let gate = sandbox.mara(&gate);
    let run = report(&sandbox, "release", "REQ-A", &input);
    assert_eq!(run.status, 1, "{run:#?}");
    assert_eq!(
        run.json["report"]["outcome"],
        gate.json["items"][0]["outcome"]
    );
    assert_eq!(run.json["report"]["outcome"], "failed");
    let rows = run.json["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 5, "{run:#?}");
    for row in rows {
        match row["values"][1].as_str() {
            Some("plain") => {
                let proof = &row["proof"];
                let atom = format!("condition(\"{}\", \"plain\")", mid(1));
                assert_eq!(proof["atom"], atom, "{run:#?}");
            }
            _ => {
                assert!(row.get("proof").is_none(), "{run:#?}");
                let reason = row["unexplained"].as_str().unwrap();
                assert!(reason.contains("cannot take"), "{reason}");
            }
        }
    }
    // A title holding a quote, from the documents.
    let text = fs::read_to_string(sandbox.project().join("docs/a.mara.md")).unwrap();
    sandbox.write(
        "docs/a.mara.md",
        &text.replace("### Héllo", "### Say \"hi\""),
    );
    let run = report(&sandbox, "titled", "REQ-B", &[]);
    assert_eq!(run.status, 1, "{run:#?}");
    assert_eq!(run.json["report"]["outcome"], "failed");
    assert!(
        run.json["rows"][0]["unexplained"]
            .as_str()
            .unwrap()
            .contains("quote")
    );
}

#[test]
fn a_proof_through_an_aggregate_is_not_explained() {
    let program = r#".include "mara.dl"
.decl counted(m: Mid, c: number)
counted(m, c) :- item(m, "need", _, _, _, _), c = count : item(_, _, _, _, _, _).
.decl total(c: number)
total(c) :- c = count : item(_, _, _, _, _, _).
.decl crowded(m: Mid)
crowded(m) :- item(m, "need", _, _, _, _), total(c), c > 1.
"#;
    let entries = invariant("counted", "p.dl", "counted", "warning")
        + &invariant("crowded", "p.dl", "crowded", "warning");
    let sandbox = Sandbox::new();
    sandbox.program("p.dl", program, &entries);
    for name in ["counted", "crowded"] {
        let run = report(&sandbox, name, "NEED-ONE", &[]);
        assert_eq!(run.status, 1, "{run:#?}");
        assert_eq!(run.json["report"]["outcome"], "failed", "{run:#?}");
        let reason = run.json["rows"][0]["unexplained"].as_str().unwrap();
        assert!(reason.contains("aggregate"), "{name}: {reason}");
    }
}

#[test]
fn a_report_names_one_item_of_an_invariant_or_a_gate() {
    let sandbox = Sandbox::new();
    sandbox.program(
        "p.dl",
        JOIN,
        &(join_entries()
            + "\n[queries.all]\nprogram = \".mara/rules/p.dl\"\nrelation = \"joined\"\n\
               description = \"d\"\n"),
    );
    let run = sandbox.mara(&["query", "report", "joined"]);
    assert_eq!(
        (run.status, &run.json["error"]["code"]),
        (2, &json!("invalid-argument"))
    );
    let run = sandbox.mara(&[
        "query", "report", "joined", "--id", "REQ-A", "--id", "REQ-B",
    ]);
    assert_eq!(
        (run.status, &run.json["error"]["code"]),
        (2, &json!("invalid-argument"))
    );
    let run = sandbox.mara(&["query", "report", "all", "--id", "REQ-A"]);
    assert_eq!(run.json["error"]["code"], "unknown-entry", "{run:#?}");
    let message = run.json["error"]["message"].as_str().unwrap();
    assert!(message.contains("not an invariant or a gate"), "{message}");
    let run = sandbox.mara(&["query", "report", "joined", "--id", "REQ-NONE"]);
    assert_eq!(run.json["error"]["code"], "unknown-item", "{run:#?}");
    assert_eq!(run.runs(), 0);
}

#[test]
fn a_fault_of_the_gate_program_is_reported_never_explained() {
    let program = ".include \"mara.dl\"\n.decl subject(m: Mid)\n\
                   subject(m) :- field(m, \"owner\", o), to_number(o) > 0.\n";
    let entries = "\n[gates.release]\nprogram = \".mara/rules/p.dl\"\ndescription = \"d\"\n\
                   subjects = [\"subject\"]\nconditions = [\"subject\"]\n";
    let sandbox = Sandbox::new();
    sandbox.program("p.dl", program, entries);
    let run = report(&sandbox, "release", "REQ-A", &[]);
    assert_eq!(run.status, 1, "{run:#?}");
    assert_eq!(run.json["report"]["outcome"], "unavailable");
    assert_eq!(run.json["rows"], json!([]));
    crate::assert_diagnostic(
        run.diagnostics(),
        "program-fault",
        ".mara/rules/p.dl",
        3,
        &[],
    );
}

/// Findings 1 to 3 of the review of `aeb4975`, through Soufflé: a negation whose value holds an
/// operator stays in the proof, a constant keeps its spaces in the printed rule, and each
/// constraint form that the parser admits is left out of an explained proof.
#[test]
fn values_and_constraints_are_read_as_soufflé_prints_them() {
    let program = r#".include "mara.dl"
.decl denied(m: Mid, reason: symbol)
denied(m, "else") :- item(m, _, "REQ-B", _, _, _).
.decl negated(m: Mid)
negated(m) :- item(m, _, "REQ-A", _, _, _), !denied(m, "a = b").
.decl spaced(m: Mid)
spaced(m) :- field(m, "owner", "a  b").
.decl compared(m: Mid)
compared(m) :- item(m, _, i, _, _, l), l > 0, l >= 1, l < 100000, l <= 99999, l != -7,
    !(l < 0), strlen(i) = 5, i != "a, \"b\"".
.decl containing(m: Mid)
containing(m) :- title(m, t), contains("lph", t).
.decl lacking(m: Mid)
lacking(m) :- field(m, "owner", o), !contains("@", o).
"#;
    let names = ["negated", "spaced", "compared", "containing", "lacking"];
    let entries = names
        .map(|name| invariant(name, "p.dl", name, "warning"))
        .concat();
    let sandbox = Sandbox::new();
    sandbox.program("p.dl", program, &entries);
    let text = fs::read_to_string(sandbox.project().join("docs/a.mara.md")).unwrap();
    sandbox.write(
        "docs/a.mara.md",
        &text.replace("| Owner | abc |", "| Owner | a  b |"),
    );
    let run = report(&sandbox, "negated", "REQ-A", &[]);
    let proof = only_proof(&run);
    assert_eq!(
        proof["children"][1],
        json!({"absent": format!("denied(\"{}\", \"a = b\")", mid(1))}),
        "{proof:#}"
    );
    let run = report(&sandbox, "spaced", "REQ-A", &[]);
    let proof = only_proof(&run);
    assert_eq!(proof["rule"], "spaced(m) :- field(m,\"owner\",\"a  b\").");
    assert_eq!(proof["children"][0]["values"][2], "a  b");
    for name in ["compared", "containing", "lacking"] {
        let run = report(&sandbox, name, "REQ-A", &[]);
        let proof = only_proof(&run);
        let children = proof["children"].as_array().unwrap();
        assert_eq!(children.len(), 1, "{name}: {proof:#}");
        assert!(children[0]["path"].is_string(), "{name}: {proof:#}");
    }
}

/// Finding 4: an item copied with its anchor shares the MID of another; a leaf that only the
/// MID places has no one place.
#[test]
fn a_leaf_about_a_shared_mid_is_not_placed() {
    let sandbox = Sandbox::new();
    let text = fs::read_to_string(sandbox.project().join("docs/a.mara.md")).unwrap();
    sandbox.write("docs/a.mara.md", &text.replace(&mid(2), &mid(1)));
    let program = ".include \"mara.dl\"\n.decl owned(m: Mid)\nowned(m) :- field(m, \"owner\", \"abc\").\n\
                   .decl listed(m: Mid)\nlisted(m) :- item(m, _, \"REQ-B\", _, _, _).\n";
    let entries = invariant("owned", "p.dl", "owned", "warning")
        + &invariant("listed", "p.dl", "listed", "warning");
    sandbox.program("p.dl", program, &entries);
    for id in ["REQ-A", "REQ-B"] {
        let run = report(&sandbox, "owned", id, &[]);
        assert_eq!(run.status, 1, "{run:#?}");
        let reason = run.json["rows"][0]["unexplained"].as_str().unwrap();
        assert!(
            reason.contains(&format!("several items have the MID `{}`", mid(1))),
            "{reason}"
        );
    }
    // An item leaf carries its own path and line.
    let run = report(&sandbox, "listed", "REQ-B", &[]);
    let proof = only_proof(&run);
    let get = sandbox.mara(&["get", "docs/a.mara.md"]);
    let line = leaves(proof)[0]["line"].as_u64().unwrap();
    assert!(line > 5, "{proof:#} {get:#?}");
    assert_eq!(leaves(proof)[0]["values"][2], "REQ-B");
}

/// A relation the program states by facts, as the `engineering` template's `relation_class`:
/// its leaf is at the line of the program that states the row. A fact written with an
/// expression is not matched to a printed row, and the row is left unexplained.
#[test]
fn a_fact_of_the_program_is_a_leaf_at_its_line() {
    let program = r#".include "mara.dl"
.decl class(t: Type, c: symbol)
class("need", "soft").
class("requirement", "strict").
.decl strict(m: Mid)
strict(m) :- item(m, t, _, _, _, _), class(t, "strict").
.decl level(n: number)
level(1 + 1).
.decl leveled(m: Mid)
leveled(m) :- item(m, "need", _, _, _, _), level(2).
"#;
    let entries = invariant("strict", "p.dl", "strict", "warning")
        + &invariant("leveled", "p.dl", "leveled", "warning");
    let sandbox = Sandbox::new();
    sandbox.program("p.dl", program, &entries);
    let run = report(&sandbox, "strict", "REQ-A", &[]);
    let proof = only_proof(&run);
    assert_eq!(
        leaves(proof)[1],
        &json!({"relation": "class", "values": ["requirement", "strict"],
                "path": ".mara/rules/p.dl", "line": 4}),
        "{proof:#}"
    );
    let run = report(&sandbox, "leveled", "NEED-ONE", &[]);
    assert_eq!(run.status, 1, "{run:#?}");
    let reason = run.json["rows"][0]["unexplained"].as_str().unwrap();
    assert!(reason.contains("`level`"), "{reason}");
}
