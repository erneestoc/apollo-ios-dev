//! Initialize command implementation.
//!
//! Mirrors Swift's `Sources/CodegenCLI/Commands/Initialize.swift`.
//! Generates a default `apollo-codegen-config.json` file using serde
//! serialization (D-74) rather than Swift's manual `minimalJSON`.

use std::path::Path;

use clap::{Args, ValueEnum};

use apollo_codegen_lib::config::ApolloCodegenConfiguration;
use apollo_codegen_lib::config::validation::validate_config_values;

use crate::constants;
use crate::error::CliError;

/// CLI-friendly module type enum without associated values.
///
/// Maps to `ApolloCodegenConfiguration.SchemaTypesFileOutput.ModuleType`.
/// Mirrors Swift's `ModuleTypeExpressibleByArgument` enum.
#[derive(Debug, Clone, ValueEnum)]
pub enum CliModuleType {
    EmbeddedInTarget,
    SwiftPackage,
    Other,
}

impl std::fmt::Display for CliModuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliModuleType::EmbeddedInTarget => write!(f, "embeddedInTarget"),
            CliModuleType::SwiftPackage => write!(f, "swiftPackageManager"),
            CliModuleType::Other => write!(f, "other"),
        }
    }
}

/// Initialize a new configuration with defaults.
#[derive(Args, Debug)]
#[command(name = "init")]
pub struct Initialize {
    /// DEPRECATED - Use --schema-namespace instead.
    #[arg(long)]
    pub schema_name: Option<String>,

    /// Name used to scope the generated schema type files.
    #[arg(long, short = 'n', default_value = "")]
    pub schema_namespace: String,

    /// How to package the schema types for dependency management.
    #[arg(long, short = 'm', value_enum)]
    pub module_type: CliModuleType,

    /// Name of the target for embeddedInTarget module type.
    #[arg(long, short = 't')]
    pub target_name: Option<String>,

    /// Write the configuration to a file at the path.
    #[arg(short, long, default_value = constants::DEFAULT_FILE_PATH)]
    pub path: String,

    /// Overwrite any file at --path.
    #[arg(long, short = 'w')]
    pub overwrite: bool,

    /// Print the configuration to stdout.
    #[arg(long, short = 's')]
    pub print: bool,
}

impl Initialize {
    pub fn run(&mut self) -> Result<(), CliError> {
        self.validate()?;

        // Build the module type JSON value programmatically to avoid injection
        let module_type_value = match (&self.module_type, &self.target_name) {
            (CliModuleType::EmbeddedInTarget, Some(name)) => {
                serde_json::json!({"embeddedInTarget": {"name": name}})
            }
            (CliModuleType::SwiftPackage, _) => serde_json::json!({"swiftPackageManager": {}}),
            (CliModuleType::Other, _) => serde_json::json!({"other": {}}),
            _ => unreachable!("validate() ensures target_name is set for embeddedInTarget"),
        };

        // Build configuration JSON using serde (D-74).
        // We construct the JSON value programmatically, then deserialize to get
        // a validated ApolloCodegenConfiguration, then re-serialize for pretty output.
        let config_value = serde_json::json!({
            "schemaNamespace": self.schema_namespace,
            "input": {
                "operationSearchPaths": ["**/*.graphql"],
                "schemaSearchPaths": ["**/*.graphqls"]
            },
            "output": {
                "testMocks": {"none": {}},
                "schemaTypes": {
                    "path": format!("./{}", self.schema_namespace),
                    "moduleType": module_type_value
                },
                "operations": {"inSchemaModule": {}}
            }
        });

        // Deserialize to get a validated config struct
        let config: ApolloCodegenConfiguration =
            serde_json::from_value(config_value).map_err(|e| CliError::InvalidConfiguration {
                source: e,
            })?;

        // Validate the config (matches Swift's encode-then-decode-then-validate pattern)
        validate_config_values(&config).map_err(|e| CliError::Validation {
            message: format!("{}", e),
        })?;

        // Re-serialize to pretty JSON for output (D-74: serde serialization)
        let json = serde_json::to_string_pretty(&config).map_err(|e| CliError::Generic {
            description: format!("Failed to serialize configuration: {}", e),
        })?;

        if self.print {
            println!("{}", json);
            return Ok(());
        }

        // Check if file exists and overwrite not set
        if !self.overwrite && Path::new(&self.path).exists() {
            return Err(CliError::FileAlreadyExists {
                path: self.path.clone(),
            });
        }

        // Write to file
        std::fs::write(&self.path, &json).map_err(|e| CliError::Generic {
            description: format!("Failed to write configuration to '{}': {}", self.path, e),
        })?;

        println!("New configuration output to {}.", self.path);
        Ok(())
    }

    fn validate(&mut self) -> Result<(), CliError> {
        // Handle deprecated --schema-name
        if let Some(ref schema_name) = self.schema_name {
            eprintln!(
                "Warning: --schema-name is deprecated, please use --schema-namespace instead."
            );

            if !self.schema_namespace.is_empty() {
                return Err(CliError::Validation {
                    message: "Cannot specify both --schema-name and --schema-namespace. \
                              Please only use --schema-namespace."
                        .to_string(),
                });
            }

            self.schema_namespace = schema_name.clone();
        }

        // embeddedInTarget requires --target-name
        if matches!(self.module_type, CliModuleType::EmbeddedInTarget) {
            match &self.target_name {
                None => {
                    return Err(CliError::Validation {
                        message: "Target name is required when using \"embeddedInTarget\" \
                                  module type. Use --target-name to specify."
                            .to_string(),
                    });
                }
                Some(name) if name.is_empty() => {
                    return Err(CliError::Validation {
                        message: "Target name is required when using \"embeddedInTarget\" \
                                  module type. Use --target-name to specify."
                            .to_string(),
                    });
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_module_type_has_all_variants() {
        // Verify all three variants exist
        let _ = CliModuleType::EmbeddedInTarget;
        let _ = CliModuleType::SwiftPackage;
        let _ = CliModuleType::Other;
    }

    #[test]
    fn test_validate_embedded_requires_target_name() {
        let mut cmd = Initialize {
            schema_name: None,
            schema_namespace: "MySchema".to_string(),
            module_type: CliModuleType::EmbeddedInTarget,
            target_name: None,
            path: constants::DEFAULT_FILE_PATH.to_string(),
            overwrite: false,
            print: false,
        };
        let result = cmd.validate();
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Target name is required"));
    }

    #[test]
    fn test_validate_embedded_empty_target_name_rejected() {
        let mut cmd = Initialize {
            schema_name: None,
            schema_namespace: "MySchema".to_string(),
            module_type: CliModuleType::EmbeddedInTarget,
            target_name: Some("".to_string()),
            path: constants::DEFAULT_FILE_PATH.to_string(),
            overwrite: false,
            print: false,
        };
        let result = cmd.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_schema_name_and_namespace_conflict() {
        let mut cmd = Initialize {
            schema_name: Some("OldName".to_string()),
            schema_namespace: "NewName".to_string(),
            module_type: CliModuleType::SwiftPackage,
            target_name: None,
            path: constants::DEFAULT_FILE_PATH.to_string(),
            overwrite: false,
            print: false,
        };
        let result = cmd.validate();
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Cannot specify both --schema-name and --schema-namespace"));
    }

    #[test]
    fn test_validate_deprecated_schema_name_sets_namespace() {
        let mut cmd = Initialize {
            schema_name: Some("LegacyName".to_string()),
            schema_namespace: "".to_string(),
            module_type: CliModuleType::SwiftPackage,
            target_name: None,
            path: constants::DEFAULT_FILE_PATH.to_string(),
            overwrite: false,
            print: false,
        };
        let result = cmd.validate();
        assert!(result.is_ok());
        assert_eq!(cmd.schema_namespace, "LegacyName");
    }

    #[test]
    fn test_validate_swift_package_no_target_needed() {
        let mut cmd = Initialize {
            schema_name: None,
            schema_namespace: "MySchema".to_string(),
            module_type: CliModuleType::SwiftPackage,
            target_name: None,
            path: constants::DEFAULT_FILE_PATH.to_string(),
            overwrite: false,
            print: false,
        };
        let result = cmd.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_other_no_target_needed() {
        let mut cmd = Initialize {
            schema_name: None,
            schema_namespace: "MySchema".to_string(),
            module_type: CliModuleType::Other,
            target_name: None,
            path: constants::DEFAULT_FILE_PATH.to_string(),
            overwrite: false,
            print: false,
        };
        let result = cmd.validate();
        assert!(result.is_ok());
    }
}
