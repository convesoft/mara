//! Structural relation policies over canonical semantic edges.
use crate::{
    Corpus, Diagnostic, DiagnosticCode, DiagnosticItem, DiagnosticLocation, Item, Project,
    RelationEdge, RelationEndpoint, Schema, Severity, ValidationDiagnostic, ValidationResult,
    ValidationScope,
};
use petgraph::{algo::kosaraju_scc, graph::DiGraph};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

struct EdgeRecord {
    edge: RelationEdge,
    source: DiagnosticLocation,
}

pub(crate) fn evaluate(
    project: &Project,
    corpus: &Corpus,
    schema: &Schema,
    prerequisites: &[Diagnostic],
    result: &mut ValidationResult,
) {
    if !schema
        .relations()
        .values()
        .any(|r| r.cardinality.is_some() || r.acyclic.is_some())
    {
        return;
    }
    if !prerequisites.is_empty()
        || !corpus.is_complete()
        || corpus
            .items()
            .any(|item| !item.validation_source_is_complete())
    {
        result.evaluation_complete = false;
        if !result.diagnostics.iter().any(|d| {
            d.code == DiagnosticCode::EvaluationUnavailable && d.scope == ValidationScope::Project
        }) {
            result.diagnostics.push(ValidationDiagnostic::new(
                DiagnosticCode::EvaluationUnavailable,
                Severity::Error,
                ValidationScope::Project,
                DiagnosticLocation::default(),
                "relationship policy evaluation skipped: fix corpus source, identity, field and reference diagnostics first",
            ));
        }
        return;
    }

    let mut items = BTreeMap::new();
    for item in corpus.items() {
        items.insert(item.id(), item);
        items.insert(item.mid().expect("validated MID"), item);
    }
    let mut edges: BTreeMap<(String, String, String), EdgeRecord> = BTreeMap::new();
    for item in corpus.items() {
        for relation in item.relations() {
            let edge = if let Some(address) = crate::external::address(relation.target()) {
                RelationEdge::external(schema, item, relation.name(), address)
            } else {
                let target = items[relation.target()];
                RelationEdge::new(schema, item, relation.name(), target)
            }
            .expect("validated relation");
            let RelationEndpoint::Item { mid: source, .. } = &edge.source else {
                unreachable!()
            };
            let target = match &edge.target {
                RelationEndpoint::Item { mid, .. } => format!("item:{mid}"),
                RelationEndpoint::External { address } => format!("external:{address}"),
            };
            edges
                .entry((edge.relation.clone(), source.clone(), target))
                .or_insert_with(|| EdgeRecord {
                    edge,
                    source: DiagnosticLocation::source(relation.source()),
                });
        }
    }

    for (name, declaration) in schema.relations() {
        let relevant = edges
            .values()
            .filter(|record| record.edge.relation == *name)
            .collect::<Vec<_>>();
        if let Some(cardinality) = &declaration.cardinality {
            for (direction, bounds, flavours) in [
                ("outgoing", &cardinality.outgoing, &declaration.source),
                ("incoming", &cardinality.incoming, &declaration.target),
                ("symmetric", &cardinality.symmetric, &declaration.source),
            ] {
                let Some(bounds) = bounds else { continue };
                for item in corpus.items().filter(|item| {
                    flavours.contains(&item.flavour().to_owned())
                        && (!declaration.same_flavour
                            || (declaration.source.contains(&item.flavour().to_owned())
                                && declaration.target.contains(&item.flavour().to_owned())))
                }) {
                    if result.target.id.as_deref().is_some_and(|selected| {
                        selected != item.id() && Some(selected) != item.mid()
                    }) {
                        continue;
                    }
                    let mid = item.mid().expect("validated MID");
                    let incident = relevant
                        .iter()
                        .filter(|record| {
                            let source = matches!(&record.edge.source, RelationEndpoint::Item { mid: m, .. } if m == mid);
                            let target = matches!(&record.edge.target, RelationEndpoint::Item { mid: m, .. } if m == mid);
                            match direction {
                                "outgoing" => source,
                                "incoming" => target,
                                _ => source || target,
                            }
                        })
                        .collect::<Vec<_>>();
                    let count = incident.len();
                    if bounds.minimum.is_some_and(|min| count < min)
                        || bounds.maximum.is_some_and(|max| count > max)
                    {
                        let mut diagnostic = item_diagnostic(
                            DiagnosticCode::RelationCardinality,
                            bounds.severity,
                            item,
                            format!(
                                "relation '{name}' {direction} cardinality is {count}; expected {}",
                                bounds_text(bounds.minimum, bounds.maximum)
                            ),
                        );
                        diagnostic.details = Some(json!({
                            "relation": name,
                            "direction": direction,
                            "minimum": bounds.minimum,
                            "maximum": bounds.maximum,
                            "actual": count,
                            "edges": incident.iter().take(8).map(|record| json!({
                                "edge": record.edge,
                                "source": record.source,
                            })).collect::<Vec<_>>(),
                        }));
                        result.diagnostics.push(diagnostic);
                    }
                }
            }
        }
        if let Some(policy) = &declaration.acyclic {
            cycle_diagnostics(
                project,
                corpus,
                schema,
                name,
                policy.severity,
                &edges,
                result,
            );
        }
    }
}

fn bounds_text(minimum: Option<usize>, maximum: Option<usize>) -> String {
    match (minimum, maximum) {
        (Some(min), Some(max)) => format!("between {min} and {max}"),
        (Some(min), None) => format!("at least {min}"),
        (None, Some(max)) => format!("at most {max}"),
        (None, None) => unreachable!(),
    }
}

fn item_diagnostic(
    code: DiagnosticCode,
    severity: Severity,
    item: &Item,
    message: String,
) -> ValidationDiagnostic {
    let mut diagnostic = ValidationDiagnostic::new(
        code,
        severity,
        ValidationScope::Item,
        DiagnosticLocation::source(item.source()),
        message,
    );
    diagnostic.item = Some(DiagnosticItem {
        id: item.id().into(),
        mid: item.mid().map(str::to_owned),
    });
    diagnostic
}

fn cycle_diagnostics(
    project: &Project,
    corpus: &Corpus,
    schema: &Schema,
    name: &str,
    severity: Severity,
    edges: &BTreeMap<(String, String, String), EdgeRecord>,
    result: &mut ValidationResult,
) {
    let mut adjacency: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (relation, source, target) in edges.keys() {
        if relation != name {
            continue;
        }
        if let Some(mid) = target.strip_prefix("item:") {
            adjacency
                .entry(source.clone())
                .or_default()
                .push(mid.into());
            adjacency.entry(mid.into()).or_default();
        }
    }
    for neighbours in adjacency.values_mut() {
        neighbours.sort();
        neighbours.dedup();
    }
    let mut graph = DiGraph::<String, ()>::new();
    let nodes = adjacency
        .keys()
        .map(|mid| (mid.clone(), graph.add_node(mid.clone())))
        .collect::<BTreeMap<_, _>>();
    for (source, neighbours) in &adjacency {
        for target in neighbours {
            graph.add_edge(nodes[source], nodes[target], ());
        }
    }
    let mut components = kosaraju_scc(&graph)
        .into_iter()
        .map(|component| {
            let mut mids = component
                .into_iter()
                .map(|index| graph[index].clone())
                .collect::<Vec<_>>();
            mids.sort();
            mids
        })
        .filter(|component| {
            component.len() > 1
                || adjacency[&component[0]]
                    .iter()
                    .any(|mid| mid == &component[0])
        })
        .collect::<Vec<_>>();
    components.sort();
    let by_mid = corpus
        .items()
        .map(|item| (item.mid().expect("validated MID"), item))
        .collect::<BTreeMap<_, _>>();
    for component in components {
        let cycle = first_cycle(&component, &adjacency);
        if result.target.id.as_deref().is_some_and(|selected| {
            !component.iter().any(|mid| {
                let item = by_mid[mid.as_str()];
                selected == item.id() || Some(selected) == item.mid()
            })
        }) {
            continue;
        }
        let focus = result
            .target
            .id
            .as_deref()
            .and_then(|selected| {
                component.iter().find_map(|mid| {
                    let item = by_mid[mid.as_str()];
                    (selected == item.id() || Some(selected) == item.mid()).then_some(item)
                })
            })
            .unwrap_or(by_mid[component[0].as_str()]);
        let mut diagnostic = item_diagnostic(
            DiagnosticCode::RelationCycle,
            severity,
            focus,
            format!(
                "relation '{name}' prohibits a cycle involving {}",
                component
                    .iter()
                    .map(|mid| by_mid[mid.as_str()].id())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
        let witness = cycle
            .windows(2)
            .map(|pair| {
                let record = &edges[&(
                    name.to_owned(),
                    pair[0].clone(),
                    format!("item:{}", pair[1]),
                )];
                let occurrence =
                    crate::relations::occurrences(project, corpus, schema, &record.edge)
                        .expect("validated edge occurrences")
                        .into_iter()
                        .next()
                        .expect("edge has authored occurrence");
                json!({
                    "edge": record.edge,
                    "source": occurrence.source,
                    "reference": occurrence.reference,
                })
            })
            .collect::<Vec<_>>();
        diagnostic.details = Some(json!({
            "relation": name,
            "members": component.iter().map(|mid| json!({"id":by_mid[mid.as_str()].id(),"mid":mid})).collect::<Vec<_>>(),
            "witness": witness,
        }));
        result.diagnostics.push(diagnostic);
    }
}

fn first_cycle(component: &[String], adjacency: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let members = component.iter().collect::<BTreeSet<_>>();
    let start = component[0].clone();
    let mut path = vec![start.clone()];
    let mut next = vec![0usize];
    let mut seen = BTreeSet::from([start.clone()]);
    while let Some(position) = next.last_mut() {
        let current = path.last().expect("nonempty traversal");
        let neighbours = &adjacency[current];
        if *position == neighbours.len() {
            seen.remove(current);
            path.pop();
            next.pop();
            continue;
        }
        let candidate = neighbours[*position].clone();
        *position += 1;
        if candidate == start {
            path.push(start);
            return path;
        }
        if members.contains(&candidate) && seen.insert(candidate.clone()) {
            path.push(candidate);
            next.push(0);
        }
    }
    unreachable!("cyclic component has a cycle through its smallest member")
}
