//! The host side of the `mara` command line: everything that touches the filesystem
//! (DES-ENGINE-BOUNDARY in `docs/engine.mara.md`).

pub mod convert;
pub mod inputs;
pub mod project;
mod tracked;
pub mod write;
