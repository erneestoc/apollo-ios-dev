//! Schema configuration template.
//!
//! Generates SchemaConfiguration.swift - an editable file.

use askama::Template;

#[derive(Template)]
#[template(path = "schema_config.swift.askama", escape = "none")]
struct SchemaConfigTemplate<'a> {
    api_target_name: &'a str,
    access_modifier: &'a str,
    enum_access_modifier: &'a str,
}

pub fn render(
    access_modifier: &str,
    api_target_name: &str,
    is_embedded: bool,
) -> String {
    // In embedded mode, SchemaConfiguration doesn't have 'public' on the enum
    // because it's accessed within the module
    let enum_am = if is_embedded { "" } else { access_modifier };

    let template = SchemaConfigTemplate {
        api_target_name,
        access_modifier,
        enum_access_modifier: enum_am,
    };

    let mut output = template.render().expect("schema_config template render failed");
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}
