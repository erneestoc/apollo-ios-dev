//! Object type template.
//!
//! Generates files like:
//! ```swift
//! // @generated
//! // This file was automatically generated and should not be edited.
//!
//! import ApolloAPI
//!
//! public extension Objects {
//!   static let Dog = ApolloAPI.Object(
//!     typename: "Dog",
//!     implementedInterfaces: [
//!       Interfaces.Animal.self,
//!       Interfaces.Pet.self
//!     ]
//!   )
//! }
//! ```

use super::header;

/// Render an Object type file.
pub fn render(
    type_name: &str,
    schema_name: &str,
    interfaces: &[String],
    access_modifier: &str,
    api_target_name: &str,
    schema_namespace: &str,
    is_in_module: bool,
    description: Option<&str>,
) -> String {
    let body = render_body(
        type_name,
        schema_name,
        interfaces,
        api_target_name,
        schema_namespace,
        is_in_module,
    );

    // For embeddedInTarget (is_in_module=false), use full namespace prefix
    let ns_prefix = if !is_in_module {
        format!("{}.Objects", crate::naming::first_uppercased(schema_namespace))
    } else {
        "Objects".to_string()
    };

    header::render_schema_file_with_doc(
        access_modifier,
        api_target_name,
        Some(&ns_prefix),
        &body,
        description,
    )
}

fn render_body(
    type_name: &str,
    schema_name: &str,
    interfaces: &[String],
    api_target_name: &str,
    schema_namespace: &str,
    is_in_module: bool,
) -> String {
    let renamed_comment = if type_name != schema_name {
        format!("// Renamed from GraphQL schema value: '{}'\n", schema_name)
    } else {
        String::new()
    };
    let interfaces_str = if interfaces.is_empty() {
        "[]".to_string()
    } else {
        let prefix = if !is_in_module {
            format!("{}.", crate::naming::first_uppercased(schema_namespace))
        } else {
            String::new()
        };

        let items: Vec<String> = interfaces
            .iter()
            .map(|iface| {
                format!(
                    "{}Interfaces.{}.self",
                    prefix,
                    crate::naming::first_uppercased(iface)
                )
            })
            .collect();

        if items.len() == 1 {
            format!("[{}]", items[0])
        } else {
            let indented: Vec<String> = items.iter().map(|i| format!("    {}", i)).collect();
            format!("[\n{}\n  ]", indented.join(",\n"))
        }
    };

    format!(
        "{}static let {} = {}.Object(\n  typename: \"{}\",\n  implementedInterfaces: {}\n)",
        renamed_comment,
        crate::naming::first_uppercased(type_name),
        api_target_name,
        schema_name,
        interfaces_str,
    )
}
