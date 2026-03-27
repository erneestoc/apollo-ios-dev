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

use askama::Template;

#[derive(Template)]
#[template(path = "object.swift.askama", escape = "none")]
struct ObjectTemplate<'a> {
    api_target_name: &'a str,
    access_modifier: &'a str,
    ns_prefix: String,
    swift_name: String,
    type_name: &'a str,
    schema_name: &'a str,
    description: Option<&'a str>,
    interfaces_str: String,
}

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
    // For embeddedInTarget (is_in_module=false), use full namespace prefix
    let ns_prefix = if !is_in_module {
        format!("{}.Objects", crate::naming::first_uppercased(schema_namespace))
    } else {
        "Objects".to_string()
    };

    let swift_name = crate::naming::first_uppercased(type_name);

    // Build the interfaces string with correct indentation
    // This will appear at 4-space indent level inside the template
    let interfaces_str = build_interfaces_str(interfaces, schema_namespace, is_in_module);

    let template = ObjectTemplate {
        api_target_name,
        access_modifier,
        ns_prefix,
        swift_name,
        type_name,
        schema_name,
        description,
        interfaces_str,
    };

    let mut output = template.render().expect("object template render failed");
    // Match Swift codegen: no trailing newline
    while output.ends_with('\n') {
        output.pop();
    }
    output
}

fn build_interfaces_str(
    interfaces: &[String],
    schema_namespace: &str,
    is_in_module: bool,
) -> String {
    if interfaces.is_empty() {
        return "[]".to_string();
    }

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
        // Multi-line: 6 spaces for items (2 extension + 4 body), 4 spaces for closing bracket
        let indented: Vec<String> = items.iter().map(|i| format!("      {}", i)).collect();
        format!("[\n{}\n    ]", indented.join(",\n"))
    }
}
