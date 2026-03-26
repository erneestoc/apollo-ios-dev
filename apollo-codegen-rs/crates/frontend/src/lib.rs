//! GraphQL parsing, validation, and compilation frontend.
//!
//! Replaces the JavaScriptCore-based `GraphQLJSFrontend` with pure Rust
//! using `apollo-compiler` and `apollo-parser`.

pub mod compilation_result;
pub mod compiler;
pub mod introspection;
pub mod schema;
pub mod types;

pub use compilation_result::*;
pub use compiler::GraphQLFrontend;
