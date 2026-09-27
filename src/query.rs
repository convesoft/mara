use crate::{Corpus, Item, Schema, SourceLocation};
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::{Component, Path, PathBuf},
};
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;
mod get;
pub use get::{EntryRange, GetResult, MetadataFragment, TextRange, get};
mod related;
pub use related::{RelatedConnection, RelatedNeighbour, RelatedResult, related};
mod page;
mod search;
pub use page::ItemCollectionResult;
pub use page::SearchExcerpt;
pub use search::{SearchHit, SearchResult, search};
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
        }
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
}

impl ItemFilters {
    pub fn with_ids(mut self, ids: Vec<String>) -> Self {
        self.ids = ids;
        self
    }
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
}

#[derive(Debug)]
pub enum QueryError {
    InvalidDiscoveryReference,
    MissingItem {
        id: String,
    },
    AmbiguousItem {
        id: String,
    },
    AmbiguousMid {
        mid: String,
    },
    AmbiguousSearchRelationName {
        name: String,
    },
    AmbiguousRelationName {
        name: String,
    },
    InvalidPage {
        message: String,
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDiscoveryReference => f.write_str("invalid or stale discovery handle; the document may have changed or moved; search again to rediscover the node"),
            Self::MissingItem { id } => write!(f,"item '{id}' was not found"),
            Self::AmbiguousItem { id } => write!(f,"item ID '{id}' is ambiguous"),
            Self::AmbiguousMid { mid } => write!(f,"item MID '{mid}' is ambiguous"),
            Self::AmbiguousSearchRelationName { name } => write!(f,"ambiguous relation '{name}'; search accepts schema relations only; use schema:{name}"),
            Self::InvalidPage { message } => f.write_str(message),
            Self::MissingRelationTarget {
                source,
                relation,
                target,
            } => write!(
                f,
                "relation '{relation}' from '{source}' references missing item '{target}'"
            ),
            Self::AmbiguousRelationTarget {
                source,
                relation,
                target,
            } => write!(
                f,
                "relation '{relation}' from '{source}' references ambiguous item '{target}'"
            ),
            Self::AmbiguousRelationName { name } => write!(f, "ambiguous relation '{name}'; use schema:{name} or builtin:{name}"),
            Self::UnknownFlavour { name } => write!(f, "unknown flavour '{name}'"),
            Self::UnknownField { name } => write!(f, "unknown field '{name}'"),
            Self::UnknownRelation { name } => write!(f, "unknown relation '{name}'"),
            Self::InvalidPath { path } => write!(
                f,
                "path filter must be a project-relative path: '{}'",
                path.display()
            ),
        }
    }
}
impl std::error::Error for QueryError {}

// @mara implements REQ-ITEM-LIST
// @mara implements DES-ITEM-LIST
pub fn list_items(
    corpus: &Corpus,
    schema: &Schema,
    filters: &ItemFilters,
) -> Result<ItemCollectionResult, QueryError> {
    page::filtered_page(corpus, schema, filters)
}

pub(crate) fn filtered_items<'a>(
    corpus: &'a Corpus,
    schema: &Schema,
    filters: &ItemFilters,
) -> Result<Vec<&'a Item>, QueryError> {
    validate_flavours(schema, &filters.flavours)?;
    validate_relations(schema, &filters.relations)?;
    validate_fields(schema, &filters.fields)?;
    let paths = normalized_paths(&filters.paths)?;
    let fields = grouped_fields(&filters.fields);
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
                item.relations().iter().any(|relation| {
                    schema
                        .resolve_relation(name)
                        .is_some_and(|(canonical, _, _)| relation.canonical == canonical)
                })
            })
        })
        .filter(|item| matches_fields(item, &fields));
    Ok(items.collect())
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
        .find(|name| schema.resolve_relation(name).is_none())
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

fn matches_name(names: &[String], candidate: &str) -> bool {
    names.is_empty() || names.iter().any(|name| name == candidate)
}

fn matches_name_filter(names: &[String], predicate: impl Fn(&str) -> bool) -> bool {
    names.is_empty() || names.iter().any(|name| predicate(name))
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RelationDirection {
    Incoming,
    Outgoing,
    Symmetric,
}

impl RelationDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Outgoing => "outgoing",
            Self::Symmetric => "symmetric",
        }
    }
}

// @mara implements DES-DETERMINISTIC-KEYWORD-SEARCH
fn rank_fields<'a>(
    fields: impl IntoIterator<Item = (&'a str, usize, bool)>,
    query: &BTreeSet<String>,
) -> Option<(bool, usize)> {
    let mut exact_words = BTreeMap::<String, usize>::new();
    let mut fuzzy_words = BTreeMap::<String, usize>::new();
    for (value, weight, fuzzy) in fields {
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

pub(crate) fn resolve_item<'a>(corpus: &'a Corpus, id: &str) -> Result<&'a Item, QueryError> {
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
