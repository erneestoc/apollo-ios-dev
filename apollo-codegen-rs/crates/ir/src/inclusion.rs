//! Inclusion conditions (@skip/@include handling).

/// A single inclusion condition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InclusionCondition {
    pub variable: String,
    pub is_inverted: bool, // true for @skip, false for @include
}

/// How multiple conditions are combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InclusionOperator {
    /// All conditions must be true (conditions on same field declaration).
    And,
    /// Any condition can be true (separate field declarations merged).
    Or,
}

/// A set of inclusion conditions combined with AND or OR.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct InclusionConditions {
    pub conditions: Vec<InclusionCondition>,
    /// How multiple conditions are combined. Defaults to AND.
    pub operator: Option<InclusionOperator>,
}

impl InclusionConditions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_conditions(conditions: Vec<InclusionCondition>) -> Self {
        Self { conditions, operator: None }
    }

    pub fn from_conditions_with_operator(conditions: Vec<InclusionCondition>, operator: InclusionOperator) -> Self {
        Self { conditions, operator: Some(operator) }
    }

    /// The effective operator (AND by default).
    pub fn effective_operator(&self) -> InclusionOperator {
        self.operator.unwrap_or(InclusionOperator::And)
    }

    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty()
    }

    pub fn is_always_included(&self) -> bool {
        self.conditions.is_empty()
    }
}
