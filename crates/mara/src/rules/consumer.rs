//! One entry evaluated for a request: a query, a view or a gate, with the inputs the request
//! supplies (DES-RULE-CONSUMERS and DES-REQUEST-INPUTS in `docs/rules/index.mara.md`,
//! DES-RULE-EXECUTION in `docs/rules/evaluation.mara.md`).

use std::collections::BTreeMap;

use mara_engine::config::CONFIG_PATH;
use mara_engine::diagnostic::Diagnostic;
use mara_engine::evaluation::{self, Fault, Limits, Roots, Rows};
use mara_engine::facts::{InputError, MARA_DL, Relation, Supplied, check_inputs};
use mara_engine::hash::Hash;
use mara_engine::program::{Column, Files};
use mara_engine::work::WorkCounts;

use super::{Evaluations, Run};
use crate::inputs::Generation;

/// The evaluation of one entry.
#[derive(Debug)]
pub struct Consumed {
    /// The columns of each relation the entry reads, as its program declares them; empty when
    /// the program could not be admitted.
    pub columns: BTreeMap<String, Vec<Column>>,
    /// The rows of the run, or the problems that leave it without rows: a missing or refused
    /// program, an entry that does not fit its program, a compile error or a fault. Each makes
    /// the outcome unavailable, never passed.
    pub outcome: Result<Rows, Vec<Diagnostic>>,
    /// The key of the evaluation, when one was run or found.
    pub key: Option<Hash>,
}

/// Evaluates the program of the entry at `entry` of the configuration's `[rules]` with the
/// snapshot's facts and the `supplied` inputs, `complete` naming those that hold every record
/// of their source. The inputs are checked against the entry's before anything runs, and a
/// refusal names the input. Every other entry of the program shapes the same run: an input
/// that only another entry lists is read empty, as validation reads it. Nothing supplied is
/// stored.
pub fn consume(
    generation: &Generation,
    entry: usize,
    supplied: Vec<(String, Supplied)>,
    complete: &[String],
    evaluations: &mut Evaluations,
    work: &mut WorkCounts,
) -> Result<Consumed, InputError> {
    let rules = (generation.project.config.rules.as_ref())
        .expect("the request names an entry of the configuration");
    let files = Files {
        declarations: MARA_DL.as_bytes(),
        programs: &generation.programs,
    };
    let plan = evaluation::plan(rules, Roots::Entry(entry), &files);
    let found = plan.programs.iter().find_map(|planned| {
        let fits = planned.entries.iter().find(|fits| fits.entry == entry)?;
        Some((planned, fits))
    });
    let Some((planned, fits)) = found else {
        return Ok(Consumed {
            columns: BTreeMap::new(),
            outcome: Err(entry_problems(rules, entry, plan.diagnostics)),
            key: None,
        });
    };
    let checked = check_inputs(&fits.inputs, supplied, complete)?;
    let program = &planned.program;
    let consumer = &rules.entries[entry];
    let columns = (consumer.relations().into_iter())
        .filter_map(|named| {
            let relation = program.relations.get(&named.name)?;
            Some((named.name.clone(), relation.columns.clone()))
        })
        .collect();
    let mut inputs = vec![checked.source_complete];
    for input in planned.inputs() {
        let given = checked
            .relations
            .iter()
            .find(|given| given.name == input.name);
        inputs.push(
            given
                .cloned()
                .unwrap_or_else(|| Relation::new(&input.name, Vec::new())),
        );
    }
    let snapshot = &generation.snapshot;
    let mut relations: Vec<&Relation> = snapshot.facts().relations.iter().collect();
    relations.extend(&inputs);
    let outputs: Vec<&str> = planned.outputs.iter().map(String::as_str).collect();
    let limits = Limits::with_time(rules.time_limit_seconds());
    let key = evaluation::key(
        Some(snapshot.key()),
        program,
        &inputs.iter().collect::<Vec<_>>(),
        &outputs,
        limits,
    );
    let outcome = match super::souffle() {
        Err(problem) => Err(Fault::Host(problem)),
        Ok(souffle) => {
            let run = Run {
                souffle: &souffle,
                program,
                relations: &relations,
                outputs: &outputs,
                limits,
            };
            evaluations.evaluate(key, &run, work).clone()
        }
    };
    Ok(Consumed {
        columns,
        outcome: outcome.map_err(|fault| fault.diagnostics(program, planned.declared_at)),
        key: Some(key),
    })
}

/// The problems of `diagnostics`, a plan of the entry's program alone, that keep the entry at
/// `entry` from running: those in program files, and those at a configuration line of the
/// entry.
fn entry_problems(
    rules: &mara_engine::config::Rules,
    entry: usize,
    diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    let consumer = &rules.entries[entry];
    let mut lines = vec![consumer.line, consumer.program.line];
    lines.extend(consumer.relations().iter().map(|named| named.line));
    lines.extend(consumer.inputs.iter().map(|named| named.line));
    let own = |diagnostic: &Diagnostic| {
        diagnostic.path != CONFIG_PATH || lines.contains(&diagnostic.line)
    };
    let problems: Vec<Diagnostic> = diagnostics.iter().filter(|d| own(d)).cloned().collect();
    match problems.is_empty() {
        true => diagnostics,
        false => problems,
    }
}
