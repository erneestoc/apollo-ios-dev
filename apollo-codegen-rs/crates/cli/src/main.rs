mod pipeline;

use clap::{Parser, Subcommand};

/// Apollo iOS Code Generation CLI (Rust implementation)
#[derive(Parser)]
#[command(name = "apollo-ios-cli-rs")]
#[command(about = "A drop-in replacement for apollo-ios-cli written in Rust")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate Swift code from GraphQL schemas and operations
    Generate {
        /// Path to the code generation configuration JSON file
        #[arg(short, long)]
        path: Option<String>,

        /// Code generation configuration as a JSON string
        #[arg(short, long)]
        string: Option<String>,

        /// Log verbosity
        #[arg(short, long)]
        verbose: bool,

        /// Fetch the GraphQL schema before generating
        #[arg(long = "fetch-schema")]
        fetch_schema: bool,

        /// Ignore version mismatch between the CLI and the configuration
        #[arg(long = "ignore-version-mismatch")]
        ignore_version_mismatch: bool,
    },

    /// Fetch a GraphQL schema via introspection
    FetchSchema {
        /// Path to the code generation configuration JSON file
        #[arg(short, long)]
        path: Option<String>,

        /// Code generation configuration as a JSON string
        #[arg(short, long)]
        string: Option<String>,

        /// Log verbosity
        #[arg(short, long)]
        verbose: bool,
    },

    /// Initialize a new code generation configuration
    Init {
        /// Path where the configuration file should be written
        #[arg(short, long)]
        path: Option<String>,

        /// Overwrite existing configuration file
        #[arg(long)]
        overwrite: bool,

        /// Print the configuration to stdout instead of writing to file
        #[arg(long = "print")]
        print_config: bool,
    },

    /// Generate an operation manifest for persisted queries
    GenerateOperationManifest {
        /// Path to the code generation configuration JSON file
        #[arg(short, long)]
        path: Option<String>,

        /// Code generation configuration as a JSON string
        #[arg(short, long)]
        string: Option<String>,

        /// Log verbosity
        #[arg(short, long)]
        verbose: bool,

        /// Ignore version mismatch between the CLI and the configuration
        #[arg(long = "ignore-version-mismatch")]
        ignore_version_mismatch: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate { path, string, verbose, fetch_schema: _, ignore_version_mismatch: _ } => {
            let config_path = path.unwrap_or_else(|| "apollo-codegen-config.json".to_string());
            if verbose {
                eprintln!("Loading configuration from: {}", config_path);
            }

            let config = if let Some(json_string) = string {
                apollo_codegen_config::ApolloCodegenConfiguration::from_json(&json_string)?
            } else {
                apollo_codegen_config::ApolloCodegenConfiguration::from_file(
                    std::path::Path::new(&config_path),
                )?
            };

            if verbose {
                eprintln!(
                    "Configuration loaded: namespace={}",
                    config.schema_namespace
                );
            }

            let root = std::path::Path::new(&config_path)
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf();

            let result = pipeline::generate(&config, &root)?;

            if verbose {
                eprintln!("Generated {} files", result.file_count());
            }

            result.write_all()?;

            // Prune stale .graphql.swift files if configured (default: true)
            if config.options.prune_generated_files {
                let pruned = result.prune_generated_files()?;
                if verbose && pruned > 0 {
                    eprintln!("Pruned {} stale file(s)", pruned);
                }
            }

            // Generate operation manifest if configured
            if let Some(ref manifest_config) = config.operation_manifest {
                if manifest_config.generate_manifest_on_codegen {
                    pipeline::generate_operation_manifest(&config, &root, verbose)?;
                }
            }

            eprintln!("Code generation complete: {} files written", result.file_count());
            Ok(())
        }
        Commands::FetchSchema { .. } => {
            eprintln!("Schema fetching not yet implemented");
            Ok(())
        }
        Commands::Init { .. } => {
            eprintln!("Configuration initialization not yet implemented");
            Ok(())
        }
        Commands::GenerateOperationManifest { path, string, verbose, ignore_version_mismatch: _ } => {
            let config_path = path.unwrap_or_else(|| "apollo-codegen-config.json".to_string());
            if verbose {
                eprintln!("Loading configuration from: {}", config_path);
            }

            let config = if let Some(json_string) = string {
                apollo_codegen_config::ApolloCodegenConfiguration::from_json(&json_string)?
            } else {
                apollo_codegen_config::ApolloCodegenConfiguration::from_file(
                    std::path::Path::new(&config_path),
                )?
            };

            let root = std::path::Path::new(&config_path)
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf();

            pipeline::generate_operation_manifest(&config, &root, verbose)?;
            Ok(())
        }
    }
}
