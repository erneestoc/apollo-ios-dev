//! Content-addressed caching for Apollo iOS code generation.
//!
//! Cache key = SHA256(schema files + operation files + config + version)
//! Cache value = compressed tar archive of all generated files.

// TODO: Phase 5 - Caching layer
