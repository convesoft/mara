//! Private asynchronous addon boundary. Public typed ESM facade is downstream.
use mara::{ClientError, ClientOptions, NativeClient};
use napi::{Env, Result, ScopedTask, Unknown, bindgen_prelude::AsyncTask};
use napi_derive::napi;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

mod operations;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Options {
    caller_directory: PathBuf,
    project: Option<PathBuf>,
    mode: Option<String>,
    daemon_directory: Option<PathBuf>,
}

// @mara implements REQ-NODE-NATIVE-BINDING
// @mara implements DES-EXECUTION-BOUNDARIES
#[napi]
pub struct NativeHandle {
    client: Arc<Mutex<Option<NativeClient>>>,
}

#[napi]
impl NativeHandle {
    /// Constructor only validates explicit context; no discovery, I/O or environment routing.
    #[napi(constructor)]
    pub fn new(options: Value) -> Result<Self> {
        let create = || -> std::result::Result<NativeClient, ClientError> {
            let options: Options = serde_json::from_value(options)
                .map_err(|e| ClientError::invalid_argument(e.to_string()))?;
            let mode = options
                .mode
                .map(|m| m.parse())
                .transpose()
                .map_err(ClientError::invalid_argument)?;
            NativeClient::new(
                options.project,
                options.caller_directory,
                ClientOptions {
                    mode,
                    daemon_directory: options.daemon_directory,
                },
            )
        };
        let client = create().map_err(|e| napi::Error::from_reason(failure(e).to_string()))?;
        Ok(Self {
            client: Arc::new(Mutex::new(Some(client))),
        })
    }

    #[napi]
    pub fn execute(&self, operation: String, params: Value) -> AsyncTask<OperationTask> {
        AsyncTask::new(OperationTask {
            client: self.client.clone(),
            operation,
            params,
            close: false,
        })
    }

    #[napi]
    pub fn close(&self) -> AsyncTask<OperationTask> {
        AsyncTask::new(OperationTask {
            client: self.client.clone(),
            operation: String::new(),
            params: Value::Null,
            close: true,
        })
    }
}

pub struct OperationTask {
    client: Arc<Mutex<Option<NativeClient>>>,
    operation: String,
    params: Value,
    close: bool,
}
impl<'task> ScopedTask<'task> for OperationTask {
    type Output = Value;
    type JsValue = Unknown<'task>;

    // All host work, parsing, routing, socket waits and close/drop run on libuv workers.
    fn compute(&mut self) -> Result<Value> {
        let mut slot = self
            .client
            .lock()
            .map_err(|_| napi::Error::from_reason("Native handle lock poisoned"))?;
        if self.close {
            slot.take();
            return Ok(json!({"ok":true,"result":null}));
        }
        let Some(client) = slot.as_ref() else {
            return Ok(json!({"ok":false,"kind":"closed","error":{
                "format_version":1,"error":{"code":"client_closed","message":"Native client is closed"}
            }}));
        };
        Ok(
            match operations::execute(client, &self.operation, self.params.take()) {
                Ok(result) => json!({"ok":true,"result":result}),
                Err(error) => failure(error),
            },
        )
    }
    fn resolve(&mut self, env: &'task Env, mut output: Value) -> Result<Unknown<'task>> {
        json_numbers(&mut output)?;
        env.to_js_value(&output)
    }
}

// JSON numbers use JavaScript's double representation, just like JSON.parse.
// Avoid generic serde's integer-to-BigInt conversion in nested owning results.
fn json_numbers(value: &mut Value) -> Result<()> {
    match value {
        Value::Number(number) => {
            *number = number
                .as_f64()
                .and_then(serde_json::Number::from_f64)
                .ok_or_else(|| {
                    napi::Error::from_reason("Result contains an invalid JSON number")
                })?;
        }
        Value::Array(values) => {
            for value in values {
                json_numbers(value)?;
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                json_numbers(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn failure(error: ClientError) -> Value {
    let kind = match &error {
        ClientError::Operation(_) => "operation",
        ClientError::InvalidArgument(_) => "invalid_argument",
        ClientError::Unavailable(_) => "unavailable",
        ClientError::Incompatible(_) => "incompatible",
        ClientError::Connection(_) => "connection",
        ClientError::Transport(_) => "transport",
        ClientError::UnknownOutcome { .. } => "unknown_outcome",
    };
    json!({"ok":false,"kind":kind,"error":error.envelope()})
}
