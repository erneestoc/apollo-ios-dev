//! File discovery via glob patterns for Apollo iOS code generation.
//!
//! Replicates the behavior of the Swift `Glob` type, supporting:
//! - `**` globstar expansion (recursive directory matching)
//! - `!` prefix for exclude patterns
//! - Directory exclusion (`.build`, `.swiftpm`, `.Pods`)
//! - Symlink resolution
//! - Deterministic ordering via BTreeSet

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

/// Default directories to exclude from globstar expansion.
pub const DEFAULT_EXCLUDED_DIRECTORIES: &[&str] = &[".build", ".swiftpm", ".Pods"];

/// Errors that can occur during glob matching.
#[derive(Debug, thiserror::Error)]
pub enum GlobError {
    #[error("glob pattern error: {0}")]
    Pattern(#[from] glob::PatternError),
    #[error("glob error: {0}")]
    Glob(#[from] glob::GlobError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cannot enumerate path: {0}")]
    CannotEnumerate(String),
    #[error("exclude paths must start with '!' - {0}")]
    InvalidExclude(String),
}

/// A path pattern matcher that replicates Swift's `Glob` type.
pub struct Glob {
    patterns: Vec<String>,
    root_url: Option<PathBuf>,
}

impl Glob {
    pub fn new(patterns: Vec<String>, root_url: Option<PathBuf>) -> Self {
        Self { patterns, root_url }
    }

    /// Execute the pattern match on the filesystem.
    ///
    /// Returns a deterministically ordered set of matched file paths.
    pub fn match_files(
        &self,
        excluding_directories: Option<&[&str]>,
    ) -> Result<BTreeSet<String>, GlobError> {
        let expanded = self.expand_all(&self.patterns, excluding_directories)?;

        let mut include_matches: Vec<String> = Vec::new();
        let mut exclude_matches: Vec<String> = Vec::new();

        for pattern in &expanded {
            if pattern.starts_with('!') {
                let pat = &pattern[1..];
                let matches = self.glob_matches(pat)?;
                exclude_matches.extend(matches);
            } else {
                let matches = self.glob_matches(pattern)?;
                include_matches.extend(matches);
            }
        }

        // Resolve symlinks in included paths
        let include_matches: Vec<String> = include_matches
            .into_iter()
            .filter_map(|path| {
                let p = PathBuf::from(&path);
                match p.canonicalize() {
                    Ok(resolved) => Some(resolved.to_string_lossy().to_string()),
                    Err(_) => Some(path),
                }
            })
            .collect();

        // Also resolve symlinks in exclude paths for consistent comparison
        let exclude_set: HashSet<String> = exclude_matches
            .into_iter()
            .map(|path| {
                let p = PathBuf::from(&path);
                match p.canonicalize() {
                    Ok(resolved) => resolved.to_string_lossy().to_string(),
                    Err(_) => path,
                }
            })
            .collect();

        let result: BTreeSet<String> = include_matches
            .into_iter()
            .filter(|p| !exclude_set.contains(p))
            .collect();

        Ok(result)
    }

    /// Expand all patterns, handling globstar expansion.
    fn expand_all(
        &self,
        patterns: &[String],
        excluding_directories: Option<&[&str]>,
    ) -> Result<Vec<String>, GlobError> {
        let mut result = Vec::new();
        // Use a set to deduplicate (like OrderedSet in Swift)
        let mut seen = HashSet::new();

        for pattern in patterns {
            // Validate: ! must only appear at the start
            if pattern.contains('!') && !pattern.starts_with('!') {
                return Err(GlobError::InvalidExclude(pattern.clone()));
            }

            for expanded in self.expand_pattern(pattern, excluding_directories)? {
                if seen.insert(expanded.clone()) {
                    result.push(expanded);
                }
            }
        }

        Ok(result)
    }

    /// Expand a single pattern, handling globstar (`**`) expansion.
    fn expand_pattern(
        &self,
        pattern: &str,
        excluding_directories: Option<&[&str]>,
    ) -> Result<Vec<String>, GlobError> {
        if !pattern.contains("**") {
            // No globstar - resolve relative to root and return
            let resolved = self.resolve_path(pattern);
            return Ok(vec![resolved]);
        }

        let is_exclude = pattern.starts_with('!');
        let pattern_without_exclude = if is_exclude { &pattern[1..] } else { pattern };

        // Split on "**" - first part is the search root, rest is the suffix
        let parts: Vec<&str> = pattern_without_exclude.splitn(2, "**").collect();
        let first_part = parts[0];
        let last_part = if parts.len() > 1 { parts[1] } else { "" };

        // Determine the search root directory
        let search_dir = if first_part.is_empty() || first_part == "./" {
            self.root_url
                .clone()
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
        } else {
            self.resolve_path_buf(first_part)
        };

        // Collect all directories (including the root itself)
        let mut directories = vec![search_dir.clone()];

        let excluded_set: HashSet<&str> = excluding_directories
            .unwrap_or(&[])
            .iter()
            .copied()
            .collect();

        if search_dir.exists() {
            for entry in walkdir::WalkDir::new(&search_dir)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if !entry.file_type().is_dir() {
                    continue;
                }

                let path = entry.path();

                // Check if any path component is in the excluded set
                let should_exclude = path.components().any(|c| {
                    if let std::path::Component::Normal(name) = c {
                        excluded_set.contains(name.to_str().unwrap_or(""))
                    } else {
                        false
                    }
                });

                if should_exclude {
                    continue;
                }

                if path != search_dir {
                    directories.push(path.to_path_buf());
                }
            }
        }

        // Build expanded patterns from all directories
        let expanded: Vec<String> = directories
            .into_iter()
            .map(|dir| {
                let path = dir.join(last_part.trim_start_matches('/'));
                // Standardize the path
                let path_str = path.to_string_lossy().to_string();
                // Clean up double slashes
                let cleaned = path_str.replace("//", "/");
                if is_exclude {
                    format!("!{}", cleaned)
                } else {
                    cleaned
                }
            })
            .collect();

        Ok(expanded)
    }

    /// Resolve a pattern path relative to root_url.
    fn resolve_path(&self, pattern: &str) -> String {
        if pattern.starts_with('!') {
            let inner = &pattern[1..];
            let resolved = self.resolve_path_buf(inner);
            format!("!{}", resolved.to_string_lossy())
        } else {
            self.resolve_path_buf(pattern).to_string_lossy().to_string()
        }
    }

    /// Resolve a path relative to root_url, returning a PathBuf.
    fn resolve_path_buf(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else if let Some(root) = &self.root_url {
            root.join(path)
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        }
    }

    /// Perform actual glob matching for a pattern.
    fn glob_matches(&self, pattern: &str) -> Result<Vec<String>, GlobError> {
        let options = glob::MatchOptions {
            case_sensitive: true,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        };

        let mut results = Vec::new();
        for entry in glob::glob_with(pattern, options)? {
            let path = entry?;
            // Only include files, not directories (matching Swift behavior which uses GLOB_MARK)
            if path.is_file() {
                results.push(path.to_string_lossy().to_string());
            }
        }

        Ok(results)
    }
}

/// Convenience function matching the Swift `ApolloCodegen.match(searchPaths:relativeTo:)` method.
pub fn match_search_paths(
    search_paths: &[String],
    relative_to: Option<&Path>,
) -> Result<BTreeSet<String>, GlobError> {
    let glob = Glob::new(
        search_paths.to_vec(),
        relative_to.map(|p| p.to_path_buf()),
    );
    glob.match_files(Some(DEFAULT_EXCLUDED_DIRECTORIES))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_simple_glob() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.graphql"), "query A { a }").unwrap();
        fs::write(dir.path().join("b.graphql"), "query B { b }").unwrap();
        fs::write(dir.path().join("c.txt"), "not graphql").unwrap();

        let glob = Glob::new(
            vec!["*.graphql".to_string()],
            Some(dir.path().to_path_buf()),
        );
        let matches = glob.match_files(None).unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_globstar_expansion() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(dir.path().join("a.graphql"), "query A { a }").unwrap();
        fs::write(sub.join("b.graphql"), "query B { b }").unwrap();

        let glob = Glob::new(
            vec!["**/*.graphql".to_string()],
            Some(dir.path().to_path_buf()),
        );
        let matches = glob.match_files(None).unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_exclude_pattern() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.graphql"), "query A { a }").unwrap();
        fs::write(dir.path().join("b.graphql"), "query B { b }").unwrap();

        let glob = Glob::new(
            vec![
                "*.graphql".to_string(),
                "!b.graphql".to_string(),
            ],
            Some(dir.path().to_path_buf()),
        );
        let matches = glob.match_files(None).unwrap();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_exclude_directory() {
        let dir = tempfile::tempdir().unwrap();
        let build_dir = dir.path().join(".build").join("sub");
        fs::create_dir_all(&build_dir).unwrap();
        fs::write(dir.path().join("a.graphql"), "query A { a }").unwrap();
        fs::write(build_dir.join("b.graphql"), "query B { b }").unwrap();

        let glob = Glob::new(
            vec!["**/*.graphql".to_string()],
            Some(dir.path().to_path_buf()),
        );
        let matches = glob
            .match_files(Some(&[".build"]))
            .unwrap();
        assert_eq!(matches.len(), 1);
    }
}
