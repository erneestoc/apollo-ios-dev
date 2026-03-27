//! Mock unions template.
//!
//! Generates MockObject+Unions.graphql.swift:
//! ```swift
//! public extension MockObject {
//!   typealias ClassroomPet = Union
//! }
//! ```

use super::header;

pub fn render(
    unions: &[String],
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
    for union_name in unions {
        result.push_str(&format!(
            "  typealias {} = Union\n",
            crate::naming::first_uppercased(union_name),
        ));
    }
    result.push('}');
    result.push('\n');

    result
}
