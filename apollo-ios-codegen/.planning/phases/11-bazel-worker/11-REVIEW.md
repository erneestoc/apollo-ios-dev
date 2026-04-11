---
phase: 11-bazel-worker
reviewed: 2026-04-10T18:42:00Z
depth: standard
files_reviewed: 10
files_reviewed_list:
  - rust/apollo-codegen-lib/src/codegen.rs
  - rust/apollo-ios-cli/build.rs
  - rust/apollo-ios-cli/Cargo.toml
  - rust/apollo-ios-cli/src/main.rs
  - rust/apollo-ios-cli/src/worker_bench.rs
  - rust/apollo-ios-cli/src/worker_io.rs
  - rust/apollo-ios-cli/src/worker_proto.rs
  - rust/apollo-ios-cli/src/worker.rs
  - rust/Cargo.toml
  - rust/proto/worker_protocol.proto
findings:
  critical: 1
  warning: 3
  info: 2
  total: 6
status: issues_found
---

# Phase 11: Code Review Report

**Reviewed:** 2026-04-10T18:42:00Z
**Depth:** standard
**Files Reviewed:** 10
**Status:** issues_found

## Summary

This review covers the Bazel persistent worker mode implementation for the Apollo iOS Rust codegen CLI. The implementation adds protobuf-based worker protocol I/O (`worker_io.rs`, `worker_proto.rs`), a persistent worker loop with schema+IR caching (`worker.rs`), benchmark tests (`worker_bench.rs`), and refactoring of `codegen.rs` to expose `compile_schema_and_ir()` and `generate_from_ir()` as separate public methods.

The architecture is sound: singleplex worker protocol, varint-delimited protobuf I/O, digest-based cache keying, graceful signal/EOF shutdown, and all non-protocol output routed to stderr. The critical issue centers on the `unsafe` mutation of `Arc`-wrapped types in `process_schema_customizations`, which is re-invoked on cached data in the worker's warm path. Three warnings address a missing loop bound, cache key completeness, and a production `unwrap()`.

## Critical Issues

### CR-01: Unsafe mutation through shared Arc on cached data in worker warm path

**File:** `rust/apollo-codegen-lib/src/codegen.rs:820-828`
**Issue:** The function `set_custom_name_on_arc` casts `Arc::as_ptr()` to `*mut T` and mutates the inner value. This violates Rust's aliasing rules -- `Arc` guarantees shared (immutable) access, and creating a mutable reference through it is undefined behavior regardless of thread count. More critically, in worker mode the cached `CompileResult` is reused across requests, so `generate_from_ir()` calls `process_schema_customizations()` on the same `Arc`-wrapped objects repeatedly. If the first call sets `custom_name = Some("Foo")` and a subsequent call sets `custom_name = Some("Foo")` again, this writes through a shared reference, which is UB even if the value is identical. Additionally, the outer function signature is safe (`fn set_custom_name_on_arc<T: HasNameField>(arc: &Arc<T>, custom_name: &str)`) -- the compiler does not enforce any threading or uniqueness invariant at call sites.

**Fix:** Replace the `unsafe` Arc mutation with a safe pattern. Two options:

Option A -- Use `Arc<RwLock<T>>` or `Arc<Mutex<T>>` for types that need post-construction mutation:
```rust
// In the type definitions, wrap mutable fields:
pub struct GraphQLName {
    pub schema_name: String,
    pub custom_name: std::sync::RwLock<Option<String>>,
}
```

Option B -- Apply customizations during IR construction (before wrapping in Arc), so the Arc-wrapped values are never mutated after creation:
```rust
// In compile_schema_and_ir, apply customizations before freezing into Arc:
let mut ir = IRBuilder::new(compilation_result.clone());
apply_schema_customizations_mut(&mut ir, config);
// Now ir types are finalized, wrap in Arc and cache
```

Option B is preferable as it avoids runtime locking overhead and eliminates the unsafe entirely. It would require `process_schema_customizations` to be called before the `CompileResult` is cached in the worker, rather than on every `generate_from_ir()` call.

## Warnings

### WR-01: Fragment reference resolution loop has no iteration bound

**File:** `rust/apollo-codegen-lib/src/codegen.rs:522-555`
**Issue:** The third-pass loop in `build_fragment_definitions` iterates until convergence (`changed == false`). While the GraphQL spec forbids circular fragment spreads (preventing true infinite loops in valid inputs), there is no defensive iteration cap. If a parser bug or future refactoring allows cyclic references to slip through, this loop would run forever. The convergence depends on a subtle property: updating fragment A creates a new Arc, which invalidates any reference to A held by other fragments, potentially triggering cascading updates. With N mutually-referencing fragments in a DAG, worst case is O(depth) iterations, but this is not bounded in code.

**Fix:** Add a maximum iteration guard:
```rust
let max_iterations = all_names.len() + 1; // DAG depth cannot exceed fragment count
let mut iteration = 0;
loop {
    iteration += 1;
    if iteration > max_iterations {
        // Should never happen with valid GraphQL, but fail loudly if it does
        return Err(CodegenError::InvalidConfiguration {
            message: "Fragment reference resolution exceeded maximum iterations".to_string(),
        });
    }
    let mut changed = false;
    // ... rest of loop ...
}
```

### WR-02: Worker cache key excludes configuration file content

**File:** `rust/apollo-ios-cli/src/worker.rs:198`
**Issue:** `DigestKey::from_inputs()` is computed solely from the `WorkRequest.inputs` field (Bazel input file digests). The configuration file is loaded separately via `generate_cmd.inputs.get_codegen_configuration()` on line 174. If the Bazel rule does not list the codegen configuration file as an input (common oversight in rule authoring), changing config options (e.g., `schemaNamespace`, `pruneGeneratedFiles`, `fieldMerging`) would not invalidate the cache, causing `generate_from_ir()` to run with the new config against stale schema+IR compiled under the old config. This could produce silently incorrect output.

**Fix:** Include a hash of the serialized configuration in the cache key:
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct DigestKey {
    digests: BTreeMap<String, Vec<u8>>,
    config_hash: Vec<u8>, // SHA-256 of serialized config
}

impl DigestKey {
    fn from_inputs_and_config(
        inputs: &[Input],
        config: &ApolloCodegenConfiguration,
    ) -> Self {
        let mut digests = BTreeMap::new();
        for input in inputs {
            digests.insert(input.path.clone(), input.digest.clone());
        }
        // Hash the config to detect changes not tracked by Bazel inputs
        let config_json = serde_json::to_string(config).unwrap_or_default();
        let config_hash = sha2::Sha256::digest(config_json.as_bytes()).to_vec();
        DigestKey { digests, config_hash }
    }
}
```

### WR-03: Unwrap on Option in production code path

**File:** `rust/apollo-ios-cli/src/worker.rs:228`
**Issue:** `cache.as_ref().unwrap()` is logically safe -- at this point either the cache was already populated (hit) or just rebuilt (miss with successful compilation). The miss-with-error case returns early on line 215. However, `unwrap()` in a production code path is fragile to refactoring. If a future change introduces a code path that reaches line 228 with `cache == None`, the worker process panics and crashes.

**Fix:** Replace with explicit match or `expect` with a descriptive message, or restructure to eliminate the need:
```rust
let compile_result = match cache.as_ref() {
    Some(c) => &c.compile_result,
    None => {
        return WorkResponse {
            exit_code: 1,
            output: "Internal error: cache unexpectedly empty after rebuild".to_string(),
            request_id: 0,
            was_cancelled: false,
        };
    }
};
```

## Info

### IN-01: Unused variable `_merge_named_fragment_fields`

**File:** `rust/apollo-codegen-lib/src/codegen.rs:911-914`
**Issue:** The variable `_merge_named_fragment_fields` is assigned from the config but never used. The leading underscore suppresses the compiler warning, but this suggests either dead code from an incomplete feature or a missing usage site.
**Fix:** If the variable is needed for future logic (e.g., conditional fragment field merging), add a TODO comment explaining the intent. Otherwise, remove the dead binding.

### IN-02: Unused parameter `_generators` in `collect_non_fatal_errors`

**File:** `rust/apollo-codegen-lib/src/codegen.rs:1099`
**Issue:** The `_generators` parameter is accepted but never used. The function groups all errors under a generic `"_codegen"` key rather than mapping them back to their originating generators. This parameter exists in the signature (likely to match a planned per-generator error tracking feature) but serves no purpose currently.
**Fix:** Either implement per-generator error tracking using the parameter, or remove it from the signature and update call sites:
```rust
fn collect_non_fatal_errors(
    errors: &[NonFatalError],
    target: &mut NonFatalErrors,
) {
    // ...
}
```

---

_Reviewed: 2026-04-10T18:42:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
