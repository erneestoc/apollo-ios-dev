//! Tests that we can parse all the real apollo-codegen-config.json files.

use apollo_codegen_config::ApolloCodegenConfiguration;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .unwrap()
        .parent() // apollo-codegen-rs/
        .unwrap()
        .parent() // repo root
        .unwrap()
        .to_path_buf()
}

fn parse_config(relative_path: &str) -> ApolloCodegenConfiguration {
    let path = repo_root().join(relative_path);
    ApolloCodegenConfiguration::from_file(&path)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", relative_path, e))
}

#[test]
fn parse_swift_package_manager_config() {
    let config = parse_config(
        "Tests/TestCodeGenConfigurations/SwiftPackageManager/apollo-codegen-config.json",
    );
    assert_eq!(config.schema_namespace, "AnimalKingdomAPI");
    assert!(matches!(
        config.output.schema_types.module_type,
        apollo_codegen_config::types::SchemaModuleType::SwiftPackageManager(_)
    ));
    assert!(matches!(
        config.output.operations,
        apollo_codegen_config::types::OperationsFileOutput::InSchemaModule(_)
    ));
    assert!(matches!(
        config.output.test_mocks,
        apollo_codegen_config::types::TestMockFileOutput::SwiftPackage(_)
    ));
    // Check schema customization
    assert!(!config.options.schema_customization.custom_type_names.is_empty());
}

#[test]
fn parse_embedded_in_target_in_schema_module_config() {
    let config = parse_config(
        "Tests/TestCodeGenConfigurations/EmbeddedInTarget-InSchemaModule/apollo-codegen-config.json",
    );
    assert_eq!(config.schema_namespace, "AnimalKingdomAPI");
    match &config.output.schema_types.module_type {
        apollo_codegen_config::types::SchemaModuleType::EmbeddedInTarget(c) => {
            assert_eq!(c.name, "TestApp");
            assert_eq!(c.access_modifier, apollo_codegen_config::types::AccessModifier::Public);
        }
        _ => panic!("Expected EmbeddedInTarget"),
    }
}

#[test]
fn parse_embedded_in_target_relative_absolute_config() {
    let config = parse_config(
        "Tests/TestCodeGenConfigurations/EmbeddedInTarget-RelativeAbsolute/apollo-codegen-config.json",
    );
    assert_eq!(config.schema_namespace, "MySchemaModule");
    match &config.output.operations {
        apollo_codegen_config::types::OperationsFileOutput::Relative(c) => {
            assert!(c.subpath.is_none());
        }
        _ => panic!("Expected Relative operations"),
    }
}

#[test]
fn parse_other_custom_target_config() {
    let config = parse_config(
        "Tests/TestCodeGenConfigurations/Other-CustomTarget/apollo-codegen-config.json",
    );
    assert!(matches!(
        config.output.schema_types.module_type,
        apollo_codegen_config::types::SchemaModuleType::Other(_)
    ));
}

#[test]
fn parse_codegen_xcframework_config() {
    let config = parse_config(
        "Tests/TestCodeGenConfigurations/CodegenXCFramework/apollo-codegen-config.json",
    );
    assert_eq!(config.schema_namespace, "MyAPI");
}

#[test]
fn parse_spm_in_xcode_project_config() {
    let config = parse_config(
        "Tests/TestCodeGenConfigurations/SPMInXcodeProject/apollo-codegen-config.json",
    );
    assert_eq!(config.schema_namespace, "AnimalKingdomAPI");
}

#[test]
fn parse_all_configs_without_error() {
    let configs = [
        "Tests/TestCodeGenConfigurations/SwiftPackageManager/apollo-codegen-config.json",
        "Tests/TestCodeGenConfigurations/EmbeddedInTarget-InSchemaModule/apollo-codegen-config.json",
        "Tests/TestCodeGenConfigurations/EmbeddedInTarget-RelativeAbsolute/apollo-codegen-config.json",
        "Tests/TestCodeGenConfigurations/Other-CustomTarget/apollo-codegen-config.json",
        "Tests/TestCodeGenConfigurations/CodegenXCFramework/apollo-codegen-config.json",
        "Tests/TestCodeGenConfigurations/SPMInXcodeProject/apollo-codegen-config.json",
        "Tests/TestCodeGenConfigurations/Other-CocoaPods/apollo-codegen-config.json",
    ];

    for config_path in configs {
        let path = repo_root().join(config_path);
        if path.exists() {
            let result = ApolloCodegenConfiguration::from_file(&path);
            assert!(
                result.is_ok(),
                "Failed to parse {}: {:?}",
                config_path,
                result.err()
            );
            println!("OK: {}", config_path);
        } else {
            println!("SKIP (not found): {}", config_path);
        }
    }
}
