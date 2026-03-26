//! IR NamedFragment - represents a compiled named GraphQL fragment.

use crate::fields::EntityField;
use std::sync::Arc;

/// A compiled named GraphQL fragment.
#[derive(Debug)]
pub struct NamedFragment {
    /// Fragment name.
    pub name: String,
    /// Type condition name.
    pub type_condition_name: String,
    /// Root field (selection set on the fragment's type condition).
    pub root_field: EntityField,
    /// Other fragments referenced by this fragment.
    pub referenced_fragments: Vec<Arc<NamedFragment>>,
    /// Whether this is a local cache mutation.
    pub is_local_cache_mutation: bool,
    /// The original GraphQL source text.
    pub source: String,
    /// The file path where this fragment is defined.
    pub file_path: String,
    /// Whether this fragment contains deferred fragments.
    pub contains_deferred_fragment: bool,
}
