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
) -> String {
    let body = format!(
        "static let {} = {}.Interface(name: \"{}\")",
        crate::naming::first_uppercased(type_name),
        api_target_name,
        schema_name,
    );

    header::render_schema_file(access_modifier, api_target_name, Some("Interfaces"), &body)
}
