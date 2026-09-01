use std::{collections::BTreeMap, env, path::PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    Corpus, Diagnostic, FieldFilter, FlavourDefinition, ItemCreationRequest, ItemFilters,
    ItemSummary, Project, RelatedFilters, RelatedItem, RelationDefinition, RelationDirection,
    ResolvedItem, Schema, Template, add_relation, create_item, get_item, initialize_project,
    list_items, load_corpus, load_corpus_for_validation, load_corpus_syntax_for_validation,
    load_schema, load_schema_for_validation, related_items, remove_relation, resolve_project,
    resolve_project_for_validation, search_items, validate_corpus, validate_corpus_independent,
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

    pub fn bound(selected: Option<PathBuf>) -> Result<Self, String> {
        let context = Self::from_environment(selected)?;
        let validation =
            resolve_project_for_validation(context.selected.as_deref(), &context.current_directory)
                .map_err(|error| error.to_string())?;
        let (project, _, _) = validation.into_parts();
        Ok(Self {
            selected: Some(project.root().to_path_buf()),
            current_directory: project.root().to_path_buf(),
        })
    }

    pub fn project_validate(&self) -> Result<ValidationResult, String> {
        self.validate(None)
    }

    pub fn item_validate(&self, id: &str) -> Result<ValidationResult, String> {
        self.validate(Some(id))
    }

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
                let definition = schema
                    .relations()
                    .get(&name)
                    .ok_or_else(|| format!("unknown relation '{name}'"))?
                    .clone();
                Ok(SchemaGetResult::Relation { name, definition })
            }
            _ => Err("schema get requires both KIND and NAME, or neither".into()),
        }
    }

    pub fn schema_list(&self, kind: SchemaKind) -> Result<SchemaListResult, String> {
        let (_, schema) = self.load_project()?;
        let declarations = match kind {
            SchemaKind::Flavour => declaration_summaries(schema.flavours()),
            SchemaKind::Relation => declaration_summaries(schema.relations()),
        };
        Ok(SchemaListResult { kind, declarations })
    }

    pub fn schema_validate(&self) -> Result<SchemaValidationResult, String> {
        let (project, schema) = self.load_project()?;
        Ok(SchemaValidationResult {
            valid: true,
            path: project.schema_path().to_path_buf(),
            flavours: schema.flavours().len(),
            relations: schema.relations().len(),
        })
    }

    pub fn item_create(&self, request: ItemCreateParams) -> Result<ItemCreationResult, String> {
        let (project, schema) = self.load_project()?;
        let id = request.id.clone();
        let created = create_item(
            &project,
            &schema,
            ItemCreationRequest {
                flavour: request.flavour,
                id: request.id,
                file: request.file,
                title: request.title,
                fields: request
                    .fields
                    .into_iter()
                    .map(|field| (field.key, field.value))
                    .collect(),
                body: request.body,
                line: request.line,
            },
        )
        .map_err(|error| error.to_string())?;
        let complete = created.is_complete();
        Ok(ItemCreationResult {
            id,
            path: created.path().to_path_buf(),
            line: created.line(),
            complete,
            missing: if complete {
                Vec::new()
            } else {
                vec!["body".into()]
            },
        })
    }

    pub fn item_get(&self, id: &str) -> Result<ResolvedItem, String> {
        let (corpus, _) = self.load_query_project()?;
        get_item(&corpus, id).map_err(|error| error.to_string())
    }

    pub fn item_list(&self, filters: ItemFilterParams) -> Result<ItemCollectionResult, String> {
        let (corpus, schema) = self.load_query_project()?;
        let items = list_items(&corpus, &schema, &filters.into_domain())
            .map_err(|error| error.to_string())?;
        Ok(ItemCollectionResult { items })
    }

    pub fn item_search(
        &self,
        query: &str,
        filters: ItemFilterParams,
    ) -> Result<ItemCollectionResult, String> {
        let (corpus, schema) = self.load_query_project()?;
        let items = search_items(&corpus, &schema, query, &filters.into_domain())
            .map_err(|error| error.to_string())?;
        Ok(ItemCollectionResult { items })
    }

    pub fn item_related(&self, params: ItemRelatedParams) -> Result<RelatedItemsResult, String> {
        let (corpus, schema) = self.load_query_project()?;
        let filters = RelatedFilters::new(params.direction, params.relations, params.flavours);
        let items = related_items(&corpus, &schema, &params.id, &filters)
            .map_err(|error| error.to_string())?;
        Ok(RelatedItemsResult { items })
    }

    pub fn relation_add(&self, params: RelationParams) -> Result<RelationMutationResult, String> {
        self.mutate_relation(RelationAction::Added, params)
    }

    pub fn relation_remove(
        &self,
        params: RelationParams,
    ) -> Result<RelationMutationResult, String> {
        self.mutate_relation(RelationAction::Removed, params)
    }

    fn load_project(&self) -> Result<(Project, Schema), String> {
        let project = resolve_project(self.selected.as_deref(), &self.current_directory)
            .map_err(|error| error.to_string())?;
        let schema = load_schema(&project).map_err(|error| error.to_string())?;
        Ok((project, schema))
    }

    fn load_query_project(&self) -> Result<(Corpus, Schema), String> {
        let (project, schema) = self.load_project()?;
        let corpus = load_corpus(&project, &schema).map_err(|error| error.to_string())?;
        Ok((corpus, schema))
    }

    fn mutate_relation(
        &self,
        action: RelationAction,
        params: RelationParams,
    ) -> Result<RelationMutationResult, String> {
        let (project, schema) = self.load_project()?;
        let mutation = match action {
            RelationAction::Added => add_relation(
                &project,
                &schema,
                &params.source,
                &params.relation,
                &params.target,
            ),
            RelationAction::Removed => remove_relation(
                &project,
                &schema,
                &params.source,
                &params.relation,
                &params.target,
            ),
        }
        .map_err(|error| error.to_string())?;
        Ok(RelationMutationResult {
            action,
            source: mutation.source().to_owned(),
            relation: mutation.relation().to_owned(),
            target: mutation.target().to_owned(),
            path: mutation.path().to_path_buf(),
        })
    }

    fn validate(&self, selected_item: Option<&str>) -> Result<ValidationResult, String> {
        let context = self.load_validation_context()?;
        Ok(collect_validation_result(context, selected_item))
    }

    fn load_validation_context(&self) -> Result<ValidationContext, String> {
        let project_validation =
            resolve_project_for_validation(self.selected.as_deref(), &self.current_directory)
                .map_err(|error| error.to_string())?;
        let (project, project_errors, schema_available) = project_validation.into_parts();
        let project_diagnostics = project_errors
            .into_iter()
            .map(|message| ValidationDiagnostic {
                scope: ValidationScope::Project,
                path: Some(project.root().join(crate::PROJECT_FILE)),
                line: None,
                message,
            })
            .collect();
        let (schema, schema_diagnostics, corpus, diagnostics) = if schema_available {
            match load_schema_for_validation(&project) {
                Ok((schema, errors)) if schema.format_version() == 1 => {
                    let schema_diagnostics = errors
                        .into_iter()
                        .map(|message| ValidationDiagnostic {
                            scope: ValidationScope::Schema,
                            path: Some(project.schema_path().to_path_buf()),
                            line: None,
                            message,
                        })
                        .collect();
                    let (corpus, diagnostics) = load_corpus_for_validation(&project, &schema)
                        .map_err(|error| error.to_string())?;
                    (Some(schema), schema_diagnostics, corpus, diagnostics)
                }
                Ok((_, errors)) => {
                    let schema_diagnostics = errors
                        .into_iter()
                        .map(|message| ValidationDiagnostic {
                            scope: ValidationScope::Schema,
                            path: Some(project.schema_path().to_path_buf()),
                            line: None,
                            message,
                        })
                        .collect();
                    let (corpus, diagnostics) = load_corpus_syntax_for_validation(&project)
                        .map_err(|error| error.to_string())?;
                    (None, schema_diagnostics, corpus, diagnostics)
                }
                Err(error) => {
                    let schema_diagnostic = ValidationDiagnostic {
                        scope: ValidationScope::Schema,
                        path: Some(project.schema_path().to_path_buf()),
                        line: None,
                        message: error.to_string(),
                    };
                    let (corpus, diagnostics) = load_corpus_syntax_for_validation(&project)
                        .map_err(|error| error.to_string())?;
                    (None, vec![schema_diagnostic], corpus, diagnostics)
                }
            }
        } else {
            let (corpus, diagnostics) =
                load_corpus_syntax_for_validation(&project).map_err(|error| error.to_string())?;
            (None, Vec::new(), corpus, diagnostics)
        };
        Ok(ValidationContext {
            project,
            corpus,
            schema,
            diagnostics,
            project_diagnostics,
            schema_diagnostics,
        })
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
        definition: RelationDefinition,
    },
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DeclarationSummary {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SchemaListResult {
    pub kind: SchemaKind,
    pub declarations: Vec<DeclarationSummary>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SchemaValidationResult {
    pub valid: bool,
    pub path: PathBuf,
    pub flavours: usize,
    pub relations: usize,
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
        })
        .collect()
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct FieldValue {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ItemCreateParams {
    pub flavour: String,
    pub id: String,
    pub file: PathBuf,
    pub title: String,
    #[serde(default)]
    pub fields: Vec<FieldValue>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ItemCreationResult {
    pub id: String,
    pub path: PathBuf,
    pub line: usize,
    pub complete: bool,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
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
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ItemSearchParams {
    pub query: String,
    #[serde(flatten)]
    pub filters: ItemFilterParams,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ItemCollectionResult {
    pub items: Vec<ItemSummary>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ItemIdParams {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ItemRelatedParams {
    pub id: String,
    #[serde(default)]
    pub direction: Option<RelationDirection>,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub flavours: Vec<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RelatedItemsResult {
    pub items: Vec<RelatedItem>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct RelationParams {
    pub source: String,
    pub relation: String,
    pub target: String,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
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

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RelationMutationResult {
    pub action: RelationAction,
    pub source: String,
    pub relation: String,
    pub target: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ValidationTargetKind {
    Project,
    Item,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidationTarget {
    pub kind: ValidationTargetKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ValidationScope {
    Project,
    Schema,
    Item,
    Document,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidationDiagnostic {
    pub scope: ValidationScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidationResult {
    pub valid: bool,
    pub project: PathBuf,
    pub target: ValidationTarget,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

struct ValidationContext {
    project: Project,
    corpus: Corpus,
    schema: Option<Schema>,
    diagnostics: Vec<Diagnostic>,
    project_diagnostics: Vec<ValidationDiagnostic>,
    schema_diagnostics: Vec<ValidationDiagnostic>,
}

fn collect_validation_result(
    mut context: ValidationContext,
    selected: Option<&str>,
) -> ValidationResult {
    match &context.schema {
        Some(schema) => context
            .diagnostics
            .extend(validate_corpus(&context.corpus, schema)),
        None => context
            .diagnostics
            .extend(validate_corpus_independent(&context.corpus)),
    }
    let selected_item_missing = selected.is_some_and(|id| {
        context.corpus.is_complete()
            && !context.corpus.items().any(|item| item.id() == id)
            && !context
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.applies_to_item(id))
    });
    let selected_item_context_incomplete = selected.is_some() && !context.corpus.is_complete();
    let selected_diagnostics = context
        .diagnostics
        .into_iter()
        .filter(|diagnostic| {
            selected.is_none_or(|id| {
                diagnostic.applies_to_item(id)
                    || context
                        .corpus
                        .items()
                        .filter(|item| item.id() == id)
                        .any(|item| {
                            diagnostic.source().span().start_byte()
                                >= item.source().span().start_byte()
                                && diagnostic.source().span().end_byte()
                                    <= item.source().span().end_byte()
                                && diagnostic.source().path() == item.source().path()
                        })
            })
        })
        .map(|diagnostic| ValidationDiagnostic {
            scope: ValidationScope::Document,
            path: Some(diagnostic.source().path().to_path_buf()),
            line: Some(diagnostic.source().span().start_line()),
            message: diagnostic.message().to_owned(),
        });
    let mut diagnostics = context.project_diagnostics;
    diagnostics.extend(context.schema_diagnostics);
    if selected_item_context_incomplete {
        diagnostics.push(ValidationDiagnostic {
            scope: ValidationScope::Item,
            path: None,
            line: None,
            message: format!(
                "item '{}' could not be fully validated because the project corpus is incomplete",
                selected.expect("incomplete selected-item validation has an item ID")
            ),
        });
    }
    if selected_item_missing {
        diagnostics.push(ValidationDiagnostic {
            scope: ValidationScope::Item,
            path: None,
            line: None,
            message: format!(
                "item '{}' was not found",
                selected.expect("missing selected item has an item ID")
            ),
        });
    }
    diagnostics.extend(selected_diagnostics);
    ValidationResult {
        valid: diagnostics.is_empty(),
        project: context.project.root().to_path_buf(),
        target: ValidationTarget {
            kind: if selected.is_some() {
                ValidationTargetKind::Item
            } else {
                ValidationTargetKind::Project
            },
            id: selected.map(str::to_owned),
        },
        diagnostics,
    }
}
