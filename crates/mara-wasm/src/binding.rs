use crate::{MemoryWorkspace, Request, envelope};
use mara_engine::{SelectionInputs, SourceSnapshot};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

/// Synchronous WASM boundary; shipped worker owns asynchronous host execution.
#[wasm_bindgen]
pub struct Workspace {
    inner: MemoryWorkspace,
}
#[wasm_bindgen]
impl Workspace {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Workspace {
        Workspace {
            inner: MemoryWorkspace::new(SourceSnapshot {
                format_version: 1,
                files: vec![],
                selection: SelectionInputs {
                    document_paths: vec![],
                    ignores: vec![],
                },
                base_identity: None,
            })
            .expect("empty snapshot is valid"),
        }
    }
    /// JSON preserves UTF-8 sources and existing operation envelopes without a second JS model.
    pub fn request(&mut self, input: &str) -> String {
        let response = match serde_json::from_str::<Request>(input) {
            Ok(request) => match self.inner.execute(request, generate_mid) {
                Ok(result) => serde_json::json!({"ok":true, "result":result}),
                Err(error) => serde_json::json!({"ok":false, "error":error}),
            },
            Err(error) => {
                serde_json::json!({"ok":false, "error":envelope("invalid_argument", error)})
            }
        };
        response.to_string()
    }
}
impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}
fn generate_mid() -> Result<String, serde_json::Value> {
    let crypto = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
        .ok()
        .and_then(|v| v.dyn_into::<web_sys::Crypto>().ok())
        .ok_or_else(|| {
            envelope(
                "host_unavailable",
                "Web Crypto is required for MID generation",
            )
        })?;
    let mut random = [0u8; 10];
    crypto
        .get_random_values_with_u8_array(&mut random)
        .map_err(|_| envelope("host_unavailable", "Web Crypto entropy is unavailable"))?;
    let milliseconds = js_sys::Date::now() as u64;
    let mut bytes = [0u8; 16];
    bytes[..6].copy_from_slice(&milliseconds.to_be_bytes()[2..]);
    bytes[6..].copy_from_slice(&random);
    Ok(ulid::Ulid::from(u128::from_be_bytes(bytes)).to_string())
}
