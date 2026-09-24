use super::{page::*, *};
use crate::{ConnectionKind, DiscoveryNodeKind, DiscoveryNodeSummary};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum RelatedNeighbour {
    Internal(DiscoveryNodeSummary),
    External { kind: String, address: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RelatedConnection {
    pub relation: String,
    pub direction: RelationDirection,
    pub neighbour: RelatedNeighbour,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ItemSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge: Option<crate::RelationEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurrence_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RelatedResult {
    pub format_version: u8,
    pub node: DiscoveryNodeSummary,
    pub connections: Vec<RelatedConnection>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
enum RelationName<'a> {
    Builtin(&'a str),
    Schema(&'a str),
}

impl<'a> RelationName<'a> {
    fn resolve(schema: &'a Schema, name: &'a str) -> Result<Self, QueryError> {
        let (namespace, short) = name
            .split_once(':')
            .map_or((None, name), |(ns, short)| (Some(ns), short));
        let builtin = matches!(short, "contains" | "mentions");
        let resolved = schema.resolve_relation(short);
        let authored = resolved.is_some();
        match (namespace, builtin, authored) {
            (None, true, true) => Err(QueryError::AmbiguousRelationName {
                name: name.to_owned(),
            }),
            (None | Some("builtin"), true, _) => Ok(Self::Builtin(short)),
            (None | Some("schema"), _, true) => Ok(Self::Schema(resolved.unwrap().0)),
            _ => Err(QueryError::UnknownRelation {
                name: name.to_owned(),
            }),
        }
    }

    fn from_kind(kind: ConnectionKind<'a>) -> Self {
        match kind {
            ConnectionKind::Contains | ConnectionKind::ContainedBy => Self::Builtin("contains"),
            ConnectionKind::Mentions => Self::Builtin("mentions"),
            ConnectionKind::Schema(name) => Self::Schema(name),
        }
    }

    fn display(&self, schema: &Schema) -> String {
        match self {
            Self::Builtin(name) if schema.resolve_relation(name).is_some() => {
                format!("builtin:{name}")
            }
            Self::Schema(name) if matches!(*name, "contains" | "mentions") => {
                format!("schema:{name}")
            }
            Self::Builtin(name) | Self::Schema(name) => (*name).to_owned(),
        }
    }
}

/// Direct connections only; outgoing first, then neighbour source order and
/// authored evidence order within each direction. Parallel edges stay distinct.
pub fn related(
    corpus: &Corpus,
    schema: &Schema,
    reference: &str,
    filters: &RelatedFilters,
) -> Result<RelatedResult, QueryError> {
    let limit = page_limit(filters.limit)?;
    validate_flavours(schema, &filters.flavours)?;
    let relations = filters
        .relations
        .iter()
        .map(|name| RelationName::resolve(schema, name))
        .collect::<Result<Vec<_>, _>>()?;
    let fingerprint = fingerprint(
        corpus,
        schema,
        &(
            "discovery-related-v2",
            reference,
            filters.direction,
            &filters.relations,
            &filters.flavours,
            limit,
        ),
    )?;
    let start = cursor_position(filters.cursor.as_deref(), &fingerprint)?;
    let graph = corpus.discovery();
    let node = graph.resolve(reference)?;
    if let DiscoveryNodeKind::Item(item) = node.kind() {
        validate_authored_targets(corpus, item, filters, &relations)?;
    }
    let matches = [RelationDirection::Outgoing, RelationDirection::Incoming, RelationDirection::Symmetric].into_iter()
        .filter(|direction| filters.direction.is_none_or(|selected| selected == *direction))
        .flat_map(|direction| node.connections(direction))
        .filter(|connection| !(filters.direction.is_none()
            && connection.direction == RelationDirection::Incoming
            && matches!(connection.kind, ConnectionKind::Schema(_))
            && connection.neighbour.reference() == node.reference()))
        .filter(|connection| relations.is_empty() || relations.contains(&RelationName::from_kind(connection.kind)))
        .filter(|connection| filters.flavours.is_empty() || matches!(connection.neighbour.kind(), DiscoveryNodeKind::Item(item) if matches_name(&filters.flavours, item.flavour())))
        .collect::<Vec<_>>();
    let mut all = Vec::new();
    let outgoing_count = matches
        .iter()
        .filter(|c| c.direction == RelationDirection::Outgoing)
        .count();
    for connection in matches.iter() {
        let (edge, label, count) = if let ConnectionKind::Schema(name) = connection.kind {
            let DiscoveryNodeKind::Item(item) = node.kind() else {
                unreachable!()
            };
            let DiscoveryNodeKind::Item(neighbour) = connection.neighbour.kind() else {
                unreachable!()
            };
            let (source, target) = if connection.direction == RelationDirection::Incoming {
                (neighbour, item)
            } else {
                (item, neighbour)
            };
            let edge = crate::RelationEdge::new(schema, source, name, target)
                .map_err(|error| page_error(&error.to_string()))?;
            let definition = &schema.relations()[name];
            let label = if connection.direction == RelationDirection::Incoming {
                definition.inverse.as_deref().unwrap_or(name)
            } else {
                name
            };
            (
                Some(edge),
                Some(label.to_owned()),
                Some(connection.occurrence_count),
            )
        } else {
            (None, None, None)
        };
        all.push(RelatedConnection {
            relation: RelationName::from_kind(connection.kind).display(schema),
            direction: connection.direction,
            neighbour: RelatedNeighbour::Internal(connection.neighbour.summary()),
            source: edge.is_none().then(|| connection.source.into()),
            edge,
            label,
            occurrence_count: count,
        });
    }
    if filters
        .direction
        .is_none_or(|d| d == RelationDirection::Outgoing)
        && filters.flavours.is_empty()
    {
        if let DiscoveryNodeKind::Item(item) = node.kind() {
            let mut externals = BTreeMap::<(String, String), usize>::new();
            for relation in item.relations() {
                let Some(address) = crate::external::address(relation.target()) else {
                    continue;
                };
                if !relations.is_empty()
                    && !relations.contains(&RelationName::Schema(&relation.canonical))
                {
                    continue;
                }
                *externals
                    .entry((address.to_owned(), relation.canonical.clone()))
                    .or_default() += 1;
            }
            let mut external_connections = Vec::new();
            for ((address, name), count) in externals {
                let edge = crate::RelationEdge::external(schema, item, &name, &address)
                    .map_err(|error| page_error(&error.to_string()))?;
                external_connections.push(RelatedConnection {
                    relation: RelationName::Schema(&name).display(schema),
                    direction: RelationDirection::Outgoing,
                    neighbour: RelatedNeighbour::External {
                        kind: "external".into(),
                        address,
                    },
                    source: None,
                    label: Some(name),
                    edge: Some(edge),
                    occurrence_count: Some(count),
                });
            }
            all.splice(outgoing_count..outgoing_count, external_connections);
        }
    }
    if filters.cursor.is_some() && (start == 0 || start >= all.len()) {
        return Err(page_error(
            "invalid continuation position; restart from the first page",
        ));
    }
    let mut page = RelatedResult {
        format_version: 2,
        node: node.summary(),
        connections: Vec::new(),
        has_more: false,
        next_cursor: None,
    };
    ensure_budget(&page)?;
    for connection in all.iter().skip(start).take(limit) {
        page.connections.push(connection.clone());
        (page.has_more, page.next_cursor) =
            continuation(start, page.connections.len(), all.len(), &fingerprint);
        if ensure_budget(&page).is_err() {
            page.connections.pop();
            if page.connections.is_empty() {
                return Err(budget_error());
            }
            (page.has_more, page.next_cursor) =
                continuation(start, page.connections.len(), all.len(), &fingerprint);
            break;
        }
    }
    ensure_budget(&page)?;
    Ok(page)
}

fn ensure_budget(page: &RelatedResult) -> Result<(), QueryError> {
    if serde_json::to_vec(page)
        .map_err(|_| page_error("could not serialize related page"))?
        .len()
        > PAGE_BYTES
    {
        return Err(budget_error());
    }
    Ok(())
}

fn budget_error() -> QueryError {
    page_error(
        "a related node or connection cannot fit the 65536-byte page budget; shorten oversized identity/location fields or relation names in the source",
    )
}

// Unresolved authored targets have no graph edge. Preserve actionable traversal
// errors for selected relations rather than silently presenting an incomplete view.
fn validate_authored_targets(
    corpus: &Corpus,
    item: &Item,
    filters: &RelatedFilters,
    names: &[RelationName<'_>],
) -> Result<(), QueryError> {
    let selected = |name| names.is_empty() || names.contains(&RelationName::Schema(name));
    for author in corpus.items() {
        for relation in author.relations().iter().filter(|r| selected(&r.canonical)) {
            let is_author = std::ptr::eq(author, item);
            if !is_author && !relation_handle_can_target_item(relation.target(), item) {
                continue;
            }
            let direction = if relation.symmetric {
                RelationDirection::Symmetric
            } else if is_author != relation.inverse {
                RelationDirection::Outgoing
            } else {
                RelationDirection::Incoming
            };
            if filters.direction.is_some_and(|d| d != direction) {
                continue;
            }
            if crate::external::address(relation.target()).is_none() {
                resolve_relation_target(corpus, author, relation.name(), relation.target())?;
            }
        }
    }
    Ok(())
}
