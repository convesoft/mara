use mara::{
    OperationContext, ProjectInitializationResult, SchemaGetResult, SchemaKind, SchemaListResult,
    Template,
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

#[tool_router]
impl MaraMcp {
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
}

#[tool_handler(
    name = "mara",
    instructions = "This rebuild checkpoint provides project_init, schema_get and schema_list. Pass an absolute project path per call, or omit it for execution-directory discovery. When the server starts with --project, omit request-level project selection, including for project_init. Initialization requires an explicit destination only when the server is unbound. Further capabilities await their implementation reviews."
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
