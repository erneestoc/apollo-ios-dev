//! Field collector for MockObject generation.
//!
//! Walks all operations and fragments to collect every field selected on each
//! concrete object type. For each object type, produces a list of collected
//! fields that can be used to generate MockObject files.

use apollo_codegen_frontend::compilation_result::CompilationResult;
use apollo_codegen_frontend::types::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// A field collected from operations/fragments for a specific object type.
#[derive(Debug, Clone)]
pub struct CollectedField {
    pub response_key: String,
    pub field_type: GraphQLType,
    pub deprecation_reason: Option<String>,
}

/// Collects all fields selected on each concrete object type across all
/// operations and fragments.
pub struct FieldCollector<'a> {
    compilation: &'a CompilationResult,
    /// Map from object type name -> set of interface names it implements
    object_interfaces: HashMap<String, HashSet<String>>,
    /// Map from interface name -> set of object names implementing it
    interface_implementers: HashMap<String, HashSet<String>>,
    /// Map from union name -> set of member object type names
    union_members: HashMap<String, HashSet<String>>,
    /// All object type names in the schema
    object_names: HashSet<String>,
    /// Map from type name to its kind (for classifying named types)
    type_kinds: HashMap<String, TypeKind>,
    /// Schema field definitions for each object type
    object_fields: HashMap<String, HashMap<String, GraphQLField>>,
    /// Schema field definitions for each interface type
    interface_fields: HashMap<String, HashMap<String, GraphQLField>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Scalar,
    Object,
    Interface,
    Union,
    Enum,
    InputObject,
}

impl<'a> FieldCollector<'a> {
    pub fn new(compilation: &'a CompilationResult) -> Self {
        let mut object_interfaces: HashMap<String, HashSet<String>> = HashMap::new();
        let mut interface_implementers: HashMap<String, HashSet<String>> = HashMap::new();
        let mut union_members: HashMap<String, HashSet<String>> = HashMap::new();
        let mut object_names: HashSet<String> = HashSet::new();
        let mut type_kinds: HashMap<String, TypeKind> = HashMap::new();
        let mut object_fields: HashMap<String, HashMap<String, GraphQLField>> = HashMap::new();
        let mut interface_fields: HashMap<String, HashMap<String, GraphQLField>> = HashMap::new();

        // Add built-in scalars
        for name in &["String", "Int", "Float", "Boolean", "ID"] {
            type_kinds.insert(name.to_string(), TypeKind::Scalar);
        }

        for named_type in &compilation.referenced_types {
            match named_type {
                GraphQLNamedType::Object(obj) => {
                    object_names.insert(obj.name.clone());
                    type_kinds.insert(obj.name.clone(), TypeKind::Object);
                    let ifaces: HashSet<String> =
                        obj.interfaces.iter().cloned().collect();
                    for iface in &ifaces {
                        interface_implementers
                            .entry(iface.clone())
                            .or_default()
                            .insert(obj.name.clone());
                    }
                    // Also collect transitive interface implementations
                    object_interfaces.insert(obj.name.clone(), ifaces);
                    // Store field definitions
                    let fields: HashMap<String, GraphQLField> = obj
                        .fields
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    object_fields.insert(obj.name.clone(), fields);
                }
                GraphQLNamedType::Interface(iface) => {
                    type_kinds.insert(iface.name.clone(), TypeKind::Interface);
                    for obj_name in &iface.implementing_objects {
                        interface_implementers
                            .entry(iface.name.clone())
                            .or_default()
                            .insert(obj_name.clone());
                    }
                    let fields: HashMap<String, GraphQLField> = iface
                        .fields
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    interface_fields.insert(iface.name.clone(), fields);
                }
                GraphQLNamedType::Union(union_t) => {
                    type_kinds.insert(union_t.name.clone(), TypeKind::Union);
                    let members: HashSet<String> =
                        union_t.member_types.iter().cloned().collect();
                    union_members.insert(union_t.name.clone(), members);
                }
                GraphQLNamedType::Enum(e) => {
                    type_kinds.insert(e.name.clone(), TypeKind::Enum);
                }
                GraphQLNamedType::Scalar(s) => {
                    type_kinds.insert(s.name.clone(), TypeKind::Scalar);
                }
                GraphQLNamedType::InputObject(io) => {
                    type_kinds.insert(io.name.clone(), TypeKind::InputObject);
                }
            }
        }

        Self {
            compilation,
            object_interfaces,
            interface_implementers,
            union_members,
            object_names,
            type_kinds,
            object_fields,
            interface_fields,
        }
    }

    /// Collect all fields for each object type across all operations and fragments.
    /// Returns a BTreeMap for deterministic ordering.
    pub fn collect_all_fields(&self) -> BTreeMap<String, Vec<CollectedField>> {
        // object_name -> (response_key -> CollectedField)
        let mut fields_map: HashMap<String, BTreeMap<String, CollectedField>> = HashMap::new();

        // Process all operations
        for op in &self.compilation.operations {
            let parent_type_name = op.root_type.name().to_string();
            self.collect_from_selection_set(
                &op.selection_set,
                &parent_type_name,
                &mut fields_map,
            );
        }

        // Process all fragments
        for frag in &self.compilation.fragments {
            let parent_type_name = frag.type_condition.name().to_string();
            self.collect_from_selection_set(
                &frag.selection_set,
                &parent_type_name,
                &mut fields_map,
            );
        }

        // Convert to sorted Vec<CollectedField> per object
        let mut result = BTreeMap::new();
        for (obj_name, field_map) in fields_map {
            let fields: Vec<CollectedField> = field_map.into_values().collect();
            if !fields.is_empty() {
                result.insert(obj_name, fields);
            }
        }

        result
    }

    /// Recursively walk a selection set and collect fields onto the applicable
    /// concrete object types.
    fn collect_from_selection_set(
        &self,
        selection_set: &SelectionSet,
        parent_type_name: &str,
        fields_map: &mut HashMap<String, BTreeMap<String, CollectedField>>,
    ) {
        // Determine which concrete object types this selection set applies to
        let applicable_objects = self.concrete_objects_for_type(parent_type_name);

        for selection in &selection_set.selections {
            match selection {
                Selection::Field(field) => {
                    let response_key = field
                        .alias
                        .as_deref()
                        .unwrap_or(&field.name)
                        .to_string();

                    // For each applicable object type, look up the field definition
                    // from the schema to get the proper type
                    for obj_name in &applicable_objects {
                        let schema_field = self.lookup_field_on_object(obj_name, &field.name);
                        if let Some(schema_field) = schema_field {
                            let collected = CollectedField {
                                response_key: response_key.clone(),
                                field_type: schema_field.field_type.clone(),
                                deprecation_reason: schema_field.deprecation_reason.clone(),
                            };
                            fields_map
                                .entry(obj_name.clone())
                                .or_default()
                                .entry(response_key.clone())
                                .or_insert(collected);
                        }
                    }

                    // Recurse into sub-selection sets
                    if let Some(ref sub_selection) = field.selection_set {
                        let field_named_type = field.field_type.named_type().to_string();
                        self.collect_from_selection_set(
                            sub_selection,
                            &field_named_type,
                            fields_map,
                        );
                    }
                }
                Selection::InlineFragment(inline) => {
                    let narrowed_type = inline
                        .type_condition
                        .as_ref()
                        .map(|tc| tc.name().to_string())
                        .unwrap_or_else(|| parent_type_name.to_string());

                    self.collect_from_selection_set(
                        &inline.selection_set,
                        &narrowed_type,
                        fields_map,
                    );
                }
                Selection::FragmentSpread(spread) => {
                    // Look up the fragment definition and walk its selections
                    if let Some(frag) = self
                        .compilation
                        .fragments
                        .iter()
                        .find(|f| f.name == spread.fragment_name)
                    {
                        let frag_type = frag.type_condition.name().to_string();
                        // The fragment applies to its type condition, but only
                        // the objects that overlap with the current parent type
                        // context. For simplicity and correctness, we process
                        // the fragment's selection set with its own type condition,
                        // since the fields it selects are valid for its type condition.
                        self.collect_from_selection_set(
                            &frag.selection_set,
                            &frag_type,
                            fields_map,
                        );
                    }
                }
            }
        }
    }

    /// Look up a field definition on a concrete object type.
    fn lookup_field_on_object(&self, object_name: &str, field_name: &str) -> Option<&GraphQLField> {
        // Skip __typename - it's automatically added and not a real field for mocks
        if field_name == "__typename" {
            return None;
        }

        // First try direct fields on the object
        if let Some(fields) = self.object_fields.get(object_name) {
            if let Some(field) = fields.get(field_name) {
                return Some(field);
            }
        }

        None
    }

    /// Given a type name (object, interface, or union), return all concrete
    /// object types that it could represent.
    fn concrete_objects_for_type(&self, type_name: &str) -> BTreeSet<String> {
        let mut result = BTreeSet::new();

        if self.object_names.contains(type_name) {
            result.insert(type_name.to_string());
        }

        if let Some(implementers) = self.interface_implementers.get(type_name) {
            for obj in implementers {
                result.insert(obj.clone());
            }
        }

        if let Some(members) = self.union_members.get(type_name) {
            for member in members {
                // Union members are always object types
                result.insert(member.clone());
            }
        }

        result
    }
}

/// Determine the TypeKind for a named type.
fn classify_type(name: &str, type_kinds: &HashMap<String, TypeKind>) -> TypeKind {
    type_kinds
        .get(name)
        .copied()
        .unwrap_or(TypeKind::Scalar)
}

/// Render the field type string for the `@Field<Type>` annotation in a MockObject.
/// This strips the outermost NonNull, then renders the type.
pub fn render_mock_field_type(ty: &GraphQLType, ns: &str, type_kinds: &HashMap<String, TypeKind>) -> String {
    // Strip outermost NonNull
    let inner = strip_outer_nonnull(ty);
    render_mock_type_inner(inner, ns, type_kinds)
}

/// Render the inner type, preserving list wrappers and inner nullability.
fn render_mock_type_inner(ty: &GraphQLType, ns: &str, type_kinds: &HashMap<String, TypeKind>) -> String {
    match ty {
        GraphQLType::Named(name) => render_mock_named_type(name, ns, type_kinds),
        GraphQLType::NonNull(inner) => render_mock_type_inner(inner, ns, type_kinds),
        GraphQLType::List(inner) => {
            // Inner nullable types become Optional in the list
            let inner_str = match inner.as_ref() {
                GraphQLType::NonNull(inner_inner) => {
                    render_mock_type_inner(inner_inner, ns, type_kinds)
                }
                other => {
                    // Nullable inner type: add ?
                    format!("{}?", render_mock_type_inner(other, ns, type_kinds))
                }
            };
            format!("[{}]", inner_str)
        }
    }
}

/// Render a named type for @Field annotation.
fn render_mock_named_type(name: &str, ns: &str, type_kinds: &HashMap<String, TypeKind>) -> String {
    match name {
        "String" => "String".to_string(),
        "Int" => "Int".to_string(),
        "Float" => "Double".to_string(),
        "Boolean" => "Bool".to_string(),
        "ID" => format!("{}.ID", ns),
        _ => {
            let kind = classify_type(name, type_kinds);
            match kind {
                TypeKind::Enum => format!("GraphQLEnum<{}.{}>", ns, name),
                TypeKind::Scalar => format!("{}.{}", ns, name), // Custom scalar
                TypeKind::Object | TypeKind::Interface | TypeKind::Union => name.to_string(),
                _ => name.to_string(),
            }
        }
    }
}

/// Render the mock type string for the convenience init parameter.
pub fn render_mock_init_type(ty: &GraphQLType, ns: &str, type_kinds: &HashMap<String, TypeKind>) -> String {
    let inner = strip_outer_nonnull(ty);
    render_mock_init_type_inner(inner, ns, type_kinds)
}

fn render_mock_init_type_inner(ty: &GraphQLType, ns: &str, type_kinds: &HashMap<String, TypeKind>) -> String {
    match ty {
        GraphQLType::Named(name) => render_mock_init_named_type(name, ns, type_kinds),
        GraphQLType::NonNull(inner) => render_mock_init_type_inner(inner, ns, type_kinds),
        GraphQLType::List(inner) => {
            let inner_str = match inner.as_ref() {
                GraphQLType::NonNull(inner_inner) => {
                    render_mock_init_type_inner(inner_inner, ns, type_kinds)
                }
                other => {
                    format!("{}?", render_mock_init_type_inner(other, ns, type_kinds))
                }
            };
            format!("[{}]", inner_str)
        }
    }
}

/// Render a named type for the init parameter.
fn render_mock_init_named_type(name: &str, ns: &str, type_kinds: &HashMap<String, TypeKind>) -> String {
    match name {
        "String" => "String".to_string(),
        "Int" => "Int".to_string(),
        "Float" => "Double".to_string(),
        "Boolean" => "Bool".to_string(),
        "ID" => format!("{}.ID", ns),
        _ => {
            let kind = classify_type(name, type_kinds);
            match kind {
                TypeKind::Enum => format!("GraphQLEnum<{}.{}>", ns, name),
                TypeKind::Scalar => format!("{}.{}", ns, name), // Custom scalar
                TypeKind::Object => format!("Mock<{}>", name),
                TypeKind::Interface | TypeKind::Union => "(any AnyMock)".to_string(),
                _ => name.to_string(),
            }
        }
    }
}

/// Determine the set function (_setScalar, _setEntity, _setList) for a field.
pub fn determine_set_function(ty: &GraphQLType, type_kinds: &HashMap<String, TypeKind>) -> String {
    let inner = strip_outer_nonnull(ty);
    match inner {
        GraphQLType::List(_) => "_setList".to_string(),
        GraphQLType::Named(name) => {
            let kind = classify_type(name, type_kinds);
            match kind {
                TypeKind::Object => "_setEntity".to_string(),
                TypeKind::Interface | TypeKind::Union => "_setEntity".to_string(),
                _ => "_setScalar".to_string(), // Scalar, Enum, CustomScalar
            }
        }
        GraphQLType::NonNull(inner) => determine_set_function_inner(inner, type_kinds),
    }
}

fn determine_set_function_inner(ty: &GraphQLType, type_kinds: &HashMap<String, TypeKind>) -> String {
    match ty {
        GraphQLType::List(_) => "_setList".to_string(),
        GraphQLType::Named(name) => {
            let kind = classify_type(name, type_kinds);
            match kind {
                TypeKind::Object => "_setEntity".to_string(),
                TypeKind::Interface | TypeKind::Union => "_setEntity".to_string(),
                _ => "_setScalar".to_string(),
            }
        }
        GraphQLType::NonNull(inner) => determine_set_function_inner(inner, type_kinds),
    }
}

/// Strip the outermost NonNull wrapper from a type.
fn strip_outer_nonnull(ty: &GraphQLType) -> &GraphQLType {
    match ty {
        GraphQLType::NonNull(inner) => inner.as_ref(),
        other => other,
    }
}

/// Build the type_kinds map from a CompilationResult, for use by the rendering functions.
pub fn build_type_kinds(compilation: &CompilationResult) -> HashMap<String, TypeKind> {
    let mut type_kinds = HashMap::new();

    // Add built-in scalars
    for name in &["String", "Int", "Float", "Boolean", "ID"] {
        type_kinds.insert(name.to_string(), TypeKind::Scalar);
    }

    for named_type in &compilation.referenced_types {
        match named_type {
            GraphQLNamedType::Object(obj) => {
                type_kinds.insert(obj.name.clone(), TypeKind::Object);
            }
            GraphQLNamedType::Interface(iface) => {
                type_kinds.insert(iface.name.clone(), TypeKind::Interface);
            }
            GraphQLNamedType::Union(u) => {
                type_kinds.insert(u.name.clone(), TypeKind::Union);
            }
            GraphQLNamedType::Enum(e) => {
                type_kinds.insert(e.name.clone(), TypeKind::Enum);
            }
            GraphQLNamedType::Scalar(s) => {
                type_kinds.insert(s.name.clone(), TypeKind::Scalar);
            }
            GraphQLNamedType::InputObject(io) => {
                type_kinds.insert(io.name.clone(), TypeKind::InputObject);
            }
        }
    }

    type_kinds
}
