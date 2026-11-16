//! The rows of an invariant as diagnostics (DES-RULE-CONSUMERS in `docs/rules.mara.md`).

use crate::config::{CONFIG_PATH, Consumer, Entry};
use crate::diagnostic::{Diagnostic, intern};
use crate::facts::Value;
use crate::snapshot::Snapshot;

/// One failure of an invariant, with the snapshot index of the item it is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvariantFinding {
    pub item: Option<usize>,
    pub diagnostic: Diagnostic,
}

/// One diagnostic per row of the invariant `entry`'s relation, of its severity, with its name
/// as code and its message, at the path, line, ID and MID of the item whose MID the row's first
/// column holds. A row about a MID that no item has is reported at the configuration line that
/// names the relation. Not an invariant: none.
pub fn invariant_findings(
    entry: &Entry,
    rows: &[Vec<Value>],
    snapshot: &Snapshot,
) -> Vec<InvariantFinding> {
    let Consumer::Invariant {
        relation,
        severity,
        message,
    } = &entry.consumer
    else {
        return Vec::new();
    };
    let code = intern(&entry.name);
    rows.iter()
        .map(|row| {
            let mid = match row.first() {
                Some(Value::Symbol(mid)) => mid.as_str(),
                _ => "",
            };
            let item = snapshot.items_with_mid(mid).first().copied();
            let mut diagnostic = match item {
                Some(index) => {
                    let document = snapshot.items()[index].document;
                    let path = &snapshot.documents()[document].path;
                    let found = snapshot.item(index);
                    let mut diagnostic =
                        Diagnostic::error(code, path.clone(), found.line, message.clone());
                    diagnostic.id = found.value("id").map(str::to_owned);
                    diagnostic.mid = Some(found.mid.clone());
                    diagnostic
                }
                None => {
                    let message = format!(
                        "{message} (a row of `{}` about `{}`, which no item has as its MID)",
                        relation.name,
                        crate::validation::quote(mid)
                    );
                    Diagnostic::error(code, CONFIG_PATH, relation.line, message)
                }
            };
            diagnostic.severity = *severity;
            InvariantFinding { item, diagnostic }
        })
        .collect()
}
