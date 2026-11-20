//! `item move` and `item rename` (REQ-MOVE-ITEM, REQ-RENAME-ID and DES-MOVE-AND-RENAME in
//! `docs/authoring/index.mara.md`): the edit that `mara_engine::relink` computes, read back and put on
//! disk by the write procedure (DES-WRITE-PROCEDURE in `docs/authoring/writes.mara.md`).

use std::fmt::Write as _;

use mara::inputs::{CodeReads, Generation, Inputs};
use mara::write::{self, Basis, FileWrite, WriteLock};
use mara_engine::code::{self, CodeEdit};
use mara_engine::diagnostic::Diagnostic;
use mara_engine::document::{Document, Item, label_name};
use mara_engine::json::{self, Object};
use mara_engine::read;
use mara_engine::relink::{self, DocumentEdit, RelinkError};
use mara_engine::snapshot::ParsedFile;
use mara_engine::validation::is_id;
use mara_engine::work::WorkCounts;
use mara_engine::write::{ReadBack, UNREADABLE_EDIT};

use super::{
    Args, Argument, Context, FAILURE, FORMAT_VERSION, Failure, Kind, Operation, Opt, REJECTED,
    Response, Run, SUCCESS, project_path,
};

pub const ITEM_MOVE: Operation = Operation {
    words: &["item", "move"],
    summary: "Moves an item to another document, or to another line of its own, and rewrites every \
              link to it.",
    arguments: &[
        ID_OR_MID,
        Argument {
            name: "path",
            help: "The destination document, relative to the project root.",
        },
    ],
    options: &[LINE],
    run: Run::Document(item_move),
};

pub const ITEM_RENAME: Operation = Operation {
    words: &["item", "rename"],
    summary: "Changes an item's ID, and every relation link and mention that shows it.",
    arguments: &[
        ID_OR_MID,
        Argument {
            name: "new-id",
            help: "The new ID, with the type's prefix.",
        },
    ],
    options: &[],
    run: Run::Document(item_rename),
};

/// `<id-or-mid>`: the item moved or renamed.
const ID_OR_MID: Argument = Argument {
    name: "id-or-mid",
    help: "The item's ID or MID.",
};

/// `--line <n>`: the line of the destination before which the item goes.
const LINE: Opt = Opt {
    name: "line",
    value: "n",
    help: "The line of the destination, from 1, before which the item goes; without it, the item \
           ends the document.",
    repeatable: false,
    required: false,
    kind: Kind::Integer,
    check: |value| match value.parse::<usize>() {
        Ok(line) if line > 0 => Ok(()),
        _ => Err(format!("a line is a whole number from 1, not `{value}`")),
    },
};

/// Rejected arguments, before anything runs.
fn rejected(message: String) -> Failure {
    Failure {
        status: REJECTED,
        ..Failure::new(
            "invalid-argument",
            format!("{message}; run `mara` for the usage"),
        )
    }
}

fn refused(error: RelinkError) -> Failure {
    let status = match error.code {
        "invalid-argument" => REJECTED,
        _ => FAILURE,
    };
    Failure {
        code: error.code,
        message: error.message,
        diagnostics: error.diagnostics,
        status,
    }
}

/// The project locked for an edit and read once.
struct Session {
    lock: WriteLock,
    inputs: Inputs,
    generation: Generation,
}

impl Session {
    /// Selects the project, takes its write lock, then reads it, doing with code files what
    /// `code` says; refuses a project whose configuration or schema has an error, with those
    /// diagnostics.
    fn open(context: &mut Context, code: CodeReads) -> Result<Session, Failure> {
        let root = context.root()?;
        let lock = WriteLock::take(&root)?;
        let mut inputs = context.inputs(root);
        let project = inputs.project(context.now, context.work);
        if project.has_errors() {
            let message = "the project's configuration or schema has errors, so Mara edits none \
                           of its documents until they are fixed"
                .to_owned();
            return Err(Failure {
                diagnostics: project.diagnostics.clone(),
                ..Failure::new("invalid-project", message)
            });
        }
        let generation = inputs.generation(context.now, context.work, code);
        Ok(Session {
            lock,
            inputs,
            generation,
        })
    }

    /// Reads each document of `edits` back and writes them in their order. Returns each written
    /// document's path and new parse.
    fn write(
        &self,
        edits: Vec<DocumentEdit>,
        work: &mut WorkCounts,
    ) -> Result<Vec<(String, Document)>, Failure> {
        let steps = edits.into_iter().map(Step::Document).collect();
        self.write_steps(steps, work)
    }

    /// [`Session::write`] of document and code edits in their order. A code edit is checked
    /// when it is computed ([`code::rename`]) and written as it is.
    fn write_steps(
        &self,
        steps: Vec<Step>,
        work: &mut WorkCounts,
    ) -> Result<Vec<(String, Document)>, Failure> {
        let edits = steps;
        let snapshot = &self.generation.snapshot;
        let empty = ParsedFile::default();
        let mut read_back = ReadBack::default();
        let mut files = Vec::with_capacity(edits.len());
        let mut written = Vec::with_capacity(edits.len());
        for step in edits {
            let edit = match step {
                Step::Document(edit) => edit,
                Step::Code(edit) => {
                    files.push(FileWrite {
                        basis: self.inputs.basis(&edit.path).unwrap_or(Basis::Absent),
                        path: edit.path,
                        contents: edit.contents,
                    });
                    continue;
                }
            };
            let old = match snapshot.document(&edit.path) {
                Some(index) => &*snapshot.documents()[index].file,
                None => &empty,
            };
            let new = read_back
                .edit(old, &edit.splices, work)
                .map_err(|misread| Failure {
                    diagnostics: vec![misread.diagnostic(&edit.path)],
                    ..Failure::new(
                        UNREADABLE_EDIT,
                        format!(
                            "the edit of {} would not read back as written, so nothing was \
                             written: {}",
                            edit.path, misread.message
                        ),
                    )
                })?;
            let ParsedFile { contents, document } = new;
            files.push(FileWrite {
                basis: self.inputs.basis(&edit.path).unwrap_or(Basis::Absent),
                path: edit.path.clone(),
                contents: contents.into_vec(),
            });
            written.push((edit.path, document));
        }
        if !files.is_empty() {
            write::apply(&self.lock, &files, work)?;
        }
        Ok(written)
    }
}

/// One file of an edit, in write order.
enum Step {
    Document(DocumentEdit),
    Code(CodeEdit),
}

impl Step {
    fn path(&self) -> &str {
        match self {
            Step::Document(edit) => &edit.path,
            Step::Code(edit) => &edit.path,
        }
    }
}

fn item_move(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let reference = args.argument(0);
    let destination =
        project_path(args.argument(1)).map_err(|reason| rejected(format!("`<path>`: {reason}")))?;
    let line = args
        .value(LINE.name)
        .map(|value| value.parse().expect("checked before the operation ran"));
    let session = Session::open(context, CodeReads::None)?;
    let snapshot = &session.generation.snapshot;
    if snapshot.document(&destination).is_none() {
        session
            .inputs
            .admits(&destination)
            .map_err(|refusal| Failure::new(refusal.code, refusal.message))?;
    }
    let moved = relink::move_item(snapshot, reference, &destination, line).map_err(refused)?;
    let item = snapshot.item(moved.item);
    let from = (
        snapshot.documents()[snapshot.items()[moved.item].document]
            .path
            .as_str(),
        item.line,
    );
    let files: Vec<String> = moved.edits.iter().map(|edit| edit.path.clone()).collect();
    let written = session.write(moved.edits, context.work)?;
    let in_destination = |document: &Document| {
        let found = document.items.iter().find(|other| other.mid == item.mid);
        found.map(|other| other.line)
    };
    let line = written
        .iter()
        .find(|(path, _)| *path == destination)
        .and_then(|(_, document)| in_destination(document))
        .or_else(|| {
            let index = snapshot.document(&destination)?;
            in_destination(&snapshot.documents()[index].file.document)
        })
        .unwrap_or(item.line);
    let id = item.value("id");
    let members = |object: &mut Object<'_>| {
        summary(object.key("item"), &destination, line, item, id);
        let mut place = Object::new(object.key("from"));
        place.string("path", from.0);
        place.number("line", from.1 as u64);
        place.end();
    };
    let human = format!(
        "moved {} from {}:{} to {destination}:{line}\n",
        id.unwrap_or(&item.mid),
        from.0,
        from.1
    );
    Ok(respond(
        members,
        human,
        &files,
        &session.generation.diagnostics,
    ))
}

fn item_rename(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let reference = args.argument(0);
    let new_id = args.argument(1);
    if !is_id(new_id) {
        return Err(rejected(format!(
            "`{new_id}` is not an ID: uppercase letters and digits in two or more parts joined \
             by `-`, the first starting with a letter, such as `REQ-SIGN-IN`"
        )));
    }
    let session = Session::open(context, CodeReads::Scan)?;
    let snapshot = &session.generation.snapshot;
    let renamed = relink::rename(snapshot, reference, new_id).map_err(refused)?;
    let layer = session
        .generation
        .code
        .as_deref()
        .expect("code was scanned");
    // A marker in a code file that cannot be read would keep the old ID (DES-CODE-READERS).
    super::author::incomplete_code(layer, &renamed.old_id)?;
    let markers = code::rename(layer, &renamed.old_id, new_id).map_err(|path| {
        let message = format!(
            "the edit of {path} would not leave each marker it changes naming {new_id}, so \
             nothing was written"
        );
        Failure::new(UNREADABLE_EDIT, message)
    })?;
    let item = snapshot.item(renamed.item);
    let path = &snapshot.documents()[snapshot.items()[renamed.item].document].path;
    // Code markers and mention text first, in path order, then the `ID` row, then relation
    // link text (DES-WRITE-PROCEDURE).
    let mut edits = renamed.edits.into_iter();
    let mut steps: Vec<Step> = edits
        .by_ref()
        .take(renamed.before_id_row)
        .map(Step::Document)
        .collect();
    steps.extend(markers.into_iter().map(Step::Code));
    steps.sort_by(|a, b| a.path().cmp(b.path()));
    steps.extend(edits.map(Step::Document));
    let files: Vec<String> = steps.iter().map(|step| step.path().to_owned()).collect();
    session.write_steps(steps, context.work)?;
    let members = |object: &mut Object<'_>| {
        summary(object.key("item"), path, item.line, item, Some(new_id));
        object.string("old_id", &renamed.old_id);
    };
    let human = format!(
        "renamed {} to {new_id} at {path}:{}\n",
        renamed.old_id, item.line
    );
    Ok(respond(
        members,
        human,
        &files,
        &session.generation.diagnostics,
    ))
}

/// The item as a result names it, at `path` and `line` and with the ID `id`, in the form of
/// `get`'s.
fn summary(out: &mut String, path: &str, line: usize, item: &Item, id: Option<&str>) {
    let mut object = Object::new(out);
    object.string("kind", "item");
    object.string("path", path);
    object.number("line", line as u64);
    object.optional_string("id", id);
    object.string("mid", &item.mid);
    object.string("type", &label_name(item.value("type").unwrap_or_default()));
    object.string("title", &item.title);
    object.end();
}

/// The result: `format_version`, the `members` of the operation, the files written, in order,
/// and the inventory's problems, which may hide links the edit did not see.
fn respond(
    members: impl FnOnce(&mut Object<'_>),
    mut human: String,
    files: &[String],
    diagnostics: &[Diagnostic],
) -> Response {
    let mut out = String::new();
    let mut object = Object::new(&mut out);
    object.number("format_version", FORMAT_VERSION);
    members(&mut object);
    let names: Vec<String> = files
        .iter()
        .map(|path| {
            let mut name = String::new();
            json::string(&mut name, path);
            name
        })
        .collect();
    object.array("files", names.iter().map(String::as_str));
    let records: Vec<String> = diagnostics
        .iter()
        .map(|diagnostic| {
            let mut record = String::new();
            diagnostic.write_json(&mut record);
            record
        })
        .collect();
    object.array("diagnostics", records.iter().map(String::as_str));
    object.end();
    match files {
        [] => human.push_str("wrote nothing: the project already reads so\n"),
        files => {
            let _ = writeln!(human, "wrote {}", files.join(", "));
        }
    }
    let shown = read::shown_diagnostics(diagnostics, 0);
    for diagnostic in &diagnostics[..shown] {
        let _ = writeln!(human, "{diagnostic}");
    }
    if shown < diagnostics.len() {
        let more = diagnostics.len() - shown;
        let _ = writeln!(
            human,
            "and {more} more problems with the project's inputs: `mara project validate` reports \
             each of them"
        );
    }
    Response {
        json: out,
        human,
        status: SUCCESS,
    }
}
