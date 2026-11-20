//! The invariants of DES-TEMPLATE-ACCEPTANCE and DES-TEMPLATE-RELATION-LIFECYCLE: for each, an
//! accepted item that breaks it is an error of that invariant at that item and of no other,
//! the same items as drafts are valid, and accepted items that keep it are valid.

use std::collections::BTreeSet;

use crate::{Item, Project, accepted, draft, retired};

/// The invariants of the template, by name.
const INVARIANTS: [&str; 13] = [
    "requirement-has-basis",
    "requirement-verified",
    "design-has-basis",
    "risk-assessed",
    "risk-threatens",
    "verification-has-method",
    "verification-has-target",
    "validation-has-method",
    "validation-has-target",
    "term-unique",
    "term-aliases",
    "links-to-accepted",
    "supersedes-retired",
];

/// An accepted item of every type that keeps every rule, and so is valid.
fn base() -> Vec<Item> {
    vec![
        accepted("GOAL-G"),
        accepted("NEED-N").link("Contributes to", "GOAL-G"),
        accepted("REQ-R")
            .link("Satisfies", "NEED-N")
            .link("Mitigates", "RISK-K"),
        accepted("VER-V")
            .field("Methods", "Test")
            .link("Verifies", "REQ-R"),
        accepted("SCN-S"),
        accepted("VAL-L")
            .field("Methods", "Demonstration")
            .link("Validates", "NEED-N")
            .link("Exercises", "SCN-S"),
        accepted("DES-D").link("Satisfies", "REQ-R"),
        accepted("ADR-A").link("Justifies", "REQ-R"),
        accepted("RISK-K")
            .field("Likelihood", "Rare")
            .field("Consequence", "Minimal")
            .link("Threatens", "REQ-R"),
        accepted("TERM-T")
            .title("Widget")
            .field("Aliases", "Gadget"),
        accepted("EVD-E").link("Supports", "REQ-R"),
    ]
}

/// Validates the base with `items`.
fn validate(project: &mut Project, items: &[Item]) -> BTreeSet<(String, String)> {
    let mut all = base();
    all.extend_from_slice(items);
    project.write(&all);
    project.validate()
}

/// Requires `breaks` to give exactly the diagnostics `expected`, as (code, ID); the same items
/// as drafts to give none of the template's; and each of `keeps` to be valid.
#[track_caller]
fn check(breaks: &[Item], expected: &[(&str, &str)], keeps: &[Vec<Item>]) {
    let mut project = Project::new();
    let expected: BTreeSet<(String, String)> = expected
        .iter()
        .map(|(code, id)| ((*code).to_owned(), (*id).to_owned()))
        .collect();
    assert_eq!(
        validate(&mut project, breaks),
        expected,
        "breaks: {breaks:#?}"
    );
    let drafts: Vec<Item> = breaks.iter().map(Item::as_draft).collect();
    let others: BTreeSet<(String, String)> = expected
        .iter()
        .filter(|(code, _)| !INVARIANTS.contains(&code.as_str()))
        .cloned()
        .collect();
    assert_eq!(
        validate(&mut project, &drafts),
        others,
        "drafts: {drafts:#?}"
    );
    for keep in keeps {
        assert_eq!(
            validate(&mut project, keep),
            BTreeSet::new(),
            "keeps: {keep:#?}"
        );
    }
}

#[test]
fn the_template_declares_exactly_these_invariants() {
    let project = Project::new();
    let run = project.mara(&["query", "list", "--limit", "100"]);
    assert_eq!(run.status, 0, "{run:#?}");
    let declared: BTreeSet<&str> = run.json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["kind"] == "invariant")
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert_eq!(declared, INVARIANTS.into_iter().collect());
}

#[test]
fn the_base_project_is_valid() {
    let mut project = Project::new();
    assert_eq!(validate(&mut project, &[]), BTreeSet::new());
}

fn verified(requirement: &str) -> Item {
    accepted(&format!("VER-OF-{requirement}"))
        .field("Methods", "Test")
        .link("Verifies", requirement)
}

#[test]
fn requirement_has_basis() {
    let verification = verified("REQ-X");
    let with = |basis: &[Item], requirement: Item| {
        let mut items = vec![requirement, verification.clone()];
        items.extend_from_slice(basis);
        items
    };
    let error = [("requirement-has-basis", "REQ-X")];
    let keeps = [
        with(&[], accepted("REQ-X").link("Satisfies", "NEED-N")),
        with(&[], accepted("REQ-X").link("Decomposes", "REQ-R")),
        with(&[], accepted("REQ-X").link("Derives from", "REQ-R")),
        with(
            &[accepted("ADR-X").link("Justifies", "REQ-X")],
            accepted("REQ-X"),
        ),
    ];
    check(&with(&[], accepted("REQ-X")), &error, &keeps);
    // Only a draft basis: the satisfied need is not accepted, which the lifecycle rule also
    // reports; a draft decision's justification is no basis either.
    check(
        &with(
            &[draft("NEED-X")],
            accepted("REQ-X").link("Satisfies", "NEED-X"),
        ),
        &[
            ("requirement-has-basis", "REQ-X"),
            ("links-to-accepted", "REQ-X"),
        ],
        &[],
    );
    check(
        &with(
            &[draft("ADR-X").link("Justifies", "REQ-X")],
            accepted("REQ-X"),
        ),
        &error,
        &[],
    );
}

#[test]
fn requirement_verified() {
    let requirement = accepted("REQ-X").link("Satisfies", "NEED-N");
    let error = [("requirement-verified", "REQ-X")];
    let keeps = [vec![requirement.clone(), verified("REQ-X")]];
    check(std::slice::from_ref(&requirement), &error, &keeps);
    // Only a draft verification.
    check(&[requirement, verified("REQ-X").as_draft()], &error, &[]);
}

#[test]
fn design_has_basis() {
    let error = [("design-has-basis", "DES-X")];
    let keeps = [
        vec![accepted("DES-X").link("Satisfies", "REQ-R")],
        vec![accepted("DES-X").link("Contributes to", "GOAL-G")],
        vec![accepted("DES-X").link("Decomposes", "DES-D")],
    ];
    check(&[accepted("DES-X")], &error, &keeps);
    // A justifying decision is no basis.
    check(
        &[
            accepted("DES-X"),
            accepted("ADR-X").link("Justifies", "DES-X"),
        ],
        &error,
        &[],
    );
}

#[test]
fn risk_assessed() {
    let risk = || accepted("RISK-X").link("Threatens", "REQ-R");
    let error = [("risk-assessed", "RISK-X")];
    let keeps = [vec![
        risk()
            .field("Likelihood", "Likely")
            .field("Consequence", "Major"),
    ]];
    check(&[risk().field("Likelihood", "Likely")], &error, &keeps);
    check(&[risk().field("Consequence", "Major")], &error, &[]);
    check(&[risk()], &error, &[]);
    // Two likelihoods: the schema also reports the repeated field.
    check(
        &[risk()
            .field("Likelihood", "Rare")
            .field("Likelihood", "Likely")
            .field("Consequence", "Major")],
        &[("risk-assessed", "RISK-X"), ("repeated-key", "RISK-X")],
        &[],
    );
}

#[test]
fn risk_threatens() {
    let risk = || {
        accepted("RISK-X")
            .field("Likelihood", "Possible")
            .field("Consequence", "Minor")
    };
    let keeps = [
        vec![risk().link("Threatens", "GOAL-G")],
        vec![risk().link("Threatens", "NEED-N")],
        vec![risk().link("Threatens", "REQ-R")],
    ];
    check(&[risk()], &[("risk-threatens", "RISK-X")], &keeps);
    check(
        &[risk().link("Threatens", "REQ-X"), draft("REQ-X")],
        &[
            ("risk-threatens", "RISK-X"),
            ("links-to-accepted", "RISK-X"),
        ],
        &[],
    );
}

#[test]
fn verification_has_method() {
    let keeps = [vec![
        accepted("VER-X")
            .field("Methods", "Analysis")
            .link("Verifies", "REQ-R"),
    ]];
    check(
        &[accepted("VER-X").link("Verifies", "REQ-R")],
        &[("verification-has-method", "VER-X")],
        &keeps,
    );
}

#[test]
fn verification_has_target() {
    let verification = || accepted("VER-X").field("Methods", "Inspection");
    let keeps = [vec![verification().link("Verifies", "REQ-R")]];
    check(
        &[verification()],
        &[("verification-has-target", "VER-X")],
        &keeps,
    );
    check(
        &[verification().link("Verifies", "REQ-X"), draft("REQ-X")],
        &[
            ("verification-has-target", "VER-X"),
            ("links-to-accepted", "VER-X"),
        ],
        &[],
    );
}

#[test]
fn validation_has_method() {
    let keeps = [vec![
        accepted("VAL-X")
            .field("Methods", "Test")
            .link("Validates", "GOAL-G"),
    ]];
    check(
        &[accepted("VAL-X").link("Validates", "GOAL-G")],
        &[("validation-has-method", "VAL-X")],
        &keeps,
    );
}

#[test]
fn validation_has_target() {
    let validation = || accepted("VAL-X").field("Methods", "Test");
    let error = [("validation-has-target", "VAL-X")];
    let keeps = [
        vec![validation().link("Validates", "NEED-N")],
        vec![validation().link("Validates", "GOAL-G")],
    ];
    check(&[validation()], &error, &keeps);
    // Exercising a scenario validates nothing.
    check(&[validation().link("Exercises", "SCN-S")], &error, &[]);
}

#[test]
fn term_unique() {
    let both = [("term-unique", "TERM-T"), ("term-unique", "TERM-X")];
    let keeps = [vec![
        accepted("TERM-X").title("Sprocket").field("Aliases", "Cog"),
    ]];
    // Title against alias, alias against title and alias against alias, ignoring case.
    check(&[accepted("TERM-X").title("gADGET")], &both, &keeps);
    check(
        &[accepted("TERM-X").title("Other").field("Aliases", "WIDGET")],
        &both,
        &[],
    );
    check(
        &[accepted("TERM-X").title("Other").field("Aliases", "gadget")],
        &both,
        &[],
    );
    // Folded to NFC: a decomposed accent equals the composed one.
    check(
        &[
            accepted("TERM-X").title("Café"),
            accepted("TERM-Y").title("CAFE\u{301}"),
        ],
        &[("term-unique", "TERM-X"), ("term-unique", "TERM-Y")],
        &[],
    );
}

#[test]
fn term_aliases() {
    let keeps = [vec![
        accepted("TERM-X").title("Bolt").field("Aliases", "Nut"),
    ]];
    check(
        &[accepted("TERM-X").title("Bolt").field("Aliases", "BOLT")],
        &[("term-aliases", "TERM-X")],
        &keeps,
    );
}

#[test]
fn links_to_accepted() {
    let validation = || {
        accepted("VAL-X")
            .field("Methods", "Test")
            .link("Validates", "NEED-N")
            .link("Exercises", "SCN-X")
    };
    let error = [("links-to-accepted", "VAL-X")];
    let keeps = [
        vec![validation(), accepted("SCN-X")],
        // Developmental relations may target drafts and retired items.
        vec![
            accepted("ADR-X").link("Justifies", "REQ-X"),
            draft("REQ-X"),
            accepted("EVD-X").link("Supports", "REQ-Y"),
            retired("REQ-Y"),
        ],
    ];
    check(&[validation(), draft("SCN-X")], &error, &keeps);
    check(
        &[
            accepted("REQ-X")
                .link("Satisfies", "NEED-N")
                .link("Decomposes", "REQ-Y"),
            verified("REQ-X"),
            retired("REQ-Y"),
        ],
        &[("links-to-accepted", "REQ-X")],
        &[],
    );
}

#[test]
fn supersedes_retired() {
    let keeps = [
        // `C supersedes B supersedes A` after B retires.
        vec![
            accepted("GOAL-C").link("Supersedes", "GOAL-B"),
            retired("GOAL-B").link("Supersedes", "GOAL-A"),
            retired("GOAL-A"),
        ],
        // A draft replacement judges nothing yet.
        vec![
            draft("GOAL-C").link("Supersedes", "GOAL-B"),
            accepted("GOAL-B"),
        ],
    ];
    check(
        &[
            accepted("GOAL-C").link("Supersedes", "GOAL-B"),
            accepted("GOAL-B"),
        ],
        &[("supersedes-retired", "GOAL-C")],
        &keeps,
    );
    check(
        &[
            retired("GOAL-C").link("Supersedes", "GOAL-B"),
            draft("GOAL-B"),
        ],
        &[("supersedes-retired", "GOAL-C")],
        &[],
    );
}
