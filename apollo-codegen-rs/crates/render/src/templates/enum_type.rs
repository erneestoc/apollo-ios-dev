//! Enum type template.
//!
//! Generates files like:
//! ```swift
//! public enum SkinCovering: String, EnumType {
//!   case fur = "FUR"
//!   case hair = "HAIR"
//! }
//! ```

use askama::Template;

/// Pre-computed enum value for the template.
pub struct EnumValue {
    pub name: String,
    pub raw_value: String,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub deprecation_reason: Option<String>,
    /// Whether this case was explicitly renamed via schema customization.
    pub is_renamed: bool,
}

/// Template-ready enum value with pre-computed case name and escaped deprecation reason.
struct TemplateEnumValue {
    case_name: String,
    raw_value: String,
    description: Option<String>,
    is_deprecated: bool,
    deprecation_reason: Option<String>,
    is_renamed: bool,
}

#[derive(Template)]
#[template(path = "enum_type.swift.askama", escape = "none")]
struct EnumTypeTemplate<'a> {
    api_target_name: &'a str,
    access_modifier: &'a str,
    swift_type_name: String,
    /// Pre-rendered type header: doc comments + renamed comment, with trailing newline.
    type_header: String,
    values: Vec<TemplateEnumValue>,
    /// Whether camelCase conversion is applied (determines if raw value literal is needed).
    /// Swift always includes ` = "value"` when conversionStrategies.enumCases != .none.
    camel_case_conversion: bool,
}

pub fn render(
    type_name: &str,
    schema_name: &str,
    values: &[EnumValue],
    access_modifier: &str,
    api_target_name: &str,
    camel_case_conversion: bool,
    description: Option<&str>,
) -> String {
    let swift_type_name = crate::naming::first_uppercased(type_name);

    // Pre-render type-level header (doc comments + rename comment)
    let type_header = render_type_header(type_name, schema_name, description);

    let template_values: Vec<TemplateEnumValue> = values
        .iter()
        .map(|v| {
            // When a case is explicitly renamed, do NOT apply camelCase conversion
            let case_name = if v.is_renamed {
                v.name.clone()
            } else if camel_case_conversion {
                crate::naming::to_camel_case(&v.name)
            } else {
                v.name.clone()
            };

            let escaped_name = crate::naming::escape_swift_name(&case_name);

            // Escape quotes in deprecation reason for the template
            let deprecation_reason = v
                .deprecation_reason
                .as_ref()
                .map(|r| r.replace('"', "\\\""));

            TemplateEnumValue {
                case_name: escaped_name,
                raw_value: v.raw_value.clone(),
                description: v.description.clone(),
                is_deprecated: v.is_deprecated,
                deprecation_reason,
                is_renamed: v.is_renamed,
            }
        })
        .collect();

    let template = EnumTypeTemplate {
        api_target_name,
        access_modifier,
        swift_type_name,
        type_header,
        values: template_values,
        camel_case_conversion,
    };

    let mut output = template.render().expect("enum_type template render failed");
    // Askama strips the final newline from the template file; add it back
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

/// Render the type-level header: doc comments and "renamed from" comment.
fn render_type_header(type_name: &str, schema_name: &str, description: Option<&str>) -> String {
    let mut header = String::new();

    // Documentation comments
    if let Some(desc) = description {
        if !desc.is_empty() {
            // Use split('\n') instead of lines() to preserve \r characters.
            // Swift's template engine writes \r bytes literally into output.
            // When a line ends with \r, the carriage return overwrites the
            // "/// " prefix of the current line, and the NEXT line gets no
            // prefix because Swift's TemplateString doesn't re-emit it.
            // We reproduce this for byte-for-byte matching.
            let parts: Vec<&str> = desc.split('\n').collect();
            let last_idx = parts.len() - 1;
            let mut prev_had_cr = false;
            for (i, line) in parts.iter().enumerate() {
                if i == last_idx && line.is_empty() {
                    header.push_str("///\n");
                    continue;
                }
                if prev_had_cr {
                    // Previous line had \r — this line gets no /// prefix
                    // (matches Swift's buggy \r behavior)
                    header.push_str(&format!("{}\n", line));
                    prev_had_cr = line.ends_with('\r');
                } else if line.is_empty() {
                    header.push_str("///\n");
                } else {
                    header.push_str(&format!("/// {}\n", line));
                    prev_had_cr = line.ends_with('\r');
                }
            }
        }
    }

    // "Renamed from" comment
    if type_name != schema_name {
        header.push_str(&format!(
            "// Renamed from GraphQL schema value: '{}'\n",
            schema_name
        ));
    }

    header
}
