//! Worker cache - holds compiled artifacts in memory across invocations.
//!
//! Cache invalidation is based on content digests from Bazel's WorkRequest.inputs.
//! Schema changes invalidate everything. Operation changes only invalidate compilation.

use crate::protocol::Input;
use apollo_codegen_pipeline::CompiledArtifacts;
use apollo_codegen_frontend::compiler::GraphQLFrontend;
use sha2::{Digest, Sha256};

/// In-memory cache for the persistent worker.
pub struct WorkerCache {
    /// Cached schema frontend (parsed + validated schema).
    schema: Option<CachedSchema>,
    /// Cached compilation artifacts (compilation result + IR + type_kinds).
    compilation: Option<CachedCompilation>,
}

struct CachedSchema {
    /// Cache key: SHA256 of sorted (path, digest) pairs for schema files.
    key: String,
    /// The compiled GraphQL frontend holding the validated schema.
    frontend: GraphQLFrontend,
}

struct CachedCompilation {
    /// Cache key: schema_key + SHA256 of sorted (path, digest) for all files + options.
    key: String,
    /// The compiled artifacts (CompilationResult + IRBuilder + type_kinds).
    artifacts: CompiledArtifacts,
}

impl WorkerCache {
    pub fn new() -> Self {
        Self {
            schema: None,
            compilation: None,
        }
    }

    /// Check if the cached schema matches the given inputs.
    pub fn schema_matches(&self, schema_key: &str) -> bool {
        self.schema.as_ref().map_or(false, |s| s.key == schema_key)
    }

    /// Get a reference to the cached schema frontend.
    pub fn get_schema(&self) -> Option<&GraphQLFrontend> {
        self.schema.as_ref().map(|s| &s.frontend)
    }

    /// Store a new schema in the cache. Invalidates compilation.
    pub fn set_schema(&mut self, key: String, frontend: GraphQLFrontend) {
        self.schema = Some(CachedSchema { key, frontend });
        self.compilation = None; // Schema changed -> compilation invalid
    }

    /// Check if the cached compilation matches the given key.
    pub fn compilation_matches(&self, compilation_key: &str) -> bool {
        self.compilation
            .as_ref()
            .map_or(false, |c| c.key == compilation_key)
    }

    /// Get a mutable reference to the cached compilation artifacts.
    pub fn get_compilation_mut(&mut self) -> Option<&mut CompiledArtifacts> {
        self.compilation.as_mut().map(|c| &mut c.artifacts)
    }

    /// Store a new compilation in the cache.
    pub fn set_compilation(&mut self, key: String, artifacts: CompiledArtifacts) {
        self.compilation = Some(CachedCompilation { key, artifacts });
    }

    /// Report cache stats for diagnostics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            has_schema: self.schema.is_some(),
            has_compilation: self.compilation.is_some(),
        }
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub has_schema: bool,
    pub has_compilation: bool,
}

// ==========================================================================
// Cache key computation
// ==========================================================================

/// Compute a cache key from a set of Bazel inputs.
///
/// Sorts inputs by path, then SHA256-hashes the concatenated `path:hex(digest)` pairs.
/// This ensures the key is deterministic regardless of input order.
pub fn compute_inputs_key(inputs: &[Input]) -> String {
    let mut sorted: Vec<_> = inputs.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    let mut hasher = Sha256::new();
    for input in sorted {
        hasher.update(input.path.as_bytes());
        hasher.update(b":");
        hasher.update(&input.digest);
        hasher.update(b"\n");
    }
    hex_encode(&hasher.finalize())
}

/// Compute a compilation cache key from schema key + all inputs + options hash.
pub fn compute_compilation_key(
    schema_key: &str,
    all_inputs: &[Input],
    options_hash: &str,
) -> String {
    let inputs_key = compute_inputs_key(all_inputs);
    let mut hasher = Sha256::new();
    hasher.update(schema_key.as_bytes());
    hasher.update(b"\n");
    hasher.update(inputs_key.as_bytes());
    hasher.update(b"\n");
    hasher.update(options_hash.as_bytes());
    hex_encode(&hasher.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Classify inputs into schema files and operation files based on extension.
///
/// Schema files: `.graphqls`, `.json` (SDL or introspection)
/// Operation files: `.graphql`
pub fn classify_inputs(inputs: &[Input]) -> (Vec<Input>, Vec<Input>) {
    let mut schema = Vec::new();
    let mut operations = Vec::new();

    for input in inputs {
        if input.path.ends_with(".graphqls")
            || input.path.ends_with(".json")
            || input.path.ends_with(".sdl")
        {
            schema.push(input.clone());
        } else if input.path.ends_with(".graphql") {
            operations.push(input.clone());
        }
        // Ignore other files
    }

    (schema, operations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_inputs_key_deterministic() {
        let inputs_a = vec![
            Input { path: "b.graphql".into(), digest: vec![2] },
            Input { path: "a.graphql".into(), digest: vec![1] },
        ];
        let inputs_b = vec![
            Input { path: "a.graphql".into(), digest: vec![1] },
            Input { path: "b.graphql".into(), digest: vec![2] },
        ];
        assert_eq!(
            compute_inputs_key(&inputs_a),
            compute_inputs_key(&inputs_b),
        );
    }

    #[test]
    fn test_compute_inputs_key_changes_on_content() {
        let inputs_a = vec![Input { path: "a.graphql".into(), digest: vec![1] }];
        let inputs_b = vec![Input { path: "a.graphql".into(), digest: vec![2] }];
        assert_ne!(
            compute_inputs_key(&inputs_a),
            compute_inputs_key(&inputs_b),
        );
    }

    #[test]
    fn test_classify_inputs() {
        let inputs = vec![
            Input { path: "schema.graphqls".into(), digest: vec![] },
            Input { path: "ops/Query.graphql".into(), digest: vec![] },
            Input { path: "introspection.json".into(), digest: vec![] },
            Input { path: "Fragment.graphql".into(), digest: vec![] },
            Input { path: "readme.md".into(), digest: vec![] },
        ];
        let (schema, ops) = classify_inputs(&inputs);
        assert_eq!(schema.len(), 2); // .graphqls + .json
        assert_eq!(ops.len(), 2); // two .graphql files
    }
}
