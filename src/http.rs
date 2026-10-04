//! Opt-in loopback transport. Authentication and exact-root authority precede RPC dispatch.
use crate::{
    NativeSessions,
    protocol::{RpcError, RpcSession},
};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

const MAX_BODY: usize = 1024 * 1024;
const MAX_HEADERS: usize = 16 * 1024;
// Bound unauthenticated readers as well as dispatched operations.
const MAX_PEERS: usize = 16;
const ACCEPT_BACKOFF: Duration = Duration::from_millis(10);

/// Startup-only policy. No HTTP operation can inspect or change this policy.
#[derive(Clone, Debug)]
pub struct HttpOptions {
    pub bind: SocketAddr,
    pub origins: Vec<String>,
    pub projects: Vec<PathBuf>,
    pub initialization_destinations: Vec<PathBuf>,
}
struct Policy {
    token: String,
    origins: HashSet<String>,
    projects: HashSet<PathBuf>,
    destinations: HashSet<PathBuf>,
}
// @mara implements REQ-LOCAL-DAEMON-HTTP
// @mara implements DES-LOCAL-DAEMON-PROTOCOL
pub struct HttpServer {
    listener: TcpListener,
    policy: Arc<Policy>,
}
impl HttpServer {
    pub fn bind(options: HttpOptions) -> io::Result<Self> {
        if !options.bind.ip().is_loopback() {
            return Err(io::Error::other(
                "HTTP must bind a numeric loopback address",
            ));
        }
        if options.origins.is_empty() || options.origins.iter().any(|o| !valid_origin(o)) {
            return Err(io::Error::other(
                "HTTP requires explicit serialized HTTP(S) origins, without wildcard, path or credentials",
            ));
        }
        let projects = options
            .projects
            .iter()
            .map(|p| resolve_destination(p))
            .collect::<io::Result<HashSet<_>>>()?;
        let destinations = options
            .initialization_destinations
            .iter()
            .map(|p| resolve_destination(p))
            .collect::<io::Result<HashSet<_>>>()?;
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes)
            .map_err(|_| io::Error::other("Cannot generate HTTP session credential"))?;
        let token = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let listener = TcpListener::bind(options.bind)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            policy: Arc::new(Policy {
                token,
                origins: options.origins.into_iter().collect(),
                projects,
                destinations,
            }),
        })
    }
    /// Only the same-user IPC host's deliberate connection-info method receives this value.
    pub(crate) fn connection_info(&self) -> io::Result<Value> {
        Ok(
            json!({"available":true,"endpoint":format!("http://{}/rpc", self.listener.local_addr()?),"token":self.policy.token}),
        )
    }
    pub(crate) fn serve_until(
        self,
        sessions: NativeSessions,
        stop: Arc<AtomicBool>,
    ) -> io::Result<()> {
        let mut workers: Vec<(TcpStream, thread::JoinHandle<io::Result<()>>)> = Vec::new();
        let result = loop {
            let mut i = 0;
            while i < workers.len() {
                if workers[i].1.is_finished() {
                    let (_, h) = workers.swap_remove(i);
                    let _ = h.join();
                } else {
                    i += 1;
                }
            }
            if stop.load(Ordering::Acquire) {
                break Ok(());
            }
            if workers.len() >= MAX_PEERS {
                thread::sleep(ACCEPT_BACKOFF);
                continue;
            }
            match self.listener.accept() {
                Ok((stream, peer)) => {
                    if !peer.ip().is_loopback() {
                        continue;
                    }
                    let wake = match stream.try_clone() {
                        Ok(wake) => wake,
                        Err(e) if resource_pressure(&e) => {
                            thread::sleep(ACCEPT_BACKOFF);
                            continue;
                        }
                        Err(e) => break Err(e),
                    };
                    let policy = self.policy.clone();
                    let sessions = sessions.clone();
                    let stop = stop.clone();
                    match thread::Builder::new()
                        .name("mara-http".into())
                        .spawn(move || serve_request(stream, policy, sessions, stop))
                    {
                        Ok(h) => workers.push((wake, h)),
                        Err(e) if resource_pressure(&e) => thread::sleep(ACCEPT_BACKOFF),
                        Err(e) => break Err(e),
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock || resource_pressure(&e) => {
                    thread::sleep(ACCEPT_BACKOFF)
                }
                Err(e)
                    if e.kind() == io::ErrorKind::Interrupted
                        || e.kind() == io::ErrorKind::ConnectionAborted
                        || e.kind() == io::ErrorKind::ConnectionReset => {}
                Err(e) => break Err(e),
            }
        };
        for (stream, _) in &workers {
            let _ = stream.shutdown(std::net::Shutdown::Read);
        }
        for (_, h) in workers {
            let _ = h.join();
        }
        result
    }
}
pub(crate) fn resource_pressure(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(libc::EMFILE | libc::ENFILE | libc::ENOMEM | libc::ENOBUFS | libc::EAGAIN)
    )
}
fn valid_origin(origin: &str) -> bool {
    let Ok(url) = url::Url::parse(origin) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && !origin.contains('*')
        && url.username().is_empty()
        && url.password().is_none()
        && url.origin().ascii_serialization() == origin
}

/// Resolve existing components (including real symlinks) without reading project files.
/// Startup authority and initialization may address absent suffixes. Reject parent traversal in an
/// absent suffix: unlike lexical collapsing it must not invent filesystem identity.
fn resolve_destination(path: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(io::Error::other("HTTP destinations must be absolute"));
    }
    let mut root = PathBuf::new();
    let mut missing = false;
    for component in path.components() {
        match component {
            Component::RootDir => root.push(component),
            Component::CurDir => {}
            Component::ParentDir if missing => {
                return Err(io::Error::other(
                    "Cannot resolve parent traversal below an absent directory",
                ));
            }
            Component::ParentDir => {
                root.pop();
            }
            Component::Normal(name) => {
                root.push(name);
                if !missing {
                    match fs::canonicalize(&root) {
                        Ok(resolved) if resolved.is_dir() => root = resolved,
                        Ok(_) => {
                            return Err(io::Error::other(
                                "HTTP destination components must be directories",
                            ));
                        }
                        Err(e) if e.kind() == io::ErrorKind::NotFound => {
                            // A dangling symlink is not an absent initialization destination.
                            if fs::symlink_metadata(&root).is_ok() {
                                return Err(io::Error::other(
                                    "HTTP destination has an unresolved symlink",
                                ));
                            }
                            missing = true;
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            _ => return Err(io::Error::other("Unsupported HTTP destination")),
        }
    }
    Ok(root)
}

fn serve_request(
    mut stream: TcpStream,
    policy: Arc<Policy>,
    sessions: NativeSessions,
    stop: Arc<AtomicBool>,
) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if bytes.len() >= MAX_HEADERS {
            return respond(&mut stream, 400, None, None);
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(&chunk[..n]);
    };
    if header_end > MAX_HEADERS {
        return respond(&mut stream, 400, None, None);
    }
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut parsed = httparse::Request::new(&mut headers);
    if !matches!(
        parsed.parse(&bytes[..header_end]),
        Ok(httparse::Status::Complete(_))
    ) {
        return respond(&mut stream, 400, None, None);
    }
    let header = |name: &str| -> Option<&str> {
        let mut values = parsed
            .headers
            .iter()
            .filter(|h| h.name.eq_ignore_ascii_case(name));
        let value = std::str::from_utf8(values.next()?.value).ok()?;
        if values.next().is_some() {
            return None;
        }
        Some(value)
    };
    // CORS preflight carries no bearer credential and never dispatches RPC or reads roots.
    let origin = header("Origin").filter(|o| policy.origins.contains(*o));
    let Some(origin) = origin else {
        return respond(&mut stream, 403, None, None);
    };
    if parsed.path != Some("/rpc") {
        return respond(&mut stream, 404, Some(origin), None);
    }
    if parsed.method == Some("OPTIONS") {
        let allowed = header("Access-Control-Request-Method") == Some("POST")
            && header("Access-Control-Request-Headers").is_none_or(|s| {
                s.split(',').all(|h| {
                    [
                        "authorization",
                        "content-type",
                        "x-mara-protocol-version",
                        "x-mara-application-version",
                    ]
                    .contains(&h.trim().to_ascii_lowercase().as_str())
                })
            });
        return respond(
            &mut stream,
            if allowed { 204 } else { 403 },
            Some(origin),
            None,
        );
    }
    if parsed.method != Some("POST") {
        return respond(&mut stream, 405, Some(origin), None);
    }
    if header("Authorization") != Some(format!("Bearer {}", policy.token).as_str()) {
        return respond(&mut stream, 401, Some(origin), None);
    }
    if header("Content-Type") != Some("application/json")
        || parsed
            .headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("Transfer-Encoding"))
    {
        return respond(&mut stream, 400, Some(origin), None);
    }
    let Some(length) = header("Content-Length")
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n <= MAX_BODY)
    else {
        return respond(&mut stream, 413, Some(origin), None);
    };
    let protocol = header("X-Mara-Protocol-Version").and_then(|s| s.parse::<u8>().ok());
    let application = header("X-Mara-Application-Version").map(str::to_owned);
    let origin = origin.to_owned();
    while bytes.len() - header_end < length {
        let remaining = length - (bytes.len() - header_end);
        let n = stream.read(&mut chunk[..remaining.min(4096)])?;
        if n == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    let mut request: Value = match serde_json::from_slice(&bytes[header_end..header_end + length]) {
        Ok(value) => value,
        Err(_) => {
            return respond(
                &mut stream,
                200,
                Some(&origin),
                Some(
                    RpcError::new(-32700, "Parse error: send one UTF-8 JSON object")
                        .response(Value::Null),
                ),
            );
        }
    };
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let mut rpc = RpcSession::new(sessions);
    // Request-scoped compatibility survives browser HTTP connection pooling without
    // adding an opaque session store. The same negotiator supplies version errors.
    let negotiation = rpc.handle(json!({"jsonrpc":"2.0","id":null,"method":"mara.negotiate","params":{
        "protocol_version":protocol.unwrap_or(0),"application_version":application.clone().unwrap_or_default()}})).expect("negotiation response");
    if protocol.is_none() || application.is_none() {
        return respond(
            &mut stream,
            200,
            Some(&origin),
            Some(
                RpcError::domain(
                    -32001,
                    "negotiation_required",
                    "Supply X-Mara-Protocol-Version and X-Mara-Application-Version before dispatch",
                )
                .response(id),
            ),
        );
    }
    if let Some(error) = negotiation.get("error") {
        return respond(
            &mut stream,
            200,
            Some(&origin),
            Some(json!({"jsonrpc":"2.0","id":id,"error":error})),
        );
    }
    if stop.load(Ordering::Acquire) {
        return respond(&mut stream, 503, Some(&origin), None);
    }
    if let Some(method) = request.get("method").and_then(Value::as_str)
        && crate::protocol::OPERATIONS.contains(&method)
    {
        let target = request
            .pointer("/params/project")
            .and_then(Value::as_str)
            .map(Path::new);
        let init = method == "project_init";
        let resolved = target.and_then(|p| {
            if !p.is_absolute() {
                return None;
            }
            if init {
                resolve_destination(p).ok()
            } else {
                fs::canonicalize(p).ok()
            }
        });
        let allowed = if init {
            &policy.destinations
        } else {
            &policy.projects
        };
        let Some(root) = resolved.filter(|p| allowed.contains(p)) else {
            return respond(&mut stream, 403, Some(&origin), None);
        };
        // JSON path parameters cannot encode non-UTF-8 resolved host paths. Fail
        // before dispatch rather than panicking or switching back to alias spelling.
        let Some(root) = root.to_str() else {
            return respond(
                &mut stream,
                200,
                Some(&origin),
                Some(RpcError::new(-32602, "Invalid params: resolved HTTP project root must be UTF-8; no operation was dispatched").response(id)),
            );
        };
        // Dispatch resolved identity, not caller spelling or a retargetable alias.
        request["params"]["project"] = json!(root);
    }
    let result = rpc.handle(request);
    respond(
        &mut stream,
        if result.is_some() { 200 } else { 204 },
        Some(&origin),
        result,
    )
}
fn respond(
    stream: &mut TcpStream,
    status: u16,
    origin: Option<&str>,
    body: Option<Value>,
) -> io::Result<()> {
    let body = body.map(|v| v.to_string()).unwrap_or_default();
    write!(
        stream,
        "HTTP/1.1 {status} Response\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        body.len()
    )?;
    if let Some(origin) = origin {
        write!(
            stream,
            "Access-Control-Allow-Origin: {origin}\r\nVary: Origin\r\nAccess-Control-Allow-Methods: POST\r\nAccess-Control-Allow-Headers: Authorization, Content-Type, X-Mara-Protocol-Version, X-Mara-Application-Version\r\n"
        )?;
    }
    write!(stream, "\r\n{body}")
}
