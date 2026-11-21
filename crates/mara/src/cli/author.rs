//! `item create`, `item update`, `item delete`, `relation add` and `relation remove`
//! (`docs/authoring/index.mara.md` and DES-AUTHORING-REQUESTS in `docs/authoring/index.mara.md`),
//! each put on disk by the write procedure (DES-WRITE-PROCEDURE in `docs/authoring/writes.mara.md`).

use mara::inputs::{CodeReads, Inputs};
use mara::write::{self, Basis, FileWrite, WriteLock};
use mara_engine::authoring::{self, Create, EditError, Update, Written};
use mara_engine::code::{CodeLayer, CodeLinks};
use mara_engine::document::label_name;
use mara_engine::snapshot::{ParsedFile, Snapshot};
use mara_engine::work::WorkCounts;
use mara_engine::write::{cell, heading, label};

use super::read::open;
use super::{
    Args, Argument, Context, Failure, Kind, Operation, Opt, Response, Run, SUCCESS, project_path,
};

pub const ITEM_CREATE: Operation = Operation {
    words: &["item", "create"],
    summary: "Creates an item with a new MID in a document, which is created when it would be an \
              input of the project.",
    arguments: &[
        Argument {
            name: "type",
            help: "The item's type, as the schema declares it.",
        },
        Argument {
            name: "path",
            help: "The document, relative to the project root.",
        },
    ],
    options: &[
        Opt {
            required: true,
            ..TITLE
        },
        ID,
        FIELD,
        Opt {
            required: true,
            ..BODY
        },
        LINE,
    ],
    run: Run::Document(item_create),
};

pub const ITEM_UPDATE: Operation = Operation {
    words: &["item", "update"],
    summary: "Changes an item's title, fields or body.",
    arguments: &[ITEM],
    options: &[TITLE, FIELD, CLEAR_FIELD, BODY],
    run: Run::Document(item_update),
};

pub const ITEM_DELETE: Operation = Operation {
    words: &["item", "delete"],
    summary: "Deletes an item that no other item or narrative targets.",
    arguments: &[ITEM],
    options: &[],
    run: Run::Document(item_delete),
};

pub const RELATION_ADD: Operation = Operation {
    words: &["relation", "add"],
    summary: "Adds a relation row to the source item that links to the target item.",
    arguments: &[SOURCE, RELATION, TARGET],
    options: &[],
    run: Run::Document(relation_add),
};

pub const RELATION_REMOVE: Operation = Operation {
    words: &["relation", "remove"],
    summary: "Removes every row of the relation on the source item that links to the target item.",
    arguments: &[SOURCE, RELATION, TARGET],
    options: &[],
    run: Run::Document(relation_remove),
};

/// `<item>`: the item an update or delete edits.
const ITEM: Argument = Argument {
    name: "item",
    help: "The item: its ID, its MID or `<path>:<line>`.",
};

/// `<source>`: the item whose table holds the relation row.
const SOURCE: Argument = Argument {
    name: "source",
    help: "The item whose table holds the row: its ID, its MID or `<path>:<line>`.",
};

/// `<relation>`: the relation's forward name.
const RELATION: Argument = Argument {
    name: "relation",
    help: "The relation's forward name, as the schema declares it.",
};

/// `<target>`: the item the row links to.
const TARGET: Argument = Argument {
    name: "target",
    help: "The item the row links to: its ID, its MID or `<path>:<line>`.",
};

/// `--title <title>`: one line of Markdown, the item's heading text.
const TITLE: Opt = Opt {
    name: "title",
    value: "title",
    help: "The item's heading text: one line of Markdown.",
    repeatable: false,
    required: false,
    kind: Kind::Text,
    check: |value| {
        heading(1, value)
            .map(drop)
            .map_err(|why| format!("a title {why}"))
    },
};

/// `--id <id>`: the new item's ID, instead of the one proposed from its title.
const ID: Opt = Opt {
    name: "id",
    value: "id",
    help: "The new item's ID, instead of the one proposed from its title.",
    repeatable: false,
    required: false,
    kind: Kind::Text,
    check: |_| Ok(()),
};

/// `--field <key>=<value>`: a value of a field or of `Status`. Repeatable; the values of one key
/// replace all its rows.
const FIELD: Opt = Opt {
    name: "field",
    value: "key=value",
    help: "`<key>=<value>`: a value of a field or of `Status`. The values of one key replace all \
           its rows.",
    repeatable: true,
    required: false,
    kind: Kind::Text,
    check: field,
};

/// `--clear-field <name>`: removes every row of an optional field. Repeatable.
const CLEAR_FIELD: Opt = Opt {
    name: "clear-field",
    value: "name",
    help: "An optional field whose rows are all removed.",
    repeatable: true,
    required: false,
    kind: Kind::Text,
    check: |name| match label_name(name.trim()).as_str() {
        "" => Err("a field to clear is named by its key".to_owned()),
        "type" | "id" | "status" => Err(format!(
            "`{}` cannot be cleared: every item has one",
            name.trim()
        )),
        _ => Ok(()),
    },
};

/// `--body <text>`: the item's body, Markdown of any number of lines.
const BODY: Opt = Opt {
    name: "body",
    value: "text",
    help: "The item's body: Markdown of any number of lines.",
    repeatable: false,
    required: false,
    kind: Kind::Text,
    check: |_| Ok(()),
};

/// `--line <n>`: the line where a new item goes, from 1; the end of the document without it.
const LINE: Opt = Opt {
    name: "line",
    value: "n",
    help: "The line, from 1, at which the item goes; without it, the item ends the document.",
    repeatable: false,
    required: false,
    kind: Kind::Integer,
    check: |value| match value.parse::<usize>() {
        Ok(line) if line >= 1 => Ok(()),
        _ => Err(format!("a line is a number from 1, not `{value}`")),
    },
};

fn field(value: &str) -> Result<(), String> {
    let Some((key, value)) = value
        .split_once('=')
        .filter(|(key, _)| !key.trim().is_empty())
    else {
        return Err(format!("a field is `<key>=<value>`, not `{value}`"));
    };
    match label_name(key.trim()).as_str() {
        "type" => Err(
            "`Type` is no field: `item create` takes the type as its first argument, and an \
             item's type does not change"
                .to_owned(),
        ),
        "id" => Err(
            "`ID` is no field: `item create` takes it as `--id`, and `item rename` changes it"
                .to_owned(),
        ),
        _ if value.is_empty() => Err(format!(
            "the value of `{key}` is empty: remove a field's rows with `--clear-field {key}`"
        )),
        _ => cell(value)
            .map(drop)
            .map_err(|why| format!("the value of `{key}` {why}")),
    }
}

/// The `--field` options, as `(key, value)` pairs in the order given.
fn fields(args: &Args) -> Vec<(String, String)> {
    args.values(FIELD.name)
        .filter_map(|value| value.split_once('='))
        .map(|(key, value)| (key.trim().to_owned(), value.to_owned()))
        .collect()
}

/// The value of the option `option`, which the operation needs.
fn required<'a>(args: &'a Args, operation: &str, option: &Opt) -> Result<&'a str, Failure> {
    args.value(option.name).ok_or_else(|| {
        Failure::rejected(format!(
            "`{operation}` needs `--{} <{}>`",
            option.name, option.value
        ))
    })
}

fn item_create(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let title = required(args, "item create", &TITLE)?;
    let body = required(args, "item create", &BODY)?;
    let path = project_path(args.argument(1)).map_err(Failure::rejected)?;
    let line = args
        .value(LINE.name)
        .map(|value| value.parse().expect("checked before the operation ran"));
    let fields = fields(args);
    let written = edit(context, CodeReads::None, |inputs, snapshot, _, work| {
        if snapshot.document(&path).is_none() {
            inputs.admits(&path).map_err(|refusal| EditError {
                code: refusal.code,
                message: refusal.message,
                diagnostics: Vec::new(),
            })?;
        }
        let mid = new_mid(snapshot)?;
        let request = Create {
            type_name: args.argument(0),
            path: &path,
            title,
            id: args.value(ID.name),
            fields: &fields,
            body,
            line,
            mid: &mid,
        };
        authoring::create(snapshot, &request, work)
    })?;
    let item = &written.item;
    let human = format!(
        "created {} at {}:{}, MID {}\n",
        name(item),
        item.path,
        item.line,
        item.mid
    );
    Ok(respond(&written, human))
}

fn item_update(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let fields = fields(args);
    let clear: Vec<String> = args
        .values(CLEAR_FIELD.name)
        .map(|name| name.trim().to_owned())
        .collect();
    let (title, body) = (args.value(TITLE.name), args.value(BODY.name));
    if title.is_none() && fields.is_empty() && clear.is_empty() && body.is_none() {
        return Err(Failure::rejected(
            "`item update` needs `--title`, `--field`, `--clear-field` or `--body`".to_owned(),
        ));
    }
    if let Some(name) = clear.iter().find(|name| {
        fields
            .iter()
            .any(|(key, _)| label_name(key) == label_name(name))
    }) {
        return Err(Failure::rejected(format!(
            "`{name}` is both set and cleared: give one of `--field` and `--clear-field`"
        )));
    }
    let request = Update {
        item: args.argument(0),
        title,
        fields: &fields,
        clear: &clear,
        body,
    };
    let written = edit(context, CodeReads::None, |_, snapshot, _, work| {
        authoring::update(snapshot, &request, work)
    })?;
    let item = &written.item;
    let verb = match written.files.is_empty() {
        true => "unchanged",
        false => "updated",
    };
    let human = format!("{verb} {} at {}:{}\n", name(item), item.path, item.line);
    Ok(respond(&written, human))
}

fn item_delete(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let written = edit(context, CodeReads::Scan, |_, snapshot, code, work| {
        authoring::delete(snapshot, code, args.argument(0), work)
    })?;
    let item = &written.item;
    let human = format!("deleted {} from {}:{}\n", name(item), item.path, item.line);
    Ok(respond(&written, human))
}

fn relation_add(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let written = edit(context, CodeReads::None, |_, snapshot, _, work| {
        let [source, relation, target] = [0, 1, 2].map(|index| args.argument(index));
        authoring::relation_add(snapshot, source, relation, target, work)
    })?;
    Ok(relation_response(&written, "added", "at"))
}

fn relation_remove(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let written = edit(context, CodeReads::None, |_, snapshot, _, work| {
        let [source, relation, target] = [0, 1, 2].map(|index| args.argument(index));
        authoring::relation_remove(snapshot, source, relation, target, work)
    })?;
    Ok(relation_response(&written, "removed", "from"))
}

/// Runs the write procedure (DES-WRITE-PROCEDURE) for the edit that `compute` makes: takes the
/// write lock, loads the project, refusing one whose configuration or schema has an error, and
/// builds the snapshot; then puts the files of the edit on disk. In direct mode nothing is read,
/// hashed, parsed or built after that (ADR-NO-REDERIVATION-IN-CLI); a daemon's session keeps the
/// read-back parses of the files written (DES-WRITE-PROCEDURE step 4).
fn edit(
    context: &mut Context,
    code: CodeReads,
    compute: impl FnOnce(
        &Inputs,
        &Snapshot,
        Option<&CodeLinks>,
        &mut WorkCounts,
    ) -> Result<Written, EditError>,
) -> Result<Written, Failure> {
    let root = context.root()?;
    let lock = WriteLock::take(&root)?;
    let (mut inputs, _) = open(context)?;
    let generation = inputs.generation(context.now, context.work, code);
    let snapshot = &generation.snapshot;
    let links = (generation.code.clone()).map(|layer| CodeLinks::new(snapshot, layer));
    let written =
        compute(&inputs, snapshot, links.as_ref(), context.work).map_err(|error| Failure {
            diagnostics: error.diagnostics,
            ..Failure::new(error.code, error.message)
        })?;
    // A marker in code that could not be read may name the item (DES-CODE-READERS).
    if let Some(layer) = &generation.code {
        let name = written.item.id.as_deref().unwrap_or(&written.item.mid);
        incomplete_code(layer, name)?;
    }
    let files: Vec<FileWrite> = written
        .files
        .iter()
        .map(|file| FileWrite {
            path: file.path.clone(),
            basis: match file.created {
                true => Basis::Absent,
                false => inputs
                    .basis(&file.path)
                    .expect("a document of the snapshot has a basis"),
            },
            contents: file.contents.to_vec(),
        })
        .collect();
    if !files.is_empty() {
        let parsed = written.files.iter().map(|file| {
            let contents = file.contents.clone();
            let document = file.document.clone();
            (file.path.clone(), ParsedFile { contents, document })
        });
        inputs.apply(&lock, &files, parsed, context.now, context.work)?;
    }
    Ok(written)
}

/// The `io-error` of an edit that depends on every code marker naming the item `name`, when
/// `layer` read less than all the code: nothing is written (DES-CODE-READERS).
pub(super) fn incomplete_code(layer: &CodeLayer, name: &str) -> Result<(), Failure> {
    if layer.unread.is_empty() {
        return Ok(());
    }
    let places = layer.unread.iter().map(|d| d.path.clone());
    let message = format!(
        "Mara cannot read every code file, and a marker there may name {name}, so nothing was \
         written: {}",
        mara_engine::validation::list(places, layer.unread.len())
    );
    Err(Failure {
        diagnostics: layer.unread.clone(),
        ..Failure::new("io-error", message)
    })
}

/// A MID that no item of `snapshot` has.
fn new_mid(snapshot: &Snapshot) -> Result<String, EditError> {
    loop {
        let mid = write::new_mid().map_err(|error| EditError {
            code: "io-error",
            message: format!("cannot read /dev/urandom for a new MID: {error}"),
            diagnostics: Vec::new(),
        })?;
        if snapshot.items_with_mid(&mid).is_empty() {
            return Ok(mid);
        }
    }
}

/// How the human form names an item: its ID, else its MID, its type and its title.
fn name(item: &mara_engine::read::ItemSummary) -> String {
    let id = item.id.as_deref().unwrap_or(&item.mid);
    format!("{id} ({}) {}", item.type_name, item.title)
}

fn relation_response(written: &Written, verb: &str, place: &str) -> Response {
    let relation = written
        .relation
        .as_ref()
        .expect("a relation result has its row");
    let source = &written.item;
    let target = &relation.target;
    let human = format!(
        "{verb} {} {} {} {place} {}:{}\n",
        source.id.as_deref().unwrap_or(&source.mid),
        label(&relation.name),
        target.id.as_deref().unwrap_or(&target.mid),
        source.path,
        relation.line
    );
    respond(written, human)
}

fn respond(written: &Written, human: String) -> Response {
    Response {
        json: written.json(),
        human,
        status: SUCCESS,
    }
}
