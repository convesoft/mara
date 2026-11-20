//! Which programs run, and which consumers read them (DES-RULE-CONSUMERS in
//! `docs/rules/index.mara.md`).

use std::collections::{BTreeMap, BTreeSet};

use crate::config::{CONFIG_PATH, EntryKind, Named, Rules};
use crate::diagnostic::Diagnostic;
use crate::facts::{CODE_LINK, Column, InputDecl};
use crate::program::{self, BoundaryError, ColumnKind, DECLARATIONS, Files, Program};

/// Which declared files run as programs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Roots {
    /// Every declared file that an entry names or that no other declared file includes, and
    /// then, in path order, each that no run so far reads, such as one in an include cycle:
    /// what `project validate` compiles. Every declared file is read by a run or refused.
    All,
    /// Only the files that invariants name: what `item validate` runs.
    Invariants,
    /// Only the file that the entry at this index of [`Rules::entries`] names: what a query,
    /// view or gate request runs.
    Entry(usize),
}

/// The programs to run, and every problem that keeps a program or an entry from running.
#[derive(Debug, Default)]
pub struct Plan {
    /// By path.
    pub programs: Vec<Planned>,
    /// Refused programs, and entries whose program or relations do not fit, in path and line
    /// order, each once.
    pub diagnostics: Vec<Diagnostic>,
}

/// An admitted program with the entries that consume it.
#[derive(Debug)]
pub struct Planned {
    pub path: String,
    pub program: Program,
    /// The configuration line that declares it ([`Rules::declaring_line`]).
    pub declared_at: usize,
    pub entries: Vec<PlannedEntry>,
    /// Every relation that one of `entries` names, each once, sorted: every consumer reads
    /// the same run.
    pub outputs: Vec<String>,
}

/// An entry whose relations and inputs fit its program.
#[derive(Debug)]
pub struct PlannedEntry {
    /// The index of the entry in [`Rules::entries`].
    pub entry: usize,
    /// Its inputs, with their columns, in the order the entry lists them.
    pub inputs: Vec<InputDecl>,
}

impl Planned {
    /// Whether its run reads code links: a clause of the program names `code_link`, or one of
    /// its entries consumes it (DES-CODE-READERS).
    pub fn uses_code(&self) -> bool {
        self.program.names(CODE_LINK) || self.outputs.iter().any(|output| output == CODE_LINK)
    }

    /// Every input that one of its entries lists, each once.
    pub fn inputs(&self) -> Vec<&InputDecl> {
        let mut inputs: Vec<&InputDecl> = Vec::new();
        for input in self.entries.iter().flat_map(|entry| &entry.inputs) {
            if !inputs.iter().any(|seen| seen.name == input.name) {
                inputs.push(input);
            }
        }
        inputs
    }
}

/// The programs of `rules` that `roots` selects, each admitted from `files`, with every entry
/// that consumes it: each shapes the one run that all of them read. A program that the parser
/// refuses is reported and does not run; neither does an entry whose program is missing or
/// refused, or whose relation or input its program does not declare or cannot pass across the
/// boundary.
pub fn plan(rules: &Rules, roots: Roots, files: &Files) -> Plan {
    let mut diagnostics = Vec::new();
    let mut named: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut by_invariants = BTreeSet::new();
    for (index, entry) in rules.entries.iter().enumerate() {
        let path = entry.program.path.as_str();
        let invariant = entry.kind == EntryKind::Invariant;
        if files.programs.contains_key(path) {
            named.entry(path).or_default().push(index);
            if invariant {
                by_invariants.insert(path);
            }
        } else if match roots {
            Roots::All => true,
            Roots::Invariants => invariant,
            Roots::Entry(entry) => entry == index,
        } {
            let message = format!("cannot read {path}: no declared program file has this path");
            let line = entry.program.line;
            diagnostics.push(Diagnostic::error("io-error", CONFIG_PATH, line, message));
        }
    }
    let admit = |path: &str| program::admit(path, files);
    let mut admitted: BTreeMap<&str, Result<Program, program::Refusal>> = BTreeMap::new();
    let selected: BTreeSet<&str> = match roots {
        Roots::Invariants => by_invariants,
        Roots::Entry(entry) => {
            let path = rules
                .entries
                .get(entry)
                .map(|entry| entry.program.path.as_str());
            path.filter(|path| files.programs.contains_key(*path))
                .into_iter()
                .collect()
        }
        Roots::All => {
            for path in files.programs.keys() {
                admitted.insert(path, admit(path));
            }
            let included: BTreeSet<&str> = admitted
                .iter()
                .filter_map(|(path, program)| Some((path, program.as_ref().ok()?)))
                .flat_map(|(path, program)| {
                    let others = program.files[1..].iter();
                    others.filter(move |file| *file != path && *file != DECLARATIONS)
                })
                .map(String::as_str)
                .collect();
            let not_included = files.programs.keys().map(String::as_str);
            let not_included = not_included.filter(|path| !included.contains(path));
            let mut selected: BTreeSet<&str> = not_included.chain(named.keys().copied()).collect();
            // A file that only files of an include cycle include is in no run yet: the first of
            // them, in path order, runs, and its run covers the others it reaches.
            let reached = |path: &str| -> Vec<String> {
                match admitted.get(path) {
                    Some(Ok(program)) => program.files.clone(),
                    _ => vec![path.to_owned()],
                }
            };
            let mut covered: BTreeSet<String> =
                selected.iter().flat_map(|path| reached(path)).collect();
            for path in files.programs.keys() {
                if !covered.contains(path) {
                    selected.insert(path);
                    covered.extend(reached(path));
                }
            }
            selected
        }
    };
    let mut programs = Vec::new();
    for path in selected {
        let program = match admitted.remove(path).unwrap_or_else(|| admit(path)) {
            Ok(program) => program,
            Err(refusal) => {
                diagnostics.push(refusal.diagnostic());
                continue;
            }
        };
        let mut entries = Vec::new();
        for &index in named.get(path).into_iter().flatten() {
            let entry = &rules.entries[index];
            let mut fits = true;
            for relation in entry.relations() {
                if let Err(diagnostic) = consumed(&program, path, relation) {
                    diagnostics.push(*diagnostic);
                    fits = false;
                }
            }
            let mut inputs = Vec::new();
            for named in &entry.inputs {
                match input(&program, path, named) {
                    Ok(input) => inputs.push(input),
                    Err(diagnostic) => {
                        diagnostics.push(*diagnostic);
                        fits = false;
                    }
                }
            }
            if fits {
                entries.push(PlannedEntry {
                    entry: index,
                    inputs,
                });
            }
        }
        let outputs: BTreeSet<String> = entries
            .iter()
            .flat_map(|planned| rules.entries[planned.entry].relations())
            .map(|relation| relation.name.clone())
            .collect();
        programs.push(Planned {
            path: path.to_owned(),
            program,
            declared_at: rules.declaring_line(path),
            entries,
            outputs: outputs.into_iter().collect(),
        });
    }
    diagnostics.sort_by(|a, b| (&a.path, a.line, &a.message).cmp(&(&b.path, b.line, &b.message)));
    diagnostics.dedup();
    Plan {
        programs,
        diagnostics,
    }
}

/// Checks a relation that an entry consumes: declared, able to cross the boundary, and with a
/// symbol in its first column, the MID of the item its row is about.
fn consumed(program: &Program, path: &str, named: &Named) -> Result<(), Box<Diagnostic>> {
    let relation = boundary(program, path, named)?;
    if relation.columns.first().map(|column| column.kind) != Some(ColumnKind::Symbol) {
        let message = format!(
            "the first column of `{}` must hold the MID of the item a row is about, a symbol",
            named.name
        );
        return Err(Box::new(Diagnostic::error(
            "invalid-relation",
            CONFIG_PATH,
            named.line,
            message,
        )));
    }
    Ok(())
}

/// The input that an entry lists: a relation of the program, not one of Mara's facts.
fn input(program: &Program, path: &str, named: &Named) -> Result<InputDecl, Box<Diagnostic>> {
    let relation = boundary(program, path, named)?;
    if program.files[relation.place.file] == DECLARATIONS {
        let message = format!(
            "`{}` is a relation whose facts Mara supplies, so no request supplies it",
            named.name
        );
        return Err(Box::new(Diagnostic::error(
            "invalid-relation",
            CONFIG_PATH,
            named.line,
            message,
        )));
    }
    let columns = relation.columns.iter().map(|column| match column.kind {
        ColumnKind::Number => Column::Number,
        _ => Column::Symbol,
    });
    Ok(InputDecl {
        name: named.name.clone(),
        columns: columns.collect(),
    })
}

fn boundary<'p>(
    program: &'p Program,
    path: &str,
    named: &Named,
) -> Result<&'p program::Relation, Box<Diagnostic>> {
    program.boundary(&named.name).map_err(|error| {
        Box::new(match error {
            BoundaryError::Undeclared => Diagnostic::error(
                "undeclared-relation",
                CONFIG_PATH,
                named.line,
                format!("`{}` is no relation of {path}", named.name),
            ),
            BoundaryError::Refused(refusal) => refusal.diagnostic(),
        })
    })
}
