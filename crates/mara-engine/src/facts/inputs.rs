//! The inputs a request supplies, checked before any run (DES-REQUEST-INPUTS in
//! `docs/rules/index.mara.md`).

use std::fmt;

use super::{Relation, SOURCE_COMPLETE, Value};

/// The kind of a column, its declared type resolved to `symbol` or `number`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Symbol,
    Number,
}

/// An input that a consumer lists: a relation of its program, with its columns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputDecl {
    pub name: String,
    pub columns: Vec<Column>,
}

/// What a request supplies for one input.
#[derive(Clone, Debug, PartialEq)]
pub enum Supplied {
    /// One value, given inline (`--input candidate=abc123`): one row of one column.
    Inline(String),
    /// A JSON list of rows, each a list of values in column order: read from a file
    /// (`--input result=@results.json`) or passed by MCP.
    Rows(serde_json::Value),
}

/// The supplied inputs, checked: each as supplied, in the consumer's order, and the rows of
/// `source_complete`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inputs {
    pub relations: Vec<Relation>,
    pub source_complete: Relation,
}

/// Why a request's inputs refuse it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputError {
    /// A listed input that the request does not supply.
    Missing(String),
    /// An input, or a `--complete` name, that the consumer does not list.
    Unlisted(String),
    /// An input supplied twice.
    Repeated(String),
    /// A supplied value that does not fit the input: the message names the input, and the row
    /// and column where they apply.
    Invalid(String),
}

impl InputError {
    /// The stable code of the refusal (DES-OPERATIONS).
    pub fn code(&self) -> &'static str {
        match self {
            InputError::Missing(_) => "missing-input",
            InputError::Unlisted(_) => "unlisted-input",
            InputError::Repeated(_) => "repeated-input",
            InputError::Invalid(_) => "invalid-input",
        }
    }
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputError::Missing(name) => {
                write!(f, "the request does not supply the input `{name}`")
            }
            InputError::Unlisted(name) => write!(f, "`{name}` is not an input of this entry"),
            InputError::Repeated(name) => write!(f, "the input `{name}` is supplied twice"),
            InputError::Invalid(message) => f.write_str(message),
        }
    }
}

/// Checks the `supplied` inputs, each with its name, against the inputs a consumer lists:
/// every listed input supplied once and nothing else, and every row's column count and values
/// ([`check_rows`]). `complete` names the inputs that hold every record of their source; each
/// becomes a row of `source_complete`. Nothing is stored.
pub fn check_inputs(
    listed: &[InputDecl],
    supplied: Vec<(String, Supplied)>,
    complete: &[String],
) -> Result<Inputs, InputError> {
    let declaration = |name: &str| listed.iter().position(|input| input.name == name);
    let mut by_input: Vec<Option<Supplied>> = vec![None; listed.len()];
    for (name, value) in supplied {
        let index = declaration(&name).ok_or_else(|| InputError::Unlisted(name.clone()))?;
        if by_input[index].replace(value).is_some() {
            return Err(InputError::Repeated(name));
        }
    }
    let mut complete_rows = Vec::new();
    for name in complete {
        declaration(name).ok_or_else(|| InputError::Unlisted(name.clone()))?;
        let row = vec![Value::Symbol(name.clone())];
        if !complete_rows.contains(&row) {
            complete_rows.push(row);
        }
    }
    let mut relations = Vec::with_capacity(listed.len());
    for (input, value) in listed.iter().zip(by_input) {
        let value = value.ok_or_else(|| InputError::Missing(input.name.clone()))?;
        relations.push(Relation::new(&input.name, check_rows(input, value)?));
    }
    Ok(Inputs {
        relations,
        source_complete: Relation::new(SOURCE_COMPLETE, complete_rows),
    })
}

/// The rows of `value` for `input`, or the first that does not fit: a row with another number
/// of columns, a value other than a string in a symbol column, or a value other than an
/// integer from −2,147,483,648 to 2,147,483,647 in a number column. An inline value is one row
/// of one column; in a number column it is written as a decimal integer. Rows and columns
/// count from 1.
fn check_rows(input: &InputDecl, value: Supplied) -> Result<Vec<Vec<Value>>, InputError> {
    let name = &input.name;
    let invalid = |at: String, problem: String| {
        InputError::Invalid(format!("input `{name}`, {at}: {problem}"))
    };
    let width = |row: usize, count: usize| {
        let columns = input.columns.len();
        invalid(
            format!("row {row}"),
            format!(
                "{count} column{} where `{name}` has {columns}",
                plural(count)
            ),
        )
    };
    let rows = match value {
        Supplied::Inline(text) => {
            if input.columns.len() != 1 {
                return Err(width(1, 1));
            }
            let value =
                match input.columns[0] {
                    Column::Symbol => Value::Symbol(text),
                    Column::Number => {
                        let number = text.parse::<i32>().ok().filter(|_| !text.starts_with('+'));
                        Value::Number(number.ok_or_else(|| {
                            invalid("row 1, column 1".into(), not_a_number(&text))
                        })?)
                    }
                };
            return Ok(vec![vec![value]]);
        }
        Supplied::Rows(serde_json::Value::Array(rows)) => rows,
        Supplied::Rows(other) => {
            return Err(InputError::Invalid(format!(
                "input `{name}` is not a JSON list of rows: it is {}",
                kind(&other)
            )));
        }
    };
    let mut checked = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let at = index + 1;
        let serde_json::Value::Array(values) = row else {
            let problem = format!("{} where a row is a list of values", kind(&row));
            return Err(invalid(format!("row {at}"), problem));
        };
        if values.len() != input.columns.len() {
            return Err(width(at, values.len()));
        }
        let mut out = Vec::with_capacity(values.len());
        for (column, (value, expected)) in values.into_iter().zip(&input.columns).enumerate() {
            let place = || format!("row {at}, column {}", column + 1);
            out.push(match (expected, value) {
                (Column::Symbol, serde_json::Value::String(text)) => Value::Symbol(text),
                (Column::Symbol, other) => {
                    let problem = format!("{} where the column takes a string", kind(&other));
                    return Err(invalid(place(), problem));
                }
                (Column::Number, serde_json::Value::Number(number)) => {
                    match number.as_i64().and_then(|n| i32::try_from(n).ok()) {
                        Some(number) => Value::Number(number),
                        None => return Err(invalid(place(), not_a_number(&number.to_string()))),
                    }
                }
                (Column::Number, other) => {
                    let problem = format!("{} where the column takes a number", kind(&other));
                    return Err(invalid(place(), problem));
                }
            });
        }
        checked.push(out);
    }
    Ok(checked)
}

fn not_a_number(text: &str) -> String {
    format!("`{text}` is not an integer from -2147483648 to 2147483647")
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// What a JSON value is, for a message.
fn kind(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(value) => format!("`{value}`"),
        serde_json::Value::Number(number) => format!("the number `{number}`"),
        serde_json::Value::String(_) => "a string".into(),
        serde_json::Value::Array(_) => "a list".into(),
        serde_json::Value::Object(_) => "an object".into(),
    }
}
