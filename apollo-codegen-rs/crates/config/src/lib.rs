//! Configuration parsing for Apollo iOS code generation.
//!
//! Parses `apollo-codegen-config.json` files into strongly-typed Rust structs
//! that mirror the Swift `ApolloCodegenConfiguration` type.

pub mod types;
pub mod validation;

pub use types::ApolloCodegenConfiguration;
