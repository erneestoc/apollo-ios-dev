//! Tests that operation and fragment templates produce output matching the golden files.

use std::path::PathBuf;

use apollo_codegen_render::templates::fragment::{self, FragmentConfig};
use apollo_codegen_render::templates::operation::{self, OperationConfig, OperationType, VariableConfig};
use apollo_codegen_render::templates::selection_set::*;

fn golden_base() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("golden-test/fixtures/AnimalKingdomAPI/AnimalKingdomAPI")
}

fn read_golden_fragment(name: &str) -> String {
    let path = golden_base().join(format!("Sources/Fragments/{}.graphql.swift", name));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read golden file {}: {}", path.display(), e))
}

fn read_golden_operation(kind: &str, name: &str) -> String {
    let path = golden_base().join(format!("Sources/Operations/{}/{}.graphql.swift", kind, name));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read golden file {}: {}", path.display(), e))
}

fn assert_matches(generated: &str, expected: &str, context: &str) {
    if generated != expected {
        let gen_lines: Vec<&str> = generated.lines().collect();
        let exp_lines: Vec<&str> = expected.lines().collect();
        for (i, (g, e)) in gen_lines.iter().zip(exp_lines.iter()).enumerate() {
            if g != e {
                panic!(
                    "Mismatch at line {} of {}:\n  expected: {:?}\n  got:      {:?}\n\nFull generated:\n{}\n\nFull expected:\n{}",
                    i + 1, context, e, g, generated, expected
                );
            }
        }
        if gen_lines.len() != exp_lines.len() {
            panic!(
                "Line count mismatch in {}: generated {} lines, expected {} lines\n\nFull generated:\n{}\n\nFull expected:\n{}",
                context, gen_lines.len(), exp_lines.len(), generated, expected
            );
        }
        panic!("Content differs but couldn't find line difference in {}", context);
    }
}

// ============================================================================
// Fragment Tests
// ============================================================================

#[test]
fn fragment_template_dog_fragment() {
    // DogFragment: simple fragment on object type, no nested types, no fragments
    // fragment DogFragment on Dog { __typename species }
    let config = FragmentConfig {
        name: "DogFragment",
        fragment_definition: "fragment DogFragment on Dog { __typename species }",
        schema_namespace: "AnimalKingdomAPI",
        access_modifier: "public ",
        selection_set: SelectionSetConfig {
            struct_name: "DogFragment",
            schema_namespace: "AnimalKingdomAPI",
            parent_type: ParentTypeRef::Object("Dog"),
            is_root: true,
            is_inline_fragment: false,
            conformance: SelectionSetConformance::Fragment,
            root_entity_type: None,
            merged_sources: vec![],
            selections: vec![
                SelectionItem::Field(FieldSelectionItem {
                    name: "__typename",
                    swift_type: "String",
                    arguments: None,
                }),
                SelectionItem::Field(FieldSelectionItem {
                    name: "species",
                    swift_type: "String",
                    arguments: None,
                }),
            ],
            field_accessors: vec![
                FieldAccessor {
                    name: "species",
                    swift_type: "String",
                },
            ],
            inline_fragment_accessors: vec![],
            fragment_spreads: vec![],
            initializer: Some(InitializerConfig {
                parameters: vec![
                    InitParam {
                        name: "species",
                        swift_type: "String",
                        default_value: None,
                    },
                ],
                data_entries: vec![
                    DataEntry {
                        key: "__typename",
                        value: DataEntryValue::Typename("AnimalKingdomAPI.Objects.Dog.typename"),
                    },
                    DataEntry {
                        key: "species",
                        value: DataEntryValue::Variable("species"),
                    },
                ],
                fulfilled_fragments: vec!["DogFragment"],
                typename_value: TypenameValue::Fixed("AnimalKingdomAPI.Objects.Dog.typename"),
            }),
            nested_types: vec![],
            type_aliases: vec![],
            indent: 0,
            access_modifier: "public ",
        },
    };

    let generated = fragment::render(&config);
    let expected = read_golden_fragment("DogFragment");
    assert_matches(&generated, &expected, "DogFragment");
}

#[test]
fn fragment_template_height_in_meters() {
    // HeightInMeters: fragment with nested entity field
    // fragment HeightInMeters on Animal { __typename height { __typename meters } }
    let config = FragmentConfig {
        name: "HeightInMeters",
        fragment_definition: "fragment HeightInMeters on Animal { __typename height { __typename meters } }",
        schema_namespace: "AnimalKingdomAPI",
        access_modifier: "public ",
        selection_set: SelectionSetConfig {
            struct_name: "HeightInMeters",
            schema_namespace: "AnimalKingdomAPI",
            parent_type: ParentTypeRef::Interface("Animal"),
            is_root: true,
            is_inline_fragment: false,
            conformance: SelectionSetConformance::Fragment,
            root_entity_type: None,
            merged_sources: vec![],
            selections: vec![
                SelectionItem::Field(FieldSelectionItem {
                    name: "__typename",
                    swift_type: "String",
                    arguments: None,
                }),
                SelectionItem::Field(FieldSelectionItem {
                    name: "height",
                    swift_type: "Height",
                    arguments: None,
                }),
            ],
            field_accessors: vec![
                FieldAccessor {
                    name: "height",
                    swift_type: "Height",
                },
            ],
            inline_fragment_accessors: vec![],
            fragment_spreads: vec![],
            initializer: Some(InitializerConfig {
                parameters: vec![
                    InitParam {
                        name: "__typename",
                        swift_type: "String",
                        default_value: None,
                    },
                    InitParam {
                        name: "height",
                        swift_type: "Height",
                        default_value: None,
                    },
                ],
                data_entries: vec![
                    DataEntry {
                        key: "__typename",
                        value: DataEntryValue::Variable("__typename"),
                    },
                    DataEntry {
                        key: "height",
                        value: DataEntryValue::FieldData("height"),
                    },
                ],
                fulfilled_fragments: vec!["HeightInMeters"],
                typename_value: TypenameValue::Parameter,
            }),
            nested_types: vec![
                NestedSelectionSet {
                    doc_comment: "/// Height",
                    parent_type_comment: "///\n  /// Parent Type: `Height`",
                    config: SelectionSetConfig {
                        struct_name: "Height",
                        schema_namespace: "AnimalKingdomAPI",
                        parent_type: ParentTypeRef::Object("Height"),
                        is_root: false,
                        is_inline_fragment: false,
                        conformance: SelectionSetConformance::SelectionSet,
                        root_entity_type: None,
                        merged_sources: vec![],
                        selections: vec![
                            SelectionItem::Field(FieldSelectionItem {
                                name: "__typename",
                                swift_type: "String",
                                arguments: None,
                            }),
                            SelectionItem::Field(FieldSelectionItem {
                                name: "meters",
                                swift_type: "Int",
                                arguments: None,
                            }),
                        ],
                        field_accessors: vec![
                            FieldAccessor {
                                name: "meters",
                                swift_type: "Int",
                            },
                        ],
                        inline_fragment_accessors: vec![],
                        fragment_spreads: vec![],
                        initializer: Some(InitializerConfig {
                            parameters: vec![
                                InitParam {
                                    name: "meters",
                                    swift_type: "Int",
                                    default_value: None,
                                },
                            ],
                            data_entries: vec![
                                DataEntry {
                                    key: "__typename",
                                    value: DataEntryValue::Typename("AnimalKingdomAPI.Objects.Height.typename"),
                                },
                                DataEntry {
                                    key: "meters",
                                    value: DataEntryValue::Variable("meters"),
                                },
                            ],
                            fulfilled_fragments: vec!["HeightInMeters.Height"],
                            typename_value: TypenameValue::Fixed("AnimalKingdomAPI.Objects.Height.typename"),
                        }),
                        nested_types: vec![],
                        type_aliases: vec![],
                        indent: 2,
                        access_modifier: "public ",
                    },
                },
            ],
            type_aliases: vec![],
            indent: 0,
            access_modifier: "public ",
        },
    };

    let generated = fragment::render(&config);
    let expected = read_golden_fragment("HeightInMeters");
    assert_matches(&generated, &expected, "HeightInMeters");
}

#[test]
fn fragment_template_pet_details() {
    // PetDetails: fragment with nullable fields and nested entity
    let config = FragmentConfig {
        name: "PetDetails",
        fragment_definition: "fragment PetDetails on Pet { __typename humanName favoriteToy owner { __typename firstName } }",
        schema_namespace: "AnimalKingdomAPI",
        access_modifier: "public ",
        selection_set: SelectionSetConfig {
            struct_name: "PetDetails",
            schema_namespace: "AnimalKingdomAPI",
            parent_type: ParentTypeRef::Interface("Pet"),
            is_root: true,
            is_inline_fragment: false,
            conformance: SelectionSetConformance::Fragment,
            root_entity_type: None,
            merged_sources: vec![],
            selections: vec![
                SelectionItem::Field(FieldSelectionItem {
                    name: "__typename",
                    swift_type: "String",
                    arguments: None,
                }),
                SelectionItem::Field(FieldSelectionItem {
                    name: "humanName",
                    swift_type: "String?",
                    arguments: None,
                }),
                SelectionItem::Field(FieldSelectionItem {
                    name: "favoriteToy",
                    swift_type: "String",
                    arguments: None,
                }),
                SelectionItem::Field(FieldSelectionItem {
                    name: "owner",
                    swift_type: "Owner?",
                    arguments: None,
                }),
            ],
            field_accessors: vec![
                FieldAccessor {
                    name: "humanName",
                    swift_type: "String?",
                },
                FieldAccessor {
                    name: "favoriteToy",
                    swift_type: "String",
                },
                FieldAccessor {
                    name: "owner",
                    swift_type: "Owner?",
                },
            ],
            inline_fragment_accessors: vec![],
            fragment_spreads: vec![],
            initializer: Some(InitializerConfig {
                parameters: vec![
                    InitParam {
                        name: "__typename",
                        swift_type: "String",
                        default_value: None,
                    },
                    InitParam {
                        name: "humanName",
                        swift_type: "String?",
                        default_value: Some("nil"),
                    },
                    InitParam {
                        name: "favoriteToy",
                        swift_type: "String",
                        default_value: None,
                    },
                    InitParam {
                        name: "owner",
                        swift_type: "Owner?",
                        default_value: Some("nil"),
                    },
                ],
                data_entries: vec![
                    DataEntry {
                        key: "__typename",
                        value: DataEntryValue::Variable("__typename"),
                    },
                    DataEntry {
                        key: "humanName",
                        value: DataEntryValue::Variable("humanName"),
                    },
                    DataEntry {
                        key: "favoriteToy",
                        value: DataEntryValue::Variable("favoriteToy"),
                    },
                    DataEntry {
                        key: "owner",
                        value: DataEntryValue::FieldData("owner"),
                    },
                ],
                fulfilled_fragments: vec!["PetDetails"],
                typename_value: TypenameValue::Parameter,
            }),
            nested_types: vec![
                NestedSelectionSet {
                    doc_comment: "/// Owner",
                    parent_type_comment: "///\n  /// Parent Type: `Human`",
                    config: SelectionSetConfig {
                        struct_name: "Owner",
                        schema_namespace: "AnimalKingdomAPI",
                        parent_type: ParentTypeRef::Object("Human"),
                        is_root: false,
                        is_inline_fragment: false,
                        conformance: SelectionSetConformance::SelectionSet,
                        root_entity_type: None,
                        merged_sources: vec![],
                        selections: vec![
                            SelectionItem::Field(FieldSelectionItem {
                                name: "__typename",
                                swift_type: "String",
                                arguments: None,
                            }),
                            SelectionItem::Field(FieldSelectionItem {
                                name: "firstName",
                                swift_type: "String",
                                arguments: None,
                            }),
                        ],
                        field_accessors: vec![
                            FieldAccessor {
                                name: "firstName",
                                swift_type: "String",
                            },
                        ],
                        inline_fragment_accessors: vec![],
                        fragment_spreads: vec![],
                        initializer: Some(InitializerConfig {
                            parameters: vec![
                                InitParam {
                                    name: "firstName",
                                    swift_type: "String",
                                    default_value: None,
                                },
                            ],
                            data_entries: vec![
                                DataEntry {
                                    key: "__typename",
                                    value: DataEntryValue::Typename("AnimalKingdomAPI.Objects.Human.typename"),
                                },
                                DataEntry {
                                    key: "firstName",
                                    value: DataEntryValue::Variable("firstName"),
                                },
                            ],
                            fulfilled_fragments: vec!["PetDetails.Owner"],
                            typename_value: TypenameValue::Fixed("AnimalKingdomAPI.Objects.Human.typename"),
                        }),
                        nested_types: vec![],
                        type_aliases: vec![],
                        indent: 2,
                        access_modifier: "public ",
                    },
                },
            ],
            type_aliases: vec![],
            indent: 0,
            access_modifier: "public ",
        },
    };

    let generated = fragment::render(&config);
    let expected = read_golden_fragment("PetDetails");
    assert_matches(&generated, &expected, "PetDetails");
}

#[test]
fn fragment_template_crocodile_fragment() {
    // CrocodileFragment: fragment with field arguments
    let config = FragmentConfig {
        name: "CrocodileFragment",
        fragment_definition: r#"fragment CrocodileFragment on Crocodile { __typename species age tag(id: "albino") }"#,
        schema_namespace: "AnimalKingdomAPI",
        access_modifier: "public ",
        selection_set: SelectionSetConfig {
            struct_name: "CrocodileFragment",
            schema_namespace: "AnimalKingdomAPI",
            parent_type: ParentTypeRef::Object("Crocodile"),
            is_root: true,
            is_inline_fragment: false,
            conformance: SelectionSetConformance::Fragment,
            root_entity_type: None,
            merged_sources: vec![],
            selections: vec![
                SelectionItem::Field(FieldSelectionItem {
                    name: "__typename",
                    swift_type: "String",
                    arguments: None,
                }),
                SelectionItem::Field(FieldSelectionItem {
                    name: "species",
                    swift_type: "String",
                    arguments: None,
                }),
                SelectionItem::Field(FieldSelectionItem {
                    name: "age",
                    swift_type: "Int",
                    arguments: None,
                }),
                SelectionItem::Field(FieldSelectionItem {
                    name: "tag",
                    swift_type: "String?",
                    arguments: Some(r#"["id": "albino"]"#),
                }),
            ],
            field_accessors: vec![
                FieldAccessor {
                    name: "species",
                    swift_type: "String",
                },
                FieldAccessor {
                    name: "age",
                    swift_type: "Int",
                },
                FieldAccessor {
                    name: "tag",
                    swift_type: "String?",
                },
            ],
            inline_fragment_accessors: vec![],
            fragment_spreads: vec![],
            initializer: Some(InitializerConfig {
                parameters: vec![
                    InitParam {
                        name: "species",
                        swift_type: "String",
                        default_value: None,
                    },
                    InitParam {
                        name: "age",
                        swift_type: "Int",
                        default_value: None,
                    },
                    InitParam {
                        name: "tag",
                        swift_type: "String?",
                        default_value: Some("nil"),
                    },
                ],
                data_entries: vec![
                    DataEntry {
                        key: "__typename",
                        value: DataEntryValue::Typename("AnimalKingdomAPI.Objects.Crocodile.typename"),
                    },
                    DataEntry {
                        key: "species",
                        value: DataEntryValue::Variable("species"),
                    },
                    DataEntry {
                        key: "age",
                        value: DataEntryValue::Variable("age"),
                    },
                    DataEntry {
                        key: "tag",
                        value: DataEntryValue::Variable("tag"),
                    },
                ],
                fulfilled_fragments: vec!["CrocodileFragment"],
                typename_value: TypenameValue::Fixed("AnimalKingdomAPI.Objects.Crocodile.typename"),
            }),
            nested_types: vec![],
            type_aliases: vec![],
            indent: 0,
            access_modifier: "public ",
        },
    };

    let generated = fragment::render(&config);
    let expected = read_golden_fragment("CrocodileFragment");
    assert_matches(&generated, &expected, "CrocodileFragment");
}

#[test]
fn fragment_template_warm_blooded_details() {
    // WarmBloodedDetails: fragment with fragment spread and type alias
    let config = FragmentConfig {
        name: "WarmBloodedDetails",
        fragment_definition: "fragment WarmBloodedDetails on WarmBlooded { __typename bodyTemperature ...HeightInMeters }",
        schema_namespace: "AnimalKingdomAPI",
        access_modifier: "public ",
        selection_set: SelectionSetConfig {
            struct_name: "WarmBloodedDetails",
            schema_namespace: "AnimalKingdomAPI",
            parent_type: ParentTypeRef::Interface("WarmBlooded"),
            is_root: true,
            is_inline_fragment: false,
            conformance: SelectionSetConformance::Fragment,
            root_entity_type: None,
            merged_sources: vec![],
            selections: vec![
                SelectionItem::Field(FieldSelectionItem {
                    name: "__typename",
                    swift_type: "String",
                    arguments: None,
                }),
                SelectionItem::Field(FieldSelectionItem {
                    name: "bodyTemperature",
                    swift_type: "Int",
                    arguments: None,
                }),
                SelectionItem::Fragment("HeightInMeters"),
            ],
            field_accessors: vec![
                FieldAccessor {
                    name: "bodyTemperature",
                    swift_type: "Int",
                },
                FieldAccessor {
                    name: "height",
                    swift_type: "Height",
                },
            ],
            inline_fragment_accessors: vec![],
            fragment_spreads: vec![
                FragmentSpreadAccessor {
                    property_name: "heightInMeters",
                    fragment_type: "HeightInMeters",
                },
            ],
            initializer: Some(InitializerConfig {
                parameters: vec![
                    InitParam {
                        name: "__typename",
                        swift_type: "String",
                        default_value: None,
                    },
                    InitParam {
                        name: "bodyTemperature",
                        swift_type: "Int",
                        default_value: None,
                    },
                    InitParam {
                        name: "height",
                        swift_type: "Height",
                        default_value: None,
                    },
                ],
                data_entries: vec![
                    DataEntry {
                        key: "__typename",
                        value: DataEntryValue::Variable("__typename"),
                    },
                    DataEntry {
                        key: "bodyTemperature",
                        value: DataEntryValue::Variable("bodyTemperature"),
                    },
                    DataEntry {
                        key: "height",
                        value: DataEntryValue::FieldData("height"),
                    },
                ],
                fulfilled_fragments: vec![
                    "WarmBloodedDetails",
                    "HeightInMeters",
                ],
                typename_value: TypenameValue::Parameter,
            }),
            nested_types: vec![],
            type_aliases: vec![
                TypeAliasConfig {
                    name: "Height",
                    target: "HeightInMeters.Height",
                },
            ],
            indent: 0,
            access_modifier: "public ",
        },
    };

    let generated = fragment::render(&config);
    let expected = read_golden_fragment("WarmBloodedDetails");
    assert_matches(&generated, &expected, "WarmBloodedDetails");
}

#[test]
fn fragment_template_classroom_pet_details() {
    // ClassroomPetDetails: complex fragment with many inline fragments on a union type
    let config = FragmentConfig {
        name: "ClassroomPetDetails",
        fragment_definition: r#"fragment ClassroomPetDetails on ClassroomPet { __typename ... on Animal { species } ... on Pet { humanName } ... on WarmBlooded { laysEggs } ... on Cat { bodyTemperature isJellicle } ... on Bird { wingspan } ... on PetRock { favoriteToy } }"#,
        schema_namespace: "AnimalKingdomAPI",
        access_modifier: "public ",
        selection_set: SelectionSetConfig {
            struct_name: "ClassroomPetDetails",
            schema_namespace: "AnimalKingdomAPI",
            parent_type: ParentTypeRef::Union("ClassroomPet"),
            is_root: true,
            is_inline_fragment: false,
            conformance: SelectionSetConformance::Fragment,
            root_entity_type: None,
            merged_sources: vec![],
            selections: vec![
                SelectionItem::Field(FieldSelectionItem {
                    name: "__typename",
                    swift_type: "String",
                    arguments: None,
                }),
                SelectionItem::InlineFragment("AsAnimal"),
                SelectionItem::InlineFragment("AsPet"),
                SelectionItem::InlineFragment("AsWarmBlooded"),
                SelectionItem::InlineFragment("AsCat"),
                SelectionItem::InlineFragment("AsBird"),
                SelectionItem::InlineFragment("AsPetRock"),
            ],
            field_accessors: vec![],
            inline_fragment_accessors: vec![
                InlineFragmentAccessor { property_name: "asAnimal", type_name: "AsAnimal" },
                InlineFragmentAccessor { property_name: "asPet", type_name: "AsPet" },
                InlineFragmentAccessor { property_name: "asWarmBlooded", type_name: "AsWarmBlooded" },
                InlineFragmentAccessor { property_name: "asCat", type_name: "AsCat" },
                InlineFragmentAccessor { property_name: "asBird", type_name: "AsBird" },
                InlineFragmentAccessor { property_name: "asPetRock", type_name: "AsPetRock" },
            ],
            fragment_spreads: vec![],
            initializer: Some(InitializerConfig {
                parameters: vec![
                    InitParam { name: "__typename", swift_type: "String", default_value: None },
                ],
                data_entries: vec![
                    DataEntry { key: "__typename", value: DataEntryValue::Variable("__typename") },
                ],
                fulfilled_fragments: vec!["ClassroomPetDetails"],
                typename_value: TypenameValue::Parameter,
            }),
            nested_types: vec![
                // AsAnimal
                NestedSelectionSet {
                    doc_comment: "/// AsAnimal",
                    parent_type_comment: "///\n  /// Parent Type: `Animal`",
                    config: SelectionSetConfig {
                        struct_name: "AsAnimal",
                        schema_namespace: "AnimalKingdomAPI",
                        parent_type: ParentTypeRef::Interface("Animal"),
                        is_root: false,
                        is_inline_fragment: true,
                        conformance: SelectionSetConformance::InlineFragment,
                        root_entity_type: Some("ClassroomPetDetails"),
                        merged_sources: vec![],
                        selections: vec![
                            SelectionItem::Field(FieldSelectionItem {
                                name: "species",
                                swift_type: "String",
                                arguments: None,
                            }),
                        ],
                        field_accessors: vec![
                            FieldAccessor { name: "species", swift_type: "String" },
                        ],
                        inline_fragment_accessors: vec![],
                        fragment_spreads: vec![],
                        initializer: Some(InitializerConfig {
                            parameters: vec![
                                InitParam { name: "__typename", swift_type: "String", default_value: None },
                                InitParam { name: "species", swift_type: "String", default_value: None },
                            ],
                            data_entries: vec![
                                DataEntry { key: "__typename", value: DataEntryValue::Variable("__typename") },
                                DataEntry { key: "species", value: DataEntryValue::Variable("species") },
                            ],
                            fulfilled_fragments: vec![
                                "ClassroomPetDetails",
                                "ClassroomPetDetails.AsAnimal",
                            ],
                            typename_value: TypenameValue::Parameter,
                        }),
                        nested_types: vec![],
                        type_aliases: vec![],
                        indent: 2,
                        access_modifier: "public ",
                    },
                },
                // AsPet
                NestedSelectionSet {
                    doc_comment: "/// AsPet",
                    parent_type_comment: "///\n  /// Parent Type: `Pet`",
                    config: SelectionSetConfig {
                        struct_name: "AsPet",
                        schema_namespace: "AnimalKingdomAPI",
                        parent_type: ParentTypeRef::Interface("Pet"),
                        is_root: false,
                        is_inline_fragment: true,
                        conformance: SelectionSetConformance::InlineFragment,
                        root_entity_type: Some("ClassroomPetDetails"),
                        merged_sources: vec![],
                        selections: vec![
                            SelectionItem::Field(FieldSelectionItem {
                                name: "humanName",
                                swift_type: "String?",
                                arguments: None,
                            }),
                        ],
                        field_accessors: vec![
                            FieldAccessor { name: "humanName", swift_type: "String?" },
                        ],
                        inline_fragment_accessors: vec![],
                        fragment_spreads: vec![],
                        initializer: Some(InitializerConfig {
                            parameters: vec![
                                InitParam { name: "__typename", swift_type: "String", default_value: None },
                                InitParam { name: "humanName", swift_type: "String?", default_value: Some("nil") },
                            ],
                            data_entries: vec![
                                DataEntry { key: "__typename", value: DataEntryValue::Variable("__typename") },
                                DataEntry { key: "humanName", value: DataEntryValue::Variable("humanName") },
                            ],
                            fulfilled_fragments: vec![
                                "ClassroomPetDetails",
                                "ClassroomPetDetails.AsPet",
                            ],
                            typename_value: TypenameValue::Parameter,
                        }),
                        nested_types: vec![],
                        type_aliases: vec![],
                        indent: 2,
                        access_modifier: "public ",
                    },
                },
                // AsWarmBlooded
                NestedSelectionSet {
                    doc_comment: "/// AsWarmBlooded",
                    parent_type_comment: "///\n  /// Parent Type: `WarmBlooded`",
                    config: SelectionSetConfig {
                        struct_name: "AsWarmBlooded",
                        schema_namespace: "AnimalKingdomAPI",
                        parent_type: ParentTypeRef::Interface("WarmBlooded"),
                        is_root: false,
                        is_inline_fragment: true,
                        conformance: SelectionSetConformance::InlineFragment,
                        root_entity_type: Some("ClassroomPetDetails"),
                        merged_sources: vec![],
                        selections: vec![
                            SelectionItem::Field(FieldSelectionItem {
                                name: "laysEggs",
                                swift_type: "Bool",
                                arguments: None,
                            }),
                        ],
                        field_accessors: vec![
                            FieldAccessor { name: "laysEggs", swift_type: "Bool" },
                            FieldAccessor { name: "species", swift_type: "String" },
                        ],
                        inline_fragment_accessors: vec![],
                        fragment_spreads: vec![],
                        initializer: Some(InitializerConfig {
                            parameters: vec![
                                InitParam { name: "__typename", swift_type: "String", default_value: None },
                                InitParam { name: "laysEggs", swift_type: "Bool", default_value: None },
                                InitParam { name: "species", swift_type: "String", default_value: None },
                            ],
                            data_entries: vec![
                                DataEntry { key: "__typename", value: DataEntryValue::Variable("__typename") },
                                DataEntry { key: "laysEggs", value: DataEntryValue::Variable("laysEggs") },
                                DataEntry { key: "species", value: DataEntryValue::Variable("species") },
                            ],
                            fulfilled_fragments: vec![
                                "ClassroomPetDetails",
                                "ClassroomPetDetails.AsWarmBlooded",
                                "ClassroomPetDetails.AsAnimal",
                            ],
                            typename_value: TypenameValue::Parameter,
                        }),
                        nested_types: vec![],
                        type_aliases: vec![],
                        indent: 2,
                        access_modifier: "public ",
                    },
                },
                // AsCat
                NestedSelectionSet {
                    doc_comment: "/// AsCat",
                    parent_type_comment: "///\n  /// Parent Type: `Cat`",
                    config: SelectionSetConfig {
                        struct_name: "AsCat",
                        schema_namespace: "AnimalKingdomAPI",
                        parent_type: ParentTypeRef::Object("Cat"),
                        is_root: false,
                        is_inline_fragment: true,
                        conformance: SelectionSetConformance::InlineFragment,
                        root_entity_type: Some("ClassroomPetDetails"),
                        merged_sources: vec![],
                        selections: vec![
                            SelectionItem::Field(FieldSelectionItem {
                                name: "bodyTemperature",
                                swift_type: "Int",
                                arguments: None,
                            }),
                            SelectionItem::Field(FieldSelectionItem {
                                name: "isJellicle",
                                swift_type: "Bool",
                                arguments: None,
                            }),
                        ],
                        field_accessors: vec![
                            FieldAccessor { name: "bodyTemperature", swift_type: "Int" },
                            FieldAccessor { name: "isJellicle", swift_type: "Bool" },
                            FieldAccessor { name: "species", swift_type: "String" },
                            FieldAccessor { name: "humanName", swift_type: "String?" },
                            FieldAccessor { name: "laysEggs", swift_type: "Bool" },
                        ],
                        inline_fragment_accessors: vec![],
                        fragment_spreads: vec![],
                        initializer: Some(InitializerConfig {
                            parameters: vec![
                                InitParam { name: "bodyTemperature", swift_type: "Int", default_value: None },
                                InitParam { name: "isJellicle", swift_type: "Bool", default_value: None },
                                InitParam { name: "species", swift_type: "String", default_value: None },
                                InitParam { name: "humanName", swift_type: "String?", default_value: Some("nil") },
                                InitParam { name: "laysEggs", swift_type: "Bool", default_value: None },
                            ],
                            data_entries: vec![
                                DataEntry { key: "__typename", value: DataEntryValue::Typename("AnimalKingdomAPI.Objects.Cat.typename") },
                                DataEntry { key: "bodyTemperature", value: DataEntryValue::Variable("bodyTemperature") },
                                DataEntry { key: "isJellicle", value: DataEntryValue::Variable("isJellicle") },
                                DataEntry { key: "species", value: DataEntryValue::Variable("species") },
                                DataEntry { key: "humanName", value: DataEntryValue::Variable("humanName") },
                                DataEntry { key: "laysEggs", value: DataEntryValue::Variable("laysEggs") },
                            ],
                            fulfilled_fragments: vec![
                                "ClassroomPetDetails",
                                "ClassroomPetDetails.AsCat",
                                "ClassroomPetDetails.AsAnimal",
                                "ClassroomPetDetails.AsPet",
                                "ClassroomPetDetails.AsWarmBlooded",
                            ],
                            typename_value: TypenameValue::Fixed("AnimalKingdomAPI.Objects.Cat.typename"),
                        }),
                        nested_types: vec![],
                        type_aliases: vec![],
                        indent: 2,
                        access_modifier: "public ",
                    },
                },
                // AsBird
                NestedSelectionSet {
                    doc_comment: "/// AsBird",
                    parent_type_comment: "///\n  /// Parent Type: `Bird`",
                    config: SelectionSetConfig {
                        struct_name: "AsBird",
                        schema_namespace: "AnimalKingdomAPI",
                        parent_type: ParentTypeRef::Object("Bird"),
                        is_root: false,
                        is_inline_fragment: true,
                        conformance: SelectionSetConformance::InlineFragment,
                        root_entity_type: Some("ClassroomPetDetails"),
                        merged_sources: vec![],
                        selections: vec![
                            SelectionItem::Field(FieldSelectionItem {
                                name: "wingspan",
                                swift_type: "Double",
                                arguments: None,
                            }),
                        ],
                        field_accessors: vec![
                            FieldAccessor { name: "wingspan", swift_type: "Double" },
                            FieldAccessor { name: "species", swift_type: "String" },
                            FieldAccessor { name: "humanName", swift_type: "String?" },
                            FieldAccessor { name: "laysEggs", swift_type: "Bool" },
                        ],
                        inline_fragment_accessors: vec![],
                        fragment_spreads: vec![],
                        initializer: Some(InitializerConfig {
                            parameters: vec![
                                InitParam { name: "wingspan", swift_type: "Double", default_value: None },
                                InitParam { name: "species", swift_type: "String", default_value: None },
                                InitParam { name: "humanName", swift_type: "String?", default_value: Some("nil") },
                                InitParam { name: "laysEggs", swift_type: "Bool", default_value: None },
                            ],
                            data_entries: vec![
                                DataEntry { key: "__typename", value: DataEntryValue::Typename("AnimalKingdomAPI.Objects.Bird.typename") },
                                DataEntry { key: "wingspan", value: DataEntryValue::Variable("wingspan") },
                                DataEntry { key: "species", value: DataEntryValue::Variable("species") },
                                DataEntry { key: "humanName", value: DataEntryValue::Variable("humanName") },
                                DataEntry { key: "laysEggs", value: DataEntryValue::Variable("laysEggs") },
                            ],
                            fulfilled_fragments: vec![
                                "ClassroomPetDetails",
                                "ClassroomPetDetails.AsBird",
                                "ClassroomPetDetails.AsAnimal",
                                "ClassroomPetDetails.AsPet",
                                "ClassroomPetDetails.AsWarmBlooded",
                            ],
                            typename_value: TypenameValue::Fixed("AnimalKingdomAPI.Objects.Bird.typename"),
                        }),
                        nested_types: vec![],
                        type_aliases: vec![],
                        indent: 2,
                        access_modifier: "public ",
                    },
                },
                // AsPetRock
                NestedSelectionSet {
                    doc_comment: "/// AsPetRock",
                    parent_type_comment: "///\n  /// Parent Type: `PetRock`",
                    config: SelectionSetConfig {
                        struct_name: "AsPetRock",
                        schema_namespace: "AnimalKingdomAPI",
                        parent_type: ParentTypeRef::Object("PetRock"),
                        is_root: false,
                        is_inline_fragment: true,
                        conformance: SelectionSetConformance::InlineFragment,
                        root_entity_type: Some("ClassroomPetDetails"),
                        merged_sources: vec![],
                        selections: vec![
                            SelectionItem::Field(FieldSelectionItem {
                                name: "favoriteToy",
                                swift_type: "String",
                                arguments: None,
                            }),
                        ],
                        field_accessors: vec![
                            FieldAccessor { name: "favoriteToy", swift_type: "String" },
                            FieldAccessor { name: "humanName", swift_type: "String?" },
                        ],
                        inline_fragment_accessors: vec![],
                        fragment_spreads: vec![],
                        initializer: Some(InitializerConfig {
                            parameters: vec![
                                InitParam { name: "favoriteToy", swift_type: "String", default_value: None },
                                InitParam { name: "humanName", swift_type: "String?", default_value: Some("nil") },
                            ],
                            data_entries: vec![
                                DataEntry { key: "__typename", value: DataEntryValue::Typename("AnimalKingdomAPI.Objects.PetRock.typename") },
                                DataEntry { key: "favoriteToy", value: DataEntryValue::Variable("favoriteToy") },
                                DataEntry { key: "humanName", value: DataEntryValue::Variable("humanName") },
                            ],
                            fulfilled_fragments: vec![
                                "ClassroomPetDetails",
                                "ClassroomPetDetails.AsPetRock",
                                "ClassroomPetDetails.AsPet",
                            ],
                            typename_value: TypenameValue::Fixed("AnimalKingdomAPI.Objects.PetRock.typename"),
                        }),
                        nested_types: vec![],
                        type_aliases: vec![],
                        indent: 2,
                        access_modifier: "public ",
                    },
                },
            ],
            type_aliases: vec![],
            indent: 0,
            access_modifier: "public ",
        },
    };

    let generated = fragment::render(&config);
    let expected = read_golden_fragment("ClassroomPetDetails");
    assert_matches(&generated, &expected, "ClassroomPetDetails");
}

// ============================================================================
// Operation Tests
// ============================================================================

#[test]
fn operation_template_dog_query() {
    // DogQuery: query with inline fragment and fragment spread
    let config = OperationConfig {
        class_name: "DogQuery",
        operation_name: "DogQuery",
        operation_type: OperationType::Query,
        schema_namespace: "AnimalKingdomAPI",
        access_modifier: "public ",
        source: "query DogQuery { allAnimals { __typename id skinCovering ... on Dog { ...DogFragment houseDetails } } }",
        fragment_names: vec!["DogFragment"],
        variables: vec![],
        data_selection_set: SelectionSetConfig {
            struct_name: "Data",
            schema_namespace: "AnimalKingdomAPI",
            parent_type: ParentTypeRef::Object("Query"),
            is_root: true,
            is_inline_fragment: false,
            conformance: SelectionSetConformance::SelectionSet,
            root_entity_type: None,
            merged_sources: vec![],
            selections: vec![
                SelectionItem::Field(FieldSelectionItem {
                    name: "allAnimals",
                    swift_type: "[AllAnimal]",
                    arguments: None,
                }),
            ],
            field_accessors: vec![
                FieldAccessor {
                    name: "allAnimals",
                    swift_type: "[AllAnimal]",
                },
            ],
            inline_fragment_accessors: vec![],
            fragment_spreads: vec![],
            initializer: Some(InitializerConfig {
                parameters: vec![
                    InitParam {
                        name: "allAnimals",
                        swift_type: "[AllAnimal]",
                        default_value: None,
                    },
                ],
                data_entries: vec![
                    DataEntry {
                        key: "__typename",
                        value: DataEntryValue::Typename("AnimalKingdomAPI.Objects.Query.typename"),
                    },
                    DataEntry {
                        key: "allAnimals",
                        value: DataEntryValue::FieldData("allAnimals"),
                    },
                ],
                fulfilled_fragments: vec!["DogQuery.Data"],
                typename_value: TypenameValue::Fixed("AnimalKingdomAPI.Objects.Query.typename"),
            }),
            nested_types: vec![
                NestedSelectionSet {
                    doc_comment: "/// AllAnimal",
                    parent_type_comment: "///\n    /// Parent Type: `Animal`",
                    config: SelectionSetConfig {
                        struct_name: "AllAnimal",
                        schema_namespace: "AnimalKingdomAPI",
                        parent_type: ParentTypeRef::Interface("Animal"),
                        is_root: false,
                        is_inline_fragment: false,
                        conformance: SelectionSetConformance::SelectionSet,
                        root_entity_type: None,
                        merged_sources: vec![],
                        selections: vec![
                            SelectionItem::Field(FieldSelectionItem {
                                name: "__typename",
                                swift_type: "String",
                                arguments: None,
                            }),
                            SelectionItem::Field(FieldSelectionItem {
                                name: "id",
                                swift_type: "AnimalKingdomAPI.ID",
                                arguments: None,
                            }),
                            SelectionItem::Field(FieldSelectionItem {
                                name: "skinCovering",
                                swift_type: "GraphQLEnum<AnimalKingdomAPI.SkinCovering>?",
                                arguments: None,
                            }),
                            SelectionItem::InlineFragment("AsDog"),
                        ],
                        field_accessors: vec![
                            FieldAccessor {
                                name: "id",
                                swift_type: "AnimalKingdomAPI.ID",
                            },
                            FieldAccessor {
                                name: "skinCovering",
                                swift_type: "GraphQLEnum<AnimalKingdomAPI.SkinCovering>?",
                            },
                        ],
                        inline_fragment_accessors: vec![
                            InlineFragmentAccessor {
                                property_name: "asDog",
                                type_name: "AsDog",
                            },
                        ],
                        fragment_spreads: vec![],
                        initializer: Some(InitializerConfig {
                            parameters: vec![
                                InitParam {
                                    name: "__typename",
                                    swift_type: "String",
                                    default_value: None,
                                },
                                InitParam {
                                    name: "id",
                                    swift_type: "AnimalKingdomAPI.ID",
                                    default_value: None,
                                },
                                InitParam {
                                    name: "skinCovering",
                                    swift_type: "GraphQLEnum<AnimalKingdomAPI.SkinCovering>?",
                                    default_value: Some("nil"),
                                },
                            ],
                            data_entries: vec![
                                DataEntry {
                                    key: "__typename",
                                    value: DataEntryValue::Variable("__typename"),
                                },
                                DataEntry {
                                    key: "id",
                                    value: DataEntryValue::Variable("id"),
                                },
                                DataEntry {
                                    key: "skinCovering",
                                    value: DataEntryValue::Variable("skinCovering"),
                                },
                            ],
                            fulfilled_fragments: vec!["DogQuery.Data.AllAnimal"],
                            typename_value: TypenameValue::Parameter,
                        }),
                        nested_types: vec![
                            NestedSelectionSet {
                                doc_comment: "/// AllAnimal.AsDog",
                                parent_type_comment: "///\n      /// Parent Type: `Dog`",
                                config: SelectionSetConfig {
                                    struct_name: "AsDog",
                                    schema_namespace: "AnimalKingdomAPI",
                                    parent_type: ParentTypeRef::Object("Dog"),
                                    is_root: false,
                                    is_inline_fragment: true,
                                    conformance: SelectionSetConformance::InlineFragment,
                                    root_entity_type: Some("DogQuery.Data.AllAnimal"),
                                    merged_sources: vec![],
                                    selections: vec![
                                        SelectionItem::Field(FieldSelectionItem {
                                            name: "houseDetails",
                                            swift_type: "AnimalKingdomAPI.Object?",
                                            arguments: None,
                                        }),
                                        SelectionItem::Fragment("DogFragment"),
                                    ],
                                    field_accessors: vec![
                                        FieldAccessor {
                                            name: "houseDetails",
                                            swift_type: "AnimalKingdomAPI.Object?",
                                        },
                                        FieldAccessor {
                                            name: "id",
                                            swift_type: "AnimalKingdomAPI.ID",
                                        },
                                        FieldAccessor {
                                            name: "skinCovering",
                                            swift_type: "GraphQLEnum<AnimalKingdomAPI.SkinCovering>?",
                                        },
                                        FieldAccessor {
                                            name: "species",
                                            swift_type: "String",
                                        },
                                    ],
                                    inline_fragment_accessors: vec![],
                                    fragment_spreads: vec![
                                        FragmentSpreadAccessor {
                                            property_name: "dogFragment",
                                            fragment_type: "DogFragment",
                                        },
                                    ],
                                    initializer: Some(InitializerConfig {
                                        parameters: vec![
                                            InitParam {
                                                name: "houseDetails",
                                                swift_type: "AnimalKingdomAPI.Object?",
                                                default_value: Some("nil"),
                                            },
                                            InitParam {
                                                name: "id",
                                                swift_type: "AnimalKingdomAPI.ID",
                                                default_value: None,
                                            },
                                            InitParam {
                                                name: "skinCovering",
                                                swift_type: "GraphQLEnum<AnimalKingdomAPI.SkinCovering>?",
                                                default_value: Some("nil"),
                                            },
                                            InitParam {
                                                name: "species",
                                                swift_type: "String",
                                                default_value: None,
                                            },
                                        ],
                                        data_entries: vec![
                                            DataEntry {
                                                key: "__typename",
                                                value: DataEntryValue::Typename("AnimalKingdomAPI.Objects.Dog.typename"),
                                            },
                                            DataEntry {
                                                key: "houseDetails",
                                                value: DataEntryValue::Variable("houseDetails"),
                                            },
                                            DataEntry {
                                                key: "id",
                                                value: DataEntryValue::Variable("id"),
                                            },
                                            DataEntry {
                                                key: "skinCovering",
                                                value: DataEntryValue::Variable("skinCovering"),
                                            },
                                            DataEntry {
                                                key: "species",
                                                value: DataEntryValue::Variable("species"),
                                            },
                                        ],
                                        fulfilled_fragments: vec![
                                            "DogQuery.Data.AllAnimal",
                                            "DogQuery.Data.AllAnimal.AsDog",
                                            "DogFragment",
                                        ],
                                        typename_value: TypenameValue::Fixed("AnimalKingdomAPI.Objects.Dog.typename"),
                                    }),
                                    nested_types: vec![],
                                    type_aliases: vec![],
                                    indent: 6,
                                    access_modifier: "public ",
                                },
                            },
                        ],
                        type_aliases: vec![],
                        indent: 4,
                        access_modifier: "public ",
                    },
                },
            ],
            type_aliases: vec![],
            indent: 2,
            access_modifier: "public ",
        },
    };

    let generated = operation::render(&config);
    let expected = read_golden_operation("Queries", "DogQuery");
    assert_matches(&generated, &expected, "DogQuery");
}

#[test]
fn operation_template_pet_adoption_mutation() {
    // PetAdoptionMutation: mutation with variables
    let config = OperationConfig {
        class_name: "PetAdoptionMutation",
        operation_name: "PetAdoptionMutation",
        operation_type: OperationType::Mutation,
        schema_namespace: "AnimalKingdomAPI",
        access_modifier: "public ",
        source: "mutation PetAdoptionMutation($input: PetAdoptionInput!) { adoptPet(input: $input) { __typename id humanName } }",
        fragment_names: vec![],
        variables: vec![
            VariableConfig {
                name: "input",
                swift_type: "PetAdoptionInput",
                default_value: None,
            },
        ],
        data_selection_set: SelectionSetConfig {
            struct_name: "Data",
            schema_namespace: "AnimalKingdomAPI",
            parent_type: ParentTypeRef::Object("Mutation"),
            is_root: true,
            is_inline_fragment: false,
            conformance: SelectionSetConformance::SelectionSet,
            root_entity_type: None,
            merged_sources: vec![],
            selections: vec![
                SelectionItem::Field(FieldSelectionItem {
                    name: "adoptPet",
                    swift_type: "AdoptPet",
                    arguments: Some(r#"["input": .variable("input")]"#),
                }),
            ],
            field_accessors: vec![
                FieldAccessor {
                    name: "adoptPet",
                    swift_type: "AdoptPet",
                },
            ],
            inline_fragment_accessors: vec![],
            fragment_spreads: vec![],
            initializer: Some(InitializerConfig {
                parameters: vec![
                    InitParam {
                        name: "adoptPet",
                        swift_type: "AdoptPet",
                        default_value: None,
                    },
                ],
                data_entries: vec![
                    DataEntry {
                        key: "__typename",
                        value: DataEntryValue::Typename("AnimalKingdomAPI.Objects.Mutation.typename"),
                    },
                    DataEntry {
                        key: "adoptPet",
                        value: DataEntryValue::FieldData("adoptPet"),
                    },
                ],
                fulfilled_fragments: vec!["PetAdoptionMutation.Data"],
                typename_value: TypenameValue::Fixed("AnimalKingdomAPI.Objects.Mutation.typename"),
            }),
            nested_types: vec![
                NestedSelectionSet {
                    doc_comment: "/// AdoptPet",
                    parent_type_comment: "///\n    /// Parent Type: `Pet`",
                    config: SelectionSetConfig {
                        struct_name: "AdoptPet",
                        schema_namespace: "AnimalKingdomAPI",
                        parent_type: ParentTypeRef::Interface("Pet"),
                        is_root: false,
                        is_inline_fragment: false,
                        conformance: SelectionSetConformance::SelectionSet,
                        root_entity_type: None,
                        merged_sources: vec![],
                        selections: vec![
                            SelectionItem::Field(FieldSelectionItem {
                                name: "__typename",
                                swift_type: "String",
                                arguments: None,
                            }),
                            SelectionItem::Field(FieldSelectionItem {
                                name: "id",
                                swift_type: "AnimalKingdomAPI.ID",
                                arguments: None,
                            }),
                            SelectionItem::Field(FieldSelectionItem {
                                name: "humanName",
                                swift_type: "String?",
                                arguments: None,
                            }),
                        ],
                        field_accessors: vec![
                            FieldAccessor {
                                name: "id",
                                swift_type: "AnimalKingdomAPI.ID",
                            },
                            FieldAccessor {
                                name: "humanName",
                                swift_type: "String?",
                            },
                        ],
                        inline_fragment_accessors: vec![],
                        fragment_spreads: vec![],
                        initializer: Some(InitializerConfig {
                            parameters: vec![
                                InitParam {
                                    name: "__typename",
                                    swift_type: "String",
                                    default_value: None,
                                },
                                InitParam {
                                    name: "id",
                                    swift_type: "AnimalKingdomAPI.ID",
                                    default_value: None,
                                },
                                InitParam {
                                    name: "humanName",
                                    swift_type: "String?",
                                    default_value: Some("nil"),
                                },
                            ],
                            data_entries: vec![
                                DataEntry {
                                    key: "__typename",
                                    value: DataEntryValue::Variable("__typename"),
                                },
                                DataEntry {
                                    key: "id",
                                    value: DataEntryValue::Variable("id"),
                                },
                                DataEntry {
                                    key: "humanName",
                                    value: DataEntryValue::Variable("humanName"),
                                },
                            ],
                            fulfilled_fragments: vec!["PetAdoptionMutation.Data.AdoptPet"],
                            typename_value: TypenameValue::Parameter,
                        }),
                        nested_types: vec![],
                        type_aliases: vec![],
                        indent: 4,
                        access_modifier: "public ",
                    },
                },
            ],
            type_aliases: vec![],
            indent: 2,
            access_modifier: "public ",
        },
    };

    let generated = operation::render(&config);
    let expected = read_golden_operation("Mutations", "PetAdoptionMutation");
    assert_matches(&generated, &expected, "PetAdoptionMutation");
}
