//! Scope descriptors - define the type scope for a selection set.
//!
//! Mirrors Swift's IR+ScopeDescriptor.swift.

use crate::inclusion::InclusionConditions;
use crate::merged_selections::{ConditionKey, ScopeConditionKey};
use crate::entity_selection_tree::ScopeDescriptorRef;
use apollo_codegen_frontend::types::GraphQLCompositeType;

/// Describes the scope at which a selection set exists.
#[derive(Debug, Clone)]
pub struct ScopeDescriptor {
    /// The parent type of this scope.
    pub parent_type: GraphQLCompositeType,
    /// Inclusion conditions that must be met for this scope.
    pub inclusion_conditions: Option<InclusionConditions>,
    /// The scope path within this entity (list of scope conditions from root to here).
    pub entity_scope_path: Vec<ScopeConditionKey>,
    /// All types that this scope matches (includes interfaces, unions).
    pub matching_types: Vec<String>,
}

/// A condition that defines a new scope.
#[derive(Debug, Clone)]
pub struct ScopeCondition {
    pub type_condition: Option<GraphQLCompositeType>,
    pub inclusion_conditions: Option<InclusionConditions>,
}

impl ScopeDescriptor {
    pub fn new(parent_type: GraphQLCompositeType) -> Self {
        let type_name = parent_type.name().to_string();
        let matching_types = compute_matching_types(&parent_type, &[]);
        let scope_key = ScopeConditionKey::new(Some(type_name));
        Self {
            parent_type,
            inclusion_conditions: None,
            entity_scope_path: vec![scope_key],
            matching_types,
        }
    }

    /// Create a root scope descriptor with full type matching support.
    pub fn new_with_schema(
        parent_type: GraphQLCompositeType,
        all_objects: &[apollo_codegen_frontend::types::GraphQLObjectType],
        all_interfaces: &[apollo_codegen_frontend::types::GraphQLInterfaceType],
        all_unions: &[apollo_codegen_frontend::types::GraphQLUnionType],
    ) -> Self {
        let type_name = parent_type.name().to_string();
        let matching_types = compute_full_matching_types(
            &parent_type,
            all_objects,
            all_interfaces,
            all_unions,
        );
        let scope_key = ScopeConditionKey::new(Some(type_name));
        Self {
            parent_type,
            inclusion_conditions: None,
            entity_scope_path: vec![scope_key],
            matching_types,
        }
    }

    /// Create a child scope by appending a scope condition.
    pub fn appending(
        &self,
        condition: &ScopeConditionKey,
        new_type: Option<&GraphQLCompositeType>,
        all_objects: &[apollo_codegen_frontend::types::GraphQLObjectType],
        all_interfaces: &[apollo_codegen_frontend::types::GraphQLInterfaceType],
        all_unions: &[apollo_codegen_frontend::types::GraphQLUnionType],
    ) -> Self {
        let mut scope_path = self.entity_scope_path.clone();
        scope_path.push(condition.clone());

        let (parent_type, matching_types) = if let Some(new_type) = new_type {
            let types = compute_full_matching_types(
                new_type,
                all_objects,
                all_interfaces,
                all_unions,
            );
            // Merge with parent's matching types
            let mut merged = self.matching_types.clone();
            for t in &types {
                if !merged.contains(t) {
                    merged.push(t.clone());
                }
            }
            (new_type.clone(), merged)
        } else {
            (self.parent_type.clone(), self.matching_types.clone())
        };

        let mut inclusion_conditions = self.inclusion_conditions.clone();
        if let Some(ref conds) = condition.conditions {
            if !conds.is_empty() {
                // Merge conditions
                let new_conds: Vec<crate::inclusion::InclusionCondition> = conds
                    .iter()
                    .map(|c| crate::inclusion::InclusionCondition {
                        variable: c.variable.clone(),
                        is_inverted: c.is_inverted,
                    })
                    .collect();
                if let Some(ref mut existing) = inclusion_conditions {
                    existing.conditions.extend(new_conds);
                } else {
                    inclusion_conditions = Some(InclusionConditions::from_conditions(new_conds));
                }
            }
        }

        Self {
            parent_type,
            inclusion_conditions,
            entity_scope_path: scope_path,
            matching_types,
        }
    }

    pub fn type_name(&self) -> &str {
        self.parent_type.name()
    }

    /// Check if this scope matches a type name.
    pub fn matches_type(&self, type_name: &str) -> bool {
        self.matching_types.contains(&type_name.to_string())
    }

    /// Check if this scope matches a scope condition.
    pub fn matches_condition(&self, condition: &ScopeConditionKey) -> bool {
        if let Some(ref type_name) = condition.type_name {
            if !self.matches_type(type_name) {
                return false;
            }
        }
        // TODO: proper inclusion condition matching
        true
    }

    /// Convert to a lightweight reference for tree operations.
    pub fn to_ref(&self) -> ScopeDescriptorRef {
        ScopeDescriptorRef {
            type_name: self.parent_type.name().to_string(),
            scope_path: self.entity_scope_path.clone(),
            matching_types: self.matching_types.clone(),
        }
    }

    /// Create the ScopeConditionKey for this scope's last condition.
    pub fn to_scope_condition_key(&self) -> ScopeConditionKey {
        self.entity_scope_path.last().cloned().unwrap_or_else(|| {
            ScopeConditionKey::new(Some(self.parent_type.name().to_string()))
        })
    }
}

/// Compute the set of type names that a composite type matches.
/// This is a simple version that just includes the type and its interfaces.
fn compute_matching_types(
    ty: &GraphQLCompositeType,
    _existing: &[String],
) -> Vec<String> {
    let mut types = vec![ty.name().to_string()];
    match ty {
        GraphQLCompositeType::Object(obj) => {
            for iface in &obj.interfaces {
                if !types.contains(iface) {
                    types.push(iface.clone());
                }
            }
        }
        GraphQLCompositeType::Interface(iface) => {
            for parent_iface in &iface.interfaces {
                if !types.contains(parent_iface) {
                    types.push(parent_iface.clone());
                }
            }
        }
        GraphQLCompositeType::Union(_) => {}
    }
    types
}

/// Compute full matching types including unions.
fn compute_full_matching_types(
    ty: &GraphQLCompositeType,
    all_objects: &[apollo_codegen_frontend::types::GraphQLObjectType],
    _all_interfaces: &[apollo_codegen_frontend::types::GraphQLInterfaceType],
    all_unions: &[apollo_codegen_frontend::types::GraphQLUnionType],
) -> Vec<String> {
    let mut types = vec![ty.name().to_string()];

    match ty {
        GraphQLCompositeType::Object(obj) => {
            // Add all interfaces this object implements
            for iface in &obj.interfaces {
                if !types.contains(iface) {
                    types.push(iface.clone());
                }
            }
            // Add all unions that contain this object
            for union_t in all_unions {
                if union_t.member_types.contains(&obj.name) && !types.contains(&union_t.name) {
                    types.push(union_t.name.clone());
                }
            }
        }
        GraphQLCompositeType::Interface(iface) => {
            // Add parent interfaces
            for parent_iface in &iface.interfaces {
                if !types.contains(parent_iface) {
                    types.push(parent_iface.clone());
                }
            }
            // Add objects implementing this interface (for matching)
            for obj in all_objects {
                if obj.interfaces.contains(&iface.name) && !types.contains(&obj.name) {
                    // Don't add object types to matching - interface just needs to match itself
                }
            }
        }
        GraphQLCompositeType::Union(union_t) => {
            // Union matches itself and all its member types
            for member in &union_t.member_types {
                if !types.contains(member) {
                    types.push(member.clone());
                }
            }
        }
    }

    types
}

/// Create inclusion condition keys from IR InclusionConditions.
pub fn conditions_to_keys(conditions: &Option<InclusionConditions>) -> Option<Vec<ConditionKey>> {
    conditions.as_ref().and_then(|c| {
        if c.is_empty() {
            None
        } else {
            Some(
                c.conditions
                    .iter()
                    .map(|ic| ConditionKey {
                        variable: ic.variable.clone(),
                        is_inverted: ic.is_inverted,
                    })
                    .collect(),
            )
        }
    })
}
