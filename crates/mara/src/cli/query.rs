//! `query list`, `query execute`, `query view`, `query gate` and `query report`: a project's
//! named consumers of its programs (DES-RULE-CONSUMERS, DES-GATE-OUTCOME, DES-REQUEST-INPUTS
//! and DES-QUERY-RESULTS in `docs/rules/index.mara.md`), and the explanation of an outcome
//! (REQ-EXPLAIN-OUTCOMES in `docs/rules/evaluation.mara.md`).
//!
//! Each refuses a project whose configuration or schema has an error, as every read does. A
//! request is refused before any program runs when it names no entry of its kind, an unknown or
//! ambiguous item, or inputs that do not fit the entry's.

use std::fmt::Write as _;

use mara::inputs::Generation;
use mara::rules::{self, Consumed, Evaluations};
use mara_engine::config::{Consumer, Entry, EntryKind};
use mara_engine::evaluation::explain::{Node, Target};
use mara_engine::evaluation::{self, Outcome, Rows, compare_rows};
use mara_engine::facts::{InputError, Supplied, Value};
use mara_engine::read::{
    self as engine_read, Documents, ExplainedRow, GateRequest, QueryRow, ReportRequest,
    RowsRequest, gate_page, query_list, query_rows, report_page,
};
use mara_engine::snapshot::Snapshot;

use super::read::{failure, item_line, open, paging, respond, respond_keeping};
use super::{
    Args, Argument, CURSOR, Context, FAILURE, Failure, Kind, LIMIT, Operation, Opt, Response, Run,
    SUCCESS, resolve_item,
};

pub const LIST: Operation = Operation {
    words: &["query", "list"],
    summary: "Lists the project's queries, views, invariants and gates, each with its \
              description and the inputs a request supplies.",
    arguments: &[],
    options: &[LIMIT, CURSOR],
    run: Run::Document(list),
};

pub const EXECUTE: Operation = Operation {
    words: &["query", "execute"],
    summary: "Runs a named query and returns the rows of its relation, or those about the items \
              given.",
    arguments: &[NAME],
    options: &[ID, INPUT, COMPLETE, LIMIT, CURSOR],
    run: Run::Document(execute),
};

pub const VIEW: Operation = Operation {
    words: &["query", "view"],
    summary: "Runs a named view and returns the rows of its relation, or those about the items \
              given; the human form is a table.",
    arguments: &[NAME],
    options: &[ID, INPUT, COMPLETE, LIMIT, CURSOR],
    run: Run::Document(view),
};

pub const GATE: Operation = Operation {
    words: &["query", "gate"],
    summary: "Judges the items given with a named gate: passed, failed, unavailable or not \
              applicable, each with the rows that caused it. Reports conditions; grants nothing.",
    arguments: &[NAME],
    options: &[
        Opt {
            help: "An item to judge, by its ID or MID; at least one.",
            required: true,
            ..ID
        },
        INPUT,
        COMPLETE,
        LIMIT,
        CURSOR,
    ],
    run: Run::Document(gate),
};

pub const REPORT: Operation = Operation {
    words: &["query", "report"],
    summary: "Explains the outcome of a named invariant or gate for one item: each row that \
              caused it, with a proof down to the document lines and supplied inputs it rests \
              on, or why Soufflé cannot give one.",
    arguments: &[NAME],
    options: &[
        Opt {
            help: "The item whose outcome to explain, by its ID or MID.",
            repeatable: false,
            required: true,
            ..ID
        },
        INPUT,
        COMPLETE,
        LIMIT,
        CURSOR,
    ],
    run: Run::Document(report),
};

/// `<name>`: the entry a request runs.
const NAME: Argument = Argument {
    name: "name",
    help: "The entry's name, as `query list` shows it.",
};

/// `--id <id-or-mid>`: the items a request is about. Repeatable.
const ID: Opt = Opt {
    name: "id",
    value: "id-or-mid",
    help: "Only the rows about the item with this ID or MID.",
    repeatable: true,
    required: false,
    kind: Kind::Text,
    check: |_| Ok(()),
};

/// `--input <name>=<value>`: an input the entry lists (DES-REQUEST-INPUTS). Repeatable.
const INPUT: Opt = Opt {
    name: "input",
    value: "name=value",
    help: "An input the entry lists: on the command line `<name>=<value>` for one value or \
           `<name>=@<file>` for a JSON file of rows, relative to the working directory; in MCP \
           an object from each input's name to one value or to its list of rows.",
    repeatable: true,
    required: false,
    kind: Kind::Inputs,
    check: |value| match value.split_once('=') {
        Some((name, _)) if !name.is_empty() => Ok(()),
        _ => Err(format!(
            "`{value}` names no input: write `<name>=<value>` or `<name>=@<file>`"
        )),
    },
};

/// `--complete <input>`: the input holds every record of its source. Repeatable.
const COMPLETE: Opt = Opt {
    name: "complete",
    value: "input",
    help: "An input that holds every record of its source, so that a record it lacks counts as \
           missing rather than unavailable.",
    repeatable: true,
    required: false,
    kind: Kind::Text,
    check: |_| Ok(()),
};

/// Every entry of the configuration. Reads no document and runs no program.
fn list(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    let (_, project) = open(context)?;
    let cursor = args.cursor();
    let layer = engine_read::schema_layer(project.config_hash, project.schema_hash);
    let rules = project.config.rules.as_ref();
    let page = query_list(rules, layer, &paging(args, &cursor)).map_err(failure)?;
    let human = |out: &mut String| {
        for entry in &page.records {
            let _ = write!(
                out,
                "{} ({}): {}",
                entry.name,
                entry.kind.name(),
                entry.description
            );
            let inputs: Vec<&str> = entry.inputs.iter().map(|i| i.name.as_str()).collect();
            if !inputs.is_empty() {
                let _ = write!(out, " [inputs: {}]", inputs.join(", "));
            }
            out.push('\n');
        }
    };
    Ok(respond(page.json.clone(), human, &[], page.next_cursor))
}

fn execute(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    rows(context, args, EntryKind::Query)
}

fn view(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    rows(context, args, EntryKind::View)
}

/// What a request evaluates: the project's inputs, its entry and the items it names.
struct Request {
    generation: Generation,
    entry: usize,
    items: Vec<usize>,
}

impl Request {
    fn entry(&self) -> &Entry {
        let rules = self.generation.project.config.rules.as_ref();
        &rules.expect("the entry is the configuration's").entries[self.entry]
    }

    fn documents(&self) -> Documents<'_> {
        Documents {
            snapshot: &self.generation.snapshot,
            diagnostics: &self.generation.diagnostics,
        }
    }
}

/// Reads the project and finds the entry of one of `kinds` that the request names and the items
/// it names, each once in the order given. Runs nothing.
fn prepare(context: &mut Context, args: &Args, kinds: &[EntryKind]) -> Result<Request, Failure> {
    let (mut inputs, project) = open(context)?;
    let name = args.argument(0);
    let entries = project
        .config
        .rules
        .as_ref()
        .map_or(&[][..], |r| &r.entries[..]);
    let Some(entry) = entries.iter().position(|entry| entry.name == name) else {
        let message = format!(
            "no entry of the configuration is named `{name}`: `mara query list` lists them"
        );
        return Err(Failure::new("unknown-entry", message));
    };
    let found = entries[entry].kind;
    if !kinds.contains(&found) {
        let verb = match found {
            EntryKind::Query => "`mara query execute`",
            EntryKind::View => "`mara query view`",
            EntryKind::Gate => "`mara query gate`",
            EntryKind::Invariant => "`mara project validate`",
        };
        let article = |kind: EntryKind| match kind {
            EntryKind::Invariant => "an",
            _ => "a",
        };
        let wanted: Vec<String> = (kinds.iter())
            .map(|&kind| format!("{} {}", article(kind), kind.name()))
            .collect();
        let message = format!(
            "`{name}` is {} {}, not {}: {verb} runs it",
            article(found),
            found.name(),
            wanted.join(" or ")
        );
        return Err(Failure::new("unknown-entry", message));
    }
    let generation = inputs.snapshot(context.now, context.work);
    let mut items = Vec::new();
    for reference in args.values(ID.name) {
        let item = resolve_item(&generation.snapshot, reference)?;
        if !items.contains(&item) {
            items.push(item);
        }
    }
    Ok(Request {
        generation,
        entry,
        items,
    })
}

/// The refusal of a request whose inputs do not fit its entry's.
fn refuse(error: InputError) -> Failure {
    Failure::new(error.code(), error.to_string())
}

/// The inputs the request supplies, and those it marks complete.
type Inputs = (Vec<(String, Supplied)>, Vec<String>);

fn supplied(context: &mut Context, args: &Args) -> Result<Inputs, Failure> {
    // The client's directory: a file an argument names is the client's (DES-DAEMON-LIFECYCLE).
    let base = context.cwd()?.to_owned();
    let mut supplied: Vec<(String, Supplied)> = Vec::new();
    for argument in args.values(INPUT.name) {
        supplied.push(rules::supplied_input(argument, &base, context.work).map_err(refuse)?);
    }
    supplied.extend(args.supplied().iter().cloned());
    let complete: Vec<String> = args.values(COMPLETE.name).map(str::to_owned).collect();
    Ok((supplied, complete))
}

/// The inputs the request supplies, then the entry evaluated with them.
fn evaluate(context: &mut Context, args: &Args, request: &Request) -> Result<Consumed, Failure> {
    let (supplied, complete) = supplied(context, args)?;
    let mut evaluations = Evaluations::default();
    rules::consume(
        &request.generation,
        request.entry,
        supplied,
        &complete,
        &mut evaluations,
        context.work,
    )
    .map_err(refuse)
}

/// `query execute` and `query view`: the rows of the entry's relation. A program that cannot
/// give them fails the request with its problems.
fn rows(context: &mut Context, args: &Args, kind: EntryKind) -> Result<Response, Failure> {
    let request = prepare(context, args, &[kind])?;
    let consumed = evaluate(context, args, &request)?;
    let entry = request.entry();
    let Consumer::Rows { relation } = &entry.consumer else {
        unreachable!("a query or view reads the rows of one relation");
    };
    let rows = match consumed.outcome {
        Ok(rows) => rows,
        Err(problems) => {
            let message = format!(
                "the program of `{}` gives no rows: the diagnostics say why",
                entry.name
            );
            return Err(Failure {
                diagnostics: problems,
                ..Failure::new("program-failed", message)
            });
        }
    };
    let all = rows
        .relations
        .get(&relation.name)
        .map_or(&[][..], Vec::as_slice);
    let columns = consumed
        .columns
        .get(&relation.name)
        .map_or(&[][..], Vec::as_slice);
    let operation = match kind {
        EntryKind::View => "query view",
        _ => "query execute",
    };
    let documents = request.documents();
    let cursor = args.cursor();
    let rows_request = RowsRequest {
        operation,
        entry,
        columns,
        rows: all,
        evaluation: consumed.key.expect("an evaluation with rows has a key"),
        items: &request.items,
    };
    let page = query_rows(&documents, &rows_request, &paging(args, &cursor)).map_err(failure)?;
    let names: Vec<&str> = columns.iter().map(|column| column.name.as_str()).collect();
    let human = |out: &mut String| match kind {
        EntryKind::View => table(out, &names, &page.records),
        _ => page.records.iter().for_each(|row| row_line(out, row)),
    };
    Ok(respond(
        page.json.clone(),
        human,
        documents.diagnostics,
        page.next_cursor,
    ))
}

/// `query gate`: the outcome of each named item and of the gate. Exit status 0 only when the
/// gate passed.
fn gate(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    if args.value(ID.name).is_none() {
        return Err(Failure::rejected(
            "`query gate` needs `--id <id-or-mid>`: the items to judge".to_owned(),
        ));
    }
    let request = prepare(context, args, &[EntryKind::Gate])?;
    let consumed = evaluate(context, args, &request)?;
    let entry = request.entry();
    let snapshot = &request.generation.snapshot;
    let (rows, problems) = match &consumed.outcome {
        Ok(rows) => (Some(rows), &[][..]),
        Err(problems) => (None, &problems[..]),
    };
    let outcome = evaluation::judge(entry, rows, &request.items, snapshot);
    let documents = request.documents();
    let cursor = args.cursor();
    let gate_request = GateRequest {
        entry,
        outcome: &outcome,
        layer: consumed.key.unwrap_or(snapshot.key().0),
        problems,
    };
    let page = gate_page(&documents, &gate_request, &paging(args, &cursor)).map_err(failure)?;
    let mut diagnostics = problems.to_vec();
    diagnostics.extend(documents.diagnostics.iter().cloned());
    let human = |out: &mut String| {
        let _ = writeln!(out, "gate {}: {}", entry.name, outcome.outcome.name());
        for judged in &page.records {
            let _ = writeln!(
                out,
                "{}: {}",
                item_line(&judged.item),
                judged.outcome.name()
            );
            for row in &judged.rows {
                let _ = writeln!(
                    out,
                    "  {} {}({})",
                    row.role.name(),
                    row.relation,
                    values(&row.values, ", ")
                );
            }
        }
    };
    let mut response = respond_keeping(
        page.json.clone(),
        human,
        &diagnostics,
        problems.len(),
        page.next_cursor,
    );
    response.status = match outcome.outcome {
        Outcome::Passed => SUCCESS,
        _ => FAILURE,
    };
    Ok(response)
}

/// `query report`: the outcome of an invariant or a gate for one item, and each row that caused
/// it with its proof, from one run in provenance mode. Exit status 0 only when every such row
/// is explained; an explanation never changes the outcome.
fn report(context: &mut Context, args: &Args) -> Result<Response, Failure> {
    if args.value(ID.name).is_none() {
        return Err(Failure::rejected(
            "`query report` needs `--id <id-or-mid>`: the item whose outcome to explain".to_owned(),
        ));
    }
    let request = prepare(context, args, &[EntryKind::Invariant, EntryKind::Gate])?;
    let (supplied, complete) = supplied(context, args)?;
    let entry = request.entry();
    let item = request.items[0];
    let snapshot = &request.generation.snapshot;
    let mut select = |rows: &Rows| caused(entry, rows, item, snapshot).0;
    let consumed = rules::consume_explained(
        &request.generation,
        request.entry,
        supplied,
        &complete,
        &mut select,
        context.work,
    )
    .map_err(refuse)?;
    let (outcome, rows, problems) = match consumed.outcome {
        Ok(explained) => {
            let (targets, roles, outcome) = caused(entry, &explained.rows, item, snapshot);
            debug_assert_eq!(targets, explained.targets);
            let rows: Vec<ExplainedRow> = (targets.into_iter().zip(roles))
                .zip(explained.explanations)
                .map(|((target, role), explanation)| ExplainedRow {
                    relation: target.relation,
                    role,
                    values: target.values,
                    explanation,
                })
                .collect();
            (outcome, rows, Vec::new())
        }
        Err(problems) => (Outcome::Unavailable, Vec::new(), problems),
    };
    let documents = request.documents();
    let cursor = args.cursor();
    let report_request = ReportRequest {
        entry,
        item,
        outcome,
        rows: &rows,
        layer: consumed.key.unwrap_or(snapshot.key().0),
        problems: &problems,
    };
    let page = report_page(&documents, &report_request, &paging(args, &cursor)).map_err(failure)?;
    let mut diagnostics = problems.clone();
    diagnostics.extend(documents.diagnostics.iter().cloned());
    let summary = engine_read::ItemSummary::of(snapshot, item);
    let human = |out: &mut String| {
        let _ = writeln!(
            out,
            "{} {}: {}: {}",
            entry.kind.name(),
            entry.name,
            item_line(&summary),
            outcome.name()
        );
        for row in &page.records {
            let role = row.role.map(|role| format!("{} ", role.name()));
            let _ = writeln!(
                out,
                "{}{}({})",
                role.unwrap_or_default(),
                row.relation,
                values(&row.values, ", ")
            );
            match &row.explanation {
                Ok(proof) => proof_lines(out, proof, 1),
                Err(reason) => {
                    let _ = writeln!(out, "  cannot explain: {reason}");
                }
            }
        }
    };
    let mut response = respond_keeping(
        page.json.clone(),
        human,
        &diagnostics,
        problems.len(),
        page.next_cursor,
    );
    let explained = rows.iter().all(|row| row.explanation.is_ok());
    response.status = match problems.is_empty() && explained {
        true => SUCCESS,
        false => FAILURE,
    };
    Ok(response)
}

/// The rows of `rows` that make the outcome of `entry` for `item`, with their role in a gate,
/// and that outcome: for an invariant, the rows of its relation about the item, failed when
/// there is one and passed otherwise; for a gate, the rows that `query gate` lists for the item
/// and its outcome there.
fn caused(
    entry: &Entry,
    rows: &Rows,
    item: usize,
    snapshot: &Snapshot,
) -> (Vec<Target>, Vec<Option<evaluation::Role>>, Outcome) {
    match &entry.consumer {
        Consumer::Invariant { relation, .. } => {
            let mid = &snapshot.item(item).mid;
            let all = rows
                .relations
                .get(&relation.name)
                .map_or(&[][..], Vec::as_slice);
            let mut own: Vec<&Vec<Value>> = all
                .iter()
                .filter(|row| matches!(row.first(), Some(Value::Symbol(first)) if first == mid))
                .collect();
            own.sort_by(|a, b| compare_rows(a, b));
            own.dedup();
            let outcome = match own.is_empty() {
                true => Outcome::Passed,
                false => Outcome::Failed,
            };
            let targets: Vec<Target> = (own.into_iter())
                .map(|values| Target {
                    relation: relation.name.clone(),
                    values: values.clone(),
                })
                .collect();
            let roles = vec![None; targets.len()];
            (targets, roles, outcome)
        }
        _ => {
            let judged = evaluation::judge(entry, Some(rows), &[item], snapshot);
            let item = judged
                .items
                .into_iter()
                .next()
                .expect("one item was judged");
            let roles = item.rows.iter().map(|row| Some(row.role)).collect();
            let targets = (item.rows.into_iter())
                .map(|row| Target {
                    relation: row.relation,
                    values: row.values,
                })
                .collect();
            (targets, roles, item.outcome)
        }
    }
}

/// The human form of a proof, one step per line, each indented under the step it serves.
fn proof_lines(out: &mut String, node: &Node, depth: usize) {
    let indent = "  ".repeat(depth);
    match node {
        Node::Derived {
            atom,
            rule,
            children,
            ..
        } => {
            let _ = writeln!(out, "{indent}{atom} by {rule}");
            for child in children {
                proof_lines(out, child, depth + 1);
            }
        }
        Node::Fact {
            relation,
            values: row,
            path,
            line,
        } => {
            let _ = writeln!(
                out,
                "{indent}{relation}({}) at {path}:{line}",
                values(row, ", ")
            );
        }
        Node::Input { name, values: row } => {
            let _ = writeln!(
                out,
                "{indent}{name}({}) from the input `{name}`",
                values(row, ", ")
            );
        }
        Node::Absent { atom } => {
            let _ = writeln!(out, "{indent}no {atom}");
        }
    }
}

/// A row of `query execute`: the item it is about, then its values.
fn row_line(out: &mut String, row: &QueryRow) {
    if let Some(item) = &row.item {
        let _ = write!(out, "{}:{}: ", item.path, item.line);
    }
    let _ = writeln!(out, "{}", values(&row.values, "\t"));
}

/// The rows of `query view` as a Markdown table under the column names.
fn table(out: &mut String, names: &[&str], rows: &[QueryRow]) {
    let cell = |text: &str| {
        text.replace('\\', "\\\\")
            .replace('|', "\\|")
            .replace('\n', " ")
    };
    let header: Vec<String> = names.iter().map(|name| cell(name)).collect();
    let _ = writeln!(out, "| {} |", header.join(" | "));
    let _ = writeln!(out, "|{}", " --- |".repeat(names.len().max(1)));
    for row in rows {
        let cells: Vec<String> = row.values.iter().map(|value| cell(&text(value))).collect();
        let _ = writeln!(out, "| {} |", cells.join(" | "));
    }
}

fn values(values: &[Value], separator: &str) -> String {
    let texts: Vec<String> = values.iter().map(text).collect();
    texts.join(separator)
}

fn text(value: &Value) -> String {
    match value {
        Value::Symbol(text) => text.clone(),
        Value::Number(number) => number.to_string(),
    }
}
