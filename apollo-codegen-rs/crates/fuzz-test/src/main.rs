//! Fuzz test generator: uses `apollo-smith` to produce random GraphQL schemas
//! and operations, then writes them to disk alongside a codegen config.
//!
//! Usage:
//!     apollo-codegen-fuzz --count 20 --seed 42 --complexity medium --output-dir /tmp/fuzz

use anyhow::{Context, Result};
use apollo_smith::DocumentBuilder;
use arbitrary::Unstructured;
use clap::Parser;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

/// Schema complexity presets controlling the size of generated schemas.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum Complexity {
    Small,
    Medium,
    Large,
    Huge,
}

impl Complexity {
    /// Number of random bytes to feed to `Unstructured` for `apollo-smith`.
    fn byte_count(self) -> usize {
        match self {
            Complexity::Small => 512,
            Complexity::Medium => 2048,
            Complexity::Large => 8192,
            Complexity::Huge => 32768,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "apollo-codegen-fuzz")]
#[command(about = "Generate random GraphQL schemas and operations for fuzz testing")]
struct Args {
    /// Number of schemas to generate
    #[arg(long, default_value_t = 20)]
    count: usize,

    /// Random seed for reproducibility (0 = random)
    #[arg(long, default_value_t = 0)]
    seed: u64,

    /// Schema size / complexity
    #[arg(long, value_enum, default_value_t = Complexity::Medium)]
    complexity: Complexity,

    /// Output directory for generated schemas
    #[arg(long)]
    output_dir: PathBuf,
}

/// Generate a single random schema and set of operations using `apollo-smith`.
fn generate_schema(rng: &mut StdRng, complexity: Complexity) -> Result<(String, Vec<String>)> {
    let byte_count = complexity.byte_count();

    // Fill a buffer with random bytes to drive apollo-smith.
    // We need the buffer to live as long as the Unstructured reference,
    // so we allocate it here and pass a reference.
    let bytes: Vec<u8> = (0..byte_count).map(|_| rng.gen()).collect();
    let mut u = Unstructured::new(&bytes);

    // Build a random document using the DocumentBuilder API
    let builder = DocumentBuilder::new(&mut u)
        .context("failed to create DocumentBuilder from random input")?;

    // Generate schema definition and convert to SDL string
    let schema_doc = builder.finish();
    let schema_sdl: String = String::from(schema_doc);

    // Split the document into schema types and operations.
    // apollo-smith generates a full document. We need to separate schema
    // definitions from operation/fragment definitions.
    let mut schema_parts: Vec<String> = Vec::new();
    let mut operation_parts: Vec<String> = Vec::new();

    for line_group in schema_sdl.as_str().split("\n\n") {
        let trimmed = line_group.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Heuristic: operation definitions start with query/mutation/subscription/fragment
        let first_word = trimmed.split_whitespace().next().unwrap_or("");
        match first_word {
            "query" | "mutation" | "subscription" | "fragment" | "{" => {
                operation_parts.push(trimmed.to_string());
            }
            _ => {
                schema_parts.push(trimmed.to_string());
            }
        }
    }

    // Deduplicate schema type definitions (apollo-smith may generate duplicates)
    let mut seen_type_names = std::collections::HashSet::new();
    let mut deduped_parts: Vec<String> = Vec::new();
    for part in &schema_parts {
        // Extract the type name from the definition
        let trimmed = part.trim();
        // Skip description strings
        let def_start = if trimmed.starts_with("\"\"\"") {
            // Find end of description
            if let Some(end) = trimmed[3..].find("\"\"\"") {
                trimmed[end + 6..].trim()
            } else {
                trimmed
            }
        } else if trimmed.starts_with('"') {
            // Single-line description
            if let Some(end) = trimmed[1..].find('"') {
                trimmed[end + 2..].trim()
            } else {
                trimmed
            }
        } else {
            trimmed
        };

        // Extract type name: "type X", "scalar X", "enum X", "union X", "interface X", "input X"
        let words: Vec<&str> = def_start.split_whitespace().collect();
        let type_name = if words.len() >= 2 {
            match words[0] {
                "type" | "scalar" | "enum" | "union" | "interface" | "input" | "extend" => {
                    Some(words[1].trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_'))
                }
                "schema" => Some("__schema__"),
                _ => None,
            }
        } else {
            None
        };

        if let Some(name) = type_name {
            if seen_type_names.insert(name.to_string()) {
                deduped_parts.push(part.clone());
            }
            // Skip duplicate
        } else {
            deduped_parts.push(part.clone());
        }
    }

    let schema_content = deduped_parts.join("\n\n");

    // If no operations were generated, create a simple query
    if operation_parts.is_empty() {
        operation_parts.push("query FuzzQuery { __typename }".to_string());
    }

    // Ensure all operations are named (Apollo requires named operations)
    let mut named_operations: Vec<String> = Vec::new();
    for (i, op) in operation_parts.iter().enumerate() {
        let trimmed = op.trim();
        if trimmed.starts_with('{') {
            // Anonymous query — give it a name
            named_operations.push(format!("query FuzzAnon{} {}", i, trimmed));
        } else if trimmed.starts_with("fragment") {
            // Fragments are fine as-is
            named_operations.push(trimmed.to_string());
        } else {
            named_operations.push(trimmed.to_string());
        }
    }

    Ok((schema_content, named_operations))
}

/// Write a single test case to disk.
fn write_test_case(
    base_dir: &Path,
    index: usize,
    schema: &str,
    operations: &[String],
) -> Result<PathBuf> {
    let case_dir = base_dir.join(format!("case-{:04}", index));
    let ops_dir = case_dir.join("operations");
    fs::create_dir_all(&ops_dir)?;

    // Write schema
    let schema_path = case_dir.join("schema.graphqls");
    fs::write(&schema_path, schema)?;

    // Write operations
    for (i, op) in operations.iter().enumerate() {
        let op_path = ops_dir.join(format!("op-{:03}.graphql", i));
        fs::write(&op_path, op)?;
    }

    // Write codegen config pointing to schema and operations
    let config = json!({
        "schemaNamespace": format!("FuzzSchema{}", index),
        "input": {
            "operationSearchPaths": [
                ops_dir.join("*.graphql").to_string_lossy()
            ],
            "schemaSearchPaths": [
                schema_path.to_string_lossy()
            ]
        },
        "output": {
            "testMocks": {"none": {}},
            "schemaTypes": {
                "path": case_dir.join("Generated").to_string_lossy(),
                "moduleType": {"swiftPackageManager": {}}
            },
            "operations": {"inSchemaModule": {}}
        },
        "options": {
            "pruneGeneratedFiles": false
        }
    });

    let config_path = case_dir.join("config.json");
    fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

    Ok(case_dir)
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Determine the actual seed
    let seed = if args.seed == 0 {
        rand::thread_rng().gen()
    } else {
        args.seed
    };

    eprintln!("Seed: {}", seed);
    eprintln!(
        "Generating {} schemas (complexity: {:?})...",
        args.count, args.complexity
    );

    fs::create_dir_all(&args.output_dir)?;

    let mut rng = StdRng::seed_from_u64(seed);
    let mut generated = 0;
    let mut failures = 0;

    for i in 0..args.count {
        match generate_schema(&mut rng, args.complexity) {
            Ok((schema, operations)) => {
                let case_dir = write_test_case(&args.output_dir, i, &schema, &operations)?;
                generated += 1;
                eprintln!(
                    "  [{}/{}] {} ({} ops)",
                    i + 1,
                    args.count,
                    case_dir.display(),
                    operations.len()
                );
            }
            Err(e) => {
                // apollo-smith may fail on some random inputs; that is expected
                failures += 1;
                eprintln!("  [{}/{}] skipped: {}", i + 1, args.count, e);
            }
        }
    }

    eprintln!("");
    eprintln!(
        "Done: {} generated, {} skipped",
        generated, failures
    );

    // Print output dir path to stdout for scripts to capture
    println!("{}", args.output_dir.display());

    Ok(())
}
