//! What a program reads: `mara.dl`, the fact relations of a document snapshot
//! (DES-FACT-BRIDGE in `docs/rules-facts.mara.md`) and the inputs a request supplies
//! (DES-REQUEST-INPUTS in `docs/rules.mara.md`). The host writes each relation's JSON form to
//! the file [`file_name`] names and generates the directives that read it (DES-RULE-EXECUTION).

mod inputs;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::fmt::Write as _;

use unicode_normalization::UnicodeNormalization;

use crate::document::label_name;
use crate::schema::FieldKind;
use crate::snapshot::{EdgeSource, Snapshot};

pub use inputs::{Column, InputDecl, InputError, Inputs, Supplied, check_inputs};

/// The format version of [`MARA_DL`].
pub const MARA_DL_VERSION: u32 = 1;

/// The declarations file that programs include with `.include "mara.dl"`.
pub const MARA_DL: &str = r#".once
// mara.dl format version 1 (DES-FACT-BRIDGE). Mara supplies these relations' facts.

.type Mid <: symbol
.type Id <: symbol
.type Type <: symbol
.type Status <: symbol
.type Rel <: symbol
.type Path <: symbol

.decl item(mid: Mid, type: Type, id: Id, status: Status, path: Path, line: number)
.decl title(mid: Mid, text: symbol)
.decl revision(mid: Mid, hash: symbol)
.decl field(mid: Mid, key: symbol, value: symbol)
.decl folded(text: symbol, folded: symbol)
.decl edge(rel: Rel, src: Mid, dst: Mid, path: Path, line: number)
.decl source_complete(source: symbol)
"#;

/// The fact relations of a snapshot, in the order [`MARA_DL`] declares them.
pub const SNAPSHOT_RELATIONS: [&str; 6] = ["item", "title", "revision", "field", "folded", "edge"];

/// The relation of the sources whose records are all present; its rows come with a request.
pub const SOURCE_COMPLETE: &str = "source_complete";

/// The file that holds the facts of `relation`, in the evaluation's private directory.
pub fn file_name(relation: &str) -> String {
    format!("{relation}.json")
}

/// A value of a column: Soufflé's `symbol` or `number`, a signed 32-bit integer.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    Symbol(String),
    Number(i32),
}

/// One relation's rows, with their JSON form: a list of rows, each a list of values in column
/// order (`IO=jsonfile`, `format=list`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relation {
    pub name: String,
    pub rows: Vec<Vec<Value>>,
    pub json: String,
}

impl Relation {
    pub fn new(name: &str, rows: Vec<Vec<Value>>) -> Relation {
        let mut json = String::from("[");
        for (index, row) in rows.iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            write_row(&mut json, row);
        }
        json.push(']');
        Relation {
            name: name.to_owned(),
            rows,
            json,
        }
    }
}

/// Appends `row` as a JSON list of its values: a string for a symbol, a number for a number.
pub fn write_row(out: &mut String, row: &[Value]) {
    out.push('[');
    for (column, value) in row.iter().enumerate() {
        if column > 0 {
            out.push(',');
        }
        match value {
            Value::Symbol(text) => crate::json::string(out, text),
            Value::Number(number) => {
                write!(out, "{number}").expect("writing to a String cannot fail")
            }
        }
    }
    out.push(']');
}

/// The fact relations of one snapshot, [`SNAPSHOT_RELATIONS`] in that order. Derived once per
/// snapshot ([`Snapshot::facts`]).
#[derive(Debug)]
pub struct Facts {
    pub relations: Vec<Relation>,
}

/// The NFC, lowercased form of `text`: search's normalization ([`crate::document::words`]),
/// without splitting words.
pub fn fold(text: &str) -> String {
    text.to_lowercase().nfc().collect()
}

/// A line as a `number` column; no realistic document has more lines.
fn line(line: usize) -> Value {
    Value::Number(i32::try_from(line).unwrap_or(i32::MAX))
}

/// The fact relations of `snapshot`, in one pass over its items. Type, status, field keys,
/// enum values and relation names are schema names; a field is a row whose name the schema
/// declares as a field of the item's type; an edge is a relation row whose value links to a
/// MID, at that row's line. Mentions are no edges.
pub(crate) fn derive(snapshot: &Snapshot) -> Facts {
    let count = snapshot.items().len();
    let (mut items, mut titles) = (Vec::with_capacity(count), Vec::with_capacity(count));
    let (mut revisions, mut fields, mut folded, mut edges) = (
        Vec::with_capacity(count),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let mut seen = HashSet::new();
    let mut fold_once = |text: &str, folded: &mut Vec<Vec<Value>>| {
        if seen.insert(text.to_owned()) {
            let symbol = Value::Symbol(text.to_owned());
            folded.push(vec![symbol, Value::Symbol(fold(text))]);
        }
    };
    let symbol = |text: &str| Value::Symbol(text.to_owned());
    for (index, entry) in snapshot.items().iter().enumerate() {
        let item = snapshot.item(index);
        let path = &snapshot.documents()[entry.document].path;
        let kind = label_name(item.value("type").unwrap_or_default());
        let status = item.value("status").map(label_name).unwrap_or_default();
        items.push(vec![
            symbol(&item.mid),
            symbol(&kind),
            symbol(item.value("id").unwrap_or_default()),
            symbol(&status),
            symbol(path),
            line(item.line),
        ]);
        titles.push(vec![symbol(&item.mid), symbol(&item.title)]);
        fold_once(&item.title, &mut folded);
        revisions.push(vec![symbol(&item.mid), symbol(&snapshot.revision(index))]);
        let declared = snapshot
            .schema()
            .and_then(|schema| schema.types.get(&kind))
            .map(|declaration| &declaration.fields);
        for row in &item.rows {
            let Some(field) = declared.and_then(|fields| fields.get(&row.name)) else {
                continue;
            };
            let value = match field.kind {
                FieldKind::Enum(_) => label_name(&row.value),
                FieldKind::Text => row.value.clone(),
            };
            fold_once(&value, &mut folded);
            fields.push(vec![
                symbol(&item.mid),
                symbol(&row.name),
                Value::Symbol(value),
            ]);
        }
        for edge in snapshot.outgoing(index) {
            let EdgeSource::Row(row) = edge.source else {
                continue;
            };
            let row = &item.rows[row];
            let target = snapshot.link(edge).fragment.as_deref().unwrap_or_default();
            edges.push(vec![
                symbol(&row.name),
                symbol(&item.mid),
                symbol(target),
                symbol(path),
                line(row.line),
            ]);
        }
    }
    let rows = [items, titles, revisions, fields, folded, edges];
    let relations = SNAPSHOT_RELATIONS
        .iter()
        .zip(rows)
        .map(|(name, rows)| Relation::new(name, rows))
        .collect();
    Facts { relations }
}
