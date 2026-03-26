//! Schema customization support.
//!
//! Provides type name remapping based on the `schemaCustomization.customTypeNames`
//! configuration. This allows users to rename GraphQL types, enum cases, and input
//! object fields in the generated Swift code while keeping the GraphQL names intact.

use apollo_codegen_config::types::{CustomizationType, SchemaCustomization};
use std::collections::HashMap;

/// Customizer that maps GraphQL type names to custom Swift names.
///
/// This applies the `customTypeNames` configuration to rename types, enum cases,
/// and input object fields in the generated code. The GraphQL names (typenames,
/// data dict keys, etc.) are preserved; only the Swift identifiers change.
#[derive(Debug, Clone)]
pub struct SchemaCustomizer {
    /// GraphQL type name -> custom Swift type name
    type_names: HashMap<String, String>,
    /// (GraphQL enum name, GraphQL case name) -> custom case name
    enum_cases: HashMap<(String, String), String>,
    /// (GraphQL input object name, GraphQL field name) -> custom field name
    input_fields: HashMap<(String, String), String>,
}

impl SchemaCustomizer {
    /// Build a customizer from the schema customization config.
    pub fn new(config: &SchemaCustomization) -> Self {
        let mut type_names = HashMap::new();
        let mut enum_cases = HashMap::new();
        let mut input_fields = HashMap::new();

        for (graphql_name, customization) in &config.custom_type_names {
            match customization {
                CustomizationType::Type(swift_name) => {
                    type_names.insert(graphql_name.clone(), swift_name.clone());
                }
                CustomizationType::Enum { name, cases } => {
                    if let Some(swift_name) = name {
                        type_names.insert(graphql_name.clone(), swift_name.clone());
                    }
                    if let Some(cases_map) = cases {
                        for (case_graphql, case_swift) in cases_map {
                            enum_cases.insert(
                                (graphql_name.clone(), case_graphql.clone()),
                                case_swift.clone(),
                            );
                        }
                    }
                }
                CustomizationType::InputObject { name, fields } => {
                    if let Some(swift_name) = name {
                        type_names.insert(graphql_name.clone(), swift_name.clone());
                    }
                    if let Some(fields_map) = fields {
                        for (field_graphql, field_swift) in fields_map {
                            input_fields.insert(
                                (graphql_name.clone(), field_graphql.clone()),
                                field_swift.clone(),
                            );
                        }
                    }
                }
            }
        }

        Self {
            type_names,
            enum_cases,
            input_fields,
        }
    }

    /// Create an empty customizer (no-op).
    pub fn empty() -> Self {
        Self {
            type_names: HashMap::new(),
            enum_cases: HashMap::new(),
            input_fields: HashMap::new(),
        }
    }

    /// Get the customized Swift type name for a GraphQL type, or the original name.
    pub fn custom_type_name<'a>(&'a self, graphql_name: &'a str) -> &'a str {
        self.type_names
            .get(graphql_name)
            .map(|s| s.as_str())
            .unwrap_or(graphql_name)
    }

    /// Get the customized enum case name, or the original name.
    pub fn custom_enum_case<'a>(&'a self, enum_graphql_name: &str, case_graphql_name: &'a str) -> &'a str {
        self.enum_cases
            .get(&(enum_graphql_name.to_string(), case_graphql_name.to_string()))
            .map(|s| s.as_str())
            .unwrap_or(case_graphql_name)
    }

    /// Get the customized input field name, or the original name.
    pub fn custom_input_field<'a>(
        &'a self,
        input_graphql_name: &str,
        field_graphql_name: &'a str,
    ) -> &'a str {
        self.input_fields
            .get(&(
                input_graphql_name.to_string(),
                field_graphql_name.to_string(),
            ))
            .map(|s| s.as_str())
            .unwrap_or(field_graphql_name)
    }

    /// Check if there is any type name customization at all.
    pub fn has_customizations(&self) -> bool {
        !self.type_names.is_empty()
            || !self.enum_cases.is_empty()
            || !self.input_fields.is_empty()
    }

    /// Apply type name customization to a variable type string.
    ///
    /// Variable type strings like `GraphQLNullable<PetSearchFilters>` contain
    /// raw GraphQL type names that need to be replaced with their customized
    /// Swift names. This handles types appearing as bare names, inside generics
    /// (angle brackets), or inside array brackets.
    pub fn customize_variable_type(&self, type_str: &str) -> String {
        if self.type_names.is_empty() {
            return type_str.to_string();
        }
        let mut result = type_str.to_string();
        for (graphql_name, swift_name) in &self.type_names {
            // Replace whole-word occurrences of the type name.
            // Type names appear in positions like:
            //   GraphQLNullable<TypeName>  [TypeName]  TypeName?  TypeName
            // We need to avoid partial matches (e.g., don't replace "Date" inside "CustomDate").
            // Use word boundary logic: the character before must not be alphanumeric/underscore,
            // and the character after must not be alphanumeric/underscore.
            let mut new_result = String::new();
            let mut i = 0;
            let bytes = result.as_bytes();
            let gname_bytes = graphql_name.as_bytes();
            let gname_len = gname_bytes.len();
            while i < bytes.len() {
                if i + gname_len <= bytes.len() && &bytes[i..i + gname_len] == gname_bytes {
                    // Check word boundary before
                    let before_ok = if i == 0 {
                        true
                    } else {
                        let c = bytes[i - 1] as char;
                        !c.is_alphanumeric() && c != '_'
                    };
                    // Check word boundary after
                    let after_ok = if i + gname_len >= bytes.len() {
                        true
                    } else {
                        let c = bytes[i + gname_len] as char;
                        !c.is_alphanumeric() && c != '_'
                    };
                    if before_ok && after_ok {
                        new_result.push_str(swift_name);
                        i += gname_len;
                        continue;
                    }
                }
                new_result.push(bytes[i] as char);
                i += 1;
            }
            result = new_result;
        }
        result
    }
    /// Apply customization to a default value string.
    /// Replaces type names and input field names within the value.
    pub fn customize_default_value(&self, default_value: &str) -> String {
        let mut result = default_value.to_string();
        // Replace type names (input object names used as constructors)
        for (graphql_name, swift_name) in &self.type_names {
            // Replace TypeName( with CustomName(
            let pattern = format!("{}(", graphql_name);
            let replacement = format!("{}(", swift_name);
            result = result.replace(&pattern, &replacement);
        }
        // Replace input field names (fieldName: with customFieldName:)
        for ((input_name, field_name), custom_name) in &self.input_fields {
            // Replace "fieldName:" with "customName:" inside the value
            // Be careful to only replace field references, not arbitrary occurrences
            let pattern = format!("{}: ", field_name);
            let replacement = format!("{}: ", custom_name);
            result = result.replace(&pattern, &replacement);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_customizer() {
        let c = SchemaCustomizer::empty();
        assert_eq!(c.custom_type_name("Foo"), "Foo");
        assert_eq!(c.custom_enum_case("Foo", "BAR"), "BAR");
        assert_eq!(c.custom_input_field("Foo", "bar"), "bar");
        assert!(!c.has_customizations());
    }

    #[test]
    fn test_type_name_customization() {
        let mut custom_type_names = indexmap::IndexMap::new();
        custom_type_names.insert(
            "Animal".to_string(),
            CustomizationType::Type("CustomAnimal".to_string()),
        );
        custom_type_names.insert(
            "SkinCovering".to_string(),
            CustomizationType::Enum {
                name: Some("CustomSkinCovering".to_string()),
                cases: Some({
                    let mut m = indexmap::IndexMap::new();
                    m.insert("HAIR".to_string(), "CUSTOMHAIR".to_string());
                    m
                }),
            },
        );
        custom_type_names.insert(
            "PetSearchFilters".to_string(),
            CustomizationType::InputObject {
                name: Some("CustomPetSearchFilters".to_string()),
                fields: Some({
                    let mut m = indexmap::IndexMap::new();
                    m.insert("size".to_string(), "customSize".to_string());
                    m
                }),
            },
        );
        let sc = SchemaCustomization { custom_type_names };
        let c = SchemaCustomizer::new(&sc);

        assert_eq!(c.custom_type_name("Animal"), "CustomAnimal");
        assert_eq!(c.custom_type_name("Dog"), "Dog");
        assert_eq!(c.custom_type_name("SkinCovering"), "CustomSkinCovering");
        assert_eq!(c.custom_type_name("PetSearchFilters"), "CustomPetSearchFilters");

        assert_eq!(c.custom_enum_case("SkinCovering", "HAIR"), "CUSTOMHAIR");
        assert_eq!(c.custom_enum_case("SkinCovering", "FUR"), "FUR");
        assert_eq!(c.custom_enum_case("OtherEnum", "HAIR"), "HAIR");

        assert_eq!(c.custom_input_field("PetSearchFilters", "size"), "customSize");
        assert_eq!(c.custom_input_field("PetSearchFilters", "species"), "species");
        assert_eq!(c.custom_input_field("OtherInput", "size"), "size");

        assert!(c.has_customizations());
    }
}
