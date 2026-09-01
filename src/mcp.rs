use std::path::PathBuf;

use mara::{
    ItemCollectionResult, ItemCreateParams, ItemCreationResult, ItemFilterParams, ItemIdParams,
    ItemRelatedParams, ItemSearchParams, OperationContext, RelatedItemsResult,
    RelationMutationResult, RelationParams, ResolvedItem, SchemaGetResult, SchemaKind,
    SchemaListResult, SchemaValidationResult, ValidationResult,
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
struct SchemaGetParams {
    #[serde(default)]
    kind: Option<SchemaKind>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SchemaListParams {
    kind: SchemaKind,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EmptyParams {}

#[tool_router]
impl MaraMcp {
    #[tool(
        name = "project_validate",
        description = "Validate the bound Mara project and return all independently discoverable diagnostics."
    )]
    fn project_validate(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<Json<ValidationResult>, String> {
        self.operations.project_validate().map(Json)
    }

    #[tool(
        name = "schema_get",
        description = "Get the complete effective schema, or one named flavour or relation declaration."
    )]
    fn schema_get(
        &self,
        Parameters(params): Parameters<SchemaGetParams>,
    ) -> Result<Json<SchemaGetResult>, String> {
        self.operations
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
        self.operations.schema_list(params.kind).map(Json)
    }

    #[tool(
        name = "schema_validate",
        description = "Validate the bound project's configured Mara schema."
    )]
    fn schema_validate(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<Json<SchemaValidationResult>, String> {
        self.operations.schema_validate().map(Json)
    }

    #[tool(
        name = "item_create",
        description = "Create one schema-valid item in a project-relative Mara document."
    )]
    fn item_create(
        &self,
        Parameters(params): Parameters<ItemCreateParams>,
    ) -> Result<Json<ItemCreationResult>, String> {
        self.operations.item_create(params).map(Json)
    }

    #[tool(
        name = "item_get",
        description = "Get one complete item with source, metadata, body, and direct relations."
    )]
    fn item_get(
        &self,
        Parameters(params): Parameters<ItemIdParams>,
    ) -> Result<Json<ResolvedItem>, String> {
        self.operations.item_get(&params.id).map(Json)
    }

    #[tool(
        name = "item_list",
        description = "List deterministic compact item summaries using exact filters."
    )]
    fn item_list(
        &self,
        Parameters(params): Parameters<ItemFilterParams>,
    ) -> Result<Json<ItemCollectionResult>, String> {
        self.operations.item_list(params).map(Json)
    }

    #[tool(
        name = "item_search",
        description = "Search item text and return deterministic compact summaries with optional exact filters."
    )]
    fn item_search(
        &self,
        Parameters(params): Parameters<ItemSearchParams>,
    ) -> Result<Json<ItemCollectionResult>, String> {
        let (query, filters) = params.into_parts();
        self.operations.item_search(&query, filters).map(Json)
    }

    #[tool(
        name = "item_related",
        description = "List direct incoming or outgoing neighbours of one item with optional exact filters."
    )]
    fn item_related(
        &self,
        Parameters(params): Parameters<ItemRelatedParams>,
    ) -> Result<Json<RelatedItemsResult>, String> {
        self.operations.item_related(params).map(Json)
    }

    #[tool(
        name = "item_validate",
        description = "Validate one item and return all independently discoverable diagnostics that apply to it."
    )]
    fn item_validate(
        &self,
        Parameters(params): Parameters<ItemIdParams>,
    ) -> Result<Json<ValidationResult>, String> {
        self.operations.item_validate(&params.id).map(Json)
    }

    #[tool(
        name = "relation_add",
        description = "Add one schema-valid authored relation to its source item."
    )]
    fn relation_add(
        &self,
        Parameters(params): Parameters<RelationParams>,
    ) -> Result<Json<RelationMutationResult>, String> {
        self.operations.relation_add(params).map(Json)
    }

    #[tool(
        name = "relation_remove",
        description = "Remove one existing authored relation from its source item."
    )]
    fn relation_remove(
        &self,
        Parameters(params): Parameters<RelationParams>,
    ) -> Result<Json<RelationMutationResult>, String> {
        self.operations.relation_remove(params).map(Json)
    }
}

#[tool_handler(
    name = "mara",
    instructions = "Structured operations for the Mara project bound when this server started."
)]
impl ServerHandler for MaraMcp {}

pub fn run(selected: Option<PathBuf>) -> Result<(), String> {
    let operations = OperationContext::bound(selected)?;
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
