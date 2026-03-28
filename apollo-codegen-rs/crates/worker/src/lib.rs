//! Bazel persistent worker for Apollo iOS code generation.
//!
//! Implements the Bazel persistent worker protocol (protobuf over stdin/stdout)
//! with in-memory caching of compiled schema and IR for near-zero latency
//! on operation-only rebuilds.
//!
//! Usage: the CLI binary detects `--persistent_worker` and delegates here.
//!
//! Architecture:
//! - Worker reads WorkRequest protobuf messages from stdin
//! - Each request carries arguments + input files with content digests
//! - Cache checks schema digest → compilation digest → renders output
//! - Worker writes WorkResponse to stdout
//! - All diagnostic output goes to stderr (stdout reserved for protocol)

pub mod protocol;
pub mod cache;
pub mod handler;

use cache::WorkerCache;
use protocol::{read_work_request, write_work_response};
use std::io::{self, BufReader, BufWriter};

/// Run the persistent worker loop.
///
/// Reads WorkRequest messages from stdin, processes them, and writes
/// WorkResponse messages to stdout. Loops until stdin is closed (Bazel
/// terminates the worker).
///
/// All diagnostic/error output goes to stderr. stdout is exclusively
/// for the protobuf protocol.
pub fn run_worker_loop() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());
    let mut cache = WorkerCache::new();

    eprintln!("[worker] Apollo iOS codegen persistent worker started");

    loop {
        // Read next request (blocks until available)
        let request = match read_work_request(&mut reader)? {
            Some(req) => req,
            None => {
                eprintln!("[worker] stdin closed, shutting down");
                break;
            }
        };

        eprintln!(
            "[worker] Request #{}: {} args, {} inputs",
            request.request_id,
            request.arguments.len(),
            request.inputs.len(),
        );

        // Process request (never panics - errors become non-zero exit codes)
        let response = handler::handle_request(&request, &mut cache);

        if response.exit_code != 0 {
            eprintln!("[worker] Request #{} failed: {}", request.request_id, response.output);
        } else {
            eprintln!("[worker] Request #{} succeeded: {}", request.request_id, response.output);
        }

        // Write response
        write_work_response(&mut writer, &response)?;
    }

    let stats = cache.stats();
    eprintln!(
        "[worker] Shutting down. Cache state: schema={}, compilation={}",
        stats.has_schema, stats.has_compilation,
    );

    Ok(())
}
