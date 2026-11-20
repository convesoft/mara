//! The host side of the `mara` command line: everything that touches the filesystem
//! (DES-ENGINE-BOUNDARY in `docs/engine.mara.md`).

pub mod caller;
pub mod convert;
pub mod daemon;
pub mod grammar;
pub mod inputs;
pub mod project;
pub mod rules;
pub mod session;
mod tracked;
pub mod watch;
pub mod write;
