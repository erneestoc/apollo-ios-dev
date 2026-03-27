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

use askama::Template;

pub struct InputField {
    pub schema_name: String,
    pub rendered_name: String,
    pub rendered_type: String,
    pub rendered_init_type: String, // type with default value for initializer
    pub description: Option<String>,
    pub deprecation_reason: Option<String>,
    /// Whether this field was explicitly renamed via schema customization.
    pub is_renamed: bool,
}

/// A pre-computed initializer parameter list.
struct InitializerParams {
    params: Vec<InitParam>,
}

/// A single initializer parameter.
struct InitParam {
    schema_name: String,
    rendered_name: String,
    rendered_init_type: String,
}

#[derive(Template)]
#[template(path = "input_object.swift.askama", escape = "none")]
struct InputObjectTemplate<'a> {
    api_target_name: &'a str,
    access_modifier: &'a str,
    swift_name: String,
    /// Pre-rendered type header: doc comments + renamed comment, with trailing newline.
    type_header: String,
    valid_fields_initializer: Option<InitializerParams>,
    deprecated_initializer_message: Option<String>,
    /// All field init params for the full initializer.
    all_init_fields: Vec<InitParam>,
    /// Pre-rendered field properties section (doc comments, deprecation, rename, var).
    field_properties: String,
}

pub fn render(
    type_name: &str,
    schema_name: &str,
    fields: &[InputField],
    access_modifier: &str,
    api_target_name: &str,
    include_deprecated_warnings: bool,
    description: Option<&str>,
) -> String {
    let swift_name = crate::naming::first_uppercased(type_name);

    // Pre-render type-level header (doc comments + rename comment)
    let type_header = render_type_header(type_name, schema_name, description);

    // Separate deprecated and valid fields
    let deprecated_fields: Vec<&InputField> = fields
        .iter()
        .filter(|f| f.deprecation_reason.is_some())
        .collect();
    let valid_fields: Vec<&InputField> = fields
        .iter()
        .filter(|f| f.deprecation_reason.is_none())
        .collect();

    // If there are both deprecated and valid fields, render a non-deprecated initializer
    let valid_fields_initializer =
        if !deprecated_fields.is_empty() && !valid_fields.is_empty() && include_deprecated_warnings
        {
            Some(InitializerParams {
                params: valid_fields
                    .iter()
                    .map(|f| InitParam {
                        schema_name: f.schema_name.clone(),
                        rendered_name: f.rendered_name.clone(),
                        rendered_init_type: f.rendered_init_type.clone(),
                    })
                    .collect(),
            })
        } else {
            None
        };

    // If there are deprecated fields, add deprecation warning on full initializer
    let deprecated_initializer_message =
        if !deprecated_fields.is_empty() && include_deprecated_warnings {
            let names: Vec<&str> = deprecated_fields
                .iter()
                .map(|f| f.rendered_name.as_str())
                .collect();
            let msg = if names.len() > 1 {
                format!("Arguments '{}' are deprecated.", names.join(", "))
            } else {
                format!("Argument '{}' is deprecated.", names[0])
            };
            Some(msg)
        } else {
            None
        };

    // All fields for the full initializer
    let all_init_fields: Vec<InitParam> = fields
        .iter()
        .map(|f| InitParam {
            schema_name: f.schema_name.clone(),
            rendered_name: f.rendered_name.clone(),
            rendered_init_type: f.rendered_init_type.clone(),
        })
        .collect();

    // Pre-render field properties section
    let field_properties = render_field_properties(fields, access_modifier);

    let template = InputObjectTemplate {
        api_target_name,
        access_modifier,
        swift_name,
        type_header,
        valid_fields_initializer,
        deprecated_initializer_message,
        all_init_fields,
        field_properties,
    };

    let mut output = template
        .render()
        .expect("input_object template render failed");
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
            for line in desc.lines() {
                if line.is_empty() {
                    header.push_str("///\n");
                } else {
                    header.push_str(&format!("/// {}\n", line));
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

/// Render the field properties section (doc comments, deprecation, rename, var accessors).
fn render_field_properties(fields: &[InputField], access_modifier: &str) -> String {
    let mut result = String::new();
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
                reason.replace('"', "\\\"")
            ));
        }
        // Renamed comment
        if field.is_renamed {
            result.push_str(&format!(
                "  // Renamed from GraphQL schema value: '{}'\n",
                field.schema_name
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
    result
}
