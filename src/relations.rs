//! Canonical edge identity and snapshot-bound authored occurrence inspection.
use crate::query::{page::*, resolve_item};
use crate::{Corpus, Item, ItemSource, Project, Relation, Schema};
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
    fn mid(&self) -> Option<&str> {
        match self {
            Self::Item { mid, .. } => Some(mid),
            Self::External { .. } => None,
            Self::Code { .. } => None,
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
    pub source: ItemSource,
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
                    graph.insert(edge, relation.source().into());
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
                    graph.insert(edge, (&marker.source).into());
                }
            }
        }
        graph
    }

    fn insert(&mut self, edge: RelationEdge, source: ItemSource) {
        let from = edge.source.identity();
        let to = edge.target.identity();
        self.nodes.insert(from.clone(), edge.source.clone());
        self.nodes.insert(to.clone(), edge.target.clone());
        self.edges
            .entry((edge.relation.clone(), from, to))
            .and_modify(|record| record.occurrence_count += 1)
            .or_insert(GraphEdge {
                edge,
                source,
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
        if inverse && definition.inverse.is_none() {
            return Err(RelationError::new(
                "invalid_relation",
                "inverse alias is not declared",
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
    pub(crate) fn matches(&self, source: &Item, relation: &Relation, target: &Item) -> bool {
        let (a, b) = if relation.inverse {
            (target, source)
        } else {
            (source, target)
        };
        self.relation == relation.canonical
            && ((self.source.mid() == a.mid() && self.target.mid() == b.mid())
                || (self.symmetric && self.source.mid() == b.mid() && self.target.mid() == a.mid()))
    }
    pub(crate) fn matches_external(&self, source: &Item, relation: &Relation) -> bool {
        self.relation == relation.canonical
            && self.source.mid() == source.mid()
            && matches!(&self.target, RelationEndpoint::External { address } if crate::external::address(relation.target()) == Some(address.as_str()))
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RelationOccurrence {
    pub reference: String,
    pub kind: String,
    pub source: ItemSource,
    pub author: RelationEndpoint,
    pub relation: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RelationInspection {
    pub format_version: u8,
    pub edge: RelationEdge,
    pub occurrence_count: usize,
    pub occurrences: Vec<RelationOccurrence>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
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
    pub(crate) fn on_edge(mut self, edge: &RelationEdge, count: usize) -> Self {
        self.edge = Some(Box::new(edge.clone()));
        self.occurrence_count = Some(count);
        self
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
        let (_, definition, inverse) = schema.resolve_relation(relation).ok_or_else(|| {
            RelationError::new("invalid_relation", format!("unknown relation '{relation}'"))
        })?;
        if !inverse || !definition.code_source {
            return Err(RelationError::new(
                "invalid_endpoint",
                "item-to-code authoring requires the declared inverse alias",
            ));
        }
        return RelationEdge::code(schema, target, relation, source_item);
    }
    let target_item = resolve_item(corpus, target).map_err(|error| {
        RelationError::new("invalid_endpoint", format!("relation target {error}"))
    })?;
    RelationEdge::new(schema, source_item, relation, target_item)
}

pub(crate) fn occurrences(
    project: &Project,
    corpus: &Corpus,
    schema: &Schema,
    edge: &RelationEdge,
) -> Result<Vec<RelationOccurrence>, RelationError> {
    let snapshot = fingerprint(corpus, schema, &("relation-occurrences-v1", project.root()))?;
    let mut result = Vec::new();
    let mut index = 0;
    for item in corpus.items() {
        for relation in item.relations() {
            let matches = if let RelationEndpoint::Code { reference } = &edge.source {
                relation.inverse
                    && relation.canonical == edge.relation
                    && relation.target() == reference
                    && item.mid() == edge.target.mid()
            } else if crate::external::address(relation.target()).is_some() {
                edge.matches_external(item, relation)
            } else {
                resolve_item(corpus, relation.target())
                    .is_ok_and(|target| edge.matches(item, relation, target))
            };
            if matches {
                result.push(RelationOccurrence {
                    reference: format!("occ-1-{snapshot}-{index:016x}"),
                    kind: if relation.inline {
                        "inline"
                    } else {
                        "metadata"
                    }
                    .into(),
                    source: relation.source().into(),
                    author: RelationEndpoint::new(item)?,
                    relation: relation.name().to_owned(),
                    target: relation.target().to_owned(),
                });
            }
            index += 1;
        }
    }
    if let RelationEndpoint::Code { reference } = &edge.source {
        for file in corpus.code().files() {
            for marker in &file.markers {
                if marker.endpoint == *reference
                    && marker.relation == edge.relation
                    && resolve_item(corpus, &marker.target)
                        .is_ok_and(|item| item.mid() == edge.target.mid())
                {
                    result.push(RelationOccurrence {
                        reference: format!("occ-1-{snapshot}-{index:016x}"),
                        kind: "code_comment".into(),
                        source: (&marker.source).into(),
                        author: RelationEndpoint::Code {
                            reference: reference.clone(),
                        },
                        relation: marker.relation.clone(),
                        target: marker.target.clone(),
                    });
                }
                index += 1;
            }
        }
    }
    Ok(result)
}

pub(crate) fn inspect(
    project: &Project,
    corpus: &Corpus,
    schema: &Schema,
    params: &crate::RelationParams,
    limit: Option<usize>,
    cursor: Option<&str>,
) -> Result<RelationInspection, RelationError> {
    let limit = page_limit(limit)?;
    let edge = resolve_edge(
        corpus,
        schema,
        &params.source,
        &params.relation,
        &params.target,
    )?;
    let occurrences = occurrences(project, corpus, schema, &edge)?;
    if occurrences.is_empty() {
        return Err(
            RelationError::new("relation_not_found", "relation does not exist").on_edge(&edge, 0),
        );
    }
    let snapshot = fingerprint(
        corpus,
        schema,
        &(
            "relation-get-v1",
            project.root(),
            &params.source,
            &params.relation,
            &params.target,
            limit,
        ),
    )?;
    let start = cursor_position(cursor, &snapshot)?;
    if cursor.is_some() && (start == 0 || start >= occurrences.len()) {
        return Err(RelationError::new(
            "invalid_cursor",
            "invalid continuation position; restart inspection",
        ));
    }
    let mut page = RelationInspection {
        format_version: 1,
        edge,
        occurrence_count: occurrences.len(),
        occurrences: Vec::new(),
        has_more: false,
        next_cursor: None,
    };
    for occurrence in occurrences.into_iter().skip(start).take(limit) {
        page.occurrences.push(occurrence);
        (page.has_more, page.next_cursor) = continuation(
            start,
            page.occurrences.len(),
            page.occurrence_count,
            &snapshot,
        );
        if serde_json::to_vec(&page)
            .expect("serializable inspection")
            .len()
            > PAGE_BYTES
        {
            page.occurrences.pop();
            if page.occurrences.is_empty() {
                return Err(RelationError::new(
                    "page_limit",
                    "an occurrence cannot fit the 65536-byte page budget; shorten oversized identity/location fields",
                ));
            }
            (page.has_more, page.next_cursor) = continuation(
                start,
                page.occurrences.len(),
                page.occurrence_count,
                &snapshot,
            );
            break;
        }
    }
    Ok(page)
}
