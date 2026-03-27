//! Swift Package Manager module template.
//!
//! Generates Package.swift for the schema types module.

use askama::Template;

#[derive(Template)]
#[template(path = "package_swift.swift.askama", escape = "none")]
struct PackageSwiftTemplate<'a> {
    ns: String,
    test_mock_target: Option<(&'a str, &'a str)>,
}

pub fn render(
    schema_namespace: &str,
    test_mock_target: Option<(&str, &str)>, // (target_name, path)
) -> String {
    let ns = crate::naming::first_uppercased(schema_namespace);

    let template = PackageSwiftTemplate {
        ns,
        test_mock_target,
    };

    let mut output = template.render().expect("package_swift template render failed");
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}
