//! Finding the project an operation runs against and loading its configuration and schema
//! (REQ-ONE-PROJECT-PER-OPERATION and DES-PROJECT-CONFIGURATION in `docs/project.mara.md`).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use mara_engine::config::{
    CONFIG_PATH, Config, LEGACY_CONFIG_PATH, PathSetting, Root, parse_config,
};
use mara_engine::diagnostic::{Diagnostic, Severity};
use mara_engine::schema::{Schema, parse_schema};
use mara_engine::work::WorkCounts;

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
    /// `None` when no schema could be read.
    pub schema: Option<Schema>,
    /// Every problem of the configuration and the schema, in path and line order.
    pub diagnostics: Vec<Diagnostic>,
}

/// Reads the configuration and the schema of the project at `root`. A file that cannot be read
/// becomes a diagnostic: at line 1 of the configuration, or at the configuration line that
/// names the schema. So does a path to either that holds a symbolic link, and a schema path
/// that enters another project. Each file it reads counts in `work.file_reads`.
pub fn load(root: PathBuf, work: &mut WorkCounts) -> Project {
    let root = fs::canonicalize(&root).unwrap_or(root);
    let mut diagnostics = Vec::new();
    let config = match read(&root, CONFIG_PATH, 1, work) {
        Ok(bytes) => {
            let (config, problems) = parse_config(&bytes);
            diagnostics.extend(problems);
            config
        }
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            Config::default()
        }
    };
    let schema = config.schema.as_ref().and_then(|setting: &PathSetting| {
        match read(&root, &setting.path, setting.line, work) {
            Ok(bytes) => {
                let (parsed, problems) = parse_schema(&setting.path, &bytes);
                diagnostics.extend(problems);
                parsed
            }
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                None
            }
        }
    });
    diagnostics.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
    Project {
        root,
        config,
        schema,
        diagnostics,
    }
}

/// Reads the project file at `relative`, a `/`-separated path below `root` named at `line` of the
/// configuration. Each component of the path as written is checked first, without following
/// it: a symbolic link is refused, and so is a directory that is the root of another project.
/// Only a file whose contents were read counts as a file read.
fn read(
    root: &Path,
    relative: &str,
    line: usize,
    work: &mut WorkCounts,
) -> Result<Vec<u8>, Diagnostic> {
    let at = |code, message| Diagnostic {
        code,
        severity: Severity::Error,
        message,
        path: CONFIG_PATH.to_owned(),
        line,
    };
    let parts: Vec<&str> = relative.split('/').collect();
    let mut path = root.to_owned();
    for (index, part) in parts.iter().enumerate() {
        path.push(part);
        let shown = parts[..=index].join("/");
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if absent(&error) => break,
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
    }
    let bytes = fs::read(root.join(relative))
        .map_err(|error| io_error(CONFIG_PATH, line, relative, &error))?;
    work.file_reads += 1;
    Ok(bytes)
}

const SYMBOLIC_LINK: &str = "symbolic-link";

fn symbolic_link_message(path: &dyn fmt::Display) -> String {
    format!(
        "{path} is a symbolic link, and Mara follows none in a project's `.mara`, configuration or schema path: put the directory or file itself there"
    )
}

fn io_error(path: &str, line: usize, unreadable: &str, error: &io::Error) -> Diagnostic {
    Diagnostic {
        code: "io-error",
        severity: Severity::Error,
        message: format!("cannot read {unreadable}: {error}"),
        path: path.to_owned(),
        line,
    }
}
