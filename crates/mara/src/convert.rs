//! The one-shot conversion of a 0.3.0 project, `project convert` (REQ-CONVERT-DIRECTIVE-CORPUS,
//! DES-CONVERSION-MAPPING, ADR-CONVERSION-RERUN and ADR-CONVERSION-REPORT-FILE in
//! `docs/migration.mara.md`).
//!
//! It reads the 0.3.0 configuration, schema and documents, computes every file it writes, and
//! writes them as DES-WRITE-PROCEDURE does: the report file first, then the documents, the
//! schema, and the configuration last. Until the configuration is written the project stays a
//! 0.3.0 project, which a rerun converts.

mod config;
mod discover;
mod documents;
pub mod report;
mod scan;
mod schema;
mod yaml;

use std::fmt::Write as _;
use std::path::Path;

use mara_engine::config::{CONFIG_PATH, LEGACY_CONFIG_PATH};
use mara_engine::diagnostic::Diagnostic;
use mara_engine::document;
use mara_engine::work::WorkCounts;

use crate::write::{self, Basis, FileWrite, WriteError, WriteLock};

pub use report::{Entry, REPORT_PATH};
pub use schema::{is_name, name_of};

/// A conversion that wrote the project.
#[derive(Debug)]
pub struct Outcome {
    /// The files written, in the order written.
    pub files: Vec<String>,
    /// The whole report, as the report file holds it.
    pub entries: Vec<Entry>,
}

/// Why nothing was converted.
#[derive(Debug)]
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
    pub diagnostics: Vec<Diagnostic>,
    /// The request lacks an option the project needs: rejected arguments.
    pub rejected: bool,
}

impl Refusal {
    fn new(code: &'static str, message: String, diagnostics: Vec<Diagnostic>) -> Refusal {
        Refusal {
            code,
            message,
            diagnostics,
            rejected: false,
        }
    }

    /// Problems of the 0.3.0 project that keep it from being converted.
    fn project(diagnostics: Vec<Diagnostic>) -> Refusal {
        let message = "the 0.3.0 project cannot be converted as it is, so nothing was written: \
                       fix the problems below and run the conversion again"
            .to_owned();
        Refusal::new("invalid-project", message, diagnostics)
    }

    /// A problem of one file.
    fn file(diagnostic: Diagnostic) -> Refusal {
        let message = format!("{}; nothing was written", diagnostic.message);
        Refusal::new(diagnostic.code, message, vec![diagnostic])
    }
}

impl From<WriteError> for Refusal {
    fn from(error: WriteError) -> Refusal {
        Refusal::new(error.code, error.message.clone(), vec![error.diagnostic()])
    }
}

/// Converts the 0.3.0 project at `root`, the canonical root that selection returns. Items
/// without a status get `status`, a status name or label.
pub fn convert(
    root: &Path,
    status: Option<&str>,
    work: &mut WorkCounts,
) -> Result<Outcome, Refusal> {
    let lock = WriteLock::take(root)?;
    let mut entries = Vec::new();
    let (contents, _) = Basis::read(&lock, LEGACY_CONFIG_PATH, work)?;
    let contents = contents.ok_or_else(|| {
        Refusal::file(Diagnostic::error(
            "io-error",
            LEGACY_CONFIG_PATH,
            1,
            format!("cannot read {LEGACY_CONFIG_PATH}: it is gone"),
        ))
    })?;
    let legacy = config::read(&contents, &mut entries).map_err(Refusal::project)?;
    let (schema_path, schema_line) = &legacy.schema;
    let unreadable = |reason: String| {
        Refusal::project(vec![Diagnostic::error(
            "io-error",
            LEGACY_CONFIG_PATH,
            *schema_line,
            format!("cannot read the schema {schema_path}: {reason}"),
        )])
    };
    let (contents, _) =
        Basis::read(&lock, schema_path, work).map_err(|error| unreadable(error.message))?;
    let contents = contents.ok_or_else(|| unreadable("it does not exist".to_owned()))?;
    let text = std::str::from_utf8(&contents)
        .map_err(|_| unreadable("it is not UTF-8 text".to_owned()))?;
    let tree = yaml::parse(text).map_err(|(line, reason)| {
        Refusal::project(vec![Diagnostic::error(
            "invalid-value",
            schema_path.as_str(),
            line,
            format!("the schema is not YAML that 0.3.0 read: {reason}"),
        )])
    })?;
    let vocabulary = schema::read(schema_path, &tree, &mut entries).map_err(Refusal::project)?;

    let paths =
        discover::documents(&lock, root, &legacy.include, work).map_err(Refusal::project)?;
    let mut sources = Vec::with_capacity(paths.len());
    let mut bases = Vec::with_capacity(paths.len());
    let mut problems = Vec::new();
    for path in paths {
        let (contents, basis) = Basis::read(&lock, &path, work)?;
        let Some(contents) = contents else { continue };
        let parsed = document::parse(&contents, work);
        match String::from_utf8(contents) {
            Ok(text) => {
                let scan = scan::scan(&text);
                sources.push(documents::Source {
                    path,
                    text,
                    parsed,
                    scan,
                });
                bases.push(basis);
            }
            Err(_) => {
                let message = format!("{path} is not UTF-8 text, so the conversion cannot read it");
                problems.push(Diagnostic::error("invalid-utf8", path, 1, message));
            }
        }
    }
    if !problems.is_empty() {
        return Err(Refusal::project(problems));
    }

    let (recorded, report_basis) = match Basis::read(&lock, REPORT_PATH, work)? {
        (Some(contents), basis) => (
            report::read(&contents).map_err(|diagnostic| Refusal::file(*diagnostic))?,
            basis,
        ),
        (None, basis) => (Vec::new(), basis),
    };
    let (_, schema_basis) = Basis::read(&lock, config::SCHEMA_PATH, work)?;
    let (_, config_basis) = Basis::read(&lock, CONFIG_PATH, work)?;

    let status = status.map(name_of);
    let needed = sources
        .iter()
        .flat_map(|source| &source.scan.blocks)
        .filter(|block| documents::needs_mid(block))
        .count();
    let mids = (0..needed)
        .map(|_| write::new_mid())
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|error| {
            Refusal::new(
                "io-error",
                format!("cannot read random bytes for new MIDs: {error}"),
                Vec::new(),
            )
        })?;
    let (contents, computed) = documents::convert(
        &sources,
        &vocabulary,
        status.as_deref(),
        mids,
        &recorded,
        work,
    )
    .map_err(|refusal| match refusal {
        documents::Refusal::MissingStatus(diagnostics) => missing_status(diagnostics),
        documents::Refusal::Unreadable(diagnostic) => Refusal::file(*diagnostic),
    })?;
    let mut statuses: Vec<String> = vocabulary
        .statuses
        .iter()
        .map(|(_, name)| name.clone())
        .collect();
    if let Some(status) = &status
        && !statuses.contains(status)
    {
        statuses.push(status.clone());
    }
    if statuses.is_empty() {
        return Err(missing_status(vec![Diagnostic::error(
            "missing-key",
            schema_path.as_str(),
            1,
            "no flavour has a `status` field with values, and a 0.4.0 schema lists its statuses: \
             name one with `--status`"
                .to_owned(),
        )]));
    }
    entries.extend(computed);
    let entries = report::merge(recorded, entries);

    let mut files = vec![FileWrite {
        path: REPORT_PATH.to_owned(),
        basis: report_basis,
        contents: report::render(&entries).into_bytes(),
    }];
    for ((source, basis), contents) in sources.iter().zip(bases).zip(contents) {
        if let Some(contents) = contents {
            files.push(FileWrite {
                path: source.path.clone(),
                basis,
                contents: contents.into_bytes(),
            });
        }
    }
    files.push(FileWrite {
        path: config::SCHEMA_PATH.to_owned(),
        basis: schema_basis,
        contents: vocabulary.render(&statuses).into_bytes(),
    });
    files.push(FileWrite {
        path: CONFIG_PATH.to_owned(),
        basis: config_basis,
        contents: config::render(&legacy.include).into_bytes(),
    });
    write::apply(&lock, &files, work)?;
    Ok(Outcome {
        files: files.into_iter().map(|file| file.path).collect(),
        entries,
    })
}

fn missing_status(diagnostics: Vec<Diagnostic>) -> Refusal {
    Refusal {
        rejected: true,
        ..Refusal::new(
            "invalid-argument",
            "the project has items without a status, or no status at all, so the conversion needs \
             `--status <status>`; nothing was written"
                .to_owned(),
            diagnostics,
        )
    }
}

/// `text` as a TOML basic string.
fn toml_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
