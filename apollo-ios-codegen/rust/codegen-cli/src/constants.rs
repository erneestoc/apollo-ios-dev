//! CLI constants matching Swift's `Constants` enum.
//!
//! Mirrors `Sources/CodegenCLI/Constants.swift`.

/// CLI version string matching Swift's `Constants.CLIVersion`.
/// Set to match the Swift CLI version for parity.
pub const CLI_VERSION: &str = "2.1.0-rc-1";

/// Default config file path matching Swift's `Constants.defaultFilePath`.
pub const DEFAULT_FILE_PATH: &str = "./apollo-codegen-config.json";
