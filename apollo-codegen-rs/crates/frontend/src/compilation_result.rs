//! CompilationResult - the output of the GraphQL frontend.
//!
//! This mirrors the Swift `CompilationResult` class, which is the bridge
//! between the GraphQL frontend and the IR builder.

// Re-export all types for use by downstream crates
pub use crate::types::*;
pub use indexmap::IndexSet;

/// The result of compiling GraphQL schemas and operations.
#[derive(Debug)]
pub struct CompilationResult {
    pub root_types: RootTypeDefinition,
    pub referenced_types: Vec<GraphQLNamedType>,
    pub operations: Vec<OperationDefinition>,
    pub fragments: Vec<FragmentDefinition>,
    pub schema_documentation: Option<String>,
}

/// Root type definitions for the schema.
#[derive(Debug)]
pub struct RootTypeDefinition {
    pub query_type: GraphQLNamedType,
    pub mutation_type: Option<GraphQLNamedType>,
    pub subscription_type: Option<GraphQLNamedType>,
}

/// A compiled GraphQL operation (query, mutation, or subscription).
#[derive(Debug)]
pub struct OperationDefinition {
    pub name: String,
    pub operation_type: OperationType,
    pub variables: Vec<VariableDefinition>,
    pub root_type: GraphQLCompositeType,
    pub selection_set: SelectionSet,
    pub directives: Option<Vec<Directive>>,
    pub referenced_fragments: Vec<String>,
    pub source: String,
    pub file_path: String,
    pub is_local_cache_mutation: bool,
    pub module_imports: IndexSet<String>,
}

/// A compiled named fragment.
#[derive(Debug)]
pub struct FragmentDefinition {
    pub name: String,
    pub type_condition: GraphQLCompositeType,
    pub selection_set: SelectionSet,
    pub directives: Option<Vec<Directive>>,
    pub referenced_fragments: Vec<String>,
    pub source: String,
    pub file_path: String,
    pub is_local_cache_mutation: bool,
    pub module_imports: IndexSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Query,
    Mutation,
    Subscription,
}
