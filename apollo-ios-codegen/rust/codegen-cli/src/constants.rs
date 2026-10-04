//! CLI constants matching Swift's `Constants` enum.
//!
//! Mirrors `Sources/CodegenCLI/Constants.swift`.

/// CLI version string matching Swift's `Constants.CLIVersion`.
///
/// Every parity branch is a drop-in replacement for one Apollo iOS release, so the CLI
/// version is the same constant that the generated `Package.swift` pins the SDK to
/// (`CODEGEN_VERSION`); `--version` therefore identifies the release a binary was built for.
///
/// 1.15.3 is one of the two releases (with 1.15.2) whose upstream `CodegenVersion` stayed at
/// 1.15.1, so the generated `Package.swift` pins the SDK to 1.15.1 (`CODEGEN_VERSION`)
/// while the CLI reports its own version, like Swift's `Constants.CLIVersion` does.
pub const CLI_VERSION: &str = "1.15.3";

/// Default config file path matching Swift's `Constants.defaultFilePath`.
pub const DEFAULT_FILE_PATH: &str = "./apollo-codegen-config.json";
