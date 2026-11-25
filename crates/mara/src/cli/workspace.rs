//! The browser workspace (REQ-BROWSER-LOCAL-WORKSPACE and DES-BROWSER-WORKSPACE in
//! `docs/hosts/index.mara.md`): the document operations on supplied files, held in the module's
//! memory ([`mara::system::memory`]), and the change set of what they and the caller changed.
//!
//! The memory holds the project at its root, `/`. A request runs as an MCP tool call runs
//! ([`tool`]), against inputs that the workspace keeps between requests, as a daemon's session
//! keeps them, so that a request reads, hashes and parses only what changed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use mara_engine::hash::Hash;
use mara_engine::work::WorkCounts;
use serde_json::{Map, Value};

use mara::caller::Caller;
use mara::inputs::Inputs;
use mara::session::{Deriving, Evaluated};
use mara::system::{self, fs, memory};

use super::tool::{self, rejected};
use super::{Context, Failure, Reply, project_path};

/// The project root in the memory.
const ROOT: &str = "/";

/// The operations a browser cannot run: they create a project rather than work on one.
const UNAVAILABLE: &[&str] = &["project_init", "project_convert"];

/// What the workspace keeps between requests: its inputs, as a daemon's session keeps them,
/// with the outcomes of its evaluations.
#[derive(Debug)]
pub struct Kept {
    inputs: Arc<Mutex<Inputs>>,
    evaluated: Arc<Evaluated>,
    deriving: Arc<Deriving>,
}

impl Kept {
    /// A view of the kept inputs for one request ([`Inputs::shared`]).
    pub(super) fn inputs(&self) -> Inputs {
        Inputs::shared(
            Arc::clone(&self.inputs),
            Arc::clone(&self.evaluated),
            Arc::clone(&self.deriving),
        )
    }
}

/// One change of the change set: a file whose final contents differ from those supplied.
#[derive(Debug, PartialEq, Eq)]
pub struct Change {
    /// Relative to the project root.
    pub path: String,
    /// The final contents; `None` for a removed file.
    pub contents: Option<Vec<u8>>,
    /// The BLAKE3 hash of the supplied contents, in lowercase hexadecimal as a `revision`;
    /// `None` for a file that was not supplied.
    pub base: Option<String>,
}

/// A workspace. The module's memory holds one filesystem, so a worker holds one workspace.
#[derive(Debug)]
pub struct Workspace {
    /// The supplied files, by path.
    supplied: BTreeMap<String, Vec<u8>>,
    kept: Kept,
}

impl Workspace {
    // @mara implements REQ-BROWSER-LOCAL-WORKSPACE
    /// A workspace of `files`, each a path relative to the project root with its contents. A
    /// path that is not in the form a diagnostic gives it, a path given twice and a path below
    /// another file are refused with `invalid-argument`.
    pub fn new(files: Vec<(String, Vec<u8>)>) -> Result<Workspace, Failure> {
        let mut supplied = BTreeMap::new();
        for (path, contents) in files {
            checked(&path)?;
            if supplied.insert(path.clone(), contents).is_some() {
                return Err(rejected(format!("`{path}` is supplied twice")));
            }
        }
        let directories: BTreeSet<&str> = (supplied.keys())
            .flat_map(|path| path.match_indices('/').map(|(end, _)| &path[..end]))
            .collect();
        if let Some(path) = supplied
            .keys()
            .find(|path| directories.contains(path.as_str()))
        {
            return Err(rejected(format!(
                "`{path}` is supplied as a file and holds other supplied files"
            )));
        }
        let files = supplied
            .iter()
            .map(|(path, contents)| (path.clone(), contents.clone()));
        memory::reset(files)
            .map_err(|error| Failure::new("io-error", format!("cannot hold the files: {error}")))?;
        Ok(Workspace {
            supplied,
            kept: Kept {
                inputs: Arc::new(Mutex::new(Inputs::new(PathBuf::from(ROOT)))),
                evaluated: Arc::default(),
                deriving: Arc::default(),
            },
        })
    }

    // @mara implements REQ-BROWSER-LOCAL-WORKSPACE
    // @mara implements REQ-UNAVAILABLE-NOT-PARTIAL
    /// Runs the document operation whose MCP tool is named `name` with `arguments`, as that
    /// tool's call does: its reply is the JSON object that `--format json` prints. `project
    /// init` and `project convert` fail with `unavailable-operation` before their arguments are
    /// read, and a `project` argument is refused: the workspace holds one project. Adds the
    /// request's work to `work`.
    pub fn run(&self, name: &str, arguments: &Map<String, Value>, work: &mut WorkCounts) -> Reply {
        Reply::new(self.outcome(name, arguments, work), true)
    }

    fn outcome(
        &self,
        name: &str,
        arguments: &Map<String, Value>,
        work: &mut WorkCounts,
    ) -> Result<super::Response, Failure> {
        if UNAVAILABLE.contains(&name) {
            let operation = name.replace('_', " ");
            let message = format!(
                "`{operation}` cannot run in a browser workspace, which works on the files of an \
                 existing project: run it with the `mara` command line"
            );
            return Err(Failure::new("unavailable-operation", message));
        }
        let Some((operation, run)) = tool::find(name) else {
            return Err(rejected(format!("unknown operation `{name}`")));
        };
        if arguments
            .get("project")
            .is_some_and(|value| !value.is_null())
        {
            return Err(rejected(
                "`project` is no parameter of a browser workspace, which holds one project"
                    .to_owned(),
            ));
        }
        let (args, project) = tool::args(operation, arguments, None)?;
        let mut context = Context {
            work,
            now: system::now(),
            project,
            cwd: Ok(PathBuf::from(ROOT)),
            direct: true,
            caller: Caller {
                umask: 0o022,
                temp_dir: PathBuf::from(ROOT),
            },
            session: None,
            kept: Some(&self.kept),
        };
        run(&mut context, &args)
    }

    // @mara implements REQ-BROWSER-LOCAL-WORKSPACE
    /// Gives the file at `path` the contents `contents`, creating it and the directories above
    /// it if needed. A path that is a directory, or that lies below a file, is refused.
    pub fn write(&mut self, path: &str, contents: Vec<u8>) -> Result<(), Failure> {
        checked(path)?;
        let absolute = Path::new(ROOT).join(path);
        let parent = absolute.parent().expect("a path below the root");
        let refused = |error: std::io::Error| {
            rejected(format!(
                "`{path}` cannot be written in the workspace: {error}"
            ))
        };
        fs::create_dir_all(parent).map_err(refused)?;
        fs::write(&absolute, contents).map_err(refused)
    }

    // @mara implements REQ-BROWSER-LOCAL-WORKSPACE
    /// Removes the file at `path`, which must be a file.
    pub fn remove(&mut self, path: &str) -> Result<(), Failure> {
        checked(path)?;
        fs::remove_file(Path::new(ROOT).join(path))
            .map_err(|error| rejected(format!("`{path}` cannot be removed: {error}")))
    }

    // @mara implements REQ-BROWSER-LOCAL-WORKSPACE
    /// The change set, in path order: one change per path whose final contents differ from
    /// its supplied contents. A file written back to its supplied contents, or added and
    /// removed again, has none. The workspace stays as it is.
    pub fn export(&self) -> Vec<Change> {
        let mut current = memory::files();
        let mut changes: Vec<Change> = Vec::new();
        for (path, supplied) in &self.supplied {
            let contents = current.remove(path);
            if contents.as_ref() != Some(supplied) {
                changes.push(Change {
                    path: path.clone(),
                    contents,
                    base: Some(Hash::of_bytes(supplied).to_string()),
                });
            }
        }
        changes.extend(current.into_iter().map(|(path, contents)| Change {
            path,
            contents: Some(contents),
            base: None,
        }));
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        changes
    }
}

/// Refuses a path that is not relative to the project root in the form a diagnostic gives it:
/// `/` between parts, none of them empty, `.` or `..`.
fn checked(path: &str) -> Result<(), Failure> {
    match project_path(path) {
        Ok(normal) if normal == path => Ok(()),
        Ok(_) => Err(rejected(format!(
            "`{path}` is no path in the form `<directory>/<file>` relative to the project root"
        ))),
        Err(reason) => Err(rejected(reason)),
    }
}
