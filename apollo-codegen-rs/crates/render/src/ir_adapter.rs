//! Adapter that converts IR types into template configuration structs.
//!
//! This bridges the IR module's structured types to the string-based
//! configuration that the templates consume.

use crate::naming;
use crate::schema_customization::SchemaCustomizer;
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
use apollo_codegen_ir::inclusion::{self, InclusionConditions};
use apollo_codegen_ir::selection_set::{
    DirectSelections, FieldSelection, InlineFragmentSelection, NamedFragmentSpread,
    SelectionKind as IrSelectionKind, SelectionSet as IrSelectionSet,
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
    customizer: &SchemaCustomizer,
    include_definition: bool,
    operation_identifier: Option<&str>,
    query_string_format: crate::templates::operation::QueryStringFormat,
    api_target_name: &str,
    mark_definitions_as_final: bool,
    variable_namespace_prefix: &str,
    init_access_modifier: Option<&str>,
) -> String {
    // Build owned strings we'll reference
    let op_type = match op.operation_type {
        OperationType::Query => TemplateOpType::Query,
        OperationType::Mutation => TemplateOpType::Mutation,
        OperationType::Subscription => TemplateOpType::Subscription,
    };

    let mut fragment_names: Vec<String> = op
        .referenced_fragments
        .iter()
        .map(|f| naming::first_uppercased(&f.name))
        .collect();
    fragment_names.sort();
    let fragment_name_refs: Vec<&str> = fragment_names.iter().map(|s| s.as_str()).collect();

    let variables: Vec<OwnedVariableConfig> = op
        .variables
        .iter()
        .map(|v| {
            let mut swift_type = customizer.customize_variable_type(&v.type_str);
            let mut default_value = v.default_value.as_ref().map(|dv| customizer.customize_variable_type(dv));
            if !variable_namespace_prefix.is_empty() {
                swift_type = add_namespace_to_variable_type(&swift_type, variable_namespace_prefix, type_kinds, customizer);
                default_value = default_value.map(|dv| add_namespace_to_variable_type(&dv, variable_namespace_prefix, type_kinds, customizer));
            }
            OwnedVariableConfig {
                name: v.name.clone(),
                swift_type,
                default_value,
            }
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
        customizer,
        None,  // entity_root_graphql_type (set per-entity below)
        &[],   // no ancestor fragments for root
        None,  // no parent scope DS for root
        None,  // no scope conditions for root
        api_target_name,
    );

    let class_keyword = if mark_definitions_as_final {
        "final class".to_string()
    } else {
        "class".to_string()
    };

    let config = OwnedOperationConfig {
        class_name: class_name.clone(),
        operation_name: op.name.clone(),
        operation_type: op_type,
        schema_namespace: schema_namespace.to_string(),
        access_modifier: access_modifier.to_string(),
        init_access_modifier: init_access_modifier.unwrap_or(access_modifier).to_string(),
        source: op.source.clone(),
        fragment_names,
        variables,
        data_selection_set: data_ss,
        is_local_cache_mutation: op.is_local_cache_mutation,
        include_definition,
        operation_identifier: operation_identifier.map(|s| s.to_string()),
        query_string_format,
        api_target_name: api_target_name.to_string(),
        class_keyword,
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
    customizer: &SchemaCustomizer,
    query_string_format: crate::templates::operation::QueryStringFormat,
    api_target_name: &str,
    include_definition: bool,
) -> String {
    let frag_conformance = if frag.is_local_cache_mutation {
        SelectionSetConformance::MutableFragment
    } else {
        SelectionSetConformance::Fragment
    };
    let frag_uc_name = naming::first_uppercased(&frag.name);
    let ss = build_selection_set_config_owned(
        &frag_uc_name,
        &frag.root_field.selection_set,
        schema_namespace,
        access_modifier,
        true,
        false,
        frag_conformance,
        None,
        0, // top-level
        &frag_uc_name,
        generate_initializers,
        &frag.referenced_fragments,
        type_kinds,
        None, // no parent fields for fragment root
        frag.is_local_cache_mutation,
        customizer,
        None, // entity_root_graphql_type
        &[],  // no ancestor fragments for fragment root
        None, // no parent scope DS for fragment root
        None, // no scope conditions for fragment root
        api_target_name,
    );

    let config = OwnedFragmentConfig {
        name: naming::first_uppercased(&frag.name),
        fragment_definition: frag.source.clone(),
        schema_namespace: schema_namespace.to_string(),
        access_modifier: access_modifier.to_string(),
        selection_set: ss,
        is_mutable: frag.is_local_cache_mutation,
        query_string_format,
        api_target_name: api_target_name.to_string(),
        include_definition,
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
    /// Access modifier for init, variable properties, and __variables.
    /// For embeddedInTarget, this is always "public " to satisfy protocol conformance.
    init_access_modifier: String,
    source: String,
    fragment_names: Vec<String>,
    variables: Vec<OwnedVariableConfig>,
    data_selection_set: OwnedSelectionSetConfig,
    is_local_cache_mutation: bool,
    include_definition: bool,
    operation_identifier: Option<String>,
    query_string_format: crate::templates::operation::QueryStringFormat,
    api_target_name: String,
    class_keyword: String,
}

struct OwnedFragmentConfig {
    name: String,
    fragment_definition: String,
    schema_namespace: String,
    access_modifier: String,
    selection_set: OwnedSelectionSetConfig,
    is_mutable: bool,
    query_string_format: crate::templates::operation::QueryStringFormat,
    api_target_name: String,
    include_definition: bool,
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
    /// Index into nested_types where type aliases should be rendered.
    /// Type aliases are rendered after nested_types[0..index] and before nested_types[index..].
    type_alias_insert_index: usize,
    indent: usize,
    access_modifier: String,
    is_mutable: bool,
    /// Type names that were absorbed (e.g., "AsAnimal" absorbed into AsPet)
    absorbed_type_names: Vec<String>,
    /// The API target name for fully-qualified type references.
    api_target_name: String,
}

#[derive(Clone)]
enum OwnedParentTypeRef {
    Object(String),
    Interface(String),
    Union(String),
}

struct OwnedSelectionItem {
    kind: OwnedSelectionKind,
}

/// A single owned condition entry for compound inclusion conditions.
#[derive(Clone, Debug)]
struct OwnedConditionEntry {
    variable: String,
    is_inverted: bool,
}

/// How multiple owned conditions are combined.
#[derive(Clone, Debug, Copy)]
enum OwnedConditionOperator {
    And,
    Or,
}

enum OwnedSelectionKind {
    Field { name: String, alias: Option<String>, swift_type: String, arguments: Option<String> },
    InlineFragment(String),
    Fragment(String),
    ConditionalField { conditions: Vec<OwnedConditionEntry>, operator: OwnedConditionOperator, name: String, alias: Option<String>, swift_type: String, arguments: Option<String> },
    ConditionalInlineFragment { conditions: Vec<OwnedConditionEntry>, operator: OwnedConditionOperator, type_name: String },
    ConditionalFieldGroup { conditions: Vec<OwnedConditionEntry>, operator: OwnedConditionOperator, fields: Vec<(String, Option<String>, String, Option<String>)> }, // (name, alias, swift_type, arguments)
}

#[derive(Clone)]
struct OwnedFieldAccessor {
    name: String,
    swift_type: String,
    description: Option<String>,
}

/// A deduplicating Vec of field accessors with O(1) membership checks.
/// Replaces the O(n) `.iter().any(|f| f.name == key)` pattern.
#[derive(Clone)]
struct FieldAccessorSet {
    entries: Vec<OwnedFieldAccessor>,
    seen: std::collections::HashSet<String>,
}

impl FieldAccessorSet {
    fn new() -> Self {
        Self { entries: Vec::new(), seen: std::collections::HashSet::new() }
    }

    fn from_vec(v: Vec<OwnedFieldAccessor>) -> Self {
        let seen = v.iter().map(|f| f.name.clone()).collect();
        Self { entries: v, seen }
    }

    fn contains(&self, name: &str) -> bool {
        self.seen.contains(name)
    }

    fn push(&mut self, accessor: OwnedFieldAccessor) {
        if self.seen.insert(accessor.name.clone()) {
            self.entries.push(accessor);
        }
    }

    /// Push without dedup check (caller guarantees uniqueness or wants duplicates).
    fn push_unchecked(&mut self, accessor: OwnedFieldAccessor) {
        self.seen.insert(accessor.name.clone());
        self.entries.push(accessor);
    }

    fn insert(&mut self, index: usize, accessor: OwnedFieldAccessor) {
        self.seen.insert(accessor.name.clone());
        self.entries.insert(index, accessor);
    }

    fn iter(&self) -> std::slice::Iter<'_, OwnedFieldAccessor> {
        self.entries.iter()
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn into_vec(self) -> Vec<OwnedFieldAccessor> {
        self.entries
    }
}

struct OwnedInlineFragmentAccessor {
    property_name: String,
    type_name: String,
}

struct OwnedFragmentSpreadAccessor {
    property_name: String,
    fragment_type: String,
    is_optional: bool,
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
    customizer: &SchemaCustomizer,
    entity_root_graphql_type: Option<&str>,
    ancestor_fragments: &[String],
    parent_scope_ds: Option<&DirectSelections>,
    scope_conditions: Option<&InclusionConditions>,
    api_target_name: &str,
) -> OwnedSelectionSetConfig {
    let parent_type = match &ir_ss.scope.parent_type {
        GraphQLCompositeType::Object(o) => OwnedParentTypeRef::Object(customizer.custom_type_name(&o.name).to_string()),
        GraphQLCompositeType::Interface(i) => OwnedParentTypeRef::Interface(customizer.custom_type_name(&i.name).to_string()),
        GraphQLCompositeType::Union(u) => OwnedParentTypeRef::Union(customizer.custom_type_name(&u.name).to_string()),
    };

    let ds = &ir_ss.direct_selections;

    // Build a lookup map for O(1) fragment resolution (avoids 74 linear scans)
    let frag_map: HashMap<&str, &Arc<NamedFragment>> = referenced_fragments
        .iter()
        .map(|f| (f.name.as_str(), f))
        .collect();

    // Absorb inline fragments whose type condition is always satisfied by the entity root type.
    // For example, `... on Animal` inside `... on Pet` when the entity root is `Animal` -
    // since the entity IS Animal, the `... on Animal` condition is always true and its
    // selections should be absorbed into the enclosing scope (field selections become direct,
    // entity field selections become nested types).
    let mut absorbed_inline_indices: Vec<usize> = Vec::new();
    let mut absorbed_type_names: Vec<String> = Vec::new();
    if let Some(entity_root_type) = entity_root_graphql_type {
        for (idx, inline) in ds.inline_fragments.iter().enumerate() {
            if let Some(ref tc) = inline.type_condition {
                let tc_name = tc.name();
                let should_absorb = tc_name == entity_root_type
                    || is_supertype_of_current(&ir_ss.scope.parent_type, tc_name);
                if should_absorb {
                    absorbed_inline_indices.push(idx);
                    absorbed_type_names.push(format!("As{}", naming::first_uppercased(customizer.custom_type_name(tc_name))));
                }
            }
        }
    }

    // Pre-compute which fragments will be promoted to inline fragments (type narrowing).
    // We need this before building selections so promoted inline fragments can be inserted
    // in the correct position (after fields, before direct inline fragments).
    let current_parent_type_name_early = ir_ss.scope.parent_type.name().to_string();
    let direct_inline_type_names_early: Vec<String> = ds
        .inline_fragments
        .iter()
        .enumerate()
        .filter(|(idx, _)| !absorbed_inline_indices.contains(idx))
        .filter_map(|(_, inline)| inline.type_condition.as_ref().map(|tc| tc.name().to_string()))
        .collect();
    let mut early_promoted_fragment_names: Vec<String> = Vec::new();
    let mut early_promoted_types: Vec<String> = Vec::new();
    {
        let mut seen_promoted_types: Vec<String> = Vec::new();
        for spread in &ds.named_fragments {
            // Skip conditional fragment spreads - they get their own conditional inline fragment
            if has_inclusion_conditions(spread.inclusion_conditions.as_ref()) { continue; }
            if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
                let ftc = &frag_arc.type_condition_name;
                let needs_narrowing = *ftc != current_parent_type_name_early
                    && !is_supertype_of_current(&ir_ss.scope.parent_type, ftc);
                if needs_narrowing
                    && !direct_inline_type_names_early.contains(ftc)
                    && !seen_promoted_types.contains(ftc)
                {
                    early_promoted_fragment_names.push(spread.fragment_name.clone());
                    early_promoted_types.push(ftc.clone());
                    seen_promoted_types.push(ftc.clone());
                }
            }
        }
    }

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
                alias: None,
                swift_type: "String".to_string(),
                arguments: None,
            },
        });
    }
    // Track conditional fields to add after unconditional selections.
    // Key: conditions + operator, Value: vec of (name, alias, swift_type, arguments)
    let mut conditional_field_groups: Vec<(Vec<OwnedConditionEntry>, OwnedConditionOperator, Vec<(String, Option<String>, String, Option<String>)>)> = Vec::new();
    // Track fields whose conditions are satisfied by the enclosing scope.
    // These appear after fragment spreads in __selections to preserve source ordering.
    let mut scope_satisfied_fields: Vec<OwnedSelectionItem> = Vec::new();
    // Helper closure to process a direct field selection into selections/conditional/scope-satisfied.
    let mut process_direct_field = |key: &str, field: &FieldSelection,
        selections: &mut Vec<OwnedSelectionItem>,
        conditional_field_groups: &mut Vec<(Vec<OwnedConditionEntry>, OwnedConditionOperator, Vec<(String, Option<String>, String, Option<String>)>)>,
        scope_satisfied_fields: &mut Vec<OwnedSelectionItem>| {
        if key == "__typename" { return; }
        let (swift_type, _is_entity) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
        let arguments = render_field_arguments(field);
        let conds = field_inclusion_conditions(field);
        let has_original_conditions = has_inclusion_conditions(conds);
        let effective_conditions = if has_original_conditions {
            let ic = conds.expect("inclusion conditions verified by has_inclusion_conditions");
            !conditions_satisfied_by_scope(ic, scope_conditions)
        } else {
            false
        };
        let (field_name, field_alias) = if key != field.name() {
            (field.name().to_string(), Some(key.to_string()))
        } else {
            (key.to_string(), None)
        };
        if effective_conditions {
            let ic = conds.expect("inclusion conditions verified by has_inclusion_conditions");
            let owned_conds: Vec<OwnedConditionEntry> = ic.conditions.iter().map(|c| OwnedConditionEntry {
                variable: c.variable.clone(),
                is_inverted: c.is_inverted,
            }).collect();
            let operator = match ic.effective_operator() {
                inclusion::InclusionOperator::And => OwnedConditionOperator::And,
                inclusion::InclusionOperator::Or => OwnedConditionOperator::Or,
            };
            let conds_match = |group_conds: &Vec<OwnedConditionEntry>, group_op: &OwnedConditionOperator| -> bool {
                if owned_conds.len() != group_conds.len() { return false; }
                if !matches!((operator, group_op), (OwnedConditionOperator::And, OwnedConditionOperator::And) | (OwnedConditionOperator::Or, OwnedConditionOperator::Or)) { return false; }
                owned_conds.iter().zip(group_conds.iter()).all(|(a, b)| a.variable == b.variable && a.is_inverted == b.is_inverted)
            };
            if let Some(group) = conditional_field_groups.iter_mut().find(|(gc, go, _)| conds_match(gc, go)) {
                group.2.push((field_name, field_alias, swift_type, arguments));
            } else {
                conditional_field_groups.push((owned_conds, operator, vec![(field_name, field_alias, swift_type, arguments)]));
            }
        } else if has_original_conditions {
            scope_satisfied_fields.push(OwnedSelectionItem {
                kind: OwnedSelectionKind::Field {
                    name: field_name,
                    alias: field_alias,
                    swift_type,
                    arguments,
                },
            });
        } else {
            selections.push(OwnedSelectionItem {
                kind: OwnedSelectionKind::Field {
                    name: field_name,
                    alias: field_alias,
                    swift_type,
                    arguments,
                },
            });
        }
    };
    // Iterate in source order: process direct fields and absorbed inline fragment fields
    // in their original source positions, preserving interleaved ordering.
    if !ds.source_order.is_empty() {
        for sk in &ds.source_order {
            match sk {
                IrSelectionKind::Field(key) => {
                    if let Some(field) = ds.fields.get(key) {
                        process_direct_field(key, field, &mut selections, &mut conditional_field_groups, &mut scope_satisfied_fields);
                    }
                }
                IrSelectionKind::InlineFragment(idx) => {
                    if absorbed_inline_indices.contains(idx) {
                        let inline = &ds.inline_fragments[*idx];
                        for (key, field) in &inline.selection_set.direct_selections.fields {
                            if key == "__typename" { continue; }
                            if ds.fields.contains_key(key) { continue; }
                            let (swift_type, _is_entity) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                            let arguments = render_field_arguments(field);
                            let (abs_field_name, abs_field_alias) = if key != field.name() {
                                (field.name().to_string(), Some(key.clone()))
                            } else {
                                (key.clone(), None)
                            };
                            if !selections.iter().any(|s| matches!(&s.kind, OwnedSelectionKind::Field { name, .. } if name == &abs_field_name)) {
                                selections.push(OwnedSelectionItem {
                                    kind: OwnedSelectionKind::Field {
                                        name: abs_field_name,
                                        alias: abs_field_alias,
                                        swift_type,
                                        arguments,
                                    },
                                });
                            }
                        }
                    }
                }
                IrSelectionKind::NamedFragment(_) => {
                    // Named fragments are handled separately below
                }
            }
        }
    } else {
        // Fallback for selection sets without source_order (e.g., programmatically constructed)
        for (key, field) in &ds.fields {
            process_direct_field(key, field, &mut selections, &mut conditional_field_groups, &mut scope_satisfied_fields);
        }
        for &idx in &absorbed_inline_indices {
            let inline = &ds.inline_fragments[idx];
            for (key, field) in &inline.selection_set.direct_selections.fields {
                if key == "__typename" { continue; }
                if ds.fields.contains_key(key) { continue; }
                let (swift_type, _is_entity) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                let arguments = render_field_arguments(field);
                let (abs_field_name, abs_field_alias) = if key != field.name() {
                    (field.name().to_string(), Some(key.clone()))
                } else {
                    (key.clone(), None)
                };
                if !selections.iter().any(|s| matches!(&s.kind, OwnedSelectionKind::Field { name, .. } if name == &abs_field_name)) {
                    selections.push(OwnedSelectionItem {
                        kind: OwnedSelectionKind::Field {
                            name: abs_field_name,
                            alias: abs_field_alias,
                            swift_type,
                            arguments,
                        },
                    });
                }
            }
        }
    }
    // Insert promoted inline fragments BEFORE direct inline fragments.
    // These come from fragment spreads whose type condition differs from the parent type
    // and requires a type narrowing inline fragment (e.g., ...WarmBloodedDetails on Animal
    // creates AsWarmBlooded because WarmBlooded != Animal).
    for promoted_type in &early_promoted_types {
        let type_name = format!("As{}", naming::first_uppercased(customizer.custom_type_name(promoted_type)));
        selections.push(OwnedSelectionItem {
            kind: OwnedSelectionKind::InlineFragment(type_name),
        });
    }
    // Track conditional fragment spreads and direct inline fragments separately for ordering:
    // fragment spreads come before direct inline fragments in the output.
    let mut conditional_frag_spread_selections: Vec<OwnedSelectionItem> = Vec::new();
    let mut conditional_inline_selections: Vec<OwnedSelectionItem> = Vec::new();
    for (idx, inline) in ds.inline_fragments.iter().enumerate() {
        // Skip absorbed inline fragments
        if absorbed_inline_indices.contains(&idx) { continue; }
        if let Some(ref tc) = inline.type_condition {
            if has_inclusion_conditions(inline.inclusion_conditions.as_ref()) {
                let ic = inline.inclusion_conditions.as_ref().expect("inline fragment missing expected inclusion conditions");
                let type_name = conditional_inline_fragment_name(Some(tc.name()), ic, customizer);
                let (owned_conds, operator) = inclusion_conditions_to_owned(ic);
                conditional_inline_selections.push(OwnedSelectionItem {
                    kind: OwnedSelectionKind::ConditionalInlineFragment {
                        conditions: owned_conds,
                        operator,
                        type_name,
                    },
                });
            } else {
                let type_name = format!("As{}", naming::first_uppercased(customizer.custom_type_name(tc.name())));
                selections.push(OwnedSelectionItem {
                    kind: OwnedSelectionKind::InlineFragment(type_name),
                });
            }
        }
    }
    for frag_spread in &ds.named_fragments {
        // Skip fragments that were promoted to inline fragments
        if early_promoted_fragment_names.contains(&frag_spread.fragment_name) { continue; }
        if has_inclusion_conditions(frag_spread.inclusion_conditions.as_ref()) {
            // Conditional fragment spread -> wrapped in .include() as inline fragment
            let ic = frag_spread.inclusion_conditions.as_ref().expect("fragment spread missing expected inclusion conditions");
            // Check if the fragment needs type narrowing (type condition differs from parent)
            if let Some(frag_arc) = frag_map.get(frag_spread.fragment_name.as_str()) {
                let ftc = &frag_arc.type_condition_name;
                let needs_narrowing = *ftc != current_parent_type_name_early
                    && !is_supertype_of_current(&ir_ss.scope.parent_type, ftc);
                let type_name = if needs_narrowing {
                    conditional_inline_fragment_name(Some(ftc), ic, customizer)
                } else {
                    conditional_inline_fragment_name(None, ic, customizer)
                };
                let (owned_conds, operator) = inclusion_conditions_to_owned(ic);
                conditional_frag_spread_selections.push(OwnedSelectionItem {
                    kind: OwnedSelectionKind::ConditionalInlineFragment {
                        conditions: owned_conds,
                        operator,
                        type_name,
                    },
                });
            }
        } else {
            selections.push(OwnedSelectionItem {
                kind: OwnedSelectionKind::Fragment(naming::first_uppercased(&frag_spread.fragment_name)),
            });
        }
    }
    // Add scope-satisfied fields after fragment spreads (these had conditions that were
    // satisfied by the enclosing scope, so they appear unconditionally but after fragments
    // to preserve source ordering).
    selections.extend(scope_satisfied_fields);
    // Add conditional fields after unconditional selections
    for (conds, operator, fields) in &conditional_field_groups {
        if fields.len() == 1 {
            let (name, alias, swift_type, arguments) = &fields[0];
            selections.push(OwnedSelectionItem {
                kind: OwnedSelectionKind::ConditionalField {
                    conditions: conds.clone(),
                    operator: *operator,
                    name: name.clone(),
                    alias: alias.clone(),
                    swift_type: swift_type.clone(),
                    arguments: arguments.clone(),
                },
            });
        } else {
            selections.push(OwnedSelectionItem {
                kind: OwnedSelectionKind::ConditionalFieldGroup {
                    conditions: conds.clone(),
                    operator: *operator,
                    fields: fields.clone(),
                },
            });
        }
    }
    // Add conditional selections in order: fragment spread wrappers, then direct inline fragments
    selections.extend(conditional_frag_spread_selections);
    selections.extend(conditional_inline_selections);

    // Build field accessors (skip __typename)
    // Fields with inclusion conditions become optional types, unless the condition
    // is already satisfied by the enclosing scope's conditions.
    // Use source_order to interleave direct fields and absorbed inline fragment fields.
    let mut field_accessors: Vec<OwnedFieldAccessor> = Vec::new();
    let mut fa_seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let build_field_accessor = |key: &str, field: &FieldSelection| -> OwnedFieldAccessor {
        let (mut swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
        let conds = field_inclusion_conditions(field);
        if has_inclusion_conditions(conds) && !conditions_satisfied_by_scope(conds.expect("inclusion conditions verified by has_inclusion_conditions"), scope_conditions) {
            if !swift_type.ends_with('?') {
                swift_type.push('?');
            }
        }
        OwnedFieldAccessor {
            name: key.to_string(),
            swift_type,
            description: field.description().map(|s| s.to_string()),
        }
    };
    if !ds.source_order.is_empty() {
        for sk in &ds.source_order {
            match sk {
                IrSelectionKind::Field(key) => {
                    if key == "__typename" { continue; }
                    if let Some(field) = ds.fields.get(key) {
                        let fa = build_field_accessor(key, field);
                        fa_seen.insert(fa.name.clone());
                        field_accessors.push(fa);
                    }
                }
                IrSelectionKind::InlineFragment(idx) => {
                    if absorbed_inline_indices.contains(idx) {
                        let inline = &ds.inline_fragments[*idx];
                        for (key, field) in &inline.selection_set.direct_selections.fields {
                            if key == "__typename" { continue; }
                            if ds.fields.contains_key(key) { continue; }
                            if !fa_seen.contains(key) {
                                let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                fa_seen.insert(key.to_string()); field_accessors.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                            }
                        }
                    }
                }
                IrSelectionKind::NamedFragment(_) => {}
            }
        }
    } else {
        // Fallback for selection sets without source_order
        field_accessors = ds.fields.iter()
            .filter(|(key, _)| key.as_str() != "__typename")
            .map(|(key, field)| build_field_accessor(key, field))
            .collect();
        for &idx in &absorbed_inline_indices {
            let inline = &ds.inline_fragments[idx];
            for (key, field) in &inline.selection_set.direct_selections.fields {
                if key == "__typename" { continue; }
                if ds.fields.contains_key(key) { continue; }
                if !fa_seen.contains(key) {
                    let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                    fa_seen.insert(key.to_string()); field_accessors.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                }
            }
        }
    }

    // For inline fragments, add merged field accessors from parent scope
    if is_inline_fragment {
        if let Some(parent) = parent_fields {
            for pf in parent {
                if pf.name != "__typename" && !fa_seen.contains(&pf.name) {
                    fa_seen.insert(pf.name.clone());
                    field_accessors.push(pf.clone());
                }
            }
        }
    }

    // For ALL selection sets with named fragment spreads, include the spread
    // fragment's fields as merged accessors (e.g., WarmBloodedDetails spreading
    // HeightInMeters gets a `height` accessor from HeightInMeters).
    // Skip promoted fragments and conditional fragments - their fields go inside the
    // promoted/conditional inline fragment, not the parent.
    for spread in &ds.named_fragments {
        if early_promoted_fragment_names.contains(&spread.fragment_name) { continue; }
        if has_inclusion_conditions(spread.inclusion_conditions.as_ref()) { continue; }
        if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
            for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                if !fa_seen.contains(key.as_str()) {
                    let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                    fa_seen.insert(key.clone());
                    field_accessors.push(OwnedFieldAccessor {
                        name: key.clone(),
                        swift_type,
                        description: field.description().map(|s| s.to_string()),
                    });
                }
            }
            // Also merge fields from the fragment's sub-fragments
            for sub_spread in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                if let Some(sub_frag) = frag_map.get(sub_spread.fragment_name.as_str()) {
                    // Only merge if the sub-fragment's type condition is satisfied
                    // by the current parent type or is the same
                    let sub_tc = &sub_frag.type_condition_name;
                    let current_type_name = ir_ss.scope.parent_type.name();
                    let should_merge = sub_tc == current_type_name
                        || is_supertype_of_current(&ir_ss.scope.parent_type, sub_tc);
                    if should_merge {
                        for (key, field) in &sub_frag.root_field.selection_set.direct_selections.fields {
                            if !fa_seen.contains(key.as_str()) {
                                let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                fa_seen.insert(key.clone());
                                field_accessors.push(OwnedFieldAccessor {
                                    name: key.clone(),
                                    swift_type,
                                    description: field.description().map(|s| s.to_string()),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Build inline fragment accessors: promoted first, then conditional fragment spreads, then direct (skip absorbed)
    let mut inline_fragment_accessors: Vec<OwnedInlineFragmentAccessor> = Vec::new();
    // Add promoted inline fragment accessors first
    for promoted_type in &early_promoted_types {
        let custom_promoted = customizer.custom_type_name(promoted_type);
        let type_name = format!("As{}", naming::first_uppercased(custom_promoted));
        inline_fragment_accessors.push(OwnedInlineFragmentAccessor {
            property_name: format!("as{}", naming::first_uppercased(custom_promoted)),
            type_name,
        });
    }
    // Then add conditional fragment spread accessors (before direct inline fragments)
    for frag_spread in &ds.named_fragments {
        if early_promoted_fragment_names.contains(&frag_spread.fragment_name) { continue; }
        if has_inclusion_conditions(frag_spread.inclusion_conditions.as_ref()) {
            let ic = frag_spread.inclusion_conditions.as_ref().expect("fragment spread missing expected inclusion conditions");
            if let Some(frag_arc) = frag_map.get(frag_spread.fragment_name.as_str()) {
                let ftc = &frag_arc.type_condition_name;
                let needs_narrowing = *ftc != current_parent_type_name_early
                    && !is_supertype_of_current(&ir_ss.scope.parent_type, ftc);
                let type_name = if needs_narrowing {
                    conditional_inline_fragment_name(Some(ftc), ic, customizer)
                } else {
                    conditional_inline_fragment_name(None, ic, customizer)
                };
                let property_name = if needs_narrowing {
                    conditional_inline_fragment_property(Some(ftc), ic, customizer)
                } else {
                    conditional_inline_fragment_property(None, ic, customizer)
                };
                inline_fragment_accessors.push(OwnedInlineFragmentAccessor {
                    property_name,
                    type_name,
                });
            }
        }
    }
    // Then add direct inline fragment accessors (skip absorbed)
    for (idx, inline) in ds.inline_fragments.iter().enumerate() {
        if absorbed_inline_indices.contains(&idx) { continue; }
        if let Some(ref tc) = inline.type_condition {
            if has_inclusion_conditions(inline.inclusion_conditions.as_ref()) {
                let ic = inline.inclusion_conditions.as_ref().expect("inline fragment missing expected inclusion conditions");
                let type_name = conditional_inline_fragment_name(Some(tc.name()), ic, customizer);
                let property_name = conditional_inline_fragment_property(Some(tc.name()), ic, customizer);
                inline_fragment_accessors.push(OwnedInlineFragmentAccessor {
                    property_name,
                    type_name,
                });
            } else {
                let custom_tc = customizer.custom_type_name(tc.name());
                let type_name = format!("As{}", naming::first_uppercased(custom_tc));
                inline_fragment_accessors.push(OwnedInlineFragmentAccessor {
                    property_name: format!("as{}", naming::first_uppercased(custom_tc)),
                    type_name,
                });
            }
        }
    }

    // Build fragment spread accessors (skip promoted fragments and conditional spreads for selections,
    // but add conditional spreads as optional in the Fragments container)
    let mut fragment_spreads: Vec<OwnedFragmentSpreadAccessor> = ds
        .named_fragments
        .iter()
        .filter(|spread| !early_promoted_fragment_names.contains(&spread.fragment_name))
        .filter(|spread| !has_inclusion_conditions(spread.inclusion_conditions.as_ref()))
        .map(|spread| OwnedFragmentSpreadAccessor {
            property_name: naming::first_lowercased(&spread.fragment_name),
            fragment_type: naming::first_uppercased(&spread.fragment_name),
            is_optional: false,
        })
        .collect();
    // Add conditional fragment spreads as optional accessors in the Fragments container,
    // but ONLY if they don't need type narrowing. When type narrowing is needed, the fragment
    // accessor belongs inside the conditional inline fragment struct, not the parent.
    for spread in &ds.named_fragments {
        if !early_promoted_fragment_names.contains(&spread.fragment_name)
            && has_inclusion_conditions(spread.inclusion_conditions.as_ref())
        {
            if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
                let ftc = &frag_arc.type_condition_name;
                let needs_narrowing = *ftc != current_parent_type_name_early
                    && !is_supertype_of_current(&ir_ss.scope.parent_type, ftc);
                if !needs_narrowing {
                    fragment_spreads.push(OwnedFragmentSpreadAccessor {
                        property_name: naming::first_lowercased(&spread.fragment_name),
                        fragment_type: naming::first_uppercased(&spread.fragment_name),
                        is_optional: true,
                    });
                }
            }
        }
    }

    // Add sub-fragment spreads from directly-spread fragments.
    // E.g., if WarmBloodedDetails spreads HeightInMeters, add HeightInMeters as a fragment accessor
    // if its type condition is satisfied by the current scope's type.
    // Skip conditional spreads - their sub-fragments go inside the conditional inline fragment.
    for spread in &ds.named_fragments {
        if early_promoted_fragment_names.contains(&spread.fragment_name) { continue; }
        if has_inclusion_conditions(spread.inclusion_conditions.as_ref()) { continue; }
        if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
            for sub_spread in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                let sub_frag_type = naming::first_uppercased(&sub_spread.fragment_name);
                if !fragment_spreads.iter().any(|fs| fs.fragment_type == sub_frag_type) {
                    if let Some(sub_frag) = frag_map.get(sub_spread.fragment_name.as_str()) {
                        let sub_tc = &sub_frag.type_condition_name;
                        let current_type_name = ir_ss.scope.parent_type.name();
                        let should_include = sub_tc == current_type_name
                            || is_supertype_of_current(&ir_ss.scope.parent_type, sub_tc);
                        if should_include {
                            fragment_spreads.push(OwnedFragmentSpreadAccessor {
                                property_name: naming::first_lowercased(&sub_spread.fragment_name),
                                fragment_type: sub_frag_type,
                                is_optional: false,
                            });
                        }
                    }
                }
            }
        }
    }

    // Build nested types
    let mut nested_types = Vec::new();
    // Nested entity fields
    for (key, field) in &ds.fields {
        if let FieldSelection::Entity(ef) = field {
            // Singularize the response key to get the struct name for list types
            // (e.g., "allAnimals" → "AllAnimal", "predators" → "Predator")
            // Non-list types use the response key directly (e.g., "starship" → "Starship")
            let child_name = if ef.field_type.is_list() {
                naming::first_uppercased(&naming::singularize(key))
            } else {
                naming::first_uppercased(key)
            };
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
                customizer,
                None, // entity_root_graphql_type: entity fields start a new entity
                &[],  // entity fields start a new entity scope
                None, // entity fields start a new scope
                None, // no scope conditions for entity fields
                api_target_name,
            );
            // Merge fields from fragment spreads that also have this entity field.
            // E.g., if HeightInMeters has `height { meters }`, merge `meters` into Height.
            // Skip conditional spreads - their entity fields belong inside the conditional inline fragment.
            for spread in &ds.named_fragments {
                if has_inclusion_conditions(spread.inclusion_conditions.as_ref()) { continue; }
                if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
                    if let Some(FieldSelection::Entity(frag_ef)) = frag_arc.root_field.selection_set.direct_selections.fields.get(key) {
                        for (frag_key, frag_field) in &frag_ef.selection_set.direct_selections.fields {
                            if frag_key == "__typename" { continue; }
                            if !child_ss.field_accessors.iter().any(|f| f.name == *frag_key) {
                                let (swift_type, _) = render_field_swift_type(frag_field, schema_namespace, type_kinds, customizer);
                                child_ss.field_accessors.push(OwnedFieldAccessor { name: frag_key.clone(), swift_type, description: frag_field.description().map(|s| s.to_string()) });
                                // Also add to initializer if it exists
                                if let Some(ref mut init) = child_ss.initializer {
                                    init.parameters.push(OwnedInitParam {
                                        name: frag_key.clone(),
                                        swift_type: {
                                            let (st, _) = render_field_swift_type(frag_field, schema_namespace, type_kinds, customizer);
                                            st
                                        },
                                        default_value: {
                                            let (st, _) = render_field_swift_type(frag_field, schema_namespace, type_kinds, customizer);
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
                            let frag_entity_qualified = format!("{}.{}", naming::first_uppercased(&spread.fragment_name), child_name);
                            if !init.fulfilled_fragments.contains(&frag_entity_qualified) {
                                init.fulfilled_fragments.push(frag_entity_qualified);
                            }
                        }
                    }
                    // Also check sub-fragments
                    for sub_spread in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                        if let Some(sub_frag) = frag_map.get(sub_spread.fragment_name.as_str()) {
                            if let Some(FieldSelection::Entity(sub_ef)) = sub_frag.root_field.selection_set.direct_selections.fields.get(key) {
                                for (frag_key, frag_field) in &sub_ef.selection_set.direct_selections.fields {
                                    if frag_key == "__typename" { continue; }
                                    if !child_ss.field_accessors.iter().any(|f| f.name == *frag_key) {
                                        let (swift_type, _) = render_field_swift_type(frag_field, schema_namespace, type_kinds, customizer);
                                        child_ss.field_accessors.push(OwnedFieldAccessor { name: frag_key.clone(), swift_type, description: frag_field.description().map(|s| s.to_string()) });
                                        if let Some(ref mut init) = child_ss.initializer {
                                            init.parameters.push(OwnedInitParam {
                                                name: frag_key.clone(),
                                                swift_type: {
                                                    let (st, _) = render_field_swift_type(frag_field, schema_namespace, type_kinds, customizer);
                                                    st
                                                },
                                                default_value: {
                                                    let (st, _) = render_field_swift_type(frag_field, schema_namespace, type_kinds, customizer);
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
                                    let sub_frag_entity_qualified = format!("{}.{}", naming::first_uppercased(&sub_spread.fragment_name), child_name);
                                    if !init.fulfilled_fragments.contains(&sub_frag_entity_qualified) {
                                        init.fulfilled_fragments.push(sub_frag_entity_qualified);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Merge entity sub-fields from sibling inline fragments in the parent scope.
            // E.g., when building AsIssue.Author, merge `login` from AsReactable.AsComment.author { login }.
            // This ensures entity types include all fields from all paths across the entity.
            if is_inline_fragment {
                if let Some(pds) = parent_scope_ds {
                    // Collect matching entity fields from sibling inline fragments
                    // (and their nested inline fragments, recursively)
                    fn collect_entity_subfields_from_inline<'a>(
                        inline: &'a InlineFragmentSelection,
                        field_key: &str,
                    ) -> Vec<(&'a str, &'a FieldSelection)> {
                        let mut result = Vec::new();
                        // Check the inline fragment's own fields
                        if let Some(FieldSelection::Entity(ef)) = inline.selection_set.direct_selections.fields.get(field_key) {
                            for (k, f) in &ef.selection_set.direct_selections.fields {
                                result.push((k.as_str(), f));
                            }
                        }
                        // Recursively check nested inline fragments
                        for nested_inline in &inline.selection_set.direct_selections.inline_fragments {
                            result.extend(collect_entity_subfields_from_inline(nested_inline, field_key));
                        }
                        result
                    }

                    for sibling in &pds.inline_fragments {
                        let subfields = collect_entity_subfields_from_inline(sibling, key);
                        for (sub_key, sub_field) in subfields {
                            if sub_key == "__typename" { continue; }
                            if !child_ss.field_accessors.iter().any(|f| f.name == sub_key) {
                                let (swift_type, _) = render_field_swift_type(sub_field, schema_namespace, type_kinds, customizer);
                                child_ss.field_accessors.push(OwnedFieldAccessor {
                                    name: sub_key.to_string(),
                                    swift_type,
                                    description: sub_field.description().map(|s| s.to_string()),
                                });
                            }
                        }
                    }
                }
            }

            let parent_type_name = customizer.custom_type_name(ef.selection_set.scope.parent_type.name());
            let doc_comment = if is_root {
                format!("/// {}", child_name)
            } else {
                // Use full entity-relative path for doc comment
                let doc_prefix = if let Some(pos) = qualified_name.find(".Data.") {
                    &qualified_name[pos + 6..]
                } else {
                    struct_name
                };
                format!("/// {}.{}", doc_prefix, child_name)
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

    // Build nested entity types from absorbed inline fragments.
    // When an absorbed inline fragment has entity fields (e.g., `... on Animal { height { relativeSize centimeters } }`
    // absorbed into AsPet), we need to create merged nested entity types that combine the absorbed
    // fields as __selections with the parent entity's fields as merged accessors.
    for &idx in &absorbed_inline_indices {
        let absorbed_inline = &ds.inline_fragments[idx];
        for (key, field) in &absorbed_inline.selection_set.direct_selections.fields {
            if let FieldSelection::Entity(absorbed_ef) = field {
                let child_name = if absorbed_ef.field_type.is_list() {
                    naming::first_uppercased(&naming::singularize(key))
                } else {
                    naming::first_uppercased(key)
                };
                // Check if we already have this entity as a nested type (from direct selections)
                let existing_idx = nested_types.iter().position(|nt: &OwnedNestedSelectionSet| nt.config.struct_name == child_name);
                if let Some(existing_pos) = existing_idx {
                    // Merge absorbed selections into the existing nested type
                    let existing = &mut nested_types[existing_pos];
                    // Add absorbed fields as selections (they become the __selections for AsPet.Height)
                    for (abs_key, abs_field) in &absorbed_ef.selection_set.direct_selections.fields {
                        if abs_key == "__typename" { continue; }
                        let (swift_type, _) = render_field_swift_type(abs_field, schema_namespace, type_kinds, customizer);
                        // Add to selections if not present
                        let (abs_fn, abs_fa) = if abs_key != abs_field.name() {
                            (abs_field.name().to_string(), Some(abs_key.clone()))
                        } else {
                            (abs_key.clone(), None)
                        };
                        if !existing.config.selections.iter().any(|s| matches!(&s.kind, OwnedSelectionKind::Field { name, .. } if name == &abs_fn)) {
                            existing.config.selections.push(OwnedSelectionItem {
                                kind: OwnedSelectionKind::Field { name: abs_fn, alias: abs_fa, swift_type: swift_type.clone(), arguments: None },
                            });
                        }
                        // Add to field accessors - INSERT at beginning before parent fields
                        if !existing.config.field_accessors.iter().any(|f| f.name == *abs_key) {
                            // Insert at position 0 (before parent fields)
                            existing.config.field_accessors.insert(0, OwnedFieldAccessor { name: abs_key.clone(), swift_type: swift_type.clone(), description: abs_field.description().map(|s| s.to_string()) });
                            // Also update initializer
                            if let Some(ref mut init) = existing.config.initializer {
                                // Find the right position - after __typename if present, before existing params
                                let insert_pos = if init.parameters.first().map(|p| p.name == "__typename").unwrap_or(false) { 1 } else { 0 };
                                init.parameters.insert(insert_pos, OwnedInitParam {
                                    name: abs_key.clone(), swift_type: swift_type.clone(),
                                    default_value: if swift_type.ends_with('?') { Some("nil".to_string()) } else { None },
                                });
                                let data_insert_pos = if init.data_entries.first().map(|e| e.key == "__typename").unwrap_or(false) { 1 } else { 0 };
                                init.data_entries.insert(data_insert_pos, OwnedDataEntry {
                                    key: abs_key.clone(), value: OwnedDataEntryValue::Variable(abs_key.clone()),
                                });
                            }
                        }
                    }
                }
                // Note: when the absorbed inline introduces an entity field that is NOT
                // in ds.fields, we don't create it here. Instead, it will be handled by
                // the inline fragment entity processing (lines below) which has access
                // to the parent scope's entity field data.
            }
        }
    }

    // Track the index where promoted inline fragment nested types should be inserted.
    // They go after entity fields/absorbed entity types but before direct inline fragments.
    // This index is incremented after each insertion to maintain order.
    let mut promoted_insert_index = nested_types.len();
    // Save the entity type count for type_alias_insert_index (type aliases should be
    // rendered after entity nested types but before inline fragment nested types).
    let entity_nested_type_count = nested_types.len();

    // Pre-compute which fragments will be promoted to inline fragments (type narrowing).
    // We need this before processing direct inline fragments so we can determine
    // which parent-scope fragments are applicable to each inline fragment.
    let current_parent_type_name_pre = ir_ss.scope.parent_type.name().to_string();
    let direct_inline_type_names_pre: Vec<String> = ds
        .inline_fragments
        .iter()
        .enumerate()
        .filter(|(idx, _)| !absorbed_inline_indices.contains(idx))
        .filter_map(|(_, inline)| inline.type_condition.as_ref().map(|tc| tc.name().to_string()))
        .collect();
    let mut pre_promoted_fragment_names: Vec<String> = Vec::new();
    {
        let mut seen_promoted_types: Vec<String> = Vec::new();
        for spread in &ds.named_fragments {
            // Skip conditional fragment spreads - they get their own conditional inline fragment
            if has_inclusion_conditions(spread.inclusion_conditions.as_ref()) { continue; }
            if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
                let ftc = &frag_arc.type_condition_name;
                let needs_narrowing = *ftc != current_parent_type_name_pre
                    && !is_supertype_of_current(&ir_ss.scope.parent_type, ftc);
                if needs_narrowing
                    && !direct_inline_type_names_pre.contains(ftc)
                    && !seen_promoted_types.contains(ftc)
                {
                    pre_promoted_fragment_names.push(spread.fragment_name.clone());
                    seen_promoted_types.push(ftc.clone());
                }
            }
        }
    }

    // Collect sibling inline fragment fields for merging.
    // For each inline fragment type, collect fields from OTHER inline fragments
    // whose type conditions are supertypes (e.g., AsCat gets fields from AsAnimal, AsPet).
    let sibling_inline_fields: Vec<(&str, Vec<OwnedFieldAccessor>)> = ds
        .inline_fragments
        .iter()
        .enumerate()
        .filter(|(idx, _)| !absorbed_inline_indices.contains(idx))
        .filter_map(|(_, inline)| {
            inline.type_condition.as_ref().map(|tc| {
                let mut merged = Vec::new();
                for (other_idx, other) in ds.inline_fragments.iter().enumerate() {
                    if absorbed_inline_indices.contains(&other_idx) { continue; }
                    if let Some(ref other_tc) = other.type_condition {
                        if other_tc.name() != tc.name()
                            && is_supertype_of_current(tc, other_tc.name())
                        {
                            // other's type is a supertype of this type - merge its fields
                            for (key, field) in &other.selection_set.direct_selections.fields {
                                let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                if !merged.iter().any(|f: &OwnedFieldAccessor| f.name == *key) {
                                    merged.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                                }
                            }
                        }
                    }
                }
                (tc.name(), merged)
            })
        })
        .collect();

    // Nested inline fragments (skip absorbed ones)
    for (inline_idx, inline) in ds.inline_fragments.iter().enumerate() {
        if absorbed_inline_indices.contains(&inline_idx) { continue; }
        if let Some(ref tc) = inline.type_condition {
            let type_name = if has_inclusion_conditions(inline.inclusion_conditions.as_ref()) {
                conditional_inline_fragment_name(Some(tc.name()), inline.inclusion_conditions.as_ref().expect("inline fragment missing expected inclusion conditions"), customizer)
            } else {
                format!("As{}", naming::first_uppercased(customizer.custom_type_name(tc.name())))
            };
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
            // Determine the entity root GraphQL type for inline fragments.
            // This is the entity's actual type (e.g., "Animal" for AllAnimal).
            // If we're already inside an inline fragment with an entity root, use it;
            // otherwise use the current parent type.
            let entity_root_for_inline = entity_root_graphql_type
                .unwrap_or_else(|| ir_ss.scope.parent_type.name());
            // Build ancestor fragment list for child scope: combine current ancestor
            // fragments with the current scope's named fragment spreads AND fragment
            // spreads from sibling inline fragments.
            let mut child_ancestor_frags: Vec<String> = ancestor_fragments.to_vec();
            for spread in &ds.named_fragments {
                // Skip conditional spreads - they have their own scope
                if has_inclusion_conditions(spread.inclusion_conditions.as_ref()) { continue; }
                if !child_ancestor_frags.contains(&spread.fragment_name) {
                    child_ancestor_frags.push(spread.fragment_name.clone());
                }
            }
            // Also include fragment spreads from sibling inline fragments
            for sibling in &ds.inline_fragments {
                if absorbed_inline_indices.contains(&ds.inline_fragments.iter().position(|s| std::ptr::eq(s, sibling)).unwrap_or(usize::MAX)) { continue; }
                for sib_spread in &sibling.selection_set.direct_selections.named_fragments {
                    if !child_ancestor_frags.contains(&sib_spread.fragment_name) {
                        child_ancestor_frags.push(sib_spread.fragment_name.clone());
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
                customizer,
                Some(entity_root_for_inline),
                &child_ancestor_frags,
                Some(ds), // pass parent scope's direct selections
                inline.inclusion_conditions.as_ref(), // pass inline fragment's conditions to strip from children
                api_target_name,
            );

            // When this inline fragment is nested inside a conditional scope (e.g., AsDroid
            // inside IfIncludeFriendsDetails), add the parent conditional scope to the child's
            // fulfilled fragments so the child includes it.
            if struct_name.starts_with("If") && is_inline_fragment {
                if let Some(ref mut init) = child_ss.initializer {
                    let parent_scope = qualified_name.to_string();
                    if !init.fulfilled_fragments.contains(&parent_scope) {
                        // Insert after the root entity, before self
                        let insert_pos = if init.fulfilled_fragments.len() >= 2 { 1 } else { init.fulfilled_fragments.len() };
                        init.fulfilled_fragments.insert(insert_pos, parent_scope);
                    }
                }
            }

            // Propagate absorbed type names from parent to child
            for atn in &absorbed_type_names {
                if !child_ss.absorbed_type_names.contains(atn) {
                    child_ss.absorbed_type_names.push(atn.clone());
                }
            }

            // Compute applicable fragments for this inline fragment type.
            // These are parent-scope fragments whose type conditions are satisfied
            // by the inline fragment's type, plus fragments from sibling inline fragments.
            let applicable_frags = collect_applicable_fragments(
                tc,
                &current_parent_type_name_pre,
                ds,
                &pre_promoted_fragment_names,
                referenced_fragments,
                &ir_ss.scope.parent_type,
                ancestor_fragments,
            );
            let applicable_frags_set: std::collections::HashSet<&str> = applicable_frags.iter().map(|s| s.as_str()).collect();

            // Reorder applicable fragments for fragment spreads: hoist sub-fragments
            // to the front, preserving relative order otherwise.
            // E.g., [PetDetails, WarmBloodedDetails, HeightInMeters] becomes
            // [HeightInMeters, PetDetails, WarmBloodedDetails] because HeightInMeters
            // is a sub-fragment of WarmBloodedDetails.
            let applicable_frags_for_spreads = {
                // Build a set of sub-fragment names
                let mut is_sub_frag: std::collections::HashSet<String> = std::collections::HashSet::new();
                for frag_name in &applicable_frags {
                    if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                        for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                            if applicable_frags_set.contains(sub.fragment_name.as_str()) {
                                is_sub_frag.insert(sub.fragment_name.clone());
                            }
                        }
                    }
                }
                // Sub-fragments first, then non-sub-fragments, preserving relative order within each group
                let mut reordered: Vec<String> = Vec::new();
                for frag_name in &applicable_frags {
                    if is_sub_frag.contains(frag_name) {
                        reordered.push(frag_name.clone());
                    }
                }
                for frag_name in &applicable_frags {
                    if !is_sub_frag.contains(frag_name) {
                        reordered.push(frag_name.clone());
                    }
                }
                reordered
            };

            // Add applicable fragment spreads that the inline fragment doesn't already have
            for frag_name in &applicable_frags_for_spreads {
                if !child_ss.fragment_spreads.iter().any(|fs| fs.fragment_type == *frag_name) {
                    child_ss.fragment_spreads.push(OwnedFragmentSpreadAccessor {
                        property_name: naming::first_lowercased(frag_name),
                        fragment_type: frag_name.clone(),
                        is_optional: false,
                    });
                }
            }

            // Add conditional parent-scope fragment spreads as optional accessors.
            // E.g., if the root scope has `...HeightInMeters @skip(if: $skipHeightInMeters)`,
            // add it as optional in all child inline fragments (AsPet, AsClassroomPet, etc.).
            // Skip if:
            // - The fragment is already handled by the child (non-conditional spread)
            // - The fragment requires type narrowing (its type condition differs from the parent scope's type)
            for spread in &ds.named_fragments {
                if has_inclusion_conditions(spread.inclusion_conditions.as_ref()) {
                    // Skip if the child's own scope already has this fragment as a non-conditional spread
                    let child_has_unconditional = inline.selection_set.direct_selections.named_fragments.iter()
                        .any(|s| s.fragment_name == spread.fragment_name && !has_inclusion_conditions(s.inclusion_conditions.as_ref()));
                    if child_has_unconditional { continue; }
                    // Skip if the fragment requires type narrowing (its type condition differs from
                    // the parent scope's type). Such fragments need their own inline fragment scope.
                    if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
                        let parent_type_name = ir_ss.scope.parent_type.name();
                        if frag_arc.type_condition_name != parent_type_name
                            && !is_supertype_of_current(&ir_ss.scope.parent_type, &frag_arc.type_condition_name)
                        {
                            continue;
                        }
                    }
                    if !child_ss.fragment_spreads.iter().any(|fs| fs.fragment_type == spread.fragment_name) {
                        child_ss.fragment_spreads.push(OwnedFragmentSpreadAccessor {
                            property_name: naming::first_lowercased(&spread.fragment_name),
                            fragment_type: spread.fragment_name.clone(),
                            is_optional: true,
                        });
                    }
                }
            }

            // Reorder applicable fragments for field merging and fulfilled fragments:
            // Fragments that have sub-fragments within the applicable set come first,
            // immediately followed by their sub-fragments, then remaining "leaf" fragments.
            // This ensures e.g. WarmBloodedDetails.bodyTemperature comes before
            // PetDetails.humanName, with HeightInMeters (sub of WBD) between them.
            let applicable_frags_for_fields = {
                // Map: parent fragment name -> list of its sub-fragment names in applicable set
                let mut sub_frags_of: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
                let mut is_sub_frag: std::collections::HashSet<String> = std::collections::HashSet::new();
                for frag_name in &applicable_frags {
                    if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                        for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                            if applicable_frags_set.contains(sub.fragment_name.as_str()) {
                                sub_frags_of.entry(frag_name.clone()).or_default().push(sub.fragment_name.clone());
                                is_sub_frag.insert(sub.fragment_name.clone());
                            }
                        }
                    }
                }
                let mut result = Vec::new();
                let mut result_seen = std::collections::HashSet::new();
                // First: parent fragments followed by their sub-fragments
                for frag_name in &applicable_frags {
                    if sub_frags_of.contains_key(frag_name) && !is_sub_frag.contains(frag_name) {
                        if result_seen.insert(frag_name.clone()) {
                            result.push(frag_name.clone());
                        }
                        if let Some(subs) = sub_frags_of.get(frag_name) {
                            for sub in subs {
                                if result_seen.insert(sub.clone()) {
                                    result.push(sub.clone());
                                }
                            }
                        }
                    }
                }
                // Then: remaining fragments (not parents, not sub-fragments)
                for frag_name in &applicable_frags {
                    if result_seen.insert(frag_name.clone()) {
                        result.push(frag_name.clone());
                    }
                }
                result
            };

            // Add merged fields from applicable fragments
            for frag_name in &applicable_frags_for_fields {
                if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                    for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                        if key == "__typename" { continue; }
                        if !child_ss.field_accessors.iter().any(|f| f.name == *key) {
                            let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                            child_ss.field_accessors.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                        }
                    }
                    // Also merge fields from sub-fragments
                    for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                        if let Some(inner) = frag_map.get(sub.fragment_name.as_str()) {
                            for (key, field) in &inner.root_field.selection_set.direct_selections.fields {
                                if key == "__typename" { continue; }
                                if !child_ss.field_accessors.iter().any(|f| f.name == *key) {
                                    let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                    child_ss.field_accessors.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                                }
                            }
                        }
                    }
                }
            }

            // Add merged fields from sibling inline fragments whose type is a supertype
            for sibling_inline in &ds.inline_fragments {
                if let Some(ref sibling_tc) = sibling_inline.type_condition {
                    if sibling_tc.name() != tc.name() && is_supertype_of_current(tc, sibling_tc.name()) {
                        // Also merge fields from fragment spreads of this sibling
                        for spread in &sibling_inline.selection_set.direct_selections.named_fragments {
                            if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
                                for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                                    if key == "__typename" { continue; }
                                    if !child_ss.field_accessors.iter().any(|f| f.name == *key) {
                                        let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                        child_ss.field_accessors.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                                    }
                                }
                                for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                                    if let Some(inner) = frag_map.get(sub.fragment_name.as_str()) {
                                        for (key, field) in &inner.root_field.selection_set.direct_selections.fields {
                                            if key == "__typename" { continue; }
                                            if !child_ss.field_accessors.iter().any(|f| f.name == *key) {
                                                let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                                child_ss.field_accessors.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Sync the initializer with the (now-complete) field_accessors.
            // Fields added from applicable/sibling fragments above were NOT in the
            // initializer that was built inside the recursive call.
            if let Some(ref mut init) = child_ss.initializer {
                // Helper closure to check if a field name refers to an entity field
                for fa in &child_ss.field_accessors {
                    if fa.name == "__typename" { continue; }
                    let check_entity_for_field = || -> bool {
                        ds.fields.get(&fa.name)
                            .map(|f| matches!(f, FieldSelection::Entity(_)))
                            .unwrap_or(false)
                            || inline.selection_set.direct_selections.fields.get(&fa.name)
                                .map(|f| matches!(f, FieldSelection::Entity(_)))
                                .unwrap_or(false)
                            || applicable_frags.iter().any(|frag_name| {
                                frag_map.get(frag_name.as_str())
                                    .map(|frag_arc| {
                                        frag_arc.root_field.selection_set.direct_selections.fields.get(&fa.name)
                                            .map(|f| matches!(f, FieldSelection::Entity(_)))
                                            .unwrap_or(false)
                                    })
                                    .unwrap_or(false)
                            })
                            || referenced_fragments.iter().any(|frag_arc| {
                                frag_arc.root_field.selection_set.direct_selections.fields.get(&fa.name)
                                    .map(|f| matches!(f, FieldSelection::Entity(_)))
                                    .unwrap_or(false)
                            })
                            // Fallback: check if the Swift type looks like an entity type
                            || swift_type_is_entity(&fa.swift_type)
                    };
                    // Add to parameters if missing
                    if !init.parameters.iter().any(|p| p.name == fa.name) {
                        init.parameters.push(OwnedInitParam {
                            name: fa.name.clone(),
                            swift_type: fa.swift_type.clone(),
                            default_value: if fa.swift_type.ends_with('?') { Some("nil".to_string()) } else { None },
                        });
                    }
                    // Add to data_entries if missing
                    if !init.data_entries.iter().any(|e| e.key == fa.name) {
                        let is_entity = check_entity_for_field();
                        let value = if is_entity {
                            OwnedDataEntryValue::FieldData(fa.name.clone())
                        } else {
                            OwnedDataEntryValue::Variable(fa.name.clone())
                        };
                        init.data_entries.push(OwnedDataEntry {
                            key: fa.name.clone(),
                            value,
                        });
                    }
                    // Fix existing data entry if it's Variable but should be FieldData
                    if let Some(entry) = init.data_entries.iter_mut().find(|e| e.key == fa.name) {
                        if matches!(entry.value, OwnedDataEntryValue::Variable(_)) && check_entity_for_field() {
                            entry.value = OwnedDataEntryValue::FieldData(fa.name.clone());
                        }
                    }
                }
            }

            // Build nested entity types and type aliases from applicable fragments.
            // For each entity field that this inline fragment INTRODUCES new selections for
            // (via applicable fragments, sibling inline fragments, or direct selections),
            // create a merged Height-style struct or type alias.
            // Entity fields that are ONLY inherited from the parent scope without any new
            // selections are NOT given nested types here (e.g., predators).
            if !applicable_frags.is_empty() || !sibling_inline_fields.is_empty() {
                // Collect entity fields that have new selections in this scope.
                // Start from applicable fragments, siblings, and inline's own selections.
                // DO NOT include parent entity fields that aren't also referenced by the above.
                let mut entity_field_keys: Vec<String> = Vec::new();
                // Collect entity fields from the inline fragment's own direct selections
                for (key, field) in &inline.selection_set.direct_selections.fields {
                    if matches!(field, FieldSelection::Entity(_)) && !entity_field_keys.contains(key) {
                        entity_field_keys.push(key.clone());
                    }
                }
                // Also check absorbed inline fragments within this inline fragment
                for nested_inline in &inline.selection_set.direct_selections.inline_fragments {
                    for (key, field) in &nested_inline.selection_set.direct_selections.fields {
                        if matches!(field, FieldSelection::Entity(_)) && !entity_field_keys.contains(key) {
                            entity_field_keys.push(key.clone());
                        }
                    }
                }
                // Collect entity fields from applicable fragments
                for frag_name in &applicable_frags {
                    if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                        for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                            if matches!(field, FieldSelection::Entity(_)) && !entity_field_keys.contains(key) {
                                entity_field_keys.push(key.clone());
                            }
                        }
                        for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                            if let Some(inner) = frag_map.get(sub.fragment_name.as_str()) {
                                for (key, field) in &inner.root_field.selection_set.direct_selections.fields {
                                    if matches!(field, FieldSelection::Entity(_)) && !entity_field_keys.contains(key) {
                                        entity_field_keys.push(key.clone());
                                    }
                                }
                            }
                        }
                    }
                }
                // Also from sibling inline fragments
                for sibling_inline in &ds.inline_fragments {
                    if let Some(ref sibling_tc) = sibling_inline.type_condition {
                        if sibling_tc.name() != tc.name() && is_supertype_of_current(tc, sibling_tc.name()) {
                            for (key, field) in &sibling_inline.selection_set.direct_selections.fields {
                                if matches!(field, FieldSelection::Entity(_)) && !entity_field_keys.contains(key) {
                                    entity_field_keys.push(key.clone());
                                }
                            }
                        }
                    }
                }

                for field_key in &entity_field_keys {
                    // Determine if this entity field is a list type by looking it up from available sources
                    let is_list_field = inline.selection_set.direct_selections.fields.get(field_key)
                        .and_then(|f| if let FieldSelection::Entity(ef) = f { Some(ef.field_type.is_list()) } else { None })
                        .or_else(|| {
                            ds.fields.get(field_key)
                                .and_then(|f| if let FieldSelection::Entity(ef) = f { Some(ef.field_type.is_list()) } else { None })
                        })
                        .or_else(|| {
                            for sibling_inline in &ds.inline_fragments {
                                if let Some(FieldSelection::Entity(ef)) = sibling_inline.selection_set.direct_selections.fields.get(field_key) {
                                    return Some(ef.field_type.is_list());
                                }
                            }
                            None
                        })
                        .or_else(|| {
                            for frag_name in &applicable_frags {
                                if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                                    if let Some(FieldSelection::Entity(ef)) = frag_arc.root_field.selection_set.direct_selections.fields.get(field_key) {
                                        return Some(ef.field_type.is_list());
                                    }
                                }
                            }
                            None
                        })
                        .unwrap_or(true); // default to singularize (list behavior) if not found
                    let entity_struct_name = if is_list_field {
                        naming::first_uppercased(&naming::singularize(field_key))
                    } else {
                        naming::first_uppercased(field_key)
                    };

                    // Check if this inline fragment already has a nested type for this entity
                    let already_has = child_ss.nested_types.iter().any(|nt| nt.config.struct_name == entity_struct_name);
                    if already_has { continue; }

                    // Check if the parent has this entity field (for the merged struct).
                    // Also check parent_scope_ds (grandparent scope) if not found in current scope.
                    let parent_entity_field = ds.fields.get(field_key).and_then(|f| {
                        if let FieldSelection::Entity(ef) = f { Some(ef) } else { None }
                    }).or_else(|| {
                        // Check absorbed inline fragments of current scope
                        for &abs_idx in &absorbed_inline_indices {
                            if let Some(FieldSelection::Entity(ef)) = ds.inline_fragments[abs_idx].selection_set.direct_selections.fields.get(field_key) {
                                return Some(ef);
                            }
                        }
                        None
                    }).or_else(|| {
                        // Check parent scope's direct fields
                        parent_scope_ds.and_then(|pds| {
                            pds.fields.get(field_key).and_then(|f| {
                                if let FieldSelection::Entity(ef) = f { Some(ef) } else { None }
                            })
                        })
                    });

                    // Check if the inline fragment itself has direct selections on this field
                    // Also check absorbed inline fragments within the inline
                    let inline_has_field = inline.selection_set.direct_selections.fields.get(field_key).and_then(|f| {
                        if let FieldSelection::Entity(ef) = f { Some(ef) } else { None }
                    }).or_else(|| {
                        // Check absorbed inline fragments within this inline fragment
                        for nested_inline in &inline.selection_set.direct_selections.inline_fragments {
                            if let Some(FieldSelection::Entity(ef)) = nested_inline.selection_set.direct_selections.fields.get(field_key) {
                                return Some(ef);
                            }
                        }
                        None
                    });

                    // Collect entity fields from sibling inline fragments that are supertypes
                    let mut sibling_entity: Vec<(String, Vec<(String, String)>)> = Vec::new();
                    for sibling_inline in &ds.inline_fragments {
                        if let Some(ref sibling_tc) = sibling_inline.type_condition {
                            if sibling_tc.name() != tc.name() && is_supertype_of_current(tc, sibling_tc.name()) {
                                // Check direct fields of the sibling
                                if let Some(FieldSelection::Entity(sib_ef)) = sibling_inline.selection_set.direct_selections.fields.get(field_key) {
                                    let sibling_height_qualified = format!("{}.As{}.{}", qualified_name, naming::first_uppercased(sibling_tc.name()), entity_struct_name);
                                    let mut sib_fields = Vec::new();
                                    for (key, field) in &sib_ef.selection_set.direct_selections.fields {
                                        if key == "__typename" { continue; }
                                        let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                        sib_fields.push((key.clone(), swift_type));
                                    }
                                    sibling_entity.push((sibling_height_qualified, sib_fields));
                                }
                                // Also check nested inline fragments within this sibling
                                // (e.g., ... on Pet { ... on Animal { height { relativeSize centimeters } } })
                                for nested_inline in &sibling_inline.selection_set.direct_selections.inline_fragments {
                                    if let Some(ref nested_tc) = nested_inline.type_condition {
                                        if is_supertype_of_current(tc, nested_tc.name()) || tc.name() == nested_tc.name() {
                                            if let Some(FieldSelection::Entity(nested_ef)) = nested_inline.selection_set.direct_selections.fields.get(field_key) {
                                                let nested_qualified = format!("{}.As{}.{}", qualified_name, naming::first_uppercased(sibling_tc.name()), entity_struct_name);
                                                let mut nested_fields = Vec::new();
                                                for (key, field) in &nested_ef.selection_set.direct_selections.fields {
                                                    if key == "__typename" { continue; }
                                                    let (mut swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                                    // Fields with inclusion conditions become optional
                                                    let conds = field_inclusion_conditions(field);
                                                    if has_inclusion_conditions(conds) && !swift_type.ends_with('?') {
                                                        swift_type.push('?');
                                                    }
                                                    nested_fields.push((key.clone(), swift_type));
                                                }
                                                // Merge into existing sibling entity or add new
                                                if let Some(existing) = sibling_entity.iter_mut().find(|(q, _)| q == &nested_qualified) {
                                                    for (key, st) in &nested_fields {
                                                        if !existing.1.iter().any(|(k, _)| k == key) {
                                                            existing.1.push((key.clone(), st.clone()));
                                                        }
                                                    }
                                                } else {
                                                    sibling_entity.push((nested_qualified, nested_fields));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Also check parent_scope_ds's inline fragments for entity fields
                    // (e.g., AsPet has ... on Animal { height { relativeSize centimeters } } in AllAnimal)
                    if let Some(pds) = parent_scope_ds {
                        // Determine the root qualified name for parent scope siblings
                        let root_qualified = root_entity_type.unwrap_or(qualified_name);
                        for pds_inline in &pds.inline_fragments {
                            if let Some(ref pds_tc) = pds_inline.type_condition {
                                if is_supertype_of_current(tc, pds_tc.name()) || tc.name() == pds_tc.name() {
                                    // Check nested inline fragments within this parent sibling
                                    for nested_inline in &pds_inline.selection_set.direct_selections.inline_fragments {
                                        if let Some(ref nested_tc) = nested_inline.type_condition {
                                            if is_supertype_of_current(tc, nested_tc.name()) || tc.name() == nested_tc.name() {
                                                if let Some(FieldSelection::Entity(nested_ef)) = nested_inline.selection_set.direct_selections.fields.get(field_key) {
                                                    let nested_qualified = format!("{}.As{}.{}", root_qualified, naming::first_uppercased(pds_tc.name()), entity_struct_name);
                                                    let mut nested_fields = Vec::new();
                                                    for (key, field) in &nested_ef.selection_set.direct_selections.fields {
                                                        if key == "__typename" { continue; }
                                                        let (mut swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                                        // Fields with inclusion conditions become optional
                                                        let conds = field_inclusion_conditions(field);
                                                        if has_inclusion_conditions(conds) && !swift_type.ends_with('?') {
                                                            swift_type.push('?');
                                                        }
                                                        nested_fields.push((key.clone(), swift_type));
                                                    }
                                                    if let Some(existing) = sibling_entity.iter_mut().find(|(q, _)| q == &nested_qualified) {
                                                        for (key, st) in &nested_fields {
                                                            if !existing.1.iter().any(|(k, _)| k == key) {
                                                                existing.1.push((key.clone(), st.clone()));
                                                            }
                                                        }
                                                    } else if !sibling_entity.iter().any(|(q, _)| q == &nested_qualified) {
                                                        sibling_entity.push((nested_qualified, nested_fields));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(parent_ef) = parent_entity_field {
                        // Build a merged entity type
                        let entity_qualified = format!("{}.{}", child_qualified, entity_struct_name);
                        // Determine if this inline fragment has direct __selections on this field
                        let has_direct = inline_has_field.is_some();

                        // If the inline fragment has its own entity field selections,
                        // add them as a sibling entity for merging
                        let mut all_sibling_entity = sibling_entity.clone();
                        if let Some(inline_ef) = inline_has_field {
                            let mut inline_fields = Vec::new();
                            for (key, field) in &inline_ef.selection_set.direct_selections.fields {
                                if key == "__typename" { continue; }
                                let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                inline_fields.push((key.clone(), swift_type));
                            }
                            // Don't add an OID for this - the inline's entity field is part of AsPet.Height selections
                        }

                        // Check if the CURRENT scope (parent) actually has this entity field
                        // in its direct fields or absorbed inlines (not just via grandparent)
                        let parent_has_this_entity = ds.fields.get(field_key)
                            .map(|f| matches!(f, FieldSelection::Entity(_)))
                            .unwrap_or(false)
                            || absorbed_inline_indices.iter().any(|&abs_idx| {
                                ds.inline_fragments[abs_idx].selection_set.direct_selections.fields.get(field_key)
                                    .map(|f| matches!(f, FieldSelection::Entity(_)))
                                    .unwrap_or(false)
                            });
                        let merged = build_inline_fragment_entity_type(
                            &entity_struct_name,
                            field_key,
                            parent_ef,  // Always pass parent scope's entity field
                            &applicable_frags,
                            &all_sibling_entity,
                            schema_namespace,
                            access_modifier,
                            indent + 4,
                            &entity_qualified,
                            qualified_name,
                            referenced_fragments,
                            type_kinds,
                            is_mutable,
                            has_direct,
                            customizer,
                            inline_has_field, // pass inline's entity field for __selections
                            root_entity_type,
                            parent_has_this_entity,
                            api_target_name,
                            generate_initializers,
                        );
                        let mut merged_config = merged;
                        // If inline fragment has direct selections, add those fields too
                        if let Some(inline_ef) = inline_has_field {
                            for (key, field) in &inline_ef.selection_set.direct_selections.fields {
                                if key == "__typename" { continue; }
                                if !merged_config.config.field_accessors.iter().any(|f| f.name == *key) {
                                    let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                    merged_config.config.field_accessors.push(OwnedFieldAccessor { name: key.clone(), swift_type: swift_type.clone(), description: field.description().map(|s| s.to_string()) });
                                    if let Some(ref mut init) = merged_config.config.initializer {
                                        init.parameters.push(OwnedInitParam {
                                            name: key.clone(), swift_type: swift_type.clone(),
                                            default_value: if swift_type.ends_with('?') { Some("nil".to_string()) } else { None },
                                        });
                                        init.data_entries.push(OwnedDataEntry {
                                            key: key.clone(),
                                            value: OwnedDataEntryValue::Variable(key.clone()),
                                        });
                                    }
                                }
                            }
                        }
                        // Insert entity types BEFORE inline fragment nested types
                        // to match the golden file ordering (entity fields before inline fragments)
                        child_ss.nested_types.insert(0, merged_config);
                        // Update type_alias_insert_index since we added an entity type before inline types
                        child_ss.type_alias_insert_index += 1;
                        // Remove any type alias that conflicts with this nested type
                        child_ss.type_aliases.retain(|ta| ta.name != entity_struct_name);
                    } else if !sibling_entity.is_empty() {
                        // Entity comes from sibling only - use typealias
                        // (This shouldn't normally happen for Height but handle it)
                        for (sib_qualified, _) in &sibling_entity {
                            child_ss.type_aliases.push(OwnedTypeAlias {
                                name: entity_struct_name.clone(),
                                target: sib_qualified.clone(),
                            });
                            break;
                        }
                    } else {
                        // Only from fragments - use typealias to the fragment's entity
                        for frag_name in &applicable_frags {
                            if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                                if frag_arc.root_field.selection_set.direct_selections.fields.contains_key(field_key) {
                                    child_ss.type_aliases.push(OwnedTypeAlias {
                                        name: entity_struct_name.clone(),
                                        target: format!("{}.{}", frag_name, entity_struct_name),
                                    });
                                    break;
                                }
                                for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                                    if let Some(inner) = frag_map.get(sub.fragment_name.as_str()) {
                                        if inner.root_field.selection_set.direct_selections.fields.contains_key(field_key) {
                                            child_ss.type_aliases.push(OwnedTypeAlias {
                                                name: entity_struct_name.clone(),
                                                target: format!("{}.{}", sub.fragment_name, entity_struct_name),
                                            });
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Add type aliases for entity fields from applicable fragments
                // (e.g., Owner = PetDetails.Owner)
                for frag_name in &applicable_frags {
                    if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                        for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                            if let FieldSelection::Entity(ef) = field {
                                // Use singularized name for list fields to match the fragment's entity struct name
                                let entity_type = if ef.field_type.is_list() {
                                    naming::first_uppercased(&naming::singularize(key))
                                } else {
                                    naming::first_uppercased(key)
                                };
                                // Only add if not already handled as nested type or typealias
                                if !child_ss.nested_types.iter().any(|nt| nt.config.struct_name == entity_type)
                                    && !child_ss.type_aliases.iter().any(|ta| ta.name == entity_type)
                                {
                                    child_ss.type_aliases.push(OwnedTypeAlias {
                                        name: entity_type.clone(),
                                        target: format!("{}.{}", frag_name, entity_type),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Add fulfilled fragment OIDs in the correct order.
            // Order: promoted scope OIDs + their fragment OIDs first,
            // then direct sibling inline fragment scope OIDs + their fragment OIDs,
            // then remaining applicable fragments.
            let mut step1_added_promoted = false;
            if let Some(ref mut init) = child_ss.initializer {
                let parent_scope_type = ir_ss.scope.parent_type.name();

                // 1. Process promoted fragments first (e.g., AsWarmBlooded from WarmBloodedDetails)
                let fulfilled_before_step1 = init.fulfilled_fragments.len();
                for frag_name in &applicable_frags {
                    if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                        if pre_promoted_fragment_names.contains(frag_name) {
                            let ftc = &frag_arc.type_condition_name;
                            // Add promoted scope OID
                            let promoted_qualified = format!("{}.As{}", qualified_name, naming::first_uppercased(customizer.custom_type_name(ftc)));
                            if !init.fulfilled_fragments.contains(&promoted_qualified) {
                                init.fulfilled_fragments.push(promoted_qualified);
                            }
                            // Add the fragment itself
                            if type_satisfies_condition(tc, &frag_arc.type_condition_name) {
                                let uc = naming::first_uppercased(frag_name);
                                if !init.fulfilled_fragments.contains(&uc) {
                                    init.fulfilled_fragments.push(uc);
                                }
                            }
                            // Add sub-fragments
                            for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                                let sub_uc = naming::first_uppercased(&sub.fragment_name);
                                if !init.fulfilled_fragments.contains(&sub_uc) {
                                    if let Some(sub_frag) = frag_map.get(sub.fragment_name.as_str()) {
                                        if type_satisfies_condition(tc, &sub_frag.type_condition_name) {
                                            init.fulfilled_fragments.push(sub_uc);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                step1_added_promoted = init.fulfilled_fragments.len() > fulfilled_before_step1;

                // 2. Process each direct sibling inline fragment that is a supertype.
                // Within each sibling, process type-narrowing fragments first (whose type
                // condition differs from the sibling), then matching-type fragments.
                for sibling_inline in &ds.inline_fragments {
                    if let Some(ref sibling_tc) = sibling_inline.type_condition {
                        if sibling_tc.name() != tc.name()
                            && sibling_tc.name() != parent_scope_type
                            && is_supertype_of_current(tc, sibling_tc.name())
                        {
                            // Add scope OID
                            let custom_sibling = customizer.custom_type_name(sibling_tc.name());
                            let sibling_qualified = format!("{}.As{}", qualified_name, naming::first_uppercased(custom_sibling));
                            if !init.fulfilled_fragments.contains(&sibling_qualified)
                                && !sibling_qualified.contains(&format!("As{}.As{}", naming::first_uppercased(custom_sibling), naming::first_uppercased(custom_sibling)))
                            {
                                init.fulfilled_fragments.push(sibling_qualified.clone());
                            }
                            // Pass 1: Add nested promoted OIDs for type-narrowing fragments first
                            for sib_spread in &sibling_inline.selection_set.direct_selections.named_fragments {
                                if let Some(sib_frag) = frag_map.get(sib_spread.fragment_name.as_str()) {
                                    if sib_frag.type_condition_name == sibling_tc.name() { continue; }
                                    if type_satisfies_condition(tc, &sib_frag.type_condition_name) {
                                        let nested_promoted = format!("{}.As{}.As{}", qualified_name, naming::first_uppercased(customizer.custom_type_name(sibling_tc.name())), naming::first_uppercased(customizer.custom_type_name(&sib_frag.type_condition_name)));
                                        if !init.fulfilled_fragments.contains(&nested_promoted) {
                                            init.fulfilled_fragments.push(nested_promoted);
                                        }
                                    }
                                }
                            }
                            // Pass 1: Add fragment OIDs for type-narrowing fragments
                            for sib_spread in &sibling_inline.selection_set.direct_selections.named_fragments {
                                if let Some(sib_frag) = frag_map.get(sib_spread.fragment_name.as_str()) {
                                    if sib_frag.type_condition_name == sibling_tc.name() { continue; }
                                    if type_satisfies_condition(tc, &sib_frag.type_condition_name) {
                                        let uc = naming::first_uppercased(&sib_spread.fragment_name);
                                        if !init.fulfilled_fragments.contains(&uc) {
                                            init.fulfilled_fragments.push(uc);
                                        }
                                        // Also add sub-fragments of narrowing fragments
                                        for sub in &sib_frag.root_field.selection_set.direct_selections.named_fragments {
                                            let sub_uc = naming::first_uppercased(&sub.fragment_name);
                                            if !init.fulfilled_fragments.contains(&sub_uc) {
                                                if let Some(sub_frag) = frag_map.get(sub.fragment_name.as_str()) {
                                                    if type_satisfies_condition(tc, &sub_frag.type_condition_name) {
                                                        init.fulfilled_fragments.push(sub_uc);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Pass 2: Add nested promoted OIDs for matching-type fragments
                            for sib_spread in &sibling_inline.selection_set.direct_selections.named_fragments {
                                if let Some(sib_frag) = frag_map.get(sib_spread.fragment_name.as_str()) {
                                    if sib_frag.type_condition_name != sibling_tc.name() { continue; }
                                    if type_satisfies_condition(tc, &sib_frag.type_condition_name) {
                                        let nested_promoted = format!("{}.As{}.As{}", qualified_name, naming::first_uppercased(customizer.custom_type_name(sibling_tc.name())), naming::first_uppercased(customizer.custom_type_name(&sib_frag.type_condition_name)));
                                        if !init.fulfilled_fragments.contains(&nested_promoted) {
                                            init.fulfilled_fragments.push(nested_promoted);
                                        }
                                    }
                                }
                            }
                            // Pass 2: Add fragment OIDs for matching-type fragments
                            for sib_spread in &sibling_inline.selection_set.direct_selections.named_fragments {
                                if let Some(sib_frag) = frag_map.get(sib_spread.fragment_name.as_str()) {
                                    if sib_frag.type_condition_name != sibling_tc.name() { continue; }
                                    if type_satisfies_condition(tc, &sib_frag.type_condition_name) {
                                        let uc = naming::first_uppercased(&sib_spread.fragment_name);
                                        if !init.fulfilled_fragments.contains(&uc) {
                                            init.fulfilled_fragments.push(uc);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // 3. Add remaining applicable fragment OIDs not yet added.
                // Use the reordered applicable_frags_for_fields to ensure correct ordering
                // (parent fragments with sub-frags before leaf fragments).
                let entity_root_type_name = entity_root_graphql_type
                    .unwrap_or_else(|| ir_ss.scope.parent_type.name());
                let parent_is_union = matches!(inline.selection_set.scope.parent_type, GraphQLCompositeType::Union(_));
                for frag_name in &applicable_frags_for_fields {
                    let should_fulfill = if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                        type_satisfies_condition(tc, &frag_arc.type_condition_name)
                            // For union parents, also include fragments whose type condition
                            // matches the entity root (build_initializer_config skips unions)
                            || (parent_is_union && frag_arc.type_condition_name == entity_root_type_name)
                    } else {
                        true
                    };
                    if should_fulfill && !init.fulfilled_fragments.contains(frag_name) {
                        init.fulfilled_fragments.push(frag_name.clone());
                    }
                }
            }

            // For types that inherit from ancestor scopes (e.g., AsBird inside AsClassroomPet),
            // add fulfilled fragment OIDs for ancestor-level scope paths that the current
            // type satisfies. Also add entity type aliases and nested types from applicable
            // fragments that come from ancestor scopes.
            if !ancestor_fragments.is_empty() {
                if let Some(ref mut init) = child_ss.initializer {
                    // Find the root-level qualified name (e.g., AllAnimalsQuery.Data.AllAnimal)
                    // by looking at the root entity type
                    if let Some(root) = root_entity_type {
                        let root_str = root.to_string();
                        // Add parent scope OID if applicable
                        if !init.fulfilled_fragments.contains(&qualified_name.to_string())
                            && init.fulfilled_fragments.contains(&root_str)
                        {
                            // Insert BEFORE self (child_qualified) to match golden ordering
                            // e.g., AsClassroomPet before AsBird
                            let insert_pos = init.fulfilled_fragments.iter()
                                .position(|f| f == &child_qualified)
                                .unwrap_or(init.fulfilled_fragments.len());
                            init.fulfilled_fragments.insert(insert_pos, qualified_name.to_string());
                        }

                        // For each applicable fragment, check if its type condition maps to
                        // a known scope path at the root level. Insert scope OIDs BEFORE
                        // their corresponding fragment names for correct ordering.
                        // Skip if the fragment's type condition is the same as the root entity's
                        // type (e.g., skip "Animal" when root is AllAnimal on Animal interface).
                        let root_entity_type_name = entity_root_graphql_type.unwrap_or("");

                        // First pass: collect all fragments that are sub-fragments of other applicable fragments
                        let mut sub_fragment_of: std::collections::HashMap<String, String> = std::collections::HashMap::new();
                        for frag_name in &applicable_frags {
                            if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                                for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                                    if applicable_frags_set.contains(sub.fragment_name.as_str()) {
                                        sub_fragment_of.insert(sub.fragment_name.clone(), frag_name.clone());
                                    }
                                }
                            }
                        }

                        // Reorder applicable_frags: parent fragments before their sub-fragments
                        // Move sub-fragments to right after their parent fragment
                        for frag_name in &applicable_frags {
                            if let Some(parent_frag) = sub_fragment_of.get(frag_name) {
                                // This fragment is a sub-fragment. Ensure it comes after its parent.
                                let self_pos = init.fulfilled_fragments.iter().position(|f| f == frag_name);
                                let parent_pos = init.fulfilled_fragments.iter().position(|f| f == parent_frag);
                                if let (Some(sp), Some(pp)) = (self_pos, parent_pos) {
                                    if sp < pp {
                                        // Sub-fragment is before parent - move it after parent
                                        let removed = init.fulfilled_fragments.remove(sp);
                                        // Recalculate parent position after removal
                                        let new_pp = init.fulfilled_fragments.iter().position(|f| f == parent_frag).unwrap_or(init.fulfilled_fragments.len());
                                        init.fulfilled_fragments.insert(new_pp + 1, removed);
                                    }
                                }
                            }
                        }

                        for frag_name in &applicable_frags {
                            if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                                let ftc = &frag_arc.type_condition_name;
                                // Skip if this fragment's type is the root entity type itself
                                if *ftc == root_entity_type_name { continue; }
                                // Add root-level scope OID (e.g., AllAnimal.AsWarmBlooded)
                                // Insert BEFORE the fragment name for correct ordering.
                                // BUT skip if the root scope doesn't have a promoted inline
                                // fragment for this type (e.g., WarmBlooded is conditional at root).
                                // Check: does the root scope have an unconditional promoted fragment
                                // for this type condition? It does if there's an entry in early_promoted_types
                                // matching ftc. If not (because the spread is conditional), skip.
                                let root_has_promoted = early_promoted_types.iter().any(|t| t == ftc)
                                    || parent_scope_ds.map_or(false, |pds| {
                                        // Also check grandparent scope: if it has a non-conditional
                                        // named fragment spread whose type condition matches ftc,
                                        // then the grandparent promoted that fragment to an inline
                                        // fragment (e.g., ...WarmBloodedDetails at root creates AsWarmBlooded).
                                        pds.named_fragments.iter().any(|spread| {
                                            !has_inclusion_conditions(spread.inclusion_conditions.as_ref())
                                            && referenced_fragments.iter().any(|f| {
                                                f.name == spread.fragment_name && f.type_condition_name == *ftc
                                            })
                                        })
                                    });
                                // Also check if there's a direct (non-conditional) inline fragment at the root
                                let root_has_direct = if let Some(pds) = parent_scope_ds {
                                    pds.inline_fragments.iter().any(|inf| {
                                        inf.type_condition.as_ref().map(|tc2| tc2.name() == ftc.as_str()).unwrap_or(false)
                                            && !has_inclusion_conditions(inf.inclusion_conditions.as_ref())
                                    })
                                } else {
                                    ds.inline_fragments.iter().any(|inf| {
                                        inf.type_condition.as_ref().map(|tc2| tc2.name() == ftc.as_str()).unwrap_or(false)
                                            && !has_inclusion_conditions(inf.inclusion_conditions.as_ref())
                                    })
                                };
                                let root_scope = format!("{}.As{}", root_str, naming::first_uppercased(customizer.custom_type_name(ftc)));
                                if (root_has_promoted || root_has_direct) && !init.fulfilled_fragments.contains(&root_scope) && type_satisfies_condition(tc, ftc) {
                                    let insert_pos = init.fulfilled_fragments.iter()
                                        .position(|f| f == frag_name)
                                        .unwrap_or(init.fulfilled_fragments.len());
                                    init.fulfilled_fragments.insert(insert_pos, root_scope.clone());
                                }

                                // Add nested promoted OIDs (e.g., AllAnimal.AsPet.AsWarmBlooded)
                                for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                                    if let Some(sub_frag) = frag_map.get(sub.fragment_name.as_str()) {
                                        // Skip if sub-fragment type matches root entity type
                                        if sub_frag.type_condition_name == root_entity_type_name { continue; }
                                        if type_satisfies_condition(tc, &sub_frag.type_condition_name) {
                                            let nested_scope = format!("{}.As{}.As{}", root_str, naming::first_uppercased(customizer.custom_type_name(ftc)), naming::first_uppercased(customizer.custom_type_name(&sub_frag.type_condition_name)));
                                            if !init.fulfilled_fragments.contains(&nested_scope) {
                                                init.fulfilled_fragments.push(nested_scope);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Also check parent scope's sibling inline fragments for promoted
                // fragment scopes (e.g., AsPet.AsWarmBlooded from AsPet spreading WarmBloodedDetails)
                if let Some(ref mut init) = child_ss.initializer {
                    if let Some(pds) = parent_scope_ds {
                        if let Some(root) = root_entity_type {
                            let root_str = root.to_string();
                            for pds_inline in &pds.inline_fragments {
                                if let Some(ref pds_tc) = pds_inline.type_condition {
                                    if is_supertype_of_current(tc, pds_tc.name()) || tc.name() == pds_tc.name() {
                                        let pds_scope = format!("{}.As{}", root_str, naming::first_uppercased(customizer.custom_type_name(pds_tc.name())));
                                        // Check this sibling's named fragment spreads for promoted fragments
                                        for sib_spread in &pds_inline.selection_set.direct_selections.named_fragments {
                                            if let Some(sib_frag) = frag_map.get(sib_spread.fragment_name.as_str()) {
                                                let sib_ftc = &sib_frag.type_condition_name;
                                                if *sib_ftc != pds_tc.name().to_string()
                                                    && type_satisfies_condition(tc, sib_ftc)
                                                {
                                                    let promoted_scope = format!("{}.As{}", pds_scope, naming::first_uppercased(customizer.custom_type_name(sib_ftc)));
                                                    if !init.fulfilled_fragments.contains(&promoted_scope) {
                                                        // Insert after AsPet scope OID, before remaining fragments
                                                        let insert_pos = init.fulfilled_fragments.iter()
                                                            .position(|f| f == &pds_scope)
                                                            .map(|p| p + 1)
                                                            .unwrap_or(init.fulfilled_fragments.len());
                                                        init.fulfilled_fragments.insert(insert_pos, promoted_scope);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Add entity type aliases from applicable ancestor fragments
                for frag_name in &applicable_frags {
                    if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                        for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                            if let FieldSelection::Entity(ef) = field {
                                // Use singularized name for list fields to match the fragment's entity struct name
                                let entity_type = if ef.field_type.is_list() {
                                    naming::first_uppercased(&naming::singularize(key))
                                } else {
                                    naming::first_uppercased(key)
                                };
                                if !child_ss.nested_types.iter().any(|nt| nt.config.struct_name == entity_type)
                                    && !child_ss.type_aliases.iter().any(|ta| ta.name == entity_type)
                                {
                                    child_ss.type_aliases.push(OwnedTypeAlias {
                                        name: entity_type.clone(),
                                        target: format!("{}.{}", frag_name, entity_type),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Post-process fulfilled fragments: when step 3 added fragment OIDs that should
            // come after scope OIDs from the ancestor handling. This only applies when
            // step 1 didn't add promoted fragment OIDs (which would ensure correct ordering).
            // Post-process fulfilled fragments: move "orphaned" bare fragment names
            // (those without a preceding scope OID) to after the last scope OID in the
            // ancestor region. This handles cases like AllAnimalsIncludeSkipQuery's AsBird
            // where WBD/HIM appear before AsPet but should come after AsPet.AsWarmBlooded.
            if !ancestor_fragments.is_empty() && !step1_added_promoted {
                if let Some(ref mut init) = child_ss.initializer {
                    let child_pos = init.fulfilled_fragments.iter()
                        .position(|f| f == &child_qualified)
                        .unwrap_or(0);
                    let ancestor_start = child_pos + 1;
                    if ancestor_start < init.fulfilled_fragments.len() {
                        // Find bare names that appear before the first scope OID in the ancestor region
                        let first_scope_in_ancestor = init.fulfilled_fragments[ancestor_start..].iter()
                            .position(|f| f.contains('.'))
                            .map(|p| p + ancestor_start);
                        if let Some(scope_pos) = first_scope_in_ancestor {
                            // Collect bare names between ancestor_start and scope_pos
                            let mut orphaned_bares: Vec<(usize, String)> = Vec::new();
                            for i in ancestor_start..scope_pos {
                                if !init.fulfilled_fragments[i].contains('.') {
                                    orphaned_bares.push((i, init.fulfilled_fragments[i].clone()));
                                }
                            }
                            if !orphaned_bares.is_empty() {
                                // Find the last scope OID in the ancestor region
                                let last_scope = init.fulfilled_fragments[ancestor_start..].iter()
                                    .rposition(|f| f.contains('.'))
                                    .map(|p| p + ancestor_start)
                                    .unwrap_or(scope_pos);
                                // Remove orphaned bares from back to front
                                for (idx, _) in orphaned_bares.iter().rev() {
                                    init.fulfilled_fragments.remove(*idx);
                                }
                                // Recalculate last_scope after removals
                                let new_last_scope = init.fulfilled_fragments[ancestor_start..].iter()
                                    .rposition(|f| f.contains('.'))
                                    .map(|p| p + ancestor_start)
                                    .unwrap_or(ancestor_start);
                                // Insert after the last scope OID
                                for (i, (_, bare)) in orphaned_bares.iter().enumerate() {
                                    init.fulfilled_fragments.insert(new_last_scope + 1 + i, bare.clone());
                                }
                            }
                        }
                    }
                }
            }

            // Update initializer data entries for entity fields to use _fieldData
            if let Some(ref mut init) = child_ss.initializer {
                for entry in init.data_entries.iter_mut() {
                    if let OwnedDataEntryValue::Variable(ref name) = entry.value {
                        let name_clone = name.clone();
                        // Check if this is an entity field
                        let is_entity_field = ds.fields.get(&name_clone)
                            .map(|f| matches!(f, FieldSelection::Entity(_)))
                            .unwrap_or(false)
                            || applicable_frags.iter().any(|frag_name| {
                                frag_map.get(frag_name.as_str())
                                    .map(|frag_arc| {
                                        frag_arc.root_field.selection_set.direct_selections.fields.get(&name_clone)
                                            .map(|f| matches!(f, FieldSelection::Entity(_)))
                                            .unwrap_or(false)
                                    })
                                    .unwrap_or(false)
                            })
                            || referenced_fragments.iter().any(|frag_arc| {
                                frag_arc.root_field.selection_set.direct_selections.fields.get(&name_clone)
                                    .map(|f| matches!(f, FieldSelection::Entity(_)))
                                    .unwrap_or(false)
                            });
                        if is_entity_field {
                            entry.value = OwnedDataEntryValue::FieldData(name_clone);
                        }
                    }
                }
            }

            // Use the qualified_name to build a doc comment path.
            // For root selection sets (Data in operations, fragment roots), use just the type name.
            // For nested scopes, use the full entity-relative path.
            let doc_comment = if is_root {
                format!("/// {}", type_name)
            } else {
                let doc_prefix = if let Some(pos) = qualified_name.find(".Data.") {
                    &qualified_name[pos + 6..]
                } else {
                    struct_name
                };
                format!("/// {}.{}", doc_prefix, type_name)
            };
            nested_types.push(OwnedNestedSelectionSet {
                doc_comment,
                parent_type_comment: format!(
                    "///\n{}/// Parent Type: `{}`",
                    " ".repeat(indent + 2),
                    customizer.custom_type_name(tc.name())
                ),
                config: child_ss,
            });
        } else if inline.type_condition.is_none() && has_inclusion_conditions(inline.inclusion_conditions.as_ref()) {
            // Conditional inline fragment WITHOUT a type condition
            // e.g., `... @include(if: $includeDetails) { name appearsIn }`
            // These generate IfVariableName wrapper structs.
            let ic = inline.inclusion_conditions.as_ref().expect("inline fragment missing expected inclusion conditions");
            let type_name = conditional_inline_fragment_name(None, ic, customizer);
            let property_name = conditional_inline_fragment_property(None, ic, customizer);
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

            // Add the selection item
            let (entries, op) = inclusion_conditions_to_owned(ic);
            selections.push(OwnedSelectionItem {
                kind: OwnedSelectionKind::ConditionalInlineFragment {
                    conditions: entries,
                    operator: op,
                    type_name: type_name.clone(),
                },
            });

            // Add the inline fragment accessor
            inline_fragment_accessors.push(OwnedInlineFragmentAccessor {
                property_name: property_name.clone(),
                type_name: type_name.clone(),
            });

            // Use the parent type as the type condition for this conditional wrapper
            let parent_type_name = customizer.custom_type_name(ir_ss.scope.parent_type.name()).to_string();

            // Build child selection set
            let child_ss = build_selection_set_config_owned(
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
                Some(&field_accessors), // pass parent field accessors
                is_mutable,
                customizer,
                entity_root_graphql_type.or(Some(ir_ss.scope.parent_type.name())),
                ancestor_fragments,
                Some(ds),
                inline.inclusion_conditions.as_ref(),
                api_target_name,
            );

            let doc_comment = if is_root {
                format!("/// {}", type_name)
            } else {
                let doc_prefix = if let Some(pos) = qualified_name.find(".Data.") {
                    &qualified_name[pos + 6..]
                } else {
                    struct_name
                };
                format!("/// {}.{}", doc_prefix, type_name)
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

    // Promote inline fragments from spread fragments.
    let current_parent_type_name = ir_ss.scope.parent_type.name().to_string();
    let direct_inline_type_names: Vec<String> = ds
        .inline_fragments
        .iter()
        .enumerate()
        .filter(|(idx, _)| !absorbed_inline_indices.contains(idx))
        .filter_map(|(_, inline)| inline.type_condition.as_ref().map(|tc| tc.name().to_string()))
        .collect();

    // Track fragments that get promoted to inline fragments (type narrowing).
    // These should be removed from the parent scope's selections/fragments/fields.
    let mut promoted_fragment_names: Vec<String> = Vec::new();

    for spread in &ds.named_fragments {
        if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
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
            {
                // Skip if this was NOT pre-computed as a promoted fragment
                // (e.g., already handled by a direct inline fragment for the same type)
                if !early_promoted_fragment_names.contains(&spread.fragment_name) { continue; }
                promoted_fragment_names.push(spread.fragment_name.clone());
                let type_name = format!("As{}", naming::first_uppercased(customizer.custom_type_name(frag_type_condition)));
                let child_qualified = format!("{}.{}", qualified_name, type_name);
                let child_root_entity = if is_root { qualified_name.to_string() } else { root_entity_type.unwrap_or(qualified_name).to_string() };

                // Selection and accessor were already added early in the function (before direct inline fragments)

                // Build field accessors for the promoted inline fragment.
                // Order: root entity fields (in parent scope order) first,
                // then the promoted fragment's own fields,
                // then non-promoted applicable fragment fields.
                let mut pfa = Vec::new();

                // 1. Root entity fields from the parent scope (e.g., AllAnimal's fields
                //    via parent_scope_ds, or the current scope's parent-inherited fields).
                //    Use parent_scope_ds if available to get the root entity field order.
                if let Some(parent_ds) = parent_scope_ds {
                    // Add fields from the parent scope's direct fields (root entity order)
                    // Fields with inclusion conditions become optional types.
                    for (key, field) in &parent_ds.fields {
                        if key == "__typename" { continue; }
                        if !pfa.iter().any(|f: &OwnedFieldAccessor| f.name == *key) {
                            let (mut swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                            let conds = field_inclusion_conditions(field);
                            if has_inclusion_conditions(conds) && !swift_type.ends_with('?') {
                                swift_type.push('?');
                            }
                            pfa.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                        }
                    }
                } else {
                    // Fallback: use the current scope's field_accessors.
                    // Determine ordering: if the fragment's fields have NO overlap with
                    // field_accessors, put fragment fields first (they're the primary content).
                    // If there IS overlap, keep parent-first order (fields are shared/merged).
                    let mut frag_field_names: Vec<String> = frag_ds.fields.keys().cloned().collect();
                    for fs in &frag_ds.named_fragments {
                        if let Some(inner) = frag_map.get(fs.fragment_name.as_str()) {
                            for key in inner.root_field.selection_set.direct_selections.fields.keys() {
                                frag_field_names.push(key.clone());
                            }
                        }
                    }
                    let has_overlap = field_accessors.iter().any(|fa| frag_field_names.contains(&fa.name));

                    if has_overlap {
                        // Parent-first: field_accessors first, then fragment fields
                        for fa in &field_accessors { pfa.push(fa.clone()); }
                    }
                    // Fragment fields will be added in step 2 below.
                    // If no overlap, pfa is empty here and fragment fields go first.
                }

                // 2. Add the promoted fragment's own fields
                for (key, field) in &frag_ds.fields {
                    if !pfa.iter().any(|f| f.name == *key) {
                        let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                        pfa.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                    }
                }
                // Add sub-fragment fields
                for fs in &frag_ds.named_fragments {
                    if let Some(inner) = frag_map.get(fs.fragment_name.as_str()) {
                        for (key, field) in &inner.root_field.selection_set.direct_selections.fields {
                            if !pfa.iter().any(|f| f.name == *key) {
                                let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                pfa.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                            }
                        }
                    }
                }

                // 2b. If no overlap, add parent fields AFTER fragment fields
                if !parent_scope_ds.is_some() {
                    let has_overlap_check = {
                        let mut frag_fn: Vec<String> = frag_ds.fields.keys().cloned().collect();
                        for fs in &frag_ds.named_fragments {
                            if let Some(inner) = frag_map.get(fs.fragment_name.as_str()) {
                                for key in inner.root_field.selection_set.direct_selections.fields.keys() {
                                    frag_fn.push(key.clone());
                                }
                            }
                        }
                        field_accessors.iter().any(|fa| frag_fn.contains(&fa.name))
                    };
                    if !has_overlap_check {
                        for fa in &field_accessors {
                            if !pfa.iter().any(|f| f.name == fa.name) {
                                pfa.push(fa.clone());
                            }
                        }
                    }
                }

                // 3. Add absorbed inline fragment fields that aren't yet included
                for &abs_idx in &absorbed_inline_indices {
                    let abs_inline = &ds.inline_fragments[abs_idx];
                    for (key, field) in &abs_inline.selection_set.direct_selections.fields {
                        if key == "__typename" { continue; }
                        if !pfa.iter().any(|f: &OwnedFieldAccessor| f.name == *key) {
                            let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                            pfa.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                        }
                    }
                }

                let mut pfs = vec![OwnedFragmentSpreadAccessor {
                    property_name: naming::first_lowercased(&spread.fragment_name),
                    fragment_type: spread.fragment_name.clone(),
                    is_optional: false,
                }];
                for fs in &frag_ds.named_fragments {
                    pfs.push(OwnedFragmentSpreadAccessor {
                        property_name: naming::first_lowercased(&fs.fragment_name),
                        fragment_type: fs.fragment_name.clone(),
                        is_optional: false,
                    });
                }

                // 4. Add parent scope's non-promoted fragments that are applicable to this promoted type.
                // E.g., when AsPet.AsWarmBlooded is promoted from WarmBloodedDetails, PetDetails
                // (non-promoted on AsPet) should also be included if Pet is satisfied by WarmBlooded's scope.
                // Since we're inside AsPet (which guarantees Pet), PetDetails is always applicable.
                for parent_spread in &ds.named_fragments {
                    if parent_spread.fragment_name == spread.fragment_name { continue; }
                    if promoted_fragment_names.contains(&parent_spread.fragment_name) { continue; }
                    let parent_uc = naming::first_uppercased(&parent_spread.fragment_name);
                    if pfs.iter().any(|fs| fs.fragment_type == parent_uc) { continue; }
                    pfs.push(OwnedFragmentSpreadAccessor {
                        property_name: naming::first_lowercased(&parent_spread.fragment_name),
                        fragment_type: parent_uc,
                        is_optional: false,
                    });
                    // Also add fields from this parent fragment
                    if let Some(parent_frag) = frag_map.get(parent_spread.fragment_name.as_str()) {
                        for (key, field) in &parent_frag.root_field.selection_set.direct_selections.fields {
                            if !pfa.iter().any(|f| f.name == *key) {
                                let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                pfa.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                            }
                        }
                    }
                }

                // Collect parent scope's non-promoted fragment names
                let parent_nonpromoted: Vec<String> = ds.named_fragments.iter()
                    .filter(|s| !promoted_fragment_names.contains(&s.fragment_name) && s.fragment_name != spread.fragment_name)
                    .map(|s| s.fragment_name.clone())
                    .collect();
                let pinit = if generate_initializers {
                    Some(build_promoted_initializer(
                        &frag_arc.root_field.selection_set.scope.parent_type,
                        &pfa, schema_namespace, &child_qualified, &child_root_entity,
                        &spread.fragment_name, &frag_ds.named_fragments, referenced_fragments,
                        customizer, ds,
                        qualified_name,
                        &parent_nonpromoted,
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
                    if let Some(inner) = frag_map.get(fs.fragment_name.as_str()) {
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
                    // Determine if this entity field is a list type
                    let is_list_field = frag_ds.fields.get(key)
                        .and_then(|f| if let FieldSelection::Entity(ef) = f { Some(ef.field_type.is_list()) } else { None })
                        .or_else(|| {
                            ds.fields.get(key)
                                .and_then(|f| if let FieldSelection::Entity(ef) = f { Some(ef.field_type.is_list()) } else { None })
                        })
                        .or_else(|| {
                            referenced_fragments.iter().find_map(|frag| {
                                frag.root_field.selection_set.direct_selections.fields.get(key)
                                    .and_then(|f| if let FieldSelection::Entity(ef) = f { Some(ef.field_type.is_list()) } else { None })
                            })
                        })
                        .unwrap_or(true); // default to singularize (list behavior) if not found
                    let child_struct_name = if is_list_field {
                        naming::first_uppercased(&naming::singularize(key))
                    } else {
                        naming::first_uppercased(key)
                    };
                    // Check if the parent scope has the same entity field
                    let parent_has_field = ds.fields.get(key).map(|f| matches!(f, FieldSelection::Entity(_))).unwrap_or(false);
                    // Also check if the field is inherited from a higher scope
                    let inherited_field = !parent_has_field && field_accessors.iter().any(|fa| fa.name == *key);
                    if parent_has_field || inherited_field {
                        // Find the best entity field source - direct, from parent scope, from absorbed inline, or from fragment.
                        // Prefer the parent scope's entity field (e.g., AllAnimal.height with feet/inches)
                        // over fragments (e.g., HeightInMeters.height with only meters).
                        let entity_field_source = if let Some(FieldSelection::Entity(ef)) = ds.fields.get(key) {
                            Some(ef)
                        } else if let Some(parent_ds) = parent_scope_ds {
                            // Check parent scope's direct entity fields first (has the most complete field set)
                            parent_ds.fields.get(key).and_then(|f| {
                                if let FieldSelection::Entity(ef) = f { Some(ef) } else { None }
                            })
                        } else {
                            None
                        }.or_else(|| {
                            // Try absorbed inline fragments (they contain entity fields from absorbed scopes)
                            absorbed_inline_indices.iter().find_map(|&idx| {
                                ds.inline_fragments[idx].selection_set.direct_selections.fields.get(key).and_then(|f| {
                                    if let FieldSelection::Entity(ef) = f { Some(ef) } else { None }
                                })
                            })
                        }).or_else(|| {
                            // Fallback: try to find from referenced fragments
                            referenced_fragments.iter().find_map(|frag| {
                                frag.root_field.selection_set.direct_selections.fields.get(key).and_then(|f| {
                                    if let FieldSelection::Entity(ef) = f { Some(ef) } else { None }
                                })
                            })
                        });
                        if let Some(parent_ef) = entity_field_source {
                            let entity_qualified = format!("{}.{}", child_qualified, child_struct_name);
                            // Collect applicable fragments for this promoted inline fragment
                            let case1_applicable = collect_applicable_fragments(
                                &frag_arc.root_field.selection_set.scope.parent_type,
                                &current_parent_type_name,
                                ds,
                                &promoted_fragment_names,
                                referenced_fragments,
                                &ir_ss.scope.parent_type,
                                ancestor_fragments,
                            );
                            // Also include the source fragment's sub-fragments
                            let mut all_applicable = case1_applicable.clone();
                            if !all_applicable.contains(source_frag) {
                                all_applicable.push(source_frag.clone());
                            }
                            for fs in &frag_ds.named_fragments {
                                if !all_applicable.contains(&fs.fragment_name) {
                                    all_applicable.push(fs.fragment_name.clone());
                                }
                            }

                            // Collect sibling entity fields from inline fragments
                            let mut case1_sibling = Vec::new();
                            for (sib_idx, sibling_inline) in ds.inline_fragments.iter().enumerate() {
                                if let Some(ref sibling_tc) = sibling_inline.type_condition {
                                    if is_supertype_of_current(&frag_arc.root_field.selection_set.scope.parent_type, sibling_tc.name())
                                        || frag_arc.root_field.selection_set.scope.parent_type.name() == sibling_tc.name() {
                                        if let Some(FieldSelection::Entity(sib_ef)) = sibling_inline.selection_set.direct_selections.fields.get(key) {
                                            // If sibling is absorbed, use parent scope qualified name directly
                                            let sib_qualified = if absorbed_inline_indices.contains(&sib_idx) {
                                                format!("{}.{}", qualified_name, child_struct_name)
                                            } else {
                                                format!("{}.As{}.{}", qualified_name, naming::first_uppercased(sibling_tc.name()), child_struct_name)
                                            };
                                            let mut sib_fields = Vec::new();
                                            for (fk, ff) in &sib_ef.selection_set.direct_selections.fields {
                                                if fk == "__typename" { continue; }
                                                let (mut swift_type, _) = render_field_swift_type(ff, schema_namespace, type_kinds, customizer);
                                                // Make conditional fields optional
                                                let conds = field_inclusion_conditions(ff);
                                                if has_inclusion_conditions(conds) && !swift_type.ends_with('?') {
                                                    swift_type.push('?');
                                                }
                                                sib_fields.push((fk.clone(), swift_type));
                                            }
                                            case1_sibling.push((sib_qualified, sib_fields));
                                        }
                                        // Also check nested inline fragments
                                        for nested in &sibling_inline.selection_set.direct_selections.inline_fragments {
                                            if let Some(ref nested_tc) = nested.type_condition {
                                                if is_supertype_of_current(&frag_arc.root_field.selection_set.scope.parent_type, nested_tc.name())
                                                    || frag_arc.root_field.selection_set.scope.parent_type.name() == nested_tc.name() {
                                                    if let Some(FieldSelection::Entity(nested_ef)) = nested.selection_set.direct_selections.fields.get(key) {
                                                        let nested_qualified = format!("{}.As{}.{}", qualified_name, naming::first_uppercased(sibling_tc.name()), child_struct_name);
                                                        let mut nested_fields = Vec::new();
                                                        for (fk, ff) in &nested_ef.selection_set.direct_selections.fields {
                                                            if fk == "__typename" { continue; }
                                                            let (swift_type, _) = render_field_swift_type(ff, schema_namespace, type_kinds, customizer);
                                                            nested_fields.push((fk.clone(), swift_type));
                                                        }
                                                        if let Some(existing) = case1_sibling.iter_mut().find(|(q, _)| q == &nested_qualified) {
                                                            for (fk, st) in &nested_fields {
                                                                if !existing.1.iter().any(|(k, _)| k == fk) {
                                                                    existing.1.push((fk.clone(), st.clone()));
                                                                }
                                                            }
                                                        } else {
                                                            case1_sibling.push((nested_qualified, nested_fields));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Check if the current scope actually has this entity field
                            let parent_has = ds.fields.get(key)
                                .map(|f| matches!(f, FieldSelection::Entity(_)))
                                .unwrap_or(false)
                                || absorbed_inline_indices.iter().any(|&abs_idx| {
                                    ds.inline_fragments[abs_idx].selection_set.direct_selections.fields.get(key)
                                        .map(|f| matches!(f, FieldSelection::Entity(_)))
                                        .unwrap_or(false)
                                });
                            let merged_struct = build_inline_fragment_entity_type(
                                &child_struct_name,
                                key,
                                parent_ef,
                                &all_applicable,
                                &case1_sibling,
                                schema_namespace,
                                access_modifier,
                                indent + 4,
                                &entity_qualified,
                                qualified_name,
                                referenced_fragments,
                                type_kinds,
                                is_mutable,
                                false,
                                customizer,
                                None,
                                root_entity_type,
                                parent_has,
                                api_target_name,
                                generate_initializers,
                            );
                            pnt.push(merged_struct);
                        } else {
                            pta.push(OwnedTypeAlias { name: n.clone(), target: format!("{}.{}", source_frag, n) });
                        }
                    } else {
                        pta.push(OwnedTypeAlias { name: n.clone(), target: format!("{}.{}", source_frag, n) });
                    }
                }

                // Add entity field type aliases from parent non-promoted fragments
                // E.g., Owner = PetDetails.Owner when PetDetails is a parent scope fragment
                for parent_frag_name in &parent_nonpromoted {
                    if let Some(parent_frag) = frag_map.get(parent_frag_name.as_str()) {
                        for (key, field) in &parent_frag.root_field.selection_set.direct_selections.fields {
                            if matches!(field, FieldSelection::Entity(_)) {
                                let entity_type = naming::first_uppercased(key);
                                if !pnt.iter().any(|nt| nt.config.struct_name == entity_type)
                                    && !pta.iter().any(|ta| ta.name == entity_type)
                                {
                                    pta.push(OwnedTypeAlias {
                                        name: entity_type,
                                        target: format!("{}.{}", parent_frag_name, naming::first_uppercased(key)),
                                    });
                                }
                            }
                        }
                    }
                }

                let frag_parent_type = match &frag_arc.root_field.selection_set.scope.parent_type {
                    GraphQLCompositeType::Object(o) => OwnedParentTypeRef::Object(customizer.custom_type_name(&o.name).to_string()),
                    GraphQLCompositeType::Interface(i) => OwnedParentTypeRef::Interface(customizer.custom_type_name(&i.name).to_string()),
                    GraphQLCompositeType::Union(u) => OwnedParentTypeRef::Union(customizer.custom_type_name(&u.name).to_string()),
                };
                let ic = if is_mutable { SelectionSetConformance::MutableInlineFragment } else { SelectionSetConformance::InlineFragment };
                let pnt_len = pnt.len();
                let pss = OwnedSelectionSetConfig {
                    struct_name: type_name.clone(), schema_namespace: schema_namespace.to_string(),
                    parent_type: frag_parent_type, is_root: false, is_inline_fragment: true,
                    conformance: ic, root_entity_type: Some(child_root_entity.clone()),
                    merged_sources: vec![], selections: vec![OwnedSelectionItem { kind: OwnedSelectionKind::Fragment(naming::first_uppercased(&spread.fragment_name)) }],
                    field_accessors: pfa, inline_fragment_accessors: vec![],
                    fragment_spreads: pfs, initializer: pinit,
                    nested_types: pnt, type_aliases: pta, type_alias_insert_index: pnt_len,
                    indent: indent + 2, access_modifier: access_modifier.to_string(), is_mutable, absorbed_type_names: vec![], api_target_name: api_target_name.to_string(),
                };
                let dc = if is_root {
                    format!("/// {}", type_name)
                } else {
                    let doc_prefix = if let Some(pos) = qualified_name.find(".Data.") {
                        &qualified_name[pos + 6..]
                    } else {
                        struct_name
                    };
                    format!("/// {}.{}", doc_prefix, type_name)
                };
                // Insert promoted nested types at the correct position
                // (after entity fields, before direct inline fragments)
                nested_types.insert(promoted_insert_index, OwnedNestedSelectionSet {
                    doc_comment: dc,
                    parent_type_comment: format!("///\n{}/// Parent Type: `{}`", " ".repeat(indent + 2), customizer.custom_type_name(frag_type_condition)),
                    config: pss,
                });
                promoted_insert_index += 1;
            }

            // Case 2: Fragment contains inline fragments - promote them as CompositeInlineFragment
            // Skip conditional fragment spreads — their inline fragments should not be promoted
            // to the parent scope (they belong inside the conditional wrapper).
            if has_inclusion_conditions(spread.inclusion_conditions.as_ref()) { continue; }
            for frag_inline in &frag_ds.inline_fragments {
                if let Some(ref tc) = frag_inline.type_condition {
                    let tc_name = tc.name().to_string();
                    let custom_tc_name = customizer.custom_type_name(&tc_name).to_string();
                    if direct_inline_type_names.contains(&tc_name) { continue; }
                    if inline_fragment_accessors.iter().any(|a| a.type_name == format!("As{}", naming::first_uppercased(&custom_tc_name))) { continue; }

                    let type_name = format!("As{}", naming::first_uppercased(&custom_tc_name));
                    let child_qualified = format!("{}.{}", qualified_name, type_name);
                    let child_root_entity = if is_root { qualified_name.to_string() } else { root_entity_type.unwrap_or(qualified_name).to_string() };

                    inline_fragment_accessors.push(OwnedInlineFragmentAccessor {
                        property_name: format!("as{}", naming::first_uppercased(&custom_tc_name)),
                        type_name: type_name.clone(),
                    });

                    let ppt = match tc {
                        GraphQLCompositeType::Object(o) => OwnedParentTypeRef::Object(customizer.custom_type_name(&o.name).to_string()),
                        GraphQLCompositeType::Interface(i) => OwnedParentTypeRef::Interface(customizer.custom_type_name(&i.name).to_string()),
                        GraphQLCompositeType::Union(u) => OwnedParentTypeRef::Union(customizer.custom_type_name(&u.name).to_string()),
                    };

                    // Build merged_sources: include self, fragment root (if it has direct fields),
                    // sibling supertype inline fragments, and the fragment's own inline fragment type.
                    let mut ms = vec![qualified_name.to_string()];
                    // Add the fragment root itself (e.g. HeroDetails.self) only if the fragment
                    // has direct field selections (not just inline fragments).
                    let frag_has_direct_fields = frag_ds.fields.iter()
                        .any(|(key, _)| key != "__typename");
                    if frag_has_direct_fields {
                        ms.push(spread.fragment_name.clone());
                    }
                    // Add sibling supertype inline fragments from the fragment
                    for other_frag_inline in &frag_ds.inline_fragments {
                        if let Some(ref other_tc) = other_frag_inline.type_condition {
                            let other_name = other_tc.name();
                            if other_name != tc_name && is_supertype_of_current(tc, other_name) {
                                ms.push(format!("{}.As{}", spread.fragment_name, naming::first_uppercased(customizer.custom_type_name(other_name))));
                            }
                        }
                    }
                    // Add the fragment's own inline fragment type last
                    ms.push(format!("{}.{}", spread.fragment_name, type_name));

                    // Build field accessors following merged_sources order:
                    // 1. Parent inherited fields (from the enclosing selection set)
                    // 2. Fields from sibling supertype inline fragments (in merged_sources order)
                    // 3. Own direct fields from this inline fragment
                    let mut pfa = Vec::new();
                    // First: parent inherited fields
                    for fa in &field_accessors {
                        if !pfa.iter().any(|f: &OwnedFieldAccessor| f.name == fa.name) { pfa.push(fa.clone()); }
                    }
                    // Then: fields from sibling supertype inline fragments within the fragment
                    for other_frag_inline in &frag_ds.inline_fragments {
                        if let Some(ref other_tc) = other_frag_inline.type_condition {
                            let other_name = other_tc.name();
                            if other_name != tc_name && is_supertype_of_current(tc, other_name) {
                                for (key, field) in &other_frag_inline.selection_set.direct_selections.fields {
                                    if !pfa.iter().any(|f: &OwnedFieldAccessor| f.name == *key) {
                                        let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                        pfa.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                                    }
                                }
                            }
                        }
                    }
                    // Finally: own direct fields from this inline fragment
                    for (key, field) in &frag_inline.selection_set.direct_selections.fields {
                        if !pfa.iter().any(|f| f.name == *key) {
                            let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                            pfa.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                        }
                    }

                    let mut pfs = vec![OwnedFragmentSpreadAccessor {
                        property_name: naming::first_lowercased(&spread.fragment_name),
                        fragment_type: spread.fragment_name.clone(),
                        is_optional: false,
                    }];

                    // Build fulfilled fragments for the initializer, including supertype inline fragment OIDs
                    let frag_uc = naming::first_uppercased(&spread.fragment_name);
                    let mut extra_frag_fulfilled = Vec::new();
                    for other_frag_inline in &frag_ds.inline_fragments {
                        if let Some(ref other_tc) = other_frag_inline.type_condition {
                            let other_name = other_tc.name();
                            if other_name != tc_name && is_supertype_of_current(tc, other_name) {
                                extra_frag_fulfilled.push(format!("{}.As{}", frag_uc, naming::first_uppercased(customizer.custom_type_name(other_name))));
                            }
                        }
                    }
                    // Add the fragment's own inline fragment type
                    extra_frag_fulfilled.push(format!("{}.{}", frag_uc, type_name));

                    // Collect applicable fragments for this Case 2 promoted inline fragment
                    let case2_applicable = collect_applicable_fragments(
                        tc,
                        &current_parent_type_name,
                        ds,
                        &promoted_fragment_names,
                        referenced_fragments,
                        &ir_ss.scope.parent_type,
                        ancestor_fragments,
                    );

                    // Add additional applicable fragment spreads
                    for frag_name in &case2_applicable {
                        if !pfs.iter().any(|fs| fs.fragment_type == *frag_name) {
                            pfs.push(OwnedFragmentSpreadAccessor {
                                property_name: naming::first_lowercased(frag_name),
                                fragment_type: frag_name.clone(),
                                is_optional: false,
                            });
                        }
                    }

                    // Add merged fields from applicable fragments
                    for frag_name in &case2_applicable {
                        if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                            for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                                if key == "__typename" { continue; }
                                if !pfa.iter().any(|f| f.name == *key) {
                                    let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                    pfa.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                                }
                            }
                        }
                    }

                    // Build nested entity types and type aliases
                    let mut case2_nested = Vec::new();
                    let mut case2_aliases: Vec<OwnedTypeAlias> = Vec::new();

                    // Find entity fields that need nested types
                    let mut case2_entity_keys: Vec<String> = Vec::new();
                    for (key, field) in &ds.fields {
                        if matches!(field, FieldSelection::Entity(_)) && !case2_entity_keys.contains(key) {
                            case2_entity_keys.push(key.clone());
                        }
                    }
                    for frag_name in &case2_applicable {
                        if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                            for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                                if matches!(field, FieldSelection::Entity(_)) && !case2_entity_keys.contains(key) {
                                    case2_entity_keys.push(key.clone());
                                }
                            }
                            for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                                if let Some(inner) = frag_map.get(sub.fragment_name.as_str()) {
                                    for (key, field) in &inner.root_field.selection_set.direct_selections.fields {
                                        if matches!(field, FieldSelection::Entity(_)) && !case2_entity_keys.contains(key) {
                                            case2_entity_keys.push(key.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }

                    for field_key in &case2_entity_keys {
                        // Determine if this entity field is a list type
                        let is_list_field = ds.fields.get(field_key)
                            .and_then(|f| if let FieldSelection::Entity(ef) = f { Some(ef.field_type.is_list()) } else { None })
                            .or_else(|| {
                                referenced_fragments.iter().find_map(|frag| {
                                    frag.root_field.selection_set.direct_selections.fields.get(field_key)
                                        .and_then(|f| if let FieldSelection::Entity(ef) = f { Some(ef.field_type.is_list()) } else { None })
                                })
                            })
                            .unwrap_or(true); // default to singularize (list behavior) if not found
                        let entity_struct_name = if is_list_field {
                            naming::first_uppercased(&naming::singularize(field_key))
                        } else {
                            naming::first_uppercased(field_key)
                        };
                        let parent_ef = ds.fields.get(field_key).and_then(|f| {
                            if let FieldSelection::Entity(ef) = f { Some(ef) } else { None }
                        });
                        if let Some(pef) = parent_ef {
                            let mut case2_sibling = Vec::new();
                            for sibling_inline in &ds.inline_fragments {
                                if let Some(ref sibling_tc) = sibling_inline.type_condition {
                                    if is_supertype_of_current(tc, sibling_tc.name()) {
                                        if let Some(FieldSelection::Entity(sib_ef)) = sibling_inline.selection_set.direct_selections.fields.get(field_key) {
                                            let sib_qualified = format!("{}.As{}.{}", qualified_name, naming::first_uppercased(sibling_tc.name()), entity_struct_name);
                                            let mut sib_fields = Vec::new();
                                            for (key, field) in &sib_ef.selection_set.direct_selections.fields {
                                                if key == "__typename" { continue; }
                                                let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                                sib_fields.push((key.clone(), swift_type));
                                            }
                                            case2_sibling.push((sib_qualified, sib_fields));
                                        }
                                        for nested in &sibling_inline.selection_set.direct_selections.inline_fragments {
                                            if let Some(ref nested_tc) = nested.type_condition {
                                                if is_supertype_of_current(tc, nested_tc.name()) || tc.name() == nested_tc.name() {
                                                    if let Some(FieldSelection::Entity(nested_ef)) = nested.selection_set.direct_selections.fields.get(field_key) {
                                                        let nested_qualified = format!("{}.As{}.{}", qualified_name, naming::first_uppercased(sibling_tc.name()), entity_struct_name);
                                                        let mut nested_fields = Vec::new();
                                                        for (key, field) in &nested_ef.selection_set.direct_selections.fields {
                                                            if key == "__typename" { continue; }
                                                            let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                                            nested_fields.push((key.clone(), swift_type));
                                                        }
                                                        if let Some(existing) = case2_sibling.iter_mut().find(|(q, _)| q == &nested_qualified) {
                                                            for (key, st) in &nested_fields {
                                                                if !existing.1.iter().any(|(k, _)| k == key) {
                                                                    existing.1.push((key.clone(), st.clone()));
                                                                }
                                                            }
                                                        } else {
                                                            case2_sibling.push((nested_qualified, nested_fields));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            let entity_qualified = format!("{}.{}", child_qualified, entity_struct_name);
                            let parent_has = ds.fields.get(field_key)
                                .map(|f| matches!(f, FieldSelection::Entity(_)))
                                .unwrap_or(false);
                            let merged_entity = build_inline_fragment_entity_type(
                                &entity_struct_name,
                                field_key,
                                pef,
                                &case2_applicable,
                                &case2_sibling,
                                schema_namespace,
                                access_modifier,
                                indent + 4,
                                &entity_qualified,
                                qualified_name,
                                referenced_fragments,
                                type_kinds,
                                is_mutable,
                                false,
                                customizer,
                                None,
                                root_entity_type,
                                parent_has,
                                api_target_name,
                                generate_initializers,
                            );
                            case2_nested.push(merged_entity);
                        }
                    }

                    // Add type aliases for entity types from applicable fragments
                    for frag_name in &case2_applicable {
                        if let Some(frag_arc) = frag_map.get(frag_name.as_str()) {
                            for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                                if matches!(field, FieldSelection::Entity(_)) {
                                    let entity_type = naming::first_uppercased(key);
                                    if !case2_nested.iter().any(|nt| nt.config.struct_name == entity_type)
                                        && !case2_aliases.iter().any(|ta| ta.name == entity_type)
                                    {
                                        case2_aliases.push(OwnedTypeAlias {
                                            name: entity_type.clone(),
                                            target: format!("{}.{}", frag_name, entity_type),
                                        });
                                    }
                                }
                            }
                        }
                    }

                    let mut pinit = if generate_initializers {
                        Some(build_promoted_composite_initializer(
                            tc, &pfa, schema_namespace, &child_qualified, &child_root_entity,
                            &spread.fragment_name, referenced_fragments,
                            &extra_frag_fulfilled,
                            customizer, ds,
                        ))
                    } else { None };

                    // Add applicable fragment OIDs to fulfilled fragments
                    if let Some(ref mut init) = pinit {
                        for frag_name in &case2_applicable {
                            if !init.fulfilled_fragments.contains(frag_name) {
                                init.fulfilled_fragments.push(frag_name.clone());
                            }
                        }
                        // Add sibling inline fragment OIDs
                        for sibling_inline in &ds.inline_fragments {
                            if let Some(ref sibling_tc) = sibling_inline.type_condition {
                                if is_supertype_of_current(tc, sibling_tc.name()) {
                                    let sibling_qualified = format!("{}.As{}", qualified_name, naming::first_uppercased(customizer.custom_type_name(sibling_tc.name())));
                                    if !init.fulfilled_fragments.contains(&sibling_qualified) {
                                        init.fulfilled_fragments.push(sibling_qualified);
                                    }
                                }
                            }
                        }
                        // Fix entity field data entries to use _fieldData
                        for entry in init.data_entries.iter_mut() {
                            if let OwnedDataEntryValue::Variable(ref name) = entry.value {
                                let name_clone = name.clone();
                                let is_entity = ds.fields.get(&name_clone)
                                    .map(|f| matches!(f, FieldSelection::Entity(_)))
                                    .unwrap_or(false)
                                    || case2_applicable.iter().any(|fn_| {
                                        frag_map.get(fn_.as_str())
                                            .map(|fa| fa.root_field.selection_set.direct_selections.fields.get(&name_clone)
                                                .map(|f| matches!(f, FieldSelection::Entity(_)))
                                                .unwrap_or(false))
                                            .unwrap_or(false)
                                    });
                                if is_entity {
                                    entry.value = OwnedDataEntryValue::FieldData(name_clone);
                                }
                            }
                        }
                    }

                    let pc = if is_mutable { SelectionSetConformance::MutableInlineFragment } else { SelectionSetConformance::CompositeInlineFragment };
                    let case2_nested_len = case2_nested.len();
                    let pss = OwnedSelectionSetConfig {
                        struct_name: type_name.clone(), schema_namespace: schema_namespace.to_string(),
                        parent_type: ppt, is_root: false, is_inline_fragment: true,
                        conformance: pc, root_entity_type: Some(child_root_entity),
                        merged_sources: ms, selections: vec![],
                        field_accessors: pfa, inline_fragment_accessors: vec![],
                        fragment_spreads: pfs, initializer: pinit,
                        nested_types: case2_nested, type_aliases: case2_aliases, type_alias_insert_index: case2_nested_len,
                        indent: indent + 2, access_modifier: access_modifier.to_string(), is_mutable, absorbed_type_names: vec![], api_target_name: api_target_name.to_string(),
                    };
                    let dc = if is_root {
                        format!("/// {}", type_name)
                    } else {
                        let doc_prefix = if let Some(pos) = qualified_name.find(".Data.") {
                            &qualified_name[pos + 6..]
                        } else {
                            struct_name
                        };
                        format!("/// {}.{}", doc_prefix, type_name)
                    };
                    // Insert Case 2 promoted nested types at the correct position
                    nested_types.insert(promoted_insert_index, OwnedNestedSelectionSet {
                        doc_comment: dc,
                        parent_type_comment: format!("///\n{}/// Parent Type: `{}`", " ".repeat(indent + 2), customizer.custom_type_name(tc.name())),
                        config: pss,
                    });
                    promoted_insert_index += 1;
                }
            }
        }
    }

    // Remove promoted fragments from the parent scope's selections, field_accessors,
    // and fragment_spreads. When a fragment is promoted to an inline fragment (type narrowing),
    // it should not appear at the parent scope.
    if !promoted_fragment_names.is_empty() {
        let promoted_uppercased: Vec<String> = promoted_fragment_names.iter()
            .map(|n| naming::first_uppercased(n))
            .collect();
        // Remove .fragment(FragName.self) from selections
        selections.retain(|s| {
            if let OwnedSelectionKind::Fragment(name) = &s.kind {
                !promoted_uppercased.contains(name)
            } else {
                true
            }
        });
        // Remove from fragment_spreads
        fragment_spreads.retain(|fs| !promoted_uppercased.contains(&fs.fragment_type));
        // Remove fields that came exclusively from promoted fragments
        // (only remove if the field doesn't exist in direct selections or non-promoted fragments)
        let mut fields_from_nonpromoted: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (key, _) in &ds.fields {
            fields_from_nonpromoted.insert(key.clone());
        }
        for spread in &ds.named_fragments {
            if !promoted_fragment_names.contains(&spread.fragment_name) {
                if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
                    for (key, _) in &frag_arc.root_field.selection_set.direct_selections.fields {
                        fields_from_nonpromoted.insert(key.clone());
                    }
                    // Also check sub-fragment fields
                    for sub_spread in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                        if let Some(sub_frag) = frag_map.get(sub_spread.fragment_name.as_str()) {
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

    // Build nested types for conditional fragment spreads.
    // E.g., `...HeightInMeters @skip(if: $skipHeightInMeters)` creates:
    //   struct IfNotSkipHeightInMeters: InlineFragment { ... .fragment(HeightInMeters.self) ... }
    // And `...WarmBloodedDetails @include(if: $getWarmBlooded)` creates:
    //   struct AsWarmBloodedIfGetWarmBlooded: InlineFragment { ... .fragment(WarmBloodedDetails.self) ... }
    for spread in &ds.named_fragments {
        if !has_inclusion_conditions(spread.inclusion_conditions.as_ref()) { continue; }
        if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
            let ic = spread.inclusion_conditions.as_ref().expect("spread missing expected inclusion conditions");
            let ftc = &frag_arc.type_condition_name;
            let needs_narrowing = *ftc != ir_ss.scope.parent_type.name()
                && !is_supertype_of_current(&ir_ss.scope.parent_type, ftc);
            let type_name = if needs_narrowing {
                conditional_inline_fragment_name(Some(ftc), ic, customizer)
            } else {
                conditional_inline_fragment_name(None, ic, customizer)
            };
            let child_qualified = format!("{}.{}", qualified_name, type_name);
            let child_root_entity = if is_root {
                qualified_name.to_string()
            } else {
                root_entity_type.unwrap_or(qualified_name).to_string()
            };

            // Build the parent type for the conditional inline fragment
            let cond_parent_type = if needs_narrowing {
                match &frag_arc.root_field.selection_set.scope.parent_type {
                    GraphQLCompositeType::Object(o) => OwnedParentTypeRef::Object(customizer.custom_type_name(&o.name).to_string()),
                    GraphQLCompositeType::Interface(i) => OwnedParentTypeRef::Interface(customizer.custom_type_name(&i.name).to_string()),
                    GraphQLCompositeType::Union(u) => OwnedParentTypeRef::Union(customizer.custom_type_name(&u.name).to_string()),
                }
            } else {
                parent_type.clone()
            };

            // Build field accessors: parent inherited fields first, then fragment's own fields.
            // This matches the golden ordering: parent-scope fields, then fragment-contributed fields.
            let mut cond_fa = Vec::new();
            // Add parent inherited fields first
            for fa in &field_accessors {
                if !cond_fa.iter().any(|f: &OwnedFieldAccessor| f.name == fa.name) {
                    cond_fa.push(fa.clone());
                }
            }
            // Add fragment's fields
            for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                if key == "__typename" { continue; }
                let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                if !cond_fa.iter().any(|f: &OwnedFieldAccessor| f.name == *key) {
                    cond_fa.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                }
            }
            // Add sub-fragment fields
            for sub_spread in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                if let Some(sub_frag) = frag_map.get(sub_spread.fragment_name.as_str()) {
                    for (key, field) in &sub_frag.root_field.selection_set.direct_selections.fields {
                        if key == "__typename" { continue; }
                        if !cond_fa.iter().any(|f: &OwnedFieldAccessor| f.name == *key) {
                            let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                            cond_fa.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                        }
                    }
                }
            }

            // Build fragment spread accessors
            let mut cond_fs = vec![OwnedFragmentSpreadAccessor {
                property_name: naming::first_lowercased(&spread.fragment_name),
                fragment_type: spread.fragment_name.clone(),
                is_optional: false,
            }];
            // Add sub-fragments
            for sub_spread in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                if !cond_fs.iter().any(|fs| fs.fragment_type == sub_spread.fragment_name) {
                    cond_fs.push(OwnedFragmentSpreadAccessor {
                        property_name: naming::first_lowercased(&sub_spread.fragment_name),
                        fragment_type: sub_spread.fragment_name.clone(),
                        is_optional: false,
                    });
                }
            }

            // Build selections: just the fragment spread
            let cond_selections = vec![OwnedSelectionItem {
                kind: OwnedSelectionKind::Fragment(naming::first_uppercased(&spread.fragment_name)),
            }];

            // Build initializer
            let cond_init = if generate_initializers {
                // Collect fragment names for fulfilled fragments
                let mut extra_fulfilled = vec![naming::first_uppercased(&spread.fragment_name)];
                for sub_spread in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                    extra_fulfilled.push(naming::first_uppercased(&sub_spread.fragment_name));
                }
                Some(build_initializer_config(
                    &if needs_narrowing {
                        frag_arc.root_field.selection_set.scope.parent_type.clone()
                    } else {
                        ir_ss.scope.parent_type.clone()
                    },
                    &frag_arc.root_field.selection_set.direct_selections,
                    schema_namespace,
                    &child_qualified,
                    true, // is_inline_fragment
                    Some(&child_root_entity),
                    referenced_fragments,
                    type_kinds,
                    &cond_fa,
                    &extra_fulfilled,
                    customizer,
                    false,
                ))
            } else {
                None
            };

            // Build nested entity types (like Height from HeightInMeters)
            let mut cond_nested = Vec::new();
            let mut cond_aliases = Vec::new();
            // Check for entity fields from the fragment
            for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                if matches!(field, FieldSelection::Entity(_)) {
                    let entity_name = naming::first_uppercased(key);
                    // Check if parent also has this entity field
                    let parent_has = ds.fields.get(key).map(|f| matches!(f, FieldSelection::Entity(_))).unwrap_or(false);
                    if parent_has {
                        // Need a merged nested type - build it similarly to promoted fragments
                        if let Some(FieldSelection::Entity(parent_ef)) = ds.fields.get(key) {
                            let entity_qualified = format!("{}.{}", child_qualified, entity_name);
                            let merged = build_inline_fragment_entity_type(
                                &entity_name,
                                key,
                                parent_ef,
                                &[spread.fragment_name.clone()],
                                &[],
                                schema_namespace,
                                access_modifier,
                                indent + 4,
                                &entity_qualified,
                                qualified_name,
                                referenced_fragments,
                                type_kinds,
                                is_mutable,
                                false,
                                customizer,
                                None,
                                root_entity_type,
                                true,
                                api_target_name,
                                generate_initializers,
                            );
                            cond_nested.push(merged);
                        }
                    } else {
                        // Use typealias to the fragment's entity
                        cond_aliases.push(OwnedTypeAlias {
                            name: entity_name.clone(),
                            target: format!("{}.{}", spread.fragment_name, entity_name),
                        });
                    }
                }
            }
            // Also check sub-fragments for entity fields
            for sub_spread in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                if let Some(sub_frag) = frag_map.get(sub_spread.fragment_name.as_str()) {
                    for (key, field) in &sub_frag.root_field.selection_set.direct_selections.fields {
                        if matches!(field, FieldSelection::Entity(_)) {
                            let entity_name = naming::first_uppercased(key);
                            if !cond_nested.iter().any(|nt: &OwnedNestedSelectionSet| nt.config.struct_name == entity_name)
                                && !cond_aliases.iter().any(|ta| ta.name == entity_name)
                            {
                                // Check if the entity field also exists in the parent scope
                                // (either direct, via parent_scope_ds, or inherited via field_accessors)
                                let parent_has = ds.fields.get(key).map(|f| matches!(f, FieldSelection::Entity(_))).unwrap_or(false);
                                let inherited_has = !parent_has && parent_scope_ds.map_or(false, |pds| {
                                    pds.fields.get(key).map(|f| matches!(f, FieldSelection::Entity(_))).unwrap_or(false)
                                });
                                let field_accessor_has = field_accessors.iter().any(|fa| fa.name == *key);
                                if parent_has || inherited_has || field_accessor_has {
                                    // Need a merged nested type
                                    let best_parent_ef = ds.fields.get(key).and_then(|f| {
                                        if let FieldSelection::Entity(ef) = f { Some(ef) } else { None }
                                    }).or_else(|| {
                                        parent_scope_ds.and_then(|pds| pds.fields.get(key).and_then(|f| {
                                            if let FieldSelection::Entity(ef) = f { Some(ef) } else { None }
                                        }))
                                    });
                                    if let (Some(parent_ef), FieldSelection::Entity(sub_ef)) = (best_parent_ef, field) {
                                        let entity_qualified = format!("{}.{}", child_qualified, entity_name);
                                        let merged = build_inline_fragment_entity_type(
                                            &entity_name,
                                            key,
                                            parent_ef,
                                            &[sub_spread.fragment_name.clone()],
                                            &[],
                                            schema_namespace,
                                            access_modifier,
                                            indent + 4,
                                            &entity_qualified,
                                            qualified_name,
                                            referenced_fragments,
                                            type_kinds,
                                            is_mutable,
                                            false,
                                            customizer,
                                            None, // Fragment fields come via applicable_fragments, preserving parent-first ordering
                                            root_entity_type,
                                            true,
                                            api_target_name,
                                            generate_initializers,
                                        );
                                        cond_nested.push(merged);
                                    } else {
                                        cond_aliases.push(OwnedTypeAlias {
                                            name: entity_name.clone(),
                                            target: format!("{}.{}", sub_spread.fragment_name, entity_name),
                                        });
                                    }
                                } else {
                                    cond_aliases.push(OwnedTypeAlias {
                                        name: entity_name.clone(),
                                        target: format!("{}.{}", sub_spread.fragment_name, entity_name),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            let cond_nested_len = cond_nested.len();
            let ic = if is_mutable { SelectionSetConformance::MutableInlineFragment } else { SelectionSetConformance::InlineFragment };
            let cond_ss = OwnedSelectionSetConfig {
                struct_name: type_name.clone(),
                schema_namespace: schema_namespace.to_string(),
                parent_type: cond_parent_type,
                is_root: false,
                is_inline_fragment: true,
                conformance: ic,
                root_entity_type: Some(child_root_entity.clone()),
                merged_sources: vec![],
                selections: cond_selections,
                field_accessors: cond_fa,
                inline_fragment_accessors: vec![],
                fragment_spreads: cond_fs,
                initializer: cond_init,
                nested_types: cond_nested,
                type_aliases: cond_aliases,
                type_alias_insert_index: cond_nested_len,
                indent: indent + 2,
                access_modifier: access_modifier.to_string(),
                is_mutable,
                absorbed_type_names: vec![],
                api_target_name: api_target_name.to_string(),
            };

            let parent_type_name = customizer.custom_type_name(if needs_narrowing { ftc.as_str() } else { ir_ss.scope.parent_type.name() });
            let dc = if is_root {
                format!("/// {}", type_name)
            } else {
                let doc_prefix = if let Some(pos) = qualified_name.find(".Data.") {
                    &qualified_name[pos + 6..]
                } else {
                    struct_name
                };
                format!("/// {}.{}", doc_prefix, type_name)
            };
            // Insert conditional fragment spread nested types at the promoted_insert_index
            // (after entity types and promoted fragments, before unconditional inline fragments)
            nested_types.insert(promoted_insert_index, OwnedNestedSelectionSet {
                doc_comment: dc,
                parent_type_comment: format!(
                    "///\n{}/// Parent Type: `{}`",
                    " ".repeat(indent + 2),
                    parent_type_name
                ),
                config: cond_ss,
            });
            promoted_insert_index += 1;
        }
    }

    // Build initializer when requested
    let initializer = if generate_initializers {
        let extra_fulfilled: Vec<String> = vec![];

        let is_fragment_definition = matches!(conformance,
            SelectionSetConformance::Fragment | SelectionSetConformance::MutableFragment);
        let mut init = build_initializer_config(
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
            customizer,
            is_fragment_definition,
        );
        // Filter out promoted fragment names from fulfilled_fragments
        if !promoted_fragment_names.is_empty() {
            let promoted_uc: Vec<String> = promoted_fragment_names.iter()
                .map(|n| naming::first_uppercased(n))
                .collect();
            init.fulfilled_fragments.retain(|f| !promoted_uc.contains(f));
        }
        Some(init)
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
        type_alias_insert_index: entity_nested_type_count,
        indent,
        access_modifier: access_modifier.to_string(),
        is_mutable,
        absorbed_type_names,
        api_target_name: api_target_name.to_string(),
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

/// Check if a Swift type string looks like a local entity type (uses _fieldData).
/// Entity types are local struct names like `Height`, `[Predator]`, `Owner?`, `[Height]?`.
/// Scalar types use qualified names like `AnimalKingdomAPI.SkinCovering` or built-in types.
fn swift_type_is_entity(swift_type: &str) -> bool {
    // Strip optional suffix and list brackets
    let inner = swift_type
        .trim_end_matches('?')
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('?');
    // Entity types are PascalCase local struct names (no dots, starts with uppercase)
    // Built-in types (String, Int, Bool, Double) are NOT entities
    !inner.is_empty()
        && inner.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
        && !inner.contains('.')
        && !matches!(inner, "String" | "Int" | "Bool" | "Double" | "Float")
        && !inner.starts_with("GraphQLEnum<")
        && !inner.starts_with("GraphQLNullable<")
}

/// Check if a type condition satisfies a fragment's type condition.
/// Returns true if the given type implements or equals the fragment's type.
fn type_satisfies_condition(tc: &GraphQLCompositeType, fragment_type_condition: &str) -> bool {
    if tc.name() == fragment_type_condition {
        return true;
    }
    is_supertype_of_current(tc, fragment_type_condition)
}

/// Collect all applicable fragment names for an inline fragment with a given type condition.
///
/// A fragment is applicable if:
/// 1. It's a non-promoted parent-scope fragment (type condition matches parent type) - always applicable
/// 2. It's from a sibling inline fragment whose type condition is satisfied by this inline fragment's type
/// 3. It's from a promoted inline fragment whose type condition is satisfied by this inline fragment's type
///
/// Returns a deduplicated list of (fragment_name, source_description) pairs.
fn collect_applicable_fragments(
    tc: &GraphQLCompositeType,
    parent_type_name: &str,
    ds: &DirectSelections,
    promoted_fragment_names: &[String],
    referenced_fragments: &[Arc<NamedFragment>],
    _parent_type: &GraphQLCompositeType,
    ancestor_fragments: &[String],
) -> Vec<String> {
    let mut applicable = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Build a lookup map for O(1) fragment resolution
    let frag_map: std::collections::HashMap<&str, &Arc<NamedFragment>> = referenced_fragments
        .iter()
        .map(|f| (f.name.as_str(), f))
        .collect();

    // 1. Non-promoted parent-scope fragments are always applicable to child inline fragments
    //    (the parent scope already guarantees the fragment's type condition).
    //    Skip conditional fragment spreads - they have their own inline fragment scope.
    for spread in &ds.named_fragments {
        if has_inclusion_conditions(spread.inclusion_conditions.as_ref()) { continue; }
        if promoted_fragment_names.contains(&spread.fragment_name) {
            // This fragment was promoted to an inline fragment because its type condition
            // differs from the parent - check if the current type satisfies it
            if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
                if type_satisfies_condition(tc, &frag_arc.type_condition_name) {
                    if seen.insert(spread.fragment_name.clone()) {
                        applicable.push(spread.fragment_name.clone());
                    }
                    for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                        if seen.insert(sub.fragment_name.clone()) {
                            applicable.push(sub.fragment_name.clone());
                        }
                    }
                }
            }
        } else {
            // Non-promoted fragment - always applicable
            if seen.insert(spread.fragment_name.clone()) {
                applicable.push(spread.fragment_name.clone());
            }
            if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
                for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                    if !seen.contains(&sub.fragment_name) {
                        if let Some(sub_frag) = frag_map.get(sub.fragment_name.as_str()) {
                            if type_satisfies_condition(tc, &sub_frag.type_condition_name)
                                || sub_frag.type_condition_name == parent_type_name
                            {
                                seen.insert(sub.fragment_name.clone());
                                applicable.push(sub.fragment_name.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Fragments from sibling inline fragments whose type condition is satisfied by this type
    for sibling_inline in &ds.inline_fragments {
        if let Some(ref sibling_tc) = sibling_inline.type_condition {
            if sibling_tc.name() != tc.name() && is_supertype_of_current(tc, sibling_tc.name()) {
                for spread in &sibling_inline.selection_set.direct_selections.named_fragments {
                    if let Some(frag_arc) = frag_map.get(spread.fragment_name.as_str()) {
                        if type_satisfies_condition(tc, &frag_arc.type_condition_name) {
                            if seen.insert(spread.fragment_name.clone()) {
                                applicable.push(spread.fragment_name.clone());
                            }
                            for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                                if seen.insert(sub.fragment_name.clone()) {
                                    applicable.push(sub.fragment_name.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Fragments from ancestor scopes whose type condition is satisfied by this type.
    for ancestor_frag_name in ancestor_fragments {
        if seen.contains(ancestor_frag_name) { continue; }
        if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == *ancestor_frag_name) {
            if type_satisfies_condition(tc, &frag_arc.type_condition_name) {
                seen.insert(ancestor_frag_name.clone());
                applicable.push(ancestor_frag_name.clone());
                for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                    if !seen.contains(&sub.fragment_name) {
                        if let Some(sub_frag) = referenced_fragments.iter().find(|f| f.name == *sub.fragment_name) {
                            if type_satisfies_condition(tc, &sub_frag.type_condition_name) {
                                seen.insert(sub.fragment_name.clone());
                                applicable.push(sub.fragment_name.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    applicable
}

/// Build a merged entity nested type for an inline fragment.
///
/// Collects fields from the parent entity field, applicable fragments' entity fields,
/// and sibling inline fragments' entity fields.
fn build_inline_fragment_entity_type(
    struct_name: &str,
    field_key: &str,
    parent_entity_field: &EntityField,
    applicable_fragments: &[String],
    sibling_entity_fields: &[(String, Vec<(String, String)>)],  // (scope_name, [(field_name, swift_type)])
    schema_namespace: &str,
    access_modifier: &str,
    indent: usize,
    qualified_name: &str,
    parent_qualified_name: &str,
    referenced_fragments: &[Arc<NamedFragment>],
    type_kinds: &HashMap<String, TypeKind>,
    is_mutable: bool,
    has_direct_selections: bool,
    customizer: &SchemaCustomizer,
    inline_entity_field: Option<&EntityField>,  // The inline fragment's own entity field (for __selections)
    root_entity_qualified: Option<&str>,  // Root entity qualified name for fulfilled fragments
    parent_has_entity_field: bool,  // Whether the parent scope actually has this entity field
    api_target_name: &str,
    generate_initializers: bool,
) -> OwnedNestedSelectionSet {
    let parent_type_name = customizer.custom_type_name(parent_entity_field.selection_set.scope.parent_type.name());
    let entity_parent_type = match &parent_entity_field.selection_set.scope.parent_type {
        GraphQLCompositeType::Object(o) => OwnedParentTypeRef::Object(customizer.custom_type_name(&o.name).to_string()),
        GraphQLCompositeType::Interface(i) => OwnedParentTypeRef::Interface(customizer.custom_type_name(&i.name).to_string()),
        GraphQLCompositeType::Union(u) => OwnedParentTypeRef::Union(customizer.custom_type_name(&u.name).to_string()),
    };

    // Collect fields - inline entity field's selections first (if present),
    // then parent entity field's selections, then fragment fields
    let mut merged_fields: Vec<OwnedFieldAccessor> = Vec::new();
    // Add inline entity field's selections first (these are the NEW fields)
    if let Some(ief) = inline_entity_field {
        for (key, field) in &ief.selection_set.direct_selections.fields {
            if key == "__typename" { continue; }
            let (mut swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
            // Fields with inclusion conditions become optional
            let conds = field_inclusion_conditions(field);
            if has_inclusion_conditions(conds) && !swift_type.ends_with('?') {
                swift_type.push('?');
            }
            if !merged_fields.iter().any(|f| f.name == *key) {
                merged_fields.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
            }
        }
    }
    // Then add parent entity field's selections
    for (key, field) in &parent_entity_field.selection_set.direct_selections.fields {
        if key == "__typename" { continue; }
        let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
        if !merged_fields.iter().any(|f| f.name == *key) {
            merged_fields.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
        }
    }

    // Collect conditional fields from sibling inline fragments' entity fields BEFORE fragment fields.
    // Unconditional sibling fields go AFTER fragment fields (matching golden ordering).
    let mut unconditional_sibling_fields: Vec<(String, String)> = Vec::new();
    for (_, sibling_fields) in sibling_entity_fields {
        for (key, swift_type) in sibling_fields {
            if !merged_fields.iter().any(|f| f.name == *key) {
                if swift_type.ends_with('?') && !key.starts_with("__") {
                    // Conditional (optional) sibling fields go before fragment fields
                    merged_fields.push(OwnedFieldAccessor { name: key.clone(), swift_type: swift_type.clone(), description: None });
                } else {
                    // Unconditional sibling fields go after fragment fields
                    unconditional_sibling_fields.push((key.clone(), swift_type.clone()));
                }
            }
        }
    }

    // Collect fields from applicable fragments' entity fields
    for frag_name in applicable_fragments {
        if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == *frag_name) {
            if let Some(FieldSelection::Entity(frag_ef)) = frag_arc.root_field.selection_set.direct_selections.fields.get(field_key) {
                for (key, field) in &frag_ef.selection_set.direct_selections.fields {
                    if key == "__typename" { continue; }
                    if !merged_fields.iter().any(|f| f.name == *key) {
                        let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                        merged_fields.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                    }
                }
            }
            // Check sub-fragments too
            for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                if let Some(inner) = referenced_fragments.iter().find(|f| f.name == *sub.fragment_name) {
                    if let Some(FieldSelection::Entity(inner_ef)) = inner.root_field.selection_set.direct_selections.fields.get(field_key) {
                        for (key, field) in &inner_ef.selection_set.direct_selections.fields {
                            if key == "__typename" { continue; }
                            if !merged_fields.iter().any(|f| f.name == *key) {
                                let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                                merged_fields.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                            }
                        }
                    }
                }
            }
        }
    }

    // Add unconditional sibling fields after fragment fields
    for (key, swift_type) in &unconditional_sibling_fields {
        if !merged_fields.iter().any(|f| f.name == *key) {
            merged_fields.push(OwnedFieldAccessor { name: key.clone(), swift_type: swift_type.clone(), description: None });
        }
    }

    // Build fulfilled fragments
    // Order: self, root entity scope (if different from parent), then parent if same as root,
    // then fragment entities, then parent if different from root.
    let parent_entity_qualified = format!("{}.{}", parent_qualified_name, struct_name);
    let mut fulfilled = vec![qualified_name.to_string()];

    // Determine if root is different from parent
    let root_differs_from_parent = root_entity_qualified.map(|root_qual| {
        let root_entity = format!("{}.{}", root_qual, struct_name);
        root_entity != parent_entity_qualified && root_entity != qualified_name.to_string()
    }).unwrap_or(false);

    if root_differs_from_parent {
        // Add root entity scope first (e.g., AllAnimal.Height)
        if let Some(root_qual) = root_entity_qualified {
            let root_entity = format!("{}.{}", root_qual, struct_name);
            fulfilled.push(root_entity);
        }
    } else if parent_has_entity_field {
        // Root == parent: add parent here (before fragments) only if parent actually has this entity
        fulfilled.push(parent_entity_qualified.clone());
    }

    // Determine if sibling entity fields have any conditional (optional) fields.
    // If they do, sibling OIDs come before fragment OIDs; otherwise after.
    let sibling_has_conditional = sibling_entity_fields.iter().any(|(_, fields)| {
        fields.iter().any(|(_, st)| st.ends_with('?'))
    });

    if sibling_has_conditional {
        // When sibling has conditional fields, add sibling OIDs + parent OID before fragment OIDs
        for (scope_name, _) in sibling_entity_fields {
            if !fulfilled.contains(scope_name) {
                fulfilled.push(scope_name.clone());
            }
        }
        if root_differs_from_parent && parent_has_entity_field && !fulfilled.contains(&parent_entity_qualified) {
            fulfilled.push(parent_entity_qualified.clone());
        }
    }

    // Add fragment entity type OIDs
    for frag_name in applicable_fragments {
        if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == *frag_name) {
            if frag_arc.root_field.selection_set.direct_selections.fields.contains_key(field_key) {
                let frag_entity = format!("{}.{}", frag_name, struct_name);
                if !fulfilled.contains(&frag_entity) {
                    fulfilled.push(frag_entity);
                }
            }
            for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                if let Some(inner) = referenced_fragments.iter().find(|f| f.name == *sub.fragment_name) {
                    if inner.root_field.selection_set.direct_selections.fields.contains_key(field_key) {
                        let sub_entity = format!("{}.{}", sub.fragment_name, struct_name);
                        if !fulfilled.contains(&sub_entity) {
                            fulfilled.push(sub_entity);
                        }
                    }
                }
            }
        }
    }

    // When sibling is non-conditional, add sibling OIDs + parent OID after fragment OIDs
    if !sibling_has_conditional {
        if root_differs_from_parent && parent_has_entity_field && !fulfilled.contains(&parent_entity_qualified) {
            fulfilled.push(parent_entity_qualified.clone());
        }
        for (scope_name, _) in sibling_entity_fields {
            if !fulfilled.contains(scope_name) {
                fulfilled.push(scope_name.clone());
            }
        }
    }

    let is_parent_object = matches!(entity_parent_type, OwnedParentTypeRef::Object(_));
    let custom_parent_name = customizer.custom_type_name(parent_type_name);
    let typename_value = if is_parent_object {
        OwnedTypenameValue::Fixed(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(custom_parent_name)))
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
            OwnedDataEntryValue::Typename(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(custom_parent_name)))
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

    // Determine if this entity type has __selections (only when the inline fragment
    // has direct selections on this field, not just inherited)
    let selections = if has_direct_selections {
        let mut sels = Vec::new();
        sels.push(OwnedSelectionItem {
            kind: OwnedSelectionKind::Field {
                name: "__typename".to_string(),
                alias: None,
                swift_type: "String".to_string(),
                arguments: None,
            },
        });
        // Use inline_entity_field for __selections if available (absorbed inline case),
        // otherwise use parent_entity_field (direct selection case)
        let sel_source = inline_entity_field.unwrap_or(parent_entity_field);
        // Collect conditional field groups (same conditions grouped together)
        let mut conditional_groups: Vec<(Vec<OwnedConditionEntry>, OwnedConditionOperator, Vec<(String, Option<String>, String, Option<String>)>)> = Vec::new();
        for (key, field) in &sel_source.selection_set.direct_selections.fields {
            if key == "__typename" { continue; }
            let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
            let conds = field_inclusion_conditions(field);
            let (nest_field_name, nest_field_alias) = if key != field.name() {
                (field.name().to_string(), Some(key.clone()))
            } else {
                (key.clone(), None)
            };
            if has_inclusion_conditions(conds) {
                let ic = conds.expect("inclusion conditions verified by has_inclusion_conditions");
                let (owned_conds, operator) = inclusion_conditions_to_owned(ic);
                let conds_match = |group_conds: &Vec<OwnedConditionEntry>, group_op: &OwnedConditionOperator| -> bool {
                    if owned_conds.len() != group_conds.len() { return false; }
                    if !matches!((operator, group_op), (OwnedConditionOperator::And, OwnedConditionOperator::And) | (OwnedConditionOperator::Or, OwnedConditionOperator::Or)) { return false; }
                    owned_conds.iter().zip(group_conds.iter()).all(|(a, b)| a.variable == b.variable && a.is_inverted == b.is_inverted)
                };
                if let Some(group) = conditional_groups.iter_mut().find(|(gc, go, _)| conds_match(gc, go)) {
                    group.2.push((nest_field_name, nest_field_alias, swift_type, None));
                } else {
                    conditional_groups.push((owned_conds, operator, vec![(nest_field_name, nest_field_alias, swift_type, None)]));
                }
            } else {
                sels.push(OwnedSelectionItem {
                    kind: OwnedSelectionKind::Field { name: nest_field_name, alias: nest_field_alias, swift_type, arguments: None },
                });
            }
        }
        // Add conditional field groups after unconditional fields
        for (conds, operator, fields) in &conditional_groups {
            if fields.len() == 1 {
                let (name, alias, swift_type, arguments) = &fields[0];
                sels.push(OwnedSelectionItem {
                    kind: OwnedSelectionKind::ConditionalField {
                        conditions: conds.clone(),
                        operator: *operator,
                        name: name.clone(),
                        alias: alias.clone(),
                        swift_type: swift_type.clone(),
                        arguments: arguments.clone(),
                    },
                });
            } else {
                sels.push(OwnedSelectionItem {
                    kind: OwnedSelectionKind::ConditionalFieldGroup {
                        conditions: conds.clone(),
                        operator: *operator,
                        fields: fields.clone(),
                    },
                });
            }
        }
        sels
    } else {
        vec![]
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
        selections,
        field_accessors: merged_fields,
        inline_fragment_accessors: vec![],
        fragment_spreads: vec![],
        initializer: if generate_initializers { Some(initializer) } else { None },
        nested_types: vec![],
        type_aliases: vec![],
        type_alias_insert_index: 0,
        indent,
        access_modifier: access_modifier.to_string(),
        is_mutable, absorbed_type_names: vec![], api_target_name: api_target_name.to_string(),
    };

    // Use full entity-relative path for doc comment (e.g., "AllAnimal.AsWarmBlooded.Height")
    let doc_path = if let Some(pos) = qualified_name.find(".Data.") {
        &qualified_name[pos + 6..]
    } else {
        struct_name
    };
    OwnedNestedSelectionSet {
        doc_comment: format!("/// {}", doc_path),
        parent_type_comment: format!("///\n{}/// Parent Type: `{}`", " ".repeat(indent), parent_type_name),
        config,
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
    customizer: &SchemaCustomizer,
    api_target_name: &str,
) -> OwnedNestedSelectionSet {
    let parent_type_name = customizer.custom_type_name(parent_entity_field.selection_set.scope.parent_type.name());
    let entity_parent_type = match &parent_entity_field.selection_set.scope.parent_type {
        GraphQLCompositeType::Object(o) => OwnedParentTypeRef::Object(customizer.custom_type_name(&o.name).to_string()),
        GraphQLCompositeType::Interface(i) => OwnedParentTypeRef::Interface(customizer.custom_type_name(&i.name).to_string()),
        GraphQLCompositeType::Union(u) => OwnedParentTypeRef::Union(customizer.custom_type_name(&u.name).to_string()),
    };

    // Collect field accessors from the parent entity field
    let mut merged_fields: Vec<OwnedFieldAccessor> = Vec::new();
    for (key, field) in &parent_entity_field.selection_set.direct_selections.fields {
        if key == "__typename" { continue; }
        let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
        merged_fields.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
    }

    // Collect fields from the source fragment's entity field
    let frag_struct_name = if parent_entity_field.field_type.is_list() {
        naming::first_uppercased(&naming::singularize(field_key))
    } else {
        naming::first_uppercased(field_key)
    };
    if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == *source_frag_name) {
        // Check the fragment's own entity fields
        if let Some(FieldSelection::Entity(frag_ef)) = frag_arc.root_field.selection_set.direct_selections.fields.get(field_key) {
            for (key, field) in &frag_ef.selection_set.direct_selections.fields {
                if key == "__typename" { continue; }
                if !merged_fields.iter().any(|f| f.name == *key) {
                    let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                    merged_fields.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
                }
            }
        }
        // Also check sub-fragment entity fields
        for fs in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
            if let Some(inner) = referenced_fragments.iter().find(|f| f.name == *fs.fragment_name) {
                if let Some(FieldSelection::Entity(inner_ef)) = inner.root_field.selection_set.direct_selections.fields.get(field_key) {
                    for (key, field) in &inner_ef.selection_set.direct_selections.fields {
                        if key == "__typename" { continue; }
                        if !merged_fields.iter().any(|f| f.name == *key) {
                            let (swift_type, _) = render_field_swift_type(field, schema_namespace, type_kinds, customizer);
                            merged_fields.push(OwnedFieldAccessor { name: key.clone(), swift_type, description: field.description().map(|s| s.to_string()) });
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
    if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == *source_frag_name) {
        // Check sub-fragments for entity fields too
        for fs in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
            if let Some(inner) = referenced_fragments.iter().find(|f| f.name == *fs.fragment_name) {
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
    let custom_parent_name = customizer.custom_type_name(parent_type_name);
    let typename_value = if is_parent_object {
        OwnedTypenameValue::Fixed(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(custom_parent_name)))
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
            OwnedDataEntryValue::Typename(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(custom_parent_name)))
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
        type_alias_insert_index: 0,
        indent,
        access_modifier: access_modifier.to_string(),
        is_mutable, absorbed_type_names: vec![], api_target_name: api_target_name.to_string(),
    };

    let doc_path = if let Some(pos) = qualified_name.find(".Data.") {
        &qualified_name[pos + 6..]
    } else {
        struct_name
    };
    OwnedNestedSelectionSet {
        doc_comment: format!("/// {}", doc_path),
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
        if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == *spread.fragment_name) {
            for (key, field) in &frag_arc.root_field.selection_set.direct_selections.fields {
                if let FieldSelection::Entity(ef) = field {
                    // Use singularized name for list fields to match the fragment's entity struct name
                    let type_name = if ef.field_type.is_list() {
                        naming::first_uppercased(&naming::singularize(key))
                    } else {
                        naming::first_uppercased(key)
                    };
                    // Only add alias if we don't have a direct entity field with the same name
                    if !ds.fields.contains_key(key) || !matches!(ds.fields.get(key), Some(FieldSelection::Entity(_))) {
                        if !aliases.iter().any(|a: &OwnedTypeAlias| a.name == type_name) {
                            aliases.push(OwnedTypeAlias {
                                name: type_name.clone(),
                                target: format!("{}.{}", spread.fragment_name, type_name),
                            });
                        }
                    }
                }
            }
            // Also check sub-fragments for entity fields
            for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                if let Some(inner) = referenced_fragments.iter().find(|f| f.name == *sub.fragment_name) {
                    for (key, field) in &inner.root_field.selection_set.direct_selections.fields {
                        if let FieldSelection::Entity(ef) = field {
                            // Use singularized name for list fields to match the fragment's entity struct name
                            let type_name = if ef.field_type.is_list() {
                                naming::first_uppercased(&naming::singularize(key))
                            } else {
                                naming::first_uppercased(key)
                            };
                            if !ds.fields.contains_key(key) || !matches!(ds.fields.get(key), Some(FieldSelection::Entity(_))) {
                                if !aliases.iter().any(|a: &OwnedTypeAlias| a.name == type_name) {
                                    aliases.push(OwnedTypeAlias {
                                        name: type_name.clone(),
                                        target: format!("{}.{}", sub.fragment_name, type_name),
                                    });
                                }
                            }
                        }
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
    _type_kinds: &HashMap<String, TypeKind>,
    all_field_accessors: &[OwnedFieldAccessor],
    extra_fulfilled: &[String],
    customizer: &SchemaCustomizer,
    is_fragment_definition: bool,
) -> OwnedInitializerConfig {
    // Determine typename handling based on parent type
    let parent_is_object = matches!(parent_type, GraphQLCompositeType::Object(_));

    let typename_value = if parent_is_object {
        let swift_name = customizer.custom_type_name(parent_type.name());
        let type_ref = format!(
            "{}.Objects.{}.typename",
            schema_namespace,
            naming::first_uppercased(swift_name)
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
            let swift_name = customizer.custom_type_name(parent_type.name());
            let type_ref = format!(
                "{}.Objects.{}.typename",
                schema_namespace,
                naming::first_uppercased(swift_name)
            );
            OwnedDataEntryValue::Typename(type_ref)
        } else {
            OwnedDataEntryValue::Variable("__typename".to_string())
        },
    });

    // Add each field accessor to data entries (includes merged fields)
    for accessor in all_field_accessors {
        // Check if this is an entity or scalar field - check direct fields first,
        // then check fragment spread fields for merged fields, including sub-fragments
        let mut is_entity = ds.fields.get(&accessor.name)
            .map(|f| matches!(f, FieldSelection::Entity(_)))
            .unwrap_or(false);
        if !is_entity {
            // Check fragment spreads for entity fields
            'outer: for spread in &ds.named_fragments {
                if let Some(frag) = referenced_fragments.iter().find(|f| f.name == *spread.fragment_name) {
                    if let Some(field) = frag.root_field.selection_set.direct_selections.fields.get(&accessor.name) {
                        if matches!(field, FieldSelection::Entity(_)) {
                            is_entity = true;
                            break;
                        }
                    }
                    // Also check sub-fragments
                    for sub in &frag.root_field.selection_set.direct_selections.named_fragments {
                        if let Some(sub_frag) = referenced_fragments.iter().find(|f| f.name == *sub.fragment_name) {
                            if let Some(field) = sub_frag.root_field.selection_set.direct_selections.fields.get(&accessor.name) {
                                if matches!(field, FieldSelection::Entity(_)) {
                                    is_entity = true;
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
            }
        }
        if !is_entity {
            // Fallback: check if the Swift type looks like an entity type
            is_entity = swift_type_is_entity(&accessor.swift_type);
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

    // Add extra fulfilled fragments first (e.g., main fragment name for conditional fragment spreads).
    // This ensures the correct ordering: main fragment before sub-fragments.
    for extra in extra_fulfilled {
        if !fulfilled_fragments.contains(extra) {
            fulfilled_fragments.push(extra.clone());
        }
    }

    // Add directly spread named fragments to fulfilled fragments.
    // For fragment definitions and inline fragments, always include spreads (they're fully
    // resolved in those scopes). For operation entity selection sets, include only if the
    // fragment doesn't have overlapping entity fields with the parent's direct selections
    // (overlapping entity fields would have different sub-selections, making the fragment
    // not fully fulfilled by the parent's initializer).
    {
        let parent_is_union = matches!(parent_type, GraphQLCompositeType::Union(_));
        if !parent_is_union {
            for spread in &ds.named_fragments {
                // Skip conditional fragment spreads — they get their own inline fragment scope
                if has_inclusion_conditions(spread.inclusion_conditions.as_ref()) { continue; }

                // For fragment definitions and inline fragments, always include spreads.
                // For operation entity selection sets, include only if the fragment's type
                // condition is satisfied AND it doesn't have overlapping entity fields.
                let should_include = if is_fragment_definition || is_inline_fragment {
                    true
                } else if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == *spread.fragment_name) {
                    // First check: type condition must be satisfied
                    if !type_satisfies_condition(parent_type, &frag_arc.type_condition_name) {
                        false
                    } else {
                        // Second check: no overlapping entity fields with parent's direct fields.
                        // Overlapping entity fields would have merged sub-selections in the fragment
                        // that the parent's initializer can't fully satisfy.
                        let has_overlapping_entity = frag_arc.root_field.selection_set.direct_selections.fields
                            .iter()
                            .any(|(key, field)| {
                                matches!(field, FieldSelection::Entity(_))
                                    && ds.fields.get(key).map(|f| matches!(f, FieldSelection::Entity(_))).unwrap_or(false)
                            });
                        !has_overlapping_entity
                    }
                } else {
                    false
                };

                if should_include {
                    let uc_name = naming::first_uppercased(&spread.fragment_name);
                    if !fulfilled_fragments.contains(&uc_name) {
                        fulfilled_fragments.push(uc_name);
                    }
                }
                // Also add sub-fragment names if their type condition is satisfied
                // and they don't have overlapping entity fields
                if should_include {
                    if let Some(frag_arc) = referenced_fragments.iter().find(|f| f.name == *spread.fragment_name) {
                        for sub in &frag_arc.root_field.selection_set.direct_selections.named_fragments {
                            let sub_uc = naming::first_uppercased(&sub.fragment_name);
                            if !fulfilled_fragments.contains(&sub_uc) {
                                if let Some(sub_frag) = referenced_fragments.iter().find(|f| f.name == *sub.fragment_name) {
                                    if type_satisfies_condition(parent_type, &sub_frag.type_condition_name) {
                                        // For non-fragment-definition/non-inline-fragment, also check entity overlap
                                        let sub_ok = if is_fragment_definition || is_inline_fragment {
                                            true
                                        } else {
                                            !sub_frag.root_field.selection_set.direct_selections.fields
                                                .iter()
                                                .any(|(key, field)| {
                                                    matches!(field, FieldSelection::Entity(_))
                                                        && ds.fields.get(key).map(|f| matches!(f, FieldSelection::Entity(_))).unwrap_or(false)
                                                })
                                        };
                                        if sub_ok {
                                            fulfilled_fragments.push(sub_uc);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
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
    customizer: &SchemaCustomizer,
    parent_ds: &DirectSelections,
    parent_qualified_name: &str,
    parent_nonpromoted_fragments: &[String],
) -> OwnedInitializerConfig {
    let parent_is_object = matches!(parent_type, GraphQLCompositeType::Object(_));
    let swift_name = customizer.custom_type_name(parent_type.name());
    let typename_value = if parent_is_object {
        OwnedTypenameValue::Fixed(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(swift_name)))
    } else { OwnedTypenameValue::Parameter };
    let mut parameters = Vec::new();
    if !parent_is_object { parameters.push(OwnedInitParam { name: "__typename".to_string(), swift_type: "String".to_string(), default_value: None }); }
    for a in all_field_accessors {
        parameters.push(OwnedInitParam { name: a.name.clone(), swift_type: a.swift_type.clone(), default_value: if a.swift_type.ends_with('?') { Some("nil".to_string()) } else { None } });
    }
    let mut data_entries = vec![OwnedDataEntry {
        key: "__typename".to_string(),
        value: if parent_is_object { OwnedDataEntryValue::Typename(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(swift_name))) } else { OwnedDataEntryValue::Variable("__typename".to_string()) },
    }];
    for a in all_field_accessors {
        let is_entity = parent_ds.fields.get(&a.name).map(|f| matches!(f, FieldSelection::Entity(_))).unwrap_or(false)
            || referenced_fragments.iter().any(|f| f.root_field.selection_set.direct_selections.fields.get(&a.name).map(|field| matches!(field, FieldSelection::Entity(_))).unwrap_or(false))
            || swift_type_is_entity(&a.swift_type);
        data_entries.push(OwnedDataEntry { key: a.name.clone(), value: if is_entity { OwnedDataEntryValue::FieldData(a.name.clone()) } else { OwnedDataEntryValue::Variable(a.name.clone()) } });
    }
    let mut fulfilled_fragments = vec![root_entity_type.to_string()];
    // Add parent scope's qualified name if different from root entity
    if parent_qualified_name != root_entity_type && !fulfilled_fragments.contains(&parent_qualified_name.to_string()) {
        fulfilled_fragments.push(parent_qualified_name.to_string());
    }
    fulfilled_fragments.push(qualified_name.to_string());
    fulfilled_fragments.push(naming::first_uppercased(fragment_name));
    for fs in frag_named_fragments { fulfilled_fragments.push(naming::first_uppercased(&fs.fragment_name)); }
    // Add parent scope's non-promoted fragments to fulfilled
    for parent_frag in parent_nonpromoted_fragments {
        let uc = naming::first_uppercased(parent_frag);
        if !fulfilled_fragments.contains(&uc) {
            fulfilled_fragments.push(uc);
        }
    }
    OwnedInitializerConfig { parameters, data_entries, fulfilled_fragments, typename_value }
}

/// Build an initializer for a promoted composite inline fragment (Case 2).
fn build_promoted_composite_initializer(
    parent_type: &GraphQLCompositeType, all_field_accessors: &[OwnedFieldAccessor],
    schema_namespace: &str, qualified_name: &str, root_entity_type: &str,
    fragment_name: &str, referenced_fragments: &[Arc<NamedFragment>],
    extra_fulfilled: &[String],
    customizer: &SchemaCustomizer,
    parent_ds: &DirectSelections,
) -> OwnedInitializerConfig {
    let parent_is_object = matches!(parent_type, GraphQLCompositeType::Object(_));
    let swift_name = customizer.custom_type_name(parent_type.name());
    let typename_value = if parent_is_object {
        OwnedTypenameValue::Fixed(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(swift_name)))
    } else { OwnedTypenameValue::Parameter };
    let mut parameters = Vec::new();
    if !parent_is_object { parameters.push(OwnedInitParam { name: "__typename".to_string(), swift_type: "String".to_string(), default_value: None }); }
    for a in all_field_accessors {
        parameters.push(OwnedInitParam { name: a.name.clone(), swift_type: a.swift_type.clone(), default_value: if a.swift_type.ends_with('?') { Some("nil".to_string()) } else { None } });
    }
    let mut data_entries = vec![OwnedDataEntry {
        key: "__typename".to_string(),
        value: if parent_is_object { OwnedDataEntryValue::Typename(format!("{}.Objects.{}.typename", schema_namespace, naming::first_uppercased(swift_name))) } else { OwnedDataEntryValue::Variable("__typename".to_string()) },
    }];
    for a in all_field_accessors {
        let is_entity = parent_ds.fields.get(&a.name).map(|f| matches!(f, FieldSelection::Entity(_))).unwrap_or(false)
            || referenced_fragments.iter().any(|f| f.root_field.selection_set.direct_selections.fields.get(&a.name).map(|field| matches!(field, FieldSelection::Entity(_))).unwrap_or(false))
            || swift_type_is_entity(&a.swift_type);
        data_entries.push(OwnedDataEntry { key: a.name.clone(), value: if is_entity { OwnedDataEntryValue::FieldData(a.name.clone()) } else { OwnedDataEntryValue::Variable(a.name.clone()) } });
    }
    let mut fulfilled_fragments = vec![root_entity_type.to_string(), qualified_name.to_string(), naming::first_uppercased(fragment_name)];
    // Add extra fulfilled fragments (sibling supertype OIDs from the fragment)
    for extra in extra_fulfilled {
        if !fulfilled_fragments.contains(extra) {
            fulfilled_fragments.push(extra.clone());
        }
    }
    OwnedInitializerConfig { parameters, data_entries, fulfilled_fragments, typename_value }
}

/// Add a namespace prefix to non-Swift-scalar type names in a variable type string.
/// For example, with prefix "MySchemaModule.":
///   "ID" -> "MySchemaModule.ID"
///   "GraphQLEnum<RelativeSize>" -> "GraphQLEnum<MySchemaModule.RelativeSize>"
///   "GraphQLNullable<MeasurementsInput>" -> "GraphQLNullable<MySchemaModule.MeasurementsInput>"
///   "String" -> "String" (no change, Swift scalar)
///   "Int" -> "Int" (no change, Swift scalar)
pub fn add_namespace_to_variable_type(
    type_str: &str,
    prefix: &str,
    type_kinds: &HashMap<String, TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    // Swift scalar types that don't get a namespace prefix
    const SWIFT_SCALARS: &[&str] = &["String", "Int", "Double", "Bool"];

    // Find all type name tokens in the string and prefix the non-scalar ones.
    // Type names appear as bare identifiers or inside angle brackets/square brackets.
    // Strategy: scan for word-character sequences and replace those that are schema types.
    let mut result = String::new();
    let chars: Vec<char> = type_str.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // Skip string literals (content inside double-quotes)
        if chars[i] == '"' {
            result.push(chars[i]);
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    result.push(chars[i]);
                    i += 1;
                }
                result.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                result.push(chars[i]); // closing quote
                i += 1;
            }
        } else if chars[i].is_alphabetic() || chars[i] == '_' {
            // Collect a word
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            // Check if this word is a schema type that needs prefixing
            if !SWIFT_SCALARS.contains(&word.as_str())
                && word != "GraphQLNullable"
                && word != "GraphQLEnum"
                && word != "nil"
                && word != "init"
            {
                // Check if the customized name corresponds to a known schema type
                // or if it's a custom scalar (like ID), input object, or enum.
                // We prefix any non-Swift-scalar type name that appears in the type_kinds
                // or is a well-known custom scalar.
                let is_schema_type = type_kinds.contains_key(&word)
                    || customizer.reverse_lookup(&word).map(|orig| type_kinds.contains_key(orig)).unwrap_or(false)
                    || word == "ID"; // ID is always a schema type
                if is_schema_type {
                    result.push_str(prefix);
                }
            }
            result.push_str(&word);
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

/// Render a GraphQL field type as a Swift type string.
fn render_field_swift_type(
    field: &FieldSelection,
    schema_namespace: &str,
    type_kinds: &HashMap<String, TypeKind>,
    customizer: &SchemaCustomizer,
) -> (String, bool) {
    match field {
        FieldSelection::Scalar(sf) => {
            let swift_type = render_graphql_type_as_swift(&sf.field_type, schema_namespace, type_kinds, customizer);
            (swift_type, false)
        }
        FieldSelection::Entity(ef) => {
            // Entity fields use the singularized struct name from the response key for list types
            // Non-list types use the response key directly
            let struct_name = if ef.field_type.is_list() {
                naming::first_uppercased(&naming::singularize(ef.response_key()))
            } else {
                naming::first_uppercased(ef.response_key())
            };
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
    // Multi-line format when there are 2+ arguments (matching Swift behavior).
    // Entries are NOT indented here — the rendering function adds proper indent
    // for inner lines. The closing ']' is on its own line so the caller can indent it.
    if entries.len() > 1 {
        Some(format!("[\n{}\n]", entries.join(",\n")))
    } else {
        Some(format!("[{}]", entries.join(", ")))
    }
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
        GraphQLValue::Enum(e) => format!("\"{}\"", e),
        GraphQLValue::List(items) => {
            let rendered: Vec<String> = items.iter().map(render_argument_value).collect();
            format!("[{}]", rendered.join(", "))
        }
        GraphQLValue::Object(map) => {
            let entries: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("\"{}\": {}", k, render_argument_value(v)))
                .collect();
            if entries.len() > 1 {
                format!("[\n{}\n]", entries.join(",\n"))
            } else {
                format!("[{}]", entries.join(", "))
            }
        }
    }
}

/// Render a GraphQL type as a Swift type string for scalar fields.
fn render_graphql_type_as_swift(
    ty: &GraphQLType,
    schema_namespace: &str,
    type_kinds: &HashMap<String, TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    match ty {
        GraphQLType::Named(name) => render_named_type_as_swift(name, schema_namespace, type_kinds, customizer),
        GraphQLType::NonNull(inner) => {
            let inner_str = render_graphql_type_as_swift(inner, schema_namespace, type_kinds, customizer);
            // Remove trailing ? if present (NonNull removes optionality)
            if inner_str.ends_with('?') {
                inner_str[..inner_str.len() - 1].to_string()
            } else {
                inner_str
            }
        }
        GraphQLType::List(inner) => {
            let inner_str = render_graphql_type_as_swift(inner, schema_namespace, type_kinds, customizer);
            format!("[{}]?", inner_str)
        }
    }
}

fn render_named_type_as_swift(
    name: &str,
    schema_namespace: &str,
    type_kinds: &HashMap<String, TypeKind>,
    customizer: &SchemaCustomizer,
) -> String {
    match name {
        "String" => "String?".to_string(),
        "Int" => "Int?".to_string(),
        "Float" => "Double?".to_string(),
        "Boolean" => "Bool?".to_string(),
        "ID" => format!("{}.ID?", schema_namespace),
        _ => {
            let swift_name = customizer.custom_type_name(name);
            let kind = type_kinds
                .get(name)
                .copied()
                .unwrap_or(TypeKind::Scalar);
            match kind {
                TypeKind::Enum => format!("GraphQLEnum<{}.{}>?", schema_namespace, swift_name),
                TypeKind::Scalar => format!("{}.{}?", schema_namespace, swift_name),
                TypeKind::Object | TypeKind::Interface | TypeKind::Union => {
                    // Composite types used as scalars (e.g., custom JSON Object type)
                    format!("{}.{}?", schema_namespace, swift_name)
                }
                TypeKind::InputObject => format!("{}?", swift_name),
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
        include_definition: config.include_definition,
        operation_identifier: config.operation_identifier.as_deref(),
        query_string_format: config.query_string_format,
        api_target_name: &config.api_target_name,
        class_keyword: &config.class_keyword,
        init_access_modifier: &config.init_access_modifier,
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
        query_string_format: config.query_string_format,
        api_target_name: &config.api_target_name,
        include_definition: config.include_definition,
    };

    crate::templates::fragment::render(&template_config)
}

/// Get inclusion conditions from a FieldSelection.
fn field_inclusion_conditions(field: &FieldSelection) -> Option<&InclusionConditions> {
    match field {
        FieldSelection::Scalar(f) => f.inclusion_conditions.as_ref(),
        FieldSelection::Entity(f) => f.inclusion_conditions.as_ref(),
    }
}

/// Check if inclusion conditions are non-trivial (non-empty and present).
fn has_inclusion_conditions(conds: Option<&InclusionConditions>) -> bool {
    conds.map(|c| !c.is_empty()).unwrap_or(false)
}

/// Build a conditional struct name suffix from inclusion conditions.
/// E.g., `@include(if: $getCat)` on `... on Cat` -> "IfGetCat"
/// E.g., `@skip(if: $skipHeightInMeters)` on fragment spread -> "IfNotSkipHeightInMeters"
fn inclusion_condition_suffix(conds: &InclusionConditions) -> String {
    let mut parts = Vec::new();
    for cond in &conds.conditions {
        if cond.is_inverted {
            parts.push(format!("IfNot{}", naming::first_uppercased(&cond.variable)));
        } else {
            parts.push(format!("If{}", naming::first_uppercased(&cond.variable)));
        }
    }
    parts.join("")
}

/// Build a conditional struct name for an inline fragment with inclusion conditions.
/// E.g., `... on Cat @include(if: $getCat)` -> "AsCatIfGetCat"
/// For fragment spreads with conditions (no type condition): "IfNotSkipHeightInMeters"
/// When a schema customizer renames the type, uses the custom name (e.g., "AsCustomCatIfGetCat").
fn conditional_inline_fragment_name(type_condition: Option<&str>, conds: &InclusionConditions, customizer: &SchemaCustomizer) -> String {
    let suffix = inclusion_condition_suffix(conds);
    if let Some(tc) = type_condition {
        format!("As{}{}", naming::first_uppercased(customizer.custom_type_name(tc)), suffix)
    } else {
        suffix
    }
}

/// Build a conditional property name (camelCase) for an inline fragment accessor.
/// E.g., "AsWarmBloodedIfGetWarmBlooded" -> "asWarmBloodedIfGetWarmBlooded"
/// E.g., "IfNotSkipHeightInMeters" -> "ifNotSkipHeightInMeters"
fn conditional_inline_fragment_property(type_condition: Option<&str>, conds: &InclusionConditions, customizer: &SchemaCustomizer) -> String {
    let suffix = inclusion_condition_suffix(conds);
    if let Some(tc) = type_condition {
        format!("as{}{}", naming::first_uppercased(customizer.custom_type_name(tc)), suffix)
    } else {
        naming::first_lowercased(&suffix)
    }
}

/// Check if all conditions in `field_conds` are satisfied by `scope_conds`.
/// A field condition is satisfied if the scope already requires the same condition.
fn conditions_satisfied_by_scope(field_conds: &InclusionConditions, scope_conds: Option<&InclusionConditions>) -> bool {
    if let Some(scope) = scope_conds {
        field_conds.conditions.iter().all(|fc| {
            scope.conditions.iter().any(|sc| sc.variable == fc.variable && sc.is_inverted == fc.is_inverted)
        })
    } else {
        false
    }
}

/// Convert IR InclusionConditions to owned condition entries + operator.
fn inclusion_conditions_to_owned(ic: &InclusionConditions) -> (Vec<OwnedConditionEntry>, OwnedConditionOperator) {
    let entries = ic.conditions.iter().map(|c| OwnedConditionEntry {
        variable: c.variable.clone(),
        is_inverted: c.is_inverted,
    }).collect();
    let operator = match ic.effective_operator() {
        inclusion::InclusionOperator::And => OwnedConditionOperator::And,
        inclusion::InclusionOperator::Or => OwnedConditionOperator::Or,
    };
    (entries, operator)
}

/// Convert owned condition entries to a template InclusionConditionRef.
fn owned_conditions_to_ref<'a>(conditions: &'a [OwnedConditionEntry], operator: OwnedConditionOperator) -> InclusionConditionRef<'a> {
    InclusionConditionRef {
        conditions: conditions.iter().map(|c| ConditionEntry {
            variable: c.variable.as_str(),
            is_inverted: c.is_inverted,
        }).collect(),
        operator: match operator {
            OwnedConditionOperator::And => ConditionOperator::And,
            OwnedConditionOperator::Or => ConditionOperator::Or,
        },
    }
}

/// Collect ALL absorbed type names from the entire nested tree.
fn collect_all_absorbed_types(config: &OwnedSelectionSetConfig) -> Vec<String> {
    let mut all = config.absorbed_type_names.clone();
    for nested in &config.nested_types {
        all.extend(collect_all_absorbed_types(&nested.config));
    }
    all
}

fn owned_to_ref_selection_set(owned: &OwnedSelectionSetConfig) -> SelectionSetConfig<'_> {
    owned_to_ref_selection_set_with_absorbed(owned, &[])
}

fn owned_to_ref_selection_set_with_absorbed<'a>(owned: &'a OwnedSelectionSetConfig, parent_absorbed: &[String]) -> SelectionSetConfig<'a> {
    // Combine parent's absorbed types with our own
    let mut combined_absorbed: Vec<String> = parent_absorbed.to_vec();
    for atn in &owned.absorbed_type_names {
        if !combined_absorbed.contains(atn) {
            combined_absorbed.push(atn.clone());
        }
    }

    let parent_type = match &owned.parent_type {
        OwnedParentTypeRef::Object(n) => ParentTypeRef::Object(n.as_str()),
        OwnedParentTypeRef::Interface(n) => ParentTypeRef::Interface(n.as_str()),
        OwnedParentTypeRef::Union(n) => ParentTypeRef::Union(n.as_str()),
    };

    let selections: Vec<SelectionItem<'_>> = owned
        .selections
        .iter()
        .map(|s| match &s.kind {
            OwnedSelectionKind::Field { name, alias, swift_type, arguments } => {
                SelectionItem::Field(FieldSelectionItem {
                    name: name.as_str(),
                    alias: alias.as_deref(),
                    swift_type: swift_type.as_str(),
                    arguments: arguments.as_deref(),
                })
            }
            OwnedSelectionKind::InlineFragment(name) => SelectionItem::InlineFragment(name.as_str()),
            OwnedSelectionKind::Fragment(name) => SelectionItem::Fragment(name.as_str()),
            OwnedSelectionKind::ConditionalField { conditions, operator, name, alias, swift_type, arguments } => {
                SelectionItem::ConditionalField(
                    owned_conditions_to_ref(conditions, *operator),
                    FieldSelectionItem {
                        name: name.as_str(),
                        alias: alias.as_deref(),
                        swift_type: swift_type.as_str(),
                        arguments: arguments.as_deref(),
                    },
                )
            }
            OwnedSelectionKind::ConditionalInlineFragment { conditions, operator, type_name } => {
                SelectionItem::ConditionalInlineFragment(
                    owned_conditions_to_ref(conditions, *operator),
                    type_name.as_str(),
                )
            }
            OwnedSelectionKind::ConditionalFieldGroup { conditions, operator, fields } => {
                SelectionItem::ConditionalFieldGroup(
                    owned_conditions_to_ref(conditions, *operator),
                    fields.iter().map(|(n, a, st, args)| FieldSelectionItem {
                        name: n.as_str(),
                        alias: a.as_deref(),
                        swift_type: st.as_str(),
                        arguments: args.as_deref(),
                    }).collect(),
                )
            }
        })
        .collect();

    let field_accessors: Vec<FieldAccessor<'_>> = owned
        .field_accessors
        .iter()
        .map(|a| FieldAccessor {
            name: &a.name,
            swift_type: &a.swift_type,
            description: a.description.as_deref(),
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
            is_optional: a.is_optional,
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
        let fulfilled: Vec<&str> = init.fulfilled_fragments.iter()
            .filter(|s| {
                // Filter out self-referencing type-narrowing paths like AsPet.AsPet
                // (but not legitimate nested entity fields like Friend.Friend)
                let parts: Vec<&str> = s.split('.').collect();
                for w in parts.windows(2) {
                    if w[0] == w[1] && w[0].starts_with("As") { return false; }
                }
                // Filter out paths containing absorbed type names (from parent chain + own)
                for absorbed in &combined_absorbed {
                    if parts.contains(&absorbed.as_str()) {
                        return false;
                    }
                }
                true
            })
            .map(|s| s.as_str())
            .collect();
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
            config: owned_to_ref_selection_set_with_absorbed(&n.config, &combined_absorbed),
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
        type_alias_insert_index: owned.type_alias_insert_index,
        indent: owned.indent,
        access_modifier: &owned.access_modifier,
        is_mutable: owned.is_mutable,
        api_target_name: &owned.api_target_name,
    }
}
