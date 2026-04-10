//! Adapter layer converting apollo-compiler types to Swift-equivalent types.
//!
//! This module is the core conversion layer. It imports apollo-compiler types
//! internally and produces the Swift-equivalent types defined in `schema.rs`,
//! `graphql_type.rs`, `graphql_value.rs`, etc. No apollo-compiler types are
//! exposed past this module's boundary (per D-19).

use std::collections::HashSet;
use std::sync::Arc;

use apollo_compiler::ast;
use apollo_compiler::executable;
use apollo_compiler::schema::{self, ExtendedType};
use apollo_compiler::validation::Valid;
use indexmap::IndexMap;

use crate::compilation_result;
use crate::graphql_name::GraphQLName;
use crate::graphql_type::GraphQLType;
use crate::graphql_value::GraphQLValue;
use crate::schema::{
    GraphQLCompositeType, GraphQLEnumType, GraphQLEnumValue, GraphQLField, GraphQLFieldArgument,
    GraphQLInputField, GraphQLInputObjectType, GraphQLInterfaceType, GraphQLNamedType,
    GraphQLObjectType, GraphQLScalarType, GraphQLUnionType,
};

// MARK: - TypeRegistry

/// Registry of all named types in a GraphQL schema, providing Arc-based
/// shared ownership for type deduplication.
///
/// Built from an apollo-compiler `Schema` in two phases:
/// 1. Create all named type stubs (scalars, enums, input objects without
///    field type resolution)
/// 2. Create types with resolved field types using the registry for lookups
///
/// This ensures type identity: the same `Arc<GraphQLObjectType>` is used
/// everywhere a type is referenced.
pub struct TypeRegistry {
    types: IndexMap<String, GraphQLNamedType>,
}

impl TypeRegistry {
    /// Builds a TypeRegistry from a validated apollo-compiler Schema.
    ///
    /// This is a three-phase build:
    /// - Phase 1: Create all type stubs without field resolution (scalars,
    ///   enums, empty interfaces/objects/unions, input objects).
    /// - Phase 2: Build the complete registry from stubs so all type names
    ///   can be resolved.
    /// - Phase 3: Rebuild interfaces, objects, and unions with resolved
    ///   field types using the complete registry. Then rebuild input objects.
    pub fn from_schema(schema: &Valid<schema::Schema>) -> Self {
        let mut types = IndexMap::new();
        let mut interface_arcs: IndexMap<String, Arc<GraphQLInterfaceType>> = IndexMap::new();
        let mut object_arcs: IndexMap<String, Arc<GraphQLObjectType>> = IndexMap::new();

        // Phase 1: Create all types as stubs (no field type resolution).
        // This populates the registry with every type name so phase 2 can
        // resolve any type reference.
        for (name, ext_type) in &schema.types {
            let name_str = name.as_str().to_string();
            match ext_type {
                ExtendedType::Scalar(s) => {
                    let named = GraphQLNamedType::Scalar(Arc::new(GraphQLScalarType {
                        name: GraphQLName::new(name_str.clone()),
                        documentation: s.description.as_ref().map(|d| d.to_string()),
                        specified_by_url: extract_specified_by_url(&s.directives),
                    }));
                    types.insert(name_str, named);
                }
                ExtendedType::Enum(e) => {
                    let values: Vec<GraphQLEnumValue> = e
                        .values
                        .iter()
                        .map(|(_, v)| GraphQLEnumValue {
                            name: GraphQLName::new(v.value.to_string()),
                            documentation: v.description.as_ref().map(|d| d.to_string()),
                            deprecation_reason: extract_deprecation_reason(&v.directives),
                        })
                        .collect();
                    let named = GraphQLNamedType::Enum(Arc::new(GraphQLEnumType {
                        name: GraphQLName::new(name_str.clone()),
                        documentation: e.description.as_ref().map(|d| d.to_string()),
                        values,
                    }));
                    types.insert(name_str, named);
                }
                ExtendedType::InputObject(io) => {
                    // Stub with empty fields; will be rebuilt in phase 3
                    let named = GraphQLNamedType::InputObject(Arc::new(GraphQLInputObjectType {
                        name: GraphQLName::new(name_str.clone()),
                        documentation: io.description.as_ref().map(|d| d.to_string()),
                        is_one_of: io.directives.get("oneOf").is_some(),
                        fields: IndexMap::new(),
                    }));
                    types.insert(name_str, named);
                }
                ExtendedType::Interface(iface) => {
                    // Stub with empty fields; will be rebuilt in phase 3
                    let iface_arc = Arc::new(GraphQLInterfaceType {
                        name: GraphQLName::new(name_str.clone()),
                        documentation: iface.description.as_ref().map(|d| d.to_string()),
                        fields: IndexMap::new(),
                        interfaces: vec![],
                        key_fields: None,
                        implementing_objects: vec![],
                    });
                    interface_arcs.insert(name_str.clone(), Arc::clone(&iface_arc));
                    types.insert(name_str, GraphQLNamedType::Interface(iface_arc));
                }
                ExtendedType::Object(obj) => {
                    // Stub with empty fields; will be rebuilt in phase 3
                    let obj_arc = Arc::new(GraphQLObjectType {
                        name: GraphQLName::new(name_str.clone()),
                        documentation: obj.description.as_ref().map(|d| d.to_string()),
                        fields: IndexMap::new(),
                        interfaces: vec![],
                        key_fields: None,
                    });
                    object_arcs.insert(name_str.clone(), Arc::clone(&obj_arc));
                    types.insert(name_str, GraphQLNamedType::Object(obj_arc));
                }
                ExtendedType::Union(union) => {
                    // Stub with empty members; will be rebuilt in phase 3
                    let named = GraphQLNamedType::Union(Arc::new(GraphQLUnionType {
                        name: GraphQLName::new(name_str.clone()),
                        documentation: union.description.as_ref().map(|d| d.to_string()),
                        types: vec![],
                    }));
                    types.insert(name_str, named);
                }
            }
        }

        // Phase 2: Build the complete registry from stubs.
        // All type names are now resolvable.
        let full_registry = TypeRegistry {
            types: types.clone(),
        };

        // Phase 3: Rebuild types that need field resolution.
        // We create new Arc instances with resolved fields and replace
        // the stubs in the types map.

        // 3a: Rebuild interfaces with resolved fields
        interface_arcs.clear();
        for (name, ext_type) in &schema.types {
            let name_str = name.as_str().to_string();
            if let ExtendedType::Interface(iface) = ext_type {
                let mut fields = IndexMap::new();
                for (field_name, field_def) in &iface.fields {
                    let field = convert_field_definition(field_def, &full_registry);
                    fields.insert(field_name.as_str().to_string(), field);
                }
                let iface_arc = Arc::new(GraphQLInterfaceType {
                    name: GraphQLName::new(name_str.clone()),
                    documentation: iface.description.as_ref().map(|d| d.to_string()),
                    fields,
                    interfaces: vec![], // Populated below after all interfaces rebuilt
                    key_fields: None,
                    implementing_objects: vec![],
                });
                interface_arcs.insert(name_str.clone(), Arc::clone(&iface_arc));
                types.insert(name_str, GraphQLNamedType::Interface(iface_arc));
            }
        }

        // 3b: Rebuild objects with resolved fields and interface references
        object_arcs.clear();
        for (name, ext_type) in &schema.types {
            let name_str = name.as_str().to_string();
            if let ExtendedType::Object(obj) = ext_type {
                // Rebuild the full_registry snapshot to include rebuilt interfaces
                let updated_registry = TypeRegistry {
                    types: types.clone(),
                };
                let mut fields = IndexMap::new();
                for (field_name, field_def) in &obj.fields {
                    let field = convert_field_definition(field_def, &updated_registry);
                    fields.insert(field_name.as_str().to_string(), field);
                }
                let interfaces: Vec<Arc<GraphQLInterfaceType>> = obj
                    .implements_interfaces
                    .iter()
                    .filter_map(|iface_name| {
                        interface_arcs.get(iface_name.as_str()).cloned()
                    })
                    .collect();
                let obj_arc = Arc::new(GraphQLObjectType {
                    name: GraphQLName::new(name_str.clone()),
                    documentation: obj.description.as_ref().map(|d| d.to_string()),
                    fields,
                    interfaces,
                    key_fields: None,
                });
                object_arcs.insert(name_str.clone(), Arc::clone(&obj_arc));
                types.insert(name_str, GraphQLNamedType::Object(obj_arc));
            }
        }

        // 3c: Rebuild unions with resolved object member references
        for (name, ext_type) in &schema.types {
            let name_str = name.as_str().to_string();
            if let ExtendedType::Union(union) = ext_type {
                let member_types: Vec<Arc<GraphQLObjectType>> = union
                    .members
                    .iter()
                    .filter_map(|member_name| {
                        object_arcs.get(member_name.as_str()).cloned()
                    })
                    .collect();
                let named = GraphQLNamedType::Union(Arc::new(GraphQLUnionType {
                    name: GraphQLName::new(name_str.clone()),
                    documentation: union.description.as_ref().map(|d| d.to_string()),
                    types: member_types,
                }));
                types.insert(name_str, named);
            }
        }

        // 3d: Rebuild input objects with resolved field types
        for (name, ext_type) in &schema.types {
            let name_str = name.as_str().to_string();
            if let ExtendedType::InputObject(io) = ext_type {
                let current_registry = TypeRegistry {
                    types: types.clone(),
                };
                let mut fields = IndexMap::new();
                for (field_name, field_def) in &io.fields {
                    let field = convert_input_value_definition(field_def, &current_registry);
                    fields.insert(field_name.as_str().to_string(), field);
                }
                let named = GraphQLNamedType::InputObject(Arc::new(GraphQLInputObjectType {
                    name: GraphQLName::new(name_str.clone()),
                    documentation: io.description.as_ref().map(|d| d.to_string()),
                    is_one_of: io.directives.get("oneOf").is_some(),
                    fields,
                }));
                types.insert(name_str, named);
            }
        }

        // Phase 4: Final rebuild pass. At this point all types have their final
        // Arc instances in `types`. But field type references from phase 3 may
        // point to intermediate Arcs. Do one last rebuild of interfaces, objects,
        // and unions using the now-complete registry to ensure Arc identity.
        let final_registry = TypeRegistry {
            types: types.clone(),
        };

        // Phase 4a: Rebuild interfaces with resolved fields.
        // First pass: build all interfaces without their implements_interfaces
        // so we have Arcs for all interfaces.
        interface_arcs.clear();
        for (name, ext_type) in &schema.types {
            let name_str = name.as_str().to_string();
            if let ExtendedType::Interface(iface) = ext_type {
                let mut fields = IndexMap::new();
                for (field_name, field_def) in &iface.fields {
                    let field = convert_field_definition(field_def, &final_registry);
                    fields.insert(field_name.as_str().to_string(), field);
                }
                let iface_arc = Arc::new(GraphQLInterfaceType {
                    name: GraphQLName::new(name_str.clone()),
                    documentation: iface.description.as_ref().map(|d| d.to_string()),
                    fields,
                    interfaces: vec![], // Populated in second pass
                    key_fields: None,
                    implementing_objects: vec![],
                });
                interface_arcs.insert(name_str.clone(), Arc::clone(&iface_arc));
                types.insert(name_str, GraphQLNamedType::Interface(iface_arc));
            }
        }

        // Phase 4b: Rebuild interfaces again with resolved implements_interfaces.
        // Now all interface Arcs exist in interface_arcs, so we can resolve
        // interface-implementing-interface relationships.
        for (name, ext_type) in &schema.types {
            let name_str = name.as_str().to_string();
            if let ExtendedType::Interface(iface) = ext_type {
                if !iface.implements_interfaces.is_empty() {
                    let mut fields = IndexMap::new();
                    let updated_registry = TypeRegistry { types: types.clone() };
                    for (field_name, field_def) in &iface.fields {
                        let field = convert_field_definition(field_def, &updated_registry);
                        fields.insert(field_name.as_str().to_string(), field);
                    }
                    let interfaces: Vec<Arc<GraphQLInterfaceType>> = iface
                        .implements_interfaces
                        .iter()
                        .filter_map(|iface_name| {
                            interface_arcs.get(iface_name.as_str()).cloned()
                        })
                        .collect();
                    let iface_arc = Arc::new(GraphQLInterfaceType {
                        name: GraphQLName::new(name_str.clone()),
                        documentation: iface.description.as_ref().map(|d| d.to_string()),
                        fields,
                        interfaces,
                        key_fields: None,
                        implementing_objects: vec![],
                    });
                    interface_arcs.insert(name_str.clone(), Arc::clone(&iface_arc));
                    types.insert(name_str, GraphQLNamedType::Interface(iface_arc));
                }
            }
        }

        object_arcs.clear();
        // Use final registry that has all final Arcs for scalars/enums/unions
        // plus the just-rebuilt interfaces.
        let final_registry2 = TypeRegistry {
            types: types.clone(),
        };
        for (name, ext_type) in &schema.types {
            let name_str = name.as_str().to_string();
            if let ExtendedType::Object(obj) = ext_type {
                let mut fields = IndexMap::new();
                for (field_name, field_def) in &obj.fields {
                    let field = convert_field_definition(field_def, &final_registry2);
                    fields.insert(field_name.as_str().to_string(), field);
                }
                let interfaces: Vec<Arc<GraphQLInterfaceType>> = obj
                    .implements_interfaces
                    .iter()
                    .filter_map(|iface_name| {
                        interface_arcs.get(iface_name.as_str()).cloned()
                    })
                    .collect();
                let obj_arc = Arc::new(GraphQLObjectType {
                    name: GraphQLName::new(name_str.clone()),
                    documentation: obj.description.as_ref().map(|d| d.to_string()),
                    fields,
                    interfaces,
                    key_fields: None,
                });
                object_arcs.insert(name_str.clone(), Arc::clone(&obj_arc));
                types.insert(name_str, GraphQLNamedType::Object(obj_arc));
            }
        }

        // Rebuild unions with final object Arcs
        for (name, ext_type) in &schema.types {
            let name_str = name.as_str().to_string();
            if let ExtendedType::Union(union) = ext_type {
                let member_types: Vec<Arc<GraphQLObjectType>> = union
                    .members
                    .iter()
                    .filter_map(|member_name| {
                        object_arcs.get(member_name.as_str()).cloned()
                    })
                    .collect();
                let named = GraphQLNamedType::Union(Arc::new(GraphQLUnionType {
                    name: GraphQLName::new(name_str.clone()),
                    documentation: union.description.as_ref().map(|d| d.to_string()),
                    types: member_types,
                }));
                types.insert(name_str, named);
            }
        }

        // Phase 5: Populate implementing_objects on interfaces.
        // For each object type, add it to each interface it implements.
        // This mirrors Swift's CompilationResult which tracks implementing
        // objects on interface types.
        let mut iface_to_objects: IndexMap<String, Vec<Arc<GraphQLObjectType>>> = IndexMap::new();
        for obj_arc in object_arcs.values() {
            for iface in &obj_arc.interfaces {
                iface_to_objects
                    .entry(iface.name.schema_name.clone())
                    .or_default()
                    .push(Arc::clone(obj_arc));
            }
        }

        for (iface_name, impl_objects) in &iface_to_objects {
            if let Some(GraphQLNamedType::Interface(old_arc)) = types.get(iface_name) {
                let rebuilt = Arc::new(GraphQLInterfaceType {
                    name: old_arc.name.clone(),
                    documentation: old_arc.documentation.clone(),
                    fields: old_arc.fields.clone(),
                    interfaces: old_arc.interfaces.clone(),
                    key_fields: old_arc.key_fields.clone(),
                    implementing_objects: impl_objects.clone(),
                });
                interface_arcs.insert(iface_name.clone(), Arc::clone(&rebuilt));
                types.insert(iface_name.clone(), GraphQLNamedType::Interface(rebuilt));
            }
        }

        // Phase 6: Rebuild objects and unions one final time so their field type
        // references point to the updated interface Arcs (which now have
        // implementing_objects populated). Without this, entity fields like
        // `predators: [Animal!]!` would hold stale interface Arcs.
        let final_registry3 = TypeRegistry {
            types: types.clone(),
        };
        object_arcs.clear();
        for (name, ext_type) in &schema.types {
            let name_str = name.as_str().to_string();
            if let ExtendedType::Object(obj) = ext_type {
                let mut fields = IndexMap::new();
                for (field_name, field_def) in &obj.fields {
                    let field = convert_field_definition(field_def, &final_registry3);
                    fields.insert(field_name.as_str().to_string(), field);
                }
                let interfaces: Vec<Arc<GraphQLInterfaceType>> = obj
                    .implements_interfaces
                    .iter()
                    .filter_map(|iface_name| {
                        interface_arcs.get(iface_name.as_str()).cloned()
                    })
                    .collect();
                let obj_arc = Arc::new(GraphQLObjectType {
                    name: GraphQLName::new(name_str.clone()),
                    documentation: obj.description.as_ref().map(|d| d.to_string()),
                    fields,
                    interfaces,
                    key_fields: None,
                });
                object_arcs.insert(name_str.clone(), Arc::clone(&obj_arc));
                types.insert(name_str, GraphQLNamedType::Object(obj_arc));
            }
        }
        // Rebuild unions with the final object Arcs
        for (name, ext_type) in &schema.types {
            let name_str = name.as_str().to_string();
            if let ExtendedType::Union(union) = ext_type {
                let member_types: Vec<Arc<GraphQLObjectType>> = union
                    .members
                    .iter()
                    .filter_map(|member_name| {
                        object_arcs.get(member_name.as_str()).cloned()
                    })
                    .collect();
                let named = GraphQLNamedType::Union(Arc::new(GraphQLUnionType {
                    name: GraphQLName::new(name_str.clone()),
                    documentation: union.description.as_ref().map(|d| d.to_string()),
                    types: member_types,
                }));
                types.insert(name_str, named);
            }
        }

        TypeRegistry { types }
    }

    /// Look up a named type by name.
    pub fn get(&self, name: &str) -> Option<&GraphQLNamedType> {
        self.types.get(name)
    }

    /// Returns all types in the registry.
    pub fn all_types(&self) -> &IndexMap<String, GraphQLNamedType> {
        &self.types
    }

    /// Returns the number of types in the registry.
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}

// MARK: - Directive Helpers

/// Extracts the deprecation reason from a single `@deprecated` directive.
fn deprecation_reason_from_directive(deprecated: &ast::Directive) -> Option<String> {
    let reason_arg = deprecated
        .arguments
        .iter()
        .find(|arg| arg.name == "reason");
    match reason_arg {
        Some(arg) => match &*arg.value {
            ast::Value::String(s) => Some(s.clone()),
            _ => Some("No longer supported".to_string()),
        },
        None => Some("No longer supported".to_string()),
    }
}

/// Extracts the deprecation reason from an `ast::DirectiveList`.
/// Used for `FieldDefinition` and `InputValueDefinition` directives.
pub fn extract_deprecation_reason(directives: &ast::DirectiveList) -> Option<String> {
    let deprecated = directives.get("deprecated")?;
    deprecation_reason_from_directive(deprecated)
}

/// Extracts the deprecation reason from a `schema::DirectiveList`.
/// Used for schema-level type directives (ScalarType, EnumType, etc.).
pub fn extract_deprecation_reason_schema(
    directives: &schema::DirectiveList,
) -> Option<String> {
    let deprecated = directives.get("deprecated")?;
    deprecation_reason_from_directive(deprecated)
}

/// Extracts the `specifiedByURL` from a `schema::DirectiveList`.
pub fn extract_specified_by_url(directives: &schema::DirectiveList) -> Option<String> {
    let specified_by = directives.get("specifiedBy")?;
    let url_arg = specified_by
        .arguments
        .iter()
        .find(|arg| arg.name == "url")?;
    match &*url_arg.value {
        ast::Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

// MARK: - Type Conversion

/// Converts an apollo-compiler `ast::Type` (4 variants) to our `GraphQLType`
/// (6 variants). Per RESEARCH.md Pitfall 3.
pub fn convert_type(ac_type: &ast::Type, registry: &TypeRegistry) -> GraphQLType {
    match ac_type {
        ast::Type::NonNullNamed(name) => {
            GraphQLType::NonNull(Box::new(resolve_named_type(name.as_str(), registry)))
        }
        ast::Type::Named(name) => resolve_named_type(name.as_str(), registry),
        ast::Type::NonNullList(inner) => GraphQLType::NonNull(Box::new(GraphQLType::List(
            Box::new(convert_type(inner, registry)),
        ))),
        ast::Type::List(inner) => {
            GraphQLType::List(Box::new(convert_type(inner, registry)))
        }
    }
}

/// Resolves a named type string to a categorized `GraphQLType`.
pub fn resolve_named_type(name: &str, registry: &TypeRegistry) -> GraphQLType {
    let named = registry
        .get(name)
        .unwrap_or_else(|| panic!("Unknown type: {}", name));
    match named {
        GraphQLNamedType::Object(t) => {
            GraphQLType::Entity(GraphQLCompositeType::Object(Arc::clone(t)))
        }
        GraphQLNamedType::Interface(t) => {
            GraphQLType::Entity(GraphQLCompositeType::Interface(Arc::clone(t)))
        }
        GraphQLNamedType::Union(t) => {
            GraphQLType::Entity(GraphQLCompositeType::Union(Arc::clone(t)))
        }
        GraphQLNamedType::Scalar(t) => GraphQLType::Scalar(Arc::clone(t)),
        GraphQLNamedType::Enum(t) => GraphQLType::Enum(Arc::clone(t)),
        GraphQLNamedType::InputObject(t) => GraphQLType::InputObject(Arc::clone(t)),
    }
}

// MARK: - Value Conversion

/// Converts an apollo-compiler `ast::Value` to our `GraphQLValue`.
pub fn convert_value(value: &ast::Value) -> GraphQLValue {
    match value {
        ast::Value::Variable(name) => GraphQLValue::Variable(name.to_string()),
        ast::Value::Int(int_val) => {
            // apollo-compiler stores integers as string internally;
            // parse to i64 for our representation.
            let val: i64 = int_val
                .as_str()
                .parse()
                .unwrap_or_else(|e| panic!("Integer value parse error: {} ({})", int_val, e));
            GraphQLValue::Int(val)
        }
        ast::Value::Float(float_val) => {
            let val = float_val
                .try_to_f64()
                .unwrap_or_else(|e| panic!("Float value parse error: {} ({})", float_val, e));
            GraphQLValue::Float(val)
        }
        ast::Value::String(s) => GraphQLValue::String(s.clone()),
        ast::Value::Boolean(b) => GraphQLValue::Boolean(*b),
        ast::Value::Null => GraphQLValue::Null,
        ast::Value::Enum(name) => GraphQLValue::Enum(name.to_string()),
        ast::Value::List(items) => {
            GraphQLValue::List(items.iter().map(|v| convert_value(v)).collect())
        }
        ast::Value::Object(fields) => {
            let mut map = IndexMap::new();
            for (key, val) in fields {
                map.insert(key.as_str().to_string(), convert_value(val));
            }
            GraphQLValue::Object(map)
        }
    }
}

// MARK: - Field/Input Conversion

/// Converts an apollo-compiler field definition to our `GraphQLField`.
pub fn convert_field_definition(
    field: &schema::FieldDefinition,
    registry: &TypeRegistry,
) -> GraphQLField {
    let arguments: Vec<GraphQLFieldArgument> = field
        .arguments
        .iter()
        .map(|arg| GraphQLFieldArgument {
            name: arg.name.as_str().to_string(),
            type_: convert_type(&arg.ty, registry),
            documentation: arg.description.as_ref().map(|d| d.to_string()),
            deprecation_reason: extract_deprecation_reason(&arg.directives),
        })
        .collect();

    GraphQLField {
        name: field.name.as_str().to_string(),
        type_: convert_type(&field.ty, registry),
        arguments,
        documentation: field.description.as_ref().map(|d| d.to_string()),
        deprecation_reason: extract_deprecation_reason(&field.directives),
    }
}

/// Converts an apollo-compiler input value definition to our `GraphQLInputField`.
pub fn convert_input_value_definition(
    input: &ast::InputValueDefinition,
    registry: &TypeRegistry,
) -> GraphQLInputField {
    let default_value = input
        .default_value
        .as_ref()
        .map(|v| convert_value(v));

    GraphQLInputField {
        name: GraphQLName::new(input.name.as_str().to_string()),
        type_: convert_type(&input.ty, registry),
        documentation: input.description.as_ref().map(|d| d.to_string()),
        deprecation_reason: extract_deprecation_reason(&input.directives),
        default_value,
    }
}

// MARK: - Selection Set Conversion

/// Converts an apollo-compiler executable selection set to our `SelectionSet`.
///
/// GAP-01: Filters out explicit `__typename` field selections. graphql-js strips
/// user-written `__typename` and injects it implicitly; apollo-rs preserves them.
/// The IR builder must not see explicit `__typename` selections to match Swift behavior.
pub fn convert_selection_set(
    selections: &[executable::Selection],
    parent_type: &GraphQLCompositeType,
    registry: &TypeRegistry,
    fragment_defs: &IndexMap<String, Arc<compilation_result::FragmentDefinition>>,
) -> compilation_result::SelectionSet {
    let converted: Vec<compilation_result::Selection> = selections
        .iter()
        .filter_map(|sel| convert_selection(sel, parent_type, registry, fragment_defs))
        .collect();

    compilation_result::SelectionSet {
        parent_type: parent_type.clone(),
        selections: converted,
    }
}

/// Converts a single apollo-compiler executable selection to our `Selection`.
/// Returns `None` for selections that should be filtered out (e.g., __typename).
fn convert_selection(
    selection: &executable::Selection,
    parent_type: &GraphQLCompositeType,
    registry: &TypeRegistry,
    fragment_defs: &IndexMap<String, Arc<compilation_result::FragmentDefinition>>,
) -> Option<compilation_result::Selection> {
    match selection {
        executable::Selection::Field(field) => {
            let field_name = field.name.as_str();

            // GAP-01: filter __typename -- graphql-js strips explicit __typename
            if field_name == "__typename" {
                return None;
            }

            let alias = field.alias.as_ref().map(|a| a.as_str().to_string());

            let arguments = if field.arguments.is_empty() {
                None
            } else {
                Some(
                    field
                        .arguments
                        .iter()
                        .map(|arg| {
                            // Resolve actual argument type from the schema field definition.
                            // field.definition.arguments has the proper types from the schema.
                            let arg_type = field
                                .definition
                                .arguments
                                .iter()
                                .find(|def_arg| def_arg.name == arg.name)
                                .map(|def_arg| convert_type(&def_arg.ty, registry))
                                .unwrap_or_else(|| {
                                    // Fallback for validated schemas — should not happen
                                    GraphQLType::Scalar(Arc::new(GraphQLScalarType {
                                        name: GraphQLName::new("String".to_string()),
                                        documentation: None,
                                        specified_by_url: None,
                                    }))
                                });

                            let deprecation_reason = field
                                .definition
                                .arguments
                                .iter()
                                .find(|def_arg| def_arg.name == arg.name)
                                .and_then(|def_arg| extract_deprecation_reason(&def_arg.directives));

                            compilation_result::Argument {
                                name: arg.name.as_str().to_string(),
                                type_: arg_type,
                                value: convert_value(&arg.value),
                                deprecation_reason,
                            }
                        })
                        .collect(),
                )
            };

            let directives = convert_directives(&field.directives);
            let inclusion_conditions = extract_inclusion_conditions(&field.directives);

            // Use the apollo-compiler's resolved field type via field.ty()
            let field_type = convert_type(field.ty(), registry);

            let sub_selection_set = if field.selection_set.selections.is_empty() {
                None
            } else {
                let sub_parent = field_type
                    .as_composite_type()
                    .expect("Field with selections must have composite type");
                Some(convert_selection_set(
                    &field.selection_set.selections,
                    sub_parent,
                    registry,
                    fragment_defs,
                ))
            };

            let deprecation_reason = extract_deprecation_reason(&field.definition.directives);
            let documentation = field.definition.description.as_ref().map(|d| d.to_string());

            Some(compilation_result::Selection::Field(
                compilation_result::Field {
                    name: field_name.to_string(),
                    alias,
                    type_: field_type,
                    arguments,
                    inclusion_conditions,
                    directives,
                    selection_set: sub_selection_set,
                    deprecation_reason,
                    documentation,
                },
            ))
        }
        executable::Selection::InlineFragment(inline) => {
            let type_condition_name = inline
                .type_condition
                .as_ref()
                .map(|tc| tc.as_str().to_string());

            let inline_parent = if let Some(ref tc_name) = type_condition_name {
                resolve_composite_type(tc_name, registry)
                    .expect("Inline fragment type condition must resolve to composite type")
            } else {
                // No type condition -- inherits parent type from enclosing selection set
                parent_type.clone()
            };

            let directives = convert_directives(&inline.directives);
            let inclusion_conditions = extract_inclusion_conditions(&inline.directives);
            let defer_condition =
                compilation_result::get_defer_condition(&directives);

            let selection_set = convert_selection_set(
                &inline.selection_set.selections,
                &inline_parent,
                registry,
                fragment_defs,
            );

            Some(compilation_result::Selection::InlineFragment(
                compilation_result::InlineFragment {
                    selection_set,
                    inclusion_conditions,
                    directives,
                    defer_condition,
                },
            ))
        }
        executable::Selection::FragmentSpread(spread) => {
            let fragment_name = spread.fragment_name.as_str();

            let fragment = fragment_defs
                .get(fragment_name)
                .unwrap_or_else(|| panic!("Unknown fragment: {}", fragment_name));

            let directives = convert_directives(&spread.directives);
            let inclusion_conditions = extract_inclusion_conditions(&spread.directives);
            let defer_condition =
                compilation_result::get_defer_condition(&directives);

            Some(compilation_result::Selection::FragmentSpread(
                compilation_result::FragmentSpread {
                    fragment: Arc::clone(fragment),
                    inclusion_conditions,
                    directives,
                    defer_condition,
                },
            ))
        }
    }
}

/// Converts apollo-compiler directives to our Directive type.
pub fn convert_directives(
    directives: &executable::DirectiveList,
) -> Option<Vec<compilation_result::Directive>> {
    if directives.is_empty() {
        return None;
    }
    Some(
        directives
            .iter()
            .map(|d| compilation_result::Directive {
                name: d.name.as_str().to_string(),
                arguments: if d.arguments.is_empty() {
                    None
                } else {
                    Some(
                        d.arguments
                            .iter()
                            .map(|arg| {
                                // Resolve argument types from built-in directive definitions.
                                let arg_type = resolve_directive_arg_type(
                                    d.name.as_str(),
                                    arg.name.as_str(),
                                );
                                compilation_result::Argument {
                                    name: arg.name.as_str().to_string(),
                                    type_: arg_type,
                                    value: convert_value(&arg.value),
                                    deprecation_reason: None,
                                }
                            })
                            .collect(),
                    )
                },
            })
            .collect(),
    )
}

/// Resolves the argument type for built-in GraphQL directives.
/// Custom directive arguments fall back to String.
fn resolve_directive_arg_type(directive_name: &str, arg_name: &str) -> GraphQLType {
    let scalar = |name: &str| -> GraphQLType {
        GraphQLType::Scalar(Arc::new(GraphQLScalarType {
            name: GraphQLName::new(name.to_string()),
            documentation: None,
            specified_by_url: None,
        }))
    };
    match (directive_name, arg_name) {
        // GAP-04 re-verified: graphql-js produces NonNull(Boolean) for @include/@skip
        // (if: Boolean! is required) but nullable Boolean for @defer (if: Boolean is optional).
        ("include", "if") | ("skip", "if") => {
            GraphQLType::NonNull(Box::new(scalar("Boolean")))
        }
        ("defer", "if") => scalar("Boolean"),
        ("defer", "label") => scalar("String"),
        ("deprecated", "reason") => scalar("String"),
        ("specifiedBy", "url") => {
            GraphQLType::NonNull(Box::new(scalar("String")))
        }
        _ => scalar("String"),
    }
}

/// Extracts inclusion conditions from directives (@include/@skip).
///
/// Handles both variable-based conditions (`@include(if: $var)`) and
/// static boolean conditions (`@include(if: true)`, `@skip(if: false)`).
/// Static conditions produce `Included` or `Skipped` variants matching
/// Swift's graphql-js behavior.
fn extract_inclusion_conditions(
    directives: &executable::DirectiveList,
) -> Option<Vec<compilation_result::InclusionCondition>> {
    let mut conditions = Vec::new();

    for directive in directives.iter() {
        let name = directive.name.as_str();
        match name {
            "include" => {
                if let Some(arg) = directive.arguments.iter().find(|a| a.name == "if") {
                    match &*arg.value {
                        ast::Value::Variable(var_name) => {
                            conditions.push(compilation_result::InclusionCondition::include_if(
                                var_name.to_string(),
                            ));
                        }
                        ast::Value::Boolean(true) => {
                            // @include(if: true) -> always included
                            conditions.push(compilation_result::InclusionCondition::Included);
                        }
                        ast::Value::Boolean(false) => {
                            // @include(if: false) -> always skipped
                            conditions.push(compilation_result::InclusionCondition::Skipped);
                        }
                        _ => {}
                    }
                }
            }
            "skip" => {
                if let Some(arg) = directive.arguments.iter().find(|a| a.name == "if") {
                    match &*arg.value {
                        ast::Value::Variable(var_name) => {
                            conditions.push(compilation_result::InclusionCondition::skip_if(
                                var_name.to_string(),
                            ));
                        }
                        ast::Value::Boolean(true) => {
                            // @skip(if: true) -> always skipped
                            conditions.push(compilation_result::InclusionCondition::Skipped);
                        }
                        ast::Value::Boolean(false) => {
                            // @skip(if: false) -> always included
                            conditions.push(compilation_result::InclusionCondition::Included);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    if conditions.is_empty() {
        None
    } else {
        Some(conditions)
    }
}

/// Resolves a type name to a `GraphQLCompositeType`.
fn resolve_composite_type(
    name: &str,
    registry: &TypeRegistry,
) -> Option<GraphQLCompositeType> {
    match registry.get(name)? {
        GraphQLNamedType::Object(t) => Some(GraphQLCompositeType::Object(Arc::clone(t))),
        GraphQLNamedType::Interface(t) => Some(GraphQLCompositeType::Interface(Arc::clone(t))),
        GraphQLNamedType::Union(t) => Some(GraphQLCompositeType::Union(Arc::clone(t))),
        _ => None,
    }
}

/// Helper to get composite type from a GraphQLType.
impl GraphQLType {
    /// Returns the composite type if this is an entity type, unwrapping NonNull/List wrappers.
    pub fn as_composite_type(&self) -> Option<&GraphQLCompositeType> {
        match self {
            GraphQLType::Entity(ct) => Some(ct),
            GraphQLType::NonNull(inner) => inner.as_composite_type(),
            GraphQLType::List(inner) => inner.as_composite_type(),
            _ => None,
        }
    }
}

// MARK: - Referenced Fragments Collection

/// Collects only the **direct** (non-transitive) fragment references from a selection set.
///
/// GAP-02: apollo-rs expands referenced_fragments to include transitive dependencies
/// (if fragment A references fragment B, both A and B appear). graphql-js lists only
/// direct references (only A appears for the operation, B only appears in A's own
/// referenced_fragments). This function returns only fragment spreads found directly
/// in the given selection set, NOT recursively descending into fragments.
pub fn collect_referenced_fragments(
    selections: &[executable::Selection],
    fragment_defs: &IndexMap<String, Arc<compilation_result::FragmentDefinition>>,
) -> Vec<Arc<compilation_result::FragmentDefinition>> {
    // GAP-02: Only collect direct fragment spreads -- do NOT recurse into fragments
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    collect_direct_fragments(selections, fragment_defs, &mut seen, &mut result);

    result
}

/// Recursively collects fragment spreads from selections and inline fragments,
/// but does NOT descend into the fragment definitions themselves.
/// This matches graphql-js behavior: referenced_fragments contains only fragments
/// directly spread in the operation/fragment's selection set tree.
fn collect_direct_fragments(
    selections: &[executable::Selection],
    fragment_defs: &IndexMap<String, Arc<compilation_result::FragmentDefinition>>,
    seen: &mut HashSet<String>,
    result: &mut Vec<Arc<compilation_result::FragmentDefinition>>,
) {
    for selection in selections {
        match selection {
            executable::Selection::FragmentSpread(spread) => {
                let name = spread.fragment_name.as_str().to_string();
                if seen.insert(name.clone()) {
                    if let Some(frag) = fragment_defs.get(&name) {
                        result.push(Arc::clone(frag));
                    }
                }
            }
            executable::Selection::InlineFragment(inline) => {
                // Recurse into inline fragment selections (they are part of the
                // same operation/fragment's selection set, not a separate definition)
                collect_direct_fragments(
                    &inline.selection_set.selections,
                    fragment_defs,
                    seen,
                    result,
                );
            }
            executable::Selection::Field(field) => {
                // Recurse into field sub-selections (still part of the same
                // operation/fragment's selection set tree)
                if !field.selection_set.selections.is_empty() {
                    collect_direct_fragments(
                        &field.selection_set.selections,
                        fragment_defs,
                        seen,
                        result,
                    );
                }
            }
        }
    }
}

// MARK: - Referenced Types Filtering

/// Filters a type registry to include only types actually referenced by the
/// compiled operations and fragments, excluding introspection meta-types.
///
/// GAP-03: apollo-rs includes ALL schema types plus introspection meta-types
/// (__Schema, __Type, __Field, __EnumValue, __Directive, __InputValue) in its
/// type list. graphql-js includes only types actually referenced by compiled
/// operations. This function filters to match the graphql-js behavior.
///
/// The collection is transitive: when an interface is referenced, all types
/// implementing that interface are also included, and their fields are
/// traversed recursively to collect further types. This matches graphql-js's
/// behavior of including all types reachable from the operations.
pub fn collect_referenced_types(
    registry: &TypeRegistry,
    operations: &[compilation_result::OperationDefinition],
    fragments: &[compilation_result::FragmentDefinition],
) -> Vec<GraphQLNamedType> {
    let mut referenced_names: HashSet<String> = HashSet::new();

    // Collect types from operations
    for op in operations {
        collect_types_from_composite(&op.root_type, &mut referenced_names);
        collect_types_from_selection_set(&op.selection_set, &mut referenced_names);
        for var in &op.variables {
            collect_types_from_graphql_type(&var.type_, &mut referenced_names);
        }
    }

    // Collect types from fragments
    for frag in fragments {
        collect_types_from_composite(&frag.type_, &mut referenced_names);
        collect_types_from_selection_set(&frag.selection_set, &mut referenced_names);
    }

    // Transitive closure: for every referenced interface, include all
    // implementing object types. For unions, include all member types.
    // This matches graphql-js which includes concrete types that can appear
    // at abstract type positions, but does NOT traverse their fields for
    // additional scalar/enum types beyond what the operations reference.
    let mut worklist: Vec<String> = referenced_names.iter().cloned().collect();
    while let Some(type_name) = worklist.pop() {
        match registry.get(&type_name) {
            Some(GraphQLNamedType::Interface(_)) => {
                // Find all objects implementing this interface
                for (name, named_type) in registry.all_types() {
                    if let GraphQLNamedType::Object(obj) = named_type {
                        if obj.interfaces.iter().any(|i| i.name.schema_name == type_name) {
                            if referenced_names.insert(name.clone()) {
                                worklist.push(name.clone());
                            }
                            // Also include all interfaces this object implements
                            for iface in &obj.interfaces {
                                if referenced_names.insert(iface.name.schema_name.clone()) {
                                    worklist.push(iface.name.schema_name.clone());
                                }
                            }
                        }
                    }
                }
            }
            Some(GraphQLNamedType::Union(u)) => {
                // Include all union member types
                for member in &u.types {
                    if referenced_names.insert(member.name.schema_name.clone()) {
                        worklist.push(member.name.schema_name.clone());
                    }
                }
            }
            Some(GraphQLNamedType::InputObject(io)) => {
                // Transitive closure through input object fields:
                // if an input object is referenced, all types used by its
                // fields are also referenced (enums, scalars, other inputs).
                for field in io.fields.values() {
                    collect_types_from_graphql_type_with_worklist(
                        &field.type_,
                        &mut referenced_names,
                        &mut worklist,
                    );
                }
            }
            _ => {}
        }
    }

    // Filter registry to only referenced types, excluding introspection meta-types
    registry
        .all_types()
        .iter()
        .filter(|(name, _)| {
            // Exclude introspection types (names starting with "__")
            !name.starts_with("__")
                && referenced_names.contains(name.as_str())
        })
        .map(|(_, named_type)| named_type.clone())
        .collect()
}

/// Collects type names referenced by a composite type.
fn collect_types_from_composite(
    composite: &GraphQLCompositeType,
    names: &mut HashSet<String>,
) {
    match composite {
        GraphQLCompositeType::Object(obj) => {
            names.insert(obj.name.schema_name.clone());
            for iface in &obj.interfaces {
                names.insert(iface.name.schema_name.clone());
            }
        }
        GraphQLCompositeType::Interface(iface) => {
            names.insert(iface.name.schema_name.clone());
        }
        GraphQLCompositeType::Union(union) => {
            names.insert(union.name.schema_name.clone());
            for member in &union.types {
                names.insert(member.name.schema_name.clone());
            }
        }
    }
}

/// Collects type names referenced by a selection set (recursively).
fn collect_types_from_selection_set(
    selection_set: &compilation_result::SelectionSet,
    names: &mut HashSet<String>,
) {
    collect_types_from_composite(&selection_set.parent_type, names);

    for selection in &selection_set.selections {
        match selection {
            compilation_result::Selection::Field(field) => {
                collect_types_from_graphql_type(&field.type_, names);
                if let Some(ref sub_set) = field.selection_set {
                    collect_types_from_selection_set(sub_set, names);
                }
                // Note: Field argument types are NOT collected here.
                // Swift's graphql-js only includes types referenced by
                // variable definitions and field return types, not by
                // field argument types directly.
            }
            compilation_result::Selection::InlineFragment(inline) => {
                collect_types_from_selection_set(&inline.selection_set, names);
            }
            compilation_result::Selection::FragmentSpread(spread) => {
                collect_types_from_composite(&spread.fragment.type_, names);
            }
        }
    }
}

/// Variant of collect_types_from_graphql_type that also pushes newly-discovered
/// type names onto a worklist for transitive closure processing.
fn collect_types_from_graphql_type_with_worklist(
    graphql_type: &GraphQLType,
    names: &mut HashSet<String>,
    worklist: &mut Vec<String>,
) {
    match graphql_type {
        GraphQLType::Scalar(s) => {
            names.insert(s.name.schema_name.clone());
        }
        GraphQLType::Enum(e) => {
            names.insert(e.name.schema_name.clone());
        }
        GraphQLType::InputObject(io) => {
            if names.insert(io.name.schema_name.clone()) {
                for field in io.fields.values() {
                    collect_types_from_graphql_type_with_worklist(&field.type_, names, worklist);
                }
            }
        }
        GraphQLType::Entity(ct) => {
            let type_name = match ct {
                GraphQLCompositeType::Object(o) => o.name.schema_name.clone(),
                GraphQLCompositeType::Interface(i) => i.name.schema_name.clone(),
                GraphQLCompositeType::Union(u) => u.name.schema_name.clone(),
            };
            if names.insert(type_name.clone()) {
                worklist.push(type_name);
            }
        }
        GraphQLType::NonNull(inner) => {
            collect_types_from_graphql_type_with_worklist(inner, names, worklist);
        }
        GraphQLType::List(inner) => {
            collect_types_from_graphql_type_with_worklist(inner, names, worklist);
        }
    }
}

/// Collects type names from a GraphQLType (unwrapping NonNull/List wrappers).
/// For InputObject types, also recurses into their fields to collect
/// transitively referenced types (matching graphql-js behavior).
fn collect_types_from_graphql_type(
    graphql_type: &GraphQLType,
    names: &mut HashSet<String>,
) {
    match graphql_type {
        GraphQLType::Scalar(s) => {
            names.insert(s.name.schema_name.clone());
        }
        GraphQLType::Enum(e) => {
            names.insert(e.name.schema_name.clone());
        }
        GraphQLType::InputObject(io) => {
            // Only recurse if we haven't already visited this input object
            // (prevents infinite recursion for self-referential input types)
            if names.insert(io.name.schema_name.clone()) {
                for field in io.fields.values() {
                    collect_types_from_graphql_type(&field.type_, names);
                }
            }
        }
        GraphQLType::Entity(ct) => {
            collect_types_from_composite(ct, names);
        }
        GraphQLType::NonNull(inner) => {
            collect_types_from_graphql_type(inner, names);
        }
        GraphQLType::List(inner) => {
            collect_types_from_graphql_type(inner, names);
        }
    }
}

// MARK: - Source Text Reconstruction (GAP-05)

/// Parent node kind for determining __typename injection behavior.
/// Mirrors graphql-js's parent kind check in SelectionSet visitor.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ParentKind {
    OperationDefinition,
    FragmentDefinition,
    Field,
    InlineFragment,
}

/// Transforms a per-definition GraphQL source text to match graphql-js's
/// `transformToNetworkRequestSourceDefinition` + `print()` output.
///
/// This adds `__typename` to selection sets whose parent is a Field or
/// FragmentDefinition, strips Apollo-specific directives, and strips
/// alias/directives from existing `__typename` fields.
///
/// If parsing fails, returns the input unchanged.
pub fn build_network_request_source(definition_text: &str) -> String {
    let doc = match ast::Document::parse(definition_text, "source_transform") {
        Ok(doc) => doc,
        Err(_) => return definition_text.to_string(),
    };

    let mut new_definitions = Vec::new();

    for def in &doc.definitions {
        match def {
            ast::Definition::OperationDefinition(op) => {
                let mut new_op = op.as_ref().clone();
                // Strip Apollo-specific directives from the operation itself
                new_op.directives = strip_apollo_directives_from_list(&op.directives);
                // Transform selection set -- operation root does NOT get __typename
                new_op.selection_set = transform_selections(
                    &op.selection_set,
                    ParentKind::OperationDefinition,
                );
                new_definitions.push(ast::Definition::OperationDefinition(
                    apollo_compiler::Node::new(new_op),
                ));
            }
            ast::Definition::FragmentDefinition(frag) => {
                let mut new_frag = frag.as_ref().clone();
                // Strip Apollo-specific directives from the fragment itself
                new_frag.directives = strip_apollo_directives_from_list(&frag.directives);
                // Transform selection set -- fragment definition DOES get __typename
                new_frag.selection_set = transform_selections(
                    &frag.selection_set,
                    ParentKind::FragmentDefinition,
                );
                new_definitions.push(ast::Definition::FragmentDefinition(
                    apollo_compiler::Node::new(new_frag),
                ));
            }
            other => {
                // Pass through any other definition unchanged
                new_definitions.push(other.clone());
            }
        }
    }

    let new_doc = ast::Document {
        sources: doc.sources.clone(),
        definitions: new_definitions,
    };

    // Use our graphql-js-compatible printer instead of apollo-compiler's Display
    print_graphql_js(&new_doc)
}

// MARK: - graphql-js compatible printer

const MAX_LINE_LENGTH: usize = 80;

/// Prints an apollo-compiler AST Document using graphql-js-compatible formatting.
///
/// Key differences from apollo-compiler's Display:
/// - Field/directive arguments wrap to multi-line (no commas) when > 80 chars
/// - Variable definitions always stay inline with commas
/// - Object values use `{ key: val }` spacing
fn print_graphql_js(doc: &ast::Document) -> String {
    let parts: Vec<String> = doc
        .definitions
        .iter()
        .map(print_definition)
        .collect();
    parts.join("\n\n")
}

fn print_definition(def: &ast::Definition) -> String {
    match def {
        ast::Definition::OperationDefinition(op) => print_operation(op),
        ast::Definition::FragmentDefinition(frag) => print_fragment(frag),
        _ => def.to_string().trim_end().to_string(),
    }
}

fn print_operation(op: &ast::OperationDefinition) -> String {
    let op_type = match op.operation_type {
        ast::OperationType::Query => "query",
        ast::OperationType::Mutation => "mutation",
        ast::OperationType::Subscription => "subscription",
    };

    let mut result = String::new();
    result.push_str(op_type);

    if let Some(ref name) = op.name {
        result.push(' ');
        result.push_str(name);
    }

    if !op.variables.is_empty() {
        // Variable definitions: always inline with commas, never wrap
        result.push('(');
        let vars: Vec<String> = op.variables.iter().map(|v| print_variable_def(v.as_ref())).collect();
        result.push_str(&vars.join(", "));
        result.push(')');
    }

    result.push_str(&print_directives(&op.directives, ""));

    if !op.selection_set.is_empty() {
        result.push_str(" {\n");
        result.push_str(&print_selection_set(&op.selection_set, "  "));
        result.push('}');
    }

    result
}

fn print_fragment(frag: &ast::FragmentDefinition) -> String {
    let mut result = format!("fragment {} on {}", frag.name, frag.type_condition);
    result.push_str(&print_directives(&frag.directives, ""));

    if !frag.selection_set.is_empty() {
        result.push_str(" {\n");
        result.push_str(&print_selection_set(&frag.selection_set, "  "));
        result.push('}');
    }

    result
}

fn print_variable_def(var: &ast::VariableDefinition) -> String {
    let mut result = format!("${}: {}", var.name, print_type(&var.ty));
    if let Some(ref default) = var.default_value {
        result.push_str(" = ");
        result.push_str(&print_value(default));
    }
    for dir in var.directives.iter() {
        result.push(' ');
        result.push_str(&print_directive_inline(dir));
    }
    result
}

fn print_type(ty: &ast::Type) -> String {
    match ty {
        ast::Type::Named(name) => name.to_string(),
        ast::Type::NonNullNamed(name) => format!("{}!", name),
        ast::Type::List(inner) => format!("[{}]", print_type(inner)),
        ast::Type::NonNullList(inner) => format!("[{}]!", print_type(inner)),
    }
}

fn print_selection_set(selections: &[ast::Selection], indent: &str) -> String {
    let mut result = String::new();
    for sel in selections {
        result.push_str(indent);
        match sel {
            ast::Selection::Field(field) => result.push_str(&print_field(field, indent)),
            ast::Selection::FragmentSpread(spread) => {
                result.push_str(&format!("...{}", spread.fragment_name));
                result.push_str(&print_directives(&spread.directives, indent));
            }
            ast::Selection::InlineFragment(frag) => {
                result.push_str("...");
                if let Some(ref tc) = frag.type_condition {
                    result.push_str(&format!(" on {}", tc));
                }
                result.push_str(&print_directives(&frag.directives, indent));
                if !frag.selection_set.is_empty() {
                    result.push_str(" {\n");
                    let child_indent = format!("{}  ", indent);
                    result.push_str(&print_selection_set(&frag.selection_set, &child_indent));
                    result.push_str(indent);
                    result.push('}');
                }
            }
        }
        result.push('\n');
    }
    result
}

fn print_field(field: &ast::Field, indent: &str) -> String {
    let mut result = String::new();

    // Alias
    let prefix = if let Some(ref alias) = field.alias {
        format!("{}: {}", alias, field.name)
    } else {
        field.name.to_string()
    };

    // Arguments: try inline first, wrap if > 80 chars
    let args_str = print_arguments(&field.arguments, &prefix, indent);
    result.push_str(&prefix);
    result.push_str(&args_str);

    // Directives
    result.push_str(&print_directives(&field.directives, indent));

    // Selection set
    if !field.selection_set.is_empty() {
        result.push_str(" {\n");
        let child_indent = format!("{}  ", indent);
        result.push_str(&print_selection_set(&field.selection_set, &child_indent));
        result.push_str(indent);
        result.push('}');
    }

    result
}

/// Prints arguments with graphql-js wrapping: inline with commas if <=80 chars,
/// otherwise multi-line with no commas.
fn print_arguments(args: &[apollo_compiler::Node<ast::Argument>], prefix: &str, indent: &str) -> String {
    if args.is_empty() {
        return String::new();
    }

    let printed_args: Vec<String> = args.iter().map(|a| print_argument(a)).collect();

    // Try inline: `prefix(arg1: val, arg2: val)`
    let inline = format!("({})", printed_args.join(", "));
    if format!("{}{}", prefix, inline).len() <= MAX_LINE_LENGTH {
        return inline;
    }

    // Wrap: each arg on its own line, no commas
    let mut result = String::from("(\n");
    for arg in &printed_args {
        result.push_str(indent);
        result.push_str("  ");
        result.push_str(arg);
        result.push('\n');
    }
    result.push_str(indent);
    result.push(')');
    result
}

fn print_argument(arg: &ast::Argument) -> String {
    format!("{}: {}", arg.name, print_value(&arg.value))
}

fn print_directives(directives: &ast::DirectiveList, indent: &str) -> String {
    let mut result = String::new();
    for dir in directives.iter() {
        result.push(' ');
        let dir_name = format!("@{}", dir.name);
        let args_str = print_arguments(&dir.arguments, &dir_name, indent);
        result.push_str(&dir_name);
        result.push_str(&args_str);
    }
    result
}

fn print_directive_inline(dir: &ast::Directive) -> String {
    let dir_name = format!("@{}", dir.name);
    if dir.arguments.is_empty() {
        return dir_name;
    }
    let printed_args: Vec<String> = dir.arguments.iter().map(|a| print_argument(a)).collect();
    format!("{}({})", dir_name, printed_args.join(", "))
}

fn print_value(value: &ast::Value) -> String {
    match value {
        ast::Value::Null => "null".to_string(),
        ast::Value::Boolean(b) => if *b { "true" } else { "false" }.to_string(),
        ast::Value::Int(i) => i.to_string(),
        ast::Value::Float(f) => f.to_string(),
        ast::Value::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        ast::Value::Enum(e) => e.to_string(),
        ast::Value::Variable(v) => format!("${}", v),
        ast::Value::List(items) => {
            let parts: Vec<String> = items.iter().map(|v| print_value(v)).collect();
            format!("[{}]", parts.join(", "))
        }
        ast::Value::Object(fields) => {
            if fields.is_empty() {
                return "{}".to_string();
            }
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, val)| format!("{}: {}", name, print_value(val)))
                .collect();
            let inline = format!("{{ {} }}", parts.join(", "));
            if inline.len() <= MAX_LINE_LENGTH {
                inline
            } else {
                format!("{{ {} }}", parts.join(" "))
            }
        }
    }
}

/// Recursively transforms a selection set, injecting __typename where needed.
///
/// Rules (matching graphql-js `transformToNetworkRequestSourceDefinition`):
/// - If parent is Field or FragmentDefinition: inject __typename as first selection if not present
/// - If parent is OperationDefinition or InlineFragment: do NOT inject __typename
/// - For existing __typename fields: strip alias and directives
/// - Strip Apollo-specific directives from all nodes
fn transform_selections(
    selections: &[ast::Selection],
    parent_kind: ParentKind,
) -> Vec<ast::Selection> {
    let mut result = Vec::new();

    // Determine if we should inject __typename
    let should_inject = matches!(parent_kind, ParentKind::Field | ParentKind::FragmentDefinition);

    // Check if __typename already exists
    let has_typename = selections.iter().any(|s| {
        matches!(s, ast::Selection::Field(f) if f.name == "__typename")
    });

    // Inject __typename as first selection if needed
    if should_inject && !has_typename {
        result.push(make_typename_selection());
    }

    // Transform each selection
    for sel in selections {
        match sel {
            ast::Selection::Field(field) => {
                let mut new_field = field.as_ref().clone();

                // Strip alias and directives from __typename fields
                if field.name == "__typename" {
                    new_field.alias = None;
                    new_field.directives = ast::DirectiveList(vec![]);
                } else {
                    // Strip Apollo-specific directives from non-typename fields
                    new_field.directives =
                        strip_apollo_directives_from_list(&field.directives);
                }

                // Recursively transform sub-selection sets (parent is Field)
                if !field.selection_set.is_empty() {
                    new_field.selection_set = transform_selections(
                        &field.selection_set,
                        ParentKind::Field,
                    );
                }

                result.push(ast::Selection::Field(
                    apollo_compiler::Node::new(new_field),
                ));
            }
            ast::Selection::InlineFragment(inline) => {
                let mut new_inline = inline.as_ref().clone();
                new_inline.directives =
                    strip_apollo_directives_from_list(&inline.directives);
                // InlineFragment children: parent is InlineFragment -> do NOT inject __typename
                new_inline.selection_set = transform_selections(
                    &inline.selection_set,
                    ParentKind::InlineFragment,
                );
                result.push(ast::Selection::InlineFragment(
                    apollo_compiler::Node::new(new_inline),
                ));
            }
            ast::Selection::FragmentSpread(spread) => {
                let mut new_spread = spread.as_ref().clone();
                new_spread.directives =
                    strip_apollo_directives_from_list(&spread.directives);
                result.push(ast::Selection::FragmentSpread(
                    apollo_compiler::Node::new(new_spread),
                ));
            }
        }
    }

    result
}

/// Creates a simple `__typename` field AST node.
fn make_typename_selection() -> ast::Selection {
    ast::Selection::Field(apollo_compiler::Node::new(ast::Field {
        alias: None,
        name: apollo_compiler::Name::new_unchecked("__typename"),
        arguments: vec![],
        directives: ast::DirectiveList(vec![]),
        selection_set: vec![],
    }))
}

/// Strips Apollo-specific directives from a DirectiveList.
/// Removes: @apollo_client_ios_localCacheMutation, @import
fn strip_apollo_directives_from_list(directives: &ast::DirectiveList) -> ast::DirectiveList {
    let filtered: Vec<_> = directives
        .0
        .iter()
        .filter(|d| {
            let name = d.name.as_str();
            name != "apollo_client_ios_localCacheMutation" && name != "import"
        })
        .cloned()
        .collect();
    ast::DirectiveList(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_schema(sdl: &str) -> Valid<schema::Schema> {
        schema::Schema::parse_and_validate(sdl, "test.graphql")
            .expect("Schema should parse and validate")
    }

    #[test]
    fn test_type_registry_builds_from_schema() {
        let schema = parse_schema(
            r#"
            type Query {
                hello: String
            }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);
        // Should have at least the built-in scalars + Query
        assert!(!registry.is_empty());
        assert!(registry.get("Query").is_some());
        assert!(registry.get("String").is_some());
    }

    #[test]
    fn test_type_registry_deduplicates_via_arc() {
        let schema = parse_schema(
            r#"
            type Query {
                user: User
                admin: User
            }
            type User {
                name: String
            }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);

        // Verify the registry has a single User entry
        let user_type = registry.get("User").unwrap();
        assert!(matches!(user_type, GraphQLNamedType::Object(_)));

        // Verify that two fields referencing the same type get the same Arc
        // (within the same object's field resolution pass)
        let query = registry.get("Query").unwrap();
        if let GraphQLNamedType::Object(query_obj) = query {
            let user_field = &query_obj.fields["user"];
            let admin_field = &query_obj.fields["admin"];

            if let (
                GraphQLType::Entity(GraphQLCompositeType::Object(arc2)),
                GraphQLType::Entity(GraphQLCompositeType::Object(arc3)),
            ) = (&user_field.type_, &admin_field.type_)
            {
                // Both fields reference the same Arc instance (same resolution pass)
                assert!(Arc::ptr_eq(arc2, arc3));
                // And the name matches the registry entry
                assert_eq!(arc2.name.schema_name, "User");
            } else {
                panic!("Expected Entity(Object) types for user and admin fields");
            }
        } else {
            panic!("Expected Object type for Query");
        }
    }

    #[test]
    fn test_extract_deprecation_reason_with_reason() {
        let schema = parse_schema(
            r#"
            type Query {
                old: String @deprecated(reason: "Use new field")
            }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);
        let query = registry.get("Query").unwrap();
        if let GraphQLNamedType::Object(obj) = query {
            let field = &obj.fields["old"];
            assert_eq!(
                field.deprecation_reason.as_deref(),
                Some("Use new field")
            );
        }
    }

    #[test]
    fn test_extract_deprecation_reason_default_message() {
        let schema = parse_schema(
            r#"
            type Query {
                old: String @deprecated
            }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);
        let query = registry.get("Query").unwrap();
        if let GraphQLNamedType::Object(obj) = query {
            let field = &obj.fields["old"];
            assert_eq!(
                field.deprecation_reason.as_deref(),
                Some("No longer supported")
            );
        }
    }

    #[test]
    fn test_extract_specified_by_url() {
        let schema = parse_schema(
            r#"
            scalar DateTime @specifiedBy(url: "https://example.com/datetime")
            type Query {
                now: DateTime
            }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);
        let dt = registry.get("DateTime").unwrap();
        if let GraphQLNamedType::Scalar(scalar) = dt {
            assert_eq!(
                scalar.specified_by_url.as_deref(),
                Some("https://example.com/datetime")
            );
            assert!(scalar.is_custom_scalar());
        } else {
            panic!("Expected Scalar type for DateTime");
        }
    }

    fn name(s: &str) -> apollo_compiler::Name {
        apollo_compiler::Name::new(s).unwrap()
    }

    #[test]
    fn test_convert_type_named() {
        let schema = parse_schema(
            r#"
            type Query { hello: String }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);

        let ac_type = ast::Type::Named(name("String"));
        let result = convert_type(&ac_type, &registry);
        assert!(matches!(result, GraphQLType::Scalar(_)));
        assert!(result.is_nullable());
    }

    #[test]
    fn test_convert_type_non_null_named() {
        let schema = parse_schema(
            r#"
            type Query { hello: String! }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);

        let ac_type = ast::Type::NonNullNamed(name("String"));
        let result = convert_type(&ac_type, &registry);
        assert!(matches!(result, GraphQLType::NonNull(_)));
        assert!(!result.is_nullable());
    }

    #[test]
    fn test_convert_type_list() {
        let schema = parse_schema(
            r#"
            type Query { items: [String] }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);

        let ac_type = ast::Type::List(Box::new(ast::Type::Named(name("String"))));
        let result = convert_type(&ac_type, &registry);
        assert!(matches!(result, GraphQLType::List(_)));
        assert_eq!(result.type_reference(), "[String]");
    }

    #[test]
    fn test_convert_type_non_null_list() {
        let schema = parse_schema(
            r#"
            type Query { items: [String!]! }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);

        let ac_type = ast::Type::NonNullList(Box::new(ast::Type::NonNullNamed(name("String"))));
        let result = convert_type(&ac_type, &registry);
        assert_eq!(result.type_reference(), "[String!]!");
    }

    #[test]
    fn test_convert_type_entity_object() {
        let schema = parse_schema(
            r#"
            type Query { user: User }
            type User { name: String }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);

        let ac_type = ast::Type::Named(name("User"));
        let result = convert_type(&ac_type, &registry);
        assert!(matches!(
            result,
            GraphQLType::Entity(GraphQLCompositeType::Object(_))
        ));
    }

    #[test]
    fn test_convert_type_entity_interface() {
        let schema = parse_schema(
            r#"
            type Query { node: Node }
            interface Node { id: ID! }
            type User implements Node { id: ID!, name: String }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);

        let ac_type = ast::Type::Named(name("Node"));
        let result = convert_type(&ac_type, &registry);
        assert!(matches!(
            result,
            GraphQLType::Entity(GraphQLCompositeType::Interface(_))
        ));
    }

    #[test]
    fn test_convert_type_entity_union() {
        let schema = parse_schema(
            r#"
            type Query { search: SearchResult }
            union SearchResult = User | Post
            type User { name: String }
            type Post { title: String }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);

        let ac_type = ast::Type::Named(name("SearchResult"));
        let result = convert_type(&ac_type, &registry);
        assert!(matches!(
            result,
            GraphQLType::Entity(GraphQLCompositeType::Union(_))
        ));
    }

    #[test]
    fn test_convert_type_enum() {
        let schema = parse_schema(
            r#"
            type Query { status: Status }
            enum Status { ACTIVE INACTIVE }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);

        let ac_type = ast::Type::Named(name("Status"));
        let result = convert_type(&ac_type, &registry);
        assert!(matches!(result, GraphQLType::Enum(_)));
    }

    #[test]
    fn test_convert_type_input_object() {
        let schema = parse_schema(
            r#"
            type Query { search(input: SearchInput): String }
            input SearchInput { query: String }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);

        let ac_type = ast::Type::Named(name("SearchInput"));
        let result = convert_type(&ac_type, &registry);
        assert!(matches!(result, GraphQLType::InputObject(_)));
    }

    #[test]
    fn test_convert_value_all_variants() {
        // Variable
        let val = ast::Value::Variable(name("myVar"));
        assert_eq!(convert_value(&val), GraphQLValue::Variable("myVar".to_string()));

        // Int
        let val = ast::Value::Int(42.into());
        assert_eq!(convert_value(&val), GraphQLValue::Int(42));

        // Float
        let val = ast::Value::Float(3.14_f64.into());
        assert_eq!(convert_value(&val), GraphQLValue::Float(3.14));

        // String
        let val = ast::Value::String("hello".to_string());
        assert_eq!(convert_value(&val), GraphQLValue::String("hello".to_string()));

        // Boolean
        let val = ast::Value::Boolean(true);
        assert_eq!(convert_value(&val), GraphQLValue::Boolean(true));

        // Null
        let val = ast::Value::Null;
        assert_eq!(convert_value(&val), GraphQLValue::Null);

        // Enum
        let val = ast::Value::Enum(name("ACTIVE"));
        assert_eq!(convert_value(&val), GraphQLValue::Enum("ACTIVE".to_string()));
    }

    #[test]
    fn test_registry_with_complex_schema() {
        let schema = parse_schema(
            r#"
            type Query {
                user(id: ID!): User
                search(input: SearchInput!): SearchResult
            }

            type Mutation {
                createUser(name: String!): User
            }

            interface Node {
                id: ID!
            }

            type User implements Node {
                id: ID!
                name: String!
                email: String
                status: UserStatus!
                friends: [User!]!
            }

            type Post implements Node {
                id: ID!
                title: String!
                author: User!
            }

            union SearchResult = User | Post

            enum UserStatus {
                ACTIVE
                INACTIVE
                BANNED @deprecated(reason: "Use SUSPENDED instead")
            }

            input SearchInput {
                query: String!
                limit: Int = 10
            }

            scalar DateTime @specifiedBy(url: "https://example.com/datetime")
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);

        // Verify all types are present
        assert!(registry.get("Query").is_some());
        assert!(registry.get("Mutation").is_some());
        assert!(registry.get("Node").is_some());
        assert!(registry.get("User").is_some());
        assert!(registry.get("Post").is_some());
        assert!(registry.get("SearchResult").is_some());
        assert!(registry.get("UserStatus").is_some());
        assert!(registry.get("SearchInput").is_some());
        assert!(registry.get("DateTime").is_some());

        // Verify enum deprecation
        if let Some(GraphQLNamedType::Enum(status)) = registry.get("UserStatus") {
            let banned = status.values.iter().find(|v| v.name.schema_name == "BANNED").unwrap();
            assert_eq!(banned.deprecation_reason.as_deref(), Some("Use SUSPENDED instead"));
            assert!(banned.is_deprecated());
        }

        // Verify union members
        if let Some(GraphQLNamedType::Union(search)) = registry.get("SearchResult") {
            assert_eq!(search.types.len(), 2);
        }

        // Verify object implements interface
        if let Some(GraphQLNamedType::Object(user)) = registry.get("User") {
            assert_eq!(user.interfaces.len(), 1);
            assert_eq!(user.interfaces[0].name.schema_name, "Node");
        }

        // Verify custom scalar
        if let Some(GraphQLNamedType::Scalar(dt)) = registry.get("DateTime") {
            assert!(dt.is_custom_scalar());
            assert_eq!(dt.specified_by_url.as_deref(), Some("https://example.com/datetime"));
        }

        // Verify input object default value
        if let Some(GraphQLNamedType::InputObject(input)) = registry.get("SearchInput") {
            let limit = &input.fields["limit"];
            assert_eq!(limit.default_value, Some(GraphQLValue::Int(10)));
        }
    }

    #[test]
    fn test_field_arguments_resolved() {
        let schema = parse_schema(
            r#"
            type Query {
                user(id: ID!, name: String): User
            }
            type User {
                name: String
            }
            "#,
        );
        let registry = TypeRegistry::from_schema(&schema);
        let query = registry.get("Query").unwrap();
        if let GraphQLNamedType::Object(obj) = query {
            let field = &obj.fields["user"];
            assert_eq!(field.arguments.len(), 2);
            assert_eq!(field.arguments[0].name, "id");
            assert!(!field.arguments[0].type_.is_nullable());
            assert_eq!(field.arguments[1].name, "name");
            assert!(field.arguments[1].type_.is_nullable());
        }
    }

    // MARK: - GAP-01 Tests (__typename filtering)

    fn parse_and_compile(sdl: &str, query_str: &str) -> (
        TypeRegistry,
        apollo_compiler::validation::Valid<executable::ExecutableDocument>,
        Valid<schema::Schema>,
    ) {
        let schema = schema::Schema::parse_and_validate(sdl, "test.graphql")
            .expect("Schema should parse and validate");
        let registry = TypeRegistry::from_schema(&schema);
        let doc = executable::ExecutableDocument::parse_and_validate(
            &schema,
            query_str,
            "test_query.graphql",
        )
        .expect("Query should parse and validate");
        (registry, doc, schema)
    }

    #[test]
    fn gap01_typename_filtered_from_selections() {
        let sdl = r#"
            type Query {
                user: User
            }
            type User {
                id: ID!
                name: String
            }
        "#;
        let query = r#"
            query GetUser {
                user {
                    __typename
                    id
                    name
                }
            }
        "#;
        let (registry, doc, _schema) = parse_and_compile(sdl, query);
        let fragment_defs: IndexMap<String, Arc<compilation_result::FragmentDefinition>> =
            IndexMap::new();

        let op = doc.operations.get(Some("GetUser")).unwrap();
        let parent_type = resolve_composite_type("Query", &registry).unwrap();
        let selection_set = convert_selection_set(
            &op.selection_set.selections,
            &parent_type,
            &registry,
            &fragment_defs,
        );

        // The top-level selection is a `user` field with a sub-selection set
        assert_eq!(selection_set.selections.len(), 1);
        if let compilation_result::Selection::Field(user_field) = &selection_set.selections[0] {
            assert_eq!(user_field.name, "user");
            let sub = user_field.selection_set.as_ref().unwrap();
            // GAP-01: __typename should be filtered out, leaving only id and name
            let field_names: Vec<&str> = sub
                .selections
                .iter()
                .filter_map(|s| match s {
                    compilation_result::Selection::Field(f) => Some(f.name.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(field_names, vec!["id", "name"]);
            assert!(
                !field_names.contains(&"__typename"),
                "GAP-01: __typename should be filtered from selections"
            );
        } else {
            panic!("Expected Field selection");
        }
    }

    #[test]
    fn gap01_typename_only_selection_produces_empty_selections() {
        let sdl = r#"
            type Query {
                user: User
            }
            type User {
                id: ID!
                name: String
            }
        "#;
        let query = r#"
            query TypenameOnly {
                user {
                    __typename
                }
            }
        "#;
        let (registry, doc, _schema) = parse_and_compile(sdl, query);
        let fragment_defs: IndexMap<String, Arc<compilation_result::FragmentDefinition>> =
            IndexMap::new();

        let op = doc.operations.get(Some("TypenameOnly")).unwrap();
        let parent_type = resolve_composite_type("Query", &registry).unwrap();
        let selection_set = convert_selection_set(
            &op.selection_set.selections,
            &parent_type,
            &registry,
            &fragment_defs,
        );

        if let compilation_result::Selection::Field(user_field) = &selection_set.selections[0] {
            let sub = user_field.selection_set.as_ref().unwrap();
            // GAP-01: Only __typename was selected, so after filtering, selections are empty
            assert!(
                sub.selections.is_empty(),
                "GAP-01: Filtering __typename from sole selection should leave empty set"
            );
        } else {
            panic!("Expected Field selection");
        }
    }

    // MARK: - GAP-02 Tests (direct fragment references only)

    #[test]
    fn gap02_only_direct_fragment_references_collected() {
        let sdl = r#"
            type Query {
                user: User
            }
            type User {
                id: ID!
                name: String
                email: String
            }
        "#;
        let query = r#"
            query GetUser {
                user {
                    ...UserBasic
                }
            }
            fragment UserBasic on User {
                id
                name
                ...UserEmail
            }
            fragment UserEmail on User {
                email
            }
        "#;
        let schema = schema::Schema::parse_and_validate(sdl, "test.graphql")
            .expect("Schema should parse");
        let doc = executable::ExecutableDocument::parse_and_validate(
            &schema,
            query,
            "test_query.graphql",
        )
        .expect("Query should parse");

        // Build fragment definitions first
        let user_type = {
            let registry = TypeRegistry::from_schema(&schema);
            resolve_composite_type("User", &registry).unwrap()
        };

        // Create mock fragment definitions
        let user_email_frag = Arc::new(compilation_result::FragmentDefinition {
            name: "UserEmail".to_string(),
            type_: user_type.clone(),
            selection_set: compilation_result::SelectionSet {
                parent_type: user_type.clone(),
                selections: vec![],
            },
            directives: None,
            referenced_fragments: vec![],
            source: String::new(),
            file_path: String::new(),
        });

        let user_basic_frag = Arc::new(compilation_result::FragmentDefinition {
            name: "UserBasic".to_string(),
            type_: user_type.clone(),
            selection_set: compilation_result::SelectionSet {
                parent_type: user_type.clone(),
                selections: vec![],
            },
            directives: None,
            referenced_fragments: vec![Arc::clone(&user_email_frag)],
            source: String::new(),
            file_path: String::new(),
        });

        let mut fragment_defs: IndexMap<String, Arc<compilation_result::FragmentDefinition>> =
            IndexMap::new();
        fragment_defs.insert("UserBasic".to_string(), Arc::clone(&user_basic_frag));
        fragment_defs.insert("UserEmail".to_string(), Arc::clone(&user_email_frag));

        // Collect referenced fragments for the operation
        let op = doc.operations.get(Some("GetUser")).unwrap();
        let refs = collect_referenced_fragments(&op.selection_set.selections, &fragment_defs);

        // GAP-02: Only UserBasic should appear (direct spread in operation).
        // UserEmail is referenced by UserBasic, but NOT directly by the operation.
        assert_eq!(refs.len(), 1, "GAP-02: Only direct fragment references should be collected");
        assert_eq!(refs[0].name, "UserBasic");
    }

    #[test]
    fn gap02_multiple_direct_fragments_collected() {
        let sdl = r#"
            type Query {
                user: User
            }
            type User {
                id: ID!
                name: String
                email: String
            }
        "#;
        let query = r#"
            query GetUser {
                user {
                    ...UserBasic
                    ...UserEmail
                }
            }
            fragment UserBasic on User {
                id
                name
            }
            fragment UserEmail on User {
                email
            }
        "#;
        let schema = schema::Schema::parse_and_validate(sdl, "test.graphql")
            .expect("Schema should parse");
        let doc = executable::ExecutableDocument::parse_and_validate(
            &schema,
            query,
            "test_query.graphql",
        )
        .expect("Query should parse");

        let user_type = {
            let registry = TypeRegistry::from_schema(&schema);
            resolve_composite_type("User", &registry).unwrap()
        };

        let user_basic_frag = Arc::new(compilation_result::FragmentDefinition {
            name: "UserBasic".to_string(),
            type_: user_type.clone(),
            selection_set: compilation_result::SelectionSet {
                parent_type: user_type.clone(),
                selections: vec![],
            },
            directives: None,
            referenced_fragments: vec![],
            source: String::new(),
            file_path: String::new(),
        });
        let user_email_frag = Arc::new(compilation_result::FragmentDefinition {
            name: "UserEmail".to_string(),
            type_: user_type.clone(),
            selection_set: compilation_result::SelectionSet {
                parent_type: user_type.clone(),
                selections: vec![],
            },
            directives: None,
            referenced_fragments: vec![],
            source: String::new(),
            file_path: String::new(),
        });

        let mut fragment_defs: IndexMap<String, Arc<compilation_result::FragmentDefinition>> =
            IndexMap::new();
        fragment_defs.insert("UserBasic".to_string(), Arc::clone(&user_basic_frag));
        fragment_defs.insert("UserEmail".to_string(), Arc::clone(&user_email_frag));

        let op = doc.operations.get(Some("GetUser")).unwrap();
        let refs = collect_referenced_fragments(&op.selection_set.selections, &fragment_defs);

        // Both are direct references in the operation
        assert_eq!(refs.len(), 2, "Both direct fragment spreads should be collected");
        let names: Vec<&str> = refs.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"UserBasic"));
        assert!(names.contains(&"UserEmail"));
    }

    // MARK: - GAP-03 Tests (referenced types filtering)

    #[test]
    fn gap03_introspection_types_excluded() {
        let sdl = r#"
            type Query {
                user: User
            }
            type User {
                name: String
            }
        "#;
        let schema = parse_schema(sdl);
        let registry = TypeRegistry::from_schema(&schema);

        // Verify the registry includes introspection types (from apollo-rs)
        let has_introspection = registry
            .all_types()
            .keys()
            .any(|name| name.starts_with("__"));
        assert!(
            has_introspection,
            "Registry should include __-prefixed introspection types from apollo-rs"
        );

        // Build a minimal operation referencing User
        let user_type = resolve_composite_type("User", &registry).unwrap();
        let query_type = resolve_composite_type("Query", &registry).unwrap();
        let op = compilation_result::OperationDefinition {
            name: "GetUser".to_string(),
            operation_type: compilation_result::OperationType::Query,
            variables: vec![],
            root_type: query_type,
            selection_set: compilation_result::SelectionSet {
                parent_type: user_type.clone(),
                selections: vec![compilation_result::Selection::Field(
                    compilation_result::Field {
                        name: "name".to_string(),
                        alias: None,
                        type_: GraphQLType::Scalar(Arc::new(GraphQLScalarType {
                            name: GraphQLName::new("String".to_string()),
                            documentation: None,
                            specified_by_url: None,
                        })),
                        arguments: None,
                        inclusion_conditions: None,
                        directives: None,
                        selection_set: None,
                        deprecation_reason: None,
                        documentation: None,
                    },
                )],
            },
            directives: None,
            referenced_fragments: vec![],
            source: String::new(),
            file_path: String::new(),
        };

        let referenced = collect_referenced_types(&registry, &[op], &[]);

        // GAP-03: No introspection types should appear in the result
        for rt in &referenced {
            let name = &rt.name().schema_name;
            assert!(
                !name.starts_with("__"),
                "GAP-03: Introspection type '{}' should be excluded from referenced_types",
                name
            );
        }
    }

    #[test]
    fn gap03_only_referenced_types_included() {
        let sdl = r#"
            type Query {
                user: User
            }
            type User {
                name: String
            }
            type UnusedType {
                value: Int
            }
            enum UnusedEnum {
                A
                B
            }
        "#;
        let schema = parse_schema(sdl);
        let registry = TypeRegistry::from_schema(&schema);

        let user_type = resolve_composite_type("User", &registry).unwrap();
        let query_type = resolve_composite_type("Query", &registry).unwrap();
        let op = compilation_result::OperationDefinition {
            name: "GetUser".to_string(),
            operation_type: compilation_result::OperationType::Query,
            variables: vec![],
            root_type: query_type,
            selection_set: compilation_result::SelectionSet {
                parent_type: user_type.clone(),
                selections: vec![compilation_result::Selection::Field(
                    compilation_result::Field {
                        name: "name".to_string(),
                        alias: None,
                        type_: GraphQLType::Scalar(Arc::new(GraphQLScalarType {
                            name: GraphQLName::new("String".to_string()),
                            documentation: None,
                            specified_by_url: None,
                        })),
                        arguments: None,
                        inclusion_conditions: None,
                        directives: None,
                        selection_set: None,
                        deprecation_reason: None,
                        documentation: None,
                    },
                )],
            },
            directives: None,
            referenced_fragments: vec![],
            source: String::new(),
            file_path: String::new(),
        };

        let referenced = collect_referenced_types(&registry, &[op], &[]);
        let ref_names: Vec<&str> = referenced
            .iter()
            .map(|rt| rt.name().schema_name.as_str())
            .collect();

        // GAP-03: Only types actually referenced by the operation should appear
        assert!(ref_names.contains(&"Query"), "Query root type should be included");
        assert!(ref_names.contains(&"User"), "User type should be included");
        assert!(ref_names.contains(&"String"), "String scalar (used by User.name) should be included");
        assert!(
            !ref_names.contains(&"UnusedType"),
            "GAP-03: UnusedType should be excluded -- not referenced by any operation"
        );
        assert!(
            !ref_names.contains(&"UnusedEnum"),
            "GAP-03: UnusedEnum should be excluded -- not referenced by any operation"
        );
    }

    #[test]
    fn gap03_fragment_types_included() {
        let sdl = r#"
            type Query {
                user: User
            }
            type User {
                name: String
                status: UserStatus
            }
            enum UserStatus {
                ACTIVE
                INACTIVE
            }
        "#;
        let schema = parse_schema(sdl);
        let registry = TypeRegistry::from_schema(&schema);

        let user_type = resolve_composite_type("User", &registry).unwrap();

        // Create a fragment that references UserStatus
        let frag = compilation_result::FragmentDefinition {
            name: "UserFields".to_string(),
            type_: user_type.clone(),
            selection_set: compilation_result::SelectionSet {
                parent_type: user_type.clone(),
                selections: vec![compilation_result::Selection::Field(
                    compilation_result::Field {
                        name: "status".to_string(),
                        alias: None,
                        type_: GraphQLType::Enum(Arc::new(GraphQLEnumType {
                            name: GraphQLName::new("UserStatus".to_string()),
                            documentation: None,
                            values: vec![],
                        })),
                        arguments: None,
                        inclusion_conditions: None,
                        directives: None,
                        selection_set: None,
                        deprecation_reason: None,
                        documentation: None,
                    },
                )],
            },
            directives: None,
            referenced_fragments: vec![],
            source: String::new(),
            file_path: String::new(),
        };

        let referenced = collect_referenced_types(&registry, &[], &[frag]);
        let ref_names: Vec<&str> = referenced
            .iter()
            .map(|rt| rt.name().schema_name.as_str())
            .collect();

        assert!(
            ref_names.contains(&"UserStatus"),
            "GAP-03: UserStatus referenced by fragment should be included"
        );
        assert!(
            ref_names.contains(&"User"),
            "GAP-03: User (fragment type condition) should be included"
        );
    }

    // MARK: - build_network_request_source tests (GAP-05)

    #[test]
    fn test_build_network_request_source_simple_query() {
        let input = "query Foo {\n  hero {\n    name\n  }\n}";
        let result = super::build_network_request_source(input);
        let expected = "query Foo {\n  hero {\n    __typename\n    name\n  }\n}";
        assert_eq!(result, expected, "Should inject __typename into field selection set");
    }

    #[test]
    fn test_build_network_request_source_existing_typename() {
        let input = "query Bar {\n  hero {\n    __typename\n    name\n  }\n}";
        let result = super::build_network_request_source(input);
        let expected = "query Bar {\n  hero {\n    __typename\n    name\n  }\n}";
        assert_eq!(result, expected, "Should not duplicate __typename");
    }

    #[test]
    fn test_build_network_request_source_fragment_definition() {
        let input = "fragment F on Human {\n  name\n}";
        let result = super::build_network_request_source(input);
        let expected = "fragment F on Human {\n  __typename\n  name\n}";
        assert_eq!(result, expected, "Should inject __typename into fragment definition");
    }

    #[test]
    fn test_build_network_request_source_no_typename_at_operation_root() {
        let input = "query Q {\n  hero {\n    name\n  }\n  search {\n    name\n  }\n}";
        let result = super::build_network_request_source(input);
        // __typename should be inside hero and search, but NOT at operation root
        assert!(result.contains("hero {\n    __typename\n    name\n  }"), "hero should get __typename");
        assert!(result.contains("search {\n    __typename\n    name\n  }"), "search should get __typename");
        // Root level should not have __typename
        let lines: Vec<&str> = result.lines().collect();
        // The first line after "query Q {" should be "  hero {", not "__typename"
        assert_eq!(lines[1].trim(), "hero {", "Root level should not have __typename injected");
    }

    #[test]
    fn test_build_network_request_source_inline_fragment_no_typename() {
        let input = "query Q {\n  hero {\n    ... on Human {\n      name\n    }\n  }\n}";
        let result = super::build_network_request_source(input);
        // __typename should be inside hero field, but NOT inside the inline fragment itself
        // The inline fragment's children (fields) should not get __typename at the inline-frag level
        assert!(result.contains("hero {\n    __typename\n    ... on Human"), "hero field should get __typename");
        // Inside ... on Human, there should NOT be __typename at the inline fragment level
        // but name is a leaf field so no sub-selection to worry about
    }

    #[test]
    fn test_build_network_request_source_strip_apollo_directives() {
        let input = "query Q @apollo_client_ios_localCacheMutation {\n  hero {\n    name\n  }\n}";
        let result = super::build_network_request_source(input);
        assert!(!result.contains("apollo_client_ios_localCacheMutation"), "Should strip Apollo directive");
        assert!(result.contains("query Q {"), "Query should remain without directive");
    }

    #[test]
    fn test_build_network_request_source_nested_fields() {
        let input = "query Q {\n  hero {\n    friends {\n      name\n    }\n  }\n}";
        let result = super::build_network_request_source(input);
        assert!(result.contains("hero {\n    __typename\n    friends"), "hero should get __typename");
        assert!(result.contains("friends {\n      __typename\n      name"), "friends should get __typename");
    }
}
