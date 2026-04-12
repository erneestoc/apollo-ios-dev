"""Apollo GraphQL codegen rules for Bazel — persistent worker mode.

Uses the Rust apollo-ios-cli binary in --persistent_worker mode for fast
incremental codegen. The worker stays alive across builds, caching schema+IR
between requests. Warm-start requests skip schema parsing entirely.

Provides two rules:
  - apollo_schema_types: Generates schema type files (Objects, Enums, etc.)
  - apollo_operations: Generates per-framework operation/fragment files

Both rules produce a single concatenated .swift file suitable for
swift_library srcs. Internally, codegen writes to a tree artifact
(directory), then a lightweight concat step merges into one file.
This avoids a rules_swift limitation where tree artifacts in srcs
cause .o file naming mismatches.

Usage from a consuming repo:

    # WORKSPACE.bazel or MODULE.bazel
    http_archive(name = "apollo_ios_cli", ...)

    # BUILD.bazel
    load("@apollo_ios_dev//bazel:apollo_codegen.bzl", "apollo_schema_types", "apollo_operations")

    apollo_schema_types(
        name = "my_schema_types",
        schema = "//path/to:schema.graphqls",
        srcs = ["//:all_graphql_files"],
        config = '{"schemaNamespace":"MyAPI",...}',
    )

    apollo_operations(
        name = "my_ops",
        framework_path = "Features/Account",
        schema = "//path/to:schema.graphqls",
        srcs = glob(["**/*.graphql"]),
        config = '{"schemaNamespace":"MyAPI",...}',
    )
"""

# --- Schema Types Rule ---

def _apollo_schema_types_impl(ctx):
    # Step 1: Codegen writes individual files to a tree artifact.
    tree_dir = ctx.actions.declare_directory(ctx.attr.name + "_tree")

    schema = ctx.file.schema
    graphql_files = [f for f in ctx.files.srcs if f.path.endswith(".graphql")]

    config_json = ctx.attr.config.format(schema = schema.path)

    args = ctx.actions.args()
    args.add("generate")
    args.add("--string", config_json)
    args.add("--bazel-output-dir", tree_dir.path)
    args.add("--bazel-mode", "schema_types")
    if ctx.attr.optimize_schema_metadata:
        args.add("--bazel-optimize-schema-metadata")

    args.use_param_file("@%s", use_always = True)
    args.set_param_file_format("multiline")

    ctx.actions.run(
        inputs = [schema] + graphql_files,
        outputs = [tree_dir],
        executable = ctx.executable._apollo_cli,
        arguments = [args],
        execution_requirements = {
            "supports-workers": "1",
            "requires-worker-protocol": "proto",
        },
        mnemonic = "ApolloSchemaTypes",
        progress_message = "Generating Apollo schema types (worker)",
    )

    # Step 2: Concatenate into a single .swift file for swift_library.
    output_file = ctx.actions.declare_file(ctx.attr.name + ".swift")
    ctx.actions.run_shell(
        command = 'find "{tree}" -name "*.swift" -print0 | sort -z | xargs -0 cat > "{out}"'.format(
            tree = tree_dir.path,
            out = output_file.path,
        ),
        inputs = [tree_dir],
        outputs = [output_file],
        mnemonic = "ApolloConcat",
        progress_message = "Concatenating Apollo schema types",
    )

    return [DefaultInfo(files = depset([output_file]))]

apollo_schema_types = rule(
    implementation = _apollo_schema_types_impl,
    attrs = {
        "config": attr.string(
            mandatory = True,
            doc = "Apollo codegen config JSON string. Use {schema} placeholder for schema path.",
        ),
        "schema": attr.label(
            allow_single_file = [".graphqls"],
            mandatory = True,
            doc = "The GraphQL schema file (schema.graphqls).",
        ),
        "srcs": attr.label_list(
            allow_files = True,
            mandatory = True,
            doc = "All .graphql operation and fragment files.",
        ),
        "optimize_schema_metadata": attr.bool(
            default = True,
            doc = "Add fast O(1) dictionary lookup to SchemaMetadata objectType function.",
        ),
        "_apollo_cli": attr.label(
            default = Label("@apollo_ios_cli//:apollocli"),
            executable = True,
            allow_files = True,
            cfg = "exec",
        ),
    },
    doc = "Generates Apollo schema type files via persistent worker, outputs a single .swift file.",
)

# --- Operations Rule ---

def _apollo_operations_impl(ctx):
    # Step 1: Codegen writes individual files to a tree artifact.
    tree_dir = ctx.actions.declare_directory(ctx.attr.name + "_tree")

    schema = ctx.file.schema
    operation_files = ctx.files.srcs
    fragment_files = ctx.files.fragments
    framework_path = ctx.attr.framework_path

    if ctx.attr.all_operations:
        all_graphql = [f for f in ctx.files.all_operations if f.path.endswith(".graphql")]
    else:
        all_graphql = operation_files + fragment_files

    config_json = ctx.attr.config.format(schema = schema.path)

    args = ctx.actions.args()
    args.add("generate")
    args.add("--string", config_json)
    args.add("--bazel-output-dir", tree_dir.path)
    args.add("--bazel-mode", "operations")
    args.add("--bazel-framework-path", framework_path)

    if ctx.attr.strip_import:
        args.add("--bazel-strip-import", ctx.attr.strip_import)

    args.use_param_file("@%s", use_always = True)
    args.set_param_file_format("multiline")

    ctx.actions.run(
        inputs = [schema] + all_graphql,
        outputs = [tree_dir],
        executable = ctx.executable._apollo_cli,
        arguments = [args],
        execution_requirements = {
            "supports-workers": "1",
            "requires-worker-protocol": "proto",
        },
        mnemonic = "ApolloOperations",
        progress_message = "Generating Apollo operations for %s (worker)" % framework_path,
    )

    # Step 2: Concatenate into a single .swift file for swift_library.
    output_file = ctx.actions.declare_file(ctx.attr.name + ".swift")
    ctx.actions.run_shell(
        command = 'find "{tree}" -name "*.swift" -print0 | sort -z | xargs -0 cat > "{out}"'.format(
            tree = tree_dir.path,
            out = output_file.path,
        ),
        inputs = [tree_dir],
        outputs = [output_file],
        mnemonic = "ApolloConcat",
        progress_message = "Concatenating Apollo operations for %s" % framework_path,
    )

    return [DefaultInfo(files = depset([output_file]))]

apollo_operations = rule(
    implementation = _apollo_operations_impl,
    attrs = {
        "config": attr.string(
            mandatory = True,
            doc = "Apollo codegen config JSON string. Use {schema} placeholder for schema path.",
        ),
        "framework_path": attr.string(
            mandatory = True,
            doc = "Framework path relative to repo root (e.g., 'Features/Account').",
        ),
        "schema": attr.label(
            allow_single_file = [".graphqls"],
            mandatory = True,
            doc = "The GraphQL schema file.",
        ),
        "srcs": attr.label_list(
            allow_files = [".graphql"],
            mandatory = True,
            doc = "This framework's .graphql operation files.",
        ),
        "fragments": attr.label_list(
            allow_files = [".graphql"],
            doc = "Shared fragment .graphql files needed for resolution.",
        ),
        "all_operations": attr.label_list(
            allow_files = True,
            doc = "ALL .graphql files for compile-all caching.",
        ),
        "strip_import": attr.string(
            doc = "Module name to strip import for (e.g., 'V4' to avoid circular imports).",
        ),
        "_apollo_cli": attr.label(
            default = Label("@apollo_ios_cli//:apollocli"),
            executable = True,
            allow_files = True,
            cfg = "exec",
        ),
    },
    doc = "Generates Apollo operation .swift files via persistent worker, outputs a single .swift file.",
)
