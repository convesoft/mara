//! JSON parameter adapter to typed NativeClient methods. No runtime/CLI dispatch.
use mara::{ClientError, NativeClient, operations::*};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::path::PathBuf;

fn decode<T: DeserializeOwned>(params: Value) -> Result<T, ClientError> {
    serde_json::from_value(params).map_err(|e| ClientError::invalid_argument(e.to_string()))
}
fn encode<T: Serialize>(result: Result<T, ClientError>) -> Result<Value, ClientError> {
    serde_json::to_value(result?).map_err(|_| {
        ClientError::Transport(
            "Result encoding failed; inspect source before retrying a mutation".into(),
        )
    })
}

// @mara implements DES-JAVASCRIPT-CLIENT-ADAPTERS
pub fn execute(
    client: &NativeClient,
    operation: &str,
    mut params: Value,
) -> Result<Value, ClientError> {
    let object = params
        .as_object_mut()
        .ok_or_else(|| ClientError::invalid_argument("Supply a named parameter object"))?;
    let project: Option<PathBuf> = object.remove("project").map(decode).transpose()?.flatten();
    let client = client
        .for_project(project)
        .map_err(ClientError::invalid_argument)?;
    match operation {
        "capabilities" => {
            let _: Empty = decode(params)?;
            Ok(json!({
                "backend":"native", "operations":mara::protocol::OPERATIONS,
                "mutation_persistence":"filesystem",
                "code":{"indexing":true,"configured_indexers":true},
                "native_recovery":true, "force_refresh":true,
                "application_version":env!("CARGO_PKG_VERSION"),
                "result_formats":{"discovery":2,"relationship":1,"validation":1,"trace":1,"query":3,"gate":1,"query_view":1,"query_report":1,"operation_error":1}
            }))
        }
        "project_init" => {
            let p: Init = decode(params)?;
            encode(client.project_initialize(None, p.template))
        }
        "project_mid_backfill" => {
            let _: Empty = decode(params)?;
            encode(client.project_mid_backfill())
        }
        "project_transaction_rollback" => {
            let _: Empty = decode(params)?;
            encode(client.project_transaction_rollback())
        }
        "project_refresh" => {
            let _: Empty = decode(params)?;
            client.force_refresh()?;
            Ok(json!({"refreshed":true}))
        }
        "schema_get" => {
            let p: SchemaGet = decode(params)?;
            encode(client.schema_get(p.kind, p.name))
        }
        "schema_list" => {
            let p: SchemaList = decode(params)?;
            encode(client.schema_list(p.kind))
        }
        "schema_validate" => {
            let p: Page = decode(params)?;
            encode(client.schema_validate_with_options(&p.options()))
        }
        "project_validate" => {
            let p: ProjectValidation = decode(params)?;
            encode(client.project_validate_with_options(
                &p.paths,
                &mara::ValidationOptions {
                    limit: p.limit,
                    cursor: p.cursor,
                    fact_snapshots: p.fact_snapshots,
                    mappings: p.mappings,
                },
            ))
        }
        "item_validate" => {
            let p: ItemValidation = decode(params)?;
            encode(client.item_validate_with_options(
                &p.id,
                &mara::ValidationOptions {
                    limit: p.limit,
                    cursor: p.cursor,
                    fact_snapshots: p.fact_snapshots,
                    mappings: p.mappings,
                },
            ))
        }
        "item_list" => encode(client.item_list(decode(params)?)),
        "search" => encode(client.search(decode(params)?)),
        "get" => encode(client.get(decode(params)?)),
        "related" => encode(client.related(decode(params)?)),
        "item_create" => encode(client.item_create(decode(params)?)),
        "item_update" => encode(client.item_update(decode(params)?)),
        "item_move" => encode(client.item_move(decode(params)?)),
        "item_delete" => {
            let p: Reference = decode(params)?;
            encode(client.item_delete(&p.reference))
        }
        "item_rename" => {
            let p: Rename = decode(params)?;
            encode(client.item_rename(&p.reference, &p.new_id))
        }
        "relation_get" => {
            let p: RelationGet = decode(params)?;
            encode(client.relation_get(p.edge(), p.limit, p.cursor))
        }
        "relation_add" => encode(client.relation_add(decode(params)?)),
        "relation_remove" => {
            let p: RelationRemove = decode(params)?;
            encode(client.relation_remove_occurrence(
                RelationParams {
                    source: p.source,
                    relation: p.relation,
                    target: p.target,
                },
                p.occurrence,
            ))
        }
        "query_execute" => encode(client.query_execute(decode(params)?)),
        "query_gate" => encode(client.query_gate(decode(params)?)),
        "query_view" => encode(client.query_view(decode(params)?)),
        "query_report" => {
            let p: QueryReportParams = decode(params)?;
            encode(client.query_report(p.subject, p.render))
        }
        "trace_matrix" => encode(client.trace_matrix(&decode(params)?)),
        _ => Err(ClientError::invalid_argument(
            "Unknown addon operation; inspect capabilities",
        )),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Init {
    #[serde(default)]
    template: mara::Template,
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
    fn options(self) -> mara::ValidationOptions {
        mara::ValidationOptions {
            limit: self.limit,
            cursor: self.cursor,
            fact_snapshots: vec![],
            mappings: vec![],
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
    #[serde(default)]
    fact_snapshots: Vec<mara::named_query::FactSnapshot>,
    #[serde(default)]
    mappings: Vec<mara::named_query::TypedMapping>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemValidation {
    id: String,
    limit: Option<usize>,
    cursor: Option<String>,
    #[serde(default)]
    fact_snapshots: Vec<mara::named_query::FactSnapshot>,
    #[serde(default)]
    mappings: Vec<mara::named_query::TypedMapping>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryReportParams {
    subject: mara::query_views::ReportRequest,
    render: Option<String>,
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
