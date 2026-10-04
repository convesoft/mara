//! Transport-independent JSON-RPC 2.0 operation dispatch. No client routing.
use crate::{NativeRuntime, NativeSessions, operations::*};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::path::PathBuf;

pub const PROTOCOL_VERSION: u8 = 1;
pub const APPLICATION_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const OPERATIONS: &[&str] = &[
    "project_init",
    "project_mid_backfill",
    "project_transaction_rollback",
    "project_refresh",
    "schema_get",
    "schema_list",
    "schema_validate",
    "item_list",
    "search",
    "get",
    "related",
    "item_create",
    "item_update",
    "item_delete",
    "item_move",
    "item_rename",
    "item_validate",
    "project_validate",
    "relation_get",
    "relation_add",
    "relation_remove",
    "trace_matrix",
];

/// One compatibility session per transport connection. Hosts must authenticate and
/// authorize callers before handing their requests here (especially future HTTP hosts).
// @mara implements DES-LOCAL-DAEMON-PROTOCOL
pub struct RpcSession {
    negotiated: bool,
    sessions: NativeSessions,
}

impl RpcSession {
    pub fn new(sessions: NativeSessions) -> Self {
        Self {
            negotiated: false,
            sessions,
        }
    }

    /// Decode one JSON message. Notifications never dispatch project operations.
    pub fn handle_bytes(&mut self, bytes: &[u8]) -> Option<Value> {
        match serde_json::from_slice(bytes) {
            Ok(value) => self.handle(value),
            Err(_) => Some(
                RpcError::new(-32700, "Parse error: send one UTF-8 JSON object")
                    .response(Value::Null),
            ),
        }
    }

    pub fn handle(&mut self, request: Value) -> Option<Value> {
        let invalid = || {
            Some(
                RpcError::new(
                    -32600,
                    "Invalid Request: batches are unsupported; use a JSON-RPC 2.0 object",
                )
                .response(Value::Null),
            )
        };
        let Some(object) = request.as_object() else {
            return invalid();
        };
        if object.get("jsonrpc") != Some(&json!("2.0"))
            || !object.get("method").is_some_and(Value::is_string)
            || object
                .keys()
                .any(|k| !["jsonrpc", "id", "method", "params"].contains(&k.as_str()))
        {
            return invalid();
        }
        let id = object.get("id")?;
        if !(id.is_null() || id.is_number() || id.is_string()) {
            return invalid();
        }
        let method = object["method"].as_str().expect("checked method");
        let params = object.get("params").cloned().unwrap_or_else(|| json!({}));
        if method == "mara.negotiate" {
            self.negotiated = false;
        }
        let result = if !params.is_object() {
            Err(RpcError::new(
                -32602,
                "Invalid params: use a named parameter object",
            ))
        } else if method == "mara.negotiate" {
            self.negotiate(params)
        } else if !OPERATIONS.contains(&method) {
            Err(RpcError::new(
                -32601,
                "Method not found; inspect mara.negotiate capabilities",
            ))
        } else if !self.negotiated {
            Err(RpcError::domain(
                -32001,
                "negotiation_required",
                "Call mara.negotiate with the exact application and protocol versions before project operations",
            ))
        } else {
            self.dispatch(method, params)
        };
        Some(match result {
            Ok(result) => json!({"jsonrpc":"2.0", "id":id, "result":result}),
            Err(error) => error.response(id.clone()),
        })
    }

    fn negotiate(&mut self, params: Value) -> Result<Value, RpcError> {
        let params: Negotiate = decode(params)?;
        if params.protocol_version != PROTOCOL_VERSION
            || params.application_version != APPLICATION_VERSION
        {
            let mut error = RpcError::domain(
                -32002,
                "incompatible_version",
                "Use the exact server application version and Mara protocol version 1; no project operation was dispatched",
            );
            error.data = Some(
                json!({"format_version":1, "error": {"code":"incompatible_version", "message":error.message}, "protocol_version":PROTOCOL_VERSION, "application_version":APPLICATION_VERSION}),
            );
            return Err(error);
        }
        self.negotiated = true;
        Ok(json!({
            "protocol_version":PROTOCOL_VERSION, "application_version":APPLICATION_VERSION,
            "result_formats":{"discovery":2, "relationship":1, "validation":1, "trace":1, "operation_error":1},
            "capabilities":{"backend":"native", "operations":OPERATIONS, "persistence":"filesystem",
                "code_indexing":true, "configured_indexers":true, "transaction_rollback":true,
                "force_refresh":true, "project_sessions":"reconciled_memory"}
        }))
    }

    fn dispatch(&self, method: &str, mut params: Value) -> Result<Value, RpcError> {
        let project: PathBuf = decode(params.as_object_mut().expect("checked params").remove("project")
            .ok_or_else(|| RpcError::new(-32602, "Invalid params: supply an absolute project root (or initialization destination)"))?)?;
        if !project.is_absolute() {
            return Err(RpcError::new(
                -32602,
                "Invalid params: project must be absolute; resolve it in the caller's context",
            ));
        }
        // Explicit selection means the daemon cwd is never used for discovery.
        let runtime = NativeRuntime::new(Some(project), PathBuf::from("/"))
            .map_err(operation_error)?
            .with_sessions(self.sessions.clone());
        match method {
            "project_init" => {
                let p: Init = decode(params)?;
                string_result(runtime.project_initialize(None, p.template))
            }
            "project_mid_backfill" => {
                let _: Empty = decode(params)?;
                string_result(runtime.project_mid_backfill())
            }
            "project_transaction_rollback" => {
                let _: Empty = decode(params)?;
                string_result(runtime.project_transaction_rollback())
            }
            "project_refresh" => {
                let _: Empty = decode(params)?;
                runtime.force_refresh().map_err(operation_error)?;
                Ok(json!({"refreshed":true}))
            }
            "schema_get" => {
                let p: SchemaGet = decode(params)?;
                string_result(runtime.schema_get(p.kind, p.name))
            }
            "schema_list" => {
                let p: SchemaList = decode(params)?;
                string_result(runtime.schema_list(p.kind))
            }
            "schema_validate" => {
                let p: Page = decode(params)?;
                validation_result(runtime.schema_validate_with_options(&p.options()))
            }
            "project_validate" => {
                let p: ProjectValidation = decode(params)?;
                validation_result(runtime.project_validate_with_options(
                    &p.paths,
                    &crate::ValidationOptions {
                        limit: p.limit,
                        cursor: p.cursor,
                    },
                ))
            }
            "item_validate" => {
                let p: ItemValidation = decode(params)?;
                validation_result(runtime.item_validate_with_options(
                    &p.id,
                    &crate::ValidationOptions {
                        limit: p.limit,
                        cursor: p.cursor,
                    },
                ))
            }
            "item_list" => string_result(runtime.item_list(decode(params)?)),
            "search" => string_result(runtime.search(decode(params)?)),
            "get" => string_result(runtime.get(decode(params)?)),
            "related" => string_result(runtime.related(decode(params)?)),
            "item_create" => string_result(runtime.item_create(decode(params)?)),
            "item_update" => string_result(runtime.item_update(decode(params)?)),
            "item_move" => string_result(runtime.item_move(decode(params)?)),
            "item_delete" => {
                let p: Reference = decode(params)?;
                string_result(runtime.item_delete(&p.reference))
            }
            "item_rename" => {
                let p: Rename = decode(params)?;
                string_result(runtime.item_rename(&p.reference, &p.new_id))
            }
            "relation_get" => {
                let p: RelationGet = decode(params)?;
                relation_result(runtime.relation_get(p.edge(), p.limit, p.cursor))
            }
            "relation_add" => relation_result(runtime.relation_add(decode(params)?)),
            "relation_remove" => {
                let p: RelationRemove = decode(params)?;
                relation_result(runtime.relation_remove_occurrence(
                    RelationParams {
                        source: p.source,
                        relation: p.relation,
                        target: p.target,
                    },
                    p.occurrence,
                ))
            }
            "trace_matrix" => validation_result(runtime.trace_matrix(&decode(params)?)),
            _ => unreachable!("operation table checked"),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
impl RpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
    fn domain(code: i32, kind: &str, message: &str) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(json!({"format_version":1,"error":{"code":kind,"message":message}})),
        }
    }
    pub fn response(self, id: Value) -> Value {
        json!({"jsonrpc":"2.0", "id":id, "error":self})
    }
}
fn decode<T: DeserializeOwned>(params: Value) -> Result<T, RpcError> {
    serde_json::from_value(params).map_err(|e| {
        RpcError::new(
            -32602,
            format!("Invalid params: {}", sanitize(&e.to_string())),
        )
    })
}
fn encode<T: Serialize>(result: T) -> Result<Value, RpcError> {
    serde_json::to_value(result).map_err(|_| RpcError::new(-32603, "Internal error: result could not be encoded; inspect source before retrying a mutation"))
}
fn sanitize(message: &str) -> String {
    message
        .chars()
        .filter(|c| !c.is_control())
        .take(2048)
        .collect()
}
fn operation_error(message: String) -> RpcError {
    RpcError::domain(-32000, "operation_failed", &sanitize(&message))
}
fn string_result<T: Serialize>(result: Result<T, String>) -> Result<Value, RpcError> {
    encode(result.map_err(operation_error)?)
}
fn validation_result<T: Serialize>(
    result: Result<T, crate::ValidationError>,
) -> Result<Value, RpcError> {
    encode(result.map_err(|e| RpcError::domain(-32000, &e.code, &sanitize(&e.message)))?)
}
fn relation_result<T: Serialize>(
    result: Result<T, crate::RelationError>,
) -> Result<Value, RpcError> {
    encode(result.map_err(|mut e| {
        e.error.message = sanitize(&e.error.message);
        RpcError {
            code: -32000,
            message: e.error.message.clone(),
            data: serde_json::to_value(e).ok(),
        }
    })?)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Negotiate {
    protocol_version: u8,
    application_version: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Init {
    #[serde(default)]
    template: crate::Template,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaGet {
    kind: Option<SchemaKind>,
    name: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaList {
    kind: SchemaKind,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    limit: Option<usize>,
    cursor: Option<String>,
}
impl Page {
    fn options(self) -> crate::ValidationOptions {
        crate::ValidationOptions {
            limit: self.limit,
            cursor: self.cursor,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectValidation {
    #[serde(default)]
    paths: Vec<PathBuf>,
    limit: Option<usize>,
    cursor: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemValidation {
    id: String,
    limit: Option<usize>,
    cursor: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    reference: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rename {
    reference: String,
    new_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationGet {
    source: String,
    relation: String,
    target: String,
    limit: Option<usize>,
    cursor: Option<String>,
}
impl RelationGet {
    fn edge(&self) -> RelationParams {
        RelationParams {
            source: self.source.clone(),
            relation: self.relation.clone(),
            target: self.target.clone(),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationRemove {
    source: String,
    relation: String,
    target: String,
    occurrence: Option<String>,
}
