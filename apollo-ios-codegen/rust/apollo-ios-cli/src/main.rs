//! Root CLI binary for Apollo iOS code generation.
//!
//! Mirrors Swift's `Sources/apollo-ios-cli/Apollo_iOS_CLI.swift`.
//! Registers 4 subcommands: init, generate, fetch-schema, generate-operation-manifest.
//! Exit codes: 0 = success, 1 = any error (D-77).

use clap::{Parser, Subcommand};

use codegen_cli::commands::{
    fetch_schema::FetchSchema, generate::Generate, generate_operation_manifest::GenerateOperationManifest,
    initialize::Initialize,
};
use codegen_cli::constants;

/// A command line utility for Apollo iOS code generation.
#[derive(Parser)]
#[command(name = "apollo-ios-cli")]
#[command(about = "A command line utility for Apollo iOS code generation.")]
#[command(version = constants::CLI_VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new configuration with defaults.
    #[command(name = "init")]
    Init(Initialize),
    /// Generate Swift source code based on a code generation configuration.
    Generate(Generate),
    /// Download a GraphQL schema from the Apollo Registry or GraphQL introspection.
    #[command(name = "fetch-schema")]
    FetchSchema(FetchSchema),
    /// Generate Persisted Queries operation manifest based on a code generation configuration.
    #[command(name = "generate-operation-manifest")]
    GenerateOperationManifest(GenerateOperationManifest),
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init(mut cmd) => cmd.run(),
        Commands::Generate(cmd) => cmd.run(),
        Commands::FetchSchema(cmd) => cmd.run(),
        Commands::GenerateOperationManifest(cmd) => cmd.run(),
    };

    // D-77: Exit codes match Swift -- 0 = success, 1 = any error
    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
