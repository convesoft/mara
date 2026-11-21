//! `item delete` (REQ-DELETE-ITEM).

use super::{
    EditError, TARGETED_ITEM, Written, WrittenFile, document_text, read_back, resolve_item,
};
use crate::code::CodeLinks;
use crate::diagnostic::Diagnostic;
use crate::read::ItemSummary;
use crate::snapshot::Snapshot;
use crate::validation;
use crate::work::WorkCounts;
use crate::write::{self, ReadBack, label};

/// Removes the item that `address` names, with one separator next to it (DES-ITEM-WRITER),
/// unless a relation or mention in another item or in narrative targets its MID, or a code
/// marker of `code` names its ID (REQ-DELETE-ITEM, DES-CODE-READERS). The refusal has a
/// diagnostic at each of them. Its own links to itself do not count.
pub fn delete(
    snapshot: &Snapshot,
    code: Option<&CodeLinks>,
    address: &str,
    work: &mut WorkCounts,
) -> Result<Written, EditError> {
    let index = resolve_item(snapshot, address)?;
    let summary = ItemSummary::of(snapshot, index);
    let name = summary.id.clone().unwrap_or_else(|| summary.mid.clone());
    let mut places: Vec<Diagnostic> = Vec::new();
    for edge in snapshot.incoming(&summary.mid) {
        if edge.item == Some(index) {
            continue;
        }
        let path = &snapshot.documents()[edge.document].path;
        let line = snapshot.link(edge).line;
        let what = match snapshot.relation(edge) {
            Some(relation) => format!("this `{}` relation", label(relation)),
            None => "this mention".to_owned(),
        };
        let message = format!("{what} targets {name}, which `item delete` would remove");
        places.push(Diagnostic::error(
            TARGETED_ITEM,
            path.clone(),
            line,
            message,
        ));
    }
    // Every marker that names one of its IDs, whatever its relation (REQ-DELETE-ITEM).
    if let Some(code) = code {
        let parsed = snapshot.item(index);
        let mut ids: Vec<&str> = parsed
            .rows
            .iter()
            .filter(|row| row.name == "id")
            .map(|row| row.value.as_str())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            for (path, line) in code.naming(id) {
                let message =
                    format!("this code marker names {id}, which `item delete` would remove");
                places.push(Diagnostic::error(TARGETED_ITEM, path, line, message));
            }
        }
    }
    if !places.is_empty() {
        let listed = places
            .iter()
            .map(|place| format!("{}:{}", place.path, place.line));
        let count = places.len();
        return Err(EditError {
            code: TARGETED_ITEM,
            message: format!(
                "{name} is the target of {count} relations, mentions or code markers outside \
                 it, at {}: remove or retarget them first",
                validation::list(listed, count)
            ),
            diagnostics: places,
        });
    }
    let document = &snapshot.documents()[snapshot.items()[index].document];
    let text = document_text(snapshot, snapshot.items()[index].document)?;
    let item = snapshot.item(index);
    let splice = write::remove(text, item.span.clone());
    let new = read_back(
        &mut ReadBack::knowing(snapshot),
        &document.path,
        &document.file,
        &[splice],
        work,
    )?;
    Ok(Written {
        files: vec![WrittenFile {
            path: document.path.clone(),
            contents: new.contents,
            document: new.document,
            created: false,
        }],
        item: summary,
        relation: None,
    })
}
