//! Configuration parsing for Apollo iOS code generation.
//!
//! Parses `apollo-codegen-config.json` files into strongly-typed Rust structs
//! that mirror the Swift `ApolloCodegenConfiguration` type.

pub mod types;
pub mod validation;

pub use types::ApolloCodegenConfiguration;
pub use types::SchemaDownloadConfiguration;
pub use types::SchemaDownloadMethod;
pub use types::IntrospectionSettings;
pub use types::ApolloRegistrySettings;
pub use types::SchemaDownloadHTTPMethod;
pub use types::SchemaDownloadOutputFormat;
pub use types::HTTPHeader;
