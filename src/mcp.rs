use std::path::PathBuf;

use mara::{
    FieldValue, ItemCollectionResult, ItemCreateParams, ItemCreationResult, ItemFilterParams,
    ItemGetParams, ItemGetResult, ItemMove, ItemMoveParams, ItemRelatedParams, ItemSearchParams,
    ItemUpdate, ItemUpdateParams, OperationContext, ProjectInitializationResult,
    ProjectMidBackfillResult, RelatedItemsResult, RelationDirection, RelationMutationResult,
    RelationParams, SchemaGetResult, SchemaKind, SchemaListResult, SchemaValidationResult,
    Template, TransactionRollbackResult, ValidationResult,
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
struct ItemUpdateToolParams {
    #[serde(default)]
    project: Option<PathBuf>,
    reference: String,
    #[serde(default)]
    title: Option<String>,
    /// Replace all values of each named custom field, repeating keys as needed.
    #[serde(default)]
    fields: Vec<FieldValue>,
    /// Clear optional custom fields; cannot also appear in fields.
    #[serde(default)]
    clear_fields: Vec<String>,
    /// Replacement body text; an empty string clears an optional body.
    #[serde(default)]
    body: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemRenameToolParams {
    #[serde(default)]
    project: Option<PathBuf>,
    reference: String,
    new_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemDeleteToolParams {
    #[serde(default)]
    project: Option<PathBuf>,
    reference: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemMoveToolParams {
    #[serde(default)]
    project: Option<PathBuf>,
    reference: String,
    file: PathBuf,
    #[serde(default)]
    line: Option<usize>,
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
struct ItemGetToolParams {
    #[serde(default)]
    project: Option<PathBuf>,
    id: String,
    /// Maximum combined relation entries per page, 1 through 100; defaults to 20.
    #[serde(default)]
    limit: Option<usize>,
    /// Continue consecutive body, metadata, and relation portions with unchanged options.
    #[serde(default)]
    cursor: Option<String>,
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
    /// Page size from 1 through 100; defaults to 20.
    limit: Option<usize>,
    #[serde(default)]
    /// Continue using next_cursor with the same query and options.
    cursor: Option<String>,
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
                cursor: self.cursor,
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
    /// Page size from 1 through 100; defaults to 20.
    limit: Option<usize>,
    #[serde(default)]
    /// Continue using next_cursor with the same query and options.
    cursor: Option<String>,
    #[serde(default)]
    /// Exact IDs or MIDs to select, intersected with other filters.
    ids: Vec<String>,
    #[serde(default)]
    /// Include up to three bounded, partial source excerpts per match.
    excerpts: bool,
}

impl ItemSearchToolParams {
    fn into_parts(self) -> (Option<PathBuf>, ItemSearchParams) {
        (
            self.project,
            ItemSearchParams {
                query: self.query,
                ids: self.ids,
                excerpts: self.excerpts,
                flavours: self.flavours,
                fields: self.fields,
                relations: self.relations,
                paths: self.paths,
                limit: self.limit,
                cursor: self.cursor,
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
    /// Page size from 1 through 100; defaults to 20.
    #[serde(default)]
    limit: Option<usize>,
    /// Continue using next_cursor with the same item and options.
    #[serde(default)]
    cursor: Option<String>,
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
                limit: self.limit,
                cursor: self.cursor,
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
        name = "project_mid_backfill",
        description = "Deliberately add generated MIDs to every legacy item that lacks one after a validation preflight."
    )]
    fn project_mid_backfill(
        &self,
        Parameters(params): Parameters<ProjectParams>,
    ) -> Result<Json<ProjectMidBackfillResult>, String> {
        self.for_project(params.project)?
            .project_mid_backfill()
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
        name = "item_update",
        description = "Partially update one item by exact MID or human ID. Requires a title, custom field replacement or clear, or body. Preserves identity, relations, and untouched source."
    )]
    fn item_update(
        &self,
        Parameters(params): Parameters<ItemUpdateToolParams>,
    ) -> Result<Json<ItemUpdate>, String> {
        self.for_project(params.project)?
            .item_update(ItemUpdateParams {
                reference: params.reference,
                title: params.title,
                fields: params.fields,
                clear_fields: params.clear_fields,
                body: params.body,
            })
            .map(Json)
    }

    #[tool(
        name = "item_rename",
        description = "Rename one human ID by exact MID or human ID, rewriting supported internal references across the valid corpus. Preserves the MID and unrelated source; retains no alias. Uses recoverable file replacement."
    )]
    fn item_rename(
        &self,
        Parameters(params): Parameters<ItemRenameToolParams>,
    ) -> Result<Json<mara::ItemRename>, String> {
        self.for_project(params.project)?
            .item_rename(&params.reference, &params.new_id)
            .map(Json)
    }

    #[tool(
        name = "item_delete",
        description = "Delete one item by exact MID or human ID after validating the project. Refuses surviving incoming relations or supported wiki mentions and reports every blocking source location. Keeps the containing document."
    )]
    fn item_delete(
        &self,
        Parameters(params): Parameters<ItemDeleteToolParams>,
    ) -> Result<Json<mara::ItemDeletion>, String> {
        self.for_project(params.project)?
            .item_delete(&params.reference)
            .map(Json)
    }

    #[tool(
        name = "item_move",
        description = "Move one item by exact MID or human ID to a project-relative document; optional line is one-based in the original destination."
    )]
    fn item_move(
        &self,
        Parameters(params): Parameters<ItemMoveToolParams>,
    ) -> Result<Json<ItemMove>, String> {
        self.for_project(params.project)?
            .item_move(ItemMoveParams {
                reference: params.reference,
                file: params.file,
                line: params.line,
            })
            .map(Json)
    }

    #[tool(
        name = "project_transaction_rollback",
        description = "Explicitly roll back a pending mutation journal to its original files. Stop other Mara writers first; later manual edits are never overwritten."
    )]
    fn project_transaction_rollback(
        &self,
        Parameters(params): Parameters<ProjectParams>,
    ) -> Result<Json<TransactionRollbackResult>, String> {
        self.for_project(params.project)?
            .project_transaction_rollback()
            .map(Json)
    }

    #[tool(
        name = "item_get",
        description = "Read an item in bounded consecutive portions: body, metadata values (including full title), then direct relations. Byte ranges are relative to each value; metadata indices preserve repeated keys. Follow next_cursor with unchanged id/limit until has_more is false; restart after source/schema changes."
    )]
    fn item_get(
        &self,
        Parameters(params): Parameters<ItemGetToolParams>,
    ) -> Result<Json<ItemGetResult>, String> {
        self.for_project(params.project)?
            .item_get(ItemGetParams {
                id: params.id,
                limit: params.limit,
                cursor: params.cursor,
            })
            .map(Json)
    }

    #[tool(
        name = "item_list",
        description = "List bounded item-summary pages in corpus order. Continue with next_cursor and unchanged options; restart after source changes."
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
        description = "Search every distinct query word, ranked by relevance before pagination. Items matching every word exactly precede any requiring typo tolerance; within each group each word's best ID/title match weighs 3, other fields 1, with corpus-order ties. ID/MID field values match only exact normalized words. Other text allows 0 edits at 1-3 characters, 1 at 4-7, and 2 at 8+; adjacent swaps count as one edit. Filters and selected ids stay exact. Optional excerpts include partial matching source passages. Continue bounded pages with next_cursor and unchanged options; restart after source changes."
    )]
    fn item_search(
        &self,
        Parameters(params): Parameters<ItemSearchToolParams>,
    ) -> Result<Json<ItemCollectionResult>, String> {
        let (project, params) = params.into_parts();
        self.for_project(project)?.item_search(params).map(Json)
    }

    #[tool(
        name = "item_related",
        description = "List bounded pages of direct incoming or outgoing relation entries with exact filters. Continue with next_cursor and unchanged item/options; restart after source changes. Neighbour bodies require item_get."
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
