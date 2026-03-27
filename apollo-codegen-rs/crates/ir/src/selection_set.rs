//! IR SelectionSet - represents selections at a specific type scope.

use crate::fields::{EntityField, ScalarField};
use crate::inclusion::InclusionConditions;
use crate::scope::ScopeDescriptor;
use apollo_codegen_frontend::types::GraphQLCompositeType;
use indexmap::IndexMap;

/// A selection set at a specific type scope.
#[derive(Debug)]
pub struct SelectionSet {
    /// The scope descriptor for this selection set.
    pub scope: ScopeDescriptor,
    /// Direct selections from the user's GraphQL document.
    pub direct_selections: DirectSelections,
    /// Whether this selection set requires a __typename field.
    pub needs_typename: bool,
}

impl SelectionSet {
    pub fn parent_type(&self) -> &GraphQLCompositeType {
        &self.scope.parent_type
    }

    pub fn parent_type_name(&self) -> &str {
        self.scope.type_name()
    }
}

/// Tracks which kind of selection appeared at a given source position.
#[derive(Debug, Clone)]
pub enum SelectionKind {
    /// A field selection, identified by its response key.
    Field(String),
    /// An inline fragment, identified by its index in `inline_fragments`.
    InlineFragment(usize),
    /// A named fragment spread, identified by its index in `named_fragments`.
    NamedFragment(usize),
}

/// Direct selections in a selection set.
#[derive(Debug, Default)]
pub struct DirectSelections {
    pub fields: IndexMap<String, FieldSelection>,
    pub inline_fragments: Vec<InlineFragmentSelection>,
    pub named_fragments: Vec<NamedFragmentSpread>,
    /// Source order of selections (fields, inline fragments, named fragments interleaved).
    /// This preserves the original ordering from the GraphQL document.
    pub source_order: Vec<SelectionKind>,
}

/// A field in a selection set (either scalar or entity).
#[derive(Debug)]
pub enum FieldSelection {
    Scalar(ScalarField),
    Entity(EntityField),
}

impl FieldSelection {
    pub fn response_key(&self) -> &str {
        match self {
            FieldSelection::Scalar(f) => f.response_key(),
            FieldSelection::Entity(f) => f.response_key(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            FieldSelection::Scalar(f) => &f.name,
            FieldSelection::Entity(f) => &f.name,
        }
    }

    pub fn description(&self) -> Option<&str> {
        match self {
            FieldSelection::Scalar(f) => f.description.as_deref(),
            FieldSelection::Entity(f) => f.description.as_deref(),
        }
    }
}

/// An inline fragment (type condition) selection.
#[derive(Debug)]
pub struct InlineFragmentSelection {
    pub type_condition: Option<GraphQLCompositeType>,
    pub selection_set: SelectionSet,
    pub inclusion_conditions: Option<InclusionConditions>,
    pub is_deferred: bool,
    pub defer_label: Option<String>,
}

/// A named fragment spread.
#[derive(Debug)]
pub struct NamedFragmentSpread {
    pub fragment_name: String,
    pub inclusion_conditions: Option<InclusionConditions>,
    pub is_deferred: bool,
    pub defer_label: Option<String>,
}
