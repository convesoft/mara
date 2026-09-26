use crate::{Corpus, FieldFilter, ItemCollectionResult, ItemFilters, list_items, load_corpus};
mod validation;
use crate::{
    FlavourDefinition, Project, RelationDefinition, Schema, Template, initialize_project,
    load_schema, resolve_project,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, env, path::PathBuf};
pub use validation::{
    ValidationDiagnostic, ValidationResult, ValidationScope, ValidationTargetKind,
};

#[derive(Debug, Clone)]
pub struct OperationContext {
    selected: Option<PathBuf>,
    current_directory: PathBuf,
}

impl OperationContext {
    pub fn from_environment(selected: Option<PathBuf>) -> Result<Self, String> {
        let current_directory = env::current_dir()
            .map_err(|error| format!("could not read current directory: {error}"))?;
        Ok(Self {
            selected,
            current_directory,
        })
    }

    // @mara implements DES-OPERATION-PROJECT-CONTEXT
    pub fn for_project(&self, requested: Option<PathBuf>) -> Result<Self, String> {
        if requested.as_ref().is_some_and(|path| !path.is_absolute()) {
            return Err("request project path must be absolute".into());
        }
        if self.selected.is_some() && requested.is_some() {
            return Err(
                "project cannot be selected per operation because the server started with --project"
                    .into(),
            );
        }
        Ok(Self {
            selected: requested.or_else(|| self.selected.clone()),
            current_directory: self.current_directory.clone(),
        })
    }

    pub fn project_initialize(
        &self,
        target: Option<PathBuf>,
        template: Template,
    ) -> Result<ProjectInitializationResult, String> {
        let operation = self.for_project(target)?;
        project_initialize(
            operation
                .selected
                .ok_or_else(|| {
                    "project init requires an absolute project path when the server is not bound with --project"
                        .to_string()
                })?,
            template,
        )
    }

    // @mara implements REQ-SCHEMA-DISCOVERY
    pub fn schema_get(
        &self,
        kind: Option<SchemaKind>,
        name: Option<String>,
    ) -> Result<SchemaGetResult, String> {
        let (_, schema) = self.load_project()?;
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

    // @mara implements REQ-SCHEMA-DISCOVERY
    pub fn schema_list(&self, kind: SchemaKind) -> Result<SchemaListResult, String> {
        let (_, schema) = self.load_project()?;
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
        Ok(SchemaListResult { kind, declarations })
    }

    pub fn item_list(&self, filters: ItemFilterParams) -> Result<ItemCollectionResult, String> {
        let (corpus, schema) = self.load_query_project()?;
        list_items(&corpus, &schema, &filters.into_domain()).map_err(|error| error.to_string())
    }

    fn load_query_project(&self) -> Result<(Corpus, Schema), String> {
        let (project, schema) = self.load_project()?;
        let corpus = load_corpus(&project, &schema).map_err(|error| error.to_string())?;
        Ok((corpus, schema))
    }

    fn load_project(&self) -> Result<(Project, Schema), String> {
        let project = resolve_project(self.selected.as_deref(), &self.current_directory)
            .map_err(|error| error.to_string())?;
        let schema = load_schema(&project).map_err(|error| error.to_string())?;
        Ok((project, schema))
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProjectInitializationResult {
    pub project: ProjectSummary,
    pub created: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProjectSummary {
    pub root: PathBuf,
    pub name: String,
    pub schema_path: PathBuf,
    pub content_patterns: Vec<String>,
}

pub fn project_initialize(
    target: PathBuf,
    template: Template,
) -> Result<ProjectInitializationResult, String> {
    let project = initialize_project(target, template).map_err(|error| error.to_string())?;
    Ok(ProjectInitializationResult {
        project: ProjectSummary {
            root: project.root().to_path_buf(),
            name: project.name().to_owned(),
            schema_path: project.schema_path().to_path_buf(),
            content_patterns: project.content_patterns().to_vec(),
        },
        created: vec![crate::PROJECT_FILE.into(), crate::SCHEMA_FILE.into()],
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SchemaKind {
    Flavour,
    Relation,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
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

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DeclarationSummary {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inverse: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symmetric: Option<bool>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SchemaListResult {
    pub kind: SchemaKind,
    pub declarations: Vec<DeclarationSummary>,
}

trait DescribedDeclaration {
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

fn declaration_summaries<T: DescribedDeclaration>(
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
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldValue {
    /// Schema-declared custom-field key; structural title/MID metadata and typed relations are excluded from authoring and retrieval field filters.
    pub key: String,
    /// Scalar text, including numbers and booleans as strings. Authoring trims surrounding whitespace and rejects line breaks; retrieval filters match exactly.
    pub value: String,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
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
    fn into_domain(self) -> ItemFilters {
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
