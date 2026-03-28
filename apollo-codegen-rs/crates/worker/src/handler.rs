//! Request handler - processes individual WorkRequests using cached artifacts.

use crate::cache::{self, WorkerCache};
use crate::protocol::{WorkRequest, WorkResponse};
use apollo_codegen_config::types::ApolloCodegenConfiguration;
use apollo_codegen_pipeline::{self as pipeline, CompiledArtifacts, GenerateOptions, RenderMode};
use std::path::{Path, PathBuf};

/// Parsed arguments from a WorkRequest.
pub struct WorkerArgs {
    /// Generation mode: schema-types, operations, or all.
    pub mode: WorkerMode,
    /// Path to the codegen config JSON file, OR the JSON string directly.
    pub config: ConfigSource,
    /// Output directory (overrides config's output path).
    pub output_dir: Option<String>,
    /// If set, concatenate all output into this single file.
    pub concat: Option<String>,
    /// Skip generating SchemaConfiguration.swift.
    pub skip_schema_configuration: bool,
    /// Skip generating custom scalar files.
    pub skip_custom_scalars: bool,
    /// Only generate operations from these specific .graphql files.
    pub only_for_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorkerMode {
    /// Generate everything (schema types + operations + fragments + mocks).
    All,
    /// Generate only schema type files.
    SchemaTypes,
    /// Generate only operation/fragment files.
    Operations,
}

pub enum ConfigSource {
    Path(String),
    Json(String),
}

/// Parse arguments from a WorkRequest.
pub fn parse_args(arguments: &[String]) -> Result<WorkerArgs, String> {
    let mut mode = WorkerMode::All;
    let mut config = None;
    let mut output_dir = None;
    let mut concat = None;
    let mut skip_schema_configuration = false;
    let mut skip_custom_scalars = false;
    let mut only_for_paths = Vec::new();

    let mut i = 0;
    while i < arguments.len() {
        let arg = &arguments[i];

        if let Some(val) = arg.strip_prefix("--mode=") {
            mode = match val {
                "schema-types" => WorkerMode::SchemaTypes,
                "operations" => WorkerMode::Operations,
                "all" => WorkerMode::All,
                other => return Err(format!("unknown mode: {}", other)),
            };
        } else if let Some(val) = arg.strip_prefix("--config=") {
            config = Some(ConfigSource::Path(val.to_string()));
        } else if let Some(val) = arg.strip_prefix("--config-json=") {
            config = Some(ConfigSource::Json(val.to_string()));
        } else if let Some(val) = arg.strip_prefix("--output-dir=") {
            output_dir = Some(val.to_string());
        } else if let Some(val) = arg.strip_prefix("--concat=") {
            concat = Some(val.to_string());
        } else if arg == "--skip-schema-configuration" {
            skip_schema_configuration = true;
        } else if arg == "--skip-custom-scalars" {
            skip_custom_scalars = true;
        } else if let Some(val) = arg.strip_prefix("--only-for-paths=") {
            only_for_paths = val.split(',').map(|s| s.to_string()).collect();
        } else if arg == "--mode" || arg == "--config" || arg == "--output-dir"
            || arg == "--concat" || arg == "--only-for-paths"
        {
            // Space-separated form: --key value
            i += 1;
            if i >= arguments.len() {
                return Err(format!("missing value for {}", arg));
            }
            let val = &arguments[i];
            match arg.as_str() {
                "--mode" => {
                    mode = match val.as_str() {
                        "schema-types" => WorkerMode::SchemaTypes,
                        "operations" => WorkerMode::Operations,
                        "all" => WorkerMode::All,
                        other => return Err(format!("unknown mode: {}", other)),
                    };
                }
                "--config" => config = Some(ConfigSource::Path(val.to_string())),
                "--output-dir" => output_dir = Some(val.to_string()),
                "--concat" => concat = Some(val.to_string()),
                "--only-for-paths" => {
                    only_for_paths = val.split(',').map(|s| s.to_string()).collect();
                }
                _ => {}
            }
        }
        // Ignore unknown args silently (forward compat)

        i += 1;
    }

    let config = config.ok_or("--config or --config-json is required")?;

    Ok(WorkerArgs {
        mode,
        config,
        output_dir,
        concat,
        skip_schema_configuration,
        skip_custom_scalars,
        only_for_paths,
    })
}

/// Handle a single WorkRequest: parse args, check cache, compile, render.
///
/// Returns a WorkResponse. Never panics - errors are returned as non-zero exit codes.
pub fn handle_request(
    request: &WorkRequest,
    cache: &mut WorkerCache,
) -> WorkResponse {
    match handle_request_inner(request, cache) {
        Ok(output) => WorkResponse {
            exit_code: 0,
            output,
            request_id: request.request_id,
        },
        Err(e) => WorkResponse {
            exit_code: 1,
            output: format!("Error: {:#}", e),
            request_id: request.request_id,
        },
    }
}

fn handle_request_inner(
    request: &WorkRequest,
    cache: &mut WorkerCache,
) -> anyhow::Result<String> {
    // 1. Parse arguments
    let args = parse_args(&request.arguments)
        .map_err(|e| anyhow::anyhow!("Invalid arguments: {}", e))?;

    // 2. Load config
    let config = load_config(&args.config)?;

    // 3. Classify inputs into schema vs operation files
    let (schema_inputs, op_inputs) = cache::classify_inputs(&request.inputs);
    let all_inputs = &request.inputs;

    // 4. Compute cache keys from Bazel-provided digests
    let schema_key = cache::compute_inputs_key(&schema_inputs);

    // 5. Check/populate schema cache
    if !cache.schema_matches(&schema_key) {
        // Schema changed or first invocation - load from disk
        let schema_sources: Vec<(String, String)> = schema_inputs
            .iter()
            .map(|input| {
                let content = std::fs::read_to_string(&input.path)
                    .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", input.path, e))?;
                Ok((content, input.path.clone()))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let frontend = pipeline::load_schema(&schema_sources)?;
        cache.set_schema(schema_key.clone(), frontend);

        eprintln!("[worker] Schema cache miss - loaded {} schema file(s)", schema_inputs.len());
    } else {
        eprintln!("[worker] Schema cache hit");
    }

    // 6. Compute options hash for compilation key.
    // Must include ALL options that affect generated output to prevent stale cache hits.
    let options_hash = format!(
        "mode={:?},reduce={},schema_doc={:?},sel_init_ops={},sel_init_frags={},sel_init_lcm={},\
         enum_case={:?},input_obj={:?},query_fmt={:?},op_doc_def={},op_doc_id={},\
         cocoapods={},field_merging={:?},legacy_safelisting={},\
         skip_schema_config={},skip_custom_scalars={}",
        args.mode,
        config.options.reduce_generated_schema_types,
        config.options.schema_documentation,
        config.options.selection_set_initializers.operations,
        config.options.selection_set_initializers.named_fragments,
        config.options.selection_set_initializers.local_cache_mutations,
        config.options.conversion_strategies.enum_cases,
        config.options.conversion_strategies.input_objects,
        config.options.query_string_literal_format,
        config.options.operation_document_format.definition,
        config.options.operation_document_format.operation_identifier,
        config.options.cocoapods_compatible_import_statements,
        config.experimental_features.field_merging,
        config.experimental_features.legacy_safelisting_compatible_operations,
        args.skip_schema_configuration,
        args.skip_custom_scalars,
    );

    // 7. Check/populate compilation cache
    let compilation_key = cache::compute_compilation_key(&schema_key, all_inputs, &options_hash);

    if !cache.compilation_matches(&compilation_key) {
        // Compilation cache miss - need to recompile
        let op_sources: Vec<(String, String)> = op_inputs
            .iter()
            .map(|input| {
                let content = std::fs::read_to_string(&input.path)
                    .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", input.path, e))?;
                Ok((content, input.path.clone()))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let frontend = cache.get_schema()
            .ok_or_else(|| anyhow::anyhow!("Schema not loaded"))?;

        let artifacts = pipeline::compile(frontend, &op_sources, &config)?;
        cache.set_compilation(compilation_key, artifacts);

        eprintln!(
            "[worker] Compilation cache miss - compiled {} operation file(s)",
            op_inputs.len()
        );
    } else {
        eprintln!("[worker] Compilation cache hit");
    }

    // 8. Render using the shared pipeline (same code as CLI)
    let root_url = match &args.config {
        ConfigSource::Path(p) => {
            Path::new(p).parent().unwrap_or(Path::new(".")).to_path_buf()
        }
        ConfigSource::Json(_) => PathBuf::from("."),
    };

    let artifacts = cache.get_compilation_mut()
        .ok_or_else(|| anyhow::anyhow!("Compilation not available"))?;

    let render_mode = match args.mode {
        WorkerMode::All => RenderMode::All,
        WorkerMode::SchemaTypes => RenderMode::SchemaOnly,
        WorkerMode::Operations => RenderMode::OperationsOnly {
            paths: args.only_for_paths,
        },
    };

    let gen_options = GenerateOptions {
        timing: false,
        skip_schema_configuration: args.skip_schema_configuration,
        skip_custom_scalars: args.skip_custom_scalars,
    };

    let generation_result = pipeline::render(
        artifacts,
        &config,
        &root_url,
        &gen_options,
        render_mode,
    )?;

    let file_count = generation_result.file_count();

    if let Some(ref concat_path) = args.concat {
        generation_result.write_concat(Path::new(concat_path))?;
    } else {
        generation_result.write_all()?;
    }

    Ok(format!("{} files generated", file_count))
}

fn load_config(source: &ConfigSource) -> anyhow::Result<ApolloCodegenConfiguration> {
    match source {
        ConfigSource::Path(path) => {
            ApolloCodegenConfiguration::from_file(Path::new(path))
        }
        ConfigSource::Json(json) => {
            serde_json::from_str(json)
                .map_err(|e| anyhow::anyhow!("Invalid config JSON: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_basic() {
        let args = vec![
            "--mode=schema-types".to_string(),
            "--config=/path/to/config.json".to_string(),
        ];
        let parsed = parse_args(&args).unwrap();
        assert_eq!(parsed.mode, WorkerMode::SchemaTypes);
        assert!(matches!(parsed.config, ConfigSource::Path(ref p) if p == "/path/to/config.json"));
    }

    #[test]
    fn test_parse_args_operations_with_paths() {
        let args = vec![
            "--mode=operations".to_string(),
            "--config=test.json".to_string(),
            "--only-for-paths=a.graphql,b.graphql".to_string(),
            "--concat=/output/ops.swift".to_string(),
        ];
        let parsed = parse_args(&args).unwrap();
        assert_eq!(parsed.mode, WorkerMode::Operations);
        assert_eq!(parsed.only_for_paths, vec!["a.graphql", "b.graphql"]);
        assert_eq!(parsed.concat.as_deref(), Some("/output/ops.swift"));
    }

    #[test]
    fn test_parse_args_space_separated() {
        let args = vec![
            "--mode".to_string(), "all".to_string(),
            "--config".to_string(), "config.json".to_string(),
        ];
        let parsed = parse_args(&args).unwrap();
        assert_eq!(parsed.mode, WorkerMode::All);
    }

    #[test]
    fn test_parse_args_missing_config() {
        let args = vec!["--mode=all".to_string()];
        assert!(parse_args(&args).is_err());
    }
}
