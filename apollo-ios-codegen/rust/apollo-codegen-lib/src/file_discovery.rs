//! GraphQL file discovery via glob-based search path matching.
//!
//! Discovers `.graphql` and `.graphqls` files by walking directories and matching
//! against glob patterns, while excluding build artifact directories.
//!
//! Mirrors Swift's `ApolloCodegen.match(searchPaths:relativeTo:)` method from
//! `Sources/ApolloCodegenLib/ApolloCodegen.swift`.

use std::path::{Path, PathBuf};

use globset::GlobBuilder;
use indexmap::IndexSet;
use walkdir::WalkDir;

/// Directories excluded from file discovery.
///
/// Matches Swift's `ApolloCodegen.match()` excludedDirectories exactly:
/// `.build` (Swift PM), `.swiftpm`, `.Pods` (CocoaPods).
const EXCLUDED_DIRECTORIES: &[&str] = &[".build", ".swiftpm", ".Pods"];

/// Matches files against glob search paths, excluding certain directories.
/// Returns an ordered set of absolute file paths.
///
/// Mirrors Swift's `ApolloCodegen.match(searchPaths:relativeTo:)`.
///
/// # Arguments
/// * `search_paths` - Glob patterns to match against (e.g. `["**/*.graphql"]`)
/// * `relative_to` - Optional root directory for resolving relative patterns
///
/// # Returns
/// An `IndexSet<String>` of matched file paths (preserving insertion order).
pub fn match_search_paths(
    search_paths: &[String],
    relative_to: Option<&Path>,
) -> Result<IndexSet<String>, std::io::Error> {
    let mut results = IndexSet::new();

    for pattern in search_paths {
        // Resolve pattern relative to root URL
        let full_pattern = if let Some(root) = relative_to {
            if Path::new(pattern).is_absolute() {
                pattern.clone()
            } else {
                root.join(pattern).to_string_lossy().to_string()
            }
        } else {
            pattern.clone()
        };

        // Extract the base directory from the pattern (everything before first wildcard)
        let base_dir = extract_base_dir(&full_pattern);

        // Build the glob matcher
        let glob = GlobBuilder::new(&full_pattern)
            .literal_separator(false)
            .build()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?
            .compile_matcher();

        // Walk the base directory
        let base = Path::new(&base_dir);
        if !base.exists() {
            continue;
        }

        for entry in WalkDir::new(base)
            .into_iter()
            .filter_entry(|e| !is_excluded_directory(e))
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let path = entry.path();
                let path_str = path.to_string_lossy().to_string();
                if glob.is_match(&path_str) {
                    // Use canonical absolute path for consistent matching
                    let abs_path = make_absolute(path);
                    results.insert(abs_path);
                }
            }
        }
    }

    Ok(results)
}

/// Checks if a walkdir entry is an excluded directory.
fn is_excluded_directory(entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    if let Some(name) = entry.file_name().to_str() {
        EXCLUDED_DIRECTORIES.contains(&name)
    } else {
        false
    }
}

/// Makes a path absolute without resolving symlinks.
///
/// Uses the current working directory for relative paths, matching Swift's
/// behavior of not resolving symlinks (which would cause `/tmp` -> `/private/tmp`
/// mismatches on macOS).
fn make_absolute(path: &Path) -> String {
    if path.is_absolute() {
        path.to_string_lossy().to_string()
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        cwd.join(path).to_string_lossy().to_string()
    }
}

/// Extracts the base directory from a glob pattern.
///
/// Returns everything before the first wildcard character (`*`, `?`, or `[`).
/// If no wildcard is found, returns the pattern's parent directory.
fn extract_base_dir(pattern: &str) -> String {
    // Find first glob special character
    let first_special = pattern.find(|c: char| c == '*' || c == '?' || c == '[');
    let prefix = match first_special {
        Some(pos) => &pattern[..pos],
        None => pattern,
    };
    // Get the parent directory of the prefix
    match prefix.rfind('/') {
        Some(pos) => prefix[..=pos].to_string(),
        None => ".".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_base_dir_simple_glob() {
        assert_eq!(extract_base_dir("**/*.graphql"), ".");
    }

    #[test]
    fn test_extract_base_dir_with_prefix() {
        assert_eq!(extract_base_dir("src/**/*.graphql"), "src/");
    }

    #[test]
    fn test_extract_base_dir_absolute_prefix() {
        assert_eq!(
            extract_base_dir("/root/project/**/*.graphql"),
            "/root/project/"
        );
    }

    #[test]
    fn test_extract_base_dir_no_glob() {
        assert_eq!(extract_base_dir("src/schema.graphql"), "src/");
    }

    #[test]
    fn test_extract_base_dir_question_mark() {
        assert_eq!(extract_base_dir("src/?.graphql"), "src/");
    }

    #[test]
    fn test_extract_base_dir_bracket() {
        assert_eq!(extract_base_dir("src/[ab].graphql"), "src/");
    }

    #[test]
    fn test_excluded_directory_detection_build() {
        // Test the EXCLUDED_DIRECTORIES list
        assert!(EXCLUDED_DIRECTORIES.contains(&".build"));
        assert!(EXCLUDED_DIRECTORIES.contains(&".swiftpm"));
        assert!(EXCLUDED_DIRECTORIES.contains(&".Pods"));
        assert!(!EXCLUDED_DIRECTORIES.contains(&"src"));
        assert!(!EXCLUDED_DIRECTORIES.contains(&"node_modules"));
    }

    #[test]
    fn test_match_search_paths_empty_returns_empty() {
        let result = match_search_paths(&[], None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_match_search_paths_nonexistent_dir_returns_empty() {
        let result = match_search_paths(
            &["/nonexistent/path/**/*.graphql".to_string()],
            None,
        )
        .unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_match_search_paths_discovers_files() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let schema_file = dir.path().join("schema.graphql");
        let op_file = dir.path().join("operations").join("query.graphql");
        std::fs::create_dir_all(op_file.parent().unwrap()).unwrap();
        std::fs::write(&schema_file, "type Query { id: ID }").unwrap();
        std::fs::write(&op_file, "query Test { id }").unwrap();

        let pattern = format!("{}/**/*.graphql", dir.path().display());
        let result = match_search_paths(&[pattern], None).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_match_search_paths_excludes_build_dir() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let normal_file = dir.path().join("schema.graphql");
        let build_file = dir.path().join(".build").join("generated.graphql");
        let swiftpm_file = dir.path().join(".swiftpm").join("generated.graphql");
        let pods_file = dir.path().join(".Pods").join("generated.graphql");

        std::fs::write(&normal_file, "type Query { id: ID }").unwrap();
        std::fs::create_dir_all(build_file.parent().unwrap()).unwrap();
        std::fs::write(&build_file, "excluded").unwrap();
        std::fs::create_dir_all(swiftpm_file.parent().unwrap()).unwrap();
        std::fs::write(&swiftpm_file, "excluded").unwrap();
        std::fs::create_dir_all(pods_file.parent().unwrap()).unwrap();
        std::fs::write(&pods_file, "excluded").unwrap();

        let pattern = format!("{}/**/*.graphql", dir.path().display());
        let result = match_search_paths(&[pattern], None).unwrap();
        assert_eq!(
            result.len(),
            1,
            "Should find only schema.graphql, not files in excluded dirs"
        );
    }

    #[test]
    fn test_match_search_paths_relative_to_root() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let schema_file = dir.path().join("src").join("schema.graphql");
        std::fs::create_dir_all(schema_file.parent().unwrap()).unwrap();
        std::fs::write(&schema_file, "type Query { id: ID }").unwrap();

        let result = match_search_paths(
            &["src/**/*.graphql".to_string()],
            Some(dir.path()),
        )
        .unwrap();
        assert_eq!(result.len(), 1);
    }
}
