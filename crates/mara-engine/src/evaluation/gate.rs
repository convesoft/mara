//! The outcome of a gate for the items a request names (DES-GATE-OUTCOME in
//! `docs/rules.mara.md`).

use crate::config::{Consumer, Entry};
use crate::facts::Value;
use crate::snapshot::Snapshot;

use super::Rows;

/// The outcome of a gate, or of one item it judges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Failed,
    Unavailable,
    /// No subject row: the gate cannot judge the item. Never the outcome of a whole gate.
    NotApplicable,
}

impl Outcome {
    /// Its name in a result.
    pub fn name(self) -> &'static str {
        match self {
            Outcome::Passed => "passed",
            Outcome::Failed => "failed",
            Outcome::Unavailable => "unavailable",
            Outcome::NotApplicable => "not-applicable",
        }
    }
}

/// What a relation of a gate says of an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Subject,
    Condition,
    Unavailable,
}

impl Role {
    /// Its name in a result.
    pub fn name(self) -> &'static str {
        match self {
            Role::Subject => "subject",
            Role::Condition => "condition",
            Role::Unavailable => "unavailable",
        }
    }
}

/// A row of one of the gate's relations about an item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateRow {
    pub relation: String,
    pub role: Role,
    pub values: Vec<Value>,
}

/// The outcome of one named item, with the rows that caused it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemOutcome {
    /// The item's index in the snapshot.
    pub item: usize,
    pub outcome: Outcome,
    pub rows: Vec<GateRow>,
}

/// The outcome of a gate request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateOutcome {
    /// Passed when every item passed; failed when one failed or is not applicable; unavailable
    /// otherwise.
    pub outcome: Outcome,
    /// In the order the request names them.
    pub items: Vec<ItemOutcome>,
}

/// Judges each of `items` by the rows of the gate `entry`'s relations whose first column is the
/// item's MID: none in a subject relation, not applicable; any in a condition relation, failed;
/// otherwise any in an unavailable relation, unavailable; otherwise passed. Each item lists the
/// rows that caused its outcome: none, the condition rows, the unavailable rows or the subject
/// rows. `rows` is `None` when the evaluation has no rows, a fault: then the gate and every item
/// are unavailable, never passed (REQ-PROGRAM-FAULTS-REPORTED). Not a gate: every item
/// unavailable.
pub fn judge(
    entry: &Entry,
    rows: Option<&Rows>,
    items: &[usize],
    snapshot: &Snapshot,
) -> GateOutcome {
    let judged = items.iter().map(|&item| {
        let unavailable = ItemOutcome {
            item,
            outcome: Outcome::Unavailable,
            rows: Vec::new(),
        };
        let (
            Some(rows),
            Consumer::Gate {
                subjects,
                conditions,
                unavailable: missing,
            },
        ) = (rows, &entry.consumer)
        else {
            return unavailable;
        };
        let mid = &snapshot.item(item).mid;
        let about = |relations: &[crate::config::Named], role: Role| -> Vec<GateRow> {
            let mut found = Vec::new();
            for relation in relations {
                let all = rows
                    .relations
                    .get(&relation.name)
                    .map_or(&[][..], Vec::as_slice);
                let mut own: Vec<&Vec<Value>> = all
                    .iter()
                    .filter(|row| matches!(row.first(), Some(Value::Symbol(first)) if first == mid))
                    .collect();
                own.sort_by(|a, b| super::compare_rows(a, b));
                found.extend(own.into_iter().map(|values| GateRow {
                    relation: relation.name.clone(),
                    role,
                    values: values.clone(),
                }));
            }
            found
        };
        let subject = about(subjects, Role::Subject);
        let condition = about(conditions, Role::Condition);
        let missing = about(missing, Role::Unavailable);
        let (outcome, rows) = if subject.is_empty() {
            (Outcome::NotApplicable, Vec::new())
        } else if !condition.is_empty() {
            (Outcome::Failed, condition)
        } else if !missing.is_empty() {
            (Outcome::Unavailable, missing)
        } else {
            (Outcome::Passed, subject)
        };
        ItemOutcome {
            item,
            outcome,
            rows,
        }
    });
    let items: Vec<ItemOutcome> = judged.collect();
    let outcome = if items.iter().all(|item| item.outcome == Outcome::Passed) {
        Outcome::Passed
    } else if items
        .iter()
        .any(|item| matches!(item.outcome, Outcome::Failed | Outcome::NotApplicable))
    {
        Outcome::Failed
    } else {
        Outcome::Unavailable
    };
    GateOutcome { outcome, items }
}
