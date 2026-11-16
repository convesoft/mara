//! `search`, `get`, `relation list`, `item list`, `schema list` and `schema get`
//! (`docs/retrieval/index.mara.md`, `docs/retrieval/index.mara.md` and REQ-SCHEMA-INSPECTION in
//! `docs/project.mara.md`).
//!
//! Each refuses a project whose configuration or schema has an error, with those diagnostics
//! (DES-PROJECT-CONFIGURATION).

use std::fmt::Write as _;
use std::sync::Arc;

use mara::inputs::{CodeReads, Inputs};
use mara::project::Project;
use mara_engine::code::CodeLinks;
use mara_engine::diagnostic::Diagnostic;
use mara_engine::document::is_mid;
use mara_engine::paging::Cursor;
use mara_engine::read::{
    self, Connection, ConnectionKind, Declaration, Direction, Documents, Filters, ItemSummary,
    Neighbour, NodeSummary, Paging, Portion, ReadError, SchemaLayer, SearchHit,
};
use mara_engine::schema::FieldKind;
use mara_engine::validation::is_id;
use mara_engine::write::label;

use super::validate::PATH;
use super::{
    Args, Argument, CURSOR, Context, Failure, Kind, LIMIT, Operation, Opt, Response, Run, SUCCESS,
    project_path,
};

pub const SEARCH: Operation = Operation {
    words: &["search"],
    summary: "Finds items and narrative by their words, ranked, among the nodes that match every \
              filter.",
    arguments: &[Argument {
        name: "query",
        help: "Words to find; a query without words matches every node.",
    }],
    options: &[TYPE, FIELD, RELATION, PATH, ID, LIMIT, CURSOR],
    run: Run::Document(search),
};

pub const GET: Operation = Operation {
    words: &["get"],
    summary: "Reads a node: an item's rows and body, a narrative unit or a whole document.",
    arguments: &[NODE],
    options: &[CURSOR],
    run: Run::Document(get),
};

pub const RELATION_LIST: Operation = Operation {
    words: &["relation", "list"],
    summary: "Lists the relations and mentions of a node, outgoing then incoming.",
    arguments: &[NODE],
    options: &[RELATION, LIMIT, CURSOR],
    run: Run::Document(relation_list),
};

pub const ITEM_LIST: Operation = Operation {
    words: &["item", "list"],
    summary: "Lists the items that match every filter.",
    arguments: &[],
    options: &[TYPE, FIELD, RELATION, PATH, ID, LIMIT, CURSOR],
    run: Run::Document(item_list),
};

pub const SCHEMA_LIST: Operation = Operation {
    words: &["schema", "list"],
    summary: "Lists the types and relations that the schema declares.",
    arguments: &[],
    options: &[LIMIT, CURSOR],
    run: Run::Document(schema_list),
};

pub const SCHEMA_GET: Operation = Operation {
    words: &["schema", "get"],
    summary: "Reads the declaration of a type or relation: its fields, values, targets and \
              constraints.",
    arguments: &[Argument {
        name: "name",
        help: "A type's name, or a relation's forward or inverse name.",
    }],
    options: &[LIMIT, CURSOR],
    run: Run::Document(schema_get),
};

/// `<node>`: what `get` and `relation list` read.
const NODE: Argument = Argument {
    name: "node",
    help: "An item's ID or MID, `<path>:<line>` of an item, a narrative unit or a line of code, \
           or the `<path>` of a document or code file.",
};

/// `--type <name>`: items of this type. Repeatable (DES-SEARCH-REQUESTS).
const TYPE: Opt = Opt {
    name: "type",
    value: "name",
    help: "Only items of this type.",
    repeatable: true,
    required: false,
    kind: Kind::Text,
    check: |_| Ok(()),
};

/// `--field <key>=<value>`: items with this row. Repeatable.
const FIELD: Opt = Opt {
    name: "field",
    value: "key=value",
    help: "`<key>=<value>`: only items with this row.",
    repeatable: true,
    required: false,
    kind: Kind::Text,
    check: |value| match value.split_once('=') {
        Some((key, _)) if !key.is_empty() => Ok(()),
        _ => Err(format!("a field filter is `<key>=<value>`, not `{value}`")),
    },
};

/// `--relation <name>`: connections of this relation, by its forward or inverse name.
/// Repeatable.
const RELATION: Opt = Opt {
    name: "relation",
    value: "name",
    help: "Only edges of this relation: by its forward name those that leave, by its inverse \
           name those that arrive.",
    repeatable: true,
    required: false,
    kind: Kind::Text,
    check: |_| Ok(()),
};

/// `--id <id-or-mid>`: the items with this `ID` value or MID. Repeatable.
const ID: Opt = Opt {
    name: "id",
    value: "id-or-mid",
    help: "Only the items with this ID or MID.",
    repeatable: true,
    required: false,
    kind: Kind::Text,
    check: |_| Ok(()),
};

/// Selects and loads the project, refusing one whose configuration or schema has an error.
pub(super) fn open(context: &mut Context) -> Result<(Inputs, Arc<Project>), Failure> {
    let mut inputs = Inputs::new(context.root()?);
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
    Ok((inputs, project))
}

/// Runs `read` on the documents of the project.
fn with_documents(
    context: &mut Context,
    read: impl FnOnce(&Documents<'_>) -> Result<Response, ReadError>,
) -> Result<Response, Failure> {
    let (mut inputs, _) = open(context)?;
    let generation = inputs.snapshot(context.now, context.work);
    let documents = Documents {
        snapshot: &generation.snapshot,
        diagnostics: &generation.diagnostics,
    };
    read(&documents).map_err(failure)
}

/// Runs `read` on the schema of the project. Reads no document.
fn with_schema(
    context: &mut Context,
    read: impl FnOnce(&SchemaLayer<'_>) -> Result<Response, ReadError>,
) -> Result<Response, Failure> {
    let (_, project) = open(context)?;
    // A project without errors has a schema; one without is refused like any other.
    let Some(schema) = project.schema.as_deref() else {
        let message = "the project has no schema that Mara could read".to_owned();
        return Err(Failure::new("invalid-project", message));
    };
    let path = project
        .config
        .schema
        .as_ref()
        .map_or("", |s| s.path.as_str());
    let layer = SchemaLayer {
        schema,
        key: read::schema_layer(project.config_hash, project.schema_hash),
        path,
    };
    read(&layer).map_err(failure)
}

pub(super) fn failure(error: ReadError) -> Failure {
    let diagnostics = match &error {
        ReadError::TooLarge(diagnostic) => vec![(**diagnostic).clone()],
        _ => Vec::new(),
    };
    Failure {
        diagnostics,
        ..Failure::new(error.code(), error.to_string())
    }
}

pub(super) fn paging<'c>(args: &Args, cursor: &'c Option<Cursor>) -> Paging<'c> {
    Paging {
        limit: args.limit(),
        cursor: cursor.as_ref(),
    }
}

fn filters(args: &Args) -> Filters {
    let values = |name| args.values(name).map(str::to_owned).collect();
    Filters {
        types: values(TYPE.name),
        fields: args
            .values(FIELD.name)
            .filter_map(|value| value.split_once('='))
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect(),
        relations: values(RELATION.name),
        paths: args
            .values(PATH.name)
            // Cannot fire: the option's check ran `project_path` on it before the operation.
            .map(|path| project_path(path).expect("checked before the operation ran"))
            .collect(),
        ids: values(ID.name),
    }
}

fn search(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let cursor = args.cursor();
    with_documents(context, |documents| {
        let page = read::search(
            documents,
            args.argument(0),
            &filters(args),
            &paging(args, &cursor),
        )?;
        Ok(respond(
            page.json,
            |out| hits(out, &page.records),
            documents.diagnostics,
            page.next_cursor,
        ))
    })
}

fn item_list(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let cursor = args.cursor();
    with_documents(context, |documents| {
        let page = read::item_list(documents, &filters(args), &paging(args, &cursor))?;
        let human = |out: &mut String| {
            for item in &page.records {
                let _ = writeln!(out, "{}", item_line(item));
            }
        };
        Ok(respond(
            page.json.clone(),
            human,
            documents.diagnostics,
            page.next_cursor,
        ))
    })
}

fn relation_list(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let cursor = args.cursor();
    let names: Vec<String> = args.values(RELATION.name).map(str::to_owned).collect();
    let (mut inputs, _) = open(context)?;
    let generation = inputs.generation(context.now, context.work, CodeReads::Scan);
    let layer = generation.code.clone().expect("code was scanned");
    let code = CodeLinks::new(&generation.snapshot, layer);
    let documents = Documents {
        snapshot: &generation.snapshot,
        diagnostics: &generation.diagnostics,
    };
    let documents = &documents;
    let listed = (|| {
        let address = args.argument(0);
        let paging = paging(args, &cursor);
        let page = read::relation_list(documents, Some(&code), address, &names, &paging)?;
        let human = |out: &mut String| {
            let _ = writeln!(out, "{}", node_line(&page.node));
            for connection in &page.page.records {
                connection_line(out, connection);
            }
        };
        Ok(respond(
            page.page.json.clone(),
            human,
            documents.diagnostics,
            page.page.next_cursor,
        ))
    })();
    listed.map_err(failure)
}

/// The path that `address` names when it may name a code file: neither an ID nor a MID, with
/// or without a line. The inventory tells whether it is one; a name ending in `.mara.md` can be
/// a code file too.
fn code_path(address: &str) -> Option<&str> {
    if is_id(address) || is_mid(address) {
        return None;
    }
    let path = match address.rsplit_once(':') {
        Some((path, line)) if !line.is_empty() && line.bytes().all(|b| b.is_ascii_digit()) => path,
        _ => address,
    };
    Some(path)
}

fn get(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let cursor = args.cursor();
    let address = args.argument(0);
    if let Some(path) = code_path(address) {
        let (mut inputs, _) = open(context)?;
        let generation = inputs.generation(context.now, context.work, CodeReads::List);
        if generation
            .code_paths
            .binary_search_by(|p| p.as_str().cmp(path))
            .is_ok()
        {
            let (hash, contents) =
                inputs
                    .read_code(path, context.now, context.work)
                    .map_err(|diagnostic| Failure {
                        message: diagnostic.message.clone(),
                        diagnostics: vec![*diagnostic],
                        ..Failure::new("io-error", String::new())
                    })?;
            let line = match address.len() > path.len() {
                true => Some(address[path.len() + 1..].parse().unwrap_or(usize::MAX)),
                false => None,
            };
            let node = NodeSummary::Code {
                path: path.to_owned(),
                line,
            };
            let diagnostics = &generation.diagnostics;
            let portion = read::get_code(node, hash, &contents, cursor.as_ref(), diagnostics)
                .map_err(failure)?;
            let human = |out: &mut String| portion_lines(out, &portion);
            let next = portion.next_cursor;
            return Ok(respond(portion.json.clone(), human, diagnostics, next));
        }
        let documents = Documents {
            snapshot: &generation.snapshot,
            diagnostics: &generation.diagnostics,
        };
        let portion = read::get(&documents, address, cursor.as_ref()).map_err(failure)?;
        let human = |out: &mut String| portion_lines(out, &portion);
        let next = portion.next_cursor;
        return Ok(respond(
            portion.json.clone(),
            human,
            documents.diagnostics,
            next,
        ));
    }
    with_documents(context, |documents| {
        let portion = read::get(documents, args.argument(0), cursor.as_ref())?;
        let human = |out: &mut String| portion_lines(out, &portion);
        Ok(respond(
            portion.json.clone(),
            human,
            documents.diagnostics,
            portion.next_cursor,
        ))
    })
}

fn schema_list(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let cursor = args.cursor();
    with_schema(context, |layer| {
        let page = read::schema_list(layer, &paging(args, &cursor))?;
        let human = |out: &mut String| page.records.iter().for_each(|d| declaration_line(out, d));
        Ok(respond(page.json.clone(), human, &[], page.next_cursor))
    })
}

fn schema_get(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let cursor = args.cursor();
    with_schema(context, |layer| {
        let page = read::schema_get(layer, args.argument(0), &paging(args, &cursor))?;
        let human = |out: &mut String| page.records.iter().for_each(|d| declaration(out, d));
        Ok(respond(page.json.clone(), human, &[], page.next_cursor))
    })
}

/// A successful read: its JSON form, and its human form of the records that `human` writes,
/// then the inventory's diagnostics that a page shows and how many others there are, then the
/// next cursor (DES-READ-RESULTS).
pub(super) fn respond(
    json: String,
    human: impl FnOnce(&mut String),
    diagnostics: &[Diagnostic],
    next_cursor: Option<Cursor>,
) -> Response {
    respond_keeping(json, human, diagnostics, 0, next_cursor)
}

/// [`respond`], showing the first `kept` diagnostics whatever their size.
pub(super) fn respond_keeping(
    json: String,
    human: impl FnOnce(&mut String),
    diagnostics: &[Diagnostic],
    kept: usize,
    next_cursor: Option<Cursor>,
) -> Response {
    let mut out = String::new();
    human(&mut out);
    let shown = read::shown_diagnostics(diagnostics, kept);
    for diagnostic in &diagnostics[..shown] {
        let _ = writeln!(out, "{diagnostic}");
    }
    if shown < diagnostics.len() {
        let more = diagnostics.len() - shown;
        let _ = writeln!(
            out,
            "and {more} more problems with the project's inputs: `mara project validate` reports \
             each of them"
        );
    }
    if let Some(cursor) = next_cursor {
        let _ = writeln!(out, "more: repeat with `--cursor {cursor}`");
    }
    Response {
        json,
        human: out,
        status: SUCCESS,
    }
}

/// How a node reads in one line, starting with `<path>:<line>: `.
fn node_line(node: &NodeSummary) -> String {
    match node {
        NodeSummary::Item(item) => item_line(item),
        NodeSummary::Unit {
            path,
            line,
            heading,
        } => format!(
            "{path}:{line}: {}",
            heading.as_deref().unwrap_or("(narrative)")
        ),
        NodeSummary::Document { path } => path.clone(),
        NodeSummary::Code {
            path,
            line: Some(line),
        } => format!("{path}:{line}: (code)"),
        NodeSummary::Code { path, line: None } => format!("{path} (code)"),
    }
}

pub(super) fn item_line(item: &ItemSummary) -> String {
    format!(
        "{}:{}: {} ({}) {}",
        item.path,
        item.line,
        item.id.as_deref().unwrap_or(&item.mid),
        item.type_name,
        item.title
    )
}

fn hits(out: &mut String, hits: &[SearchHit]) {
    for hit in hits {
        let _ = writeln!(out, "{}", node_line(&hit.node));
        let _ = writeln!(out, "    {}: {}", hit.excerpt.line, hit.excerpt.text);
    }
}

fn connection_line(out: &mut String, connection: &Connection) {
    let name = match (connection.kind, &connection.relation, connection.direction) {
        (ConnectionKind::Relation, Some(relation), _) => label(relation),
        (_, _, Direction::Outgoing) => "Mentions".to_owned(),
        (_, _, Direction::Incoming) => "Mentioned by".to_owned(),
    };
    let neighbour = match &connection.neighbour {
        Neighbour::Node(node) => node_line(node),
        Neighbour::Unresolved { mid } => format!("{mid}, which no item has"),
    };
    let source = &connection.source;
    let _ = writeln!(
        out,
        "  {name} {neighbour}  [at {}:{}]",
        source.path, source.line
    );
}

fn portion_lines(out: &mut String, portion: &Portion) {
    let _ = writeln!(out, "{}", node_line(&portion.node));
    if let NodeSummary::Item(item) = &portion.node {
        let _ = writeln!(out, "MID: {}", item.mid);
    }
    if let Some(revision) = &portion.revision {
        let _ = writeln!(out, "revision: {revision}");
    }
    let path = portion.node.location().0;
    for row in &portion.rows {
        let more = if row.partial {
            " (continued on the next page)"
        } else {
            ""
        };
        let _ = writeln!(out, "{path}:{}: {}: {}{more}", row.line, row.key, row.value);
    }
    if !portion.content.is_empty() {
        out.push('\n');
        out.push_str(&portion.content);
        if !portion.content.ends_with(['\n', '\r']) {
            out.push('\n');
        }
    }
}

/// One declaration in one line, as `schema list` shows it.
fn declaration_line(out: &mut String, declaration: &Declaration<'_>) {
    let _ = match declaration {
        Declaration::Type {
            name, declaration, ..
        } => writeln!(
            out,
            "type {name} ({}): {}",
            declaration.id_prefix.as_deref().unwrap_or("no ID prefix"),
            declaration.description
        ),
        Declaration::Relation { name, declaration } => writeln!(
            out,
            "relation {name} ({}): {}",
            declaration.inverse.as_deref().unwrap_or("no inverse"),
            declaration.description
        ),
    };
}

/// A whole declaration, as `schema get` shows it.
fn declaration(out: &mut String, declaration: &Declaration<'_>) {
    match declaration {
        Declaration::Type {
            name,
            declaration,
            statuses,
        } => {
            let _ = writeln!(out, "type {name}");
            if let Some(prefix) = &declaration.id_prefix {
                let _ = writeln!(out, "  ID prefix: {prefix}");
            }
            let _ = writeln!(out, "  description: {}", declaration.description);
            let _ = writeln!(out, "  statuses: {}", statuses.join(", "));
            for (field, decl) in &declaration.fields {
                let kind = match &decl.kind {
                    FieldKind::Text => "text".to_owned(),
                    FieldKind::Enum(values) => format!("enum of {}", values.join(", ")),
                };
                let required = if decl.required { ", required" } else { "" };
                let repeatable = if decl.repeatable { ", repeatable" } else { "" };
                let _ = writeln!(out, "  field {field}: {kind}{required}{repeatable}");
            }
        }
        Declaration::Relation { name, declaration } => {
            let _ = writeln!(out, "relation {name}");
            if let Some(inverse) = &declaration.inverse {
                let _ = writeln!(out, "  inverse: {inverse}");
            }
            let _ = writeln!(out, "  description: {}", declaration.description);
            for (source, targets) in &declaration.targets {
                let _ = writeln!(out, "  targets of {source}: {}", targets.join(", "));
            }
            let yes = |flag: bool| if flag { "yes" } else { "no" };
            let _ = writeln!(
                out,
                "  same type: {}; acyclic: {}",
                yes(declaration.same_type),
                yes(declaration.acyclic)
            );
        }
    }
}
