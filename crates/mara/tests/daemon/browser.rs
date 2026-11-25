//! VER-BROWSER-ACCESS (`docs/daemon/verification.mara.md`) but its Chromium case: the daemon's
//! listener for browsers (DES-BROWSER-LISTENER and DES-BROWSER-PROTOCOL in
//! `docs/daemon/index.mara.md`), reached by an HTTP/1.1 client of the test at `127.0.0.1`.
//!
//! Each listening daemon is started from the test's directory, outside every project, allowing
//! the origins [`ORIGIN`] and [`SECOND`] and the projects A and C; B is a third project. The
//! documents of each project hold a word of their own ([`WORDS`]). A refusal is compared with the
//! same request naming a path where nothing exists: the same status and body, a 4xx status, no
//! project's word, the CORS headers of an accepted response exactly when its one `Origin` is
//! allowed, no new session, `served` unchanged and every file of A, B and C byte-identical.

use std::collections::BTreeMap;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

use super::{CONFIG, SCHEMA, Test, mara};

/// The origin of an accepted request.
const ORIGIN: &str = "http://127.0.0.1:5173";
/// The other allowed origin.
const SECOND: &str = "http://127.0.0.1:5175";
/// An origin the daemon does not allow.
const THIRD: &str = "http://127.0.0.1:5174";
/// The word of each project's documents: A, B and C.
const WORDS: [&str; 3] = ["Alpha", "Bravo", "Charlie"];

/// An HTTP response as the test's client read it.
#[derive(Debug, PartialEq, Eq)]
struct Http {
    status: u16,
    /// Names in lowercase.
    headers: Vec<(String, String)>,
    body: String,
}

impl Http {
    fn header(&self, name: &str) -> Option<&str> {
        (self.headers.iter())
            .find(|(header, _)| header == name)
            .map(|(_, value)| value.as_str())
    }

    /// Its `Access-Control-*` headers.
    fn access_control(&self) -> Vec<&(String, String)> {
        (self.headers.iter())
            .filter(|(name, _)| name.starts_with("access-control-"))
            .collect()
    }

    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|error| panic!("{error}: {self:?}"))
    }

    /// The result the operation printed, as JSON.
    fn result(&self) -> Value {
        assert_eq!(self.status, 200, "{self:?}");
        let text = self.json()["text"].as_str().expect("a text").to_owned();
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}: {text}"))
    }

    /// Requires the CORS headers of an accepted response to `origin`, and no others.
    fn allows(&self, origin: &str) {
        assert_eq!(
            self.header("access-control-allow-origin"),
            Some(origin),
            "{self:?}"
        );
        assert_eq!(self.header("vary"), Some("Origin"), "{self:?}");
        assert_eq!(self.access_control().len(), 1, "{self:?}");
    }
}

/// Sends `request` to `127.0.0.1:<port>` and reads the response until the daemon closes the
/// connection.
fn exchange(port: u16, request: &[u8]) -> Http {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the listener");
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    stream.write_all(request).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).expect("read the response");
    let text = String::from_utf8(bytes).expect("a UTF-8 response");
    let (head, body) = text
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("no response head to {}", String::from_utf8_lossy(request)));
    let mut lines = head.split("\r\n");
    let status = lines.next().unwrap();
    assert!(status.starts_with("HTTP/1.1 "), "{status}");
    let status = status[9..12].parse().unwrap();
    let headers = lines
        .map(|line| {
            let (name, value) = line.split_once(": ").expect("a header");
            (name.to_ascii_lowercase(), value.to_owned())
        })
        .collect();
    Http {
        status,
        headers,
        body: body.to_owned(),
    }
}

/// An HTTP/1.1 request to the listener, built part by part.
#[derive(Clone)]
struct Request {
    method: &'static str,
    target: String,
    /// Headers besides `Host` and `Content-Length`.
    headers: Vec<(&'static str, String)>,
    body: String,
}

impl Request {
    fn bytes(&self, port: u16) -> Vec<u8> {
        let mut out = format!(
            "{} {} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n",
            self.method, self.target
        );
        for (name, value) in &self.headers {
            out.push_str(&format!("{name}: {value}\r\n"));
        }
        out.push_str(&format!("Content-Length: {}\r\n\r\n", self.body.len()));
        out.push_str(&self.body);
        out.into_bytes()
    }

    /// Without the headers named `name`.
    fn without(mut self, name: &str) -> Request {
        self.headers.retain(|(header, _)| *header != name);
        self
    }

    /// With the header `name: value` added.
    fn with(mut self, name: &'static str, value: &str) -> Request {
        self.headers.push((name, value.to_owned()));
        self
    }

    /// With the one origin `origin`.
    fn from(self, origin: &str) -> Request {
        self.without("Origin").with("Origin", origin)
    }

    /// With the body's field `name` set to `value`, or removed when it is null.
    fn field(mut self, name: &str, value: Value) -> Request {
        let mut body: Value = serde_json::from_str(&self.body).unwrap();
        match value {
            Value::Null => drop(body.as_object_mut().unwrap().remove(name)),
            value => body[name] = value,
        }
        self.body = body.to_string();
        self
    }
}

/// A test with projects A, B and C and a daemon that listens for browsers.
struct Browser {
    test: Test,
    /// The canonical roots of A, B and C.
    projects: [PathBuf; 3],
    pid: u32,
    port: u16,
    token: String,
}

impl Browser {
    fn new() -> Browser {
        Browser::with_env(&[])
    }

    /// Starts the listening daemon with `env`.
    fn with_env(env: &[(&str, &str)]) -> Browser {
        Browser::in_test(Test::new(), env)
    }

    /// Starts the listening daemon of `test` with `env`.
    fn in_test(test: Test, env: &[(&str, &str)]) -> Browser {
        let projects = WORDS.map(|word| project(&test, &word.to_ascii_lowercase(), word));
        let mut browser = Browser {
            test,
            projects,
            pid: 0,
            port: 0,
            token: String::new(),
        };
        let started = browser.start(&browser.options(), env);
        assert_eq!(started.status.code(), Some(0), "{started:?}");
        let json = started.json();
        assert_eq!(json["started"], true, "{json}");
        browser.pid = u32::try_from(json["pid"].as_u64().unwrap()).unwrap();
        browser.port = u16::try_from(json["port"].as_u64().expect("a port")).unwrap();
        browser.token = json["token"].as_str().expect("a token").to_owned();
        browser
    }

    fn a(&self) -> &Path {
        &self.projects[0]
    }

    fn b(&self) -> &Path {
        &self.projects[1]
    }

    fn c(&self) -> &Path {
        &self.projects[2]
    }

    /// The listener options of the listening daemon.
    fn options(&self) -> Vec<String> {
        let mut options = vec!["--http".to_owned()];
        for origin in [ORIGIN, SECOND] {
            options.extend(["--allow-origin".to_owned(), origin.to_owned()]);
        }
        for root in [self.a(), self.c()] {
            options.extend(["--allow-project".to_owned(), root.display().to_string()]);
        }
        options
    }

    /// `mara --format json daemon start <options>` in the test's directory.
    fn start(&self, options: &[String], env: &[(&str, &str)]) -> super::Run {
        let mut args = vec!["--format", "json", "daemon", "start"];
        args.extend(options.iter().map(String::as_str));
        self.test.run_binary(&mara(), &args, env)
    }

    /// The accepted request: the token, [`ORIGIN`] and project A, running `words` with
    /// `arguments` and `options`, each `[name, value]` with the option's name without `--`.
    fn request(&self, words: &[&str], arguments: &[&str], options: &[[&str; 2]]) -> Request {
        let body = json!({
            "version": 1,
            "request": "operation",
            "project": self.a(),
            "words": words,
            "arguments": arguments,
            "options": options,
        });
        Request {
            method: "POST",
            target: "/operation".to_owned(),
            headers: vec![
                ("Origin", ORIGIN.to_owned()),
                ("Authorization", format!("Bearer {}", self.token)),
                ("Content-Type", "application/json".to_owned()),
            ],
            body: body.to_string(),
        }
    }

    /// The accepted read: `item list` of A.
    fn read(&self) -> Request {
        self.request(&["item", "list"], &[], &[])
    }

    fn send(&self, request: &Request) -> Http {
        exchange(self.port, &request.bytes(self.port))
    }

    /// `served` of `mara daemon status`, which must name the listening daemon.
    fn served(&self) -> u64 {
        let status = self.test.status();
        assert_eq!(status["pid"], self.pid, "{status}");
        status["served"].as_u64().unwrap()
    }

    /// Sends `request`, which the daemon must answer itself: `served` rises by one across it.
    fn served_by_daemon(&self, request: &Request) -> Http {
        let before = self.served();
        let response = self.send(request);
        assert_eq!(self.served(), before + 1, "{response:?}");
        response
    }

    /// `--format json <args>` in direct mode in `root`: what the listener must answer.
    fn direct(&self, root: &Path, args: &[&str]) -> Value {
        let mut all = vec!["--format", "json"];
        all.extend(args);
        self.test.direct(root, &all).json()
    }

    /// Every file of A, B and C with its bytes.
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut files = BTreeMap::new();
        for root in &self.projects {
            collect(root, &mut files);
        }
        files
    }

    /// Requires `request` to be refused as the same request naming a path where nothing exists
    /// is: with the CORS headers of an accepted response when `allowed` names its one allowed
    /// origin, and no `Access-Control-*` header otherwise. The daemon serves nothing, opens no
    /// session and changes no file of A, B or C.
    fn refused(&self, case: &str, request: &Request, allowed: Option<&str>) -> Http {
        let nowhere = self.test.path().join("nowhere");
        let elsewhere = request.clone().field("project", json!(nowhere));
        let (files, sessions, served) = (self.files(), self.test.sessions(), self.served());
        let response = self.send(request);
        assert!(
            (400..500).contains(&response.status),
            "{case}: {response:?}"
        );
        assert_eq!(response, self.send(&elsewhere), "{case}");
        for word in WORDS {
            assert!(!response.body.contains(word), "{case}: {response:?}");
        }
        match allowed {
            Some(origin) => response.allows(origin),
            None => assert!(response.access_control().is_empty(), "{case}: {response:?}"),
        }
        assert_eq!(self.served(), served, "{case}: the daemon served it");
        assert_eq!(
            self.test.sessions(),
            sessions,
            "{case}: a session was opened"
        );
        assert!(self.files() == files, "{case}: a file changed");
        response
    }
}

/// A project in `name/` of the test's directory whose document holds `word`: the item
/// `REQ-<WORD>`, written in direct mode. Its canonical root.
fn project(test: &Test, name: &str, word: &str) -> PathBuf {
    let root = test.path().join(name);
    fs::create_dir_all(root.join(".mara")).unwrap();
    fs::write(root.join(".mara/config.toml"), CONFIG).unwrap();
    fs::write(root.join(".mara/schema.toml"), SCHEMA).unwrap();
    let id = format!("REQ-{}", word.to_ascii_uppercase());
    let body = format!("{word} text.");
    let args = [
        "item",
        "create",
        "requirement",
        "docs/reqs.mara.md",
        "--title",
        word,
        "--id",
        &id,
        "--field",
        "status=draft",
        "--body",
        &body,
    ];
    test.direct(&root, &args).succeeded();
    fs::canonicalize(root).unwrap()
}

/// Adds every file under `directory` with its bytes, following no link.
fn collect(directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            collect(&entry.path(), files);
        } else if kind.is_file() {
            files.insert(entry.path(), fs::read(entry.path()).unwrap());
        }
    }
}

/// The local addresses of the listening TCP sockets of process `pid`, as `/proc/net/tcp` and
/// `/proc/net/tcp6` write them.
fn listening(pid: u32) -> Vec<String> {
    let inodes: Vec<String> = fs::read_dir(format!("/proc/{pid}/fd"))
        .unwrap()
        .filter_map(|fd| {
            let target = fs::read_link(fd.ok()?.path()).ok()?;
            let target = target.to_str()?;
            Some(
                target
                    .strip_prefix("socket:[")?
                    .strip_suffix(']')?
                    .to_owned(),
            )
        })
        .collect();
    let mut found = Vec::new();
    for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
        for line in fs::read_to_string(table).unwrap().lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            // `st` 0A is TCP_LISTEN.
            if fields[3] == "0A" && inodes.iter().any(|inode| inode == fields[9]) {
                found.push(fields[1].to_owned());
            }
        }
    }
    found
}

/// `127.0.0.1` and `::1` as `/proc/net/tcp` and `/proc/net/tcp6` write them on a little-endian
/// machine.
const LOOPBACK: [&str; 2] = ["0100007F:", "00000000000000000000000001000000:"];

#[test]
fn start_rejects_listener_options_out_of_place() {
    let test = Test::new();
    let root = test.path().join("p");
    fs::create_dir(&root).unwrap();
    let root = root.to_str().unwrap();
    let missing = test.path().join("missing");
    let missing = missing.to_str().unwrap();
    let mut cases: Vec<Vec<&str>> = vec![
        vec!["--http", "--allow-project", root],
        vec!["--http", "--allow-origin", ORIGIN],
        vec!["--http"],
        vec!["--allow-origin", ORIGIN],
        vec!["--allow-project", root],
        vec!["--allow-origin", ORIGIN, "--allow-project", root],
        vec![
            "--http=1",
            "--allow-origin",
            ORIGIN,
            "--allow-project",
            root,
        ],
        vec![
            "--http",
            "--allow-origin",
            ORIGIN,
            "--allow-project",
            missing,
        ],
    ];
    for origin in [
        "*",
        "null",
        "http://*.example.com",
        "http://127.0.0.1:5173/",
        "127.0.0.1:5173",
        "file:///",
    ] {
        cases.push(vec![
            "--http",
            "--allow-origin",
            origin,
            "--allow-project",
            root,
        ]);
    }
    for options in cases {
        let mut args = vec!["daemon", "start"];
        args.extend(&options);
        let run = test.run(&args);
        assert_eq!(run.status.code(), Some(2), "{options:?}: {run:?}");
        assert!(
            run.stderr.starts_with("mara: error[invalid-argument]: "),
            "{options:?}: {run:?}"
        );
        assert!(test.daemons().is_empty(), "{options:?} started a daemon");
    }
}

#[test]
fn daemons_started_without_the_listener_never_listen() {
    let test = Test::new();
    let root = project(&test, "a", "Alpha");
    let options = [
        "--http",
        "--allow-origin",
        ORIGIN,
        "--allow-project",
        root.to_str().unwrap(),
    ];
    // Started on demand by a read of the CLI.
    test.run_in(&root, &["item", "list"], &[]).succeeded();
    let pid = test.wait_for_daemon();
    assert_eq!(listening(pid), Vec::<String>::new());
    let status = test.status();
    assert_eq!(
        (&status["port"], &status["token"]),
        (&Value::Null, &Value::Null)
    );
    // `daemon start --http` while it runs changes nothing.
    let mut args = vec!["daemon", "start"];
    args.extend(options);
    let run = test.run(&args);
    assert_eq!(run.status.code(), Some(1), "{run:?}");
    assert!(
        run.stderr.starts_with("mara: error[daemon-running]: "),
        "{run:?}"
    );
    assert_eq!(test.daemons(), [pid]);
    assert_eq!(listening(pid), Vec::<String>::new());
    test.run(&["daemon", "stop"]).succeeded();

    // Started by `mara daemon start`.
    let (_, pid) = test.start();
    assert_eq!(listening(pid), Vec::<String>::new());
    let status = test.status();
    assert_eq!(
        (&status["port"], &status["token"]),
        (&Value::Null, &Value::Null)
    );
    test.run(&["daemon", "stop"]).succeeded();
}

#[test]
fn the_listener_serves_the_allowed_origins_on_the_allowed_projects() {
    let browser = Browser::new();
    let (port, token) = (browser.port, browser.token.as_str());
    assert!(token.len() >= 32, "{token}");
    let status = browser.test.status();
    assert_eq!(
        (&status["port"], &status["token"]),
        (&json!(port), &json!(token))
    );
    let human = browser.test.run(&["daemon", "status"]).succeeded().stdout;
    assert!(
        human.contains(&format!("http://127.0.0.1:{port} with the token {token}")),
        "{human}"
    );
    let sockets = listening(browser.pid);
    assert_eq!(sockets.len(), 1, "{sockets:?}");
    for socket in &sockets {
        assert!(
            LOOPBACK.iter().any(|loopback| socket.starts_with(loopback)),
            "{socket}"
        );
    }

    // The accepted read, and the same from each allowed origin to each allowed project.
    let read = browser.served_by_daemon(&browser.read());
    assert_eq!(
        read.result(),
        browser.direct(browser.a(), &["item", "list"])
    );
    assert_eq!(read.json()["pid"], browser.pid);
    read.allows(ORIGIN);
    for origin in [ORIGIN, SECOND] {
        for root in [browser.a(), browser.c()] {
            let request = browser.read().from(origin).field("project", json!(root));
            let response = browser.served_by_daemon(&request);
            assert_eq!(response.result(), browser.direct(root, &["item", "list"]));
            response.allows(origin);
        }
    }
    // A symbolic link outside A to A names A.
    let link = browser.test.path().join("link-to-a");
    std::os::unix::fs::symlink(browser.a(), &link).unwrap();
    let response = browser.served_by_daemon(&browser.read().field("project", json!(link)));
    assert_eq!(
        response.result(),
        browser.direct(browser.a(), &["item", "list"])
    );

    // The accepted write: the result and files of direct mode on a copy of A.
    let copy = browser.test.path().join("a-copy");
    for (path, bytes) in {
        let mut files = BTreeMap::new();
        collect(browser.a(), &mut files);
        files
    } {
        let target = copy.join(path.strip_prefix(browser.a()).unwrap());
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
    let update = ["title", "Alpha changed"];
    let write = browser.request(&["item", "update"], &["REQ-ALPHA"], &[update]);
    let response = browser.served_by_daemon(&write);
    response.allows(ORIGIN);
    let args = ["item", "update", "REQ-ALPHA", "--title", "Alpha changed"];
    assert_eq!(response.result(), browser.direct(&copy, &args));
    let document = "docs/reqs.mara.md";
    assert_eq!(
        fs::read(browser.a().join(document)).unwrap(),
        fs::read(copy.join(document)).unwrap()
    );

    // A write at a path in B: as in direct mode, and B is untouched.
    let b = browser.b().file_name().unwrap().to_str().unwrap();
    let path = format!("../{b}/docs/x.mara.md");
    let options = [["title", "X"], ["field", "status=draft"], ["body", "Text."]];
    let create = browser.request(&["item", "create"], &["requirement", &path], &options);
    let mut b_files = BTreeMap::new();
    collect(browser.b(), &mut b_files);
    let response = browser.served_by_daemon(&create);
    let args = [
        "item",
        "create",
        "requirement",
        &path,
        "--title",
        "X",
        "--field",
        "status=draft",
        "--body",
        "Text.",
    ];
    let result = response.result();
    assert_eq!(result["error"]["code"], "invalid-argument", "{result}");
    assert_eq!(response.json()["status"], 2, "{response:?}");
    assert_eq!(result, browser.direct(&copy, &args));
    let mut after = BTreeMap::new();
    collect(browser.b(), &mut after);
    assert!(after == b_files, "B changed");
}

#[test]
fn every_unauthorized_request_is_refused_alike() {
    let browser = Browser::new();
    // A session the daemon holds before the refusals.
    browser.served_by_daemon(&browser.read()).result();
    let changed = {
        let mut token = browser.token.clone().into_bytes();
        token[7] = if token[7] == b'0' { b'1' } else { b'0' };
        String::from_utf8(token).unwrap()
    };
    let read = browser.read();
    let no_token = read.clone().without("Authorization");
    for (case, request) in [
        ("token left out", no_token.clone()),
        (
            "empty token",
            no_token.clone().with("Authorization", "Bearer "),
        ),
        (
            "token changed",
            no_token
                .clone()
                .with("Authorization", &format!("Bearer {changed}")),
        ),
        (
            "token in the query string",
            Request {
                target: format!("/operation?token={}", browser.token),
                ..no_token.clone()
            },
        ),
    ] {
        let response = browser.refused(case, &request, Some(ORIGIN));
        assert_eq!(response.status, 401, "{case}: {response:?}");
        assert_eq!(response.json(), token_refusal(), "{case}");
    }

    browser.refused("origin left out", &read.clone().without("Origin"), None);
    browser.refused(
        "origin given twice",
        &read.clone().with("Origin", THIRD),
        None,
    );
    for origin in [
        "null",
        "https://127.0.0.1:5173",
        "http://localhost:5173",
        THIRD,
        "http://127.0.0.1:51730",
        "http://127.0.0.1:5173.example.com",
        "http://127.0.0.1:5173/",
    ] {
        browser.refused(origin, &read.clone().from(origin), None);
    }

    let a = browser.a();
    let b_name = browser.b().file_name().unwrap().to_str().unwrap();
    std::os::unix::fs::symlink(browser.b(), a.join("link-to-b")).unwrap();
    let nested = a.join("docs/nested");
    fs::create_dir_all(nested.join(".mara")).unwrap();
    fs::write(nested.join(".mara/config.toml"), CONFIG).unwrap();
    fs::write(nested.join(".mara/schema.toml"), SCHEMA).unwrap();
    for (case, project) in [
        ("project B", json!(browser.b())),
        ("no project", Value::Null),
        ("A/../B", json!(format!("{}/../{b_name}", a.display()))),
        ("a link in A to B", json!(a.join("link-to-b"))),
        ("a project nested in A", json!(nested)),
        ("a regular file of A", json!(a.join("docs/reqs.mara.md"))),
        ("a relative path", json!("a")),
    ] {
        browser.refused(case, &read.clone().field("project", project), Some(ORIGIN));
    }

    // Preflights need no token, and open no session.
    let sessions = browser.test.sessions();
    let preflight = Request {
        method: "OPTIONS",
        target: "/operation".to_owned(),
        headers: vec![
            ("Origin", ORIGIN.to_owned()),
            ("Access-Control-Request-Method", "POST".to_owned()),
            (
                "Access-Control-Request-Headers",
                "authorization,content-type".to_owned(),
            ),
        ],
        body: String::new(),
    };
    let allowed = browser.send(&preflight);
    assert_eq!(allowed.status, 204, "{allowed:?}");
    assert_eq!(allowed.header("access-control-allow-origin"), Some(ORIGIN));
    assert_eq!(allowed.header("vary"), Some("Origin"));
    assert_eq!(allowed.header("access-control-allow-methods"), Some("POST"));
    let headers = allowed
        .header("access-control-allow-headers")
        .unwrap()
        .to_ascii_lowercase();
    assert!(
        headers.contains("authorization") && headers.contains("content-type"),
        "{headers}"
    );
    assert_eq!(allowed.header("access-control-max-age"), Some("600"));
    let served = browser.served();
    let other = browser.send(&preflight.clone().from(THIRD));
    assert!((400..500).contains(&other.status), "{other:?}");
    assert!(other.access_control().is_empty(), "{other:?}");
    assert_eq!(browser.test.sessions(), sessions);
    assert_eq!(browser.served(), served);
}

#[test]
fn operations_a_browser_cannot_run_are_unavailable() {
    let browser = Browser::new();
    let home = browser.test.path().join("home");
    let tree = |root: &Path| {
        let mut files = BTreeMap::new();
        if root.exists() {
            collect(root, &mut files);
        }
        files
    };
    let (files, cache) = (tree(browser.a()), tree(&home));
    for (words, arguments) in [
        (&["project", "init"][..], &[][..]),
        (&["project", "convert"], &[]),
        (&["daemon", "stop"], &[]),
        (&["code", "add"], &["rust"]),
    ] {
        let response = browser.send(&browser.request(words, arguments, &[]));
        response.allows(ORIGIN);
        assert_eq!(response.json()["status"], 1, "{response:?}");
        assert_eq!(response.json()["failed"], true, "{response:?}");
        let error = &response.result()["error"];
        assert_eq!(error["code"], "unavailable-operation", "{words:?}: {error}");
        assert!(
            error["message"]
                .as_str()
                .unwrap()
                .contains(&words.join(" ")),
            "{error}"
        );
    }
    assert!(tree(browser.a()) == files, "A changed");
    assert!(tree(&home) == cache, "something was written under HOME");
    assert_eq!(browser.test.daemons(), [browser.pid]);
    assert_eq!(browser.test.status()["pid"], browser.pid);
}

#[test]
fn a_start_while_the_listener_runs_changes_nothing() {
    let browser = Browser::new();
    let (port, token) = (json!(browser.port), json!(browser.token));
    // The same origins and projects, in another order.
    let mut same = browser.options();
    same.swap(2, 4);
    let run = browser.start(&same, &[]);
    assert_eq!(run.status.code(), Some(0), "{run:?}");
    let json = run.json();
    assert_eq!(
        (json["started"].clone(), &json["port"], &json["token"]),
        (json!(false), &port, &token)
    );

    let with = |extra: &[&str]| {
        let mut options = browser.options();
        options.extend(extra.iter().map(|option| (*option).to_owned()));
        options
    };
    let mut without_c = browser.options();
    without_c.truncate(without_c.len() - 2);
    for options in [
        with(&["--allow-origin", THIRD]),
        with(&["--allow-project", browser.b().to_str().unwrap()]),
        without_c,
    ] {
        let run = browser.start(&options, &[]);
        assert_eq!(run.status.code(), Some(1), "{options:?}: {run:?}");
        assert_eq!(run.json()["error"]["code"], "daemon-running", "{run:?}");
    }
    let status = browser.test.status();
    assert_eq!((&status["port"], &status["token"]), (&port, &token));
    assert_eq!(browser.test.daemons(), [browser.pid]);
    browser.refused("the third origin", &browser.read().from(THIRD), None);
    let to_b = browser.read().field("project", json!(browser.b()));
    browser.refused("project B", &to_b, Some(ORIGIN));
    for origin in [ORIGIN, SECOND] {
        for root in [browser.a(), browser.c()] {
            let request = browser.read().from(origin).field("project", json!(root));
            let response = browser.served_by_daemon(&request);
            assert_eq!(response.result(), browser.direct(root, &["item", "list"]));
            response.allows(origin);
        }
    }
}

#[test]
fn the_listener_outlives_the_idle_period_and_ends_with_stop() {
    let browser = Browser::with_env(&[("MARA_TEST_IDLE_SECONDS", "4")]);
    let expected = browser.direct(browser.a(), &["item", "list"]);
    assert_eq!(browser.served_by_daemon(&browser.read()).result(), expected);
    thread::sleep(Duration::from_secs(10));
    assert_eq!(browser.test.daemons(), [browser.pid]);
    assert_eq!(browser.test.sessions(), Vec::<PathBuf>::new());
    assert_eq!(browser.served_by_daemon(&browser.read()).result(), expected);

    // The token is in no file of the test, while the daemon runs and after it stopped: its
    // runtime directory, `HOME` and `TMPDIR` are in it.
    let no_file_holds = |token: &[u8]| {
        let mut files = BTreeMap::new();
        collect(browser.test.path(), &mut files);
        for (path, bytes) in &files {
            assert!(
                !bytes.windows(token.len()).any(|window| window == token),
                "{path:?}"
            );
        }
    };
    no_file_holds(browser.token.as_bytes());
    browser.test.run(&["daemon", "stop"]).succeeded();
    assert_eq!(browser.test.daemons(), Vec::<u32>::new(), "the daemon runs");
    let error = TcpStream::connect(("127.0.0.1", browser.port))
        .map(drop)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ConnectionRefused);
    no_file_holds(browser.token.as_bytes());

    // The previous run's token, to a new listening daemon.
    let started = browser.start(&browser.options(), &[]).json();
    let port = u16::try_from(started["port"].as_u64().unwrap()).unwrap();
    assert_ne!(started["token"], json!(browser.token));
    // `read` carries the previous token.
    let stale = browser.read();
    let response = exchange(port, &stale.bytes(port));
    assert_eq!(response.status, 401, "{response:?}");
    assert_eq!(response.json(), token_refusal());
    response.allows(ORIGIN);
    let elsewhere = stale.field("project", json!(browser.test.path().join("nowhere")));
    assert_eq!(response, exchange(port, &elsewhere.bytes(port)));
    let status = browser.test.status();
    assert_eq!(
        (&status["served"], &status["sessions"]),
        (&json!(0), &json!([]))
    );
    let pid = status["pid"].as_u64().unwrap();
    browser.test.run(&["daemon", "stop"]).succeeded();
    assert_eq!(browser.test.daemons(), Vec::<u32>::new(), "{pid} runs");

    // The next daemon started on demand does not listen.
    browser
        .test
        .run_in(browser.a(), &["item", "list"], &[])
        .succeeded();
    let pid = browser.test.wait_for_daemon();
    assert_eq!(listening(pid), Vec::<String>::new());
    browser.test.run(&["daemon", "stop"]).succeeded();
    assert_eq!(browser.test.daemons(), Vec::<u32>::new(), "{pid} runs");
}

/// The body of a refusal for the token.
fn token_refusal() -> Value {
    json!({
        "version": 1,
        "error": "refused",
        "message": "the request carries no `Authorization: Bearer` header with this daemon's token",
    })
}

#[test]
fn a_body_shorter_than_its_length_is_malformed() {
    let browser = Browser::new();
    let request = browser.read();
    let mut bytes = request.bytes(browser.port);
    // Declares one byte more than it sends, then ends its side of the connection.
    let declared = format!("Content-Length: {}", request.body.len());
    let longer = format!("Content-Length: {}", request.body.len() + 1);
    let text = String::from_utf8(bytes)
        .unwrap()
        .replace(&declared, &longer);
    bytes = text.into_bytes();
    let mut stream = TcpStream::connect(("127.0.0.1", browser.port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    stream.write_all(&bytes).unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    assert!(answer.starts_with("HTTP/1.1 400 "), "{answer}");
    assert!(
        answer.contains("\"error\":\"malformed-request\""),
        "{answer}"
    );
    assert_eq!(browser.test.sessions(), Vec::<PathBuf>::new());
}

/// A write whose answer is not yet sent when `mara daemon stop` arrives: the daemon sends it
/// whole before it exits (DES-DAEMON-LIFECYCLE). The listener holds the answer at the pause point
/// `MARA_TEST_PAUSE_ANSWER` until the daemon has removed its socket, which it does only once it
/// stops; then a daemon that no longer counted the write would exit at once.
#[cfg(debug_assertions)]
#[test]
fn a_write_whose_answer_is_unsent_at_stop_gets_it_before_the_daemon_exits() {
    let test = Test::new();
    let pauses = test.path().join("pauses");
    fs::create_dir(&pauses).unwrap();
    let (socket, _) = test.paths_of(&mara());
    let browser = Browser::in_test(
        test,
        &[("MARA_TEST_PAUSE_ANSWER", pauses.to_str().unwrap())],
    );
    let write = browser.request(
        &["item", "update"],
        &["REQ-ALPHA"],
        &[["title", "Alpha changed"]],
    );
    let (port, bytes) = (browser.port, write.bytes(browser.port));
    let writing = thread::spawn(move || exchange(port, &bytes));
    let paused = pauses.join("paused");
    wait_until("the answer is held", || paused.exists());
    let stopping = browser.test.spawn(&mara(), &["daemon", "stop"], &[]);
    wait_until("the daemon removes its socket", || !socket.exists());
    fs::remove_file(&paused).unwrap();
    let response = writing.join().unwrap();
    response.allows(ORIGIN);
    assert_eq!(
        response.result()["item"]["title"],
        "Alpha changed",
        "{response:?}"
    );
    let document = fs::read_to_string(browser.a().join("docs/reqs.mara.md")).unwrap();
    assert!(document.contains("Alpha changed"), "{document}");
    super::finish(stopping).succeeded();
    assert_eq!(browser.test.daemons(), Vec::<u32>::new());
}

/// Waits until `done` holds, at most 10 s.
#[cfg(debug_assertions)]
fn wait_until(what: &str, done: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + super::WAIT;
    while !done() {
        assert!(
            std::time::Instant::now() < deadline,
            "{what}: not within 10 s"
        );
        thread::sleep(Duration::from_millis(5));
    }
}
