//! Field types in the IR.

use crate::inclusion::InclusionConditions;
use crate::selection_set::SelectionSet;
use apollo_codegen_frontend::types::{Argument, GraphQLType};

/// A scalar (leaf) field.
#[derive(Debug, Clone)]
pub struct ScalarField {
    pub name: String,
    pub alias: Option<String>,
    pub field_type: GraphQLType,
    pub arguments: Vec<Argument>,
    pub inclusion_conditions: Option<InclusionConditions>,
    pub deprecation_reason: Option<String>,
}

/// An entity (composite) field that has a selection set.
#[derive(Debug)]
pub struct EntityField {
    pub name: String,
    pub alias: Option<String>,
    pub field_type: GraphQLType,
    pub arguments: Vec<Argument>,
    pub inclusion_conditions: Option<InclusionConditions>,
    pub selection_set: SelectionSet,
    pub deprecation_reason: Option<String>,
}

impl ScalarField {
    /// The response key (alias if present, otherwise name).
    pub fn response_key(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}

impl EntityField {
    /// The response key (alias if present, otherwise name).
    pub fn response_key(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}
