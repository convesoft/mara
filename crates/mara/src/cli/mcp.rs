//! `mara mcp`: the document operations as MCP tools over stdio (REQ-CLI-MCP-PARITY,
//! DES-OPERATIONS and DES-MCP-SERVER in `docs/hosts.mara.md`).
//!
//! `rmcp` speaks the protocol (ADR-RMCP-SERVER). A tool call builds its operation's [`Args`]
//! from the call's arguments and runs the operation's function, as the CLI does; the tool's text
//! is the JSON form that `--format json` prints. The server keeps nothing between calls, so each
//! call is a cold request. Only this command starts an async runtime.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use mara_engine::work::WorkCounts;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    JsonObject, ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::{RequestContext, ServerInitializeError};
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};
use serde_json::{Value, json};

use super::guide::{self, Surface};
use super::{
    Args, Context, FAILURE, Failure, OPERATIONS, Operation, Opt, PROJECT, REJECTED, Response, Run,
    SUCCESS,
};

pub const MCP: Operation = Operation {
    words: &["mcp"],
    summary: "Serves the document operations as MCP tools over stdio.",
    arguments: &[],
    options: &[],
    run: Run::Serve(serve),
};

/// The `project` parameter of every tool of a server started without `--project`.
const PROJECT_HELP: &str = "The project root, absolute or relative to the server's working \
                            directory. Without it, the project is found from the server's \
                            working directory upward.";

/// The server's `instructions`: the agent guide rendered for MCP, for the project of
/// `--project` or the one found from where the server starts (DES-AGENT-GUIDE). A selection
/// error other than finding no project sends none; each tool call reports it.
fn instructions(bound: Option<&Path>) -> Option<String> {
    let start = std::env::current_dir().ok()?;
    let surface = Surface::Mcp {
        bound: bound.is_some(),
    };
    guide::render(surface, &start, bound).ok()
}

/// A document operation's function.
type Document = fn(&mut Context, &Args) -> Result<Response, Failure>;

/// Serves MCP on stdin and stdout until the client closes stdin, on a single-threaded runtime
/// whose blocking pool runs the operations. Every tool call adds its work to `context.work`,
/// which `--debug-stats` prints when the server exits.
fn serve(context: &mut Context) -> u8 {
    let work = Arc::new(Mutex::new(WorkCounts::default()));
    let server = Server {
        bound: context.project.clone(),
        work: Arc::clone(&work),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let outcome = match runtime {
        Ok(runtime) => runtime.block_on(async move {
            match server.serve(rmcp::transport::stdio()).await {
                Ok(service) => service.waiting().await.map(drop).map_err(|e| e.to_string()),
                // The client left before it opened a session.
                Err(ServerInitializeError::ConnectionClosed(_)) => Ok(()),
                Err(error) => Err(error.to_string()),
            }
        }),
        Err(error) => Err(format!("cannot start the runtime: {error}")),
    };
    add(
        context.work,
        &work.lock().unwrap_or_else(PoisonError::into_inner),
    );
    match outcome {
        Ok(()) => SUCCESS,
        Err(message) => {
            eprintln!("mara: error[io-error]: the MCP session failed: {message}");
            FAILURE
        }
    }
}

struct Server {
    /// The project of `--project`, which every call runs against.
    bound: Option<PathBuf>,
    /// The work of every call so far.
    work: Arc<Mutex<WorkCounts>>,
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        let config = ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("mara", env!("CARGO_PKG_VERSION")));
        match instructions(self.bound.as_deref()) {
            Some(text) => config.with_instructions(text),
            None => config,
        }
    }

    /// One tool per document operation, all on one page.
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = OPERATIONS
            .iter()
            .filter(|operation| matches!(operation.run, Run::Document(_)))
            .map(|operation| tool(operation, self.bound.is_none()))
            .collect();
        Ok(ListToolsResult::with_all_items(tools))
    }

    /// Runs the tool's operation on the blocking pool. A failure, rejected arguments included,
    /// is a tool result with `isError`; an unknown tool is a protocol error.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let Some((operation, run)) = find(&request.name) else {
            let message = format!("unknown tool `{}`", request.name);
            return Err(ErrorData::invalid_params(message, None));
        };
        let given = request.arguments.unwrap_or_default();
        let bound = self.bound.clone();
        let work = Arc::clone(&self.work);
        let outcome = tokio::task::spawn_blocking(move || {
            let (args, project) = args(operation, &given, bound)?;
            let mut counts = WorkCounts::default();
            let mut context = Context {
                work: &mut counts,
                now: SystemTime::now(),
                project,
            };
            let outcome = run(&mut context, &args);
            add(
                &mut work.lock().unwrap_or_else(PoisonError::into_inner),
                &counts,
            );
            outcome
        })
        .await
        .map_err(|error| ErrorData::internal_error(format!("the tool stopped: {error}"), None))?;
        let result = match outcome {
            Ok(response) => CallToolResult::success(vec![ContentBlock::text(response.json)]),
            Err(failure) => CallToolResult::error(vec![ContentBlock::text(failure.json())]),
        };
        Ok(result.into())
    }
}

/// Adds the work of one call to `total`.
fn add(total: &mut WorkCounts, call: &WorkCounts) {
    total.file_reads += call.file_reads;
    total.hashes += call.hashes;
    total.parses += call.parses;
    total.snapshot_builds += call.snapshot_builds;
    total.code_scans += call.code_scans;
    total.subprocess_spawns += call.subprocess_spawns;
    total.check_steps += call.check_steps;
    total.program_runs += call.program_runs;
}

/// An operation's tool name: its words joined by `_`.
fn name(operation: &Operation) -> String {
    operation.words.join("_")
}

/// The document operation whose tool is named `name`.
fn find(name: &str) -> Option<(&'static Operation, Document)> {
    OPERATIONS.iter().find_map(|operation| match operation.run {
        Run::Document(run) if self::name(operation) == name => Some((operation, run)),
        _ => None,
    })
}

/// The tool of `operation`: its positional arguments and options as parameters, and `project`
/// when the server is not bound to one.
fn tool(operation: &Operation, project: bool) -> Tool {
    let mut properties = JsonObject::new();
    let mut required = Vec::new();
    for argument in operation.arguments {
        let schema = parameter("string", false, argument.help);
        properties.insert(argument.name.to_owned(), schema);
        required.push(argument.name);
    }
    for option in operation.options {
        let kind = if option.integer { "integer" } else { "string" };
        let schema = parameter(kind, option.repeatable, option.help);
        properties.insert(option.name.to_owned(), schema);
        if option.required {
            required.push(option.name);
        }
    }
    if project {
        let schema = parameter("string", false, PROJECT_HELP);
        properties.insert(PROJECT.name.to_owned(), schema);
    }
    let mut schema = JsonObject::new();
    schema.insert("type".to_owned(), json!("object"));
    schema.insert("properties".to_owned(), Value::Object(properties));
    if !required.is_empty() {
        schema.insert("required".to_owned(), json!(required));
    }
    schema.insert("additionalProperties".to_owned(), json!(false));
    Tool::new(name(operation), operation.summary, Arc::new(schema))
}

/// The schema of a parameter of type `kind`, or of an array of them.
fn parameter(kind: &str, repeatable: bool, help: &str) -> Value {
    match repeatable {
        true => json!({"type": "array", "items": {"type": kind}, "description": help}),
        false => json!({"type": kind, "description": help}),
    }
}

/// The operation's arguments and the project of the call, from the tool's arguments, checked
/// as the CLI checks its own. A null stands for a parameter left out, as clients that send
/// every parameter write it.
fn args(
    operation: &Operation,
    given: &JsonObject,
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
            let values = match (option.repeatable, value) {
                (true, Value::Array(values)) => values.iter().collect(),
                (true, _) => {
                    let kind = if option.integer {
                        "integers"
                    } else {
                        "strings"
                    };
                    return Err(rejected(format!("`{name}` is an array of {kind}")));
                }
                (false, value) => vec![value],
            };
            for value in values {
                let value = scalar(option, value)?;
                (option.check)(&value).map_err(|reason| rejected(format!("`{name}`: {reason}")))?;
                args.options.push((option.name, value));
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
    match (option.integer, value) {
        (true, Value::Number(number)) if number.is_i64() || number.is_u64() => {
            Ok(number.to_string())
        }
        (true, _) => Err(rejected(format!("`{}` is a whole number", option.name))),
        (false, value) => string(option.name, value),
    }
}

/// Rejected arguments: nothing ran.
fn rejected(message: String) -> Failure {
    Failure {
        status: REJECTED,
        ..Failure::new("invalid-argument", message)
    }
}
