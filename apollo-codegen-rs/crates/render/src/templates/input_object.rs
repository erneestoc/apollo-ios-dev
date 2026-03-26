//! Input object type template.
//!
//! Generates files like:
//! ```swift
//! public struct PetAdoptionInput: InputObject {
//!   public private(set) var __data: InputDict
//!   public init(_ data: InputDict) { __data = data }
//!   public init(ownerID: ID, petID: ID, ...) { ... }
//!   public var ownerID: ID { get { __data["ownerID"] } set { ... } }
//! }
//! ```

use super::header;

pub struct InputField {
    pub schema_name: String,
    pub rendered_name: String,
    pub rendered_type: String,
    pub rendered_init_type: String, // type with default value for initializer
    pub description: Option<String>,
    pub deprecation_reason: Option<String>,
}

pub fn render(
    type_name: &str,
    fields: &[InputField],
    access_modifier: &str,
    api_target_name: &str,
    include_deprecated_warnings: bool,
    description: Option<&str>,
) -> String {
    let mut result = String::new();
    result.push_str(header::HEADER);
    result.push_str("\n\n");
    result.push_str(&format!("import {}\n\n", api_target_name));

    // Documentation
    if let Some(doc) = description {
        if !doc.is_empty() {
            for line in doc.lines() {
                if line.is_empty() {
                    result.push_str("///\n");
                } else {
                    result.push_str(&format!("/// {}\n", line));
                }
            }
        }
    }

    let swift_name = crate::naming::first_uppercased(type_name);

    result.push_str(&format!(
        "{}struct {}: InputObject {{\n",
        access_modifier, swift_name,
    ));

    // __data property
    result.push_str(&format!(
        "  {}private(set) var __data: InputDict\n\n",
        access_modifier
    ));

    // Primary init
    result.push_str(&format!(
        "  {}init(_ data: InputDict) {{\n    __data = data\n  }}\n\n",
        access_modifier
    ));

    // Separate deprecated and valid fields
    let deprecated_fields: Vec<&InputField> = fields
        .iter()
        .filter(|f| f.deprecation_reason.is_some())
        .collect();
    let valid_fields: Vec<&InputField> = fields
        .iter()
        .filter(|f| f.deprecation_reason.is_none())
        .collect();

    // If there are both deprecated and valid fields, render a non-deprecated initializer first
    if !deprecated_fields.is_empty() && !valid_fields.is_empty() && include_deprecated_warnings {
        render_initializer(&mut result, &valid_fields, access_modifier);
        result.push('\n');
    }

    // If there are deprecated fields, add deprecation warning on full initializer
    if !deprecated_fields.is_empty() && include_deprecated_warnings {
        let names: Vec<&str> = deprecated_fields.iter().map(|f| f.rendered_name.as_str()).collect();
        let msg = if names.len() > 1 {
            format!("Arguments '{}' are deprecated.", names.join(", "))
        } else {
            format!("Argument '{}' is deprecated.", names[0])
        };
        result.push_str(&format!(
            "  @available(*, deprecated, message: \"{}\")\n",
            msg
        ));
    }

    // Full initializer with all fields
    let all_fields: Vec<&InputField> = fields.iter().collect();
    render_initializer(&mut result, &all_fields, access_modifier);
    result.push('\n');

    // Field properties
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        // Documentation
        if let Some(ref doc) = field.description {
            if !doc.is_empty() {
                for line in doc.lines() {
                    if line.is_empty() {
                        result.push_str("  ///\n");
                    } else {
                        result.push_str(&format!("  /// {}\n", line));
                    }
                }
            }
        }
        // Deprecation
        if let Some(ref reason) = field.deprecation_reason {
            result.push_str(&format!(
                "  @available(*, deprecated, message: \"{}\")\n",
                reason.replace('\"', "\\\"")
            ));
        }

        result.push_str(&format!(
            "  {}var {}: {} {{\n\
             {s}{s}get {{ __data[\"{}\"] }}\n\
             {s}{s}set {{ __data[\"{}\"] = newValue }}\n\
             {s}}}\n",
            access_modifier,
            field.rendered_name,
            field.rendered_type,
            field.schema_name,
            field.schema_name,
            s = "  ",
        ));
    }

    result.push_str("}\n");

    result
}

fn render_initializer(
    result: &mut String,
    fields: &[&InputField],
    access_modifier: &str,
) {
    result.push_str(&format!("  {}init(\n", access_modifier));
    for (i, field) in fields.iter().enumerate() {
        let comma = if i < fields.len() - 1 { "," } else { "" };
        result.push_str(&format!(
            "    {}: {}{}\n",
            field.rendered_name, field.rendered_init_type, comma
        ));
    }
    result.push_str("  ) {\n");
    result.push_str("    __data = InputDict([\n");
    for (i, field) in fields.iter().enumerate() {
        let comma = if i < fields.len() - 1 { "," } else { "" };
        result.push_str(&format!(
            "      \"{}\": {}{}\n",
            field.schema_name, field.rendered_name, comma
        ));
    }
    result.push_str("    ])\n");
    result.push_str("  }\n");
}
