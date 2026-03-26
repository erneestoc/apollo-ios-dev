//! IR Builder - transforms CompilationResult into IR.

use crate::fields::{EntityField, ScalarField};
use crate::inclusion::{InclusionCondition, InclusionConditions};
use crate::named_fragment::NamedFragment;
use crate::operation::{Operation, VariableDefinition};
use crate::schema::Schema;
use crate::scope::ScopeDescriptor;
use crate::selection_set::{
    DirectSelections, FieldSelection, InlineFragmentSelection, NamedFragmentSpread, SelectionSet,
};
use apollo_codegen_frontend::compilation_result::{CompilationResult, OperationType};
use apollo_codegen_frontend::types::*;
use std::collections::HashMap;
use std::sync::Arc;

/// Builds IR from a CompilationResult.
pub struct IRBuilder {
    pub schema: Schema,
    fragments: HashMap<String, Arc<NamedFragment>>,
}

impl IRBuilder {
    /// Build IR from a compilation result.
    pub fn build(result: &CompilationResult) -> Self {
        let schema = Schema::from_referenced_types(
            &result.referenced_types,
            result.schema_documentation.clone(),
        );

        let mut builder = IRBuilder {
            schema,
            fragments: HashMap::new(),
        };

        // Build fragments first (operations may reference them)
        for frag_def in &result.fragments {
            let frag = builder.build_fragment(frag_def, result);
            builder.fragments.insert(frag.name.clone(), Arc::new(frag));
        }

        builder
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
                type_str: render_graphql_type(&v.variable_type),
                default_value: v.default_value.as_ref().map(|dv| render_graphql_value(dv)),
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

                        direct.fields.insert(
                            response_key.to_string(),
                            FieldSelection::Entity(EntityField {
                                name: field.name.clone(),
                                alias: field.alias.clone(),
                                field_type: field.field_type.clone(),
                                arguments: field.arguments.clone().unwrap_or_default(),
                                inclusion_conditions: inclusion,
                                selection_set: sub_selection,
                                deprecation_reason: None,
                            }),
                        );
                    } else {
                        // Scalar field
                        direct.fields.insert(
                            response_key.to_string(),
                            FieldSelection::Scalar(ScalarField {
                                name: field.name.clone(),
                                alias: field.alias.clone(),
                                field_type: field.field_type.clone(),
                                arguments: field.arguments.clone().unwrap_or_default(),
                                inclusion_conditions: inclusion,
                                deprecation_reason: None,
                            }),
                        );
                    }
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

                    direct.inline_fragments.push(InlineFragmentSelection {
                        type_condition,
                        selection_set: sub_selection,
                        inclusion_conditions: inclusion,
                        is_deferred,
                        defer_label,
                    });
                }
                Selection::FragmentSpread(spread) => {
                    let inclusion = self.convert_inclusion_conditions(&spread.inclusion_conditions);
                    let (is_deferred, defer_label) = self.extract_defer_info(&spread.directives);

                    direct.named_fragments.push(NamedFragmentSpread {
                        fragment_name: spread.fragment_name.clone(),
                        inclusion_conditions: inclusion,
                        is_deferred,
                        defer_label,
                    });
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
}

/// Render a GraphQL type to its Swift variable type string.
///
/// Swift variable types differ from GraphQL notation:
/// - NonNull types are bare: `PetAdoptionInput` (not `PetAdoptionInput!`)
/// - Nullable types use `GraphQLNullable<T>`: `GraphQLNullable<PetSearchFilters>`
/// - List types follow GraphQL: `[Type]`
fn render_graphql_type(ty: &GraphQLType) -> String {
    match ty {
        GraphQLType::Named(name) => format!("GraphQLNullable<{}>", render_swift_variable_named_type(name)),
        GraphQLType::NonNull(inner) => render_graphql_type_nonnull(inner),
        GraphQLType::List(inner) => format!("GraphQLNullable<[{}]>", render_graphql_type_list_inner(inner)),
    }
}

/// Render the inner type for a NonNull wrapper (no GraphQLNullable wrapping).
fn render_graphql_type_nonnull(ty: &GraphQLType) -> String {
    match ty {
        GraphQLType::Named(name) => render_swift_variable_named_type(name),
        GraphQLType::NonNull(inner) => render_graphql_type_nonnull(inner),
        GraphQLType::List(inner) => format!("[{}]", render_graphql_type_list_inner(inner)),
    }
}

/// Render the inner type for a List wrapper.
fn render_graphql_type_list_inner(ty: &GraphQLType) -> String {
    match ty {
        GraphQLType::Named(name) => format!("{}?", render_swift_variable_named_type(name)),
        GraphQLType::NonNull(inner) => render_graphql_type_nonnull(inner),
        GraphQLType::List(inner) => format!("[{}]?", render_graphql_type_list_inner(inner)),
    }
}

/// Render a named type for Swift variable declarations.
fn render_swift_variable_named_type(name: &str) -> String {
    match name {
        "String" => "String".to_string(),
        "Int" => "Int".to_string(),
        "Float" => "Double".to_string(),
        "Boolean" => "Bool".to_string(),
        "ID" => "ID".to_string(),
        other => other.to_string(),
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
