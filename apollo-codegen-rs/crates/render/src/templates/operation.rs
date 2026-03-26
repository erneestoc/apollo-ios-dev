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

/// Controls how the query string literal is formatted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryStringFormat {
    /// Single-line raw string: `#"query { ... }"#`
    SingleLine,
    /// Multi-line raw string with indentation.
    Multiline,
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
    /// Whether this is a local cache mutation.
    pub is_local_cache_mutation: bool,
    /// Whether to include the definition in operationDocument (default: true).
    pub include_definition: bool,
    /// Operation identifier (SHA256 hash) if configured, None otherwise.
    pub operation_identifier: Option<&'a str>,
    /// How to format the query string literal (default: SingleLine).
    pub query_string_format: QueryStringFormat,
    /// The API target name for import statements (default: "ApolloAPI").
    pub api_target_name: &'a str,
}

/// Render a complete operation file.
pub fn render(config: &OperationConfig) -> String {
    if config.is_local_cache_mutation {
        render_local_cache_mutation(config)
    } else {
        render_regular_operation(config)
    }
}

/// Render a local cache mutation operation.
fn render_local_cache_mutation(config: &OperationConfig) -> String {
    let mut result = String::new();

    // Header
    result.push_str(header::HEADER);
    result.push_str("\n\n");
    result.push_str(&format!("@_exported import {}\n\n", config.api_target_name));

    // Class declaration: uses LocalCacheMutation instead of GraphQLQuery/Mutation/Subscription
    result.push_str(&format!(
        "{}class {}: LocalCacheMutation {{\n",
        config.access_modifier, config.class_name
    ));

    // operationType (instead of operationName + operationDocument)
    let op_type_value = match config.operation_type {
        OperationType::Query => ".query",
        OperationType::Mutation => ".mutation",
        OperationType::Subscription => ".subscription",
    };
    result.push_str(&format!(
        "  {}static let operationType: GraphQLOperationType = {}\n",
        config.access_modifier, op_type_value
    ));

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
        if config.variables.len() == 1 {
            let var = &config.variables[0];
            if let Some(default) = var.default_value {
                // Single variable with default value - inline format
                result.push_str(&format!(
                    "  {}init({}: {} = {}) {{\n",
                    config.access_modifier, var.name, var.swift_type, default
                ));
            } else {
                // Single variable without default - inline format
                result.push_str(&format!(
                    "  {}init({}: {}) {{\n",
                    config.access_modifier, var.name, var.swift_type
                ));
            }
        } else {
            result.push_str(&format!("  {}init(\n", config.access_modifier));
            for (i, var) in config.variables.iter().enumerate() {
                let comma = if i < config.variables.len() - 1 { "," } else { "" };
                if let Some(default) = var.default_value {
                    result.push_str(&format!(
                        "    {}: {} = {}{}\n",
                        var.name, var.swift_type, default, comma
                    ));
                } else {
                    result.push_str(&format!(
                        "    {}: {}{}\n",
                        var.name, var.swift_type, comma
                    ));
                }
            }
            result.push_str("  ) {\n");
        }
        for var in &config.variables {
            result.push_str(&format!("    self.{} = {}\n", var.name, var.name));
        }
        result.push_str("  }\n");

        // __variables - uses GraphQLOperation.Variables for local cache mutations
        result.push('\n');
        if config.variables.len() == 1 {
            let v = &config.variables[0];
            result.push_str(&format!(
                "  {}var __variables: GraphQLOperation.Variables? {{ [\"{}\": {}] }}\n",
                config.access_modifier, v.name, v.name
            ));
        } else {
            result.push_str(&format!(
                "  {}var __variables: GraphQLOperation.Variables? {{ [\n",
                config.access_modifier,
            ));
            for (i, v) in config.variables.iter().enumerate() {
                let comma = if i < config.variables.len() - 1 { "," } else { "" };
                result.push_str(&format!(
                    "    \"{}\": {}{}\n",
                    v.name, v.name, comma
                ));
            }
            result.push_str("  ] }\n");
        }
    }

    // Data selection set
    result.push('\n');
    result.push_str(&selection_set::render(&config.data_selection_set));

    // Close class
    result.push_str("}\n");

    result
}

/// Render a regular (non-local-cache-mutation) operation.
fn render_regular_operation(config: &OperationConfig) -> String {
    let mut result = String::new();

    // Header
    result.push_str(header::HEADER);
    result.push_str("\n\n");
    result.push_str(&format!("@_exported import {}\n\n", config.api_target_name));

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
        "  {}static let operationDocument: {}.OperationDocument = .init(\n",
        config.access_modifier, config.api_target_name
    ));

    // operationIdentifier (before definition if present)
    if let Some(op_id) = config.operation_identifier {
        result.push_str(&format!("    operationIdentifier: \"{}\"{}\n",
            op_id,
            if config.include_definition { "," } else { "" }
        ));
    }

    // definition (only if include_definition is true)
    if config.include_definition {
        result.push_str("    definition: .init(\n");

        // Render the query string based on format
        match config.query_string_format {
            QueryStringFormat::SingleLine => {
                result.push_str(&format!(
                    "      #\"{}\"#",
                    config.source
                ));
            }
            QueryStringFormat::Multiline => {
                result.push_str("      #\"\"\"\n");
                // Indent each line of the source with 6 spaces
                for line in config.source.lines() {
                    if line.is_empty() {
                        result.push('\n');
                    } else {
                        result.push_str(&format!("      {}\n", line));
                    }
                }
                result.push_str("      \"\"\"#");
            }
        }

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
        result.push_str("    )");
    }
    result.push_str(")\n");

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
        if config.variables.len() == 1 {
            let var = &config.variables[0];
            if let Some(default) = var.default_value {
                // Single variable with default value - inline format
                result.push_str(&format!(
                    "  {}init({}: {} = {}) {{\n",
                    config.access_modifier, var.name, var.swift_type, default
                ));
            } else {
                // Single variable without default - inline format
                result.push_str(&format!(
                    "  {}init({}: {}) {{\n",
                    config.access_modifier, var.name, var.swift_type
                ));
            }
        } else {
            result.push_str(&format!("  {}init(\n", config.access_modifier));
            for (i, var) in config.variables.iter().enumerate() {
                let comma = if i < config.variables.len() - 1 { "," } else { "" };
                if let Some(default) = var.default_value {
                    result.push_str(&format!(
                        "    {}: {} = {}{}\n",
                        var.name, var.swift_type, default, comma
                    ));
                } else {
                    result.push_str(&format!(
                        "    {}: {}{}\n",
                        var.name, var.swift_type, comma
                    ));
                }
            }
            result.push_str("  ) {\n");
        }
        for var in &config.variables {
            result.push_str(&format!("    self.{} = {}\n", var.name, var.name));
        }
        result.push_str("  }\n");

        // __variables - multi-line when more than 1 entry
        result.push('\n');
        if config.variables.len() == 1 {
            let v = &config.variables[0];
            result.push_str(&format!(
                "  {}var __variables: Variables? {{ [\"{}\": {}] }}\n",
                config.access_modifier, v.name, v.name
            ));
        } else {
            result.push_str(&format!(
                "  {}var __variables: Variables? {{ [\n",
                config.access_modifier,
            ));
            for (i, v) in config.variables.iter().enumerate() {
                let comma = if i < config.variables.len() - 1 { "," } else { "" };
                result.push_str(&format!(
                    "    \"{}\": {}{}\n",
                    v.name, v.name, comma
                ));
            }
            result.push_str("  ] }\n");
        }
    }

    // Data selection set
    result.push('\n');
    result.push_str(&selection_set::render(&config.data_selection_set));

    // Close class
    result.push_str("}\n");

    result
}
