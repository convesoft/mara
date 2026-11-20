//! The code-dependent cases of VER-TEMPLATE-CASES: `implemented` from `implements` markers in
//! code files (DES-TEMPLATE-QUERIES), and `verified` and `release-ready` when code links are
//! there, missing, or unknown because a code file cannot be read (DES-TEMPLATE-GATES,
//! DES-CODE-READERS).

use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::readiness::{Readiness, set};
use crate::{Item, accepted, draft, rows};

/// A requirement of NEED-N with a verification, a design and a validation of the need.
fn items(design: Item) -> Vec<Item> {
    vec![
        accepted("GOAL-G"),
        accepted("NEED-N").link("Contributes to", "GOAL-G"),
        accepted("REQ-A").link("Satisfies", "NEED-N"),
        accepted("VER-A")
            .field("Methods", "Test")
            .link("Verifies", "REQ-A"),
        design.link("Satisfies", "REQ-A"),
        accepted("VAL-N")
            .field("Methods", "Demonstration")
            .link("Validates", "NEED-N"),
    ]
}

/// `src/lib.rs` with an `implements` marker for each of `ids`, from line 2.
fn markers(ids: &[&str]) -> Vec<(&'static str, String)> {
    let mut text = String::from("pub fn run() {}\n");
    for id in ids {
        text.push_str(&format!("// @mara implements {id}\n"));
    }
    vec![("src/lib.rs", text)]
}

/// The arguments of `release-ready` for REQ-A and VAL-N: passing results for candidate `c1`
/// and context `prod`, both inputs complete.
fn passing(r: &Readiness) -> Vec<String> {
    let verification = r.result("VER-A", "REQ-A", "c1", "pass");
    let validation = r.result("VAL-N", "NEED-N", "prod", "pass");
    r.release(&[verification], &[["VAL-N", "prod"]], &[validation], true)
}

/// The outcome of `verified` for REQ-A and of `release-ready` for REQ-A and VAL-N, each with
/// passing results, and REQ-A's rows in `release-ready`.
fn outcomes(r: &Readiness) -> (String, String, std::collections::BTreeSet<String>) {
    let result = r.result("VER-A", "REQ-A", "c1", "pass");
    let verified = r.gate("verified", &["REQ-A"], &r.verification(&[result], true));
    let release = r.gate("release-ready", &["REQ-A", "VAL-N"], &passing(r));
    (
        verified.outcome,
        release.outcome,
        release.items[0].rows.clone(),
    )
}

#[test]
fn a_requirement_that_a_marker_implements_passes_with_passing_results() {
    let r = Readiness::with_code(&items(accepted("DES-A")), Some(&markers(&["REQ-A"])));
    assert_eq!(
        r.rows("execute", "implemented", &["REQ-A"], &[]),
        rows(&[["REQ-A", "src/lib.rs", "2", ""]])
            .into_iter()
            .collect::<Vec<_>>()
    );
    let (verified, release, _) = outcomes(&r);
    assert_eq!((verified.as_str(), release.as_str()), ("passed", "passed"));
}

#[test]
fn markers_naming_a_design_and_a_verification_each_give_a_row() {
    let r = Readiness::with_code(
        &items(accepted("DES-A")),
        Some(&markers(&["DES-A", "VER-A"])),
    );
    assert_eq!(
        r.rows("execute", "implemented", &[], &[]),
        rows(&[
            ["DES-A", "src/lib.rs", "2", ""],
            ["REQ-A", "src/lib.rs", "2", "DES-A"],
            ["VER-A", "src/lib.rs", "3", ""],
        ])
        .into_iter()
        .collect::<Vec<_>>()
    );
}

#[test]
fn an_implementation_through_a_design_counts_only_when_the_design_is_accepted() {
    let r = Readiness::with_code(&items(accepted("DES-A")), Some(&markers(&["DES-A"])));
    let (verified, release, _) = outcomes(&r);
    assert_eq!((verified.as_str(), release.as_str()), ("passed", "passed"));

    let r = Readiness::with_code(&items(draft("DES-A")), Some(&markers(&["DES-A"])));
    assert_eq!(
        r.rows("execute", "implemented", &[], &[]),
        rows(&[["DES-A", "src/lib.rs", "2", ""]])
            .into_iter()
            .collect::<Vec<_>>()
    );
    let (verified, release, rows) = outcomes(&r);
    assert_eq!((verified.as_str(), release.as_str()), ("failed", "failed"));
    assert!(
        rows.contains("condition not_implemented(REQ-A)"),
        "{rows:?}"
    );
}

#[test]
fn a_requirement_that_no_marker_names_fails_once_all_code_was_read() {
    // Code declared and read, with a marker for another item; then no code language at all.
    let declared = markers(&["VER-A"]);
    for code in [Some(&declared[..]), None] {
        let r = Readiness::with_code(&items(accepted("DES-A")), code);
        let (verified, release, rows) = outcomes(&r);
        assert_eq!(
            (verified.as_str(), release.as_str()),
            ("failed", "failed"),
            "{code:?}"
        );
        assert_eq!(rows, set(&["condition not_implemented(REQ-A)"]), "{code:?}");
    }
}

#[test]
fn a_requirement_that_no_marker_names_is_unavailable_while_code_cannot_be_read() {
    assert_ne!(
        // SAFETY: `geteuid` has no preconditions.
        unsafe { libc::geteuid() },
        0,
        "this case needs a user without CAP_DAC_OVERRIDE"
    );
    let mut files = markers(&["VER-A"]);
    files.push(("src/hidden.rs", "// @mara implements REQ-A\n".to_owned()));
    let r = Readiness::with_code(&items(accepted("DES-A")), Some(&files));
    let hidden = r.project.path().join("src/hidden.rs");
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o000)).unwrap();
    let (verified, release, rows) = outcomes(&r);
    let result = r.result("VER-A", "REQ-A", "c1", "pass");
    let view = r.rows(
        "view",
        "delivery-readiness",
        &["REQ-A"],
        &r.verification(&[result], true),
    );
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        (verified.as_str(), release.as_str()),
        ("unavailable", "unavailable")
    );
    assert_eq!(rows, set(&["unavailable code_unavailable(REQ-A)"]));
    assert_eq!(view[0][3], "unavailable", "{view:?}");
    // Readable again: the marker implements REQ-A.
    let (verified, release, _) = outcomes(&r);
    assert_eq!((verified.as_str(), release.as_str()), ("passed", "passed"));
}

/// Every leaf of `node`, a proof node of `query report`.
fn leaves(node: &serde_json::Value, out: &mut Vec<serde_json::Value>) {
    match node["children"].as_array() {
        Some(children) => children.iter().for_each(|child| leaves(child, out)),
        None => out.push(node.clone()),
    }
}

/// The leaves of the proofs of `query report verified --id REQ-A`, with passing results.
fn report_leaves(r: &Readiness) -> Vec<serde_json::Value> {
    let result = r.result("VER-A", "REQ-A", "c1", "pass");
    let args = r.verification(&[result], true);
    let mut command = vec!["query", "report", "verified", "--id", "REQ-A"];
    command.extend(args.iter().map(String::as_str));
    let run = r.project.mara(&command);
    assert_eq!(run.status, 0, "{run:#?}");
    let mut out = Vec::new();
    for row in run.json["rows"].as_array().unwrap() {
        leaves(&row["proof"], &mut out);
    }
    out
}

/// `not_implemented` rests on `source_complete("code")`, which Mara supplied; `code_unavailable`
/// on its absence.
#[test]
fn a_report_shows_whether_all_code_was_read() {
    let complete = serde_json::json!({"input": "source_complete", "values": ["code"]});
    let r = Readiness::with_code(&items(accepted("DES-A")), Some(&markers(&["VER-A"])));
    assert!(
        report_leaves(&r).contains(&complete),
        "{:?}",
        report_leaves(&r)
    );

    assert_ne!(
        // SAFETY: `geteuid` has no preconditions.
        unsafe { libc::geteuid() },
        0,
        "this case needs a user without CAP_DAC_OVERRIDE"
    );
    let mut files = markers(&["VER-A"]);
    files.push(("src/hidden.rs", "fn hidden() {}\n".to_owned()));
    let r = Readiness::with_code(&items(accepted("DES-A")), Some(&files));
    let hidden = r.project.path().join("src/hidden.rs");
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o000)).unwrap();
    let found = report_leaves(&r);
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!found.contains(&complete), "{found:?}");
    assert!(
        found.iter().any(|leaf| leaf["absent"]
            .as_str()
            .is_some_and(|atom| atom.contains("source_complete"))),
        "{found:?}"
    );
}
