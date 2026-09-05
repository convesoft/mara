use std::path::PathBuf;

use mara::{
    FieldValue, InitialRelation, ItemCollectionResult, ItemCreateParams, ItemCreationResult,
    ItemFilterParams, ItemGetParams, ItemGetResult, ItemMove, ItemMoveParams, ItemRelatedParams,
    ItemSearchParams, ItemUpdate, ItemUpdateParams, OperationContext, ProjectInitializationResult,
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
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectValidateParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Exact documents or directory subtrees relative to the project root, combined with OR. No globs, absolute paths, or ..; omitted or empty selects the whole project. Example: ["packages/query/docs/"]. Selects reported diagnostics only; validity still covers the whole project.
    #[serde(default)]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectInitParams {
    /// Absolute destination directory; required unless the server was started with --project, in which case omit it. Creates a missing directory; rejects an existing Mara project.
    #[serde(default)]
    project: Option<PathBuf>,
    /// Initial schema: minimal (default) includes common flavours and relations; empty declares none.
    #[serde(default)]
    template: Template,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SchemaGetParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Declaration kind: flavour or relation. Supply together with name, or omit/null both for the complete effective schema.
    #[serde(default)]
    kind: Option<SchemaKind>,
    /// Exact schema declaration name; requires kind. Omit both name and kind (or set both to null) for the complete schema.
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SchemaListParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Kind of declarations to list: flavour or relation.
    kind: SchemaKind,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemCreateToolParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Schema-declared flavour; discover names with schema_list(kind="flavour").
    flavour: String,
    /// New unique human ID with the flavour's prefix (for example REQ-EXAMPLE), not a MID. Mara generates the MID.
    id: String,
    /// Destination project-relative *.mara.md path selected by project discovery; parent directory must exist. Creates the file if absent.
    file: PathBuf,
    /// Nonempty single-line title; surrounding whitespace is trimmed.
    title: String,
    /// Schema-declared custom fields only; excludes structural title/MID metadata and typed relations. Repeat keys only when schema-repeatable. Omitted or empty supplies none; required fields must be supplied. Use relations for initial edges or relation_add for later edits.
    #[serde(default)]
    fields: Vec<FieldValue>,
    /// Initial outgoing typed relations, created atomically with the item. Omitted or empty adds none; use relation_add/relation_remove for later edits.
    #[serde(default)]
    relations: Vec<InitialRelation>,
    /// Literal Markdown body (- is literal; no stdin). Omitted or null supplies no body; a missing or blank required body creates a scaffold.
    #[serde(default)]
    body: Option<String>,
    /// Insert before this one-based destination line; line_count + 1 means end of file. Omitted or null appends. Insertion inside another item is rejected.
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
                relations: self.relations,
                body: self.body,
                line: self.line,
            },
        )
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemUpdateToolParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Exact human ID or canonical MID (uppercase 26-character ULID without a prefix).
    reference: String,
    /// Replacement nonempty single-line title, trimmed; omitted or null leaves it unchanged.
    #[serde(default)]
    title: Option<String>,
    /// Replace all values of each named schema-declared custom field; repeat keys only when schema-repeatable. Excludes structural title/MID metadata and typed relations; use relation_add/relation_remove for edges. Omitted or empty leaves fields unchanged; an empty value is not a clear.
    #[serde(default)]
    fields: Vec<FieldValue>,
    /// Remove all values of named optional custom fields; cannot also set those keys in fields. Excludes title, MID, and typed relations. Omitted or empty clears nothing.
    #[serde(default)]
    clear_fields: Vec<String>,
    /// Replacement literal Markdown body (- is literal). Omitted or null leaves it unchanged; an empty string clears an optional body. Empty or whitespace-only replacement of a required body is rejected.
    #[serde(default)]
    body: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemRenameToolParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Exact human ID or canonical MID (uppercase 26-character ULID without a prefix).
    reference: String,
    /// New unique human ID with the item's flavour prefix. Preserves the MID; the old human ID is not kept as an alias.
    new_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemDeleteToolParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Exact human ID or canonical MID (uppercase 26-character ULID without a prefix).
    reference: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemMoveToolParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Exact human ID or canonical MID (uppercase 26-character ULID without a prefix).
    reference: String,
    /// Destination project-relative *.mara.md path selected by project discovery; parent directory must exist. Creates the file if absent.
    file: PathBuf,
    /// Insert before this one-based line in the original destination, including same-file moves. Omitted or null appends; line_count + 1 means end of file. Insertion inside an item is rejected.
    #[serde(default)]
    line: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemIdToolParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Exact human ID or canonical MID (uppercase 26-character ULID without a prefix).
    id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemGetToolParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Exact human ID or canonical MID (uppercase 26-character ULID without a prefix).
    id: String,
    /// Maximum combined direct relation entries per page, 1 through 100; omitted or null defaults to 20. Body and metadata portions are byte-bounded independently of this count.
    #[serde(default)]
    limit: Option<usize>,
    /// Opaque next_cursor from the previous response; keep all other inputs unchanged. Omit or null for the first page; restart after source/schema changes. Follow body, metadata, then relations until has_more is false.
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemFilterToolParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Exact flavour names, combined with OR and intersected with other filter categories. Omitted or empty selects all flavours.
    #[serde(default)]
    flavours: Vec<String>,
    /// Exact schema-declared custom-field key/value filters; excludes title/MID and typed relations. OR within one key, AND across keys and other filter categories. Omitted or empty adds no restriction.
    #[serde(default)]
    fields: Vec<FieldValue>,
    /// Exact authored outgoing relation names, combined with OR and intersected with other filters. Omitted or empty adds no restriction.
    #[serde(default)]
    relations: Vec<String>,
    /// Exact documents or directory subtrees relative to the project root, combined with OR. No globs, absolute paths, or ..; omitted or empty selects the whole project. Example: ["packages/query/docs/"].
    #[serde(default)]
    paths: Vec<PathBuf>,
    /// Maximum entries per page, 1 through 100; omitted or null defaults to 20. The response byte budget may return fewer.
    #[serde(default)]
    limit: Option<usize>,
    /// Opaque next_cursor from the previous response; keep all other inputs unchanged. Omit or null for the first page; restart after source/schema changes.
    #[serde(default)]
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
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Unicode case-insensitive words matched across ID, title, body, and metadata. Every distinct word must match; empty or punctuation-only text matches all items within the filters.
    query: String,
    /// Exact flavour names, combined with OR and intersected with other filter categories. Omitted or empty selects all flavours.
    #[serde(default)]
    flavours: Vec<String>,
    /// Exact schema-declared custom-field key/value filters; excludes title/MID and typed relations. OR within one key, AND across keys and other filter categories. Omitted or empty adds no restriction.
    #[serde(default)]
    fields: Vec<FieldValue>,
    /// Exact authored outgoing relation names, combined with OR and intersected with other filters. Omitted or empty adds no restriction.
    #[serde(default)]
    relations: Vec<String>,
    /// Exact documents or directory subtrees relative to the project root, combined with OR. No globs, absolute paths, or ..; omitted or empty selects the whole project. Example: ["packages/query/docs/"].
    #[serde(default)]
    paths: Vec<PathBuf>,
    /// Maximum entries per page, 1 through 100; omitted or null defaults to 20. The response byte budget may return fewer.
    #[serde(default)]
    limit: Option<usize>,
    /// Opaque next_cursor from the previous response; keep all other inputs unchanged. Omit or null for the first page; restart after source/schema changes.
    #[serde(default)]
    cursor: Option<String>,
    /// Exact human IDs or canonical MIDs (uppercase 26-character ULIDs), combined with OR and intersected with other filters. Omitted or empty adds no restriction.
    #[serde(default)]
    ids: Vec<String>,
    /// Include up to three bounded, partial source excerpts per match. Defaults to false; excerpts may skip content and do not replace item_get.
    #[serde(default)]
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
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Exact human ID or canonical MID (uppercase 26-character ULID without a prefix).
    id: String,
    /// Edge direction relative to the selected item: incoming or outgoing. Omitted or null includes both, outgoing first.
    #[serde(default)]
    direction: Option<RelationDirection>,
    /// Exact relation names, combined with OR and intersected with the neighbour flavour filter. Omitted or empty includes all.
    #[serde(default)]
    relations: Vec<String>,
    /// Exact neighbour flavour names, combined with OR. Omitted or empty includes all.
    #[serde(default)]
    flavours: Vec<String>,
    /// Maximum entries per page, 1 through 100; omitted or null defaults to 20. The response byte budget may return fewer. Counts relation entries, not unique neighbours.
    #[serde(default)]
    limit: Option<usize>,
    /// Opaque next_cursor from the previous response; keep all other inputs unchanged. Omit or null for the first page; restart after source/schema changes.
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
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Source item's exact human ID or canonical MID (uppercase 26-character ULID without a prefix).
    source: String,
    /// Schema-declared outgoing relation name; discover names with schema_list(kind="relation").
    relation: String,
    /// Target item's exact human ID or canonical MID (uppercase 26-character ULID without a prefix).
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
        description = "Validate the complete configured Mara project. Optional paths select reported diagnostics only; project/schema diagnostics always appear. Validity still covers the whole project, including omitted diagnostics counted in selection.omitted_diagnostics."
    )]
    fn project_validate(
        &self,
        Parameters(params): Parameters<ProjectValidateParams>,
    ) -> Result<Json<ValidationResult>, String> {
        self.for_project(params.project)?
            .project_validate(&params.paths)
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
        description = "Create one item and optional initial outgoing relations atomically in a project-relative Mara document; an omitted required body creates a scaffold."
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
        description = "Search every distinct query word with typo tolerance, ranked by relevance before pagination. Exact matches rank first; ID/title matches carry more weight. ID/MID field words and all filters stay exact. Optional excerpts are partial source passages. Follow next_cursor with unchanged inputs; restart after source/schema changes."
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
        description = "Add one schema-valid authored outgoing relation to its source item; rejects an existing edge."
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
        description = "Remove one existing authored outgoing relation from its source item; rejects a missing edge."
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
    instructions = "Structured Mara operations. Pass an absolute project path, or omit it for execution-directory discovery (project_init requires an explicit destination only when the server is unbound). When the server starts with --project, omit request-level project selection, including for project_init; overrides are rejected."
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
