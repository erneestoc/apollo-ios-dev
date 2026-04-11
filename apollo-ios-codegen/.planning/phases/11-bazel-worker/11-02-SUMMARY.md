---
phase: 11-bazel-worker
plan: 02
subsystem: worker-loop
tags: [bazel, worker, caching, protobuf, cli]
dependency_graph:
  requires: [11-01]
  provides: [worker-loop, persistent-worker-mode, digest-cache]
  affects: [apollo-ios-cli, main.rs]
tech_stack:
  added: []
  patterns: [scope-based-memory-isolation, digest-keyed-caching, pre-clap-flag-detection]
key_files:
  created:
    - apollo-ios-codegen/rust/apollo-ios-cli/src/worker.rs
  modified:
    - apollo-ios-codegen/rust/apollo-ios-cli/src/main.rs
    - apollo-ios-codegen/rust/apollo-ios-cli/Cargo.toml
    - apollo-ios-codegen/rust/Cargo.lock
decisions:
  - "DigestKey uses BTreeMap for deterministic ordering per FNDN-05"
  - "Panic hook redirects to stderr to protect stdout protocol stream (WRKR-04)"
  - "init command requires --module-type arg; test updated to pass valid args for unsupported-command path"
metrics:
  duration: 5m
  completed: "2026-04-11T00:07:30Z"
  tasks_completed: 1
  tasks_total: 1
  tests_added: 8
  tests_passing: 13
---

# Phase 11 Plan 02: Worker Loop and Cache Summary

Worker loop with DigestKey-based schema+IR caching, --persistent_worker pre-clap detection, scope-based per-request memory isolation, ctrlc signal handling, and panic hook for stdout protection.

## Tasks Completed

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | Worker loop with cache, signal handling, and CLI integration | e1281c51c | worker.rs (new), main.rs, Cargo.toml |

## Implementation Details

### worker.rs (new, 370 lines)

**Core structures:**
- `DigestKey` -- BTreeMap-based cache key from Bazel input digests (D-90, FNDN-05)
- `CachedCompilation` -- holds CompileResult + digest key in worker loop outer scope (D-89, D-93)

**Public API:**
- `run_worker_loop()` -- main entry point called from main.rs when --persistent_worker detected
  - Sets panic hook to redirect panics to stderr (WRKR-04, T-11-06)
  - Installs ctrlc signal handler for SIGINT/SIGTERM graceful shutdown (D-95)
  - Reads WorkRequests from stdin via length-delimited protobuf (plan 11-01's worker_io)
  - Dispatches to handle_request() per request
  - Writes WorkResponse to stdout (only protocol bytes)
  - Exits cleanly on EOF or signal

**Private functions:**
- `handle_request()` -- per-request handler (D-88, D-89, D-91, D-92)
  - Prepends dummy argv[0] and parses WorkRequest.arguments through clap
  - Rejects non-generate commands with exit_code=1
  - Loads configuration from parsed args
  - Computes DigestKey from WorkRequest.inputs
  - Cache miss: calls compile_schema_and_ir(), stores in CachedCompilation
  - Cache hit: reuses existing CompileResult
  - Calls generate_from_ir() with cached/fresh CompileResult
  - All request-local state drops when function returns (D-92)

### main.rs (modified)

- `Cli` and `Commands` made `pub` so worker.rs can import them
- `mod worker;` declaration added alongside existing worker_io/worker_proto
- Pre-clap `--persistent_worker` detection added before `Cli::parse()`
- Module declarations consolidated at top of file

### Cargo.toml (modified)

- Added `apollo-codegen-lib` as direct dependency for worker module imports

## Test Results

8 new tests added in worker.rs, all passing:
- `test_digest_key_from_empty_inputs` -- empty input produces empty BTreeMap
- `test_digest_key_deterministic_ordering` -- unordered inputs produce sorted keys
- `test_digest_key_equality` -- same path+digest pairs are equal
- `test_digest_key_inequality_different_digest` -- different digests are not equal
- `test_digest_key_inequality_different_path` -- different paths are not equal
- `test_handle_request_unsupported_command` -- init command returns exit_code=1 with clear message
- `test_handle_request_invalid_args` -- invalid flags return exit_code=1 with parse error
- `test_handle_request_singleplex_request_id` -- all responses have request_id=0 (WRKR-02)

Total: 13 tests pass in apollo-ios-cli crate (8 worker + 5 worker_io from plan 11-01).

## Verification

1. `cargo test -p apollo-ios-cli -- worker` -- 8/8 worker tests pass
2. `cargo build -p apollo-ios-cli` -- binary builds cleanly
3. No `println!` in worker.rs -- only `eprintln!` (WRKR-04 verified)
4. All WorkResponse constructions use `request_id: 0` (WRKR-02 verified)
5. 11 pre-existing failures in apollo-codegen-lib template tests (unrelated to this plan)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed test_handle_request_unsupported_command test**
- **Found during:** Task 1 verification
- **Issue:** Plan's test passed `["init"]` as arguments, but clap requires `--module-type` for the init command, so clap returned a parse error before reaching the command-type check
- **Fix:** Updated test to pass `["init", "--module-type", "swift-package"]` so clap succeeds and the unsupported-command branch is exercised
- **Files modified:** worker.rs (test only)
- **Commit:** e1281c51c

**2. [Rule 3 - Blocking] Added apollo-codegen-lib as direct dependency**
- **Found during:** Task 1 implementation
- **Issue:** worker.rs imports `apollo_codegen_lib::codegen::*` and `apollo_codegen_lib::templates::*` directly, but Cargo.toml only had `codegen-cli` as a dependency
- **Fix:** Added `apollo-codegen-lib = { path = "../apollo-codegen-lib" }` to Cargo.toml
- **Files modified:** Cargo.toml, Cargo.lock
- **Commit:** e1281c51c

**3. [Rule 1 - Bug] Removed unused import warning**
- **Found during:** Task 1 build verification
- **Issue:** `use codegen_cli::commands::generate::Generate` was unused because the Generate type is accessed through `Commands::Generate(cmd)` destructuring, not directly
- **Fix:** Removed the unused import
- **Files modified:** worker.rs
- **Commit:** e1281c51c

## Self-Check: PASSED

- worker.rs: FOUND
- main.rs: FOUND
- SUMMARY.md: FOUND
- Commit e1281c51c: FOUND
