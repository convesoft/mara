//! The invariant cases of VER-PROGRAM-CONSUMERS, and the configuration of every kind of
//! consumer (DES-RULE-CONSUMERS).

use crate::{Sandbox, both_severities, config, invariant, mid};

/// Accepted items, which `REQ-A` and `NEED-ONE` are.
const ACCEPTED: &str = r#".include "mara.dl"
.decl accepted(m: Mid, id: Id)
accepted(m, id) :- item(m, _, id, "accepted", _, _).
.decl none(m: Mid)
none(m) :- item(m, _, _, "retired", _, _).
"#;

/// The line of the anchor of the item with MID `mid(n)` in `docs/a.mara.md`.
fn anchor_line(sandbox: &Sandbox, n: u32) -> u64 {
    let text = std::fs::read_to_string(sandbox.project().join("docs/a.mara.md")).unwrap();
    let anchor = format!("<a id=\"{}\">", mid(n));
    text.lines()
        .position(|line| line == format!("{anchor}</a>"))
        .unwrap() as u64
        + 1
}

#[test]
fn an_error_invariant_reports_each_row_at_its_item() {
    let sandbox = Sandbox::new();
    let entry = invariant("accepted-items", "accepted.dl", "accepted", "error");
    sandbox.program("accepted.dl", ACCEPTED, &entry);
    let run = sandbox.validate();
    assert_eq!(run.status, 1, "{run:#?}");
    assert_eq!(run.json["valid"], false);
    let found = run.with_code("accepted-items");
    assert_eq!(found.len(), 2, "{run:#?}");
    for (diagnostic, (n, id)) in found.iter().zip([(1, "REQ-A"), (3, "NEED-ONE")]) {
        assert_eq!(diagnostic["severity"], "error");
        assert_eq!(diagnostic["message"], "accepted-items failed");
        assert_eq!(diagnostic["path"], "docs/a.mara.md");
        assert_eq!(diagnostic["line"], anchor_line(&sandbox, n));
        assert_eq!(diagnostic["id"], id);
        assert_eq!(diagnostic["mid"], mid(n));
    }
    assert_eq!(run.diagnostics().len(), 2, "{run:#?}");
    // One evaluation, in a private directory that is gone.
    assert_eq!((run.runs(), run.spawns()), (1, 1));
    assert_eq!(sandbox.tmp_entries(), Vec::<String>::new());
}

#[test]
fn a_warning_invariant_leaves_the_project_valid() {
    let sandbox = Sandbox::new();
    let entry = invariant("accepted-items", "accepted.dl", "accepted", "warning");
    sandbox.program("accepted.dl", ACCEPTED, &entry);
    let run = sandbox.validate();
    assert_eq!(run.status, 0, "{run:#?}");
    assert_eq!(run.json["valid"], true);
    assert_eq!(run.json["warnings"], 2);
    let found = run.with_code("accepted-items");
    assert!(found.iter().all(|d| d["severity"] == "warning"), "{run:#?}");
    assert_eq!(found.len(), 2);
}

#[test]
fn an_empty_invariant_reports_nothing() {
    let sandbox = Sandbox::new();
    sandbox.program(
        "accepted.dl",
        ACCEPTED,
        &both_severities("accepted.dl", "none"),
    );
    let run = sandbox.validate();
    assert_eq!(run.status, 0, "{run:#?}");
    assert_eq!(run.diagnostics(), [] as [serde_json::Value; 0]);
    assert_eq!(run.runs(), 1);
}

#[test]
fn a_rule_added_between_requests_counts_in_the_second() {
    let sandbox = Sandbox::new();
    let entry = invariant("accepted-items", "accepted.dl", "none", "error");
    sandbox.program("accepted.dl", ACCEPTED, &entry);
    assert_eq!(sandbox.validate().status, 0);
    let added = format!("{ACCEPTED}none(m) :- item(m, \"need\", _, _, _, _).\n");
    sandbox.write(".mara/rules/accepted.dl", &added);
    let run = sandbox.validate();
    assert_eq!(run.status, 1, "{run:#?}");
    let found = run.with_code("accepted-items");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["mid"], mid(3));
}

#[test]
fn item_validation_reports_the_rows_about_its_item() {
    let sandbox = Sandbox::new();
    let entry = invariant("accepted-items", "accepted.dl", "accepted", "error");
    let unused = "\n[queries.accepted]\nprogram = \".mara/rules/other.dl\"\nrelation = \"r\"\n\
                  description = \"Not run by item validation\"\n";
    sandbox.program("accepted.dl", ACCEPTED, &(entry + unused));
    sandbox.write(".mara/rules/other.dl", ".decl r(x: symbol)\nr(\"a\").\n");
    let run = sandbox.mara(&["item", "validate", "NEED-ONE"]);
    assert_eq!(run.status, 1, "{run:#?}");
    let found = run.with_code("accepted-items");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["mid"], mid(3));
    assert_eq!(run.runs(), 1, "only the program an invariant consumes runs");
    let run = sandbox.mara(&["item", "validate", "REQ-B"]);
    assert_eq!((run.status, run.diagnostics().len()), (0, 0), "{run:#?}");
}

/// Every declared program runs once, an invariant consumes it or not; a file that only another
/// declared file includes runs with it.
#[test]
fn validation_runs_each_declared_program_once() {
    let sandbox = Sandbox::new();
    let entries = invariant("accepted-items", "accepted.dl", "none", "error")
        + "\n[queries.listed]\nprogram = \".mara/rules/query.dl\"\nrelation = \"listed\"\n\
           description = \"Every item\"\n";
    sandbox.program("accepted.dl", ACCEPTED, &entries);
    let query = ".include \"mara.dl\"\n.include \"lib/shared.dl\"\n.decl listed(m: Mid)\n\
                 listed(m) :- item(m, _, _, _, _, _), shared(1).\n";
    sandbox.write(".mara/rules/query.dl", query);
    sandbox.write(
        ".mara/rules/lib/shared.dl",
        ".once\n.decl shared(x: number)\nshared(1).\n",
    );
    sandbox.write(
        ".mara/rules/alone.dl",
        ".decl alone(x: number)\nalone(1).\n",
    );
    let run = sandbox.validate();
    assert_eq!(run.status, 0, "{run:#?}");
    assert_eq!((run.runs(), run.spawns()), (3, 3));
}

/// Entries of each kind are checked against their program: a relation it does not declare,
/// one whose first column is no symbol, and an input that is one of Mara's facts.
#[test]
fn every_kind_of_entry_is_checked_against_its_program() {
    let sandbox = Sandbox::new();
    let program = ".include \"mara.dl\"\n.decl r(m: Mid)\nr(m) :- item(m, _, _, _, _, _).\n\
                   .decl n(x: number, m: Mid)\nn(1, m) :- r(m).\n";
    let entries = "\n[queries.q]\nprogram = \".mara/rules/p.dl\"\nrelation = \"missing\"\n\
                   description = \"d\"\n\
                   \n[views.v]\nprogram = \".mara/rules/p.dl\"\nrelation = \"n\"\n\
                   description = \"d\"\n\
                   \n[gates.g]\nprogram = \".mara/rules/p.dl\"\ndescription = \"d\"\n\
                   subjects = [\"r\"]\nconditions = [\"absent\"]\ninputs = [\"item\"]\n\
                   \n[invariants.i]\nprogram = \".mara/rules/p.dl\"\nrelation = \"r\"\n\
                   severity = \"warning\"\nmessage = \"m\"\n";
    sandbox.program("p.dl", program, entries);
    let run = sandbox.validate();
    assert_eq!(run.status, 1, "{run:#?}");
    let at = |code: &str| -> Vec<u64> {
        let found = run.with_code(code).into_iter();
        found.map(|d| d["line"].as_u64().unwrap()).collect()
    };
    let text = config(entries);
    let line = |needle: &str| text.lines().position(|line| line == needle).unwrap() as u64 + 1;
    assert_eq!(
        at("undeclared-relation"),
        [
            line("relation = \"missing\""),
            line("conditions = [\"absent\"]")
        ]
    );
    assert_eq!(
        at("invalid-relation"),
        [line("relation = \"n\""), line("inputs = [\"item\"]")]
    );
    // The invariant's relation fits: its rows are reported.
    assert_eq!(run.with_code("i").len(), 3);
}

#[test]
fn an_entry_without_its_program_or_with_a_wrong_setting_is_a_configuration_error() {
    let sandbox = Sandbox::new();
    let entries = "\n[invariants.gone]\nprogram = \".mara/rules/gone.dl\"\nrelation = \"r\"\n\
                   severity = \"error\"\nmessage = \"m\"\n\
                   \n[invariants.Bad]\nprogram = \".mara/rules/p.dl\"\nrelation = \"r\"\n\
                   severity = \"fatal\"\nmessage = \"m\"\n\
                   \n[queries.outside]\nprogram = \"elsewhere/p.dl\"\nrelation = \"r\"\n\
                   description = \"d\"\n";
    sandbox.program("p.dl", ".decl r(x: symbol)\nr(\"a\").\n", entries);
    let run = sandbox.validate();
    assert_eq!(run.status, 1, "{run:#?}");
    let codes: Vec<&str> = run
        .diagnostics()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert_eq!(
        codes,
        [
            "io-error",
            "invalid-value",
            "invalid-value",
            "invalid-value"
        ],
        "{run:#?}"
    );
}

/// Item validation reads the run that every consumer of the program shapes: a gate's inputs
/// are read, empty, as in project validation.
#[test]
fn item_validation_reads_the_same_run_as_project_validation() {
    let sandbox = Sandbox::new();
    let program = format!(
        "{ACCEPTED}.decl candidate(c: symbol)\n.decl failed(m: Mid)\n\
         failed(m) :- accepted(m, _), candidate(\"x\").\n"
    );
    let entries = invariant("accepted-items", "accepted.dl", "none", "error")
        + "\n[gates.g]\nprogram = \".mara/rules/accepted.dl\"\ndescription = \"d\"\n\
           subjects = [\"accepted\"]\nconditions = [\"failed\"]\ninputs = [\"candidate\"]\n";
    sandbox.program("accepted.dl", &program, &entries);
    for args in [&["project", "validate"][..], &["item", "validate", "REQ-A"]] {
        let run = sandbox.mara(args);
        assert_eq!(
            (run.status, run.diagnostics().len()),
            (0, 0),
            "{args:?}: {run:#?}"
        );
        assert_eq!(run.runs(), 1);
    }
}

/// A row about a MID that no item has is reported at the line that names the relation.
#[test]
fn a_row_about_no_item_is_reported_at_its_relation() {
    let sandbox = Sandbox::new();
    let entry = invariant("strays", "stray.dl", "stray", "error");
    sandbox.program(
        "stray.dl",
        ".decl stray(m: symbol)\nstray(\"nobody\").\n",
        &entry,
    );
    let run = sandbox.validate();
    let found = run.with_code("strays");
    assert_eq!(found.len(), 1, "{run:#?}");
    let line = config(&entry)
        .lines()
        .position(|l| l == "relation = \"stray\"")
        .unwrap()
        + 1;
    assert_eq!(
        (&found[0]["path"], &found[0]["line"], &found[0]["mid"]),
        (
            &".mara/config.toml".into(),
            &line.into(),
            &serde_json::Value::Null
        )
    );
    assert!(found[0]["message"].as_str().unwrap().contains("`nobody`"));
}

/// Two declared files that include each other and nothing else includes: one of them runs, so
/// their compile error is reported; an entry naming the other runs only that one.
#[test]
fn an_include_cycle_is_compiled() {
    let sandbox = Sandbox::new();
    let a = ".once\n.include \"mara.dl\"\n.include \"b.dl\"\n.decl bad(m: symbol)\nbad(m) :- missing(m).\n";
    sandbox.program("a.dl", a, "");
    sandbox.write(".mara/rules/b.dl", ".once\n.include \"a.dl\"\n");
    let run = sandbox.validate();
    assert_eq!(run.status, 1, "{run:#?}");
    let errors = run.with_code("program-error");
    assert_eq!(errors.len(), 1, "{run:#?}");
    assert_eq!(
        (&errors[0]["path"], &errors[0]["line"]),
        (&".mara/rules/a.dl".into(), &5.into())
    );
    assert_eq!(run.runs(), 1);
    // Repaired, and consumed through the other file of the cycle.
    let fixed = a.replace(
        "bad(m) :- missing(m).",
        "bad(m) :- item(m, _, _, \"draft\", _, _).",
    );
    sandbox.program("a.dl", &fixed, &invariant("drafts", "b.dl", "bad", "error"));
    let run = sandbox.validate();
    let found = run.with_code("drafts");
    assert_eq!((found.len(), run.diagnostics().len()), (1, 1), "{run:#?}");
    assert_eq!(found[0]["mid"], mid(2));
    assert_eq!(run.runs(), 1);
}

/// An invariant that the configuration drops could have judged the item: item validation
/// reports the configuration's problems as project validation does.
#[test]
fn item_validation_reports_the_configuration_problems() {
    let entry = |name: &str, program: &str, keys: &str| {
        format!("\n[invariants.{name}]\nprogram = \"{program}\"\nmessage = \"m\"\n{keys}\n")
    };
    let declared = ".mara/rules/accepted.dl";
    let cases = [
        (
            entry(
                "check",
                declared,
                "relation = \"accepted\"\nseverity = \"fatal\"",
            ),
            "invalid-value",
        ),
        (
            entry(
                "check",
                declared,
                "relation = \"accepted\"\nseverity = \"error\"\noops = 1",
            ),
            "unknown-key",
        ),
        (
            entry("check", declared, "severity = \"error\""),
            "missing-key",
        ),
        (
            entry(
                "Check",
                declared,
                "relation = \"accepted\"\nseverity = \"error\"",
            ),
            "invalid-value",
        ),
        (
            entry(
                "check",
                "elsewhere/accepted.dl",
                "relation = \"accepted\"\nseverity = \"error\"",
            ),
            "invalid-value",
        ),
    ];
    for (entry, code) in cases {
        let sandbox = Sandbox::new();
        sandbox.program("accepted.dl", ACCEPTED, &entry);
        for args in [&["project", "validate"][..], &["item", "validate", "REQ-A"]] {
            let run = sandbox.mara(args);
            assert_eq!(run.status, 1, "{args:?} {entry}: {run:#?}");
            assert_eq!(run.with_code(code).len(), 1, "{args:?} {entry}: {run:#?}");
        }
    }
}
