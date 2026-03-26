//! Intermediate representation for Apollo iOS code generation.
//!
//! Transforms the `CompilationResult` from the GraphQL frontend into
//! an IR suitable for Swift code generation. Mirrors the Swift IR module.

pub mod schema;
pub mod operation;
pub mod named_fragment;
pub mod selection_set;
pub mod entity;
pub mod fields;
pub mod scope;
pub mod inclusion;
pub mod builder;
pub mod computed;
pub mod field_collector;

pub use schema::Schema;
pub use operation::Operation;
pub use named_fragment::NamedFragment;
pub use selection_set::SelectionSet;
pub use entity::Entity;
pub use fields::{ScalarField, EntityField};
pub use scope::ScopeDescriptor;
pub use inclusion::InclusionConditions;
pub use builder::IRBuilder;
