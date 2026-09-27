//! Read-only canonical relation identity and occurrence counts.
use crate::query::resolve_item;
use crate::{Corpus, Item, Schema};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RelationEndpoint {
    Item { id: String, mid: String },
    External { address: String },
    Code { reference: String },
}

impl RelationEndpoint {
    fn new(item: &Item) -> Result<Self, RelationError> {
        Ok(Self::Item {
            id: item.id().to_owned(),
            mid: item
                .mid()
                .ok_or_else(|| {
                    RelationError::new(
                        "invalid_endpoint",
                        "item has no MID; run project mid backfill",
                    )
                })?
                .to_owned(),
        })
    }
    pub fn id(&self) -> &str {
        match self {
            Self::Item { id, .. } => id,
            Self::External { address } => address,
            Self::Code { reference } => reference,
        }
    }
    fn identity(&self) -> NodeIdentity {
        match self {
            Self::Item { mid, .. } => NodeIdentity::Item(mid.clone()),
            Self::Code { reference } => NodeIdentity::Code(reference.clone()),
            Self::External { address } => NodeIdentity::External(address.clone()),
        }
    }
}

/// Identity in the disposable relation graph. Only item identities are persisted MIDs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum NodeIdentity {
    Item(String),
    Code(String),
    External(String),
}

#[derive(Debug, Clone)]
pub(crate) struct GraphEdge {
    pub edge: RelationEdge,
    pub occurrence_count: usize,
}

/// One semantic graph for item, code and external endpoints. Invalid assertions
/// have no edge; source validation reports them before policy evaluation.
#[derive(Debug, Default)]
pub(crate) struct RelationGraph {
    pub nodes: BTreeMap<NodeIdentity, RelationEndpoint>,
    edges: BTreeMap<(String, NodeIdentity, NodeIdentity), GraphEdge>,
}

impl RelationGraph {
    // @mara implements DES-CANONICAL-TRACE-RELATIONS
    pub(crate) fn new(corpus: &Corpus, schema: &Schema) -> Self {
        let mut graph = Self::default();
        for item in corpus.items() {
            if let Ok(endpoint) = RelationEndpoint::new(item) {
                graph.nodes.insert(endpoint.identity(), endpoint);
            }
            for relation in item.relations() {
                if let Ok(edge) = resolve_edge(
                    corpus,
                    schema,
                    item.id(),
                    relation.name(),
                    relation.target(),
                ) {
                    graph.insert(edge);
                }
            }
        }
        for file in corpus.code().files() {
            for marker in &file.markers {
                if let Ok(edge) = resolve_edge(
                    corpus,
                    schema,
                    &marker.endpoint,
                    &marker.relation,
                    &marker.target,
                ) {
                    graph.insert(edge);
                }
            }
        }
        graph
    }

    fn insert(&mut self, edge: RelationEdge) {
        let from = edge.source.identity();
        let to = edge.target.identity();
        self.nodes.insert(from.clone(), edge.source.clone());
        self.nodes.insert(to.clone(), edge.target.clone());
        self.edges
            .entry((edge.relation.clone(), from, to))
            .and_modify(|record| record.occurrence_count += 1)
            .or_insert(GraphEdge {
                edge,
                occurrence_count: 1,
            });
    }

    pub(crate) fn edges(&self) -> impl Iterator<Item = &GraphEdge> {
        self.edges.values()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RelationEdge {
    pub relation: String,
    pub symmetric: bool,
    pub source: RelationEndpoint,
    pub target: RelationEndpoint,
}

impl RelationEdge {
    pub(crate) fn code(
        schema: &Schema,
        reference: &str,
        name: &str,
        item: &Item,
    ) -> Result<Self, RelationError> {
        let (canonical, definition, inverse) = schema.resolve_relation(name).ok_or_else(|| {
            RelationError::new("invalid_relation", format!("unknown relation '{name}'"))
        })?;
        if !definition.code_source || !definition.target.iter().any(|f| f == item.flavour()) {
            return Err(RelationError::new(
                "invalid_endpoint",
                format!("relation '{name}' does not allow this code/item pair"),
            ));
        }
        if inverse {
            return Err(RelationError::new(
                "invalid_relation",
                "code source assertions require the canonical relation name",
            ));
        }
        Ok(Self {
            relation: canonical.into(),
            symmetric: false,
            source: RelationEndpoint::Code {
                reference: reference.into(),
            },
            target: RelationEndpoint::new(item)?,
        })
    }
    pub(crate) fn external(
        schema: &Schema,
        source: &Item,
        name: &str,
        address: &str,
    ) -> Result<Self, RelationError> {
        let (canonical, definition, inverse) = schema.resolve_relation(name).ok_or_else(|| {
            RelationError::new("invalid_relation", format!("unknown relation '{name}'"))
        })?;
        if inverse
            || !definition.external
            || !definition.source.iter().any(|f| f == source.flavour())
            || !crate::external::valid_address(address)
        {
            return Err(RelationError::new(
                "invalid_endpoint",
                format!("relation '{name}' does not allow this external target"),
            ));
        }
        Ok(Self {
            relation: canonical.into(),
            symmetric: false,
            source: RelationEndpoint::new(source)?,
            target: RelationEndpoint::External {
                address: address.into(),
            },
        })
    }
    pub(crate) fn new(
        schema: &Schema,
        source: &Item,
        name: &str,
        target: &Item,
    ) -> Result<Self, RelationError> {
        let (canonical, definition, inverse) = schema.resolve_relation(name).ok_or_else(|| {
            RelationError::new("invalid_relation", format!("unknown relation '{name}'"))
        })?;
        let (mut source, mut target) = if inverse {
            (target, source)
        } else {
            (source, target)
        };
        if !definition.source.iter().any(|f| f == source.flavour()) {
            return Err(RelationError::new(
                "invalid_endpoint",
                format!(
                    "relation '{name}' does not allow source flavour '{}'",
                    source.flavour()
                ),
            ));
        }
        if !definition.target.iter().any(|f| f == target.flavour()) {
            return Err(RelationError::new(
                "invalid_endpoint",
                format!(
                    "relation '{name}' does not allow target flavour '{}'",
                    target.flavour()
                ),
            ));
        }
        if definition.same_flavour && source.flavour() != target.flavour() {
            return Err(RelationError::new(
                "invalid_endpoint",
                format!("relation '{name}' requires matching source and target flavours"),
            ));
        }
        if definition.symmetric && source.mid() > target.mid() {
            std::mem::swap(&mut source, &mut target);
        }
        Ok(Self {
            relation: canonical.to_owned(),
            symmetric: definition.symmetric,
            source: RelationEndpoint::new(source)?,
            target: RelationEndpoint::new(target)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RelationError {
    pub format_version: u8,
    pub error: RelationErrorDetail,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge: Option<Box<RelationEdge>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurrence_count: Option<usize>,
}
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RelationErrorDetail {
    pub code: String,
    pub message: String,
}
impl RelationError {
    pub(crate) fn new(code: &str, message: impl ToString) -> Self {
        Self {
            format_version: 1,
            error: RelationErrorDetail {
                code: code.into(),
                message: message.to_string(),
            },
            edge: None,
            occurrence_count: None,
        }
    }
}
impl std::fmt::Display for RelationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.message.fmt(f)
    }
}
impl std::error::Error for RelationError {}
impl From<String> for RelationError {
    fn from(message: String) -> Self {
        Self::new("invalid_relation", message)
    }
}
impl From<crate::Error> for RelationError {
    fn from(error: crate::Error) -> Self {
        Self::new("invalid_relation", error)
    }
}
impl From<crate::QueryError> for RelationError {
    fn from(error: crate::QueryError) -> Self {
        Self::new("invalid_relation", error)
    }
}

pub(crate) fn resolve_edge(
    corpus: &Corpus,
    schema: &Schema,
    source: &str,
    relation: &str,
    target: &str,
) -> Result<RelationEdge, RelationError> {
    if source.starts_with("code:") {
        corpus
            .code()
            .resolve(source)
            .map_err(|e| RelationError::new("invalid_endpoint", format!("code source {e:?}")))?;
        let item =
            resolve_item(corpus, target).map_err(|e| RelationError::new("invalid_endpoint", e))?;
        return RelationEdge::code(schema, source, relation, item);
    }
    let source_item = resolve_item(corpus, source).map_err(|error| {
        RelationError::new("invalid_endpoint", format!("relation source {error}"))
    })?;
    if let Some(address) = crate::external::address(target) {
        return RelationEdge::external(schema, source_item, relation, address);
    }
    if target.starts_with("code:") {
        corpus
            .code()
            .resolve(target)
            .map_err(|e| RelationError::new("invalid_endpoint", format!("code target {e:?}")))?;
        let (canonical, definition, inverse) =
            schema.resolve_relation(relation).ok_or_else(|| {
                RelationError::new("invalid_relation", format!("unknown relation '{relation}'"))
            })?;
        if !inverse || !definition.code_source {
            return Err(RelationError::new(
                "invalid_endpoint",
                "item-to-code authoring requires the declared inverse alias",
            ));
        }
        return RelationEdge::code(schema, target, canonical, source_item);
    }
    let target_item = resolve_item(corpus, target).map_err(|error| {
        RelationError::new("invalid_endpoint", format!("relation target {error}"))
    })?;
    RelationEdge::new(schema, source_item, relation, target_item)
}
