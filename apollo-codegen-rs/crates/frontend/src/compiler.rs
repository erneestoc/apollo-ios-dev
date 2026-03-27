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

        // Add Apollo-specific directives only if not already defined in the schema sources.
        // Some schemas (e.g. StarWarsAPI, UploadAPI) already include these directives,
        // and re-defining them causes a "defined multiple times" error.
        let all_sources: String = sources.iter().map(|(content, _)| content.as_str()).collect::<Vec<_>>().join("\n");
        let mut extra_directives = String::new();
        if !all_sources.contains("@apollo_client_ios_localCacheMutation") {
            extra_directives.push_str("directive @apollo_client_ios_localCacheMutation on QUERY | MUTATION | SUBSCRIPTION | FRAGMENT_DEFINITION\n");
        }
        if !all_sources.contains("@typePolicy") {
            extra_directives.push_str("directive @typePolicy(keyFields: String!) on OBJECT | INTERFACE\n");
        }
        if !all_sources.contains("@fieldPolicy") {
            extra_directives.push_str("directive @fieldPolicy(forField: String!, keyArgs: String!) on FIELD_DEFINITION\n");
        }
        if !extra_directives.is_empty() {
            builder = builder.parse(extra_directives, "apollo_extensions.graphql");
        }

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
                let source = self.print_operation(op, options.legacy_safelisting_compatible_operations);

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
            let source = self.print_fragment(frag, options.legacy_safelisting_compatible_operations);
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
                                    deprecation_reason: get_directive_arg_string(&fdef.directives, "deprecated", "reason"),
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
    fn print_operation(&self, op: &executable::Operation, legacy_safelisting: bool) -> String {
        let raw = op.serialize().no_indent().to_string();
        let raw = strip_local_cache_mutation_directive(&raw);
        let raw = fix_default_value_formatting(&raw);
        let raw = fix_inline_argument_format(&raw);
        add_typename_to_selection_sets(&raw, legacy_safelisting)
    }

    /// Print a fragment to its source text.
    fn print_fragment(&self, frag: &executable::Fragment, legacy_safelisting: bool) -> String {
        let raw = frag.serialize().no_indent().to_string();
        let raw = strip_local_cache_mutation_directive(&raw);
        let raw = fix_inline_argument_format(&raw);
        add_typename_to_selection_sets(&raw, legacy_safelisting)
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
        doc: &ExecutableDocument,
        options: &CompileOptions,
    ) -> Vec<GraphQLNamedType> {
        // Built-in scalars that should be skipped
        let builtin_scalars = ["String", "Int", "Float", "Boolean"];

        // Collect types in encounter order, matching the JS frontend behavior.
        // The JS frontend calls `addReferencedType` during compilation, which:
        // - Adds the type itself
        // - For interfaces: recursively adds all implementing objects
        // - For unions: recursively adds all member types
        // - For objects: recursively adds all interface types
        // - For input objects: recursively adds all field types
        let mut seen = IndexSet::new();

        // Walk all operations, sorted by type (queries first, then subscriptions,
        // then mutations) to match the JS frontend's encounter order.
        // The JS frontend processes operations from the merged document, where
        // query-type operations tend to be encountered before mutation-type ones.
        let mut sorted_ops: Vec<_> = doc.operations.iter().collect();
        sorted_ops.sort_by_key(|op| match op.operation_type {
            apollo_compiler::ast::OperationType::Query => 0,
            apollo_compiler::ast::OperationType::Subscription => 1,
            apollo_compiler::ast::OperationType::Mutation => 2,
        });
        for op in sorted_ops {
            // 1. Variable types first (matching JS: variables processed before rootType)
            for var in &op.variables {
                self.add_referenced_type_from_ast(&var.ty, schema, &builtin_scalars, &mut seen);
            }

            // 2. Root type
            let root_name = op.object_type();
            self.add_referenced_type(root_name.as_str(), schema, &builtin_scalars, &mut seen);

            // 3. Walk the selection set
            self.collect_types_from_selection_set(&op.selection_set, schema, doc, &builtin_scalars, &mut seen);
        }

        // Walk remaining fragments (those not encountered via fragment spreads)
        for (_name, frag) in &doc.fragments {
            let tc_name = frag.type_condition();
            self.add_referenced_type(tc_name.as_str(), schema, &builtin_scalars, &mut seen);
            self.collect_types_from_selection_set(&frag.selection_set, schema, doc, &builtin_scalars, &mut seen);
        }

        // NOTE: We intentionally do NOT add all remaining schema types here.
        // The Swift/JS compiler only includes types actually referenced by
        // operations and fragments. Unreferenced types are excluded from generation.

        // Build the final list
        seen.iter()
            .map(|name| self.build_named_type(name.as_str(), schema))
            .collect()
    }

    /// Add a referenced type with recursive expansion, matching JS `addReferencedType`.
    ///
    /// This mirrors the JS behavior:
    /// - If interface: add all implementing objects (via schema.getPossibleTypes)
    /// - If union: add all member types
    /// - If input object: add all field types
    /// - If object: add all interface types
    fn add_referenced_type(
        &self,
        name: &str,
        schema: &Schema,
        builtin_scalars: &[&str],
        seen: &mut IndexSet<String>,
    ) {
        if name.starts_with("__") || builtin_scalars.contains(&name) {
            return;
        }
        if !seen.insert(name.to_string()) {
            return; // Already seen
        }

        match schema.types.get(name) {
            Some(ExtendedType::Interface(_)) => {
                // Add all implementing objects (matching JS: schema.getPossibleTypes)
                let implementers = schema.implementers_map();
                if let Some(imp) = implementers.get(name) {
                    for obj_name in &imp.objects {
                        self.add_referenced_type(obj_name.as_str(), schema, builtin_scalars, seen);
                    }
                }
            }
            Some(ExtendedType::Union(u)) => {
                for member in &u.members {
                    self.add_referenced_type(member.name.as_str(), schema, builtin_scalars, seen);
                }
            }
            Some(ExtendedType::InputObject(io)) => {
                for (_fname, fdef) in io.fields.iter() {
                    self.add_referenced_type_from_schema_type(&fdef.ty, schema, builtin_scalars, seen);
                }
            }
            Some(ExtendedType::Object(o)) => {
                for iface in &o.implements_interfaces {
                    self.add_referenced_type(iface.name.as_str(), schema, builtin_scalars, seen);
                }
            }
            _ => {}
        }
    }

    /// Extract the named type from a schema type and add it as referenced.
    fn add_referenced_type_from_schema_type(
        &self,
        ty: &apollo_compiler::ast::Type,
        schema: &Schema,
        builtin_scalars: &[&str],
        seen: &mut IndexSet<String>,
    ) {
        match ty {
            apollo_compiler::ast::Type::Named(name)
            | apollo_compiler::ast::Type::NonNullNamed(name) => {
                self.add_referenced_type(name.as_str(), schema, builtin_scalars, seen);
            }
            apollo_compiler::ast::Type::List(inner)
            | apollo_compiler::ast::Type::NonNullList(inner) => {
                self.add_referenced_type_from_schema_type(inner, schema, builtin_scalars, seen);
            }
        }
    }

    /// Add a referenced type from an AST type node.
    fn add_referenced_type_from_ast(
        &self,
        ty: &Node<apollo_compiler::ast::Type>,
        schema: &Schema,
        builtin_scalars: &[&str],
        seen: &mut IndexSet<String>,
    ) {
        self.add_referenced_type_from_schema_type(ty.as_ref(), schema, builtin_scalars, seen);
    }

    /// Collect types from a selection set, in encounter order.
    fn collect_types_from_selection_set(
        &self,
        sel_set: &executable::SelectionSet,
        schema: &Schema,
        doc: &ExecutableDocument,
        builtin_scalars: &[&str],
        seen: &mut IndexSet<String>,
    ) {
        for sel in &sel_set.selections {
            match sel {
                AcSelection::Field(field) => {
                    // Collect the field's return type (with expansion)
                    self.add_referenced_type_from_schema_type(&field.definition.ty, schema, builtin_scalars, seen);
                    // Recurse into nested selection set
                    if !field.selection_set.selections.is_empty() {
                        self.collect_types_from_selection_set(&field.selection_set, schema, doc, builtin_scalars, seen);
                    }
                }
                AcSelection::InlineFragment(inline) => {
                    if let Some(ref tc) = inline.type_condition {
                        self.add_referenced_type(tc.as_str(), schema, builtin_scalars, seen);
                    }
                    self.collect_types_from_selection_set(&inline.selection_set, schema, doc, builtin_scalars, seen);
                }
                AcSelection::FragmentSpread(spread) => {
                    if let Some(frag) = doc.fragments.get(&spread.fragment_name) {
                        let tc_name = frag.type_condition();
                        self.add_referenced_type(tc_name.as_str(), schema, builtin_scalars, seen);
                    }
                }
            }
        }
    }
}

/// Add `__typename` to selection sets in a printed GraphQL document.
/// This matches the behavior of graphql-js's `transformToNetworkRequestSourceDefinition`:
/// - Add `__typename` in field selection sets (e.g., `allAnimals { __typename id }`)
/// - Do NOT add `__typename` at the root operation level (e.g., `query Foo { field }`)
/// Strip the `@apollo_client_ios_localCacheMutation` directive from a source string.
/// This directive is used for code generation purposes but should not appear in the
/// emitted fragment definition source.
/// Fix inline argument formatting to match graphql-js output.
///
/// graphql-js's printer uses multi-line format for arguments when the single-line
/// form would exceed 80 characters. When collapsed to single-line, the multi-line
/// format produces `( arg1 arg2 )` instead of `(arg1, arg2)`.
///
/// Rules:
///   1. Only reformat argument lists that contain non-variable literal values
///   2. Only reformat when the field+args line would exceed 80 characters
///   3. Add space after `(` and before `)`
///   4. Remove commas between top-level arguments
///   5. Add spaces inside `{` and `}` for input object literals
///   6. Commas INSIDE object values are KEPT
///
/// Additionally, always add spaces inside `{}` for input object literals in arguments,
/// even when the argument list doesn't need the multi-line reformat.
fn fix_inline_argument_format(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut result = String::with_capacity(len + 50);
    let mut i = 0;

    while i < len {
        // Look for `(` that starts an argument list (preceded by identifier)
        if chars[i] == '(' {
            // Check if this is an argument list (preceded by an identifier char)
            let is_arg_list = i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
            if is_arg_list {
                // Find matching `)` and extract the argument list
                let mut depth = 1;
                let start = i;
                let mut j = i + 1;
                while j < len && depth > 0 {
                    if chars[j] == '(' { depth += 1; }
                    else if chars[j] == ')' { depth -= 1; }
                    else if chars[j] == '"' {
                        // Skip string contents
                        j += 1;
                        while j < len && chars[j] != '"' {
                            if chars[j] == '\\' { j += 1; }
                            j += 1;
                        }
                    }
                    j += 1;
                }
                let end = j; // one past the closing `)`

                // Extract argument list content (between parens)
                let arg_content: String = chars[start + 1..end - 1].iter().collect();

                // Skip variable definition lists (e.g., `($var: Type!)`)
                // Variable definitions have `$name:` format ($ before the first colon).
                let is_var_def_list = is_variable_definition_list(&arg_content);

                // Check if any argument value is a non-variable literal
                let has_inline_literal = !is_var_def_list && has_non_variable_argument(&arg_content);

                if has_inline_literal {
                    // First, add spaces inside `{}` for object literals
                    let with_brace_spaces = add_brace_spaces(&arg_content);

                    // Find the field name that precedes this argument list by scanning
                    // backwards from `(`. The field call is `fieldName(args)`.
                    let field_start = {
                        let mut fs = start;
                        while fs > 0 && (chars[fs - 1].is_alphanumeric() || chars[fs - 1] == '_') {
                            fs -= 1;
                        }
                        fs
                    };
                    // The full "line" that graphql-js would measure includes the field name + (args)
                    let full_line_len = (start - field_start) + 1 + with_brace_spaces.len() + 1; // fieldName + ( + args + )

                    if full_line_len > 80 {
                        // Multi-line format: spaces around parens, remove top-level commas
                        let reformatted = remove_top_level_commas(&with_brace_spaces);
                        result.push_str("( ");
                        result.push_str(&reformatted);
                        result.push_str(" )");
                    } else {
                        // Compact format but with spaces inside braces
                        result.push('(');
                        result.push_str(&with_brace_spaces);
                        result.push(')');
                    }
                    i = end;
                } else {
                    // All variables - leave unchanged
                    result.push(chars[i]);
                    i += 1;
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

/// Check if this parenthesized content is a variable definition list
/// (e.g., `$var: Type!, $var2: Type`). Variable definitions start with `$`.
fn is_variable_definition_list(content: &str) -> bool {
    let trimmed = content.trim();
    trimmed.starts_with('$')
}

/// Check if an argument list string contains any non-variable literal value.
/// Variable references start with `$` after `:`.
fn has_non_variable_argument(args: &str) -> bool {
    let chars: Vec<char> = args.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == ':' {
            // Skip whitespace after `:`
            i += 1;
            while i < len && chars[i] == ' ' { i += 1; }
            if i < len && chars[i] != '$' {
                return true; // Non-variable argument value
            }
        } else if chars[i] == '"' {
            // Skip string
            i += 1;
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' { i += 1; }
                i += 1;
            }
            if i < len { i += 1; }
        } else {
            i += 1;
        }
    }
    false
}

/// Add spaces inside `{` and `}` for input object literals.
fn add_brace_spaces(content: &str) -> String {
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();
    let mut result = String::with_capacity(len + 20);
    let mut i = 0;

    while i < len {
        match chars[i] {
            '{' => {
                result.push_str("{ ");
                i += 1;
                // Skip any existing space after `{`
                while i < len && chars[i] == ' ' { i += 1; }
            }
            '}' => {
                // Ensure space before `}`
                if !result.ends_with(' ') {
                    result.push(' ');
                }
                result.push('}');
                i += 1;
            }
            '"' => {
                // Copy string literal verbatim
                result.push(chars[i]);
                i += 1;
                while i < len && chars[i] != '"' {
                    if chars[i] == '\\' {
                        result.push(chars[i]);
                        i += 1;
                        if i < len {
                            result.push(chars[i]);
                            i += 1;
                        }
                    } else {
                        result.push(chars[i]);
                        i += 1;
                    }
                }
                if i < len {
                    result.push(chars[i]); // closing quote
                    i += 1;
                }
            }
            _ => {
                result.push(chars[i]);
                i += 1;
            }
        }
    }

    result
}

/// Remove commas between top-level arguments (brace_depth == 0).
/// Commas inside `{}` are kept.
fn remove_top_level_commas(content: &str) -> String {
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();
    let mut result = String::with_capacity(len);
    let mut i = 0;
    let mut brace_depth = 0;

    while i < len {
        match chars[i] {
            '{' => {
                brace_depth += 1;
                result.push(chars[i]);
                i += 1;
            }
            '}' => {
                brace_depth -= 1;
                result.push(chars[i]);
                i += 1;
            }
            ',' if brace_depth == 0 => {
                // Top-level comma between arguments — remove it
                i += 1;
            }
            '"' => {
                // Copy string literal verbatim
                result.push(chars[i]);
                i += 1;
                while i < len && chars[i] != '"' {
                    if chars[i] == '\\' {
                        result.push(chars[i]);
                        i += 1;
                        if i < len {
                            result.push(chars[i]);
                            i += 1;
                        }
                    } else {
                        result.push(chars[i]);
                        i += 1;
                    }
                }
                if i < len {
                    result.push(chars[i]); // closing quote
                    i += 1;
                }
            }
            _ => {
                result.push(chars[i]);
                i += 1;
            }
        }
    }

    result
}

/// Fix default value formatting in the source string to match graphql-js output.
/// apollo-compiler prints: `{key: val, key2: val2}`
/// graphql-js prints:      `{ key: val key2: val2 }` (spaces around braces, commas
/// removed between fields that have complex values but kept between simple scalar fields)
fn fix_default_value_formatting(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut result = String::with_capacity(len + 50);
    let mut i = 0;
    let mut in_default_value = false;
    let mut default_brace_depth = 0;

    while i < len {
        if !in_default_value {
            if chars[i] == '=' && i + 1 < len && chars[i + 1] == ' ' {
                let before = &source[..i];
                let paren_depth: i32 = before.chars().map(|c| match c {
                    '(' => 1, ')' => -1, _ => 0,
                }).sum();
                if paren_depth > 0 {
                    result.push(chars[i]);
                    i += 1;
                    result.push(chars[i]);
                    i += 1;
                    in_default_value = true;
                    continue;
                }
            }
            result.push(chars[i]);
            i += 1;
        } else {
            if chars[i] == '{' {
                default_brace_depth += 1;
                result.push_str("{ ");
                i += 1;
            } else if chars[i] == '}' {
                default_brace_depth -= 1;
                // Remove trailing space before closing brace if present
                if result.ends_with(' ') {
                    // Already has space, just add }
                } else {
                    result.push(' ');
                }
                result.push('}');
                i += 1;
                if default_brace_depth == 0 {
                    in_default_value = false;
                }
            } else if chars[i] == ',' && default_brace_depth > 0 {
                // Only remove commas between object fields at brace depth 1 (outermost default value object).
                // Inner objects (depth >= 2) keep their commas, matching graphql-js behavior.
                if default_brace_depth == 1 {
                    // Check if followed by space + fieldName:
                    let mut j = i + 1;
                    while j < len && chars[j] == ' ' { j += 1; }
                    let mut k = j;
                    while k < len && (chars[k].is_alphanumeric() || chars[k] == '_') { k += 1; }
                    if k < len && k > j && chars[k] == ':' {
                        // Top-level object field comma - skip
                        i += 1;
                    } else {
                        result.push(',');
                        i += 1;
                    }
                } else {
                    // Inner brace depth - keep comma
                    result.push(',');
                    i += 1;
                }
            } else if chars[i] == ')' && default_brace_depth == 0 {
                in_default_value = false;
                result.push(chars[i]);
                i += 1;
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }
    }

    result
}

/// Strip existing __typename fields from the source string.
/// Used in legacy safelisting mode where __typename fields are first removed,
/// then re-added at the start of each selection set.
fn strip_existing_typenames(source: &str) -> String {
    // Remove "__typename " (with trailing space) or " __typename" (with leading space)
    let result = source.replace("__typename ", "");
    // Clean up any double spaces left behind
    let mut cleaned = String::with_capacity(result.len());
    let mut prev_space = false;
    for c in result.chars() {
        if c == ' ' && prev_space {
            continue;
        }
        prev_space = c == ' ';
        cleaned.push(c);
    }
    // Clean up "{ }" (empty selection sets after removing __typename)
    cleaned
}

fn strip_local_cache_mutation_directive(source: &str) -> String {
    source.replace(" @apollo_client_ios_localCacheMutation", "")
          .replace("@apollo_client_ios_localCacheMutation ", "")
          .replace("@apollo_client_ios_localCacheMutation", "")
}

/// - Do NOT add `__typename` inside inline fragments (unless legacy_safelisting is true)
/// - DO add `__typename` in fragment definition root selection sets
fn add_typename_to_selection_sets(source: &str, legacy_safelisting: bool) -> String {
    // When legacy safelisting is enabled, first strip all existing __typename fields
    // (the JS transform removes them first, then re-adds at the beginning of each selection set)
    let source = if legacy_safelisting {
        strip_existing_typenames(source)
    } else {
        source.to_string()
    };
    let source = &source;

    let mut result = String::with_capacity(source.len() + 100);
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '{' {
            // Determine context by looking at what precedes this `{`.
            // We look backwards through the already-built result string, skipping whitespace.
            let should_add_typename = should_add_typename_before_brace(&result, legacy_safelisting);

            result.push('{');
            i += 1;
            // Skip whitespace after {
            while i < len && chars[i] == ' ' {
                result.push(' ');
                i += 1;
            }
            // Only inject __typename for field selection sets (not root operations,
            // not inline fragments, not argument objects)
            if should_add_typename
                && i < len
                && (chars[i].is_alphabetic()
                    || chars[i] == '_'
                    || (i + 2 < len
                        && chars[i] == '.'
                        && chars[i + 1] == '.'
                        && chars[i + 2] == '.'))
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

/// Determine whether `__typename` should be injected after the `{` that is about
/// to be appended to `result`.
///
/// Returns `true` for field selection sets (the `{` follows a field name or `)` after
/// field arguments).
/// Returns `false` for:
/// - Root operation braces: `{` preceded by the operation name/closing `)` of variable
///   list at depth 0 (first `{` in the document)
/// - Inline fragment braces: `{` preceded by a type name after `... on`
/// - Argument object literals: `{` preceded by `:` or `[` etc.
fn should_add_typename_before_brace(result_so_far: &str, legacy_safelisting: bool) -> bool {
    let trimmed = result_so_far.trim_end();
    if trimmed.is_empty() {
        return false;
    }

    // Check if this is the root operation brace (first `{` at paren depth 0).
    // Default values inside `(...)` can contain `{` but those don't count.
    let mut paren_depth = 0;
    let mut has_brace_at_depth_0 = false;
    for c in result_so_far.chars() {
        match c {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '{' if paren_depth == 0 => { has_brace_at_depth_0 = true; break; }
            _ => {}
        }
    }
    if !has_brace_at_depth_0 {
        // This is the first `{` at paren depth 0. Check if document starts with operation keyword.
        let start = trimmed.trim_start();
        if start.starts_with("query")
            || start.starts_with("mutation")
            || start.starts_with("subscription")
        {
            return false;
        }
        // For fragments (`fragment Name on Type {`), we DO want __typename
        // so fall through to the normal logic.
    }

    // Check if preceded by `... on TypeName` (inline fragment).
    // Walk backwards: we expect an identifier (type name), then "on", then "...".
    let bytes = trimmed.as_bytes();
    let pos = bytes.len();

    // Skip trailing identifier (type name) or closing paren of directives
    // First, skip the type name or closing paren
    let last_char = bytes[pos - 1];

    if last_char == b')' {
        // Could be a closing paren of field arguments -> this is a field selection set.
        // Could also be directive arguments on an inline fragment like `... on Cat @include(if: $x)`.
        // We need to skip past the balanced parens and check what's before them.
        let mut depth = 0;
        let mut p = pos;
        while p > 0 {
            p -= 1;
            if bytes[p] == b')' {
                depth += 1;
            } else if bytes[p] == b'(' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
        }
        // p now points to the opening '('
        // Check what's before the opening paren (skip whitespace)
        let before_paren = trimmed[..p].trim_end();
        if before_paren.is_empty() {
            return false;
        }

        // Extract the identifier before the paren
        let before_bytes = before_paren.as_bytes();
        let end = before_bytes.len();
        // The thing before '(' could be a directive like @include, or a field name, or a type name
        // If it's a directive, we need to keep looking backwards
        let last_b = before_bytes[end - 1];

        if last_b.is_ascii_alphanumeric() || last_b == b'_' {
            // Extract this identifier
            let mut start = end;
            while start > 0
                && (before_bytes[start - 1].is_ascii_alphanumeric()
                    || before_bytes[start - 1] == b'_')
            {
                start -= 1;
            }
            // Check if preceded by '@' (directive)
            if start > 0 && before_bytes[start - 1] == b'@' {
                // This is a directive like @include(...). Look further back to see
                // if there's a `... on TypeName` pattern.
                let before_directive = trimmed[..start - 1].trim_end();
                // In legacy safelisting mode, inline fragments also get __typename
                return legacy_safelisting || !is_inline_fragment_context(before_directive);
            }
            // Not a directive - this is likely a field name with arguments
            // But we should still check if the identifier is a type name after `... on`
            let before_ident = before_paren[..start].trim_end();
            if before_ident.ends_with("on") {
                let before_on = before_ident[..before_ident.len() - 2].trim_end();
                if before_on.ends_with("...") {
                    // inline fragment with arguments: `... on Type(...) {`
                    return legacy_safelisting;
                }
            }
            return true;
        }
        return true;
    }

    // The last non-whitespace character is an identifier character
    if last_char.is_ascii_alphanumeric() || last_char == b'_' {
        // In legacy safelisting mode, inline fragments also get __typename
        return legacy_safelisting || !is_inline_fragment_context(trimmed);
    }

    // For any other character (e.g., `:`, `[`), don't add __typename
    // (this covers argument object literals like `{key: value}`)
    false
}

/// Check if the trimmed text before a `{` ends with an inline fragment pattern:
/// `... on TypeName` possibly followed by directives like `@include(if: $var)`.
fn is_inline_fragment_context(trimmed: &str) -> bool {
    let bytes = trimmed.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return false;
    }

    let mut pos = len;

    // We may have multiple directives like `@skip(if: $x) @include(if: $y)`.
    // Skip past all of them.
    loop {
        // Try to skip a directive at the end: `@name(args)` or `@name`
        let at_pos = pos;

        // Check if last char is ')' - directive with arguments
        if pos > 0 && bytes[pos - 1] == b')' {
            let mut depth = 0;
            let mut p = pos;
            while p > 0 {
                p -= 1;
                if bytes[p] == b')' {
                    depth += 1;
                } else if bytes[p] == b'(' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
            pos = p; // points to '('
        }

        // Now check for `@identifier` before the current pos
        let s = trimmed[..pos].trim_end();
        let sb = s.as_bytes();
        if sb.is_empty() {
            return false;
        }

        // Extract identifier
        let end = sb.len();
        if !sb[end - 1].is_ascii_alphanumeric() && sb[end - 1] != b'_' {
            // Not an identifier - we've gone past any directives
            pos = at_pos; // restore
            break;
        }
        let mut start = end;
        while start > 0
            && (sb[start - 1].is_ascii_alphanumeric() || sb[start - 1] == b'_')
        {
            start -= 1;
        }

        // Check if preceded by '@'
        if start > 0 && sb[start - 1] == b'@' {
            // This is a directive, continue skipping
            pos = start - 1;
            let remaining = trimmed[..pos].trim_end();
            pos = remaining.len();
            // Need to re-assign trimmed context for next iteration
            continue;
        }

        // Not a directive - this is the final identifier before `{`
        pos = at_pos; // restore since we didn't find a directive
        break;
    }

    // Now `trimmed[..pos]` should end with the type name (if inline fragment) or field name.
    let text = trimmed[..pos].trim_end();
    let tb = text.as_bytes();
    if tb.is_empty() {
        return false;
    }

    // Extract the last identifier (should be type name or field name)
    if !tb[tb.len() - 1].is_ascii_alphanumeric() && tb[tb.len() - 1] != b'_' {
        return false;
    }
    let end = tb.len();
    let mut start = end;
    while start > 0 && (tb[start - 1].is_ascii_alphanumeric() || tb[start - 1] == b'_') {
        start -= 1;
    }

    // Check what's before this identifier
    let before_ident = text[..start].trim_end();
    if !before_ident.ends_with("on") {
        return false;
    }
    let before_on = before_ident[..before_ident.len() - 2].trim_end();
    before_on.ends_with("...")
}
