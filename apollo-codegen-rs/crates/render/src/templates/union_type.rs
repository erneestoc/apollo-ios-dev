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

use askama::Template;

#[derive(Template)]
#[template(path = "union_type.swift.askama", escape = "none")]
struct UnionTypeTemplate<'a> {
    api_target_name: &'a str,
    access_modifier: &'a str,
    ns_prefix: String,
    swift_name: String,
    type_name: &'a str,
    schema_name: &'a str,
    description: Option<&'a str>,
    members_str: String,
}

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
    // For embeddedInTarget (is_in_module=false), use full namespace prefix
    let ns_prefix = if !is_in_module {
        format!("{}.Unions", crate::naming::first_uppercased(schema_namespace))
    } else {
        "Unions".to_string()
    };

    let swift_name = crate::naming::first_uppercased(type_name);
    let members_str = build_members_str(member_types, schema_namespace, is_in_module);

    let template = UnionTypeTemplate {
        api_target_name,
        access_modifier,
        ns_prefix,
        swift_name,
        type_name,
        schema_name,
        description,
        members_str,
    };

    let mut output = template.render().expect("union_type template render failed");
    // Match Swift codegen: no trailing newline
    while output.ends_with('\n') {
        output.pop();
    }
    output
}

fn build_members_str(
    member_types: &[String],
    schema_namespace: &str,
    is_in_module: bool,
) -> String {
    if member_types.is_empty() {
        return "[]".to_string();
    }

    let prefix = if !is_in_module {
        format!("{}.", crate::naming::first_uppercased(schema_namespace))
    } else {
        String::new()
    };

    let items: Vec<String> = member_types
        .iter()
        .map(|m| {
            format!(
                "{}Objects.{}.self",
                prefix,
                crate::naming::first_uppercased(m)
            )
        })
        .collect();

    // Single-member: inline `[Objects.Cat.self]`
    // Multi-member: multi-line with 6-space indent for items, 4-space for closing bracket
    if items.len() == 1 {
        format!("[{}]", items[0])
    } else {
        let indented: Vec<String> = items.iter().map(|i| format!("      {}", i)).collect();
        format!("[\n{}\n    ]", indented.join(",\n"))
    }
}
