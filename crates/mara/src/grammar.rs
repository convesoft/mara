//! Grammars of the grammar level (DES-GRAMMAR-ASSETS and DES-CODE-MARKERS in
//! `docs/code/index.mara.md`): a pinned grammar's files in the per-user cache, checked against
//! the pin, loaded into tree-sitter's WebAssembly store and run on a thread of its own, which a
//! request leaves behind when it does not answer in time.
//!
//! A grammar is WebAssembly that Mara did not write. It runs in tree-sitter's store, which gives
//! it no file, network or native code and at most 128 MiB of memory; a trap ends the parse
//! without a tree. Mara bounds its time: a parse and its tags query are cancelled after a budget
//! that grows with the file, and a grammar whose thread does not answer by twice that budget
//! and 1 s more, or that does not load within 10 s, is dropped for the rest of the
//! process, its thread left running. The record of dropped grammars is the process's, so that no
//! later request of any project waits on one again. Each grammar loads under a lock of its own,
//! so that a slow load holds up only the requests that need that grammar.

mod module;

use std::collections::HashMap;
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, TryLockError};
use std::thread;
use std::time::{Duration, Instant};

use mara_engine::code::{Syntax, SyntaxDeclaration};
use mara_engine::config::{CHECKSUM_PREFIX, CONFIG_PATH, GrammarPin, Language};
use mara_engine::diagnostic::Diagnostic;
use mara_engine::hash::Hash;
use tree_sitter::{
    LANGUAGE_VERSION, MIN_COMPATIBLE_LANGUAGE_VERSION, ParseOptions, Parser, Query, QueryCursor,
    QueryCursorOptions, StreamingIterator, Tree, WasmStore, wasmtime,
};

use crate::hooks::{self, Hook};

/// A pinned grammar that is not in the cache, does not match its pin or cannot be used.
pub const UNAVAILABLE_GRAMMAR: &str = "unavailable-grammar";
/// The grammar's WebAssembly module in its cache directory.
pub const WASM_FILE: &str = "grammar.wasm";
/// The grammar's upstream `queries/tags.scm` in its cache directory.
pub const TAGS_FILE: &str = "tags.scm";

/// Node kinds that are comments (DES-CODE-MARKERS).
const COMMENTS: &[&str] = &["comment", "line_comment", "block_comment"];
/// Node kinds that are attributes, decorators or annotations, which may stand between a marker's
/// comment and its declaration.
const ATTRIBUTES: &[&str] = &[
    "attribute_item",
    "inner_attribute_item",
    "decorator",
    "annotation",
    "marker_annotation",
    "attribute_list",
];

/// The time a grammar may take to load: to compile, instantiate and initialize.
const LOAD_LIMIT: Duration = Duration::from_secs(10);
/// The time a parse may take, besides [`BYTES_PER_SECOND`].
const BASE_BUDGET: Duration = Duration::from_secs(1);
/// How many bytes of a file add one second to its budget.
const BYTES_PER_SECOND: usize = 256 * 1024;

/// The per-user directory of cached grammars: `$XDG_CACHE_HOME/mara/grammars`, else
/// `~/.cache/mara/grammars`, and `~/Library/Caches/mara/grammars` on macOS. `None` when the
/// environment names no home.
pub fn cache_root() -> Option<PathBuf> {
    let home = || std::env::var_os("HOME").filter(|home| !home.is_empty());
    let cache = if cfg!(target_os = "macos") {
        PathBuf::from(home()?).join("Library/Caches")
    } else if cfg!(windows) {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").filter(|dir| !dir.is_empty())?)
    } else {
        match std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from) {
            Some(cache) if cache.is_absolute() => cache,
            _ => PathBuf::from(home()?).join(".cache"),
        }
    };
    Some(cache.join("mara").join("grammars"))
}

/// The directory of the grammar that `pin` pins for `language` below `root`:
/// `<language>/<version>/<checksum>`, the checksum in hexadecimal.
pub fn grammar_dir(root: &Path, language: &str, pin: &GrammarPin) -> PathBuf {
    root.join(language)
        .join(&pin.version)
        .join(pin.checksum.to_string())
}

/// The checksum of a grammar's files: the BLAKE3 hash of the BLAKE3 hashes of its `.wasm` and
/// its `tags.scm`, in that order.
pub fn checksum(wasm: &[u8], tags: &[u8]) -> Hash {
    let mut hashes = Vec::with_capacity(64);
    hashes.extend_from_slice(Hash::of_bytes(wasm).as_bytes());
    hashes.extend_from_slice(Hash::of_bytes(tags).as_bytes());
    Hash::of_bytes(&hashes)
}

/// The grammars of this process: those loaded, each kept until its files' stats change, and
/// those dropped, each kept until the process ends. Every request and project the process serves
/// shares them. The registry is locked only to look a grammar up, never across a load.
#[derive(Debug, Default)]
struct Registry {
    /// By language name and checksum.
    slots: HashMap<(String, Hash), Arc<Slot>>,
    /// By language name and checksum, with the reason.
    dropped: HashMap<(String, Hash), String>,
}

/// The place of one grammar, which a load holds locked: requests for that grammar wait for the
/// load and share what it gives, and those for other grammars do not wait.
#[derive(Debug, Default)]
struct Slot {
    loaded: Mutex<Option<Loaded>>,
    /// How many loads have ended, so that a request tells whether one ended while it waited.
    attempts: AtomicU64,
}

#[derive(Debug)]
struct Loaded {
    /// The directory of the files it was loaded from.
    directory: PathBuf,
    stats: Option<[Stat; 2]>,
    grammar: Result<Arc<Grammar>, String>,
}

/// What tells that a cached file changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stat {
    length: u64,
    modified: Option<std::time::SystemTime>,
    #[cfg(unix)]
    inode: u64,
}

impl Stat {
    fn of(path: &Path) -> Option<Stat> {
        let metadata = fs::metadata(path).ok()?;
        Some(Stat {
            length: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            inode: std::os::unix::fs::MetadataExt::ino(&metadata),
        })
    }
}

/// The registry of this process, locked.
fn registry() -> MutexGuard<'static, Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    lock(REGISTRY.get_or_init(Mutex::default))
}

/// `mutex` locked, also after a panic of another holder.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Why the grammar of `key` is dropped for the process, if it is.
fn dropped(key: &(String, Hash)) -> Option<String> {
    registry().dropped.get(key).cloned()
}

/// Drops the grammar of `key` for the rest of the process, for `reason` unless it was dropped
/// before. Its slot leaves the registry, so that a grammar it holds is released once no request
/// holds the slot.
fn drop_for_process(key: &(String, Hash), reason: String) {
    let mut registry = registry();
    registry.dropped.entry(key.clone()).or_insert(reason);
    let slot = registry.slots.remove(key);
    drop(registry);
    drop(slot);
}

/// The grammar that `language` pins, loaded unless this process loaded it from files whose
/// stats are unchanged: `None` without a pin, else the grammar or its `unavailable-grammar`
/// diagnostic at the configuration line of the pin. A grammar dropped in this process is
/// neither loaded nor waited on. A request waits only for a load of the same grammar.
pub fn get(language: &Language) -> Option<Result<Arc<Grammar>, Diagnostic>> {
    let pin = language.grammar.as_ref()?;
    let unavailable = |reason: String| unavailable(language, &reason);
    let key = (language.name.clone(), pin.checksum);
    let slot = {
        let mut registry = registry();
        if let Some(reason) = registry.dropped.get(&key) {
            return Some(Err(unavailable(reason.clone())));
        }
        Arc::clone(registry.slots.entry(key.clone()).or_default())
    };
    let Some(root) = cache_root() else {
        let reason = "is not cached: the environment names no home directory for the cache";
        return Some(Err(unavailable(reason.to_owned())));
    };
    let directory = grammar_dir(&root, &language.name, pin);
    let (wasm, tags) = (directory.join(WASM_FILE), directory.join(TAGS_FILE));
    let seen = slot.attempts.load(Ordering::Acquire);
    let mut held = match slot.loaded.try_lock() {
        Ok(loaded) => loaded,
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => {
            event("wait", &key.0);
            lock(&slot.loaded)
        }
    };
    // Dropped while this request waited for another's load of it, as when that load timed out.
    if let Some(reason) = dropped(&key) {
        return Some(Err(unavailable(reason)));
    }
    // A load of the same files that ended while this request waited gives its grammar or its
    // failure to this request too; a later request checks the files again and loads again after
    // a failure.
    let shared = slot.attempts.load(Ordering::Acquire) != seen;
    let stats = Stat::of(&wasm).zip(Stat::of(&tags)).map(|(a, b)| [a, b]);
    let reusable = held.as_ref().is_some_and(|loaded| {
        (shared && loaded.directory == directory)
            || (loaded.stats.is_some() && loaded.stats == stats && loaded.grammar.is_ok())
    });
    let loaded = match &mut *held {
        Some(loaded) if reusable => loaded,
        empty_or_stale => {
            let grammar = load(&key, pin, &wasm, &tags).map(Arc::new);
            let fresh = empty_or_stale.insert(Loaded {
                directory,
                stats,
                grammar,
            });
            slot.attempts.fetch_add(1, Ordering::Release);
            fresh
        }
    };
    let grammar = match &loaded.grammar {
        Ok(grammar) => Ok(Arc::clone(grammar)),
        Err(reason) => Err(reason.clone()),
    };
    drop(held);
    // Dropped during this request's load: by its timeout, or by a stalled parse of the grammar
    // that the load replaced.
    if !reusable && let Some(reason) = dropped(&key) {
        return Some(Err(unavailable(reason)));
    }
    Some(grammar.map_err(unavailable))
}

/// The `unavailable-grammar` diagnostic of the grammar that `language` pins, which `reason`
/// says is not used: at the pin's configuration line, naming `mara code add`.
pub fn unavailable(language: &Language, reason: &str) -> Diagnostic {
    let (line, version) = match &language.grammar {
        Some(pin) => (pin.line, pin.version.as_str()),
        None => (1, ""),
    };
    let message = format!(
        "the {} grammar {version} {reason}, so Mara scans its files at the basic level: run \
         `mara code add {}`",
        language.name, language.name
    );
    Diagnostic::error(UNAVAILABLE_GRAMMAR, CONFIG_PATH, line, message)
}

/// The `unavailable-grammar` diagnostic of the file at `path` that the grammar of `language`
/// could not parse, for `reason`: at the pin's configuration line.
pub fn fallback(language: &Language, path: &str, reason: &str) -> Diagnostic {
    let (line, version) = match &language.grammar {
        Some(pin) => (pin.line, pin.version.as_str()),
        None => (1, ""),
    };
    let message = format!(
        "the {} grammar {version} could not parse {path}: {reason}, so Mara scans that file at \
         the basic level",
        language.name
    );
    Diagnostic::error(UNAVAILABLE_GRAMMAR, CONFIG_PATH, line, message)
}

/// Reads the grammar's files, checks them against `pin` and loads them on a new thread.
fn load(
    key: &(String, Hash),
    pin: &GrammarPin,
    wasm: &Path,
    tags: &Path,
) -> Result<Grammar, String> {
    let read = |path: &Path| {
        fs::read(path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => {
                format!(
                    "is not in the grammar cache ({} is missing)",
                    path.display()
                )
            }
            _ => format!(
                "cannot be read from the cache ({}: {error})",
                path.display()
            ),
        })
    };
    let (wasm, tags) = (read(wasm)?, read(tags)?);
    let found = checksum(&wasm, &tags);
    if found != pin.checksum {
        return Err(format!(
            "in the cache does not match its pin: its checksum is {CHECKSUM_PREFIX}{found}"
        ));
    }
    let Ok(tags) = String::from_utf8(tags) else {
        return Err("cannot be loaded: its tags.scm is not UTF-8".to_owned());
    };
    let symbol = format!("tree_sitter_{}", key.0.replace('-', "_"));
    if let Err(reason) = module::check(&wasm, &symbol) {
        return Err(format!(
            "cannot be loaded: it is no tree-sitter grammar ({reason})"
        ));
    }
    Grammar::start(key, wasm, tags)
}

/// A loaded grammar and the thread that runs it.
#[derive(Debug)]
pub struct Grammar {
    /// `None` only while the grammar is dropped.
    jobs: Option<Sender<Job>>,
    /// The thread, which a dropped grammar waits for unless it was abandoned: it must not tear
    /// its WebAssembly code down while the process exits.
    worker: Option<thread::JoinHandle<()>>,
    /// Its language's name and its checksum, under which it is recorded when abandoned.
    key: (String, Hash),
    /// Where the thread stands with its requests, shared with it: [`GAVE_UP`] once a parse did
    /// not answer in time; the registry keeps the reason.
    state: Arc<AtomicU8>,
    /// Held during a parse, so that one request's wait never includes another's job.
    busy: Mutex<()>,
}

/// A grammar's thread is loading it, and its request waits.
const LOADING: u8 = 0;
/// A grammar's thread answers its request: the request waits for it, and joins it when it
/// releases the grammar.
const ANSWERED: u8 = 1;
/// A grammar's request gave up on its thread, which then never drops what it loaded: it must not
/// tear its WebAssembly code down while the process exits.
const GAVE_UP: u8 = 2;

impl Drop for Grammar {
    fn drop(&mut self) {
        drop(self.jobs.take());
        if let Some(worker) = self.worker.take()
            && self.state.load(Ordering::Acquire) != GAVE_UP
        {
            let _ = worker.join();
        }
    }
}

struct Job {
    contents: Arc<[u8]>,
    budget: Duration,
    reply: Sender<Result<Syntax, String>>,
}

/// Why a file was not scanned at the grammar level.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseFailure {
    /// The grammar could not parse this file, which falls back to the basic level.
    File(String),
    /// The grammar did not answer, and is dropped for the process ([`Grammar::abandoned`] gives
    /// the reason).
    Abandoned,
}

impl Grammar {
    /// Loads the module `wasm` and its tags query `tags` on a new thread, which keeps them.
    fn start(key: &(String, Hash), wasm: Vec<u8>, tags: String) -> Result<Grammar, String> {
        let (jobs, received) = mpsc::channel::<Job>();
        let (ready, loaded) = mpsc::channel::<Result<(), String>>();
        let name = &key.0;
        let symbol = name.replace('-', "_");
        let state = Arc::new(AtomicU8::new(LOADING));
        let shared = Arc::clone(&state);
        let spawned = thread::Builder::new()
            .name(format!("mara-grammar-{name}"))
            .spawn(move || {
                // Called before the thread drops anything it loaded: parks, keeping it, once the
                // request gave up.
                let answer = || {
                    let answered = shared.compare_exchange(
                        LOADING,
                        ANSWERED,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                    if answered.is_err() {
                        park_forever();
                    }
                };
                match Worker::load(&symbol, &wasm, &tags, &answer) {
                    Ok(mut worker) => {
                        answer();
                        if ready.send(Ok(())).is_ok() {
                            worker.run(received);
                        }
                        if shared.load(Ordering::Acquire) == GAVE_UP {
                            park_forever();
                        }
                    }
                    Err(reason) => {
                        let _ = ready.send(Err(reason));
                    }
                }
            });
        let worker = match spawned {
            Ok(worker) => worker,
            Err(error) => {
                return Err(format!("cannot be loaded: no thread could start ({error})"));
            }
        };
        let limit = load_limit();
        let mut reply = loaded.recv_timeout(limit);
        if matches!(reply, Err(RecvTimeoutError::Timeout)) {
            let gave_up =
                state.compare_exchange(LOADING, GAVE_UP, Ordering::AcqRel, Ordering::Acquire);
            if gave_up.is_ok() {
                // Its thread is left running, like that of a parse that does not answer.
                let reason = format!(
                    "did not load within {:.1} s and is no longer used in this process",
                    limit.as_secs_f64()
                );
                drop_for_process(key, reason.clone());
                return Err(reason);
            }
            // The thread answered as the time ran out: its answer follows.
            reply = loaded.recv().map_err(|_| RecvTimeoutError::Disconnected);
        }
        match reply {
            Ok(Ok(())) => Ok(Grammar {
                jobs: Some(jobs),
                worker: Some(worker),
                key: key.clone(),
                state,
                busy: Mutex::new(()),
            }),
            Ok(Err(reason)) => {
                // The thread ends once it has dropped what it loaded.
                let _ = worker.join();
                Err(format!("cannot be loaded: {reason}"))
            }
            Err(_) => {
                let _ = worker.join();
                Err("cannot be loaded: its thread stopped while it loaded".to_owned())
            }
        }
    }

    /// Why the grammar is dropped for the process, once a parse did not answer in time.
    pub fn abandoned(&self) -> Option<String> {
        dropped(&self.key)
    }

    /// What the grammar finds in `contents`, the file at `path`, within its budget.
    pub fn parse(&self, contents: &[u8], path: &str) -> Result<Syntax, ParseFailure> {
        let _busy = lock(&self.busy);
        if self.state.load(Ordering::Acquire) == GAVE_UP {
            return Err(ParseFailure::Abandoned);
        }
        let budget = budget(contents.len());
        let (reply, answer) = mpsc::channel();
        let job = Job {
            contents: Arc::from(contents),
            budget,
            reply,
        };
        if self
            .jobs
            .as_ref()
            .is_none_or(|jobs| jobs.send(job).is_err())
        {
            let reason = format!("stopped while it parsed {path} and is no longer used");
            return Err(self.abandon(reason));
        }
        match answer.recv_timeout(budget * 2 + Duration::from_secs(1)) {
            Ok(Ok(syntax)) => Ok(syntax),
            Ok(Err(reason)) => Err(ParseFailure::File(reason)),
            Err(RecvTimeoutError::Timeout) => Err(self.abandon(format!(
                "did not return from parsing {path} within {:.1} s and is no longer used in this \
                 process",
                (budget * 2 + Duration::from_secs(1)).as_secs_f64()
            ))),
            Err(RecvTimeoutError::Disconnected) => Err(self.abandon(format!(
                "stopped while it parsed {path} and is no longer used"
            ))),
        }
    }

    /// Drops the grammar for the rest of the process, for `reason`.
    fn abandon(&self, reason: String) -> ParseFailure {
        // Set first, so that its thread is never waited for once the slot releases it.
        self.state.store(GAVE_UP, Ordering::Release);
        drop_for_process(&self.key, reason);
        ParseFailure::Abandoned
    }
}

/// The time a grammar may take to load.
fn load_limit() -> Duration {
    hooks::number(Hook::GrammarLoadMs).map_or(LOAD_LIMIT, Duration::from_millis)
}

/// The time a grammar may take on a file of `length` bytes.
fn budget(length: usize) -> Duration {
    hooks::number(Hook::GrammarBudgetMs).map_or_else(
        || BASE_BUDGET + Duration::from_secs((length / BYTES_PER_SECOND) as u64),
        Duration::from_millis,
    )
}

/// The parser, its WebAssembly store and the tags query of one grammar, on its own thread.
struct Worker {
    parser: Parser,
    query: Query,
    /// Whether each capture of the query is a definition (`@definition.*`).
    definitions: Vec<bool>,
    name: Option<u32>,
}

impl Worker {
    /// Loads the grammar, calling `answer` before a failure drops anything it loaded.
    fn load(symbol: &str, wasm: &[u8], tags: &str, answer: &dyn Fn()) -> Result<Worker, String> {
        stall_load(symbol);
        let failed = |reason: String| {
            answer();
            reason
        };
        let engine = wasmtime::Engine::default();
        let mut store = WasmStore::new(&engine).map_err(|error| failed(error.message))?;
        let language = store
            .load_language(symbol, wasm)
            .map_err(|error| failed(error.message))?;
        // Tree-sitter's store refuses another ABI too; the grammar is never used with one.
        let abi = language.abi_version();
        if !(MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&abi) {
            return Err(failed(format!(
                "its ABI version is {abi}, and Mara's tree-sitter runtime takes \
                 {MIN_COMPATIBLE_LANGUAGE_VERSION} to {LANGUAGE_VERSION}"
            )));
        }
        let query = Query::new(&language, tags)
            .map_err(|error| failed(format!("its tags.scm is no query of the grammar: {error}")))?;
        let mut parser = Parser::new();
        parser
            .set_wasm_store(store)
            .and_then(|()| parser.set_language(&language))
            .map_err(|error| failed(error.to_string()))?;
        let definitions = (query.capture_names().iter())
            .map(|name| name.starts_with("definition."))
            .collect();
        let name = query.capture_index_for_name("name");
        Ok(Worker {
            parser,
            query,
            definitions,
            name,
        })
    }

    fn run(&mut self, jobs: Receiver<Job>) {
        while let Ok(job) = jobs.recv() {
            stall(&job.contents);
            let deadline = Instant::now() + job.budget;
            let outline = self.outline(&job.contents, deadline);
            if job.reply.send(outline).is_err() {
                return;
            }
        }
    }

    /// The comments, attributes and declarations of `contents`, by `deadline`.
    fn outline(&mut self, contents: &[u8], deadline: Instant) -> Result<Syntax, String> {
        let late = || "it did not finish within its time".to_owned();
        let mut progress = |_: &tree_sitter::ParseState| match Instant::now() < deadline {
            true => ControlFlow::Continue(()),
            false => ControlFlow::Break(()),
        };
        let options = ParseOptions::new().progress_callback(&mut progress);
        let mut read = |at: usize, _| contents.get(at..).unwrap_or_default();
        let tree = self
            .parser
            .parse_with_options(&mut read, None, Some(options));
        self.parser.reset();
        let Some(tree) = tree else {
            return Err(match Instant::now() < deadline {
                true => "it stopped without a tree (a trap, or memory beyond its limit)".to_owned(),
                false => late(),
            });
        };
        let mut syntax = Syntax::default();
        nodes(&tree, &mut syntax);
        let mut progress = |_: &tree_sitter::QueryCursorState| match Instant::now() < deadline {
            true => ControlFlow::Continue(()),
            false => ControlFlow::Break(()),
        };
        let options = QueryCursorOptions::new().progress_callback(&mut progress);
        let mut cursor = QueryCursor::new();
        let mut matches =
            cursor.matches_with_options(&self.query, tree.root_node(), contents, options);
        while let Some(found) = matches.next() {
            let definition = found
                .captures()
                .iter()
                .find(|capture| self.definitions[capture.index as usize]);
            let name = found
                .captures()
                .iter()
                .find(|capture| Some(capture.index) == self.name);
            if let (Some(definition), Some(name)) = (definition, name) {
                let text = &contents[name.node.byte_range()];
                syntax.declarations.push(SyntaxDeclaration {
                    name: String::from_utf8_lossy(text).into_owned(),
                    range: definition.node.byte_range(),
                    parent_start: definition.node.parent().map(|parent| parent.start_byte()),
                });
            }
        }
        if Instant::now() >= deadline {
            return Err(late());
        }
        Ok(syntax)
    }
}

/// The comment and attribute nodes of `tree`, into `syntax`; none inside another.
fn nodes(tree: &Tree, syntax: &mut Syntax) {
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        let kind = node.kind();
        let leaf = if COMMENTS.contains(&kind) {
            syntax.comments.push(node.byte_range());
            true
        } else if ATTRIBUTES.contains(&kind) {
            syntax.attributes.push(node.byte_range());
            true
        } else {
            false
        };
        if !leaf && cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}

/// A grammar that never returns, for tests in debug builds (ADR-DEBUG-PAUSE-POINT): with
/// `MARA_TEST_GRAMMAR_STALL` set, a file whose contents hold its value is never parsed.
fn stall(contents: &[u8]) {
    let Some(text) = hooks::value(Hook::GrammarStall) else {
        return;
    };
    let text = text.into_encoded_bytes();
    if contents.windows(text.len()).any(|window| window == text) {
        park_forever();
    }
}

/// Parks the calling thread for the rest of the process.
fn park_forever() -> ! {
    loop {
        thread::park();
    }
}

/// A load to hold, for tests in debug builds (ADR-DEBUG-PAUSE-POINT): records the event `load`,
/// and with `MARA_TEST_GRAMMAR_LOAD_STALL` set to the grammar's tree-sitter name, its thread
/// stops before it loads anything: until `hold` is gone from the directory that
/// `MARA_TEST_GRAMMAR_EVENTS` names, or for the rest of the process without that directory.
fn stall_load(symbol: &str) {
    event("load", symbol);
    if hooks::value(Hook::GrammarLoadStall).is_none_or(|name| name != symbol) {
        return;
    }
    let Some(events) = hooks::path(Hook::GrammarEvents) else {
        park_forever();
    };
    let hold = events.join("hold");
    while hold.exists() {
        thread::sleep(Duration::from_millis(10));
    }
}

/// Records `what` for the grammar `name`, for tests in debug builds (ADR-DEBUG-PAUSE-POINT):
/// with `MARA_TEST_GRAMMAR_EVENTS` naming a directory, appends the line `<what> <name>` to its
/// file `events`. The events are `load`, when a grammar's thread starts to load it, and `wait`,
/// when a request finds a load of the grammar it needs in progress and waits for it.
fn event(what: &str, name: &str) {
    hooks::append(Hook::GrammarEvents, format_args!("{what} {name}"));
}
