---
phase: 11-bazel-worker
verified: 2026-04-10T22:30:00Z
status: human_needed
score: 11/11 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Run the binary with --persistent_worker flag and send a real WorkRequest via stdin pipe"
    expected: "Binary enters worker loop, reads the protobuf-encoded WorkRequest, runs codegen, writes WorkResponse to stdout, and continues waiting for next request"
    why_human: "End-to-end worker protocol I/O with actual piped stdin/stdout cannot be verified via static code analysis or unit tests alone"
  - test: "Send two consecutive WorkRequests with identical input digests, observe stderr messages"
    expected: "First request prints 'Cache miss', second prints 'Cache hit'; second request is measurably faster"
    why_human: "Full protocol-level cache behavior requires a live worker process and timed observation"
  - test: "Close stdin (send EOF) while worker is running"
    expected: "Worker prints 'Worker received EOF, shutting down' to stderr and exits with code 0"
    why_human: "Graceful shutdown behavior requires process lifecycle observation"
  - test: "Send SIGINT to the running worker process"
    expected: "Worker prints signal message to stderr and exits with code 0"
    why_human: "Signal handling requires process lifecycle observation"
---

# Phase 11: Bazel Worker Verification Report

**Phase Goal:** The binary runs as a persistent Bazel worker, processing codegen requests via the worker protocol with caching for warm-start performance
**Verified:** 2026-04-10T22:30:00Z
**Status:** human_needed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | The binary accepts WorkRequest via stdin and returns WorkResponse via stdout in singleplex mode, with all non-protocol output redirected to stderr | VERIFIED | worker.rs:run_worker_loop() reads via read_work_request(), writes via write_work_response(); all WorkResponse constructions use request_id: 0 (9 occurrences); zero println! in worker files; all output uses eprintln! |
| 2 | Schema/IR caching between requests (keyed by input digests) produces measurably faster warm-start times compared to cold one-shot mode -- verified by benchmarks | VERIFIED | bench_warm_vs_cold: cold=57ms, warm=37ms, 1.6x speedup; DigestKey uses BTreeMap for deterministic ordering; cache hit/miss logic in handle_request() with compile_schema_and_ir() on miss only |
| 3 | Arena-per-request memory management prevents cross-request state leaks, and the worker shuts down gracefully on EOF/signal | VERIFIED | handle_request() scoped so all request-local state drops on return; cache lives in outer scope of run_worker_loop(); EOF returns None triggering break; ctrlc::set_handler with AtomicBool for SIGINT/SIGTERM; process::exit(0) on shutdown |
| 4 | Memory usage profiling shows no regression or unbounded growth across repeated requests | VERIFIED | bench_memory_stability: 11.6% RSS growth (well within 20% threshold); 5 warm-up iterations before measurement; 20 measurement iterations |
| 5 | prost-build compiles the vendored proto file into Rust types at build time | VERIFIED | build.rs uses protox::compile + prost_build::Config::new().compile_fds(); cargo build succeeds |
| 6 | WorkRequest and WorkResponse structs are available as generated Rust types | VERIFIED | worker_proto.rs: include!(concat!(env!("OUT_DIR"), "/blaze.worker.rs")) + pub use blaze_worker::{Input, WorkRequest, WorkResponse} |
| 7 | Length-delimited protobuf messages can be read from stdin and written to stdout | VERIFIED | worker_io.rs: read_work_request() with varint parsing + decode, write_work_response() with encode_length_delimited_to_vec(); 5 unit tests all pass |
| 8 | ApolloCodegen exposes a split pipeline: compile_schema_and_ir() and generate_from_ir() | VERIFIED | codegen.rs: pub fn compile_schema_and_ir() at line 150, pub fn generate_from_ir() at line 190; build_with_context() delegates to both at lines 136-137; CompileResult struct holds Arc<CompilationResult> + IRBuilder |
| 9 | All non-protocol output goes to stderr, nothing writes to stdout except WorkResponse | VERIFIED | Zero println! in worker.rs, worker_io.rs, worker_proto.rs, worker_bench.rs; panic hook redirects to stderr (set_hook at line 67); all logging via eprintln! |
| 10 | Binary with --persistent_worker flag enters worker loop instead of normal CLI | VERIFIED | main.rs line 55: args.iter().any(a == "--persistent_worker") triggers worker::run_worker_loop() before Cli::parse() |
| 11 | Worker reuses cached schema+IR when Bazel input digests match previous request | VERIFIED | worker.rs: DigestKey::from_inputs() builds BTreeMap from request.inputs; needs_rebuild checks digest_key equality; cache miss calls compile_schema_and_ir(), cache hit skips it |

**Score:** 11/11 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `rust/proto/worker_protocol.proto` | Vendored Bazel worker protocol proto definition | VERIFIED | 27 lines; contains WorkRequest, WorkResponse, Input messages |
| `rust/apollo-ios-cli/build.rs` | prost-build + protox compilation of proto file | VERIFIED | 13 lines; protox::compile + prost_build::Config::new().compile_fds() + rerun-if-changed |
| `rust/apollo-ios-cli/src/worker_proto.rs` | Module that re-exports generated proto types | VERIFIED | 10 lines; include! macro + pub use of Input, WorkRequest, WorkResponse |
| `rust/apollo-ios-cli/src/worker_io.rs` | Read/write functions for length-delimited protobuf I/O | VERIFIED | 195 lines; read_work_request + write_work_response + 5 tests; MAX_MESSAGE_SIZE guard |
| `rust/apollo-ios-cli/src/worker.rs` | Worker loop, cache management, signal handling, request dispatching | VERIFIED | 383 lines (>150 min); run_worker_loop + handle_request + DigestKey + CachedCompilation + 8 tests |
| `rust/apollo-ios-cli/src/main.rs` | Pre-clap --persistent_worker detection branching into worker loop | VERIFIED | --persistent_worker check at line 55; pub struct Cli; pub enum Commands; mod worker |
| `rust/apollo-codegen-lib/src/codegen.rs` | Refactored pipeline with compile_schema_and_ir() and generate_from_ir() public functions | VERIFIED | CompileResult struct at line 86; compile_schema_and_ir at line 150; generate_from_ir at line 190; build_with_context delegates at lines 136-137 |
| `rust/apollo-ios-cli/src/worker_bench.rs` | Benchmark harness for warm-vs-cold timing and RSS memory profiling | VERIFIED | 343 lines (>100 min); bench_warm_vs_cold + bench_memory_stability + bench_cold_consistency + get_rss_bytes |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| main.rs | worker.rs | worker::run_worker_loop() | WIRED | Line 56: `worker::run_worker_loop()` called when --persistent_worker detected |
| worker.rs | worker_io.rs | read_work_request/write_work_response | WIRED | Line 23: import; Lines 97, 114: called in loop |
| worker.rs | codegen.rs | compile_schema_and_ir/generate_from_ir | WIRED | Line 17: import; Lines 207, 229: called in handle_request |
| build.rs | worker_protocol.proto | protox::compile | WIRED | Line 4: protox::compile([proto_file], ["../proto/"]) |
| worker_proto.rs | build.rs output | include! macro | WIRED | Line 7: include!(concat!(env!("OUT_DIR"), "/blaze.worker.rs")) |
| worker_io.rs | worker_proto.rs | WorkRequest/WorkResponse types | WIRED | Line 10: use crate::worker_proto::{WorkRequest, WorkResponse}; used throughout |
| worker_bench.rs | codegen.rs | compile_schema_and_ir/generate_from_ir | WIRED | Line 15: import; called in all 3 benchmark functions |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|-------------------|--------|
| worker.rs | request (WorkRequest) | read_work_request() from stdin | Real protobuf decode from stdin stream | FLOWING |
| worker.rs | compile_result (CompileResult) | ApolloCodegen::compile_schema_and_ir() | Real schema parsing + IR construction | FLOWING |
| worker.rs | response (WorkResponse) | handle_request() result | Real codegen results (exit_code, output) | FLOWING |
| worker_bench.rs | compile_result | ApolloCodegen::compile_schema_and_ir() with AnimalKingdomAPI | Real fixtures, benchmarks run successfully | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Proto compilation | cargo build -p apollo-ios-cli | Finished dev profile in 13.80s | PASS |
| Worker I/O tests | cargo test -p apollo-ios-cli -- worker_io | 5/5 pass | PASS |
| Worker logic tests | cargo test -p apollo-ios-cli -- worker::tests | 8/8 pass | PASS |
| PERF-02 warm vs cold | cargo test -p apollo-ios-cli -- bench_warm_vs_cold | Cold=57ms, Warm=37ms, 1.6x speedup | PASS |
| PERF-03 RSS stability | cargo test -p apollo-ios-cli -- bench_memory_stability | 11.6% growth (<20% threshold) | PASS |
| No println! in worker files | grep for println! | Zero matches | PASS |
| All request_id=0 | grep for request_id: 0 in worker.rs | 9 occurrences, all 0 | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-----------|-------------|--------|----------|
| WRKR-01 | 11-01, 11-02 | Persistent Bazel worker mode -- long-running process accepting WorkRequest via stdin, returning WorkResponse via stdout | SATISFIED | run_worker_loop() reads/writes protobuf, loop continues until EOF/signal |
| WRKR-02 | 11-02 | Singleplex mode (serial request processing, request_id=0) | SATISFIED | All 9 WorkResponse constructions use request_id: 0; sequential loop processing |
| WRKR-03 | 11-02 | Schema/IR caching between requests (keyed by input digests) | SATISFIED | DigestKey from BTreeMap of path->digest; CachedCompilation persists in outer scope; hit/miss logic verified |
| WRKR-04 | 11-01, 11-02 | Stdout protection -- redirect all non-protocol output to stderr to prevent protocol corruption | SATISFIED | Zero println!; panic hook to stderr; all logging via eprintln! |
| WRKR-05 | 11-02 | Graceful shutdown on EOF/signal | SATISFIED | EOF returns None triggering clean break; ctrlc::set_handler for SIGINT/SIGTERM; process::exit(0) |
| WRKR-06 | 11-02 | Arena-per-request memory management to prevent cross-request state leaks | SATISFIED | handle_request() scope-based isolation; all request-local state drops on return; only cache persists in outer scope |
| PERF-02 | 11-03 | Benchmarks proving Bazel worker mode is faster than one-shot mode (warm start) | SATISFIED | bench_warm_vs_cold: 1.6x speedup measured; assertion warm_duration < cold_duration passes |
| PERF-03 | 11-03 | Memory usage profiling to prevent regression | SATISFIED | bench_memory_stability: 11.6% growth after 20 iterations + 5 warm-up; assertion growth_pct < 20.0 passes |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | - | - | - | No anti-patterns found in any worker files |

No TODO, FIXME, XXX, HACK, or PLACEHOLDER comments found. No empty implementations. No hardcoded empty data. No console.log-only handlers. All files are substantive implementations with tests.

### Human Verification Required

### 1. End-to-End Worker Protocol Test

**Test:** Build the binary and pipe a protobuf-encoded WorkRequest to stdin, verify WorkResponse appears on stdout
**Expected:** Worker reads the request, processes codegen, returns a WorkResponse with exit_code and output
**Why human:** Requires running the binary as a process with piped stdin/stdout; cannot be tested via static analysis

### 2. Cache Hit/Miss via Protocol

**Test:** Send two WorkRequests with identical input digests via stdin pipe, observe stderr
**Expected:** First request logs "Cache miss", second logs "Cache hit"; second is faster
**Why human:** Full protocol-level cache behavior requires a live worker process

### 3. EOF Graceful Shutdown

**Test:** Close stdin (EOF) while worker is running
**Expected:** Worker prints "Worker received EOF, shutting down" to stderr and exits with code 0
**Why human:** Process lifecycle shutdown behavior requires observation

### 4. Signal Graceful Shutdown

**Test:** Send SIGINT to the running worker process
**Expected:** Worker prints signal message to stderr and exits cleanly
**Why human:** Signal handling requires process lifecycle observation

### Gaps Summary

No gaps found. All 11 observable truths are verified through code analysis and behavioral spot-checks. All 8 requirements (WRKR-01 through WRKR-06, PERF-02, PERF-03) are satisfied with implementation evidence.

The 4 human verification items are standard process-lifecycle tests that require running the binary as a persistent process, which cannot be fully validated through static analysis or unit tests alone. All automated checks pass.

---

_Verified: 2026-04-10T22:30:00Z_
_Verifier: Claude (gsd-verifier)_
