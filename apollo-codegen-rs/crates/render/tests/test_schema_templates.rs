//! Tests that schema templates produce output matching the golden files.

use std::path::PathBuf;

fn golden_base() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("golden-test/fixtures/AnimalKingdomAPI/AnimalKingdomAPI")
}

fn golden_dir() -> PathBuf {
    golden_base().join("Sources/Schema")
}

fn read_golden(relative_path: &str) -> String {
    let path = golden_dir().join(relative_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read golden file {}: {}", path.display(), e))
}

fn assert_matches_golden(generated: &str, golden_path: &str) {
    let expected = read_golden(golden_path);
    if generated != expected {
        // Find first difference
        let gen_lines: Vec<&str> = generated.lines().collect();
        let exp_lines: Vec<&str> = expected.lines().collect();
        for (i, (g, e)) in gen_lines.iter().zip(exp_lines.iter()).enumerate() {
            if g != e {
                panic!(
                    "Mismatch at line {} of {}:\n  expected: {:?}\n  got:      {:?}\n\nFull generated:\n{}\n\nFull expected:\n{}",
                    i + 1, golden_path, e, g, generated, expected
                );
            }
        }
        if gen_lines.len() != exp_lines.len() {
            panic!(
                "Line count mismatch in {}: generated {} lines, expected {} lines\n\nFull generated:\n{}\n\nFull expected:\n{}",
                golden_path, gen_lines.len(), exp_lines.len(), generated, expected
            );
        }
        panic!("Content differs but couldn't find line difference in {}", golden_path);
    }
}

#[test]
fn object_template_height_no_interfaces() {
    let generated = apollo_codegen_render::templates::object::render(
        "Height",
        "Height",
        &[],
        "public ",
        "ApolloAPI",
        "AnimalKingdomAPI",
        true,
        None,
    );
    assert_matches_golden(&generated, "Objects/Height.graphql.swift");
}

#[test]
fn object_template_dog_with_interfaces() {
    let generated = apollo_codegen_render::templates::object::render(
        "Dog",
        "Dog",
        &[
            "Animal".to_string(),
            "Pet".to_string(),
            "HousePet".to_string(),
            "WarmBlooded".to_string(),
        ],
        "public ",
        "ApolloAPI",
        "AnimalKingdomAPI",
        true,
        None,
    );
    assert_matches_golden(&generated, "Objects/Dog.graphql.swift");
}

#[test]
fn interface_template_animal() {
    let generated = apollo_codegen_render::templates::interface::render(
        "Animal",
        "Animal",
        "public ",
        "ApolloAPI",
        None,
        "AnimalKingdomAPI",
        true,
    );
    assert_matches_golden(&generated, "Interfaces/Animal.graphql.swift");
}

#[test]
fn union_template_classroom_pet() {
    let generated = apollo_codegen_render::templates::union_type::render(
        "ClassroomPet",
        "ClassroomPet",
        &[
            "Cat".to_string(),
            "Bird".to_string(),
            "Rat".to_string(),
            "PetRock".to_string(),
        ],
        "public ",
        "ApolloAPI",
        "AnimalKingdomAPI",
        true,
        None,
    );
    assert_matches_golden(&generated, "Unions/ClassroomPet.graphql.swift");
}

#[test]
fn enum_template_skin_covering() {
    use apollo_codegen_render::templates::enum_type::EnumValue;

    let generated = apollo_codegen_render::templates::enum_type::render(
        "SkinCovering",
        "SkinCovering",
        &[
            EnumValue {
                name: "fur".to_string(),
                raw_value: "FUR".to_string(),
                description: None,
                is_deprecated: false,
                deprecation_reason: None,
                is_renamed: false,
            },
            EnumValue {
                name: "hair".to_string(),
                raw_value: "HAIR".to_string(),
                description: None,
                is_deprecated: false,
                deprecation_reason: None,
                is_renamed: false,
            },
            EnumValue {
                name: "feathers".to_string(),
                raw_value: "FEATHERS".to_string(),
                description: None,
                is_deprecated: false,
                deprecation_reason: None,
                is_renamed: false,
            },
            EnumValue {
                name: "scales".to_string(),
                raw_value: "SCALES".to_string(),
                description: None,
                is_deprecated: false,
                deprecation_reason: None,
                is_renamed: false,
            },
        ],
        "public ",
        "ApolloAPI",
        true, // AnimalKingdomAPI config uses camelCase enum conversion
        None,
    );
    assert_matches_golden(&generated, "Enums/SkinCovering.graphql.swift");
}

// --- CustomScalar tests ---

#[test]
fn custom_scalar_template_custom_date() {
    let generated = apollo_codegen_render::templates::custom_scalar::render(
        "CustomDate",
        None,
        None,
        "public ",
        "ApolloAPI",
    );
    let golden_path = golden_dir().join("CustomScalars/CustomDate.swift");
    let expected = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", golden_path.display(), e));
    if generated != expected {
        let gen_lines: Vec<&str> = generated.lines().collect();
        let exp_lines: Vec<&str> = expected.lines().collect();
        for (i, (g, e)) in gen_lines.iter().zip(exp_lines.iter()).enumerate() {
            if g != e {
                panic!(
                    "CustomDate mismatch at line {}:\n  expected: {:?}\n  got:      {:?}\n\nFull generated:\n{}\n\nFull expected:\n{}",
                    i + 1, e, g, generated, expected
                );
            }
        }
        if gen_lines.len() != exp_lines.len() {
            panic!(
                "CustomDate line count: generated {} vs expected {}\n\nFull generated:\n{}\n\nFull expected:\n{}",
                gen_lines.len(), exp_lines.len(), generated, expected
            );
        }
    }
}

#[test]
fn custom_scalar_template_id() {
    let generated = apollo_codegen_render::templates::custom_scalar::render(
        "ID",
        Some("The `ID` scalar type represents a unique identifier, often used to refetch an object or as key for a cache. The ID type appears in a JSON response as a String; however, it is not intended to be human-readable. When expected as an input type, any string (such as `\"4\"`) or integer (such as `4`) input value will be accepted as an ID."),
        None,
        "public ",
        "ApolloAPI",
    );
    let golden_path = golden_dir().join("CustomScalars/ID.swift");
    let expected = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", golden_path.display(), e));
    if generated != expected {
        let gen_lines: Vec<&str> = generated.lines().collect();
        let exp_lines: Vec<&str> = expected.lines().collect();
        for (i, (g, e)) in gen_lines.iter().zip(exp_lines.iter()).enumerate() {
            if g != e {
                panic!(
                    "ID mismatch at line {}:\n  expected: {:?}\n  got:      {:?}",
                    i + 1, e, g,
                );
            }
        }
        if gen_lines.len() != exp_lines.len() {
            panic!("ID line count: generated {} vs expected {}", gen_lines.len(), exp_lines.len());
        }
    }
}

// --- SchemaMetadata test ---

#[test]
fn schema_metadata_template() {
    let generated = apollo_codegen_render::templates::schema_metadata::render(
        "AnimalKingdomAPI",
        &[
            ("Query".to_string(), "Query".to_string()),
            ("Human".to_string(), "Human".to_string()),
            ("Cat".to_string(), "Cat".to_string()),
            ("Dog".to_string(), "Dog".to_string()),
            ("Bird".to_string(), "Bird".to_string()),
            ("Fish".to_string(), "Fish".to_string()),
            ("Rat".to_string(), "Rat".to_string()),
            ("PetRock".to_string(), "PetRock".to_string()),
            ("Crocodile".to_string(), "Crocodile".to_string()),
            ("Height".to_string(), "Height".to_string()),
            ("Mutation".to_string(), "Mutation".to_string()),
        ],
        "public ",
        "ApolloAPI",
        false,
    );
    assert_matches_golden(&generated, "SchemaMetadata.graphql.swift");
}

// --- SchemaConfiguration test ---

#[test]
fn schema_configuration_template() {
    let generated = apollo_codegen_render::templates::schema_config::render(
        "public ",
        "ApolloAPI",
        false,
    );
    let golden_path = golden_dir().join("SchemaConfiguration.swift");
    let expected = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", golden_path.display(), e));
    if generated != expected {
        let gen_lines: Vec<&str> = generated.lines().collect();
        let exp_lines: Vec<&str> = expected.lines().collect();
        for (i, (g, e)) in gen_lines.iter().zip(exp_lines.iter()).enumerate() {
            if g != e {
                panic!(
                    "SchemaConfig mismatch at line {}:\n  expected: {:?}\n  got:      {:?}",
                    i + 1, e, g,
                );
            }
        }
        if gen_lines.len() != exp_lines.len() {
            panic!("SchemaConfig line count: generated {} vs expected {}\n\nFull generated:\n{}\n\nFull expected:\n{}", gen_lines.len(), exp_lines.len(), generated, expected);
        }
    }
}

// --- Package.swift test ---

#[test]
fn package_swift_template() {
    let generated = apollo_codegen_render::templates::package_swift::render(
        "AnimalKingdomAPI",
        Some(("AnimalKingdomAPITestMocks", "./TestMocks")),
    );
    let golden_path = golden_base().join("Package.swift");
    let expected = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", golden_path.display(), e));
    if generated != expected {
        let gen_lines: Vec<&str> = generated.lines().collect();
        let exp_lines: Vec<&str> = expected.lines().collect();
        for (i, (g, e)) in gen_lines.iter().zip(exp_lines.iter()).enumerate() {
            if g != e {
                panic!(
                    "Package.swift mismatch at line {}:\n  expected: {:?}\n  got:      {:?}",
                    i + 1, e, g,
                );
            }
        }
        if gen_lines.len() != exp_lines.len() {
            panic!("Package.swift line count: generated {} vs expected {}\n\nFull generated:\n{}\n\nFull expected:\n{}", gen_lines.len(), exp_lines.len(), generated, expected);
        }
    }
}

// --- MockInterfaces test ---

#[test]
fn mock_interfaces_template() {
    let generated = apollo_codegen_render::templates::mock_interfaces::render(
        &[
            "Animal".to_string(),
            "WarmBlooded".to_string(),
            "Pet".to_string(),
            "HousePet".to_string(),
        ],
        "public ",
        "AnimalKingdomAPI",
        "AnimalKingdomAPI",
    );
    let golden_path = golden_base()
        .join("TestMocks/MockObject+Interfaces.graphql.swift");
    let expected = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", golden_path.display(), e));
    if generated != expected {
        let gen_lines: Vec<&str> = generated.lines().collect();
        let exp_lines: Vec<&str> = expected.lines().collect();
        for (i, (g, e)) in gen_lines.iter().zip(exp_lines.iter()).enumerate() {
            if g != e {
                panic!(
                    "MockInterfaces mismatch at line {}:\n  expected: {:?}\n  got:      {:?}",
                    i + 1, e, g,
                );
            }
        }
        if gen_lines.len() != exp_lines.len() {
            panic!("MockInterfaces line count: generated {} vs expected {}\n\nFull generated:\n{}\n\nFull expected:\n{}", gen_lines.len(), exp_lines.len(), generated, expected);
        }
    }
}

// --- MockUnions test ---

#[test]
fn mock_unions_template() {
    let generated = apollo_codegen_render::templates::mock_unions::render(
        &["ClassroomPet".to_string()],
        "public ",
        "AnimalKingdomAPI",
        "AnimalKingdomAPI",
    );
    let golden_path = golden_base()
        .join("TestMocks/MockObject+Unions.graphql.swift");
    let expected = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", golden_path.display(), e));
    if generated != expected {
        let gen_lines: Vec<&str> = generated.lines().collect();
        let exp_lines: Vec<&str> = expected.lines().collect();
        for (i, (g, e)) in gen_lines.iter().zip(exp_lines.iter()).enumerate() {
            if g != e {
                panic!(
                    "MockUnions mismatch at line {}:\n  expected: {:?}\n  got:      {:?}",
                    i + 1, e, g,
                );
            }
        }
        if gen_lines.len() != exp_lines.len() {
            panic!("MockUnions line count: generated {} vs expected {}\n\nFull generated:\n{}\n\nFull expected:\n{}", gen_lines.len(), exp_lines.len(), generated, expected);
        }
    }
}
