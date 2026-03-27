//! Mock interfaces template.
//!
//! Generates MockObject+Interfaces.graphql.swift:
//! ```swift
//! public extension MockObject {
//!   typealias Animal = Interface
//!   typealias Pet = Interface
//! }
//! ```

use super::header;

pub fn render(
    interfaces: &[String],
    access_modifier: &str,
    schema_module_name: &str,
    import_module: &str,
) -> String {
    let mut result = String::new();
    result.push_str(header::HEADER);
    result.push_str("\n\n");
    result.push_str("import ApolloTestSupport\n");
    result.push_str(&format!("import {}\n\n", import_module));

    result.push_str(&format!("{}extension MockObject {{\n", access_modifier));
    for iface in interfaces {
        result.push_str(&format!(
            "  typealias {} = Interface\n",
            crate::naming::first_uppercased(iface),
        ));
    }
    result.push('}');
    result.push('\n');

    result
}
