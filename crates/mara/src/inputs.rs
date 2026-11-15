//! A project's inputs, tracked between requests (DES-INPUTS-AND-KEYS and DES-SNAPSHOT in
//! `docs/engine.mara.md`): the inventory of the inputs the configuration declares, each read and
//! hashed only when its stat changed, each document parsed only when its contents changed, and
//! the snapshot rebuilt only when its key changed.
//!
//! The CLI starts each request with a new [`Inputs`], so it reads everything once. A host that
//! keeps one between requests does work only for what changed.

mod admit;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use mara_engine::config::Config;
use mara_engine::diagnostic::Diagnostic;
use mara_engine::document::DocumentKey;
use mara_engine::pattern::{Patterns, Positions};
use mara_engine::snapshot::{DocumentInput, ParsedFile, Snapshot, SnapshotInputs};
use mara_engine::work::WorkCounts;

use crate::project::{Project, ProjectFiles, SYMBOLIC_LINK, SelectError, recognize};
use crate::tracked::{self, Check, Kind, Listing, Reads, Tracked};
use crate::write::{Basis, Version};

pub use admit::{Inadmissible, NOT_AN_INPUT};

/// The inputs of the project at one root, as of the last request.
#[derive(Debug)]
pub struct Inputs {
    root: PathBuf,
    files: ProjectFiles,
    /// The project as loaded by the request that began at the given time.
    project: Option<(SystemTime, Arc<Project>)>,
    /// The project files that request read, for their other roles in it.
    reads: Reads,
    state: State,
    snapshot: Option<Arc<Snapshot>>,
}

/// What the inventory found, by path relative to the project root.
#[derive(Debug, Default)]
struct State {
    directories: HashMap<String, Listing>,
    /// By the path of the directory that holds the file.
    gitignores: HashMap<String, Tracked<Compiled>>,
    documents: HashMap<String, Tracked<Arc<ParsedFile>>>,
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
}

impl Inputs {
    /// The inputs of the project at `root`, before any request.
    pub fn new(root: PathBuf) -> Inputs {
        Inputs {
            root: fs::canonicalize(&root).unwrap_or(root),
            files: ProjectFiles::default(),
            project: None,
            reads: Reads::default(),
            state: State::default(),
            snapshot: None,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Loads the configuration and the schema ([`crate::project::load`]) in the request that
    /// began at `now`, reading only the files whose stat changed since the last request. A
    /// second call with the same `now` returns the same project and does no work.
    pub fn project(&mut self, now: SystemTime, work: &mut WorkCounts) -> Arc<Project> {
        if let Some((loaded, project)) = &self.project
            && *loaded == now
        {
            return Arc::clone(project);
        }
        self.reads = Reads::default();
        let project = self
            .files
            .load(self.root.clone(), now, work, &mut self.reads);
        let project = Arc::new(project);
        self.project = Some((now, Arc::clone(&project)));
        project
    }

    /// The basis of the document at `path` for an edit: the contents that the last snapshot
    /// holds, as that request read them. `None` when it holds no document at `path`.
    pub fn basis(&self, path: &str) -> Option<Basis> {
        let tracked = self.state.documents.get(path)?;
        Some(Basis::Contents(Version::of(tracked)))
    }

    /// Lists the inputs in the request that began at `now`, reads and hashes those whose stat
    /// changed, parses the documents whose contents changed, and builds the snapshot when its
    /// key changed. A project whose configuration or schema has errors still gets a snapshot,
    /// so that validation can report on it.
    pub fn snapshot(&mut self, now: SystemTime, work: &mut WorkCounts) -> Generation {
        let project = self.project(now, work);
        let previous = std::mem::take(&mut self.state);
        let reads = &mut self.reads;
        let mut walk = Walk::new(&self.root, &project.config, previous, now, work, reads);
        walk.run();
        let Walk {
            next,
            documents,
            mut diagnostics,
            ..
        } = walk;
        self.state = next;
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
        Generation {
            project,
            diagnostics,
            snapshot,
        }
    }
}

/// One pass of the inventory over the project tree.
struct Walk<'w> {
    root: &'w Path,
    patterns: Patterns,
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
    diagnostics: Vec<Diagnostic>,
}

/// A directory to list: its path, where the include patterns stand there, and the `.gitignore`
/// files that apply to it, root first, each with the path of its directory.
struct Directory {
    path: String,
    at: Positions,
    ignores: Vec<(String, Arc<Gitignore>)>,
}

impl<'w> Walk<'w> {
    fn new(
        root: &'w Path,
        config: &Config,
        previous: State,
        now: SystemTime,
        work: &'w mut WorkCounts,
        reads: &'w mut Reads,
    ) -> Walk<'w> {
        let patterns = config.include.iter().flatten();
        let parsed = previous
            .documents
            .values()
            .map(|tracked| (DocumentKey::new(tracked.hash), Arc::clone(&tracked.value)))
            .collect();
        Walk {
            root,
            patterns: Patterns::new(patterns.map(|pattern| pattern.path.as_str())),
            now,
            work,
            reads,
            previous,
            next: State::default(),
            parsed,
            documents: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Lists the project root and every directory below it in which a pattern can match.
    fn run(&mut self) {
        let at = self.patterns.start();
        if !self.patterns.can_continue(&at) {
            return;
        }
        let mut pending = vec![Directory {
            path: String::new(),
            at,
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
            mut ignores,
        } = directory;
        let absolute = self.root.join(&path);
        let previous = self.previous.directories.remove(&path);
        let listed = fs::symlink_metadata(&absolute)
            .and_then(|metadata| tracked::list(&absolute, &metadata, previous, self.now));
        let listing = match listed {
            Ok(listing) => listing,
            Err(error) => {
                let shown = if path.is_empty() { "." } else { &path };
                let message = format!("cannot list {shown}: {error}");
                return self.problem("io-error", shown.to_owned(), message);
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
                Kind::File => self.gitignore(&path, &consequence),
                Kind::SymbolicLink => {
                    let message = format!(
                        "{file} is a symbolic link, and Mara follows none among a project's \
                         inputs, {consequence}: put the file itself there"
                    );
                    self.problem(SYMBOLIC_LINK, file, message);
                    None
                }
                Kind::Directory | Kind::Other => {
                    let message = format!("{file} is not a regular file, {consequence}");
                    self.problem("io-error", file, message);
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
            if entry.kind == Kind::Directory {
                if !self.patterns.can_continue(&next) || ignored(&ignores, &child, true) {
                    continue;
                }
                if !entry.utf8 {
                    self.unnamed(child);
                    continue;
                }
                match recognize(&self.root.join(&child)) {
                    Ok(None) => pending.push(Directory {
                        path: child,
                        at: next,
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
                        self.problem(SYMBOLIC_LINK, format!("{child}/.mara"), message);
                    }
                    Err(error) => {
                        let message = format!(
                            "cannot tell whether {child} is another project's root, so Mara \
                             reads nothing below it: {}",
                            self.relative(&error)
                        );
                        self.problem("io-error", child, message);
                    }
                }
                continue;
            }
            let document = entry.name.ends_with(".mara.md") && self.patterns.is_match(&next);
            if !document || ignored(&ignores, &child, false) {
                continue;
            }
            if !entry.utf8 {
                self.unnamed(child);
                continue;
            }
            match entry.kind {
                Kind::File => self.document(child),
                Kind::SymbolicLink => {
                    let message = format!(
                        "{child} is a symbolic link, and Mara follows none among a project's \
                         inputs: put the document itself there"
                    );
                    self.problem(SYMBOLIC_LINK, child, message);
                }
                Kind::Directory | Kind::Other => {
                    let message = format!("cannot read {child}: it is not a regular file");
                    self.problem("io-error", child, message);
                }
            }
        }
    }

    /// The `.gitignore` file of the directory at `directory`, read and compiled only when it
    /// changed. A file that cannot be read or compiled is reported, with `consequence`.
    fn gitignore(&mut self, directory: &str, consequence: &str) -> Option<Arc<Gitignore>> {
        let path = join(directory, ".gitignore");
        let absolute = self.root.join(&path);
        let previous = self.previous.gitignores.remove(directory);
        let checked = fs::symlink_metadata(&absolute).and_then(|metadata| {
            let (now, work, reads) = (self.now, &mut *self.work, &mut *self.reads);
            tracked::check(&absolute, &metadata, previous, now, work, reads, false)
        });
        let tracked = match checked {
            Ok(Check::Kept(tracked)) => tracked,
            Ok(Check::Read(read)) => {
                let compiled = compile_gitignore(&read.contents).map(Arc::new);
                read.with(compiled)
            }
            Err(error) => {
                let message = format!("cannot read {path}, {consequence}: {error}");
                self.problem("io-error", path, message);
                return None;
            }
        };
        let compiled = tracked.value.clone();
        self.next.gitignores.insert(directory.to_owned(), tracked);
        match compiled {
            Ok(gitignore) => Some(gitignore),
            Err((line, reason)) => {
                let message = format!("{reason}, {consequence}");
                self.problem_at(UNSUPPORTED_GITIGNORE, path, line, message);
                None
            }
        }
    }

    /// Adds the document at `path`, read when its stat changed and parsed when its contents
    /// changed.
    fn document(&mut self, path: String) {
        let absolute = self.root.join(&path);
        let previous = self.previous.documents.remove(&path);
        let checked = fs::symlink_metadata(&absolute).and_then(|metadata| {
            let (now, work, reads) = (self.now, &mut *self.work, &mut *self.reads);
            tracked::check(&absolute, &metadata, previous, now, work, reads, false)
        });
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

    /// Reports an entry that may hold or be an input but whose name is not UTF-8.
    fn unnamed(&mut self, path: String) {
        let message = format!("the name of {path} is not UTF-8, so Mara cannot read it: rename it");
        self.problem("io-error", path, message);
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
        self.problem_at(code, path, 1, message);
    }

    fn problem_at(&mut self, code: &'static str, path: String, line: usize, message: String) {
        self.diagnostics
            .push(Diagnostic::error(code, path, line, message));
    }
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
fn ignored(ignores: &[(String, Arc<Gitignore>)], path: &str, is_dir: bool) -> bool {
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
fn compile_gitignore(contents: &[u8]) -> Result<Gitignore, (usize, String)> {
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
