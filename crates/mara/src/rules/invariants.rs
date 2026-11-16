//! Invariants in validation: every program evaluated once, and the rows of its invariants as
//! diagnostics (DES-RULE-CONSUMERS in `docs/rules/index.mara.md`, DES-RULE-EXECUTION in
//! `docs/rules/evaluation.mara.md`).

use mara_engine::config::EntryKind;
use mara_engine::diagnostic::Diagnostic;
use mara_engine::evaluation::{self, Fault, InvariantFinding, Limits, Roots};
use mara_engine::facts::{MARA_DL, Relation, SNAPSHOT_RELATIONS, SOURCE_COMPLETE};
use mara_engine::hash::Hash;
use mara_engine::program::Files;
use mara_engine::work::WorkCounts;

use super::{Evaluations, Run};
use crate::inputs::Generation;

/// What validation evaluates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// `project validate`: every declared program, and the invariants.
    Project,
    /// `item validate`: the programs that invariants consume, and the invariants.
    Item,
}

/// The outcome of the invariants of one validation.
#[derive(Debug, Default)]
pub struct Invariants {
    /// One per row of an invariant.
    pub findings: Vec<InvariantFinding>,
    /// Refused programs, entries that cannot run, Soufflé's compile diagnostics and faults:
    /// each an error that makes the outcome unavailable.
    pub diagnostics: Vec<Diagnostic>,
    /// The key of every evaluation, for the result's key.
    pub keys: Vec<Hash>,
}

/// Evaluates the programs of `scope` in `generation`, each once with the facts of its snapshot
/// when an invariant consumes it, else against empty facts, which compiles it. Validation
/// supplies no input: each input that an entry lists is empty, and no source is complete.
pub fn invariants(generation: &Generation, scope: Scope, work: &mut WorkCounts) -> Invariants {
    let mut outcome = Invariants::default();
    let Some(rules) = &generation.project.config.rules else {
        return outcome;
    };
    let files = Files {
        declarations: MARA_DL.as_bytes(),
        programs: &generation.programs,
    };
    let roots = match scope {
        Scope::Project => Roots::All,
        Scope::Item => Roots::Invariants,
    };
    let plan = evaluation::plan(rules, roots, &files);
    outcome.diagnostics = plan.diagnostics;
    let snapshot = &generation.snapshot;
    let limits = Limits::with_time(rules.time_limit_seconds());
    let empty_facts: Vec<Relation> = SNAPSHOT_RELATIONS
        .iter()
        .map(|name| Relation::new(name, Vec::new()))
        .collect();
    let mut evaluations = Evaluations::default();
    let souffle = super::souffle();
    for planned in &plan.programs {
        let souffle = match &souffle {
            Ok(souffle) => souffle.as_path(),
            Err(problem) => {
                let fault = Fault::Host(problem.clone());
                let diagnostics = fault.diagnostics(&planned.program, planned.declared_at);
                outcome.diagnostics.extend(diagnostics);
                continue;
            }
        };
        let invariants: Vec<_> = planned
            .entries
            .iter()
            .map(|planned| &rules.entries[planned.entry])
            .filter(|entry| entry.kind == EntryKind::Invariant)
            .collect();
        let with_facts = !invariants.is_empty();
        let facts = match with_facts {
            true => &snapshot.facts().relations,
            false => &empty_facts,
        };
        let mut supplied = vec![Relation::new(SOURCE_COMPLETE, Vec::new())];
        supplied.extend(
            planned
                .inputs()
                .into_iter()
                .map(|input| Relation::new(&input.name, Vec::new())),
        );
        let mut relations: Vec<&Relation> = facts.iter().collect();
        relations.extend(&supplied);
        let outputs: Vec<&str> = planned.outputs.iter().map(String::as_str).collect();
        let run = Run {
            souffle,
            program: &planned.program,
            relations: &relations,
            outputs: &outputs,
            limits,
        };
        let key = evaluation::key(
            with_facts.then(|| snapshot.key()),
            &planned.program,
            &supplied.iter().collect::<Vec<_>>(),
            &outputs,
            limits,
        );
        outcome.keys.push(key);
        match evaluations.evaluate(key, &run, work) {
            Ok(rows) => {
                for entry in invariants {
                    let relation = &entry.relations()[0].name;
                    let rows = rows.relations.get(relation).map_or(&[][..], Vec::as_slice);
                    let findings = evaluation::invariant_findings(entry, rows, snapshot);
                    outcome.findings.extend(findings);
                }
            }
            Err(fault) => outcome
                .diagnostics
                .extend(fault.diagnostics(&planned.program, planned.declared_at)),
        }
    }
    outcome
        .diagnostics
        .sort_by(|a, b| (&a.path, a.line, &a.message).cmp(&(&b.path, b.line, &b.message)));
    outcome.diagnostics.dedup();
    outcome
}
