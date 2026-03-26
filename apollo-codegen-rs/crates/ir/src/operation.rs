//! IR Operation - represents a compiled GraphQL operation.

use crate::fields::EntityField;
use crate::named_fragment::NamedFragment;
use apollo_codegen_frontend::compilation_result::OperationType;
use indexmap::IndexSet;
use std::sync::Arc;

/// A compiled GraphQL operation (query, mutation, or subscription).
#[derive(Debug)]
pub struct Operation {
    /// The operation name.
    pub name: String,
    /// The operation type.
    pub operation_type: OperationType,
    /// The root field (selection set on the root type).
    pub root_field: EntityField,
    /// All fragments referenced by this operation.
    pub referenced_fragments: Vec<Arc<NamedFragment>>,
    /// Whether this is a local cache mutation.
    pub is_local_cache_mutation: bool,
    /// The original GraphQL source text.
    pub source: String,
    /// The file path where this operation is defined.
    pub file_path: String,
    /// Whether this operation contains any deferred fragments.
    pub contains_deferred_fragment: bool,
    /// Variable definitions.
    pub variables: Vec<VariableDefinition>,
}

/// A variable definition on an operation.
#[derive(Debug, Clone)]
pub struct VariableDefinition {
    pub name: String,
    pub type_str: String,
    pub default_value: Option<String>,
}
