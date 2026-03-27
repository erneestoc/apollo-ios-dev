//! Mock object template.
//!
//! Generates files like Dog+Mock.graphql.swift with MockObject class,
//! MockFields struct, and convenience initializer.

use askama::Template;

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

/// Pre-processed field data for the Askama template.
struct TemplateField {
    response_key: String,
    property_name: String,
    field_type_str: String,
    mock_type_str: String,
    set_function: String,
    escaped_deprecation_reason: Option<String>,
    init_param_str: String,
    init_arg_name: String,
}

#[derive(Template)]
#[template(path = "mock_object.swift.askama", escape = "none")]
struct MockObjectTemplate<'a> {
    access_modifier: &'a str,
    api_target_name: &'a str,
    import_module: &'a str,
    swift_name: String,
    ns: String,
    fields: Vec<TemplateField>,
}

pub fn render(
    object_name: &str,
    fields: &[MockField],
    access_modifier: &str,
    schema_namespace: &str,
    api_target_name: &str,
    import_module: &str,
) -> String {
    let swift_name = crate::naming::first_uppercased(object_name);
    let ns = crate::naming::first_uppercased(schema_namespace);

    let template_fields: Vec<TemplateField> = fields
        .iter()
        .map(|f| {
            let init_param_str = if let Some(ref init_name) = f.initializer_param_name {
                format!("{} {}", f.property_name, init_name)
            } else {
                f.property_name.clone()
            };
            let init_arg_name = f
                .initializer_param_name
                .as_deref()
                .unwrap_or(&f.property_name)
                .to_string();
            let escaped_deprecation_reason = f
                .deprecation_reason
                .as_ref()
                .map(|r| r.replace('\"', "\\\""));
            TemplateField {
                response_key: f.response_key.clone(),
                property_name: f.property_name.clone(),
                field_type_str: f.field_type_str.clone(),
                mock_type_str: f.mock_type_str.clone(),
                set_function: f.set_function.clone(),
                escaped_deprecation_reason,
                init_param_str,
                init_arg_name,
            }
        })
        .collect();

    let template = MockObjectTemplate {
        access_modifier,
        api_target_name,
        import_module,
        swift_name,
        ns,
        fields: template_fields,
    };

    let mut output = template.render().expect("mock_object template render failed");
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}
