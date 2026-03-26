//! Templates for generating Swift code from GraphQL types and operations.

pub mod header;
pub mod object;
pub mod interface;
pub mod union_type;
pub mod enum_type;
pub mod input_object;
pub mod custom_scalar;
pub mod schema_metadata;
pub mod schema_config;
pub mod package_swift;
pub mod mock_object;
pub mod mock_interfaces;
pub mod mock_unions;
pub mod selection_set;
pub mod fragment;
pub mod operation;
