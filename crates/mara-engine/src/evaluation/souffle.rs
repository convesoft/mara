//! Soufflé's exit, what it printed and the files it wrote, read into rows or a fault.

use super::{Fault, Limit, Limits, Message, QUOTED_BYTES, copy_index};
use crate::diagnostic::Severity;
use crate::facts::Value;
use crate::program::{ColumnKind, Program};

/// How the Soufflé process ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exit {
    /// It exited with status 0.
    pub success: bool,
    /// The host stopped it, or the system ended it, at a limit.
    pub limit: Option<Limit>,
}

// @mara implements REQ-PROGRAM-FAULTS-REPORTED
/// Whether a run of `program` evaluated it, from how Soufflé ended and what it printed, or the
/// fault that makes the outcome unavailable: a compile diagnostic, a warning included, a stop
/// at a limit, a failed exit, and anything else Soufflé prints on either stream, also with
/// status 0 (DES-RULE-EXECUTION).
pub fn interpret(
    program: &Program,
    exit: Exit,
    stdout: &[u8],
    stderr: &[u8],
    limits: Limits,
) -> Result<(), Fault> {
    if let Some(limit) = exit.limit {
        return Err(Fault::Limit(limit, limits));
    }
    let stderr = String::from_utf8_lossy(stderr);
    if !exit.success
        && ["bad_alloc", "Cannot allocate memory", "out of memory"]
            .iter()
            .any(|sign| stderr.contains(sign))
    {
        return Err(Fault::Limit(Limit::Memory, limits));
    }
    let (messages, other) = diagnostics(program, &stderr);
    if !messages.is_empty() {
        return Err(Fault::Compile(messages));
    }
    let other = other.trim();
    if !exit.success || !other.is_empty() {
        let message = match other {
            "" => "Soufflé failed without a message".to_owned(),
            other => quote(&rename(program, other)),
        };
        let at = runtime_location(program, other);
        return Err(Fault::Runtime { message, at });
    }
    if !stdout.is_empty() {
        let printed = String::from_utf8_lossy(stdout);
        let message = format!("Soufflé printed {}", quote(printed.trim()));
        return Err(Fault::Runtime { message, at: None });
    }
    Ok(())
}

/// The compile diagnostics in Soufflé's standard error, and every other line. A diagnostic
/// starts with `Error: ` or `Warning: `, may go on over further lines, ends with `in file
/// <name> at line <n>`, and is followed by the source line and a line with a caret. Right
/// after one, a block of that form without the prefix, such as `Following constraints are
/// incompatible:`, is another diagnostic of the same severity.
fn diagnostics(program: &Program, stderr: &str) -> (Vec<Message>, String) {
    let lines: Vec<&str> = stderr.lines().collect();
    let starts = |line: &str| line.starts_with("Error: ") || line.starts_with("Warning: ");
    let caret = |line: &str| {
        line.contains('^') && line.chars().all(|c| matches!(c, '-' | '^' | ' ' | '\t'))
    };
    let mut messages = Vec::new();
    let mut other = String::new();
    let mut previous = None;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let severity = if line.starts_with("Error: ") {
            Some(Severity::Error)
        } else if line.starts_with("Warning: ") {
            Some(Severity::Warning)
        } else {
            previous
        };
        let end = (index..lines.len())
            .take_while(|&end| end == index || !starts(lines[end]))
            .find(|&end| location(lines[end]).is_some())
            .filter(|&end| end + 1 < lines.len());
        if let (Some(severity), Some(end)) = (severity, end) {
            let (prefix, name, number) = location(lines[end]).expect("found");
            let mut text: Vec<&str> = lines[index..end].to_vec();
            text.push(prefix);
            let text = text.join("\n");
            let text = text
                .strip_prefix("Error: ")
                .or_else(|| text.strip_prefix("Warning: "))
                .unwrap_or(&text)
                .to_owned();
            let pointer = lines.get(end + 2).copied().filter(|line| caret(line));
            messages.push(Message {
                severity,
                text,
                file: copy_index(program, name),
                line: number,
                offset: pointer.and_then(|line| line.find('^')).unwrap_or(0),
            });
            index = end + 2 + usize::from(pointer.is_some());
            previous = Some(severity);
            continue;
        }
        previous = None;
        if !is_summary(line) {
            other.push_str(line);
            other.push('\n');
        }
        index += 1;
    }
    (messages, other)
}

/// `<text> in file <name> at line <n>`, split into its parts.
fn location(line: &str) -> Option<(&str, &str, usize)> {
    let (text, place) = line.rsplit_once(" in file ")?;
    let (name, number) = place.rsplit_once(" at line ")?;
    let number = number.parse().ok()?;
    (!name.contains(' ')).then_some((text, name, number))
}

/// `<n> errors generated, evaluation aborted`: Soufflé's count of its compile errors.
fn is_summary(line: &str) -> bool {
    line.split_once(' ').is_some_and(|(count, rest)| {
        count.bytes().all(|byte| byte.is_ascii_digit())
            && rest == "errors generated, evaluation aborted"
    })
}

/// The file and line of a runtime fault: `in file <name> [<line>:<column>-...]`.
fn runtime_location(program: &Program, text: &str) -> Option<(usize, usize)> {
    text.match_indices("in file ").find_map(|(at, marker)| {
        let rest = &text[at + marker.len()..];
        let (name, rest) = rest.split_once(" [")?;
        let (line, _) = rest.split_once(':')?;
        Some((copy_index(program, name)?, line.parse().ok()?))
    })
}

/// `text` with each private copy's name replaced by its project path.
fn rename(program: &Program, text: &str) -> String {
    let mut text = text.to_owned();
    for file in (0..program.files.len()).rev() {
        let copy = format!("in file {}", super::copy_name(program, file));
        text = text.replace(&copy, &format!("in file {}", program.files[file]));
    }
    text
}

/// At most [`QUOTED_BYTES`] of `text`.
fn quote(text: &str) -> String {
    if text.len() <= QUOTED_BYTES {
        return text.to_owned();
    }
    let mut end = QUOTED_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// The rows that Soufflé wrote for `relation` of `program`: a JSON list of rows, each a list of
/// a string for every symbol column and an integer for every number column. Text that is not
/// UTF-8, such as a value that `substr` split inside a character, or rows that do not fit the
/// relation, are a fault.
pub fn read_output(
    program: &Program,
    relation: &str,
    bytes: &[u8],
    limits: Limits,
) -> Result<Vec<Vec<Value>>, Fault> {
    if bytes.len() as u64 > limits.output_bytes {
        return Err(Fault::Limit(Limit::Output, limits));
    }
    let problem = |problem: String| Fault::Output {
        relation: relation.to_owned(),
        problem,
    };
    let text = std::str::from_utf8(bytes).map_err(|_| {
        problem("it is not valid UTF-8, as when `substr` splits a character".to_owned())
    })?;
    let json: serde_json::Value =
        serde_json::from_str(text).map_err(|error| problem(format!("it is not JSON: {error}")))?;
    let columns = &program
        .relations
        .get(relation)
        .ok_or_else(|| problem("the program declares no such relation".to_owned()))?
        .columns;
    let serde_json::Value::Array(rows) = json else {
        return Err(problem("it is not a list of rows".to_owned()));
    };
    let mut read = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let wrong = || problem(format!("row {} does not fit the relation", index + 1));
        // Soufflé writes a row of a relation without columns as `null`.
        let values = match row {
            serde_json::Value::Array(values) => values,
            serde_json::Value::Null if columns.is_empty() => Vec::new(),
            _ => return Err(wrong()),
        };
        if values.len() != columns.len() {
            return Err(wrong());
        }
        let mut out = Vec::with_capacity(values.len());
        for (value, column) in values.into_iter().zip(columns) {
            out.push(match (column.kind, value) {
                (ColumnKind::Symbol, serde_json::Value::String(text)) => Value::Symbol(text),
                (ColumnKind::Number, serde_json::Value::Number(number)) => {
                    let number = number.as_i64().and_then(|n| i32::try_from(n).ok());
                    Value::Number(number.ok_or_else(wrong)?)
                }
                _ => return Err(wrong()),
            });
        }
        read.push(out);
    }
    Ok(read)
}
