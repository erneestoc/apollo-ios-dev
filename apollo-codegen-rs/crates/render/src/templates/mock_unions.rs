//! Mock unions template.
//!
//! Generates MockObject+Unions.graphql.swift:
//! ```swift
//! public extension MockObject {
//!   typealias ClassroomPet = Union
//! }
//! ```

use askama::Template;

#[derive(Template)]
#[template(path = "mock_unions.swift.askama", escape = "none")]
struct MockUnionsTemplate<'a> {
    access_modifier: &'a str,
    import_module: &'a str,
    union_names: Vec<String>,
}

pub fn render(
    unions: &[String],
    access_modifier: &str,
    _schema_module_name: &str,
    import_module: &str,
) -> String {
    let union_names: Vec<String> = unions
        .iter()
        .map(|u| crate::naming::first_uppercased(u))
        .collect();

    let template = MockUnionsTemplate {
        access_modifier,
        import_module,
        union_names,
    };

    let mut output = template.render().expect("mock_unions template render failed");
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}
