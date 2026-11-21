//! The CLI and the MCP server as clients of the daemon (REQ-FIRST-CALL-IN-PROCESS and
//! DES-DAEMON-LIFECYCLE in `docs/daemon/index.mara.md`).
//!
//! A document operation goes to the daemon of this build when one runs ([`request`]). When none
//! runs, the client starts one in the background with `mara daemon start` and runs the operation
//! in-process. A request is dispatched once its frame is written in full: from then on it is
//! never run again, by the client or by the daemon, and a daemon that ends before it answers
//! gives `daemon-lost`. Before that, any failure, and a daemon's refusal, which says that it ran
//! nothing, leave the request to run in-process.
//!
//! The request carries what the operation needs from the client's side: the operation's words,
//! its checked arguments, the inputs an MCP call passes, `--project`, the form of the reply, and
//! the client's working directory, umask and temporary directory ([`Caller`]). The daemon runs the
//! same function in a [`Context`] with that directory and its project's session, inside
//! [`caller::serve`] with the other two, and answers with the reply and the work counts of the
//! session since its previous response ([`answer`]). Until it answers, it sends a heartbeat every
//! [`daemon::HEARTBEAT`]; a client that hears nothing for [`SILENCE`] gives up with
//! `daemon-lost`.

use std::io::{self, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use mara::caller::{self, Caller};
use mara::daemon::{self, Build, PROTOCOL_VERSION, Runtime};
use mara::project;
use mara::session::Sessions;
use mara::watch::Watcher;
use mara_engine::facts::Supplied;
use mara_engine::work::WorkCounts;
use serde_json::{Value, json};

use super::{Args, Context, FORMAT, Failure, OPERATIONS, Operation, PROJECT, Reply, Run};

/// The document operations that always run in the calling process: they create or convert a
/// project, as `prime` reads one (DES-DAEMON-LIFECYCLE).
const IN_PROCESS: &[&[&str]] = &[&["project", "init"], &["project", "convert"]];

/// The document operations that write: a daemon that stops lets them finish
/// (DES-DAEMON-LIFECYCLE).
const WRITES: &[&[&str]] = &[
    &["item", "create"],
    &["item", "update"],
    &["item", "delete"],
    &["item", "move"],
    &["item", "rename"],
    &["relation", "add"],
    &["relation", "remove"],
    &["project", "fix"],
];

/// How long a client waits to hand a request to the daemon, in all. A request that is not
/// written in full by then runs in-process.
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a client waits for a frame of the daemon once its request is dispatched. The daemon
/// sends one every [`daemon::HEARTBEAT`] while the request runs, so only a daemon that stopped
/// or hangs stays silent this long; the request then fails with `daemon-lost`.
pub const SILENCE: Duration = Duration::from_secs(15);

/// A document operation's function.
type Document = fn(&mut Context, &Args) -> Result<super::Response, Failure>;

/// Whether a client runs in direct mode, neither reaching a daemon nor starting one: `CI` set to
/// any non-empty value, `MARA_NO_DAEMON=1` or the direct-mode flag `--no-daemon`.
pub fn direct_mode(flag: bool) -> bool {
    let ci = std::env::var_os("CI").is_some_and(|value| !value.is_empty());
    let no_daemon = std::env::var_os("MARA_NO_DAEMON").is_some_and(|value| value == "1");
    flag || ci || no_daemon
}

/// Runs the document operation `operation` for a client: through the daemon of this build when
/// one runs, otherwise in-process after starting one. The work the daemon reports is added to
/// `context.work`; the client adds none of its own to it.
pub fn request(
    operation: &'static Operation,
    run: Document,
    context: &mut Context,
    args: &Args,
    json: bool,
) -> Reply {
    if context.direct || IN_PROCESS.contains(&operation.words) {
        return Reply::new(run(context, args), json);
    }
    match dispatch(operation, context, args, json) {
        Sent::Answered(reply, work) => {
            *context.work += &work;
            reply
        }
        Sent::Lost { pid, why } => Reply::new(Err(lost(operation, pid, &why)), json),
        Sent::Not { start } => {
            if start && let Ok(cwd) = context.cwd() {
                start_daemon(cwd.to_owned(), context.project.clone());
            }
            Reply::new(run(context, args), json)
        }
    }
}

/// What became of a request offered to the daemon.
enum Sent {
    /// The daemon ran it and answered.
    Answered(Reply, WorkCounts),
    /// It was dispatched, and the daemon ended, or its answer could not be read.
    Lost { pid: Option<u32>, why: String },
    /// It was not dispatched, or the daemon refused it without running it. `start` when no daemon
    /// of this build runs, so that the client starts one.
    Not { start: bool },
}

/// Offers the request to the daemon of this build.
fn dispatch(operation: &Operation, context: &Context, args: &Args, json: bool) -> Sent {
    let not = Sent::Not { start: false };
    let Ok(build) = Build::current() else {
        return not;
    };
    let Ok(runtime) = Runtime::find(&build) else {
        return not;
    };
    let mut stream = match runtime.connect() {
        Ok(Some(stream)) => stream,
        Ok(None) => return Sent::Not { start: true },
        // A runtime directory or socket that is not ours: `mara daemon status` says why.
        Err(_) => return not,
    };
    let Some(message) = message(operation, context, args, json, &build) else {
        return not;
    };
    let pid = runtime.lock_holder();
    if send(&mut stream, &message).is_err() {
        // Not written in full: the daemon cannot have read the request, so it ran nothing.
        return not;
    }
    // Dispatched. A long validation may take any time, as it would in-process, while the
    // daemon says that it still runs; silence past `SILENCE` ends the wait.
    let frame = loop {
        let answer = stream
            .set_read_timeout(Some(SILENCE))
            .and_then(|()| daemon::read_frame(&mut stream));
        match answer {
            Ok(Some(frame)) if is_heartbeat(&frame) => {}
            Ok(Some(frame)) => break frame,
            Ok(None) => {
                let why = "it closed the connection before it answered".to_owned();
                return Sent::Lost { pid, why };
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                let why = format!(
                    "it sent nothing for {} s, so it has stopped or hangs",
                    SILENCE.as_secs()
                );
                return Sent::Lost { pid, why };
            }
            Err(error) => {
                let why = format!("its answer could not be read: {error}");
                return Sent::Lost { pid, why };
            }
        }
    };
    let answer: Value = match serde_json::from_slice(&frame) {
        Ok(answer) => answer,
        Err(error) => {
            let why = format!("its answer is not JSON: {error}");
            return Sent::Lost { pid, why };
        }
    };
    let pid = answer["pid"]
        .as_u64()
        .and_then(|pid| u32::try_from(pid).ok());
    if answer["error"].is_string() {
        // Refused before it ran anything: stopping, or another build.
        return not;
    }
    match reply(&answer) {
        Some(answered) => answered,
        None => Sent::Lost {
            pid,
            why: "its answer holds no reply".to_owned(),
        },
    }
}

/// The operation request, or `None` when a part of it cannot travel as JSON text, such as a
/// working directory that is not UTF-8: then the request runs in-process.
fn message(
    operation: &Operation,
    context: &Context,
    args: &Args,
    json: bool,
    build: &Build,
) -> Option<Value> {
    let cwd = context.cwd.as_ref().ok()?;
    // A relative `TMPDIR` names a directory of the client's.
    let temp_dir = cwd.join(&context.caller.temp_dir);
    let temp_dir = temp_dir.to_str()?;
    let cwd = cwd.to_str()?;
    let project = match &context.project {
        Some(project) => Value::from(project.to_str()?),
        None => Value::Null,
    };
    let options: Vec<Value> = (args.options.iter())
        .map(|(name, value)| json!([name, value]))
        .collect();
    let supplied: Vec<Value> = (args.supplied.iter())
        .map(|(name, value)| match value {
            Supplied::Inline(text) => json!([name, {"inline": text}]),
            Supplied::Rows(rows) => json!([name, {"rows": rows}]),
        })
        .collect();
    Some(json!({
        "version": PROTOCOL_VERSION,
        "build": build.key(),
        "request": "operation",
        "words": operation.words,
        "arguments": args.arguments,
        "options": options,
        "supplied": supplied,
        "project": project,
        "cwd": cwd,
        "umask": context.caller.umask,
        "temp_dir": temp_dir,
        "json": json,
    }))
}

/// Writes the request's frame in full within [`SEND_TIMEOUT`], or fails and ends the
/// connection for writing, as [`daemon::send_frame`] does.
fn send(stream: &mut UnixStream, message: &Value) -> io::Result<()> {
    let sent = write_request(stream, message);
    if sent.is_err() {
        let _ = stream.shutdown(Shutdown::Write);
    }
    sent
}

fn write_request(stream: &mut UnixStream, message: &Value) -> io::Result<()> {
    let payload = message.to_string();
    let length = daemon::frame_length(payload.as_bytes())?;
    let deadline = Instant::now() + SEND_TIMEOUT;
    for part in [&length[..], payload.as_bytes()] {
        let mut written = 0;
        while written < part.len() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            stream.set_write_timeout(Some(left))?;
            match stream.write(&part[written..]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(count) => written += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(())
}

/// Whether `frame` only says that the daemon still runs the request.
fn is_heartbeat(frame: &[u8]) -> bool {
    serde_json::from_slice::<Value>(frame).is_ok_and(|message| message["working"] == true)
}

/// The reply and work counts of an answer.
fn reply(answer: &Value) -> Option<Sent> {
    let reply = Reply {
        text: answer["text"].as_str()?.to_owned(),
        failed: answer["failed"].as_bool()?,
        status: u8::try_from(answer["status"].as_u64()?).ok()?,
    };
    let mut work = WorkCounts::default();
    let counts = answer["work"].as_object()?;
    for (name, field) in [
        ("file_reads", &mut work.file_reads),
        ("hashes", &mut work.hashes),
        ("parses", &mut work.parses),
        ("snapshot_builds", &mut work.snapshot_builds),
        ("code_scans", &mut work.code_scans),
        ("subprocess_spawns", &mut work.subprocess_spawns),
        ("check_steps", &mut work.check_steps),
        ("program_runs", &mut work.program_runs),
    ] {
        *field = counts.get(name)?.as_u64()?;
    }
    Some(Sent::Answered(reply, work))
}

/// `daemon-lost`: the daemon ended, stopped or hung before it answered a dispatched request.
fn lost(operation: &Operation, pid: Option<u32>, why: &str) -> Failure {
    let daemon = match pid {
        Some(pid) => format!("the daemon (process {pid})"),
        None => "the daemon".to_owned(),
    };
    let words = operation.words.join(" ");
    Failure::new(
        "daemon-lost",
        format!(
            "{daemon} gave no answer to `{words}`: {why}. The request was not run again; a \
             write may be left interrupted, which `mara project validate` reports"
        ),
    )
}

/// Starts `mara daemon start` of this executable in the background, detached from the client's
/// standard streams and process group, in the client's directory and with its `--project`. A
/// failure to start it is no failure of the request, which runs in-process all the same.
fn start_daemon(cwd: PathBuf, project: Option<PathBuf>) {
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let mut command = Command::new(executable);
    if let Some(project) = project {
        command.arg("--project").arg(cwd.join(project));
    }
    command
        .args(["daemon", "start"])
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    if let Ok(mut child) = command.spawn() {
        // Reaped by a thread, so that a long-lived MCP server leaves no zombie.
        let _ = std::thread::Builder::new()
            .name("mara-daemon-start".to_owned())
            .spawn(move || child.wait());
    }
}

/// The daemon's answer to an operation request: the reply and the work counts of the request's
/// session since its previous response, or a refusal, which runs nothing, when the request
/// names no operation the daemon serves or carries arguments that the operation does not take.
///
/// The request runs in the session of its project, opened unless it is open
/// (DES-DAEMON-SESSIONS); a request that selects no project runs without one and reports its own
/// counts. Every request checks the project's inputs on disk (REQ-NEVER-STALE).
pub fn answer(
    request: &Value,
    sessions: &Sessions,
    watcher: Option<&Watcher>,
) -> Result<Value, String> {
    let (operation, run) = served(request)?;
    let args = args(operation, request)?;
    let cwd = request["cwd"]
        .as_str()
        .ok_or("the request names no working directory")?;
    let project = match &request["project"] {
        Value::Null => None,
        Value::String(project) => Some(PathBuf::from(project)),
        _ => return Err("`project` is no string".to_owned()),
    };
    let json = request["json"].as_bool().ok_or("`json` is no boolean")?;
    let caller = Caller {
        umask: (request["umask"].as_u64())
            .and_then(|umask| u32::try_from(umask).ok())
            .filter(|umask| *umask <= 0o777)
            .ok_or("`umask` is no umask")?,
        temp_dir: PathBuf::from(
            request["temp_dir"]
                .as_str()
                .ok_or("the request names no temporary directory")?,
        ),
    };
    let cwd = PathBuf::from(cwd);
    let entered =
        (project::select(&cwd, project.as_deref()).ok()).map(|root| sessions.enter(&root));
    let mut work = WorkCounts::default();
    let mut context = Context {
        work: &mut work,
        now: SystemTime::now(),
        project,
        cwd: Ok(cwd),
        direct: true,
        caller: caller.clone(),
        session: entered.as_ref().map(|(session, _)| Arc::clone(session)),
    };
    // Files and directories get the client's modes, and evaluations its temporary directory.
    let reply = caller::serve(caller, || Reply::new(run(&mut context, &args), json));
    let work = match &entered {
        Some((session, _)) => {
            session.add(&work);
            // The directories of its inputs may have changed: they are watched from now on.
            if let Some(watcher) = watcher {
                watcher.sync(session);
            }
            session.take_work()
        }
        None => work,
    };
    let counts: serde_json::Map<String, Value> = (work.fields().into_iter())
        .map(|(name, value)| (name.to_owned(), Value::from(value)))
        .collect();
    Ok(json!({
        "text": reply.text,
        "failed": reply.failed,
        "status": reply.status,
        "work": counts,
    }))
}

/// Whether the operation request `request` names an operation that writes.
pub fn writes(request: &Value) -> bool {
    let words: Vec<&str> = (request["words"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    WRITES.contains(&words.as_slice())
}

/// The operation a request names, if the daemon serves it.
fn served(request: &Value) -> Result<(&'static Operation, Document), String> {
    let words: Vec<&str> = (request["words"].as_array().into_iter().flatten())
        .map(|word| word.as_str().ok_or("a word is no string"))
        .collect::<Result<_, _>>()?;
    OPERATIONS
        .iter()
        .find_map(|operation| match operation.run {
            Run::Document(run)
                if operation.words == words && !IN_PROCESS.contains(&operation.words) =>
            {
                Some((operation, run))
            }
            _ => None,
        })
        .ok_or_else(|| format!("the daemon serves no operation `{}`", words.join(" ")))
}

/// The arguments of the request, checked again as the client checked them, so that no request
/// reaches an operation with arguments it does not take.
fn args(operation: &Operation, request: &Value) -> Result<Args, String> {
    let strings = |name: &str| -> Result<Vec<&str>, String> {
        (request[name]
            .as_array()
            .ok_or(format!("`{name}` is no array"))?)
        .iter()
        .map(|value| value.as_str().ok_or(format!("`{name}` holds no strings")))
        .collect()
    };
    let mut args = Args::default();
    let arguments = strings("arguments")?;
    if arguments.len() != operation.arguments.len() {
        return Err("the request has another number of arguments".to_owned());
    }
    args.arguments = arguments.into_iter().map(str::to_owned).collect();
    for pair in request["options"]
        .as_array()
        .ok_or("`options` is no array")?
    {
        let (Some(name), Some(value)) = (pair[0].as_str(), pair[1].as_str()) else {
            return Err("an option is no pair of strings".to_owned());
        };
        let option = [PROJECT, FORMAT]
            .iter()
            .chain(operation.options)
            .find(|option| option.name == name)
            .ok_or_else(|| format!("unknown option `{name}`"))?;
        args.push(option, value)
            .map_err(|reason| format!("`{name}`: {reason}"))?;
    }
    for pair in request["supplied"]
        .as_array()
        .ok_or("`supplied` is no array")?
    {
        let name = pair[0].as_str().ok_or("an input has no name")?;
        let value = match (pair[1]["inline"].as_str(), &pair[1]["rows"]) {
            (Some(text), Value::Null) => Supplied::Inline(text.to_owned()),
            (None, rows) if !rows.is_null() => Supplied::Rows(rows.clone()),
            _ => return Err(format!("input `{name}` holds no value")),
        };
        args.supplied.push((name.to_owned(), value));
    }
    Ok(args)
}
