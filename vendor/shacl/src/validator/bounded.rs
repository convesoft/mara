//! Mara's opt-in, thread-local native evaluator controls (patch revision 1).
//! The host calls a single focus shape synchronously; state is restored on unwind.
use crate::{error::ValidationError, ir::IRComponent, validator::nodes::ValueNodes};
use rudof_rdf::rdf_core::{Rdf, term::Term};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

#[derive(Default, Clone)]
pub struct Control {
    pub used: usize,
    pub limit: usize,
    pub exhausted: bool,
    pub unavailable: BTreeSet<String>,
    pub invalid_paths: BTreeSet<String>,
    pub counts: BTreeMap<(String, String), (usize, usize)>,
    pub order: BTreeMap<(String, String), usize>,
    pub patterns: BTreeMap<String, usize>,
    pub path_work: BTreeMap<(String, String, bool), usize>,
    pub explanations:
        BTreeMap<(String, String, String), Vec<crate::validator::report::ValidationResult>>,
    pub events: Vec<(String, String, String)>,
}
thread_local! { static CONTROL: RefCell<Option<Control>> = const { RefCell::new(None) }; }
pub fn active() -> bool {
    CONTROL.with(|c| c.borrow().is_some())
}
pub fn run<T>(control: Control, operation: impl FnOnce() -> T) -> (T, Control) {
    struct Restore(Option<Control>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CONTROL.with(|c| *c.borrow_mut() = self.0.take());
        }
    }
    let restore = Restore(CONTROL.with(|c| c.replace(Some(control))));
    let value = operation();
    let control = CONTROL.with(|c| c.borrow_mut().take().unwrap());
    drop(restore);
    (value, control)
}
pub fn charge(units: usize) -> Result<(), ValidationError> {
    CONTROL.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(c) = c.as_mut() {
            if c.exhausted || units > c.limit.saturating_sub(c.used) {
                c.exhausted = true;
                return Err(ValidationError::WorkLimit);
            }
            c.used += units;
        }
        Ok(())
    })
}
pub fn visit(node: &str) -> Result<(), ValidationError> {
    let node = node.trim_start_matches('<').trim_end_matches('>');
    charge(1 + node.len())?;
    if CONTROL.with(|c| {
        c.borrow()
            .as_ref()
            .is_some_and(|c| c.unavailable.contains(node))
    }) {
        return Err(ValidationError::Unavailable(node.into()));
    }
    Ok(())
}
pub fn order(shape: &str, component: &str) -> usize {
    CONTROL.with(|c| {
        c.borrow()
            .as_ref()
            .and_then(|c| c.order.get(&(shape.into(), component.into())).copied())
            .unwrap_or(usize::MAX)
    })
}
pub fn component<R: Rdf>(
    shape: &str,
    component: &IRComponent,
    values: &ValueNodes<R>,
) -> Result<(), ValidationError> {
    if !active() {
        return Ok(());
    }
    charge(1)?;
    // Reserve primitive comparisons and their complete operands before invoking the
    // native component. Nested shape invocations charge themselves recursively.
    for (focus, nodes) in values.iter() {
        charge(1 + focus.to_string().len() + nodes.len())?;
        let focus_id = focus.to_string();
        let focus_id = focus_id.trim_start_matches('<').trim_end_matches('>');
        CONTROL.with(|c| {
            if let Some(c) = c.borrow_mut().as_mut() {
                c.counts
                    .entry((shape.into(), focus_id.into()))
                    .or_insert((nodes.len(), nodes.len()));
            }
        });
        for node in nodes.iter() {
            let text = node.lexical_form();
            charge(1 + text.len())?;
            match component {
                IRComponent::In(c) => {
                    for v in c.values() {
                        charge(1 + text.len() + v.to_string().len())?;
                    }
                }
                IRComponent::HasValue(c) => charge(1 + text.len() + c.value().to_string().len())?,
                IRComponent::Pattern(c) => {
                    let states = CONTROL
                        .with(|control| {
                            control
                                .borrow()
                                .as_ref()
                                .and_then(|control| control.patterns.get(c.pattern()).copied())
                        })
                        .ok_or_else(|| {
                            ValidationError::Unavailable("pattern bound is unavailable".into())
                        })?;
                    charge(states.saturating_mul(text.len().saturating_add(1)))?;
                }
                IRComponent::Class(c) => charge(1 + text.len() + c.class_rule().to_string().len())?,
                IRComponent::Datatype(c) => charge(1 + text.len() + c.datatype().as_str().len())?,
                _ => {}
            }
        }
    }
    Ok(())
}

/// Compilation belongs to rule loading, outside logical evaluation. Matching
/// reserves the finite Thompson program size times the input length.
pub fn pattern_bound(pattern: &str) -> Result<usize, String> {
    regex_automata::nfa::thompson::NFA::compiler()
        .configure(regex_automata::nfa::thompson::NFA::config().nfa_size_limit(Some(1_000_000)))
        .build(pattern)
        .map(|nfa| nfa.states().len())
        .map_err(|e| e.to_string())
}

pub fn path(focus: &str, path: &rudof_rdf::rdf_core::SHACLPath) -> Result<(), ValidationError> {
    if !active() {
        return Ok(());
    }
    use rudof_rdf::rdf_core::SHACLPath;
    let (predicate, inverse) = match path {
        SHACLPath::Predicate { pred } => (pred.as_str(), false),
        SHACLPath::Inverse { path } => match path.as_ref() {
            SHACLPath::Predicate { pred } => (pred.as_str(), true),
            _ => {
                return Err(ValidationError::Unavailable(
                    "unsupported bounded path".into(),
                ));
            }
        },
        _ => {
            return Err(ValidationError::Unavailable(
                "unsupported bounded path".into(),
            ));
        }
    };
    let focus = focus.trim_start_matches('<').trim_end_matches('>');
    let (invalid, cost) = CONTROL.with(|c| {
        c.borrow()
            .as_ref()
            .map(|c| {
                (
                    c.invalid_paths.contains(predicate),
                    c.path_work
                        .get(&(focus.into(), predicate.into(), inverse))
                        .copied()
                        .unwrap_or(0),
                )
            })
            .unwrap_or((false, 0))
    });
    charge(1 + predicate.len() + focus.len() + cost)?;
    if invalid {
        return Err(ValidationError::Unavailable(
            "invalid relationship prerequisite".into(),
        ));
    }
    Ok(())
}

pub fn qualified(shape: &str, node: &str, selected: usize, qualifying: usize) {
    let node = node.trim_start_matches('<').trim_end_matches('>');
    CONTROL.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            c.counts
                .insert((shape.into(), node.into()), (selected, qualifying));
        }
    });
}

pub fn explain(
    shape: &str,
    focus: &str,
    component: &str,
    outcome: &crate::validator::report::ValidationOutcome,
) {
    let focus = focus.trim_start_matches('<').trim_end_matches('>');
    CONTROL.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            c.explanations
                .entry((shape.into(), focus.into(), component.into()))
                .or_default()
                .extend_from_slice(outcome.violations());
        }
    });
}
pub fn record(outcome: &crate::validator::report::ValidationOutcome) {
    CONTROL.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            let mut violations = outcome.violations().iter().collect::<Vec<_>>();
            violations.sort_by_key(|v| {
                c.order
                    .get(&(
                        v.source().map(ToString::to_string).unwrap_or_default(),
                        v.constraint_component().to_string(),
                    ))
                    .copied()
                    .unwrap_or(usize::MAX)
            });
            for v in violations {
                c.events.push((
                    v.source().map(ToString::to_string).unwrap_or_default(),
                    v.focus_node().to_string(),
                    v.constraint_component().to_string(),
                ));
            }
        }
    });
}
pub fn reset_trace() {
    CONTROL.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            c.events.clear();
            c.explanations.clear();
            c.counts.clear();
        }
    });
}
