//! Computed selection set - both direct and merged selections.

use crate::selection_set::{DirectSelections, FieldSelection, InlineFragmentSelection, NamedFragmentSpread};
use indexmap::IndexMap;

/// A computed selection set containing both direct selections and
/// merged selections from ancestors, siblings, and named fragments.
#[derive(Debug, Default)]
pub struct ComputedSelectionSet {
    pub direct: DirectSelections,
    pub merged_fields: IndexMap<String, FieldSelection>,
    pub merged_inline_fragments: Vec<InlineFragmentSelection>,
    pub merged_named_fragments: Vec<NamedFragmentSpread>,
}
