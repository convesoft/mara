//! Shared canonical RDF preparation and source support. No host effects.
use crate::{Corpus, FieldType, Item, ItemSource, Schema};
use oxigraph::model::{Literal, NamedNode, Term, Triple};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(crate) fn hash(value: &impl Serialize) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("serializable query input"))
    )
}
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Endpoint {
    Item { id: String, mid: Option<String> },
    Code { reference: String },
    External { address: String },
}
impl Endpoint {
    pub(crate) fn item(item: &Item) -> Self {
        Self::Item {
            id: item.id().into(),
            mid: item.mid().map(str::to_owned),
        }
    }
    pub(crate) fn iri(&self) -> String {
        let encode =
            |s: &str| url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>();
        match self {
            Self::Item { mid: Some(mid), .. } => format!("urn:mara:mid:{mid}"),
            Self::Item { id, .. } => format!("urn:mara:id:{}", encode(id)),
            Self::Code { reference } => format!("urn:mara:code:{}", encode(reference)),
            Self::External { address } => format!("urn:mara:external:{}", encode(address)),
        }
    }
    fn existing(value: &crate::RelationEndpoint) -> Self {
        match value {
            crate::RelationEndpoint::Item { id, mid } => Self::Item {
                id: id.clone(),
                mid: Some(mid.clone()),
            },
            crate::RelationEndpoint::Code { reference } => Self::Code {
                reference: reference.clone(),
            },
            crate::RelationEndpoint::External { address } => Self::External {
                address: address.clone(),
            },
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Edge {
    pub relation: String,
    pub symmetric: bool,
    pub source: Endpoint,
    pub target: Endpoint,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Occurrence {
    pub identity: String,
    pub source: ItemSource,
    pub author: Endpoint,
    pub kind: String,
    pub relation: String,
    pub target: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Support {
    pub edge: Edge,
    pub occurrences: Vec<Occurrence>,
}
pub(crate) struct PreparedGraph {
    pub triples: Vec<Triple>,
    pub items: BTreeMap<String, (Endpoint, ItemSource)>,
    pub endpoints: BTreeMap<String, Endpoint>,
    pub support: BTreeMap<String, Support>,
}
fn node(iri: &str) -> NamedNode {
    NamedNode::new(iri).expect("canonical graph IRI")
}
fn triple(subject: &str, predicate: &str, object: impl Into<Term>) -> Triple {
    Triple::new(node(subject), node(predicate), object.into())
}
pub(crate) fn scalar(raw: &str, kind: FieldType) -> Result<Literal, String> {
    use oxrdf::vocab::xsd;
    let literal = match kind {
        FieldType::String | FieldType::Enum => Literal::new_simple_literal(raw),
        FieldType::Integer => Literal::new_typed_literal(
            raw.parse::<i64>()
                .map_err(|_| "invalid integer")?
                .to_string(),
            xsd::INTEGER,
        ),
        FieldType::Number => {
            let value = raw.parse::<f64>().map_err(|_| "invalid number")?;
            if !value.is_finite() {
                return Err("nonfinite number".into());
            }
            // Numeric values, not source lexical spellings; both zero signs represent zero.
            Literal::new_typed_literal(
                if value == 0.0 {
                    "0".into()
                } else {
                    value.to_string()
                },
                xsd::DOUBLE,
            )
        }
        FieldType::Boolean => Literal::new_typed_literal(
            raw.parse::<bool>()
                .map_err(|_| "invalid boolean")?
                .to_string(),
            xsd::BOOLEAN,
        ),
    };
    Ok(literal)
}

pub(crate) fn prepare(
    project: &crate::Project,
    corpus: &Corpus,
    schema: &Schema,
    fingerprint: &str,
) -> Result<PreparedGraph, String> {
    let mut graph = PreparedGraph {
        triples: vec![],
        items: BTreeMap::new(),
        endpoints: BTreeMap::new(),
        support: BTreeMap::new(),
    };
    for item in corpus.items() {
        let endpoint = Endpoint::item(item);
        let iri = endpoint.iri();
        graph
            .items
            .insert(iri.clone(), (endpoint.clone(), item.source().into()));
        graph.endpoints.insert(iri.clone(), endpoint);
        graph.triples.push(triple(
            &iri,
            oxrdf::vocab::rdf::TYPE.as_str(),
            node(&format!("urn:mara:flavour:{}", item.flavour())),
        ));
        // Structural metadata lives in a distinct namespace from schema fields.
        graph.triples.push(triple(
            &iri,
            "urn:mara:item:id",
            Literal::new_simple_literal(item.id()),
        ));
        graph.triples.push(triple(
            &iri,
            "urn:mara:item:title",
            Literal::new_simple_literal(item.title()),
        ));
        for field in item.metadata() {
            if let Some(definition) = schema.flavours()[item.flavour()].fields.get(field.key()) {
                graph.triples.push(triple(
                    &iri,
                    &format!("urn:mara:field:{}", field.key()),
                    scalar(field.value(), definition.field_type)?,
                ));
            }
        }
    }
    // Reuse canonical relation normalization and authored occurrences for MID-bearing edges.
    for record in corpus.relation_graph(schema).edges() {
        let edge = Edge {
            relation: record.edge.relation.clone(),
            symmetric: record.edge.symmetric,
            source: Endpoint::existing(&record.edge.source),
            target: Endpoint::existing(&record.edge.target),
        };
        let occurrences = crate::relations::occurrences(project, corpus, schema, &record.edge)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|o| Occurrence {
                identity: hash(&(
                    fingerprint,
                    &o.source,
                    &o.author,
                    &o.kind,
                    &o.relation,
                    &o.target,
                )),
                source: o.source,
                author: Endpoint::existing(&o.author),
                kind: o.kind,
                relation: o.relation,
                target: o.target,
            })
            .collect();
        insert_edge(&mut graph, edge, occurrences);
    }
    // Legacy items without MIDs use project-scoped ID identity, without manufacturing MIDs.
    for item in corpus.items() {
        for relation in item.relations() {
            let target = if let Some(address) = crate::external::address(relation.target()) {
                Endpoint::External {
                    address: address.into(),
                }
            } else if relation.target().starts_with("code:") {
                Endpoint::Code {
                    reference: relation.target().into(),
                }
            } else {
                Endpoint::item(
                    crate::resolve_item(corpus, relation.target()).map_err(|e| e.to_string())?,
                )
            };
            if item.mid().is_some() && !matches!(&target, Endpoint::Item { mid: None, .. }) {
                continue;
            }
            let (canonical, definition, inverse) = schema
                .resolve_relation(relation.name())
                .ok_or("unknown relation")?;
            let author = Endpoint::item(item);
            let (mut source, mut target_endpoint) = if inverse {
                (target, author.clone())
            } else {
                (author.clone(), target)
            };
            if definition.symmetric && source.iri() > target_endpoint.iri() {
                std::mem::swap(&mut source, &mut target_endpoint);
            }
            let occurrence = Occurrence {
                identity: hash(&(
                    fingerprint,
                    ItemSource::from(relation.source()),
                    &author,
                    relation.name(),
                    relation.target(),
                )),
                source: relation.source().into(),
                author,
                kind: if relation.inline {
                    "inline"
                } else {
                    "metadata"
                }
                .into(),
                relation: relation.name().into(),
                target: relation.target().into(),
            };
            insert_edge(
                &mut graph,
                Edge {
                    relation: canonical.into(),
                    symmetric: definition.symmetric,
                    source,
                    target: target_endpoint,
                },
                vec![occurrence],
            );
        }
    }
    Ok(graph)
}
fn insert_edge(graph: &mut PreparedGraph, edge: Edge, occurrences: Vec<Occurrence>) {
    let key = format!("urn:mara:query:edge:{}", hash(&edge));
    // Authored occurrences are finite source facts, not hidden path derivations.
    for occurrence in &occurrences {
        graph.triples.push(triple(
            &key,
            "urn:mara:query:occurrence",
            node(&format!(
                "urn:mara:query:occurrence:{}",
                occurrence.identity
            )),
        ));
    }
    if let Some(existing) = graph.support.get_mut(&key) {
        existing.occurrences.extend(occurrences);
        return;
    }
    let source = edge.source.iri();
    let target = edge.target.iri();
    graph.endpoints.insert(source.clone(), edge.source.clone());
    graph.endpoints.insert(target.clone(), edge.target.clone());
    let relation = format!("urn:mara:relation:{}", edge.relation);
    graph
        .triples
        .push(triple(&source, &relation, node(&target)));
    if edge.symmetric {
        graph
            .triples
            .push(triple(&target, &relation, node(&source)));
    }
    graph
        .triples
        .push(triple(&key, "urn:mara:query:source", node(&source)));
    graph
        .triples
        .push(triple(&key, "urn:mara:query:target", node(&target)));
    graph
        .triples
        .push(triple(&key, "urn:mara:query:relation", node(&relation)));
    graph.support.insert(key, Support { edge, occurrences });
}
