//! Mock object template.
//!
//! Generates files like Dog+Mock.graphql.swift with MockObject class,
//! MockFields struct, and convenience initializer.

use super::header;

/// A field on a mock object.
pub struct MockField {
    pub response_key: String,
    pub property_name: String,
    pub initializer_param_name: Option<String>,
    pub field_type_str: String,     // For @Field<Type> annotation
    pub mock_type_str: String,      // For initializer parameter type
    pub set_function: String,       // _setScalar, _setEntity, _setList
    pub deprecation_reason: Option<String>,
}

pub fn render(
    object_name: &str,
    fields: &[MockField],
    access_modifier: &str,
    schema_namespace: &str,
    api_target_name: &str,
) -> String {
    let mut result = String::new();
    result.push_str(header::HEADER);
    result.push_str("\n\n");
    result.push_str("import ApolloTestSupport\n");
    result.push_str(&format!("import {}\n\n", schema_namespace));

    let swift_name = crate::naming::first_uppercased(object_name);
    let ns = crate::naming::first_uppercased(schema_namespace);

    // Class definition
    result.push_str(&format!(
        "{}class {}: MockObject {{\n",
        access_modifier, swift_name,
    ));
    result.push_str(&format!(
        "  {}static let objectType: {}.Object = {}.Objects.{}\n",
        access_modifier, api_target_name, ns, swift_name,
    ));
    result.push_str(&format!(
        "  {}static let _mockFields = MockFields()\n",
        access_modifier,
    ));
    result.push_str(&format!(
        "  {}typealias MockValueCollectionType = Array<Mock<{}>>\n",
        access_modifier, swift_name,
    ));

    // MockFields struct
    result.push('\n');
    result.push_str(&format!(
        "  {}struct MockFields {{\n",
        access_modifier,
    ));
    for field in fields {
        if let Some(ref reason) = field.deprecation_reason {
            result.push_str(&format!(
                "    @available(*, deprecated, message: \"{}\")\n",
                reason.replace('\"', "\\\"")
            ));
        }
        result.push_str(&format!(
            "    @Field<{}>(\"{}\")",
            field.field_type_str, field.response_key,
        ));
        result.push_str(&format!(" public var {}\n", field.property_name));
    }
    result.push_str("  }\n");
    result.push_str("}\n");

    // Extension with convenience init (only if there are fields)
    if !fields.is_empty() {
        result.push('\n');
        result.push_str(&format!(
            "{}extension Mock where O == {} {{\n",
            access_modifier, swift_name,
        ));
        result.push_str("  convenience init(\n");
        for (i, field) in fields.iter().enumerate() {
            let param = if let Some(ref init_name) = field.initializer_param_name {
                format!("{} {}", field.property_name, init_name)
            } else {
                field.property_name.clone()
            };
            let comma = if i < fields.len() - 1 { "," } else { "" };
            result.push_str(&format!(
                "    {}: {}? = nil{}\n",
                param, field.mock_type_str, comma,
            ));
        }
        result.push_str("  ) {\n");
        result.push_str("    self.init()\n");
        for field in fields {
            let arg_name = field
                .initializer_param_name
                .as_deref()
                .unwrap_or(&field.property_name);
            result.push_str(&format!(
                "    {}({}, for: \\.{})\n",
                field.set_function, arg_name, field.property_name,
            ));
        }
        result.push_str("  }\n");
        result.push_str("}\n");
    }

    result
}
