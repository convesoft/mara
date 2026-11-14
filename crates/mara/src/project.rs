//! Finding the project an operation runs against and loading its configuration and schema
//! (REQ-ONE-PROJECT-PER-OPERATION and DES-PROJECT-CONFIGURATION in `docs/project.mara.md`).

use std::fmt;
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use mara_engine::config::{CONFIG_PATH, Config, LEGACY_CONFIG_PATH, Root, parse_config};
use mara_engine::diagnostic::{Diagnostic, Severity};
use mara_engine::hash::Hash;
use mara_engine::schema::{Schema, parse_schema};
use mara_engine::work::WorkCounts;

use crate::tracked::{self, Check, Reads, Tracked};

/// Why no project was selected.
#[derive(Debug)]
pub enum SelectError {
    /// Neither the start directory nor any ancestor is a project root.
    NoProject {
        start: PathBuf,
    },
    /// `--project` names a directory that is not a project root.
    NotAProject {
        directory: PathBuf,
    },
    /// The selected root belongs to a 0.3.0 project.
    Legacy {
        root: PathBuf,
    },
    /// A `.mara` met on the way is a symbolic link, which Mara never follows.
    SymbolicLink {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        error: io::Error,
    },
}

impl SelectError {
    /// The stable diagnostic code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoProject { .. } => "no-project",
            Self::NotAProject { .. } => "not-a-project",
            Self::Legacy { .. } => "legacy-project",
            Self::SymbolicLink { .. } => SYMBOLIC_LINK,
            Self::Io { .. } => "io-error",
        }
    }

    /// A problem with a directory's `.mara` as a diagnostic of that directory's project: at
    /// line 1 of its configuration, as loading reports it, or of the file in `.mara` that
    /// cannot be examined (DES-PROJECT-CONFIGURATION). Every other selection problem lies
    /// outside any project and has no location.
    pub fn diagnostic(&self) -> Option<Diagnostic> {
        let (Self::SymbolicLink { path } | Self::Io { path, .. }) = self else {
            return None;
        };
        fn name(path: &Path) -> Option<&str> {
            path.file_name().and_then(|name| name.to_str())
        }
        let relative = if name(path) == Some(".mara") {
            CONFIG_PATH.to_owned()
        } else if path.parent().and_then(name) == Some(".mara") {
            format!(".mara/{}", name(path)?)
        } else {
            return None;
        };
        Some(Diagnostic::error(
            self.code(),
            relative,
            1,
            self.to_string(),
        ))
    }
}

impl fmt::Display for SelectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProject { start } => write!(
                f,
                "no Mara project in {} or any directory above it: run `mara project init` to create one, or name one with `--project`",
                start.display()
            ),
            Self::NotAProject { directory } => write!(
                f,
                "{} is not a project root: it has no {CONFIG_PATH}",
                directory.display()
            ),
            Self::Legacy { root } => write!(
                f,
                "{} is a Mara 0.3.0 project ({LEGACY_CONFIG_PATH}): convert it with `mara project convert`",
                root.display()
            ),
            Self::SymbolicLink { path } => f.write_str(&symbolic_link_message(&path.display())),
            Self::Io { path, error } => write!(f, "cannot read {}: {error}", path.display()),
        }
    }
}

impl std::error::Error for SelectError {}

/// Selects the canonical project root: the directory `project` names, resolved from `start`,
/// or else the nearest of `start` and its ancestors that is a project root. A 0.3.0 project is
/// refused.
pub fn select(start: &Path, project: Option<&Path>) -> Result<PathBuf, SelectError> {
    let canonical =
        |path: PathBuf| fs::canonicalize(&path).map_err(|error| SelectError::Io { path, error });
    let start = canonical(start.to_owned())?;
    let (root, kind) = match project {
        Some(directory) => {
            let directory = canonical(start.join(directory))?;
            match recognize(&directory)? {
                Some(kind) => (directory, kind),
                None => return Err(SelectError::NotAProject { directory }),
            }
        }
        None => 'search: {
            for directory in start.ancestors() {
                if let Some(kind) = recognize(directory)? {
                    break 'search (directory.to_owned(), kind);
                }
            }
            return Err(SelectError::NoProject { start });
        }
    };
    match kind {
        Root::Project => Ok(root),
        Root::Legacy => Err(SelectError::Legacy { root }),
    }
}

/// Whether `directory` is the root of a project or of a 0.3.0 project. A `.mara` that is a
/// symbolic link is an error, never followed and never skipped. The input inventory skips every
/// project root below the project's own.
pub fn recognize(directory: &Path) -> Result<Option<Root>, SelectError> {
    let mara = directory.join(".mara");
    match fs::symlink_metadata(&mara) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(SelectError::SymbolicLink { path: mara });
        }
        Ok(metadata) if !metadata.is_dir() => return Ok(None),
        Ok(_) => {}
        Err(error) if absent(&error) => return Ok(None),
        Err(error) => return Err(SelectError::Io { path: mara, error }),
    }
    let exists = |relative: &str| {
        let path = directory.join(relative);
        match fs::symlink_metadata(&path) {
            Ok(_) => Ok(true),
            Err(error) if absent(&error) => Ok(false),
            Err(error) => Err(SelectError::Io { path, error }),
        }
    };
    Ok(Root::recognize(
        exists(CONFIG_PATH)?,
        exists(LEGACY_CONFIG_PATH)?,
    ))
}

fn absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

/// A project's configuration and schema, read as far as they are valid.
#[derive(Debug)]
pub struct Project {
    pub root: PathBuf,
    pub config: Config,
    /// The hash of the configuration file as read: `None` when it could not be read.
    pub config_hash: Option<Hash>,
    /// `None` when no schema could be read.
    pub schema: Option<Arc<Schema>>,
    /// The hash of the schema file as read: `None` when it could not be read.
    pub schema_hash: Option<Hash>,
    /// Every problem of the configuration and the schema, in path and line order.
    pub diagnostics: Vec<Diagnostic>,
}

impl Project {
    /// Whether the configuration or the schema has an error. Every operation that reads
    /// documents or the schema, except validation, then refuses the project
    /// (DES-PROJECT-CONFIGURATION).
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }
}

/// Reads the configuration and the schema of the project at `root`. A file that cannot be read
/// becomes a diagnostic: at line 1 of the configuration, or at the configuration line that
/// names the schema. So does a path to either that holds a symbolic link, and a schema path
/// that enters another project. Each file it reads counts one file read and one hash in `work`.
pub fn load(root: PathBuf, work: &mut WorkCounts) -> Project {
    let root = fs::canonicalize(&root).unwrap_or(root);
    let mut reads = Reads::default();
    ProjectFiles::default().load(root, SystemTime::now(), work, &mut reads)
}

/// The configuration and the schema as last read, with what was read of them, kept between
/// requests so that a file whose stat is unchanged is neither read nor parsed again.
#[derive(Debug, Default)]
pub(crate) struct ProjectFiles {
    config: Option<Tracked<(Config, Vec<Diagnostic>)>>,
    /// With the path it was read from.
    schema: Option<(String, Tracked<SchemaFile>)>,
}

type SchemaFile = (Option<Arc<Schema>>, Vec<Diagnostic>);

impl ProjectFiles {
    /// [`load`] in a request that began at `now`, reading only the files whose stat changed.
    /// What it reads joins `reads`, for the request's other roles of the same files.
    pub(crate) fn load(
        &mut self,
        root: PathBuf,
        now: SystemTime,
        work: &mut WorkCounts,
        reads: &mut Reads,
    ) -> Project {
        let mut diagnostics = Vec::new();
        let previous = self.config.take();
        let mut request = Request { now, work, reads };
        let config = track(&root, CONFIG_PATH, 1, previous, &mut request, parse_config);
        let (config, config_hash) = match config {
            Ok(tracked) => {
                let (config, problems) = &tracked.value;
                diagnostics.extend_from_slice(problems);
                let read = (config.clone(), Some(tracked.hash));
                self.config = Some(tracked);
                read
            }
            Err(diagnostic) => {
                diagnostics.push(*diagnostic);
                (Config::default(), None)
            }
        };
        let (mut schema, mut schema_hash) = (None, None);
        let previous = self.schema.take();
        if let Some(setting) = &config.schema {
            let path = setting.path.as_str();
            let previous = previous
                .filter(|(read_from, _)| read_from == path)
                .map(|(_, tracked)| tracked);
            let parse = |bytes: &[u8]| {
                let (schema, problems) = parse_schema(path, bytes);
                (schema.map(Arc::new), problems)
            };
            match track(&root, path, setting.line, previous, &mut request, parse) {
                Ok(tracked) => {
                    let (read, problems) = &tracked.value;
                    diagnostics.extend_from_slice(problems);
                    (schema, schema_hash) = (read.clone(), Some(tracked.hash));
                    self.schema = Some((path.to_owned(), tracked));
                }
                Err(diagnostic) => diagnostics.push(*diagnostic),
            }
        }
        diagnostics.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        Project {
            root,
            config,
            config_hash,
            schema,
            schema_hash,
            diagnostics,
        }
    }
}

/// Checks the project file at `relative` ([`check`]) and derives its value from its contents
/// with `derive` when they changed.
fn track<T>(
    root: &Path,
    relative: &str,
    line: usize,
    previous: Option<Tracked<T>>,
    request: &mut Request<'_>,
    derive: impl FnOnce(&[u8]) -> T,
) -> Result<Tracked<T>, Box<Diagnostic>> {
    Ok(match check(root, relative, line, previous, request)? {
        Check::Kept(tracked) => tracked,
        Check::Read(read) => {
            let value = derive(&read.contents);
            read.with(value)
        }
    })
}

/// Checks the project file at `relative`, a `/`-separated path below `root` named at `line` of
/// the configuration, and reads it when its stat changed ([`tracked::check`]). Each component
/// of the path as written is checked first, without following it: a symbolic link is refused,
/// and so is a directory that is the root of another project. Only a file whose contents were
/// read counts as a file read.
fn check<T>(
    root: &Path,
    relative: &str,
    line: usize,
    previous: Option<Tracked<T>>,
    request: &mut Request<'_>,
) -> Result<Check<T>, Box<Diagnostic>> {
    let at = |code, message| Box::new(Diagnostic::error(code, CONFIG_PATH, line, message));
    let parts: Vec<&str> = relative.split('/').collect();
    let mut path = root.to_owned();
    let mut last: Option<Metadata> = None;
    for (index, part) in parts.iter().enumerate() {
        path.push(part);
        let shown = parts[..=index].join("/");
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if absent(&error) => {
                return Err(io_error(CONFIG_PATH, line, relative, &error));
            }
            Err(error) => return Err(io_error(CONFIG_PATH, line, &shown, &error)),
        };
        if metadata.file_type().is_symlink() {
            return Err(at(SYMBOLIC_LINK, symbolic_link_message(&shown)));
        }
        if index + 1 < parts.len() {
            match recognize(&path) {
                Ok(None) => {}
                Ok(Some(_)) => {
                    return Err(at(
                        "outside-project",
                        format!(
                            "{relative} lies in the project at {shown}, and nothing of another project is an input"
                        ),
                    ));
                }
                Err(SelectError::SymbolicLink { .. }) => {
                    return Err(at(
                        SYMBOLIC_LINK,
                        symbolic_link_message(&format!("{shown}/.mara")),
                    ));
                }
                Err(error) => return Err(at("io-error", error.to_string())),
            }
        }
        last = Some(metadata);
    }
    let metadata = last.expect("a configured path has a part");
    let Request { now, work, reads } = request;
    tracked::check(&path, &metadata, previous, *now, work, reads, true)
        .map_err(|error| io_error(CONFIG_PATH, line, relative, &error))
}

/// The request a project file is read in.
struct Request<'r> {
    now: SystemTime,
    work: &'r mut WorkCounts,
    reads: &'r mut Reads,
}

pub(crate) const SYMBOLIC_LINK: &str = "symbolic-link";

fn symbolic_link_message(path: &dyn fmt::Display) -> String {
    format!(
        "{path} is a symbolic link, and Mara follows none in a project's `.mara`, configuration or schema path: put the directory or file itself there"
    )
}

fn io_error(path: &str, line: usize, unreadable: &str, error: &io::Error) -> Box<Diagnostic> {
    let message = format!("cannot read {unreadable}: {error}");
    Box::new(Diagnostic::error("io-error", path, line, message))
}
