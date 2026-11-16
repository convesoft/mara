//! `project init`: REQ-PROJECT-INIT in `docs/project.mara.md` and DES-PROJECT-INIT in
//! `docs/project.mara.md`.
//!
//! The templates are embedded in the binary. Their files are written through the write
//! procedure (DES-WRITE-PROCEDURE) as files that must not exist, the configuration last.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use mara::project::{self, SelectError};
use mara::write::{self, Basis, FileWrite, WriteLock};
use mara_engine::config::{CONFIG_PATH, Root};
use mara_engine::diagnostic::Diagnostic;
use mara_engine::json::{self, Object};
use mara_engine::validation;

use super::{Args, Context, FORMAT_VERSION, Failure, Kind, Operation, Opt, Response, Run, SUCCESS};

pub const PROJECT: Operation = Operation {
    words: &["project", "init"],
    summary: "Makes the directory a project: writes `.mara/config.toml` and `.mara/schema.toml` \
              of a template, and overwrites no file.",
    arguments: &[],
    options: &[TEMPLATE],
    run: Run::Document(project_init),
};

/// `--template engineering|empty`: the template to write, `engineering` when left out.
const TEMPLATE: Opt = Opt {
    name: "template",
    value: "engineering|empty",
    help: "`engineering`, the default, or `empty`, which declares no type and no relation.",
    repeatable: false,
    required: false,
    kind: Kind::Text,
    check: |value| match template(value) {
        Some(_) => Ok(()),
        None => Err(format!(
            "the template is `engineering` or `empty`, not `{value}`"
        )),
    },
};

/// The configuration of every template: the example of DES-PROJECT-CONFIGURATION.
const CONFIG: &str = include_str!("../../templates/config.toml");

/// The schema of the `engineering` template: DES-ENGINEERING-VOCABULARY.
const ENGINEERING_SCHEMA: &str = include_str!("../../templates/engineering/schema.toml");

/// The schema of the `empty` template: the statuses and no type or relation.
const EMPTY_SCHEMA: &str = include_str!("../../templates/empty/schema.toml");

/// The files of the template `name`, relative to the project root, in the order they are
/// written: the configuration last, so that no operation selects a project that is half
/// written.
fn template(name: &str) -> Option<[(&'static str, &'static str); 2]> {
    let schema = match name {
        "engineering" => ENGINEERING_SCHEMA,
        "empty" => EMPTY_SCHEMA,
        _ => return None,
    };
    Some([(".mara/schema.toml", schema), (CONFIG_PATH, CONFIG)])
}

/// Writes the template's files into the working directory or the one `--project` names,
/// which is created when missing. Refuses to overwrite any file, and reports those it created.
fn project_init(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let name = args.value(TEMPLATE.name).unwrap_or("engineering");
    let files = template(name).expect("checked before the operation ran");
    let start = std::env::current_dir().map_err(|error| {
        Failure::new(
            "io-error",
            format!("cannot read the working directory: {error}"),
        )
    })?;
    let directory = match &context.project {
        Some(named) => start.join(named),
        None => start,
    };
    // The directories this request created, in order; a failure removes those still empty.
    let mut created = Vec::new();
    let result = (|| {
        let root = root(&directory, &mut created)?;
        refuse_existing(&root, &files)?;
        // The write lock is on `.mara`, which a new project does not have yet.
        let mara = root.join(".mara");
        let made = match fs::create_dir(&mara) {
            Ok(()) => {
                created.push(mara.clone());
                File::open(&root).and_then(|root| root.sync_all())
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(error),
        };
        made.map_err(|error| cannot_create(&mara, &error))?;
        let lock = WriteLock::take(&root)?;
        let writes: Vec<FileWrite> = files
            .iter()
            .map(|(path, contents)| FileWrite {
                path: (*path).to_owned(),
                basis: Basis::Absent,
                contents: contents.as_bytes().to_vec(),
            })
            .collect();
        let written = write::apply(&lock, &writes, context.work).map_err(Failure::from);
        // Held until the directories it created are removed, so that no other writer takes
        // a lock on a directory about to go.
        Ok((written, lock))
    })();
    let written = match result {
        Ok((written, lock)) => {
            if written.is_err() {
                remove_empty(&created);
            }
            drop(lock);
            written
        }
        Err(failure) => {
            remove_empty(&created);
            Err(failure)
        }
    };
    written?;
    let paths: Vec<&str> = files.iter().map(|(path, _)| *path).collect();
    Ok(respond(&paths))
}

/// The canonical directory to initialize, created with each missing directory above it.
fn root(directory: &Path, created: &mut Vec<PathBuf>) -> Result<PathBuf, Failure> {
    let mut missing = Vec::new();
    let mut path = directory;
    loop {
        match fs::symlink_metadata(path) {
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::NotFound => missing.push(path),
            Err(error) => return Err(cannot_create(directory, &error)),
        }
        match path.parent() {
            Some(parent) => path = parent,
            None => break,
        }
    }
    for path in missing.into_iter().rev() {
        match fs::create_dir(path) {
            Ok(()) => {
                created.push(path.to_owned());
                if let Some(parent) = path.parent() {
                    File::open(parent)
                        .and_then(|parent| parent.sync_all())
                        .map_err(|error| cannot_create(path, &error))?;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(cannot_create(directory, &error)),
        }
    }
    let root = fs::canonicalize(directory).map_err(|error| cannot_create(directory, &error))?;
    if !fs::metadata(&root).is_ok_and(|metadata| metadata.is_dir()) {
        let message = format!("{} is not a directory", directory.display());
        return Err(Failure::new("io-error", message));
    }
    Ok(root)
}

fn cannot_create(directory: &Path, error: &io::Error) -> Failure {
    let message = format!("cannot create {}: {error}", directory.display());
    Failure::new("io-error", message)
}

/// Refuses a 0.3.0 project, a `.mara` that is a symbolic link or no directory, and a file of
/// `files` that exists, whatever it is.
fn refuse_existing(root: &Path, files: &[(&str, &str)]) -> Result<(), Failure> {
    if let Some(Root::Legacy) = project::recognize(root)? {
        let root = root.to_owned();
        return Err(Failure::from(SelectError::Legacy { root }));
    }
    let cannot_read = |path: &str, error: io::Error| {
        Failure::new("io-error", format!("cannot read {path}: {error}"))
    };
    let mut existing = Vec::new();
    match fs::symlink_metadata(root.join(".mara")) {
        Ok(metadata) if !metadata.is_dir() => existing.push(".mara"),
        Ok(_) => {
            for (path, _) in files {
                match fs::symlink_metadata(root.join(path)) {
                    Ok(_) => existing.push(*path),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(cannot_read(path, error)),
                }
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(cannot_read(".mara", error)),
    }
    if existing.is_empty() {
        return Ok(());
    }
    let count = existing.len();
    let (verb, pronoun) = if count == 1 {
        ("exists", "it")
    } else {
        ("exist", "them")
    };
    let names = existing.iter().map(|path| format!("`{path}`"));
    let message = format!(
        "{} already {verb}, and `project init` overwrites no file, so nothing was written: \
         remove {pronoun} or initialize another directory",
        validation::list(names, count)
    );
    let diagnostics = existing
        .iter()
        .map(|path| {
            let message = format!("{path} already exists");
            Diagnostic::error("existing-file", *path, 1, message)
        })
        .collect();
    Err(Failure {
        diagnostics,
        ..Failure::new("existing-file", message)
    })
}

/// Removes each of `created` that is empty, the last created first.
fn remove_empty(created: &[PathBuf]) {
    for directory in created.iter().rev() {
        let _ = fs::remove_dir(directory);
    }
}

/// `{"format_version":1,"created":[...]}`, and one `created <path>` line per file.
fn respond(paths: &[&str]) -> Response {
    let names: Vec<String> = paths
        .iter()
        .map(|path| {
            let mut name = String::new();
            json::string(&mut name, path);
            name
        })
        .collect();
    let mut out = String::new();
    let mut object = Object::new(&mut out);
    object.number("format_version", FORMAT_VERSION);
    object.array("created", names.iter().map(String::as_str));
    object.end();
    let human = paths
        .iter()
        .map(|path| format!("created {path}\n"))
        .collect();
    Response {
        json: out,
        human,
        status: SUCCESS,
    }
}
