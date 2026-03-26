//! Inclusion conditions (@skip/@include handling).

/// A single inclusion condition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InclusionCondition {
    pub variable: String,
    pub is_inverted: bool, // true for @skip, false for @include
}

/// A set of inclusion conditions combined with AND.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct InclusionConditions {
    pub conditions: Vec<InclusionCondition>,
}

impl InclusionConditions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_conditions(conditions: Vec<InclusionCondition>) -> Self {
        Self { conditions }
    }

    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty()
    }

    pub fn is_always_included(&self) -> bool {
        self.conditions.is_empty()
    }
}
