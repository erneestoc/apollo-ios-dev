//! Interface type template.
//!
//! Generates files like:
//! ```swift
//! public extension Interfaces {
//!   static let Animal = ApolloAPI.Interface(name: "Animal")
//! }
//! ```

use super::header;

pub fn render(
    type_name: &str,
    schema_name: &str,
    access_modifier: &str,
    api_target_name: &str,
    description: Option<&str>,
    schema_namespace: &str,
    is_in_module: bool,
) -> String {
    let renamed_comment = if type_name != schema_name {
        format!("// Renamed from GraphQL schema value: '{}'\n", schema_name)
    } else {
        String::new()
    };
    let body = format!(
        "{}static let {} = {}.Interface(name: \"{}\")",
        renamed_comment,
        crate::naming::first_uppercased(type_name),
        api_target_name,
        schema_name,
    );

    // For embeddedInTarget (is_in_module=false), use full namespace prefix
    let ns_prefix = if !is_in_module {
        format!("{}.Interfaces", crate::naming::first_uppercased(schema_namespace))
    } else {
        "Interfaces".to_string()
    };

    header::render_schema_file_with_doc(access_modifier, api_target_name, Some(&ns_prefix), &body, description)
}
