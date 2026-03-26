#!/usr/bin/env python3
"""Generate apollo-codegen-config.json files for each test API.

Each API gets a config that mirrors what SwiftScripts/Sources/TargetConfig/Target.swift
used to produce, placed in a given output directory.

Usage:
    python3 generate-api-configs.py <output_dir> <source_root>

For each API, writes <output_dir>/<APIName>/apollo-codegen-config.json.
"""

import json
import os
import sys


def make_config(
    schema_namespace,
    schema_search_paths,
    operation_search_paths,
    include_test_mocks=False,
    schema_documentation=None,
    selection_set_initializers=None,
    operation_document_format=None,
    legacy_safelisting=False,
):
    """Build a codegen config dict matching ApolloCodegenConfiguration JSON."""

    # -- options --
    options = {}
    if schema_documentation == "include":
        options["schemaDocumentation"] = "include"
    if selection_set_initializers == "all":
        options["selectionSetInitializers"] = {"operations": True, "namedFragments": True, "localCacheMutations": True}
    if operation_document_format:
        options["operationDocumentFormat"] = operation_document_format

    # -- experimental features --
    experimental = {}
    if legacy_safelisting:
        experimental["legacySafelistingCompatibleOperations"] = True

    # -- output --
    output_path = "./" + schema_namespace

    output = {
        "schemaTypes": {
            "path": output_path,
            "moduleType": {
                "swiftPackageManager": {}
            },
        },
        "operations": {
            "inSchemaModule": {}
        },
    }

    if include_test_mocks:
        output["testMocks"] = {
            "swiftPackage": {
                "targetName": schema_namespace + "TestMocks"
            }
        }
    else:
        output["testMocks"] = {"none": {}}

    config = {
        "schemaNamespace": schema_namespace,
        "input": {
            "schemaSearchPaths": schema_search_paths,
            "operationSearchPaths": operation_search_paths,
        },
        "output": output,
    }

    if options:
        config["options"] = options
    if experimental:
        config["experimentalFeatures"] = experimental

    return config


def main():
    if len(sys.argv) < 3:
        print(f"Usage: {sys.argv[0]} <output_dir> <source_root>", file=sys.stderr)
        sys.exit(1)

    output_dir = sys.argv[1]
    source_root = os.path.abspath(sys.argv[2])

    # Helper: absolute path into Sources/<api>/...
    def src(*parts):
        return os.path.join(source_root, "Sources", *parts)

    apis = {
        "AnimalKingdomAPI": make_config(
            schema_namespace="AnimalKingdomAPI",
            schema_search_paths=[
                src("AnimalKingdomAPI", "animalkingdom-graphql", "AnimalSchema.graphqls")
            ],
            operation_search_paths=[
                src("AnimalKingdomAPI", "animalkingdom-graphql", "**/*.graphql")
            ],
            include_test_mocks=True,
            schema_documentation="include",
            selection_set_initializers="all",
        ),
        "StarWarsAPI": make_config(
            schema_namespace="StarWarsAPI",
            schema_search_paths=[
                src("StarWarsAPI", "starwars-graphql", "schema.graphqls")
            ],
            operation_search_paths=[
                src("StarWarsAPI", "starwars-graphql", "**/*.graphql")
            ],
            schema_documentation="include",
            selection_set_initializers="all",
            operation_document_format=["definition", "operationId"],
            legacy_safelisting=True,
        ),
        "GitHubAPI": make_config(
            schema_namespace="GitHubAPI",
            schema_search_paths=[
                src("GitHubAPI", "graphql", "schema.graphqls")
            ],
            operation_search_paths=[
                src("GitHubAPI", "graphql", "**/*.graphql")
            ],
            legacy_safelisting=True,
        ),
        "UploadAPI": make_config(
            schema_namespace="UploadAPI",
            schema_search_paths=[
                src("UploadAPI", "graphql", "schema.graphqls")
            ],
            operation_search_paths=[
                src("UploadAPI", "graphql", "**/*.graphql")
            ],
        ),
        "SubscriptionAPI": make_config(
            schema_namespace="SubscriptionAPI",
            schema_search_paths=[
                src("SubscriptionAPI", "graphql", "schema.graphqls")
            ],
            operation_search_paths=[
                src("SubscriptionAPI", "graphql", "**/*.graphql")
            ],
        ),
    }

    for api_name, config in apis.items():
        api_dir = os.path.join(output_dir, api_name)
        os.makedirs(api_dir, exist_ok=True)
        config_path = os.path.join(api_dir, "apollo-codegen-config.json")
        with open(config_path, "w") as f:
            json.dump(config, f, indent=2)
            f.write("\n")
        print(f"  Generated config: {config_path}")


if __name__ == "__main__":
    main()
