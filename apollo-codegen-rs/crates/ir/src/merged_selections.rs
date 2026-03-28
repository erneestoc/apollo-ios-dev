//! Merged selections - selections merged from other selection sets.
//!
//! Mirrors Swift's IR+MergedSelections.swift.
//! Represents the selections merged into a selection set from ancestors,
//! sibling inline fragments, and named fragment spreads.

use indexmap::{IndexMap, IndexSet};

/// Selections merged from other selection sets into a target selection set.
///
/// Selections in the MergedSelections are guaranteed to be selected if the
/// target SelectionSet's selections are selected. This means they can be
/// merged into the generated object as field accessors.
#[derive(Debug, Clone)]
pub struct MergedSelections {
    pub merged_sources: IndexSet<MergedSource>,
    pub merging_strategy: MergingStrategy,
    pub fields: IndexMap<String, MergedField>,
    pub inline_fragments: IndexMap<ScopeConditionKey, MergedInlineFragment>,
    pub named_fragments: IndexMap<String, MergedNamedFragment>,
}

impl MergedSelections {
    pub fn empty(strategy: MergingStrategy) -> Self {
        Self {
            merged_sources: IndexSet::new(),
            merging_strategy: strategy,
            fields: IndexMap::new(),
            inline_fragments: IndexMap::new(),
            named_fragments: IndexMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty() && self.inline_fragments.is_empty() && self.named_fragments.is_empty()
    }
}

/// Identifies the source of a merged selection.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct MergedSource {
    /// The scope path of the source selection set (entity-level path).
    pub scope_path: Vec<ScopeConditionKey>,
    /// The entity scope path within the source selection set.
    pub entity_scope_path: Vec<ScopeConditionKey>,
    /// If from a named fragment, the fragment name.
    pub fragment_name: Option<String>,
}

/// A field that was merged from another selection set.
#[derive(Debug, Clone)]
pub struct MergedField {
    /// The response key (alias if present, otherwise name).
    pub response_key: String,
    /// The field name.
    pub name: String,
    /// The field alias, if any.
    pub alias: Option<String>,
    /// The GraphQL type of the field.
    pub field_type: apollo_codegen_frontend::types::GraphQLType,
    /// Whether this is an entity field (has sub-selections).
    pub is_entity: bool,
    /// The type name of the entity field's selection set parent type (if entity).
    pub entity_type_name: Option<String>,
}

/// A merged inline fragment (type case).
#[derive(Debug, Clone)]
pub struct MergedInlineFragment {
    pub scope_condition: ScopeConditionKey,
    pub derived_from_sources: IndexSet<MergedSource>,
}

/// A merged named fragment spread.
#[derive(Debug, Clone)]
pub struct MergedNamedFragment {
    pub fragment_name: String,
}

/// Key for identifying a scope condition in maps.
/// Matches Swift's ScopeCondition hashability.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ScopeConditionKey {
    pub type_name: Option<String>,
    pub conditions: Option<Vec<ConditionKey>>,
    pub defer_label: Option<String>,
}

impl ScopeConditionKey {
    pub fn new(type_name: Option<String>) -> Self {
        Self {
            type_name,
            conditions: None,
            defer_label: None,
        }
    }

    pub fn with_conditions(type_name: Option<String>, conditions: Option<Vec<ConditionKey>>) -> Self {
        Self {
            type_name,
            conditions,
            defer_label: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.type_name.is_none()
            && self.conditions.as_ref().map_or(true, |c| c.is_empty())
            && self.defer_label.is_none()
    }

    pub fn is_deferred(&self) -> bool {
        self.defer_label.is_some()
    }
}

/// Key for identifying an inclusion condition.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ConditionKey {
    pub variable: String,
    pub is_inverted: bool,
}

/// Determines what types of merged selections are included.
///
/// This is an option set (bitflags) matching Swift's MergingStrategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MergingStrategy(u8);

impl MergingStrategy {
    /// No merging strategy (empty set).
    pub const NONE: Self = Self(0);
    /// Merge selections from direct ancestors.
    pub const ANCESTORS: Self = Self(1 << 0);
    /// Merge selections from sibling inline fragments that match the scope.
    pub const SIBLINGS: Self = Self(1 << 1);
    /// Merge selections from named fragment spreads.
    pub const NAMED_FRAGMENTS: Self = Self(1 << 2);
    /// Merge all possible selections from all sources.
    pub const ALL: Self = Self((1 << 0) | (1 << 1) | (1 << 2));

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for MergingStrategy {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for MergingStrategy {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}
