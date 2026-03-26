#!/usr/bin/env python3
"""Rewrite an apollo-codegen-config.json so that:
  - All INPUT paths become absolute (resolved relative to the config file's directory).
  - All OUTPUT paths point into a given temp directory, preserving the relative structure.
  - For configs with 'relative' operations output, input files are copied into the
    output directory so that operation files generated next to sources also land in
    the temp dir.

Usage:
    python3 rewrite-config-paths.py <original_config> <output_config> <output_base_dir>

The rewritten config is written to <output_config>.
Generated files will land under <output_base_dir>.
"""

import glob as globmod
import json
import os
import shutil
import sys


def resolve_input_path(path: str, config_dir: str) -> str:
    """Resolve a potentially-relative input path against the config directory."""
    if os.path.isabs(path):
        return path
    return os.path.normpath(os.path.join(config_dir, path))


def redirect_output_path(path: str, output_base: str) -> str:
    """Redirect an output path into the output_base directory.

    Strips leading ./ and any leading ../ sequences, then joins with output_base.
    This keeps the meaningful part of the path (e.g. 'AnimalKingdomAPI') while
    placing it under the temp dir.
    """
    # Strip leading ./
    cleaned = path
    while cleaned.startswith("./"):
        cleaned = cleaned[2:]
    # Strip leading ../ sequences (we don't want to escape the temp dir)
    while cleaned.startswith("../"):
        cleaned = cleaned[3:]
    # If path was just "." or empty, use the base
    if not cleaned or cleaned == ".":
        return output_base
    return os.path.join(output_base, cleaned)


def copy_input_files_for_relative_ops(config: dict, config_dir: str, output_base: str) -> dict:
    """For configs with 'relative' operations, copy source files into the output dir.

    When operations output is 'relative', generated files are placed next to the
    source .graphql files. To capture these in the temp dir, we copy the entire
    source tree into the output dir and rewrite input paths accordingly.

    Also copies schema files so they can be found at the new input paths.

    Returns the modified config dict with updated input paths.
    """
    config = json.loads(json.dumps(config))  # deep copy
    inp = config.get("input", {})

    # We need to map each original input path to a new location in output_base.
    # Strategy: preserve the path structure relative to config_dir inside output_base.
    new_schema_paths = []
    new_op_paths = []

    for path in inp.get("schemaSearchPaths", []):
        abs_path = resolve_input_path(path, config_dir)
        # Copy each matching file
        for f in globmod.glob(abs_path, recursive=True):
            rel = os.path.relpath(f, config_dir)
            dest = os.path.join(output_base, "_input", rel)
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            shutil.copy2(f, dest)
        # Rewrite the path pattern
        rel_pattern = os.path.relpath(abs_path, config_dir)
        new_schema_paths.append(os.path.join(output_base, "_input", rel_pattern))

    for path in inp.get("operationSearchPaths", []):
        abs_path = resolve_input_path(path, config_dir)
        # Copy each matching file
        for f in globmod.glob(abs_path, recursive=True):
            rel = os.path.relpath(f, config_dir)
            dest = os.path.join(output_base, "_input", rel)
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            shutil.copy2(f, dest)
        # Rewrite the path pattern
        rel_pattern = os.path.relpath(abs_path, config_dir)
        new_op_paths.append(os.path.join(output_base, "_input", rel_pattern))

    inp["schemaSearchPaths"] = new_schema_paths
    inp["operationSearchPaths"] = new_op_paths

    return config


def has_relative_operations(config: dict) -> bool:
    """Check if config uses 'relative' operations output."""
    ops = config.get("output", {}).get("operations", {})
    return "relative" in ops


def rewrite_config(config: dict, config_dir: str, output_base: str) -> dict:
    """Return a new config dict with paths rewritten."""
    # Handle relative operations specially: copy source files into output dir
    if has_relative_operations(config):
        config = copy_input_files_for_relative_ops(config, config_dir, output_base)
    else:
        config = json.loads(json.dumps(config))  # deep copy
        # Rewrite input paths to absolute
        inp = config.get("input", {})
        for key in ("schemaSearchPaths", "operationSearchPaths"):
            if key in inp:
                inp[key] = [resolve_input_path(p, config_dir) for p in inp[key]]

    # -- Rewrite output paths to temp dir --
    out = config.get("output", {})

    # schemaTypes.path
    schema_types = out.get("schemaTypes", {})
    if "path" in schema_types:
        schema_types["path"] = redirect_output_path(schema_types["path"], output_base)

    # operations: absolute
    ops = out.get("operations", {})
    if "absolute" in ops and "path" in ops["absolute"]:
        ops["absolute"]["path"] = redirect_output_path(ops["absolute"]["path"], output_base)
    # relative operations: leave subpath as-is, files go next to (now-copied) sources

    # testMocks: absolute or swiftPackage
    test_mocks = out.get("testMocks", {})
    if "absolute" in test_mocks and "path" in test_mocks["absolute"]:
        test_mocks["absolute"]["path"] = redirect_output_path(
            test_mocks["absolute"]["path"], output_base
        )
    # swiftPackage test mocks go into the schemaTypes path, already redirected

    # operationManifest (if present)
    if "operationManifest" in out and "path" in out["operationManifest"]:
        out["operationManifest"]["path"] = redirect_output_path(
            out["operationManifest"]["path"], output_base
        )

    # Disable pruneGeneratedFiles since we're writing to a clean temp dir
    # and pruning would try to clean files that don't exist
    options = config.get("options", {})
    options["pruneGeneratedFiles"] = False
    config["options"] = options

    return config


def main():
    if len(sys.argv) < 4:
        print(
            f"Usage: {sys.argv[0]} <original_config> <output_config> <output_base_dir>",
            file=sys.stderr,
        )
        sys.exit(1)

    original_config_path = sys.argv[1]
    output_config_path = sys.argv[2]
    output_base = os.path.abspath(sys.argv[3])

    config_dir = os.path.dirname(os.path.abspath(original_config_path))

    with open(original_config_path) as f:
        config = json.load(f)

    rewritten = rewrite_config(config, config_dir, output_base)

    os.makedirs(os.path.dirname(output_config_path), exist_ok=True)
    with open(output_config_path, "w") as f:
        json.dump(rewritten, f, indent=2)
        f.write("\n")


if __name__ == "__main__":
    main()
