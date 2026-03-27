//! Interface type template.
//!
//! Generates files like:
//! ```swift
//! public extension Interfaces {
//!   static let Animal = ApolloAPI.Interface(name: "Animal")
//! }
//! ```

use askama::Template;

#[derive(Template)]
#[template(path = "interface.swift.askama", escape = "none")]
struct InterfaceTemplate<'a> {
    api_target_name: &'a str,
    access_modifier: &'a str,
    ns_prefix: String,
    swift_name: String,
    type_name: &'a str,
    schema_name: &'a str,
    description: Option<&'a str>,
}

pub fn render(
    type_name: &str,
    schema_name: &str,
    access_modifier: &str,
    api_target_name: &str,
    description: Option<&str>,
    schema_namespace: &str,
    is_in_module: bool,
) -> String {
    // For embeddedInTarget (is_in_module=false), use full namespace prefix
    let ns_prefix = if !is_in_module {
        format!("{}.Interfaces", crate::naming::first_uppercased(schema_namespace))
    } else {
        "Interfaces".to_string()
    };

    let swift_name = crate::naming::first_uppercased(type_name);

    let template = InterfaceTemplate {
        api_target_name,
        access_modifier,
        ns_prefix,
        swift_name,
        type_name,
        schema_name,
        description,
    };

    let mut output = template.render().expect("interface template render failed");
    // Match Swift codegen: no trailing newline
    while output.ends_with('\n') {
        output.pop();
    }
    output
}
