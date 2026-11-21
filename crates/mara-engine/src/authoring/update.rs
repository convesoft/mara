//! `item update` (REQ-UPDATE-ITEM).

use super::{
    EditError, Written, WrittenFile, body, body_text, document_text, field_values, heading,
    is_relation, kind, own_item, problems_of, read_back, resolve_item, schema, summary,
};
use crate::document::label_name;
use crate::schema::{Schema, TypeDecl};
use crate::snapshot::Snapshot;
use crate::work::WorkCounts;
use crate::write::{self, Expect, ReadBack, Splice};

/// A request to update an item: each part given replaces that part.
#[derive(Clone, Copy, Debug)]
pub struct Update<'r> {
    /// The item: an ID, a MID or `<path>:<line>`.
    pub item: &'r str,
    pub title: Option<&'r str>,
    /// `<key>=<value>` pairs, `Status` among them. The values of a key replace all its rows.
    pub fields: &'r [(String, String)],
    /// Names of optional fields whose rows all go.
    pub clear: &'r [String],
    pub body: Option<&'r str>,
}

/// Replaces the title, the rows of each field given and the body of an item, and removes the rows
/// of each field cleared, and nothing else (REQ-UPDATE-ITEM): its anchor, its `Type` and `ID`
/// rows, its relations and every other row stay as they are.
pub fn update(
    snapshot: &Snapshot,
    request: &Update<'_>,
    work: &mut WorkCounts,
) -> Result<Written, EditError> {
    if request.title.is_none()
        && request.fields.is_empty()
        && request.clear.is_empty()
        && request.body.is_none()
    {
        return Err(EditError::new(
            "invalid-argument",
            "an update names a new title, field values, fields to clear or body".to_owned(),
        ));
    }
    let schema = schema(snapshot)?;
    let index = resolve_item(snapshot, request.item)?;
    let document = &snapshot.documents()[snapshot.items()[index].document];
    let (path, file) = (document.path.as_str(), &*document.file);
    let item = snapshot.item(index);
    let text = document_text(snapshot, snapshot.items()[index].document)?;
    let mut splices: Vec<Splice> = Vec::new();
    let mut table_end = item.table.end;

    if let Some(title) = request.title {
        let heading = heading(file, item).ok_or_else(|| {
            EditError::invalid(format!("{path}: the heading of the item cannot be found"))
        })?;
        let written = write::heading(heading.level, title)
            .map_err(|why| EditError::invalid(format!("the title {why}")))?;
        let text = match heading.atx {
            true => written[heading.level + 1..].to_owned(),
            false => title.to_owned(),
        };
        splices.push(Splice {
            range: heading.text,
            text,
            expect: Expect::Bytes,
        });
    }

    if !request.fields.is_empty() || !request.clear.is_empty() {
        let kind = kind(item);
        let declared = schema.types.get(&kind).ok_or_else(|| {
            EditError::invalid(format!(
                "the schema declares no type `{kind}`, the type of this item, so Mara cannot \
                 check the values of its fields"
            ))
        })?;
        let groups = field_values(schema, &kind, declared, request.fields)?;
        let cleared = cleared(schema, &kind, declared, request.clear, &groups)?;
        let mut removed: Vec<usize> = item
            .rows
            .iter()
            .filter(|row| cleared.contains(&row.name))
            .map(|row| row.span.start)
            .collect();
        let mut added = Vec::new();
        for (name, values) in &groups {
            let rows: Vec<_> = item.rows.iter().filter(|row| row.name == *name).collect();
            for (row, value) in rows.iter().zip(values) {
                splices.push(write::set_value(row, value));
            }
            added.extend(values.iter().skip(rows.len()).map(|value| (name, value)));
            removed.extend(rows.iter().skip(values.len()).map(|row| row.span.start));
        }
        // New rows and a new body go after the last row that stays, which is the table's end
        // unless the update removes the last rows.
        let kept = item
            .rows
            .iter()
            .rev()
            .find(|row| !removed.contains(&row.span.start))
            .expect("an item keeps its `Type` row");
        table_end = kept.span.end;
        for (name, value) in added {
            let mut splice = write::add_row(text, item, name, value);
            if table_end != item.table.end {
                let rest = &text[table_end..];
                let eol = &rest[rest.find(['\n', '\r']).unwrap_or(0)..];
                let eol = match eol.as_bytes() {
                    [b'\r', b'\n', ..] => "\r\n",
                    [b'\r', ..] => "\r",
                    _ => "\n",
                };
                splice.range = table_end..table_end;
                splice.text = format!("{eol}{}", write::row(name, value));
            }
            splices.push(splice);
        }
        for row in item
            .rows
            .iter()
            .filter(|row| removed.contains(&row.span.start))
        {
            // Only a header row without a row after it has no removal, and the `Type` row stays.
            let removal = write::remove_row(text, item, row)
                .expect("a row that goes is never the only row of the table");
            splices.push(removal);
        }
    }

    let mut body_at = None;
    if let Some(given) = request.body {
        let new_body = body_text(body(given)?, write::line_ending(text));
        let eol = write::line_ending(text);
        let splice = match item.body.is_empty() {
            true => Splice {
                range: table_end..table_end,
                text: format!("{eol}{eol}{new_body}"),
                expect: Expect::Bytes,
            },
            false => Splice {
                range: item.body.clone(),
                text: new_body,
                expect: Expect::Bytes,
            },
        };
        body_at = Some(splices.len());
        splices.push(splice);
    }

    // Stable, so rows added at the table's end come before a body inserted there.
    let mut order: Vec<usize> = (0..splices.len()).collect();
    order.sort_by_key(|&at| splices[at].range.start);
    let body_at = body_at.map(|at| order.iter().position(|&other| other == at).unwrap());
    let splices: Vec<Splice> = order.into_iter().map(|at| splices[at].clone()).collect();
    if splices
        .windows(2)
        .any(|pair| pair[0].range.end > pair[1].range.start)
    {
        return Err(EditError::invalid(format!(
            "{path}: the changes of this update overlap in the item's table; make them one at a \
             time"
        )));
    }
    let mut written = item.span.start..item.span.start;
    if let Some(at) = body_at {
        let shift: isize = splices[..at]
            .iter()
            .map(|splice| splice.text.len() as isize - splice.range.len() as isize)
            .sum();
        let start = splices[at].range.start.saturating_add_signed(shift);
        written = start..start + splices[at].text.len();
    }
    let old_problems = problems_of(path, file, item);
    let new = read_back(&mut ReadBack::knowing(snapshot), path, file, &splices, work)?;
    // Every change comes after the anchor, which stays where it is.
    let found = own_item(path, &new, item.span.start, written, old_problems)?;
    let files = match *new.contents == *file.contents {
        true => Vec::new(),
        false => vec![WrittenFile {
            path: path.to_owned(),
            contents: new.contents.clone(),
            document: new.document.clone(),
            created: false,
        }],
    };
    Ok(Written {
        item: summary(path, &new.document.items[found]),
        files,
        relation: None,
    })
}

/// The schema names of the fields that `clear` names for an item of type `kind`, declared as
/// `declared`. Refuses `Type`, `ID` and `Status`, which every item has, a required field, a
/// relation, a key the type does not declare, and a field that `fields` also sets.
fn cleared(
    schema: &Schema,
    kind: &str,
    declared: &TypeDecl,
    clear: &[String],
    fields: &[(String, Vec<String>)],
) -> Result<Vec<String>, EditError> {
    let mut names = Vec::with_capacity(clear.len());
    for given in clear {
        let name = label_name(given.trim());
        let shown = write::label(&name);
        if matches!(name.as_str(), "type" | "id" | "status") {
            return Err(EditError::invalid(format!(
                "`{}` cannot be cleared: every item has one",
                match name.as_str() {
                    "id" => "ID",
                    _ => &shown,
                }
            )));
        }
        if fields.iter().any(|(set, _)| *set == name) {
            return Err(EditError::new(
                "invalid-argument",
                format!(
                    "`{shown}` is both set and cleared: give one of `--field` and `--clear-field`"
                ),
            ));
        }
        match declared.fields.get(&name) {
            Some(field) if field.required => {
                return Err(EditError::invalid(format!(
                    "type `{kind}` requires `{shown}`, so its rows stay: give new values with \
                     `--field`"
                )));
            }
            Some(_) => names.push(name),
            None if is_relation(schema, &name) => {
                return Err(EditError::invalid(format!(
                    "`{shown}` is a relation, which `item update` does not write: remove it with \
                     `mara relation remove`"
                )));
            }
            None => {
                return Err(EditError::invalid(format!(
                    "type `{kind}` declares no field `{}`{}",
                    given.trim(),
                    super::fields_of(declared)
                )));
            }
        }
    }
    Ok(names)
}
