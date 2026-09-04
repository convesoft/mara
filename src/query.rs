use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    path::{Component, Path, PathBuf},
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

use crate::{Corpus, Item, Schema, SourceLocation};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ItemSource {
    path: PathBuf,
    start_byte: usize,
    end_byte: usize,
    start_line: usize,
    end_line: usize,
}

impl ItemSource {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn start_byte(&self) -> usize {
        self.start_byte
    }

    pub const fn end_byte(&self) -> usize {
        self.end_byte
    }

    pub const fn start_line(&self) -> usize {
        self.start_line
    }

    pub const fn end_line(&self) -> usize {
        self.end_line
    }
}

impl From<&SourceLocation> for ItemSource {
    fn from(source: &SourceLocation) -> Self {
        let span = source.span();
        Self {
            path: source.path().to_path_buf(),
            start_byte: span.start_byte(),
            end_byte: span.end_byte(),
            start_line: span.start_line(),
            end_line: span.end_line(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ItemSummary {
    id: String,
    mid: Option<String>,
    flavour: String,
    title: String,
    path: PathBuf,
    line: usize,
}

impl ItemSummary {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn mid(&self) -> Option<&str> {
        self.mid.as_deref()
    }

    pub fn flavour(&self) -> &str {
        &self.flavour
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn line(&self) -> usize {
        self.line
    }
}

impl From<&Item> for ItemSummary {
    fn from(item: &Item) -> Self {
        Self {
            id: item.id().to_owned(),
            mid: item.mid().map(ToOwned::to_owned),
            flavour: item.flavour().to_owned(),
            title: item.title().to_owned(),
            path: item.source().path().to_path_buf(),
            line: item.source().span().start_line(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct MetadataValue {
    key: String,
    value: String,
}

impl MetadataValue {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RelationSummary {
    relation: String,
    item: ItemSummary,
}

impl RelationSummary {
    pub fn relation(&self) -> &str {
        &self.relation
    }

    pub const fn item(&self) -> &ItemSummary {
        &self.item
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ResolvedItem {
    summary: ItemSummary,
    source: ItemSource,
    metadata: Vec<MetadataValue>,
    body: String,
    outgoing_relations: Vec<RelationSummary>,
    incoming_relations: Vec<RelationSummary>,
}

impl ResolvedItem {
    pub const fn summary(&self) -> &ItemSummary {
        &self.summary
    }

    pub const fn source(&self) -> &ItemSource {
        &self.source
    }

    pub fn metadata(&self) -> &[MetadataValue] {
        &self.metadata
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn outgoing_relations(&self) -> &[RelationSummary] {
        &self.outgoing_relations
    }

    pub fn incoming_relations(&self) -> &[RelationSummary] {
        &self.incoming_relations
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldFilter {
    name: String,
    value: String,
}

impl FieldFilter {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemFilters {
    flavours: Vec<String>,
    fields: Vec<FieldFilter>,
    relations: Vec<String>,
    paths: Vec<PathBuf>,
    limit: Option<usize>,
}

impl ItemFilters {
    pub fn new(
        flavours: Vec<String>,
        fields: Vec<FieldFilter>,
        relations: Vec<String>,
        paths: Vec<PathBuf>,
        limit: Option<usize>,
    ) -> Self {
        Self {
            flavours,
            fields,
            relations,
            paths,
            limit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RelationDirection {
    Incoming,
    Outgoing,
}

impl RelationDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Outgoing => "outgoing",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelatedFilters {
    direction: Option<RelationDirection>,
    relations: Vec<String>,
    flavours: Vec<String>,
}

impl RelatedFilters {
    pub fn new(
        direction: Option<RelationDirection>,
        relations: Vec<String>,
        flavours: Vec<String>,
    ) -> Self {
        Self {
            direction,
            relations,
            flavours,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RelatedItem {
    direction: RelationDirection,
    relation: String,
    item: ItemSummary,
}

impl RelatedItem {
    pub const fn direction(&self) -> RelationDirection {
        self.direction
    }

    pub fn relation(&self) -> &str {
        &self.relation
    }

    pub const fn item(&self) -> &ItemSummary {
        &self.item
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    MissingItem {
        id: String,
    },
    AmbiguousItem {
        id: String,
    },
    AmbiguousMid {
        mid: String,
    },
    MissingRelationTarget {
        source: String,
        relation: String,
        target: String,
    },
    AmbiguousRelationTarget {
        source: String,
        relation: String,
        target: String,
    },
    UnknownFlavour {
        name: String,
    },
    UnknownField {
        name: String,
    },
    UnknownRelation {
        name: String,
    },
    InvalidPath {
        path: PathBuf,
    },
}

impl fmt::Display for QueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingItem { id } => write!(formatter, "item '{id}' was not found"),
            Self::AmbiguousItem { id } => write!(formatter, "item ID '{id}' is ambiguous"),
            Self::AmbiguousMid { mid } => write!(formatter, "item MID '{mid}' is ambiguous"),
            Self::MissingRelationTarget {
                source,
                relation,
                target,
            } => write!(
                formatter,
                "relation '{relation}' from '{source}' references missing item '{target}'"
            ),
            Self::AmbiguousRelationTarget {
                source,
                relation,
                target,
            } => write!(
                formatter,
                "relation '{relation}' from '{source}' references ambiguous item '{target}'"
            ),
            Self::UnknownFlavour { name } => write!(formatter, "unknown flavour '{name}'"),
            Self::UnknownField { name } => write!(formatter, "unknown field '{name}'"),
            Self::UnknownRelation { name } => write!(formatter, "unknown relation '{name}'"),
            Self::InvalidPath { path } => write!(
                formatter,
                "path filter must be a project-relative path: '{}'",
                path.display()
            ),
        }
    }
}

impl Error for QueryError {}

pub fn get_item(corpus: &Corpus, id: &str) -> Result<ResolvedItem, QueryError> {
    let item = resolve_item(corpus, id)?;
    let outgoing_relations = item
        .relations()
        .iter()
        .map(|relation| {
            Ok(RelationSummary {
                relation: relation.name().to_owned(),
                item: resolve_relation_target(corpus, item, relation.name(), relation.target())?
                    .into(),
            })
        })
        .collect::<Result<_, QueryError>>()?;
    let mut incoming_relations = Vec::new();
    for source in corpus.items() {
        for relation in source
            .relations()
            .iter()
            .filter(|relation| relation_handle_can_target_item(relation.target(), item))
        {
            let target =
                resolve_relation_target(corpus, source, relation.name(), relation.target())?;
            if same_item_identity(target, item) {
                incoming_relations.push(RelationSummary {
                    relation: relation.name().to_owned(),
                    item: source.into(),
                });
            }
        }
    }

    Ok(ResolvedItem {
        summary: item.into(),
        source: item.source().into(),
        metadata: item
            .metadata()
            .iter()
            .map(|entry| MetadataValue {
                key: entry.key().to_owned(),
                value: entry.value().to_owned(),
            })
            .collect(),
        body: item.body().to_owned(),
        outgoing_relations,
        incoming_relations,
    })
}

pub fn list_items(
    corpus: &Corpus,
    schema: &Schema,
    filters: &ItemFilters,
) -> Result<Vec<ItemSummary>, QueryError> {
    filtered_items(corpus, schema, filters, None)
}

pub fn search_items(
    corpus: &Corpus,
    schema: &Schema,
    query: &str,
    filters: &ItemFilters,
) -> Result<Vec<ItemSummary>, QueryError> {
    filtered_items(corpus, schema, filters, Some(query))
}

pub fn related_items(
    corpus: &Corpus,
    schema: &Schema,
    id: &str,
    filters: &RelatedFilters,
) -> Result<Vec<RelatedItem>, QueryError> {
    validate_flavours(schema, &filters.flavours)?;
    validate_relations(schema, &filters.relations)?;
    let item = resolve_item(corpus, id)?;
    let mut related = Vec::new();

    if filters.direction != Some(RelationDirection::Incoming) {
        for relation in item.relations() {
            if !matches_name(&filters.relations, relation.name()) {
                continue;
            }
            let neighbour =
                resolve_relation_target(corpus, item, relation.name(), relation.target())?;
            if matches_name(&filters.flavours, neighbour.flavour()) {
                related.push(RelatedItem {
                    direction: RelationDirection::Outgoing,
                    relation: relation.name().to_owned(),
                    item: neighbour.into(),
                });
            }
        }
    }

    if filters.direction != Some(RelationDirection::Outgoing) {
        for source in corpus.items() {
            if !matches_name(&filters.flavours, source.flavour()) {
                continue;
            }
            for relation in source.relations().iter().filter(|relation| {
                matches_name(&filters.relations, relation.name())
                    && relation_handle_can_target_item(relation.target(), item)
            }) {
                let target =
                    resolve_relation_target(corpus, source, relation.name(), relation.target())?;
                if !same_item_identity(target, item) {
                    continue;
                }
                related.push(RelatedItem {
                    direction: RelationDirection::Incoming,
                    relation: relation.name().to_owned(),
                    item: source.into(),
                });
            }
        }
    }

    Ok(related)
}

fn filtered_items(
    corpus: &Corpus,
    schema: &Schema,
    filters: &ItemFilters,
    query: Option<&str>,
) -> Result<Vec<ItemSummary>, QueryError> {
    validate_flavours(schema, &filters.flavours)?;
    validate_relations(schema, &filters.relations)?;
    validate_fields(schema, &filters.fields)?;
    let paths = normalized_paths(&filters.paths)?;
    let fields = grouped_fields(&filters.fields);
    let query = query.map(keyword_terms);

    Ok(corpus
        .items()
        .filter(|item| matches_name(&filters.flavours, item.flavour()))
        .filter(|item| paths.is_empty() || paths.iter().any(|path| path == item.source().path()))
        .filter(|item| {
            matches_name_filter(&filters.relations, |name| {
                item.relations()
                    .iter()
                    .any(|relation| relation.name() == name)
            })
        })
        .filter(|item| matches_fields(item, &fields))
        .filter(|item| query.as_ref().is_none_or(|query| matches_text(item, query)))
        .take(filters.limit.unwrap_or(usize::MAX))
        .map(ItemSummary::from)
        .collect())
}

fn validate_flavours(schema: &Schema, names: &[String]) -> Result<(), QueryError> {
    if let Some(name) = names
        .iter()
        .find(|name| !schema.flavours().contains_key(name.as_str()))
    {
        return Err(QueryError::UnknownFlavour { name: name.clone() });
    }
    Ok(())
}

fn validate_relations(schema: &Schema, names: &[String]) -> Result<(), QueryError> {
    if let Some(name) = names
        .iter()
        .find(|name| !schema.relations().contains_key(name.as_str()))
    {
        return Err(QueryError::UnknownRelation { name: name.clone() });
    }
    Ok(())
}

fn validate_fields(schema: &Schema, fields: &[FieldFilter]) -> Result<(), QueryError> {
    if let Some(field) = fields.iter().find(|field| {
        !schema
            .flavours()
            .values()
            .any(|flavour| flavour.fields().contains_key(field.name()))
    }) {
        return Err(QueryError::UnknownField {
            name: field.name.clone(),
        });
    }
    Ok(())
}

fn normalized_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>, QueryError> {
    paths
        .iter()
        .map(|path| {
            if path.as_os_str().is_empty()
                || path.is_absolute()
                || path.components().any(|component| {
                    matches!(
                        component,
                        Component::ParentDir | Component::RootDir | Component::Prefix(_)
                    )
                })
            {
                return Err(QueryError::InvalidPath { path: path.clone() });
            }
            let normalized = path
                .components()
                .filter(|component| *component != Component::CurDir)
                .collect::<PathBuf>();
            if normalized.as_os_str().is_empty() {
                return Err(QueryError::InvalidPath { path: path.clone() });
            }
            Ok(normalized)
        })
        .collect()
}

fn grouped_fields(fields: &[FieldFilter]) -> BTreeMap<&str, Vec<&str>> {
    let mut grouped = BTreeMap::<&str, Vec<&str>>::new();
    for field in fields {
        grouped.entry(field.name()).or_default().push(field.value());
    }
    grouped
}

fn matches_fields(item: &Item, fields: &BTreeMap<&str, Vec<&str>>) -> bool {
    fields.iter().all(|(name, values)| {
        item.metadata()
            .iter()
            .any(|entry| entry.key() == *name && values.iter().any(|value| entry.value() == *value))
    })
}

fn matches_text(item: &Item, query: &BTreeSet<String>) -> bool {
    let mut item_terms = BTreeSet::new();
    for value in [item.id(), item.title(), item.body()] {
        item_terms.extend(keyword_terms(value));
    }
    for entry in item.metadata() {
        item_terms.extend(keyword_terms(entry.key()));
        item_terms.extend(keyword_terms(entry.value()));
    }

    query.is_subset(&item_terms)
}

fn keyword_terms(value: &str) -> BTreeSet<String> {
    let canonical = value.nfc().case_fold().nfc().collect::<String>();
    canonical.unicode_words().map(ToOwned::to_owned).collect()
}

fn matches_name(names: &[String], candidate: &str) -> bool {
    names.is_empty() || names.iter().any(|name| name == candidate)
}

fn matches_name_filter(names: &[String], predicate: impl Fn(&str) -> bool) -> bool {
    names.is_empty() || names.iter().any(|name| predicate(name))
}

fn resolve_item<'a>(corpus: &'a Corpus, id: &str) -> Result<&'a Item, QueryError> {
    let by_mid = crate::is_mid(id);
    let mut matches = corpus.items().filter(|item| {
        if by_mid {
            item.mid() == Some(id)
        } else {
            item.id() == id
        }
    });
    let Some(item) = matches.next() else {
        return Err(QueryError::MissingItem { id: id.to_owned() });
    };
    if matches.next().is_some() {
        if by_mid {
            return Err(QueryError::AmbiguousMid { mid: id.to_owned() });
        }
        return Err(QueryError::AmbiguousItem { id: id.to_owned() });
    }
    Ok(item)
}

fn relation_handle_can_target_item(handle: &str, item: &Item) -> bool {
    if crate::is_mid(handle) {
        item.mid() == Some(handle)
    } else {
        item.id() == handle
    }
}

fn same_item_identity(left: &Item, right: &Item) -> bool {
    match (left.mid(), right.mid()) {
        (Some(left_mid), Some(right_mid)) => left_mid == right_mid,
        _ => left.id() == right.id(),
    }
}

fn resolve_relation_target<'a>(
    corpus: &'a Corpus,
    source: &Item,
    relation: &str,
    target: &str,
) -> Result<&'a Item, QueryError> {
    match resolve_item(corpus, target) {
        Ok(item) => Ok(item),
        Err(QueryError::MissingItem { .. }) => Err(QueryError::MissingRelationTarget {
            source: source.id().to_owned(),
            relation: relation.to_owned(),
            target: target.to_owned(),
        }),
        Err(QueryError::AmbiguousItem { .. }) => Err(QueryError::AmbiguousRelationTarget {
            source: source.id().to_owned(),
            relation: relation.to_owned(),
            target: target.to_owned(),
        }),
        Err(error) => Err(error),
    }
}
