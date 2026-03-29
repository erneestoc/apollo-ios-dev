//! EntitySelectionTree - tree tracking all selections for an entity across type scopes.
//!
//! Mirrors Swift's IR+EntitySelectionTree.swift.
//! This data structure memoizes selections for an Entity to quickly compute
//! merged selections for SelectionSets.
//!
//! During IR construction, selections are added to entity trees at appropriate
//! type scopes. During rendering, the tree is traversed to collect merged
//! selections for a given scope.

use crate::merged_selections::{
    ConditionKey, MergedSource, MergingStrategy, ScopeConditionKey,
};
use indexmap::IndexMap;

/// The entity selection tree for a single entity.
/// Tracks all selections across different type scopes.
#[derive(Debug)]
pub struct EntitySelectionTree {
    /// Type path from root entity to this entity.
    pub root_type_path: Vec<String>,
    /// Root node of the tree.
    pub root_node: EntityNode,
}

impl EntitySelectionTree {
    /// Create a new tree for an entity with the given root type path.
    pub fn new(root_type_path: Vec<String>) -> Self {
        let root_node = EntityNode::from_type_path(&root_type_path, 0);
        Self {
            root_type_path,
            root_node,
        }
    }

    // MARK: - Merge Selection Sets Into Tree

    /// Merge direct selections into the tree at the scope described by the scope paths.
    ///
    /// `scope_path` is the entity-level path (one ScopeDescriptorRef per entity level).
    /// Each ScopeDescriptorRef has its own `scope_path` field containing the condition
    /// path within that entity level.
    pub fn merge_in_selections(
        &mut self,
        selections: EntityTreeScopeSelections,
        source: MergedSource,
        scope_path: &[ScopeDescriptorRef],
        _entity_scope_path: &[ScopeConditionKey],
    ) {
        if selections.is_empty() {
            return;
        }

        let target_node = Self::find_or_create_node(
            &mut self.root_node,
            scope_path,
            0,
            &self.root_type_path,
            0,
        );

        target_node.merge_in_selections(selections, source);
    }

    /// Navigate the tree to find or create the target node for the given scope paths.
    ///
    /// This mirrors Swift's findOrCreateNode which simultaneously traverses:
    /// - The enclosing entity scope path (scope_path) - one entry per entity level
    /// - The root type path - determines tree depth
    ///
    /// Within each entity level, the ScopeDescriptorRef's own scope_path
    /// provides the condition path for navigating type cases.
    fn find_or_create_node<'a>(
        node: &'a mut EntityNode,
        scope_path: &[ScopeDescriptorRef],
        scope_idx: usize,
        root_type_path: &[String],
        root_type_idx: usize,
    ) -> &'a mut EntityNode {
        // Get the condition path for the current entity level
        let entity_conditions = &scope_path[scope_idx].scope_path;

        // Check if we've reached the end of the root type path
        if root_type_idx + 1 >= root_type_path.len() {
            // At leaf entity level - navigate by this entity's condition scope path
            return Self::find_or_create_condition_node(
                node,
                entity_conditions,
                0,
            );
        }

        // Check if the current entity level has more conditions to navigate
        // entity_conditions[0] is the root condition of this entity, additional
        // elements are type case conditions (inline fragments)
        if entity_conditions.len() <= 1 {
            // No additional conditions - advance to next entity
            let next_scope_idx = scope_idx + 1;
            if next_scope_idx >= scope_path.len() {
                return node;
            }

            let next_entity_node = node.child_as_entity_node(&root_type_path[root_type_idx + 1]);

            return Self::find_or_create_node(
                next_entity_node,
                scope_path,
                next_scope_idx,
                root_type_path,
                root_type_idx + 1,
            );
        }

        // Navigate through type case conditions within this entity,
        // then advance to next entity
        let mut current_node = node;
        for i in 1..entity_conditions.len() {
            let condition = &entity_conditions[i];
            if current_node.scope != *condition {
                current_node = current_node.scope_condition_node(condition.clone());
            }
        }

        // After navigating all conditions, advance to next entity
        let next_scope_idx = scope_idx + 1;
        if next_scope_idx >= scope_path.len() {
            return current_node;
        }

        let next_entity_node = current_node.child_as_entity_node(&root_type_path[root_type_idx + 1]);

        Self::find_or_create_node(
            next_entity_node,
            scope_path,
            next_scope_idx,
            root_type_path,
            root_type_idx + 1,
        )
    }

    /// Navigate by condition scope path to find the target leaf node.
    fn find_or_create_condition_node<'a>(
        node: &'a mut EntityNode,
        scope_path: &[ScopeConditionKey],
        idx: usize,
    ) -> &'a mut EntityNode {
        if idx >= scope_path.len() {
            return node;
        }

        let condition = &scope_path[idx];
        let next_node = if *condition != node.scope {
            node.scope_condition_node(condition.clone())
        } else {
            node
        };

        if idx + 1 >= scope_path.len() {
            return next_node;
        }

        Self::find_or_create_condition_node(next_node, scope_path, idx + 1)
    }

    // MARK: - Calculate Merged Selections From Tree

    /// Compute merged selections for a target selection set and add them to the builder.
    pub fn add_merged_selections(
        &self,
        target_scope_path: &[ScopeDescriptorRef],
        target_entity_scope_path: &[ScopeConditionKey],
        target_matching_types: &[String],
        builder: &mut ComputedSelectionSetBuilder,
    ) {
        self.root_node.merge_selections(
            target_scope_path,
            0,
            target_entity_scope_path,
            0,
            target_matching_types,
            builder,
            MergingStrategy::ANCESTORS,
            &None,
        );
    }

    // MARK: - Merge In Other Entity Trees

    /// Merge another entity's selection tree (from a fragment spread) into this tree.
    pub fn merge_in_tree(
        &mut self,
        other_tree: &EntitySelectionTree,
        fragment_spread: &FragmentSpreadInfo,
        spread_scope_path: &[ScopeConditionKey],
    ) {
        let other_count = other_tree.root_type_path.len();
        let self_count = self.root_type_path.len();
        assert!(self_count >= other_count, "Cannot merge in tree shallower than current tree.");

        let diff = self_count - other_count;

        // Navigate down to the matching depth
        let mut target = &mut self.root_node;
        for _ in 0..diff {
            let type_name = target.entity_type_name.clone();
            target = target.child_as_entity_node(&type_name);
        }

        // Navigate to the correct scope condition node based on the spread's scope
        let merge_root = Self::navigate_to_fragment_scope(target, spread_scope_path, 0);

        // Determine if types match
        let fragment_type = &fragment_spread.type_condition_name;
        let root_types_match = merge_root.entity_type_name == *fragment_type;

        if let Some(ref inclusion_conditions) = fragment_spread.inclusion_conditions {
            // For each condition group, create a scope node and merge the fragment tree
            for condition_group in inclusion_conditions {
                let scope = ScopeConditionKey {
                    type_name: if root_types_match { None } else { Some(fragment_type.clone()) },
                    conditions: Some(condition_group.clone()),
                    defer_label: None,
                };
                let node = merge_root.scope_condition_node(scope);
                node.merged_fragment_trees.push(MergedFragmentTree {
                    fragment_spread: fragment_spread.clone(),
                    tree_root_node_snapshot: snapshot_node(&other_tree.root_node),
                });
            }
        } else {
            let node = if root_types_match {
                merge_root
            } else {
                let scope = ScopeConditionKey::new(Some(fragment_type.clone()));
                merge_root.scope_condition_node(scope)
            };
            node.merged_fragment_trees.push(MergedFragmentTree {
                fragment_spread: fragment_spread.clone(),
                tree_root_node_snapshot: snapshot_node(&other_tree.root_node),
            });
        }
    }

    fn navigate_to_fragment_scope<'a>(
        node: &'a mut EntityNode,
        scope_path: &[ScopeConditionKey],
        idx: usize,
    ) -> &'a mut EntityNode {
        if idx + 1 >= scope_path.len() {
            return node;
        }
        let next_idx = idx + 1;
        let next_scope = scope_path[next_idx].clone();
        let next_node = node.scope_condition_node(next_scope);
        Self::navigate_to_fragment_scope(next_node, scope_path, next_idx)
    }
}

/// Collect ALL fields from ALL scopes in the entity's selection tree.
/// This provides the complete set of fields that any scope selects on this entity,
/// deduplicated by response key.
pub fn collect_all_entity_fields(tree: &EntitySelectionTree) -> IndexMap<String, TreeField> {
    let mut fields = IndexMap::new();
    collect_fields_recursive(&tree.root_node, &mut fields);
    fields
}

fn collect_fields_recursive(node: &EntityNode, fields: &mut IndexMap<String, TreeField>) {
    match &node.child {
        Some(EntityNodeChild::Entity(child)) => {
            collect_fields_recursive(child, fields);
        }
        Some(EntityNodeChild::Selections(sel_map)) => {
            for (_, scope_selections) in sel_map {
                for (key, field) in &scope_selections.fields {
                    if !fields.contains_key(key) {
                        fields.insert(key.clone(), field.clone());
                    }
                }
            }
        }
        None => {}
    }
    // Also collect from scope condition children (inline fragment scopes)
    for (_, cond_node) in &node.scope_conditions {
        collect_fields_recursive(cond_node, fields);
    }
    // And from merged fragment trees
    for merged_frag in &node.merged_fragment_trees {
        collect_fields_from_snapshot(&merged_frag.tree_root_node_snapshot, fields);
    }
}

fn collect_fields_from_snapshot(node: &EntityNodeSnapshot, fields: &mut IndexMap<String, TreeField>) {
    match &node.child {
        Some(EntityNodeChildSnapshot::Entity(child)) => {
            collect_fields_from_snapshot(child, fields);
        }
        Some(EntityNodeChildSnapshot::Selections(sel_map)) => {
            for (_, scope_selections) in sel_map {
                for (key, field) in &scope_selections.fields {
                    if !fields.contains_key(key) {
                        fields.insert(key.clone(), field.clone());
                    }
                }
            }
        }
        None => {}
    }
    for (_, cond_node) in &node.scope_conditions {
        collect_fields_from_snapshot(cond_node, fields);
    }
    for merged_frag in &node.merged_fragment_trees {
        collect_fields_from_snapshot(&merged_frag.tree_root_node_snapshot, fields);
    }
}

/// A node in the entity selection tree.
#[derive(Debug)]
pub struct EntityNode {
    /// The scope condition for this node.
    pub scope: ScopeConditionKey,
    /// The entity type name at this node.
    pub entity_type_name: String,
    /// Index into root_type_path.
    pub root_type_path_index: usize,
    /// Child: either a deeper entity node or leaf selections.
    pub child: Option<EntityNodeChild>,
    /// Conditional scope children (inline fragments).
    pub scope_conditions: IndexMap<ScopeConditionKey, EntityNode>,
    /// Fragment trees merged into this node.
    pub merged_fragment_trees: Vec<MergedFragmentTree>,
}

/// The child of an entity node.
#[derive(Debug)]
pub enum EntityNodeChild {
    /// A child entity node (for multi-level entity paths).
    Entity(Box<EntityNode>),
    /// Leaf selections indexed by their source.
    Selections(IndexMap<MergedSource, EntityTreeScopeSelections>),
}

/// Selections at a specific scope in the entity tree.
#[derive(Debug, Clone, Default)]
pub struct EntityTreeScopeSelections {
    /// Fields keyed by response key.
    pub fields: IndexMap<String, TreeField>,
    /// Named fragments keyed by fragment name.
    pub named_fragments: IndexMap<String, TreeNamedFragment>,
}

impl EntityTreeScopeSelections {
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty() && self.named_fragments.is_empty()
    }

    pub fn merge_in(&mut self, other: &EntityTreeScopeSelections) {
        for (key, field) in &other.fields {
            self.fields.insert(key.clone(), field.clone());
        }
        for (key, frag) in &other.named_fragments {
            self.named_fragments.insert(key.clone(), frag.clone());
        }
    }

    pub fn merge_in_fields_and_fragments(
        &mut self,
        fields: &IndexMap<String, TreeField>,
        fragments: &IndexMap<String, TreeNamedFragment>,
    ) {
        for (key, field) in fields {
            self.fields.insert(key.clone(), field.clone());
        }
        for (key, frag) in fragments {
            self.named_fragments.insert(key.clone(), frag.clone());
        }
    }
}

/// A field stored in the entity tree.
#[derive(Debug, Clone)]
pub struct TreeField {
    pub response_key: String,
    pub name: String,
    pub alias: Option<String>,
    pub field_type: apollo_codegen_frontend::types::GraphQLType,
    pub is_entity: bool,
    pub entity_type_name: Option<String>,
    pub arguments: Vec<apollo_codegen_frontend::types::Argument>,
    pub inclusion_conditions: Option<crate::inclusion::InclusionConditions>,
    pub deprecation_reason: Option<String>,
    pub description: Option<String>,
}

/// A named fragment reference stored in the entity tree.
#[derive(Debug, Clone)]
pub struct TreeNamedFragment {
    pub fragment_name: String,
    pub inclusion_conditions: Option<crate::inclusion::InclusionConditions>,
}

/// Information about a fragment spread for tree merging.
#[derive(Debug, Clone)]
pub struct FragmentSpreadInfo {
    pub fragment_name: String,
    pub type_condition_name: String,
    /// Inclusion conditions (outer Vec = OR groups, inner Vec = AND conditions within a group).
    pub inclusion_conditions: Option<Vec<Vec<ConditionKey>>>,
    /// The scope path of the TypeInfo where the fragment was spread.
    pub scope_path: Vec<ScopeDescriptorRef>,
}

/// A snapshot of a merged fragment's selection tree root.
#[derive(Debug, Clone)]
pub struct MergedFragmentTree {
    pub fragment_spread: FragmentSpreadInfo,
    pub tree_root_node_snapshot: EntityNodeSnapshot,
}

/// A lightweight reference to a ScopeDescriptor for tree navigation.
#[derive(Debug, Clone)]
pub struct ScopeDescriptorRef {
    pub type_name: String,
    pub scope_path: Vec<ScopeConditionKey>,
    pub matching_types: Vec<String>,
}

impl ScopeDescriptorRef {
    pub fn matches_condition(&self, condition: &ScopeConditionKey) -> bool {
        // Check type match
        if let Some(ref type_name) = condition.type_name {
            if !self.matching_types.contains(type_name) {
                return false;
            }
        }
        // Check inclusion conditions match
        if let Some(ref conditions) = condition.conditions {
            if !conditions.is_empty() {
                // TODO: proper inclusion condition subset checking
                // For now, check if all condition variables are in scope
                return true;
            }
        }
        true
    }
}

/// A read-only snapshot of an entity node for merging.
#[derive(Debug, Clone)]
pub struct EntityNodeSnapshot {
    pub scope: ScopeConditionKey,
    pub entity_type_name: String,
    pub child: Option<EntityNodeChildSnapshot>,
    pub scope_conditions: IndexMap<ScopeConditionKey, EntityNodeSnapshot>,
    pub merged_fragment_trees: Vec<MergedFragmentTree>,
}

#[derive(Debug, Clone)]
pub enum EntityNodeChildSnapshot {
    Entity(Box<EntityNodeSnapshot>),
    Selections(IndexMap<MergedSource, EntityTreeScopeSelections>),
}

/// Create a snapshot of an entity node (for storing in merged fragment trees).
fn snapshot_node(node: &EntityNode) -> EntityNodeSnapshot {
    EntityNodeSnapshot {
        scope: node.scope.clone(),
        entity_type_name: node.entity_type_name.clone(),
        child: node.child.as_ref().map(|c| match c {
            EntityNodeChild::Entity(e) => EntityNodeChildSnapshot::Entity(Box::new(snapshot_node(e))),
            EntityNodeChild::Selections(s) => EntityNodeChildSnapshot::Selections(s.clone()),
        }),
        scope_conditions: node.scope_conditions.iter().map(|(k, v)| (k.clone(), snapshot_node(v))).collect(),
        merged_fragment_trees: node.merged_fragment_trees.clone(),
    }
}

impl EntityNode {
    /// Create a node for the given position in the root type path.
    fn from_type_path(root_type_path: &[String], index: usize) -> Self {
        let type_name = root_type_path[index].clone();
        let scope = ScopeConditionKey::new(Some(type_name.clone()));

        let child = if index + 1 < root_type_path.len() {
            Some(EntityNodeChild::Entity(Box::new(Self::from_type_path(
                root_type_path,
                index + 1,
            ))))
        } else {
            Some(EntityNodeChild::Selections(IndexMap::new()))
        };

        Self {
            scope,
            entity_type_name: type_name,
            root_type_path_index: index,
            child,
            scope_conditions: IndexMap::new(),
            merged_fragment_trees: Vec::new(),
        }
    }

    /// Merge selections into this node's leaf selections.
    pub fn merge_in_selections(
        &mut self,
        selections: EntityTreeScopeSelections,
        source: MergedSource,
    ) {
        match &mut self.child {
            Some(EntityNodeChild::Selections(sel)) => {
                sel.entry(source)
                    .or_insert_with(EntityTreeScopeSelections::default)
                    .merge_in(&selections);
            }
            None => {
                let mut map = IndexMap::new();
                map.insert(source, selections);
                self.child = Some(EntityNodeChild::Selections(map));
            }
            Some(EntityNodeChild::Entity(_)) => {
                panic!("Selection Merging Error: attempted to merge selections into an entity node.");
            }
        }
    }

    /// Get or create a child entity node.
    fn child_as_entity_node(&mut self, type_name: &str) -> &mut EntityNode {
        match self.child {
            Some(EntityNodeChild::Entity(_)) => {
                // Already have entity child - return it
            }
            Some(EntityNodeChild::Selections(_)) => {
                panic!(
                    "Selection Merging Error: attempted to get entity child from selections node. \
                     Node type: {}, scope: {:?}, requested type: {}",
                    self.entity_type_name, self.scope, type_name
                );
            }
            None => {
                // Create new entity node
                let node = EntityNode {
                    scope: ScopeConditionKey::new(Some(type_name.to_string())),
                    entity_type_name: type_name.to_string(),
                    root_type_path_index: self.root_type_path_index + 1,
                    child: Some(EntityNodeChild::Selections(IndexMap::new())),
                    scope_conditions: IndexMap::new(),
                    merged_fragment_trees: Vec::new(),
                };
                self.child = Some(EntityNodeChild::Entity(Box::new(node)));
            }
        }
        match &mut self.child {
            Some(EntityNodeChild::Entity(e)) => e.as_mut(),
            _ => unreachable!(),
        }
    }

    /// Get or create a scope condition child node.
    fn scope_condition_node(&mut self, condition: ScopeConditionKey) -> &mut EntityNode {
        // Normalize: strip type if same as current node
        let node_condition = ScopeConditionKey {
            type_name: if condition.type_name.as_deref() == Some(&self.entity_type_name) {
                None
            } else {
                condition.type_name.clone()
            },
            conditions: condition.conditions.clone(),
            defer_label: condition.defer_label.clone(),
        };

        if !self.scope_conditions.contains_key(&node_condition) {
            let node_type = node_condition
                .type_name
                .as_ref()
                .unwrap_or(&self.entity_type_name)
                .clone();
            let node = EntityNode {
                scope: node_condition.clone(),
                entity_type_name: node_type,
                root_type_path_index: self.root_type_path_index,
                child: None,
                scope_conditions: IndexMap::new(),
                merged_fragment_trees: Vec::new(),
            };
            self.scope_conditions.insert(node_condition.clone(), node);
        }

        self.scope_conditions.get_mut(&node_condition).unwrap()
    }

    // MARK: - Merge Selections Computation

    /// Traverse the tree to collect merged selections for a target scope.
    ///
    /// This is the core algorithm from Swift's EntityNode.mergeSelections().
    fn merge_selections(
        &self,
        target_scope_path: &[ScopeDescriptorRef],
        scope_idx: usize,
        entity_scope_path: &[ScopeConditionKey],
        entity_scope_idx: usize,
        target_matching_types: &[String],
        builder: &mut ComputedSelectionSetBuilder,
        current_merge_strategy: MergingStrategy,
        transform_fragment: &Option<String>,
    ) {
        // Process child (entity or selections)
        match &self.child {
            Some(EntityNodeChild::Entity(entity_node)) => {
                let next_scope_idx = scope_idx + 1;
                if next_scope_idx >= target_scope_path.len() {
                    return;
                }

                let merge_strategy = self.calculate_merge_strategy_for_next_entity(
                    current_merge_strategy,
                    entity_scope_path,
                    entity_scope_idx,
                );

                let next_entity_scope = &target_scope_path[next_scope_idx].scope_path;
                entity_node.merge_selections(
                    target_scope_path,
                    next_scope_idx,
                    next_entity_scope,
                    0,
                    target_matching_types,
                    builder,
                    merge_strategy,
                    transform_fragment,
                );
            }
            Some(EntityNodeChild::Selections(selections)) => {
                // Determine if this is the target's exact scope
                let is_targets_exact_scope = entity_scope_idx + 1 >= entity_scope_path.len()
                    && current_merge_strategy == MergingStrategy::ANCESTORS;
                let merge_strategy = if is_targets_exact_scope {
                    MergingStrategy::NONE
                } else {
                    current_merge_strategy
                };

                for (source, scope_selections) in selections {
                    let source_with_fragment = if let Some(frag_name) = transform_fragment {
                        let mut s = source.clone();
                        if s.fragment_name.is_none() {
                            s.fragment_name = Some(frag_name.clone());
                        }
                        s
                    } else {
                        source.clone()
                    };
                    builder.merge_in(scope_selections, &source_with_fragment, merge_strategy);
                }
            }
            None => {}
        }

        // Process scope condition children
        for (condition, cond_node) in &self.scope_conditions {
            if condition.is_deferred() {
                continue;
            }

            // Check if this is an ancestor condition
            if entity_scope_idx + 1 < entity_scope_path.len()
                && entity_scope_path[entity_scope_idx + 1] == *condition
            {
                // Ancestor: continue traversal with ancestor strategy
                cond_node.merge_selections(
                    target_scope_path,
                    scope_idx,
                    entity_scope_path,
                    entity_scope_idx + 1,
                    target_matching_types,
                    builder,
                    MergingStrategy::ANCESTORS,
                    transform_fragment,
                );
            } else if scope_idx < target_scope_path.len()
                && target_scope_path[scope_idx].matches_condition(condition)
            {
                // Sibling: merge with sibling strategy
                cond_node.merge_selections(
                    target_scope_path,
                    scope_idx,
                    entity_scope_path,
                    entity_scope_idx,
                    target_matching_types,
                    builder,
                    MergingStrategy::SIBLINGS,
                    transform_fragment,
                );
            } else if matches!(&self.child, Some(EntityNodeChild::Selections(_))) {
                // Add as merged inline fragment
                if let Some(EntityNodeChild::Selections(cond_selections)) = &cond_node.child {
                    let sources: Vec<MergedSource> = cond_selections.keys().cloned().collect();
                    builder.add_merged_inline_fragment(
                        condition.clone(),
                        sources,
                        current_merge_strategy,
                    );
                }
            }
        }

        // Process merged fragment trees
        for merged_frag in &self.merged_fragment_trees {
            let frag_name = &merged_frag.fragment_spread.fragment_name;

            // Determine merge strategy for fragment
            let frag_merge_strategy = if scope_idx < target_scope_path.len()
                && is_same_type_info(
                    &merged_frag.fragment_spread.scope_path,
                    target_scope_path,
                )
            {
                MergingStrategy::NAMED_FRAGMENTS
            } else {
                current_merge_strategy | MergingStrategy::NAMED_FRAGMENTS
            };

            let frag_transform = Some(frag_name.clone());

            // Merge from the fragment tree snapshot
            merge_from_snapshot(
                &merged_frag.tree_root_node_snapshot,
                target_scope_path,
                scope_idx,
                entity_scope_path,
                entity_scope_idx,
                target_matching_types,
                builder,
                frag_merge_strategy,
                &frag_transform,
            );
        }
    }

    fn calculate_merge_strategy_for_next_entity(
        &self,
        current_strategy: MergingStrategy,
        entity_scope_path: &[ScopeConditionKey],
        entity_scope_idx: usize,
    ) -> MergingStrategy {
        if current_strategy.contains(MergingStrategy::SIBLINGS) {
            return current_strategy;
        }

        // If at end of entity scope path, we're traversing a direct ancestor
        let mut new_strategy = if entity_scope_idx + 1 >= entity_scope_path.len() {
            MergingStrategy::ANCESTORS
        } else {
            MergingStrategy::SIBLINGS
        };

        // Preserve named fragments flag
        if current_strategy.contains(MergingStrategy::NAMED_FRAGMENTS) {
            new_strategy.insert(MergingStrategy::NAMED_FRAGMENTS);
        }

        new_strategy
    }
}

/// Merge selections from a snapshot node (for fragment tree traversal).
fn merge_from_snapshot(
    snapshot: &EntityNodeSnapshot,
    target_scope_path: &[ScopeDescriptorRef],
    scope_idx: usize,
    entity_scope_path: &[ScopeConditionKey],
    entity_scope_idx: usize,
    target_matching_types: &[String],
    builder: &mut ComputedSelectionSetBuilder,
    current_merge_strategy: MergingStrategy,
    transform_fragment: &Option<String>,
) {
    // Process child
    match &snapshot.child {
        Some(EntityNodeChildSnapshot::Entity(entity_node)) => {
            let next_scope_idx = scope_idx + 1;
            if next_scope_idx >= target_scope_path.len() {
                return;
            }

            // Calculate strategy for next entity
            let new_strategy = if current_merge_strategy.contains(MergingStrategy::SIBLINGS) {
                current_merge_strategy
            } else {
                let mut s = if entity_scope_idx + 1 >= entity_scope_path.len() {
                    MergingStrategy::ANCESTORS
                } else {
                    MergingStrategy::SIBLINGS
                };
                if current_merge_strategy.contains(MergingStrategy::NAMED_FRAGMENTS) {
                    s.insert(MergingStrategy::NAMED_FRAGMENTS);
                }
                s
            };

            let next_entity_scope = &target_scope_path[next_scope_idx].scope_path;
            merge_from_snapshot(
                entity_node,
                target_scope_path,
                next_scope_idx,
                next_entity_scope,
                0,
                target_matching_types,
                builder,
                new_strategy,
                transform_fragment,
            );
        }
        Some(EntityNodeChildSnapshot::Selections(selections)) => {
            let is_targets_exact_scope = entity_scope_idx + 1 >= entity_scope_path.len()
                && current_merge_strategy == MergingStrategy::ANCESTORS;
            let merge_strategy = if is_targets_exact_scope {
                MergingStrategy::NONE
            } else {
                current_merge_strategy
            };

            for (source, scope_selections) in selections {
                let source_with_fragment = if let Some(frag_name) = transform_fragment {
                    let mut s = source.clone();
                    if s.fragment_name.is_none() {
                        s.fragment_name = Some(frag_name.clone());
                    }
                    s
                } else {
                    source.clone()
                };
                builder.merge_in(scope_selections, &source_with_fragment, merge_strategy);
            }
        }
        None => {}
    }

    // Process scope condition children
    for (condition, cond_node) in &snapshot.scope_conditions {
        if condition.is_deferred() {
            continue;
        }

        if entity_scope_idx + 1 < entity_scope_path.len()
            && entity_scope_path[entity_scope_idx + 1] == *condition
        {
            merge_from_snapshot(
                cond_node,
                target_scope_path,
                scope_idx,
                entity_scope_path,
                entity_scope_idx + 1,
                target_matching_types,
                builder,
                MergingStrategy::ANCESTORS,
                transform_fragment,
            );
        } else if scope_idx < target_scope_path.len()
            && target_scope_path[scope_idx].matches_condition(condition)
        {
            merge_from_snapshot(
                cond_node,
                target_scope_path,
                scope_idx,
                entity_scope_path,
                entity_scope_idx,
                target_matching_types,
                builder,
                MergingStrategy::SIBLINGS,
                transform_fragment,
            );
        } else if matches!(&snapshot.child, Some(EntityNodeChildSnapshot::Selections(_))) {
            if let Some(EntityNodeChildSnapshot::Selections(cond_selections)) = &cond_node.child {
                let sources: Vec<MergedSource> = cond_selections.keys().cloned().collect();
                builder.add_merged_inline_fragment(
                    condition.clone(),
                    sources,
                    current_merge_strategy,
                );
            }
        }
    }

    // Process merged fragment trees
    for merged_frag in &snapshot.merged_fragment_trees {
        let frag_name = &merged_frag.fragment_spread.fragment_name;
        let frag_merge_strategy = if scope_idx < target_scope_path.len()
            && is_same_type_info(&merged_frag.fragment_spread.scope_path, target_scope_path)
        {
            MergingStrategy::NAMED_FRAGMENTS
        } else {
            current_merge_strategy | MergingStrategy::NAMED_FRAGMENTS
        };

        let frag_transform = Some(frag_name.clone());
        merge_from_snapshot(
            &merged_frag.tree_root_node_snapshot,
            target_scope_path,
            scope_idx,
            entity_scope_path,
            entity_scope_idx,
            target_matching_types,
            builder,
            frag_merge_strategy,
            &frag_transform,
        );
    }
}

/// Compute the merged source names that should appear in fulfilledFragments.
/// Returns a list of fully-qualified Swift type names (e.g., "HeightInMeters",
/// "AllAnimalsQuery.Data.AllAnimal.AsPet", "PetDetails.AsDog") that the tree's
/// merged selections contributed to the target scope.
///
/// This mirrors Swift's Phase 2 of InitializerFulfilledFragments: iterate
/// over merged.mergedSources and call generatedSelectionSetNamesOfFullfilledFragments.
pub fn compute_fulfilled_fragment_names(
    tree: &EntitySelectionTree,
    scope_path: &[ScopeDescriptorRef],
    direct_field_keys: &[String],
    direct_fragment_keys: &[String],
    operation_name: Option<&str>,
    naming_fn: &dyn Fn(&MergedSource) -> Vec<String>,
) -> Vec<String> {
    let entity_scope = scope_path.last()
        .map(|s| s.scope_path.clone())
        .unwrap_or_default();
    let matching = scope_path.last()
        .map(|s| s.matching_types.clone())
        .unwrap_or_default();

    let mut builder = ComputedSelectionSetBuilder::new(
        MergingStrategy::ALL,
        true,
        direct_field_keys.to_vec(),
        direct_fragment_keys.to_vec(),
        vec![],
    );

    tree.add_merged_selections(
        scope_path,
        &entity_scope,
        &matching,
        &mut builder,
    );

    let mut names = Vec::new();
    for source in &builder.merged_sources {
        names.extend(naming_fn(source));
    }
    names
}

fn is_same_type_info(a: &[ScopeDescriptorRef], b: &[ScopeDescriptorRef]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (sa, sb) in a.iter().zip(b.iter()) {
        if sa.type_name != sb.type_name || sa.scope_path != sb.scope_path {
            return false;
        }
    }
    true
}

/// Builder for ComputedSelectionSet - accumulates merged selections.
///
/// Mirrors Swift's ComputedSelectionSet.Builder.
#[derive(Debug)]
pub struct ComputedSelectionSetBuilder {
    pub merging_strategy: MergingStrategy,
    pub is_entity_root: bool,
    pub direct_field_keys: Vec<String>,
    pub direct_fragment_keys: Vec<String>,
    pub direct_inline_fragment_keys: Vec<ScopeConditionKey>,
    // Accumulated merged selections
    pub merged_sources: indexmap::IndexSet<MergedSource>,
    pub merged_fields: IndexMap<String, TreeField>,
    pub merged_inline_fragments: IndexMap<ScopeConditionKey, MergedInlineFragmentBuilder>,
    pub merged_named_fragments: IndexMap<String, TreeNamedFragment>,
    /// Sources this selection set was derived from (for shouldMergeIn checks).
    pub derived_from_merged_sources: Vec<MergedSource>,
}

#[derive(Debug, Clone)]
pub struct MergedInlineFragmentBuilder {
    pub scope_condition: ScopeConditionKey,
    pub sources: Vec<MergedSource>,
}

impl ComputedSelectionSetBuilder {
    pub fn new(
        merging_strategy: MergingStrategy,
        is_entity_root: bool,
        direct_field_keys: Vec<String>,
        direct_fragment_keys: Vec<String>,
        direct_inline_fragment_keys: Vec<ScopeConditionKey>,
    ) -> Self {
        Self {
            merging_strategy,
            is_entity_root,
            direct_field_keys,
            direct_fragment_keys,
            direct_inline_fragment_keys,
            merged_sources: indexmap::IndexSet::new(),
            merged_fields: IndexMap::new(),
            merged_inline_fragments: IndexMap::new(),
            merged_named_fragments: IndexMap::new(),
            derived_from_merged_sources: Vec::new(),
        }
    }

    /// Check if this source's selections should be merged.
    fn should_merge_in(&self, source: &MergedSource, strategy: MergingStrategy) -> bool {
        if self.merging_strategy.contains(strategy) {
            return true;
        }
        // Also merge if derived from this source
        for derived in &self.derived_from_merged_sources {
            if derived.scope_path == source.scope_path
                && derived.fragment_name == source.fragment_name
            {
                return true;
            }
        }
        false
    }

    fn should_merge_in_sources(&self, sources: &[MergedSource], strategy: MergingStrategy) -> bool {
        if self.merging_strategy.contains(strategy) {
            return true;
        }
        for source in sources {
            for derived in &self.derived_from_merged_sources {
                if derived.scope_path == source.scope_path
                    && derived.fragment_name == source.fragment_name
                {
                    return true;
                }
            }
        }
        false
    }

    /// Merge selections from an entity tree scope.
    pub fn merge_in(
        &mut self,
        selections: &EntityTreeScopeSelections,
        source: &MergedSource,
        strategy: MergingStrategy,
    ) {
        if !self.should_merge_in(source, strategy) {
            return;
        }

        let mut did_merge = false;

        for (key, field) in &selections.fields {
            if self.merge_in_field(key, field) {
                did_merge = true;
            }
        }

        for (key, frag) in &selections.named_fragments {
            if self.merge_in_named_fragment(key, frag) {
                did_merge = true;
            }
        }

        if did_merge {
            self.merged_sources.insert(source.clone());
        }
    }

    fn merge_in_field(&mut self, key: &str, field: &TreeField) -> bool {
        // Skip if already in direct selections
        if self.direct_field_keys.contains(&key.to_string()) {
            return false;
        }
        // Merge (last write wins for same key, which is correct)
        self.merged_fields.insert(key.to_string(), field.clone());
        true
    }

    fn merge_in_named_fragment(&mut self, key: &str, frag: &TreeNamedFragment) -> bool {
        if self.direct_fragment_keys.contains(&key.to_string()) {
            return false;
        }
        self.merged_named_fragments
            .insert(key.to_string(), frag.clone());
        true
    }

    /// Add a merged inline fragment.
    pub fn add_merged_inline_fragment(
        &mut self,
        condition: ScopeConditionKey,
        sources: Vec<MergedSource>,
        merge_strategy: MergingStrategy,
    ) {
        if !self.is_entity_root {
            return;
        }
        if !self.should_merge_in_sources(&sources, merge_strategy) {
            return;
        }
        if self.direct_inline_fragment_keys.contains(&condition) {
            return;
        }
        let entry = self
            .merged_inline_fragments
            .entry(condition.clone())
            .or_insert_with(|| MergedInlineFragmentBuilder {
                scope_condition: condition,
                sources: Vec::new(),
            });
        entry.sources.extend(sources);
    }
}
