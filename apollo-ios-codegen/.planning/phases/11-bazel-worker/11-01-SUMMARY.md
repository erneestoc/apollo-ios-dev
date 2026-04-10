---
phase: 11-bazel-worker
plan: 01
subsystem: worker-protocol
tags: [bazel, protobuf, worker, pipeline-refactor]
dependency_graph:
  requires: []
  provides: [worker-proto-types, worker-io, split-codegen-pipeline]
  affects: [apollo-ios-cli, apollo-codegen-lib]
tech_stack:
  added: [prost-0.14, prost-build-0.14, protox-0.9, bytes-1.11, ctrlc-3.5]
  patterns: [length-delimited-protobuf, build-rs-codegen, split-pipeline]
key_files:
  created:
    - rust/proto/worker_protocol.proto
    - rust/apollo-ios-cli/build.rs
    - rust/apollo-ios-cli/src/worker_proto.rs
    - rust/apollo-ios-cli/src/worker_io.rs
  modified:
    - rust/Cargo.toml
    - rust/apollo-ios-cli/Cargo.toml
    - rust/apollo-ios-cli/src/main.rs
    - rust/apollo-codegen-lib/src/codegen.rs
decisions:
  - "Proto codegen in apollo-ios-cli crate (not a separate crate) since worker is part of the binary"
  - "CompileResult struct defined outside impl ApolloCodegen for importability by worker module"
  - "build_with_context() delegates to compile_schema_and_ir() + generate_from_ir() preserving identical behavior"
metrics:
  duration: ~6 minutes
  completed: "2026-04-10T23:59:00Z"
  tasks_completed: 2
  tasks_total: 2
  files_created: 4
  files_modified: 4
---

# Phase 11 Plan 01: Worker Protocol Foundation Summary

Vendored Bazel worker proto with prost-build+protox codegen, length-delimited I/O with 5 round-trip tests, and ApolloCodegen pipeline split into compile_schema_and_ir() + generate_from_ir() for worker cache reuse.

## Task Results

| Task | Name | Commit | Status |
|------|------|--------|--------|
| 1 | Vendored proto, prost-build, and worker I/O module | e9441882b | Done |
| 2 | Refactor ApolloCodegen pipeline for cache-compatible split | ef4ef73fa | Done |

## What Was Built

### Task 1: Worker Protocol Foundation
- **Proto file** (`rust/proto/worker_protocol.proto`): Vendored Bazel worker protocol with WorkRequest, WorkResponse, and Input message types
- **Build script** (`rust/apollo-ios-cli/build.rs`): prost-build + protox compilation at build time -- no protoc system dependency required
- **Proto module** (`worker_proto.rs`): Re-exports generated types from OUT_DIR via `include!` macro
- **I/O module** (`worker_io.rs`): `read_work_request()` and `write_work_response()` with:
  - Varint length-delimited encoding/decoding
  - EOF detection returning `None` for graceful shutdown (D-95)
  - 64MB max message size guard (T-11-02)
  - Truncated varint detection
  - 5 unit tests: round-trip request, round-trip response, EOF, oversized rejection, inputs
- **Dependencies**: prost 0.14, bytes 1.11, ctrlc 3.5 as workspace deps; prost-build 0.14, protox 0.9 as build-deps

### Task 2: Pipeline Refactoring
- **CompileResult struct**: Holds `Arc<CompilationResult>` and `IRBuilder` for cross-request caching
- **compile_schema_and_ir()**: Public method running stages 1-7 (config validation through IR construction)
- **generate_from_ir()**: Public method running stages 8-12 (customizations, codegen, manifest, pruning, errors)
- **build_with_context()**: Now delegates to both methods in sequence -- identical behavior preserved

## Deviations from Plan

None -- plan executed exactly as written.

## Verification Results

| Check | Result |
|-------|--------|
| `cargo build -p apollo-ios-cli` | Pass -- proto compilation succeeds |
| `cargo test -p apollo-ios-cli -- worker_io` | 5/5 tests pass |
| `cargo test -p apollo-codegen-lib --lib` | 601 pass, 11 fail (pre-existing template failures, unrelated) |
| `cargo build --workspace` | Pass |
| No `println!` in worker files | Verified (WRKR-04 compliant) |

## Threat Mitigations Applied

| Threat ID | Mitigation |
|-----------|------------|
| T-11-01 | prost::decode returns Err on malformed protobuf; varint loop capped at 10 bytes |
| T-11-02 | MAX_MESSAGE_SIZE constant (64MB) prevents unbounded allocation |
| T-11-03 | No println! in worker_io.rs or worker_proto.rs; all output to stderr |

## Pre-existing Issues (Not Introduced by This Plan)

11 template rendering test failures in apollo-codegen-lib exist both before and after these changes. They are in object_template, input_object_template, one_of_input_object_template, graphql_name_rendering, and operation_template_renderer tests. These are unrelated to the pipeline refactoring.
