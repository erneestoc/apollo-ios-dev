//! IR definition rendering helpers for code generation.
//!
//! Mirrors Swift's `IRDefinition+RenderingHelpers.swift` from
//! `Sources/ApolloCodegenLib/Templates/RenderingHelpers/IRDefinition+RenderingHelpers.swift`.

use super::string_casing::first_uppercased;
use crate::templates::ConfigurationContext;

/// Returns the rendered selection set type for a definition.
///
/// Mirrors Swift's `IR.Definition.renderedSelectionSetType(_:)`.
pub fn rendered_selection_set_type(
  config: &ConfigurationContext,
  is_mutable: bool,
) -> String {
  let mutable_prefix = if is_mutable { "Mutable" } else { "" };
  format!(
    "{}.{}SelectionSet",
    first_uppercased(config.schema_namespace()),
    mutable_prefix
  )
}

/// Generates the definition name for an operation, appending a suffix based on
/// the operation type if the name doesn't already end with it.
///
/// Mirrors Swift's `CompilationResult.OperationDefinition.generatedDefinitionName`.
pub fn generated_definition_name(
  name: &str,
  operation_type: &str,
  is_local_cache_mutation: bool,
) -> String {
  let suffix = if is_local_cache_mutation {
    "LocalCacheMutation".to_string()
  } else {
    match operation_type {
      "query" => "Query".to_string(),
      "mutation" => "Mutation".to_string(),
      "subscription" => "Subscription".to_string(),
      _ => operation_type.to_string(),
    }
  };

  let name_with_suffix = if name.ends_with(&suffix) {
    name.to_string()
  } else {
    format!("{}{}", name, suffix)
  };

  first_uppercased(&name_with_suffix)
}

/// Generates the definition name for a fragment.
///
/// Mirrors Swift's `CompilationResult.FragmentDefinition.generatedDefinitionName`.
pub fn generated_fragment_definition_name(name: &str) -> String {
  first_uppercased(name)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::ApolloCodegenConfiguration;

  fn make_config() -> ConfigurationContext {
    let config: ApolloCodegenConfiguration = serde_json::from_str(r#"{
      "schemaNamespace": "testSchema",
      "input": {},
      "output": {
        "schemaTypes": { "path": "./gen", "moduleType": {"swiftPackageManager": {}} },
        "operations": {"inSchemaModule": {}},
        "testMocks": {"none": {}}
      }
    }"#).unwrap();
    ConfigurationContext::new(config, None)
  }

  #[test]
  fn test_rendered_selection_set_type_immutable() {
    let config = make_config();
    let result = rendered_selection_set_type(&config, false);
    assert_eq!(result, "TestSchema.SelectionSet");
  }

  #[test]
  fn test_rendered_selection_set_type_mutable() {
    let config = make_config();
    let result = rendered_selection_set_type(&config, true);
    assert_eq!(result, "TestSchema.MutableSelectionSet");
  }

  #[test]
  fn test_generated_definition_name_query() {
    let result = generated_definition_name("getAllUsers", "query", false);
    assert_eq!(result, "GetAllUsersQuery");
  }

  #[test]
  fn test_generated_definition_name_already_has_suffix() {
    let result = generated_definition_name("GetAllUsersQuery", "query", false);
    assert_eq!(result, "GetAllUsersQuery");
  }

  #[test]
  fn test_generated_definition_name_mutation() {
    let result = generated_definition_name("createUser", "mutation", false);
    assert_eq!(result, "CreateUserMutation");
  }

  #[test]
  fn test_generated_definition_name_subscription() {
    let result = generated_definition_name("onUserUpdated", "subscription", false);
    assert_eq!(result, "OnUserUpdatedSubscription");
  }

  #[test]
  fn test_generated_definition_name_local_cache_mutation() {
    let result = generated_definition_name("updateCache", "query", true);
    assert_eq!(result, "UpdateCacheLocalCacheMutation");
  }

  #[test]
  fn test_generated_fragment_definition_name() {
    let result = generated_fragment_definition_name("userDetails");
    assert_eq!(result, "UserDetails");
  }
}
