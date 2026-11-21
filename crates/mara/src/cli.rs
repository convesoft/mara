//! The command line: arguments, operations, output and exit status (REQ-CLI-MCP-PARITY and
//! DES-OPERATIONS in `docs/hosts/index.mara.md`).
//!
//! An operation is an [`Operation`] in [`OPERATIONS`]: its words, positional arguments and
//! options, and a function from a [`Context`] and its [`Args`] to a [`Response`] or a
//! [`Failure`]. Arguments are checked before the function runs. `mara mcp` serves each document
//! operation as an MCP tool that runs the same function ([`mcp`]). Both are clients of the daemon:
//! they send a document operation to the daemon of their build, or run it in-process
//! ([`client`]). Each operation also adds its case to the performance harness
//! (`crates/mara/tests/performance/`).

mod author;
mod client;
mod code;
mod convert;
mod daemon;
mod fix;
mod guide;
mod init;
mod mcp;
mod query;
mod read;
mod relink;
mod validate;

use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use mara_engine::diagnostic::Diagnostic;
use mara_engine::document::is_mid;
use mara_engine::facts::Supplied;
use mara_engine::json::Object;
use mara_engine::paging::{self, Cursor};
use mara_engine::snapshot::Snapshot;
use mara_engine::validation;
use mara_engine::work::WorkCounts;

use mara::caller::Caller;
use mara::inputs::Inputs;
use mara::project::{self, SelectError};
use mara::session::Session;
use mara::write::WriteError;

/// Every operation, in the order the usage lists them.
const OPERATIONS: &[Operation] = &[
    validate::PROJECT,
    init::PROJECT,
    fix::PROJECT,
    convert::PROJECT_CONVERT,
    validate::ITEM,
    validate::SCHEMA,
    read::SEARCH,
    read::GET,
    read::RELATION_LIST,
    read::ITEM_LIST,
    read::SCHEMA_LIST,
    read::SCHEMA_GET,
    relink::ITEM_MOVE,
    relink::ITEM_RENAME,
    author::ITEM_CREATE,
    author::ITEM_UPDATE,
    author::ITEM_DELETE,
    author::RELATION_ADD,
    author::RELATION_REMOVE,
    query::LIST,
    query::EXECUTE,
    query::VIEW,
    query::GATE,
    query::REPORT,
    code::ADD,
    daemon::STATUS,
    daemon::START,
    daemon::STOP,
    mcp::MCP,
    guide::PRIME,
];

/// The exit status of a valid result, or of a successful operation.
pub const SUCCESS: u8 = 0;
/// The exit status of an invalid result, or of a request that failed.
pub const FAILURE: u8 = 1;
/// The exit status of rejected arguments: nothing ran.
pub const REJECTED: u8 = 2;

/// Version of the JSON form of results and failures.
pub const FORMAT_VERSION: u64 = 1;

/// A CLI operation. A document operation is also an MCP tool of the same inputs and results.
pub struct Operation {
    /// The words that name it, such as `["project", "validate"]`.
    pub words: &'static [&'static str],
    /// What it does, in one sentence: its MCP tool's description.
    pub summary: &'static str,
    /// Its positional arguments. Each is required.
    pub arguments: &'static [Argument],
    pub options: &'static [Opt],
    pub run: Run,
}

/// What an operation runs.
pub enum Run {
    /// Answers one request with a result or a failure: a document operation, which `mara mcp`
    /// also serves as a tool.
    Document(fn(&mut Context, &Args) -> Result<Response, Failure>),
    /// Answers one request as a document operation does, but is no MCP tool: `prime`.
    Command(fn(&mut Context, &Args) -> Result<Response, Failure>),
    /// Serves requests until its input ends, and returns the exit status.
    Serve(fn(&mut Context) -> u8),
}

/// A positional argument.
pub struct Argument {
    pub name: &'static str,
    /// What it names: its MCP parameter's description.
    pub help: &'static str,
}

/// An option with a value: `--<name> <value>` or `--<name>=<value>`.
pub struct Opt {
    pub name: &'static str,
    /// What the value is, as the usage names it.
    pub value: &'static str,
    /// What it does: its MCP parameter's description.
    pub help: &'static str,
    pub repeatable: bool,
    /// Whether the operation needs it. The operation itself refuses a request without it; MCP
    /// lists it as a required parameter.
    pub required: bool,
    /// What its value is, which sets its MCP parameter's type.
    pub kind: Kind,
    /// Checks a value before anything runs, with the reason it is rejected.
    pub check: fn(&str) -> Result<(), String>,
}

/// What an option's value is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Text: a string in MCP.
    Text,
    /// A whole number: a JSON integer in MCP.
    Integer,
    /// `<name>=<value>` or `<name>=@<file>`, an input a request supplies: in MCP one object from
    /// each input's name to one value as a string or to a list of rows (DES-REQUEST-INPUTS).
    Inputs,
}

/// `--project <dir>`: the project root, relative to the working directory (DES-PROJECT-CONFIGURATION).
const PROJECT: Opt = Opt {
    name: "project",
    value: "dir",
    help: "The project root, absolute or relative to the working directory. Without it, the \
           project is found from the working directory upward.",
    repeatable: false,
    required: false,
    kind: Kind::Text,
    check: |_| Ok(()),
};

/// `--format human|json`: how results and failures are printed.
const FORMAT: Opt = Opt {
    name: "format",
    value: "human|json",
    help: "How results and failures are printed.",
    repeatable: false,
    required: false,
    kind: Kind::Text,
    check: |value| match value {
        "human" | "json" => Ok(()),
        _ => Err(format!("the format is `human` or `json`, not `{value}`")),
    },
};

/// The flag that prints the work counts (DES-PERFORMANCE-HARNESS).
const DEBUG_STATS: &str = "debug-stats";

/// The direct-mode flag: the request runs in-process, and no daemon is reached or started
/// (DES-DAEMON-LIFECYCLE).
const NO_DAEMON: &str = "no-daemon";

/// `--limit <n>`: records per page (REQ-PAGE-LIMITS).
pub const LIMIT: Opt = Opt {
    name: "limit",
    value: "n",
    help: "Records per page, 1 to 100.",
    repeatable: false,
    required: false,
    kind: Kind::Integer,
    check: |value| paging::parse_limit(value).map(drop),
};

/// `--cursor <cursor>`: where the next page starts (DES-PAGING).
pub const CURSOR: Opt = Opt {
    name: "cursor",
    value: "cursor",
    help: "Where the page starts: the `next_cursor` of the previous page.",
    repeatable: false,
    required: false,
    kind: Kind::Text,
    check: |value| match Cursor::parse(value) {
        Some(_) => Ok(()),
        None => Err(format!(
            "`{value}` is not a cursor: pass the `next_cursor` of the previous page"
        )),
    },
};

/// A path relative to the project root as an option names it, in the form diagnostics carry:
/// empty and `.` parts and a trailing `/` dropped, as 0.3.0 read it. An absolute path, a `..`
/// part, and a path that names the root itself are refused.
pub fn project_path(text: &str) -> Result<String, String> {
    let refuse = |why: &str| {
        Err(format!(
            "`{text}` is {why}: name a file or directory relative to the project root"
        ))
    };
    if text.starts_with('/') {
        return refuse("absolute");
    }
    let mut parts = Vec::new();
    for part in text.split('/') {
        match part {
            "" | "." => {}
            ".." => return refuse("not below the project root"),
            part => parts.push(part),
        }
    }
    if parts.is_empty() {
        return refuse("the project root itself; leave the option out for the whole project");
    }
    Ok(parts.join("/"))
}

/// The snapshot index of the one item whose ID or MID is `reference`: `unknown-item` when no
/// item has it, `ambiguous-item`, with their places, when several have it.
pub fn resolve_item(snapshot: &Snapshot, reference: &str) -> Result<usize, Failure> {
    let mut items = match is_mid(reference) {
        true => snapshot.items_with_mid(reference),
        false => snapshot.items_with_id(reference),
    }
    .to_vec();
    items.dedup();
    match items[..] {
        [item] => Ok(item),
        [] => {
            let message = format!("no item has the ID or MID `{reference}`");
            Err(Failure::new("unknown-item", message))
        }
        ref several => {
            let places = several.iter().map(|&item| {
                let path = &snapshot.documents()[snapshot.items()[item].document].path;
                format!("`{path}:{}`", snapshot.item(item).line)
            });
            let other = if is_mid(reference) { "ID" } else { "MID" };
            let message = format!(
                "{} items have `{reference}`, at {}: name one by its {other}",
                several.len(),
                validation::list(places, several.len())
            );
            Err(Failure::new("ambiguous-item", message))
        }
    }
}

/// The values of an operation's options and its positional arguments, as checked.
#[derive(Debug, Default)]
pub struct Args {
    options: Vec<(&'static str, String)>,
    arguments: Vec<String>,
    /// The inputs an MCP call passes as values ([`Kind::Inputs`]); the CLI gives them as
    /// option values.
    supplied: Vec<(String, Supplied)>,
    /// Whether the CLI read the direct-mode flag (DES-DAEMON-LIFECYCLE).
    no_daemon: bool,
}

impl Args {
    /// Adds `value` of `option`, which the CLI, the daemon and MCP all check this way
    /// (DES-OPERATIONS): the option's own check, then a value of an option that does not
    /// repeat given before. The reason it is rejected names no option: each caller names it as
    /// its request does.
    pub fn push(&mut self, option: &Opt, value: &str) -> Result<(), String> {
        (option.check)(value)?;
        if !option.repeatable && self.value(option.name).is_some() {
            return Err("given twice, but it takes one value".to_owned());
        }
        self.options.push((option.name, value.to_owned()));
        Ok(())
    }

    /// The value of the option `name`, if given.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.values(name).next()
    }

    /// Every value of the option `name`, in the order given.
    pub fn values<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a str> + use<'a> {
        let name = name.to_owned();
        self.options
            .iter()
            .filter(move |(option, _)| *option == name)
            .map(|(_, value)| value.as_str())
    }

    /// The positional argument at `index`, which [`Operation::arguments`] declares.
    pub fn argument(&self, index: usize) -> &str {
        &self.arguments[index]
    }

    /// `--limit`, or the default number of records per page.
    pub fn limit(&self) -> usize {
        self.value(LIMIT.name)
            .map_or(paging::DEFAULT_LIMIT, |value| {
                paging::parse_limit(value).expect("checked before the operation ran")
            })
    }

    /// The inputs that an MCP call passes as values.
    pub fn supplied(&self) -> &[(String, Supplied)] {
        &self.supplied
    }

    /// `--cursor`, if given.
    pub fn cursor(&self) -> Option<Cursor> {
        self.value(CURSOR.name)
            .map(|value| Cursor::parse(value).expect("checked before the operation ran"))
    }
}

/// What every operation of one request shares.
pub struct Context<'w> {
    pub work: &'w mut WorkCounts,
    /// When the request began.
    pub now: SystemTime,
    project: Option<PathBuf>,
    /// The client's working directory, from which `--project` and every file an argument names
    /// are resolved, or why it cannot be read. In the daemon it is the client's, never the
    /// daemon's own.
    cwd: Result<PathBuf, String>,
    /// Whether the client runs in direct mode ([`client::direct_mode`]).
    direct: bool,
    /// The client's umask and temporary directory, which a request to the daemon carries.
    caller: Caller,
    /// The session of the request's project, when the daemon serves it (DES-DAEMON-SESSIONS).
    session: Option<Arc<Session>>,
}

impl Context<'_> {
    /// The root of the project the request runs against (REQ-ONE-PROJECT-PER-OPERATION).
    pub fn root(&self) -> Result<PathBuf, Failure> {
        project::select(self.cwd()?, self.project.as_deref()).map_err(Failure::from)
    }

    /// The inputs of the project at `root`, the canonical root that [`Context::root`] returns:
    /// a view of its session's when the daemon serves the request in that session, otherwise
    /// new ones, which read everything once.
    pub fn inputs(&self, root: PathBuf) -> Inputs {
        match &self.session {
            Some(session) if session.root() == root => session.inputs(),
            _ => Inputs::new(root),
        }
    }

    /// The client's working directory.
    pub fn cwd(&self) -> Result<&std::path::Path, Failure> {
        self.cwd.as_deref().map_err(|error| {
            Failure::new(
                "io-error",
                format!("cannot read the working directory: {error}"),
            )
        })
    }
}

/// The working directory of this process, or why it cannot be read.
fn working_directory() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|error| error.to_string())
}

/// The result of an operation.
pub struct Response {
    /// The JSON form, which MCP returns.
    pub json: String,
    /// The human form, printed on stdout.
    pub human: String,
    pub status: u8,
}

/// What a request prints: its result or its failure in the form asked for, and the exit status.
#[derive(Debug, PartialEq, Eq)]
pub struct Reply {
    /// The JSON object without a line end, or the human form.
    pub text: String,
    /// Whether the request failed: the human form of a failure goes to stderr, and MCP marks
    /// the result as an error.
    pub failed: bool,
    pub status: u8,
}

impl Reply {
    pub fn new(outcome: Result<Response, Failure>, json: bool) -> Reply {
        match outcome {
            Ok(response) => Reply {
                text: if json { response.json } else { response.human },
                failed: false,
                status: response.status,
            },
            Err(failure) => Reply {
                text: if json {
                    failure.json()
                } else {
                    failure.human()
                },
                failed: true,
                status: failure.status,
            },
        }
    }
}

/// Why a request returns no result.
#[derive(Debug)]
pub struct Failure {
    pub code: &'static str,
    pub message: String,
    /// The problems in project files that caused it.
    pub diagnostics: Vec<Diagnostic>,
    pub status: u8,
}

impl Failure {
    /// A failed request: exit status [`FAILURE`].
    pub fn new(code: &'static str, message: String) -> Failure {
        Failure {
            code,
            message,
            diagnostics: Vec::new(),
            status: FAILURE,
        }
    }

    /// Rejected arguments: exit status [`REJECTED`].
    fn rejected(message: String) -> Failure {
        Failure {
            status: REJECTED,
            ..Failure::new(
                "invalid-argument",
                format!("{message}; run `mara` for the usage"),
            )
        }
    }

    /// `{"format_version":1,"error":{"code","message","diagnostics"}}`.
    fn json(&self) -> String {
        let records: Vec<String> = self
            .diagnostics
            .iter()
            .map(|diagnostic| {
                let mut record = String::new();
                diagnostic.write_json(&mut record);
                record
            })
            .collect();
        let mut out = String::new();
        let mut object = Object::new(&mut out);
        object.number("format_version", FORMAT_VERSION);
        let mut error = Object::new(object.key("error"));
        error.string("code", self.code);
        error.string("message", &self.message);
        error.array("diagnostics", records.iter().map(String::as_str));
        error.end();
        object.end();
        out
    }

    /// `mara: error[<code>]: <message>`, then each diagnostic.
    fn human(&self) -> String {
        let mut out = format!("mara: error[{}]: {}\n", self.code, self.message);
        for diagnostic in &self.diagnostics {
            writeln!(out, "{diagnostic}").expect("writing to a String cannot fail");
        }
        out
    }
}

impl From<WriteError> for Failure {
    /// A refused or stopped edit, with a diagnostic at the file concerned
    /// (DES-WRITE-PROCEDURE).
    fn from(error: WriteError) -> Failure {
        Failure {
            diagnostics: vec![error.diagnostic()],
            ..Failure::new(error.code, error.message)
        }
    }
}

impl From<SelectError> for Failure {
    /// Keeps the location of a problem with a directory's `.mara` as a diagnostic.
    fn from(error: SelectError) -> Failure {
        Failure {
            diagnostics: error.diagnostic().into_iter().collect(),
            ..Failure::new(error.code(), error.to_string())
        }
    }
}

/// Runs the request that `arguments` make, prints its result or failure and returns the exit
/// status.
pub fn run(arguments: &[OsString], work: &mut WorkCounts) -> u8 {
    // Read while this process runs one thread: reading the umask sets it for a moment.
    let caller = Caller::of_this_process();
    let json = requested_json(arguments);
    let reply = match parse(arguments) {
        Ok((None, _)) => {
            print(&mut std::io::stdout(), &usage());
            return SUCCESS;
        }
        Ok((Some(operation), args)) => {
            let mut context = Context {
                work,
                now: SystemTime::now(),
                project: args.value(PROJECT.name).map(PathBuf::from),
                cwd: working_directory(),
                direct: client::direct_mode(args.no_daemon),
                caller,
                session: None,
            };
            match operation.run {
                Run::Document(run) => client::request(operation, run, &mut context, &args, json),
                Run::Command(run) => Reply::new(run(&mut context, &args), json),
                Run::Serve(serve) => return serve(&mut context),
            }
        }
        Err(failure) => Reply::new(Err(failure), json),
    };
    match (json, reply.failed) {
        (true, _) => print(&mut std::io::stdout(), &(reply.text + "\n")),
        (false, false) => print(&mut std::io::stdout(), &reply.text),
        (false, true) => print(&mut std::io::stderr(), &reply.text),
    }
    reply.status
}

/// Writes `text`, ignoring a closed stream: the exit status still tells the outcome.
fn print(stream: &mut dyn std::io::Write, text: &str) {
    let _ = stream.write_all(text.as_bytes());
    let _ = stream.flush();
}

/// Whether `--format json` is among `arguments`, so that rejected arguments are reported in
/// that form too.
fn requested_json(arguments: &[OsString]) -> bool {
    let mut json = false;
    let mut iter = arguments.iter().map(|argument| argument.to_str());
    while let Some(argument) = iter.next() {
        let value = match argument {
            Some("--format") => iter.next().flatten(),
            Some(argument) => argument.strip_prefix("--format="),
            None => None,
        };
        match value {
            Some("json") => json = true,
            Some("human") => json = false,
            _ => {}
        }
    }
    json
}

/// The operation that `arguments` name, if any, and its checked arguments. Global options may
/// stand anywhere; an operation's options follow its words.
fn parse(arguments: &[OsString]) -> Result<(Option<&'static Operation>, Args), Failure> {
    let mut words: Vec<&str> = Vec::new();
    let mut operation: Option<&'static Operation> = None;
    let mut args = Args::default();
    let mut iter = arguments.iter();
    fn text(argument: &OsString) -> Result<&str, Failure> {
        argument.to_str().ok_or_else(|| {
            let shown = argument.to_string_lossy();
            Failure::rejected(format!("the argument `{shown}` is not UTF-8"))
        })
    }
    while let Some(argument) = iter.next() {
        let argument = text(argument)?;
        if let Some(option) = argument.strip_prefix("--") {
            let (name, inline) = match option.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (option, None),
            };
            if name == DEBUG_STATS || name == NO_DAEMON {
                if inline.is_some() {
                    return Err(Failure::rejected(format!("`--{name}` takes no value")));
                }
                args.no_daemon |= name == NO_DAEMON;
                continue;
            }
            let own = operation.map_or(&[][..], |operation| operation.options);
            let Some(option) = [PROJECT, FORMAT]
                .iter()
                .chain(own)
                .find(|option| option.name == name)
            else {
                let place = match operation {
                    Some(operation) => format!(" of `{}`", operation.words.join(" ")),
                    None => String::new(),
                };
                return Err(Failure::rejected(format!(
                    "unknown option `--{name}`{place}"
                )));
            };
            let value = match inline {
                Some(value) => value,
                None => match iter.next() {
                    Some(value) => text(value)?,
                    None => {
                        return Err(Failure::rejected(format!(
                            "`--{name}` needs a value: `--{name} <{}>`",
                            option.value
                        )));
                    }
                },
            };
            args.push(option, value)
                .map_err(|reason| Failure::rejected(format!("`--{name}`: {reason}")))?;
        } else if argument.starts_with('-') {
            return Err(Failure::rejected(format!("unknown option `{argument}`")));
        } else if let Some(operation) = operation {
            if args.arguments.len() == operation.arguments.len() {
                return Err(Failure::rejected(format!(
                    "unexpected argument `{argument}`: `{}` takes {}",
                    operation.words.join(" "),
                    describe_arguments(operation)
                )));
            }
            args.arguments.push(argument.to_owned());
        } else {
            words.push(argument);
            operation = OPERATIONS.iter().find(|operation| operation.words == words);
            if operation.is_none() && !OPERATIONS.iter().any(|o| o.words.starts_with(&words)) {
                return Err(Failure::rejected(format!(
                    "unknown operation `{}`",
                    words.join(" ")
                )));
            }
        }
    }
    match operation {
        None if !words.is_empty() => {
            let named: Vec<String> = OPERATIONS
                .iter()
                .filter(|operation| operation.words.starts_with(&words))
                .map(|operation| format!("`{}`", operation.words.join(" ")))
                .collect();
            let count = named.len();
            Err(Failure::rejected(format!(
                "`{}` is not an operation; the operations it starts: {}",
                words.join(" "),
                validation::list(named, count)
            )))
        }
        Some(operation) if args.arguments.len() < operation.arguments.len() => {
            Err(Failure::rejected(format!(
                "`{}` takes {}",
                operation.words.join(" "),
                describe_arguments(operation)
            )))
        }
        operation => Ok((operation, args)),
    }
}

/// The positional arguments of `operation`, for a message.
fn describe_arguments(operation: &Operation) -> String {
    match operation.arguments {
        [] => "no argument".to_owned(),
        arguments => {
            let names: Vec<String> = arguments
                .iter()
                .map(|argument| format!("<{}>", argument.name))
                .collect();
            names.join(" ")
        }
    }
}

/// What bare `mara` prints.
fn usage() -> String {
    let option = |option: &Opt| {
        let repeat = if option.repeatable { "..." } else { "" };
        format!(" [--{} <{}>]{repeat}", option.name, option.value)
    };
    let mut out = format!(
        "Usage: mara{}{} [--{DEBUG_STATS}] [--{NO_DAEMON}] <operation>\n\nOperations:\n",
        option(&PROJECT),
        option(&FORMAT)
    );
    for operation in OPERATIONS {
        let mut line = format!("  mara {}", operation.words.join(" "));
        for argument in operation.arguments {
            write!(line, " <{}>", argument.name).expect("writing to a String cannot fail");
        }
        for opt in operation.options {
            line.push_str(&option(opt));
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(text: &str) -> Vec<OsString> {
        text.split_whitespace().map(OsString::from).collect()
    }

    fn rejected(text: &str) -> String {
        match parse(&arguments(text)) {
            Ok(_) => panic!("{text:?} was accepted"),
            Err(failure) => {
                assert_eq!(
                    (failure.code, failure.status),
                    ("invalid-argument", REJECTED)
                );
                failure.message
            }
        }
    }

    #[test]
    fn reads_global_options_anywhere_and_operation_options_after_the_operation() {
        let (operation, args) = parse(&arguments(
            "--format json project --project=x validate --path a --path b",
        ))
        .unwrap();
        assert_eq!(operation.unwrap().words, ["project", "validate"]);
        assert_eq!(args.value("format"), Some("json"));
        assert_eq!(args.value("project"), Some("x"));
        assert_eq!(args.values("path").collect::<Vec<_>>(), ["a", "b"]);
        let (operation, args) = parse(&arguments("item validate REQ-A --limit 5")).unwrap();
        assert_eq!(operation.unwrap().words, ["item", "validate"]);
        assert_eq!((args.argument(0), args.limit()), ("REQ-A", 5));
        let (operation, _) = parse(&arguments("--debug-stats")).unwrap();
        assert!(operation.is_none());
    }

    #[test]
    fn rejects_every_undeclared_or_malformed_argument() {
        for (text, expected) in [
            ("--no-such-option", "unknown option `--no-such-option`"),
            ("--limit 1 project validate", "unknown option `--limit`"),
            (
                "schema validate --path a",
                "unknown option `--path` of `schema validate`",
            ),
            ("project validate -x", "unknown option `-x`"),
            (
                "project",
                "`project` is not an operation; the operations it starts: `project validate`",
            ),
            ("project check", "unknown operation `project check`"),
            ("item validate", "`item validate` takes <id-or-mid>"),
            ("item validate A B", "unexpected argument `B`"),
            ("project validate extra", "unexpected argument `extra`"),
            ("project validate --limit", "`--limit` needs a value"),
            (
                "project validate --limit 0",
                "`--limit`: a page holds 1 to 100",
            ),
            (
                "project validate --limit 1 --limit 2",
                "`--limit`: given twice, but it takes one value",
            ),
            (
                "--format json project validate --format json",
                "`--format`: given twice",
            ),
            (
                "project validate --cursor x",
                "`--cursor`: `x` is not a cursor",
            ),
            (
                "--format xml",
                "`--format`: the format is `human` or `json`",
            ),
            ("--debug-stats=1", "takes no value"),
            ("--no-daemon=1", "`--no-daemon` takes no value"),
            ("project validate -- --no-daemon", "unknown option `--`"),
        ] {
            let message = rejected(text);
            assert!(message.contains(expected), "{text:?}: {message}");
        }
    }

    /// Direct mode comes from the parsed flag, wherever it stands, and never from an option's
    /// value that reads as the flag.
    #[test]
    fn reads_the_direct_mode_flag_only_where_it_is_a_flag() {
        for (text, flag) in [
            ("--no-daemon item list", true),
            ("item list --no-daemon --limit 2", true),
            ("item list", false),
            ("item list --path --no-daemon", false),
            ("item list --path=--no-daemon", false),
            ("query view v --input x=--no-daemon", false),
        ] {
            let (_, args) = parse(&arguments(text)).unwrap();
            assert_eq!(args.no_daemon, flag, "{text:?}");
        }
        let (_, args) = parse(&arguments("item list --path --no-daemon")).unwrap();
        assert_eq!(args.value("path"), Some("--no-daemon"));
    }

    #[test]
    fn normalizes_project_paths_and_refuses_those_outside() {
        for (text, path) in [
            ("docs", "docs"),
            ("docs/", "docs"),
            ("./docs", "docs"),
            ("./docs/./a.mara.md", "docs/a.mara.md"),
            ("docs//links/", "docs/links"),
        ] {
            assert_eq!(project_path(text).as_deref(), Ok(path), "{text:?}");
        }
        for text in ["", ".", "./", "/docs", "../docs", "docs/../x", "docs/.."] {
            assert!(project_path(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn finds_the_requested_format_even_in_rejected_arguments() {
        assert!(requested_json(&arguments("--bogus --format json")));
        assert!(requested_json(&arguments("--format=json")));
        assert!(!requested_json(&arguments("--format json --format human")));
        assert!(!requested_json(&arguments("project validate")));
    }

    #[test]
    fn lists_every_operation_in_the_usage() {
        let usage = usage();
        for operation in OPERATIONS {
            assert!(usage.contains(&operation.words.join(" ")), "{usage}");
        }
        assert!(usage.contains("mara item validate <id-or-mid> [--limit <n>] [--cursor <cursor>]"));
    }
}
