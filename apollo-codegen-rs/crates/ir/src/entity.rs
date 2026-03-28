//! Entity - represents a concrete entity in an operation/fragment.
//!
//! Mirrors Swift's IR+Entity.swift.
//! An entity is a concrete object in the response tree that fields are selected upon.
//! Multiple SelectionSets may select fields on the same Entity.

use crate::entity_selection_tree::EntitySelectionTree;

/// An entity in the selection tree.
#[derive(Debug)]
pub struct Entity {
    /// Where this entity is located in the operation/fragment.
    pub location: EntityLocation,
    /// The selection tree tracking all selections for this entity across scopes.
    pub selection_tree: EntitySelectionTree,
}

impl Entity {
    /// Create a new entity with a selection tree.
    pub fn new(location: EntityLocation, root_type_path: Vec<String>) -> Self {
        Self {
            location,
            selection_tree: EntitySelectionTree::new(root_type_path),
        }
    }

    /// Create a simple entity without a selection tree (backward compat).
    pub fn simple(location: EntityLocation) -> Self {
        // Use a minimal root type path
        let type_name = location.field_path.last()
            .map(|c| c.type_name.clone())
            .unwrap_or_else(|| "Query".to_string());
        Self {
            location,
            selection_tree: EntitySelectionTree::new(vec![type_name]),
        }
    }
}

/// The location of an entity.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct EntityLocation {
    /// The source definition (operation or fragment).
    pub source_name: String,
    /// Field path from root to this entity.
    pub field_path: Vec<FieldPathComponent>,
}

impl EntityLocation {
    /// Append a field component to create a new location.
    pub fn appending(&self, component: FieldPathComponent) -> Self {
        let mut path = self.field_path.clone();
        path.push(component);
        Self {
            source_name: self.source_name.clone(),
            field_path: path,
        }
    }

    /// Append multiple field components.
    pub fn appending_path(&self, components: &[FieldPathComponent]) -> Self {
        let mut path = self.field_path.clone();
        path.extend(components.iter().cloned());
        Self {
            source_name: self.source_name.clone(),
            field_path: path,
        }
    }
}

/// A component in the field path to an entity.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct FieldPathComponent {
    pub name: String,
    pub type_name: String,
}
