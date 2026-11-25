//! `mara daemon status | start | stop` and the client's side of the daemon's requests
//! (DES-DAEMON-LIFECYCLE in `docs/daemon/index.mara.md`).
//!
//! A client finds the daemon of its own build ([`Runtime::find`]), connects to its socket and
//! sends one request per frame: a JSON object with `version`, the client's `build` key and
//! `request`, the request's name. The daemon answers each with one frame: a JSON object with
//! its `pid`, and `error` with `message` when it refuses the request ([`server`]).
//!
//! `daemon start --http` also starts the daemon's listener for browsers (DES-BROWSER-LISTENER),
//! whose port and token `start` and `status` print.

mod server;

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::net::Ipv6Addr;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use mara::caller::Caller;
use mara::daemon::{self, Build, PROTOCOL_VERSION, Runtime, Unavailable};
use mara_engine::json::Object;
use serde_json::{Value, json};

use super::{Args, Context, FORMAT_VERSION, Failure, Kind, Operation, Opt, Response, Run, SUCCESS};

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
    summary: "Starts the daemon of this build in the background unless it runs; with `--http`, \
              one that also serves browsers of the allowed origins on the allowed projects.",
    arguments: &[],
    options: &[HTTP, ALLOW_ORIGIN, ALLOW_PROJECT],
    run: Run::Command(start),
};

/// `--http`: the daemon listens for browsers (DES-BROWSER-LISTENER).
const HTTP: Opt = Opt {
    name: "http",
    value: "",
    help: "Listens for browsers on a loopback port, until `mara daemon stop`.",
    repeatable: false,
    required: false,
    kind: Kind::Flag,
    check: |_| Ok(()),
};

/// `--allow-origin <origin>`: an origin whose pages may reach the listener. Repeatable.
const ALLOW_ORIGIN: Opt = Opt {
    name: "allow-origin",
    value: "origin",
    help: "An origin whose pages may reach the listener, as a browser sends it: \
           `http://127.0.0.1:5173`.",
    repeatable: true,
    required: false,
    kind: Kind::Text,
    check: origin,
};

/// `--allow-project <dir>`: a project root that browsers may reach. Repeatable.
const ALLOW_PROJECT: Opt = Opt {
    name: "allow-project",
    value: "dir",
    help: "A project root that browsers may reach, absolute or relative to the working \
           directory.",
    repeatable: true,
    required: false,
    kind: Kind::Text,
    check: |_| Ok(()),
};

// @mara implements REQ-BROWSER-ACCESS-AUTHORIZED
/// Checks that `value` has a form a browser sends in `Origin` (DES-BROWSER-LISTENER): `http://` or
/// `https://`, then a non-empty host and an optional `:port`, with no `/`, `?`, `#`, `@`, `*`,
/// whitespace, control character or ASCII uppercase letter after the scheme; a port of 1 to 5
/// digits naming 1 to 65535, without a leading zero and not the scheme's default; a host in
/// brackets an IPv6 address. Nothing else of the host is checked: a request's `Origin` is
/// compared byte for byte, so an origin no browser sends only never matches.
fn origin(value: &str) -> Result<(), String> {
    let refuse = || {
        Err(format!(
            "`{value}` is no origin as a browser sends it: write `http://` or `https://`, a host \
             in lowercase and an optional port, such as `http://127.0.0.1:5173`, with no path, \
             no `/` at the end and no `*`"
        ))
    };
    let (rest, default) = match (
        value.strip_prefix("http://"),
        value.strip_prefix("https://"),
    ) {
        (Some(rest), _) => (rest, "80"),
        (_, Some(rest)) => (rest, "443"),
        _ => return refuse(),
    };
    let forbidden = |c: char| {
        "/?#@*".contains(c) || c.is_whitespace() || c.is_control() || c.is_ascii_uppercase()
    };
    if rest.contains(forbidden) {
        return refuse();
    }
    let (host, port) = match rest.strip_prefix('[') {
        Some(bracketed) => match bracketed.split_once(']') {
            Some((address, after)) if address.parse::<Ipv6Addr>().is_ok() => {
                match after.strip_prefix(':') {
                    Some(port) => (address, Some(port)),
                    None if after.is_empty() => (address, None),
                    None => return refuse(),
                }
            }
            _ => return refuse(),
        },
        None => match rest.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (rest, None),
        },
    };
    let port_ok = |port: &str| {
        (1..=5).contains(&port.len())
            && port.bytes().all(|byte| byte.is_ascii_digit())
            && !port.starts_with('0')
            && port.parse::<u16>().is_ok()
            && port != default
    };
    match (host.is_empty(), port) {
        (false, None) => Ok(()),
        (false, Some(port)) if port_ok(port) => Ok(()),
        _ => refuse(),
    }
}

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
    /// Its listener for browsers, if it has one.
    http: Option<Listening>,
}

/// A listener for browsers: what it was started with and what it serves on.
#[derive(Debug, PartialEq, Eq)]
struct Listening {
    port: u64,
    token: String,
    origins: BTreeSet<String>,
    /// Canonical roots.
    projects: BTreeSet<String>,
}

impl Listening {
    fn of(http: &Value) -> Option<Listening> {
        let strings = |name: &str| -> BTreeSet<String> {
            (http[name].as_array().into_iter().flatten())
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect()
        };
        Some(Listening {
            port: http["port"].as_u64()?,
            token: http["token"].as_str()?.to_owned(),
            origins: strings("origins"),
            projects: strings("projects"),
        })
    }

    /// The human form's line that names it.
    fn line(&self) -> String {
        format!(
            "It listens for browsers at http://127.0.0.1:{} with the token {}.\n",
            self.port, self.token
        )
    }
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
    daemon::send_frame(stream, message.to_string().as_bytes())?;
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
        http: Listening::of(&answer["http"]),
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
            if let Some(http) = &running.http {
                text.push_str(&http.line());
            }
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
    let http = found.as_ref().and_then(|running| running.http.as_ref());
    listener_fields(&mut object, http);
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

/// `port` and `token` of a listener, or null for each.
fn listener_fields(object: &mut Object, http: Option<&Listening>) {
    null_or_number(object, "port", http.map(|http| http.port));
    object.optional_string("token", http.map(|http| http.token.as_str()));
}

/// The origins and the canonical roots of the projects of a listener for browsers.
type Allowed = (BTreeSet<String>, BTreeSet<String>);

/// The listener that `daemon start` asks for, if any: its origins and the canonical roots of its
/// projects, which `--allow-project` names from the working directory.
fn wanted(context: &Context, args: &Args) -> Result<Option<Allowed>, Failure> {
    let http = args.value(HTTP.name).is_some();
    let origins: BTreeSet<String> = args.values(ALLOW_ORIGIN.name).map(str::to_owned).collect();
    let paths: Vec<&str> = args.values(ALLOW_PROJECT.name).collect();
    match (http, origins.is_empty(), paths.is_empty()) {
        (false, true, true) => return Ok(None),
        (false, _, _) => {
            return Err(Failure::rejected(
                "`--allow-origin` and `--allow-project` need `--http`".to_owned(),
            ));
        }
        (true, false, false) => {}
        (true, _, _) => {
            return Err(Failure::rejected(
                "`--http` needs `--allow-origin <origin>` and `--allow-project <dir>`, each at \
                 least once"
                    .to_owned(),
            ));
        }
    }
    let cwd = context.cwd()?;
    let mut projects = BTreeSet::new();
    for path in paths {
        let refuse = |why: String| Failure::rejected(format!("`--allow-project`: `{path}` {why}"));
        let root = fs::canonicalize(cwd.join(path))
            .map_err(|error| refuse(format!("cannot be resolved: {error}")))?;
        if !root.is_dir() {
            return Err(refuse("names no directory".to_owned()));
        }
        let root = root
            .to_str()
            .ok_or_else(|| refuse("is not UTF-8".to_owned()))?;
        projects.insert(root.to_owned());
    }
    Ok(Some((origins, projects)))
}

/// Starts the daemon unless one of this build runs. When another process holds the lock, as a
/// daemon does while it starts, waits until a daemon answers. The daemon opens the session of
/// the project that `--project` names or that holds the working directory, if any. With `--http`
/// it also listens for browsers; a daemon that runs already must listen with the same origins
/// and projects.
fn start(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let wanted = wanted(context, args)?;
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
    let (started, pid, http) = loop {
        if let Some(running) = running(&runtime, &build).map_err(io_failure)? {
            break (false, running.pid, running.http);
        }
        if daemon::try_lock(&lock).map_err(io_failure)? {
            let browsers = match &wanted {
                Some((origins, projects)) => {
                    let caller = Caller {
                        // A relative `TMPDIR` names a directory of the caller's.
                        temp_dir: context.cwd()?.join(&context.caller.temp_dir),
                        ..context.caller.clone()
                    };
                    let projects = projects.iter().map(Into::into).collect();
                    let origins = origins.iter().cloned().collect();
                    Some(server::bind(origins, projects, caller).map_err(|error| {
                        Failure::new("io-error", format!("cannot listen for browsers: {error}"))
                    })?)
                }
                None => None,
            };
            let http = browsers.as_ref().map(|browsers| Listening {
                port: u64::from(browsers.access.port),
                token: browsers.access.token.clone(),
                origins: browsers.access.origins.iter().cloned().collect(),
                projects: (browsers.access.projects.iter())
                    .map(|root| root.to_string_lossy().into_owned())
                    .collect(),
            });
            let pid = server::launch(&runtime, &build, lock, project.clone(), browsers)
                .map_err(|message| Failure::new("io-error", message))?;
            break (true, u64::from(pid), http);
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
    if let Some((origins, projects)) = &wanted
        && !started
    {
        let other = match &http {
            None => "does not listen for browsers",
            Some(http) if (&http.origins, &http.projects) == (origins, projects) => "",
            Some(_) => "listens for browsers with other origins or projects",
        };
        if !other.is_empty() {
            return Err(Failure::new(
                "daemon-running",
                format!(
                    "the daemon of this build already runs as process {pid} and {other}; it is \
                     left as it is: stop it with `mara daemon stop` first"
                ),
            ));
        }
    }
    let mut human = if started {
        format!("Started the daemon of this build as process {pid}.\n")
    } else {
        format!("The daemon of this build already runs as process {pid}.\n")
    };
    if let Some(http) = &http {
        human.push_str(&http.line());
    }
    let mut json = String::new();
    let mut object = Object::new(&mut json);
    object.number("format_version", FORMAT_VERSION);
    object.bool("started", started);
    object.number("pid", pid);
    listener_fields(&mut object, http.as_ref());
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

#[cfg(test)]
mod tests {
    use super::origin;

    #[test]
    fn accepts_origins_only_in_a_form_browsers_send() {
        for good in [
            "http://127.0.0.1:5173",
            "https://example.com",
            "http://localhost",
            "http://example.com.",
            "http://example.com.:8080",
            "http://[::1]:8080",
            "https://[2001:db8::1]",
            "https://a-b.example.com:8443",
            "http://xn--bcher-kva.example",
            "http://1:65535",
        ] {
            assert_eq!(origin(good), Ok(()), "{good}");
        }
        for bad in [
            // Scheme.
            "null",
            "",
            "*",
            "file:///",
            "ws://example.com",
            "HTTP://example.com",
            "127.0.0.1:5173",
            // No host.
            "http://",
            "http://:8080",
            // Forbidden characters after the scheme.
            "http://127.0.0.1:5173/",
            "http://127.0.0.1:5173/path",
            "http://example.com?x",
            "http://example.com#x",
            "http://user@example.com",
            "http://*.example.com",
            "http://exa mple.com",
            "http://example.com\t",
            "http://example.com\u{7f}",
            "http://Example.com",
            // Ports.
            "http://example.com:",
            "http://example.com:0",
            "http://example.com:08080",
            "http://example.com:65536",
            "http://example.com:123456",
            "http://example.com:+80",
            "http://example.com:80",
            "https://example.com:443",
            "http://a:b:80",
            // Brackets.
            "http://[::1",
            "http://[]:80",
            "http://[abc]",
            "http://[::1]x",
            "http://[::1]:80",
        ] {
            assert!(origin(bad).is_err(), "{bad:?}");
        }
    }
}
