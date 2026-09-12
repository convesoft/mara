use std::path::PathBuf;

use mara::{
    FieldValue, GetParams, GetResult, InitialRelation, ItemCollectionResult, ItemCreateParams,
    ItemCreationResult, ItemFilterParams, ItemMove, ItemMoveParams, ItemUpdate, ItemUpdateParams,
    OperationContext, ProjectInitializationResult, ProjectMidBackfillResult, RelatedParams,
    RelatedResult, RelationDirection, RelationMutationResult, RelationParams, SchemaGetResult,
    SchemaKind, SchemaListResult, SchemaValidationResult, SearchParams, Template,
    TransactionRollbackResult, ValidationResult,
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
    /// Exact documents or directory subtrees relative to the project root, combined with OR. No globs, absolute paths, .., empty path elements, . or ./; omit paths or use [] to select the whole project. Example: ["packages/query/docs/"]. Selects reported diagnostics only; validity still covers the whole project.
    #[serde(default)]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectInitParams {
    /// Absolute destination directory; required unless the server was started with --project, in which case omit it. Creates a missing directory; rejects an existing Mara project.
    #[serde(default)]
    project: Option<PathBuf>,
    /// Initial schema: minimal (default) includes common flavours and relations; empty declares none; engineering adds the full engineering vocabulary and traceability relations.
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
    /// Schema-declared flavour; discover names with schema_list(kind="flavour"), then read selection guidance with schema_get(kind="flavour", name=NAME).
    flavour: String,
    /// New unique human ID with the flavour's prefix (for example REQ-EXAMPLE), not a MID. Mara generates the MID.
    id: String,
    /// Destination project-relative *.mara.md path selected by project discovery; parent directory must exist. Creates the file if absent; no absolute paths or .. components.
    file: PathBuf,
    /// Single-line title; surrounding whitespace is trimmed. Empty or whitespace-only titles and line breaks are rejected.
    title: String,
    /// Schema-declared custom fields only; excludes structural title/MID metadata and typed relations. Repeat keys only when schema-repeatable. Omitted or [] supplies none; required fields must be supplied. Values are schema-validated scalar text, trimmed, with line breaks rejected; empty values remain present when schema-valid. Use relations for initial edges or relation_add for later edits.
    #[serde(default)]
    fields: Vec<FieldValue>,
    /// Initial schema-declared outgoing typed relations, created atomically with the item. Targets are exact human IDs or canonical MIDs; the new ID may target itself. Duplicate edges are rejected. Omitted or [] adds none; use relation_add/relation_remove for later edits.
    #[serde(default)]
    relations: Vec<InitialRelation>,
    /// Literal Markdown body (- is literal; no stdin). An omitted, null, empty, or whitespace-only required body creates an incomplete scaffold.
    #[serde(default)]
    body: Option<String>,
    /// Insert before this one-based destination line; valid range is 1 through line_count + 1 (end of file). Omitted or null appends. Insertion inside another item is rejected.
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
    /// Replacement single-line title; surrounding whitespace is trimmed. Empty or whitespace-only titles and line breaks are rejected; omitted or null leaves it unchanged.
    #[serde(default)]
    title: Option<String>,
    /// Replace all values of each named schema-declared custom field; repeat keys only when schema-repeatable. Values are schema-validated scalar text, trimmed, with line breaks rejected. Excludes structural title/MID metadata and typed relations; use relation_add/relation_remove for edges. Omitted or [] leaves fields unchanged; an empty value is not a clear (use clear_fields).
    #[serde(default)]
    fields: Vec<FieldValue>,
    /// Remove all values of named optional custom fields; cannot also set those keys in fields. Excludes title/MID and typed relations. Omitted or [] clears nothing; an absent optional field is a no-op.
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
    /// New unique human ID with the item's flavour prefix. Preserves the MID; the old human ID is not kept as an alias. The current ID is a no-op.
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
    /// Destination project-relative *.mara.md path selected by project discovery; parent directory must exist. Creates the file if absent; no absolute paths or .. components.
    file: PathBuf,
    /// Insert before this one-based line in the original destination, including same-file moves; valid range is 1 through line_count + 1 (end of file). Omitted or null appends. Insertion inside an item is rejected; the moved item's boundaries are no-ops.
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
struct GetToolParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Exact item ID/MID or a discovery handle returned by search, get, or related.
    reference: String,
    /// Opaque next_cursor from the previous response; keep all other inputs unchanged until has_more is false. Omit or null for the first page; restart after source/schema changes. Empty strings are invalid. Portions follow content, then item metadata.
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ItemFilterToolParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Exact flavour names, combined with OR and intersected with other filter categories. Omitted or [] selects all flavours.
    #[serde(default)]
    flavours: Vec<String>,
    /// Exact schema-declared custom-field key/value filters; excludes title/MID and typed relations. Key and scalar text value match exactly, without trimming; an empty value matches an empty field value. OR within one key, AND across keys and other filter categories. Omitted or [] adds no restriction.
    #[serde(default)]
    fields: Vec<FieldValue>,
    /// Exact authored outgoing relation names, combined with OR and intersected with other filters. Omitted or [] adds no restriction.
    #[serde(default)]
    relations: Vec<String>,
    /// Exact documents or directory subtrees relative to the project root, combined with OR. No globs, absolute paths, .., empty path elements, . or ./; omit paths or use [] to select the whole project. Example: ["packages/query/docs/"].
    #[serde(default)]
    paths: Vec<PathBuf>,
    /// Maximum entries per page, 1 through 100; omitted or null defaults to 20. The response byte budget may return fewer.
    #[serde(default)]
    limit: Option<usize>,
    /// Opaque next_cursor from the previous response; keep all other inputs unchanged until has_more is false. Omit or null for the first page; restart after source/schema changes. Empty strings are invalid.
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
struct SearchToolParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Unicode case-insensitive words matched across items, section headings, and ordinary Markdown blocks. Every distinct word must match; empty or punctuation-only text matches all search units within the filters.
    query: String,
    /// Exact flavour names, combined with OR and intersected with other filter categories. Omitted or [] selects all flavours.
    #[serde(default)]
    flavours: Vec<String>,
    /// Exact schema-declared custom-field key/value filters; excludes title/MID and typed relations. Key and scalar text value match exactly, without trimming; an empty value matches an empty field value. OR within one key, AND across keys and other filter categories. Omitted or [] adds no restriction.
    #[serde(default)]
    fields: Vec<FieldValue>,
    /// Exact authored outgoing schema relation names, combined with OR and intersected with other filters. Use schema:name to qualify; an unqualified name shared with a built-in is ambiguous. Omitted or [] adds no restriction.
    #[serde(default)]
    relations: Vec<String>,
    /// Exact documents or directory subtrees relative to the project root, combined with OR. No globs, absolute paths, .., empty path elements, . or ./; omit paths or use [] to select the whole project. Example: ["packages/query/docs/"].
    #[serde(default)]
    paths: Vec<PathBuf>,
    /// Maximum entries per page, 1 through 100; omitted or null defaults to 20. The response byte budget may return fewer.
    #[serde(default)]
    limit: Option<usize>,
    /// Opaque next_cursor from the previous response; keep all other inputs unchanged until has_more is false. Omit or null for the first page; restart after source/schema changes. Empty strings are invalid.
    #[serde(default)]
    cursor: Option<String>,
    /// Exact human IDs or canonical MIDs (uppercase 26-character ULIDs), combined with OR and intersected with other filters. Omitted or [] adds no restriction.
    #[serde(default)]
    ids: Vec<String>,
}

impl SearchToolParams {
    fn into_parts(self) -> (Option<PathBuf>, SearchParams) {
        (
            self.project,
            SearchParams {
                query: self.query,
                ids: self.ids,
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
struct RelatedToolParams {
    /// Absolute project root. Omit or null to discover from the server working directory; when started with --project, omit this parameter (overrides are rejected).
    #[serde(default)]
    project: Option<PathBuf>,
    /// Exact item ID/MID or a discovery handle returned by search, get, or related.
    reference: String,
    /// Edge direction relative to the selected node: incoming or outgoing. Omitted or null includes both, outgoing first.
    #[serde(default)]
    direction: Option<RelationDirection>,
    /// Relation names (schema:name or builtin:name; shorthand only when unambiguous), combined with OR and intersected with the neighbour flavour filter. Omitted or [] includes all.
    #[serde(default)]
    relations: Vec<String>,
    /// Exact neighbour flavour names, combined with OR. Nonempty selects item neighbours only; omitted or [] includes all.
    #[serde(default)]
    flavours: Vec<String>,
    /// Maximum entries per page, 1 through 100; omitted or null defaults to 20. The response byte budget may return fewer. Counts relation entries, not unique neighbours.
    #[serde(default)]
    limit: Option<usize>,
    /// Opaque next_cursor from the previous response; keep all other inputs unchanged until has_more is false. Omit or null for the first page; restart after source/schema changes. Empty strings are invalid.
    #[serde(default)]
    cursor: Option<String>,
}

impl RelatedToolParams {
    fn into_parts(self) -> (Option<PathBuf>, RelatedParams) {
        (
            self.project,
            RelatedParams {
                reference: self.reference,
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
        description = "Initialize a Mara project without overwriting existing content. Pass an absolute project path unless the server was started with --project. Templates create only .mara/project.toml and .mara/schema.yaml with schema format 2 and flavour guidance, no starter documents or items. Customize the project-owned schema; migrate existing schemas in place instead of reinitializing. Inspect guidance and relation endpoints with schema_get, then run schema_validate and project_validate."
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
        description = "Deliberately add generated MIDs to every legacy item that lacks one after a validation preflight; preserve existing MIDs."
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
        description = "Get the complete effective schema, or one named flavour or relation declaration. Before authoring, use description for purpose, use_when for selection criteria, avoid_when for exclusions, and distinguish_from to compare confusable flavours. These are schema guidance, not item fields. Inspect id_prefix, body, and fields for item constraints and relation source/target for allowed endpoints."
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
        description = "List declaration names and descriptions for flavours or relations in the effective schema. Follow with schema_get for full selection guidance, field constraints, and relation endpoints."
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
        description = "Validate schema format 2 without validating item content. Every flavour requires a nonblank description, nonempty use_when list, avoid_when list ([] is valid), and distinguish_from mapping ({} is valid). Entries must be nonblank; distinction targets must be other declared flavours. Migrate format 1 explicitly in the existing schema, preserving custom declarations and item identities; no automatic upgrade. Then run project_validate for the corpus."
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
        description = "Create one item with a generated MID and optional initial outgoing relations atomically in a project-relative Mara document; an omitted or blank required body creates an incomplete scaffold. Read schema_get flavour guidance and relation endpoints first. Newly authored references must resolve; reject changes that break or retarget surviving links, including shifted heading anchors."
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
        description = "Partially update one item by exact MID or human ID. Requires a title, custom field replacement or clear, or body. Preserves identity, relations, and untouched source. Validate newly authored references; reject changes that break or retarget surviving internal links, including links to headings or blocks inside the item. Resolve reported impacts before retrying; Markdown links are not automatically repaired."
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
        description = "Rename one human ID by exact MID or human ID, rewriting typed relations and supported wiki mentions in items and narrative across the valid corpus. Preserves the MID, Markdown links, and unrelated source; retains no alias. Uses recoverable file replacement."
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
        description = "Delete one item by exact MID or human ID after validating the project. Refuses surviving incoming typed relations, wiki mentions, and Markdown links to the item or its contained nodes, reporting blocking source locations. Also rejects broken or retargeted surviving links from shifted heading anchors. Keeps the containing document. Resolve reported impacts before retrying; Markdown links are not automatically repaired."
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
        description = "Move one item by exact MID or human ID within a valid project to a project-relative document. Preserve identity, content, and relations; keep the source document. Optional line is one-based in the original destination. Relative Markdown links carried with the item, incoming links to its contained nodes, and shifted heading anchors must keep their destinations. Resolve reported impacts before retrying; Markdown links are not automatically repaired."
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
        description = "Explicitly roll back a pending mutation journal to its original files. Stop other Mara writers first; conflicting manual edits are rejected. No journal is a no-op."
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
        name = "get",
        description = "Read an item, section, Markdown block, or document in bounded consecutive portions. Discovery format_version: 1 returns node, content, content_range, metadata, and metadata_range. Sections and documents include contained source; items return their parsed body, then ordered metadata fragments; non-items have empty metadata. Byte ranges are relative to each value; metadata indices preserve repeated keys. Follow next_cursor with unchanged reference until has_more is false; restart after source/schema changes. Search again if a structural handle is stale. Enumerate neighbours with related; get has no limit."
    )]
    fn get(
        &self,
        Parameters(params): Parameters<GetToolParams>,
    ) -> Result<Json<GetResult>, String> {
        self.for_project(params.project)?
            .get(GetParams {
                reference: params.reference,
                cursor: params.cursor,
            })
            .map(Json)
    }

    #[tool(
        name = "item_list",
        description = "List bounded item-summary pages in document-path and source order. Continue with next_cursor and unchanged options; restart after source/schema changes."
    )]
    fn item_list(
        &self,
        Parameters(params): Parameters<ItemFilterToolParams>,
    ) -> Result<Json<ItemCollectionResult>, String> {
        let (project, params) = params.into_parts();
        self.for_project(project)?.item_list(params).map(Json)
    }

    #[tool(
        name = "search",
        description = "Search every distinct query word with typo tolerance, ranked by relevance before pagination. Exact matches rank first; ID/title/heading matches carry more weight. ID/MID field words and all filters stay exact. Search items, section headings, and outermost Markdown blocks. Discovery format_version: 1 returns results: [{node, excerpt}]. One bounded source excerpt is automatic; item filters exclude narrative, and there is no node-kind filter. Pass node.reference to get for complete content or related for direct connections. Follow next_cursor with unchanged inputs; restart after source/schema changes. Structural handles identify a document snapshot; search again after that document changes."
    )]
    fn search(
        &self,
        Parameters(params): Parameters<SearchToolParams>,
    ) -> Result<Json<mara::SearchResult>, String> {
        let (project, params) = params.into_parts();
        self.for_project(project)?.search(params).map(Json)
    }

    #[tool(
        name = "related",
        description = "Explore direct schema relations, mentions, and containment from any node. Discovery format_version: 1 returns node and connections: [{relation, direction, neighbour, source}]. Counts connections, not unique neighbours; traversal is caller-controlled. Pass neighbour.reference to get or another related call. JSON uses contains with direction; its incoming view is displayed as contained_by in human CLI output. Select builtin:contains and incoming for the parent, then outgoing on that parent for its children. Continue with next_cursor and unchanged reference/options; restart after source/schema changes. Search again if a structural handle is stale."
    )]
    fn related(
        &self,
        Parameters(params): Parameters<RelatedToolParams>,
    ) -> Result<Json<RelatedResult>, String> {
        let (project, params) = params.into_parts();
        self.for_project(project)?.related(params).map(Json)
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
    instructions = "Structured Mara operations. Pass an absolute project path, or omit it for execution-directory discovery (project_init requires an explicit destination only when the server is unbound). When the server starts with --project, omit request-level project selection, including for project_init; overrides are rejected. Discovery and reading: search, get, and related cover items and narrative; item tools author, list, or validate items only. Before authoring, inspect schema_get flavour selection guidance and relation endpoints. Schema format 2 requires description, use_when, avoid_when, and distinguish_from for each flavour. Upgrade guidance: https://github.com/convesoft/mara/blob/main/docs/migration-0.2.mara.md"
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
