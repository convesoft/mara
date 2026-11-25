//! The host side of the `mara` command line: everything that touches the filesystem
//! (DES-ENGINE-BOUNDARY in `docs/engine.mara.md`). Its WebAssembly build runs the document
//! operations in a browser workspace, on files in memory ([`system`], DES-BROWSER-WORKSPACE in
//! `docs/hosts/index.mara.md`), without the daemon, MCP, grammars and downloads.

// The operations name the library's modules as `mara::…`, as a crate that depends on it does.
extern crate self as mara;

pub mod caller;
pub mod cli;
pub mod convert;
#[cfg(not(target_arch = "wasm32"))]
pub mod daemon;
pub mod debug_stats;
#[cfg(not(target_arch = "wasm32"))]
pub mod grammar;
#[cfg(target_arch = "wasm32")]
#[path = "grammar/browser.rs"]
pub mod grammar;
#[doc(hidden)]
pub mod hooks;
pub mod inputs;
pub mod project;
pub mod rules;
pub mod session;
pub mod system;
mod tracked;
#[cfg(not(target_arch = "wasm32"))]
pub mod watch;
pub mod write;
