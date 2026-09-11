use super::{page::*, *};
use crate::{ConnectionKind, DiscoveryNodeKind, DiscoveryNodeSummary};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RelatedConnection {
    pub relation: String,
    pub direction: RelationDirection,
    pub neighbour: DiscoveryNodeSummary,
    pub source: ItemSource,
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
    fn resolve(schema: &Schema, name: &'a str) -> Result<Self, QueryError> {
        let (namespace, short) = name
            .split_once(':')
            .map_or((None, name), |(ns, short)| (Some(ns), short));
        let builtin = matches!(short, "contains" | "mentions");
        let authored = schema.relations().contains_key(short);
        match (namespace, builtin, authored) {
            (None, true, true) => Err(QueryError::AmbiguousRelationName {
                name: name.to_owned(),
            }),
            (None | Some("builtin"), true, _) => Ok(Self::Builtin(short)),
            (None | Some("schema"), _, true) => Ok(Self::Schema(short)),
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
            Self::Builtin(name) if schema.relations().contains_key(*name) => {
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
            "discovery-related-v1",
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
    let matches = [RelationDirection::Outgoing, RelationDirection::Incoming].into_iter()
        .filter(|direction| filters.direction.is_none_or(|selected| selected == *direction))
        .flat_map(|direction| node.connections(direction))
        .filter(|connection| relations.is_empty() || relations.contains(&RelationName::from_kind(connection.kind)))
        .filter(|connection| filters.flavours.is_empty() || matches!(connection.neighbour.kind(), DiscoveryNodeKind::Item(item) if matches_name(&filters.flavours, item.flavour())))
        .collect::<Vec<_>>();
    if filters.cursor.is_some() && (start == 0 || start >= matches.len()) {
        return Err(page_error(
            "invalid continuation position; restart from the first page",
        ));
    }
    let mut page = RelatedResult {
        format_version: 1,
        node: node.summary(),
        connections: Vec::new(),
        has_more: false,
        next_cursor: None,
    };
    ensure_budget(&page)?;
    for connection in matches.iter().skip(start).take(limit) {
        page.connections.push(RelatedConnection {
            relation: RelationName::from_kind(connection.kind).display(schema),
            direction: connection.direction,
            neighbour: connection.neighbour.summary(),
            source: connection.source.into(),
        });
        (page.has_more, page.next_cursor) =
            continuation(start, page.connections.len(), matches.len(), &fingerprint);
        if ensure_budget(&page).is_err() {
            page.connections.pop();
            if page.connections.is_empty() {
                return Err(budget_error());
            }
            (page.has_more, page.next_cursor) =
                continuation(start, page.connections.len(), matches.len(), &fingerprint);
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
    if filters.direction != Some(RelationDirection::Incoming) {
        for relation in item
            .relations()
            .iter()
            .filter(|relation| selected(relation.name()))
        {
            resolve_relation_target(corpus, item, relation.name(), relation.target())?;
        }
    }
    if filters.direction != Some(RelationDirection::Outgoing) {
        for source in corpus
            .items()
            .filter(|source| matches_name(&filters.flavours, source.flavour()))
        {
            for relation in source.relations().iter().filter(|relation| {
                selected(relation.name())
                    && relation_handle_can_target_item(relation.target(), item)
            }) {
                resolve_relation_target(corpus, source, relation.name(), relation.target())?;
            }
        }
    }
    Ok(())
}
