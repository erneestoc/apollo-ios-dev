"""Bazel rules for Apollo iOS GraphQL code generation.

Uses the Rust codegen CLI as a persistent worker for low-latency incremental builds.
Schema types are generated once per schema; operation targets are generated per-module.
"""

def _apollo_codegen_config(ctx, schema_files, operation_files, output_dir):
    """Generate a codegen config JSON string."""
    schema_paths = [f.path for f in schema_files]
    op_paths = [f.path for f in operation_files]

    config = {
        "schemaNamespace": ctx.attr.schema_namespace,
        "input": {
            "schemaSearchPaths": schema_paths,
            "operationSearchPaths": op_paths,
        },
        "output": {
            "testMocks": {"none": {}},
            "schemaTypes": {
                "path": output_dir,
                "moduleType": {"swiftPackageManager": {}},
            },
            "operations": {"inSchemaModule": {}},
        },
    }

    if ctx.attr.schema_documentation:
        config.setdefault("options", {})["schemaDocumentation"] = "include"

    if ctx.attr.selection_set_initializers:
        config.setdefault("options", {})["selectionSetInitializers"] = {
            "operations": True,
            "namedFragments": True,
            "localCacheMutations": True,
        }

    return json.encode(config)

def _apollo_schema_types_impl(ctx):
    """Generate schema type files (Objects, Enums, Interfaces, etc.)."""
    output = ctx.actions.declare_file(ctx.attr.name + ".swift")

    schema_files = ctx.files.schema
    operation_files = ctx.files.operations

    config_json = _apollo_codegen_config(
        ctx, schema_files, operation_files, output.dirname,
    )

    config_file = ctx.actions.declare_file(ctx.attr.name + "_config.json")
    ctx.actions.write(config_file, config_json)

    args = ctx.actions.args()
    args.add("--mode=schema-types")
    args.add("--config=" + config_file.path)
    args.add("--concat=" + output.path)
    args.add("--skip-schema-configuration")

    inputs = depset(schema_files + operation_files + [config_file])

    ctx.actions.run(
        executable = ctx.executable._codegen_tool,
        arguments = [args],
        inputs = inputs,
        outputs = [output],
        mnemonic = "ApolloSchemaCodegen",
        progress_message = "Generating schema types for %s" % ctx.attr.schema_namespace,
        execution_requirements = {
            "supports-workers": "1",
            "requires-worker-protocol": "proto",
        },
    )

    return [DefaultInfo(files = depset([output]))]

def _apollo_operations_impl(ctx):
    """Generate operation/fragment Swift files."""
    output = ctx.actions.declare_file(ctx.attr.name + ".swift")

    schema_files = ctx.files.schema
    operation_files = ctx.files.operations

    # Need all operations for compilation (fragment/type resolution),
    # but only generate output for this target's operations.
    all_operation_files = ctx.files.operations
    if ctx.attr.all_operations:
        all_operation_files = ctx.files.all_operations

    config_json = _apollo_codegen_config(
        ctx, schema_files, all_operation_files, output.dirname,
    )

    config_file = ctx.actions.declare_file(ctx.attr.name + "_config.json")
    ctx.actions.write(config_file, config_json)

    args = ctx.actions.args()
    args.add("--mode=operations")
    args.add("--config=" + config_file.path)
    args.add("--concat=" + output.path)

    # Filter to only this target's operation files
    only_paths = [f.path for f in ctx.files.operations]
    if only_paths:
        args.add("--only-for-paths=" + ",".join(only_paths))

    inputs = depset(schema_files + all_operation_files + [config_file])

    ctx.actions.run(
        executable = ctx.executable._codegen_tool,
        arguments = [args],
        inputs = inputs,
        outputs = [output],
        mnemonic = "ApolloOperationCodegen",
        progress_message = "Generating operations for %s" % ctx.attr.name,
        execution_requirements = {
            "supports-workers": "1",
            "requires-worker-protocol": "proto",
        },
    )

    return [DefaultInfo(files = depset([output]))]

def _apollo_codegen_impl(ctx):
    """Generate all codegen files (schema + operations) in one action."""
    output = ctx.actions.declare_file(ctx.attr.name + ".swift")

    schema_files = ctx.files.schema
    operation_files = ctx.files.operations

    config_json = _apollo_codegen_config(
        ctx, schema_files, operation_files, output.dirname,
    )

    config_file = ctx.actions.declare_file(ctx.attr.name + "_config.json")
    ctx.actions.write(config_file, config_json)

    args = ctx.actions.args()
    args.add("--mode=all")
    args.add("--config=" + config_file.path)
    args.add("--concat=" + output.path)
    args.add("--skip-schema-configuration")

    inputs = depset(schema_files + operation_files + [config_file])

    ctx.actions.run(
        executable = ctx.executable._codegen_tool,
        arguments = [args],
        inputs = inputs,
        outputs = [output],
        mnemonic = "ApolloCodegen",
        progress_message = "Generating Apollo code for %s" % ctx.attr.schema_namespace,
        execution_requirements = {
            "supports-workers": "1",
            "requires-worker-protocol": "proto",
        },
    )

    return [DefaultInfo(files = depset([output]))]

# Common attributes shared by all rules
_COMMON_ATTRS = {
    "schema": attr.label_list(
        allow_files = [".graphqls", ".json"],
        mandatory = True,
        doc = "GraphQL schema file(s) (.graphqls or introspection .json)",
    ),
    "operations": attr.label_list(
        allow_files = [".graphql"],
        mandatory = True,
        doc = "GraphQL operation and fragment files (.graphql)",
    ),
    "schema_namespace": attr.string(
        mandatory = True,
        doc = "Swift namespace for generated schema types (e.g., 'AnimalKingdomAPI')",
    ),
    "schema_documentation": attr.bool(
        default = False,
        doc = "Include schema documentation in generated code",
    ),
    "selection_set_initializers": attr.bool(
        default = False,
        doc = "Generate selection set initializers for operations and fragments",
    ),
    "_codegen_tool": attr.label(
        default = "//bazel:codegen_tool",
        executable = True,
        cfg = "exec",
        doc = "The Apollo codegen CLI binary (built from Rust)",
    ),
}

apollo_schema_types = rule(
    implementation = _apollo_schema_types_impl,
    attrs = _COMMON_ATTRS,
    doc = """Generate Apollo iOS schema type files.

    Produces Objects, Enums, Interfaces, Unions, InputObjects, CustomScalars,
    and SchemaMetadata as a single concatenated Swift file.

    This rule uses the persistent worker for caching — the schema is parsed
    once and reused across all subsequent operation targets.
    """,
)

apollo_operations = rule(
    implementation = _apollo_operations_impl,
    attrs = dict(_COMMON_ATTRS, **{
        "all_operations": attr.label_list(
            allow_files = [".graphql"],
            doc = """All operation files across the entire schema (for type resolution).
            If not set, uses 'operations' for both compilation and output.""",
        ),
    }),
    doc = """Generate Apollo iOS operation/fragment Swift files.

    Can generate for a subset of operations (per-module) while compiling
    against all operations for correct type resolution.

    Uses the persistent worker — schema and compilation results are cached
    in memory across invocations.
    """,
)

apollo_codegen = rule(
    implementation = _apollo_codegen_impl,
    attrs = _COMMON_ATTRS,
    doc = """Generate all Apollo iOS codegen output in a single action.

    Use this for simple setups where you don't need modular split.
    For modular builds, use apollo_schema_types + apollo_operations.
    """,
)

def apollo_library(
    name,
    schema,
    operations,
    schema_namespace,
    deps = [],
    module_operations = None,
    all_operations = None,
    schema_documentation = False,
    selection_set_initializers = False,
    visibility = None,
    **kwargs):
    """Convenience macro: generates code and creates a swift_library target.

    For modular builds, creates separate schema and operations targets.

    Args:
        name: Target name for the swift_library.
        schema: GraphQL schema file(s).
        operations: GraphQL operation files for this module.
        schema_namespace: Swift namespace.
        deps: Additional swift_library dependencies.
        module_operations: If set, only generate these operations (for per-module splits).
        all_operations: All operations across the schema (for type resolution in modular builds).
        schema_documentation: Include schema docs.
        selection_set_initializers: Generate initializers.
        visibility: Bazel visibility.
        **kwargs: Passed to swift_library.
    """
    schema_target = name + "_schema"
    ops_target = name + "_ops"

    # Schema types (shared across modules)
    apollo_schema_types(
        name = schema_target,
        schema = schema,
        operations = all_operations or operations,
        schema_namespace = schema_namespace,
        schema_documentation = schema_documentation,
        selection_set_initializers = selection_set_initializers,
        visibility = visibility,
    )

    # Operations (per-module)
    if module_operations:
        apollo_operations(
            name = ops_target,
            schema = schema,
            operations = module_operations,
            all_operations = all_operations or operations,
            schema_namespace = schema_namespace,
            schema_documentation = schema_documentation,
            selection_set_initializers = selection_set_initializers,
            visibility = visibility,
        )
    else:
        apollo_operations(
            name = ops_target,
            schema = schema,
            operations = operations,
            schema_namespace = schema_namespace,
            schema_documentation = schema_documentation,
            selection_set_initializers = selection_set_initializers,
            visibility = visibility,
        )

    # Swift library combining generated code
    native.filegroup(
        name = name + "_generated_srcs",
        srcs = [":" + schema_target, ":" + ops_target],
        visibility = visibility,
    )
