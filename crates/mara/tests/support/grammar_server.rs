//! A grammar repository of a test (VER-CODE-ADD): HTTP/1.1 on a loopback port, serving the
//! files the test gives it and counting the connections it accepts.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// The version of the fixture grammars (VER-GRAMMAR-MARKERS) by language.
pub const FIXTURE_VERSIONS: [(&str, &str); 2] = [("rust", "0.24.2"), ("python", "0.25.0")];

/// The `.wasm` and `tags.scm` of the fixture grammar of `language`, and its version.
pub fn fixture(language: &str) -> (Vec<u8>, Vec<u8>, &'static str) {
    let version = FIXTURE_VERSIONS
        .iter()
        .find(|(name, _)| *name == language)
        .expect("a fixture grammar")
        .1;
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grammars")
        .join(language);
    let wasm = std::fs::read(directory.join(format!("tree-sitter-{language}.wasm"))).unwrap();
    let tags = std::fs::read(directory.join("tags.scm")).unwrap();
    (wasm, tags, version)
}

/// A running server, stopped on drop.
pub struct GrammarServer {
    port: u16,
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    /// The peer of each connection accepted, in the order accepted.
    accepted: Arc<Mutex<Vec<SocketAddr>>>,
    /// The connections that [`GrammarServer::connections`] made itself.
    sentinels: AtomicUsize,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl GrammarServer {
    /// A server on `127.0.0.1` at a free port, serving nothing yet.
    #[allow(clippy::new_without_default, reason = "each server binds a new port")]
    pub fn start() -> GrammarServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let port = listener.local_addr().unwrap().port();
        let files = Arc::new(Mutex::new(HashMap::new()));
        let accepted = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (files, accepted, stop) = (files.clone(), accepted.clone(), stop.clone());
            thread::spawn(move || {
                while let Ok((stream, peer)) = listener.accept() {
                    if stop.load(Ordering::SeqCst) {
                        return;
                    }
                    accepted.lock().unwrap().push(peer);
                    answer(stream, &files);
                }
            })
        };
        GrammarServer {
            port,
            files,
            accepted,
            sentinels: AtomicUsize::new(0),
            stop,
            thread: Some(thread),
        }
    }

    /// The repository's URL, without a final `/`.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Serves `contents` at `path`, which starts with `/`.
    pub fn serve(&self, path: &str, contents: Vec<u8>) {
        self.files.lock().unwrap().insert(path.to_owned(), contents);
    }

    /// Serves `wasm` and `tags` in the layout of a given repository (DES-GRAMMAR-ASSETS).
    pub fn serve_grammar(&self, language: &str, version: &str, wasm: Vec<u8>, tags: Vec<u8>) {
        self.serve(&format!("/{language}/{version}/grammar.wasm"), wasm);
        self.serve(&format!("/{language}/{version}/tags.scm"), tags);
    }

    /// The connections made so far, its own aside. It connects once more and waits until that
    /// connection is accepted: the listen queue is first in, first out, so every connection
    /// completed before, also one its client closed at once, has been accepted and counted.
    pub fn connections(&self) -> usize {
        let sentinel = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to the server");
        let own = sentinel.local_addr().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let accepted = self.accepted.lock().unwrap();
            if accepted.contains(&own) {
                let sentinels = self.sentinels.fetch_add(1, Ordering::SeqCst) + 1;
                return accepted.len() - sentinels;
            }
            drop(accepted);
            assert!(
                Instant::now() < deadline,
                "the server accepts no connection"
            );
            thread::sleep(Duration::from_millis(1));
        }
    }
}

impl Drop for GrammarServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Reads one request's head and answers it with the file at its path, or 404, and closes.
fn answer(mut stream: TcpStream, files: &Mutex<HashMap<String, Vec<u8>>>) {
    let mut head = Vec::new();
    let mut byte = [0];
    while !head.ends_with(b"\r\n\r\n") && head.len() < 64 * 1024 {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => return,
        }
    }
    let head = String::from_utf8_lossy(&head);
    let path = head.split(' ').nth(1).unwrap_or_default().to_owned();
    let found = files.lock().unwrap().get(&path).cloned();
    let (status, body) = match found {
        Some(body) => ("200 OK", body),
        None => ("404 Not Found", b"not found".to_vec()),
    };
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}
