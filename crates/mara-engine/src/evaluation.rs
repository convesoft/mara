//! What one evaluation of a program by Soufflé needs and what it means, without running it
//! (DES-RULE-EXECUTION in `docs/rules-evaluation.mara.md`): the files of the private directory,
//! the generated wrapper, the evaluation key, the limits, and the reading of Soufflé's exit,
//! diagnostics and output files into rows or a fault. The host writes the files, runs Soufflé
//! and hands back what it printed and wrote.

mod invariants;
mod plan;
mod souffle;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, Severity};
use crate::facts::{self, Relation, Value};
use crate::hash::{Hash, KeyHasher};
use crate::program::{DECLARATIONS, Program};
use crate::snapshot::SnapshotKey;

pub use invariants::{InvariantFinding, invariant_findings};
pub use plan::{Plan, Planned, PlannedEntry, Roots, plan};
pub use souffle::{Exit, interpret, read_output};

/// The version of what an evaluation derives, part of its key.
pub const EVALUATION_VERSION: u32 = 1;

/// The wrapper program that the host runs, in the private directory.
pub const WRAPPER: &str = "wrapper.dl";
/// The directory, in the private directory, that receives the output files.
pub const OUTPUT_DIR: &str = "out";
/// The files that receive Soufflé's standard output and error, in the private directory.
pub const STDOUT: &str = "stdout.txt";
pub const STDERR: &str = "stderr.txt";

/// The memory an evaluation may use: its address space, natively.
pub const MEMORY_LIMIT_BYTES: u64 = 1 << 30;
/// The size each file an evaluation writes may reach: an output relation, or what Soufflé
/// prints.
pub const OUTPUT_LIMIT_BYTES: u64 = 64 << 20;
/// The most of Soufflé's standard error that a fault quotes.
pub const QUOTED_BYTES: usize = 4096;

/// The limits of one evaluation (REQ-EVALUATION-TIME-LIMIT, DES-RULE-EXECUTION).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub time_seconds: u32,
    pub memory_bytes: u64,
    pub output_bytes: u64,
}

impl Limits {
    /// The fixed memory and output limits with a time limit of `time_seconds`.
    pub fn with_time(time_seconds: u32) -> Limits {
        Limits {
            time_seconds,
            memory_bytes: MEMORY_LIMIT_BYTES,
            output_bytes: OUTPUT_LIMIT_BYTES,
        }
    }
}

/// A limit that ended an evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    Time,
    Memory,
    Output,
}

/// The rows of each output relation of a run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rows {
    pub relations: BTreeMap<String, Vec<Vec<Value>>>,
}

/// A compile diagnostic of Soufflé, at a line of one of the program's files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub severity: Severity,
    /// Soufflé's message, without its location.
    pub text: String,
    /// The file, an index into `Program::files`; `None` for the wrapper.
    pub file: Option<usize>,
    pub line: usize,
    /// The byte offset in the line that Soufflé's caret points at.
    pub offset: usize,
}

/// Why an evaluation has no rows. Each makes its outcome unavailable, never passed
/// (REQ-PROGRAM-FAULTS-REPORTED).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Soufflé's compile diagnostics, its warnings included.
    Compile(Vec<Message>),
    /// A fault while evaluating, with what Soufflé printed, at the line of a program file when
    /// it names one.
    Runtime {
        message: String,
        at: Option<(usize, usize)>,
    },
    Limit(Limit, Limits),
    /// An output file that does not hold rows of its relation.
    Output {
        relation: String,
        problem: String,
    },
    /// The host could not run the evaluation.
    Host(String),
}

impl Fault {
    /// The error diagnostics of the fault in `program`, declared at line `declared_at` of the
    /// configuration: at the project path and line of the file concerned, else at that line.
    pub fn diagnostics(&self, program: &Program, declared_at: usize) -> Vec<Diagnostic> {
        let at_declaration = |message: String| {
            let message = format!("{}: {message}", program.files[0]);
            Diagnostic::error(
                "program-fault",
                crate::config::CONFIG_PATH,
                declared_at,
                message,
            )
        };
        match self {
            Fault::Compile(messages) => messages
                .iter()
                .map(|message| message.diagnostic(program, declared_at))
                .collect(),
            Fault::Runtime { message, at } => {
                let message = format!("the evaluation failed: {}", message.trim_end());
                vec![match at {
                    Some((file, line)) if program.files[*file] != DECLARATIONS => {
                        Diagnostic::error("program-fault", &program.files[*file], *line, message)
                    }
                    _ => at_declaration(message),
                }]
            }
            Fault::Limit(limit, limits) => {
                let message = match limit {
                    Limit::Time => format!(
                        "the evaluation was stopped at its time limit of {} s (`[rules] time_limit_seconds`)",
                        limits.time_seconds
                    ),
                    Limit::Memory => format!(
                        "the evaluation was stopped at its memory limit of {} MiB",
                        limits.memory_bytes >> 20
                    ),
                    Limit::Output => format!(
                        "the evaluation was stopped at its output-size limit of {} MiB per file",
                        limits.output_bytes >> 20
                    ),
                };
                vec![at_declaration(message)]
            }
            Fault::Output { relation, problem } => vec![at_declaration(format!(
                "the output of `{relation}` cannot be read: {problem}"
            ))],
            Fault::Host(problem) => vec![at_declaration(problem.clone())],
        }
    }
}

impl Message {
    /// The error diagnostic at the project path and line of its file, its message ending with
    /// the project file's line and a caret under the column; at line `declared_at` of the
    /// configuration when it is in the wrapper or in Mara's declarations. A warning of Soufflé
    /// is an error too: only an invariant gives a warning (DES-DIAGNOSTICS).
    pub fn diagnostic(&self, program: &Program, declared_at: usize) -> Diagnostic {
        let code = "program-error";
        let text = match self.severity {
            Severity::Error => self.text.clone(),
            Severity::Warning => format!("Soufflé warns: {}", self.text),
        };
        let place = self
            .file
            .filter(|&file| program.files[file] != DECLARATIONS)
            .and_then(|file| Some((file, program.line_start(file, self.line)?)));
        match place {
            Some((file, start)) => {
                let source = program.text(file);
                let line_end = source[start..]
                    .find('\n')
                    .map_or(source.len(), |end| start + end);
                let offset = start + original_offset(program, file, start..line_end, self.offset);
                let mut offset = offset.min(line_end);
                while !source.is_char_boundary(offset) {
                    offset -= 1;
                }
                let location = program.locate(file, offset);
                let message = format!("{text}\n{}", location.excerpt());
                Diagnostic::error(code, location.path, location.line, message)
            }
            None => Diagnostic::error(
                code,
                crate::config::CONFIG_PATH,
                declared_at,
                format!("{}: {text}", program.files[0]),
            ),
        }
    }
}

/// The offset in the line `line` of the file at `file` that `copied` points at in its private
/// copy: an `.include` path earlier on the line has another length in the copy, and an offset
/// inside one points at its start.
fn original_offset(
    program: &Program,
    file: usize,
    line: std::ops::Range<usize>,
    copied: usize,
) -> usize {
    let text = program.text(file);
    // How much longer the copy is up to here.
    let mut longer: isize = 0;
    let sites = program.includes.iter();
    for site in sites.filter(|site| site.file == file && line.contains(&site.at)) {
        let original = text[site.at + 1..].find('"').expect("quoted") + 2;
        let copy = copy_name(program, site.target).len() + 2;
        let at = (site.at - line.start) as isize;
        let copied_at = at + longer;
        if (copied as isize) < copied_at {
            break;
        }
        if (copied as isize) < copied_at + copy as isize {
            return at as usize;
        }
        longer += copy as isize - original as isize;
    }
    (copied as isize - longer).max(0) as usize
}

/// The name of the private copy of the file at `file` of `program`: `mara.dl` for Mara's
/// declarations, `p<file>.dl` otherwise. Soufflé names a file in a diagnostic only by its base
/// name, and these base names are distinct.
pub fn copy_name(program: &Program, file: usize) -> String {
    if program.files[file] == DECLARATIONS {
        DECLARATIONS.to_owned()
    } else {
        format!("p{file}.dl")
    }
}

/// The file index that the private copy `name` holds, or `None` for any other name.
fn copy_index(program: &Program, name: &str) -> Option<usize> {
    if name == DECLARATIONS {
        return program.files.iter().position(|file| file == DECLARATIONS);
    }
    let index: usize = name.strip_prefix('p')?.strip_suffix(".dl")?.parse().ok()?;
    (index < program.files.len() && copy_name(program, index) == name).then_some(index)
}

/// The private copies of the program's files, each with its name: the parsed text, with each
/// `.include` path replaced by the name of the copy that the parser resolved it to. Only those
/// paths change, so every other line keeps its text and number, and Soufflé reads exactly the
/// files the parser read.
pub fn copies(program: &Program) -> Vec<(String, String)> {
    (0..program.files.len())
        .map(|file| {
            let text = program.text(file);
            let mut copy = String::with_capacity(text.len());
            let mut from = 0;
            for include in program.includes.iter().filter(|site| site.file == file) {
                let end = include.at + 1 + text[include.at + 1..].find('"').expect("quoted") + 1;
                copy.push_str(&text[from..include.at]);
                copy.push('"');
                copy.push_str(&copy_name(program, include.target));
                copy.push('"');
                from = end;
            }
            copy.push_str(&text[from..]);
            (copy_name(program, file), copy)
        })
        .collect()
}

/// The file that receives the rows of output relation `relation`, relative to the private
/// directory.
pub fn output_file(relation: &str) -> String {
    format!("{OUTPUT_DIR}/{}", facts::file_name(relation))
}

/// The wrapper: it includes the program's first file and holds one `.input` directive for each
/// relation of `inputs` and one `.output` directive for each relation of `outputs`, all JSON
/// files in the private directory (`IO=jsonfile`, `format=list`).
pub fn wrapper(program: &Program, inputs: &[&str], outputs: &[&str]) -> String {
    let mut text = format!(".include \"{}\"\n", copy_name(program, 0));
    for relation in inputs {
        let file = facts::file_name(relation);
        text.push_str(&format!(
            ".input {relation}(IO=jsonfile, format=list, filename=\"{file}\")\n"
        ));
    }
    for relation in outputs {
        let file = output_file(relation);
        text.push_str(&format!(
            ".output {relation}(IO=jsonfile, format=list, filename=\"{file}\")\n"
        ));
    }
    text
}

/// The relations of `relations` that the program declares, which its wrapper reads.
pub fn read_relations<'r>(program: &Program, relations: &[&'r Relation]) -> Vec<&'r Relation> {
    relations
        .iter()
        .copied()
        .filter(|relation| program.relations.contains_key(&relation.name))
        .collect()
}

/// The key of an evaluation: the snapshot whose facts it reads (`None` for empty facts), the
/// program with every file it includes, the supplied inputs with their completeness, the
/// outputs and the limits.
pub fn key(
    snapshot: Option<SnapshotKey>,
    program: &Program,
    inputs: &[&Relation],
    outputs: &[&str],
    limits: Limits,
) -> Hash {
    let mut key = KeyHasher::new("mara evaluation", EVALUATION_VERSION);
    key.optional(snapshot.map(|key| key.0).as_ref());
    key.u32(program.files.len() as u32);
    for (file, path) in program.files.iter().enumerate() {
        key.bytes(path.as_bytes());
        key.bytes(program.text(file).as_bytes());
    }
    key.u32(inputs.len() as u32);
    for input in inputs {
        key.bytes(input.name.as_bytes());
        key.bytes(input.json.as_bytes());
    }
    key.u32(outputs.len() as u32);
    for output in outputs {
        key.bytes(output.as_bytes());
    }
    key.u32(limits.time_seconds);
    key.bytes(&limits.memory_bytes.to_le_bytes());
    key.bytes(&limits.output_bytes.to_le_bytes());
    key.finish()
}
