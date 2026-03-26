use crate::ModuleType;
use std::path::Path;

/// Default file path for the configuration file.
const DEFAULT_CONFIG_PATH: &str = "./apollo-codegen-config.json";

/// Build the minimal configuration JSON for the given parameters.
///
/// This produces the same JSON structure as the Swift CLI's `init` command.
fn build_config_json(
    schema_namespace: &str,
    module_type: &ModuleType,
    target_name: Option<&str>,
) -> String {
    let module_type_key = module_type.to_string();

    let module_type_body = match (module_type, target_name) {
        (ModuleType::EmbeddedInTarget, Some(name)) => {
            format!(
                "\"{module_type_key}\" : {{\n            \"name\" : \"{name}\"\n          }}"
            )
        }
        _ => {
            format!("\"{module_type_key}\" : {{\n          }}")
        }
    };

    format!(
        r#"{{
  "schemaNamespace" : "{schema_namespace}",
  "input" : {{
    "operationSearchPaths" : [
      "**/*.graphql"
    ],
    "schemaSearchPaths" : [
      "**/*.graphqls"
    ]
  }},
  "output" : {{
    "testMocks" : {{
      "none" : {{
      }}
    }},
    "schemaTypes" : {{
      "path" : "./{schema_namespace}",
      "moduleType" : {{
        {module_type_body}
      }}
    }},
    "operations" : {{
      "inSchemaModule" : {{
      }}
    }}
  }}
}}"#
    )
}

/// Run the `init` command.
pub(crate) fn run(
    schema_name: Option<String>,
    schema_namespace: Option<String>,
    module_type: ModuleType,
    target_name: Option<String>,
    path: Option<String>,
    overwrite: bool,
    print_config: bool,
) -> anyhow::Result<()> {
    // Resolve schema_namespace, handling deprecated --schema-name flag
    let resolved_namespace = resolve_schema_namespace(schema_name, schema_namespace)?;

    // Validate: embeddedInTarget requires --target-name
    validate_target_name(&module_type, &target_name)?;

    let json = build_config_json(
        &resolved_namespace,
        &module_type,
        target_name.as_deref(),
    );

    // Validate the generated JSON is parseable as a valid configuration
    apollo_codegen_config::ApolloCodegenConfiguration::from_json(&json)
        .map_err(|e| anyhow::anyhow!("Generated configuration is invalid: {}", e))?;

    if print_config {
        println!("{}", json);
        return Ok(());
    }

    let output_path = path.as_deref().unwrap_or(DEFAULT_CONFIG_PATH);
    write_config(&json, output_path, overwrite)
}

/// Resolve the schema namespace from the (deprecated) --schema-name and --schema-namespace flags.
fn resolve_schema_namespace(
    schema_name: Option<String>,
    schema_namespace: Option<String>,
) -> anyhow::Result<String> {
    match (schema_name, schema_namespace) {
        (Some(_), Some(_)) => {
            anyhow::bail!(
                "Cannot specify both --schema-name and --schema-namespace. \
                 Please only use --schema-namespace."
            );
        }
        (Some(name), None) => {
            eprintln!("Warning: --schema-name is deprecated, please use --schema-namespace instead.");
            Ok(name)
        }
        (None, Some(ns)) => Ok(ns),
        (None, None) => {
            anyhow::bail!(
                "A schema namespace is required. Use --schema-namespace (-n) to specify one."
            );
        }
    }
}

/// Validate that --target-name is provided when module type is embeddedInTarget.
fn validate_target_name(
    module_type: &ModuleType,
    target_name: &Option<String>,
) -> anyhow::Result<()> {
    if matches!(module_type, ModuleType::EmbeddedInTarget) {
        match target_name {
            None => {
                anyhow::bail!(
                    "Target name is required when using \"embeddedInTarget\" module type. \
                     Use --target-name to specify."
                );
            }
            Some(name) if name.is_empty() => {
                anyhow::bail!(
                    "Target name is required when using \"embeddedInTarget\" module type. \
                     Use --target-name to specify."
                );
            }
            _ => {}
        }
    }
    Ok(())
}

/// Write the config JSON to the given path, respecting the overwrite flag.
fn write_config(json: &str, path: &str, overwrite: bool) -> anyhow::Result<()> {
    let file_path = Path::new(path);

    if !overwrite && file_path.exists() {
        anyhow::bail!(
            "File already exists at {}. Hint: use --overwrite to overwrite any existing file at the path.",
            path
        );
    }

    // Create parent directories if needed
    if let Some(parent) = file_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    std::fs::write(file_path, json)?;
    eprintln!("New configuration output to {}.", path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_config_json_swift_package_manager() {
        let json = build_config_json("MySchema", &ModuleType::SwiftPackageManager, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["schemaNamespace"], "MySchema");
        assert_eq!(parsed["input"]["operationSearchPaths"][0], "**/*.graphql");
        assert_eq!(parsed["input"]["schemaSearchPaths"][0], "**/*.graphqls");
        assert_eq!(parsed["output"]["schemaTypes"]["path"], "./MySchema");
        assert!(parsed["output"]["schemaTypes"]["moduleType"]["swiftPackageManager"].is_object());
        assert!(parsed["output"]["operations"]["inSchemaModule"].is_object());
        assert!(parsed["output"]["testMocks"]["none"].is_object());
    }

    #[test]
    fn test_build_config_json_embedded_in_target() {
        let json = build_config_json(
            "MySchema",
            &ModuleType::EmbeddedInTarget,
            Some("MyApp"),
        );
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(
            parsed["output"]["schemaTypes"]["moduleType"]["embeddedInTarget"]["name"],
            "MyApp"
        );
    }

    #[test]
    fn test_build_config_json_other() {
        let json = build_config_json("MySchema", &ModuleType::Other, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed["output"]["schemaTypes"]["moduleType"]["other"].is_object());
    }

    #[test]
    fn test_generated_config_is_parseable() {
        // Ensure the generated JSON can be deserialized by the config crate
        let json = build_config_json("TestSchema", &ModuleType::SwiftPackageManager, None);
        let result = apollo_codegen_config::ApolloCodegenConfiguration::from_json(&json);
        assert!(result.is_ok(), "Failed to parse generated config: {:?}", result.err());
    }

    #[test]
    fn test_generated_config_embedded_is_parseable() {
        let json = build_config_json("TestSchema", &ModuleType::EmbeddedInTarget, Some("MyTarget"));
        let result = apollo_codegen_config::ApolloCodegenConfiguration::from_json(&json);
        assert!(result.is_ok(), "Failed to parse generated config: {:?}", result.err());
    }

    #[test]
    fn test_resolve_schema_namespace_from_namespace() {
        let result = resolve_schema_namespace(None, Some("MyNS".to_string()));
        assert_eq!(result.unwrap(), "MyNS");
    }

    #[test]
    fn test_resolve_schema_namespace_from_deprecated_name() {
        let result = resolve_schema_namespace(Some("OldName".to_string()), None);
        assert_eq!(result.unwrap(), "OldName");
    }

    #[test]
    fn test_resolve_schema_namespace_both_specified_errors() {
        let result = resolve_schema_namespace(
            Some("Name".to_string()),
            Some("Namespace".to_string()),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot specify both"));
    }

    #[test]
    fn test_resolve_schema_namespace_neither_specified_errors() {
        let result = resolve_schema_namespace(None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("required"));
    }

    #[test]
    fn test_validate_target_name_embedded_without_name_errors() {
        let result = validate_target_name(&ModuleType::EmbeddedInTarget, &None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Target name is required"));
    }

    #[test]
    fn test_validate_target_name_embedded_with_empty_name_errors() {
        let result = validate_target_name(
            &ModuleType::EmbeddedInTarget,
            &Some(String::new()),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_target_name_embedded_with_name_ok() {
        let result = validate_target_name(
            &ModuleType::EmbeddedInTarget,
            &Some("MyTarget".to_string()),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_target_name_spm_without_name_ok() {
        let result = validate_target_name(&ModuleType::SwiftPackageManager, &None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_config_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-config.json");
        let path_str = path.to_str().unwrap();

        write_config("{}", path_str, false).unwrap();

        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
    }

    #[test]
    fn test_write_config_no_overwrite_existing_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-config.json");
        let path_str = path.to_str().unwrap();

        std::fs::write(&path, "existing").unwrap();

        let result = write_config("{}", path_str, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("File already exists"));
    }

    #[test]
    fn test_write_config_overwrite_existing_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-config.json");
        let path_str = path.to_str().unwrap();

        std::fs::write(&path, "existing").unwrap();

        write_config("{\"new\": true}", path_str, true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"new\": true}");
    }

    #[test]
    fn test_run_print_config() {
        // This test verifies the full run path with --print.
        // We can't easily capture stdout in a unit test, but we can verify it doesn't error.
        let result = run(
            None,
            Some("TestSchema".to_string()),
            ModuleType::SwiftPackageManager,
            None,
            None,
            false,
            true, // print_config
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_write_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.json");
        let path_str = path.to_str().unwrap().to_string();

        let result = run(
            None,
            Some("MySchema".to_string()),
            ModuleType::SwiftPackageManager,
            None,
            Some(path_str),
            false,
            false,
        );
        assert!(result.is_ok());
        assert!(path.exists());

        // Verify the file content is valid JSON with expected fields
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["schemaNamespace"], "MySchema");
    }

    #[test]
    fn test_run_embedded_without_target_name_errors() {
        let result = run(
            None,
            Some("MySchema".to_string()),
            ModuleType::EmbeddedInTarget,
            None,
            None,
            false,
            true,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_run_embedded_with_target_name_succeeds() {
        let result = run(
            None,
            Some("MySchema".to_string()),
            ModuleType::EmbeddedInTarget,
            Some("MyApp".to_string()),
            None,
            false,
            true, // print so we don't write files
        );
        assert!(result.is_ok());
    }
}
