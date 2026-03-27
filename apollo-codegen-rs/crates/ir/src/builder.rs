//! IR Builder - transforms CompilationResult into IR.

use crate::field_collector::{TypeKind, build_type_kinds};
use crate::fields::{EntityField, ScalarField};
use crate::inclusion::{InclusionCondition, InclusionConditions, InclusionOperator};
use crate::named_fragment::NamedFragment;
use crate::operation::{Operation, VariableDefinition};
use crate::schema::Schema;
use crate::scope::ScopeDescriptor;
use crate::selection_set::{
    DirectSelections, FieldSelection, InlineFragmentSelection, NamedFragmentSpread, SelectionKind,
    SelectionSet,
};
use apollo_codegen_frontend::compilation_result::{CompilationResult, OperationType};
use apollo_codegen_frontend::types::*;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::Arc;

/// Builds IR from a CompilationResult.
pub struct IRBuilder {
    pub schema: Schema,
    fragments: HashMap<String, Arc<NamedFragment>>,
    /// Map from type name to its kind (enum, input object, etc.)
    type_kinds: HashMap<String, TypeKind>,
    /// Map from input object type name to its field definitions
    input_object_fields: HashMap<String, IndexMap<String, GraphQLInputField>>,
    /// Map from (type_name, field_name) to field description for schema documentation
    field_descriptions: HashMap<(String, String), String>,
}

impl IRBuilder {
    /// Build IR from a compilation result.
    pub fn build(result: &CompilationResult) -> Self {
        let schema = Schema::from_referenced_types(
            &result.referenced_types,
            result.schema_documentation.clone(),
        );

        let type_kinds = build_type_kinds(result);

        let mut input_object_fields: HashMap<String, IndexMap<String, GraphQLInputField>> = HashMap::new();
        let mut field_descriptions: HashMap<(String, String), String> = HashMap::new();
        for named_type in &result.referenced_types {
            match named_type {
                GraphQLNamedType::InputObject(io) => {
                    input_object_fields.insert(io.name.clone(), io.fields.clone());
                }
                GraphQLNamedType::Object(obj) => {
                    for (fname, fdef) in &obj.fields {
                        if let Some(ref desc) = fdef.description {
                            field_descriptions.insert((obj.name.clone(), fname.clone()), desc.clone());
                        }
                    }
                }
                GraphQLNamedType::Interface(iface) => {
                    for (fname, fdef) in &iface.fields {
                        if let Some(ref desc) = fdef.description {
                            field_descriptions.insert((iface.name.clone(), fname.clone()), desc.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        let mut builder = IRBuilder {
            schema,
            fragments: HashMap::new(),
            type_kinds,
            input_object_fields,
            field_descriptions,
        };

        // Build fragments in dependency order (leaves first) so that when a fragment
        // references another fragment, the referenced one is already built and available
        // in `self.fragments` for lookup.
        let ordered_indices = topological_sort_fragments(&result.fragments);
        for idx in ordered_indices {
            let frag_def = &result.fragments[idx];
            let frag = builder.build_fragment(frag_def, result);
            builder.fragments.insert(frag.name.clone(), Arc::new(frag));
        }

        builder
    }

    /// Look up a field's schema description by parent type name and field name.
    pub fn field_description(&self, parent_type: &str, field_name: &str) -> Option<&str> {
        self.field_descriptions.get(&(parent_type.to_string(), field_name.to_string())).map(|s| s.as_str())
    }

    /// Clear all field descriptions (use when schema documentation should not be included).
    pub fn clear_field_descriptions(&mut self) {
        self.field_descriptions.clear();
    }

    /// Build an operation from its definition.
    pub fn build_operation(&self, op_def: &apollo_codegen_frontend::compilation_result::OperationDefinition) -> Operation {
        let root_type = op_def.root_type.clone();
        let selection_set = self.build_selection_set_from_compiled(
            &op_def.selection_set,
            &root_type,
        );

        let root_field = EntityField {
            name: root_field_name(op_def.operation_type).to_string(),
            alias: None,
            field_type: GraphQLType::Named(root_type.name().to_string()),
            arguments: vec![],
            inclusion_conditions: None,
            selection_set,
            deprecation_reason: None,
            description: None,
        };

        let referenced = op_def
            .referenced_fragments
            .iter()
            .filter_map(|name| self.fragments.get(name).cloned())
            .collect();

        let variables = op_def
            .variables
            .iter()
            .map(|v| VariableDefinition {
                name: v.name.clone(),
                type_str: render_graphql_type(&v.variable_type, &self.type_kinds),
                default_value: v.default_value.as_ref().map(|dv| {
                    self.render_swift_default_value(dv, &v.variable_type, 2)
                }),
            })
            .collect();

        Operation {
            name: op_def.name.clone(),
            operation_type: op_def.operation_type,
            root_field,
            referenced_fragments: referenced,
            is_local_cache_mutation: op_def.is_local_cache_mutation,
            source: op_def.source.clone(),
            file_path: op_def.file_path.clone(),
            contains_deferred_fragment: false, // TODO: detect @defer
            variables,
        }
    }

    /// Build a named fragment from its definition.
    fn build_fragment(
        &self,
        frag_def: &apollo_codegen_frontend::compilation_result::FragmentDefinition,
        _result: &CompilationResult,
    ) -> NamedFragment {
        let type_condition = frag_def.type_condition.clone();
        let selection_set = self.build_selection_set_from_compiled(
            &frag_def.selection_set,
            &type_condition,
        );

        let root_field = EntityField {
            name: frag_def.name.clone(),
            alias: None,
            field_type: GraphQLType::Named(type_condition.name().to_string()),
            arguments: vec![],
            inclusion_conditions: None,
            selection_set,
            deprecation_reason: None,
            description: None,
        };

        let referenced = frag_def
            .referenced_fragments
            .iter()
            .filter_map(|name| self.fragments.get(name).cloned())
            .collect();

        NamedFragment {
            name: frag_def.name.clone(),
            type_condition_name: type_condition.name().to_string(),
            root_field,
            referenced_fragments: referenced,
            is_local_cache_mutation: frag_def.is_local_cache_mutation,
            source: frag_def.source.clone(),
            file_path: frag_def.file_path.clone(),
            contains_deferred_fragment: false,
        }
    }

    /// Get all built fragments.
    pub fn fragments(&self) -> &HashMap<String, Arc<NamedFragment>> {
        &self.fragments
    }

    /// Build a SelectionSet from a compiled SelectionSet.
    fn build_selection_set_from_compiled(
        &self,
        compiled: &apollo_codegen_frontend::types::SelectionSet,
        parent_type: &GraphQLCompositeType,
    ) -> SelectionSet {
        let scope = ScopeDescriptor::new(parent_type.clone());
        let mut direct = DirectSelections::default();

        for selection in &compiled.selections {
            match selection {
                Selection::Field(field) => {
                    let response_key = field.alias.as_deref().unwrap_or(&field.name);
                    let inclusion = self.convert_inclusion_conditions(&field.inclusion_conditions);

                    // Check if this response key already exists (e.g., duplicate field
                    // declarations like `name @skip(if: $a)` + `name @include(if: $b)`).
                    // When merging, combine conditions with OR since the field is included
                    // if EITHER condition is met.
                    if let Some(existing) = direct.fields.get(response_key) {
                        let existing_conds = match existing {
                            FieldSelection::Scalar(f) => f.inclusion_conditions.as_ref(),
                            FieldSelection::Entity(f) => f.inclusion_conditions.as_ref(),
                        };
                        if let (Some(existing_ic), Some(new_ic)) = (existing_conds, &inclusion) {
                            // Merge: flatten both conditions into a single OR set
                            let mut merged = existing_ic.conditions.clone();
                            merged.extend(new_ic.conditions.clone());
                            let merged_inclusion = Some(InclusionConditions::from_conditions_with_operator(
                                merged,
                                InclusionOperator::Or,
                            ));
                            // Update the existing field's conditions
                            match direct.fields.get_mut(response_key).unwrap() {
                                FieldSelection::Scalar(f) => f.inclusion_conditions = merged_inclusion,
                                FieldSelection::Entity(f) => f.inclusion_conditions = merged_inclusion,
                            }
                            continue;
                        }
                        // If one has no conditions, the field is unconditional.
                        // For entity fields: if the new conditional field has sub-selections,
                        // add them as a conditional inline fragment within the existing entity's
                        // sub-selection set, so the conditional selections are preserved.
                        if let (FieldSelection::Entity(existing_ef), Some(new_ic)) = (direct.fields.get_mut(response_key).unwrap(), &inclusion) {
                            if let Some(ref new_ss) = field.selection_set {
                                let sub_type = infer_composite_type(&field.field_type, &field.name, &self.schema);
                                let new_sub = self.build_selection_set_from_compiled(new_ss, &sub_type);
                                // Create a conditional inline fragment with no type condition
                                // but with the inclusion conditions from the conditional field.
                                existing_ef.selection_set.direct_selections.inline_fragments.push(
                                    InlineFragmentSelection {
                                        type_condition: None,
                                        selection_set: new_sub,
                                        inclusion_conditions: Some(new_ic.clone()),
                                        is_deferred: false,
                                        defer_label: None,
                                    }
                                );
                            }
                        }
                        continue;
                    }

                    let rk = response_key.to_string();
                    if field.selection_set.is_some() {
                        // Entity field
                        let sub_type = infer_composite_type(&field.field_type, &field.name, &self.schema);
                        let sub_selection = field.selection_set.as_ref().map(|ss| {
                            self.build_selection_set_from_compiled(ss, &sub_type)
                        }).unwrap_or_else(|| SelectionSet {
                            scope: ScopeDescriptor::new(sub_type.clone()),
                            direct_selections: DirectSelections::default(),
                            needs_typename: false,
                        });

                        let desc = self.field_description(parent_type.name(), &field.name).map(|s| s.to_string());
                        direct.fields.insert(
                            rk.clone(),
                            FieldSelection::Entity(EntityField {
                                name: field.name.clone(),
                                alias: field.alias.clone(),
                                field_type: field.field_type.clone(),
                                arguments: field.arguments.clone().unwrap_or_default(),
                                inclusion_conditions: inclusion,
                                selection_set: sub_selection,
                                deprecation_reason: None,
                                description: desc,
                            }),
                        );
                    } else {
                        // Scalar field
                        let desc = self.field_description(parent_type.name(), &field.name).map(|s| s.to_string());
                        direct.fields.insert(
                            rk.clone(),
                            FieldSelection::Scalar(ScalarField {
                                name: field.name.clone(),
                                alias: field.alias.clone(),
                                field_type: field.field_type.clone(),
                                arguments: field.arguments.clone().unwrap_or_default(),
                                inclusion_conditions: inclusion,
                                deprecation_reason: None,
                                description: desc,
                            }),
                        );
                    }
                    direct.source_order.push(SelectionKind::Field(rk));
                }
                Selection::InlineFragment(inline) => {
                    let type_condition = inline.type_condition.clone();
                    let sub_parent = type_condition
                        .as_ref()
                        .unwrap_or(parent_type);
                    let sub_selection = self.build_selection_set_from_compiled(
                        &inline.selection_set,
                        sub_parent,
                    );
                    let inclusion = self.convert_inclusion_conditions(&inline.inclusion_conditions);

                    // Check for @defer
                    let (is_deferred, defer_label) = self.extract_defer_info(&inline.directives);

                    let inline_idx = direct.inline_fragments.len();
                    direct.inline_fragments.push(InlineFragmentSelection {
                        type_condition,
                        selection_set: sub_selection,
                        inclusion_conditions: inclusion,
                        is_deferred,
                        defer_label,
                    });
                    direct.source_order.push(SelectionKind::InlineFragment(inline_idx));
                }
                Selection::FragmentSpread(spread) => {
                    let inclusion = self.convert_inclusion_conditions(&spread.inclusion_conditions);
                    let (is_deferred, defer_label) = self.extract_defer_info(&spread.directives);

                    let frag_idx = direct.named_fragments.len();
                    direct.named_fragments.push(NamedFragmentSpread {
                        fragment_name: spread.fragment_name.clone(),
                        inclusion_conditions: inclusion,
                        is_deferred,
                        defer_label,
                    });
                    direct.source_order.push(SelectionKind::NamedFragment(frag_idx));
                }
            }
        }

        // Determine if __typename is needed
        let needs_typename = !direct.inline_fragments.is_empty()
            || !direct.named_fragments.is_empty();

        SelectionSet {
            scope,
            direct_selections: direct,
            needs_typename,
        }
    }

    fn convert_inclusion_conditions(
        &self,
        conditions: &Option<Vec<apollo_codegen_frontend::types::InclusionCondition>>,
    ) -> Option<InclusionConditions> {
        conditions.as_ref().map(|conds| {
            InclusionConditions::from_conditions(
                conds
                    .iter()
                    .map(|c| InclusionCondition {
                        variable: c.variable.clone(),
                        is_inverted: c.is_inverted,
                    })
                    .collect(),
            )
        })
    }

    fn extract_defer_info(
        &self,
        directives: &Option<Vec<Directive>>,
    ) -> (bool, Option<String>) {
        if let Some(dirs) = directives {
            for dir in dirs {
                if dir.name == "defer" {
                    let label = dir.arguments.as_ref().and_then(|args| {
                        args.iter()
                            .find(|a| a.name == "label")
                            .and_then(|a| match &a.value {
                                GraphQLValue::String(s) => Some(s.clone()),
                                _ => None,
                            })
                    });
                    return (true, label);
                }
            }
        }
        (false, None)
    }

    /// Render a GraphQL default value as a Swift expression for an operation variable.
    ///
    /// This is type-aware: it uses the GraphQL type to determine type names for
    /// input objects, whether to wrap enum values, etc.
    ///
    /// `indent` is the base indentation level (number of spaces) for the enclosing scope.
    fn render_swift_default_value(
        &self,
        val: &GraphQLValue,
        graphql_type: &GraphQLType,
        indent: usize,
    ) -> String {
        // If the type is nullable (Named or List without NonNull), wrap in .init(...)
        let is_nullable = matches!(graphql_type, GraphQLType::Named(_) | GraphQLType::List(_));
        let inner_type = match graphql_type {
            GraphQLType::NonNull(inner) => inner.as_ref(),
            _ => graphql_type,
        };

        if is_nullable {
            let is_complex = matches!(val, GraphQLValue::Object(_));
            if is_complex {
                // Multi-line .init() wrapper for complex values
                let content_indent = indent + 2;
                let inner_rendered = self.render_swift_value_for_type(val, inner_type, content_indent);
                format!(".init(\n{}{}\n{})",
                    " ".repeat(content_indent),
                    inner_rendered,
                    " ".repeat(indent))
            } else {
                // Inline .init() for simple values
                let inner_rendered = self.render_swift_value_for_type(val, inner_type, indent);
                format!(".init({})", inner_rendered)
            }
        } else {
            self.render_swift_value_for_type(val, inner_type, indent)
        }
    }

    /// Render a value given the "unwrapped" (non-null) type.
    fn render_swift_value_for_type(
        &self,
        val: &GraphQLValue,
        graphql_type: &GraphQLType,
        indent: usize,
    ) -> String {
        match val {
            GraphQLValue::Object(map) => {
                // Input object: render as TypeName(field1: value1, field2: value2)
                let type_name = graphql_type.named_type();
                self.render_input_object_value(type_name, map, indent)
            }
            GraphQLValue::Enum(e) => {
                // Enum: render as .camelCasedValue
                let camel = to_camel_case(e);
                format!(".{}", camel)
            }
            GraphQLValue::String(s) => format!("\"{}\"", s),
            GraphQLValue::Int(i) => i.to_string(),
            GraphQLValue::Float(f) => render_swift_float(*f),
            GraphQLValue::Boolean(b) => b.to_string(),
            GraphQLValue::Null => "nil".to_string(),
            GraphQLValue::List(list) => {
                // Determine inner type for list elements
                let elem_type = match graphql_type {
                    GraphQLType::List(inner) => {
                        // Unwrap NonNull wrapper if present
                        match inner.as_ref() {
                            GraphQLType::NonNull(inner2) => inner2.as_ref(),
                            other => other,
                        }
                    }
                    _ => graphql_type,
                };
                let items: Vec<String> = list
                    .iter()
                    .map(|item| self.render_swift_value_for_type(item, elem_type, indent))
                    .collect();
                format!("[{}]", items.join(", "))
            }
            GraphQLValue::Variable(v) => format!("${}", v),
        }
    }

    /// Render a nullable field value, wrapping in `.init()` if needed.
    fn render_nullable_field_value(
        &self,
        val: &GraphQLValue,
        field_type: &GraphQLType,
        indent: usize,
    ) -> String {
        let is_nullable = matches!(field_type, GraphQLType::Named(_) | GraphQLType::List(_));
        let inner_type = match field_type {
            GraphQLType::NonNull(inner) => inner.as_ref(),
            _ => field_type,
        };

        if is_nullable && !matches!(val, GraphQLValue::Null) {
            let is_complex = matches!(val, GraphQLValue::Object(_));
            if is_complex {
                // Multi-line .init() for complex nested values
                let content_indent = indent + 2;
                let inner_rendered = self.render_swift_value_for_type(val, inner_type, content_indent);
                format!(".init(\n{}{}\n{})",
                    " ".repeat(content_indent),
                    inner_rendered,
                    " ".repeat(indent))
            } else {
                let inner_rendered = self.render_swift_value_for_type(val, inner_type, indent);
                format!(".init({})", inner_rendered)
            }
        } else if matches!(val, GraphQLValue::Null) {
            "nil".to_string()
        } else {
            self.render_swift_value_for_type(val, inner_type, indent)
        }
    }

    /// Render an input object value as `TypeName(field1: value1, field2: value2)`.
    fn render_input_object_value(
        &self,
        type_name: &str,
        map: &IndexMap<String, GraphQLValue>,
        indent: usize,
    ) -> String {
        let fields = self.input_object_fields.get(type_name);
        let inner_indent = indent + 2;

        let mut field_strs = Vec::new();
        for (key, value) in map {
            let field_type = fields
                .and_then(|f| f.get(key))
                .map(|f| &f.field_type);

            let rendered_value = if let Some(ft) = field_type {
                self.render_nullable_field_value(value, ft, inner_indent)
            } else {
                // Fallback: render without type context
                render_graphql_value(value)
            };

            field_strs.push(format!("{}: {}", key, rendered_value));
        }

        format!("{}(\n{}{}\n{})",
            type_name,
            " ".repeat(inner_indent),
            field_strs.join(&format!(",\n{}", " ".repeat(inner_indent))),
            " ".repeat(indent))
    }
}

/// Convert a SCREAMING_SNAKE_CASE enum value to camelCase.
fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    let mut first = true;

    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_uppercase().next().unwrap_or(c));
            capitalize_next = false;
        } else if first {
            result.push(c.to_lowercase().next().unwrap_or(c));
            first = false;
        } else {
            result.push(c.to_lowercase().next().unwrap_or(c));
        }
    }

    result
}

/// Render a float value ensuring it always has a decimal point.
fn render_swift_float(f: f64) -> String {
    let s = f.to_string();
    if s.contains('.') {
        s
    } else {
        format!("{}.0", s)
    }
}

/// Render a GraphQL type to its Swift variable type string.
///
/// Swift variable types differ from GraphQL notation:
/// - NonNull types are bare: `PetAdoptionInput` (not `PetAdoptionInput!`)
/// - Nullable types use `GraphQLNullable<T>`: `GraphQLNullable<PetSearchFilters>`
/// - List types follow GraphQL: `[Type]`
fn render_graphql_type(ty: &GraphQLType, type_kinds: &HashMap<String, TypeKind>) -> String {
    match ty {
        GraphQLType::Named(name) => format!("GraphQLNullable<{}>", render_swift_variable_named_type(name, type_kinds)),
        GraphQLType::NonNull(inner) => render_graphql_type_nonnull(inner, type_kinds),
        GraphQLType::List(inner) => format!("GraphQLNullable<[{}]>", render_graphql_type_list_inner(inner, type_kinds)),
    }
}

/// Render the inner type for a NonNull wrapper (no GraphQLNullable wrapping).
fn render_graphql_type_nonnull(ty: &GraphQLType, type_kinds: &HashMap<String, TypeKind>) -> String {
    match ty {
        GraphQLType::Named(name) => render_swift_variable_named_type(name, type_kinds),
        GraphQLType::NonNull(inner) => render_graphql_type_nonnull(inner, type_kinds),
        GraphQLType::List(inner) => format!("[{}]", render_graphql_type_list_inner(inner, type_kinds)),
    }
}

/// Render the inner type for a List wrapper.
fn render_graphql_type_list_inner(ty: &GraphQLType, type_kinds: &HashMap<String, TypeKind>) -> String {
    match ty {
        GraphQLType::Named(name) => format!("{}?", render_swift_variable_named_type(name, type_kinds)),
        GraphQLType::NonNull(inner) => render_graphql_type_nonnull(inner, type_kinds),
        GraphQLType::List(inner) => format!("[{}]?", render_graphql_type_list_inner(inner, type_kinds)),
    }
}

/// Render a named type for Swift variable declarations.
/// Enum types are wrapped in `GraphQLEnum<>` to match Swift codegen behavior.
fn render_swift_variable_named_type(name: &str, type_kinds: &HashMap<String, TypeKind>) -> String {
    match name {
        "String" => "String".to_string(),
        "Int" => "Int".to_string(),
        "Float" => "Double".to_string(),
        "Boolean" => "Bool".to_string(),
        "ID" => "ID".to_string(),
        other => {
            if matches!(type_kinds.get(other), Some(TypeKind::Enum)) {
                format!("GraphQLEnum<{}>", other)
            } else {
                other.to_string()
            }
        }
    }
}

/// Render a GraphQL value to a string.
fn render_graphql_value(val: &GraphQLValue) -> String {
    match val {
        GraphQLValue::String(s) => format!("\"{}\"", s),
        GraphQLValue::Int(i) => i.to_string(),
        GraphQLValue::Float(f) => f.to_string(),
        GraphQLValue::Boolean(b) => b.to_string(),
        GraphQLValue::Null => "null".to_string(),
        GraphQLValue::Enum(e) => e.clone(),
        GraphQLValue::List(list) => {
            let items: Vec<String> = list.iter().map(render_graphql_value).collect();
            format!("[{}]", items.join(", "))
        }
        GraphQLValue::Object(map) => {
            let items: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", k, render_graphql_value(v)))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
        GraphQLValue::Variable(v) => format!("${}", v),
    }
}

/// Infer a composite type from a GraphQL type, using the schema to
/// determine the correct kind (Object, Interface, or Union).
fn infer_composite_type(ty: &GraphQLType, _field_name: &str, schema: &Schema) -> GraphQLCompositeType {
    let named = ty.named_type();

    // Look up in schema's referenced types
    for obj in &schema.referenced_types.objects {
        if obj.name == named {
            return GraphQLCompositeType::Object(obj.clone());
        }
    }
    for iface in &schema.referenced_types.interfaces {
        if iface.name == named {
            return GraphQLCompositeType::Interface(iface.clone());
        }
    }
    for union_t in &schema.referenced_types.unions {
        if union_t.name == named {
            return GraphQLCompositeType::Union(union_t.clone());
        }
    }

    // Fallback: create a minimal object type (should not happen with a valid schema)
    GraphQLCompositeType::Object(GraphQLObjectType {
        name: named.to_string(),
        description: None,
        fields: Default::default(),
        interfaces: vec![],
    })
}

fn root_field_name(op_type: OperationType) -> &'static str {
    match op_type {
        OperationType::Query => "query",
        OperationType::Mutation => "mutation",
        OperationType::Subscription => "subscription",
    }
}

/// Topologically sort fragment definitions so that dependencies are built before
/// the fragments that reference them. This ensures that when building fragment A
/// which references fragment B, B is already available in `self.fragments`.
fn topological_sort_fragments(
    fragments: &[apollo_codegen_frontend::compilation_result::FragmentDefinition],
) -> Vec<usize> {
    use std::collections::{HashMap, HashSet};

    // Build name-to-index map
    let name_to_idx: HashMap<&str, usize> = fragments
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name.as_str(), i))
        .collect();

    let mut order = Vec::with_capacity(fragments.len());
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new(); // cycle detection

    fn visit(
        idx: usize,
        fragments: &[apollo_codegen_frontend::compilation_result::FragmentDefinition],
        name_to_idx: &HashMap<&str, usize>,
        visited: &mut HashSet<usize>,
        visiting: &mut HashSet<usize>,
        order: &mut Vec<usize>,
    ) {
        if visited.contains(&idx) {
            return;
        }
        if visiting.contains(&idx) {
            // Cycle detected - just skip to avoid infinite loop
            return;
        }
        visiting.insert(idx);
        // Visit dependencies first
        for dep_name in &fragments[idx].referenced_fragments {
            if let Some(&dep_idx) = name_to_idx.get(dep_name.as_str()) {
                visit(dep_idx, fragments, name_to_idx, visited, visiting, order);
            }
        }
        visiting.remove(&idx);
        visited.insert(idx);
        order.push(idx);
    }

    for i in 0..fragments.len() {
        visit(i, fragments, &name_to_idx, &mut visited, &mut visiting, &mut order);
    }

    order
}
