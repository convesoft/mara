use mara::SearchParams;
use mara::{FieldValue, ItemCollectionResult, ItemFilterParams};
use mara::{
    OperationContext, ProjectInitializationResult, SchemaGetResult, SchemaKind, SchemaListResult,
    Template, ValidationResult,
};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::wrapper::{Json, Parameters},
    schemars::JsonSchema,
    tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone)]
struct MaraMcp {
    operations: OperationContext,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectInitParams {
    /// Absolute destination directory; required unless the server was started with --project, in which case omit it. Creates a missing directory; rejects an existing Mara project.
    #[serde(default)]
    project: Option<PathBuf>,
    /// Initial schema: minimal (default) includes common flavours and relations; empty declares none; engineering adds engineering vocabulary, accepted-knowledge policies and request-local coverage checks.
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
struct SchemaValidateParams {
    /// Absolute project root; omit when this server is bound with --project.
    project: Option<PathBuf>,
    #[serde(flatten)]
    options: mara::ValidationOptions,
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
    /// Exact authored relation name or inverse aliases, combined with OR and intersected with other filters. Omitted or [] adds no restriction.
    #[serde(default)]
    relations: Vec<String>,
    /// Exact documents or directory subtrees relative to the project root, combined with OR. No glob expansion, absolute paths, .. or empty/root-only paths; dot components and repeated separators normalize; omit paths or use [] to select the whole project. Example: ["packages/query/docs/"].
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
    /// Exact documents or directory subtrees relative to the project root, combined with OR. No glob expansion, absolute paths, .. or empty/root-only paths; dot components and repeated separators normalize; omit paths or use [] to select the whole project. Example: ["packages/query/docs/"].
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

#[tool_router]
impl MaraMcp {
    #[tool(
        name = "search",
        description = "Search every distinct query word with typo tolerance, ranked by relevance before pagination. Exact matches rank first; ID/title/heading matches carry more weight. ID/MID field words and all filters stay exact. Search items, section headings, and outermost Markdown blocks. Discovery format_version: 2 returns results: [{node, excerpt}]. One bounded source excerpt is automatic; item filters exclude narrative, and there is no node-kind filter. Pass node.reference to get for complete content or related for direct connections. Follow next_cursor with unchanged inputs; restart after source/schema changes. Structural handles identify a document snapshot; search again after that document changes."
    )]
    fn search(
        &self,
        Parameters(params): Parameters<SearchToolParams>,
    ) -> Result<Json<mara::SearchResult>, String> {
        let (project, params) = params.into_parts();
        self.for_project(project)?.search(params).map(Json)
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

    fn for_project(&self, project: Option<PathBuf>) -> Result<OperationContext, String> {
        self.operations.for_project(project)
    }
    #[tool(
        name = "project_init",
        description = "Initialize a Mara project without overwriting existing content. Pass an absolute project path unless the server was started with --project. Templates create .mara/project.toml and .mara/schema.yaml with schema format 3 and flavour guidance. Engineering also installs .mara/engineering-rules.yaml (enabled policy) and .mara/engineering-checks.yaml and .mara/engineering-execution.yaml (request-local checks), with required item status; no starter documents or items. Customize the project-owned schema; migrate existing projects manually on a recoverable checkpoint instead of reinitializing. See https://github.com/convesoft/mara/blob/main/docs/migration-0.3.mara.md. Inspect declarations with schema_get, then run schema_validate and project_validate."
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
        name = "schema_get",
        description = "Get the complete effective schema, or one named flavour or relation declaration. Before authoring, use description, use_when, avoid_when and distinguish_from to choose flavours; these are schema guidance, not item fields. Inspect id_prefix, body and fields for item constraints. Relation source/target, inverse, symmetric and external define endpoints and spelling; cardinality and acyclic are optional graph policies. Migrate existing projects manually and validate the result."
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
        output_schema = rmcp::handler::server::common::schema_for_type::<ValidationResult>(),
        description = "Validate schema format 3 without validating item content. Every flavour requires nonblank description, nonempty use_when, avoid_when ([] is valid), and distinguish_from ({} is valid); entries must be nonblank and distinction targets declared. Migrate existing schemas manually in place, preserving declarations and item identities; see https://github.com/convesoft/mara/blob/main/docs/migration-0.3.mara.md. Corpus validation is pending its capability review. Returns validation format 1, with null declaration counts if the schema cannot load. Follow next_cursor with unchanged options; invalid schemas return valid:false without a tool error."
    )]
    fn schema_validate(
        &self,
        Parameters(params): Parameters<SchemaValidateParams>,
    ) -> rmcp::model::CallToolResult {
        validation_result(
            self.for_project(params.project)
                .map_err(mara::ValidationError::invalid_argument)
                .and_then(|context| context.schema_validate_with_options(&params.options)),
        )
    }
}

#[tool_handler(
    name = "mara",
    instructions = "This rebuild checkpoint provides project_init, schema_get, schema_list and schema_validate. Pass an absolute project path per call, or omit it for execution-directory discovery. When the server starts with --project, omit request-level project selection, including for project_init. Initialization requires an explicit destination only when the server is unbound. Further capabilities await their implementation reviews."
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

fn validation_result(
    result: Result<ValidationResult, mara::ValidationError>,
) -> rmcp::model::CallToolResult {
    let (value, failed) = match result {
        Ok(result) => (
            serde_json::to_value(result).expect("serializable validation result"),
            false,
        ),
        Err(error) => (error.envelope(), true),
    };
    let mut response = rmcp::model::CallToolResult::structured(value);
    response.is_error = Some(failed);
    response
}
