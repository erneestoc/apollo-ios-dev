# apollo-codegen-rs

Pure Rust replacement for the Apollo iOS code generation pipeline. Drop-in compatible with the Swift+JavaScript implementation at version 1.15.1.

## What This Is

The Apollo iOS codegen takes GraphQL schemas and operations and generates Swift types, selection sets, fragments, mock objects, and supporting infrastructure. The original implementation uses a TypeScript compiler running in JavaScriptCore (bridged through Swift), POSIX glob for file discovery, and Swift templates for code rendering.

This Rust implementation replaces the entire pipeline: parsing, validation, compilation, IR building, template rendering, and file output. It produces **byte-for-byte identical output** to the Swift codegen for all 55 generated files in the AnimalKingdomAPI test suite, including the most complex operations with 30+ nested structs, 6 inline fragment types, `@skip`/`@include` conditional inclusion, and full selection merging.

## Architecture

```
Config (JSON) --> Glob (file discovery) --> Frontend (GraphQL parsing)
    --> IR (intermediate representation) --> Render (Swift templates) --> Files
```

### Crates

| Crate | Purpose | Key Types |
|-------|---------|-----------|
| `config` | Parse `apollo-codegen-config.json` | `ApolloCodegenConfiguration` |
| `glob` | File discovery with globstar, excludes | `Glob`, `match_search_paths` |
| `frontend` | GraphQL parsing via `apollo-compiler`, introspection JSON, compilation | `GraphQLFrontend`, `CompilationResult` |
| `ir` | Intermediate representation, field collection | `IRBuilder`, `Schema`, `Operation`, `NamedFragment`, `FieldCollector` |
| `render` | TemplateString engine, 14 templates, IR adapter, schema customization | `ir_adapter`, `SchemaCustomizer` |
| `cache` | Content-addressed caching (skeleton) | - |
| `cli` | Drop-in CLI with full pipeline | `pipeline::generate` |
| `golden-test` | 372 fixture files + comparison harness | `compare_api`, `load_golden_files` |

### Pipeline Flow

1. **Config**: Parses JSON config with serde, handling externally-tagged enums (`swiftPackageManager`, `embeddedInTarget`, `inSchemaModule`, etc.)
2. **Glob**: Discovers schema (`.graphqls`, `.json`) and operation (`.graphql`) files with globstar expansion, exclude patterns, directory exclusion
3. **Frontend**: Loads schemas (SDL or introspection JSON) via `apollo-compiler`, parses operations, merges documents, compiles to `CompilationResult`. Injects `__typename` into source fields matching `graphql-js` format. Strips `@apollo_client_ios_localCacheMutation` directives.
4. **IR**: Builds `Operation` and `NamedFragment` from compilation result. `FieldCollector` walks all operations/fragments to gather per-object field lists for mock generation.
5. **Render**: The `ir_adapter` converts IR types into template configs. Templates generate Swift source strings. The `SchemaCustomizer` applies type/enum/field renaming.
6. **CLI**: Orchestrates the pipeline, writes files to disk.

### Key Algorithms

**EntitySelectionTree Merging** (`ir_adapter.rs`): The most complex piece. Determines which nested types (Height, Predator, Fragments) exist within each inline fragment scope by computing merged selections from ancestors, siblings, and named fragments. Implements:
- Entity field nesting rules (only when adding new selections beyond parent scope)
- Absorbed inline fragments (e.g., `AsAnimal` into `AsPet` when Pet implements Animal)
- Sibling type merging with transitive fragment field propagation
- Promoted inline fragments from fragment spreads with proper ordering
- `@skip`/`@include` conditional struct renaming and field optionality

**TemplateString Engine** (`template_string.rs`): Port of Swift's indent-aware `StringInterpolation` with backward-scanning indent detection, line removal on empty interpolation, and section semantics.

**Source Field Formatting** (`compiler.rs`): Post-processes `apollo-compiler`'s printed output to match `graphql-js` format: `__typename` injection with context-aware rules (skip root operations, skip inline fragments), object default value reformatting (spaces around braces, comma handling by brace depth).

## Templates

| Template | Generates | Notes |
|----------|-----------|-------|
| `object` | `Schema/Objects/*.graphql.swift` | Single-interface bracket formatting |
| `interface` | `Schema/Interfaces/*.graphql.swift` | |
| `union_type` | `Schema/Unions/*.graphql.swift` | |
| `enum_type` | `Schema/Enums/*.graphql.swift` | camelCase conversion, deprecation |
| `input_object` | `Schema/InputObjects/*.graphql.swift` | Dual initializers for deprecated fields |
| `custom_scalar` | `Schema/CustomScalars/*.swift` | Editable file header |
| `schema_metadata` | `SchemaMetadata.graphql.swift` | Type encounter ordering |
| `schema_config` | `SchemaConfiguration.swift` | Editable, never overwritten |
| `package_swift` | `Package.swift` | SPM module with test mock target |
| `operation` | `Operations/{Queries,Mutations,Subscriptions}/*.graphql.swift` | Local cache mutation variant |
| `fragment` | `Fragments/*.graphql.swift` | Mutable variant for LCM |
| `selection_set` | Nested within operations/fragments | Conditional field groups, `@skip`/`@include` |
| `mock_object` | `TestMocks/*+Mock.graphql.swift` | Field collection from all operations |
| `mock_interfaces` | `TestMocks/MockObject+Interfaces.graphql.swift` | |
| `mock_unions` | `TestMocks/MockObject+Unions.graphql.swift` | |

## Verification

```bash
# Build
cargo build -p apollo-codegen-cli

# Run Rust tests (67+ tests)
cargo test

# Generate for a test config
cd Tests/TestCodeGenConfigurations/SwiftPackageManager
apollo-codegen-rs/target/debug/apollo-ios-cli-rs generate --path apollo-codegen-config.json

# Verify Swift compilation (0 errors, 0 warnings)
swift build

# Verify Swift tests pass
swift test

# Byte-for-byte comparison against golden files (55/55 match)
cargo test -p apollo-codegen-golden-test
```

## Compatibility

- **55/55 generated files** byte-for-byte identical to Swift codegen at v1.15.1
- **Schema customization**: Type renaming, enum case renaming, input field renaming
- **`@skip`/`@include`**: Conditional struct renaming, optional types, field groups
- **Local cache mutations**: `MutableSelectionSet`, `var __data`, get/set accessors
- **Introspection JSON**: StarWarsAPI schema format supported
- **All output modes**: `inSchemaModule`, `absolute`, `relative`, `swiftPackage`, `embeddedInTarget`, `other`
- **Test mocks**: MockObject with field collection, MockInterfaces, MockUnions

## What's Not Implemented

- `fetch-schema` CLI command (schema download via introspection)
- `init` CLI command (configuration file scaffolding)
- `generate-operation-manifest` CLI command (persisted queries)
- Content-addressed caching layer
- Other test API validation (StarWarsAPI, GitHubAPI, UploadAPI, SubscriptionAPI)
- Other configuration variant validation (EmbeddedInTarget, Other-CustomTarget, etc.)
- Performance benchmarking vs Swift codegen
- Version upgrade diff tooling

## Branch

`rust/1.15.1` branched from commit `9cff59ba` (Release 1.15.1). 27 commits.
