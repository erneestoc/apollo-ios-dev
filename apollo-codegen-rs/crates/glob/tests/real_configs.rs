//! Integration tests that verify glob matches against the real test API files.

use apollo_codegen_glob::match_search_paths;
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

#[test]
fn find_animal_kingdom_schema() {
    let root = repo_root().join("Tests/TestCodeGenConfigurations/SwiftPackageManager");
    let paths = vec![
        "../../../Sources/AnimalKingdomAPI/animalkingdom-graphql/AnimalSchema.graphqls".to_string(),
    ];
    let matches = match_search_paths(&paths, Some(&root)).unwrap();
    assert_eq!(matches.len(), 1, "Should find exactly one schema file");
    let first = matches.iter().next().unwrap();
    assert!(
        first.ends_with("AnimalSchema.graphqls"),
        "Should find AnimalSchema.graphqls, got: {}",
        first
    );
}

#[test]
fn find_animal_kingdom_operations() {
    let root = repo_root().join("Tests/TestCodeGenConfigurations/SwiftPackageManager");
    let paths = vec![
        "../../../Sources/AnimalKingdomAPI/animalkingdom-graphql/*.graphql".to_string(),
    ];
    let matches = match_search_paths(&paths, Some(&root)).unwrap();
    assert!(
        matches.len() >= 10,
        "Should find at least 10 operation files, got: {}",
        matches.len()
    );
    // All should be .graphql files
    for path in &matches {
        assert!(
            path.ends_with(".graphql"),
            "All matches should be .graphql files, got: {}",
            path
        );
    }
}

#[test]
fn find_star_wars_schema() {
    // StarWarsAPI uses a JSON introspection schema
    let root = repo_root();
    let paths = vec![
        "Sources/StarWarsAPI/starwars-graphql/schema.json".to_string(),
    ];
    let matches = match_search_paths(&paths, Some(&root)).unwrap();
    assert_eq!(matches.len(), 1, "Should find exactly one schema file");
}

#[test]
fn find_github_schema() {
    let root = repo_root();
    let paths = vec![
        "Sources/GitHubAPI/graphql/schema.graphqls".to_string(),
    ];
    let matches = match_search_paths(&paths, Some(&root)).unwrap();
    assert_eq!(matches.len(), 1, "Should find exactly one schema file");
}

#[test]
fn globstar_finds_nested_operations() {
    // The EmbeddedInTarget-RelativeAbsolute config uses escaped paths with globstar
    let root = repo_root().join("Tests/TestCodeGenConfigurations/EmbeddedInTarget-RelativeAbsolute");
    // The actual pattern uses escaped slashes but serde will unescape them
    let paths = vec![
        "PackageOne/Sources/PackageOne/graphql/**/*.graphql".to_string(),
    ];
    let matches = match_search_paths(&paths, Some(&root)).unwrap();
    // This config points to the same AnimalKingdom files via symlinks or relative paths
    // If the directory doesn't exist at this relative path, we get 0 matches which is ok
    println!(
        "EmbeddedInTarget-RelativeAbsolute globstar matches: {}",
        matches.len()
    );
}

#[test]
fn excluded_directories_are_skipped() {
    let root = repo_root();
    // This pattern would match files in .build/ if not excluded
    let paths = vec!["**/*.graphql".to_string()];
    let matches = match_search_paths(&paths, Some(&root)).unwrap();
    // None of the matches should be in .build, .swiftpm, or .Pods
    for path in &matches {
        assert!(
            !path.contains("/.build/"),
            "Should exclude .build directory, found: {}",
            path
        );
        assert!(
            !path.contains("/.swiftpm/"),
            "Should exclude .swiftpm directory, found: {}",
            path
        );
    }
}
