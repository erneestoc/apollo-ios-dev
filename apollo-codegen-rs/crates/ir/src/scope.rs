//! Scope descriptors - define the type scope for a selection set.

use crate::inclusion::InclusionConditions;
use apollo_codegen_frontend::types::GraphQLCompositeType;

/// Describes the scope at which a selection set exists.
#[derive(Debug, Clone)]
pub struct ScopeDescriptor {
    /// The parent type of this scope.
    pub parent_type: GraphQLCompositeType,
    /// Inclusion conditions that must be met for this scope.
    pub inclusion_conditions: Option<InclusionConditions>,
}

/// A condition that defines a new scope.
#[derive(Debug, Clone)]
pub struct ScopeCondition {
    pub type_condition: Option<GraphQLCompositeType>,
    pub inclusion_conditions: Option<InclusionConditions>,
}

impl ScopeDescriptor {
    pub fn new(parent_type: GraphQLCompositeType) -> Self {
        Self {
            parent_type,
            inclusion_conditions: None,
        }
    }

    pub fn type_name(&self) -> &str {
        self.parent_type.name()
    }
}
