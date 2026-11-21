//! `project fix`: REQ-ONE-FIX-OPERATION in `docs/authoring/index.mara.md` and DES-FIX-REPAIRS in
//! `docs/authoring/index.mara.md`.
//!
//! It refuses a project whose configuration or schema has an error, as every operation but
//! validation does (DES-PROJECT-CONFIGURATION); the problems of documents are what it repairs.

use std::fmt::Write as _;

use mara::write::{self, FileWrite, WriteLock};
use mara_engine::fix::{self, Repair};
use mara_engine::snapshot::ParsedFile;
use mara_engine::write::ReadBack;

use super::{Context, Failure, Operation, Response, Run, SUCCESS};

pub const PROJECT: Operation = Operation {
    words: &["project", "fix"],
    summary: "Repairs what hand edits leave: items without an anchor, missing separators and blank \
              lines, setext headings, bare-ID relation values, and stale link paths and text.",
    arguments: &[],
    options: &[],
    run: Run::Document(project_fix),
};

// @mara implements REQ-ONE-FIX-OPERATION
/// Repairs every problem that fix may repair, in one edit of the documents that have any
/// (DES-WRITE-PROCEDURE).
fn project_fix(context: &mut Context, _: &super::Args) -> Result<Response, Failure> {
    let root = context.root()?;
    let lock = WriteLock::take(&root)?;
    let mut inputs = context.inputs(root);
    let project = inputs.project(context.now, context.work);
    if project.has_errors() {
        let message = "the project's configuration or schema has errors, so Mara reads none of \
                       its documents or its schema until they are fixed"
            .to_owned();
        return Err(Failure {
            diagnostics: project.diagnostics.clone(),
            ..Failure::new("invalid-project", message)
        });
    }
    let generation = inputs.snapshot(context.now, context.work);
    let snapshot = &generation.snapshot;
    let plan = fix::plan(snapshot, &mut write::new_mid).map_err(|error| {
        Failure::new(
            "io-error",
            format!("cannot read random bits for a new MID: {error}"),
        )
    })?;
    let mut read_back = ReadBack::knowing(snapshot);
    let mut files = Vec::with_capacity(plan.len());
    let mut parsed: Vec<(String, ParsedFile)> = Vec::with_capacity(plan.len());
    let mut repairs: Vec<Repair> = Vec::new();
    for document_fix in plan {
        let document = &snapshot.documents()[document_fix.document];
        let path = &document.path;
        let new = read_back
            .edit(&document.file, &document_fix.splices, context.work)
            .map_err(|misread| Failure {
                diagnostics: vec![misread.diagnostic(path)],
                ..Failure::new(
                    mara_engine::write::UNREADABLE_EDIT,
                    format!(
                        "the repairs of {path} would not read back as written, so nothing was \
                         written: {}",
                        misread.message
                    ),
                )
            })?;
        files.push(FileWrite {
            path: path.clone(),
            basis: inputs.basis(path).expect("a document of the snapshot"),
            contents: new.contents.to_vec(),
        });
        parsed.push((path.clone(), new));
        repairs.extend(document_fix.repairs);
    }
    if !files.is_empty() {
        // A session keeps what the edit read back (DES-WRITE-PROCEDURE step 4).
        inputs.apply(&lock, &files, parsed, context.now, context.work)?;
    }
    Ok(respond(&repairs, files.len()))
}

/// The result: the repairs, as many as a result shows, and how many there are in how many files.
fn respond(repairs: &[Repair], files: usize) -> Response {
    let shown = fix::shown(repairs);
    let mut human = String::new();
    for repair in &repairs[..shown] {
        writeln!(
            human,
            "{}:{}: fixed[{}]: {}",
            repair.path, repair.line, repair.code, repair.message
        )
        .expect("writing to a String cannot fail");
    }
    if shown < repairs.len() {
        writeln!(human, "and {} more", repairs.len() - shown)
            .expect("writing to a String cannot fail");
    }
    let summary = match (repairs.len(), files) {
        (0, _) => "nothing to fix".to_owned(),
        (1, _) => "fixed 1 problem in 1 file".to_owned(),
        (count, 1) => format!("fixed {count} problems in 1 file"),
        (count, files) => format!("fixed {count} problems in {files} files"),
    };
    writeln!(
        human,
        "{summary}; `mara project validate` reports what is left"
    )
    .expect("writing to a String cannot fail");
    Response {
        json: fix::to_json(repairs),
        human,
        status: SUCCESS,
    }
}
