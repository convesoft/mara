//! A document operation called as an MCP tool is (DES-MCP-SERVER in `docs/hosts/index.mara.md`):
//! by its tool name, `<group>_<verb>`, with its arguments as one JSON object. `mara mcp` and the
//! browser workspace (`workspace.rs`, in the WebAssembly build) call operations so, and check the
//! arguments as the CLI checks its own.

use std::path::PathBuf;

use mara_engine::facts::Supplied;
use serde_json::{Map, Value};

use super::{
    Args, Context, Failure, Kind, OPERATIONS, Operation, Opt, PROJECT, REJECTED, Response, Run,
};

/// A document operation's function.
pub type Document = fn(&mut Context, &Args) -> Result<Response, Failure>;

/// An operation's tool name: its words joined by `_`.
pub fn name(operation: &Operation) -> String {
    operation.words.join("_")
}

/// The document operation whose tool is named `name`.
pub fn find(name: &str) -> Option<(&'static Operation, Document)> {
    OPERATIONS.iter().find_map(|operation| match operation.run {
        Run::Document(run) if self::name(operation) == name => Some((operation, run)),
        _ => None,
    })
}

// @mara implements REQ-CLI-MCP-PARITY
/// The operation's arguments and the project of the call, from the tool's arguments, checked
/// as the CLI checks its own. A null stands for a parameter left out, as clients that send
/// every parameter write it.
pub fn args(
    operation: &Operation,
    given: &Map<String, Value>,
    bound: Option<PathBuf>,
) -> Result<(Args, Option<PathBuf>), Failure> {
    let tool = name(operation);
    let mut args = Args::default();
    let mut arguments: Vec<Option<String>> = vec![None; operation.arguments.len()];
    let per_call = bound.is_none();
    let mut project = bound;
    for (name, value) in given.iter().filter(|(_, value)| !value.is_null()) {
        let name = name.as_str();
        if let Some(index) = operation.arguments.iter().position(|a| a.name == name) {
            arguments[index] = Some(string(name, value)?);
        } else if let Some(option) = operation.options.iter().find(|o| o.name == name) {
            if option.kind == Kind::Inputs {
                args.supplied = inputs(name, value)?;
                continue;
            }
            let values = match (option.repeatable, value) {
                (true, Value::Array(values)) => values.iter().collect(),
                (true, _) => {
                    let kind = match option.kind {
                        Kind::Integer => "integers",
                        _ => "strings",
                    };
                    return Err(rejected(format!("`{name}` is an array of {kind}")));
                }
                (false, value) => vec![value],
            };
            for value in values {
                let value = scalar(option, value)?;
                (args.push(option, &value))
                    .map_err(|reason| rejected(format!("`{name}`: {reason}")))?;
            }
        } else if name == PROJECT.name && per_call {
            project = Some(PathBuf::from(string(name, value)?));
        } else if name == PROJECT.name {
            return Err(rejected(
                "`project` is no parameter of a server started with `--project`, which serves \
                 that project only"
                    .to_owned(),
            ));
        } else {
            return Err(rejected(format!("unknown parameter `{name}` of `{tool}`")));
        }
    }
    for (value, argument) in arguments.into_iter().zip(operation.arguments) {
        let value = value.ok_or_else(|| rejected(format!("`{tool}` needs `{}`", argument.name)))?;
        args.arguments.push(value);
    }
    Ok((args, project))
}

/// A string parameter's value.
fn string(name: &str, value: &Value) -> Result<String, Failure> {
    match value {
        Value::String(text) => Ok(text.clone()),
        _ => Err(rejected(format!("`{name}` is a string"))),
    }
}

/// One value of `option`, as the CLI would have read it.
fn scalar(option: &Opt, value: &Value) -> Result<String, Failure> {
    match (option.kind, value) {
        (Kind::Integer, Value::Number(number)) if number.is_i64() || number.is_u64() => {
            Ok(number.to_string())
        }
        (Kind::Integer, _) => Err(rejected(format!("`{}` is a whole number", option.name))),
        (_, value) => string(option.name, value),
    }
}

/// The inputs that the parameter `name` passes: an object from each input's name to one value
/// as a string, or to a list of rows (DES-REQUEST-INPUTS). A string is always one value: MCP
/// passes rows, not files. The rows are checked with the request's other inputs.
fn inputs(name: &str, value: &Value) -> Result<Vec<(String, Supplied)>, Failure> {
    let refuse = || {
        rejected(format!(
            "`{name}` is an object from each input's name to a string or to a list of rows"
        ))
    };
    let Value::Object(map) = value else {
        return Err(refuse());
    };
    let mut supplied = Vec::with_capacity(map.len());
    for (input, value) in map {
        let value = match value {
            Value::String(text) => Supplied::Inline(text.clone()),
            Value::Array(_) => Supplied::Rows(value.clone()),
            _ => return Err(refuse()),
        };
        supplied.push((input.clone(), value));
    }
    Ok(supplied)
}

/// Rejected arguments: nothing ran.
pub(super) fn rejected(message: String) -> Failure {
    Failure {
        status: REJECTED,
        ..Failure::new("invalid-argument", message)
    }
}
