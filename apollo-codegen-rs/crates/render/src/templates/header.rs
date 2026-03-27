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

/// Wrap already-rendered file content in a namespace extension for embeddedInTarget mode.
///
/// Takes the full rendered content of a file and wraps the body (everything after
/// the import lines) in `{access_modifier}extension {namespace} { ... }`.
/// The first declaration keyword (class, struct, enum, typealias) has its access
/// modifier removed since it's inherited from the extension.
pub fn wrap_in_namespace_extension(content: &str, namespace: &str, access_modifier: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();
    let mut body_start = 0;

    // Copy header and import lines as-is, find where body starts
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("import ") || line.starts_with("@_exported import ") {
            // This is the last import line; body starts after the blank line following it
            result.push_str(line);
            result.push('\n');
            body_start = i + 1;
            // Skip blank line after import
            if body_start < lines.len() && lines[body_start].is_empty() {
                result.push('\n');
                body_start += 1;
            }
            break;
        }
        result.push_str(line);
        result.push('\n');
    }

    // Open extension
    result.push_str(&format!("{}extension {} {{\n", access_modifier, namespace));

    // Process body lines: indent by 2 spaces, remove access modifier from first declaration
    let mut stripped_access = false;
    for i in body_start..lines.len() {
        let line = lines[i];
        if line.is_empty() {
            result.push('\n');
            continue;
        }

        let trimmed = line.trim_start();
        let processed = if !stripped_access
            && !trimmed.starts_with("//")
            && !trimmed.starts_with("///")
            && trimmed.starts_with(access_modifier)
        {
            // Strip the access modifier from the first actual declaration line
            stripped_access = true;
            trimmed[access_modifier.len()..].to_string()
        } else {
            line.to_string()
        };

        result.push_str("  ");
        result.push_str(&processed);
        result.push('\n');
    }

    // Close extension: ensure blank line before closing brace, no trailing newline
    // Strip any trailing newlines from body
    while result.ends_with('\n') {
        result.pop();
    }
    // Add blank line + closing brace (no trailing newline to match Swift output)
    result.push_str("\n\n}");

    result
}
