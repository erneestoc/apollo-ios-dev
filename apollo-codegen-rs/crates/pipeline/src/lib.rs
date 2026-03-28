//! Shared code generation pipeline.
//!
//! Wires together: config → glob → frontend → IR → templates → file output.
//! Used by both the CLI binary and the Bazel persistent worker.

use apollo_codegen_config::types::*;
use apollo_codegen_frontend::compilation_result::CompilationResult;
use apollo_codegen_frontend::compiler::{CompileOptions, GraphQLFrontend};
use apollo_codegen_frontend::types::GraphQLNamedType;
use apollo_codegen_frontend::compilation_result::OperationType;
use apollo_codegen_ir::builder::IRBuilder;
use apollo_codegen_render::ir_adapter;
use apollo_codegen_render::naming;
use apollo_codegen_render::schema_customization::SchemaCustomizer;
use apollo_codegen_render::templates;
use sha2::{Sha256, Digest};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Timing data for pipeline phases, printed to stderr when --timing is set.
struct PipelineTimer {
    enabled: bool,
    entries: Vec<(&'static str, std::time::Duration)>,
    start: Instant,
}

impl PipelineTimer {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            entries: Vec::new(),
            start: Instant::now(),
        }
    }

    /// Record the duration of a phase. Returns the Instant for the next phase.
    fn record(&mut self, label: &'static str, since: Instant) -> Instant {
        if self.enabled {
            self.entries.push((label, since.elapsed()));
        }
        Instant::now()
    }

    /// Print all recorded timings and total to stderr.
    fn print_summary(&self, file_count: usize) {
        if !self.enabled {
            return;
        }
        for (label, dur) in &self.entries {
            eprintln!("[timing] {:<20} {:>4}ms", format!("{}:", label), dur.as_millis());
        }
        let total = self.start.elapsed();
        eprintln!("[timing] {:<20} {:>4}ms ({} files)", "Total:", total.as_millis(), file_count);
    }
}

/// Options for controlling which files are generated.
#[derive(Default)]
pub struct GenerateOptions {
    pub timing: bool,
    pub skip_schema_configuration: bool,
    pub skip_custom_scalars: bool,
}

// ==========================================================================
// Shared Pipeline Stages - usable by both CLI and persistent worker
// ==========================================================================

use apollo_codegen_ir::field_collector::TypeKind;
use std::collections::HashMap;

/// The compiled artifacts from the frontend pipeline.
/// This is the cacheable unit for the persistent worker.
pub struct CompiledArtifacts {
    pub compilation_result: CompilationResult,
    pub ir_builder: IRBuilder,
    pub type_kinds: HashMap<String, TypeKind>,
}

/// Stage 1: Load and validate a GraphQL schema from source files.
///
/// Returns the frontend which holds the validated schema and can be reused
/// across multiple compile() calls (e.g., when only operations change).
pub fn load_schema(
    schema_sources: &[(String, String)],
) -> anyhow::Result<GraphQLFrontend> {
    GraphQLFrontend::load_schema(schema_sources)
        .map_err(|errs| anyhow::anyhow!("Schema errors: {}", errs.join(", ")))
}

/// Stage 2: Parse operations and compile against a loaded schema.
///
/// Returns the compilation result, IR builder, and type kinds map.
/// These can be cached and reused for rendering.
pub fn compile(
    frontend: &GraphQLFrontend,
    op_sources: &[(String, String)],
    config: &ApolloCodegenConfiguration,
) -> anyhow::Result<CompiledArtifacts> {
    let source_map: BTreeMap<String, (String, String)> = op_sources
        .iter()
        .map(|(content, path)| (path.clone(), (content.clone(), path.clone())))
        .collect();

    let doc = frontend
        .parse_operations(op_sources)
        .map_err(|errs| anyhow::anyhow!("Parse errors: {}", errs.join(", ")))?;

    let compile_options = CompileOptions {
        legacy_safelisting_compatible_operations: config
            .experimental_features
            .legacy_safelisting_compatible_operations,
        reduce_generated_schema_types: config.options.reduce_generated_schema_types,
    };

    let compilation_result = frontend
        .compile(&doc, &source_map, &compile_options)
        .map_err(|errs| anyhow::anyhow!("Compilation errors: {}", errs.join(", ")))?;

    let camel_case_enums = matches!(
        config.options.conversion_strategies.enum_cases,
        EnumCaseConversionStrategy::CamelCase
    );
    let ir_builder = IRBuilder::build(&compilation_result, camel_case_enums);
    let type_kinds = apollo_codegen_ir::field_collector::build_type_kinds(&compilation_result);

    Ok(CompiledArtifacts {
        compilation_result,
        ir_builder,
        type_kinds,
    })
}

/// Stage 3: Render output files from compiled artifacts.
///
/// This is the output-configuration + rendering stage. It produces the
/// GenerationResult which contains all files to be written.
///
/// `mode` controls which files are generated:
/// - `RenderMode::All` — schema types + operations + fragments + mocks
/// - `RenderMode::SchemaOnly` — schema types + module files + mocks only
/// - `RenderMode::OperationsOnly { paths }` — filtered operations/fragments only
pub fn render(
    artifacts: &mut CompiledArtifacts,
    config: &ApolloCodegenConfiguration,
    root_url: &Path,
    options: &GenerateOptions,
    mode: RenderMode,
) -> anyhow::Result<GenerationResult> {
    let ns = naming::first_uppercased(&config.schema_namespace);
    let api_target = if config.options.cocoapods_compatible_import_statements {
        "Apollo"
    } else {
        "ApolloAPI"
    };
    let access_mod = determine_access_modifier(config);
    let is_in_module = matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::SwiftPackageManager(_)
    ) || matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::Other(_)
    );
    let is_embedded = matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::EmbeddedInTarget(_)
    );
    let ops_in_schema_module = matches!(
        config.output.operations,
        OperationsFileOutput::InSchemaModule(_)
    );
    let ops_access_mod = if is_embedded && !ops_in_schema_module {
        "public ".to_string()
    } else {
        access_mod.clone()
    };
    let mock_access_mod = if is_embedded {
        "public ".to_string()
    } else {
        access_mod.clone()
    };
    let embedded_target_name = if let SchemaModuleType::EmbeddedInTarget(ref c) = config.output.schema_types.module_type {
        Some(c.name.clone())
    } else {
        None
    };
    let include_schema_docs = matches!(
        config.options.schema_documentation,
        SchemaDocumentation::Include
    );
    if !include_schema_docs {
        artifacts.ir_builder.clear_field_descriptions();
    }
    let query_string_format = match config.options.query_string_literal_format {
        QueryStringLiteralFormat::SingleLine => {
            apollo_codegen_render::templates::operation::QueryStringFormat::SingleLine
        }
        QueryStringLiteralFormat::Multiline => {
            apollo_codegen_render::templates::operation::QueryStringFormat::Multiline
        }
    };
    let camel_case_input_objects = matches!(
        config.options.conversion_strategies.input_objects,
        apollo_codegen_config::types::InputObjectConversionStrategy::CamelCase
    );

    let schema_output_path = resolve_path(root_url, &config.output.schema_types.path);
    let customizer = SchemaCustomizer::new(&config.options.schema_customization);
    let camel_case_enums = matches!(
        config.options.conversion_strategies.enum_cases,
        EnumCaseConversionStrategy::CamelCase
    );

    let mut result = GenerationResult::new();

    // Namespace file for embeddedInTarget
    let needs_namespace = is_embedded && !matches!(mode, RenderMode::OperationsOnly { .. });
    if needs_namespace {
        let namespace_content = format!(
            "{}\n\n{}enum {} {{ }}\n",
            apollo_codegen_render::templates::header::HEADER,
            &access_mod,
            &ns,
        );
        result.add_file(
            schema_output_path.join(format!("{}.graphql.swift", &ns)),
            namespace_content,
        );
    }

    // Schema types
    if !matches!(mode, RenderMode::OperationsOnly { .. }) {
        generate_schema_files(
            &mut result,
            &artifacts.compilation_result,
            &artifacts.ir_builder,
            &schema_output_path,
            &ns,
            api_target,
            &access_mod,
            is_in_module,
            camel_case_enums,
            camel_case_input_objects,
            config,
            &artifacts.type_kinds,
            &customizer,
            include_schema_docs,
            is_embedded,
            ops_in_schema_module,
            options.skip_schema_configuration,
            options.skip_custom_scalars,
        );

        generate_module_files(&mut result, config, &schema_output_path, &ns);
    }

    // Operations and fragments
    match mode {
        RenderMode::All => {
            generate_operation_files(
                &mut result,
                &artifacts.compilation_result,
                &artifacts.ir_builder,
                &schema_output_path,
                &ns,
                &ops_access_mod,
                config,
                &artifacts.type_kinds,
                &customizer,
                query_string_format,
                api_target,
                is_embedded,
                root_url,
                ops_in_schema_module,
                embedded_target_name.as_deref(),
                include_schema_docs,
            );
            generate_fragment_files(
                &mut result,
                &artifacts.compilation_result,
                &artifacts.ir_builder,
                &schema_output_path,
                &ns,
                &ops_access_mod,
                config,
                &artifacts.type_kinds,
                &customizer,
                query_string_format,
                api_target,
                is_embedded,
                root_url,
                ops_in_schema_module,
                embedded_target_name.as_deref(),
                include_schema_docs,
            );
        }
        RenderMode::OperationsOnly { ref paths } => {
            let path_matchers: Vec<glob::Pattern> = paths
                .iter()
                .filter_map(|p| glob::Pattern::new(p).ok())
                .collect();
            generate_operation_files_filtered(
                &mut result,
                &artifacts.compilation_result,
                &artifacts.ir_builder,
                &schema_output_path,
                &ns,
                &ops_access_mod,
                config,
                &artifacts.type_kinds,
                &customizer,
                query_string_format,
                api_target,
                is_embedded,
                root_url,
                ops_in_schema_module,
                embedded_target_name.as_deref(),
                include_schema_docs,
                &path_matchers,
            );
            generate_fragment_files_filtered(
                &mut result,
                &artifacts.compilation_result,
                &artifacts.ir_builder,
                &schema_output_path,
                &ns,
                &ops_access_mod,
                config,
                &artifacts.type_kinds,
                &customizer,
                query_string_format,
                api_target,
                is_embedded,
                root_url,
                ops_in_schema_module,
                embedded_target_name.as_deref(),
                include_schema_docs,
                &path_matchers,
            );
        }
        RenderMode::SchemaOnly => {
            // No operations or fragments
        }
    }

    // Test mocks (for All and SchemaOnly modes)
    if !matches!(mode, RenderMode::OperationsOnly { .. }) {
        generate_test_mock_files(
            &mut result,
            &artifacts.compilation_result,
            config,
            root_url,
            &ns,
            api_target,
            &mock_access_mod,
            &customizer,
            embedded_target_name.as_deref(),
        );
    }

    Ok(result)
}

/// Controls which files the render stage produces.
pub enum RenderMode {
    /// Generate everything: schema types + operations + fragments + mocks.
    All,
    /// Generate only schema types + module files + mocks.
    SchemaOnly,
    /// Generate only operations/fragments, optionally filtered to specific paths.
    OperationsOnly { paths: Vec<String> },
}

// ==========================================================================
// Convenience functions (CLI entry points) - thin wrappers over the stages
// ==========================================================================

/// Run the full code generation pipeline.
pub fn generate(config: &ApolloCodegenConfiguration, root_url: &Path, options: &GenerateOptions) -> anyhow::Result<GenerationResult> {
    let timing = options.timing;
    let mut timer = PipelineTimer::new(timing);
    let mut t = Instant::now();

    // 1. Discover files
    let schema_files = apollo_codegen_glob::match_search_paths(
        &config.input.schema_search_paths,
        Some(root_url),
    )?;
    let operation_files = apollo_codegen_glob::match_search_paths(
        &config.input.operation_search_paths,
        Some(root_url),
    )?;

    if schema_files.is_empty() {
        anyhow::bail!("No schema files found");
    }
    if operation_files.is_empty() {
        anyhow::bail!("No operation files found");
    }

    // 2. Load schema and parse operations
    let schema_sources: Vec<(String, String)> = schema_files
        .iter()
        .map(|path| {
            let content = std::fs::read_to_string(path)?;
            Ok((content, path.clone()))
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;

    let frontend = GraphQLFrontend::load_schema(&schema_sources)
        .map_err(|errs| anyhow::anyhow!("Schema errors: {}", errs.join(", ")))?;

    t = timer.record("Schema loading", t);

    let op_sources: Vec<(String, String)> = operation_files
        .iter()
        .map(|path| {
            let content = std::fs::read_to_string(path)?;
            Ok((content, path.clone()))
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;

    let source_map: BTreeMap<String, (String, String)> = op_sources
        .iter()
        .map(|(content, path)| (path.clone(), (content.clone(), path.clone())))
        .collect();

    let doc = frontend
        .parse_operations(&op_sources)
        .map_err(|errs| anyhow::anyhow!("Parse errors: {}", errs.join(", ")))?;

    t = timer.record("Operation parsing", t);

    // 3. Compile
    let compile_options = CompileOptions {
        legacy_safelisting_compatible_operations: config
            .experimental_features
            .legacy_safelisting_compatible_operations,
        reduce_generated_schema_types: config.options.reduce_generated_schema_types,
    };

    let compilation_result = frontend
        .compile(&doc, &source_map, &compile_options)
        .map_err(|errs| anyhow::anyhow!("Compilation errors: {}", errs.join(", ")))?;

    t = timer.record("Compilation", t);

    // 4. Build IR
    let camel_case_enums = matches!(
        config.options.conversion_strategies.enum_cases,
        EnumCaseConversionStrategy::CamelCase
    );
    let mut ir = IRBuilder::build(&compilation_result, camel_case_enums);

    // 4b. Build type kind map for type resolution in templates
    let type_kinds = apollo_codegen_ir::field_collector::build_type_kinds(&compilation_result);

    let _ = timer.record("IR building", t);

    // 5. Determine output configuration
    let ns = naming::first_uppercased(&config.schema_namespace);
    let api_target = if config.options.cocoapods_compatible_import_statements {
        "Apollo"
    } else {
        "ApolloAPI"
    };
    let access_mod = determine_access_modifier(config);
    let is_in_module = matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::SwiftPackageManager(_)
    ) || matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::Other(_)
    );
    let is_embedded = matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::EmbeddedInTarget(_)
    );
    let ops_in_schema_module = matches!(
        config.output.operations,
        OperationsFileOutput::InSchemaModule(_)
    );
    // For embeddedInTarget, operations/fragments/mocks that live outside the schema module
    // must always use "public" access, regardless of the configured access modifier.
    // Only operations that are inSchemaModule use the embedded access modifier.
    let ops_access_mod = if is_embedded && !ops_in_schema_module {
        "public ".to_string()
    } else {
        access_mod.clone()
    };
    // Test mocks are always outside the schema module, so they always need "public"
    let mock_access_mod = if is_embedded {
        "public ".to_string()
    } else {
        access_mod.clone()
    };
    // For embedded mode, get the target name for mock imports
    let embedded_target_name = if let SchemaModuleType::EmbeddedInTarget(ref c) = config.output.schema_types.module_type {
        Some(c.name.clone())
    } else {
        None
    };
    let include_schema_docs = matches!(
        config.options.schema_documentation,
        SchemaDocumentation::Include
    );
    if !include_schema_docs {
        ir.clear_field_descriptions();
    }
    let query_string_format = match config.options.query_string_literal_format {
        QueryStringLiteralFormat::SingleLine => {
            apollo_codegen_render::templates::operation::QueryStringFormat::SingleLine
        }
        QueryStringLiteralFormat::Multiline => {
            apollo_codegen_render::templates::operation::QueryStringFormat::Multiline
        }
    };
    let camel_case_input_objects = matches!(
        config.options.conversion_strategies.input_objects,
        apollo_codegen_config::types::InputObjectConversionStrategy::CamelCase
    );

    let schema_output_path = resolve_path(root_url, &config.output.schema_types.path);

    // 5b. Build schema customizer
    let customizer = SchemaCustomizer::new(&config.options.schema_customization);

    // 6. Generate files
    let mut result = GenerationResult::new();
    let t_gen = Instant::now();

    // Namespace file for embeddedInTarget
    if is_embedded {
        let sources_path = schema_output_path.to_path_buf();
        let namespace_content = format!(
            "{}\n\n{}enum {} {{ }}\n",
            apollo_codegen_render::templates::header::HEADER,
            &access_mod,
            &ns,
        );
        result.add_file(
            sources_path.join(format!("{}.graphql.swift", &ns)),
            namespace_content,
        );
    }

    // Schema type files
    generate_schema_files(
        &mut result,
        &compilation_result,
        &ir,
        &schema_output_path,
        &ns,
        api_target,
        &access_mod,
        is_in_module,
        camel_case_enums,
        camel_case_input_objects,
        config,
        &type_kinds,
        &customizer,
        include_schema_docs,
        is_embedded,
        ops_in_schema_module,
        options.skip_schema_configuration,
        options.skip_custom_scalars,
    );

    // Package.swift (for SPM module type)
    generate_module_files(&mut result, config, &schema_output_path, &ns);

    // Operation and fragment files
    generate_operation_files(
        &mut result,
        &compilation_result,
        &ir,
        &schema_output_path,
        &ns,
        &ops_access_mod,
        config,
        &type_kinds,
        &customizer,
        query_string_format,
        api_target,
        is_embedded,
        root_url,
        ops_in_schema_module,
        embedded_target_name.as_deref(),
        include_schema_docs,
    );

    generate_fragment_files(
        &mut result,
        &compilation_result,
        &ir,
        &schema_output_path,
        &ns,
        &ops_access_mod,
        config,
        &type_kinds,
        &customizer,
        query_string_format,
        api_target,
        is_embedded,
        root_url,
        ops_in_schema_module,
        embedded_target_name.as_deref(),
        include_schema_docs,
    );

    // Test mock files
    generate_test_mock_files(
        &mut result,
        &compilation_result,
        config,
        root_url,
        &ns,
        api_target,
        &mock_access_mod,
        &customizer,
        embedded_target_name.as_deref(),
    );

    let _t = timer.record("Code generation", t_gen);
    timer.print_summary(result.file_count());

    Ok(result)
}

/// Generate only schema type files (no operations or fragments).
///
/// Produces: Objects, Enums, Unions, InputObjects, Interfaces, CustomScalars,
/// SchemaMetadata, SchemaConfiguration, Package.swift, MockObjects.
pub fn generate_schema_only(config: &ApolloCodegenConfiguration, root_url: &Path, options: &GenerateOptions) -> anyhow::Result<GenerationResult> {
    let timing = options.timing;
    let mut timer = PipelineTimer::new(timing);
    let mut t = Instant::now();

    // 1. Discover schema files (operations still needed for compilation to determine referenced types)
    let schema_files = apollo_codegen_glob::match_search_paths(
        &config.input.schema_search_paths,
        Some(root_url),
    )?;
    let operation_files = apollo_codegen_glob::match_search_paths(
        &config.input.operation_search_paths,
        Some(root_url),
    )?;

    if schema_files.is_empty() {
        anyhow::bail!("No schema files found");
    }

    // 2. Load schema
    let schema_sources: Vec<(String, String)> = schema_files
        .iter()
        .map(|path| {
            let content = std::fs::read_to_string(path)?;
            Ok((content, path.clone()))
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;

    let frontend = GraphQLFrontend::load_schema(&schema_sources)
        .map_err(|errs| anyhow::anyhow!("Schema errors: {}", errs.join(", ")))?;

    t = timer.record("Schema loading", t);

    // Parse operations if available (needed for referenced types / compilation)
    let op_sources: Vec<(String, String)> = operation_files
        .iter()
        .map(|path| {
            let content = std::fs::read_to_string(path)?;
            Ok((content, path.clone()))
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;

    let source_map: BTreeMap<String, (String, String)> = op_sources
        .iter()
        .map(|(content, path)| (path.clone(), (content.clone(), path.clone())))
        .collect();

    let doc = if !op_sources.is_empty() {
        Some(frontend
            .parse_operations(&op_sources)
            .map_err(|errs| anyhow::anyhow!("Parse errors: {}", errs.join(", ")))?)
    } else {
        None
    };

    t = timer.record("Operation parsing", t);

    // 3. Compile
    let compile_options = CompileOptions {
        legacy_safelisting_compatible_operations: config
            .experimental_features
            .legacy_safelisting_compatible_operations,
        reduce_generated_schema_types: config.options.reduce_generated_schema_types,
    };

    let compilation_result = if let Some(ref parsed_doc) = doc {
        frontend
            .compile(parsed_doc, &source_map, &compile_options)
            .map_err(|errs| anyhow::anyhow!("Compilation errors: {}", errs.join(", ")))?
    } else {
        // Compile with empty operations - schema types only
        let empty_source_map = BTreeMap::new();
        let empty_doc = frontend.parse_operations(&[])
            .map_err(|errs| anyhow::anyhow!("Parse errors: {}", errs.join(", ")))?;
        frontend
            .compile(&empty_doc, &empty_source_map, &compile_options)
            .map_err(|errs| anyhow::anyhow!("Compilation errors: {}", errs.join(", ")))?
    };

    t = timer.record("Compilation", t);

    // 4. Build IR
    let camel_case_enums = matches!(
        config.options.conversion_strategies.enum_cases,
        EnumCaseConversionStrategy::CamelCase
    );
    let mut ir = IRBuilder::build(&compilation_result, camel_case_enums);
    let type_kinds = apollo_codegen_ir::field_collector::build_type_kinds(&compilation_result);

    let _ = timer.record("IR building", t);

    // 5. Output configuration
    let ns = naming::first_uppercased(&config.schema_namespace);
    let api_target = if config.options.cocoapods_compatible_import_statements {
        "Apollo"
    } else {
        "ApolloAPI"
    };
    let access_mod = determine_access_modifier(config);
    let is_in_module = matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::SwiftPackageManager(_)
    ) || matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::Other(_)
    );
    let is_embedded = matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::EmbeddedInTarget(_)
    );
    let ops_in_schema_module = matches!(
        config.output.operations,
        OperationsFileOutput::InSchemaModule(_)
    );
    let mock_access_mod = if is_embedded {
        "public ".to_string()
    } else {
        access_mod.clone()
    };
    let embedded_target_name = if let SchemaModuleType::EmbeddedInTarget(ref c) = config.output.schema_types.module_type {
        Some(c.name.clone())
    } else {
        None
    };
    let include_schema_docs = matches!(
        config.options.schema_documentation,
        SchemaDocumentation::Include
    );
    if !include_schema_docs {
        ir.clear_field_descriptions();
    }
    let camel_case_input_objects = matches!(
        config.options.conversion_strategies.input_objects,
        apollo_codegen_config::types::InputObjectConversionStrategy::CamelCase
    );

    let schema_output_path = resolve_path(root_url, &config.output.schema_types.path);
    let customizer = SchemaCustomizer::new(&config.options.schema_customization);

    // 6. Generate schema files only
    let mut result = GenerationResult::new();
    let t_gen = Instant::now();

    // Namespace file for embeddedInTarget
    if is_embedded {
        let sources_path = schema_output_path.to_path_buf();
        let namespace_content = format!(
            "{}\n\n{}enum {} {{ }}\n",
            apollo_codegen_render::templates::header::HEADER,
            &access_mod,
            &ns,
        );
        result.add_file(
            sources_path.join(format!("{}.graphql.swift", &ns)),
            namespace_content,
        );
    }

    generate_schema_files(
        &mut result,
        &compilation_result,
        &ir,
        &schema_output_path,
        &ns,
        api_target,
        &access_mod,
        is_in_module,
        camel_case_enums,
        camel_case_input_objects,
        config,
        &type_kinds,
        &customizer,
        include_schema_docs,
        is_embedded,
        ops_in_schema_module,
        options.skip_schema_configuration,
        options.skip_custom_scalars,
    );

    generate_module_files(&mut result, config, &schema_output_path, &ns);

    generate_test_mock_files(
        &mut result,
        &compilation_result,
        config,
        root_url,
        &ns,
        api_target,
        &mock_access_mod,
        &customizer,
        embedded_target_name.as_deref(),
    );

    let _t = timer.record("Code generation", t_gen);
    timer.print_summary(result.file_count());

    Ok(result)
}

/// Generate only operation and fragment files, optionally filtered to specific source paths.
///
/// Still runs the full frontend (schema + all operations) for type resolution,
/// but only renders operations/fragments whose source file matches `only_for_paths`.
/// If `only_for_paths` is empty, renders all operations and fragments.
pub fn generate_operations_only(
    config: &ApolloCodegenConfiguration,
    root_url: &Path,
    only_for_paths: &[String],
    timing: bool,
) -> anyhow::Result<GenerationResult> {
    let mut timer = PipelineTimer::new(timing);
    let mut t = Instant::now();

    // 1. Discover files
    let schema_files = apollo_codegen_glob::match_search_paths(
        &config.input.schema_search_paths,
        Some(root_url),
    )?;
    let operation_files = apollo_codegen_glob::match_search_paths(
        &config.input.operation_search_paths,
        Some(root_url),
    )?;

    if schema_files.is_empty() {
        anyhow::bail!("No schema files found");
    }
    if operation_files.is_empty() {
        anyhow::bail!("No operation files found");
    }

    // 2. Load schema and parse all operations (needed for type resolution)
    let schema_sources: Vec<(String, String)> = schema_files
        .iter()
        .map(|path| {
            let content = std::fs::read_to_string(path)?;
            Ok((content, path.clone()))
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;

    let frontend = GraphQLFrontend::load_schema(&schema_sources)
        .map_err(|errs| anyhow::anyhow!("Schema errors: {}", errs.join(", ")))?;

    t = timer.record("Schema loading", t);

    let op_sources: Vec<(String, String)> = operation_files
        .iter()
        .map(|path| {
            let content = std::fs::read_to_string(path)?;
            Ok((content, path.clone()))
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;

    let source_map: BTreeMap<String, (String, String)> = op_sources
        .iter()
        .map(|(content, path)| (path.clone(), (content.clone(), path.clone())))
        .collect();

    let doc = frontend
        .parse_operations(&op_sources)
        .map_err(|errs| anyhow::anyhow!("Parse errors: {}", errs.join(", ")))?;

    t = timer.record("Operation parsing", t);

    // 3. Compile (full compilation for type resolution)
    let compile_options = CompileOptions {
        legacy_safelisting_compatible_operations: config
            .experimental_features
            .legacy_safelisting_compatible_operations,
        reduce_generated_schema_types: config.options.reduce_generated_schema_types,
    };

    let compilation_result = frontend
        .compile(&doc, &source_map, &compile_options)
        .map_err(|errs| anyhow::anyhow!("Compilation errors: {}", errs.join(", ")))?;

    t = timer.record("Compilation", t);

    // 4. Build IR
    let camel_case_enums = matches!(
        config.options.conversion_strategies.enum_cases,
        EnumCaseConversionStrategy::CamelCase
    );
    let mut ir = IRBuilder::build(&compilation_result, camel_case_enums);
    let type_kinds = apollo_codegen_ir::field_collector::build_type_kinds(&compilation_result);

    let _ = timer.record("IR building", t);

    // 5. Output configuration
    let ns = naming::first_uppercased(&config.schema_namespace);
    let api_target = if config.options.cocoapods_compatible_import_statements {
        "Apollo"
    } else {
        "ApolloAPI"
    };
    let access_mod = determine_access_modifier(config);
    let _is_in_module = matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::SwiftPackageManager(_)
    ) || matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::Other(_)
    );
    let is_embedded = matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::EmbeddedInTarget(_)
    );
    let ops_in_schema_module = matches!(
        config.output.operations,
        OperationsFileOutput::InSchemaModule(_)
    );
    let ops_access_mod = if is_embedded && !ops_in_schema_module {
        "public ".to_string()
    } else {
        access_mod.clone()
    };
    let embedded_target_name = if let SchemaModuleType::EmbeddedInTarget(ref c) = config.output.schema_types.module_type {
        Some(c.name.clone())
    } else {
        None
    };
    let include_schema_docs = matches!(
        config.options.schema_documentation,
        SchemaDocumentation::Include
    );
    if !include_schema_docs {
        ir.clear_field_descriptions();
    }
    let query_string_format = match config.options.query_string_literal_format {
        QueryStringLiteralFormat::SingleLine => {
            apollo_codegen_render::templates::operation::QueryStringFormat::SingleLine
        }
        QueryStringLiteralFormat::Multiline => {
            apollo_codegen_render::templates::operation::QueryStringFormat::Multiline
        }
    };

    let schema_output_path = resolve_path(root_url, &config.output.schema_types.path);
    let customizer = SchemaCustomizer::new(&config.options.schema_customization);

    // 6. Build glob patterns for filtering
    let path_matchers: Vec<glob::Pattern> = only_for_paths
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    // 7. Generate filtered operation/fragment files
    let mut result = GenerationResult::new();
    let t_gen = Instant::now();

    generate_operation_files_filtered(
        &mut result,
        &compilation_result,
        &ir,
        &schema_output_path,
        &ns,
        &ops_access_mod,
        config,
        &type_kinds,
        &customizer,
        query_string_format,
        api_target,
        is_embedded,
        root_url,
        ops_in_schema_module,
        embedded_target_name.as_deref(),
        include_schema_docs,
        &path_matchers,
    );

    generate_fragment_files_filtered(
        &mut result,
        &compilation_result,
        &ir,
        &schema_output_path,
        &ns,
        &ops_access_mod,
        config,
        &type_kinds,
        &customizer,
        query_string_format,
        api_target,
        is_embedded,
        root_url,
        ops_in_schema_module,
        embedded_target_name.as_deref(),
        include_schema_docs,
        &path_matchers,
    );

    let _t = timer.record("Code generation", t_gen);
    timer.print_summary(result.file_count());

    Ok(result)
}

/// Generate an operation manifest file.
///
/// This produces a JSON manifest of all operations with their identifiers (SHA256 hashes).
/// Used for persisted queries / automatic persisted queries.
pub fn generate_operation_manifest(
    config: &ApolloCodegenConfiguration,
    root_url: &Path,
    verbose: bool,
) -> anyhow::Result<()> {
    let manifest_config = config.operation_manifest.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "operationManifest configuration is required for generate-operation-manifest command"
        )
    })?;

    // 1. Discover files
    let schema_files = apollo_codegen_glob::match_search_paths(
        &config.input.schema_search_paths,
        Some(root_url),
    )?;
    let operation_files = apollo_codegen_glob::match_search_paths(
        &config.input.operation_search_paths,
        Some(root_url),
    )?;

    if schema_files.is_empty() {
        anyhow::bail!("No schema files found");
    }
    if operation_files.is_empty() {
        anyhow::bail!("No operation files found");
    }

    // 2. Load schema and parse operations
    let schema_sources: Vec<(String, String)> = schema_files
        .iter()
        .map(|path| {
            let content = std::fs::read_to_string(path)?;
            Ok((content, path.clone()))
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;

    let frontend = apollo_codegen_frontend::compiler::GraphQLFrontend::load_schema(&schema_sources)
        .map_err(|errs| anyhow::anyhow!("Schema errors: {}", errs.join(", ")))?;

    let op_sources: Vec<(String, String)> = operation_files
        .iter()
        .map(|path| {
            let content = std::fs::read_to_string(path)?;
            Ok((content, path.clone()))
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;

    let source_map: BTreeMap<String, (String, String)> = op_sources
        .iter()
        .map(|(content, path)| (path.clone(), (content.clone(), path.clone())))
        .collect();

    let doc = frontend
        .parse_operations(&op_sources)
        .map_err(|errs| anyhow::anyhow!("Parse errors: {}", errs.join(", ")))?;

    // 3. Compile
    let compile_options = apollo_codegen_frontend::compiler::CompileOptions {
        legacy_safelisting_compatible_operations: config
            .experimental_features
            .legacy_safelisting_compatible_operations,
        reduce_generated_schema_types: config.options.reduce_generated_schema_types,
    };

    let compilation_result = frontend
        .compile(&doc, &source_map, &compile_options)
        .map_err(|errs| anyhow::anyhow!("Compilation errors: {}", errs.join(", ")))?;

    // 4. Build IR to get operation sources
    let camel_case_enums = matches!(
        config.options.conversion_strategies.enum_cases,
        EnumCaseConversionStrategy::CamelCase
    );
    let ir = IRBuilder::build(&compilation_result, camel_case_enums);

    // 5. Generate manifest entries
    let mut entries: Vec<serde_json::Value> = Vec::new();

    for op_def in &compilation_result.operations {
        let operation = ir.build_operation(op_def);
        // Skip local cache mutations - they don't go in the manifest
        if operation.is_local_cache_mutation {
            continue;
        }

        let mut hasher = Sha256::new();
        hasher.update(operation.source.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let type_suffix = match op_def.operation_type {
            OperationType::Query => "Query",
            OperationType::Mutation => "Mutation",
            OperationType::Subscription => "Subscription",
        };
        let operation_name = if operation.name.ends_with(type_suffix) {
            operation.name.clone()
        } else {
            format!("{}{}", operation.name, type_suffix)
        };

        match manifest_config.version {
            OperationManifestVersion::PersistedQueries => {
                entries.push(serde_json::json!({
                    "id": hash,
                    "body": operation.source,
                    "name": operation_name,
                    "type": match op_def.operation_type {
                        OperationType::Query => "query",
                        OperationType::Mutation => "mutation",
                        OperationType::Subscription => "subscription",
                    }
                }));
            }
            OperationManifestVersion::Legacy => {
                entries.push(serde_json::json!({
                    "operationIdentifier": hash,
                    "operationName": operation_name,
                    "sourceText": operation.source,
                }));
            }
        }
    }

    // 6. Write manifest file
    let manifest_path = resolve_path(root_url, &manifest_config.path);
    if let Some(parent) = manifest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let manifest = match manifest_config.version {
        OperationManifestVersion::PersistedQueries => {
            serde_json::json!({
                "format": "apollo-persisted-query-manifest",
                "version": 1,
                "operations": entries,
            })
        }
        OperationManifestVersion::Legacy => {
            serde_json::Value::Array(entries)
        }
    };

    let manifest_str = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_path, manifest_str)?;

    if verbose {
        eprintln!("Operation manifest written to: {}", manifest_path.display());
    }
    eprintln!("Generated operation manifest with {} operation(s)", compilation_result.operations.len());

    Ok(())
}

fn determine_access_modifier(config: &ApolloCodegenConfiguration) -> String {
    match &config.output.schema_types.module_type {
        SchemaModuleType::EmbeddedInTarget(c) => match c.access_modifier {
            AccessModifier::Public => "public ".to_string(),
            AccessModifier::Internal => String::new(),
        },
        SchemaModuleType::SwiftPackageManager(_) | SchemaModuleType::Other(_) => {
            "public ".to_string()
        }
    }
}

fn resolve_path(root: &Path, relative: &str) -> PathBuf {
    let p = PathBuf::from(relative);
    if p.is_absolute() {
        p
    } else {
        root.join(relative)
    }
}

/// Convert a source string to a single line, matching Swift's `convertedToSingleLine()`.
/// Splits by newlines, trims whitespace from each line, and joins with spaces.
fn convert_to_single_line(source: &str) -> String {
    source
        .lines()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Add a module import line after the existing import lines in a generated file.
fn add_module_import(content: &str, module_name: &str) -> String {
    let import_line = format!("import {}", module_name);
    let mut result = String::new();
    let mut inserted = false;
    for line in content.lines() {
        result.push_str(line);
        result.push('\n');
        // Insert after the @_exported import or import line
        if !inserted && (line.starts_with("@_exported import ") || line.starts_with("import ")) {
            result.push_str(&import_line);
            result.push('\n');
            inserted = true;
        }
    }
    result
}

fn generate_schema_files(
    result: &mut GenerationResult,
    compilation: &CompilationResult,
    ir: &IRBuilder,
    schema_path: &Path,
    ns: &str,
    api_target: &str,
    access_mod: &str,
    is_in_module: bool,
    camel_case_enums: bool,
    camel_case_input_objects: bool,
    config: &ApolloCodegenConfiguration,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
    include_schema_docs: bool,
    is_embedded: bool,
    ops_in_schema_module: bool,
    skip_schema_configuration: bool,
    skip_custom_scalars: bool,
) {
    // Only swiftPackageManager adds a Sources/ subdirectory
    let sources_path = if matches!(config.output.schema_types.module_type, SchemaModuleType::SwiftPackageManager(_)) {
        schema_path.join("Sources")
    } else {
        schema_path.to_path_buf()
    };

    // Schema/ subdirectory is used when operations are inSchemaModule (to separate schema types
    // from operation files in the same directory), but NOT when operations are relative/absolute
    let schema_subdir = match &config.output.operations {
        OperationsFileOutput::InSchemaModule(_) => "Schema/",
        _ => "",
    };

    // Objects
    for named_type in &compilation.referenced_types {
        if let GraphQLNamedType::Object(obj) = named_type {
            let swift_name = customizer.custom_type_name(&obj.name);
            // Customize interface names referenced by this object
            let custom_interfaces: Vec<String> = obj.interfaces
                .iter()
                .map(|iface| customizer.custom_type_name(iface).to_string())
                .collect();
            let doc = if include_schema_docs { obj.description.as_deref() } else { None };
            let content = templates::object::render(
                swift_name,
                &obj.name, // GraphQL typename stays original
                &custom_interfaces,
                access_mod,
                api_target,
                &config.schema_namespace,
                is_in_module,
                doc,
            );
            let file_path = sources_path
                .join(format!("{}Objects", schema_subdir))
                .join(format!("{}.graphql.swift", naming::first_uppercased(swift_name)));
            result.add_file(file_path, content);
        }
    }

    // Interfaces
    for named_type in &compilation.referenced_types {
        if let GraphQLNamedType::Interface(iface) = named_type {
            let swift_name = customizer.custom_type_name(&iface.name);
            let doc = if include_schema_docs { iface.description.as_deref() } else { None };
            let content = templates::interface::render(
                swift_name,
                &iface.name, // GraphQL name stays original
                access_mod,
                api_target,
                doc,
                &config.schema_namespace,
                is_in_module,
            );
            let file_path = sources_path
                .join(format!("{}Interfaces", schema_subdir))
                .join(format!("{}.graphql.swift", naming::first_uppercased(swift_name)));
            result.add_file(file_path, content);
        }
    }

    // Unions
    for named_type in &compilation.referenced_types {
        if let GraphQLNamedType::Union(union_t) = named_type {
            let swift_name = customizer.custom_type_name(&union_t.name);
            // Customize member type names referenced by this union
            let custom_members: Vec<String> = union_t.member_types
                .iter()
                .map(|m| customizer.custom_type_name(m).to_string())
                .collect();
            let doc = if include_schema_docs { union_t.description.as_deref() } else { None };
            let content = templates::union_type::render(
                swift_name,
                &union_t.name, // GraphQL name stays original
                &custom_members,
                access_mod,
                api_target,
                &config.schema_namespace,
                is_in_module,
                doc,
            );
            let file_path = sources_path
                .join(format!("{}Unions", schema_subdir))
                .join(format!("{}.graphql.swift", naming::first_uppercased(swift_name)));
            result.add_file(file_path, content);
        }
    }

    // Enums
    let exclude_deprecated_enums = matches!(
        config.options.deprecated_enum_cases,
        Composition::Exclude
    );
    for named_type in &compilation.referenced_types {
        if let GraphQLNamedType::Enum(enum_t) = named_type {
            let swift_name = customizer.custom_type_name(&enum_t.name);
            let values: Vec<templates::enum_type::EnumValue> = enum_t
                .values
                .iter()
                .filter(|v| !exclude_deprecated_enums || !v.is_deprecated)
                .map(|v| {
                    let custom_case = customizer.custom_enum_case(&enum_t.name, &v.name);
                    let is_renamed = custom_case != v.name;
                    templates::enum_type::EnumValue {
                        name: custom_case.to_string(),
                        raw_value: v.name.clone(), // GraphQL value stays original
                        description: if include_schema_docs { v.description.clone() } else { None },
                        is_deprecated: v.is_deprecated,
                        deprecation_reason: v.deprecation_reason.clone(),
                        is_renamed,
                    }
                })
                .collect();

            let enum_doc = if include_schema_docs { enum_t.description.as_deref() } else { None };
            let mut content = templates::enum_type::render(
                swift_name,
                &enum_t.name, // GraphQL schema name
                &values,
                access_mod,
                api_target,
                camel_case_enums,
                enum_doc,
            );
            if is_embedded {
                content = apollo_codegen_render::templates::header::wrap_in_namespace_extension(
                    &content, ns, access_mod,
                );
            }
            let file_path = sources_path
                .join(format!("{}Enums", schema_subdir))
                .join(format!("{}.graphql.swift", naming::first_uppercased(swift_name)));
            result.add_file(file_path, content);
        }
    }

    // Input Objects
    for named_type in &compilation.referenced_types {
        if let GraphQLNamedType::InputObject(input) = named_type {
            let swift_name = customizer.custom_type_name(&input.name);
            let fields: Vec<templates::input_object::InputField> = input
                .fields
                .iter()
                .map(|(fname, fdef)| {
                    let custom_field_name = customizer.custom_input_field(&input.name, fname);
                    let is_renamed = custom_field_name != fname;
                    // Apply camelCase conversion if enabled and no explicit customization.
                    // Only converts snake_case names (containing '_'). Names already in
                    // camelCase (like ownerID) are left unchanged.
                    let rendered_name = if camel_case_input_objects && custom_field_name == fname && fname.contains('_') {
                        naming::to_camel_case(fname)
                    } else {
                        custom_field_name.to_string()
                    };
                    let mut swift_type = render_input_field_type(&fdef.field_type, ns, &type_kinds, customizer);
                    // Non-null fields with default values become optional
                    if matches!(fdef.field_type, GraphQLType::NonNull(_)) && fdef.default_value.is_some() {
                        swift_type = format!("{}?", swift_type);
                    }
                    let mut init_type = render_input_field_init_type(&fdef.field_type, ns, &fdef.default_value, &type_kinds, customizer);
                    // Add namespace prefix when operations are outside the schema module
                    if !ops_in_schema_module {
                        let prefix = format!("{}.", ns);
                        swift_type = ir_adapter::add_namespace_to_variable_type(&swift_type, &prefix, type_kinds, customizer);
                        init_type = ir_adapter::add_namespace_to_variable_type(&init_type, &prefix, type_kinds, customizer);
                    }
                    templates::input_object::InputField {
                        schema_name: fname.clone(), // GraphQL field name for __data access
                        rendered_name,
                        rendered_type: swift_type,
                        rendered_init_type: init_type,
                        description: if include_schema_docs { fdef.description.clone() } else { None },
                        deprecation_reason: fdef.deprecation_reason.clone(),
                        is_renamed,
                    }
                })
                .collect();

            let mut content = templates::input_object::render(
                swift_name,
                &input.name, // GraphQL schema name
                &fields,
                access_mod,
                api_target,
                config.options.warnings_on_deprecated_usage == apollo_codegen_config::types::Composition::Include,
                if include_schema_docs { input.description.as_deref() } else { None },
            );
            if is_embedded {
                content = apollo_codegen_render::templates::header::wrap_in_namespace_extension(
                    &content, ns, access_mod,
                );
            }
            let file_path = sources_path
                .join(format!("{}InputObjects", schema_subdir))
                .join(format!("{}.graphql.swift", naming::first_uppercased(swift_name)));
            result.add_file(file_path, content);
        }
    }

    // Custom Scalars (skip if --skip-custom-scalars)
    if !skip_custom_scalars {
    for named_type in &compilation.referenced_types {
        if let GraphQLNamedType::Scalar(scalar) = named_type {
            let swift_name = customizer.custom_type_name(&scalar.name);
            // Skip built-in scalars
            if matches!(
                scalar.name.as_str(),
                "String" | "Int" | "Float" | "Boolean" | "ID"
            ) {
                // ID is a custom scalar in Apollo
                if scalar.name == "ID" {
                    let mut content = templates::custom_scalar::render(
                        swift_name,
                        if include_schema_docs { scalar.description.as_deref() } else { None },
                        if include_schema_docs { scalar.specified_by_url.as_deref() } else { None },
                        access_mod,
                        api_target,
                    );
                    if is_embedded {
                        content = apollo_codegen_render::templates::header::wrap_in_namespace_extension(
                            &content, ns, access_mod,
                        );
                    }
                    let file_path = sources_path
                        .join(format!("{}CustomScalars", schema_subdir))
                        .join(format!("{}.swift", naming::first_uppercased(swift_name)));
                    result.add_file(file_path, content);
                }
                continue;
            }
            let mut content = templates::custom_scalar::render(
                swift_name,
                if include_schema_docs { scalar.description.as_deref() } else { None },
                if include_schema_docs { scalar.specified_by_url.as_deref() } else { None },
                access_mod,
                api_target,
            );
            if is_embedded {
                content = apollo_codegen_render::templates::header::wrap_in_namespace_extension(
                    &content, ns, access_mod,
                );
            }
            let file_path = sources_path
                .join(format!("{}CustomScalars", schema_subdir))
                .join(format!("{}.swift", naming::first_uppercased(swift_name)));
            result.add_file(file_path, content);
        }
    }
    } // end if !skip_custom_scalars

    // SchemaMetadata
    let object_types: Vec<(String, String)> = compilation
        .referenced_types
        .iter()
        .filter_map(|t| {
            if let GraphQLNamedType::Object(obj) = t {
                let swift_name = customizer.custom_type_name(&obj.name).to_string();
                Some((obj.name.clone(), swift_name))
            } else {
                None
            }
        })
        .collect();

    let content = templates::schema_metadata::render(
        &config.schema_namespace,
        &object_types,
        access_mod,
        api_target,
        is_embedded,
    );
    result.add_file(
        sources_path.join(format!("{}SchemaMetadata.graphql.swift", schema_subdir)),
        content,
    );

    // SchemaConfiguration (skip if --skip-schema-configuration)
    if !skip_schema_configuration {
        let content = templates::schema_config::render(access_mod, api_target, is_embedded);
        result.add_file(
            sources_path.join(format!("{}SchemaConfiguration.swift", schema_subdir)),
            content,
        );
    }
}

fn generate_module_files(
    result: &mut GenerationResult,
    config: &ApolloCodegenConfiguration,
    schema_path: &Path,
    ns: &str,
) {
    if let SchemaModuleType::SwiftPackageManager(_) = &config.output.schema_types.module_type {
        let test_mock_target = match &config.output.test_mocks {
            TestMockFileOutput::SwiftPackage(pkg) => {
                let target_name = pkg
                    .target_name
                    .as_deref()
                    .map(|n| naming::first_uppercased(n))
                    .unwrap_or_else(|| format!("{}TestMocks", ns));
                Some((target_name.clone(), format!("./{}", target_name)))
            }
            _ => None,
        };

        let content = templates::package_swift::render(
            &config.schema_namespace,
            test_mock_target
                .as_ref()
                .map(|(name, path)| (name.as_str(), path.as_str())),
        );
        result.add_file(schema_path.join("Package.swift"), content);
    }
}

fn generate_operation_files(
    result: &mut GenerationResult,
    compilation: &CompilationResult,
    ir: &IRBuilder,
    schema_path: &Path,
    ns: &str,
    access_mod: &str,
    config: &ApolloCodegenConfiguration,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
    query_string_format: apollo_codegen_render::templates::operation::QueryStringFormat,
    api_target: &str,
    is_embedded: bool,
    root_url: &Path,
    ops_in_schema_module: bool,
    embedded_target_name: Option<&str>,
    include_schema_docs: bool,
) {
    let sources_path = if matches!(config.output.schema_types.module_type, SchemaModuleType::SwiftPackageManager(_)) {
        schema_path.join("Sources")
    } else {
        schema_path.to_path_buf()
    };

    // Check if operations output mode is "relative" or "absolute"
    let relative_subpath = match &config.output.operations {
        OperationsFileOutput::Relative(c) => Some(c.subpath.clone()),
        _ => None,
    };
    let absolute_ops_path = match &config.output.operations {
        OperationsFileOutput::Absolute(c) => Some(resolve_path(root_url, &c.path)),
        _ => None,
    };

    for op_def in &compilation.operations {
        let mut operation = ir.build_operation(op_def);
        // Apply schema customization to variable default values
        for var in &mut operation.variables {
            if let Some(ref mut dv) = var.default_value {
                *dv = customizer.customize_default_value(dv);
            }
            var.type_str = customizer.customize_variable_type(&var.type_str);
        }
        // Determine whether to generate initializers based on config.
        // Mirrors Swift's shouldGenerateSelectionSetInitializers(for:):
        //   guard experimentalFeatures.fieldMerging == .all else { return false }
        //   if isLocalCacheMutation { return true }  // always generate inits
        //   else check selectionSetInitializers.operations
        let generate_init = if !config.experimental_features.field_merging.is_all() {
            false
        } else if operation.is_local_cache_mutation {
            true
        } else {
            config.options.selection_set_initializers.operations
        };
        // Compute operation identifier (SHA256 hash of source) when configured
        let op_id = if config.options.operation_document_format.operation_identifier {
            let mut hasher = Sha256::new();
            // Hash the operation source + all referenced fragment sources (matching Swift's rawSource format).
            // Swift's convertedToSingleLine() splits by newlines, trims whitespace, joins with spaces.
            let single_line = convert_to_single_line(&operation.source);
            hasher.update(single_line.as_bytes());
            // Sort referenced fragments alphabetically by name (matching Swift's allReferencedFragments)
            let mut sorted_frags: Vec<_> = operation.referenced_fragments.iter().collect();
            sorted_frags.sort_by(|a, b| a.name.cmp(&b.name));
            for frag in &sorted_frags {
                hasher.update(b"\n");
                let frag_single_line = convert_to_single_line(&frag.source);
                hasher.update(frag_single_line.as_bytes());
            }
            Some(format!("{:x}", hasher.finalize()))
        } else {
            None
        };
        // When operations are not in the schema module (relative/absolute),
        // variable types need the schema namespace prefix (e.g., "MySchemaModule.ID")
        let var_prefix = if !ops_in_schema_module {
            format!("{}.", ns)
        } else {
            String::new()
        };
        // For embeddedInTarget, init/variables/__variables always need "public "
        let init_mod: Option<&str> = if is_embedded { Some("public ") } else { None };
        let mut content = ir_adapter::render_operation(
            &operation,
            ns,
            access_mod,
            generate_init,
            type_kinds,
            customizer,
            config.options.operation_document_format.definition,
            op_id.as_deref(),
            query_string_format,
            api_target,
            false, // markOperationDefinitionsAsFinal
            &var_prefix,
            init_mod,
        );

        // Strip parent type doc comments when schema docs excluded
        if !include_schema_docs {
            content = strip_parent_type_comments(&content);
        }

        // Wrap in namespace extension for embeddedInTarget with inSchemaModule operations
        if is_embedded && ops_in_schema_module {
            content = apollo_codegen_render::templates::header::wrap_in_namespace_extension(
                &content, ns, access_mod,
            );
        } else if !ops_in_schema_module {
            // When operations are outside the schema module, add import for the schema module
            if is_embedded {
                if let Some(target) = embedded_target_name {
                    content = add_module_import(&content, target);
                }
            } else {
                content = add_module_import(&content, ns);
            }
        }

        let subdir = match op_def.operation_type {
            OperationType::Query => "Queries",
            OperationType::Mutation => "Mutations",
            OperationType::Subscription => "Subscriptions",
        };

        // File name uses operation name + type suffix (e.g., "ClassroomPetsQuery")
        let type_suffix = match op_def.operation_type {
            OperationType::Query => "Query",
            OperationType::Mutation => "Mutation",
            OperationType::Subscription => "Subscription",
        };
        let file_name = if operation.name.ends_with(type_suffix) {
            operation.name.clone()
        } else {
            format!("{}{}", operation.name, type_suffix)
        };

        if operation.is_local_cache_mutation {
            // Local cache mutations use the operation name without type suffix
            if let Some(ref subpath_opt) = relative_subpath {
                let source_dir = Path::new(&op_def.file_path).parent().unwrap_or(Path::new(""));
                let file_path = if let Some(subpath) = subpath_opt {
                    source_dir.join(subpath).join(format!("{}.graphql.swift", operation.name))
                } else {
                    source_dir.join(format!("{}.graphql.swift", operation.name))
                };
                result.add_file(file_path, content);
            } else if let Some(ref abs_path) = absolute_ops_path {
                let file_path = abs_path
                    .join("LocalCacheMutations")
                    .join(format!("{}.graphql.swift", operation.name));
                result.add_file(file_path, content);
            } else {
                let file_path = sources_path
                    .join("LocalCacheMutations")
                    .join(format!("{}.graphql.swift", operation.name));
                result.add_file(file_path, content);
            }
        } else {
            if let Some(ref subpath_opt) = relative_subpath {
                // Relative mode: place file next to .graphql source file
                let source_dir = Path::new(&op_def.file_path).parent().unwrap_or(Path::new(""));
                let file_path = if let Some(subpath) = subpath_opt {
                    source_dir.join(subpath).join(format!("{}.graphql.swift", file_name))
                } else {
                    source_dir.join(format!("{}.graphql.swift", file_name))
                };
                result.add_file(file_path, content);
            } else if let Some(ref abs_path) = absolute_ops_path {
                let file_path = abs_path
                    .join(subdir)
                    .join(format!("{}.graphql.swift", file_name));
                result.add_file(file_path, content);
            } else {
                let file_path = sources_path
                    .join(format!("Operations/{}", subdir))
                    .join(format!("{}.graphql.swift", file_name));
                result.add_file(file_path, content);
            }
        }
    }
}

fn generate_fragment_files(
    result: &mut GenerationResult,
    compilation: &CompilationResult,
    ir: &IRBuilder,
    schema_path: &Path,
    ns: &str,
    access_mod: &str,
    config: &ApolloCodegenConfiguration,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
    query_string_format: apollo_codegen_render::templates::operation::QueryStringFormat,
    api_target: &str,
    is_embedded: bool,
    root_url: &Path,
    ops_in_schema_module: bool,
    embedded_target_name: Option<&str>,
    include_schema_docs: bool,
) {
    let sources_path = if matches!(config.output.schema_types.module_type, SchemaModuleType::SwiftPackageManager(_)) {
        schema_path.join("Sources")
    } else {
        schema_path.to_path_buf()
    };

    // Check if operations output mode is "relative" or "absolute"
    let relative_subpath = match &config.output.operations {
        OperationsFileOutput::Relative(c) => Some(c.subpath.clone()),
        _ => None,
    };
    let absolute_ops_path = match &config.output.operations {
        OperationsFileOutput::Absolute(c) => Some(resolve_path(root_url, &c.path)),
        _ => None,
    };

    for frag_def in &compilation.fragments {
        if let Some(frag) = ir.fragments().get(&frag_def.name) {
            // Determine whether to generate initializers based on config.
            // Mirrors Swift's shouldGenerateSelectionSetInitializers(for:):
            //   guard experimentalFeatures.fieldMerging == .all else { return false }
            //   if namedFragments flag { return true }
            //   if isLocalCacheMutation { return true }  // always generate inits
            //   else check per-definition name
            let generate_init = if !config.experimental_features.field_merging.is_all() {
                false
            } else if config.options.selection_set_initializers.named_fragments {
                true
            } else if frag.is_local_cache_mutation {
                true
            } else {
                false
            };
            let mut content = ir_adapter::render_fragment(
                frag,
                ns,
                access_mod,
                generate_init,
                type_kinds,
                customizer,
                query_string_format,
                api_target,
                config.options.operation_document_format.definition,
            );

            // Strip parent type doc comments when schema docs excluded
            if !include_schema_docs {
                content = strip_parent_type_comments(&content);
            }

            // Wrap in namespace extension for embeddedInTarget with inSchemaModule operations
            if is_embedded && ops_in_schema_module {
                content = apollo_codegen_render::templates::header::wrap_in_namespace_extension(
                    &content, ns, access_mod,
                );
            } else if !ops_in_schema_module {
                // When operations are outside the schema module, add import for the schema module
                if is_embedded {
                    if let Some(target) = embedded_target_name {
                    content = add_module_import(&content, target);
                }
                } else {
                    content = add_module_import(&content, ns);
                }
            }

            let frag_file_name = naming::first_uppercased(&frag.name);
            if frag.is_local_cache_mutation {
                if let Some(ref subpath_opt) = relative_subpath {
                    let source_dir = Path::new(&frag_def.file_path).parent().unwrap_or(Path::new(""));
                    let file_path = if let Some(subpath) = subpath_opt {
                        source_dir.join(subpath).join(format!("{}.graphql.swift", frag_file_name))
                    } else {
                        source_dir.join(format!("{}.graphql.swift", frag_file_name))
                    };
                    result.add_file(file_path, content);
                } else if let Some(ref abs_path) = absolute_ops_path {
                    let file_path = abs_path
                        .join("LocalCacheMutations")
                        .join(format!("{}.graphql.swift", frag_file_name));
                    result.add_file(file_path, content);
                } else {
                    let file_path = sources_path
                        .join("LocalCacheMutations")
                        .join(format!("{}.graphql.swift", frag_file_name));
                    result.add_file(file_path, content);
                }
            } else {
                if let Some(ref subpath_opt) = relative_subpath {
                    let source_dir = Path::new(&frag_def.file_path).parent().unwrap_or(Path::new(""));
                    let file_path = if let Some(subpath) = subpath_opt {
                        source_dir.join(subpath).join(format!("{}.graphql.swift", frag_file_name))
                    } else {
                        source_dir.join(format!("{}.graphql.swift", frag_file_name))
                    };
                    result.add_file(file_path, content);
                } else if let Some(ref abs_path) = absolute_ops_path {
                    let file_path = abs_path
                        .join("Fragments")
                        .join(format!("{}.graphql.swift", frag_file_name));
                    result.add_file(file_path, content);
                } else {
                    let file_path = sources_path
                        .join("Fragments")
                        .join(format!("{}.graphql.swift", frag_file_name));
                    result.add_file(file_path, content);
                }
            }
        }
    }
}

/// Like `generate_operation_files` but filters operations to only those whose source path
/// matches one of the given glob patterns. If `path_matchers` is empty, renders all operations.
#[allow(clippy::too_many_arguments)]
fn generate_operation_files_filtered(
    result: &mut GenerationResult,
    compilation: &CompilationResult,
    ir: &IRBuilder,
    schema_path: &Path,
    ns: &str,
    access_mod: &str,
    config: &ApolloCodegenConfiguration,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
    query_string_format: apollo_codegen_render::templates::operation::QueryStringFormat,
    api_target: &str,
    is_embedded: bool,
    root_url: &Path,
    ops_in_schema_module: bool,
    embedded_target_name: Option<&str>,
    include_schema_docs: bool,
    path_matchers: &[glob::Pattern],
) {
    // If no path filters, delegate to the unfiltered version
    if path_matchers.is_empty() {
        generate_operation_files(
            result, compilation, ir, schema_path, ns, access_mod, config,
            type_kinds, customizer, query_string_format, api_target,
            is_embedded, root_url, ops_in_schema_module, embedded_target_name,
            include_schema_docs,
        );
        return;
    }

    let sources_path = if matches!(config.output.schema_types.module_type, SchemaModuleType::SwiftPackageManager(_)) {
        schema_path.join("Sources")
    } else {
        schema_path.to_path_buf()
    };

    let relative_subpath = match &config.output.operations {
        OperationsFileOutput::Relative(c) => Some(c.subpath.clone()),
        _ => None,
    };
    let absolute_ops_path = match &config.output.operations {
        OperationsFileOutput::Absolute(c) => Some(resolve_path(root_url, &c.path)),
        _ => None,
    };

    for op_def in &compilation.operations {
        // Filter: skip operations whose source file does not match any pattern
        if !path_matchers.iter().any(|pat| pat.matches(&op_def.file_path)) {
            continue;
        }

        let mut operation = ir.build_operation(op_def);
        for var in &mut operation.variables {
            if let Some(ref mut dv) = var.default_value {
                *dv = customizer.customize_default_value(dv);
            }
            var.type_str = customizer.customize_variable_type(&var.type_str);
        }
        let generate_init = if !config.experimental_features.field_merging.is_all() {
            false
        } else if operation.is_local_cache_mutation {
            true
        } else {
            config.options.selection_set_initializers.operations
        };
        let op_id = if config.options.operation_document_format.operation_identifier {
            let mut hasher = Sha256::new();
            let single_line = convert_to_single_line(&operation.source);
            hasher.update(single_line.as_bytes());
            let mut sorted_frags: Vec<_> = operation.referenced_fragments.iter().collect();
            sorted_frags.sort_by(|a, b| a.name.cmp(&b.name));
            for frag in &sorted_frags {
                hasher.update(b"\n");
                let frag_single_line = convert_to_single_line(&frag.source);
                hasher.update(frag_single_line.as_bytes());
            }
            Some(format!("{:x}", hasher.finalize()))
        } else {
            None
        };
        let var_prefix = if !ops_in_schema_module {
            format!("{}.", ns)
        } else {
            String::new()
        };
        let init_mod: Option<&str> = if is_embedded { Some("public ") } else { None };
        let mut content = ir_adapter::render_operation(
            &operation, ns, access_mod, generate_init, type_kinds, customizer,
            config.options.operation_document_format.definition,
            op_id.as_deref(), query_string_format, api_target,
            false, &var_prefix, init_mod,
        );
        if !include_schema_docs {
            content = strip_parent_type_comments(&content);
        }
        if is_embedded && ops_in_schema_module {
            content = apollo_codegen_render::templates::header::wrap_in_namespace_extension(
                &content, ns, access_mod,
            );
        } else if !ops_in_schema_module {
            if is_embedded {
                if let Some(target) = embedded_target_name {
                    content = add_module_import(&content, target);
                }
            } else {
                content = add_module_import(&content, ns);
            }
        }

        let subdir = match op_def.operation_type {
            OperationType::Query => "Queries",
            OperationType::Mutation => "Mutations",
            OperationType::Subscription => "Subscriptions",
        };
        let type_suffix = match op_def.operation_type {
            OperationType::Query => "Query",
            OperationType::Mutation => "Mutation",
            OperationType::Subscription => "Subscription",
        };
        let file_name = if operation.name.ends_with(type_suffix) {
            operation.name.clone()
        } else {
            format!("{}{}", operation.name, type_suffix)
        };

        if operation.is_local_cache_mutation {
            if let Some(ref subpath_opt) = relative_subpath {
                let source_dir = Path::new(&op_def.file_path).parent().unwrap_or(Path::new(""));
                let file_path = if let Some(subpath) = subpath_opt {
                    source_dir.join(subpath).join(format!("{}.graphql.swift", operation.name))
                } else {
                    source_dir.join(format!("{}.graphql.swift", operation.name))
                };
                result.add_file(file_path, content);
            } else if let Some(ref abs_path) = absolute_ops_path {
                let file_path = abs_path.join("LocalCacheMutations").join(format!("{}.graphql.swift", operation.name));
                result.add_file(file_path, content);
            } else {
                let file_path = sources_path.join("LocalCacheMutations").join(format!("{}.graphql.swift", operation.name));
                result.add_file(file_path, content);
            }
        } else {
            if let Some(ref subpath_opt) = relative_subpath {
                let source_dir = Path::new(&op_def.file_path).parent().unwrap_or(Path::new(""));
                let file_path = if let Some(subpath) = subpath_opt {
                    source_dir.join(subpath).join(format!("{}.graphql.swift", file_name))
                } else {
                    source_dir.join(format!("{}.graphql.swift", file_name))
                };
                result.add_file(file_path, content);
            } else if let Some(ref abs_path) = absolute_ops_path {
                let file_path = abs_path.join(subdir).join(format!("{}.graphql.swift", file_name));
                result.add_file(file_path, content);
            } else {
                let file_path = sources_path.join(format!("Operations/{}", subdir)).join(format!("{}.graphql.swift", file_name));
                result.add_file(file_path, content);
            }
        }
    }
}

/// Like `generate_fragment_files` but filters fragments to only those whose source path
/// matches one of the given glob patterns. If `path_matchers` is empty, renders all fragments.
#[allow(clippy::too_many_arguments)]
fn generate_fragment_files_filtered(
    result: &mut GenerationResult,
    compilation: &CompilationResult,
    ir: &IRBuilder,
    schema_path: &Path,
    ns: &str,
    access_mod: &str,
    config: &ApolloCodegenConfiguration,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
    query_string_format: apollo_codegen_render::templates::operation::QueryStringFormat,
    api_target: &str,
    is_embedded: bool,
    root_url: &Path,
    ops_in_schema_module: bool,
    embedded_target_name: Option<&str>,
    include_schema_docs: bool,
    path_matchers: &[glob::Pattern],
) {
    // If no path filters, delegate to the unfiltered version
    if path_matchers.is_empty() {
        generate_fragment_files(
            result, compilation, ir, schema_path, ns, access_mod, config,
            type_kinds, customizer, query_string_format, api_target,
            is_embedded, root_url, ops_in_schema_module, embedded_target_name,
            include_schema_docs,
        );
        return;
    }

    let sources_path = if matches!(config.output.schema_types.module_type, SchemaModuleType::SwiftPackageManager(_)) {
        schema_path.join("Sources")
    } else {
        schema_path.to_path_buf()
    };

    let relative_subpath = match &config.output.operations {
        OperationsFileOutput::Relative(c) => Some(c.subpath.clone()),
        _ => None,
    };
    let absolute_ops_path = match &config.output.operations {
        OperationsFileOutput::Absolute(c) => Some(resolve_path(root_url, &c.path)),
        _ => None,
    };

    for frag_def in &compilation.fragments {
        // Filter: skip fragments whose source file does not match any pattern
        if !path_matchers.iter().any(|pat| pat.matches(&frag_def.file_path)) {
            continue;
        }

        if let Some(frag) = ir.fragments().get(&frag_def.name) {
            let generate_init = if !config.experimental_features.field_merging.is_all() {
                false
            } else if config.options.selection_set_initializers.named_fragments {
                true
            } else if frag.is_local_cache_mutation {
                true
            } else {
                false
            };
            let mut content = ir_adapter::render_fragment(
                frag, ns, access_mod, generate_init, type_kinds, customizer,
                query_string_format, api_target,
                config.options.operation_document_format.definition,
            );
            if !include_schema_docs {
                content = strip_parent_type_comments(&content);
            }
            if is_embedded && ops_in_schema_module {
                content = apollo_codegen_render::templates::header::wrap_in_namespace_extension(
                    &content, ns, access_mod,
                );
            } else if !ops_in_schema_module {
                if is_embedded {
                    if let Some(target) = embedded_target_name {
                        content = add_module_import(&content, target);
                    }
                } else {
                    content = add_module_import(&content, ns);
                }
            }

            let frag_file_name = naming::first_uppercased(&frag.name);
            if frag.is_local_cache_mutation {
                if let Some(ref subpath_opt) = relative_subpath {
                    let source_dir = Path::new(&frag_def.file_path).parent().unwrap_or(Path::new(""));
                    let file_path = if let Some(subpath) = subpath_opt {
                        source_dir.join(subpath).join(format!("{}.graphql.swift", frag_file_name))
                    } else {
                        source_dir.join(format!("{}.graphql.swift", frag_file_name))
                    };
                    result.add_file(file_path, content);
                } else if let Some(ref abs_path) = absolute_ops_path {
                    let file_path = abs_path.join("LocalCacheMutations").join(format!("{}.graphql.swift", frag_file_name));
                    result.add_file(file_path, content);
                } else {
                    let file_path = sources_path.join("LocalCacheMutations").join(format!("{}.graphql.swift", frag_file_name));
                    result.add_file(file_path, content);
                }
            } else {
                if let Some(ref subpath_opt) = relative_subpath {
                    let source_dir = Path::new(&frag_def.file_path).parent().unwrap_or(Path::new(""));
                    let file_path = if let Some(subpath) = subpath_opt {
                        source_dir.join(subpath).join(format!("{}.graphql.swift", frag_file_name))
                    } else {
                        source_dir.join(format!("{}.graphql.swift", frag_file_name))
                    };
                    result.add_file(file_path, content);
                } else if let Some(ref abs_path) = absolute_ops_path {
                    let file_path = abs_path.join("Fragments").join(format!("{}.graphql.swift", frag_file_name));
                    result.add_file(file_path, content);
                } else {
                    let file_path = sources_path.join("Fragments").join(format!("{}.graphql.swift", frag_file_name));
                    result.add_file(file_path, content);
                }
            }
        }
    }
}

fn generate_test_mock_files(
    result: &mut GenerationResult,
    compilation: &CompilationResult,
    config: &ApolloCodegenConfiguration,
    root_url: &Path,
    ns: &str,
    api_target: &str,
    access_mod: &str,
    customizer: &SchemaCustomizer,
    embedded_target_name: Option<&str>,
) {
    // For embedded mode, import the target name; otherwise import the schema namespace
    let import_module = embedded_target_name.unwrap_or(ns);
    let mock_path = match &config.output.test_mocks {
        TestMockFileOutput::None(_) => return,
        TestMockFileOutput::Absolute(abs) => resolve_path(root_url, &abs.path),
        TestMockFileOutput::SwiftPackage(pkg) => {
            // SwiftPackage test mocks go to ./{targetName} relative to schema types path
            let target_name = pkg
                .target_name
                .as_deref()
                .map(|n| naming::first_uppercased(n))
                .unwrap_or_else(|| format!("{}TestMocks", ns));
            resolve_path(root_url, &config.output.schema_types.path)
                .join(&target_name)
        }
    };

    // MockInterfaces
    let interfaces: Vec<String> = compilation
        .referenced_types
        .iter()
        .filter_map(|t| {
            if let GraphQLNamedType::Interface(i) = t {
                Some(customizer.custom_type_name(&i.name).to_string())
            } else {
                None
            }
        })
        .collect();

    if !interfaces.is_empty() {
        let content = templates::mock_interfaces::render(&interfaces, access_mod, ns, import_module);
        result.add_file(
            mock_path.join("MockObject+Interfaces.graphql.swift"),
            content,
        );
    }

    // MockUnions
    let unions: Vec<String> = compilation
        .referenced_types
        .iter()
        .filter_map(|t| {
            if let GraphQLNamedType::Union(u) = t {
                Some(customizer.custom_type_name(&u.name).to_string())
            } else {
                None
            }
        })
        .collect();

    if !unions.is_empty() {
        let content = templates::mock_unions::render(&unions, access_mod, ns, import_module);
        result.add_file(
            mock_path.join("MockObject+Unions.graphql.swift"),
            content,
        );
    }

    // MockObject files
    let collector = apollo_codegen_ir::field_collector::FieldCollector::new(compilation);
    let all_fields = collector.collect_all_fields();
    let type_kinds = apollo_codegen_ir::field_collector::build_type_kinds(compilation);

    for (object_name, collected_fields) in &all_fields {
        let swift_object_name = customizer.custom_type_name(object_name);
        let mock_fields: Vec<templates::mock_object::MockField> = collected_fields
            .iter()
            .map(|cf| {
                let field_type_str = render_mock_field_type_customized(
                    &cf.field_type,
                    ns,
                    &type_kinds,
                    customizer,
                );
                let mock_type_str = render_mock_init_type_customized(
                    &cf.field_type,
                    ns,
                    &type_kinds,
                    customizer,
                );
                let set_function = apollo_codegen_ir::field_collector::determine_set_function(
                    &cf.field_type,
                    &type_kinds,
                );
                templates::mock_object::MockField {
                    response_key: cf.response_key.clone(),
                    property_name: cf.response_key.clone(),
                    initializer_param_name: None,
                    field_type_str,
                    mock_type_str,
                    set_function,
                    deprecation_reason: cf.deprecation_reason.clone(),
                }
            })
            .collect();

        let content = templates::mock_object::render(
            swift_object_name,
            &mock_fields,
            access_mod,
            ns,
            api_target,
            import_module,
        );
        let file_path = mock_path.join(format!(
            "{}+Mock.graphql.swift",
            naming::first_uppercased(swift_object_name),
        ));
        result.add_file(file_path, content);
    }
}

/// The result of code generation - a map of file paths to file contents.
#[derive(Debug)]
pub struct GenerationResult {
    pub files: BTreeMap<PathBuf, String>,
}

impl GenerationResult {
    fn new() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    fn add_file(&mut self, path: PathBuf, content: String) {
        self.files.insert(path, content);
    }

    /// Get the number of generated files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Write all generated files to disk.
    pub fn write_all(&self) -> std::io::Result<()> {
        for (path, content) in &self.files {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, content)?;
        }
        Ok(())
    }

    /// Prune stale `.graphql.swift` files from the output directories.
    ///
    /// Walks each directory that contains generated files and removes any
    /// `.graphql.swift` files that are NOT in the generated file set.
    pub fn prune_generated_files(&self) -> std::io::Result<usize> {
        use std::collections::BTreeSet;

        // Canonicalize all generated file paths for comparison
        let generated_paths: BTreeSet<PathBuf> = self
            .files
            .keys()
            .filter_map(|p| std::fs::canonicalize(p).ok())
            .collect();

        // Collect all unique parent directories that contain generated files
        let mut dirs_to_scan: BTreeSet<PathBuf> = BTreeSet::new();
        for path in self.files.keys() {
            if let Some(parent) = path.parent() {
                // Walk up to find the root output directories
                // We scan each directory that directly contains generated files
                dirs_to_scan.insert(parent.to_path_buf());
            }
        }

        let mut pruned_count = 0;
        for dir in &dirs_to_scan {
            if !dir.exists() {
                continue;
            }
            // Only scan the specific directory (not recursively) for .graphql.swift files
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if name.ends_with(".graphql.swift") {
                        if let Ok(canonical) = std::fs::canonicalize(&path) {
                            if !generated_paths.contains(&canonical) {
                                std::fs::remove_file(&path)?;
                                pruned_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(pruned_count)
    }

    /// Write all generated file contents concatenated into a single output file.
    ///
    /// Files are separated by newlines. This is useful for build systems (e.g. Bazel)
    /// where producing a single output file avoids shell concatenation overhead.
    pub fn write_concat(&self, path: &Path) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(path)?;
        for (file_path, content) in &self.files {
            writeln!(out, "// Source: {}", file_path.display())?;
            out.write_all(content.as_bytes())?;
            out.write_all(b"\n")?;
        }
        Ok(())
    }
}

// === Type rendering helpers for InputObject fields ===

use apollo_codegen_frontend::types::{GraphQLType, GraphQLValue};

fn render_input_field_type(
    ty: &GraphQLType,
    ns: &str,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    match ty {
        GraphQLType::Named(name) => {
            let base = render_scalar_swift(name, ns, type_kinds, customizer);
            format!("GraphQLNullable<{}>", base)
        }
        GraphQLType::NonNull(inner) => match inner.as_ref() {
            GraphQLType::Named(name) => render_scalar_swift(name, ns, type_kinds, customizer),
            GraphQLType::List(list_inner) => {
                format!("[{}]", render_input_field_type(list_inner, ns, type_kinds, customizer))
            }
            _ => render_input_field_type(inner, ns, type_kinds, customizer),
        },
        GraphQLType::List(inner) => {
            format!("GraphQLNullable<[{}]>", render_input_field_type(inner, ns, type_kinds, customizer))
        }
    }
}

/// Render init parameter type - nullable gets default = nil
fn render_input_field_property_type(
    ty: &GraphQLType,
    ns: &str,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    // Property type same as field type for input objects
    render_input_field_type(ty, ns, type_kinds, customizer)
}

fn render_input_field_init_type(
    ty: &GraphQLType,
    ns: &str,
    default_value: &Option<GraphQLValue>,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    let base = render_input_field_type(ty, ns, type_kinds, customizer);
    match ty {
        GraphQLType::Named(_) | GraphQLType::List(_) => {
            // Nullable fields get default = nil
            format!("{} = nil", base)
        }
        GraphQLType::NonNull(_) => {
            // Non-null fields with a default value become optional in the initializer
            if default_value.is_some() {
                format!("{}? = nil", base)
            } else {
                base
            }
        }
    }
}

fn render_scalar_swift(
    name: &str,
    _ns: &str,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    use apollo_codegen_ir::field_collector::TypeKind;
    match name {
        "String" => "String".to_string(),
        "Int" => "Int".to_string(),
        "Float" => "Double".to_string(),
        "Boolean" => "Bool".to_string(),
        "ID" => "ID".to_string(),
        _ => {
            let swift_name = customizer.custom_type_name(name);
            let kind = type_kinds.get(name).copied().unwrap_or(TypeKind::Scalar);
            match kind {
                TypeKind::Enum => format!("GraphQLEnum<{}>", swift_name),
                _ => swift_name.to_string(),
            }
        }
    }
}

// === Mock type rendering helpers with schema customization ===

/// Render mock field type with schema customization applied.
fn render_mock_field_type_customized(
    ty: &GraphQLType,
    ns: &str,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    let inner = strip_outer_nonnull(ty);
    render_mock_type_inner_customized(inner, ns, type_kinds, customizer)
}

fn render_mock_type_inner_customized(
    ty: &GraphQLType,
    ns: &str,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    use apollo_codegen_ir::field_collector::TypeKind;
    match ty {
        GraphQLType::Named(name) => render_mock_named_type_customized(name, ns, type_kinds, customizer),
        GraphQLType::NonNull(inner) => render_mock_type_inner_customized(inner, ns, type_kinds, customizer),
        GraphQLType::List(inner) => {
            let inner_str = match inner.as_ref() {
                GraphQLType::NonNull(inner_inner) => {
                    render_mock_type_inner_customized(inner_inner, ns, type_kinds, customizer)
                }
                other => {
                    format!("{}?", render_mock_type_inner_customized(other, ns, type_kinds, customizer))
                }
            };
            format!("[{}]", inner_str)
        }
    }
}

fn render_mock_named_type_customized(
    name: &str,
    ns: &str,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    use apollo_codegen_ir::field_collector::TypeKind;
    match name {
        "String" => "String".to_string(),
        "Int" => "Int".to_string(),
        "Float" => "Double".to_string(),
        "Boolean" => "Bool".to_string(),
        "ID" => format!("{}.ID", ns),
        _ => {
            let swift_name = customizer.custom_type_name(name);
            let kind = type_kinds.get(name).copied().unwrap_or(TypeKind::Scalar);
            match kind {
                TypeKind::Enum => format!("GraphQLEnum<{}.{}>", ns, swift_name),
                TypeKind::Scalar => format!("{}.{}", ns, swift_name),
                TypeKind::Object | TypeKind::Interface | TypeKind::Union => swift_name.to_string(),
                _ => swift_name.to_string(),
            }
        }
    }
}

/// Render mock init type with schema customization applied.
fn render_mock_init_type_customized(
    ty: &GraphQLType,
    ns: &str,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    let inner = strip_outer_nonnull(ty);
    render_mock_init_type_inner_customized(inner, ns, type_kinds, customizer)
}

fn render_mock_init_type_inner_customized(
    ty: &GraphQLType,
    ns: &str,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    use apollo_codegen_ir::field_collector::TypeKind;
    match ty {
        GraphQLType::Named(name) => render_mock_init_named_type_customized(name, ns, type_kinds, customizer),
        GraphQLType::NonNull(inner) => render_mock_init_type_inner_customized(inner, ns, type_kinds, customizer),
        GraphQLType::List(inner) => {
            let inner_str = match inner.as_ref() {
                GraphQLType::NonNull(inner_inner) => {
                    render_mock_init_type_inner_customized(inner_inner, ns, type_kinds, customizer)
                }
                other => {
                    format!("{}?", render_mock_init_type_inner_customized(other, ns, type_kinds, customizer))
                }
            };
            format!("[{}]", inner_str)
        }
    }
}

fn render_mock_init_named_type_customized(
    name: &str,
    ns: &str,
    type_kinds: &std::collections::HashMap<String, apollo_codegen_ir::field_collector::TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    use apollo_codegen_ir::field_collector::TypeKind;
    match name {
        "String" => "String".to_string(),
        "Int" => "Int".to_string(),
        "Float" => "Double".to_string(),
        "Boolean" => "Bool".to_string(),
        "ID" => format!("{}.ID", ns),
        _ => {
            let swift_name = customizer.custom_type_name(name);
            let kind = type_kinds.get(name).copied().unwrap_or(TypeKind::Scalar);
            match kind {
                TypeKind::Enum => format!("GraphQLEnum<{}.{}>", ns, swift_name),
                TypeKind::Scalar => format!("{}.{}", ns, swift_name),
                TypeKind::Object => format!("Mock<{}>", swift_name),
                TypeKind::Interface | TypeKind::Union => "(any AnyMock)".to_string(),
                _ => swift_name.to_string(),
            }
        }
    }
}

/// Strip `///\n  /// Parent Type:` doc comments from rendered output.
/// These are schema documentation comments that should be excluded when
/// `schemaDocumentation: exclude` is set.
fn strip_parent_type_comments(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        // Look for pattern: line with just "  ///" (with any indent) followed by "  /// Parent Type: `...`"
        if i + 1 < lines.len() {
            let trimmed = lines[i].trim();
            let next_trimmed = lines[i + 1].trim();
            if trimmed == "///" && next_trimmed.starts_with("/// Parent Type: `") {
                // Skip both lines
                i += 2;
                continue;
            }
        }
        result.push_str(lines[i]);
        result.push('\n');
        i += 1;
    }
    // Remove trailing newline to match original
    if result.ends_with('\n') && !content.ends_with('\n') {
        result.pop();
    }
    result
}

fn strip_outer_nonnull(ty: &GraphQLType) -> &GraphQLType {
    match ty {
        GraphQLType::NonNull(inner) => inner.as_ref(),
        other => other,
    }
}
