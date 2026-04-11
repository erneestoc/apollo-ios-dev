//! Root CLI binary for Apollo iOS code generation.
//!
//! Mirrors Swift's `Sources/apollo-ios-cli/Apollo_iOS_CLI.swift`.
//! Registers 4 subcommands: init, generate, fetch-schema, generate-operation-manifest.
//! Exit codes: 0 = success, 1 = any error (D-77).
//!
//! When invoked with `--persistent_worker`, enters Bazel worker mode (D-87).

mod worker;
mod worker_io;
mod worker_proto;

#[cfg(test)]
mod worker_bench;

use clap::{Parser, Subcommand};

use codegen_cli::commands::{
    fetch_schema::FetchSchema, generate::Generate,
    generate_operation_manifest::GenerateOperationManifest, initialize::Initialize,
};
use codegen_cli::constants;

/// A command line utility for Apollo iOS code generation.
#[derive(Parser)]
#[command(name = "apollo-ios-cli")]
#[command(about = "A command line utility for Apollo iOS code generation.")]
#[command(version = constants::CLI_VERSION)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
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
    // D-87: Check for --persistent_worker before clap parsing.
    // Bazel passes this flag when spawning persistent workers.
    // Must be checked in raw args because Bazel may also pass
    // arguments that clap doesn't understand.
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--persistent_worker") {
        worker::run_worker_loop();
        return; // run_worker_loop calls process::exit, but belt-and-suspenders
    }

    // Normal CLI mode
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
