//! The Node addon (REQ-NODE-LIBRARY and DES-NODE-LIBRARY in `docs/hosts/index.mara.md`): one
//! asynchronous function, `call`, that runs a document operation by its MCP tool name, as `mara
//! mcp` runs a tool call ([`mara::cli::tool::call`]), on a thread of libuv's pool, so that Node's
//! event loop never waits for it.
//!
//! The addon is a host embedded in Node ([`mara::system::embed`]): its build is the `mara`
//! executable beside its own file, which it reaches as its daemon and starts, and whose Soufflé it
//! runs; it leaves Node's signals to Node.

use std::ffi::CStr;
use std::path::PathBuf;
use std::sync::OnceLock;

use mara_engine::work::WorkCounts;
use napi::bindgen_prelude::AsyncTask;
use napi::{Env, Error, Result, Task};
use napi_derive::napi;
use serde_json::{Value, json};

/// One call: a document operation's tool name, its arguments as JSON text, the client's project
/// and whether it runs in direct mode.
pub struct Call {
    name: String,
    arguments: String,
    project: Option<String>,
    direct: bool,
}

impl Task for Call {
    type Output = String;
    type JsValue = String;

    /// Runs on a thread of libuv's pool. A panic of the operation fails the call alone.
    fn compute(&mut self) -> Result<String> {
        std::panic::catch_unwind(|| answer(self)).map_err(|panic| {
            let reason = (panic.downcast_ref::<&str>().map(|text| text.to_string()))
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            Error::from_reason(format!("Mara's addon failed: {reason}"))
        })?
    }

    fn resolve(&mut self, _: Env, output: String) -> Result<String> {
        Ok(output)
    }
}

// @mara implements REQ-NODE-LIBRARY
/// Runs the document operation whose MCP tool is `name` with the arguments of the JSON text
/// `arguments` in the project `project`, or the one found from the working directory, through
/// the daemon of the addon's build or in-process, or always in-process when `direct`. Resolves
/// with JSON text: `text`, the JSON object that `--format json` prints, `failed`, `status`, the
/// CLI's exit status, and `stats`, the object of the request's stats line
/// (DES-PERFORMANCE-HARNESS).
#[napi]
pub fn call(
    name: String,
    arguments: String,
    project: Option<String>,
    direct: bool,
) -> AsyncTask<Call> {
    AsyncTask::new(Call {
        name,
        arguments,
        project,
        direct,
    })
}

/// The answer to `call` as JSON text.
fn answer(call: &Call) -> Result<String> {
    let executable = executable().map_err(Error::from_reason)?;
    // The first call embeds the host; the executable is the same for every later one.
    mara::system::embed(executable.clone());
    let arguments: Value = serde_json::from_str(&call.arguments)
        .map_err(|error| Error::from_reason(format!("the arguments are no JSON: {error}")))?;
    let mut work = WorkCounts::default();
    let reply = mara::cli::tool::call(
        &call.name,
        &arguments,
        call.project.as_ref().map(PathBuf::from),
        call.direct,
        &mut work,
    );
    let stats: Value = serde_json::from_str(&mara::debug_stats::line(&work))
        .map_err(|error| Error::from_reason(format!("the stats are no JSON: {error}")))?;
    Ok(json!({
        "text": reply.text,
        "failed": reply.failed,
        "status": reply.status,
        "stats": stats,
    })
    .to_string())
}

/// The `mara` executable of this addon's build: `mara` in the directory of the addon's own
/// file, as the native package installs them, found once.
fn executable() -> std::result::Result<PathBuf, String> {
    static EXECUTABLE: OnceLock<std::result::Result<PathBuf, String>> = OnceLock::new();
    EXECUTABLE
        .get_or_init(|| {
            let file = own_file()?;
            let directory = file.parent().ok_or("the addon's file has no directory")?;
            Ok(directory.join("mara"))
        })
        .clone()
}

/// The path of the file that this addon was loaded from, as the dynamic loader names it.
fn own_file() -> std::result::Result<PathBuf, String> {
    let mut info = std::mem::MaybeUninit::<libc::Dl_info>::uninit();
    let address = own_file as *const ();
    // SAFETY: `dladdr` reads the address and writes only the struct it is given.
    let found = unsafe { libc::dladdr(address.cast(), info.as_mut_ptr()) };
    if found == 0 {
        return Err("the dynamic loader cannot name the addon's file".to_owned());
    }
    // SAFETY: `dladdr` succeeded, so it filled the struct.
    let info = unsafe { info.assume_init() };
    if info.dli_fname.is_null() {
        return Err("the dynamic loader names no file for the addon".to_owned());
    }
    // SAFETY: a non-null `dli_fname` is a NUL-terminated string that the loader keeps while
    // the library stays loaded.
    let name = unsafe { CStr::from_ptr(info.dli_fname) };
    use std::os::unix::ffi::OsStrExt;
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(name.to_bytes())))
}
