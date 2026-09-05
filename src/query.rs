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

mod get;
mod page;
pub use get::{EntryRange, ItemGetResult, MetadataFragment, TextRange, get_item_page};
pub use page::{ItemCollectionResult, RelatedItemsResult, SearchExcerpt};

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
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    title_truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    excerpts: Option<Vec<SearchExcerpt>>,
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

    pub const fn title_truncated(&self) -> bool {
        self.title_truncated
    }

    pub fn excerpts(&self) -> Option<&[SearchExcerpt]> {
        self.excerpts.as_deref()
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
            title_truncated: false,
            excerpts: None,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    cursor: Option<String>,
    ids: Vec<String>,
    excerpts: bool,
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
            ..Self::default()
        }
    }

    pub fn with_cursor(mut self, cursor: Option<String>) -> Self {
        self.cursor = cursor;
        self
    }

    pub fn with_search_options(mut self, ids: Vec<String>, excerpts: bool) -> Self {
        self.ids = ids;
        self.excerpts = excerpts;
        self
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
    limit: Option<usize>,
    cursor: Option<String>,
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
            ..Self::default()
        }
    }

    pub fn with_page(mut self, limit: Option<usize>, cursor: Option<String>) -> Self {
        self.limit = limit;
        self.cursor = cursor;
        self
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
    InvalidPage {
        message: String,
    },
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
            Self::InvalidPage { message } => formatter.write_str(message),
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
) -> Result<ItemCollectionResult, QueryError> {
    page::filtered_page(corpus, schema, filters, None)
}

pub fn search_items(
    corpus: &Corpus,
    schema: &Schema,
    query: &str,
    filters: &ItemFilters,
) -> Result<ItemCollectionResult, QueryError> {
    page::filtered_page(corpus, schema, filters, Some(query))
}

pub fn related_items(
    corpus: &Corpus,
    schema: &Schema,
    id: &str,
    filters: &RelatedFilters,
) -> Result<RelatedItemsResult, QueryError> {
    page::related_page(corpus, schema, id, filters)
}

fn related_matches(
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

fn filtered_items<'a>(
    corpus: &'a Corpus,
    schema: &Schema,
    filters: &ItemFilters,
    query: Option<&str>,
) -> Result<Vec<&'a Item>, QueryError> {
    validate_flavours(schema, &filters.flavours)?;
    validate_relations(schema, &filters.relations)?;
    validate_fields(schema, &filters.fields)?;
    let paths = normalized_paths(&filters.paths)?;
    let fields = grouped_fields(&filters.fields);
    let query = query.map(keyword_terms);
    let selected = filters
        .ids
        .iter()
        .map(|id| resolve_item(corpus, id))
        .collect::<Result<Vec<_>, _>>()?;

    let items = corpus
        .items()
        .filter(|item| {
            selected.is_empty()
                || selected
                    .iter()
                    .any(|selected| std::ptr::eq(*selected, *item))
        })
        .filter(|item| matches_name(&filters.flavours, item.flavour()))
        .filter(|item| {
            paths.is_empty()
                || paths
                    .iter()
                    .any(|path| item.source().path().starts_with(path))
        })
        .filter(|item| {
            matches_name_filter(&filters.relations, |name| {
                item.relations()
                    .iter()
                    .any(|relation| relation.name() == name)
            })
        })
        .filter(|item| matches_fields(item, &fields));
    let Some(query) = query else {
        return Ok(items.collect());
    };
    let mut ranked = items
        .filter_map(|item| search_rank(item, &query).map(|rank| (rank, item)))
        .collect::<Vec<_>>();
    // Stable sorting retains document-path/source order for equal ranks.
    ranked.sort_by_key(|(rank, _)| std::cmp::Reverse(*rank));
    Ok(ranked.into_iter().map(|(_, item)| item).collect())
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

pub(crate) fn normalized_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>, QueryError> {
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

// The exact-match group dominates field weights, regardless of occurrences.
fn search_rank(item: &Item, query: &BTreeSet<String>) -> Option<(bool, usize)> {
    if query.is_empty() {
        return Some((true, 0));
    }
    let mut exact_words = BTreeMap::<String, usize>::new();
    let mut fuzzy_words = BTreeMap::<String, usize>::new();
    let mut include = |value: &str, weight: usize, fuzzy: bool| {
        for word in keyword_terms(value) {
            if fuzzy {
                fuzzy_words
                    .entry(word.clone())
                    .and_modify(|current| *current = (*current).max(weight))
                    .or_insert(weight);
            }
            exact_words
                .entry(word)
                .and_modify(|current| *current = (*current).max(weight))
                .or_insert(weight);
        }
    };
    include(item.id(), 3, false);
    include(item.body(), 1, true);
    for entry in item.metadata() {
        include(entry.key(), 1, true);
        include(
            entry.value(),
            if entry.key() == "title" { 3 } else { 1 },
            entry.key() != "mid",
        );
    }

    let mut all_exact = true;
    let mut score = 0;
    for term in query {
        let exact_weight = exact_words.get(term).copied().unwrap_or(0);
        all_exact &= exact_weight > 0;
        let mut weight = exact_weight;
        for (word, candidate_weight) in &fuzzy_words {
            if *candidate_weight > weight && word_matches(term, word) {
                weight = *candidate_weight;
            }
        }
        if weight == 0 {
            return None;
        }
        score += weight;
    }
    Some((all_exact, score))
}

// Both item selection and source excerpts compare complete normalized words.
fn word_matches(query: &str, word: &str) -> bool {
    if query == word {
        return true;
    }
    let query_length = query.chars().count();
    let max_edits = match query_length {
        0..=3 => return false,
        4..=7 => 1,
        _ => 2,
    };
    if query_length.abs_diff(word.chars().count()) > max_edits {
        return false;
    }
    strsim::damerau_levenshtein(query, word) <= max_edits
}

fn keyword_terms(value: &str) -> BTreeSet<String> {
    let canonical = canonical_text(value);
    canonical.unicode_words().map(ToOwned::to_owned).collect()
}

fn canonical_text(value: &str) -> String {
    value.nfc().case_fold().nfc().collect()
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
