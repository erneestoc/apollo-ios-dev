//! Integration tests for schema loading and operation parsing.

use apollo_codegen_frontend::GraphQLFrontend;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn read_file(relative_path: &str) -> String {
    let path = repo_root().join(relative_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e))
}

#[test]
fn load_animal_kingdom_sdl_schema() {
    let schema_content =
        read_file("Sources/AnimalKingdomAPI/animalkingdom-graphql/AnimalSchema.graphqls");
    let frontend = GraphQLFrontend::load_schema(&[(
        schema_content,
        "AnimalSchema.graphqls".to_string(),
    )]);
    assert!(frontend.is_ok(), "Failed to load schema: {:?}", frontend.err());
}

#[test]
fn load_star_wars_introspection_json_schema() {
    let schema_content = read_file("Sources/StarWarsAPI/starwars-graphql/schema.json");
    let frontend = GraphQLFrontend::load_schema(&[(
        schema_content,
        "schema.json".to_string(),
    )]);
    assert!(
        frontend.is_ok(),
        "Failed to load introspection schema: {:?}",
        frontend.err()
    );
}

#[test]
fn load_github_sdl_schema() {
    let schema_content = read_file("Sources/GitHubAPI/graphql/schema.graphqls");
    let frontend = GraphQLFrontend::load_schema(&[(
        schema_content,
        "schema.graphqls".to_string(),
    )]);
    assert!(
        frontend.is_ok(),
        "Failed to load GitHub schema: {:?}",
        frontend.err()
    );
}

#[test]
fn parse_animal_kingdom_operations() {
    let schema_content =
        read_file("Sources/AnimalKingdomAPI/animalkingdom-graphql/AnimalSchema.graphqls");
    let frontend = GraphQLFrontend::load_schema(&[(
        schema_content,
        "AnimalSchema.graphqls".to_string(),
    )])
    .expect("Failed to load schema");

    // Read all .graphql operation files
    let ops_dir = repo_root().join("Sources/AnimalKingdomAPI/animalkingdom-graphql");
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(&ops_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "graphql") {
            let content = std::fs::read_to_string(&path).unwrap();
            sources.push((content, path.to_string_lossy().to_string()));
        }
    }

    assert!(!sources.is_empty(), "Should find operation files");
    println!("Found {} operation files", sources.len());

    let doc = frontend.parse_operations(&sources);
    assert!(
        doc.is_ok(),
        "Failed to parse operations: {:?}",
        doc.err()
    );
    let doc = doc.unwrap();
    println!(
        "Parsed {} operations and {} fragments",
        doc.operations.named.len() + if doc.operations.anonymous.is_some() { 1 } else { 0 },
        doc.fragments.len()
    );
}

#[test]
fn compile_animal_kingdom() {
    let schema_content =
        read_file("Sources/AnimalKingdomAPI/animalkingdom-graphql/AnimalSchema.graphqls");
    let frontend = GraphQLFrontend::load_schema(&[(
        schema_content,
        "AnimalSchema.graphqls".to_string(),
    )])
    .expect("Failed to load schema");

    let ops_dir = repo_root().join("Sources/AnimalKingdomAPI/animalkingdom-graphql");
    let mut sources = Vec::new();
    let mut source_map = std::collections::BTreeMap::new();

    for entry in std::fs::read_dir(&ops_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "graphql") {
            let content = std::fs::read_to_string(&path).unwrap();
            let file_path = path.to_string_lossy().to_string();
            source_map.insert(file_path.clone(), (content.clone(), file_path.clone()));
            sources.push((content, file_path));
        }
    }

    let doc = frontend.parse_operations(&sources).expect("Failed to parse operations");

    let options = apollo_codegen_frontend::compiler::CompileOptions::default();
    let result = frontend.compile(&doc, &source_map, &options);
    assert!(
        result.is_ok(),
        "Failed to compile: {:?}",
        result.err()
    );

    let result = result.unwrap();
    println!("Compilation result:");
    println!("  Operations: {}", result.operations.len());
    println!("  Fragments: {}", result.fragments.len());
    println!("  Referenced types: {}", result.referenced_types.len());
    println!("  Root query type: {}", result.root_types.query_type.name());

    assert!(result.operations.len() > 0, "Should have operations");
    assert!(result.fragments.len() > 0, "Should have fragments");
    assert!(result.referenced_types.len() > 0, "Should have referenced types");

    // Check a known operation
    let all_animals = result
        .operations
        .iter()
        .find(|op| op.name == "AllAnimalsQuery");
    assert!(
        all_animals.is_some(),
        "Should find AllAnimalsQuery. Found operations: {:?}",
        result.operations.iter().map(|o| &o.name).collect::<Vec<_>>()
    );

    if let Some(op) = all_animals {
        assert_eq!(op.operation_type, apollo_codegen_frontend::OperationType::Query);
        println!("  AllAnimalsQuery source length: {}", op.source.len());
        assert!(!op.source.is_empty(), "Operation source should not be empty");
    }
}
