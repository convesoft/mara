//! The host side of the `mara` command line: everything that touches the filesystem
//! (DES-ENGINE-BOUNDARY in `docs/engine.mara.md`).

pub mod caller;
pub mod convert;
pub mod daemon;
pub mod grammar;
pub mod inputs;
pub mod project;
pub mod rules;
mod tracked;
pub mod write;
