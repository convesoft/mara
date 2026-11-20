//! The results of the `query` operations, in pages (DES-QUERY-RESULTS in `docs/rules/index.mara.md`):
//! the entries `query list` shows, the rows of `query execute` and `query view`, the items a
//! gate judges, and the rows `query report` explains.

use crate::config::{CONFIG_PATH, Consumer, Entry, Rules};
use crate::diagnostic::Diagnostic;
use crate::evaluation::explain::Explanation;
use crate::evaluation::{GateOutcome, GateRow, Outcome, Role, compare_rows};
use crate::facts::{Value, write_row};
use crate::hash::Hash;
use crate::json::Object;
use crate::program::{Column, ColumnKind};

use super::node::ItemSummary;
use super::page::{self, Envelope, Head, strings};
use super::{Documents, Page, Paging, ReadError, result_key};

/// `query list`: every query, view, invariant and gate entry of `rules`, in the order of their
/// tables, then by name. `layer` is the key of the configuration as read.
pub fn query_list<'r>(
    rules: Option<&'r Rules>,
    layer: Hash,
    paging: &Paging<'_>,
) -> Result<Page<&'r Entry>, ReadError> {
    let entries: Vec<&Entry> = rules.map_or(Vec::new(), |rules| rules.entries.iter().collect());
    let key = result_key(layer, "query list", [], &[]);
    let envelope = Envelope {
        head: None,
        records: "entries",
        diagnostics: &[],
        kept: 0,
    };
    page::list(
        key,
        paging,
        entries.len(),
        &envelope,
        |index| {
            let entry = entries[index];
            let mut out = String::new();
            let mut object = Object::new(&mut out);
            object.string("name", &entry.name);
            object.string("kind", entry.kind.name());
            object.string("description", &entry.description);
            strings(
                &mut object,
                "inputs",
                entry.inputs.iter().map(|input| input.name.as_str()),
            );
            object.end();
            (entry, out)
        },
        |entry| {
            page::too_large(
                &format!("the entry `{}`", entry.name),
                CONFIG_PATH,
                entry.line,
            )
        },
    )
}

/// A row of a query or view, with the item whose MID its first column holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryRow {
    pub item: Option<ItemSummary>,
    pub values: Vec<Value>,
}

/// What `query execute` or `query view` returns of one evaluation.
#[derive(Clone, Copy, Debug)]
pub struct RowsRequest<'a> {
    /// `query execute` or `query view`.
    pub operation: &'static str,
    /// A query or a view.
    pub entry: &'a Entry,
    /// The columns of its relation, as its program declares them.
    pub columns: &'a [Column],
    /// Every row of its relation.
    pub rows: &'a [Vec<Value>],
    /// The key of the evaluation.
    pub evaluation: Hash,
    /// The items the request names, by their index in the snapshot; empty for every row.
    pub items: &'a [usize],
}

/// The rows of the entry's relation, or of those whose first column is the MID of an item the
/// request names, in the order of their values, each with that item.
pub fn query_rows(
    documents: &Documents<'_>,
    request: &RowsRequest<'_>,
    paging: &Paging<'_>,
) -> Result<Page<QueryRow>, ReadError> {
    let snapshot = documents.snapshot;
    let mut mids: Vec<&str> = (request.items.iter())
        .map(|&item| snapshot.item(item).mid.as_str())
        .collect();
    mids.sort_unstable();
    mids.dedup();
    fn about(row: &[Value]) -> Option<&str> {
        match row.first() {
            Some(Value::Symbol(mid)) => Some(mid.as_str()),
            _ => None,
        }
    }
    let mut rows: Vec<&Vec<Value>> = (request.rows.iter())
        .filter(|row| mids.is_empty() || about(row).is_some_and(|mid| mids.contains(&mid)))
        .collect();
    rows.sort_by(|a, b| compare_rows(a, b));
    let relation = match &request.entry.consumer {
        Consumer::Rows { relation } => relation,
        _ => unreachable!("a query or view reads the rows of one relation"),
    };
    let mut parts = vec![request.entry.name.as_str()];
    parts.extend(&mids);
    let key = result_key(
        request.evaluation,
        request.operation,
        parts,
        documents.diagnostics,
    );
    let mut head = String::new();
    let mut object = Object::new(&mut head);
    object.string("name", &request.entry.name);
    object.string("kind", request.entry.kind.name());
    object.string("relation", &relation.name);
    let columns: Vec<String> = request.columns.iter().map(column_json).collect();
    object.array("columns", columns.iter().map(String::as_str));
    object.end();
    let envelope = Envelope {
        head: Some(Head {
            name: "query",
            json: head,
            path: CONFIG_PATH.to_owned(),
            line: request.entry.line,
        }),
        records: "rows",
        diagnostics: documents.diagnostics,
        kept: 0,
    };
    let item = |row: &Vec<Value>| {
        let mid = about(row)?;
        let index = *snapshot.items_with_mid(mid).first()?;
        Some(ItemSummary::of(snapshot, index))
    };
    page::list(
        key,
        paging,
        rows.len(),
        &envelope,
        |index| {
            let row = QueryRow {
                item: item(rows[index]),
                values: rows[index].clone(),
            };
            let mut out = String::new();
            let mut object = Object::new(&mut out);
            match &row.item {
                Some(item) => object.key("item").push_str(&item.json()),
                None => object.key("item").push_str("null"),
            }
            write_row(object.key("values"), &row.values);
            object.end();
            (row, out)
        },
        |row| match &row.item {
            Some(item) => page::too_large("a row about this item", &item.path, item.line),
            None => page::too_large(
                &format!("a row of `{}`", relation.name),
                CONFIG_PATH,
                relation.line,
            ),
        },
    )
}

/// `{"name", "type"}`: `type` is `symbol` or `number`.
fn column_json(column: &Column) -> String {
    let mut out = String::new();
    let mut object = Object::new(&mut out);
    object.string("name", &column.name);
    let kind = match column.kind {
        ColumnKind::Number => "number",
        _ => "symbol",
    };
    object.string("type", kind);
    object.end();
    out
}

/// An item a gate judged, as its result shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JudgedItem {
    pub item: ItemSummary,
    pub outcome: Outcome,
    pub rows: Vec<GateRow>,
}

/// What `query gate` returns.
#[derive(Clone, Copy, Debug)]
pub struct GateRequest<'a> {
    pub entry: &'a Entry,
    pub outcome: &'a GateOutcome,
    /// The key of the evaluation, or of the snapshot when nothing was evaluated.
    pub layer: Hash,
    /// The problems that made the gate unavailable: a refused program, an entry that does not
    /// fit it, a compile error or a fault.
    pub problems: &'a [Diagnostic],
}

/// The gate's outcome and each named item's, with the rows that caused it. The result's
/// diagnostics are the problems that made it unavailable, every one on every page whatever its
/// size, then the inventory's within their budget.
pub fn gate_page(
    documents: &Documents<'_>,
    request: &GateRequest<'_>,
    paging: &Paging<'_>,
) -> Result<Page<JudgedItem>, ReadError> {
    let snapshot = documents.snapshot;
    let mut diagnostics = request.problems.to_vec();
    diagnostics.extend(documents.diagnostics.iter().cloned());
    let mids: Vec<&str> = (request.outcome.items.iter())
        .map(|judged| snapshot.item(judged.item).mid.as_str())
        .collect();
    let mut parts = vec![request.entry.name.as_str()];
    parts.extend(&mids);
    let key = result_key(request.layer, "query gate", parts, &diagnostics);
    let mut head = String::new();
    let mut object = Object::new(&mut head);
    object.string("name", &request.entry.name);
    object.string("outcome", request.outcome.outcome.name());
    object.end();
    let envelope = Envelope {
        head: Some(Head {
            name: "gate",
            json: head,
            path: CONFIG_PATH.to_owned(),
            line: request.entry.line,
        }),
        records: "items",
        diagnostics: &diagnostics,
        kept: request.problems.len(),
    };
    let items = &request.outcome.items;
    page::list(
        key,
        paging,
        items.len(),
        &envelope,
        |index| {
            let judged = &items[index];
            let record = JudgedItem {
                item: ItemSummary::of(snapshot, judged.item),
                outcome: judged.outcome,
                rows: judged.rows.clone(),
            };
            let rows: Vec<String> = record.rows.iter().map(gate_row_json).collect();
            let mut out = String::new();
            let mut object = Object::new(&mut out);
            object.key("item").push_str(&record.item.json());
            object.string("outcome", record.outcome.name());
            object.array("rows", rows.iter().map(String::as_str));
            object.end();
            (record, out)
        },
        |record| page::too_large("this item's outcome", &record.item.path, record.item.line),
    )
}

/// `{"relation", "role", "values"}`.
fn gate_row_json(row: &GateRow) -> String {
    let mut out = String::new();
    let mut object = Object::new(&mut out);
    object.string("relation", &row.relation);
    object.string("role", row.role.name());
    write_row(object.key("values"), &row.values);
    object.end();
    out
}

/// A row that `query report` explains: a row of an invariant's relation, or of a gate's
/// relation with its role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplainedRow {
    pub relation: String,
    /// The relation's role in a gate; `None` for an invariant.
    pub role: Option<Role>,
    pub values: Vec<Value>,
    pub explanation: Explanation,
}

/// What `query report` returns.
#[derive(Clone, Copy, Debug)]
pub struct ReportRequest<'a> {
    /// An invariant or a gate.
    pub entry: &'a Entry,
    /// The item the report is about, by its index in the snapshot.
    pub item: usize,
    pub outcome: Outcome,
    pub rows: &'a [ExplainedRow],
    /// The key of the evaluation, or of the snapshot when nothing was evaluated.
    pub layer: Hash,
    /// The problems that left the evaluation without rows, as for a gate.
    pub problems: &'a [Diagnostic],
}

/// The entry's outcome for the item and each row that caused it, with its proof or why it has
/// none. The diagnostics are those of [`gate_page`].
pub fn report_page(
    documents: &Documents<'_>,
    request: &ReportRequest<'_>,
    paging: &Paging<'_>,
) -> Result<Page<ExplainedRow>, ReadError> {
    let snapshot = documents.snapshot;
    let mut diagnostics = request.problems.to_vec();
    diagnostics.extend(documents.diagnostics.iter().cloned());
    let item = ItemSummary::of(snapshot, request.item);
    let parts = [request.entry.name.as_str(), item.mid.as_str()];
    let key = result_key(request.layer, "query report", parts, &diagnostics);
    let mut head = String::new();
    let mut object = Object::new(&mut head);
    object.string("name", &request.entry.name);
    object.string("kind", request.entry.kind.name());
    object.key("item").push_str(&item.json());
    object.string("outcome", request.outcome.name());
    object.end();
    let envelope = Envelope {
        head: Some(Head {
            name: "report",
            json: head,
            path: CONFIG_PATH.to_owned(),
            line: request.entry.line,
        }),
        records: "rows",
        diagnostics: &diagnostics,
        kept: request.problems.len(),
    };
    page::list(
        key,
        paging,
        request.rows.len(),
        &envelope,
        |index| {
            let row = &request.rows[index];
            let mut out = String::new();
            let mut object = Object::new(&mut out);
            object.string("relation", &row.relation);
            if let Some(role) = row.role {
                object.string("role", role.name());
            }
            write_row(object.key("values"), &row.values);
            match &row.explanation {
                Ok(proof) => object.key("proof").push_str(&proof.json()),
                Err(reason) => object.string("unexplained", reason),
            }
            object.end();
            (row.clone(), out)
        },
        |_| page::too_large("a row about this item", &item.path, item.line),
    )
}
