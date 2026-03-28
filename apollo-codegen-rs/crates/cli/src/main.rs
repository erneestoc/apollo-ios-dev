mod fetch_schema;
mod init_command;

// Re-export pipeline from lib crate
use apollo_codegen_cli::pipeline;

use clap::{Parser, Subcommand, ValueEnum};

/// Apollo iOS Code Generation CLI (Rust implementation)
#[derive(Parser)]
#[command(name = "apollo-ios-cli-rs")]
#[command(about = "A drop-in replacement for apollo-ios-cli written in Rust")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// How to package the schema types for dependency management.
#[derive(Debug, Clone, ValueEnum)]
enum ModuleType {
    SwiftPackageManager,
    EmbeddedInTarget,
    Other,
}

impl std::fmt::Display for ModuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleType::SwiftPackageManager => write!(f, "swiftPackageManager"),
            ModuleType::EmbeddedInTarget => write!(f, "embeddedInTarget"),
            ModuleType::Other => write!(f, "other"),
        }
    }
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

        /// Print timing for each pipeline phase to stderr
        #[arg(long)]
        timing: bool,

        /// Fetch the GraphQL schema before generating
        #[arg(long = "fetch-schema")]
        fetch_schema: bool,

        /// Ignore version mismatch between the CLI and the configuration
        #[arg(long = "ignore-version-mismatch")]
        ignore_version_mismatch: bool,

        /// Concatenate all generated files into a single output file
        #[arg(long)]
        concat: Option<String>,

        /// Skip generating SchemaConfiguration.swift (editable file)
        #[arg(long = "skip-schema-configuration")]
        skip_schema_configuration: bool,

        /// Skip generating CustomScalars/ directory
        #[arg(long = "skip-custom-scalars")]
        skip_custom_scalars: bool,

        /// Save compiled IR to a file for reuse by other commands
        #[arg(long = "save-ir")]
        save_ir: Option<String>,

        /// Load compiled IR from a file instead of re-parsing
        #[arg(long = "load-ir")]
        load_ir: Option<String>,
    },

    /// Generate only schema type files (Objects, Enums, Unions, InputObjects, etc.)
    GenerateSchemaTypes {
        /// Path to the code generation configuration JSON file
        #[arg(short, long)]
        path: Option<String>,

        /// Code generation configuration as a JSON string
        #[arg(short, long)]
        string: Option<String>,

        /// Log verbosity
        #[arg(short, long)]
        verbose: bool,

        /// Print timing for each pipeline phase to stderr
        #[arg(long)]
        timing: bool,

        /// Concatenate all generated files into a single output file
        #[arg(long)]
        concat: Option<String>,

        /// Skip generating SchemaConfiguration.swift (editable file)
        #[arg(long = "skip-schema-configuration")]
        skip_schema_configuration: bool,

        /// Skip generating CustomScalars/ directory
        #[arg(long = "skip-custom-scalars")]
        skip_custom_scalars: bool,

        /// Save compiled IR to a file for reuse by other commands
        #[arg(long = "save-ir")]
        save_ir: Option<String>,
    },

    /// Generate only operation and fragment files, optionally filtered by source path
    GenerateOperations {
        /// Path to the code generation configuration JSON file
        #[arg(short, long)]
        path: Option<String>,

        /// Code generation configuration as a JSON string
        #[arg(short, long)]
        string: Option<String>,

        /// Log verbosity
        #[arg(short, long)]
        verbose: bool,

        /// Print timing for each pipeline phase to stderr
        #[arg(long)]
        timing: bool,

        /// Only generate operations/fragments whose source file matches these glob patterns
        #[arg(long = "only-for-paths", value_delimiter = ',')]
        only_for_paths: Vec<String>,

        /// Concatenate all generated files into a single output file
        #[arg(long)]
        concat: Option<String>,

        /// Load compiled IR from a file instead of re-parsing
        #[arg(long = "load-ir")]
        load_ir: Option<String>,
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
        /// [DEPRECATED: Use --schema-namespace instead]
        #[arg(long = "schema-name", hide = true)]
        schema_name: Option<String>,

        /// Name used to scope the generated schema type files
        #[arg(short = 'n', long = "schema-namespace")]
        schema_namespace: Option<String>,

        /// How to package the schema types for dependency management
        #[arg(short = 'm', long = "module-type")]
        module_type: ModuleType,

        /// Name of the target for embeddedInTarget module type
        #[arg(short = 't', long = "target-name")]
        target_name: Option<String>,

        /// Write the configuration to a file at the path
        #[arg(short, long)]
        path: Option<String>,

        /// Overwrite any existing file at --path
        #[arg(short = 'w', long)]
        overwrite: bool,

        /// Print the configuration to stdout instead of writing to file
        #[arg(short = 's', long = "print")]
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

/// Load configuration from --path or --string, returning (config, root_dir).
fn load_config(
    path: Option<String>,
    string: Option<String>,
    verbose: bool,
) -> anyhow::Result<(apollo_codegen_config::ApolloCodegenConfiguration, std::path::PathBuf)> {
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

    Ok((config, root))
}

fn main() -> anyhow::Result<()> {
    // Check for Bazel persistent worker mode BEFORE clap parsing.
    // Bazel passes --persistent_worker as the only arg when starting a worker.
    if std::env::args().any(|a| a == "--persistent_worker") {
        return apollo_codegen_worker::run_worker_loop();
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            path, string, verbose, timing, fetch_schema, ignore_version_mismatch: _,
            concat, save_ir, load_ir,
            skip_schema_configuration, skip_custom_scalars,
        } => {
            let (config, root) = load_config(path, string, verbose)?;

            // Fetch schema before generating if --fetch-schema flag is set
            if fetch_schema {
                if let Some(ref schema_download) = config.schema_download {
                    fetch_schema::fetch_schema(schema_download, &root, verbose)?;
                } else {
                    anyhow::bail!(
                        "Missing schema download configuration. \
                         Hint: check the `schemaDownload` property of your configuration."
                    );
                }
            }

            // TODO: --load-ir / --save-ir are stubbed for now.
            // The serialization format for CompilationResult needs careful design
            // (field ordering, version tagging, etc.) before we commit to a binary format.
            if load_ir.is_some() {
                eprintln!("Warning: --load-ir is not yet implemented; ignoring.");
            }

            let show_timing = timing || verbose;
            let gen_options = pipeline::GenerateOptions {
                timing: show_timing,
                skip_schema_configuration,
                skip_custom_scalars,
            };
            let result = pipeline::generate(&config, &root, &gen_options)?;

            if save_ir.is_some() {
                eprintln!("Warning: --save-ir is not yet implemented; ignoring.");
            }

            if verbose {
                eprintln!("Generated {} files", result.file_count());
            }

            // Write output: either concatenated or individual files
            let t_write = std::time::Instant::now();
            if let Some(ref concat_path) = concat {
                result.write_concat(std::path::Path::new(concat_path))?;
            } else {
                result.write_all()?;
            }
            if show_timing {
                eprintln!("[timing] {:<20} {:>4}ms", "File writing:", t_write.elapsed().as_millis());
            }

            // Prune stale .graphql.swift files if configured (default: true)
            // Only prune in normal (non-concat) mode
            if concat.is_none() && config.options.prune_generated_files {
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

        Commands::GenerateSchemaTypes {
            path, string, verbose, timing, concat, save_ir,
            skip_schema_configuration, skip_custom_scalars,
        } => {
            let (config, root) = load_config(path, string, verbose)?;
            let show_timing = timing || verbose;
            let gen_options = pipeline::GenerateOptions {
                timing: show_timing,
                skip_schema_configuration,
                skip_custom_scalars,
            };

            let result = pipeline::generate_schema_only(&config, &root, &gen_options)?;

            // TODO: --save-ir is stubbed for now.
            if save_ir.is_some() {
                eprintln!("Warning: --save-ir is not yet implemented; ignoring.");
            }

            if verbose {
                eprintln!("Generated {} schema type files", result.file_count());
            }

            let t_write = std::time::Instant::now();
            if let Some(ref concat_path) = concat {
                result.write_concat(std::path::Path::new(concat_path))?;
            } else {
                result.write_all()?;
            }
            if show_timing {
                eprintln!("[timing] {:<20} {:>4}ms", "File writing:", t_write.elapsed().as_millis());
            }

            eprintln!("Schema type generation complete: {} files written", result.file_count());
            Ok(())
        }

        Commands::GenerateOperations {
            path, string, verbose, timing, only_for_paths, concat, load_ir,
        } => {
            let (config, root) = load_config(path, string, verbose)?;
            let show_timing = timing || verbose;

            // TODO: --load-ir is stubbed for now.
            if load_ir.is_some() {
                eprintln!("Warning: --load-ir is not yet implemented; ignoring.");
            }

            let result = pipeline::generate_operations_only(
                &config,
                &root,
                &only_for_paths,
                show_timing,
            )?;

            if verbose {
                eprintln!("Generated {} operation/fragment files", result.file_count());
            }

            let t_write = std::time::Instant::now();
            if let Some(ref concat_path) = concat {
                result.write_concat(std::path::Path::new(concat_path))?;
            } else {
                result.write_all()?;
            }
            if show_timing {
                eprintln!("[timing] {:<20} {:>4}ms", "File writing:", t_write.elapsed().as_millis());
            }

            eprintln!("Operation generation complete: {} files written", result.file_count());
            Ok(())
        }

        Commands::FetchSchema { path, string, verbose } => {
            let (config, root) = load_config(path, string, verbose)?;

            if let Some(ref schema_download) = config.schema_download {
                fetch_schema::fetch_schema(schema_download, &root, verbose)?;
            } else {
                anyhow::bail!(
                    "Missing schema download configuration. \
                     Hint: check the `schemaDownload` property of your configuration."
                );
            }

            Ok(())
        }
        Commands::Init {
            schema_name,
            schema_namespace,
            module_type,
            target_name,
            path,
            overwrite,
            print_config,
        } => {
            init_command::run(
                schema_name,
                schema_namespace,
                module_type,
                target_name,
                path,
                overwrite,
                print_config,
            )
        }
        Commands::GenerateOperationManifest { path, string, verbose, ignore_version_mismatch: _ } => {
            let (config, root) = load_config(path, string, verbose)?;
            pipeline::generate_operation_manifest(&config, &root, verbose)?;
            Ok(())
        }
    }
}
