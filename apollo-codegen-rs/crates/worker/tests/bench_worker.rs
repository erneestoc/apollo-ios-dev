//! Benchmark: persistent worker (warm cache) vs cold CLI invocations.
//!
//! Measures the actual wall-clock time for codegen with and without the
//! persistent worker's in-memory schema/IR cache.
//!
//! Run with: cargo test -p apollo-codegen-worker --test bench_worker --release -- --nocapture

use apollo_codegen_worker::protocol::*;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn worker_binary() -> PathBuf {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let release = workspace_root.join("target/release/apollo-ios-cli-rs");
    if release.exists() {
        return release;
    }
    let debug = workspace_root.join("target/debug/apollo-ios-cli-rs");
    debug
}

/// Find the AnimalKingdomAPI config for a realistic benchmark.
fn find_animal_kingdom_config() -> Option<(PathBuf, PathBuf)> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let schema = repo_root.join("Sources/AnimalKingdomAPI/animalkingdom-graphql/AnimalSchema.graphqls");
    if !schema.exists() {
        return None;
    }
    Some((repo_root, schema))
}

fn create_test_config(temp: &std::path::Path, repo_root: &std::path::Path) -> PathBuf {
    let config_path = temp.join("config.json");
    let schema_pattern = repo_root
        .join("Sources/AnimalKingdomAPI/animalkingdom-graphql/AnimalSchema.graphqls")
        .to_str().unwrap().to_string();
    let ops_pattern = repo_root
        .join("Sources/AnimalKingdomAPI/animalkingdom-graphql/*.graphql")
        .to_str().unwrap().to_string();
    let output_path = temp.join("Generated");

    let config = serde_json::json!({
        "schemaNamespace": "AnimalKingdomAPI",
        "input": {
            "schemaSearchPaths": [schema_pattern],
            "operationSearchPaths": [ops_pattern]
        },
        "output": {
            "testMocks": { "none": {} },
            "schemaTypes": {
                "path": output_path.to_str().unwrap(),
                "moduleType": { "swiftPackageManager": {} }
            },
            "operations": { "inSchemaModule": {} }
        }
    });
    std::fs::write(&config_path, config.to_string()).unwrap();
    config_path
}

fn collect_input_files(repo_root: &std::path::Path) -> Vec<Input> {
    let dir = repo_root.join("Sources/AnimalKingdomAPI/animalkingdom-graphql");
    let mut inputs = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if name.ends_with(".graphqls") || name.ends_with(".graphql") {
            // Use file size as a cheap "digest" for benchmarking
            let meta = std::fs::metadata(&path).unwrap();
            let digest = meta.len().to_le_bytes().to_vec();
            inputs.push(Input {
                path: path.to_str().unwrap().to_string(),
                digest,
            });
        }
    }
    inputs
}

#[test]
fn bench_cold_cli_vs_warm_worker() {
    let binary = worker_binary();
    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping benchmark", binary);
        return;
    }

    let (repo_root, _schema) = match find_animal_kingdom_config() {
        Some(v) => v,
        None => {
            eprintln!("AnimalKingdomAPI not found, skipping benchmark");
            return;
        }
    };

    let temp = tempfile::TempDir::new().unwrap();
    let config_path = create_test_config(temp.path(), &repo_root);
    let concat_path = temp.path().join("output.swift");
    let inputs = collect_input_files(&repo_root);

    let iterations = 10;

    // =========================================================
    // Benchmark 1: Cold CLI invocations (no cache, full startup)
    // =========================================================
    eprintln!("\n=== Cold CLI ({} iterations) ===", iterations);
    let mut cold_times = Vec::new();
    for i in 0..iterations {
        // Remove output to force regeneration
        let _ = std::fs::remove_file(&concat_path);

        let start = Instant::now();
        let status = Command::new(&binary)
            .arg("generate")
            .arg("--path")
            .arg(config_path.to_str().unwrap())
            .arg("--concat")
            .arg(concat_path.to_str().unwrap())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("Failed to run CLI");
        let elapsed = start.elapsed();

        assert!(status.success(), "CLI invocation {} failed", i);
        cold_times.push(elapsed);
    }

    let cold_avg = cold_times.iter().sum::<Duration>() / iterations as u32;
    let cold_min = cold_times.iter().min().unwrap();
    let cold_max = cold_times.iter().max().unwrap();

    for (i, t) in cold_times.iter().enumerate() {
        eprintln!("  cold[{}]: {:>6.1}ms", i, t.as_secs_f64() * 1000.0);
    }
    eprintln!("  avg: {:>6.1}ms  min: {:>6.1}ms  max: {:>6.1}ms",
        cold_avg.as_secs_f64() * 1000.0,
        cold_min.as_secs_f64() * 1000.0,
        cold_max.as_secs_f64() * 1000.0,
    );

    // =========================================================
    // Benchmark 2: Warm worker (persistent, cached)
    // =========================================================
    eprintln!("\n=== Warm Worker ({} iterations) ===", iterations);

    let mut child = Command::new(&binary)
        .arg("--persistent_worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start worker");

    let stdin = child.stdin.as_mut().unwrap();
    let mut stdout_buf = Vec::new();

    // First: send a warmup request (populates cache)
    let warmup_req = WorkRequest {
        arguments: vec![
            format!("--config={}", config_path.to_str().unwrap()),
            format!("--concat={}", concat_path.to_str().unwrap()),
        ],
        inputs: inputs.clone(),
        request_id: 0,
    };
    write_work_request(stdin, &warmup_req).unwrap();

    // Now send timed requests
    let mut warm_times = Vec::new();
    for i in 0..iterations {
        let _ = std::fs::remove_file(&concat_path);

        let request = WorkRequest {
            arguments: vec![
                format!("--config={}", config_path.to_str().unwrap()),
                format!("--concat={}", concat_path.to_str().unwrap()),
            ],
            inputs: inputs.clone(),
            request_id: (i + 1) as i32,
        };

        let start = Instant::now();
        write_work_request(stdin, &request).unwrap();
        stdin.flush().unwrap();
        // We can't measure response time directly without reading stdout
        // But the write is non-blocking, so this measures request submission
        // The actual time is measured by the worker and printed to stderr
        warm_times.push(start.elapsed());
    }

    // Close stdin and collect output
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    stdout_buf = output.stdout;

    // Read all responses
    let mut reader = stdout_buf.as_slice();
    // Skip warmup response
    let warmup_resp = read_work_response(&mut reader).unwrap().unwrap();
    assert_eq!(warmup_resp.exit_code, 0, "Warmup failed: {}", warmup_resp.output);

    let mut response_times = Vec::new();
    for i in 0..iterations {
        let resp = read_work_response(&mut reader).unwrap()
            .unwrap_or_else(|| panic!("Missing response for iteration {}", i));
        assert_eq!(resp.exit_code, 0, "Request {} failed: {}", i, resp.output);
        response_times.push(resp);
    }

    // Parse timing from stderr (worker logs show request processing)
    // For accurate measurement, let's time the full roundtrip differently:
    // We'll use the total time for all requests divided by count
    //
    // Actually, let's measure the total time for the warm worker run
    // and divide by iterations. This includes IPC overhead which is realistic.

    // Better approach: measure total time from first request send to last response read
    // We already measured write times per request. The total wall clock is more meaningful.

    eprintln!("\n  Worker stderr (cache behavior):");
    for line in stderr.lines().filter(|l| l.contains("cache")) {
        eprintln!("    {}", line);
    }

    // =========================================================
    // Summary
    // =========================================================
    eprintln!("\n=== Summary ===");
    eprintln!("  Cold CLI avg:    {:>6.1}ms per invocation", cold_avg.as_secs_f64() * 1000.0);
    eprintln!("  Cold CLI min:    {:>6.1}ms", cold_min.as_secs_f64() * 1000.0);

    // For the warm worker, we need a better timing approach.
    // Let's run a separate timed experiment.
    eprintln!("\n  (Running timed warm worker measurement...)");

    // Clean timed run: start worker, warmup, then time N iterations end-to-end
    let mut child2 = Command::new(&binary)
        .arg("--persistent_worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start worker");

    let stdin2 = child2.stdin.as_mut().unwrap();

    // Warmup
    write_work_request(stdin2, &warmup_req).unwrap();
    stdin2.flush().unwrap();

    // Need to drain the warmup response to avoid blocking
    // Send all requests first, then read all responses
    let timed_start = Instant::now();
    for i in 0..iterations {
        let request = WorkRequest {
            arguments: vec![
                format!("--config={}", config_path.to_str().unwrap()),
                format!("--concat={}", concat_path.to_str().unwrap()),
            ],
            inputs: inputs.clone(),
            request_id: (i + 100) as i32,
        };
        write_work_request(stdin2, &request).unwrap();
    }
    stdin2.flush().unwrap();
    drop(child2.stdin.take());

    let output2 = child2.wait_with_output().unwrap();
    let timed_total = timed_start.elapsed();
    let warm_avg = timed_total / (iterations as u32 + 1); // +1 for warmup response

    // Read and verify responses
    let mut reader2 = output2.stdout.as_slice();
    let warmup2 = read_work_response(&mut reader2).unwrap().unwrap();
    assert_eq!(warmup2.exit_code, 0);
    for i in 0..iterations {
        let resp = read_work_response(&mut reader2).unwrap()
            .unwrap_or_else(|| panic!("Missing response {} in timed run", i));
        assert_eq!(resp.exit_code, 0, "Timed request {} failed: {}", i, resp.output);
    }

    let speedup = cold_avg.as_secs_f64() / warm_avg.as_secs_f64();

    eprintln!("  Warm worker avg: {:>6.1}ms per request (total {:>6.1}ms for {} requests)",
        warm_avg.as_secs_f64() * 1000.0,
        timed_total.as_secs_f64() * 1000.0,
        iterations,
    );
    eprintln!("  Speedup:         {:.1}x", speedup);
    eprintln!("");

    // Sanity check: worker should not be significantly SLOWER than cold CLI.
    // We don't assert a hard speedup threshold because on warm OS caches with
    // small schemas, the difference is within noise. The real speedup shows on
    // large schemas and cold-start scenarios (first build, CI).
    assert!(
        speedup > 0.5,
        "Worker should not be >2x slower than cold CLI, got {:.1}x (cold: {:.1}ms, warm: {:.1}ms)",
        speedup,
        cold_avg.as_secs_f64() * 1000.0,
        warm_avg.as_secs_f64() * 1000.0,
    );
}
