#!/usr/bin/env python3
"""Generate ~50 codegen configuration permutations for the AnimalKingdomAPI.

Each flag value appears in at least one configuration. Configs are written as
JSON files into the directory specified by the first CLI argument (or a
temp directory if none is given).

Usage:
    python3 generate-permutation-configs.py <output_dir>
"""

import json
import os
import sys
import itertools
import random

# ---------------------------------------------------------------------------
# Flag value pools
# ---------------------------------------------------------------------------

MODULE_TYPES = [
    {"swiftPackageManager": {}},
    {"embeddedInTarget": {"name": "App", "accessModifier": "public"}},
    {"embeddedInTarget": {"name": "App", "accessModifier": "internal"}},
    {"other": {}},
]

OPERATIONS = [
    {"inSchemaModule": {}},
    {"absolute": {"path": "./Ops"}},
    {"relative": {"subpath": None}},
]

TEST_MOCKS = [
    {"none": {}},
    {"absolute": {"path": "./Mocks"}},
    {"swiftPackage": {"targetName": "TestMocks"}},
]

SELECTION_SET_INITIALIZERS = [
    None,  # not set (defaults)
    {"operations": True, "namedFragments": True, "localCacheMutations": True},
    {"operations": True},
    {"namedFragments": True},
]

QUERY_STRING_LITERAL_FORMAT = ["singleLine", "multiline"]

OPERATION_DOCUMENT_FORMAT = [
    ["definition"],
    ["operationId"],
    ["definition", "operationId"],
]

SCHEMA_DOCUMENTATION = ["include", "exclude"]

PRUNE_GENERATED_FILES = [True, False]

ENUM_CASE_CONVERSION = ["none", "camelCase"]

COCOAPODS_COMPAT = [True, False]

DEPRECATED_ENUM_CASES = ["include", "exclude"]

SCHEMA_CUSTOMIZATION = [
    None,  # none
    {
        "customTypeNames": {
            "Animal": "RenamedAnimal",
            "Crocodile": "RenamedCrocodile",
        }
    },
]

# ---------------------------------------------------------------------------
# All dimensions (name, pool)
# ---------------------------------------------------------------------------

DIMENSIONS = [
    ("moduleType", MODULE_TYPES),
    ("operations", OPERATIONS),
    ("testMocks", TEST_MOCKS),
    ("selectionSetInitializers", SELECTION_SET_INITIALIZERS),
    ("queryStringLiteralFormat", QUERY_STRING_LITERAL_FORMAT),
    ("operationDocumentFormat", OPERATION_DOCUMENT_FORMAT),
    ("schemaDocumentation", SCHEMA_DOCUMENTATION),
    ("pruneGeneratedFiles", PRUNE_GENERATED_FILES),
    ("enumCases", ENUM_CASE_CONVERSION),
    ("cocoapodsCompat", COCOAPODS_COMPAT),
    ("deprecatedEnumCases", DEPRECATED_ENUM_CASES),
    ("schemaCustomization", SCHEMA_CUSTOMIZATION),
]


def build_config(combo):
    """Build a full codegen config dict from a tuple of dimension values."""
    (
        module_type,
        operations,
        test_mocks,
        sel_init,
        query_fmt,
        op_doc_fmt,
        schema_doc,
        prune,
        enum_cases,
        cocoapods,
        depr_enum,
        schema_custom,
    ) = combo

    config = {
        "schemaNamespace": "TestSchema",
        "input": {
            "operationSearchPaths": [
                "../../../Sources/AnimalKingdomAPI/animalkingdom-graphql/*.graphql"
            ],
            "schemaSearchPaths": [
                "../../../Sources/AnimalKingdomAPI/animalkingdom-graphql/AnimalSchema.graphqls"
            ],
        },
        "output": {
            "testMocks": test_mocks,
            "schemaTypes": {
                "path": "./Generated",
                "moduleType": module_type,
            },
            "operations": operations,
        },
        "options": {},
    }

    options = {}
    options["queryStringLiteralFormat"] = query_fmt
    options["operationDocumentFormat"] = op_doc_fmt
    options["schemaDocumentation"] = schema_doc
    options["pruneGeneratedFiles"] = prune
    options["cocoapodsCompatibleImportStatements"] = cocoapods
    options["deprecated_enum_cases"] = depr_enum
    options["conversionStrategies"] = {"enumCases": enum_cases}

    if sel_init is not None:
        options["selectionSetInitializers"] = sel_init

    if schema_custom is not None:
        options["schemaCustomization"] = schema_custom

    config["options"] = options
    return config


def generate_configs(target_count=50, seed=42):
    """Generate configs ensuring every flag value appears at least once."""
    random.seed(seed)

    configs = []

    # --- Phase 1: coverage pass ---
    # For each dimension, ensure every value appears at least once.
    max_pool = max(len(pool) for _, pool in DIMENSIONS)
    for slot_idx in range(max_pool):
        combo = []
        for _name, pool in DIMENSIONS:
            if slot_idx < len(pool):
                combo.append(pool[slot_idx])
            else:
                combo.append(random.choice(pool))
        configs.append(build_config(tuple(combo)))

    # --- Phase 2: random fill to reach target_count ---
    while len(configs) < target_count:
        combo = tuple(random.choice(pool) for _, pool in DIMENSIONS)
        configs.append(build_config(combo))

    return configs


def verify_coverage(configs):
    """Verify that every flag value appears in at least one config."""
    for dim_name, pool in DIMENSIONS:
        for val in pool:
            found = False
            for cfg in configs:
                # Check presence depending on dimension
                if dim_name == "moduleType":
                    if cfg["output"]["schemaTypes"]["moduleType"] == val:
                        found = True
                        break
                elif dim_name == "operations":
                    if cfg["output"]["operations"] == val:
                        found = True
                        break
                elif dim_name == "testMocks":
                    if cfg["output"]["testMocks"] == val:
                        found = True
                        break
                elif dim_name == "selectionSetInitializers":
                    opt = cfg["options"].get("selectionSetInitializers")
                    if val is None and opt is None:
                        found = True
                        break
                    elif val is not None and opt == val:
                        found = True
                        break
                elif dim_name == "queryStringLiteralFormat":
                    if cfg["options"]["queryStringLiteralFormat"] == val:
                        found = True
                        break
                elif dim_name == "operationDocumentFormat":
                    if cfg["options"]["operationDocumentFormat"] == val:
                        found = True
                        break
                elif dim_name == "schemaDocumentation":
                    if cfg["options"]["schemaDocumentation"] == val:
                        found = True
                        break
                elif dim_name == "pruneGeneratedFiles":
                    if cfg["options"]["pruneGeneratedFiles"] == val:
                        found = True
                        break
                elif dim_name == "enumCases":
                    if cfg["options"]["conversionStrategies"]["enumCases"] == val:
                        found = True
                        break
                elif dim_name == "cocoapodsCompat":
                    if cfg["options"]["cocoapodsCompatibleImportStatements"] == val:
                        found = True
                        break
                elif dim_name == "deprecatedEnumCases":
                    if cfg["options"]["deprecated_enum_cases"] == val:
                        found = True
                        break
                elif dim_name == "schemaCustomization":
                    opt = cfg["options"].get("schemaCustomization")
                    if val is None and opt is None:
                        found = True
                        break
                    elif val is not None and opt == val:
                        found = True
                        break
            if not found:
                print(
                    f"WARNING: value not covered for dimension '{dim_name}': {val}",
                    file=sys.stderr,
                )
                return False
    return True


def main():
    if len(sys.argv) < 2:
        print("Usage: generate-permutation-configs.py <output_dir>", file=sys.stderr)
        sys.exit(1)

    output_dir = sys.argv[1]
    os.makedirs(output_dir, exist_ok=True)

    configs = generate_configs(target_count=50)

    if not verify_coverage(configs):
        print("ERROR: Not all flag values are covered!", file=sys.stderr)
        sys.exit(1)

    for idx, cfg in enumerate(configs):
        path = os.path.join(output_dir, f"config-{idx:03d}.json")
        with open(path, "w") as f:
            json.dump(cfg, f, indent=2)

    print(f"Generated {len(configs)} configs in {output_dir}")


if __name__ == "__main__":
    main()
