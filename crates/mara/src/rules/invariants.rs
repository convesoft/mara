//! Invariants in validation: every program evaluated once, and the rows of its invariants as
//! diagnostics (DES-RULE-CONSUMERS in `docs/rules/index.mara.md`, DES-RULE-EXECUTION in
//! `docs/rules/evaluation.mara.md`).

use mara_engine::config::EntryKind;
use mara_engine::diagnostic::Diagnostic;
use mara_engine::evaluation::{self, Fault, InvariantFinding, Limits, Roots};
use mara_engine::facts::{CodeFacts, MARA_DL, Relation, SNAPSHOT_RELATIONS, SOURCE_COMPLETE};
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
/// supplies no input: each input that an entry lists is empty, and no source is complete but
/// code. A program that an invariant consumes and that uses code links reads those that `code`
/// gives, called at most once, and `source_complete("code")` when all code was read
/// (DES-CODE-READERS).
pub fn invariants(
    generation: &Generation,
    scope: Scope,
    code: &mut dyn FnMut(&mut WorkCounts) -> CodeFacts,
    work: &mut WorkCounts,
) -> Invariants {
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
    let mut code_facts: Option<CodeFacts> = None;
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
        let none_complete = Relation::new(SOURCE_COMPLETE, Vec::new());
        let unread = CodeFacts::unread();
        let code = match with_facts && planned.uses_code() {
            true => &*code_facts.get_or_insert_with(|| code(work)),
            false => &unread,
        };
        let mut supplied = vec![code.source_complete(&none_complete), code.links.clone()];
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
        // A session's request reads what an earlier request of the session ran
        // (DES-DAEMON-SESSIONS).
        let kept;
        let evaluated = match &generation.evaluated {
            Some(evaluated) => {
                kept = evaluated.evaluate(key, &run, work);
                &*kept
            }
            None => evaluations.evaluate(key, &run, work),
        };
        match evaluated {
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
