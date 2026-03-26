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

    // Class name: local cache mutations use the operation name as-is,
    // regular operations include the operation type suffix if not already present.
    let class_name = if op.is_local_cache_mutation {
        op.name.clone()
    } else {
        let type_suffix = match op.operation_type {
            OperationType::Query => "Query",
            OperationType::Mutation => "Mutation",
            OperationType::Subscription => "Subscription",
        };
        if op.name.ends_with(type_suffix) {
            op.name.clone()
        } else {
            format!("{}{}", op.name, type_suffix)
        }
    };

    // Build the Data selection set config
    let data_conformance = if op.is_local_cache_mutation {
        SelectionSetConformance::MutableSelectionSet
    } else {
        SelectionSetConformance::SelectionSet
    };
    let data_ss = build_selection_set_config_owned(
        "Data",
        &op.root_field.selection_set,
        schema_namespace,
        access_modifier,
        true,  // is_root
        false, // is_inline_fragment
        data_conformance,
        None,  // root_entity_type
        2,     // indent (inside class)
        &format!("{}.Data", class_name),
        generate_initializers,
        &op.referenced_fragments,
        type_kinds,
        None,  // no parent fields for root
        op.is_local_cache_mutation,
    );

    let config = OwnedOperationConfig {
        class_name: class_name.clone(),
        operation_name: op.name.clone(),
        operation_type: op_type,
        schema_namespace: schema_namespace.to_string(),
        access_modifier: access_modifier.to_string(),
        source: op.source.clone(),
        fragment_names,
        variables,
        data_selection_set: data_ss,
        is_local_cache_mutation: op.is_local_cache_mutation,
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
    let frag_conformance = if frag.is_local_cache_mutation {
        SelectionSetConformance::MutableFragment
    } else {
        SelectionSetConformance::Fragment
    };
    let ss = build_selection_set_config_owned(
        &frag.name,
        &frag.root_field.selection_set,
        schema_namespace,
        access_modifier,
        true,
        false,
        frag_conformance,
        None,
        0, // top-level
        &frag.name,
        generate_initializers,
        &frag.referenced_fragments,
        type_kinds,
        None, // no parent fields for fragment root
        frag.is_local_cache_mutation,
    );

    let config = OwnedFragmentConfig {
        name: frag.name.clone(),
        fragment_definition: frag.source.clone(),
        schema_namespace: schema_namespace.to_string(),
        access_modifier: access_modifier.to_string(),
        selection_set: ss,
        is_mutable: frag.is_local_cache_mutation,
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
    is_local_cache_mutation: bool,
}

struct OwnedFragmentConfig {
    name: String,
    fragment_definition: String,
    schema_namespace: String,
    access_modifier: String,
    selection_set: OwnedSelectionSetConfig,
    is_mutable: bool,
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
    is_mutable: bool,
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
    is_mutable: bool,
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
    let is_root_operation_data = is_root && matches!(conformance, SelectionSetConformance::SelectionSet | SelectionSetConformance::MutableSelectionSet);
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
        // Skip __typename since it's added explicitly above when needed
        if key == "__typename" { continue; }
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
        .filter(|(key, _)| key.as_str() != "__typename") // __typename handled separately
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
                if pf.name != "__typename" && !field_accessors.iter().any(|f| f.name == pf.name) {
                    field_accessors.push(pf.clone());
                }
            }
        }
    }

    // For ALL selection sets with named fragment spreads, include the spread
    // fragment's fields as merged accessors (e.g., WarmBloodedDetails spreading
    // HeightInMeters gets a `height` accessor from HeightInMeters)
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

    // Build inline fragment accessors (start with direct, add promoted later)
    let mut inline_fragment_accessors: Vec<OwnedInlineFragmentAccessor> = ds
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
    let mut fragment_spreads: Vec<OwnedFragmentSpreadAccessor> = ds
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
            let child_conformance = if is_mutable {
                SelectionSetConformance::MutableSelectionSet
            } else {
                SelectionSetConformance::SelectionSet
            };
            let mut child_ss = build_selection_set_config_owned(
                &child_name,
                &ef.selection_set,
                schema_namespace,
                access_modifier,
                false,
                false,
                child_conformance,
                None,
                indent + 2,
                &child_qualified,
                generate_initializers,
                referenced_fragments,
                type_kinds,
                None, // entity fields don't inherit parent fields
                is_mutable,
            );
            // Merge fields from fragment spreads that also have this entity field.
            // E.g., if HeightInMeters has `height { meters }`, merge `meters` into Height.
            for spread in &ds.named_fragments {
                if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == spread.fragment_name) {
                    if let Some(FieldSelection::Entity(frag_ef)) = frag_arc.root_field.selection_set.direct_selections.fields.get(key) {
                        for (frag_key, frag_field) in &frag_ef.selection_set.direct_selections.fields {
                            if frag_key == "__typename" { continue; }
                            if !child_ss.field_accessors.iter().any(|f| f.name == *frag_key) {
                                let (swift_type, _) = render_field_swift_type(frag_field, schema_namespace, type_kinds);
                                child_ss.field_accessors.push(OwnedFieldAccessor { name: frag_key.clone(), swift_type });
                                // Also add to initializer if it exists
                                if let Some(ref mut init) = child_ss.initializer {
                                    init.parameters.push(OwnedInitParam {
                                        name: frag_key.clone(),
                                        swift_type: {
                                            let (st, _) = render_field_swift_type(frag_field, schema_namespace, type_kinds);
                                            st
                                        },
                                        default_value: {
                                            let (st, _) = render_field_swift_type(frag_field, schema_namespace, type_kinds);
                                            if st.ends_with('?') { Some("nil".to_string()) } else { None }
                                        },
                                    });
                                    init.data_entries.push(OwnedDataEntry {
                                        key: frag_key.clone(),
                                        value: OwnedDataEntryValue::Variable(frag_key.clone()),
                                    });
                                }
                            }
                        }
                        // Add the fragment's entity type to fulfilled fragments
                        if let Some(ref mut init) = child_ss.initializer {
                            let frag_entity_qualified = format!("{}.{}", spread.fragment_name, child_name);
                            if !init.fulfilled_fragments.contains(&frag_entity_qualified) {
                                init.fulfilled_fragments.push(frag_entity_qualified);
                            }
                        }
                    }
                    // Also check sub-fragments
                    for sub_spread in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                        if let Some(sub_frag) = referenced_fragments.iter().find(|f| f.name == sub_spread.fragment_name) {
                            if let Some(FieldSelection::Entity(sub_ef)) = sub_frag.root_field.selection_set.direct_selections.fields.get(key) {
                                for (frag_key, frag_field) in &sub_ef.selection_set.direct_selections.fields {
                                    if frag_key == "__typename" { continue; }
                                    if !child_ss.field_accessors.iter().any(|f| f.name == *frag_key) {
                                        let (swift_type, _) = render_field_swift_type(frag_field, schema_namespace, type_kinds);
                                        child_ss.field_accessors.push(OwnedFieldAccessor { name: frag_key.clone(), swift_type });
                                        if let Some(ref mut init) = child_ss.initializer {
                                            init.parameters.push(OwnedInitParam {
                                                name: frag_key.clone(),
                                                swift_type: {
                                                    let (st, _) = render_field_swift_type(frag_field, schema_namespace, type_kinds);
                                                    st
                                                },
                                                default_value: {
                                                    let (st, _) = render_field_swift_type(frag_field, schema_namespace, type_kinds);
                                                    if st.ends_with('?') { Some("nil".to_string()) } else { None }
                                                },
                                            });
                                            init.data_entries.push(OwnedDataEntry {
                                                key: frag_key.clone(),
                                                value: OwnedDataEntryValue::Variable(frag_key.clone()),
                                            });
                                        }
                                    }
                                }
                                if let Some(ref mut init) = child_ss.initializer {
                                    let sub_frag_entity_qualified = format!("{}.{}", sub_spread.fragment_name, child_name);
                                    if !init.fulfilled_fragments.contains(&sub_frag_entity_qualified) {
                                        init.fulfilled_fragments.push(sub_frag_entity_qualified);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let parent_type_name = ef.selection_set.scope.parent_type.name();
            let doc_comment = if is_root {
                format!("/// {}", child_name)
            } else {
                format!("/// {}.{}", struct_name, child_name)
            };
            nested_types.push(OwnedNestedSelectionSet {
                doc_comment,
                parent_type_comment: format!(
                    "///\n{}/// Parent Type: `{}`",
                    " ".repeat(indent + 2),
                    parent_type_name
                ),
                config: child_ss,
            });
        }
    }
    // Collect sibling inline fragment fields for merging.
    // For each inline fragment type, collect fields from OTHER inline fragments
    // whose type conditions are supertypes (e.g., AsCat gets fields from AsAnimal, AsPet).
    let sibling_inline_fields: Vec<(&str, Vec<OwnedFieldAccessor>)> = ds
        .inline_fragments
        .iter()
        .filter_map(|inline| {
            inline.type_condition.as_ref().map(|tc| {
                let mut merged = Vec::new();
                for other in &ds.inline_fragments {
                    if let Some(ref other_tc) = other.type_condition {
                        if other_tc.name() != tc.name()
                            && is_supertype_of_current(tc, other_tc.name())
                        {
                            // other's type is a supertype of this type - merge its fields
                            for (key, field) in &other.selection_set.direct_selections.fields {
                                let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds);
                                if !merged.iter().any(|f: &OwnedFieldAccessor| f.name == *key) {
                                    merged.push(OwnedFieldAccessor { name: key.clone(), swift_type });
                                }
                            }
                        }
                    }
                }
                (tc.name(), merged)
            })
        })
        .collect();

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
            let inline_conformance = if is_mutable {
                SelectionSetConformance::MutableInlineFragment
            } else {
                SelectionSetConformance::InlineFragment
            };
            // Combine parent fields with sibling merged fields
            let mut merged_parent = field_accessors.clone();
            if let Some((_, sibling_fields)) = sibling_inline_fields.iter().find(|(name, _)| *name == tc.name()) {
                for sf in sibling_fields {
                    if !merged_parent.iter().any(|f| f.name == sf.name) {
                        merged_parent.push(sf.clone());
                    }
                }
            }
            let mut child_ss = build_selection_set_config_owned(
                &type_name,
                &inline.selection_set,
                schema_namespace,
                access_modifier,
                false,
                true,
                inline_conformance,
                Some(&child_root_entity),
                indent + 2,
                &child_qualified,
                generate_initializers,
                referenced_fragments,
                type_kinds,
                Some(&merged_parent), // pass parent + sibling field accessors
                is_mutable,
            );

            // Add sibling supertype fulfilled fragments to the initializer
            if let Some(ref mut init) = child_ss.initializer {
                for (other_name, _) in &sibling_inline_fields {
                    if *other_name != tc.name() && is_supertype_of_current(tc, other_name) {
                        let sibling_qualified = format!("{}.As{}", qualified_name, naming::first_uppercased(other_name));
                        if !init.fulfilled_fragments.contains(&sibling_qualified) {
                            init.fulfilled_fragments.push(sibling_qualified);
                        }
                    }
                }
            }

            let doc_comment = if is_root {
                format!("/// {}", type_name)
            } else {
                format!("/// {}.{}", struct_name, type_name)
            };
            nested_types.push(OwnedNestedSelectionSet {
                doc_comment,
                parent_type_comment: format!(
                    "///\n{}/// Parent Type: `{}`",
                    " ".repeat(indent + 2),
                    tc.name()
                ),
                config: child_ss,
            });
        }
    }

    // Promote inline fragments from spread fragments.
    let current_parent_type_name = ir_ss.scope.parent_type.name().to_string();
    let direct_inline_type_names: Vec<String> = ds
        .inline_fragments
        .iter()
        .filter_map(|inline| inline.type_condition.as_ref().map(|tc| tc.name().to_string()))
        .collect();

    // Track fragments that get promoted to inline fragments (type narrowing).
    // These should be removed from the parent scope's selections/fragments/fields.
    let mut promoted_fragment_names: Vec<String> = Vec::new();

    for spread in &ds.named_fragments {
        if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == spread.fragment_name) {
            let frag_type_condition = &frag_arc.type_condition_name;
            let frag_ds = &frag_arc.root_field.selection_set.direct_selections;

            // Case 1: Fragment type condition differs from parent type - create synthetic inline fragment
            // BUT only if the fragment's type is NOT a supertype of the current parent type.
            // E.g., WarmBlooded implements Animal, so spreading HeightInMeters (on Animal)
            // into WarmBloodedDetails does NOT need an AsAnimal wrapper.
            let needs_narrowing = *frag_type_condition != current_parent_type_name
                && !is_supertype_of_current(&ir_ss.scope.parent_type, frag_type_condition);
            if needs_narrowing
                && !direct_inline_type_names.contains(frag_type_condition)
                && !inline_fragment_accessors.iter().any(|a| a.type_name == format!("As{}", naming::first_uppercased(frag_type_condition)))
            {
                promoted_fragment_names.push(spread.fragment_name.clone());
                let type_name = format!("As{}", naming::first_uppercased(frag_type_condition));
                let child_qualified = format!("{}.{}", qualified_name, type_name);
                let child_root_entity = if is_root { qualified_name.to_string() } else { root_entity_type.unwrap_or(qualified_name).to_string() };

                selections.push(OwnedSelectionItem { kind: OwnedSelectionKind::InlineFragment(type_name.clone()) });
                inline_fragment_accessors.push(OwnedInlineFragmentAccessor {
                    property_name: format!("as{}", naming::first_uppercased(frag_type_condition)),
                    type_name: type_name.clone(),
                });

                let mut pfa = Vec::new();
                for fa in &field_accessors { pfa.push(fa.clone()); }
                for (key, field) in &frag_ds.fields {
                    if !pfa.iter().any(|f| f.name == *key) {
                        let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds);
                        pfa.push(OwnedFieldAccessor { name: key.clone(), swift_type });
                    }
                }
                for fs in &frag_ds.named_fragments {
                    if let Some(inner) = referenced_fragments.iter().find(|f| f.name == fs.fragment_name) {
                        for (key, field) in &inner.root_field.selection_set.direct_selections.fields {
                            if !pfa.iter().any(|f| f.name == *key) {
                                let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds);
                                pfa.push(OwnedFieldAccessor { name: key.clone(), swift_type });
                            }
                        }
                    }
                }

                let mut pfs = vec![OwnedFragmentSpreadAccessor {
                    property_name: naming::first_lowercased(&spread.fragment_name),
                    fragment_type: spread.fragment_name.clone(),
                }];
                for fs in &frag_ds.named_fragments {
                    pfs.push(OwnedFragmentSpreadAccessor {
                        property_name: naming::first_lowercased(&fs.fragment_name),
                        fragment_type: fs.fragment_name.clone(),
                    });
                }

                let pinit = if generate_initializers {
                    Some(build_promoted_initializer(
                        &frag_arc.root_field.selection_set.scope.parent_type,
                        &pfa, schema_namespace, &child_qualified, &child_root_entity,
                        &spread.fragment_name, &frag_ds.named_fragments, referenced_fragments,
                    ))
                } else { None };

                // Build nested types and type aliases for entity fields.
                // When the parent scope also has the same entity field, generate a merged
                // nested struct instead of a typealias. Otherwise, use a typealias.
                let mut pta = Vec::new();
                let mut pnt = Vec::new();
                // Collect entity fields from the fragment and its sub-fragments
                let mut entity_fields_from_frag: Vec<(String, String)> = Vec::new(); // (field_key, source_fragment_name)
                for (key, field) in &frag_ds.fields {
                    if matches!(field, FieldSelection::Entity(_)) {
                        entity_fields_from_frag.push((key.clone(), spread.fragment_name.clone()));
                    }
                }
                for fs in &frag_ds.named_fragments {
                    if let Some(inner) = referenced_fragments.iter().find(|f| f.name == fs.fragment_name) {
                        for (key, field) in &inner.root_field.selection_set.direct_selections.fields {
                            if matches!(field, FieldSelection::Entity(_)) {
                                if !entity_fields_from_frag.iter().any(|(k, _)| k == key) {
                                    entity_fields_from_frag.push((key.clone(), fs.fragment_name.clone()));
                                }
                            }
                        }
                    }
                }
                for (key, source_frag) in &entity_fields_from_frag {
                    let n = naming::first_uppercased(key);
                    let singularized = naming::singularize(key);
                    let child_struct_name = naming::first_uppercased(&singularized);
                    // Check if the parent scope has the same entity field
                    let parent_has_field = ds.fields.get(key).map(|f| matches!(f, FieldSelection::Entity(_))).unwrap_or(false);
                    if parent_has_field && generate_initializers {
                        // Generate a merged nested struct
                        if let Some(FieldSelection::Entity(parent_ef)) = ds.fields.get(key) {
                            let entity_qualified = format!("{}.{}", child_qualified, child_struct_name);
                            let merged_struct = build_merged_entity_nested_type(
                                &child_struct_name,
                                parent_ef,
                                source_frag,
                                key,
                                schema_namespace,
                                access_modifier,
                                indent + 4, // inside promoted inline fragment
                                &entity_qualified,
                                qualified_name,
                                referenced_fragments,
                                type_kinds,
                                is_mutable,
                            );
                            pnt.push(merged_struct);
                        }
                    } else {
                        pta.push(OwnedTypeAlias { name: n.clone(), target: format!("{}.{}", source_frag, n) });
                    }
                }

                let frag_parent_type = match &frag_arc.root_field.selection_set.scope.parent_type {
                    GraphQLCompositeType::Object(o) => OwnedParentTypeRef::Object(o.name.clone()),
                    GraphQLCompositeType::Interface(i) => OwnedParentTypeRef::Interface(i.name.clone()),
                    GraphQLCompositeType::Union(u) => OwnedParentTypeRef::Union(u.name.clone()),
                };
                let ic = if is_mutable { SelectionSetConformance::MutableInlineFragment } else { SelectionSetConformance::InlineFragment };
                let pss = OwnedSelectionSetConfig {
                    struct_name: type_name.clone(), schema_namespace: schema_namespace.to_string(),
                    parent_type: frag_parent_type, is_root: false, is_inline_fragment: true,
                    conformance: ic, root_entity_type: Some(child_root_entity.clone()),
                    merged_sources: vec![], selections: vec![OwnedSelectionItem { kind: OwnedSelectionKind::Fragment(spread.fragment_name.clone()) }],
                    field_accessors: pfa, inline_fragment_accessors: vec![],
                    fragment_spreads: pfs, initializer: pinit,
                    nested_types: pnt, type_aliases: pta,
                    indent: indent + 2, access_modifier: access_modifier.to_string(), is_mutable,
                };
                let dc = if is_root { format!("/// {}", type_name) } else { format!("/// {}.{}", struct_name, type_name) };
                nested_types.push(OwnedNestedSelectionSet {
                    doc_comment: dc,
                    parent_type_comment: format!("///\n{}/// Parent Type: `{}`", " ".repeat(indent + 2), frag_type_condition),
                    config: pss,
                });
            }

            // Case 2: Fragment contains inline fragments - promote them as CompositeInlineFragment
            for frag_inline in &frag_ds.inline_fragments {
                if let Some(ref tc) = frag_inline.type_condition {
                    let tc_name = tc.name().to_string();
                    if direct_inline_type_names.contains(&tc_name) { continue; }
                    if inline_fragment_accessors.iter().any(|a| a.type_name == format!("As{}", naming::first_uppercased(&tc_name))) { continue; }

                    let type_name = format!("As{}", naming::first_uppercased(&tc_name));
                    let child_qualified = format!("{}.{}", qualified_name, type_name);
                    let child_root_entity = if is_root { qualified_name.to_string() } else { root_entity_type.unwrap_or(qualified_name).to_string() };

                    inline_fragment_accessors.push(OwnedInlineFragmentAccessor {
                        property_name: format!("as{}", naming::first_uppercased(&tc_name)),
                        type_name: type_name.clone(),
                    });

                    let ppt = match tc {
                        GraphQLCompositeType::Object(o) => OwnedParentTypeRef::Object(o.name.clone()),
                        GraphQLCompositeType::Interface(i) => OwnedParentTypeRef::Interface(i.name.clone()),
                        GraphQLCompositeType::Union(u) => OwnedParentTypeRef::Union(u.name.clone()),
                    };

                    // Build merged_sources: include self, the fragment's own inline fragment,
                    // and sibling supertype inline fragments from the same fragment.
                    let mut ms = vec![qualified_name.to_string()];
                    // Add sibling supertype inline fragments from the fragment
                    for other_frag_inline in &frag_ds.inline_fragments {
                        if let Some(ref other_tc) = other_frag_inline.type_condition {
                            let other_name = other_tc.name();
                            if other_name != tc_name && is_supertype_of_current(tc, other_name) {
                                ms.push(format!("{}.As{}", spread.fragment_name, naming::first_uppercased(other_name)));
                            }
                        }
                    }
                    // Add the fragment's own inline fragment type last
                    ms.push(format!("{}.{}", spread.fragment_name, type_name));

                    // Build field accessors following merged_sources order:
                    // 1. Fields from sibling supertype inline fragments (in merged_sources order)
                    // 2. Own direct fields
                    // 3. Parent inherited fields
                    let mut pfa = Vec::new();
                    // First: fields from sibling supertype inline fragments within the fragment
                    for other_frag_inline in &frag_ds.inline_fragments {
                        if let Some(ref other_tc) = other_frag_inline.type_condition {
                            let other_name = other_tc.name();
                            if other_name != tc_name && is_supertype_of_current(tc, other_name) {
                                for (key, field) in &other_frag_inline.selection_set.direct_selections.fields {
                                    if !pfa.iter().any(|f: &OwnedFieldAccessor| f.name == *key) {
                                        let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds);
                                        pfa.push(OwnedFieldAccessor { name: key.clone(), swift_type });
                                    }
                                }
                            }
                        }
                    }
                    // Then: own direct fields from this inline fragment
                    for (key, field) in &frag_inline.selection_set.direct_selections.fields {
                        if !pfa.iter().any(|f| f.name == *key) {
                            let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds);
                            pfa.push(OwnedFieldAccessor { name: key.clone(), swift_type });
                        }
                    }
                    // Finally: parent inherited fields
                    for fa in &field_accessors {
                        if !pfa.iter().any(|f| f.name == fa.name) { pfa.push(fa.clone()); }
                    }

                    let pfs = vec![OwnedFragmentSpreadAccessor {
                        property_name: naming::first_lowercased(&spread.fragment_name),
                        fragment_type: spread.fragment_name.clone(),
                    }];

                    // Build fulfilled fragments for the initializer, including supertype inline fragment OIDs
                    let mut extra_frag_fulfilled = Vec::new();
                    for other_frag_inline in &frag_ds.inline_fragments {
                        if let Some(ref other_tc) = other_frag_inline.type_condition {
                            let other_name = other_tc.name();
                            if other_name != tc_name && is_supertype_of_current(tc, other_name) {
                                extra_frag_fulfilled.push(format!("{}.As{}", spread.fragment_name, naming::first_uppercased(other_name)));
                            }
                        }
                    }
                    // Add the fragment's own inline fragment type
                    extra_frag_fulfilled.push(format!("{}.{}", spread.fragment_name, type_name));

                    let pinit = if generate_initializers {
                        Some(build_promoted_composite_initializer(
                            tc, &pfa, schema_namespace, &child_qualified, &child_root_entity,
                            &spread.fragment_name, referenced_fragments,
                            &extra_frag_fulfilled,
                        ))
                    } else { None };

                    let pc = if is_mutable { SelectionSetConformance::MutableInlineFragment } else { SelectionSetConformance::CompositeInlineFragment };
                    let pss = OwnedSelectionSetConfig {
                        struct_name: type_name.clone(), schema_namespace: schema_namespace.to_string(),
                        parent_type: ppt, is_root: false, is_inline_fragment: true,
                        conformance: pc, root_entity_type: Some(child_root_entity),
                        merged_sources: ms, selections: vec![],
                        field_accessors: pfa, inline_fragment_accessors: vec![],
                        fragment_spreads: pfs, initializer: pinit,
                        nested_types: vec![], type_aliases: vec![],
                        indent: indent + 2, access_modifier: access_modifier.to_string(), is_mutable,
                    };
                    let dc = if is_root { format!("/// {}", type_name) } else { format!("/// {}.{}", struct_name, type_name) };
                    nested_types.push(OwnedNestedSelectionSet {
                        doc_comment: dc,
                        parent_type_comment: format!("///\n{}/// Parent Type: `{}`", " ".repeat(indent + 2), tc.name()),
                        config: pss,
                    });
                }
            }
        }
    }

    // Remove promoted fragments from the parent scope's selections, field_accessors,
    // and fragment_spreads. When a fragment is promoted to an inline fragment (type narrowing),
    // it should not appear at the parent scope.
    if !promoted_fragment_names.is_empty() {
        // Remove .fragment(FragName.self) from selections
        selections.retain(|s| {
            if let OwnedSelectionKind::Fragment(name) = &s.kind {
                !promoted_fragment_names.contains(name)
            } else {
                true
            }
        });
        // Remove from fragment_spreads
        fragment_spreads.retain(|fs| !promoted_fragment_names.contains(&fs.fragment_type));
        // Remove fields that came exclusively from promoted fragments
        // (only remove if the field doesn't exist in direct selections or non-promoted fragments)
        let mut fields_from_nonpromoted: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (key, _) in &ds.fields {
            fields_from_nonpromoted.insert(key.clone());
        }
        for spread in &ds.named_fragments {
            if !promoted_fragment_names.contains(&spread.fragment_name) {
                if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == spread.fragment_name) {
                    for (key, _) in &frag_arc.root_field.selection_set.direct_selections.fields {
                        fields_from_nonpromoted.insert(key.clone());
                    }
                    // Also check sub-fragment fields
                    for sub_spread in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                        if let Some(sub_frag) = referenced_fragments.iter().find(|f| f.name == sub_spread.fragment_name) {
                            for (key, _) in &sub_frag.root_field.selection_set.direct_selections.fields {
                                fields_from_nonpromoted.insert(key.clone());
                            }
                        }
                    }
                }
            }
        }
        if is_inline_fragment {
            // For inline fragments, also consider fields from parent_fields
            if let Some(parent) = parent_fields {
                for pf in parent {
                    fields_from_nonpromoted.insert(pf.name.clone());
                }
            }
        }
        field_accessors.retain(|fa| fields_from_nonpromoted.contains(&fa.name));
    }

    // Build initializer when requested
    let initializer = if generate_initializers {
        let extra_fulfilled: Vec<String> = vec![];

        Some(build_initializer_config(
            &ir_ss.scope.parent_type,
            ds,
            schema_namespace,
            qualified_name,
            is_inline_fragment,
            root_entity_type,
            referenced_fragments,
            type_kinds,
            &field_accessors,
            &extra_fulfilled,
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
        type_aliases: build_type_aliases(ds, referenced_fragments),
        indent,
        access_modifier: access_modifier.to_string(),
        is_mutable,
    }
}

/// Build type aliases for entity types from spread fragments.
/// E.g., `typealias Height = HeightInMeters.Height` when the current selection set
/// spreads HeightInMeters which has a nested Height entity type.
/// Check if `type_name` is a supertype of `current_type`.
/// This is true when the current type implements the interface `type_name`.
fn is_supertype_of_current(current_type: &GraphQLCompositeType, type_name: &str) -> bool {
    match current_type {
        GraphQLCompositeType::Object(obj) => {
            obj.interfaces.iter().any(|i| i == type_name)
        }
        GraphQLCompositeType::Interface(iface) => {
            iface.interfaces.iter().any(|i| i == type_name)
        }
        GraphQLCompositeType::Union(_) => false,
    }
}

/// Build a merged nested entity type struct for a promoted inline fragment.
///
/// When a promoted inline fragment (Case 1) inherits an entity field from the parent scope
/// AND the fragment adds additional fields to that entity, we create a merged struct
/// that includes fields from both scopes instead of a simple typealias.
///
/// For example, AsWarmBlooded in AllAnimalsQuery gets a Height struct that merges:
/// - feet, inches (from the parent AllAnimal's Height)
/// - meters (from HeightInMeters.Height, via WarmBloodedDetails)
fn build_merged_entity_nested_type(
    struct_name: &str,
    parent_entity_field: &EntityField,
    source_frag_name: &str,
    field_key: &str,
    schema_namespace: &str,
    access_modifier: &str,
    indent: usize,
    qualified_name: &str,
    parent_qualified_name: &str,
    referenced_fragments: &[Arc<NamedFragment>],
    type_kinds: &HashMap<String, TypeKind>,
    is_mutable: bool,
) -> OwnedNestedSelectionSet {
    let parent_type_name = parent_entity_field.selection_set.scope.parent_type.name();
    let entity_parent_type = match &parent_entity_field.selection_set.scope.parent_type {
        GraphQLCompositeType::Object(o) => OwnedParentTypeRef::Object(o.name.clone()),
        GraphQLCompositeType::Interface(i) => OwnedParentTypeRef::Interface(i.name.clone()),
        GraphQLCompositeType::Union(u) => OwnedParentTypeRef::Union(u.name.clone()),
    };

    // Collect field accessors from the parent entity field
    let mut merged_fields: Vec<OwnedFieldAccessor> = Vec::new();
    for (key, field) in &parent_entity_field.selection_set.direct_selections.fields {
        if key == "__typename" { continue; }
        let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds);
        merged_fields.push(OwnedFieldAccessor { name: key.clone(), swift_type });
    }

    // Collect fields from the source fragment's entity field
    let singularized = naming::singularize(field_key);
    let frag_struct_name = naming::first_uppercased(&singularized);
    if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == source_frag_name) {
        // Check the fragment's own entity fields
        if let Some(FieldSelection::Entity(frag_ef)) = frag_arc.root_field.selection_set.direct_selections.fields.get(field_key) {
            for (key, field) in &frag_ef.selection_set.direct_selections.fields {
                if key == "__typename" { continue; }
                if !merged_fields.iter().any(|f| f.name == *key) {
                    let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds);
                    merged_fields.push(OwnedFieldAccessor { name: key.clone(), swift_type });
                }
            }
        }
        // Also check sub-fragment entity fields
        for fs in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
            if let Some(inner) = referenced_fragments.iter().find(|f| f.name == fs.fragment_name) {
                if let Some(FieldSelection::Entity(inner_ef)) = inner.root_field.selection_set.direct_selections.fields.get(field_key) {
                    for (key, field) in &inner_ef.selection_set.direct_selections.fields {
                        if key == "__typename" { continue; }
                        if !merged_fields.iter().any(|f| f.name == *key) {
                            let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds);
                            merged_fields.push(OwnedFieldAccessor { name: key.clone(), swift_type });
                        }
                    }
                }
            }
        }
    }

    // Build fulfilled fragments for the initializer
    let parent_height_qualified = format!("{}.{}", parent_qualified_name, struct_name);
    let mut fulfilled = vec![
        qualified_name.to_string(),
        parent_height_qualified.clone(),
    ];
    // Add fragment's entity type OID
    if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == source_frag_name) {
        // Check sub-fragments for entity fields too
        for fs in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
            if let Some(inner) = referenced_fragments.iter().find(|f| f.name == fs.fragment_name) {
                if inner.root_field.selection_set.direct_selections.fields.contains_key(field_key) {
                    let frag_entity_qualified = format!("{}.{}", fs.fragment_name, frag_struct_name);
                    if !fulfilled.contains(&frag_entity_qualified) {
                        fulfilled.push(frag_entity_qualified);
                    }
                }
            }
        }
        if frag_arc.root_field.selection_set.direct_selections.fields.contains_key(field_key) {
            let frag_entity_qualified = format!("{}.{}", source_frag_name, frag_struct_name);
            if !fulfilled.contains(&frag_entity_qualified) {
                fulfilled.push(frag_entity_qualified);
            }
        }
    }

    let is_parent_object = matches!(entity_parent_type, OwnedParentTypeRef::Object(_));
    let typename_value = if is_parent_object {
        OwnedTypenameValue::Fixed(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(parent_type_name)))
    } else {
        OwnedTypenameValue::Parameter
    };

    let mut init_params = Vec::new();
    if !is_parent_object {
        init_params.push(OwnedInitParam { name: "__typename".to_string(), swift_type: "String".to_string(), default_value: None });
    }
    for fa in &merged_fields {
        init_params.push(OwnedInitParam {
            name: fa.name.clone(), swift_type: fa.swift_type.clone(),
            default_value: if fa.swift_type.ends_with('?') { Some("nil".to_string()) } else { None },
        });
    }

    let mut data_entries = vec![OwnedDataEntry {
        key: "__typename".to_string(),
        value: if is_parent_object {
            OwnedDataEntryValue::Typename(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(parent_type_name)))
        } else {
            OwnedDataEntryValue::Variable("__typename".to_string())
        },
    }];
    for fa in &merged_fields {
        data_entries.push(OwnedDataEntry {
            key: fa.name.clone(),
            value: OwnedDataEntryValue::Variable(fa.name.clone()),
        });
    }

    let initializer = OwnedInitializerConfig {
        parameters: init_params,
        data_entries,
        fulfilled_fragments: fulfilled,
        typename_value,
    };

    let conformance = if is_mutable {
        SelectionSetConformance::MutableSelectionSet
    } else {
        SelectionSetConformance::SelectionSet
    };

    let config = OwnedSelectionSetConfig {
        struct_name: struct_name.to_string(),
        schema_namespace: schema_namespace.to_string(),
        parent_type: entity_parent_type,
        is_root: false,
        is_inline_fragment: false,
        conformance,
        root_entity_type: None,
        merged_sources: vec![],
        selections: vec![],  // No __selections for merged view
        field_accessors: merged_fields,
        inline_fragment_accessors: vec![],
        fragment_spreads: vec![],
        initializer: Some(initializer),
        nested_types: vec![],
        type_aliases: vec![],
        indent,
        access_modifier: access_modifier.to_string(),
        is_mutable,
    };

    OwnedNestedSelectionSet {
        doc_comment: format!("/// {}", struct_name),
        parent_type_comment: format!("///\n{}/// Parent Type: `{}`", " ".repeat(indent), parent_type_name),
        config,
    }
}

fn build_type_aliases(
    ds: &DirectSelections,
    referenced_fragments: &[Arc<NamedFragment>],
) -> Vec<OwnedTypeAlias> {
    let mut aliases = Vec::new();

    for spread in &ds.named_fragments {
        if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == spread.fragment_name) {
            for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                if let FieldSelection::Entity(_) = field {
                    let type_name = naming::first_uppercased(key);
                    // Only add alias if we don't have a direct entity field with the same name
                    if !ds.fields.contains_key(key) || !matches!(ds.fields.get(key), Some(FieldSelection::Entity(_))) {
                        aliases.push(OwnedTypeAlias {
                            name: type_name.clone(),
                            target: format!("{}.{}", spread.fragment_name, type_name),
                        });
                    }
                }
            }
        }
    }

    aliases
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
    extra_fulfilled: &[String],
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
        // Check if this is an entity or scalar field - check direct fields first,
        // then check fragment spread fields for merged fields
        let mut is_entity = ds.fields.get(&accessor.name)
            .map(|f| matches!(f, FieldSelection::Entity(_)))
            .unwrap_or(false);
        if !is_entity {
            // Check fragment spreads for entity fields
            for spread in &ds.named_fragments {
                if let Some(frag) = referenced_fragments.iter().find(|f| f.name == spread.fragment_name) {
                    if let Some(field) = frag.root_field.selection_set.direct_selections.fields.get(&accessor.name) {
                        if matches!(field, FieldSelection::Entity(_)) {
                            is_entity = true;
                            break;
                        }
                    }
                }
            }
        }
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

    // Add directly spread named fragments to fulfilled fragments,
    // but only when the fragment's type condition matches the current scope.
    // For unions, the fragment is fulfilled per-type-case, not at the union level.
    let parent_is_union = matches!(parent_type, GraphQLCompositeType::Union(_));
    if !parent_is_union {
        for spread in &ds.named_fragments {
            fulfilled_fragments.push(spread.fragment_name.clone());
        }
    }

    // Add extra fulfilled fragments from sibling type merging
    for extra in extra_fulfilled {
        if !fulfilled_fragments.contains(extra) {
            fulfilled_fragments.push(extra.clone());
        }
    }

    OwnedInitializerConfig {
        parameters,
        data_entries,
        fulfilled_fragments,
        typename_value,
    }
}

/// Build an initializer for a promoted type-narrowing inline fragment (Case 1).
fn build_promoted_initializer(
    parent_type: &GraphQLCompositeType, all_field_accessors: &[OwnedFieldAccessor],
    schema_namespace: &str, qualified_name: &str, root_entity_type: &str,
    fragment_name: &str, frag_named_fragments: &[NamedFragmentSpread],
    referenced_fragments: &[Arc<NamedFragment>],
) -> OwnedInitializerConfig {
    let parent_is_object = matches!(parent_type, GraphQLCompositeType::Object(_));
    let typename_value = if parent_is_object {
        OwnedTypenameValue::Fixed(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(parent_type.name())))
    } else { OwnedTypenameValue::Parameter };
    let mut parameters = Vec::new();
    if !parent_is_object { parameters.push(OwnedInitParam { name: "__typename".to_string(), swift_type: "String".to_string(), default_value: None }); }
    for a in all_field_accessors {
        parameters.push(OwnedInitParam { name: a.name.clone(), swift_type: a.swift_type.clone(), default_value: if a.swift_type.ends_with('?') { Some("nil".to_string()) } else { None } });
    }
    let mut data_entries = vec![OwnedDataEntry {
        key: "__typename".to_string(),
        value: if parent_is_object { OwnedDataEntryValue::Typename(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(parent_type.name()))) } else { OwnedDataEntryValue::Variable("__typename".to_string()) },
    }];
    for a in all_field_accessors {
        let is_entity = referenced_fragments.iter().any(|f| f.root_field.selection_set.direct_selections.fields.get(&a.name).map(|field| matches!(field, FieldSelection::Entity(_))).unwrap_or(false));
        data_entries.push(OwnedDataEntry { key: a.name.clone(), value: if is_entity { OwnedDataEntryValue::FieldData(a.name.clone()) } else { OwnedDataEntryValue::Variable(a.name.clone()) } });
    }
    let mut fulfilled_fragments = vec![root_entity_type.to_string(), qualified_name.to_string(), fragment_name.to_string()];
    for fs in frag_named_fragments { fulfilled_fragments.push(fs.fragment_name.clone()); }
    OwnedInitializerConfig { parameters, data_entries, fulfilled_fragments, typename_value }
}

/// Build an initializer for a promoted composite inline fragment (Case 2).
fn build_promoted_composite_initializer(
    parent_type: &GraphQLCompositeType, all_field_accessors: &[OwnedFieldAccessor],
    schema_namespace: &str, qualified_name: &str, root_entity_type: &str,
    fragment_name: &str, referenced_fragments: &[Arc<NamedFragment>],
    extra_fulfilled: &[String],
) -> OwnedInitializerConfig {
    let parent_is_object = matches!(parent_type, GraphQLCompositeType::Object(_));
    let typename_value = if parent_is_object {
        OwnedTypenameValue::Fixed(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(parent_type.name())))
    } else { OwnedTypenameValue::Parameter };
    let mut parameters = Vec::new();
    if !parent_is_object { parameters.push(OwnedInitParam { name: "__typename".to_string(), swift_type: "String".to_string(), default_value: None }); }
    for a in all_field_accessors {
        parameters.push(OwnedInitParam { name: a.name.clone(), swift_type: a.swift_type.clone(), default_value: if a.swift_type.ends_with('?') { Some("nil".to_string()) } else { None } });
    }
    let mut data_entries = vec![OwnedDataEntry {
        key: "__typename".to_string(),
        value: if parent_is_object { OwnedDataEntryValue::Typename(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(parent_type.name()))) } else { OwnedDataEntryValue::Variable("__typename".to_string()) },
    }];
    for a in all_field_accessors {
        let is_entity = referenced_fragments.iter().any(|f| f.root_field.selection_set.direct_selections.fields.get(&a.name).map(|field| matches!(field, FieldSelection::Entity(_))).unwrap_or(false));
        data_entries.push(OwnedDataEntry { key: a.name.clone(), value: if is_entity { OwnedDataEntryValue::FieldData(a.name.clone()) } else { OwnedDataEntryValue::Variable(a.name.clone()) } });
    }
    let mut fulfilled_fragments = vec![root_entity_type.to_string(), qualified_name.to_string(), fragment_name.to_string()];
    // Add extra fulfilled fragments (sibling supertype OIDs from the fragment)
    for extra in extra_fulfilled {
        if !fulfilled_fragments.contains(extra) {
            fulfilled_fragments.push(extra.clone());
        }
    }
    OwnedInitializerConfig { parameters, data_entries, fulfilled_fragments, typename_value }
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
        is_local_cache_mutation: config.is_local_cache_mutation,
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
        is_mutable: config.is_mutable,
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
        is_mutable: owned.is_mutable,
    }
}
