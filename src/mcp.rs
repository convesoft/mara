use std::path::PathBuf;

use mara::{
    FieldValue, ItemCollectionResult, ItemCreateParams, ItemCreationResult, ItemFilterParams,
    ItemRelatedParams, OperationContext, ProjectInitializationResult, RelatedItemsResult,
    RelationDirection, RelationMutationResult, RelationParams, ResolvedItem, SchemaGetResult,
    SchemaKind, SchemaListResult, SchemaValidationResult, Template, ValidationResult,
};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::wrapper::{Json, Parameters},
    schemars::JsonSchema,
    tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;

#[derive(Clone)]
struct MaraMcp {
    operations: OperationContext,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectParams {
    #[serde(default)]
    project: Option<PathBuf>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectInitParams {
    #[serde(default)]
    project: Option<PathBuf>,
    #[serde(default)]
    template: Template,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SchemaGetParams {
    #[serde(default)]
    project: Option<PathBuf>,
    #[serde(default)]
    kind: Option<SchemaKind>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SchemaListParams {
    #[serde(default)]
    project: Option<PathBuf>,
    kind: SchemaKind,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemCreateToolParams {
    #[serde(default)]
    project: Option<PathBuf>,
    flavour: String,
    id: String,
    file: PathBuf,
    title: String,
    #[serde(default)]
    fields: Vec<FieldValue>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    line: Option<usize>,
}

impl ItemCreateToolParams {
    fn into_parts(self) -> (Option<PathBuf>, ItemCreateParams) {
        (
            self.project,
            ItemCreateParams {
                flavour: self.flavour,
                id: self.id,
                file: self.file,
                title: self.title,
                fields: self.fields,
                body: self.body,
                line: self.line,
            },
        )
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemIdToolParams {
    #[serde(default)]
    project: Option<PathBuf>,
    id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemFilterToolParams {
    #[serde(default)]
    project: Option<PathBuf>,
    #[serde(default)]
    flavours: Vec<String>,
    #[serde(default)]
    fields: Vec<FieldValue>,
    #[serde(default)]
    relations: Vec<String>,
    #[serde(default)]
    paths: Vec<PathBuf>,
    #[serde(default)]
    limit: Option<usize>,
}

impl ItemFilterToolParams {
    fn into_parts(self) -> (Option<PathBuf>, ItemFilterParams) {
        (
            self.project,
            ItemFilterParams {
                flavours: self.flavours,
                fields: self.fields,
                relations: self.relations,
                paths: self.paths,
                limit: self.limit,
            },
        )
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemSearchToolParams {
    #[serde(default)]
    project: Option<PathBuf>,
    query: String,
    #[serde(default)]
    flavours: Vec<String>,
    #[serde(default)]
    fields: Vec<FieldValue>,
    #[serde(default)]
    relations: Vec<String>,
    #[serde(default)]
    paths: Vec<PathBuf>,
    #[serde(default)]
    limit: Option<usize>,
}

impl ItemSearchToolParams {
    fn into_parts(self) -> (Option<PathBuf>, String, ItemFilterParams) {
        (
            self.project,
            self.query,
            ItemFilterParams {
                flavours: self.flavours,
                fields: self.fields,
                relations: self.relations,
                paths: self.paths,
                limit: self.limit,
            },
        )
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemRelatedToolParams {
    #[serde(default)]
    project: Option<PathBuf>,
    id: String,
    #[serde(default)]
    direction: Option<RelationDirection>,
    #[serde(default)]
    relations: Vec<String>,
    #[serde(default)]
    flavours: Vec<String>,
}

impl ItemRelatedToolParams {
    fn into_parts(self) -> (Option<PathBuf>, ItemRelatedParams) {
        (
            self.project,
            ItemRelatedParams {
                id: self.id,
                direction: self.direction,
                relations: self.relations,
                flavours: self.flavours,
            },
        )
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RelationToolParams {
    #[serde(default)]
    project: Option<PathBuf>,
    source: String,
    relation: String,
    target: String,
}

impl RelationToolParams {
    fn into_parts(self) -> (Option<PathBuf>, RelationParams) {
        (
            self.project,
            RelationParams {
                source: self.source,
                relation: self.relation,
                target: self.target,
            },
        )
    }
}

#[tool_router]
impl MaraMcp {
    fn for_project(&self, project: Option<PathBuf>) -> Result<OperationContext, String> {
        self.operations.for_project(project)
    }

    #[tool(
        name = "project_init",
        description = "Initialize a Mara project without overwriting existing content. Pass an absolute project path unless the server was started with --project."
    )]
    fn project_init(
        &self,
        Parameters(params): Parameters<ProjectInitParams>,
    ) -> Result<Json<ProjectInitializationResult>, String> {
        self.operations
            .project_initialize(params.project, params.template)
            .map(Json)
    }

    #[tool(
        name = "project_validate",
        description = "Validate the selected Mara project and return all independently discoverable diagnostics."
    )]
    fn project_validate(
        &self,
        Parameters(params): Parameters<ProjectParams>,
    ) -> Result<Json<ValidationResult>, String> {
        self.for_project(params.project)?
            .project_validate()
            .map(Json)
    }

    #[tool(
        name = "schema_get",
        description = "Get the complete effective schema, or one named flavour or relation declaration."
    )]
    fn schema_get(
        &self,
        Parameters(params): Parameters<SchemaGetParams>,
    ) -> Result<Json<SchemaGetResult>, String> {
        self.for_project(params.project)?
            .schema_get(params.kind, params.name)
            .map(Json)
    }

    #[tool(
        name = "schema_list",
        description = "List all flavour or relation declarations in the effective schema."
    )]
    fn schema_list(
        &self,
        Parameters(params): Parameters<SchemaListParams>,
    ) -> Result<Json<SchemaListResult>, String> {
        self.for_project(params.project)?
            .schema_list(params.kind)
            .map(Json)
    }

    #[tool(
        name = "schema_validate",
        description = "Validate the selected project's configured Mara schema."
    )]
    fn schema_validate(
        &self,
        Parameters(params): Parameters<ProjectParams>,
    ) -> Result<Json<SchemaValidationResult>, String> {
        self.for_project(params.project)?
            .schema_validate()
            .map(Json)
    }

    #[tool(
        name = "item_create",
        description = "Create one schema-valid item in a project-relative Mara document."
    )]
    fn item_create(
        &self,
        Parameters(params): Parameters<ItemCreateToolParams>,
    ) -> Result<Json<ItemCreationResult>, String> {
        let (project, params) = params.into_parts();
        self.for_project(project)?.item_create(params).map(Json)
    }

    #[tool(
        name = "item_get",
        description = "Get one complete item with source, metadata, body, and direct relations."
    )]
    fn item_get(
        &self,
        Parameters(params): Parameters<ItemIdToolParams>,
    ) -> Result<Json<ResolvedItem>, String> {
        self.for_project(params.project)?
            .item_get(&params.id)
            .map(Json)
    }

    #[tool(
        name = "item_list",
        description = "List deterministic compact item summaries using exact filters."
    )]
    fn item_list(
        &self,
        Parameters(params): Parameters<ItemFilterToolParams>,
    ) -> Result<Json<ItemCollectionResult>, String> {
        let (project, params) = params.into_parts();
        self.for_project(project)?.item_list(params).map(Json)
    }

    #[tool(
        name = "item_search",
        description = "Search item text and return deterministic compact summaries with optional exact filters."
    )]
    fn item_search(
        &self,
        Parameters(params): Parameters<ItemSearchToolParams>,
    ) -> Result<Json<ItemCollectionResult>, String> {
        let (project, query, filters) = params.into_parts();
        self.for_project(project)?
            .item_search(&query, filters)
            .map(Json)
    }

    #[tool(
        name = "item_related",
        description = "List direct incoming or outgoing neighbours of one item with optional exact filters."
    )]
    fn item_related(
        &self,
        Parameters(params): Parameters<ItemRelatedToolParams>,
    ) -> Result<Json<RelatedItemsResult>, String> {
        let (project, params) = params.into_parts();
        self.for_project(project)?.item_related(params).map(Json)
    }

    #[tool(
        name = "item_validate",
        description = "Validate one item and return all independently discoverable diagnostics that apply to it."
    )]
    fn item_validate(
        &self,
        Parameters(params): Parameters<ItemIdToolParams>,
    ) -> Result<Json<ValidationResult>, String> {
        self.for_project(params.project)?
            .item_validate(&params.id)
            .map(Json)
    }

    #[tool(
        name = "relation_add",
        description = "Add one schema-valid authored relation to its source item."
    )]
    fn relation_add(
        &self,
        Parameters(params): Parameters<RelationToolParams>,
    ) -> Result<Json<RelationMutationResult>, String> {
        let (project, params) = params.into_parts();
        self.for_project(project)?.relation_add(params).map(Json)
    }

    #[tool(
        name = "relation_remove",
        description = "Remove one existing authored relation from its source item."
    )]
    fn relation_remove(
        &self,
        Parameters(params): Parameters<RelationToolParams>,
    ) -> Result<Json<RelationMutationResult>, String> {
        let (project, params) = params.into_parts();
        self.for_project(project)?.relation_remove(params).map(Json)
    }
}

#[tool_handler(
    name = "mara",
    instructions = "Structured Mara operations. Pass an absolute project path to each project-bound tool, omit it for execution-directory discovery, or start the server with --project to bind all calls."
)]
impl ServerHandler for MaraMcp {}

pub fn run(selected: Option<PathBuf>) -> Result<(), String> {
    let operations = OperationContext::from_environment(selected)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("could not start MCP runtime: {error}"))?;
    runtime.block_on(async move {
        let service = MaraMcp { operations }
            .serve(stdio())
            .await
            .map_err(|error| format!("could not start MCP server: {error}"))?;
        service
            .waiting()
            .await
            .map_err(|error| format!("MCP server failed: {error}"))?;
        Ok(())
    })
}
