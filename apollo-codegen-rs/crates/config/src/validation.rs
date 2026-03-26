//! Configuration validation logic.

use crate::types::ApolloCodegenConfiguration;

impl ApolloCodegenConfiguration {
    /// Validate the configuration values.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.schema_namespace.is_empty() {
            errors.push("schemaNamespace cannot be empty".to_string());
        }

        if self.input.schema_search_paths.is_empty() {
            errors.push("input.schemaSearchPaths cannot be empty".to_string());
        }

        if self.input.operation_search_paths.is_empty() {
            errors.push("input.operationSearchPaths cannot be empty".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
