//! Mock interfaces template.
//!
//! Generates MockObject+Interfaces.graphql.swift:
//! ```swift
//! public extension MockObject {
//!   typealias Animal = Interface
//!   typealias Pet = Interface
//! }
//! ```

use askama::Template;

#[derive(Template)]
#[template(path = "mock_interfaces.swift.askama", escape = "none")]
struct MockInterfacesTemplate<'a> {
    access_modifier: &'a str,
    import_module: &'a str,
    interface_names: Vec<String>,
}

pub fn render(
    interfaces: &[String],
    access_modifier: &str,
    _schema_module_name: &str,
    import_module: &str,
) -> String {
    let interface_names: Vec<String> = interfaces
        .iter()
        .map(|i| crate::naming::first_uppercased(i))
        .collect();

    let template = MockInterfacesTemplate {
        access_modifier,
        import_module,
        interface_names,
    };

    let mut output = template.render().expect("mock_interfaces template render failed");
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}
