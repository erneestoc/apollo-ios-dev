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

use apollo_codegen_lib::codegen::{ApolloCodegen, CompileResult, ItemsToGenerate};
use apollo_codegen_lib::codegen_logger::CodegenLogger;
use apollo_codegen_lib::templates::ConfigurationContext;

use codegen_cli::input_options;

use crate::worker_io::{read_work_request, write_work_response};
use crate::worker_proto::{Input, WorkRequest, WorkResponse};
use crate::{Cli, Commands};

/// Cache key derived from Bazel input digests (D-90).
///
/// Uses BTreeMap for deterministic ordering per FNDN-05.
/// Two requests with the same set of file paths and digests
/// produce the same DigestKey, meaning the schema+IR can be reused.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DigestKey {
    /// Sorted map of input file path -> digest bytes.
    digests: BTreeMap<String, Vec<u8>>,
}

impl DigestKey {
    /// Computes a digest key from WorkRequest inputs (D-90).
    fn from_inputs(inputs: &[Input]) -> Self {
        let mut digests = BTreeMap::new();
        for input in inputs {
            digests.insert(input.path.clone(), input.digest.clone());
        }
        DigestKey { digests }
    }
}

/// Cached compilation artifacts (D-89, D-93).
///
/// Lives in the worker loop's outer scope and persists across requests.
/// Replaced when input digests change.
struct CachedCompilation {
    digest_key: DigestKey,
    compile_result: CompileResult,
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

    // Long-lived cache in outer scope (D-93)
    let mut cache: Option<CachedCompilation> = None;

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
        // drops at the end of this block. Only the cache persists.
        let response = handle_request(&request, &mut cache);

        // Write response to stdout (only protocol bytes, WRKR-04)
        if let Err(e) = write_work_response(&mut stdout, &response) {
            eprintln!("Error writing WorkResponse: {}", e);
            break;
        }
    }

    // Graceful shutdown (D-95) -- drop cache explicitly
    drop(cache);
    eprintln!("Worker shutdown complete");
    std::process::exit(0);
}

/// Handles a single WorkRequest (D-88, D-89, D-91).
///
/// 1. Parse WorkRequest.arguments through clap (D-88)
/// 2. Check cache for digest key match (D-89, D-90)
/// 3. On cache miss: run full pipeline via compile_schema_and_ir()
/// 4. On cache hit or after rebuild: run generate_from_ir() (D-91)
/// 5. Return WorkResponse with exit_code and output
///
/// All request-local state (config, file generators, rendered strings)
/// lives in this function's scope and drops when it returns (D-92).
fn handle_request(
    request: &WorkRequest,
    cache: &mut Option<CachedCompilation>,
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
    let config = ConfigurationContext::new(configuration.clone(), root_url);

    // Determine items to generate
    let mut items_to_generate = ItemsToGenerate::CODE;
    if let Some(ref manifest) = configuration.operation_manifest {
        if manifest.generate_manifest_on_code_generation {
            items_to_generate |= ItemsToGenerate::OPERATION_MANIFEST;
        }
    }

    // Compute cache key from Bazel input digests (D-90)
    let digest_key = DigestKey::from_inputs(&request.inputs);

    // Check cache (D-89)
    let needs_rebuild = cache
        .as_ref()
        .map_or(true, |c| c.digest_key != digest_key);

    if needs_rebuild {
        eprintln!("Cache miss -- running full compilation pipeline");
        match ApolloCodegen::compile_schema_and_ir(&config) {
            Ok(compile_result) => {
                *cache = Some(CachedCompilation {
                    digest_key,
                    compile_result,
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
        eprintln!("Cache hit -- reusing schema+IR from previous request");
    }

    // Generate files using cached or freshly-built schema+IR (D-91)
    let compile_result = &cache.as_ref().unwrap().compile_result;
    match ApolloCodegen::generate_from_ir(compile_result, &config, items_to_generate) {
        Ok(()) => WorkResponse {
            exit_code: 0,
            output: String::new(),
            request_id: 0, // Singleplex mode (WRKR-02)
            was_cancelled: false,
        },
        Err(e) => WorkResponse {
            exit_code: 1,
            output: format!("{}", e),
            request_id: 0,
            was_cancelled: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker_proto::Input;

    #[test]
    fn test_digest_key_from_empty_inputs() {
        let key = DigestKey::from_inputs(&[]);
        assert!(key.digests.is_empty());
    }

    #[test]
    fn test_digest_key_deterministic_ordering() {
        let inputs = vec![
            Input {
                path: "b.graphql".to_string(),
                digest: vec![2],
            },
            Input {
                path: "a.graphql".to_string(),
                digest: vec![1],
            },
        ];
        let key = DigestKey::from_inputs(&inputs);
        let keys: Vec<&String> = key.digests.keys().collect();
        assert_eq!(keys, vec!["a.graphql", "b.graphql"]);
    }

    #[test]
    fn test_digest_key_equality() {
        let inputs1 = vec![Input {
            path: "schema.graphqls".to_string(),
            digest: vec![1, 2, 3],
        }];
        let inputs2 = vec![Input {
            path: "schema.graphqls".to_string(),
            digest: vec![1, 2, 3],
        }];
        assert_eq!(
            DigestKey::from_inputs(&inputs1),
            DigestKey::from_inputs(&inputs2)
        );
    }

    #[test]
    fn test_digest_key_inequality_different_digest() {
        let inputs1 = vec![Input {
            path: "schema.graphqls".to_string(),
            digest: vec![1, 2, 3],
        }];
        let inputs2 = vec![Input {
            path: "schema.graphqls".to_string(),
            digest: vec![4, 5, 6],
        }];
        assert_ne!(
            DigestKey::from_inputs(&inputs1),
            DigestKey::from_inputs(&inputs2)
        );
    }

    #[test]
    fn test_digest_key_inequality_different_path() {
        let inputs1 = vec![Input {
            path: "schema_v1.graphqls".to_string(),
            digest: vec![1, 2, 3],
        }];
        let inputs2 = vec![Input {
            path: "schema_v2.graphqls".to_string(),
            digest: vec![1, 2, 3],
        }];
        assert_ne!(
            DigestKey::from_inputs(&inputs1),
            DigestKey::from_inputs(&inputs2)
        );
    }

    #[test]
    fn test_handle_request_unsupported_command() {
        // Pass valid init args so clap succeeds parsing, then we hit
        // the "unsupported command" branch in handle_request.
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
        let mut cache = None;
        let response = handle_request(&request, &mut cache);
        assert_eq!(response.exit_code, 1);
        assert!(
            response
                .output
                .contains("Worker only supports the 'generate' command"),
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
        let mut cache = None;
        let response = handle_request(&request, &mut cache);
        assert_eq!(response.exit_code, 1);
        assert!(response.output.contains("Failed to parse arguments"));
        assert_eq!(response.request_id, 0);
    }

    #[test]
    fn test_handle_request_singleplex_request_id() {
        // All responses must have request_id=0 (WRKR-02)
        let request = WorkRequest {
            arguments: vec!["generate".to_string()],
            inputs: vec![],
            request_id: 0,
            cancel: false,
            verbosity: 0,
            sandbox_dir: String::new(),
        };
        let mut cache = None;
        let response = handle_request(&request, &mut cache);
        // Will fail on missing config, but request_id must still be 0
        assert_eq!(response.request_id, 0);
    }
}
