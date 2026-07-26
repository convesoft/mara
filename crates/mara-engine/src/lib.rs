//! Reusable Mara application and infrastructure APIs.

pub mod content;
mod diagnostic;
pub mod identity;
pub mod project;
pub mod schema;
pub mod semantic;
pub mod validation;

pub use semantic::{SemanticCompilation, compile_documents};
pub use validation::{ValidationResult, check_project, check_schema, validate_documents};
