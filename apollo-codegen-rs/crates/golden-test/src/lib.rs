//! Golden file comparison test harness.
//!
//! Compares Rust codegen output against the known-good Swift codegen output
//! captured from the 1.15.1 release. All generated files must match byte-for-byte.

use similar::{ChangeTag, TextDiff};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Result of comparing a single file.
#[derive(Debug)]
pub enum FileComparisonResult {
    /// Files are identical.
    Match,
    /// Files differ.
    Mismatch {
        expected_path: PathBuf,
        actual_path: PathBuf,
        diff_summary: String,
    },
    /// Expected file exists but no actual file was produced.
    Missing { expected_path: PathBuf },
    /// Actual file was produced but no expected file exists.
    Extra { actual_path: PathBuf },
}

/// Result of comparing an entire API's generated output.
#[derive(Debug)]
pub struct ApiComparisonResult {
    pub api_name: String,
    pub total_expected: usize,
    pub total_actual: usize,
    pub matches: usize,
    pub mismatches: Vec<FileComparisonResult>,
    pub missing: Vec<FileComparisonResult>,
    pub extra: Vec<FileComparisonResult>,
}

impl ApiComparisonResult {
    pub fn is_perfect_match(&self) -> bool {
        self.mismatches.is_empty() && self.missing.is_empty() && self.extra.is_empty()
    }

    pub fn summary(&self) -> String {
        format!(
            "{}: {}/{} match, {} mismatch, {} missing, {} extra",
            self.api_name,
            self.matches,
            self.total_expected,
            self.mismatches.len(),
            self.missing.len(),
            self.extra.len(),
        )
    }
}

/// Directory containing the golden fixture files.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// Load all golden files for a given API from the fixtures directory.
pub fn load_golden_files(api_name: &str) -> BTreeMap<String, String> {
    let dir = fixtures_dir().join(api_name);
    load_swift_files_from_dir(&dir)
}

/// Load all .swift files from a directory tree, returning a map of
/// relative path -> file contents.
pub fn load_swift_files_from_dir(dir: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();

    if !dir.exists() {
        return files;
    }

    for entry in walkdir::WalkDir::new(dir)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "swift") {
            let relative = path
                .strip_prefix(dir)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if let Ok(contents) = std::fs::read_to_string(path) {
                files.insert(relative, contents);
            }
        }
    }

    files
}

/// Compare actual generated output against golden files for an API.
pub fn compare_api(
    api_name: &str,
    actual_files: &BTreeMap<String, String>,
) -> ApiComparisonResult {
    let expected_files = load_golden_files(api_name);

    let mut matches = 0;
    let mut mismatches = Vec::new();
    let mut missing = Vec::new();
    let mut extra = Vec::new();

    for (rel_path, expected_content) in &expected_files {
        match actual_files.get(rel_path) {
            Some(actual_content) => {
                if expected_content == actual_content {
                    matches += 1;
                } else {
                    let diff = TextDiff::from_lines(expected_content, actual_content);
                    let mut diff_lines = Vec::new();
                    let mut shown = 0;

                    for change in diff.iter_all_changes() {
                        if shown >= 20 {
                            diff_lines.push("... (truncated)\n".to_string());
                            break;
                        }
                        let sign = match change.tag() {
                            ChangeTag::Delete => "-",
                            ChangeTag::Insert => "+",
                            ChangeTag::Equal => {
                                if shown > 0 {
                                    " "
                                } else {
                                    continue;
                                }
                            }
                        };
                        diff_lines.push(format!("{}{}", sign, change));
                        if change.tag() != ChangeTag::Equal {
                            shown += 1;
                        }
                    }

                    mismatches.push(FileComparisonResult::Mismatch {
                        expected_path: PathBuf::from(rel_path),
                        actual_path: PathBuf::from(rel_path),
                        diff_summary: diff_lines.join(""),
                    });
                }
            }
            None => {
                missing.push(FileComparisonResult::Missing {
                    expected_path: PathBuf::from(rel_path),
                });
            }
        }
    }

    for rel_path in actual_files.keys() {
        if !expected_files.contains_key(rel_path) {
            extra.push(FileComparisonResult::Extra {
                actual_path: PathBuf::from(rel_path),
            });
        }
    }

    ApiComparisonResult {
        api_name: api_name.to_string(),
        total_expected: expected_files.len(),
        total_actual: actual_files.len(),
        matches,
        mismatches,
        missing,
        extra,
    }
}

/// Compare with an empty set of actual files.
pub fn compare_api_empty(api_name: &str) -> ApiComparisonResult {
    compare_api(api_name, &BTreeMap::new())
}

/// Names of all test APIs.
pub const TEST_APIS: &[&str] = &[
    "AnimalKingdomAPI",
    "StarWarsAPI",
    "GitHubAPI",
    "UploadAPI",
    "SubscriptionAPI",
];
