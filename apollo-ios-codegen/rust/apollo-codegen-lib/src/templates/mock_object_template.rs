//! Mock object template for Apollo iOS code generation.
//!
//! Renders a `final class` with MockFields struct, @Field properties,
//! and convenience initializer matching Swift byte-for-byte.
//!
//! Mirrors Swift's `MockObjectTemplate.swift` from
//! `Sources/ApolloCodegenLib/Templates/MockObjectTemplate.swift`.

use std::sync::Arc;

use graphql_compiler::graphql_type::GraphQLType;
use graphql_compiler::schema::{
    GraphQLCompositeType, GraphQLNamedType, GraphQLObjectType,
};

use crate::config::composition::Composition;
use crate::config::ApolloCodegenConfiguration;
use crate::templates::rendering_helpers::graphql_name_rendering::{
    render_enum_value, render_named_type, EnumRenderContext, RenderContext,
};
use crate::templates::rendering_helpers::graphql_type_rendered::{rendered, TypeRenderContext};
use crate::templates::rendering_helpers::string_casing::first_uppercased;
use crate::templates::rendering_helpers::string_swift_name_escaping::{
    as_test_mock_field_property_name, as_test_mock_initializer_parameter_name,
    is_conflicting_test_mock_field_name,
};
use crate::templates::{
    ConfigurationContext, NonFatalErrorRecorder, Scope, TemplateRenderer, TemplateTarget,
};

/// Provides the format to convert a GraphQL ObjectType into a Swift test mock class.
///
/// Mirrors Swift's `MockObjectTemplate` struct.
pub struct MockObjectTemplate {
    pub graphql_object: Arc<GraphQLObjectType>,
    pub fields: Vec<(String, GraphQLType, Option<String>)>, // (response_key, type, deprecation_reason)
    pub config: ConfigurationContext,
}

/// Internal representation of a template field.
struct TemplateField {
    response_key: String,
    property_name: String,
    initializer_parameter_name: Option<String>,
    graphql_type: GraphQLType,
    mock_type: String,
    deprecation_reason: Option<String>,
}

impl TemplateField {
    fn default_initializer(&self, config: &ApolloCodegenConfiguration) -> String {
        if self.graphql_type.is_nullable() {
            " = nil".to_string()
        } else {
            format!(" = {}", default_mock_value(&self.graphql_type, config))
        }
    }
}

impl TemplateRenderer for MockObjectTemplate {
    fn config(&self) -> &ConfigurationContext {
        &self.config
    }

    fn target(&self) -> TemplateTarget {
        TemplateTarget::TestMockFile
    }

    fn render_body_template(
        &self,
        _non_fatal_error_recorder: &NonFatalErrorRecorder,
    ) -> String {
        let object_name = render_named_type(
            &GraphQLNamedType::Object(Arc::clone(&self.graphql_object)),
            &RenderContext::Typename { is_input_value: false },
        );

        let mut sorted_fields = self.fields.clone();
        sorted_fields.sort_by(|a, b| a.0.cmp(&b.0));

        let template_fields: Vec<TemplateField> = sorted_fields
            .iter()
            .map(|(key, gql_type, deprecation)| {
                let property_name = as_test_mock_field_property_name(key);
                let init_param = as_test_mock_initializer_parameter_name(key);
                let mock_type = mock_type_name(gql_type, &self.config.config);
                TemplateField {
                    response_key: key.clone(),
                    property_name,
                    initializer_parameter_name: init_param,
                    graphql_type: gql_type.clone(),
                    mock_type,
                    deprecation_reason: deprecation.clone(),
                }
            })
            .collect();

        let member_access = self.access_control_renderer(Scope::Member).render();
        let parent_access = self.access_control_renderer(Scope::Parent).render();

        let schema_ns = first_uppercased(self.config.schema_namespace());

        // Render the mock fields
        let mock_fields_body = if template_fields.is_empty() {
            String::new() // empty struct body
        } else {
            template_fields
                .iter()
                .map(|f| {
                    let deprecation_line = render_deprecation(
                        f.deprecation_reason.as_deref(),
                        &self.config.config,
                    );
                    let field_type = rendered(
                        &f.graphql_type,
                        &TypeRenderContext::TestMockField { force_non_null: true },
                        None,
                        &self.config.config,
                    );
                    format!(
                        "{}    {}@Field<{}>(\"{}\") public var {}",
                        deprecation_line,
                        if deprecation_line.is_empty() { "" } else { "    " },
                        field_type,
                        f.response_key,
                        f.property_name,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let mut result = String::new();

        // Class definition
        result.push_str(&format!(
            "{}final class {}: MockObject {{\n",
            parent_access, object_name,
        ));
        result.push_str(&format!(
            "  {}static let objectType: ApolloAPI.Object = {}.Objects.{}\n",
            member_access, schema_ns, object_name,
        ));
        result.push_str(&format!(
            "  {}static let _mockFields = MockFields()\n",
            member_access,
        ));
        result.push_str(&format!(
            "  {}typealias MockValueCollectionType = Array<Mock<{}>>\n",
            member_access, object_name,
        ));
        result.push('\n');
        if mock_fields_body.is_empty() {
            result.push_str(&format!(
                "  {}struct MockFields: Sendable {{\n  }}\n",
                member_access,
            ));
        } else {
            result.push_str(&format!(
                "  {}struct MockFields: Sendable {{\n{}\n  }}\n",
                member_access, mock_fields_body,
            ));
        }
        result.push_str("}\n");

        // Convenience initializer extension (only if fields are non-empty)
        if !template_fields.is_empty() {
            result.push('\n');
            result.push_str(&format!(
                "{}extension Mock where O == {} {{\n",
                parent_access, object_name,
            ));

            // Conflicting field name properties
            let conflicting = conflicting_field_name_properties(&template_fields);
            result.push_str(&conflicting);

            // Convenience init
            result.push_str("  convenience init(\n");

            let init_params: Vec<String> = template_fields
                .iter()
                .map(|f| {
                    let param_name = if let Some(ref init_name) = f.initializer_parameter_name {
                        format!("{} {}", f.property_name, init_name)
                    } else {
                        f.property_name.clone()
                    };
                    let default_init = f.default_initializer(&self.config.config);
                    format!("    {}: {}{}", param_name, f.mock_type, default_init)
                })
                .collect();

            result.push_str(&init_params.join(",\n"));
            result.push('\n');
            result.push_str("  ) {\n");
            result.push_str("    self.init()\n");

            for f in &template_fields {
                let descriptor = mock_function_descriptor(&f.graphql_type);
                let arg_name = f
                    .initializer_parameter_name
                    .as_ref()
                    .unwrap_or(&f.property_name);
                result.push_str(&format!(
                    "    _set{}({}, for: \\.{})\n",
                    descriptor, arg_name, f.property_name,
                ));
            }

            result.push_str("  }\n");
            result.push_str("}\n");
        }

        result
    }
}

/// Renders the deprecation warning annotation if applicable.
fn render_deprecation(
    deprecation_reason: Option<&str>,
    config: &ApolloCodegenConfiguration,
) -> String {
    match (deprecation_reason, config.options.warnings_on_deprecated_usage) {
        (Some(reason), Composition::Include) => {
            format!("@available(*, deprecated, message: \"{}\")\n", reason)
        }
        _ => String::new(),
    }
}

/// Returns the default mock value for the given GraphQL type.
///
/// Mirrors Swift's `GraphQLType.defaultMockValue(config:)` from
/// `DefaultMockValueProviding.swift`.
fn default_mock_value(graphql_type: &GraphQLType, config: &ApolloCodegenConfiguration) -> String {
    match graphql_type {
        GraphQLType::List(_) => "[]".to_string(),
        GraphQLType::NonNull(inner) => default_mock_value(inner, config),
        GraphQLType::Scalar(scalar) => match scalar.name.schema_name.as_str() {
            "String" | "ID" => "\"\"".to_string(),
            "Int" => "0".to_string(),
            "Float" => "0.0".to_string(),
            "Boolean" => "false".to_string(),
            _ => "try! .init(_jsonValue: \"\")".to_string(),
        },
        GraphQLType::Enum(enum_type) => {
            if let Some(first) = enum_type.values.first() {
                let case_name = render_enum_value(first, EnumRenderContext::EnumCase, config);
                format!(".case(.{})", case_name)
            } else {
                panic!(
                    "Cannot provide a default value for caseless enum {}",
                    enum_type.name.schema_name
                )
            }
        }
        GraphQLType::Entity(composite) => match composite {
            GraphQLCompositeType::Object(obj) => {
                let name = render_named_type(
                    &GraphQLNamedType::Object(Arc::clone(obj)),
                    &RenderContext::Typename { is_input_value: false },
                );
                format!("Mock<{}>()", name)
            }
            GraphQLCompositeType::Interface(iface) => {
                let impl_obj = iface.implementing_objects.first().unwrap_or_else(|| {
                    panic!(
                        "Cannot provide a default value for interface {} because no types conform to it.",
                        iface.name.schema_name
                    )
                });
                // Swift uses `implementingObject.name` directly (raw schema name)
                format!("Mock<{}>()", impl_obj.name.schema_name)
            }
            GraphQLCompositeType::Union(union) => {
                let impl_type = union.types.first().unwrap_or_else(|| {
                    panic!(
                        "Cannot provide a default value for empty union {}",
                        union.name.schema_name
                    )
                });
                // Swift uses `implementingType.name` directly (raw schema name)
                format!("Mock<{}>()", impl_type.name.schema_name)
            }
        },
        GraphQLType::InputObject(_) => panic!("InputObjects aren't mocked"),
    }
}

/// Returns the mock function descriptor string for use in `_set{Descriptor}(...)`.
///
/// Mirrors Swift's `MockObjectTemplate.mockFunctionDescriptor(_:)`.
fn mock_function_descriptor(graphql_type: &GraphQLType) -> &'static str {
    match graphql_type {
        GraphQLType::List(inner) => match inner.as_ref() {
            GraphQLType::NonNull(inner2) => match inner2.as_ref() {
                GraphQLType::List(_) => mock_function_descriptor(inner),
                GraphQLType::Entity(_) => "List",
                _ => "ScalarList",
            },
            GraphQLType::List(_) => mock_function_descriptor(inner),
            GraphQLType::Entity(_) => "List",
            _ => "ScalarList",
        },
        GraphQLType::Scalar(_) | GraphQLType::Enum(_) => "Scalar",
        GraphQLType::Entity(_) => "Entity",
        GraphQLType::InputObject(_) => {
            panic!("Input object found when determining mock set function descriptor.")
        }
        GraphQLType::NonNull(inner) => mock_function_descriptor(inner),
    }
}

/// Returns the mock type name for use in the convenience initializer parameter type.
///
/// Mirrors Swift's `MockObjectTemplate.mockTypeName(for:)`.
fn mock_type_name(graphql_type: &GraphQLType, config: &ApolloCodegenConfiguration) -> String {
    fn name_replacement(
        graphql_type: &GraphQLType,
        force_non_null: bool,
        config: &ApolloCodegenConfiguration,
    ) -> String {
        match graphql_type {
            GraphQLType::Entity(composite) => {
                let mock_type = match composite {
                    GraphQLCompositeType::Interface(_) | GraphQLCompositeType::Union(_) => {
                        "(any AnyMock)".to_string()
                    }
                    GraphQLCompositeType::Object(obj) => {
                        let name = render_named_type(
                            &GraphQLNamedType::Object(Arc::clone(obj)),
                            &RenderContext::Typename { is_input_value: false },
                        );
                        format!("Mock<{}>", name)
                    }
                };
                if force_non_null {
                    mock_type
                } else {
                    format!("{}?", mock_type)
                }
            }
            GraphQLType::Scalar(_) | GraphQLType::Enum(_) | GraphQLType::InputObject(_) => {
                let type_str = rendered(
                    graphql_type,
                    &TypeRenderContext::TestMockField { force_non_null: true },
                    None,
                    config,
                );
                if force_non_null {
                    type_str
                } else {
                    format!("{}?", type_str)
                }
            }
            GraphQLType::NonNull(inner) => name_replacement(inner, true, config),
            GraphQLType::List(inner) => {
                let inner_name = name_replacement(inner, false, config);
                if force_non_null {
                    format!("[{}]", inner_name)
                } else {
                    format!("[{}]?", inner_name)
                }
            }
        }
    }

    name_replacement(graphql_type, false, config)
}

/// Renders explicit property declarations for fields that conflict with `Mock` properties.
///
/// Mirrors Swift's `MockObjectTemplate.conflictingFieldNameProperties(_:)`.
fn conflicting_field_name_properties(fields: &[TemplateField]) -> String {
    let mut result = String::new();
    for f in fields {
        if is_conflicting_test_mock_field_name(&f.response_key) {
            let descriptor = mock_function_descriptor(&f.graphql_type);
            result.push_str(&format!(
                "  var {}: {}? {{\n    get {{ _data[\"{}\"] as? {} }}\n    set {{ _set{}(newValue, for: \\.{}) }}\n  }}\n",
                f.property_name,
                f.mock_type,
                f.property_name,
                f.mock_type,
                descriptor,
                f.property_name,
            ));
            result.push('\n');
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ApolloCodegenConfiguration;
    use crate::templates::ConfigurationContext;
    use graphql_compiler::graphql_name::GraphQLName;
    use graphql_compiler::schema::{
        GraphQLEnumType, GraphQLEnumValue, GraphQLInterfaceType, GraphQLScalarType,
        GraphQLUnionType,
    };
    use indexmap::IndexMap;
    use std::sync::Arc;

    fn make_config_json(
        schema_namespace: &str,
        test_mocks_json: &str,
        warnings: &str,
    ) -> ConfigurationContext {
        let json = format!(
            r#"{{
            "schemaNamespace": "{}",
            "input": {{}},
            "output": {{
                "schemaTypes": {{ "path": "./gen", "moduleType": {{"swiftPackageManager": {{}}}} }},
                "operations": {{"inSchemaModule": {{}}}},
                "testMocks": {}
            }},
            "options": {{
                "warningsOnDeprecatedUsage": "{}"
            }}
        }}"#,
            schema_namespace, test_mocks_json, warnings,
        );
        let config: ApolloCodegenConfiguration = serde_json::from_str(&json).unwrap();
        ConfigurationContext::new(config, None)
    }

    fn swift_package_config() -> ConfigurationContext {
        make_config_json(
            "TestSchema",
            r#"{"swiftPackage": {"targetName": null}}"#,
            "exclude",
        )
    }

    fn absolute_public_config() -> ConfigurationContext {
        make_config_json(
            "TestSchema",
            r#"{"absolute": {"path": "", "accessModifier": "public"}}"#,
            "exclude",
        )
    }

    fn absolute_internal_config() -> ConfigurationContext {
        make_config_json(
            "TestSchema",
            r#"{"absolute": {"path": "", "accessModifier": "internal"}}"#,
            "exclude",
        )
    }

    fn mock_object(name: &str) -> Arc<GraphQLObjectType> {
        Arc::new(GraphQLObjectType {
            name: GraphQLName::new(name.to_string()),
            documentation: None,
            fields: IndexMap::new(),
            interfaces: vec![],
            key_fields: None,
        })
    }

    fn mock_scalar(name: &str) -> Arc<GraphQLScalarType> {
        Arc::new(GraphQLScalarType {
            name: GraphQLName::new(name.to_string()),
            documentation: None,
            specified_by_url: None,
        })
    }

    fn mock_enum_type(name: &str, values: &[&str]) -> Arc<GraphQLEnumType> {
        Arc::new(GraphQLEnumType {
            name: GraphQLName::new(name.to_string()),
            documentation: None,
            values: values
                .iter()
                .map(|v| GraphQLEnumValue {
                    name: GraphQLName::new(v.to_string()),
                    documentation: None,
                    deprecation_reason: None,
                })
                .collect(),
        })
    }

    fn mock_interface(
        name: &str,
        implementing: &[Arc<GraphQLObjectType>],
    ) -> Arc<GraphQLInterfaceType> {
        Arc::new(GraphQLInterfaceType {
            name: GraphQLName::new(name.to_string()),
            documentation: None,
            fields: IndexMap::new(),
            interfaces: vec![],
            key_fields: None,
            implementing_objects: implementing.to_vec(),
        })
    }

    fn mock_union(name: &str, types: &[Arc<GraphQLObjectType>]) -> Arc<GraphQLUnionType> {
        Arc::new(GraphQLUnionType {
            name: GraphQLName::new(name.to_string()),
            documentation: None,
            types: types.to_vec(),
        })
    }

    fn string_type() -> GraphQLType {
        GraphQLType::Scalar(mock_scalar("String"))
    }

    fn non_null_string() -> GraphQLType {
        GraphQLType::NonNull(Box::new(string_type()))
    }

    fn build_subject(
        name: &str,
        custom_name: Option<&str>,
        fields: Vec<(&str, GraphQLType, Option<&str>)>,
        config: ConfigurationContext,
    ) -> MockObjectTemplate {
        let mut obj = GraphQLObjectType {
            name: GraphQLName::new(name.to_string()),
            documentation: None,
            fields: IndexMap::new(),
            interfaces: vec![],
            key_fields: None,
        };
        if let Some(cn) = custom_name {
            obj.name.custom_name = Some(cn.to_string());
        }
        MockObjectTemplate {
            graphql_object: Arc::new(obj),
            fields: fields
                .into_iter()
                .map(|(k, t, d)| (k.to_string(), t, d.map(|s| s.to_string())))
                .collect(),
            config,
        }
    }

    fn render_body(subject: &MockObjectTemplate) -> String {
        let recorder = NonFatalErrorRecorder::new();
        subject.render_body_template(&recorder)
    }

    // MARK: - Boilerplate Tests

    #[test]
    fn test__target__is_test_mock_file() {
        let subject = build_subject("Dog", None, vec![], swift_package_config());
        assert!(matches!(subject.target(), TemplateTarget::TestMockFile));
    }

    // MARK: - Class Rendering Tests

    #[test]
    fn test__render__given_schema_type_generates_extension() {
        let subject = build_subject("Dog", None, vec![], swift_package_config());
        let actual = render_body(&subject);

        let expected = "\
public final class Dog: MockObject {
  public static let objectType: ApolloAPI.Object = TestSchema.Objects.Dog
  public static let _mockFields = MockFields()
  public typealias MockValueCollectionType = Array<Mock<Dog>>

  public struct MockFields: Sendable {
  }
}
\n";

        assert_eq!(actual, expected);
    }

    // MARK: - Casing Tests

    #[test]
    fn test__render__given_schema_type_with_lowercase_name_generates_capitalized_class_name() {
        let subject = build_subject("dog", None, vec![], swift_package_config());
        let actual = render_body(&subject);

        assert!(actual.contains("public final class Dog: MockObject {"));
        assert!(actual.contains("TestSchema.Objects.Dog"));
        assert!(actual.contains("Array<Mock<Dog>>"));
    }

    #[test]
    fn test__render__given_lowercased_schema_name_generates_first_uppercased_schema_name_references()
    {
        let config = make_config_json(
            "lowercased",
            r#"{"swiftPackage": {"targetName": null}}"#,
            "exclude",
        );
        let subject = build_subject("Dog", None, vec![], config);
        let actual = render_body(&subject);

        assert!(actual.contains("Lowercased.Objects.Dog"));
    }

    #[test]
    fn test__render__given_uppercased_schema_name_generates_capitalized_schema_name_references() {
        let config = make_config_json(
            "UPPER",
            r#"{"swiftPackage": {"targetName": null}}"#,
            "exclude",
        );
        let subject = build_subject("Dog", None, vec![], config);
        let actual = render_body(&subject);

        assert!(actual.contains("UPPER.Objects.Dog"));
    }

    #[test]
    fn test__render__given_capitalized_schema_name_generates_capitalized_schema_name_references() {
        let config = make_config_json(
            "MySchema",
            r#"{"swiftPackage": {"targetName": null}}"#,
            "exclude",
        );
        let subject = build_subject("Dog", None, vec![], config);
        let actual = render_body(&subject);

        assert!(actual.contains("MySchema.Objects.Dog"));
    }

    // MARK: - Field Accessor Tests

    #[test]
    fn test__render__given_schema_type_generates_field_accessors() {
        let cat_entity =
            GraphQLType::Entity(GraphQLCompositeType::Object(mock_object("Cat")));

        let fields = vec![
            ("string", non_null_string(), None),
            (
                "customScalar",
                GraphQLType::NonNull(Box::new(GraphQLType::Scalar(mock_scalar("CustomScalar")))),
                None,
            ),
            ("optionalString", string_type(), None),
            ("object", cat_entity.clone(), None),
            (
                "objectList",
                GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(cat_entity.clone())))),
                None,
            ),
            (
                "objectNestedList",
                GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(GraphQLType::List(
                    Box::new(GraphQLType::NonNull(Box::new(cat_entity.clone()))),
                ))))),
                None,
            ),
            (
                "objectOptionalList",
                GraphQLType::List(Box::new(cat_entity.clone())),
                None,
            ),
        ];

        let subject = build_subject("Dog", None, fields, swift_package_config());
        let actual = render_body(&subject);

        assert!(actual.contains(r#"@Field<TestSchema.CustomScalar>("customScalar") public var customScalar"#));
        assert!(actual.contains(r#"@Field<Cat>("object") public var object"#));
        assert!(actual.contains(r#"@Field<[Cat]>("objectList") public var objectList"#));
        assert!(actual.contains(r#"@Field<[[Cat]]>("objectNestedList") public var objectNestedList"#));
        assert!(actual.contains(r#"@Field<[Cat?]>("objectOptionalList") public var objectOptionalList"#));
        assert!(actual.contains(r#"@Field<String>("optionalString") public var optionalString"#));
        assert!(actual.contains(r#"@Field<String>("string") public var string"#));
    }

    #[test]
    fn test__render__given_fields_with_lowercase_type_names_generates_field_accessors() {
        let cat_entity =
            GraphQLType::Entity(GraphQLCompositeType::Object(mock_object("cat")));

        let fields = vec![
            (
                "customScalar",
                GraphQLType::NonNull(Box::new(GraphQLType::Scalar(mock_scalar("customScalar")))),
                None,
            ),
            (
                "enumType",
                GraphQLType::Enum(mock_enum_type("enumType", &[])),
                None,
            ),
            ("object", cat_entity, None),
        ];

        let subject = build_subject("Dog", None, fields, swift_package_config());
        let actual = render_body(&subject);

        assert!(actual.contains(r#"@Field<TestSchema.CustomScalar>("customScalar") public var customScalar"#));
        assert!(actual.contains(r#"@Field<GraphQLEnum<TestSchema.EnumType>>("enumType") public var enumType"#));
        assert!(actual.contains(r#"@Field<Cat>("object") public var object"#));
    }

    #[test]
    fn test__render__given_fields_with_swift_reserved_keyword_names_generates_fields_escaped_with_backticks()
    {
        let keywords = vec![
            "associatedtype", "class", "deinit", "enum", "extension", "fileprivate", "func",
            "import", "init", "inout", "internal", "let", "operator", "private",
            "precedencegroup", "protocol", "Protocol", "public", "rethrows", "static", "struct",
            "subscript", "typealias", "var", "break", "case", "catch", "continue", "default",
            "defer", "do", "else", "fallthrough", "for", "guard", "if", "in", "repeat", "return",
            "throw", "switch", "where", "while", "as", "false", "is", "nil", "self", "Self",
            "super", "throws", "true", "try", "Type", "Any",
        ];

        let fields: Vec<(&str, GraphQLType, Option<&str>)> = keywords
            .iter()
            .map(|k| (*k, non_null_string(), None))
            .collect();

        let subject = build_subject("Dog", None, fields, swift_package_config());
        let actual = render_body(&subject);

        assert!(actual.contains(r#"@Field<String>("Any") public var `Any`"#));
        assert!(actual.contains(r#"@Field<String>("Protocol") public var `Protocol`"#));
        assert!(actual.contains(r#"@Field<String>("Self") public var `Self`"#));
        assert!(actual.contains(r#"@Field<String>("Type") public var `Type`"#));
        assert!(actual.contains(r#"@Field<String>("as") public var `as`"#));
        assert!(actual.contains(r#"@Field<String>("class") public var `class`"#));
        assert!(actual.contains(r#"@Field<String>("self") public var `self`"#));
        assert!(actual.contains(r#"@Field<String>("var") public var `var`"#));
    }

    #[test]
    fn test__render__given_field_type_interface_named_actor_generates_fields_with_namespace() {
        let actor_interface =
            GraphQLType::Entity(GraphQLCompositeType::Interface(mock_interface("Actor", &[])));

        let fields = vec![("actor", actor_interface, None)];
        let subject = build_subject("Dog", None, fields, swift_package_config());
        let actual = render_body(&subject);

        assert!(actual.contains(r#"@Field<MockObject.Actor>("actor") public var actor"#));
    }

    #[test]
    fn test__render__given_field_type_union_named_actor_generates_fields_with_namespace() {
        let actor_union =
            GraphQLType::Entity(GraphQLCompositeType::Union(mock_union("Actor", &[])));

        let fields = vec![("actor", actor_union, None)];
        let subject = build_subject("Dog", None, fields, swift_package_config());
        let actual = render_body(&subject);

        assert!(actual.contains(r#"@Field<MockObject.Actor>("actor") public var actor"#));
    }

    #[test]
    fn test__render__given_field_type_object_named_actor_generates_fields_without_namespace() {
        let actor_object =
            GraphQLType::Entity(GraphQLCompositeType::Object(mock_object("Actor")));

        let fields = vec![("actor", actor_object, None)];
        let subject = build_subject("Dog", None, fields, swift_package_config());
        let actual = render_body(&subject);

        assert!(actual.contains(r#"@Field<Actor>("actor") public var actor"#));
    }

    // MARK: - Conflicting Field Name Tests

    #[test]
    fn test__render__given_conflicting_field_name_generates_property_with_field_name() {
        let fields = vec![("hash", non_null_string(), None)];
        let subject = build_subject("Dog", None, fields, swift_package_config());
        let actual = render_body(&subject);

        assert!(actual.contains("var hash: String? {"));
        assert!(actual.contains(r#"get { _data["hash"] as? String }"#));
        assert!(actual.contains(r"set { _setScalar(newValue, for: \.hash) }"));
    }

    // MARK: - Convenience Initializer Tests

    #[test]
    fn test__render__given_schema_type_generates_convenience_initializer() {
        let cat = GraphQLType::Entity(GraphQLCompositeType::Object(mock_object("Cat")));
        let animal = GraphQLType::Entity(GraphQLCompositeType::Interface(mock_interface(
            "Animal",
            &[],
        )));
        let pet = GraphQLType::Entity(GraphQLCompositeType::Union(mock_union("Pet", &[])));

        let fields = vec![
            ("string", non_null_string(), None),
            (
                "stringList",
                GraphQLType::List(Box::new(non_null_string())),
                None,
            ),
            (
                "stringNestedList",
                GraphQLType::List(Box::new(GraphQLType::List(Box::new(non_null_string())))),
                None,
            ),
            (
                "stringOptionalList",
                GraphQLType::List(Box::new(string_type())),
                None,
            ),
            (
                "customScalar",
                GraphQLType::NonNull(Box::new(GraphQLType::Scalar(mock_scalar("CustomScalar")))),
                None,
            ),
            (
                "customScalarList",
                GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(GraphQLType::Scalar(
                    mock_scalar("CustomScalar"),
                ))))),
                None,
            ),
            (
                "customScalarOptionalList",
                GraphQLType::List(Box::new(GraphQLType::Scalar(mock_scalar("CustomScalar")))),
                None,
            ),
            ("optionalString", string_type(), None),
            ("object", cat.clone(), None),
            (
                "objectList",
                GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(cat.clone())))),
                None,
            ),
            (
                "objectNestedList",
                GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(GraphQLType::List(
                    Box::new(GraphQLType::NonNull(Box::new(cat.clone()))),
                ))))),
                None,
            ),
            (
                "objectOptionalList",
                GraphQLType::List(Box::new(cat.clone())),
                None,
            ),
            ("interface", animal.clone(), None),
            (
                "interfaceList",
                GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(animal.clone())))),
                None,
            ),
            (
                "interfaceNestedList",
                GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(GraphQLType::List(
                    Box::new(GraphQLType::NonNull(Box::new(animal.clone()))),
                ))))),
                None,
            ),
            (
                "interfaceOptionalList",
                GraphQLType::List(Box::new(animal.clone())),
                None,
            ),
            ("union", pet.clone(), None),
            (
                "unionList",
                GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(pet.clone())))),
                None,
            ),
            (
                "unionNestedList",
                GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(GraphQLType::List(
                    Box::new(GraphQLType::NonNull(Box::new(pet.clone()))),
                ))))),
                None,
            ),
            (
                "unionOptionalList",
                GraphQLType::List(Box::new(pet.clone())),
                None,
            ),
            (
                "enumType",
                GraphQLType::Enum(mock_enum_type("enumType", &[])),
                None,
            ),
            (
                "enumList",
                GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(GraphQLType::Enum(
                    mock_enum_type("enumType", &[]),
                ))))),
                None,
            ),
            (
                "enumOptionalList",
                GraphQLType::List(Box::new(GraphQLType::Enum(mock_enum_type("enumType", &[])))),
                None,
            ),
        ];

        let subject = build_subject("Dog", None, fields, swift_package_config());
        let actual = render_body(&subject);

        // Verify the extension is present
        assert!(actual.contains("public extension Mock where O == Dog {"));
        // Verify sorted init params with correct types
        assert!(actual.contains("customScalar: TestSchema.CustomScalar = .defaultMockValue"));
        assert!(actual.contains("customScalarList: [TestSchema.CustomScalar]? = nil"));
        assert!(actual.contains("customScalarOptionalList: [TestSchema.CustomScalar?]? = nil"));
        assert!(actual.contains("enumList: [GraphQLEnum<TestSchema.EnumType>]? = nil"));
        assert!(actual.contains("enumOptionalList: [GraphQLEnum<TestSchema.EnumType>?]? = nil"));
        assert!(actual.contains("enumType: GraphQLEnum<TestSchema.EnumType>? = nil"));
        assert!(actual.contains("interface: (any AnyMock)? = nil"));
        assert!(actual.contains("interfaceList: [(any AnyMock)]? = nil"));
        assert!(actual.contains("interfaceNestedList: [[(any AnyMock)]]? = nil"));
        assert!(actual.contains("interfaceOptionalList: [(any AnyMock)?]? = nil"));
        assert!(actual.contains("object: Mock<Cat>? = nil"));
        assert!(actual.contains("objectList: [Mock<Cat>]? = nil"));
        assert!(actual.contains("objectNestedList: [[Mock<Cat>]]? = nil"));
        assert!(actual.contains("objectOptionalList: [Mock<Cat>?]? = nil"));
        assert!(actual.contains("optionalString: String? = nil"));
        assert!(actual.contains("string: String = \"\""));
        assert!(actual.contains("stringList: [String]? = nil"));
        assert!(actual.contains("stringNestedList: [[String]?]? = nil"));
        assert!(actual.contains("stringOptionalList: [String?]? = nil"));
        assert!(actual.contains("union: (any AnyMock)? = nil"));
        assert!(actual.contains("unionList: [(any AnyMock)]? = nil"));
        assert!(actual.contains("unionNestedList: [[(any AnyMock)]]? = nil"));
        assert!(actual.contains("unionOptionalList: [(any AnyMock)?]? = nil"));
        // Verify set calls
        assert!(actual.contains(r"_setScalar(customScalar, for: \.customScalar)"));
        assert!(actual.contains(r"_setScalarList(customScalarList, for: \.customScalarList)"));
        assert!(actual.contains(r"_setEntity(object, for: \.object)"));
        assert!(actual.contains(r"_setList(objectList, for: \.objectList)"));
        assert!(actual.contains(r"_setEntity(interface, for: \.interface)"));
        assert!(actual.contains(r"_setList(interfaceList, for: \.interfaceList)"));
        assert!(actual.contains(r"_setEntity(union, for: \.union)"));
        assert!(actual.contains(r"_setList(unionList, for: \.unionList)"));
    }

    #[test]
    fn test__render__given_schema_type_and_default_parameter_flag_on_generates_default_value_for_required_fields()
    {
        let aardvark = GraphQLType::Entity(GraphQLCompositeType::Object(mock_object("aardvark")));
        let cat = GraphQLType::Entity(GraphQLCompositeType::Object(mock_object("Cat")));
        let duck = mock_object("Duck");
        let animal = GraphQLType::Entity(GraphQLCompositeType::Interface(mock_interface(
            "Animal",
            &[duck],
        )));
        let goldfish = mock_object("Goldfish");
        let hamster = mock_object("Hamster");
        let pet = GraphQLType::Entity(GraphQLCompositeType::Union(mock_union(
            "Pet",
            &[goldfish, hamster],
        )));

        let fields = vec![
            ("string", GraphQLType::NonNull(Box::new(string_type())), None),
            (
                "stringList",
                GraphQLType::NonNull(Box::new(GraphQLType::List(Box::new(non_null_string())))),
                None,
            ),
            (
                "stringNestedList",
                GraphQLType::NonNull(Box::new(GraphQLType::List(Box::new(GraphQLType::NonNull(
                    Box::new(GraphQLType::List(Box::new(non_null_string()))),
                ))))),
                None,
            ),
            (
                "customScalar",
                GraphQLType::NonNull(Box::new(GraphQLType::Scalar(mock_scalar("CustomScalar")))),
                None,
            ),
            (
                "customScalarList",
                GraphQLType::NonNull(Box::new(GraphQLType::List(Box::new(GraphQLType::NonNull(
                    Box::new(GraphQLType::Scalar(mock_scalar("CustomScalar"))),
                ))))),
                None,
            ),
            (
                "lowercaseObject",
                GraphQLType::NonNull(Box::new(aardvark)),
                None,
            ),
            ("object", GraphQLType::NonNull(Box::new(cat.clone())), None),
            (
                "objectList",
                GraphQLType::NonNull(Box::new(GraphQLType::List(Box::new(GraphQLType::NonNull(
                    Box::new(cat.clone()),
                ))))),
                None,
            ),
            (
                "objectNestedList",
                GraphQLType::NonNull(Box::new(GraphQLType::List(Box::new(GraphQLType::NonNull(
                    Box::new(GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(
                        cat.clone(),
                    ))))),
                ))))),
                None,
            ),
            (
                "interface",
                GraphQLType::NonNull(Box::new(animal.clone())),
                None,
            ),
            (
                "interfaceList",
                GraphQLType::NonNull(Box::new(GraphQLType::List(Box::new(GraphQLType::NonNull(
                    Box::new(animal.clone()),
                ))))),
                None,
            ),
            (
                "interfaceNestedList",
                GraphQLType::NonNull(Box::new(GraphQLType::List(Box::new(GraphQLType::NonNull(
                    Box::new(GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(
                        animal.clone(),
                    ))))),
                ))))),
                None,
            ),
            ("union", GraphQLType::NonNull(Box::new(pet.clone())), None),
            (
                "unionList",
                GraphQLType::NonNull(Box::new(GraphQLType::List(Box::new(GraphQLType::NonNull(
                    Box::new(pet.clone()),
                ))))),
                None,
            ),
            (
                "unionNestedList",
                GraphQLType::NonNull(Box::new(GraphQLType::List(Box::new(GraphQLType::NonNull(
                    Box::new(GraphQLType::List(Box::new(GraphQLType::NonNull(Box::new(
                        pet.clone(),
                    ))))),
                ))))),
                None,
            ),
            (
                "enumType",
                GraphQLType::NonNull(Box::new(GraphQLType::Enum(mock_enum_type(
                    "enumType",
                    &["foo", "bar"],
                )))),
                None,
            ),
            (
                "enumList",
                GraphQLType::NonNull(Box::new(GraphQLType::List(Box::new(GraphQLType::NonNull(
                    Box::new(GraphQLType::Enum(mock_enum_type("enumType", &["foo", "bar"]))),
                ))))),
                None,
            ),
        ];

        let subject = build_subject("Dog", None, fields, swift_package_config());
        let actual = render_body(&subject);

        assert!(actual.contains("customScalar: TestSchema.CustomScalar = .defaultMockValue"));
        assert!(actual.contains("customScalarList: [TestSchema.CustomScalar] = []"));
        assert!(actual.contains("enumList: [GraphQLEnum<TestSchema.EnumType>] = []"));
        assert!(actual.contains("enumType: GraphQLEnum<TestSchema.EnumType> = .case(.foo)"));
        assert!(actual.contains("interface: (any AnyMock) = Mock<Duck>()"));
        assert!(actual.contains("interfaceList: [(any AnyMock)] = []"));
        assert!(actual.contains("interfaceNestedList: [[(any AnyMock)]] = []"));
        assert!(actual.contains("lowercaseObject: Mock<Aardvark> = Mock<Aardvark>()"));
        assert!(actual.contains("object: Mock<Cat> = Mock<Cat>()"));
        assert!(actual.contains("objectList: [Mock<Cat>] = []"));
        assert!(actual.contains("objectNestedList: [[Mock<Cat>]] = []"));
        assert!(actual.contains("string: String = \"\""));
        assert!(actual.contains("stringList: [String] = []"));
        assert!(actual.contains("stringNestedList: [[String]] = []"));
        assert!(actual.contains("union: (any AnyMock) = Mock<Goldfish>()"));
        assert!(actual.contains("unionList: [(any AnyMock)] = []"));
        assert!(actual.contains("unionNestedList: [[(any AnyMock)]] = []"));
    }

    #[test]
    fn test__render__given_schema_type_without_fields_does_not_generate_convenience_initializer() {
        let subject = build_subject("Dog", None, vec![], swift_package_config());
        let actual = render_body(&subject);

        assert!(!actual.contains("extension Mock where O == Dog"));
        assert!(!actual.contains("convenience init"));
        // Should end with the class closing brace and trailing newline
        assert!(actual.ends_with("}\n\n"));
    }

    #[test]
    fn test__render__given_fields_with_swift_reserved_keyword_names_generates_convenience_initializer_parameters_escaped_with_backticks_and_internal_names()
    {
        let keywords = vec![
            "associatedtype", "class", "deinit", "enum", "extension", "fileprivate", "func",
            "import", "init", "inout", "internal", "let", "operator", "private",
            "precedencegroup", "protocol", "Protocol", "public", "rethrows", "static", "struct",
            "subscript", "typealias", "var", "break", "case", "catch", "continue", "default",
            "defer", "do", "else", "fallthrough", "for", "guard", "if", "in", "repeat", "return",
            "throw", "switch", "where", "while", "as", "false", "is", "nil", "self", "Self",
            "super", "throws", "true", "try", "Type", "Any",
        ];

        let fields: Vec<(&str, GraphQLType, Option<&str>)> = keywords
            .iter()
            .map(|k| (*k, non_null_string(), None))
            .collect();

        let subject = build_subject("Dog", None, fields, swift_package_config());
        let actual = render_body(&subject);

        // Check the "self" field gets special treatment
        assert!(actual.contains("`self` self_value: String = \"\""));
        assert!(actual.contains(r"_setScalar(self_value, for: \.`self`)"));

        // Check backtick-escaped params
        assert!(actual.contains("`Any`: String = \"\""));
        assert!(actual.contains("`Protocol`: String = \"\""));
        assert!(actual.contains("`Self`: String = \"\""));
        assert!(actual.contains("`Type`: String = \"\""));
        assert!(actual.contains("`class`: String = \"\""));
        assert!(actual.contains("`var`: String = \"\""));

        // Check _setScalar calls use backticked names
        assert!(actual.contains(r"_setScalar(`Any`, for: \.`Any`)"));
        assert!(actual.contains(r"_setScalar(`class`, for: \.`class`)"));
    }

    // MARK: - Access Level Tests

    #[test]
    fn test__render__given_schema_type_and_fields_when_test_mocks_is_swift_package_should_render_with_public_access()
    {
        let fields = vec![("string", non_null_string(), None)];
        let subject = build_subject("Dog", None, fields, swift_package_config());
        let actual = render_body(&subject);

        assert!(actual.contains("public final class Dog: MockObject {"));
        assert!(actual.contains("public static let objectType"));
        assert!(actual.contains("public struct MockFields: Sendable {"));
        assert!(actual.contains("public extension Mock where O == Dog {"));
    }

    #[test]
    fn test__render__given_schema_type_when_test_mocks_absolute_with_public_access_modifier_should_render_with_public_access()
    {
        let fields = vec![("string", non_null_string(), None)];
        let subject = build_subject("Dog", None, fields, absolute_public_config());
        let actual = render_body(&subject);

        assert!(actual.contains("public final class Dog: MockObject {"));
        assert!(actual.contains("public extension Mock where O == Dog {"));
    }

    #[test]
    fn test__render__given_schema_type_when_test_mocks_absolute_with_internal_access_modifier_should_render_with_internal_access()
    {
        let fields = vec![("string", non_null_string(), None)];
        let subject = build_subject("Dog", None, fields, absolute_internal_config());
        let actual = render_body(&subject);

        assert!(actual.contains("final class Dog: MockObject {"));
        assert!(!actual.contains("public final class Dog"));
        assert!(actual.contains("static let objectType"));
        assert!(actual.contains("struct MockFields: Sendable {"));
        assert!(actual.contains("extension Mock where O == Dog {"));
        assert!(!actual.contains("public extension Mock"));
    }

    // MARK: - Deprecation Warning Tests

    #[test]
    fn test__render__given_warnings_on_deprecated_usage_include_has_deprecated_field_should_generate_warning()
    {
        let config = make_config_json(
            "TestSchema",
            r#"{"swiftPackage": {"targetName": null}}"#,
            "include",
        );
        let fields = vec![(
            "string",
            non_null_string(),
            Some("Cause I said so!"),
        )];
        let subject = build_subject("Dog", None, fields, config);
        let actual = render_body(&subject);

        assert!(actual.contains(r#"@available(*, deprecated, message: "Cause I said so!")"#));
        assert!(actual.contains(r#"@Field<String>("string") public var string"#));
    }

    #[test]
    fn test__render__given_warnings_on_deprecated_usage_exclude_has_deprecated_field_should_not_generate_warning()
    {
        let fields = vec![(
            "string",
            non_null_string(),
            Some("Cause I said so!"),
        )];
        let subject = build_subject("Dog", None, fields, swift_package_config());
        let actual = render_body(&subject);

        assert!(!actual.contains("@available"));
        assert!(actual.contains(r#"@Field<String>("string") public var string"#));
    }

    // MARK: - Reserved Keyword Type Name Tests

    #[test]
    fn test__render__given_object_using_reserved_keyword_generates_type_with_suffix() {
        for keyword in &["Type", "type"] {
            let fields = vec![("string", non_null_string(), None)];
            let subject = build_subject(keyword, None, fields, swift_package_config());
            let actual = render_body(&subject);

            assert!(
                actual.contains("public final class Type_Object: MockObject {"),
                "Expected 'Type_Object' for keyword '{}', got:\n{}",
                keyword,
                actual,
            );
            assert!(actual.contains("TestSchema.Objects.Type_Object"));
            assert!(actual.contains("Array<Mock<Type_Object>>"));
            assert!(actual.contains("extension Mock where O == Type_Object {"));
        }
    }

    // MARK: - Schema Customization Tests

    #[test]
    fn test__render__given_mock_object_with_custom_name_should_render_with_custom_name() {
        let subject = build_subject("MyObject", Some("MyCustomObject"), vec![], swift_package_config());
        let actual = render_body(&subject);

        assert!(actual.contains("public final class MyCustomObject: MockObject {"));
        assert!(actual.contains("TestSchema.Objects.MyCustomObject"));
        assert!(actual.contains("Array<Mock<MyCustomObject>>"));
    }
}
