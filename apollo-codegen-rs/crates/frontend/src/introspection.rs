//! Introspection JSON → SDL conversion.
//!
//! apollo-compiler doesn't support loading schemas from introspection JSON directly,
//! so we convert introspection JSON to SDL first, then parse the SDL.

use serde::Deserialize;

/// Convert an introspection JSON result to SDL string.
pub fn introspection_json_to_sdl(json: &str) -> Result<String, String> {
    let root: IntrospectionRoot =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse introspection JSON: {}", e))?;

    let schema_data = if let Some(data) = root.data {
        data.__schema
    } else if let Some(schema) = root.__schema {
        schema
    } else {
        return Err("Introspection JSON must contain 'data.__schema' or '__schema'".to_string());
    };

    let mut sdl = String::new();

    // Schema definition (if non-default root types)
    let has_custom_roots = schema_data
        .mutation_type
        .as_ref()
        .map(|t| t.name != "Mutation")
        .unwrap_or(false)
        || schema_data
            .subscription_type
            .as_ref()
            .map(|t| t.name != "Subscription")
            .unwrap_or(false)
        || schema_data.query_type.name != "Query";

    if has_custom_roots {
        sdl.push_str("schema {\n");
        sdl.push_str(&format!("  query: {}\n", schema_data.query_type.name));
        if let Some(ref mt) = schema_data.mutation_type {
            sdl.push_str(&format!("  mutation: {}\n", mt.name));
        }
        if let Some(ref st) = schema_data.subscription_type {
            sdl.push_str(&format!("  subscription: {}\n", st.name));
        }
        sdl.push_str("}\n\n");
    }

    // Types
    for type_def in &schema_data.types {
        // Skip built-in types
        if type_def.name.starts_with("__") {
            continue;
        }
        // Skip built-in scalars
        if matches!(
            type_def.name.as_str(),
            "String" | "Int" | "Float" | "Boolean" | "ID"
        ) {
            continue;
        }

        match type_def.kind.as_str() {
            "SCALAR" => {
                write_description(&mut sdl, &type_def.description, "");
                sdl.push_str(&format!("scalar {}\n\n", type_def.name));
            }
            "OBJECT" => {
                write_description(&mut sdl, &type_def.description, "");
                sdl.push_str(&format!("type {}", type_def.name));
                if let Some(ref ifaces) = type_def.interfaces {
                    if !ifaces.is_empty() {
                        let names: Vec<&str> = ifaces.iter().map(|i| i.name.as_str()).collect();
                        sdl.push_str(&format!(" implements {}", names.join(" & ")));
                    }
                }
                sdl.push_str(" {\n");
                if let Some(ref fields) = type_def.fields {
                    for field in fields {
                        write_field(&mut sdl, field);
                    }
                }
                sdl.push_str("}\n\n");
            }
            "INTERFACE" => {
                write_description(&mut sdl, &type_def.description, "");
                sdl.push_str(&format!("interface {}", type_def.name));
                if let Some(ref ifaces) = type_def.interfaces {
                    if !ifaces.is_empty() {
                        let names: Vec<&str> = ifaces.iter().map(|i| i.name.as_str()).collect();
                        sdl.push_str(&format!(" implements {}", names.join(" & ")));
                    }
                }
                sdl.push_str(" {\n");
                if let Some(ref fields) = type_def.fields {
                    for field in fields {
                        write_field(&mut sdl, field);
                    }
                }
                sdl.push_str("}\n\n");
            }
            "UNION" => {
                write_description(&mut sdl, &type_def.description, "");
                if let Some(ref possible) = type_def.possible_types {
                    let names: Vec<&str> = possible.iter().map(|t| t.name.as_str()).collect();
                    sdl.push_str(&format!(
                        "union {} = {}\n\n",
                        type_def.name,
                        names.join(" | ")
                    ));
                }
            }
            "ENUM" => {
                write_description(&mut sdl, &type_def.description, "");
                sdl.push_str(&format!("enum {} {{\n", type_def.name));
                if let Some(ref values) = type_def.enum_values {
                    for val in values {
                        write_description(&mut sdl, &val.description, "  ");
                        sdl.push_str(&format!("  {}", val.name));
                        if val.is_deprecated {
                            if let Some(ref reason) = val.deprecation_reason {
                                sdl.push_str(&format!(
                                    " @deprecated(reason: \"{}\")",
                                    reason.replace('\"', "\\\"")
                                ));
                            } else {
                                sdl.push_str(" @deprecated");
                            }
                        }
                        sdl.push('\n');
                    }
                }
                sdl.push_str("}\n\n");
            }
            "INPUT_OBJECT" => {
                write_description(&mut sdl, &type_def.description, "");
                sdl.push_str(&format!("input {} {{\n", type_def.name));
                if let Some(ref fields) = type_def.input_fields {
                    for field in fields {
                        write_description(&mut sdl, &field.description, "  ");
                        sdl.push_str(&format!(
                            "  {}: {}",
                            field.name,
                            type_ref_to_sdl(&field.field_type)
                        ));
                        if let Some(ref dv) = field.default_value {
                            sdl.push_str(&format!(" = {}", dv));
                        }
                        if field.is_deprecated {
                            if let Some(ref reason) = field.deprecation_reason {
                                sdl.push_str(&format!(
                                    " @deprecated(reason: \"{}\")",
                                    reason.replace('\"', "\\\"")
                                ));
                            } else {
                                sdl.push_str(" @deprecated");
                            }
                        }
                        sdl.push('\n');
                    }
                }
                sdl.push_str("}\n\n");
            }
            _ => {}
        }
    }

    Ok(sdl)
}

fn write_description(sdl: &mut String, desc: &Option<String>, indent: &str) {
    if let Some(ref d) = desc {
        if !d.is_empty() {
            if d.contains('\n') {
                sdl.push_str(&format!("{}\"\"\"{}\"\"\" \n", indent, d));
            } else {
                sdl.push_str(&format!("{}\"{}\" \n", indent, d.replace('\"', "\\\"")));
            }
        }
    }
}

fn write_field(sdl: &mut String, field: &IntrospectionField) {
    write_description(sdl, &field.description, "  ");
    sdl.push_str(&format!("  {}", field.name));
    if let Some(ref args) = field.args {
        if !args.is_empty() {
            let arg_strs: Vec<String> = args
                .iter()
                .map(|arg| {
                    let mut s = format!("{}: {}", arg.name, type_ref_to_sdl(&arg.field_type));
                    if let Some(ref dv) = arg.default_value {
                        s.push_str(&format!(" = {}", dv));
                    }
                    s
                })
                .collect();
            sdl.push_str(&format!("({})", arg_strs.join(", ")));
        }
    }
    sdl.push_str(&format!(": {}", type_ref_to_sdl(&field.field_type)));
    if field.is_deprecated {
        if let Some(ref reason) = field.deprecation_reason {
            sdl.push_str(&format!(
                " @deprecated(reason: \"{}\")",
                reason.replace('\"', "\\\"")
            ));
        } else {
            sdl.push_str(" @deprecated");
        }
    }
    sdl.push('\n');
}

fn type_ref_to_sdl(type_ref: &IntrospectionTypeRef) -> String {
    match type_ref.kind.as_str() {
        "NON_NULL" => {
            if let Some(ref of_type) = type_ref.of_type {
                format!("{}!", type_ref_to_sdl(of_type))
            } else {
                "!".to_string()
            }
        }
        "LIST" => {
            if let Some(ref of_type) = type_ref.of_type {
                format!("[{}]", type_ref_to_sdl(of_type))
            } else {
                "[]".to_string()
            }
        }
        _ => type_ref.name.clone().unwrap_or_default(),
    }
}

// --- Introspection JSON types ---

#[derive(Debug, Deserialize)]
struct IntrospectionRoot {
    data: Option<IntrospectionData>,
    #[serde(rename = "__schema")]
    __schema: Option<IntrospectionSchema>,
}

#[derive(Debug, Deserialize)]
struct IntrospectionData {
    #[serde(rename = "__schema")]
    __schema: IntrospectionSchema,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntrospectionSchema {
    query_type: TypeName,
    mutation_type: Option<TypeName>,
    subscription_type: Option<TypeName>,
    types: Vec<IntrospectionType>,
    #[serde(default)]
    directives: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct TypeName {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntrospectionType {
    kind: String,
    name: String,
    description: Option<String>,
    fields: Option<Vec<IntrospectionField>>,
    input_fields: Option<Vec<IntrospectionInputField>>,
    interfaces: Option<Vec<TypeName>>,
    enum_values: Option<Vec<IntrospectionEnumValue>>,
    possible_types: Option<Vec<TypeName>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntrospectionField {
    name: String,
    description: Option<String>,
    args: Option<Vec<IntrospectionInputField>>,
    #[serde(rename = "type")]
    field_type: IntrospectionTypeRef,
    is_deprecated: bool,
    deprecation_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntrospectionInputField {
    name: String,
    description: Option<String>,
    #[serde(rename = "type")]
    field_type: IntrospectionTypeRef,
    default_value: Option<String>,
    #[serde(default)]
    is_deprecated: bool,
    deprecation_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntrospectionEnumValue {
    name: String,
    description: Option<String>,
    is_deprecated: bool,
    deprecation_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntrospectionTypeRef {
    kind: String,
    name: Option<String>,
    of_type: Option<Box<IntrospectionTypeRef>>,
}
