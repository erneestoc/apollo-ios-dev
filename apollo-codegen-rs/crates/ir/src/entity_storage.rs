//! DefinitionEntityStorage - cache/factory for entities within an operation or fragment.
//!
//! Mirrors Swift's IR+DefinitionEntityStorage.swift.
//! Ensures that all SelectionSets referring to the same response object share the same Entity.

use crate::entity::{Entity, EntityLocation, FieldPathComponent};
use std::collections::HashMap;

/// Caches entities by their location within a definition (operation or fragment).
#[derive(Debug)]
pub struct DefinitionEntityStorage {
    /// The source definition name (operation or fragment name).
    pub source_name: String,
    /// Entities indexed by their location.
    pub entities: HashMap<EntityLocation, Entity>,
}

impl DefinitionEntityStorage {
    /// Create storage for a root entity.
    pub fn new(source_name: String, root_type_name: String) -> Self {
        let root_location = EntityLocation {
            source_name: source_name.clone(),
            field_path: vec![],
        };
        let root_entity = Entity::new(root_location.clone(), vec![root_type_name]);

        let mut entities = HashMap::new();
        entities.insert(root_location, root_entity);

        Self {
            source_name,
            entities,
        }
    }

    /// Get or create the root entity.
    pub fn root_entity(&self) -> &Entity {
        let root_location = EntityLocation {
            source_name: self.source_name.clone(),
            field_path: vec![],
        };
        self.entities.get(&root_location).expect("Root entity must exist")
    }

    /// Get a mutable reference to the root entity.
    pub fn root_entity_mut(&mut self) -> &mut Entity {
        let root_location = EntityLocation {
            source_name: self.source_name.clone(),
            field_path: vec![],
        };
        self.entities.get_mut(&root_location).expect("Root entity must exist")
    }

    /// Get or create an entity for a field on an enclosing entity.
    pub fn entity_for_field(
        &mut self,
        field_name: &str,
        field_type_name: &str,
        enclosing_entity_location: &EntityLocation,
        enclosing_root_type_path: &[String],
    ) -> &mut Entity {
        let location = enclosing_entity_location.appending(FieldPathComponent {
            name: field_name.to_string(),
            type_name: field_type_name.to_string(),
        });

        if !self.entities.contains_key(&location) {
            let mut root_type_path = enclosing_root_type_path.to_vec();
            root_type_path.push(field_type_name.to_string());
            let entity = Entity::new(location.clone(), root_type_path);
            self.entities.insert(location.clone(), entity);
        }

        self.entities.get_mut(&location).unwrap()
    }

    /// Get or create an entity mapped from a fragment's entity into this definition.
    pub fn entity_for_fragment_entity(
        &mut self,
        fragment_entity_location: &EntityLocation,
        spread_entity_location: &EntityLocation,
        spread_root_type_path: &[String],
        fragment_root_type_path: &[String],
    ) -> &mut Entity {
        // Build the location by appending fragment's field path to spread's entity location
        let location = if fragment_entity_location.field_path.is_empty() {
            spread_entity_location.clone()
        } else {
            spread_entity_location.appending_path(&fragment_entity_location.field_path)
        };

        if !self.entities.contains_key(&location) {
            // Build root type path: spread's path + fragment's remaining path
            let mut root_type_path = spread_root_type_path.to_vec();
            if fragment_root_type_path.len() > 1 {
                root_type_path.extend_from_slice(&fragment_root_type_path[1..]);
            }
            let entity = Entity::new(location.clone(), root_type_path);
            self.entities.insert(location.clone(), entity);
        }

        self.entities.get_mut(&location).unwrap()
    }
}
