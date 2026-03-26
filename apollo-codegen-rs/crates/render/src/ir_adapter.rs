//! Adapter that converts IR types into template configuration structs.
//!
//! This bridges the IR module's structured types to the string-based
//! configuration that the templates consume.

use crate::naming;
use crate::templates::fragment::FragmentConfig;
use crate::templates::operation::{OperationConfig, OperationType as TemplateOpType, VariableConfig as TemplateVariableConfig};
use crate::templates::selection_set::*;
use apollo_codegen_frontend::compilation_result::OperationType;
use apollo_codegen_frontend::types::{Argument, GraphQLCompositeType, GraphQLType, GraphQLValue};
use apollo_codegen_ir::builder::IRBuilder;
use apollo_codegen_ir::field_collector::TypeKind;
use apollo_codegen_ir::fields::EntityField;
use apollo_codegen_ir::named_fragment::NamedFragment;
use apollo_codegen_ir::operation::Operation;
use apollo_codegen_ir::selection_set::{
    DirectSelections, FieldSelection, InlineFragmentSelection, NamedFragmentSpread,
    SelectionSet as IrSelectionSet,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Render an operation to its complete Swift file content.
pub fn render_operation(
    op: &Operation,
    schema_namespace: &str,
    access_modifier: &str,
    generate_initializers: bool,
    type_kinds: &HashMap<String, TypeKind>,
) -> String {
    // Build owned strings we'll reference
    let op_type = match op.operation_type {
        OperationType::Query => TemplateOpType::Query,
        OperationType::Mutation => TemplateOpType::Mutation,
        OperationType::Subscription => TemplateOpType::Subscription,
    };

    let fragment_names: Vec<String> = op
        .referenced_fragments
        .iter()
        .map(|f| f.name.clone())
        .collect();
    let fragment_name_refs: Vec<&str> = fragment_names.iter().map(|s| s.as_str()).collect();

    let variables: Vec<OwnedVariableConfig> = op
        .variables
        .iter()
        .map(|v| OwnedVariableConfig {
            name: v.name.clone(),
            swift_type: v.type_str.clone(),
            default_value: v.default_value.clone(),
        })
        .collect();

    // Build the Data selection set config
    let data_ss = build_selection_set_config_owned(
        "Data",
        &op.root_field.selection_set,
        schema_namespace,
        access_modifier,
        true,  // is_root
        false, // is_inline_fragment
        SelectionSetConformance::SelectionSet,
        None,  // root_entity_type
        2,     // indent (inside class)
        &format!("{}.Data", op.name),
        generate_initializers,
        &op.referenced_fragments,
        type_kinds,
        None,  // no parent fields for root
    );

    let config = OwnedOperationConfig {
        class_name: op.name.clone(),
        operation_name: op.name.clone(),
        operation_type: op_type,
        schema_namespace: schema_namespace.to_string(),
        access_modifier: access_modifier.to_string(),
        source: op.source.clone(),
        fragment_names,
        variables,
        data_selection_set: data_ss,
    };

    render_owned_operation(&config)
}

/// Render a fragment to its complete Swift file content.
pub fn render_fragment(
    frag: &NamedFragment,
    schema_namespace: &str,
    access_modifier: &str,
    generate_initializers: bool,
    type_kinds: &HashMap<String, TypeKind>,
) -> String {
    let ss = build_selection_set_config_owned(
        &frag.name,
        &frag.root_field.selection_set,
        schema_namespace,
        access_modifier,
        true,
        false,
        SelectionSetConformance::Fragment,
        None,
        0, // top-level
        &frag.name,
        generate_initializers,
        &frag.referenced_fragments,
        type_kinds,
        None, // no parent fields for fragment root
    );

    let config = OwnedFragmentConfig {
        name: frag.name.clone(),
        fragment_definition: frag.source.clone(),
        schema_namespace: schema_namespace.to_string(),
        access_modifier: access_modifier.to_string(),
        selection_set: ss,
    };

    render_owned_fragment(&config)
}

// === Owned config types (own their strings) ===

struct OwnedOperationConfig {
    class_name: String,
    operation_name: String,
    operation_type: TemplateOpType,
    schema_namespace: String,
    access_modifier: String,
    source: String,
    fragment_names: Vec<String>,
    variables: Vec<OwnedVariableConfig>,
    data_selection_set: OwnedSelectionSetConfig,
}

struct OwnedFragmentConfig {
    name: String,
    fragment_definition: String,
    schema_namespace: String,
    access_modifier: String,
    selection_set: OwnedSelectionSetConfig,
}

struct OwnedVariableConfig {
    name: String,
    swift_type: String,
    default_value: Option<String>,
}

struct OwnedSelectionSetConfig {
    struct_name: String,
    schema_namespace: String,
    parent_type: OwnedParentTypeRef,
    is_root: bool,
    is_inline_fragment: bool,
    conformance: SelectionSetConformance<'static>,
    root_entity_type: Option<String>,
    merged_sources: Vec<String>,
    selections: Vec<OwnedSelectionItem>,
    field_accessors: Vec<OwnedFieldAccessor>,
    inline_fragment_accessors: Vec<OwnedInlineFragmentAccessor>,
    fragment_spreads: Vec<OwnedFragmentSpreadAccessor>,
    initializer: Option<OwnedInitializerConfig>,
    nested_types: Vec<OwnedNestedSelectionSet>,
    type_aliases: Vec<OwnedTypeAlias>,
    indent: usize,
    access_modifier: String,
}

enum OwnedParentTypeRef {
    Object(String),
    Interface(String),
    Union(String),
}

struct OwnedSelectionItem {
    kind: OwnedSelectionKind,
}

enum OwnedSelectionKind {
    Field { name: String, swift_type: String, arguments: Option<String> },
    InlineFragment(String),
    Fragment(String),
}

#[derive(Clone)]
struct OwnedFieldAccessor {
    name: String,
    swift_type: String,
}

struct OwnedInlineFragmentAccessor {
    property_name: String,
    type_name: String,
}

struct OwnedFragmentSpreadAccessor {
    property_name: String,
    fragment_type: String,
}

struct OwnedInitializerConfig {
    parameters: Vec<OwnedInitParam>,
    data_entries: Vec<OwnedDataEntry>,
    fulfilled_fragments: Vec<String>,
    typename_value: OwnedTypenameValue,
}

struct OwnedInitParam {
    name: String,
    swift_type: String,
    default_value: Option<String>,
}

struct OwnedDataEntry {
    key: String,
    value: OwnedDataEntryValue,
}

enum OwnedDataEntryValue {
    Variable(String),
    Typename(String),
    FieldData(String),
}

enum OwnedTypenameValue {
    Parameter,
    Fixed(String),
}

struct OwnedNestedSelectionSet {
    doc_comment: String,
    parent_type_comment: String,
    config: OwnedSelectionSetConfig,
}

struct OwnedTypeAlias {
    name: String,
    target: String,
}

// === Build functions ===

fn build_selection_set_config_owned(
    struct_name: &str,
    ir_ss: &IrSelectionSet,
    schema_namespace: &str,
    access_modifier: &str,
    is_root: bool,
    is_inline_fragment: bool,
    conformance: SelectionSetConformance<'static>,
    root_entity_type: Option<&str>,
    indent: usize,
    qualified_name: &str,
    generate_initializers: bool,
    referenced_fragments: &[Arc<NamedFragment>],
    type_kinds: &HashMap<String, TypeKind>,
    parent_fields: Option<&[OwnedFieldAccessor]>,
) -> OwnedSelectionSetConfig {
    let parent_type = match &ir_ss.scope.parent_type {
        GraphQLCompositeType::Object(o) => OwnedParentTypeRef::Object(o.name.clone()),
        GraphQLCompositeType::Interface(i) => OwnedParentTypeRef::Interface(i.name.clone()),
        GraphQLCompositeType::Union(u) => OwnedParentTypeRef::Union(u.name.clone()),
    };

    let ds = &ir_ss.direct_selections;

    // Determine whether __typename should appear in __selections.
    // It is added for all selection sets EXCEPT:
    //   - Inline fragments (they inherit __typename from the parent entity)
    //   - Root operation Data structs (is_root && conformance == SelectionSet)
    let is_root_operation_data = is_root && matches!(conformance, SelectionSetConformance::SelectionSet);
    let should_add_typename = !is_inline_fragment && !is_root_operation_data;

    // Build selections
    let mut selections = Vec::new();
    if should_add_typename {
        selections.push(OwnedSelectionItem {
            kind: OwnedSelectionKind::Field {
                name: "__typename".to_string(),
                swift_type: "String".to_string(),
                arguments: None,
            },
        });
    }
    for (key, field) in &ds.fields {
        let (swift_type, _is_entity) = render_field_swift_type(field, schema_namespace, type_kinds);
        let arguments = render_field_arguments(field);
        selections.push(OwnedSelectionItem {
            kind: OwnedSelectionKind::Field {
                name: key.clone(),
                swift_type,
                arguments,
            },
        });
    }
    for inline in &ds.inline_fragments {
        if let Some(ref tc) = inline.type_condition {
            let type_name = format!("As{}", naming::first_uppercased(tc.name()));
            selections.push(OwnedSelectionItem {
                kind: OwnedSelectionKind::InlineFragment(type_name),
            });
        }
    }
    for frag_spread in &ds.named_fragments {
        selections.push(OwnedSelectionItem {
            kind: OwnedSelectionKind::Fragment(frag_spread.fragment_name.clone()),
        });
    }

    // Build field accessors (skip __typename)
    let mut field_accessors: Vec<OwnedFieldAccessor> = ds
        .fields
        .iter()
        .map(|(key, field)| {
            let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds);
            OwnedFieldAccessor {
                name: key.clone(),
                swift_type,
            }
        })
        .collect();

    // For inline fragments, add merged field accessors from parent scope
    if is_inline_fragment {
        if let Some(parent) = parent_fields {
            for pf in parent {
                // Don't duplicate fields already directly selected
                if !field_accessors.iter().any(|f| f.name == pf.name) {
                    field_accessors.push(pf.clone());
                }
            }
        }
        // Also add fields from spread fragments
        for spread in &ds.named_fragments {
            if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == spread.fragment_name) {
                for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                    if !field_accessors.iter().any(|f| f.name == *key) {
                        let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds);
                        field_accessors.push(OwnedFieldAccessor {
                            name: key.clone(),
                            swift_type,
                        });
                    }
                }
            }
        }
    }

    // Build inline fragment accessors
    let inline_fragment_accessors: Vec<OwnedInlineFragmentAccessor> = ds
        .inline_fragments
        .iter()
        .filter_map(|inline| {
            inline.type_condition.as_ref().map(|tc| {
                let type_name = format!("As{}", naming::first_uppercased(tc.name()));
                OwnedInlineFragmentAccessor {
                    property_name: format!("as{}", naming::first_uppercased(tc.name())),
                    type_name,
                }
            })
        })
        .collect();

    // Build fragment spread accessors
    let fragment_spreads: Vec<OwnedFragmentSpreadAccessor> = ds
        .named_fragments
        .iter()
        .map(|spread| OwnedFragmentSpreadAccessor {
            property_name: naming::first_lowercased(&spread.fragment_name),
            fragment_type: spread.fragment_name.clone(),
        })
        .collect();

    // Build nested types
    let mut nested_types = Vec::new();
    // Nested entity fields
    for (key, field) in &ds.fields {
        if let FieldSelection::Entity(ef) = field {
            // Singularize the response key to get the struct name
            // (e.g., "allAnimals" → "AllAnimal", "predators" → "Predator")
            let singularized_key = naming::singularize(key);
            let child_name = naming::first_uppercased(&singularized_key);
            let child_qualified = format!("{}.{}", qualified_name, child_name);
            let child_ss = build_selection_set_config_owned(
                &child_name,
                &ef.selection_set,
                schema_namespace,
                access_modifier,
                false,
                false,
                SelectionSetConformance::SelectionSet,
                None,
                indent + 2,
                &child_qualified,
                generate_initializers,
                referenced_fragments,
                type_kinds,
                None, // entity fields don't inherit parent fields
            );
            let parent_type_name = ef.selection_set.scope.parent_type.name();
            nested_types.push(OwnedNestedSelectionSet {
                doc_comment: format!("/// {}", child_name),
                parent_type_comment: format!(
                    "///\n{}/// Parent Type: `{}`",
                    " ".repeat(indent + 2),
                    parent_type_name
                ),
                config: child_ss,
            });
        }
    }
    // Nested inline fragments
    for inline in &ds.inline_fragments {
        if let Some(ref tc) = inline.type_condition {
            let type_name = format!("As{}", naming::first_uppercased(tc.name()));
            let child_qualified = format!("{}.{}", qualified_name, type_name);
            let child_root_entity = if is_root {
                qualified_name.to_string()
            } else {
                root_entity_type.unwrap_or(qualified_name).to_string()
            };
            let child_ss = build_selection_set_config_owned(
                &type_name,
                &inline.selection_set,
                schema_namespace,
                access_modifier,
                false,
                true,
                SelectionSetConformance::InlineFragment,
                Some(&child_root_entity),
                indent + 2,
                &child_qualified,
                generate_initializers,
                referenced_fragments,
                type_kinds,
                Some(&field_accessors), // pass parent field accessors for merging
            );
            nested_types.push(OwnedNestedSelectionSet {
                doc_comment: format!(
                    "/// {}.{}",
                    struct_name,
                    type_name
                ),
                parent_type_comment: format!(
                    "///\n{}/// Parent Type: `{}`",
                    " ".repeat(indent + 2),
                    tc.name()
                ),
                config: child_ss,
            });
        }
    }

    // Build initializer when requested
    let initializer = if generate_initializers {
        Some(build_initializer_config(
            &ir_ss.scope.parent_type,
            ds,
            schema_namespace,
            qualified_name,
            is_inline_fragment,
            root_entity_type,
            referenced_fragments,
            type_kinds,
            &field_accessors, // includes merged fields for inline fragments
        ))
    } else {
        None
    };

    OwnedSelectionSetConfig {
        struct_name: struct_name.to_string(),
        schema_namespace: schema_namespace.to_string(),
        parent_type,
        is_root,
        is_inline_fragment,
        conformance,
        root_entity_type: root_entity_type.map(|s| s.to_string()),
        merged_sources: vec![],
        selections,
        field_accessors,
        inline_fragment_accessors,
        fragment_spreads,
        initializer,
        nested_types,
        type_aliases: vec![],
        indent,
        access_modifier: access_modifier.to_string(),
    }
}

/// Build an `OwnedInitializerConfig` for a selection set.
///
/// Rules:
/// - Object parent types get a fixed `__typename` in the data dict (no parameter).
/// - Interface/Union parent types get `__typename` as a parameter.
/// - Scalar fields become plain variable entries; entity fields use `._fieldData`.
/// - Optional Swift types (ending with `?`) get `= nil` default in parameters.
/// - `fulfilledFragments` always contains the current `qualified_name`.
///   For inline fragments it also contains the root entity type.
///   For named fragment spreads it also contains each spread fragment name.
fn build_initializer_config(
    parent_type: &GraphQLCompositeType,
    ds: &DirectSelections,
    schema_namespace: &str,
    qualified_name: &str,
    is_inline_fragment: bool,
    root_entity_type: Option<&str>,
    referenced_fragments: &[Arc<NamedFragment>],
    type_kinds: &HashMap<String, TypeKind>,
    all_field_accessors: &[OwnedFieldAccessor],
) -> OwnedInitializerConfig {
    // Determine typename handling based on parent type
    let parent_is_object = matches!(parent_type, GraphQLCompositeType::Object(_));

    let typename_value = if parent_is_object {
        let type_ref = format!(
            "{}.Objects.{}.typename",
            schema_namespace,
            naming::first_uppercased(parent_type.name())
        );
        OwnedTypenameValue::Fixed(type_ref)
    } else {
        OwnedTypenameValue::Parameter
    };

    // Build parameters
    let mut parameters = Vec::new();

    // If parent is Interface/Union, __typename is a parameter
    if !parent_is_object {
        parameters.push(OwnedInitParam {
            name: "__typename".to_string(),
            swift_type: "String".to_string(),
            default_value: None,
        });
    }

    // Add a parameter for each field accessor (includes merged fields for inline fragments)
    for accessor in all_field_accessors {
        let default_value = if accessor.swift_type.ends_with('?') {
            Some("nil".to_string())
        } else {
            None
        };
        parameters.push(OwnedInitParam {
            name: accessor.name.clone(),
            swift_type: accessor.swift_type.clone(),
            default_value,
        });
    }

    // Build data dict entries
    let mut data_entries = Vec::new();

    // __typename always comes first in the data dict
    data_entries.push(OwnedDataEntry {
        key: "__typename".to_string(),
        value: if parent_is_object {
            let type_ref = format!(
                "{}.Objects.{}.typename",
                schema_namespace,
                naming::first_uppercased(parent_type.name())
            );
            OwnedDataEntryValue::Typename(type_ref)
        } else {
            OwnedDataEntryValue::Variable("__typename".to_string())
        },
    });

    // Add each field accessor to data entries (includes merged fields)
    for accessor in all_field_accessors {
        // Check if this is an entity or scalar field
        let is_entity = ds.fields.get(&accessor.name)
            .map(|f| matches!(f, FieldSelection::Entity(_)))
            .unwrap_or(false);
        let value = if is_entity {
            OwnedDataEntryValue::FieldData(accessor.name.clone())
        } else {
            OwnedDataEntryValue::Variable(accessor.name.clone())
        };
        data_entries.push(OwnedDataEntry {
            key: accessor.name.clone(),
            value,
        });
    }

    // Build fulfilled fragments
    let mut fulfilled_fragments = Vec::new();

    if is_inline_fragment {
        // Inline fragments include the root entity type first, then self
        if let Some(root_entity) = root_entity_type {
            fulfilled_fragments.push(root_entity.to_string());
        }
        fulfilled_fragments.push(qualified_name.to_string());
    } else {
        // Non-inline-fragment selection sets include self first
        fulfilled_fragments.push(qualified_name.to_string());
    }

    // Add directly spread named fragments to fulfilled fragments
    for spread in &ds.named_fragments {
        fulfilled_fragments.push(spread.fragment_name.clone());
    }

    OwnedInitializerConfig {
        parameters,
        data_entries,
        fulfilled_fragments,
        typename_value,
    }
}

/// Render a GraphQL field type as a Swift type string.
fn render_field_swift_type(
    field: &FieldSelection,
    schema_namespace: &str,
    type_kinds: &HashMap<String, TypeKind>,
) -> (String, bool) {
    match field {
        FieldSelection::Scalar(sf) => {
            let swift_type = render_graphql_type_as_swift(&sf.field_type, schema_namespace, type_kinds);
            (swift_type, false)
        }
        FieldSelection::Entity(ef) => {
            // Entity fields use the singularized struct name from the response key
            let singularized_key = naming::singularize(ef.response_key());
            let struct_name = naming::first_uppercased(&singularized_key);
            let swift_type = wrap_type_with_struct_name(&ef.field_type, &struct_name);
            (swift_type, true)
        }
    }
}

/// Render field arguments as a Swift dictionary literal string.
fn render_field_arguments(field: &FieldSelection) -> Option<String> {
    let args = match field {
        FieldSelection::Scalar(sf) => &sf.arguments,
        FieldSelection::Entity(ef) => &ef.arguments,
    };
    if args.is_empty() {
        return None;
    }
    let entries: Vec<String> = args
        .iter()
        .map(|arg| format!("\"{}\": {}", arg.name, render_argument_value(&arg.value)))
        .collect();
    Some(format!("[{}]", entries.join(", ")))
}

/// Render a GraphQL argument value as a Swift expression.
fn render_argument_value(value: &GraphQLValue) -> String {
    match value {
        GraphQLValue::Variable(name) => format!(".variable(\"{}\")", name),
        GraphQLValue::String(s) => format!("\"{}\"", s),
        GraphQLValue::Int(i) => i.to_string(),
        GraphQLValue::Float(f) => f.to_string(),
        GraphQLValue::Boolean(b) => if *b { "true".to_string() } else { "false".to_string() },
        GraphQLValue::Null => ".null".to_string(),
        GraphQLValue::Enum(e) => format!(".init(.{})", naming::to_camel_case(e)),
        GraphQLValue::List(items) => {
            let rendered: Vec<String> = items.iter().map(render_argument_value).collect();
            format!("[{}]", rendered.join(", "))
        }
        GraphQLValue::Object(map) => {
            let entries: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("\"{}\": {}", k, render_argument_value(v)))
                .collect();
            format!("[{}]", entries.join(", "))
        }
    }
}

/// Render a GraphQL type as a Swift type string for scalar fields.
fn render_graphql_type_as_swift(
    ty: &GraphQLType,
    schema_namespace: &str,
    type_kinds: &HashMap<String, TypeKind>,
) -> String {
    match ty {
        GraphQLType::Named(name) => render_named_type_as_swift(name, schema_namespace, type_kinds),
        GraphQLType::NonNull(inner) => {
            let inner_str = render_graphql_type_as_swift(inner, schema_namespace, type_kinds);
            // Remove trailing ? if present (NonNull removes optionality)
            if inner_str.ends_with('?') {
                inner_str[..inner_str.len() - 1].to_string()
            } else {
                inner_str
            }
        }
        GraphQLType::List(inner) => {
            let inner_str = render_graphql_type_as_swift(inner, schema_namespace, type_kinds);
            format!("[{}]?", inner_str)
        }
    }
}

fn render_named_type_as_swift(
    name: &str,
    schema_namespace: &str,
    type_kinds: &HashMap<String, TypeKind>,
) -> String {
    match name {
        "String" => "String?".to_string(),
        "Int" => "Int?".to_string(),
        "Float" => "Double?".to_string(),
        "Boolean" => "Bool?".to_string(),
        "ID" => format!("{}.ID?", schema_namespace),
        _ => {
            let kind = type_kinds
                .get(name)
                .copied()
                .unwrap_or(TypeKind::Scalar);
            match kind {
                TypeKind::Enum => format!("GraphQLEnum<{}.{}>?", schema_namespace, name),
                TypeKind::Scalar => format!("{}.{}?", schema_namespace, name),
                TypeKind::Object | TypeKind::Interface | TypeKind::Union => {
                    // Composite types used as scalars (e.g., custom JSON Object type)
                    format!("{}.{}?", schema_namespace, name)
                }
                TypeKind::InputObject => format!("{}?", name),
            }
        }
    }
}

/// Wrap a GraphQL type using a local struct name for entity fields.
fn wrap_type_with_struct_name(ty: &GraphQLType, struct_name: &str) -> String {
    match ty {
        GraphQLType::Named(_) => format!("{}?", struct_name),
        GraphQLType::NonNull(inner) => {
            let inner_str = wrap_type_with_struct_name(inner, struct_name);
            if inner_str.ends_with('?') {
                inner_str[..inner_str.len() - 1].to_string()
            } else {
                inner_str
            }
        }
        GraphQLType::List(inner) => {
            let inner_str = wrap_type_with_struct_name(inner, struct_name);
            format!("[{}]?", inner_str)
        }
    }
}

// === Rendering owned configs to template configs ===

fn render_owned_operation(config: &OwnedOperationConfig) -> String {
    let frag_refs: Vec<&str> = config.fragment_names.iter().map(|s| s.as_str()).collect();
    let var_refs: Vec<TemplateVariableConfig> = config
        .variables
        .iter()
        .map(|v| TemplateVariableConfig {
            name: &v.name,
            swift_type: &v.swift_type,
            default_value: v.default_value.as_deref(),
        })
        .collect();
    let data_ss = owned_to_ref_selection_set(&config.data_selection_set);

    let template_config = OperationConfig {
        class_name: &config.class_name,
        operation_name: &config.operation_name,
        operation_type: config.operation_type,
        schema_namespace: &config.schema_namespace,
        access_modifier: &config.access_modifier,
        source: &config.source,
        fragment_names: frag_refs,
        variables: var_refs,
        data_selection_set: data_ss,
    };

    crate::templates::operation::render(&template_config)
}

fn render_owned_fragment(config: &OwnedFragmentConfig) -> String {
    let ss = owned_to_ref_selection_set(&config.selection_set);

    let template_config = FragmentConfig {
        name: &config.name,
        fragment_definition: &config.fragment_definition,
        schema_namespace: &config.schema_namespace,
        access_modifier: &config.access_modifier,
        selection_set: ss,
    };

    crate::templates::fragment::render(&template_config)
}

fn owned_to_ref_selection_set(owned: &OwnedSelectionSetConfig) -> SelectionSetConfig<'_> {
    let parent_type = match &owned.parent_type {
        OwnedParentTypeRef::Object(n) => ParentTypeRef::Object(n.as_str()),
        OwnedParentTypeRef::Interface(n) => ParentTypeRef::Interface(n.as_str()),
        OwnedParentTypeRef::Union(n) => ParentTypeRef::Union(n.as_str()),
    };

    let selections: Vec<SelectionItem<'_>> = owned
        .selections
        .iter()
        .map(|s| match &s.kind {
            OwnedSelectionKind::Field { name, swift_type, arguments } => {
                SelectionItem::Field(FieldSelectionItem {
                    name: name.as_str(),
                    swift_type: swift_type.as_str(),
                    arguments: arguments.as_deref(),
                })
            }
            OwnedSelectionKind::InlineFragment(name) => SelectionItem::InlineFragment(name.as_str()),
            OwnedSelectionKind::Fragment(name) => SelectionItem::Fragment(name.as_str()),
        })
        .collect();

    let field_accessors: Vec<FieldAccessor<'_>> = owned
        .field_accessors
        .iter()
        .map(|a| FieldAccessor {
            name: &a.name,
            swift_type: &a.swift_type,
        })
        .collect();

    let inline_fragment_accessors: Vec<InlineFragmentAccessor<'_>> = owned
        .inline_fragment_accessors
        .iter()
        .map(|a| InlineFragmentAccessor {
            property_name: &a.property_name,
            type_name: &a.type_name,
        })
        .collect();

    let fragment_spreads: Vec<FragmentSpreadAccessor<'_>> = owned
        .fragment_spreads
        .iter()
        .map(|a| FragmentSpreadAccessor {
            property_name: &a.property_name,
            fragment_type: &a.fragment_type,
        })
        .collect();

    let merged_sources: Vec<&str> = owned.merged_sources.iter().map(|s| s.as_str()).collect();

    let initializer = owned.initializer.as_ref().map(|init| {
        let params: Vec<InitParam<'_>> = init
            .parameters
            .iter()
            .map(|p| InitParam {
                name: &p.name,
                swift_type: &p.swift_type,
                default_value: p.default_value.as_deref(),
            })
            .collect();
        let entries: Vec<DataEntry<'_>> = init
            .data_entries
            .iter()
            .map(|e| DataEntry {
                key: &e.key,
                value: match &e.value {
                    OwnedDataEntryValue::Variable(v) => DataEntryValue::Variable(v.as_str()),
                    OwnedDataEntryValue::Typename(t) => DataEntryValue::Typename(t.as_str()),
                    OwnedDataEntryValue::FieldData(f) => DataEntryValue::FieldData(f.as_str()),
                },
            })
            .collect();
        let fulfilled: Vec<&str> = init.fulfilled_fragments.iter().map(|s| s.as_str()).collect();
        let typename = match &init.typename_value {
            OwnedTypenameValue::Parameter => TypenameValue::Parameter,
            OwnedTypenameValue::Fixed(f) => TypenameValue::Fixed(f.as_str()),
        };
        // Leak the vecs to get 'static references - this is fine since we're rendering immediately
        // Actually we can't leak, so we'll box them
        InitializerConfig {
            parameters: params,
            data_entries: entries,
            fulfilled_fragments: fulfilled,
            typename_value: typename,
        }
    });

    let nested_types: Vec<NestedSelectionSet<'_>> = owned
        .nested_types
        .iter()
        .map(|n| NestedSelectionSet {
            doc_comment: &n.doc_comment,
            parent_type_comment: &n.parent_type_comment,
            config: owned_to_ref_selection_set(&n.config),
        })
        .collect();

    let type_aliases: Vec<TypeAliasConfig<'_>> = owned
        .type_aliases
        .iter()
        .map(|a| TypeAliasConfig {
            name: &a.name,
            target: &a.target,
        })
        .collect();

    SelectionSetConfig {
        struct_name: &owned.struct_name,
        schema_namespace: &owned.schema_namespace,
        parent_type,
        is_root: owned.is_root,
        is_inline_fragment: owned.is_inline_fragment,
        conformance: owned.conformance.clone(),
        root_entity_type: owned.root_entity_type.as_deref(),
        merged_sources,
        selections,
        field_accessors,
        inline_fragment_accessors,
        fragment_spreads,
        initializer,
        nested_types,
        type_aliases,
        indent: owned.indent,
        access_modifier: &owned.access_modifier,
    }
}
