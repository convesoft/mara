use super::*;
use crate::{
    ipc,
    protocol::{APPLICATION_VERSION, OPERATIONS, PROTOCOL_VERSION},
};
use std::{
    io::{self, BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

pub(super) struct Connection(BufReader<UnixStream>);
impl Connection {
    pub(super) fn open(directory: Option<&Path>) -> Result<Self, ClientError> {
        let directory = match directory {
            Some(p) => p.to_owned(),
            None => ipc::runtime_directory().map_err(connection_error)?,
        };
        let stream = ipc::connect(&directory).map_err(connection_error)?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(connection_error)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(connection_error)?;
        let mut connection = Self(BufReader::new(stream));
        let frame = request_frame(
            "mara.negotiate",
            json!({"protocol_version":PROTOCOL_VERSION,"application_version":APPLICATION_VERSION}),
            1,
        )?;
        let result = connection.exchange(&frame, 1)
            .map_err(|_| ClientError::Connection("Could not complete daemon negotiation; inspect the endpoint and server. No project operation was dispatched".into()))?;
        let result = match result {
            Ok(value) => value,
            Err(error) if error["code"] == -32002
                && error["data"]["format_version"] == 1
                && error["data"]["error"]["code"] == "incompatible_version"
                && error["data"]["error"]["message"].is_string()
                && error["data"]["protocol_version"].is_u64()
                && error["data"]["application_version"].is_string() => return Err(incompatible()),
            Err(_) => return Err(ClientError::Connection("Daemon rejected negotiation; inspect authentication/protocol diagnostics. No project operation was dispatched".into())),
        };
        let formats =
            json!({"discovery":2,"relationship":1,"validation":1,"trace":1,"operation_error":1});
        let caps = &result["capabilities"];
        // An absent/wrongly typed field is not a compatibility advertisement.
        // Establish its shape before any comparison can authorize auto fallback.
        if !result.is_object()
            || !result["application_version"].is_string()
            || !result["protocol_version"].is_u64()
            || !result["result_formats"].is_object()
            || formats
                .as_object()
                .unwrap()
                .keys()
                .any(|k| !result["result_formats"][k].is_u64())
            || !caps.is_object()
            || ["backend", "persistence", "project_sessions"]
                .iter()
                .any(|k| !caps[k].is_string())
            || [
                "code_indexing",
                "configured_indexers",
                "transaction_rollback",
                "force_refresh",
            ]
            .iter()
            .any(|k| !caps[k].is_boolean())
            || !caps["operations"]
                .as_array()
                .is_some_and(|a| a.iter().all(Value::is_string))
        {
            return Err(ClientError::Connection("Malformed daemon negotiation result; inspect the endpoint/protocol. No project operation was dispatched".into()));
        }
        if result["application_version"] != APPLICATION_VERSION
            || result["protocol_version"] != PROTOCOL_VERSION
            || formats
                .as_object()
                .unwrap()
                .iter()
                .any(|(k, v)| result["result_formats"][k] != *v)
            || caps["backend"] != "native"
            || caps["persistence"] != "filesystem"
            || caps["project_sessions"] != "reconciled_memory"
            || [
                "code_indexing",
                "configured_indexers",
                "transaction_rollback",
                "force_refresh",
            ]
            .iter()
            .any(|k| caps[k] != true)
            || !OPERATIONS.iter().all(|method| {
                caps["operations"]
                    .as_array()
                    .is_some_and(|a| a.contains(&json!(method)))
            })
        {
            return Err(incompatible());
        }
        // Owning operations can run real indexers; negotiation's short timeout is
        // not an arbitrary operation deadline.
        connection
            .0
            .get_ref()
            .set_read_timeout(None)
            .map_err(connection_error)?;
        connection
            .0
            .get_ref()
            .set_write_timeout(None)
            .map_err(connection_error)?;
        Ok(connection)
    }
    fn exchange(&mut self, frame: &[u8], id: u8) -> io::Result<Result<Value, Value>> {
        self.0.get_mut().write_all(frame)?;
        let mut line = String::new();
        if self.0.read_line(&mut line)? == 0 || !line.ends_with('\n') {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "daemon response lost",
            ));
        }
        let response: Value = serde_json::from_str(&line)?;
        if response["jsonrpc"] != "2.0"
            || response["id"] != id
            || !response.is_object()
            || response.get("result").is_some() == response.get("error").is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid daemon response",
            ));
        }
        if let Some(value) = response.get("result") {
            return Ok(Ok(value.clone()));
        }
        let error = &response["error"];
        if !error["code"].is_i64() || !error["message"].is_string() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid daemon error",
            ));
        }
        Ok(Err(error.clone()))
    }
    pub(super) fn operation(
        &mut self,
        method: &str,
        params: Value,
        mutation: bool,
        root: &Path,
    ) -> Result<Value, ClientError> {
        let frame = request_frame(method, params, 2)?;
        match self.exchange(&frame, 2) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(error)) => {
                if mutation && error["code"] == -32603 {
                    return Err(ClientError::unknown(method, root));
                }
                let data = &error["data"];
                if error["code"] == -32000
                    && data["format_version"] == 1
                    && data["error"]["code"].is_string()
                    && data["error"]["message"].is_string()
                {
                    Err(ClientError::Operation(data.clone()))
                } else if mutation
                    && ![-32700, -32600, -32601, -32602, -32001, -32002]
                        .iter()
                        .any(|code| error["code"] == *code)
                {
                    // No owning outcome and no established protocol rejection.
                    Err(ClientError::unknown(method, root))
                } else {
                    Err(ClientError::Transport(format!(
                        "Daemon rejected {method} (RPC {}); inspect the request/protocol. No operation was replayed",
                        error["code"]
                    )))
                }
            }
            Err(_) if mutation => Err(ClientError::unknown(method, root)),
            Err(_) => Err(ClientError::Transport(
                "Daemon response lost or unusable; no operation was retried or replayed".into(),
            )),
        }
    }
}
fn request_frame(method: &str, params: Value, id: u8) -> Result<Vec<u8>, ClientError> {
    let mut frame =
        serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .map_err(|_| {
                ClientError::invalid_argument(
                    "request could not be encoded; no operation was dispatched",
                )
            })?;
    frame.push(b'\n');
    if frame.len() as u64 > ipc::MAX_REQUEST_BYTES {
        return Err(ClientError::invalid_argument(
            "request exceeds native 1 MiB frame limit; reduce the body; no operation was dispatched",
        ));
    }
    Ok(frame)
}
fn connection_error(error: io::Error) -> ClientError {
    if error.kind() == io::ErrorKind::NotFound {
        ClientError::Unavailable("No daemon endpoint exists; start a compatible daemon explicitly or select direct execution".into())
    } else {
        ClientError::Connection(format!(
            "Cannot connect to the daemon ({:?}); inspect endpoint ownership, permissions and server state. No project operation was dispatched",
            error.kind()
        ))
    }
}
fn incompatible() -> ClientError {
    ClientError::Incompatible("Daemon is incompatible; use the exact Mara application/protocol versions and native result formats/capabilities, or select direct execution. No project operation was dispatched".into())
}
