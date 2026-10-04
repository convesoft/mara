use crate::*;
#[derive(Debug, Clone, Copy, Serialize, JsonSchema, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RelationAction {
    Added,
    Removed,
}

impl RelationAction {
    pub const fn past_tense(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema, serde::Deserialize)]
pub struct RelationMutationResult {
    pub format_version: u8,
    pub action: RelationAction,
    pub scope: String,
    pub edge: crate::RelationEdge,
    pub changed_occurrences: usize,
    pub remaining_occurrences: usize,
    pub edge_exists: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ItemUpdateParams {
    pub reference: String,
    #[serde(default)]
    pub title: Option<String>,
    /// Replace all values of each named custom field; repeat keys for repeated values.
    #[serde(default)]
    pub fields: Vec<FieldValue>,
    /// Remove all values of optional custom fields.
    #[serde(default)]
    pub clear_fields: Vec<String>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ItemMoveParams {
    pub reference: String,
    pub file: PathBuf,
    #[serde(default)]
    pub line: Option<usize>,
}

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SchemaKind {
    Flavour,
    Relation,
}

#[derive(Debug, Clone, Serialize, JsonSchema, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SchemaGetResult {
    Schema {
        schema: Box<Schema>,
    },
    Flavour {
        name: String,
        definition: FlavourDefinition,
    },
    Relation {
        name: String,
        requested_name: String,
        inverse: bool,
        definition: RelationDefinition,
    },
}

#[derive(Debug, Clone, Serialize, JsonSchema, serde::Deserialize)]
pub struct DeclarationSummary {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inverse: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symmetric: Option<bool>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, serde::Deserialize)]
pub struct SchemaListResult {
    pub kind: SchemaKind,
    pub declarations: Vec<DeclarationSummary>,
}

pub trait DescribedDeclaration {
    fn description(&self) -> &str;
}

impl DescribedDeclaration for FlavourDefinition {
    fn description(&self) -> &str {
        self.description()
    }
}

impl DescribedDeclaration for RelationDefinition {
    fn description(&self) -> &str {
        self.description()
    }
}

pub fn declaration_summaries<T: DescribedDeclaration>(
    declarations: &BTreeMap<String, T>,
) -> Vec<DeclarationSummary> {
    declarations
        .iter()
        .map(|(name, definition)| DeclarationSummary {
            name: name.clone(),
            description: definition.description().to_owned(),
            inverse: None,
            symmetric: None,
        })
        .collect()
}
#[derive(Debug, Clone, Deserialize, JsonSchema, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldValue {
    /// Schema-declared custom-field key; structural title/MID metadata and typed relations are excluded from authoring and retrieval field filters.
    pub key: String,
    /// Scalar text, including numbers and booleans as strings. Authoring trims surrounding whitespace and rejects line breaks; retrieval filters match exactly.
    pub value: String,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ItemFilterParams {
    #[serde(default)]
    pub flavours: Vec<String>,
    #[serde(default)]
    pub fields: Vec<FieldValue>,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub cursor: Option<String>,
}

impl ItemFilterParams {
    pub fn into_domain(self) -> ItemFilters {
        ItemFilters::new(
            self.flavours,
            self.fields
                .into_iter()
                .map(|field| FieldFilter::new(field.key, field.value))
                .collect(),
            self.relations,
            self.paths,
            self.limit,
        )
        .with_cursor(self.cursor)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SearchParams {
    pub query: String,
    #[serde(default)]
    pub flavours: Vec<String>,
    #[serde(default)]
    pub fields: Vec<FieldValue>,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub ids: Vec<String>,
}

impl SearchParams {
    pub fn into_parts(self) -> (String, ItemFilterParams, Vec<String>) {
        (
            self.query,
            ItemFilterParams {
                flavours: self.flavours,
                fields: self.fields,
                relations: self.relations,
                paths: self.paths,
                limit: self.limit,
                cursor: self.cursor,
            },
            self.ids,
        )
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct GetParams {
    pub reference: String,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct RelatedParams {
    pub reference: String,
    #[serde(default)]
    pub direction: Option<RelationDirection>,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub flavours: Vec<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct RelationParams {
    pub source: String,
    pub relation: String,
    pub target: String,
}

pub fn schema_get(
    schema: Schema,
    kind: Option<SchemaKind>,
    name: Option<String>,
) -> Result<SchemaGetResult, String> {
    match (kind, name) {
        (None, None) => Ok(SchemaGetResult::Schema {
            schema: Box::new(schema),
        }),
        (Some(SchemaKind::Flavour), Some(name)) => {
            let definition = schema
                .flavours()
                .get(&name)
                .ok_or_else(|| format!("unknown flavour '{name}'"))?
                .clone();
            Ok(SchemaGetResult::Flavour { name, definition })
        }
        (Some(SchemaKind::Relation), Some(name)) => {
            let (canonical, definition, inverse) = schema
                .resolve_relation(&name)
                .ok_or_else(|| format!("unknown relation '{name}'"))?;
            Ok(SchemaGetResult::Relation {
                name: canonical.to_owned(),
                requested_name: name,
                inverse,
                definition: definition.clone(),
            })
        }
        _ => Err("schema get requires both KIND and NAME, or neither".into()),
    }
}

pub fn schema_list(schema: &Schema, kind: SchemaKind) -> SchemaListResult {
    let declarations = match kind {
        SchemaKind::Flavour => declaration_summaries(schema.flavours()),
        SchemaKind::Relation => schema
            .relations()
            .iter()
            .map(|(name, definition)| DeclarationSummary {
                name: name.clone(),
                description: definition.description.clone(),
                inverse: definition.inverse.clone(),
                symmetric: Some(definition.symmetric),
            })
            .collect(),
    };
    SchemaListResult { kind, declarations }
}
