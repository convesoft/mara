//! `mara daemon status | start | stop` and the client's side of the daemon's requests
//! (DES-DAEMON-LIFECYCLE in `docs/daemon/index.mara.md`).
//!
//! A client finds the daemon of its own build ([`Runtime::find`]), connects to its socket and
//! sends one request per frame: a JSON object with `version`, the client's `build` key and
//! `request`, the request's name. The daemon answers each with one frame: a JSON object with
//! its `pid`, and `error` with `message` when it refuses the request ([`server`]).

mod server;

use std::io;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use mara::daemon::{self, Build, PROTOCOL_VERSION, Runtime, Unavailable};
use mara_engine::json::Object;
use serde_json::{Value, json};

use super::{Args, Context, FORMAT_VERSION, Failure, Operation, Response, Run, SUCCESS};

pub const STATUS: Operation = Operation {
    words: &["daemon", "status"],
    summary: "Prints whether the daemon of this build runs, its process ID, the root of each open \
              session and how many operation requests it has served.",
    arguments: &[],
    options: &[],
    run: Run::Command(status),
};

pub const START: Operation = Operation {
    words: &["daemon", "start"],
    summary: "Starts the daemon of this build in the background unless it runs.",
    arguments: &[],
    options: &[],
    run: Run::Command(start),
};

pub const STOP: Operation = Operation {
    words: &["daemon", "stop"],
    summary: "Stops the daemon of this build and returns once it has exited.",
    arguments: &[],
    options: &[],
    run: Run::Command(stop),
};

/// How long a client waits for the answer to a request that does no project work.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);
/// How long `daemon start` waits for another process that holds the lock to serve.
const START_TIMEOUT: Duration = Duration::from_secs(10);
/// How often `daemon start` looks again meanwhile.
const START_POLL: Duration = Duration::from_millis(20);

/// The daemon of this build that answered a status request.
struct Running {
    pid: u64,
    sessions: Vec<String>,
    /// Operation requests it has run and answered.
    served: u64,
}

/// This build, and its runtime directory or why it has none.
fn target() -> Result<(Build, Result<Runtime, Unavailable>), Failure> {
    let build = Build::current().map_err(|error| {
        Failure::new(
            "io-error",
            format!("cannot examine the running executable: {error}"),
        )
    })?;
    let runtime = Runtime::find(&build);
    Ok((build, runtime))
}

fn io_failure(error: io::Error) -> Failure {
    Failure::new("io-error", error.to_string())
}

/// Sends `request` and returns the daemon's answer, refusals included.
fn ask(stream: &mut UnixStream, build: &Build, request: &str) -> io::Result<Value> {
    let message = json!({
        "version": PROTOCOL_VERSION,
        "build": build.key(),
        "request": request,
    });
    daemon::write_frame(stream, message.to_string().as_bytes())?;
    let Some(frame) = daemon::read_frame(stream)? else {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "the daemon closed the connection before it answered",
        ));
    };
    serde_json::from_slice(&frame)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// The daemon of `build` if one runs and takes requests: none while it stops.
fn running(runtime: &Runtime, build: &Build) -> io::Result<Option<Running>> {
    let Some(mut stream) = runtime.connect()? else {
        return Ok(None);
    };
    stream.set_read_timeout(Some(ANSWER_TIMEOUT))?;
    stream.set_write_timeout(Some(ANSWER_TIMEOUT))?;
    let answer = match ask(&mut stream, build, "status") {
        Ok(answer) => answer,
        // It exited between the connection and the answer.
        Err(error) if ended(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    match answer["error"].as_str() {
        None => {}
        Some("stopping") => return Ok(None),
        Some(_) => return Err(refused(&answer)),
    }
    let sessions = (answer["sessions"].as_array().into_iter().flatten())
        .filter_map(|root| root.as_str().map(str::to_owned))
        .collect();
    Ok(Some(Running {
        pid: answer["pid"].as_u64().unwrap_or_default(),
        sessions,
        served: answer["served"].as_u64().unwrap_or_default(),
    }))
}

/// Whether `error` means that the daemon closed the connection.
fn ended(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::UnexpectedEof
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
    )
}

fn refused(answer: &Value) -> io::Error {
    let message = answer["message"].as_str().unwrap_or("no reason given");
    io::Error::other(format!("the daemon refused the request: {message}"))
}

fn null_or_number(object: &mut Object, name: &str, value: Option<u64>) {
    match value {
        Some(value) => object.number(name, value),
        None => object.key(name).push_str("null"),
    }
}

fn status(_: &mut Context, _: &Args) -> Result<Response, Failure> {
    let (build, runtime) = target()?;
    let found = match &runtime {
        Ok(runtime) => running(runtime, &build).map_err(io_failure)?,
        Err(_) => None,
    };
    let human = match (&runtime, &found) {
        (Err(Unavailable(reason)), _) => {
            format!("No daemon can run: {reason}. Every call runs in direct mode.\n")
        }
        (Ok(_), None) => "No daemon of this build runs.\n".to_owned(),
        (Ok(_), Some(running)) => {
            let mut text = format!(
                "The daemon of this build runs as process {} and has served {} operation \
                 requests.\n",
                running.pid, running.served
            );
            if running.sessions.is_empty() {
                text.push_str("Sessions: none\n");
            } else {
                text.push_str("Sessions:\n");
                for root in &running.sessions {
                    text.push_str(&format!("  {root}\n"));
                }
            }
            text
        }
    };
    let sessions: Vec<String> = (found.iter())
        .flat_map(|running| &running.sessions)
        .map(|root| {
            let mut out = String::new();
            mara_engine::json::string(&mut out, root);
            out
        })
        .collect();
    let paths = runtime.as_ref().ok().map(|runtime| {
        (
            runtime.socket.display().to_string(),
            runtime.lock.display().to_string(),
        )
    });
    let mut json = String::new();
    let mut object = Object::new(&mut json);
    object.number("format_version", FORMAT_VERSION);
    object.bool("running", found.is_some());
    null_or_number(
        &mut object,
        "pid",
        found.as_ref().map(|running| running.pid),
    );
    object.array("sessions", sessions.iter().map(String::as_str));
    null_or_number(
        &mut object,
        "served",
        found.as_ref().map(|running| running.served),
    );
    object.optional_string("socket", paths.as_ref().map(|paths| paths.0.as_str()));
    object.optional_string("lock", paths.as_ref().map(|paths| paths.1.as_str()));
    object.optional_string(
        "unavailable",
        runtime
            .as_ref()
            .err()
            .map(|Unavailable(reason)| reason.as_str()),
    );
    object.end();
    Ok(Response {
        json,
        human,
        status: SUCCESS,
    })
}

/// Starts the daemon unless one of this build runs. When another process holds the lock, as a
/// daemon does while it starts, waits until a daemon answers. The daemon opens the session of
/// the project that `--project` names or that holds the working directory, if any.
fn start(context: &mut Context, _: &Args) -> Result<Response, Failure> {
    let (build, runtime) = target()?;
    // Before the fork: the daemon runs in `/`.
    let project = context.root().ok();
    let runtime = runtime.map_err(|Unavailable(reason)| {
        Failure::new(
            "io-error",
            format!("the daemon cannot run: {reason}; every call runs in direct mode"),
        )
    })?;
    runtime.create().map_err(io_failure)?;
    let lock = runtime.open_lock().map_err(io_failure)?;
    let deadline = Instant::now() + START_TIMEOUT;
    let (started, pid) = loop {
        if let Some(running) = running(&runtime, &build).map_err(io_failure)? {
            break (false, running.pid);
        }
        if daemon::try_lock(&lock).map_err(io_failure)? {
            let pid = server::launch(&runtime, &build, lock, project.clone())
                .map_err(|message| Failure::new("io-error", message))?;
            break (true, u64::from(pid));
        }
        if Instant::now() >= deadline {
            let holder = match runtime.lock_holder() {
                Some(pid) => format!("process {pid}"),
                None => "another process".to_owned(),
            };
            return Err(Failure::new(
                "io-error",
                format!(
                    "{holder} holds the daemon lock `{}`, but no daemon answered on `{}` \
                     within {} s; stop that process, or run every call in direct mode",
                    runtime.lock.display(),
                    runtime.socket.display(),
                    START_TIMEOUT.as_secs()
                ),
            ));
        }
        std::thread::sleep(START_POLL);
    };
    let human = if started {
        format!("Started the daemon of this build as process {pid}.\n")
    } else {
        format!("The daemon of this build already runs as process {pid}.\n")
    };
    let mut json = String::new();
    let mut object = Object::new(&mut json);
    object.number("format_version", FORMAT_VERSION);
    object.bool("started", started);
    object.number("pid", pid);
    object.end();
    Ok(Response {
        json,
        human,
        status: SUCCESS,
    })
}

/// Asks the daemon to stop and waits until it has exited: its end of the connection closes
/// only when the process ends.
fn stop(_: &mut Context, _: &Args) -> Result<Response, Failure> {
    let (build, runtime) = target()?;
    let stopped = match &runtime {
        Ok(runtime) => stop_daemon(runtime, &build).map_err(io_failure)?,
        Err(_) => None,
    };
    let human = match stopped {
        Some(Some(pid)) => format!("Stopped the daemon of this build (process {pid}).\n"),
        Some(None) => "Stopped the daemon of this build.\n".to_owned(),
        None => "No daemon of this build runs.\n".to_owned(),
    };
    let mut json = String::new();
    let mut object = Object::new(&mut json);
    object.number("format_version", FORMAT_VERSION);
    object.bool("stopped", stopped.is_some());
    null_or_number(&mut object, "pid", stopped.flatten());
    object.end();
    Ok(Response {
        json,
        human,
        status: SUCCESS,
    })
}

/// The daemon that stopped, once it has exited, with its process ID if known; `None` when none
/// ran.
fn stop_daemon(runtime: &Runtime, build: &Build) -> io::Result<Option<Option<u64>>> {
    let Some(mut stream) = runtime.connect()? else {
        return Ok(None);
    };
    stream.set_read_timeout(Some(ANSWER_TIMEOUT))?;
    stream.set_write_timeout(Some(ANSWER_TIMEOUT))?;
    // Read before asking: a daemon that ends before it answers, for this request or another
    // reason, has stopped all the same.
    let holder = runtime.lock_holder().map(u64::from);
    let answer = match ask(&mut stream, build, "stop") {
        Ok(answer) => answer,
        Err(error) if ended(&error) => return Ok(Some(holder)),
        Err(error) => return Err(error),
    };
    // A daemon that already stops refuses the request, and ends all the same.
    if !matches!(answer["error"].as_str(), None | Some("stopping")) {
        return Err(refused(&answer));
    }
    // The daemon finishes the requests it runs before it exits.
    stream.set_read_timeout(None)?;
    loop {
        match daemon::read_frame(&mut stream) {
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(error) if ended(&error) => break,
            Err(error) => return Err(error),
        }
    }
    Ok(Some(answer["pid"].as_u64()))
}
