# Apollo iOS Rust Codegen — Progress Report

## Overview

We've built a pure Rust drop-in replacement for the Apollo iOS code generation CLI (which currently uses JavaScript via JavaScriptCore + Swift). The Rust implementation produces **byte-for-byte identical Swift output** across all test suites and is **3.6–6.5x faster**.

## Compatibility Results

### API Tests (5/5 PASS — 367 files)

| API | Files | Status |
|-----|-------|--------|
| AnimalKingdomAPI | 55/55 | PASS |
| StarWarsAPI | 70/70 | PASS |
| GitHubAPI | 232/232 | PASS |
| UploadAPI | 10/10 | PASS |
| SubscriptionAPI | 5/5 | PASS |

### Config Tests (7/7 PASS — 367 files)

| Config | Files | Status |
|--------|-------|--------|
| SwiftPackageManager | 55/55 | PASS |
| EmbeddedInTarget-InSchemaModule | 55/55 | PASS |
| EmbeddedInTarget-RelativeAbsolute | 52/52 | PASS |
| Other-CustomTarget | 54/54 | PASS |
| Other-CocoaPods | 54/54 | PASS |
| SPMInXcodeProject | 55/55 | PASS |
| CodegenXCFramework | 42/42 | PASS |

### Permutation Tests (50/50 PASS)

50 randomly generated config combinations testing all option interactions — every permutation produces byte-for-byte identical output.

### Rust Unit Tests

All pass (0 failures).

## Performance Benchmarks

Measured with `hyperfine` (5+ runs, release builds, median):

| Schema | Rust | Swift | Speedup |
|--------|------|-------|---------|
| AnimalKingdomAPI (55 files) | **9.4ms** | 38.8ms | **4.1x** |
| StarWarsAPI (70 files) | **13.9ms** | 49.4ms | **3.6x** |
| GitHubAPI (232 files) | **42.8ms** | 279.2ms | **6.5x** |

Speedup increases with schema complexity.

## New CLI Features for Large Modular Projects

Built specifically for the ~150 framework use case where the current pipeline runs full codegen 150+ times:

### `--timing` flag

Shows per-phase timing breakdown:

```
$ apollo-ios-cli-rs generate --path config.json --timing

[timing] Schema loading:        10ms
[timing] Operation parsing:      0ms
[timing] Compilation:            2ms
[timing] IR building:            0ms
[timing] Code generation:        0ms
[timing] Total:                 15ms (232 files)
[timing] File writing:          11ms
```

### `generate-schema-types` (schema-only output)

Only emits schema type files (Objects, Enums, Unions, InputObjects, Interfaces, CustomScalars, SchemaMetadata, etc.) — no operations or fragments.

```
$ apollo-ios-cli-rs generate-schema-types --path config.json --timing
# 228 schema files in 14ms
```

### `generate-operations` (per-framework filtering)

Only emits operation/fragment files, with `--only-for-paths` glob filtering. Still parses the full schema for type resolution but only renders matching operations.

```
$ apollo-ios-cli-rs generate-operations --path config.json \
    --only-for-paths "Features/Cart/**/*.graphql" --timing
# 3 files in 2ms
```

### `--concat` (single output file)

Writes all generated output to a single file instead of individual files — eliminates shell concatenation in Bazel wrappers.

```
$ apollo-ios-cli-rs generate-operations --path config.json \
    --only-for-paths "**/Cart*" --concat cart-operations.swift
```

### `--save-ir` / `--load-ir` (stubbed for next iteration)

Will serialize the parsed IR (schema + fragment index) to a binary cache. The schema_types step produces it once, then each operations step loads it instead of re-parsing. Expected to cut per-framework time from milliseconds to sub-millisecond.

### Expected Impact for 150-Framework Project

| Step | Before | After |
|------|--------|-------|
| Schema types | Full codegen + filter ~1200 files | `generate-schema-types` (~228 files, **14ms**) |
| Per-framework ops (x150) | Full codegen + filter ~1200 files each | `generate-operations --only-for-paths` (1-10 files, **2ms** each) |
| Total wall time | ~150x full pipeline (**150s+**) | 14ms + 150x2ms = **~0.3s** |

## Bug Fixes Applied

- **`legacyAPQ` to `legacy`**: Operation manifest version now matches Swift CLI naming (accepts both for backwards compatibility)
- **Duplicate `__typename`**: Fixed in selection set initializer rendering
- **Query string format**: Inline literal arguments now match graphql-js print output

## Template Architecture

Migrated 12 of 14 templates from manual string building to Askama declarative templates (`.swift.askama` files). The remaining 3 (operation, fragment, selection_set) use programmatic rendering — matching the Swift codegen's own architecture which uses Swift string interpolation DSL, not a template engine, for these complex recursive structures.

## What's Next

1. **IR Cache (`--save-ir`/`--load-ir`)**: Serialize parsed schema to binary cache for sub-millisecond per-framework generation
2. **Fuzz Testing**: Schema generator improvements for better coverage of edge cases
3. **Daemon Mode** (future): Long-running process accepting generation requests over IPC for zero-startup overhead
