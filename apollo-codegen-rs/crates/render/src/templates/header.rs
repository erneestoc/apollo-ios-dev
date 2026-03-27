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
    render_schema_file_with_doc(access_modifier, import_name, namespace, body, None)
}

/// Render a complete schema file with optional documentation comment before the body.
pub fn render_schema_file_with_doc(
    access_modifier: &str,
    import_name: &str,
    namespace: Option<&str>,
    body: &str,
    description: Option<&str>,
) -> String {
    let mut result = String::new();
    result.push_str(HEADER);
    result.push('\n');
    result.push('\n');
    result.push_str(&format!("import {}\n", import_name));

    if let Some(ns) = namespace {
        result.push('\n');
        result.push_str(&format!("{}extension {} {{\n", access_modifier, ns));
        // Add documentation comment if present
        if let Some(desc) = description {
            if !desc.is_empty() {
                for line in desc.lines() {
                    if line.is_empty() {
                        result.push_str("  ///\n");
                    } else {
                        result.push_str(&format!("  /// {}\n", line));
                    }
                }
            }
        }
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
