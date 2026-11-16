//! VER-ADMITTED-LANGUAGE in `docs/rules/verification.mara.md`, on the fixtures in
//! `tests/programs/`: the review probes (`review4/`, `review5/`), the trial programs rewritten in
//! the admitted language (`trial/`) and the trial's compile errors (`trial-errors/`).
//!
//! Each probe's `case.json` holds the relations its wrapper read and wrote, which are the
//! boundary, what the probe recorded natively, and `parser`: `"admitted"`, or the line and
//! column where the parser refuses it.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mara_engine::program::{self, BoundaryError, Files};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs")
}

/// The `.dl` files of `dir`, by name.
fn programs(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "dl"))
        .map(|path| {
            let name = path.file_name().unwrap().to_str().unwrap().to_owned();
            (name, fs::read(&path).unwrap())
        })
        .collect()
}

/// `"admitted"`, or `"path:line:column"` where the program or a boundary relation is refused,
/// with the refusal's message.
fn verdict(
    root: &str,
    programs: &BTreeMap<String, Vec<u8>>,
    boundary: &[String],
) -> (String, String) {
    let declarations = programs
        .get(program::DECLARATIONS)
        .cloned()
        .unwrap_or_else(|| fs::read(fixtures().join("trial/mara.dl")).unwrap());
    let mut declared = programs.clone();
    declared.remove(program::DECLARATIONS);
    let files = Files {
        declarations: &declarations,
        programs: &declared,
    };
    let refused = |refusal: program::Refusal| {
        let location = refusal.location;
        let place = format!("{}:{}:{}", location.path, location.line, location.column);
        (place, refusal.message)
    };
    let program = match program::admit(root, &files) {
        Ok(program) => program,
        Err(refusal) => return refused(refusal),
    };
    for name in boundary {
        match program.boundary(name) {
            Ok(_) => {}
            Err(BoundaryError::Refused(refusal)) => return refused(refusal),
            Err(BoundaryError::Undeclared) => {
                return (format!("undeclared {name}"), String::new());
            }
        }
    }
    ("admitted".to_owned(), String::new())
}

fn strings(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_str().unwrap().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn probes_are_admitted_or_refused_where_expected() {
    let mut mismatches = Vec::new();
    let mut admitted = 0;
    for set in ["review4", "review5"] {
        let mut cases: Vec<_> = fs::read_dir(fixtures().join(set))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        cases.sort();
        for case in cases {
            let name = format!("{set}/{}", case.file_name().unwrap().to_str().unwrap());
            let json: serde_json::Value =
                serde_json::from_slice(&fs::read(case.join("case.json")).unwrap()).unwrap();
            let (actual, message) =
                verdict("program.dl", &programs(&case), &strings(&json["boundary"]));
            let expected = json["parser"].as_str().unwrap_or("<missing>");
            if actual != expected {
                mismatches.push(format!(
                    "{name}: expected {expected}, got {actual} ({message})"
                ));
            }
            if actual == "admitted" {
                admitted += 1;
                // Every admitted probe compiled: Soufflé's compile diagnostics start with
                // `Error:`, and a compile error ends the run before any output.
                let stderr = json["native"]["stderr"].as_str().unwrap();
                if stderr.contains("Error:") || stderr.contains("Warning: Deprecated") {
                    mismatches.push(format!("{name}: admitted, but did not compile: {stderr}"));
                }
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    assert!(admitted > 50, "only {admitted} probes admitted");
}

#[test]
fn rewritten_trial_programs_are_admitted() {
    let dir = fixtures().join("trial");
    let programs = programs(&dir);
    let cases: [(&str, &[&str]); 5] = [
        ("ancestry.dl", &["ancestor", "in_cycle"]),
        ("coverage.dl", &["covered", "uncovered"]),
        (
            "risk.dl",
            &["threshold", "risk_score", "risk_above", "risk_unavailable"],
        ),
        ("results.dl", &["verification_result", "result_status"]),
        (
            "gate_implementation_ready.dl",
            &["threshold", "failed", "unavailable", "outcome"],
        ),
    ];
    for (root, boundary) in cases {
        let boundary: Vec<String> = boundary.iter().map(|name| (*name).to_owned()).collect();
        assert_eq!(verdict(root, &programs, &boundary).0, "admitted", "{root}");
    }
}

/// The compile errors of EVD-SOUFFLE-TRIAL are Soufflé's to report; the division is refused.
#[test]
fn trial_compile_errors_reach_soufflé_and_division_is_refused() {
    let programs = programs(&fixtures().join("trial-errors"));
    for root in [
        "e1_undeclared.dl",
        "e2_arity.dl",
        "e3_subtype.dl",
        "e4_number_symbol.dl",
        "e5_unstratified.dl",
    ] {
        assert_eq!(verdict(root, &programs, &[]).0, "admitted", "{root}");
    }
    assert_eq!(
        verdict("e6_div_zero.dl", &programs, &[]).0,
        "e6_div_zero.dl:4:6"
    );
}
