//! Full code generation pipeline.
//!
//! Wires together: config → glob → frontend → IR → templates → file output.

use apollo_codegen_config::types::*;
use apollo_codegen_frontend::compilation_result::CompilationResult;
use apollo_codegen_frontend::compiler::{CompileOptions, GraphQLFrontend};
use apollo_codegen_frontend::types::GraphQLNamedType;
use apollo_codegen_frontend::compilation_result::OperationType;
use apollo_codegen_ir::builder::IRBuilder;
use apollo_codegen_render::ir_adapter;
use apollo_codegen_render::naming;
use apollo_codegen_render::templates;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Run the full code generation pipeline.
pub fn generate(config: &ApolloCodegenConfiguration, root_url: &Path) -> anyhow::Result<GenerationResult> {
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
    let compile_options = CompileOptions {
        legacy_safelisting_compatible_operations: config
            .experimental_features
            .legacy_safelisting_compatible_operations,
        reduce_generated_schema_types: config.options.reduce_generated_schema_types,
    };

    let compilation_result = frontend
        .compile(&doc, &source_map, &compile_options)
        .map_err(|errs| anyhow::anyhow!("Compilation errors: {}", errs.join(", ")))?;

    // 4. Build IR
    let ir = IRBuilder::build(&compilation_result);

    // 4b. Build type kind map for type resolution in templates
    let type_kinds = apollo_codegen_ir::field_collector::build_type_kinds(&compilation_result);

    // 5. Determine output configuration
    let ns = naming::first_uppercased(&config.schema_namespace);
    let api_target = "ApolloAPI";
    let access_mod = determine_access_modifier(config);
    let is_in_module = matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::SwiftPackageManager(_)
    ) || matches!(
        config.output.schema_types.module_type,
        SchemaModuleType::Other(_)
    );
    let camel_case_enums = matches!(
        config.options.conversion_strategies.enum_cases,
        EnumCaseConversionStrategy::CamelCase
    );

    let schema_output_path = resolve_path(root_url, &config.output.schema_types.path);

    // 6. Generate files
    let mut result = GenerationResult::new();

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
        config,
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
        &access_mod,
        config,
        &type_kinds,
    );

    generate_fragment_files(
        &mut result,
        &compilation_result,
        &ir,
        &schema_output_path,
        &ns,
        &access_mod,
        config,
        &type_kinds,
    );

    // Test mock files
    generate_test_mock_files(
        &mut result,
        &compilation_result,
        config,
        root_url,
        &ns,
        api_target,
        &access_mod,
    );

    Ok(result)
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
    config: &ApolloCodegenConfiguration,
) {
    let sources_path = schema_path.join("Sources");

    // Objects
    for named_type in &compilation.referenced_types {
        if let GraphQLNamedType::Object(obj) = named_type {
            let content = templates::object::render(
                &obj.name,
                &obj.name,
                &obj.interfaces,
                access_mod,
                api_target,
                &config.schema_namespace,
                is_in_module,
            );
            let file_path = sources_path
                .join("Schema/Objects")
                .join(format!("{}.graphql.swift", naming::first_uppercased(&obj.name)));
            result.add_file(file_path, content);
        }
    }

    // Interfaces
    for named_type in &compilation.referenced_types {
        if let GraphQLNamedType::Interface(iface) = named_type {
            let content = templates::interface::render(
                &iface.name,
                &iface.name,
                access_mod,
                api_target,
            );
            let file_path = sources_path
                .join("Schema/Interfaces")
                .join(format!("{}.graphql.swift", naming::first_uppercased(&iface.name)));
            result.add_file(file_path, content);
        }
    }

    // Unions
    for named_type in &compilation.referenced_types {
        if let GraphQLNamedType::Union(union_t) = named_type {
            let content = templates::union_type::render(
                &union_t.name,
                &union_t.name,
                &union_t.member_types,
                access_mod,
                api_target,
                &config.schema_namespace,
                is_in_module,
            );
            let file_path = sources_path
                .join("Schema/Unions")
                .join(format!("{}.graphql.swift", naming::first_uppercased(&union_t.name)));
            result.add_file(file_path, content);
        }
    }

    // Enums
    for named_type in &compilation.referenced_types {
        if let GraphQLNamedType::Enum(enum_t) = named_type {
            let values: Vec<templates::enum_type::EnumValue> = enum_t
                .values
                .iter()
                .map(|v| templates::enum_type::EnumValue {
                    name: v.name.clone(),
                    raw_value: v.name.clone(),
                    description: v.description.clone(),
                    is_deprecated: v.is_deprecated,
                    deprecation_reason: v.deprecation_reason.clone(),
                })
                .collect();

            let content = templates::enum_type::render(
                &enum_t.name,
                &values,
                access_mod,
                api_target,
                camel_case_enums,
            );
            let file_path = sources_path
                .join("Schema/Enums")
                .join(format!("{}.graphql.swift", naming::first_uppercased(&enum_t.name)));
            result.add_file(file_path, content);
        }
    }

    // Input Objects
    for named_type in &compilation.referenced_types {
        if let GraphQLNamedType::InputObject(input) = named_type {
            let fields: Vec<templates::input_object::InputField> = input
                .fields
                .iter()
                .map(|(fname, fdef)| {
                    let mut swift_type = render_input_field_type(&fdef.field_type, ns);
                    // Non-null fields with default values become optional
                    if matches!(fdef.field_type, GraphQLType::NonNull(_)) && fdef.default_value.is_some() {
                        swift_type = format!("{}?", swift_type);
                    }
                    let init_type = render_input_field_init_type(&fdef.field_type, ns, &fdef.default_value);
                    templates::input_object::InputField {
                        schema_name: fname.clone(),
                        rendered_name: fname.clone(),
                        rendered_type: swift_type,
                        rendered_init_type: init_type,
                        description: fdef.description.clone(),
                        deprecation_reason: fdef.deprecation_reason.clone(),
                    }
                })
                .collect();

            let content = templates::input_object::render(
                &input.name,
                &fields,
                access_mod,
                api_target,
                config.options.warnings_on_deprecated_usage == apollo_codegen_config::types::Composition::Include,
                input.description.as_deref(),
            );
            let file_path = sources_path
                .join("Schema/InputObjects")
                .join(format!("{}.graphql.swift", naming::first_uppercased(&input.name)));
            result.add_file(file_path, content);
        }
    }

    // Custom Scalars
    for named_type in &compilation.referenced_types {
        if let GraphQLNamedType::Scalar(scalar) = named_type {
            // Skip built-in scalars
            if matches!(
                scalar.name.as_str(),
                "String" | "Int" | "Float" | "Boolean" | "ID"
            ) {
                // ID is a custom scalar in Apollo
                if scalar.name == "ID" {
                    let content = templates::custom_scalar::render(
                        &scalar.name,
                        scalar.description.as_deref(),
                        scalar.specified_by_url.as_deref(),
                        access_mod,
                        api_target,
                    );
                    let file_path = sources_path
                        .join("Schema/CustomScalars")
                        .join(format!("{}.swift", naming::first_uppercased(&scalar.name)));
                    result.add_file(file_path, content);
                }
                continue;
            }
            let content = templates::custom_scalar::render(
                &scalar.name,
                scalar.description.as_deref(),
                scalar.specified_by_url.as_deref(),
                access_mod,
                api_target,
            );
            let file_path = sources_path
                .join("Schema/CustomScalars")
                .join(format!("{}.swift", naming::first_uppercased(&scalar.name)));
            result.add_file(file_path, content);
        }
    }

    // SchemaMetadata
    let object_types: Vec<(String, String)> = compilation
        .referenced_types
        .iter()
        .filter_map(|t| {
            if let GraphQLNamedType::Object(obj) = t {
                Some((obj.name.clone(), obj.name.clone()))
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
    );
    result.add_file(
        sources_path.join("Schema/SchemaMetadata.graphql.swift"),
        content,
    );

    // SchemaConfiguration
    let content = templates::schema_config::render(access_mod, api_target);
    result.add_file(
        sources_path.join("Schema/SchemaConfiguration.swift"),
        content,
    );
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
                Some((target_name, "./TestMocks".to_string()))
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
) {
    let sources_path = schema_path.join("Sources");

    for op_def in &compilation.operations {
        let operation = ir.build_operation(op_def);
        let content = ir_adapter::render_operation(
            &operation,
            ns,
            access_mod,
            true, // generate initializers
            type_kinds,
        );

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
            let file_path = sources_path
                .join("LocalCacheMutations")
                .join(format!("{}.graphql.swift", operation.name));
            result.add_file(file_path, content);
        } else {
            let file_path = sources_path
                .join(format!("Operations/{}", subdir))
                .join(format!("{}.graphql.swift", file_name));
            result.add_file(file_path, content);
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
) {
    let sources_path = schema_path.join("Sources");

    for frag_def in &compilation.fragments {
        if let Some(frag) = ir.fragments().get(&frag_def.name) {
            let content = ir_adapter::render_fragment(
                frag,
                ns,
                access_mod,
                true, // generate initializers
                type_kinds,
            );

            if frag.is_local_cache_mutation {
                let file_path = sources_path
                    .join("LocalCacheMutations")
                    .join(format!("{}.graphql.swift", frag.name));
                result.add_file(file_path, content);
            } else {
                let file_path = sources_path
                    .join("Fragments")
                    .join(format!("{}.graphql.swift", frag.name));
                result.add_file(file_path, content);
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
) {
    let mock_path = match &config.output.test_mocks {
        TestMockFileOutput::None(_) => return,
        TestMockFileOutput::Absolute(abs) => resolve_path(root_url, &abs.path),
        TestMockFileOutput::SwiftPackage(pkg) => {
            // SwiftPackage test mocks always go to ./TestMocks relative to schema types path
            // (matching the Swift behavior where the targetName is for Package.swift, not the directory)
            let target_name = pkg
                .target_name
                .as_deref()
                .map(|n| naming::first_uppercased(n))
                .unwrap_or_else(|| format!("{}TestMocks", ns));
            resolve_path(root_url, &config.output.schema_types.path)
                .join("TestMocks")
        }
    };

    // MockInterfaces
    let interfaces: Vec<String> = compilation
        .referenced_types
        .iter()
        .filter_map(|t| {
            if let GraphQLNamedType::Interface(i) = t {
                Some(i.name.clone())
            } else {
                None
            }
        })
        .collect();

    if !interfaces.is_empty() {
        let content = templates::mock_interfaces::render(&interfaces, access_mod, ns);
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
                Some(u.name.clone())
            } else {
                None
            }
        })
        .collect();

    if !unions.is_empty() {
        let content = templates::mock_unions::render(&unions, access_mod, ns);
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
        let mock_fields: Vec<templates::mock_object::MockField> = collected_fields
            .iter()
            .map(|cf| {
                let field_type_str = apollo_codegen_ir::field_collector::render_mock_field_type(
                    &cf.field_type,
                    ns,
                    &type_kinds,
                );
                let mock_type_str = apollo_codegen_ir::field_collector::render_mock_init_type(
                    &cf.field_type,
                    ns,
                    &type_kinds,
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
            object_name,
            &mock_fields,
            access_mod,
            ns,
            api_target,
        );
        let file_path = mock_path.join(format!(
            "{}+Mock.graphql.swift",
            naming::first_uppercased(object_name),
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
}

// === Type rendering helpers for InputObject fields ===

use apollo_codegen_frontend::types::{GraphQLType, GraphQLValue};

fn render_input_field_type(ty: &GraphQLType, ns: &str) -> String {
    match ty {
        GraphQLType::Named(name) => {
            let base = render_scalar_swift(name, ns);
            // In input objects, nullable types use GraphQLNullable<T> wrapper
            format!("GraphQLNullable<{}>", base)
        }
        GraphQLType::NonNull(inner) => match inner.as_ref() {
            GraphQLType::Named(name) => render_scalar_swift(name, ns),
            GraphQLType::List(list_inner) => {
                format!("[{}]", render_input_field_type(list_inner, ns))
            }
            _ => render_input_field_type(inner, ns),
        },
        GraphQLType::List(inner) => {
            format!("GraphQLNullable<[{}]>", render_input_field_type(inner, ns))
        }
    }
}

/// Render init parameter type - nullable gets default = nil
fn render_input_field_property_type(ty: &GraphQLType, ns: &str) -> String {
    // Property type same as field type for input objects
    render_input_field_type(ty, ns)
}

fn render_input_field_init_type(
    ty: &GraphQLType,
    ns: &str,
    default_value: &Option<GraphQLValue>,
) -> String {
    let base = render_input_field_type(ty, ns);
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

fn render_scalar_swift(name: &str, _ns: &str) -> String {
    match name {
        "String" => "String".to_string(),
        "Int" => "Int".to_string(),
        "Float" => "Double".to_string(),
        "Boolean" => "Bool".to_string(),
        "ID" => "ID".to_string(),
        _ => name.to_string(),
    }
}
