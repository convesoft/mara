//! The WebAssembly module of the browser workspace (DES-BROWSER-WORKSPACE in
//! `docs/hosts/index.mara.md`): the calls of the package's worker, over the workspace of the
//! `mara` library ([`mara::cli::workspace`]). It is built for `wasm32-unknown-unknown` and
//! exports plain functions, without generated bindings (ADR-RAW-WASM-MODULE in
//! `docs/hosts/index.mara.md`); on any other target the crate is empty.
//!
//! A call is one message in the module's memory, and its answer another, each a frame: the
//! length of a JSON header as a 32-bit little-endian number, the header, then the bytes of the
//! files it carries, which the header names by offset and length after its end. The worker
//! allocates the call's frame with `mara_alloc`, passes it to `mara_call`, which takes it, and
//! reads the answer from `mara_answer` and `mara_answer_length` until its next call.
//!
//! | Call | Header | Answer |
//! | --- | --- | --- |
//! | Create | `{"call": "create", "files": [{"path", "at", "length"}]}` | `{"failed": false}` |
//! | Run | `{"call": "run", "name", "arguments"}` | `{"failed", "status", "stats", "reply"}`, or `{"pending": [run]}` |
//! | Ran | `{"call": "ran", "runs": [ran]}` | As a run's |
//! | Write | `{"call": "write", "path", "at", "length"}` | `{"failed": false}` |
//! | Remove | `{"call": "remove", "path"}` | `{"failed": false}` |
//! | Export | `{"call": "export"}` | `{"failed": false, "changes": [{"path", "base", "at", "length"}]}` |
//!
//! `reply` is the JSON object that `--format json` prints, and `stats` the object of the stats
//! line (DES-PERFORMANCE-HARNESS). A refused call answers `{"failed": true, "status", "reply"}`
//! with the failure's object. A removed file's change has `at` and `length` null. The instance
//! holds one workspace for its life: a second `create` is refused.
//!
//! A run whose request evaluates programs answers the runs of Soufflé that it waits for instead
//! ([`mara::rules::browser`]), each `{"id", "arguments", "files": [{"path", "at", "length"}],
//! "stdin": {"at", "length"} or null, "time_limit_ms", "output_bytes", "stdout_bytes",
//! "stderr_bytes"}`. The worker makes them and answers with `ran`, one per run: `{"id", "status",
//! "limit", "fault", "stdout": {"at", "length"}, "stdout_length", "stderr": {"at", "length"},
//! "stderr_at_stdout", "outputs": [{"path", "at", "length"}]}`, `status` the exit status of
//! `main` or null, `limit` `"time"`, `"memory"`, `"output"` or null, `fault` a message or null.
//! The module then runs the same request again, until it answers its result, whose stats count
//! the work of every pass. Any other call forgets a request that waits.

#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::fmt::Write as _;

use mara::cli::workspace::Workspace;
use mara::cli::{Failure, Reply};
use mara::debug_stats;
use mara::rules::browser::{self, Pending, Ran};
use mara_engine::evaluation::Limit;
use mara_engine::hash::Hash;
use mara_engine::work::WorkCounts;
use serde_json::{Map, Value, json};

thread_local! {
    /// The module's one workspace, once created.
    static WORKSPACE: RefCell<Option<Workspace>> = const { RefCell::new(None) };
    /// The answer of the last call.
    static ANSWER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// The request that waits for runs of Soufflé, with the work of its passes so far.
    static WAITING: RefCell<Option<(String, Value, WorkCounts)>> = const { RefCell::new(None) };
}

/// `length` bytes for the worker to write a call's frame into: a boxed slice of exactly that
/// length, which `mara_call` takes back.
#[unsafe(no_mangle)]
pub extern "C" fn mara_alloc(length: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; length].into_boxed_slice()).cast::<u8>()
}

/// Answers the call whose frame of `length` bytes `mara_alloc` gave at `frame`, and frees it.
///
/// # Safety
///
/// `frame` is a pointer that `mara_alloc(length)` returned, filled with `length` bytes, and not
/// passed here before.
// @mara implements REQ-BROWSER-LOCAL-WORKSPACE
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mara_call(frame: *mut u8, length: usize) {
    // SAFETY: the caller's contract: `mara_alloc(length)` returned it, a boxed slice of `length`
    // bytes, which the worker filled.
    let frame: Box<[u8]> =
        unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(frame, length)) };
    let answer = match unframe(&frame) {
        Ok((header, files)) => answer(&header, files),
        Err(message) => refused(rejected(message)),
    };
    ANSWER.with(|stored| *stored.borrow_mut() = answer);
}

/// Where the last call's answer begins.
#[unsafe(no_mangle)]
pub extern "C" fn mara_answer() -> *const u8 {
    ANSWER.with(|stored| stored.borrow().as_ptr())
}

/// How many bytes the last call's answer holds.
#[unsafe(no_mangle)]
pub extern "C" fn mara_answer_length() -> usize {
    ANSWER.with(|stored| stored.borrow().len())
}

/// The header and the files' bytes of a frame.
fn unframe(frame: &[u8]) -> Result<(Map<String, Value>, &[u8]), String> {
    let malformed = || "the call is no frame of a JSON header and files".to_owned();
    let (length, rest) = frame.split_first_chunk::<4>().ok_or_else(malformed)?;
    let length = u32::from_le_bytes(*length) as usize;
    if rest.len() < length {
        return Err(malformed());
    }
    let (header, files) = rest.split_at(length);
    match serde_json::from_slice(header) {
        Ok(Value::Object(header)) => Ok((header, files)),
        _ => Err(malformed()),
    }
}

/// A frame of `header` and `files`.
fn frame(header: &str, files: &[u8]) -> Vec<u8> {
    let length = u32::try_from(header.len()).expect("a header is less than 4 GiB");
    let mut frame = Vec::with_capacity(4 + header.len() + files.len());
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(header.as_bytes());
    frame.extend_from_slice(files);
    frame
}

fn answer(header: &Map<String, Value>, files: &[u8]) -> Vec<u8> {
    let text = |name: &str| header.get(name).and_then(Value::as_str);
    let bytes = |at: &Map<String, Value>| -> Result<Vec<u8>, String> {
        let number = |name: &str| {
            let number = at.get(name).and_then(Value::as_u64)?;
            usize::try_from(number).ok()
        };
        match (number("at"), number("length")) {
            (Some(at), Some(length))
                if at.checked_add(length).is_some_and(|end| end <= files.len()) =>
            {
                Ok(files[at..at + length].to_vec())
            }
            _ => Err("a file's bytes lie outside the call's frame".to_owned()),
        }
    };
    // Any call but `ran` forgets a request that waits for runs.
    if text("call") != Some("ran") {
        WAITING.with(|waiting| waiting.borrow_mut().take());
        browser::end();
    }
    let outcome: Result<Vec<u8>, Failure> = match text("call") {
        Some("create") => (|| {
            let listed = header.get("files").and_then(Value::as_array);
            let listed = listed.ok_or_else(|| rejected("`files` is no list".to_owned()))?;
            let mut supplied = Vec::with_capacity(listed.len());
            for file in listed {
                let file = file
                    .as_object()
                    .ok_or_else(|| rejected("a file is no object".to_owned()))?;
                let path = file.get("path").and_then(Value::as_str);
                let path = path.ok_or_else(|| rejected("a file has no `path`".to_owned()))?;
                supplied.push((path.to_owned(), bytes(file).map_err(rejected)?));
            }
            let workspace = Workspace::new(supplied)?;
            WORKSPACE.with(|stored| *stored.borrow_mut() = Some(workspace));
            Ok(done())
        })(),
        Some("run") => with_workspace(|workspace| {
            let name = text("name").ok_or_else(|| rejected("`name` is no string".to_owned()))?;
            let arguments = header.get("arguments").cloned().unwrap_or(Value::Null);
            Ok(pass(
                workspace,
                name.to_owned(),
                arguments,
                WorkCounts::default(),
            ))
        }),
        Some("ran") => with_workspace(|workspace| {
            let waiting = WAITING.with(|waiting| waiting.borrow_mut().take());
            let Some((name, arguments, work)) = waiting else {
                return Err(rejected("no request waits for runs of Soufflé".to_owned()));
            };
            let runs = header.get("runs").and_then(Value::as_array);
            let runs = runs.ok_or_else(|| rejected("`runs` is no list".to_owned()))?;
            for run in runs {
                let (id, ran) = supplied(run, &bytes).map_err(rejected)?;
                browser::supply(id, ran);
            }
            Ok(pass(workspace, name, arguments, work))
        }),
        Some("write") => with_workspace(|workspace| {
            let path = text("path").ok_or_else(|| rejected("`path` is no string".to_owned()))?;
            workspace.write(path, bytes(header).map_err(rejected)?)?;
            Ok(done())
        }),
        Some("remove") => with_workspace(|workspace| {
            let path = text("path").ok_or_else(|| rejected("`path` is no string".to_owned()))?;
            workspace.remove(path)?;
            Ok(done())
        }),
        Some("export") => with_workspace(|workspace| Ok(exported(workspace))),
        _ => Err(rejected("unknown call".to_owned())),
    };
    outcome.unwrap_or_else(refused)
}

/// Runs `call` on the workspace, which must have been created.
fn with_workspace(
    call: impl FnOnce(&mut Workspace) -> Result<Vec<u8>, Failure>,
) -> Result<Vec<u8>, Failure> {
    WORKSPACE.with(|stored| match &mut *stored.borrow_mut() {
        Some(workspace) => call(workspace),
        None => Err(rejected("no workspace was created".to_owned())),
    })
}

/// One pass of the request `name` with `arguments` ([`browser`]), `work` the work of its passes
/// before: its answer, or the runs of Soufflé that it waits for.
fn pass(
    workspace: &mut Workspace,
    name: String,
    arguments: Value,
    mut work: WorkCounts,
) -> Vec<u8> {
    let reply = workspace.run(&name, &arguments, &mut work);
    let pending = browser::pending();
    if pending.is_empty() {
        browser::end();
        return ran(&reply, &work);
    }
    WAITING.with(|waiting| *waiting.borrow_mut() = Some((name, arguments, work)));
    waits(&pending)
}

/// The answer that lists the runs `pending`, with their files after the header.
fn waits(pending: &[Pending]) -> Vec<u8> {
    let mut files = Vec::new();
    let mut place = |bytes: &[u8]| {
        let at = files.len();
        files.extend_from_slice(bytes);
        json!({"at": at, "length": bytes.len()})
    };
    let runs: Vec<Value> = (pending.iter())
        .map(|run| {
            let listed: Vec<Value> = (run.files.iter())
                .map(|(path, contents)| {
                    let mut file = place(contents);
                    file["path"] = json!(path);
                    file
                })
                .collect();
            json!({
                "id": run.id.to_string(),
                "arguments": run.arguments,
                "files": listed,
                "stdin": run.stdin.as_deref().map(&mut place),
                "time_limit_ms": u64::from(run.limits.time_seconds) * 1000,
                "output_bytes": run.limits.output_bytes,
                "stdout_bytes": run.stdout_bytes,
                "stderr_bytes": browser::PRINTED_BYTES,
            })
        })
        .collect();
    frame(&json!({ "pending": runs }).to_string(), &files)
}

/// Takes the bytes that a JSON object names by `at` and `length` from a call's frame.
type Bytes<'a> = dyn Fn(&Map<String, Value>) -> Result<Vec<u8>, String> + 'a;

/// The run that one entry of `ran` names, and what it left; `bytes` takes a file's bytes from
/// the call's frame.
fn supplied(run: &Value, bytes: &Bytes) -> Result<(Hash, Ran), String> {
    let run = run.as_object().ok_or("a run is no object")?;
    let id = run
        .get("id")
        .and_then(Value::as_str)
        .and_then(Hash::from_hex);
    let id = id.ok_or("a run has no `id`")?;
    let number = |name: &str| {
        let number = run.get(name).and_then(Value::as_u64);
        number.and_then(|number| usize::try_from(number).ok())
    };
    let part = |name: &str| match run.get(name) {
        Some(Value::Object(at)) => bytes(at),
        _ => Ok(Vec::new()),
    };
    let mut outputs = std::collections::BTreeMap::new();
    for output in run
        .get("outputs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let output = output.as_object().ok_or("an output is no object")?;
        let path = output.get("path").and_then(Value::as_str);
        let path = path.ok_or("an output has no `path`")?;
        outputs.insert(path.to_owned(), bytes(output)?);
    }
    let stdout = part("stdout")?;
    let stderr = part("stderr")?;
    let ran = Ran {
        status: (run.get("status").and_then(Value::as_i64)).and_then(|s| i32::try_from(s).ok()),
        limit: match run.get("limit").and_then(Value::as_str) {
            Some("time") => Some(Limit::Time),
            Some("memory") => Some(Limit::Memory),
            Some("output") => Some(Limit::Output),
            _ => None,
        },
        fault: run.get("fault").and_then(Value::as_str).map(str::to_owned),
        stdout_length: number("stdout_length").unwrap_or(stdout.len()),
        stderr_at_stdout: number("stderr_at_stdout").unwrap_or(stderr.len()),
        stdout,
        stderr,
        outputs,
    };
    Ok((id, ran))
}

fn done() -> Vec<u8> {
    frame(r#"{"failed":false}"#, &[])
}

/// The answer of a run: its reply as is, beside the stats line's object.
fn ran(reply: &Reply, work: &WorkCounts) -> Vec<u8> {
    let mut header = String::new();
    let _ = write!(
        header,
        r#"{{"failed":{},"status":{},"stats":{},"reply":{}}}"#,
        reply.failed,
        reply.status,
        debug_stats::line(work),
        reply.text
    );
    frame(&header, &[])
}

/// The answer of an export: each change, and the final contents after the header.
fn exported(workspace: &Workspace) -> Vec<u8> {
    let mut files = Vec::new();
    let changes: Vec<Value> = (workspace.export().into_iter())
        .map(|change| {
            let (at, length) = match &change.contents {
                Some(contents) => {
                    let at = files.len();
                    files.extend_from_slice(contents);
                    (json!(at), json!(contents.len()))
                }
                None => (Value::Null, Value::Null),
            };
            json!({"path": change.path, "base": change.base, "at": at, "length": length})
        })
        .collect();
    frame(
        &json!({"failed": false, "changes": changes}).to_string(),
        &files,
    )
}

/// The answer of a call refused with `failure`.
fn refused(failure: Failure) -> Vec<u8> {
    let status = failure.status;
    let reply = Reply::new(Err(failure), true);
    frame(
        &format!(
            r#"{{"failed":true,"status":{status},"reply":{}}}"#,
            reply.text
        ),
        &[],
    )
}

/// A call whose arguments are refused, as an operation's are: `invalid-argument`, status 2.
fn rejected(message: String) -> Failure {
    Failure {
        status: mara::cli::REJECTED,
        ..Failure::new("invalid-argument", message)
    }
}
