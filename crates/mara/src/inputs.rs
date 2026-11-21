//! A project's inputs, tracked between requests (DES-INPUTS-AND-KEYS and DES-SNAPSHOT in
//! `docs/engine.mara.md`): the inventory of the inputs the configuration declares, each read and
//! hashed only when its stat changed, each document parsed only when its contents changed, and
//! the snapshot rebuilt only when its key changed.
//!
//! Code files are inputs too, read only by a request that reads code (DES-CODE-READERS in
//! `docs/code/index.mara.md`): each is read and hashed only when its stat changed, and scanned for
//! markers only when its contents, its language's comment prefixes or its grammar changed. A
//! language's pinned grammar is loaded once per process, again only when its files' stats change
//! ([`crate::grammar`]).
//!
//! The CLI starts each request with a new [`Inputs`], so it reads everything once. A host that
//! keeps one between requests does work only for what changed. The daemon keeps one per session
//! and gives each request a view of it ([`Inputs::shared`]): the view checks and derives on the
//! session's inputs, holding them only for that, and keeps what that check found for the rest
//! of the request, so that an edit is computed and checked against the same contents. After a
//! write, the view gives the session what the write read back ([`Inputs::written`]).

mod admit;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use mara_engine::code::{CodeFile, CodeLayer, LanguageKey, MarkerKey, ScannedFile};
use mara_engine::config::{Config, Language};
use mara_engine::diagnostic::Diagnostic;
use mara_engine::document::DocumentKey;
use mara_engine::hash::Hash;
use mara_engine::pattern::{Patterns, Positions};
use mara_engine::snapshot::{DocumentInput, ParsedFile, Snapshot, SnapshotInputs};
use mara_engine::work::WorkCounts;

use crate::grammar::{self, Grammar, ParseFailure};
use crate::project::{Project, ProjectFiles, SYMBOLIC_LINK, SelectError, recognize};
use crate::session::{Deriving, Evaluated};
use crate::tracked::{self, Check, Keep, Kind, Listing, Reads, Stat, Tracked};
use crate::write::{self, Basis, FileWrite, TEMPORARY_PREFIX, Version, WriteError, WriteLock};

pub use admit::{Inadmissible, NOT_AN_INPUT};

/// The inputs of the project at one root, as of the last request.
#[derive(Debug)]
pub struct Inputs {
    root: PathBuf,
    files: ProjectFiles,
    /// The request this value or view serves ([`next_request`]): the calls of one request load
    /// the project once.
    request: u64,
    /// The project as loaded by the request of the given number.
    project: Option<(u64, Arc<Project>)>,
    /// The project files that request read, for their other roles in it.
    reads: Reads,
    /// Shared with the views that a request keeps after its check ([`Inputs::shared`]).
    state: Arc<State>,
    snapshot: Option<Arc<Snapshot>>,
    /// The inputs that a view checks and derives on: those of a daemon's session.
    shared: Option<Arc<Mutex<Inputs>>>,
    /// The outcomes of the session's evaluations, for a view.
    evaluated: Option<Arc<Evaluated>>,
    /// The session's background derivation, which a write served by a view keeps out of its
    /// window ([`Inputs::apply`]).
    deriving: Option<Arc<Deriving>>,
}

/// What the inventory found, by path relative to the project root.
#[derive(Clone, Debug, Default)]
struct State {
    directories: HashMap<String, Listing>,
    /// By the path of the directory that holds the file.
    gitignores: HashMap<String, Tracked<Compiled>>,
    documents: HashMap<String, Tracked<Arc<ParsedFile>>>,
    programs: HashMap<String, Tracked<Arc<Vec<u8>>>>,
    code: HashMap<String, Tracked<Scan>>,
}

/// The markers of a code file's contents, with their key.
#[derive(Clone, Debug)]
struct Scan {
    key: MarkerKey,
    file: Arc<ScannedFile>,
}

/// What a request does with the code files that the configuration declares
/// (DES-CODE-READERS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeReads {
    /// Nothing: no directory is listed for code and no code file is read.
    None,
    /// Lists them without reading any, so that the request can read one ([`Inputs::read_code`])
    /// or, once it knows it needs them, all ([`Inputs::scan_listed`]).
    List,
    /// Reads, hashes and scans every one.
    Scan,
}

/// A `.gitignore` file compiled by the `ignore` crate, or the line (1 for the whole file) and the
/// reason it cannot be applied.
type Compiled = Result<Arc<Gitignore>, (usize, String)>;

/// What one request found.
#[derive(Debug)]
pub struct Generation {
    pub project: Arc<Project>,
    /// Inputs that cannot be read or are symbolic links, and places that may hide inputs, in
    /// path order. The configuration's and the schema's problems are the project's.
    pub diagnostics: Vec<Diagnostic>,
    pub snapshot: Arc<Snapshot>,
    /// The contents of every file that a pattern of `[rules] programs` declares, by path
    /// (DES-PROGRAM-PROFILE).
    pub programs: BTreeMap<String, Vec<u8>>,
    /// Every code file read and scanned, with [`CodeReads::Scan`].
    pub code: Option<Arc<CodeLayer>>,
    /// The paths of the code files listed, in path order, each with the index of its language
    /// in the configuration, with [`CodeReads::List`].
    pub code_paths: Vec<(String, usize)>,
    /// With [`CodeReads::List`], the listing's problems that may hide code, for
    /// [`Inputs::scan_listed`].
    listed_unread: Vec<Diagnostic>,
    /// For a request in a daemon's session, the outcomes of the session's evaluations, which
    /// an evaluation reads before it runs Soufflé (DES-DAEMON-SESSIONS).
    pub(crate) evaluated: Option<Arc<Evaluated>>,
}

/// A document that a write put on disk, as the session takes it ([`Inputs::written`]).
#[derive(Debug)]
struct Written {
    absolute: PathBuf,
    stat: Stat,
    hash: Hash,
    file: Arc<ParsedFile>,
}

impl Inputs {
    /// The inputs of the project at `root`, before any request.
    pub fn new(root: PathBuf) -> Inputs {
        Inputs {
            root: fs::canonicalize(&root).unwrap_or(root),
            files: ProjectFiles::default(),
            request: next_request(),
            project: None,
            reads: Reads::default(),
            state: Arc::default(),
            snapshot: None,
            shared: None,
            evaluated: None,
            deriving: None,
        }
    }

    /// A view of `shared`, the inputs a daemon's session keeps: [`Inputs::project`] and
    /// [`Inputs::generation`] check and derive on them, holding their lock only meanwhile, so a
    /// request never repeats work that another did and waits only for a check in progress. The
    /// view then answers [`Inputs::basis`] and [`Inputs::admits`] from what its own check found,
    /// whatever later requests find. Its generations carry `evaluated`, the outcomes of the
    /// session's evaluations, and its writes keep `deriving`, the session's background
    /// derivation, out of their window ([`Inputs::apply`]).
    pub fn shared(
        shared: Arc<Mutex<Inputs>>,
        evaluated: Arc<Evaluated>,
        deriving: Arc<Deriving>,
    ) -> Inputs {
        let root = lock(&shared).root.clone();
        Inputs {
            shared: Some(shared),
            evaluated: Some(evaluated),
            deriving: Some(deriving),
            ..Inputs::new(root)
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directories that the last request listed, relative to the root (`""` for the root):
    /// those that may hold inputs, which a daemon watches (DES-DAEMON-SESSIONS).
    pub fn directories(&self) -> Vec<String> {
        self.state.directories.keys().cloned().collect()
    }

    /// The schema's path as the last loaded configuration names it.
    pub fn schema_path(&self) -> Option<String> {
        let (_, project) = self.project.as_ref()?;
        Some(project.config.schema.as_ref()?.path.clone())
    }

    /// Takes on what the shared inputs found in the check that just ended.
    fn adopt(&mut self, shared: &Inputs) {
        self.project.clone_from(&shared.project);
        self.state = Arc::clone(&shared.state);
        self.snapshot.clone_from(&shared.snapshot);
    }

    /// Loads the configuration and the schema ([`crate::project::load`]) in the request that
    /// began at `now`, reading only the files whose stat changed since the last request. A
    /// second call of the same request returns the same project and does no work. A request is
    /// a view ([`Inputs::shared`]), or the calls on a value up to and including its next
    /// [`Inputs::generation`]. Requests are told apart by their number, never by `now`: a clock
    /// that stands still or steps back makes no file look checked.
    pub fn project(&mut self, now: SystemTime, work: &mut WorkCounts) -> Arc<Project> {
        if let Some(shared) = self.shared.clone() {
            let mut shared = lock(&shared);
            let project = shared.project_for(self.request, now, work, Keep::None);
            self.adopt(&shared);
            return project;
        }
        self.project_for(self.request, now, work, Keep::None)
    }

    /// [`Inputs::project`] for the request numbered `request`, keeping unsettled files as
    /// `keep` says.
    fn project_for(
        &mut self,
        request: u64,
        now: SystemTime,
        work: &mut WorkCounts,
        keep: Keep,
    ) -> Arc<Project> {
        if let Some((loaded, project)) = &self.project
            && *loaded == request
        {
            return Arc::clone(project);
        }
        self.reads = Reads::new(keep);
        let project = self
            .files
            .load(self.root.clone(), now, work, &mut self.reads);
        let project = Arc::new(project);
        self.project = Some((request, Arc::clone(&project)));
        project
    }

    /// The basis of the document at `path` for an edit: the contents that the last snapshot
    /// holds, as that request read them. `None` when it holds no document at `path`.
    pub fn basis(&self, path: &str) -> Option<Basis> {
        let version = match self.state.documents.get(path) {
            Some(tracked) => Version::of(tracked),
            None => Version::of(self.state.code.get(path)?),
        };
        Some(Basis::Contents(version))
    }

    /// Reads and hashes the code file at `path`, which the last request listed
    /// ([`CodeReads::List`]), in the request that began at `now`: its hash and contents, or the
    /// `io-error` of a file that cannot be read.
    pub fn read_code(
        &self,
        path: &str,
        now: SystemTime,
        work: &mut WorkCounts,
    ) -> Result<(Hash, Vec<u8>), Box<Diagnostic>> {
        let absolute = self.root.join(path);
        let checked = fs::symlink_metadata(&absolute).and_then(|metadata| {
            let reads = &mut Reads::default();
            tracked::check::<()>(&absolute, &metadata, None, now, work, reads, false)
        });
        match checked {
            Ok(Check::Read(read)) => Ok((read.hash, read.contents)),
            Ok(Check::Kept(_)) => unreachable!("a check without a previous value reads"),
            Err(error) => {
                let message = format!("cannot read {path}: {error}");
                Err(Box::new(Diagnostic::error("io-error", path, 1, message)))
            }
        }
    }

    /// Scans `contents`, the code file at `path` of the language at index `language`, which the
    /// last request listed ([`CodeReads::List`]) and [`Inputs::read_code`] read with hash `hash`,
    /// when its language pins a grammar: what `get` of a code location reads at the grammar
    /// level. `None` at the basic level, which needs no scan, with the diagnostics of a grammar
    /// that cannot be used or could not scan the file. The key is the scan's.
    pub fn scan_code(
        &mut self,
        path: &str,
        language: usize,
        hash: Hash,
        contents: &[u8],
        work: &mut WorkCounts,
    ) -> (Option<(MarkerKey, Arc<ScannedFile>)>, Vec<Diagnostic>) {
        let Some((_, project)) = &self.project else {
            return (None, Vec::new());
        };
        let Some(language) = project.config.code.get(language) else {
            return (None, Vec::new());
        };
        let grammar = match grammar::get(language) {
            None => return (None, Vec::new()),
            Some(Err(diagnostic)) => return (None, vec![diagnostic]),
            Some(Ok(grammar)) => grammar,
        };
        let key = MarkerKey::new(key_of(language, true), hash);
        let kept = self.state.code.get(path).map(|tracked| &tracked.value);
        let scanned = match kept.filter(|scan| scan.key == key) {
            Some(scan) => Arc::clone(&scan.file),
            None => {
                let prefixes = &language.prefixes;
                match grammar.parse(contents, path) {
                    Ok(syntax) => {
                        Arc::new(ScannedFile::scan_syntax(contents, prefixes, &syntax, work))
                    }
                    Err(ParseFailure::File(reason)) => {
                        Arc::new(ScannedFile::fallback(contents, prefixes, reason, work))
                    }
                    Err(ParseFailure::Abandoned) => {
                        let reason = grammar.abandoned().unwrap_or_default();
                        return (None, vec![grammar::unavailable(language, &reason)]);
                    }
                }
            }
        };
        let diagnostics = (scanned.fallback.iter())
            .map(|reason| grammar::fallback(language, path, reason))
            .collect();
        (Some((key, scanned)), diagnostics)
    }

    /// The index of the language of the code file at `path`, in the configuration, as the
    /// request that began at `now` lists it, with the listing's diagnostics: what `get` of a code
    /// location reads instead of a [`Generation`] (DES-CODE-READERS). `None` when the
    /// configuration declares no code file at `path`: no language's include pattern matches it,
    /// or it is a document, a program or a temporary file, which lists nothing; or the listing
    /// does not find it. Lists only the directories where a code include pattern can match, as
    /// [`CodeReads::List`] does, and reads no document or program and builds no snapshot. The
    /// listing starts from the directories and `.gitignore` files of the last check, each
    /// listed or read again only when its stat says so, and changes nothing that these inputs
    /// or a session keep.
    pub fn code_file(
        &mut self,
        path: &str,
        now: SystemTime,
        work: &mut WorkCounts,
    ) -> (Option<usize>, Vec<Diagnostic>) {
        let project = self.project(now, work);
        let config = &project.config;
        let patterns = |paths: &mut dyn Iterator<Item = &mara_engine::config::PathSetting>| {
            Patterns::new(
                paths
                    .map(|pattern| pattern.path.as_str())
                    .collect::<Vec<_>>(),
            )
        };
        let documents = patterns(&mut config.include.iter().flatten());
        let programs = patterns(&mut config.rules.iter().flat_map(|rules| &rules.programs));
        let code = patterns(&mut config.code.iter().flat_map(|language| &language.include));
        let name = path.rsplit('/').next().unwrap_or(path);
        let excluded = (name.ends_with(".mara.md") && documents.matches(path))
            || programs.matches(path)
            || name.starts_with(TEMPORARY_PREFIX);
        if excluded || !code.matches(path) {
            return (None, Vec::new());
        }
        let config = Config {
            include: None,
            rules: None,
            ..config.clone()
        };
        let previous = State {
            directories: self.state.directories.clone(),
            gitignores: self.state.gitignores.clone(),
            ..State::default()
        };
        let mut problems = Vec::new();
        let code = Code::new(&config, CodeReads::List, &previous, &mut problems);
        let mut reads = Reads::default();
        let mut walk = Walk::new(&self.root, &config, code, previous, now, work, &mut reads);
        walk.run();
        let listed = (walk.code.paths.iter()).find(|(listed, _)| listed == path);
        let language = listed.map(|&(_, language)| language);
        let mut diagnostics = walk.diagnostics;
        diagnostics.extend(problems);
        diagnostics.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        (language, diagnostics)
    }

    /// Reads, hashes and scans every code file that `generation`, of the request that began at
    /// `now` with [`CodeReads::List`], listed, as [`CodeReads::Scan`] would have, through the
    /// same scan: each read only when its stat changed, scanned only when its contents, its
    /// language's prefixes or its grammar changed, and every file of a language whose grammar
    /// is dropped meanwhile scanned at the basic level. Returns the layer, whose problems are the
    /// listing's that may hide code and each code file that cannot be read, and the diagnostics
    /// of the scan: those of the layer's own problems, `unavailable-grammar` and fallbacks.
    ///
    /// A view scans without holding the session's inputs ([`Inputs::shared`]): it copies the
    /// session's record of the code files under its lock, reads, loads grammars and parses
    /// without it, and gives back under the lock again only the results whose file still has
    /// the stat it was read with. So no request waits for another's scan, and a scan that a
    /// change overtook is not kept. The view keeps its own results, which an edit is checked
    /// against.
    pub fn scan_listed(
        &mut self,
        generation: &Generation,
        now: SystemTime,
        work: &mut WorkCounts,
    ) -> (Arc<CodeLayer>, Vec<Diagnostic>) {
        let Some(shared) = self.shared.clone() else {
            // Copied only while a view of an earlier check still holds it.
            let mut state = Arc::unwrap_or_clone(std::mem::take(&mut self.state));
            let previous = std::mem::take(&mut state.code);
            let (code, layer, diagnostics) = scan(&self.root, generation, previous, now, work);
            state.code = code;
            self.state = Arc::new(state);
            return (layer, diagnostics);
        };
        let previous = lock(&shared).state.code.clone();
        let (code, layer, diagnostics) = scan(&self.root, generation, previous, now, work);
        give_back(&shared, &self.root, &code);
        let mut state = Arc::unwrap_or_clone(std::mem::take(&mut self.state));
        state.code = code;
        self.state = Arc::new(state);
        (layer, diagnostics)
    }

    /// Lists the inputs in the request that began at `now`, reads and hashes those whose stat
    /// changed, parses the documents whose contents changed, and builds the snapshot when its
    /// key changed. A project whose configuration or schema has errors still gets a snapshot,
    /// so that validation can report on it.
    pub fn snapshot(&mut self, now: SystemTime, work: &mut WorkCounts) -> Generation {
        self.generation(now, work, CodeReads::None)
    }

    /// [`Inputs::snapshot`], doing with code files what `code` says. [`CodeReads::Scan`] lists
    /// the code files with the inputs, then scans them ([`Inputs::scan_listed`]), so that a
    /// view holds the session's inputs only for the listing.
    pub fn generation(
        &mut self,
        now: SystemTime,
        work: &mut WorkCounts,
        code: CodeReads,
    ) -> Generation {
        let listing = match code {
            CodeReads::Scan => CodeReads::List,
            other => other,
        };
        let mut generation = match self.shared.clone() {
            Some(shared) => {
                let mut shared = lock(&shared);
                let derivation = Derivation::Request(listing);
                let generation = shared.generation_for(self.request, now, work, derivation);
                self.adopt(&shared);
                generation
            }
            None => {
                let derivation = Derivation::Request(listing);
                let generation = self.generation_for(self.request, now, work, derivation);
                // A generation ends its request: the next call on this value begins another.
                self.request = next_request();
                generation
            }
        };
        if code == CodeReads::Scan {
            let (layer, diagnostics) = self.scan_listed(&generation, now, work);
            generation.code = Some(layer);
            generation.diagnostics.extend(diagnostics);
            (generation.diagnostics).sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        }
        generation.evaluated.clone_from(&self.evaluated);
        generation
    }

    /// [`Inputs::snapshot`] as the daemon's background derivation after a watched change does it
    /// (DES-DAEMON-SESSIONS), on a session's view: a file whose stat is that of its last check
    /// and that a check at `now` would still leave unsettled is not read again. Such a read
    /// could not settle it, so the next request's own check reads it all the same; that check,
    /// never this one, finds an edit made in the timestamp tick of the last (REQ-NEVER-STALE).
    /// The configuration and the schema are kept alike. `publish` gets the work done while the
    /// session's inputs are still held, so that it reaches the response of the next request
    /// that uses what this derived.
    pub fn rederive(&mut self, now: SystemTime, publish: impl FnOnce(&WorkCounts)) {
        let derivation = Derivation::Background;
        let mut work = WorkCounts::default();
        match self.shared.clone() {
            Some(shared) => {
                let mut shared = lock(&shared);
                drop(shared.generation_for(self.request, now, &mut work, derivation));
                publish(&work);
            }
            None => {
                drop(self.generation_for(self.request, now, &mut work, derivation));
                self.request = next_request();
                publish(&work);
            }
        }
    }

    /// Writes the edit `files` into the project that `lock` locks ([`write::apply`]), in the
    /// request that began at `now`. `parsed` holds the read-back parse of each document of the
    /// edit, which a session's view then hands over ([`Inputs::written`]). From before the
    /// first rename until that hand-over, the session's background derivation does not run: one
    /// in progress is waited for, and the next waits for the hand-over, so that none reads and
    /// parses a file that the hand-over brings with its read-back parse.
    pub fn apply(
        &self,
        lock: &WriteLock,
        files: &[FileWrite],
        parsed: impl IntoIterator<Item = (String, ParsedFile)>,
        now: SystemTime,
        work: &mut WorkCounts,
    ) -> Result<(), WriteError> {
        let _window = self.deriving.as_ref().map(Deriving::window);
        write::apply(lock, files, work)?;
        self.written(parsed, now, work);
        Ok(())
    }

    /// After the renames of a write that the request which began at `now` made, on a session's
    /// view: gives the session `files`, each document the write put on disk with its read-back
    /// parse (ADR-READ-BACK-EVERY-EDIT), and derives the session's snapshot from them
    /// (DES-WRITE-PROCEDURE step 4). Hashes each file's contents, counting one hash and no file
    /// read, and parses nothing; every other input whose stat is that of its last check is kept
    /// without reading it, since the write changed none ([`Keep::Unchanged`]). Each file's stat is taken now but recorded as checked at
    /// `now`, before the renames, so the next check reads and hashes it again until a check
    /// settles it (DES-INPUTS-AND-KEYS): a change made by another program after the rename is
    /// never taken for the written contents. A file that is no longer a regular file is left
    /// to the next check. Outside a session it does nothing: a direct CLI call derives nothing
    /// after the renames (ADR-NO-REDERIVATION-IN-CLI).
    fn written(
        &self,
        files: impl IntoIterator<Item = (String, ParsedFile)>,
        now: SystemTime,
        work: &mut WorkCounts,
    ) {
        let Some(shared) = self.shared.clone() else {
            return;
        };
        let mut written = Vec::new();
        for (path, file) in files {
            let absolute = self.root.join(path);
            let Ok(metadata) = fs::symlink_metadata(&absolute) else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            written.push(Written {
                absolute,
                stat: Stat::of(&metadata),
                hash: Hash::of_file(&file.contents, work),
                file: Arc::new(file),
            });
        }
        let derivation = Derivation::Written(written);
        drop(lock(&shared).generation_for(self.request, now, work, derivation));
    }

    /// [`Inputs::generation`] for the request numbered `request`, deriving as `derivation`
    /// says.
    fn generation_for(
        &mut self,
        request: u64,
        now: SystemTime,
        work: &mut WorkCounts,
        derivation: Derivation,
    ) -> Generation {
        let (code, keep, written) = match derivation {
            Derivation::Request(code) => (code, Keep::None, Vec::new()),
            Derivation::Background => (CodeReads::None, Keep::Unsettleable, Vec::new()),
            Derivation::Written(written) => (CodeReads::None, Keep::Unchanged, written),
        };
        let project = self.project_for(request, now, work, keep);
        self.reads.keep = keep;
        // Copied only while a view of an earlier check still holds it.
        let previous = Arc::unwrap_or_clone(std::mem::take(&mut self.state));
        for file in &written {
            let contents = Arc::from(&*file.file.contents);
            (self.reads).insert(file.absolute.clone(), file.stat, file.hash, contents);
        }
        let reads = &mut self.reads;
        let config = &project.config;
        let mut grammar_problems = Vec::new();
        let code = Code::new(config, code, &previous, &mut grammar_problems);
        let mut walk = Walk::new(&self.root, config, code, previous, now, work, reads);
        for file in written {
            walk.parsed.insert(DocumentKey::new(file.hash), file.file);
        }
        walk.diagnostics.extend(grammar_problems);
        walk.run();
        let Walk {
            mut next,
            mut previous,
            documents,
            programs,
            mut diagnostics,
            code: found,
            ..
        } = walk;
        if found.reads != Some(CodeReads::Scan) {
            // Code tracked by an earlier request stays for the next one that reads code.
            next.code = std::mem::take(&mut previous.code);
        }
        self.state = Arc::new(next);
        self.reads = Reads::default();
        diagnostics.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        let inputs = SnapshotInputs::new(
            documents,
            project.schema.clone(),
            project.schema_hash,
            project.config_hash,
        );
        let snapshot = match &self.snapshot {
            Some(snapshot) if snapshot.key() == inputs.key() => Arc::clone(snapshot),
            _ => Arc::new(inputs.build(work)),
        };
        self.snapshot = Some(Arc::clone(&snapshot));
        let mut code_paths = found.paths;
        code_paths.sort_unstable();
        let (layer, listed_unread) = match found.reads {
            Some(CodeReads::Scan) => (
                Some(Arc::new(CodeLayer::new(found.files, found.unread))),
                Vec::new(),
            ),
            _ => (None, found.unread),
        };
        Generation {
            project,
            diagnostics,
            snapshot,
            programs,
            code: layer,
            code_paths,
            listed_unread,
            evaluated: None,
        }
    }
}

/// What a derivation of the inputs does.
enum Derivation {
    /// A request's, with code files as [`CodeReads`] says.
    Request(CodeReads),
    /// The daemon's background derivation ([`Inputs::rederive`]).
    Background,
    /// After a write ([`Inputs::written`]): the documents it wrote are not read again.
    Written(Vec<Written>),
}

/// Reads, hashes and scans the code files that `generation` listed, against `previous`, the
/// code files as last read: the record of the code files after the scan, the layer and the
/// scan's diagnostics ([`Inputs::scan_listed`]).
fn scan(
    root: &Path,
    generation: &Generation,
    previous: HashMap<String, Tracked<Scan>>,
    now: SystemTime,
    work: &mut WorkCounts,
) -> (
    HashMap<String, Tracked<Scan>>,
    Arc<CodeLayer>,
    Vec<Diagnostic>,
) {
    let config = &generation.project.config;
    let previous = State {
        code: previous,
        ..State::default()
    };
    let mut grammar_problems = Vec::new();
    let code = Code::new(config, CodeReads::Scan, &previous, &mut grammar_problems);
    let mut reads = Reads::default();
    let mut walk = Walk::new(root, config, code, previous, now, work, &mut reads);
    walk.diagnostics.extend(grammar_problems);
    for (path, language) in &generation.code_paths {
        walk.code_file(path.clone(), *language);
    }
    let Walk {
        next,
        mut diagnostics,
        code: found,
        ..
    } = walk;
    let mut unread = generation.listed_unread.clone();
    unread.extend(found.unread);
    diagnostics.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
    let layer = Arc::new(CodeLayer::new(found.files, unread));
    (next.code, layer, diagnostics)
}

/// Gives the session `shared` the results of a scan, `code`, made without its lock: only those
/// whose file still has the stat it was read with, so that a scan a change overtook is not
/// kept. The next check reads such a file again.
fn give_back(shared: &Mutex<Inputs>, root: &Path, code: &HashMap<String, Tracked<Scan>>) {
    let current: Vec<(String, Tracked<Scan>)> = (code.iter())
        .filter(|(path, tracked)| tracked.still_as_read(&root.join(path)))
        .map(|(path, tracked)| (path.clone(), tracked.clone()))
        .collect();
    let mut shared = lock(shared);
    Arc::make_mut(&mut shared.state).code.extend(current);
}

/// A number that no other request of this process has: what tells the calls of one request
/// from those of another.
fn next_request() -> u64 {
    static REQUESTS: AtomicU64 = AtomicU64::new(0);
    REQUESTS.fetch_add(1, Ordering::Relaxed)
}

/// The shared inputs, also after a request that held them panicked: what it left is checked
/// again by the next request like anything else.
fn lock(shared: &Mutex<Inputs>) -> std::sync::MutexGuard<'_, Inputs> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The code languages' include patterns, as a walk matches them, and what it found.
#[derive(Default)]
struct Code {
    reads: Option<CodeReads>,
    /// Every language's patterns, language by language in the configuration's order.
    patterns: Patterns,
    /// The language of each pattern.
    languages: Vec<usize>,
    keys: Vec<LanguageKey>,
    prefixes: Vec<Vec<String>>,
    /// The languages, in the configuration's order, and the grammar that scans each one's files
    /// at the grammar level.
    configured: Vec<Language>,
    grammars: Vec<Option<Arc<Grammar>>>,
    /// The scans of the last request and of this one, by key, so that two files with the same
    /// contents and language are scanned once.
    scanned: HashMap<MarkerKey, Arc<ScannedFile>>,
    files: Vec<CodeFile>,
    /// The language of each of `files`.
    file_languages: Vec<usize>,
    paths: Vec<(String, usize)>,
    /// Problems that may hide code.
    unread: Vec<Diagnostic>,
}

impl Code {
    /// What a walk does with code files. With [`CodeReads::Scan`], loads the grammar of each
    /// language that pins one, and puts the `unavailable-grammar` diagnostic of each that cannot
    /// be used into `problems`.
    fn new(
        config: &Config,
        reads: CodeReads,
        previous: &State,
        problems: &mut Vec<Diagnostic>,
    ) -> Code {
        if reads == CodeReads::None {
            return Code::default();
        }
        let loaded: Vec<Option<Arc<Grammar>>> = match reads {
            CodeReads::Scan => (config.code.iter())
                .map(|language| match grammar::get(language)? {
                    Ok(grammar) => Some(grammar),
                    Err(diagnostic) => {
                        problems.push(diagnostic);
                        None
                    }
                })
                .collect(),
            _ => vec![None; config.code.len()],
        };
        let mut patterns = Vec::new();
        let mut languages = Vec::new();
        for (index, language) in config.code.iter().enumerate() {
            for pattern in &language.include {
                patterns.push(pattern.path.as_str());
                languages.push(index);
            }
        }
        let scanned = previous
            .code
            .values()
            .map(|tracked| (tracked.value.key, Arc::clone(&tracked.value.file)))
            .collect();
        let keys = (config.code.iter().zip(&loaded))
            .map(|(language, grammar)| key_of(language, grammar.is_some()))
            .collect();
        Code {
            reads: Some(reads),
            patterns: Patterns::new(patterns),
            languages,
            keys,
            prefixes: config.code.iter().map(|l| l.prefixes.clone()).collect(),
            scanned,
            configured: config.code.clone(),
            grammars: loaded,
            ..Code::default()
        }
    }

    /// The language of the path that led to `at`: that of the first pattern that matches it.
    fn language(&self, at: &Positions) -> Option<usize> {
        self.reads?;
        Some(self.languages[self.patterns.first_match(at)?])
    }
}

/// One pass of the inventory over the project tree.
struct Walk<'w> {
    root: &'w Path,
    /// The document include patterns, then the program patterns.
    patterns: Patterns,
    /// How many of `patterns` are document include patterns.
    document_patterns: usize,
    now: SystemTime,
    work: &'w mut WorkCounts,
    reads: &'w mut Reads,
    /// What the last request found; each entry still found moves to `next`.
    previous: State,
    next: State,
    /// The parsed documents of the last request and of this one, by key, so that a moved or
    /// copied document is not parsed again.
    parsed: HashMap<DocumentKey, Arc<ParsedFile>>,
    documents: Vec<DocumentInput>,
    programs: BTreeMap<String, Vec<u8>>,
    diagnostics: Vec<Diagnostic>,
    code: Code,
}

/// A directory to list: its path, where the document and code include patterns stand there,
/// and the `.gitignore` files that apply to it, root first, each with the path of its
/// directory.
struct Directory {
    path: String,
    at: Positions,
    code_at: Positions,
    ignores: Vec<(String, Arc<Gitignore>)>,
}

impl<'w> Walk<'w> {
    fn new(
        root: &'w Path,
        config: &Config,
        code: Code,
        previous: State,
        now: SystemTime,
        work: &'w mut WorkCounts,
        reads: &'w mut Reads,
    ) -> Walk<'w> {
        let documents = config.include.iter().flatten();
        let programs = config.rules.iter().flat_map(|rules| &rules.programs);
        let document_patterns = documents.clone().count();
        let patterns = documents.chain(programs);
        let parsed = previous
            .documents
            .values()
            .map(|tracked| (DocumentKey::new(tracked.hash), Arc::clone(&tracked.value)))
            .collect();
        Walk {
            root,
            patterns: Patterns::new(patterns.map(|pattern| pattern.path.as_str())),
            document_patterns,
            now,
            work,
            reads,
            previous,
            next: State::default(),
            parsed,
            documents: Vec::new(),
            programs: BTreeMap::new(),
            diagnostics: Vec::new(),
            code,
        }
    }

    /// Checks the file at `absolute` against `previous` ([`tracked::check`]).
    fn check<T>(&mut self, absolute: &Path, previous: Option<Tracked<T>>) -> io::Result<Check<T>> {
        let metadata = fs::symlink_metadata(absolute)?;
        let (now, work, reads) = (self.now, &mut *self.work, &mut *self.reads);
        tracked::check(absolute, &metadata, previous, now, work, reads, false)
    }

    /// Lists the project root and every directory below it in which a pattern can match.
    fn run(&mut self) {
        let at = self.patterns.start();
        let code_at = self.code.patterns.start();
        if !self.patterns.can_continue(&at) && !self.code.patterns.can_continue(&code_at) {
            return;
        }
        let mut pending = vec![Directory {
            path: String::new(),
            at,
            code_at,
            ignores: Vec::new(),
        }];
        while let Some(directory) = pending.pop() {
            self.directory(directory, &mut pending);
        }
    }

    fn directory(&mut self, directory: Directory, pending: &mut Vec<Directory>) {
        let Directory {
            path,
            at,
            code_at,
            mut ignores,
        } = directory;
        // Whether a problem here may hide a code file.
        let code_here = self.code.patterns.can_continue(&code_at);
        let absolute = self.root.join(&path);
        let previous = self.previous.directories.remove(&path);
        let listed = fs::symlink_metadata(&absolute)
            .and_then(|metadata| tracked::list(&absolute, &metadata, previous, self.now));
        let listing = match listed {
            Ok(listing) => listing,
            Err(error) => {
                let shown = if path.is_empty() { "." } else { &path };
                let message = format!("cannot list {shown}: {error}");
                return self.hiding(code_here, "io-error", shown.to_owned(), 1, message);
            }
        };
        let entries = Arc::clone(&listing.entries);
        self.next.directories.insert(path.clone(), listing);

        // A `.gitignore` that cannot be applied might ignore any entry, so none is an input.
        if let Ok(index) = entries.binary_search_by(|entry| entry.name.as_str().cmp(".gitignore")) {
            let file = join(&path, ".gitignore");
            let place = match path.as_str() {
                "" => "the project root".to_owned(),
                path => path.to_owned(),
            };
            let consequence =
                format!("so Mara cannot apply its patterns and reads nothing in {place} or below");
            let gitignore = match entries[index].kind {
                Kind::File => self.gitignore(&path, &consequence, code_here),
                Kind::SymbolicLink => {
                    let message = format!(
                        "{file} is a symbolic link, and Mara follows none among a project's \
                         inputs, {consequence}: put the file itself there"
                    );
                    self.hiding(code_here, SYMBOLIC_LINK, file, 1, message);
                    None
                }
                Kind::Directory | Kind::Other => {
                    let message = format!("{file} is not a regular file, {consequence}");
                    self.hiding(code_here, "io-error", file, 1, message);
                    None
                }
            };
            match gitignore {
                Some(gitignore) => ignores.push((path.clone(), gitignore)),
                None => return,
            }
        }

        for entry in entries.iter() {
            // Git keeps no path inside a `.git`, and neither does Mara.
            if entry.name == ".git" {
                continue;
            }
            let child = join(&path, &entry.name);
            let next = self.patterns.advance(&at, &entry.name);
            let code_next = self.code.patterns.advance(&code_at, &entry.name);
            if entry.kind == Kind::Directory {
                let code = self.code.patterns.can_continue(&code_next);
                if !(self.patterns.can_continue(&next) || code) || ignored(&ignores, &child, true) {
                    continue;
                }
                if !entry.utf8 {
                    self.unnamed(child, code);
                    continue;
                }
                match recognize(&self.root.join(&child)) {
                    Ok(None) => pending.push(Directory {
                        path: child,
                        at: next,
                        code_at: code_next,
                        ignores: ignores.clone(),
                    }),
                    // Another project's root: nothing below it is an input.
                    Ok(Some(_)) => {}
                    Err(SelectError::SymbolicLink { .. }) => {
                        let message = format!(
                            "{child}/.mara is a symbolic link, so Mara cannot tell whether \
                             {child} is another project's root and reads nothing below it: put \
                             the directory itself there"
                        );
                        let link = format!("{child}/.mara");
                        self.hiding(code, SYMBOLIC_LINK, link, 1, message);
                    }
                    Err(error) => {
                        let message = format!(
                            "cannot tell whether {child} is another project's root, so Mara \
                             reads nothing below it: {}",
                            self.relative(&error)
                        );
                        self.hiding(code, "io-error", child, 1, message);
                    }
                }
                continue;
            }
            let documents = 0..self.document_patterns;
            let document = entry.name.ends_with(".mara.md")
                && self.patterns.is_match_among(&next, documents.clone());
            let programs = self.document_patterns..usize::MAX;
            let program = self.patterns.is_match_among(&next, programs);
            // A temporary file that an interrupted write left is never a code file.
            let language = match document || program || entry.name.starts_with(TEMPORARY_PREFIX) {
                true => None,
                false => self.code.language(&code_next),
            };
            if !(document || program || language.is_some()) || ignored(&ignores, &child, false) {
                continue;
            }
            if !entry.utf8 {
                self.unnamed(child, language.is_some());
                continue;
            }
            let role = if document { "document" } else { "program" };
            match (language, entry.kind) {
                (None, Kind::File) => {
                    if program {
                        self.program(child.clone());
                    }
                    if document {
                        self.document(child);
                    }
                }
                (None, Kind::SymbolicLink) => {
                    let message = format!(
                        "{child} is a symbolic link, and Mara follows none among a project's \
                         inputs: put the {role} itself there"
                    );
                    self.problem(SYMBOLIC_LINK, child, message);
                }
                (Some(language), Kind::File) => self.code_file(child, language),
                // A link is no code input, and code files are not checked for links.
                (Some(_), Kind::SymbolicLink) => {}
                (language, Kind::Directory | Kind::Other) => {
                    let message = format!("cannot read {child}: it is not a regular file");
                    self.hiding(language.is_some(), "io-error", child, 1, message);
                }
            }
        }
    }

    /// Adds the code file at `path`, of the language at index `language`: listed, or read when
    /// its stat changed and scanned when its contents, its language's prefixes or its grammar
    /// changed.
    fn code_file(&mut self, path: String, language: usize) {
        if self.code.reads == Some(CodeReads::List) {
            self.code.paths.push((path, language));
            return;
        }
        let absolute = self.root.join(&path);
        let previous = self.previous.code.remove(&path).filter(|tracked| {
            tracked.value.key == MarkerKey::new(self.code.keys[language], tracked.hash)
        });
        let checked = self.check(&absolute, previous);
        let tracked = match checked {
            Ok(Check::Kept(tracked)) => tracked,
            Ok(Check::Read(read)) => {
                let (key, file) = self.scan(&path, language, read.hash, &read.contents);
                read.with(Scan { key, file })
            }
            Err(error) => {
                let message = format!("cannot read {path}: {error}");
                return self.hiding(true, "io-error", path, 1, message);
            }
        };
        if let Some(reason) = &tracked.value.file.fallback {
            let diagnostic = grammar::fallback(&self.code.configured[language], &path, reason);
            self.diagnostics.push(diagnostic);
        }
        self.code.files.push(CodeFile {
            path: path.clone(),
            key: tracked.value.key,
            hash: tracked.hash,
            scanned: Arc::clone(&tracked.value.file),
        });
        self.code.file_languages.push(language);
        self.next.code.insert(path, tracked);
    }

    /// Reads and scans again, at the basic level, each file of the language at index
    /// `language` that this request scanned with its grammar, once that grammar is dropped: a
    /// request reads all of a language's files at one level.
    fn rescan(&mut self, language: usize) {
        let mut index = 0;
        while index < self.code.files.len() {
            let file = &self.code.files[index];
            if self.code.file_languages[index] != language || file.scanned.outline.is_none() {
                index += 1;
                continue;
            }
            let path = file.path.clone();
            let absolute = self.root.join(&path);
            let checked = fs::symlink_metadata(&absolute).and_then(|metadata| {
                let (now, work, reads) = (self.now, &mut *self.work, &mut *self.reads);
                tracked::check::<Scan>(&absolute, &metadata, None, now, work, reads, false)
            });
            match checked {
                Ok(Check::Read(read)) => {
                    let (key, file) = self.scan(&path, language, read.hash, &read.contents);
                    self.code.files[index] = CodeFile {
                        path: path.clone(),
                        key,
                        hash: read.hash,
                        scanned: Arc::clone(&file),
                    };
                    self.next.code.insert(path, read.with(Scan { key, file }));
                    index += 1;
                }
                Ok(Check::Kept(_)) => unreachable!("a check without a previous value reads"),
                Err(error) => {
                    self.code.files.remove(index);
                    self.code.file_languages.remove(index);
                    self.next.code.remove(&path);
                    let message = format!("cannot read {path}: {error}");
                    self.hiding(true, "io-error", path, 1, message);
                }
            }
        }
    }

    /// The markers of `contents`, the file at `path` of the language at index `language`, with
    /// their key: scanned at its language's level unless another file had the same key. A
    /// grammar that is abandoned meanwhile leaves the language at the basic level, for the files
    /// scanned before too.
    fn scan(
        &mut self,
        path: &str,
        language: usize,
        hash: Hash,
        contents: &[u8],
    ) -> (MarkerKey, Arc<ScannedFile>) {
        loop {
            let key = MarkerKey::new(self.code.keys[language], hash);
            if let Some(file) = self.code.scanned.get(&key) {
                return (key, Arc::clone(file));
            }
            let prefixes = &self.code.prefixes[language];
            let file = match &self.code.grammars[language] {
                None => ScannedFile::scan(contents, prefixes, self.work),
                Some(grammar) => match grammar.parse(contents, path) {
                    Ok(syntax) => ScannedFile::scan_syntax(contents, prefixes, &syntax, self.work),
                    Err(ParseFailure::File(reason)) => {
                        ScannedFile::fallback(contents, prefixes, reason, self.work)
                    }
                    Err(ParseFailure::Abandoned) => {
                        let reason = grammar.abandoned().unwrap_or_default();
                        let configured = &self.code.configured[language];
                        self.diagnostics
                            .push(grammar::unavailable(configured, &reason));
                        self.code.grammars[language] = None;
                        self.code.keys[language] = key_of(configured, false);
                        self.rescan(language);
                        continue;
                    }
                },
            };
            let file = Arc::new(file);
            self.code.scanned.insert(key, Arc::clone(&file));
            return (key, file);
        }
    }

    /// The `.gitignore` file of the directory at `directory`, read and compiled only when it
    /// changed. A file that cannot be read or compiled is reported, with `consequence`.
    fn gitignore(
        &mut self,
        directory: &str,
        consequence: &str,
        code_here: bool,
    ) -> Option<Arc<Gitignore>> {
        let path = join(directory, ".gitignore");
        let absolute = self.root.join(&path);
        let previous = self.previous.gitignores.remove(directory);
        let checked = self.check(&absolute, previous);
        let tracked = match checked {
            Ok(Check::Kept(tracked)) => tracked,
            Ok(Check::Read(read)) => {
                let compiled = compile_gitignore(&read.contents).map(Arc::new);
                read.with(compiled)
            }
            Err(error) => {
                let message = format!("cannot read {path}, {consequence}: {error}");
                self.hiding(code_here, "io-error", path, 1, message);
                return None;
            }
        };
        let compiled = tracked.value.clone();
        self.next.gitignores.insert(directory.to_owned(), tracked);
        match compiled {
            Ok(gitignore) => Some(gitignore),
            Err((line, reason)) => {
                let message = format!("{reason}, {consequence}");
                self.hiding(code_here, UNSUPPORTED_GITIGNORE, path, line, message);
                None
            }
        }
    }

    /// Adds the document at `path`, read when its stat changed and parsed when its contents
    /// changed.
    fn document(&mut self, path: String) {
        let absolute = self.root.join(&path);
        let previous = self.previous.documents.remove(&path);
        let checked = self.check(&absolute, previous);
        let tracked = match checked {
            Ok(Check::Kept(tracked)) => tracked,
            Ok(Check::Read(mut read)) => {
                let key = DocumentKey::new(read.hash);
                let file = match self.parsed.get(&key) {
                    Some(file) => Arc::clone(file),
                    None => {
                        let contents = std::mem::take(&mut read.contents);
                        let file = Arc::new(ParsedFile::parse(contents, self.work));
                        self.parsed.insert(key, Arc::clone(&file));
                        file
                    }
                };
                read.with(file)
            }
            Err(error) => {
                let message = format!("cannot read {path}: {error}");
                return self.problem("io-error", path, message);
            }
        };
        self.documents.push(DocumentInput {
            path: path.clone(),
            key: DocumentKey::new(tracked.hash),
            file: Arc::clone(&tracked.value),
        });
        self.next.documents.insert(path, tracked);
    }

    /// Adds the program file at `path`, read when its stat changed.
    fn program(&mut self, path: String) {
        let absolute = self.root.join(&path);
        let previous = self.previous.programs.remove(&path);
        let checked = self.check(&absolute, previous);
        let tracked = match checked {
            Ok(Check::Kept(tracked)) => tracked,
            Ok(Check::Read(mut read)) => {
                let contents = Arc::new(std::mem::take(&mut read.contents));
                read.with(contents)
            }
            Err(error) => {
                let message = format!("cannot read {path}: {error}");
                return self.problem("io-error", path, message);
            }
        };
        let contents = tracked.value.to_vec();
        self.programs.insert(path.clone(), contents);
        self.next.programs.insert(path, tracked);
    }

    /// Reports an entry that may hold or be an input but whose name is not UTF-8.
    fn unnamed(&mut self, path: String, code: bool) {
        let message = format!("the name of {path} is not UTF-8, so Mara cannot read it: rename it");
        self.hiding(code, "io-error", path, 1, message);
    }

    /// A selection error with its path relative to the project root.
    fn relative(&self, error: &SelectError) -> String {
        match error {
            SelectError::Io { path, error } => {
                let path = path.strip_prefix(self.root).unwrap_or(path);
                format!("cannot read {}: {error}", path.display())
            }
            error => error.to_string(),
        }
    }

    fn problem(&mut self, code: &'static str, path: String, message: String) {
        self.hiding(false, code, path, 1, message);
    }

    /// [`Walk::problem`] at `line`, also of the code layer when `code` says it may hide a code
    /// file.
    fn hiding(&mut self, code: bool, kind: &'static str, path: String, line: usize, text: String) {
        let diagnostic = Diagnostic::error(kind, path, line, text);
        if code && self.code.reads.is_some() {
            self.code.unread.push(diagnostic.clone());
        }
        self.diagnostics.push(diagnostic);
    }
}

/// The key of `language`'s scans, at the grammar level when `grammar` is set.
fn key_of(language: &Language, grammar: bool) -> LanguageKey {
    let checksum = language.grammar.as_ref().map(|pin| &pin.checksum);
    LanguageKey::new(language, checksum.filter(|_| grammar))
}

/// `name` in the directory at `directory`, the project root when empty.
fn join(directory: &str, name: &str) -> String {
    if directory.is_empty() {
        name.to_owned()
    } else {
        format!("{directory}/{name}")
    }
}

/// Whether the `.gitignore` files `ignores`, root first, ignore `path`. The deepest file that
/// has a matching pattern decides, and within a file the last matching pattern, as in Git.
pub(crate) fn ignored(ignores: &[(String, Arc<Gitignore>)], path: &str, is_dir: bool) -> bool {
    ignores
        .iter()
        .rev()
        .find_map(|(directory, gitignore)| {
            let relative = match directory.len() {
                0 => path,
                length => &path[length + 1..],
            };
            match gitignore.matched(relative, is_dir) {
                Match::Ignore(_) => Some(true),
                Match::Whitelist(_) => Some(false),
                Match::None => None,
            }
        })
        .unwrap_or(false)
}

/// A `.gitignore` that Mara's matcher, the `ignore` crate, cannot apply (DES-INPUTS-AND-KEYS).
const UNSUPPORTED_GITIGNORE: &str = "unsupported-gitignore";

/// Compiles the contents of a `.gitignore` file to match paths relative to its directory, with
/// the `ignore` crate, given the lines as its own reader splits them: a UTF-8 byte order mark
/// skipped, and a CR before each LF removed. The crate takes a pattern only as text, and its
/// reasons are kept: a line that is not UTF-8, and a pattern it cannot compile, give the line;
/// a pattern set it cannot compile gives line 1. A file with any of them is never used in part.
pub(crate) fn compile_gitignore(contents: &[u8]) -> Result<Gitignore, (usize, String)> {
    // The root `.` keeps the matcher from stripping any prefix of the paths it is given.
    let mut builder = GitignoreBuilder::new(".");
    let contents = contents.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(contents);
    for (index, line) in contents.split(|&byte| byte == b'\n').enumerate() {
        let number = index + 1;
        // The crate skips a comment before it reads anything else of the line.
        if line.first() == Some(&b'#') {
            continue;
        }
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let Ok(line) = std::str::from_utf8(line) else {
            let reason =
                "this line is not UTF-8, and Mara reads `.gitignore` patterns only as UTF-8 text";
            return Err((number, reason.to_owned()));
        };
        if let Err(error) = builder.add_line(None, line) {
            return Err((
                number,
                format!("Mara cannot compile this pattern ({error})"),
            ));
        }
    }
    builder.build().map_err(|error| {
        (
            1,
            format!("Mara cannot compile the patterns of this file ({error})"),
        )
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const CONFIG: &str = "format_version = 1\n\n[documents]\nformat_version = 1\ninclude = [\"docs/**/*.mara.md\"]\n";

    /// A scan made without the session's lock gives back only what is still on disk: a file
    /// changed after the scan read it is not kept, and the next request reads it again.
    #[test]
    fn a_scan_that_a_change_overtook_is_not_kept() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".mara")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        let config = format!(
            "{CONFIG}\n[code]\nformat_version = 1\n\n[code.languages.rust]\n\
             include = [\"src/**/*.rs\"]\ncomment_prefixes = [\"//\"]\n"
        );
        fs::write(root.join(".mara/config.toml"), config).unwrap();
        let lib = root.join("src/lib.rs");
        fs::write(&lib, "// @mara implements REQ-A\n").unwrap();
        let session = Arc::new(Mutex::new(Inputs::new(root.to_owned())));
        let root = lock(&session).root.clone();
        let mut work = WorkCounts::default();
        let now = SystemTime::now();
        let hash = |session: &Mutex<Inputs>| lock(session).state.code["src/lib.rs"].hash;
        let first = Inputs::shared(Arc::clone(&session), Arc::default(), Arc::default())
            .generation(now, &mut work, CodeReads::Scan);
        let read_a = hash(&session);
        // A request scans B without the lock; B is replaced by C before it gives back.
        fs::write(&lib, "// @mara implements REQ-B\n").unwrap();
        let mut view = Inputs::shared(Arc::clone(&session), Arc::default(), Arc::default());
        let listed = view.generation(now, &mut work, CodeReads::List);
        let previous = lock(&session).state.code.clone();
        let (code, _, _) = scan(&root, &listed, previous, now, &mut work);
        assert_ne!(code["src/lib.rs"].hash, read_a, "the scan read B");
        fs::write(&lib, "// @mara implements REQ-CC\n").unwrap();
        give_back(&session, &root, &code);
        assert_eq!(hash(&session), read_a, "the overtaken scan of B was kept");
        // The next request reads C.
        let next = Inputs::shared(Arc::clone(&session), Arc::default(), Arc::default()).generation(
            now,
            &mut work,
            CodeReads::Scan,
        );
        let markers = |generation: &Generation| format!("{:?}", generation.code.as_ref().unwrap());
        assert!(markers(&next).contains("REQ-CC"), "{}", markers(&next));
        assert!(markers(&first).contains("REQ-A"));
    }

    /// Requests of a session whose clock stands still, then steps back: each checks the
    /// configuration again, so none is answered from one that changed.
    #[test]
    fn requests_with_the_same_time_each_check_the_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".mara")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join(".mara/config.toml"), CONFIG).unwrap();
        fs::write(root.join("docs/a.mara.md"), "Text.\n").unwrap();
        let session = Arc::new(Mutex::new(Inputs::new(root.to_owned())));
        let mut work = WorkCounts::default();
        let mut documents = |now: SystemTime| {
            let generation = Inputs::shared(Arc::clone(&session), Arc::default(), Arc::default())
                .snapshot(now, &mut work);
            generation.snapshot.documents().len()
        };
        let now = SystemTime::now();
        assert_eq!(documents(now), 1);
        let other = CONFIG.replace("docs/**", "other/**");
        fs::write(root.join(".mara/config.toml"), other).unwrap();
        assert_eq!(documents(now), 0, "answered from the old configuration");
        fs::write(root.join(".mara/config.toml"), CONFIG).unwrap();
        assert_eq!(documents(now - Duration::from_secs(3600)), 1);

        // A value a host keeps between requests, as the inventory's own tests do.
        let mut kept = Inputs::new(root.to_owned());
        let mut work = WorkCounts::default();
        assert_eq!(kept.snapshot(now, &mut work).snapshot.documents().len(), 1);
        let other = CONFIG.replace("docs/**", "other/**");
        fs::write(root.join(".mara/config.toml"), other).unwrap();
        let generation = kept.snapshot(now, &mut work);
        assert_eq!(
            generation.snapshot.documents().len(),
            0,
            "answered from the old one"
        );
    }
}
