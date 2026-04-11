//! Bazel persistent worker mode.
//!
//! Implements the Bazel worker protocol (singleplex mode, WRKR-02):
//! reads WorkRequest from stdin, runs the codegen pipeline, and writes
//! WorkResponse to stdout. Schema+IR is cached across requests keyed
//! by Bazel input digests (D-89, D-90).
//!
//! All non-protocol output goes to stderr (WRKR-04, D-96).

use std::collections::BTreeMap;
use std::io::{self, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;

use apollo_codegen_lib::codegen::{ApolloCodegen, CompiledSchema, ItemsToGenerate};
use apollo_codegen_lib::codegen_logger::CodegenLogger;
use apollo_codegen_lib::templates::ConfigurationContext;

use codegen_cli::input_options;

use crate::worker_io::{read_work_request, write_work_response};
use crate::worker_proto::{Input, WorkRequest, WorkResponse};
use crate::{Cli, Commands};

/// Cached parsed schema (the expensive part).
///
/// Schema parsing takes ~5s for large schemas but is identical across
/// all operation targets. Keyed by schema file digests so the cache
/// invalidates when the schema changes but persists across different
/// operation requests.
struct CachedSchema {
    /// Digests of schema files only (subset of WorkRequest.inputs).
    schema_digests: BTreeMap<String, Vec<u8>>,
    /// The parsed schema, reusable for any operation compilation.
    compiled_schema: CompiledSchema,
}

/// Runs the persistent worker loop (WRKR-01, WRKR-02).
///
/// Called from main() when --persistent_worker is detected (D-87).
/// Reads WorkRequests from stdin, processes each through the codegen
/// pipeline (with caching), and writes WorkResponses to stdout.
///
/// Exits on stdin EOF or SIGINT/SIGTERM (D-95).
pub fn run_worker_loop() {
    // Redirect panics to stderr so they never corrupt stdout (WRKR-04, T-11-06)
    std::panic::set_hook(Box::new(|info| {
        eprintln!("Worker panic: {}", info);
    }));

    // Set up signal handler for graceful shutdown (D-95)
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_signal = shutdown.clone();
    ctrlc::set_handler(move || {
        eprintln!("Worker received shutdown signal, exiting...");
        shutdown_signal.store(true, Ordering::SeqCst);
    })
    .expect("Failed to set signal handler");

    let stdin = io::stdin().lock();
    let mut reader = BufReader::new(stdin);
    let mut stdout = io::stdout().lock();

    // Long-lived schema cache in outer scope (D-93).
    // Schema parsing is expensive (~5s) but identical across all operation
    // targets. Caching it here means only the first request pays the cost.
    let mut schema_cache: Option<CachedSchema> = None;

    eprintln!("Apollo iOS codegen worker started (singleplex mode)");

    loop {
        // Check shutdown signal before reading next request (D-95)
        if shutdown.load(Ordering::SeqCst) {
            eprintln!("Worker shutting down due to signal");
            break;
        }

        // Read next request; None = EOF = graceful shutdown (D-95)
        let request = match read_work_request(&mut reader) {
            Ok(Some(req)) => req,
            Ok(None) => {
                eprintln!("Worker received EOF, shutting down");
                break;
            }
            Err(e) => {
                eprintln!("Error reading WorkRequest: {}", e);
                break;
            }
        };

        // Per-request scope (D-92) -- all request state is local and
        // drops at the end of this block. Only the schema cache persists.
        let response = handle_request(&request, &mut schema_cache);

        // Write response to stdout (only protocol bytes, WRKR-04)
        if let Err(e) = write_work_response(&mut stdout, &response) {
            eprintln!("Error writing WorkResponse: {}", e);
            break;
        }
    }

    // Graceful shutdown (D-95) -- drop cache explicitly
    drop(schema_cache);
    eprintln!("Worker shutdown complete");
    std::process::exit(0);
}

/// Handles a single WorkRequest (D-88, D-89, D-91).
///
/// 1. Parse WorkRequest.arguments through clap (D-88)
/// 2. Check schema cache for digest match (D-89, D-90)
/// 3. On schema cache miss: parse schema (~5s, expensive)
/// 4. Always: compile operations with cached schema (fast, ~ms)
/// 5. Run generate_from_ir() (D-91)
/// 6. Return WorkResponse with exit_code and output
///
/// All request-local state (config, file generators, rendered strings)
/// lives in this function's scope and drops when it returns (D-92).
/// Only the parsed schema persists across requests.
fn handle_request(
    request: &WorkRequest,
    schema_cache: &mut Option<CachedSchema>,
) -> WorkResponse {
    // Parse arguments through clap (D-88)
    // Prepend dummy program name since clap expects argv[0]
    let mut args = vec!["apollo-ios-cli".to_string()];
    args.extend(request.arguments.iter().cloned());

    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => {
            return WorkResponse {
                exit_code: 1,
                output: format!("Failed to parse arguments: {}", e),
                request_id: 0, // Singleplex mode (WRKR-02)
                was_cancelled: false,
            };
        }
    };

    // Only the generate command is supported in worker mode
    let generate_cmd = match cli.command {
        Commands::Generate(cmd) => cmd,
        _ => {
            return WorkResponse {
                exit_code: 1,
                output: "Worker only supports the 'generate' command".to_string(),
                request_id: 0,
                was_cancelled: false,
            };
        }
    };

    // Set log level from verbose flag
    CodegenLogger::set_level(generate_cmd.inputs.verbose);

    // Load configuration (T-11-05: errors are caught and returned in WorkResponse)
    let configuration = match generate_cmd.inputs.get_codegen_configuration() {
        Ok(config) => config,
        Err(e) => {
            return WorkResponse {
                exit_code: 1,
                output: format!("Failed to load configuration: {}", e),
                request_id: 0,
                was_cancelled: false,
            };
        }
    };

    let root_url = input_options::root_output_url(&generate_cmd.inputs);

    // Disable pruning in worker mode (D-92): multiple workers share the
    // execroot and write intermediate files to the same paths (e.g.
    // _schema_types_unused/). Pruning in one worker deletes files another
    // worker is actively using, causing TOCTOU races in delete_file.
    // Bazel manages its own outputs via tree artifacts, so pruning is
    // unnecessary and actively harmful here.
    let mut configuration = configuration;
    configuration.options.prune_generated_files = false;

    let config = ConfigurationContext::new(configuration.clone(), root_url);

    // Determine items to generate
    let mut items_to_generate = ItemsToGenerate::CODE;
    if let Some(ref manifest) = configuration.operation_manifest {
        if manifest.generate_manifest_on_code_generation {
            items_to_generate |= ItemsToGenerate::OPERATION_MANIFEST;
        }
    }

    // Schema-level caching: parse schema once, reuse for all operation targets.

    // Step 1: Discover files (cheap ~ms glob, runs every request)
    let (schema_matches, operation_matches) = match ApolloCodegen::discover_files(&config) {
        Ok(result) => result,
        Err(e) => {
            return WorkResponse {
                exit_code: 1,
                output: format!("{}", e),
                request_id: 0,
                was_cancelled: false,
            };
        }
    };

    // Step 2: Check if schema needs re-parsing
    let schema_digests = extract_schema_digests(&request.inputs, &config);
    let schema_changed = schema_cache
        .as_ref()
        .map_or(true, |c| c.schema_digests != schema_digests);

    if schema_changed {
        let label = if schema_cache.is_none() { "first request" } else { "schema changed" };
        eprintln!("Schema cache miss ({}) -- parsing schema", label);
        match ApolloCodegen::parse_schema_files(&schema_matches) {
            Ok(compiled) => {
                *schema_cache = Some(CachedSchema {
                    schema_digests,
                    compiled_schema: compiled,
                });
            }
            Err(e) => {
                return WorkResponse {
                    exit_code: 1,
                    output: format!("{}", e),
                    request_id: 0,
                    was_cancelled: false,
                };
            }
        }
    } else {
        eprintln!("Schema cache hit -- reusing parsed schema");
    }

    let compiled_schema = &schema_cache.as_ref().unwrap().compiled_schema;

    // Step 3: Compile operations with cached schema
    let compile_result = match ApolloCodegen::compile_operations_with_schema(
        compiled_schema,
        &operation_matches,
        &config,
    ) {
        Ok(result) => result,
        Err(e) => {
            return WorkResponse {
                exit_code: 1,
                output: format!("{}", e),
                request_id: 0,
                was_cancelled: false,
            };
        }
    };

    // Step 4: Generate files
    // In operations mode, skip schema type generation (5000+ files written to
    // _schema_types_unused/ and thrown away). Only generate operation/fragment files.
    let is_operations_mode = generate_cmd.bazel_mode == "operations";
    let generate_result = if is_operations_mode {
        ApolloCodegen::generate_from_ir_operations_only(&compile_result, &config, items_to_generate)
    } else {
        ApolloCodegen::generate_from_ir(&compile_result, &config, items_to_generate)
    };

    match generate_result {
        Ok(()) => {
            // Bazel tree artifact post-processing (copy + optimize + strip imports)
            if let Some(ref output_dir) = generate_cmd.bazel_output_dir {
                if let Err(e) = generate_cmd.populate_bazel_tree_artifact(output_dir) {
                    return WorkResponse {
                        exit_code: 1,
                        output: format!("Bazel post-processing failed: {}", e),
                        request_id: 0,
                        was_cancelled: false,
                    };
                }
            }

            WorkResponse {
                exit_code: 0,
                output: String::new(),
                request_id: 0, // Singleplex mode (WRKR-02)
                was_cancelled: false,
            }
        }
        Err(e) => WorkResponse {
            exit_code: 1,
            output: format!("{}", e),
            request_id: 0,
            was_cancelled: false,
        },
    }
}

/// Extracts digests for schema files from the WorkRequest inputs.
///
/// Uses the config's schemaSearchPaths to identify which input files are
/// schema files (typically just "V4/schema.graphqls"). Returns a sorted
/// map of schema file path -> digest for cache key comparison.
fn extract_schema_digests(
    inputs: &[Input],
    config: &ConfigurationContext,
) -> BTreeMap<String, Vec<u8>> {
    let schema_paths: std::collections::HashSet<&str> = config
        .config
        .input
        .schema_search_paths
        .iter()
        .map(|s| s.as_str())
        .collect();

    let mut digests = BTreeMap::new();
    for input in inputs {
        // Match by exact path or by suffix (Bazel paths may be relative to execroot)
        if schema_paths.contains(input.path.as_str())
            || schema_paths.iter().any(|sp| input.path.ends_with(sp))
            || input.path.ends_with(".graphqls")
        {
            digests.insert(input.path.clone(), input.digest.clone());
        }
    }
    digests
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker_proto::Input;

    #[test]
    fn test_extract_schema_digests_matches_graphqls() {
        let inputs = vec![
            Input { path: "V4/schema.graphqls".to_string(), digest: vec![1, 2, 3] },
            Input { path: "Features/X/Query.graphql".to_string(), digest: vec![4, 5] },
        ];
        // Minimal config with schema search paths
        let config_json = r#"{"schemaNamespace":"V4","input":{"operationSearchPaths":["**/*.graphql"],"schemaSearchPaths":["V4/schema.graphqls"]},"output":{"testMocks":{"none":{}},"schemaTypes":{"moduleType":{"other":{}},"path":"out"},"operations":{"inSchemaModule":{}}}}"#;
        let cfg: apollo_codegen_lib::config::ApolloCodegenConfiguration =
            serde_json::from_str(config_json).unwrap();
        let ctx = ConfigurationContext::new(cfg, None);

        let digests = extract_schema_digests(&inputs, &ctx);
        assert_eq!(digests.len(), 1);
        assert_eq!(digests["V4/schema.graphqls"], vec![1, 2, 3]);
    }

    #[test]
    fn test_handle_request_unsupported_command() {
        let request = WorkRequest {
            arguments: vec![
                "init".to_string(),
                "--module-type".to_string(),
                "swift-package".to_string(),
            ],
            inputs: vec![],
            request_id: 0,
            cancel: false,
            verbosity: 0,
            sandbox_dir: String::new(),
        };
        let mut schema_cache = None;
        let response = handle_request(&request, &mut schema_cache);
        assert_eq!(response.exit_code, 1);
        assert!(
            response.output.contains("Worker only supports the 'generate' command"),
            "expected 'Worker only supports' in output, got: {}",
            response.output
        );
        assert_eq!(response.request_id, 0);
    }

    #[test]
    fn test_handle_request_invalid_args() {
        let request = WorkRequest {
            arguments: vec!["--not-a-real-flag".to_string()],
            inputs: vec![],
            request_id: 0,
            cancel: false,
            verbosity: 0,
            sandbox_dir: String::new(),
        };
        let mut schema_cache = None;
        let response = handle_request(&request, &mut schema_cache);
        assert_eq!(response.exit_code, 1);
        assert!(response.output.contains("Failed to parse arguments"));
        assert_eq!(response.request_id, 0);
    }

    #[test]
    fn test_handle_request_singleplex_request_id() {
        let request = WorkRequest {
            arguments: vec!["generate".to_string()],
            inputs: vec![],
            request_id: 0,
            cancel: false,
            verbosity: 0,
            sandbox_dir: String::new(),
        };
        let mut schema_cache = None;
        let response = handle_request(&request, &mut schema_cache);
        assert_eq!(response.request_id, 0);
    }
}
