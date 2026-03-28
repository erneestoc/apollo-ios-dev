//! Integration tests for the Bazel persistent worker.
//!
//! These tests pipe real protobuf WorkRequests through the worker binary
//! and verify the WorkResponses.

use apollo_codegen_worker::protocol::*;
use std::io::Write;
use std::process::{Command, Stdio};

fn worker_binary() -> std::path::PathBuf {
    // Try debug first (cargo test builds in debug), then release
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..");
    let debug = workspace_root.join("target/debug/apollo-ios-cli-rs");
    if debug.exists() {
        return debug;
    }
    let release = workspace_root.join("target/release/apollo-ios-cli-rs");
    if release.exists() {
        return release;
    }
    // Fallback: try from current_exe
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // deps
    path.pop(); // debug
    path.push("apollo-ios-cli-rs");
    path
}

/// Test: worker starts, receives empty-args request, returns error (no --config), shuts down.
#[test]
fn test_worker_handles_bad_request_without_crashing() {
    let binary = worker_binary();
    if !binary.exists() {
        eprintln!("Skipping integration test: binary not found at {:?}", binary);
        return;
    }

    let mut child = Command::new(&binary)
        .arg("--persistent_worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start worker");

    let stdin = child.stdin.as_mut().unwrap();

    // Send a request with no --config (should fail gracefully)
    let request = WorkRequest {
        arguments: vec!["--mode=all".to_string()],
        inputs: vec![],
        request_id: 1,
    };
    write_work_request(stdin, &request).unwrap();

    // Close stdin to signal shutdown
    drop(child.stdin.take());

    // Read response from stdout
    let stdout = child.wait_with_output().unwrap();
    let mut reader = stdout.stdout.as_slice();

    let response = read_work_response(&mut reader).unwrap().unwrap();
    assert_eq!(response.request_id, 1, "response should echo request_id");
    assert_ne!(response.exit_code, 0, "should fail without --config");
    assert!(
        response.output.contains("config"),
        "error should mention config: {}",
        response.output,
    );
}

/// Test: worker processes multiple sequential requests (singleplex mode).
#[test]
fn test_worker_sequential_requests() {
    let binary = worker_binary();
    if !binary.exists() {
        eprintln!("Skipping integration test: binary not found at {:?}", binary);
        return;
    }

    let mut child = Command::new(&binary)
        .arg("--persistent_worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start worker");

    let stdin = child.stdin.as_mut().unwrap();

    // Send 3 requests (all will fail since no real config, but worker should handle all 3)
    for i in 1..=3 {
        let request = WorkRequest {
            arguments: vec!["--mode=all".to_string()],
            inputs: vec![],
            request_id: i,
        };
        write_work_request(stdin, &request).unwrap();
    }

    // Close stdin
    drop(child.stdin.take());

    let output = child.wait_with_output().unwrap();
    let mut reader = output.stdout.as_slice();

    // Should get 3 responses, one per request
    for i in 1..=3 {
        let response = read_work_response(&mut reader)
            .unwrap_or_else(|e| panic!("Failed to read response {}: {}", i, e))
            .unwrap_or_else(|| panic!("Unexpected EOF reading response {}", i));
        assert_eq!(response.request_id, i, "response {} should have correct request_id", i);
        assert_ne!(response.exit_code, 0, "all requests should fail (no config)");
    }

    // No more responses
    let eof = read_work_response(&mut reader).unwrap();
    assert!(eof.is_none(), "should be EOF after all responses");
}

/// Test: worker processes a real codegen request with a test schema.
#[test]
fn test_worker_real_codegen() {
    let binary = worker_binary();
    if !binary.exists() {
        eprintln!("Skipping integration test: binary not found at {:?}", binary);
        return;
    }

    // Create a temp directory with a schema and operation
    let temp = tempfile::TempDir::new().unwrap();
    let schema_path = temp.path().join("schema.graphqls");
    let op_path = temp.path().join("Query.graphql");
    let output_path = temp.path().join("output.swift");

    std::fs::write(&schema_path, "type Query { hello: String! }").unwrap();
    std::fs::write(&op_path, "query HelloQuery { hello }").unwrap();

    // Create a config file
    let config_path = temp.path().join("config.json");
    let config_json = serde_json::json!({
        "schemaNamespace": "TestAPI",
        "input": {
            "schemaSearchPaths": [schema_path.to_str().unwrap()],
            "operationSearchPaths": [op_path.to_str().unwrap()]
        },
        "output": {
            "testMocks": { "none": {} },
            "schemaTypes": {
                "path": temp.path().join("Generated").to_str().unwrap(),
                "moduleType": { "swiftPackageManager": {} }
            },
            "operations": { "inSchemaModule": {} }
        }
    });
    std::fs::write(&config_path, config_json.to_string()).unwrap();

    let mut child = Command::new(&binary)
        .arg("--persistent_worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start worker");

    let stdin = child.stdin.as_mut().unwrap();

    // Send a real codegen request
    let request = WorkRequest {
        arguments: vec![
            format!("--config={}", config_path.to_str().unwrap()),
            format!("--concat={}", output_path.to_str().unwrap()),
        ],
        inputs: vec![
            Input {
                path: schema_path.to_str().unwrap().to_string(),
                digest: vec![0x01], // dummy digest
            },
            Input {
                path: op_path.to_str().unwrap().to_string(),
                digest: vec![0x02],
            },
        ],
        request_id: 1,
    };
    write_work_request(stdin, &request).unwrap();

    // Send second request (same inputs — should hit cache)
    let request2 = WorkRequest {
        arguments: request.arguments.clone(),
        inputs: request.inputs.clone(),
        request_id: 2,
    };
    write_work_request(stdin, &request2).unwrap();

    // Send third request with different digest (cache miss)
    let request3 = WorkRequest {
        arguments: request.arguments.clone(),
        inputs: vec![
            Input {
                path: schema_path.to_str().unwrap().to_string(),
                digest: vec![0x01],
            },
            Input {
                path: op_path.to_str().unwrap().to_string(),
                digest: vec![0xFF], // different digest
            },
        ],
        request_id: 3,
    };
    write_work_request(stdin, &request3).unwrap();

    // Close stdin
    drop(child.stdin.take());

    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut reader = output.stdout.as_slice();

    // Response 1: should succeed
    let resp1 = read_work_response(&mut reader).unwrap().unwrap();
    assert_eq!(resp1.request_id, 1);
    assert_eq!(resp1.exit_code, 0, "First request should succeed: {}", resp1.output);

    // Response 2: should also succeed (cache hit)
    let resp2 = read_work_response(&mut reader).unwrap().unwrap();
    assert_eq!(resp2.request_id, 2);
    assert_eq!(resp2.exit_code, 0, "Second request should succeed (cache hit): {}", resp2.output);

    // Response 3: should succeed (cache miss for compilation, schema hit)
    let resp3 = read_work_response(&mut reader).unwrap().unwrap();
    assert_eq!(resp3.request_id, 3);
    assert_eq!(resp3.exit_code, 0, "Third request should succeed: {}", resp3.output);

    // Verify output file was written
    assert!(output_path.exists(), "Output file should exist");
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("HelloQuery"), "Output should contain HelloQuery: {}", &content[..content.len().min(200)]);

    // Verify cache behavior from stderr
    assert!(stderr.contains("Schema cache miss"), "First request should miss schema cache");
    assert!(stderr.contains("Schema cache hit"), "Second request should hit schema cache");
    assert!(stderr.contains("Compilation cache hit"), "Second request should hit compilation cache");

    eprintln!("Worker stderr:\n{}", stderr);
}
