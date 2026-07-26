//! Reusable Mara application and infrastructure APIs.

pub mod command;
pub mod content;
mod diagnostic;
pub mod identity;
pub mod index;
pub mod project;
pub mod schema;
pub mod semantic;
pub mod transaction;
pub mod validation;

pub use index::{IndexError, IndexProjection, IndexWriteResult, write_index};
pub use semantic::{SemanticCompilation, compile_documents, compile_scalar};
pub use validation::{ValidationResult, check_project, check_schema, validate_documents};
