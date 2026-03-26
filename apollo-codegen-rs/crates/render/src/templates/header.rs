//! Header comment template.

/// The standard generated file header.
pub const HEADER: &str = "\
// @generated
// This file was automatically generated and should not be edited.";

/// Render a complete schema file with header, import, namespace wrapping, and body.
pub fn render_schema_file(
    access_modifier: &str,
    import_name: &str,
    namespace: Option<&str>,
    body: &str,
) -> String {
    let mut result = String::new();
    result.push_str(HEADER);
    result.push('\n');
    result.push('\n');
    result.push_str(&format!("import {}\n", import_name));

    if let Some(ns) = namespace {
        result.push('\n');
        result.push_str(&format!("{}extension {} {{\n", access_modifier, ns));
        // Indent the body by 2 spaces
        for line in body.lines() {
            if line.is_empty() {
                result.push('\n');
            } else {
                result.push_str("  ");
                result.push_str(line);
                result.push('\n');
            }
        }
        result.push('}');
    } else {
        result.push('\n');
        result.push_str(body);
    }

    result
}
