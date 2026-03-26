//! Entity - represents a concrete entity in an operation/fragment.

/// An entity in the selection tree.
#[derive(Debug)]
pub struct Entity {
    /// Where this entity is located in the operation/fragment.
    pub location: EntityLocation,
}

/// The location of an entity.
#[derive(Debug, Clone)]
pub struct EntityLocation {
    /// The source definition (operation or fragment).
    pub source_name: String,
    /// Field path from root to this entity.
    pub field_path: Vec<FieldPathComponent>,
}

/// A component in the field path to an entity.
#[derive(Debug, Clone)]
pub struct FieldPathComponent {
    pub name: String,
    pub type_name: String,
}
