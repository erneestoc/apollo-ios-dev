//! Operation definition template.
//!
//! Generates files like:
//! ```swift
//! // @generated
//! // This file was automatically generated and should not be edited.
//!
//! @_exported import ApolloAPI
//!
//! public class DogQuery: GraphQLQuery {
//!   public static let operationName: String = "DogQuery"
//!   public static let operationDocument: ApolloAPI.OperationDocument = .init(
//!     definition: .init(
//!       #"query DogQuery { ... }"#,
//!       fragments: [DogFragment.self, ...]
//!     ))
//!   public init() {}
//!   public struct Data: SchemaNamespace.SelectionSet { ... }
//! }
//! ```

use super::header;
use super::selection_set;

/// The operation type (maps to the Swift protocol name).
#[derive(Debug, Clone, Copy)]
pub enum OperationType {
    Query,
    Mutation,
    Subscription,
}

impl OperationType {
    pub fn swift_protocol(&self) -> &str {
        match self {
            OperationType::Query => "GraphQLQuery",
            OperationType::Mutation => "GraphQLMutation",
            OperationType::Subscription => "GraphQLSubscription",
        }
    }
}

/// A variable definition for the operation.
#[derive(Debug, Clone)]
pub struct VariableConfig<'a> {
    /// Property name (e.g. "input").
    pub name: &'a str,
    /// Swift type (e.g. "PetAdoptionInput").
    pub swift_type: &'a str,
    /// Default value expression if any.
    pub default_value: Option<&'a str>,
}

/// Configuration for rendering an operation file.
#[derive(Debug)]
pub struct OperationConfig<'a> {
    /// Class name (e.g. "DogQuery").
    pub class_name: &'a str,
    /// Operation name (e.g. "DogQuery" or "ClassroomPets" if different from class name).
    pub operation_name: &'a str,
    /// Operation type (query, mutation, subscription).
    pub operation_type: OperationType,
    /// Schema namespace (e.g. "AnimalKingdomAPI").
    pub schema_namespace: &'a str,
    /// Access modifier (e.g. "public ").
    pub access_modifier: &'a str,
    /// The operation source string (the GraphQL query text).
    pub source: &'a str,
    /// Referenced fragment names.
    pub fragment_names: Vec<&'a str>,
    /// Variable definitions (empty for no-variable operations).
    pub variables: Vec<VariableConfig<'a>>,
    /// The Data selection set config.
    pub data_selection_set: selection_set::SelectionSetConfig<'a>,
}

/// Render a complete operation file.
pub fn render(config: &OperationConfig) -> String {
    let mut result = String::new();

    // Header
    result.push_str(header::HEADER);
    result.push_str("\n\n");
    result.push_str("@_exported import ApolloAPI\n\n");

    // Class declaration
    result.push_str(&format!(
        "{}class {}: {} {{\n",
        config.access_modifier, config.class_name, config.operation_type.swift_protocol()
    ));

    // operationName
    result.push_str(&format!(
        "  {}static let operationName: String = \"{}\"\n",
        config.access_modifier, config.operation_name
    ));

    // operationDocument
    result.push_str(&format!(
        "  {}static let operationDocument: ApolloAPI.OperationDocument = .init(\n",
        config.access_modifier
    ));
    result.push_str("    definition: .init(\n");
    result.push_str(&format!(
        "      #\"{}\"#",
        config.source
    ));

    if config.fragment_names.is_empty() {
        result.push('\n');
    } else {
        result.push_str(",\n");
        let fragments: Vec<String> = config
            .fragment_names
            .iter()
            .map(|name| format!("{}.self", name))
            .collect();
        result.push_str(&format!("      fragments: [{}]\n", fragments.join(", ")));
    }
    result.push_str("    ))\n");

    // Variables or init
    if config.variables.is_empty() {
        result.push('\n');
        result.push_str(&format!("  {}init() {{}}\n", config.access_modifier));
    } else {
        result.push('\n');
        // Variable properties
        for var in &config.variables {
            result.push_str(&format!(
                "  {}var {}: {}\n",
                config.access_modifier, var.name, var.swift_type
            ));
        }
        result.push('\n');

        // init with variables
        result.push_str(&format!("  {}init(", config.access_modifier));
        if config.variables.len() == 1 && config.variables[0].default_value.is_none() {
            let var = &config.variables[0];
            result.push_str(&format!("{}: {}", var.name, var.swift_type));
            result.push_str(") {\n");
        } else {
            // Multi-line or default-valued init
            for (i, var) in config.variables.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&format!("{}: {}", var.name, var.swift_type));
                if let Some(default) = var.default_value {
                    result.push_str(&format!(" = {}", default));
                }
            }
            result.push_str(") {\n");
        }
        for var in &config.variables {
            result.push_str(&format!("    self.{} = {}\n", var.name, var.name));
        }
        result.push_str("  }\n");

        // __variables
        result.push('\n');
        let var_entries: Vec<String> = config
            .variables
            .iter()
            .map(|v| format!("\"{}\": {}", v.name, v.name))
            .collect();
        result.push_str(&format!(
            "  {}var __variables: Variables? {{ [{}] }}\n",
            config.access_modifier,
            var_entries.join(", ")
        ));
    }

    // Data selection set
    result.push('\n');
    result.push_str(&selection_set::render(&config.data_selection_set));

    // Close class
    result.push_str("}\n");

    result
}
