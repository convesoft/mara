//! `relation add` and `relation remove` (REQ-EDIT-RELATIONS).

use super::{
    EditError, INVERSE_RELATION, MISSING_RELATION, RelationRow, Written, WrittenFile, describe,
    document_text, kind, read_back, resolve_item, schema, summary,
};
use crate::document::{Item, label_name};
use crate::read::ItemSummary;
use crate::schema::Schema;
use crate::snapshot::{ParsedFile, Snapshot};
use crate::validation;
use crate::work::WorkCounts;
use crate::write::{self, ReadBack, Splice, label};

/// Adds the row of relation `relation` from the item `source` to the item `target`: the
/// relation's label and a canonical link (DES-RELATION-LINKS), after the last row of the
/// source's table. Refused for an inverse name, a relation that the schema does not lead from
/// the source's type to the target's, and a row that is already there.
pub fn relation_add(
    snapshot: &Snapshot,
    source: &str,
    relation: &str,
    target: &str,
    work: &mut WorkCounts,
) -> Result<Written, EditError> {
    let schema = schema(snapshot)?;
    let name = forward(schema, relation, ["add", source, target])?;
    let (from, to) = (
        resolve_item(snapshot, source)?,
        resolve_item(snapshot, target)?,
    );
    let (source_item, target_item) = (snapshot.item(from), snapshot.item(to));
    let (source_kind, target_kind) = (kind(source_item), kind(target_item));
    let declaration = &schema.relations[&name];
    let Some(allowed) = declaration.targets.get(&source_kind) else {
        let sources: Vec<String> = declaration
            .targets
            .keys()
            .map(|name| format!("`{name}`"))
            .collect();
        let count = sources.len();
        return Err(EditError::invalid(format!(
            "relation `{name}` does not lead from type `{source_kind}`, the type of {}: it leads \
             from {}",
            describe(snapshot, from),
            validation::list(sources, count)
        )));
    };
    if !allowed.contains(&target_kind) {
        let targets: Vec<String> = allowed.iter().map(|name| format!("`{name}`")).collect();
        let count = targets.len();
        return Err(EditError::invalid(format!(
            "relation `{name}` leads from type `{source_kind}` only to {}, and {} has type \
             `{target_kind}`",
            validation::list(targets, count),
            describe(snapshot, to)
        )));
    }
    let target_mid = &target_item.mid;
    if let Some(row) = rows_to(source_item, &name, target_mid).next() {
        return Err(EditError::invalid(format!(
            "{} already has this relation, at line {}",
            describe(snapshot, from),
            row.line
        )));
    }
    let Some(id) = target_item.value("id") else {
        return Err(EditError::invalid(format!(
            "{} has no `ID` row, and a relation link's text is the target's ID: give it one first",
            describe(snapshot, to)
        )));
    };
    let document = &snapshot.documents()[snapshot.items()[from].document];
    let target_path = &snapshot.documents()[snapshot.items()[to].document].path;
    let text = document_text(snapshot, snapshot.items()[from].document)?;
    let link = write::link(id, target_mid, &document.path, target_path);
    let splice = write::add_row(text, source_item, &name, &link);
    let splices = [splice];
    let new = read_back(
        &mut ReadBack::default(),
        &document.path,
        &document.file,
        &splices,
        work,
    )?;
    let new_source = after(&new, source_item, &splices).expect("the read-back keeps the source");
    let line = rows_to(new_source, &name, target_mid)
        .last()
        .expect("the read-back keeps the added row")
        .line;
    let target = summary_after(snapshot, to, &document.path, &new, &splices);
    Ok(Written {
        item: summary(&document.path, new_source),
        relation: Some(RelationRow { name, target, line }),
        files: vec![WrittenFile {
            path: document.path.clone(),
            contents: new.contents,
            created: false,
        }],
    })
}

/// Removes each row of relation `relation` on the item `source` that links to the item `target`
/// (REQ-EDIT-RELATIONS). Refused for an inverse name and when there is none.
pub fn relation_remove(
    snapshot: &Snapshot,
    source: &str,
    relation: &str,
    target: &str,
    work: &mut WorkCounts,
) -> Result<Written, EditError> {
    let schema = schema(snapshot)?;
    let name = forward(schema, relation, ["remove", source, target])?;
    let (from, to) = (
        resolve_item(snapshot, source)?,
        resolve_item(snapshot, target)?,
    );
    let (source_item, target_item) = (snapshot.item(from), snapshot.item(to));
    // A row of a type that is no source of the relation is a field or an undeclared key, never
    // this relation (DES-SNAPSHOT).
    let source_kind = kind(source_item);
    if schema.relation_from(&source_kind, &name).is_none() {
        return Err(EditError::invalid(format!(
            "relation `{name}` does not lead from type `{source_kind}`, the type of {}, so it has \
             no such relation to remove",
            describe(snapshot, from)
        )));
    }
    let rows: Vec<_> = rows_to(source_item, &name, &target_item.mid).collect();
    let Some(first) = rows.first() else {
        return Err(EditError::new(
            MISSING_RELATION,
            format!(
                "{} has no `{}` row that links to {}",
                describe(snapshot, from),
                label(&name),
                describe(snapshot, to)
            ),
        ));
    };
    let line = first.line;
    let document = &snapshot.documents()[snapshot.items()[from].document];
    let text = document_text(snapshot, snapshot.items()[from].document)?;
    let mut splices = Vec::with_capacity(rows.len());
    for row in &rows {
        // An item's table always holds its `Type` row besides a relation row.
        let removal = write::remove_row(text, source_item, row).ok_or_else(|| {
            EditError::invalid(format!(
                "{}: the row at line {} is the table's only row",
                document.path, row.line
            ))
        })?;
        splices.push(removal);
    }
    splices.sort_by_key(|splice| splice.range.start);
    if splices
        .windows(2)
        .any(|pair| pair[0].range.end > pair[1].range.start)
    {
        return Err(EditError::invalid(format!(
            "{}: the item repeats this row next to its header row; remove the repeated rows by \
             hand",
            document.path
        )));
    }
    let new = read_back(
        &mut ReadBack::default(),
        &document.path,
        &document.file,
        &splices,
        work,
    )?;
    let new_source = after(&new, source_item, &splices).expect("the read-back keeps the source");
    let target = summary_after(snapshot, to, &document.path, &new, &splices);
    Ok(Written {
        item: summary(&document.path, new_source),
        relation: Some(RelationRow { name, target, line }),
        files: vec![WrittenFile {
            path: document.path.clone(),
            contents: new.contents,
            created: false,
        }],
    })
}

/// The forward name of relation `relation`, matched as a label. An inverse name is refused with
/// the forward request (REQ-EDIT-RELATIONS): `request` is the verb, the source and the target.
fn forward(schema: &Schema, relation: &str, request: [&str; 3]) -> Result<String, EditError> {
    let name = label_name(relation.trim());
    if schema.relations.contains_key(&name) {
        return Ok(name);
    }
    let [verb, source, target] = request;
    if let Some((forward, _)) = schema
        .relations
        .iter()
        .find(|(_, declaration)| declaration.inverse.as_deref() == Some(name.as_str()))
    {
        return Err(EditError::new(
            INVERSE_RELATION,
            format!(
                "`{relation}` is the inverse name of `{forward}`, and `relation {verb}` takes only \
                 forward names: run `mara relation {verb} {target} {forward} {source}`"
            ),
        ));
    }
    let names: Vec<String> = schema
        .relations
        .keys()
        .map(|name| format!("`{name}`"))
        .collect();
    let count = names.len();
    Err(EditError::new(
        "unknown-declaration",
        format!(
            "the schema declares no relation `{relation}`: {}",
            validation::list(names, count)
        ),
    ))
}

/// The rows of `item` of relation `name` whose link targets `mid`.
fn rows_to<'i>(
    item: &'i Item,
    name: &'i str,
    mid: &'i str,
) -> impl Iterator<Item = &'i crate::document::Row> + 'i {
    item.rows.iter().filter(move |row| {
        row.name == name
            && row
                .link
                .as_ref()
                .is_some_and(|link| link.fragment.as_deref() == Some(mid))
    })
}

/// Where the old offset `offset`, which no splice replaces, is after `splices`.
fn moved(offset: usize, splices: &[Splice]) -> usize {
    let shift: isize = splices
        .iter()
        .filter(|splice| splice.range.end <= offset)
        .map(|splice| splice.text.len() as isize - splice.range.len() as isize)
        .sum();
    offset.saturating_add_signed(shift)
}

/// The item of `new` that `item` of the old document became after `splices`.
fn after<'n>(new: &'n ParsedFile, item: &Item, splices: &[Splice]) -> Option<&'n Item> {
    let at = moved(item.span.start, splices);
    let items = &new.document.items;
    items
        .get(items.partition_point(|other| other.span.start < at))
        .filter(|other| other.span.start == at && other.mid == item.mid)
}

/// What a result says of the item at `index` of `snapshot` once the document at `path` holds
/// `new`, written with `splices`.
fn summary_after(
    snapshot: &Snapshot,
    index: usize,
    path: &str,
    new: &ParsedFile,
    splices: &[Splice],
) -> ItemSummary {
    let entry = &snapshot.items()[index];
    if snapshot.documents()[entry.document].path != path {
        return ItemSummary::of(snapshot, index);
    }
    match after(new, snapshot.item(index), splices) {
        Some(item) => summary(path, item),
        None => ItemSummary::of(snapshot, index),
    }
}
