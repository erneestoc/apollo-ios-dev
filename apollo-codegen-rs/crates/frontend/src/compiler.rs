//! GraphQL frontend that loads schemas, parses operations, and compiles
//! them into a CompilationResult.

use crate::compilation_result::*;
use crate::introspection;
use crate::types::*;
use apollo_compiler::executable::{self, Selection as AcSelection};
use apollo_compiler::schema::ExtendedType;
use apollo_compiler::validation::Valid;
use apollo_compiler::{ExecutableDocument, Node, Schema};
use indexmap::IndexSet;
use std::collections::BTreeMap;

/// Helper to get a directive argument value from an AST DirectiveList.
/// This avoids the `argument_by_name` API which requires a schema reference.
fn get_directive_arg_string(
    directives: &apollo_compiler::ast::DirectiveList,
    directive_name: &str,
    arg_name: &str,
) -> Option<String> {
    directives.iter().find(|d| d.name == directive_name).and_then(|d| {
        d.arguments.iter().find(|a| a.name == arg_name).and_then(|a| {
            if let apollo_compiler::ast::Value::String(s) = a.value.as_ref() {
                Some(s.to_string())
            } else {
                None
            }
        })
    })
}

fn has_directive(directives: &apollo_compiler::ast::DirectiveList, name: &str) -> bool {
    directives.iter().any(|d| d.name == name)
}

fn has_schema_directive(directives: &apollo_compiler::schema::DirectiveList, name: &str) -> bool {
    directives.iter().any(|d| d.name == name)
}

fn get_schema_directive_arg_string(
    directives: &apollo_compiler::schema::DirectiveList,
    directive_name: &str,
    arg_name: &str,
) -> Option<String> {
    directives.iter().find(|d| d.name == directive_name).and_then(|d| {
        d.arguments.iter().find(|a| a.name == arg_name).and_then(|a| {
            if let apollo_compiler::ast::Value::String(s) = a.value.as_ref() {
                Some(s.to_string())
            } else {
                None
            }
        })
    })
}

fn get_directive_arg_value<'a>(
    directive: &'a apollo_compiler::ast::Directive,
    arg_name: &str,
) -> Option<&'a apollo_compiler::ast::Value> {
    directive
        .arguments
        .iter()
        .find(|a| a.name == arg_name)
        .map(|a| a.value.as_ref())
}

/// The main GraphQL frontend that replaces GraphQLJSFrontend.
pub struct GraphQLFrontend {
    schema: Valid<Schema>,
}

/// Configuration for the compilation.
pub struct CompileOptions {
    pub legacy_safelisting_compatible_operations: bool,
    pub reduce_generated_schema_types: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            legacy_safelisting_compatible_operations: false,
            reduce_generated_schema_types: false,
        }
    }
}

impl GraphQLFrontend {
    /// Load a schema from one or more source files (SDL or introspection JSON).
    pub fn load_schema(sources: &[(String, String)]) -> Result<Self, Vec<String>> {
        let mut builder = Schema::builder();
        let mut _had_introspection = false;

        for (content, file_path) in sources {
            // Detect introspection JSON by checking if it starts with '{'
            let trimmed = content.trim();
            if trimmed.starts_with('{') {
                // Introspection JSON - convert to SDL first
                let sdl = introspection::introspection_json_to_sdl(content)
                    .map_err(|e| vec![e])?;
                builder = builder.parse(sdl, file_path);
                _had_introspection = true;
            } else {
                builder = builder.parse(content, file_path);
            }
        }

        // Add Apollo-specific directives if not already present
        let apollo_directives = r#"
directive @apollo_client_ios_localCacheMutation on QUERY | MUTATION | SUBSCRIPTION | FRAGMENT_DEFINITION
directive @typePolicy(keyFields: String!) on OBJECT | INTERFACE
directive @fieldPolicy(forField: String!, keyArgs: String!) on FIELD_DEFINITION
"#;
        builder = builder.parse(apollo_directives, "apollo_extensions.graphql");

        let schema = builder.build().map_err(|e| {
            e.errors
                .iter()
                .map(|d| format!("{}", d))
                .collect::<Vec<_>>()
        })?;

        let valid = schema.validate().map_err(|e| {
            e.errors
                .iter()
                .map(|d| format!("{}", d))
                .collect::<Vec<_>>()
        })?;

        Ok(Self { schema: valid })
    }

    /// Parse and merge multiple operation documents.
    pub fn parse_operations(
        &self,
        sources: &[(String, String)],
    ) -> Result<ExecutableDocument, Vec<String>> {
        if sources.is_empty() {
            return Err(vec!["No operation sources provided".to_string()]);
        }

        // Parse all documents
        let mut all_operations = Vec::new();
        let mut all_fragments = Vec::new();

        for (content, file_path) in sources {
            let doc = ExecutableDocument::parse(&self.schema, content, file_path).map_err(|e| {
                e.errors
                    .iter()
                    .map(|d| format!("{}", d))
                    .collect::<Vec<_>>()
            })?;

            for op in doc.operations.iter() {
                all_operations.push((file_path.clone(), op.clone()));
            }
            for (name, frag) in &doc.fragments {
                all_fragments.push((file_path.clone(), name.clone(), frag.clone()));
            }
        }

        // Build merged document by parsing all sources together
        let combined_source: String = sources
            .iter()
            .map(|(content, _)| content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let merged = ExecutableDocument::parse(&self.schema, &combined_source, "merged.graphql")
            .map_err(|e| {
                e.errors
                    .iter()
                    .map(|d| format!("{}", d))
                    .collect::<Vec<_>>()
            })?;

        Ok(merged)
    }

    /// Validate operations against the schema.
    pub fn validate_operations(
        &self,
        doc: ExecutableDocument,
    ) -> Result<Valid<ExecutableDocument>, Vec<String>> {
        match doc.validate(&self.schema) {
            Ok(valid) => Ok(valid),
            Err(e) => Err(e
                .errors
                .iter()
                .map(|d| format!("{}", d))
                .collect()),
        }
    }

    /// Compile schema and operations into a CompilationResult.
    pub fn compile(
        &self,
        doc: &ExecutableDocument,
        source_map: &BTreeMap<String, (String, String)>,
        options: &CompileOptions,
    ) -> Result<CompilationResult, Vec<String>> {
        let schema = &*self.schema;

        // Build root types
        let query_type_name = schema
            .root_operation(apollo_compiler::ast::OperationType::Query)
            .ok_or_else(|| vec!["Schema must have a query type".to_string()])?;
        let mutation_type_name =
            schema.root_operation(apollo_compiler::ast::OperationType::Mutation);
        let subscription_type_name =
            schema.root_operation(apollo_compiler::ast::OperationType::Subscription);

        let root_types = RootTypeDefinition {
            query_type: self.build_named_type(query_type_name.as_str(), schema),
            mutation_type: mutation_type_name
                .map(|n| self.build_named_type(n.as_str(), schema)),
            subscription_type: subscription_type_name
                .map(|n| self.build_named_type(n.as_str(), schema)),
        };

        // Build referenced types
        let referenced_types = self.collect_referenced_types(schema, doc, options);

        // Build operations
        let mut operations = Vec::new();
        for op in doc.operations.iter() {
            if let Some(name) = &op.name {
                let op_type = match op.operation_type {
                    apollo_compiler::ast::OperationType::Query => OperationType::Query,
                    apollo_compiler::ast::OperationType::Mutation => OperationType::Mutation,
                    apollo_compiler::ast::OperationType::Subscription => {
                        OperationType::Subscription
                    }
                };

                // Get the source text for this operation
                let source = self.print_operation(op);

                // Determine file path from source mapping
                let file_path = self.find_file_path_for_operation(name, source_map);

                // Check for local cache mutation directive
                let is_local_cache_mutation = op.directives.iter().any(|d| {
                    d.name.as_str() == "apollo_client_ios_localCacheMutation"
                });

                // Build variables
                let variables = op
                    .variables
                    .iter()
                    .map(|v| VariableDefinition {
                        name: v.name.to_string(),
                        variable_type: self.convert_ast_type(&v.ty),
                        default_value: v.default_value.as_ref().map(|dv| self.convert_value(dv)),
                    })
                    .collect();

                // Build selection set
                let root_type_name = op.object_type();
                let root_composite = self.build_composite_type(root_type_name.as_str(), schema);
                let selection_set = self.build_selection_set(&op.selection_set, schema);

                // Collect referenced fragments
                let referenced_fragments = self.collect_referenced_fragments_for_op(op, doc);

                operations.push(OperationDefinition {
                    name: name.to_string(),
                    operation_type: op_type,
                    variables,
                    root_type: root_composite,
                    selection_set,
                    directives: Some(self.convert_directives(&op.directives)),
                    referenced_fragments,
                    source,
                    file_path,
                    is_local_cache_mutation,
                    module_imports: IndexSet::new(),
                });
            }
        }

        // Build fragments
        let mut fragments = Vec::new();
        for (name, frag) in &doc.fragments {
            let source = self.print_fragment(frag);
            let file_path = self.find_file_path_for_fragment(name, source_map);

            let is_local_cache_mutation = frag.directives.iter().any(|d| {
                d.name.as_str() == "apollo_client_ios_localCacheMutation"
            });

            let type_condition =
                self.build_composite_type(frag.type_condition().as_str(), schema);
            let selection_set = self.build_selection_set(&frag.selection_set, schema);
            let referenced_fragments = self.collect_referenced_fragments_for_frag(frag, doc);

            fragments.push(FragmentDefinition {
                name: name.to_string(),
                type_condition,
                selection_set,
                directives: Some(self.convert_directives(&frag.directives)),
                referenced_fragments,
                source,
                file_path,
                is_local_cache_mutation,
                module_imports: IndexSet::new(),
            });
        }

        Ok(CompilationResult {
            root_types,
            referenced_types,
            operations,
            fragments,
            schema_documentation: schema.schema_definition.description.as_ref().map(|d| d.to_string()),
        })
    }

    // --- Internal helpers ---

    fn build_named_type(&self, name: &str, schema: &Schema) -> GraphQLNamedType {
        match schema.types.get(name) {
            Some(ExtendedType::Scalar(s)) => GraphQLNamedType::Scalar(GraphQLScalarType {
                name: name.to_string(),
                description: s.description.as_ref().map(|d| d.to_string()),
                specified_by_url: None,
            }),
            Some(ExtendedType::Object(o)) => GraphQLNamedType::Object(GraphQLObjectType {
                name: name.to_string(),
                description: o.description.as_ref().map(|d| d.to_string()),
                fields: o
                    .fields
                    .iter()
                    .map(|(fname, fdef)| {
                        (
                            fname.to_string(),
                            self.convert_field_def(fdef),
                        )
                    })
                    .collect(),
                interfaces: o
                    .implements_interfaces
                    .iter()
                    .map(|i| i.name.to_string())
                    .collect(),
            }),
            Some(ExtendedType::Interface(i)) => {
                let implementers = schema.implementers_map();
                let implementing_objects = implementers
                    .get(name)
                    .map(|imp| imp.objects.iter().map(|n| n.to_string()).collect())
                    .unwrap_or_default();

                GraphQLNamedType::Interface(GraphQLInterfaceType {
                    name: name.to_string(),
                    description: i.description.as_ref().map(|d| d.to_string()),
                    fields: i
                        .fields
                        .iter()
                        .map(|(fname, fdef)| {
                            (
                                fname.to_string(),
                                self.convert_field_def(fdef),
                            )
                        })
                        .collect(),
                    interfaces: i
                        .implements_interfaces
                        .iter()
                        .map(|iface| iface.name.to_string())
                        .collect(),
                    implementing_objects,
                })
            }
            Some(ExtendedType::Union(u)) => GraphQLNamedType::Union(GraphQLUnionType {
                name: name.to_string(),
                description: u.description.as_ref().map(|d| d.to_string()),
                member_types: u.members.iter().map(|m| m.name.to_string()).collect(),
            }),
            Some(ExtendedType::Enum(e)) => GraphQLNamedType::Enum(GraphQLEnumType {
                name: name.to_string(),
                description: e.description.as_ref().map(|d| d.to_string()),
                values: e
                    .values
                    .iter()
                    .map(|(_, vdef)| {
                        let is_deprecated = has_directive(&vdef.directives, "deprecated");
                        let deprecation_reason =
                            get_directive_arg_string(&vdef.directives, "deprecated", "reason");
                        GraphQLEnumValue {
                            name: vdef.value.to_string(),
                            description: vdef.description.as_ref().map(|d| d.to_string()),
                            deprecation_reason: deprecation_reason.clone(),
                            is_deprecated,
                        }
                    })
                    .collect(),
            }),
            Some(ExtendedType::InputObject(io)) => {
                GraphQLNamedType::InputObject(GraphQLInputObjectType {
                    name: name.to_string(),
                    description: io.description.as_ref().map(|d| d.to_string()),
                    fields: io
                        .fields
                        .iter()
                        .map(|(fname, fdef)| {
                            (
                                fname.to_string(),
                                GraphQLInputField {
                                    name: fname.to_string(),
                                    field_type: self.convert_schema_type(&fdef.ty),
                                    description: fdef
                                        .description
                                        .as_ref()
                                        .map(|d| d.to_string()),
                                    default_value: fdef
                                        .default_value
                                        .as_ref()
                                        .map(|v| self.convert_ast_value(v)),
                                    deprecation_reason: None,
                                    is_deprecated: has_directive(&fdef.directives, "deprecated"),
                                },
                            )
                        })
                        .collect(),
                    is_one_of: has_schema_directive(&io.directives, "oneOf"),
                })
            }
            None => GraphQLNamedType::Scalar(GraphQLScalarType {
                name: name.to_string(),
                description: None,
                specified_by_url: None,
            }),
        }
    }

    fn build_composite_type(&self, name: &str, schema: &Schema) -> GraphQLCompositeType {
        match schema.types.get(name) {
            Some(ExtendedType::Object(o)) => {
                GraphQLCompositeType::Object(GraphQLObjectType {
                    name: name.to_string(),
                    description: o.description.as_ref().map(|d| d.to_string()),
                    fields: o
                        .fields
                        .iter()
                        .map(|(fname, fdef)| {
                            (fname.to_string(), self.convert_field_def(fdef))
                        })
                        .collect(),
                    interfaces: o
                        .implements_interfaces
                        .iter()
                        .map(|i| i.name.to_string())
                        .collect(),
                })
            }
            Some(ExtendedType::Interface(i)) => {
                let implementers = schema.implementers_map();
                let implementing_objects = implementers
                    .get(name)
                    .map(|imp| imp.objects.iter().map(|n| n.to_string()).collect())
                    .unwrap_or_default();

                GraphQLCompositeType::Interface(GraphQLInterfaceType {
                    name: name.to_string(),
                    description: i.description.as_ref().map(|d| d.to_string()),
                    fields: i
                        .fields
                        .iter()
                        .map(|(fname, fdef)| {
                            (fname.to_string(), self.convert_field_def(fdef))
                        })
                        .collect(),
                    interfaces: i
                        .implements_interfaces
                        .iter()
                        .map(|iface| iface.name.to_string())
                        .collect(),
                    implementing_objects,
                })
            }
            Some(ExtendedType::Union(u)) => GraphQLCompositeType::Union(GraphQLUnionType {
                name: name.to_string(),
                description: u.description.as_ref().map(|d| d.to_string()),
                member_types: u.members.iter().map(|m| m.name.to_string()).collect(),
            }),
            _ => panic!("Type '{}' is not a composite type", name),
        }
    }

    fn convert_field_def(
        &self,
        fdef: &apollo_compiler::schema::FieldDefinition,
    ) -> GraphQLField {
        let is_deprecated = has_directive(&fdef.directives, "deprecated");
        let deprecation_reason =
            get_directive_arg_string(&fdef.directives, "deprecated", "reason");

        GraphQLField {
            name: fdef.name.to_string(),
            field_type: self.convert_schema_type(&fdef.ty),
            description: fdef.description.as_ref().map(|d| d.to_string()),
            deprecation_reason,
            is_deprecated,
            arguments: fdef
                .arguments
                .iter()
                .map(|arg| GraphQLArgument {
                    name: arg.name.to_string(),
                    argument_type: self.convert_schema_type(&arg.ty),
                    default_value: arg.default_value.as_ref().map(|v| self.convert_ast_value(v)),
                    deprecation_reason: None,
                    is_deprecated: has_directive(&arg.directives, "deprecated"),
                })
                .collect(),
        }
    }

    fn convert_schema_type(&self, ty: &apollo_compiler::ast::Type) -> GraphQLType {
        match ty {
            apollo_compiler::ast::Type::Named(name) => {
                GraphQLType::Named(name.to_string())
            }
            apollo_compiler::ast::Type::NonNullNamed(name) => {
                GraphQLType::NonNull(Box::new(GraphQLType::Named(name.to_string())))
            }
            apollo_compiler::ast::Type::List(inner) => {
                GraphQLType::List(Box::new(self.convert_schema_type(inner)))
            }
            apollo_compiler::ast::Type::NonNullList(inner) => GraphQLType::NonNull(Box::new(
                GraphQLType::List(Box::new(self.convert_schema_type(inner))),
            )),
        }
    }

    fn convert_ast_type(&self, ty: &Node<apollo_compiler::ast::Type>) -> GraphQLType {
        self.convert_schema_type(ty.as_ref())
    }

    fn convert_ast_value(&self, val: &Node<apollo_compiler::ast::Value>) -> GraphQLValue {
        self.convert_value(val.as_ref())
    }

    fn convert_value(&self, val: &apollo_compiler::ast::Value) -> GraphQLValue {
        match val {
            apollo_compiler::ast::Value::Null => GraphQLValue::Null,
            apollo_compiler::ast::Value::Boolean(b) => GraphQLValue::Boolean(*b),
            apollo_compiler::ast::Value::Int(n) => {
                GraphQLValue::Int(n.as_str().parse::<i64>().unwrap_or(0))
            }
            apollo_compiler::ast::Value::Float(f) => {
                GraphQLValue::Float(f.try_to_f64().unwrap_or(0.0))
            }
            apollo_compiler::ast::Value::String(s) => GraphQLValue::String(s.to_string()),
            apollo_compiler::ast::Value::Enum(e) => GraphQLValue::Enum(e.to_string()),
            apollo_compiler::ast::Value::List(list) => {
                GraphQLValue::List(list.iter().map(|v| self.convert_value(v)).collect())
            }
            apollo_compiler::ast::Value::Object(fields) => {
                let map = fields
                    .iter()
                    .map(|(k, v)| (k.to_string(), self.convert_value(v)))
                    .collect();
                GraphQLValue::Object(map)
            }
            apollo_compiler::ast::Value::Variable(v) => {
                GraphQLValue::Variable(v.to_string())
            }
        }
    }

    fn convert_directives(
        &self,
        directives: &apollo_compiler::ast::DirectiveList,
    ) -> Vec<Directive> {
        directives
            .iter()
            .map(|d| Directive {
                name: d.name.to_string(),
                arguments: if d.arguments.is_empty() {
                    None
                } else {
                    Some(
                        d.arguments
                            .iter()
                            .map(|arg| Argument {
                                name: arg.name.to_string(),
                                value: self.convert_value(&arg.value),
                            })
                            .collect(),
                    )
                },
            })
            .collect()
    }

    fn build_selection_set(
        &self,
        sel_set: &executable::SelectionSet,
        schema: &Schema,
    ) -> SelectionSet {
        let parent_type = self.build_composite_type(sel_set.ty.as_str(), schema);
        let selections = sel_set
            .selections
            .iter()
            .map(|sel| self.convert_selection(sel, schema))
            .collect();

        SelectionSet {
            parent_type,
            selections,
        }
    }

    fn convert_selection(
        &self,
        sel: &AcSelection,
        schema: &Schema,
    ) -> Selection {
        match sel {
            AcSelection::Field(field) => {
                let sub_selection = if field.selection_set.selections.is_empty() {
                    None
                } else {
                    Some(self.build_selection_set(&field.selection_set, schema))
                };

                let inclusion_conditions = self.extract_inclusion_conditions(&field.directives);

                Selection::Field(Field {
                    name: field.name.to_string(),
                    alias: field.alias.as_ref().map(|a| a.to_string()),
                    field_type: self.convert_schema_type(&field.definition.ty),
                    arguments: if field.arguments.is_empty() {
                        None
                    } else {
                        Some(
                            field
                                .arguments
                                .iter()
                                .map(|arg| Argument {
                                    name: arg.name.to_string(),
                                    value: self.convert_value(&arg.value),
                                })
                                .collect(),
                        )
                    },
                    directives: if field.directives.is_empty() {
                        None
                    } else {
                        Some(self.convert_directives(&field.directives))
                    },
                    inclusion_conditions,
                    selection_set: sub_selection,
                })
            }
            AcSelection::InlineFragment(inline) => {
                let type_condition = inline
                    .type_condition
                    .as_ref()
                    .map(|tc| self.build_composite_type(tc.as_str(), schema));
                let selection_set = self.build_selection_set(&inline.selection_set, schema);
                let inclusion_conditions =
                    self.extract_inclusion_conditions(&inline.directives);

                Selection::InlineFragment(InlineFragment {
                    type_condition,
                    selection_set,
                    directives: if inline.directives.is_empty() {
                        None
                    } else {
                        Some(self.convert_directives(&inline.directives))
                    },
                    inclusion_conditions,
                })
            }
            AcSelection::FragmentSpread(spread) => {
                let inclusion_conditions =
                    self.extract_inclusion_conditions(&spread.directives);

                Selection::FragmentSpread(FragmentSpread {
                    fragment_name: spread.fragment_name.to_string(),
                    directives: if spread.directives.is_empty() {
                        None
                    } else {
                        Some(self.convert_directives(&spread.directives))
                    },
                    inclusion_conditions,
                })
            }
        }
    }

    fn extract_inclusion_conditions(
        &self,
        directives: &apollo_compiler::ast::DirectiveList,
    ) -> Option<Vec<InclusionCondition>> {
        let mut conditions = Vec::new();

        for dir in directives.iter() {
            match dir.name.as_str() {
                "skip" => {
                    if let Some(val) = get_directive_arg_value(dir, "if") {
                        if let apollo_compiler::ast::Value::Variable(var) = val {
                            conditions.push(InclusionCondition {
                                variable: var.to_string(),
                                is_inverted: true, // @skip means "exclude if true"
                            });
                        }
                    }
                }
                "include" => {
                    if let Some(val) = get_directive_arg_value(dir, "if") {
                        if let apollo_compiler::ast::Value::Variable(var) = val {
                            conditions.push(InclusionCondition {
                                variable: var.to_string(),
                                is_inverted: false,
                            });
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

    /// Print an operation to its source text (matching graphql-js print format).
    /// Adds __typename to every composite selection set, matching the behavior of
    /// graphql-js's transformToNetworkRequestSourceDefinition.
    fn print_operation(&self, op: &executable::Operation) -> String {
        let raw = op.serialize().no_indent().to_string();
        add_typename_to_selection_sets(&raw)
    }

    /// Print a fragment to its source text.
    fn print_fragment(&self, frag: &executable::Fragment) -> String {
        let raw = frag.serialize().no_indent().to_string();
        add_typename_to_selection_sets(&raw)
    }

    fn find_file_path_for_operation(
        &self,
        name: &apollo_compiler::Name,
        source_map: &BTreeMap<String, (String, String)>,
    ) -> String {
        // Search source files for the operation name
        for (file_path, (content, _)) in source_map {
            if content.contains(&format!("query {}", name))
                || content.contains(&format!("mutation {}", name))
                || content.contains(&format!("subscription {}", name))
            {
                return file_path.clone();
            }
        }
        String::new()
    }

    fn find_file_path_for_fragment(
        &self,
        name: &apollo_compiler::Name,
        source_map: &BTreeMap<String, (String, String)>,
    ) -> String {
        for (file_path, (content, _)) in source_map {
            if content.contains(&format!("fragment {}", name)) {
                return file_path.clone();
            }
        }
        String::new()
    }

    fn collect_referenced_fragments_for_op(
        &self,
        op: &executable::Operation,
        doc: &ExecutableDocument,
    ) -> Vec<String> {
        let mut referenced = IndexSet::new();
        self.collect_fragments_in_selections(&op.selection_set, doc, &mut referenced);
        referenced.into_iter().collect()
    }

    fn collect_referenced_fragments_for_frag(
        &self,
        frag: &executable::Fragment,
        doc: &ExecutableDocument,
    ) -> Vec<String> {
        let mut referenced = IndexSet::new();
        self.collect_fragments_in_selections(&frag.selection_set, doc, &mut referenced);
        referenced.into_iter().collect()
    }

    fn collect_fragments_in_selections(
        &self,
        sel_set: &executable::SelectionSet,
        doc: &ExecutableDocument,
        collected: &mut IndexSet<String>,
    ) {
        for sel in &sel_set.selections {
            match sel {
                AcSelection::Field(field) => {
                    self.collect_fragments_in_selections(&field.selection_set, doc, collected);
                }
                AcSelection::InlineFragment(inline) => {
                    self.collect_fragments_in_selections(
                        &inline.selection_set,
                        doc,
                        collected,
                    );
                }
                AcSelection::FragmentSpread(spread) => {
                    let name = spread.fragment_name.to_string();
                    if collected.insert(name.clone()) {
                        // Only recurse if this is the first time we've seen this fragment
                        if let Some(frag) = doc.fragments.get(&spread.fragment_name) {
                            self.collect_fragments_in_selections(
                                &frag.selection_set,
                                doc,
                                collected,
                            );
                        }
                    }
                }
            }
        }
    }

    fn collect_referenced_types(
        &self,
        schema: &Schema,
        _doc: &ExecutableDocument,
        _options: &CompileOptions,
    ) -> Vec<GraphQLNamedType> {
        let mut types = Vec::new();

        // Built-in scalars that should be skipped
        let builtin_scalars = ["String", "Int", "Float", "Boolean"];

        for (name, _extended_type) in &schema.types {
            // Skip built-in introspection types
            if name.starts_with("__") {
                continue;
            }
            // Skip built-in scalars (but keep ID since it's treated as custom scalar)
            if builtin_scalars.contains(&name.as_str()) {
                continue;
            }

            types.push(self.build_named_type(name.as_str(), schema));
        }

        types
    }
}

/// Add `__typename` to every selection set in a printed GraphQL document.
/// This matches the behavior of graphql-js's `transformToNetworkRequestSourceDefinition`.
fn add_typename_to_selection_sets(source: &str) -> String {
    let mut result = String::with_capacity(source.len() + 100);
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '{' {
            result.push('{');
            i += 1;
            // Skip whitespace after {
            while i < len && chars[i] == ' ' {
                result.push(' ');
                i += 1;
            }
            // Check if this is a selection set (not an argument object like {key: value})
            // Selection sets have field names or spread notation as first content
            if i < len
                && (chars[i].is_alphabetic()
                    || chars[i] == '_'
                    || (i + 2 < len && chars[i] == '.' && chars[i + 1] == '.' && chars[i + 2] == '.'))
            {
                // Check if __typename is already first
                let remaining: String = chars[i..].iter().collect();
                if !remaining.starts_with("__typename") {
                    result.push_str("__typename ");
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}
