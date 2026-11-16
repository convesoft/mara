//! VER-ADMITTED-LANGUAGE's standing compile check, and the rows the review probes recorded
//! natively (VER-PROGRAM-REACH-CASES), through the product's evaluation (`mara::rules`): every
//! probe fixture of `crates/mara-engine/tests/programs/` that the parser admits, and every
//! rewritten trial program, run with a wrapper that reads and writes its boundary relations as
//! JSON. A relation the probe supplied rows for reads them; every other relation reads none.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use mara::rules::{Run, evaluate};
use mara_engine::diagnostic::Severity;
use mara_engine::evaluation::{Fault, Limits, Rows};
use mara_engine::facts::{Relation, Value};
use mara_engine::program::{self, Files};
use mara_engine::work::WorkCounts;
use serde_json::Value as Json;

/// The pinned Soufflé that `scripts/build-souffle.sh` builds (DES-SOUFFLE-BUILD).
pub fn souffle() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/souffle-2.5/souffle");
    assert!(
        path.is_file(),
        "{} is missing: run scripts/build-souffle.sh first",
        path.display()
    );
    path
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../mara-engine/tests/programs")
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

/// Evaluates `root` of `dir` with the boundary relations `boundary` read and written, those of
/// `inputs` holding its rows. `None` when the parser refuses it.
fn run(dir: &Path, root: &str, boundary: &[String], inputs: &Json) -> Option<Result<Rows, Fault>> {
    let mut programs = programs(dir);
    let declarations = programs
        .remove(program::DECLARATIONS)
        .unwrap_or_else(|| fs::read(fixtures().join("trial/mara.dl")).unwrap());
    let files = Files {
        declarations: &declarations,
        programs: &programs,
    };
    let program = program::admit(root, &files).ok()?;
    if boundary.iter().any(|name| program.boundary(name).is_err()) {
        return None;
    }
    let relations: Vec<Relation> = boundary
        .iter()
        .map(|name| {
            let rows = inputs.get(name).filter(|rows| rows.is_array());
            Relation {
                name: name.clone(),
                rows: Vec::new(),
                json: rows.map_or("[]".to_owned(), Json::to_string),
            }
        })
        .collect();
    let relations: Vec<&Relation> = relations.iter().collect();
    let outputs: Vec<&str> = boundary.iter().map(String::as_str).collect();
    let souffle = souffle();
    let run = Run {
        souffle: &souffle,
        program: &program,
        relations: &relations,
        outputs: &outputs,
        limits: Limits::with_time(2),
    };
    Some(evaluate(&run, &mut WorkCounts::default()))
}

fn json_rows(rows: &[Vec<Value>]) -> BTreeSet<String> {
    let value = |value: &Value| match value {
        Value::Symbol(text) => Json::String(text.clone()),
        Value::Number(number) => Json::from(*number),
    };
    rows.iter()
        .map(|row| Json::Array(row.iter().map(value).collect()).to_string())
        .collect()
}

/// The probes whose evaluation faults: runtime warnings and errors, outputs that are not UTF-8,
/// an input that is no row (which the product refuses before any run), a run that never ends,
/// and Soufflé's compile warning for a relation without rules or facts.
const FAULTING: [&str; 15] = [
    "review4/not-row",
    "review4/substr-bad",
    "review4/unicode-substr",
    "review5/range_overflow_cycle",
    "review5/substr_beyond",
    "review5/substr_neg_start",
    "review5/tonum_15",
    "review5/tonum_16",
    "review5/tonum_19",
    "review5/tonum_20",
    "review5/tonum_21",
    "review5/tonum_23",
    "review5/tonum_25",
    "review5/substr_invalid_utf8",
    "review5/empty_aggs",
];

#[test]
fn every_admitted_probe_compiles_and_gives_its_recorded_rows() {
    let mut problems = Vec::new();
    let mut faulting = BTreeSet::new();
    let mut compared = 0;
    for set in ["review4", "review5"] {
        let mut cases: Vec<PathBuf> = fs::read_dir(fixtures().join(set))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        cases.sort();
        for case in cases {
            let name = format!("{set}/{}", case.file_name().unwrap().to_str().unwrap());
            let json: Json =
                serde_json::from_slice(&fs::read(case.join("case.json")).unwrap()).unwrap();
            if json["parser"] != "admitted" {
                continue;
            }
            let boundary: Vec<String> = json["boundary"]
                .as_array()
                .unwrap()
                .iter()
                .map(|name| name.as_str().unwrap().to_owned())
                .collect();
            let Some(outcome) = run(&case, "program.dl", &boundary, &json["inputs"]) else {
                problems.push(format!("{name}: refused"));
                continue;
            };
            let rows = match outcome {
                Ok(rows) => rows,
                Err(Fault::Compile(messages))
                    if messages
                        .iter()
                        .any(|message| message.severity == Severity::Error) =>
                {
                    problems.push(format!("{name}: does not compile: {messages:?}"));
                    continue;
                }
                Err(fault) => {
                    faulting.insert(name.clone());
                    eprintln!("{name}: {fault:?}");
                    continue;
                }
            };
            let native = &json["native"];
            if native["exit"] != 0 {
                continue;
            }
            for (relation, recorded) in native["outputs"].as_object().unwrap() {
                let Some(recorded) = recorded.as_array() else {
                    continue;
                };
                // Soufflé writes a row without columns as `null`.
                let recorded: BTreeSet<String> = recorded
                    .iter()
                    .map(|row| {
                        if row.is_null() {
                            "[]".to_owned()
                        } else {
                            row.to_string()
                        }
                    })
                    .collect();
                let got = json_rows(&rows.relations[relation]);
                if got != recorded {
                    problems.push(format!("{name}: {relation} is {got:?}, not {recorded:?}"));
                }
                compared += 1;
            }
        }
    }
    let expected: BTreeSet<String> = FAULTING.iter().map(|name| (*name).to_owned()).collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert_eq!(faulting, expected);
    assert!(compared > 80, "only {compared} relations compared");
}

#[test]
fn the_rewritten_trial_programs_compile() {
    let dir = fixtures().join("trial");
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
        let mut relations: Vec<String> = boundary.iter().map(|name| (*name).to_owned()).collect();
        let facts = mara_engine::facts::SNAPSHOT_RELATIONS.iter();
        let facts = facts.chain([&mara_engine::facts::SOURCE_COMPLETE]);
        relations.extend(facts.map(|name| (*name).to_owned()));
        let outcome = run(&dir, root, &relations, &Json::Null).expect("admitted");
        outcome.unwrap_or_else(|fault| panic!("{root}: {fault:?}"));
    }
}
