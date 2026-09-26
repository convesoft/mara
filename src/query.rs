use crate::{Corpus, Item, Schema};
use schemars::JsonSchema;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fmt,
    path::{Component, Path, PathBuf},
};
mod page;
pub use page::ItemCollectionResult;
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
}

#[derive(Debug)]
pub enum QueryError {
    InvalidPage { message: String },
    UnknownFlavour { name: String },
    UnknownField { name: String },
    UnknownRelation { name: String },
    InvalidPath { path: PathBuf },
}
impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPage { message } => f.write_str(message),
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
    let items = corpus
        .items()
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
