//! The queries, gates and views of DES-TEMPLATE-QUERIES, DES-TEMPLATE-GATES and
//! DES-TEMPLATE-VIEWS with the inputs of DES-TEMPLATE-INPUTS. Without code links (mara-hk2arx)
//! no requirement is implemented and code is never read completely, so `verified` and
//! `release-ready` fail an item with a known failure and leave every other requirement
//! unavailable.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::{Item, Project, accepted, draft, rows};

/// Requirements with their verifications, risks around a decomposed requirement, and
/// validations, all valid unless their name says otherwise.
fn fixture() -> Vec<Item> {
    let requirement = |id: &str| accepted(id).link("Satisfies", "NEED-N");
    let verification = |id: &str, requirement: &str| {
        accepted(id)
            .field("Methods", "Test")
            .link("Verifies", requirement)
    };
    let risk = |id: &str, likelihood: &str, consequence: &str, target: &str| {
        accepted(id)
            .field("Likelihood", likelihood)
            .field("Consequence", consequence)
            .link("Threatens", target)
    };
    let validation = |id: &str, targets: &[&str]| {
        targets.iter().fold(
            accepted(id).field("Methods", "Demonstration"),
            |item, target| item.link("Validates", target),
        )
    };
    vec![
        accepted("GOAL-G"),
        accepted("NEED-N").link("Contributes to", "GOAL-G"),
        requirement("REQ-PASS"),
        verification("VER-PASS", "REQ-PASS"),
        requirement("REQ-TWO"),
        verification("VER-TWO-A", "REQ-TWO"),
        verification("VER-TWO-B", "REQ-TWO"),
        requirement("REQ-STALE"),
        verification("VER-STALE", "REQ-STALE"),
        requirement("REQ-NONE"),
        draft("REQ-DRAFT"),
        // Risk scope: REQ-C decomposes REQ-P, which satisfies NEED-P, which contributes to
        // GOAL-P; REQ-S is REQ-C's sibling and REQ-D derives from REQ-C.
        accepted("GOAL-P"),
        accepted("NEED-P").link("Contributes to", "GOAL-P"),
        accepted("REQ-P").link("Satisfies", "NEED-P"),
        verification("VER-P", "REQ-P"),
        accepted("REQ-C").link("Decomposes", "REQ-P"),
        verification("VER-C", "REQ-C"),
        accepted("REQ-S").link("Decomposes", "REQ-P"),
        verification("VER-S", "REQ-S"),
        accepted("REQ-D").link("Derives from", "REQ-C"),
        verification("VER-D", "REQ-D"),
        risk("RISK-P", "Rare", "Minimal", "REQ-P"),
        risk("RISK-NP", "Unlikely", "Minor", "NEED-P"),
        risk("RISK-GP", "Possible", "Minimal", "GOAL-P"),
        risk("RISK-S", "Almost-certain", "Severe", "REQ-S"),
        // A HIGH risk with a mitigation, and a risk without a level.
        requirement("REQ-H"),
        verification("VER-H", "REQ-H"),
        risk("RISK-H", "Likely", "Major", "REQ-H"),
        accepted("DES-M")
            .link("Satisfies", "REQ-H")
            .link("Mitigates", "RISK-H"),
        requirement("REQ-L"),
        verification("VER-L", "REQ-L"),
        draft("REQ-LD"),
        accepted("RISK-L")
            .field("Likelihood", "Likely")
            .link("Threatens", "REQ-L")
            .link("Threatens", "REQ-LD"),
        // Validations.
        accepted("NEED-V1").link("Contributes to", "GOAL-G"),
        accepted("NEED-V2").link("Contributes to", "GOAL-G"),
        validation("VAL-ONE", &["NEED-V1"]),
        validation("VAL-TWO", &["NEED-V1", "NEED-V2"]),
        validation("VAL-OTHER", &["NEED-V2"]),
    ]
}

/// The fixture in a new project, with the current revisions of results' items.
struct Readiness {
    project: Project,
    /// (check ID, target ID) → (check revision, target revision).
    stamps: BTreeMap<(String, String), (String, String)>,
}

impl Readiness {
    fn new(items: &[Item]) -> Readiness {
        let mut project = Project::new();
        project.write(items);
        let stamps = project
            .rows("execute", "result-stamps", &[])
            .into_iter()
            .map(|row| {
                (
                    (row[1].clone(), row[2].clone()),
                    (row[3].clone(), row[4].clone()),
                )
            })
            .collect();
        Readiness { project, stamps }
    }

    /// A result row of `check` for `target` in `candidate` or context, with the current
    /// revisions.
    fn result(&self, check: &str, target: &str, candidate: &str, result: &str) -> Value {
        let (check_revision, target_revision) =
            &self.stamps[&(check.to_owned(), target.to_owned())];
        json!([
            check,
            target,
            candidate,
            check_revision,
            target_revision,
            result
        ])
    }

    /// The arguments that supply `candidate` `c1`, `verification` results and, when
    /// `complete`, mark them complete.
    fn verification(&self, results: &[Value], complete: bool) -> Vec<String> {
        let mut args = vec![
            "--input".to_owned(),
            "candidate=c1".to_owned(),
            "--input".to_owned(),
            self.project
                .input("verification_result", &Value::Array(results.to_vec())),
        ];
        if complete {
            args.extend(["--complete".to_owned(), "verification_result".to_owned()]);
        }
        args
    }

    /// The arguments that supply validation `contexts` and `results` and, when `complete`, mark
    /// the results complete.
    fn validation(&self, contexts: &[[&str; 2]], results: &[Value], complete: bool) -> Vec<String> {
        let mut args = vec![
            "--input".to_owned(),
            self.project.input("validation_context", &json!(contexts)),
            "--input".to_owned(),
            self.project
                .input("validation_result", &Value::Array(results.to_vec())),
        ];
        if complete {
            args.extend(["--complete".to_owned(), "validation_result".to_owned()]);
        }
        args
    }

    /// [`Readiness::verification`] and [`Readiness::validation`], for `release-ready`.
    fn release(
        &self,
        verification: &[Value],
        contexts: &[[&str; 2]],
        results: &[Value],
        complete: bool,
    ) -> Vec<String> {
        let mut args = self.verification(verification, complete);
        args.extend(self.validation(contexts, results, complete));
        args
    }

    fn gate(&self, name: &str, ids: &[&str], args: &[String]) -> crate::Gate {
        let mut all: Vec<&str> = ids.iter().flat_map(|id| ["--id", *id]).collect();
        all.extend(args.iter().map(String::as_str));
        self.project.gate(name, &all)
    }

    fn rows(&self, operation: &str, name: &str, ids: &[&str], args: &[String]) -> Vec<Vec<String>> {
        let mut all: Vec<&str> = ids.iter().flat_map(|id| ["--id", *id]).collect();
        all.extend(args.iter().map(String::as_str));
        self.project
            .rows(operation, name, &all)
            .into_iter()
            .collect()
    }
}

/// The set of rows of a gate item, for comparing.
fn set(rows: &[&str]) -> std::collections::BTreeSet<String> {
    rows.iter().map(|row| (*row).to_owned()).collect()
}

#[test]
fn every_verification_must_pass() {
    let r = Readiness::new(&fixture());
    let pass = r.result("VER-TWO-A", "REQ-TWO", "c1", "pass");
    let fail = r.result("VER-TWO-B", "REQ-TWO", "c1", "fail");
    // One passing and one failing verification: failed, although code is unavailable.
    let gate = r.gate(
        "verified",
        &["REQ-TWO"],
        &r.verification(&[pass.clone(), fail], false),
    );
    assert_eq!(gate.outcome, "failed");
    let item = gate.only();
    assert_eq!(item.outcome, "failed");
    assert_eq!(
        item.rows,
        set(&["condition verification_not_passed(REQ-TWO, VER-TWO-B, fail)"])
    );
    // The other result absent from a complete input: pending, failed.
    let gate = r.gate(
        "verified",
        &["REQ-TWO"],
        &r.verification(std::slice::from_ref(&pass), true),
    );
    assert_eq!(gate.only().outcome, "failed");
    assert!(
        gate.only()
            .rows
            .contains("condition verification_not_passed(REQ-TWO, VER-TWO-B, pending)"),
        "{gate:#?}"
    );
    // Absent from an incomplete input: unavailable, never passed.
    let gate = r.gate("verified", &["REQ-TWO"], &r.verification(&[pass], false));
    assert_eq!(gate.outcome, "unavailable");
    assert_eq!(
        gate.only().rows,
        set(&[
            "unavailable code_unavailable(REQ-TWO)",
            "unavailable result_unavailable(REQ-TWO, VER-TWO-B)",
        ])
    );
}

#[test]
fn without_code_links_a_passing_requirement_is_unavailable() {
    let r = Readiness::new(&fixture());
    let pass = r.result("VER-PASS", "REQ-PASS", "c1", "pass");
    let gate = r.gate(
        "verified",
        &["REQ-PASS"],
        &r.verification(std::slice::from_ref(&pass), true),
    );
    assert_eq!(gate.outcome, "unavailable");
    assert_eq!(
        gate.only().rows,
        set(&["unavailable code_unavailable(REQ-PASS)"])
    );
    let args = r.release(&[pass], &[], &[], true);
    let gate = r.gate("release-ready", &["REQ-PASS"], &args);
    assert_eq!(gate.outcome, "unavailable");
    assert_eq!(
        gate.only().rows,
        set(&["unavailable code_unavailable(REQ-PASS)"])
    );
    // `implemented` has no row; the query runs.
    assert!(r.rows("execute", "implemented", &[], &[]).is_empty());
}

#[test]
fn verified_fails_without_an_accepted_verification_or_when_not_accepted() {
    let r = Readiness::new(&fixture());
    let gate = r.gate(
        "verified",
        &["REQ-NONE", "REQ-DRAFT", "NEED-N"],
        &r.verification(&[], true),
    );
    assert_eq!(gate.outcome, "failed");
    let outcomes: Vec<(&str, &str)> = gate
        .items
        .iter()
        .map(|item| (item.id.as_str(), item.outcome.as_str()))
        .collect();
    assert_eq!(
        outcomes,
        [
            ("REQ-NONE", "failed"),
            ("REQ-DRAFT", "failed"),
            ("NEED-N", "not-applicable")
        ]
    );
    assert!(
        gate.items[0]
            .rows
            .contains("condition unverified_requirement(REQ-NONE)")
    );
    assert!(
        gate.items[1]
            .rows
            .contains("condition not_accepted(REQ-DRAFT, draft)")
    );
}

#[test]
fn a_result_for_another_candidate_or_revision_does_not_count() {
    let r = Readiness::new(&fixture());
    let current = r.result("VER-STALE", "REQ-STALE", "c1", "pass");
    let mut other_candidate = current.clone();
    other_candidate[2] = json!("c2");
    let mut old_verification = current.clone();
    old_verification[3] = json!("an older revision");
    let mut old_requirement = current;
    old_requirement[4] = json!("an older revision");
    for result in [other_candidate, old_verification, old_requirement] {
        for (complete, status) in [(true, "pending"), (false, "unavailable")] {
            let args = r.verification(std::slice::from_ref(&result), complete);
            assert_eq!(
                r.rows("execute", "verification-status", &["REQ-STALE"], &args),
                rows(&[["REQ-STALE", "VER-STALE", "accepted", status]])
                    .into_iter()
                    .collect::<Vec<_>>(),
                "{result}"
            );
            let gate = r.gate("verified", &["REQ-STALE"], &args);
            assert_ne!(gate.outcome, "passed", "{result}");
            let outcome = if complete { "failed" } else { "unavailable" };
            assert_eq!(gate.outcome, outcome, "{result}: {gate:#?}");
        }
    }
    // With two candidates no result counts.
    let pass = r.result("VER-STALE", "REQ-STALE", "c1", "pass");
    let mut args = r.verification(&[pass], true);
    args[1] = r.project.input("candidate", &json!([["c1"], ["c2"]]));
    assert_eq!(
        r.rows("execute", "verification-status", &["REQ-STALE"], &args),
        vec![vec!["REQ-STALE", "VER-STALE", "accepted", "pending"]]
    );
}

#[test]
fn the_worst_result_wins() {
    let r = Readiness::new(&fixture());
    for (worse, expected) in [
        ("fail", "fail"),
        ("error", "error"),
        ("inconclusive", "inconclusive"),
        // A result that is not one of the four counts as an error.
        ("passed", "error"),
    ] {
        let results = [
            r.result("VER-PASS", "REQ-PASS", "c1", "pass"),
            r.result("VER-PASS", "REQ-PASS", "c1", worse),
        ];
        let args = r.verification(&results, true);
        assert_eq!(
            r.rows("execute", "verification-status", &["REQ-PASS"], &args),
            vec![vec!["REQ-PASS", "VER-PASS", "accepted", expected]]
        );
        let gate = r.gate("verified", &["REQ-PASS"], &args);
        assert_eq!(gate.outcome, "failed", "{worse}");
        let row = format!("condition verification_not_passed(REQ-PASS, VER-PASS, {expected})");
        assert!(gate.only().rows.contains(&row), "{gate:#?}");

        let results = [
            r.result("VAL-ONE", "NEED-V1", "ctx", "pass"),
            r.result("VAL-ONE", "NEED-V1", "ctx", worse),
        ];
        let status = r.rows(
            "execute",
            "validation-status",
            &["NEED-V1"],
            &r.validation(&[["VAL-ONE", "ctx"]], &results, true),
        );
        let args = r.release(&[], &[["VAL-ONE", "ctx"]], &results, true);
        assert!(
            status.contains(&vec![
                "NEED-V1".to_owned(),
                "VAL-ONE".to_owned(),
                "accepted".to_owned(),
                "ctx".to_owned(),
                expected.to_owned()
            ]),
            "{status:?}"
        );
        let gate = r.gate("release-ready", &["VAL-ONE"], &args);
        assert_eq!(gate.outcome, "failed", "{worse}");
        let row = format!("condition validation_not_passed(VAL-ONE, NEED-V1, {expected})");
        assert!(gate.only().rows.contains(&row), "{gate:#?}");
    }
}

#[test]
fn a_validation_result_for_another_context_or_revision_does_not_count() {
    let r = Readiness::new(&fixture());
    let current = r.result("VAL-ONE", "NEED-V1", "ctx", "pass");
    let mut other_context = current.clone();
    other_context[2] = json!("another");
    let mut old_validation = current.clone();
    old_validation[3] = json!("an older revision");
    let mut old_need = current.clone();
    old_need[4] = json!("an older revision");
    for result in [other_context, old_validation, old_need] {
        for (complete, status, outcome) in [
            (true, "pending", "failed"),
            (false, "unavailable", "unavailable"),
        ] {
            let validation = r.validation(
                &[["VAL-ONE", "ctx"]],
                std::slice::from_ref(&result),
                complete,
            );
            let found = r.rows("execute", "validation-status", &["NEED-V1"], &validation);
            let args = r.release(
                &[],
                &[["VAL-ONE", "ctx"]],
                std::slice::from_ref(&result),
                complete,
            );
            let row = |validation: &str, status_: &str, context: &str, result: &str| {
                ["NEED-V1", validation, status_, context, result]
                    .map(str::to_owned)
                    .to_vec()
            };
            assert!(
                found.contains(&row("VAL-ONE", "accepted", "ctx", status)),
                "{found:?}"
            );
            let gate = r.gate("release-ready", &["VAL-ONE"], &args);
            assert_eq!(gate.outcome, outcome, "{result}: {gate:#?}");
        }
    }
    // The current result passes: a validation needs no code, so it passes.
    let args = r.release(
        &[],
        &[["VAL-ONE", "ctx"]],
        std::slice::from_ref(&current),
        true,
    );
    let gate = r.gate("release-ready", &["VAL-ONE"], &args);
    assert_eq!(gate.outcome, "passed", "{gate:#?}");
    // Its need's summary: the worst of its accepted validations, VAL-TWO pending.
    let args = r.validation(&[["VAL-ONE", "ctx"]], &[current], true);
    let found = r.rows("execute", "validation-status", &["NEED-V1"], &args);
    assert!(
        found.contains(
            &["NEED-V1", "", "", "", "pending"]
                .map(str::to_owned)
                .to_vec()
        ),
        "{found:?}"
    );
    // A need or goal without an accepted validation is not validated.
    let found = r.rows("execute", "validation-status", &["NEED-N"], &args);
    assert_eq!(
        found,
        vec![
            ["NEED-N", "", "", "", "not-validated"]
                .map(str::to_owned)
                .to_vec()
        ]
    );
}

#[test]
fn release_ready_judges_only_the_named_validations() {
    let r = Readiness::new(&fixture());
    let pass_one = r.result("VAL-TWO", "NEED-V1", "ctx", "pass");
    let fail_two = r.result("VAL-TWO", "NEED-V2", "ctx", "fail");
    let other_fails = r.result("VAL-OTHER", "NEED-V2", "ctx", "fail");
    let contexts = [["VAL-TWO", "ctx"], ["VAL-OTHER", "ctx"]];
    // A pass for one need and a failure for the other.
    let args = r.release(&[], &contexts, &[pass_one.clone(), fail_two], true);
    let gate = r.gate("release-ready", &["VAL-TWO"], &args);
    assert_eq!(gate.outcome, "failed");
    assert_eq!(
        gate.only().rows,
        set(&["condition validation_not_passed(VAL-TWO, NEED-V2, fail)"])
    );
    // A pass for one need and none for the other in a complete input.
    let args = r.release(
        &[],
        &contexts,
        &[pass_one.clone(), other_fails.clone()],
        true,
    );
    let gate = r.gate("release-ready", &["VAL-TWO"], &args);
    assert_eq!(gate.outcome, "failed");
    assert!(
        gate.only()
            .rows
            .contains("condition validation_not_passed(VAL-TWO, NEED-V2, pending)")
    );
    // Not named, VAL-OTHER's failure does not block a requirement; named, it does.
    let pass = r.result("VER-PASS", "REQ-PASS", "c1", "pass");
    let args = r.release(&[pass], &contexts, &[pass_one, other_fails], true);
    let gate = r.gate("release-ready", &["REQ-PASS"], &args);
    assert_eq!(gate.outcome, "unavailable");
    assert!(
        gate.only().rows.iter().all(|row| !row.contains("VAL-")),
        "{gate:#?}"
    );
    let gate = r.gate("release-ready", &["REQ-PASS", "VAL-OTHER"], &args);
    assert_eq!(gate.outcome, "failed");
    // A named validation without a context fails.
    let args = r.release(&[], &[], &[], true);
    let gate = r.gate("release-ready", &["VAL-ONE"], &args);
    assert_eq!(gate.outcome, "failed");
    assert!(
        gate.only()
            .rows
            .contains("condition validation_without_context(VAL-ONE)"),
        "{gate:#?}"
    );
}

#[test]
fn risks_of_ancestors_needs_and_goals_are_relevant_and_siblings_are_not() {
    let r = Readiness::new(&fixture());
    let relevant = r.rows("execute", "relevant-risks", &["REQ-C"], &[]);
    assert_eq!(
        relevant,
        rows(&[
            ["REQ-C", "RISK-GP", "accepted", "GOAL-P"],
            ["REQ-C", "RISK-NP", "accepted", "NEED-P"],
            ["REQ-C", "RISK-P", "accepted", "REQ-P"],
        ])
        .into_iter()
        .collect::<Vec<_>>()
    );
    let scope = r.rows("execute", "risk-scope", &["REQ-D"], &[]);
    assert_eq!(
        scope,
        rows(&[
            ["REQ-D", "GOAL-P", "goal"],
            ["REQ-D", "NEED-P", "need"],
            ["REQ-D", "REQ-C", "requirement"],
            ["REQ-D", "REQ-D", "requirement"],
            ["REQ-D", "REQ-P", "requirement"],
        ])
        .into_iter()
        .collect::<Vec<_>>()
    );
    // REQ-S's HIGH risk does not block its sibling, and only the named item is judged.
    let gate = r.gate("implementation-ready", &["REQ-C"], &[]);
    assert_eq!(gate.outcome, "passed", "{gate:#?}");
    let gate = r.gate("implementation-ready", &["REQ-S"], &[]);
    assert_eq!(gate.outcome, "failed", "{gate:#?}");
    assert!(
        gate.only()
            .rows
            .contains("condition blocking_risk(REQ-S, RISK-S)")
    );
}

#[test]
fn a_high_risk_blocks_until_reassessed_or_accepted() {
    let r = Readiness::new(&fixture());
    let gate = r.gate("implementation-ready", &["REQ-H"], &[]);
    assert_eq!(gate.outcome, "failed", "{gate:#?}");
    assert_eq!(
        gate.only().rows,
        set(&["condition blocking_risk(REQ-H, RISK-H)"])
    );
    let with = |change: &dyn Fn(Item) -> Item, extra: Vec<Item>| {
        let mut items: Vec<Item> = fixture()
            .into_iter()
            .map(|item| {
                if item.id == "RISK-H" {
                    change(item)
                } else {
                    item
                }
            })
            .collect();
        items.extend(extra);
        let r = Readiness::new(&items);
        r.gate("implementation-ready", &["REQ-H"], &[]).outcome
    };
    // Reassessed: rare and major is MEDIUM.
    let reassessed = |_: Item| {
        accepted("RISK-H")
            .field("Likelihood", "Rare")
            .field("Consequence", "Major")
            .link("Threatens", "REQ-H")
    };
    assert_eq!(with(&reassessed, vec![]), "passed");
    let same = |item: Item| item;
    assert_eq!(
        with(
            &same,
            vec![accepted("ADR-ACCEPT").link("Accepts", "RISK-H")]
        ),
        "passed"
    );
    assert_eq!(
        with(&same, vec![draft("ADR-ACCEPT").link("Accepts", "RISK-H")]),
        "failed"
    );
}

#[test]
fn a_missing_level_is_unavailable_and_a_failure_beside_it_fails() {
    let r = Readiness::new(&fixture());
    let gate = r.gate("implementation-ready", &["REQ-L", "REQ-LD"], &[]);
    assert_eq!(gate.outcome, "failed");
    assert_eq!(gate.items[0].outcome, "unavailable");
    assert_eq!(
        gate.items[0].rows,
        set(&["unavailable risk_level_unavailable(REQ-L, RISK-L)"])
    );
    assert_eq!(gate.items[1].outcome, "failed");
    // A relevant risk without a level, and a failure: only the failure is shown.
    assert_eq!(
        gate.items[1].rows,
        set(&["condition not_accepted(REQ-LD, draft)"])
    );
    let treatment = r.rows("view", "risk-treatment", &["REQ-LD"], &[]);
    assert_eq!(treatment[0][9], "unavailable", "{treatment:?}");
    assert!(r.rows("execute", "risk-level", &["RISK-L"], &[]).is_empty());
}

#[test]
fn implementation_ready_fails_a_requirement_that_breaks_a_rule() {
    let r = Readiness::new(&fixture());
    let gate = r.gate("implementation-ready", &["REQ-NONE"], &[]);
    assert_eq!(gate.outcome, "failed");
    assert!(
        gate.only()
            .rows
            .contains("condition breaks_rule(REQ-NONE, unverified_requirement)"),
        "{gate:#?}"
    );
    let gate = r.gate("implementation-ready", &["REQ-PASS"], &[]);
    assert_eq!(gate.outcome, "passed", "{gate:#?}");
}

#[test]
fn each_cell_of_the_risk_matrix() {
    let likelihoods = ["Rare", "Unlikely", "Possible", "Likely", "Almost-certain"];
    let consequences = ["Minimal", "Minor", "Moderate", "Major", "Severe"];
    // DES-TEMPLATE-RISK-LEVEL's table, row by row.
    let matrix = [
        ["low", "low", "low", "medium", "medium"],
        ["low", "low", "medium", "medium", "high"],
        ["low", "medium", "medium", "high", "high"],
        ["medium", "medium", "high", "high", "high"],
        ["medium", "high", "high", "high", "high"],
    ];
    let mut items = vec![accepted("GOAL-G")];
    let mut expected = Vec::new();
    for (row, likelihood) in likelihoods.iter().enumerate() {
        for (column, consequence) in consequences.iter().enumerate() {
            let id = format!("RISK-{row}{column}");
            items.push(
                accepted(&id)
                    .field("Likelihood", likelihood)
                    .field("Consequence", consequence)
                    .link("Threatens", "GOAL-G"),
            );
            expected.push(vec![
                id,
                likelihood.to_lowercase(),
                consequence.to_lowercase(),
                matrix[row][column].to_owned(),
            ]);
        }
    }
    expected.sort();
    let r = Readiness::new(&items);
    assert_eq!(r.rows("execute", "risk-level", &[], &[]), expected);
}

#[test]
fn requirement_basis_keeps_every_status() {
    let items = vec![
        accepted("GOAL-G"),
        accepted("NEED-A").link("Contributes to", "GOAL-G"),
        draft("NEED-D"),
        accepted("REQ-X")
            .link("Satisfies", "NEED-A")
            .link("Satisfies", "NEED-D"),
        draft("ADR-X").link("Justifies", "REQ-X"),
        accepted("DES-X")
            .link("Satisfies", "REQ-X")
            .link("Contributes to", "GOAL-G"),
        accepted("ADR-Y").link("Justifies", "DES-X"),
    ];
    let r = Readiness::new(&items);
    assert_eq!(
        r.rows("execute", "requirement-basis", &["REQ-X"], &[]),
        rows(&[
            ["REQ-X", "ADR-X", "decision", "justifies", "draft"],
            ["REQ-X", "NEED-A", "need", "satisfies", "accepted"],
            ["REQ-X", "NEED-D", "need", "satisfies", "draft"],
        ])
        .into_iter()
        .collect::<Vec<_>>()
    );
    // A justifying decision is no basis of a design.
    assert_eq!(
        r.rows("execute", "design-basis", &["DES-X"], &[]),
        rows(&[
            ["DES-X", "GOAL-G", "contributes_to", "accepted"],
            ["DES-X", "REQ-X", "satisfies", "accepted"],
        ])
        .into_iter()
        .collect::<Vec<_>>()
    );
}

#[test]
fn views_keep_missing_unavailable_and_failed_apart_and_agree_with_gates() {
    let r = Readiness::new(&fixture());
    // REQ-NONE has no verification, REQ-PASS's result is absent from an incomplete input, and
    // REQ-TWO has a failing one.
    let fail = r.result("VER-TWO-B", "REQ-TWO", "c1", "fail");
    let pass = r.result("VER-TWO-A", "REQ-TWO", "c1", "pass");
    let args = r.verification(&[pass, fail], false);
    let ids = ["REQ-NONE", "REQ-PASS", "REQ-TWO"];
    assert_eq!(
        r.rows("view", "delivery-readiness", &ids, &args),
        rows(&[
            [
                "REQ-NONE",
                "accepted",
                "invalid",
                "unavailable",
                "none",
                "clear"
            ],
            [
                "REQ-PASS",
                "accepted",
                "valid",
                "unavailable",
                "unavailable",
                "clear"
            ],
            [
                "REQ-TWO",
                "accepted",
                "valid",
                "unavailable",
                "fail",
                "clear"
            ],
        ])
        .into_iter()
        .collect::<Vec<_>>()
    );
    assert_eq!(
        r.rows("view", "requirement-verification", &ids, &args),
        rows(&[
            ["REQ-NONE", "unavailable", "0", "", "none", "none"],
            [
                "REQ-PASS",
                "unavailable",
                "0",
                "VER-PASS",
                "accepted",
                "unavailable"
            ],
            [
                "REQ-TWO",
                "unavailable",
                "0",
                "VER-TWO-A",
                "accepted",
                "pass"
            ],
            [
                "REQ-TWO",
                "unavailable",
                "0",
                "VER-TWO-B",
                "accepted",
                "fail"
            ],
        ])
        .into_iter()
        .collect::<Vec<_>>()
    );
    // The gate over the same items reports the same facts.
    let gate = r.gate("verified", &ids, &args);
    let outcomes: Vec<&str> = gate
        .items
        .iter()
        .map(|item| item.outcome.as_str())
        .collect();
    assert_eq!(outcomes, ["failed", "unavailable", "failed"]);
    assert!(
        gate.items[0]
            .rows
            .contains("condition unverified_requirement(REQ-NONE)")
    );
    assert!(
        gate.items[1]
            .rows
            .contains("unavailable result_unavailable(REQ-PASS, VER-PASS)")
    );
    assert!(
        gate.items[2]
            .rows
            .contains("condition verification_not_passed(REQ-TWO, VER-TWO-B, fail)")
    );
    // Risks: blocking, without a level, and clear.
    let treatment = r.rows("view", "risk-treatment", &["REQ-H", "REQ-L", "REQ-C"], &[]);
    assert_eq!(
        treatment,
        rows(&[
            [
                "REQ-C", "RISK-GP", "accepted", "GOAL-P", "possible", "minimal", "low", "0", "",
                "no"
            ],
            [
                "REQ-C", "RISK-NP", "accepted", "NEED-P", "unlikely", "minor", "low", "0", "", "no"
            ],
            [
                "REQ-C", "RISK-P", "accepted", "REQ-P", "rare", "minimal", "low", "0", "", "no"
            ],
            [
                "REQ-H", "RISK-H", "accepted", "REQ-H", "likely", "major", "high", "1", "",
                "blocking"
            ],
            [
                "REQ-L",
                "RISK-L",
                "accepted",
                "REQ-L",
                "likely",
                "missing",
                "unavailable",
                "0",
                "",
                "unavailable"
            ],
        ])
        .into_iter()
        .collect::<Vec<_>>()
    );
    let readiness = r.rows("view", "delivery-readiness", &["REQ-H", "REQ-L"], &args);
    let risks: Vec<&str> = readiness.iter().map(|row| row[5].as_str()).collect();
    assert_eq!(risks, ["blocking", "unavailable"]);
    let gate = r.gate("implementation-ready", &["REQ-H", "REQ-L"], &[]);
    let outcomes: Vec<&str> = gate
        .items
        .iter()
        .map(|item| item.outcome.as_str())
        .collect();
    assert_eq!(outcomes, ["failed", "unavailable"]);
}

#[test]
fn risk_treatment_names_each_accepting_decision() {
    let base = || {
        vec![
            accepted("GOAL-G"),
            accepted("NEED-N").link("Contributes to", "GOAL-G"),
            accepted("REQ-R").link("Satisfies", "NEED-N"),
            accepted("VER-V")
                .field("Methods", "Test")
                .link("Verifies", "REQ-R"),
            accepted("RISK-K")
                .field("Likelihood", "Likely")
                .field("Consequence", "Major")
                .link("Threatens", "REQ-R"),
        ]
    };
    let row = |decision: &str, blocking: &str| {
        [
            "REQ-R", "RISK-K", "accepted", "REQ-R", "likely", "major", "high", "0", decision,
            blocking,
        ]
        .map(str::to_owned)
        .to_vec()
    };
    let treatment = |extra: Vec<Item>| {
        let mut items = base();
        items.extend(extra);
        let r = Readiness::new(&items);
        let gate = r.gate("implementation-ready", &["REQ-R"], &[]).outcome;
        (r.rows("view", "risk-treatment", &["REQ-R"], &[]), gate)
    };
    // An unaccepted HIGH risk blocks, and no decision is named; a draft decision accepts nothing.
    let (rows, gate) = treatment(vec![draft("ADR-D").link("Accepts", "RISK-K")]);
    assert_eq!(rows, [row("", "blocking")]);
    assert_eq!(gate, "failed");
    // One accepting decision: named, and the risk no longer blocks.
    let (rows, gate) = treatment(vec![accepted("ADR-A").link("Accepts", "RISK-K")]);
    assert_eq!(rows, [row("ADR-A", "no")]);
    assert_eq!(gate, "passed");
    // Two: a row for each.
    let (rows, _) = treatment(vec![
        accepted("ADR-A").link("Accepts", "RISK-K"),
        accepted("ADR-B").link("Accepts", "RISK-K"),
        draft("ADR-D").link("Accepts", "RISK-K"),
    ]);
    assert_eq!(rows, [row("ADR-A", "no"), row("ADR-B", "no")]);
}

/// Decided by the orchestrator on 2026-11-16: a counting result decides whatever the
/// completeness of its source; completeness matters only without one (DES-TEMPLATE-INPUTS).
#[test]
fn a_counting_result_decides_whatever_the_completeness() {
    let r = Readiness::new(&fixture());
    let pass = r.result("VER-PASS", "REQ-PASS", "c1", "pass");
    for complete in [false, true] {
        // A counting pass: the verification passes, and only code is unavailable.
        let args = r.verification(std::slice::from_ref(&pass), complete);
        assert_eq!(
            r.rows("execute", "verification-status", &["REQ-PASS"], &args),
            vec![vec!["REQ-PASS", "VER-PASS", "accepted", "pass"]],
            "complete: {complete}"
        );
        let gate = r.gate("verified", &["REQ-PASS"], &args);
        assert_eq!(gate.outcome, "unavailable", "complete: {complete}");
        assert_eq!(
            gate.only().rows,
            set(&["unavailable code_unavailable(REQ-PASS)"])
        );
        // No counting result: unavailable when incomplete, failed when complete.
        let gate = r.gate("verified", &["REQ-PASS"], &r.verification(&[], complete));
        let (outcome, row) = match complete {
            false => (
                "unavailable",
                "unavailable result_unavailable(REQ-PASS, VER-PASS)",
            ),
            true => (
                "failed",
                "condition verification_not_passed(REQ-PASS, VER-PASS, pending)",
            ),
        };
        assert_eq!(gate.outcome, outcome, "complete: {complete}");
        assert!(gate.only().rows.contains(row), "{gate:#?}");

        // A counting pass of a Validation passes `release-ready`.
        let result = r.result("VAL-ONE", "NEED-V1", "ctx", "pass");
        let args = r.release(&[], &[["VAL-ONE", "ctx"]], &[result], complete);
        let gate = r.gate("release-ready", &["VAL-ONE"], &args);
        assert_eq!(gate.outcome, "passed", "complete: {complete}: {gate:#?}");
        // No counting result: unavailable when incomplete, failed when complete.
        let args = r.release(&[], &[["VAL-ONE", "ctx"]], &[], complete);
        let gate = r.gate("release-ready", &["VAL-ONE"], &args);
        let (outcome, row) = match complete {
            false => (
                "unavailable",
                "unavailable validation_result_unavailable(VAL-ONE, NEED-V1)",
            ),
            true => (
                "failed",
                "condition validation_not_passed(VAL-ONE, NEED-V1, pending)",
            ),
        };
        assert_eq!(gate.outcome, outcome, "complete: {complete}");
        assert!(gate.only().rows.contains(row), "{gate:#?}");
    }
}
