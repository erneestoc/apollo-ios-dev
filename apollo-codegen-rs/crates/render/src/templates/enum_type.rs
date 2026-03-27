//! Enum type template.
//!
//! Generates files like:
//! ```swift
//! public enum SkinCovering: String, EnumType {
//!   case fur = "FUR"
//!   case hair = "HAIR"
//! }
//! ```

use super::header;

pub struct EnumValue {
    pub name: String,
    pub raw_value: String,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub deprecation_reason: Option<String>,
    /// Whether this case was explicitly renamed via schema customization.
    pub is_renamed: bool,
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
    let mut body = String::new();

    // Type-level documentation comment
    if let Some(desc) = description {
        if !desc.is_empty() {
            for line in desc.lines() {
                if line.is_empty() {
                    body.push_str("///\n");
                } else {
                    body.push_str(&format!("/// {}\n", line));
                }
            }
        }
    }

    // "Renamed from" comment for the enum type
    if type_name != schema_name {
        body.push_str(&format!(
            "// Renamed from GraphQL schema value: '{}'\n",
            schema_name
        ));
    }

    body.push_str(&format!(
        "{}enum {}: String, EnumType {{\n",
        access_modifier,
        crate::naming::first_uppercased(type_name),
    ));

    for value in values {
        // When a case is explicitly renamed, do NOT apply camelCase conversion
        let case_name = if value.is_renamed {
            value.name.clone()
        } else if camel_case_conversion {
            crate::naming::to_camel_case(&value.name)
        } else {
            value.name.clone()
        };

        let escaped_name = crate::naming::escape_swift_name(&case_name);

        // Value-level documentation comment
        if let Some(ref desc) = value.description {
            if !desc.is_empty() {
                for line in desc.lines() {
                    if line.is_empty() {
                        body.push_str("  ///\n");
                    } else {
                        body.push_str(&format!("  /// {}\n", line));
                    }
                }
            }
        }

        if value.is_deprecated {
            if let Some(ref reason) = value.deprecation_reason {
                body.push_str(&format!(
                    "  @available(*, deprecated, message: \"{}\")\n",
                    reason.replace('\"', "\\\"")
                ));
            } else {
                body.push_str("  @available(*, deprecated)\n");
            }
        }

        // "Renamed from" comment for renamed cases
        if value.is_renamed {
            body.push_str(&format!(
                "  // Renamed from GraphQL schema value: '{}'\n",
                value.raw_value
            ));
        }

        // Omit raw value when case name matches (Swift enum optimization)
        if escaped_name == value.raw_value {
            body.push_str(&format!("  case {}\n", escaped_name));
        } else {
            body.push_str(&format!(
                "  case {} = \"{}\"\n",
                escaped_name, value.raw_value
            ));
        }
    }

    body.push_str("}\n");

    let mut result = String::new();
    result.push_str(header::HEADER);
    result.push_str("\n\n");
    result.push_str(&format!("import {}\n\n", api_target_name));
    result.push_str(&body);

    result
}
