//! Native high-level routing. Backend choice happens before project dispatch.
use crate::{NativeRuntime, operations::*};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{env, fmt, path::PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExecutionMode {
    Direct,
    #[default]
    Auto,
    DaemonRequired,
}
impl std::str::FromStr for ExecutionMode {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "direct" => Ok(Self::Direct),
            "auto" => Ok(Self::Auto),
            "daemon-required" => Ok(Self::DaemonRequired),
            _ => Err("execution mode must be direct, auto or daemon-required".into()),
        }
    }
}

/// Explicit values override matching environment values. No persisted preferences.
#[derive(Debug, Clone, Default)]
pub struct ClientOptions {
    pub mode: Option<ExecutionMode>,
    /// Private IPC host directory, not a project/data directory.
    pub daemon_directory: Option<PathBuf>,
}
impl ClientOptions {
    pub fn from_environment(self) -> Result<Self, ClientError> {
        let mode = match self.mode {
            Some(mode) => mode,
            None => match env::var("MARA_EXECUTION_MODE") {
                Ok(value) => value.parse().map_err(ClientError::invalid_argument)?,
                Err(env::VarError::NotPresent) => ExecutionMode::Auto,
                Err(_) => {
                    return Err(ClientError::invalid_argument(
                        "MARA_EXECUTION_MODE must be UTF-8",
                    ));
                }
            },
        };
        Ok(Self {
            mode: Some(mode),
            daemon_directory: self
                .daemon_directory
                .or_else(|| env::var_os("MARA_DAEMON_DIRECTORY").map(PathBuf::from)),
        })
    }
}

/// Routing failures never imply rollback or authorize retry. Operation retains its
/// owning error envelope (including relationship context).
#[derive(Debug, Clone)]
pub enum ClientError {
    Operation(Value),
    InvalidArgument(String),
    Unavailable(String),
    Incompatible(String),
    Connection(String),
    Transport(String),
    UnknownOutcome { operation: String, project: PathBuf },
}
impl ClientError {
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::InvalidArgument(message.into())
    }
    pub fn code(&self) -> &str {
        match self {
            Self::Operation(data) => data["error"]["code"].as_str().unwrap_or("operation_failed"),
            Self::InvalidArgument(_) => "invalid_argument",
            Self::Unavailable(_) => "daemon_unavailable",
            Self::Incompatible(_) => "daemon_incompatible",
            Self::Connection(_) => "daemon_connection_error",
            Self::Transport(_) => "daemon_transport_error",
            Self::UnknownOutcome { .. } => "unknown_outcome",
        }
    }
    pub fn envelope(&self) -> Value {
        if let Self::Operation(data) = self {
            return data.clone();
        }
        let mut error = json!({"code":self.code(),"message":self.to_string()});
        if let Self::UnknownOutcome { operation, project } = self {
            error["operation"] = json!(operation);
            error["project"] = json!(project);
        }
        json!({"format_version":1,"error":error})
    }
    fn unknown(operation: &str, project: &std::path::Path) -> Self {
        Self::UnknownOutcome {
            operation: operation.into(),
            project: project.to_owned(),
        }
    }
}
impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Operation(data) => f.write_str(
                data["error"]["message"]
                    .as_str()
                    .unwrap_or("operation failed"),
            ),
            Self::InvalidArgument(m)
            | Self::Unavailable(m)
            | Self::Incompatible(m)
            | Self::Connection(m)
            | Self::Transport(m) => f.write_str(m),
            Self::UnknownOutcome { operation, project } => write!(
                f,
                "Unknown outcome for {operation} in {}: the dispatched mutation response was lost or unusable. Do not retry or replay it. Inspect canonical source and pending recovery state; stop writers before explicit recovery. No rollback is implied.",
                project.display()
            ),
        }
    }
}
impl std::error::Error for ClientError {}
impl From<&str> for ClientError {
    fn from(message: &str) -> Self {
        Self::from(message.to_owned())
    }
}
impl From<String> for ClientError {
    fn from(message: String) -> Self {
        Self::Operation(
            json!({"format_version":1,"error":{"code":"operation_failed","message":message}}),
        )
    }
}
impl From<crate::ValidationError> for ClientError {
    fn from(error: crate::ValidationError) -> Self {
        Self::Operation(error.envelope())
    }
}
impl From<crate::RelationError> for ClientError {
    fn from(error: crate::RelationError) -> Self {
        Self::Operation(serde_json::to_value(error).expect("serializable relation error"))
    }
}

#[derive(Debug, Clone)]
pub struct NativeClient {
    runtime: NativeRuntime,
    selected: Option<PathBuf>,
    current_directory: PathBuf,
    options: ClientOptions,
}
// @mara implements DES-NATIVE-CLIENT-ROUTING
// @mara implements REQ-OPTIONAL-DAEMON-EXECUTION
impl NativeClient {
    /// Environment-free constructor. Unspecified mode is auto.
    pub fn new(
        selected: Option<PathBuf>,
        current_directory: PathBuf,
        options: ClientOptions,
    ) -> Result<Self, ClientError> {
        let runtime = NativeRuntime::new(selected.clone(), current_directory.clone())?;
        let selected = selected.map(|p| {
            if p.is_absolute() {
                p
            } else {
                current_directory.join(p)
            }
        });
        if options
            .daemon_directory
            .as_ref()
            .is_some_and(|p| !p.is_absolute())
        {
            return Err(ClientError::invalid_argument(
                "daemon directory must be absolute",
            ));
        }
        Ok(Self {
            runtime,
            selected,
            current_directory,
            options,
        })
    }
    pub fn from_environment(
        selected: Option<PathBuf>,
        options: ClientOptions,
    ) -> Result<Self, ClientError> {
        let cwd = env::current_dir().map_err(|_| {
            ClientError::invalid_argument("could not read caller current directory")
        })?;
        Self::new(selected, cwd, options.from_environment()?)
    }
    pub fn with_sessions(mut self, sessions: crate::NativeSessions) -> Self {
        self.runtime = self.runtime.with_sessions(sessions);
        self
    }
    // @mara implements DES-OPERATION-PROJECT-CONTEXT
    pub fn for_project(&self, requested: Option<PathBuf>) -> Result<Self, String> {
        let runtime = self.runtime.for_project(requested.clone())?;
        Ok(Self {
            runtime,
            selected: requested.or_else(|| self.selected.clone()),
            ..self.clone()
        })
    }
    fn call<T: DeserializeOwned, P: serde::Serialize>(
        &self,
        method: &str,
        params: P,
        mutation: bool,
        direct: impl FnOnce(&NativeRuntime, P) -> Result<T, ClientError>,
    ) -> Result<T, ClientError> {
        // Resolve only the root here; malformed configuration must remain owning
        // validation data, not a strict-load routing failure.
        if self.options.mode.unwrap_or_default() == ExecutionMode::Direct {
            return direct(&self.runtime, params);
        }
        let root =
            crate::host::resolve_project_root(self.selected.as_deref(), &self.current_directory)
                .map_err(|e| match method {
                    "schema_validate" | "project_validate" | "item_validate" => {
                        let code = if matches!(e, crate::Error::Io { .. }) {
                            "io_error"
                        } else {
                            "invalid_argument"
                        };
                        ClientError::from(crate::ValidationError::new(code, e.to_string()))
                    }
                    "trace_matrix" => {
                        ClientError::from(crate::ValidationError::new("io_error", e.to_string()))
                    }
                    "relation_get" | "relation_add" | "relation_remove" => {
                        ClientError::from(crate::RelationError::from(e))
                    }
                    _ => ClientError::from(e.to_string()),
                })?;
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let connection = transport::Connection::open(self.options.daemon_directory.as_deref());
            let mut connection = match connection {
                Ok(c) => c,
                Err(ClientError::Unavailable(_) | ClientError::Incompatible(_))
                    if self.options.mode.unwrap_or_default() == ExecutionMode::Auto =>
                {
                    return direct(&self.runtime, params);
                }
                Err(error) => return Err(error),
            };
            // Direct execution (including established auto fallback) never needs
            // JSON-encodable host paths. Only the selected daemon request does.
            let encoding_error = |_| {
                ClientError::invalid_argument(
                    "Native IPC request paths must be UTF-8; inspect caller inputs or explicitly select direct execution. No operation was dispatched",
                )
            };
            let mut params = serde_json::to_value(&params).map_err(encoding_error)?;
            params["project"] = serde_json::to_value(&root).map_err(encoding_error)?;
            let result = connection.operation(method, params, mutation, &root)?;
            serde_json::from_value(result).map_err(|_| {
                if mutation {
                    ClientError::unknown(method, &root)
                } else {
                    ClientError::Transport(
                        "Daemon result does not match the owning format; no operation was replayed"
                            .into(),
                    )
                }
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (method, params, mutation, root);
            if self.options.mode.unwrap_or_default() == ExecutionMode::Auto {
                direct(&self.runtime, params)
            } else {
                Err(ClientError::Unavailable(
                    "Native IPC requires Linux or macOS".into(),
                ))
            }
        }
    }
    pub fn project_initialize(
        &self,
        target: Option<PathBuf>,
        template: crate::Template,
    ) -> Result<ProjectInitializationResult, ClientError> {
        let selected = self.for_project(target)?;
        if selected.selected.is_none() {
            return Err(ClientError::from("project init requires an absolute project path when the server is not bound with --project".to_owned()));
        }
        selected.call(
            "project_init",
            json!({"template":template}),
            true,
            |r, _| r.project_initialize(None, template).map_err(Into::into),
        )
    }
    pub fn schema_get(
        &self,
        kind: Option<SchemaKind>,
        name: Option<String>,
    ) -> Result<SchemaGetResult, ClientError> {
        self.call(
            "schema_get",
            json!({"kind":kind,"name":name}),
            false,
            |r, _| r.schema_get(kind, name).map_err(Into::into),
        )
    }
    pub fn schema_list(&self, kind: SchemaKind) -> Result<SchemaListResult, ClientError> {
        self.call("schema_list", json!({"kind":kind}), false, |r, _| {
            r.schema_list(kind).map_err(Into::into)
        })
    }
    pub fn get(&self, params: GetParams) -> Result<crate::GetResult, ClientError> {
        self.call("get", params, false, |r, params| {
            r.get(params).map_err(Into::into)
        })
    }
    pub fn related(&self, params: RelatedParams) -> Result<crate::RelatedResult, ClientError> {
        self.call("related", params, false, |r, params| {
            r.related(params).map_err(Into::into)
        })
    }
    pub fn search(&self, params: SearchParams) -> Result<crate::SearchResult, ClientError> {
        self.call("search", params, false, |r, params| {
            r.search(params).map_err(Into::into)
        })
    }
    pub fn item_list(
        &self,
        params: ItemFilterParams,
    ) -> Result<crate::ItemCollectionResult, ClientError> {
        self.call("item_list", params, false, |r, params| {
            r.item_list(params).map_err(Into::into)
        })
    }
    pub fn item_create(&self, params: ItemCreateParams) -> Result<ItemCreationResult, ClientError> {
        self.call("item_create", params, true, |r, params| {
            r.item_create(params).map_err(Into::into)
        })
    }
    pub fn item_update(
        &self,
        params: crate::ItemUpdateParams,
    ) -> Result<crate::ItemUpdate, ClientError> {
        self.call("item_update", params, true, |r, params| {
            r.item_update(params).map_err(Into::into)
        })
    }
    pub fn item_move(&self, params: crate::ItemMoveParams) -> Result<crate::ItemMove, ClientError> {
        self.call("item_move", params, true, |r, params| {
            r.item_move(params).map_err(Into::into)
        })
    }
    pub fn item_delete(&self, reference: &str) -> Result<crate::ItemDeletion, ClientError> {
        self.call(
            "item_delete",
            json!({"reference":reference}),
            true,
            |r, _| r.item_delete(reference).map_err(Into::into),
        )
    }
    pub fn item_rename(
        &self,
        reference: &str,
        new_id: &str,
    ) -> Result<crate::ItemRename, ClientError> {
        self.call(
            "item_rename",
            json!({"reference":reference,"new_id":new_id}),
            true,
            |r, _| r.item_rename(reference, new_id).map_err(Into::into),
        )
    }
    pub fn project_mid_backfill(&self) -> Result<ProjectMidBackfillResult, ClientError> {
        self.call("project_mid_backfill", json!({}), true, |r, _| {
            r.project_mid_backfill().map_err(Into::into)
        })
    }
    pub fn project_transaction_rollback(&self) -> Result<TransactionRollbackResult, ClientError> {
        self.call("project_transaction_rollback", json!({}), true, |r, _| {
            r.project_transaction_rollback().map_err(Into::into)
        })
    }
    pub fn force_refresh(&self) -> Result<(), ClientError> {
        let _: Value = self.call("project_refresh", json!({}), false, |r, _| {
            r.force_refresh()?;
            Ok(json!({"refreshed":true}))
        })?;
        Ok(())
    }
    pub fn relation_get(
        &self,
        params: RelationParams,
        limit: Option<usize>,
        cursor: Option<String>,
    ) -> Result<crate::RelationInspection, ClientError> {
        self.call("relation_get", json!({"source":params.source,"relation":params.relation,"target":params.target,"limit":limit,"cursor":cursor}), false, |r, _| r.relation_get(params,limit,cursor).map_err(Into::into))
    }
    pub fn relation_add(
        &self,
        params: RelationParams,
    ) -> Result<crate::RelationMutationResult, ClientError> {
        self.call("relation_add", params, true, |r, params| {
            r.relation_add(params).map_err(Into::into)
        })
    }
    pub fn relation_remove(
        &self,
        params: RelationParams,
    ) -> Result<crate::RelationMutationResult, ClientError> {
        self.relation_remove_occurrence(params, None)
    }
    pub fn relation_remove_occurrence(
        &self,
        params: RelationParams,
        occurrence: Option<String>,
    ) -> Result<crate::RelationMutationResult, ClientError> {
        self.call("relation_remove", json!({"source":params.source,"relation":params.relation,"target":params.target,"occurrence":occurrence}), true, |r, _| r.relation_remove_occurrence(params,occurrence).map_err(Into::into))
    }
    /// Contextual gates return conditions, never permissions or actions.
    pub fn query_gate(
        &self,
        request: crate::query_consumers::QueryGateRequest,
    ) -> Result<crate::query_consumers::QueryGateResult, ClientError> {
        self.call("query_gate", request, false, |runtime, request| {
            runtime.query_gate(request).map_err(|error| {
                ClientError::Operation(
                    json!({"format_version":2,"error":{"code":error.code,"message":error.message}}),
                )
            })
        })
    }

    /// Execute an exact named factual query.
    pub fn query_execute(
        &self,
        request: crate::named_query::Request,
    ) -> Result<crate::named_query::Page, ClientError> {
        self.call("query_execute", request, false, |runtime, request| {
            runtime
                .query_execute(request)
                .map_err(|error| ClientError::Operation(json!({"format_version":2,"error":error})))
        })
    }

    /// Render a factual view over one named-query page.
    pub fn query_view(
        &self,
        params: crate::query_views::ViewParams,
    ) -> Result<crate::query_views::FactualView, ClientError> {
        self.call("query_view", params, false, |runtime, params| {
            runtime
                .query_view(params)
                .map_err(|error| ClientError::Operation(json!({"format_version":2,"error":error})))
        })
    }

    /// Render a gate or enabled-policy evaluation report.
    pub fn query_report(
        &self,
        request: crate::query_views::ReportRequest,
        render: Option<String>,
    ) -> Result<crate::query_views::EvaluationReport, ClientError> {
        #[derive(serde::Serialize)]
        struct Params {
            subject: crate::query_views::ReportRequest,
            render: Option<String>,
        }
        self.call(
            "query_report",
            Params {
                subject: request,
                render,
            },
            false,
            |runtime, params| {
                runtime
                    .query_report(params.subject, params.render)
                    .map_err(|error| {
                        ClientError::Operation(
                            json!({"format_version":2,"error":{"code":error.code,"message":error.message}}),
                        )
                    })
            },
        )
    }

    pub fn trace_matrix(
        &self,
        params: &crate::TraceMatrixParams,
    ) -> Result<crate::TraceMatrixResult, ClientError> {
        self.call("trace_matrix", params, false, |r, params| {
            r.trace_matrix(params).map_err(Into::into)
        })
    }
    pub fn schema_validate_with_options(
        &self,
        options: &crate::ValidationOptions,
    ) -> Result<crate::ValidationResult, ClientError> {
        self.call("schema_validate", json!(options), false, |r, _| {
            r.schema_validate_with_options(options).map_err(Into::into)
        })
    }
    pub fn project_validate_with_options(
        &self,
        paths: &[PathBuf],
        options: &crate::ValidationOptions,
    ) -> Result<crate::ValidationResult, ClientError> {
        #[derive(serde::Serialize)]
        struct Params<'a> {
            paths: &'a [PathBuf],
            #[serde(flatten)]
            options: &'a crate::ValidationOptions,
        }
        self.call(
            "project_validate",
            Params { paths, options },
            false,
            |r, _| {
                r.project_validate_with_options(paths, options)
                    .map_err(Into::into)
            },
        )
    }
    pub fn item_validate_with_options(
        &self,
        id: &str,
        options: &crate::ValidationOptions,
    ) -> Result<crate::ValidationResult, ClientError> {
        #[derive(serde::Serialize)]
        struct Params<'a> {
            id: &'a str,
            #[serde(flatten)]
            options: &'a crate::ValidationOptions,
        }
        self.call("item_validate", Params { id, options }, false, |r, _| {
            r.item_validate_with_options(id, options)
                .map_err(Into::into)
        })
    }
    pub fn schema_validate(&self) -> Result<crate::ValidationResult, ClientError> {
        self.schema_validate_with_options(&crate::ValidationOptions::default())
    }
    pub fn project_validate(
        &self,
        paths: &[PathBuf],
    ) -> Result<crate::ValidationResult, ClientError> {
        self.project_validate_with_options(paths, &crate::ValidationOptions::default())
    }
    pub fn item_validate(&self, id: &str) -> Result<crate::ValidationResult, ClientError> {
        self.item_validate_with_options(id, &crate::ValidationOptions::default())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod transport;
