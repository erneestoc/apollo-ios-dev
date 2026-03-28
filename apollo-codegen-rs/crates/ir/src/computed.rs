//! Computed selection set - both direct and merged selections.
//!
//! Mirrors Swift's IR+ComputedSelectionSet.swift.
//! A computed selection set combines direct selections (from the user's GraphQL)
//! with merged selections (from ancestors, siblings, and named fragments).

use crate::entity_selection_tree::{
    ComputedSelectionSetBuilder, EntitySelectionTree, ScopeDescriptorRef, TreeField,
    TreeNamedFragment, MergedInlineFragmentBuilder,
};
use crate::merged_selections::{
    MergedField, MergedInlineFragment, MergedNamedFragment, MergedSelections, MergedSource,
    MergingStrategy, ScopeConditionKey,
};
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

/// A fully computed selection set with proper MergedSelections.
/// This is the "proper" version matching Swift's ComputedSelectionSet.
#[derive(Debug)]
pub struct ComputedSelectionSetV2 {
    pub direct_fields: IndexMap<String, TreeField>,
    pub direct_inline_fragments: Vec<ScopeConditionKey>,
    pub direct_named_fragments: Vec<TreeNamedFragment>,
    pub merged: MergedSelectionsResult,
}

/// The result of computing merged selections from the entity tree.
#[derive(Debug)]
pub struct MergedSelectionsResult {
    pub merged_sources: indexmap::IndexSet<MergedSource>,
    pub merging_strategy: MergingStrategy,
    pub fields: IndexMap<String, TreeField>,
    pub inline_fragments: IndexMap<ScopeConditionKey, MergedInlineFragmentBuilder>,
    pub named_fragments: IndexMap<String, TreeNamedFragment>,
}

impl MergedSelectionsResult {
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
            && self.inline_fragments.is_empty()
            && self.named_fragments.is_empty()
    }
}

/// Build a ComputedSelectionSetV2 from an entity's selection tree.
///
/// This mirrors Swift's ComputedSelectionSet.Builder.build().
pub fn build_computed_selection_set(
    entity_tree: &EntitySelectionTree,
    target_scope_path: &[ScopeDescriptorRef],
    target_entity_scope_path: &[ScopeConditionKey],
    target_matching_types: &[String],
    merging_strategy: MergingStrategy,
    is_entity_root: bool,
    direct_field_keys: Vec<String>,
    direct_fragment_keys: Vec<String>,
    direct_inline_fragment_keys: Vec<ScopeConditionKey>,
) -> ComputedSelectionSetBuilder {
    let mut builder = ComputedSelectionSetBuilder::new(
        merging_strategy,
        is_entity_root,
        direct_field_keys,
        direct_fragment_keys,
        direct_inline_fragment_keys,
    );

    entity_tree.add_merged_selections(
        target_scope_path,
        target_entity_scope_path,
        target_matching_types,
        &mut builder,
    );

    builder
}
