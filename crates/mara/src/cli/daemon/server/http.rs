//! The daemon's listener for browsers (DES-BROWSER-LISTENER and DES-BROWSER-PROTOCOL in
//! `docs/daemon/index.mara.md`): HTTP/1.1 on a port of `127.0.0.1` that the system assigns, one
//! request per connection, its head read by `httparse` (ADR-HTTPARSE-LISTENER).
//!
//! `daemon start --http` binds the socket and draws the token before it forks the daemon
//! ([`bind`]), which accepts each connection on a thread of its own ([`accept`]). A request is
//! checked in this order, and refused at the first check it fails, before anything later is read:
//! its origin and its token, its method and path, its body, then its project, which must be the
//! canonical root of an allowed project. Only then is the operation looked up and run, as an
//! operation request of a native client run in that project's root, with the umask and temporary
//! directory of the process that started the daemon.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::os::fd::AsRawFd;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use mara::caller::Caller;
use mara::daemon::{MAX_FRAME, PROTOCOL_VERSION};
use mara::hooks::{self, Hook};
use mara_engine::work::WorkCounts;
use serde_json::{Map, Value, json};

use super::Daemon;
use crate::cli::{Failure, Reply, client};

/// Version of the requests and answers that browsers exchange with the listener.
pub const HTTP_VERSION: u64 = 1;
/// The one path the listener serves.
const PATH: &str = "/operation";
/// The fields of a request's body.
const FIELDS: &[&str] = &[
    "version",
    "request",
    "project",
    "words",
    "arguments",
    "options",
    "supplied",
    "json",
];
/// The longest head of a request: its request line and headers.
const MAX_HEAD: usize = 64 << 10;
/// The most headers a request may have.
const MAX_HEADERS: usize = 64;
/// How long the listener waits for each read or write of a connection.
const IO_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the listener reads what a client still sends after the response, so that closing
/// the connection does not reset it before the client has read the response.
const LINGER: Duration = Duration::from_secs(2);
/// The token's bytes from the system's random generator: 256 bits.
const TOKEN_BYTES: usize = 32;

/// The listening socket, and what the daemon checks each request against.
pub struct Listener {
    pub socket: TcpListener,
    pub access: Access,
}

/// The listener's port and token, the origins and projects it allows, and the client values of
/// the process that started it, which every request it serves uses.
pub struct Access {
    pub port: u16,
    pub token: String,
    pub origins: Vec<String>,
    /// Canonical roots.
    pub projects: Vec<PathBuf>,
    pub caller: Caller,
}

impl Access {
    /// What `status` tells a native client of this build, which may print the token.
    pub fn json(&self) -> Value {
        let projects: Vec<String> = (self.projects.iter())
            .map(|root| root.to_string_lossy().into_owned())
            .collect();
        json!({
            "port": self.port,
            "token": self.token,
            "origins": self.origins,
            "projects": projects,
        })
    }
}

// @mara implements REQ-BROWSER-ACCESS-AUTHORIZED
/// Binds a socket on `127.0.0.1` at a port the system assigns and draws a new token. Nothing of
/// either is written to a file.
pub fn bind(origins: Vec<String>, projects: Vec<PathBuf>, caller: Caller) -> io::Result<Listener> {
    let socket = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let port = socket.local_addr()?.port();
    let mut bytes = [0u8; TOKEN_BYTES];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let token = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(Listener {
        socket,
        access: Access {
            port,
            token,
            origins,
            projects,
            caller,
        },
    })
}

/// How often the listener looks whether the daemon stops while no browser connects.
const POLL: Duration = Duration::from_millis(100);

/// Accepts the connections of browsers, each on a thread of its own, until the daemon stops;
/// then closes the socket, so that no connection is accepted while the daemon ends.
pub(super) fn accept(daemon: &Arc<Daemon>, socket: TcpListener) {
    // Ready connections only: one that a client reset after `poll` would block `accept`.
    if socket.set_nonblocking(true).is_err() {
        return;
    }
    let mut ready = libc::pollfd {
        fd: socket.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let timeout = libc::c_int::try_from(POLL.as_millis()).unwrap_or(100);
    while !daemon.state().stopping {
        // SAFETY: `ready` is one valid `pollfd`, whose descriptor `socket` keeps open.
        if unsafe { libc::poll(&mut ready, 1, timeout) } <= 0 {
            // No connection, or a signal.
            continue;
        }
        let stream = match socket.accept() {
            Ok((stream, _)) => stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
            Err(_) => {
                // Out of descriptors, for instance: let connections end before the next.
                thread::sleep(Duration::from_millis(10));
                continue;
            }
        };
        // A connection may inherit the socket's mode, as on macOS.
        if stream.set_nonblocking(false).is_err() {
            continue;
        }
        let daemon = Arc::clone(daemon);
        let _ = thread::Builder::new()
            .name("mara-http".to_owned())
            .spawn(move || exchange(&daemon, stream));
    }
}

/// The head of a request: what the listener reads of it.
struct Head {
    method: String,
    target: String,
    /// The values of every `Origin` header.
    origins: Vec<Vec<u8>>,
    /// The values of every `Authorization` header.
    authorizations: Vec<Vec<u8>>,
    /// The values of every `Content-Length` header.
    lengths: Vec<Vec<u8>>,
    /// Whether it has a `Transfer-Encoding` header.
    encoded: bool,
}

/// A response: its status, its JSON body, if any, and headers besides those of every response.
struct Response {
    status: u16,
    body: Option<Value>,
    headers: Vec<(&'static str, &'static str)>,
}

impl Response {
    fn json(status: u16, body: Value) -> Response {
        Response {
            status,
            body: Some(body),
            headers: Vec::new(),
        }
    }

    /// A refusal: `{"version": 1, "error": <code>, "message": <message>}`.
    fn refusal(status: u16, code: &str, message: &str) -> Response {
        let body = json!({"version": HTTP_VERSION, "error": code, "message": message});
        Response::json(status, body)
    }

    fn malformed(message: &str) -> Response {
        Response::refusal(400, "malformed-request", message)
    }
}

/// Answers the one request of a connection, then closes it.
fn exchange(daemon: &Daemon, mut stream: TcpStream) {
    let Some(access) = &daemon.browsers else {
        return;
    };
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let (head, read) = match read_head(&mut stream) {
        Ok(found) => found,
        Err(None) => return,
        Err(Some(message)) => {
            send(&mut stream, None, &Response::malformed(&message));
            return linger(stream);
        }
    };
    let origin = allowed_origin(access, &head);
    // A panic is a defect: the connection closes without an answer.
    let response = panic::catch_unwind(AssertUnwindSafe(|| {
        respond(daemon, access, &head, origin, read, &mut stream)
    }));
    match response {
        Ok(Some(response)) => {
            send(&mut stream, origin, &response);
            linger(stream);
        }
        Ok(None) => linger(stream),
        Err(_) => {}
    }
}

/// The response to a request, or `None` when nothing more is sent: an operation's answer, which
/// [`run`] sends itself, or none when the daemon stopped while it ran a read.
fn respond(
    daemon: &Daemon,
    access: &Access,
    head: &Head,
    origin: Option<&str>,
    read: Vec<u8>,
    stream: &mut TcpStream,
) -> Option<Response> {
    if head.method == "OPTIONS" {
        return Some(preflight(origin));
    }
    // Origin and token, before anything else of the request is read.
    if origin.is_none() {
        return Some(Response::refusal(
            403,
            "refused",
            "the request comes from no origin that this daemon allows",
        ));
    }
    if !authorized(access, head) {
        return Some(Response::refusal(
            401,
            "refused",
            "the request carries no `Authorization: Bearer` header with this daemon's token",
        ));
    }
    if head.method != "POST" || head.target != PATH {
        return Some(Response::refusal(
            404,
            "unknown-request",
            &format!("the daemon serves browsers only `POST {PATH}`"),
        ));
    }
    let length = match length(head) {
        Ok(length) => length,
        Err(message) => return Some(Response::malformed(message)),
    };
    let body = match read_body(stream, read, length) {
        Ok(body) => body,
        Err(_) => {
            return Some(Response::malformed(
                "the body is not as long as its `Content-Length`",
            ));
        }
    };
    let body = match parse(&body) {
        Ok(body) => body,
        Err(message) => return Some(Response::malformed(&message)),
    };
    // The project, before anything in it is read.
    let Some(root) = allowed_project(access, body.get("project").unwrap_or(&Value::Null)) else {
        return Some(Response::refusal(
            403,
            "refused",
            "the request names no project root that this daemon allows",
        ));
    };
    let request = match operation(daemon, access, &body, &root) {
        Ok(Ok(request)) => request,
        Ok(Err(unavailable)) => return Some(unavailable),
        Err(message) => return Some(Response::malformed(&message)),
    };
    run(daemon, &request, stream, origin)
}

/// The answer to a preflight: the method and headers of a request, allowed to an allowed origin
/// without a token.
fn preflight(origin: Option<&str>) -> Response {
    match origin {
        Some(_) => Response {
            status: 204,
            body: None,
            headers: vec![
                ("Access-Control-Allow-Methods", "POST"),
                (
                    "Access-Control-Allow-Headers",
                    "Authorization, Content-Type",
                ),
                ("Access-Control-Max-Age", "600"),
            ],
        },
        None => Response::refusal(
            403,
            "refused",
            "the request comes from no origin that this daemon allows",
        ),
    }
}

// @mara implements REQ-BROWSER-ACCESS-AUTHORIZED
/// The request's origin, when it has exactly one `Origin` header and it is an allowed origin
/// byte for byte.
fn allowed_origin<'a>(access: &'a Access, head: &Head) -> Option<&'a str> {
    let [origin] = &head.origins[..] else {
        return None;
    };
    (access.origins.iter())
        .find(|allowed| allowed.as_bytes() == &origin[..])
        .map(String::as_str)
}

// @mara implements REQ-BROWSER-ACCESS-AUTHORIZED
/// Whether the request has exactly one `Authorization` header, `Bearer <token>`, with this
/// daemon's token, compared in constant time.
fn authorized(access: &Access, head: &Head) -> bool {
    let [authorization] = &head.authorizations[..] else {
        return false;
    };
    authorization
        .strip_prefix(b"Bearer ")
        .is_some_and(|token| same(token, access.token.as_bytes()))
}

/// Whether `given` equals `token`, in a time that depends only on their lengths: every byte is
/// compared, whichever differs.
fn same(given: &[u8], token: &[u8]) -> bool {
    if given.len() != token.len() {
        return false;
    }
    let differences = (given.iter().zip(token)).fold(0u8, |acc, (a, b)| acc | (a ^ b));
    std::hint::black_box(differences) == 0
}

// @mara implements REQ-BROWSER-ACCESS-AUTHORIZED
/// The canonical root of the project the request names, if it is one of the allowed projects'.
/// Every path that is not one, absent, relative, unresolvable or another directory, gives `None`
/// alike.
fn allowed_project(access: &Access, project: &Value) -> Option<PathBuf> {
    let path = Path::new(project.as_str()?);
    if !path.is_absolute() {
        return None;
    }
    let root = fs::canonicalize(path).ok()?;
    access.projects.contains(&root).then_some(root)
}

/// The body's length, which `Content-Length` gives; no other framing is read.
fn length(head: &Head) -> Result<usize, &'static str> {
    if head.encoded {
        return Err(
            "the listener reads no `Transfer-Encoding`: send the body with `Content-Length`",
        );
    }
    let mut lengths = head.lengths.iter().map(|value| {
        (std::str::from_utf8(value).ok())
            .filter(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|text| text.parse::<usize>().ok())
    });
    let length = match lengths.next() {
        None => return Err("the request has no `Content-Length`"),
        Some(length) => length,
    };
    match length {
        Some(length) if lengths.all(|other| other == Some(length)) => match length <= MAX_FRAME {
            true => Ok(length),
            false => Err("the body passes 64 MiB"),
        },
        _ => Err("the request's `Content-Length` is no single length"),
    }
}

/// The body: what was read with the head, then the rest of `length` bytes.
fn read_body(stream: &mut TcpStream, mut body: Vec<u8>, length: usize) -> io::Result<Vec<u8>> {
    if body.len() > length {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let start = body.len();
    body.resize(length, 0);
    stream.read_exact(&mut body[start..])?;
    Ok(body)
}

/// The body as a JSON object of the protocol's version, with no field it does not know.
fn parse(body: &[u8]) -> Result<Map<String, Value>, String> {
    let body: Value =
        serde_json::from_slice(body).map_err(|error| format!("the body is not JSON: {error}"))?;
    let Value::Object(body) = body else {
        return Err("the body is no JSON object".to_owned());
    };
    if let Some(field) = body.keys().find(|key| !FIELDS.contains(&key.as_str())) {
        return Err(format!("`{field}` is no field of a request"));
    }
    if body.get("version").and_then(Value::as_u64) != Some(HTTP_VERSION) {
        return Err(format!("the request's `version` is not {HTTP_VERSION}"));
    }
    if body.get("request").and_then(Value::as_str) != Some("operation") {
        return Err("the request's `request` is not `operation`".to_owned());
    }
    Ok(body)
}

/// The native operation request that runs the browser's request in `root`, or the answer of an
/// operation that a browser cannot run, or why the request is malformed.
fn operation(
    daemon: &Daemon,
    access: &Access,
    body: &Map<String, Value>,
    root: &Path,
) -> Result<Result<Value, Response>, String> {
    let words: Vec<&str> = (body.get("words").and_then(Value::as_array))
        .ok_or("`words` is no array")?
        .iter()
        .map(|word| word.as_str().ok_or("a word is no string"))
        .collect::<Result<_, _>>()?;
    let json = match body.get("json") {
        None => true,
        Some(json) => json.as_bool().ok_or("`json` is no boolean")?,
    };
    match client::served_to_browsers(&words) {
        None => return Err(format!("Mara has no operation `{}`", words.join(" "))),
        Some(false) => return Ok(Err(unavailable(daemon, &words, json))),
        Some(true) => {}
    }
    let list = |name: &str| -> Result<Value, String> {
        match body.get(name) {
            None => Ok(json!([])),
            Some(value @ Value::Array(_)) => Ok(value.clone()),
            Some(_) => Err(format!("`{name}` is no array")),
        }
    };
    let options = list("options")?;
    for pair in options.as_array().into_iter().flatten() {
        match pair[0].as_str() {
            Some("project") => {
                return Err("the project is the request's `project`, not an option".to_owned());
            }
            // A file that `--input <name>=@<file>` names would be the daemon's.
            Some("input") => {
                return Err("inputs are values of the request's `supplied`, not options".to_owned());
            }
            _ => {}
        }
    }
    let root = root.to_str().ok_or("the project's root is not UTF-8")?;
    let temp_dir =
        (access.caller.temp_dir.to_str()).ok_or("the daemon's temporary directory is not UTF-8")?;
    Ok(Ok(json!({
        "version": PROTOCOL_VERSION,
        "build": daemon.build,
        "request": "operation",
        "words": words,
        "arguments": list("arguments")?,
        "options": options,
        "supplied": list("supplied")?,
        "project": root,
        "cwd": root,
        "umask": access.caller.umask,
        "temp_dir": temp_dir,
        "json": json,
    })))
}

/// The answer to an operation that a browser cannot run: `unavailable-operation`, with nothing
/// run.
fn unavailable(daemon: &Daemon, words: &[&str], json: bool) -> Response {
    let message = format!(
        "a browser cannot run `{}` through the daemon; run it with the CLI",
        words.join(" ")
    );
    let reply = Reply::new(Err(Failure::new("unavailable-operation", message)), json);
    let work: Map<String, Value> = (WorkCounts::default().fields().into_iter())
        .map(|(name, value)| (name.to_owned(), Value::from(value)))
        .collect();
    Response::json(
        200,
        json!({
            "version": HTTP_VERSION,
            "pid": daemon.pid,
            "text": reply.text,
            "failed": reply.failed,
            "status": reply.status,
            "work": work,
        }),
    )
}

/// A request that [`Daemon::admit`] took, released when dropped: a write is counted until its
/// answer is sent, so that a daemon that stops sends it before it exits.
struct Admitted<'a> {
    daemon: &'a Daemon,
    write: bool,
}

impl Drop for Admitted<'_> {
    fn drop(&mut self) {
        self.daemon.release(self.write);
    }
}

/// Runs the native operation request `request` as the daemon runs a native client's and sends
/// its answer, counted as running until it is sent. Returns only a refusal that is left to send:
/// a daemon that stops refuses the request, and leaves a read it ran meanwhile unanswered.
fn run(
    daemon: &Daemon,
    request: &Value,
    stream: &mut TcpStream,
    origin: Option<&str>,
) -> Option<Response> {
    let write = client::writes(request);
    if !daemon.admit(write) {
        return Some(Response::refusal(503, "stopping", "the daemon is stopping"));
    }
    let _admitted = Admitted { daemon, write };
    let mut answer = daemon.answer(&Ok(request.clone()));
    if !write && daemon.state().stopping {
        return None;
    }
    let response = match answer["error"].as_str() {
        Some(code) => {
            let message = answer["message"].as_str().unwrap_or_default();
            Response::refusal(400, code, message)
        }
        None => {
            answer["version"] = json!(HTTP_VERSION);
            Response::json(200, answer)
        }
    };
    // A test holds the answer here, while the daemon is asked to stop.
    let _ = hooks::pause(Hook::PauseAnswer, "");
    send(stream, origin, &response);
    None
}

/// Reads the head of a request, with what was read past it. `Err(None)` when the connection
/// ended or failed first, `Err(Some(why))` when the head is no HTTP/1.x request head.
fn read_head(stream: &mut impl Read) -> Result<(Head, Vec<u8>), Option<String>> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let count = match stream.read(&mut chunk) {
            Ok(0) => return Err(None),
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(None),
        };
        buffer.extend_from_slice(&chunk[..count]);
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut request = httparse::Request::new(&mut headers);
        match request.parse(&buffer) {
            Ok(httparse::Status::Complete(length)) if length <= MAX_HEAD => {
                let head = Head::of(&request);
                return Ok((head, buffer[length..].to_vec()));
            }
            Ok(httparse::Status::Partial) if buffer.len() < MAX_HEAD => {}
            Ok(_) => {
                return Err(Some("the request's head passes 64 KiB".to_owned()));
            }
            Err(error) => return Err(Some(format!("the request is no HTTP/1.x request: {error}"))),
        }
    }
}

impl Head {
    fn of(request: &httparse::Request) -> Head {
        let mut head = Head {
            method: request.method.unwrap_or_default().to_owned(),
            target: request.path.unwrap_or_default().to_owned(),
            origins: Vec::new(),
            authorizations: Vec::new(),
            lengths: Vec::new(),
            encoded: false,
        };
        for header in request.headers.iter() {
            let value = header.value.to_vec();
            match header.name.to_ascii_lowercase().as_str() {
                "origin" => head.origins.push(value),
                "authorization" => head.authorizations.push(value),
                "content-length" => head.lengths.push(value),
                "transfer-encoding" => head.encoded = true,
                _ => {}
            }
        }
        head
    }
}

/// Writes `response`, with `Access-Control-Allow-Origin` and `Vary: Origin` when the request
/// came from the allowed origin `origin`, and no `Access-Control-*` header otherwise.
fn send(stream: &mut TcpStream, origin: Option<&str>, response: &Response) {
    let reason = match response.status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let mut head = format!(
        "HTTP/1.1 {} {reason}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        response.status
    );
    if let Some(origin) = origin {
        head.push_str(&format!(
            "Access-Control-Allow-Origin: {origin}\r\nVary: Origin\r\n"
        ));
        for (name, value) in &response.headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    let body = response.body.as_ref().map(Value::to_string);
    if let Some(body) = &body {
        head.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    head.push_str("\r\n");
    let _ = stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body.unwrap_or_default().as_bytes()))
        .and_then(|()| stream.flush());
}

/// Ends the connection for writing, then reads what the client still sends for at most
/// [`LINGER`], so that the client reads the whole response before the connection closes.
fn linger(mut stream: TcpStream) {
    let _ = stream.shutdown(Shutdown::Write);
    let deadline = Instant::now() + LINGER;
    let mut sink = [0u8; 4096];
    let mut drained = 0;
    while drained <= MAX_FRAME {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || stream.set_read_timeout(Some(left)).is_err() {
            return;
        }
        match stream.read(&mut sink) {
            Ok(0) | Err(_) => return,
            Ok(count) => drained += count,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::{MAX_HEAD, read_head, same};

    /// Gives one byte, then 4096 bytes per read, so that a read can complete a head past the
    /// limit.
    struct Uneven {
        bytes: Vec<u8>,
        at: usize,
    }

    impl Read for Uneven {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            let count = if self.at == 0 { 1 } else { 4096 };
            let count = count.min(out.len()).min(self.bytes.len() - self.at);
            out[..count].copy_from_slice(&self.bytes[self.at..self.at + count]);
            self.at += count;
            Ok(count)
        }
    }

    /// A head of `length` bytes: a request line, one long header and the blank line.
    fn head(length: usize) -> Vec<u8> {
        let start = "GET / HTTP/1.1\r\nX: ";
        let filler = length - start.len() - 4;
        format!("{start}{}\r\n\r\n", "a".repeat(filler)).into_bytes()
    }

    #[test]
    fn refuses_a_head_past_64_kib_whichever_read_completes_it() {
        for (length, accepted) in [(MAX_HEAD, true), (MAX_HEAD + 1, false)] {
            let mut stream = Uneven {
                bytes: head(length),
                at: 0,
            };
            let read = read_head(&mut stream);
            assert_eq!(read.is_ok(), accepted, "{length}");
            if let Err(why) = read {
                assert_eq!(why.as_deref(), Some("the request's head passes 64 KiB"));
            }
        }
    }

    #[test]
    fn compares_tokens_whole() {
        assert!(same(b"abcd", b"abcd"));
        for other in [&b"abce"[..], b"bbcd", b"abc", b"abcde", b""] {
            assert!(!same(other, b"abcd"), "{other:?}");
        }
    }
}
