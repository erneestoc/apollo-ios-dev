//! Generate command implementation.
//!
//! Mirrors Swift's `Sources/CodegenCLI/Commands/Generate.swift`.
//! Loads configuration, determines items to generate, and calls
//! `ApolloCodegen::build()` via the `CodegenProvider` trait.
//!
//! When `--bazel-output-dir` is set, post-generates into a Bazel tree
//! artifact directory with optional import stripping and SchemaMetadata
//! optimization. This enables the Rust CLI to serve as a direct Bazel
//! persistent worker.

use std::path::Path;

use clap::Args;
use regex::Regex;

use apollo_codegen_lib::codegen::{ApolloCodegen, CodegenProvider, ItemsToGenerate};
use apollo_codegen_lib::codegen_logger::CodegenLogger;

use crate::error::CliError;
use crate::input_options::{self, InputOptions};

/// Generate Swift source code based on a code generation configuration.
#[derive(Args, Debug)]
pub struct Generate {
    #[command(flatten)]
    pub inputs: InputOptions,

    /// Fetch the GraphQL schema before Swift code generation.
    #[arg(short, long)]
    pub fetch_schema: bool,

    /// Bazel tree artifact output directory. When set, copies relevant
    /// generated files into this directory after codegen completes.
    #[arg(long)]
    pub bazel_output_dir: Option<String>,

    /// Bazel output mode: "schema_types" copies schema type files,
    /// "operations" copies operation files for a framework.
    #[arg(long, default_value = "schema_types")]
    pub bazel_mode: String,

    /// Framework path for operations mode (e.g. "Features/Account").
    #[arg(long)]
    pub bazel_framework_path: Option<String>,

    /// Strip this module's import from generated files (e.g. "V4").
    #[arg(long)]
    pub bazel_strip_import: Option<String>,

    /// Add fast dictionary lookup to SchemaMetadata objectType function.
    #[arg(long)]
    pub bazel_optimize_schema_metadata: bool,
}

impl Generate {
    pub fn run(&self) -> Result<(), CliError> {
        CodegenLogger::set_level(self.inputs.verbose);

        // D-70: --fetch-schema is accepted but errors if used (stub)
        if self.fetch_schema {
            return Err(CliError::Generic {
                description: "Schema downloading is not yet supported in the Rust CLI. \
                              Use the Swift CLI for now."
                    .to_string(),
            });
        }

        let configuration = self.inputs.get_codegen_configuration()?;

        let mut items_to_generate = ItemsToGenerate::CODE;

        if let Some(ref manifest) = configuration.operation_manifest {
            if manifest.generate_manifest_on_code_generation {
                items_to_generate |= ItemsToGenerate::OPERATION_MANIFEST;
            }
        }

        let root_url = input_options::root_output_url(&self.inputs);

        ApolloCodegen::build(&configuration, root_url.as_deref(), items_to_generate)?;

        // Bazel tree artifact post-processing
        if let Some(ref output_dir) = self.bazel_output_dir {
            self.populate_bazel_tree_artifact(output_dir)?;
        }

        Ok(())
    }

    /// Copies generated files into a Bazel tree artifact directory,
    /// applying post-processing (import stripping, SchemaMetadata optimization).
    pub fn populate_bazel_tree_artifact(&self, output_dir: &str) -> Result<(), CliError> {
        let out = Path::new(output_dir);
        std::fs::create_dir_all(out).map_err(|e| CliError::Generic {
            description: format!("Failed to create output dir {}: {}", output_dir, e),
        })?;

        match self.bazel_mode.as_str() {
            "schema_types" => self.copy_schema_types(out),
            "operations" => self.copy_operations(out),
            other => Err(CliError::Generic {
                description: format!("Unknown --bazel-mode: {}", other),
            }),
        }
    }

    /// Copies schema type files (Objects, Enums, Unions, etc.) to the tree artifact.
    /// Excludes hand-written CustomScalars/ and SchemaConfiguration.swift.
    fn copy_schema_types(&self, out: &Path) -> Result<(), CliError> {
        let schema_dir = Path::new("V4/ApolloGenerated");
        if !schema_dir.exists() {
            return Err(CliError::Generic {
                description: format!("Schema types directory not found: {}", schema_dir.display()),
            });
        }

        let mut count = 0u32;
        copy_swift_files_recursive(schema_dir, out, &mut count, |rel_path| {
            // Exclude hand-written files
            !rel_path.starts_with("CustomScalars/")
                && rel_path != "SchemaConfiguration.swift"
        })?;

        // Post-process SchemaMetadata
        if self.bazel_optimize_schema_metadata {
            let metadata_file = out.join("SchemaMetadata.graphql.swift");
            if metadata_file.exists() {
                optimize_schema_metadata(&metadata_file)?;
            }
        }

        eprintln!("Bazel schema_types: {} files -> {}", count, out.display());
        Ok(())
    }

    /// Copies operation files for a specific framework to the tree artifact.
    fn copy_operations(&self, out: &Path) -> Result<(), CliError> {
        let framework_path = self.bazel_framework_path.as_deref().ok_or_else(|| {
            CliError::Generic {
                description: "--bazel-framework-path is required for operations mode".to_string(),
            }
        })?;

        let fw = Path::new(framework_path);
        if !fw.exists() {
            return Err(CliError::Generic {
                description: format!("Framework path not found: {}", fw.display()),
            });
        }

        // Find all ApolloGenerated directories under the framework path
        let mut count = 0u32;
        collect_apollo_generated_files(fw, out, &mut count)?;

        // Strip import if requested
        if let Some(ref module) = self.bazel_strip_import {
            strip_import_from_dir(out, module)?;
        }

        eprintln!(
            "Bazel operations ({}): {} files -> {}",
            framework_path,
            count,
            out.display()
        );
        Ok(())
    }
}

/// Recursively copies .graphql.swift files from `src` to `dest`, preserving
/// subdirectory structure. The `filter` closure receives the relative path
/// and returns false to skip a file.
fn copy_swift_files_recursive(
    src: &Path,
    dest: &Path,
    count: &mut u32,
    filter: impl Fn(&str) -> bool,
) -> Result<(), CliError> {
    walk_dir_recursive(src, &mut |entry_path| {
        let ext = entry_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        if !ext.ends_with(".graphql.swift") {
            return Ok(());
        }
        let rel = entry_path
            .strip_prefix(src)
            .unwrap_or(entry_path)
            .to_string_lossy();
        if !filter(&rel) {
            return Ok(());
        }
        let dest_path = dest.join(&*rel);
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CliError::Generic {
                description: format!("mkdir {}: {}", parent.display(), e),
            })?;
        }
        std::fs::copy(entry_path, &dest_path).map_err(|e| CliError::Generic {
            description: format!(
                "copy {} -> {}: {}",
                entry_path.display(),
                dest_path.display(),
                e
            ),
        })?;
        *count += 1;
        Ok(())
    })
}

/// Finds all ApolloGenerated/ directories under `root` and copies their
/// .graphql.swift files (flattened) to `dest`.
///
/// When `<root>/ApolloGenerated/` contains schema type subdirectories
/// (Objects/, Enums/, etc.) from non-Bazel codegen runs, those are skipped
/// to avoid filename collisions with the schema_types tree artifact.
/// In Bazel operations mode, schema types go to `_schema_types_unused/`,
/// so only the operation files should be copied.
fn collect_apollo_generated_files(
    root: &Path,
    dest: &Path,
    count: &mut u32,
) -> Result<(), CliError> {
    // Detect if <root>/ApolloGenerated/ contains schema type files from
    // non-Bazel codegen runs. If it has Objects/ or Enums/ subdirs, it's
    // a schema types directory and should be skipped entirely.
    let root_apollo_gen = root.join("ApolloGenerated");
    let has_schema_types = root_apollo_gen.join("Objects").is_dir()
        || root_apollo_gen.join("Enums").is_dir();

    walk_dir_recursive(root, &mut |entry_path| {
        // Skip files under <root>/ApolloGenerated/ if it contains schema types
        if has_schema_types && entry_path.starts_with(&root_apollo_gen) {
            return Ok(());
        }

        // We're looking for files inside ApolloGenerated/ directories
        let path_str = entry_path.to_string_lossy();
        if !path_str.contains("/ApolloGenerated/") && !path_str.contains("\\ApolloGenerated\\") {
            return Ok(());
        }
        let name = entry_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        if !name.ends_with(".graphql.swift") {
            return Ok(());
        }
        let dest_path = dest.join(name);
        std::fs::copy(entry_path, &dest_path).map_err(|e| CliError::Generic {
            description: format!(
                "copy {} -> {}: {}",
                entry_path.display(),
                dest_path.display(),
                e
            ),
        })?;
        *count += 1;
        Ok(())
    })
}

/// Recursively walks a directory, calling `visitor` for each file.
fn walk_dir_recursive(
    dir: &Path,
    visitor: &mut dyn FnMut(&Path) -> Result<(), CliError>,
) -> Result<(), CliError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| CliError::Generic {
        description: format!("read_dir {}: {}", dir.display(), e),
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| CliError::Generic {
            description: format!("dir entry error: {}", e),
        })?;
        let path = entry.path();
        if path.is_dir() {
            walk_dir_recursive(&path, visitor)?;
        } else {
            visitor(&path)?;
        }
    }
    Ok(())
}

/// Strips `import <module>\n` from all .graphql.swift files in a directory.
fn strip_import_from_dir(dir: &Path, module: &str) -> Result<(), CliError> {
    let import_line = format!("import {}", module);
    let entries = std::fs::read_dir(dir).map_err(|e| CliError::Generic {
        description: format!("read_dir {}: {}", dir.display(), e),
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| CliError::Generic {
            description: format!("dir entry error: {}", e),
        })?;
        let path = entry.path();
        if !path
            .file_name()
            .and_then(|f| f.to_str())
            .map_or(false, |n| n.ends_with(".graphql.swift"))
        {
            continue;
        }
        let content = std::fs::read_to_string(&path).map_err(|e| CliError::Generic {
            description: format!("read {}: {}", path.display(), e),
        })?;
        // Remove the import line (with trailing newline)
        let new_content = content.replace(&format!("{}\n", import_line), "");
        if new_content != content {
            std::fs::write(&path, new_content).map_err(|e| CliError::Generic {
                description: format!("write {}: {}", path.display(), e),
            })?;
        }
    }
    Ok(())
}

/// Adds a fast O(1) dictionary lookup to SchemaMetadata's objectType function,
/// gated behind a `fastObjectTypeLookup` flag.
fn optimize_schema_metadata(path: &Path) -> Result<(), CliError> {
    let content = std::fs::read_to_string(path).map_err(|e| CliError::Generic {
        description: format!("read {}: {}", path.display(), e),
    })?;

    // Match the objectType switch statement
    let func_re = Regex::new(
        r"(?s)( {2}public static func objectType\(forTypename typename: String\) -> ApolloAPI\.Object\? \{\n)(    switch typename \{\n(.*?)    default: return nil\n    \}\n  \})"
    ).unwrap();

    let caps = match func_re.captures(&content) {
        Some(c) => c,
        None => return Ok(()), // No match, nothing to optimize
    };

    let cases_block = &caps[3];
    let case_re = Regex::new(r#"^ {4}case (".*?"): return (V4\.Objects\.\w+)$"#).unwrap();

    let entries: Vec<(String, String)> = cases_block
        .lines()
        .filter_map(|line| {
            case_re.captures(line).map(|c| {
                (c[1].to_string(), c[2].to_string())
            })
        })
        .collect();

    if entries.is_empty() {
        return Ok(());
    }

    let dict_entries = entries
        .iter()
        .map(|(key, value)| format!("    {}: {}", key, value))
        .collect::<Vec<_>>()
        .join(",\n");

    let original_switch = &caps[2];

    let replacement = [
        "  public static var fastObjectTypeLookup = false",
        "",
        "  static let objectTypeMap: [String: ApolloAPI.Object] = [",
        &format!("{},", dict_entries),
        "  ]",
        "",
        "  public static func objectType(forTypename typename: String) -> ApolloAPI.Object? {",
        "    if fastObjectTypeLookup {",
        "      return objectTypeMap[typename]",
        "    }",
        original_switch,
    ]
    .join("\n");

    let new_content = func_re.replace(&content, &replacement);
    std::fs::write(path, new_content.as_ref()).map_err(|e| CliError::Generic {
        description: format!("write {}: {}", path.display(), e),
    })?;

    eprintln!(
        "Optimized SchemaMetadata with {} type entries",
        entries.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Wrapper struct for testing Generate arg parsing.
    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        cmd: Generate,
    }

    #[test]
    fn test_generate_parses_fetch_schema_flag() {
        let cli = TestCli::try_parse_from(["test", "--fetch-schema"]).unwrap();
        assert!(cli.cmd.fetch_schema);
    }

    #[test]
    fn test_generate_default_no_fetch_schema() {
        let cli = TestCli::try_parse_from(["test"]).unwrap();
        assert!(!cli.cmd.fetch_schema);
    }

    #[test]
    fn test_generate_verbose_flag() {
        let cli = TestCli::try_parse_from(["test", "--verbose"]).unwrap();
        assert!(cli.cmd.inputs.verbose);
    }

    #[test]
    fn test_generate_fetch_schema_returns_stub_error() {
        let cmd = Generate {
            inputs: InputOptions {
                path: "./config.json".to_string(),
                string: None,
                verbose: false,
            },
            fetch_schema: true,
            bazel_output_dir: None,
            bazel_mode: "schema_types".to_string(),
            bazel_framework_path: None,
            bazel_strip_import: None,
            bazel_optimize_schema_metadata: false,
        };
        let result = cmd.run();
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("Schema downloading is not yet supported"));
    }

    #[test]
    fn test_bazel_args_parsing() {
        let cli = TestCli::try_parse_from([
            "test",
            "--bazel-output-dir",
            "/tmp/out",
            "--bazel-mode",
            "operations",
            "--bazel-framework-path",
            "Features/Account",
            "--bazel-strip-import",
            "V4",
            "--bazel-optimize-schema-metadata",
        ])
        .unwrap();
        assert_eq!(cli.cmd.bazel_output_dir.as_deref(), Some("/tmp/out"));
        assert_eq!(cli.cmd.bazel_mode, "operations");
        assert_eq!(
            cli.cmd.bazel_framework_path.as_deref(),
            Some("Features/Account")
        );
        assert_eq!(cli.cmd.bazel_strip_import.as_deref(), Some("V4"));
        assert!(cli.cmd.bazel_optimize_schema_metadata);
    }

    #[test]
    fn test_bazel_mode_defaults_to_schema_types() {
        let cli = TestCli::try_parse_from(["test"]).unwrap();
        assert_eq!(cli.cmd.bazel_mode, "schema_types");
        assert!(cli.cmd.bazel_output_dir.is_none());
        assert!(!cli.cmd.bazel_optimize_schema_metadata);
    }

    #[test]
    fn test_strip_import_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let file1 = dir.path().join("Query.graphql.swift");
        let file2 = dir.path().join("Fragment.graphql.swift");
        let file3 = dir.path().join("Other.txt");
        std::fs::write(
            &file1,
            "import ApolloAPI\nimport V4\n\npublic struct Query {}\n",
        )
        .unwrap();
        std::fs::write(
            &file2,
            "import ApolloAPI\n\npublic struct Fragment {}\n",
        )
        .unwrap();
        std::fs::write(&file3, "import V4\nshould not be touched\n").unwrap();

        strip_import_from_dir(dir.path(), "V4").unwrap();

        let content1 = std::fs::read_to_string(&file1).unwrap();
        assert!(!content1.contains("import V4"));
        assert!(content1.contains("import ApolloAPI"));
        assert!(content1.contains("public struct Query"));

        // file2 had no "import V4" — should be unchanged
        let content2 = std::fs::read_to_string(&file2).unwrap();
        assert!(content2.contains("import ApolloAPI"));

        // file3 is not .graphql.swift — should be untouched
        let content3 = std::fs::read_to_string(&file3).unwrap();
        assert!(content3.contains("import V4"));
    }

    #[test]
    fn test_optimize_schema_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("SchemaMetadata.graphql.swift");
        let input = r#"import ApolloAPI

public enum SchemaMetadata: ApolloAPI.SchemaMetadata {
  public static let configuration: any ApolloAPI.SchemaConfiguration.Type = SchemaConfiguration.self

  public static func objectType(forTypename typename: String) -> ApolloAPI.Object? {
    switch typename {
    case "Cat": return V4.Objects.Cat
    case "Dog": return V4.Objects.Dog
    case "Bird": return V4.Objects.Bird
    default: return nil
    }
  }
}
"#;
        std::fs::write(&file, input).unwrap();

        optimize_schema_metadata(&file).unwrap();

        let result = std::fs::read_to_string(&file).unwrap();
        // Check 2-space indentation for declarations (matching enum body)
        assert!(result.contains("  public static var fastObjectTypeLookup = false"));
        assert!(result.contains("  static let objectTypeMap: [String: ApolloAPI.Object] = ["));
        // Check 4-space indentation for dictionary entries
        assert!(result.contains(r#"    "Cat": V4.Objects.Cat"#));
        assert!(result.contains(r#"    "Dog": V4.Objects.Dog"#));
        assert!(result.contains(r#"    "Bird": V4.Objects.Bird"#));
        // Check if-guard with correct indentation
        assert!(result.contains("    if fastObjectTypeLookup {"));
        assert!(result.contains("      return objectTypeMap[typename]"));
        // Original switch should still be present (gated behind !fastObjectTypeLookup)
        assert!(result.contains("switch typename"));
        assert!(result.contains("default: return nil"));
    }

    #[test]
    fn test_optimize_schema_metadata_no_match() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("SchemaMetadata.graphql.swift");
        let input = "// No objectType function here\npublic enum SchemaMetadata {}\n";
        std::fs::write(&file, input).unwrap();

        optimize_schema_metadata(&file).unwrap();

        let result = std::fs::read_to_string(&file).unwrap();
        assert_eq!(result, input); // unchanged
    }

    #[test]
    fn test_copy_swift_files_recursive() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();

        // Create directory structure
        std::fs::create_dir_all(src.path().join("Objects")).unwrap();
        std::fs::create_dir_all(src.path().join("CustomScalars")).unwrap();
        std::fs::write(
            src.path().join("Objects/Cat.graphql.swift"),
            "cat content",
        )
        .unwrap();
        std::fs::write(
            src.path().join("SchemaConfiguration.swift"),
            "config content",
        )
        .unwrap();
        std::fs::write(
            src.path().join("CustomScalars/Date.graphql.swift"),
            "date content",
        )
        .unwrap();
        std::fs::write(
            src.path().join("SchemaMetadata.graphql.swift"),
            "metadata content",
        )
        .unwrap();

        let mut count = 0u32;
        copy_swift_files_recursive(src.path(), dest.path(), &mut count, |rel_path| {
            !rel_path.starts_with("CustomScalars/") && rel_path != "SchemaConfiguration.swift"
        })
        .unwrap();

        assert_eq!(count, 2); // Cat + SchemaMetadata
        assert!(dest.path().join("Objects/Cat.graphql.swift").exists());
        assert!(dest.path().join("SchemaMetadata.graphql.swift").exists());
        assert!(!dest.path().join("CustomScalars/Date.graphql.swift").exists());
        assert!(!dest.path().join("SchemaConfiguration.swift").exists());
    }

    #[test]
    fn test_unknown_bazel_mode_errors() {
        let dir = tempfile::tempdir().unwrap();
        let gen = Generate {
            inputs: InputOptions {
                path: "./config.json".to_string(),
                string: None,
                verbose: false,
            },
            fetch_schema: false,
            bazel_output_dir: None,
            bazel_mode: "invalid".to_string(),
            bazel_framework_path: None,
            bazel_strip_import: None,
            bazel_optimize_schema_metadata: false,
        };
        let result = gen.populate_bazel_tree_artifact(dir.path().to_str().unwrap());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("Unknown --bazel-mode: invalid"));
    }
}
