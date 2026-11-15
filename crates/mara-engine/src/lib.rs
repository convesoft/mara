//! Mara's engine: a library over supplied file contents that does no I/O (DES-ENGINE-BOUNDARY in
//! `docs/engine.mara.md`).

pub mod config;
pub mod diagnostic;
pub mod document;
pub mod hash;
pub mod json;
pub mod paging;
pub mod pattern;
pub mod read;
pub mod relink;
pub mod schema;
pub mod snapshot;
mod toml_file;
pub mod validation;
pub mod work;
pub mod write;
