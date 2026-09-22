//! Canonical edge identity and snapshot-bound authored occurrence inspection.
use crate::query::{page::*, resolve_item};
use crate::{Corpus, Item, ItemSource, Project, Relation, Schema};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RelationEndpoint {
    Item { id: String, mid: String },
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
        }
    }
    fn mid(&self) -> &str {
        match self {
            Self::Item { mid, .. } => mid,
        }
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
            && ((Some(self.source.mid()) == a.mid() && Some(self.target.mid()) == b.mid())
                || (self.symmetric
                    && Some(self.source.mid()) == b.mid()
                    && Some(self.target.mid()) == a.mid()))
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
    let source_item = resolve_item(corpus, source).map_err(|error| {
        RelationError::new("invalid_endpoint", format!("relation source {error}"))
    })?;
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
            if let Ok(target) = resolve_item(corpus, relation.target())
                && edge.matches(item, relation, target)
            {
                result.push(RelationOccurrence {
                    reference: format!("occ-1-{snapshot}-{index:016x}"),
                    kind: "metadata".into(),
                    source: relation.source().into(),
                    author: RelationEndpoint::new(item)?,
                    relation: relation.name().to_owned(),
                    target: relation.target().to_owned(),
                });
            }
            index += 1;
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
