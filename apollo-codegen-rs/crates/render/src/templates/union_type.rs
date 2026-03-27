//! Union type template.
//!
//! Generates files like:
//! ```swift
//! public extension Unions {
//!   static let ClassroomPet = Union(
//!     name: "ClassroomPet",
//!     possibleTypes: [
//!       Objects.Cat.self,
//!       Objects.Bird.self
//!     ]
//!   )
//! }
//! ```

use super::header;

pub fn render(
    type_name: &str,
    schema_name: &str,
    member_types: &[String],
    access_modifier: &str,
    api_target_name: &str,
    schema_namespace: &str,
    is_in_module: bool,
    description: Option<&str>,
) -> String {
    let body = render_body(type_name, schema_name, member_types, schema_namespace, is_in_module);
    header::render_schema_file_with_doc(access_modifier, api_target_name, Some("Unions"), &body, description)
}

fn render_body(
    type_name: &str,
    schema_name: &str,
    member_types: &[String],
    schema_namespace: &str,
    is_in_module: bool,
) -> String {
    let prefix = if !is_in_module {
        format!("{}.", crate::naming::first_uppercased(schema_namespace))
    } else {
        String::new()
    };

    let members_str = if member_types.is_empty() {
        "[]".to_string()
    } else {
        let items: Vec<String> = member_types
            .iter()
            .map(|m| {
                format!(
                    "    {}Objects.{}.self",
                    prefix,
                    crate::naming::first_uppercased(m)
                )
            })
            .collect();
        format!("[\n{}\n  ]", items.join(",\n"))
    };

    format!(
        "static let {} = Union(\n  name: \"{}\",\n  possibleTypes: {}\n)",
        crate::naming::first_uppercased(type_name),
        schema_name,
        members_str,
    )
}
