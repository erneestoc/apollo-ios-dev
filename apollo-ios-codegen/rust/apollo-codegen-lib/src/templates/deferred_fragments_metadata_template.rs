//! Deferred fragments metadata template for code generation.
//!
//! Renders metadata for deferred fragments in an operation, including
//! DeferredFragmentIdentifiers and the responseFormat property.
//!
//! NOT a TemplateRenderer -- called internally by OperationDefinitionTemplate.
//!
//! Mirrors Swift's `DeferredFragmentsMetadataTemplate.swift` from
//! `Sources/ApolloCodegenLib/Templates/DeferredFragmentsMetadataTemplate.swift`.

use std::collections::HashSet;

use graphql_compiler::DeferCondition;

use crate::templates::rendering_helpers::selection_set_name_generator::SelectionSetNameGenerator;
use crate::templates::rendering_helpers::string_swift_name_escaping::as_fragment_name;
use crate::templates::rendering_helpers::string_casing::first_uppercased;
use crate::templates::ConfigurationContext;

use ir::direct_selections::DirectSelections;
use ir::fields::Field;

/// Template for rendering deferred fragment metadata for an operation.
///
/// This is not a TemplateRenderer; it is called by OperationDefinitionTemplate
/// when the operation contains deferred fragments.
pub struct DeferredFragmentsMetadataTemplate<'a> {
    pub operation: &'a ir::Operation,
    pub config: &'a ConfigurationContext,
    pub render_access_control: String,
}

/// Internal struct representing a deferred fragment path with type information.
#[derive(Debug)]
struct DeferredPathTypeInfo {
    path: Vec<String>,
    defer_condition: DeferCondition,
    type_name: String,
}

impl DeferredPathTypeInfo {
    /// Returns a hash value combining path and defer_condition (not type_name).
    /// Used for deduplication of DeferredFragmentIdentifiers.
    fn path_defer_condition_key(&self) -> (Vec<String>, String) {
        (self.path.clone(), self.defer_condition.label.clone())
    }
}

impl<'a> DeferredFragmentsMetadataTemplate<'a> {
    /// Renders the deferred fragments metadata section as an extension.
    ///
    /// Returns an empty string if there are no deferred fragments.
    /// In 1.15.1, renders as `extension OperationName { ... }` with
    /// DeferredFragmentIdentifiers enum and deferredFragments property.
    pub fn render(&self) -> String {
        let path_type_info = self.collect_deferred_paths(
            self.operation.root_field.selection_set.selections.as_deref(),
            &[],
        );

        if path_type_info.is_empty() {
            return String::new();
        }

        let definition_name = crate::templates::rendering_helpers::ir_definition_rendering::generated_definition_name(
            &self.operation.definition.name,
            &self.operation.definition.operation_type.to_string(),
            self.operation.definition.is_local_cache_mutation(),
        );

        let mut inner = String::new();

        // DeferredFragmentIdentifiers enum
        inner.push_str(&self.render_deferred_fragment_identifiers(&path_type_info));
        inner.push('\n');

        // deferredFragments property
        inner.push_str(&self.render_deferred_fragments_property(&path_type_info));

        // Indent inner content
        let indented: Vec<String> = inner
            .lines()
            .map(|line| {
                if line.trim().is_empty() {
                    String::new()
                } else {
                    format!("  {}", line)
                }
            })
            .collect();

        format!(
            "\n// MARK: Deferred Fragment Metadata\n\n{}extension {} {{\n{}\n}}",
            self.render_access_control,
            definition_name,
            indented.join("\n"),
        )
    }

    /// Renders the DeferredFragmentIdentifiers enum.
    fn render_deferred_fragment_identifiers(&self, infos: &[DeferredPathTypeInfo]) -> String {
        let mut result = String::new();
        result.push_str("enum DeferredFragmentIdentifiers {\n");

        // Deduplicate by (path, label) key
        let mut seen: HashSet<(Vec<String>, String)> = HashSet::new();
        for info in infos {
            let key = info.path_defer_condition_key();
            if !seen.insert(key) {
                continue;
            }

            let path_elements: Vec<String> =
                info.path.iter().map(|p| format!("\"{}\"", p)).collect();

            result.push_str(&format!(
                "  static let {} = DeferredFragmentIdentifier(label: \"{}\", fieldPath: [{}])\n",
                info.defer_condition.label,
                info.defer_condition.label,
                path_elements.join(", "),
            ));
        }

        result.push_str("}\n");
        result
    }

    /// Renders the deferredFragments property.
    ///
    /// In 1.15.1: `static var deferredFragments: [DeferredFragmentIdentifier: any ApolloAPI.SelectionSet.Type]? {[...]}`
    fn render_deferred_fragments_property(&self, infos: &[DeferredPathTypeInfo]) -> String {
        let mut result = String::new();
        result.push_str(
            "static var deferredFragments: [DeferredFragmentIdentifier: any ApolloAPI.SelectionSet.Type]? {[\n",
        );

        for info in infos {
            result.push_str(&format!(
                "  DeferredFragmentIdentifiers.{}: {}.self,\n",
                info.defer_condition.label, info.type_name,
            ));
        }

        result.push_str("]}\n");
        result
    }

    /// Recursively collects deferred paths from direct selections.
    ///
    /// Mirrors Swift's `DeferredFragmentsPathTypeInfo(from:path:)`.
    fn collect_deferred_paths(
        &self,
        direct_selections: Option<&DirectSelections>,
        path: &[String],
    ) -> Vec<DeferredPathTypeInfo> {
        let direct_selections = match direct_selections {
            Some(ds) if !ds.is_empty() => ds,
            _ => return Vec::new(),
        };

        let mut infos: Vec<DeferredPathTypeInfo> = Vec::new();

        // Process entity fields -- recurse into their selection sets
        for field in direct_selections.fields.values() {
            if let Field::Entity(entity_field) = field {
                let field_name = entity_field
                    .underlying_field
                    .alias
                    .as_deref()
                    .unwrap_or(&entity_field.underlying_field.name);
                let mut field_path = path.to_vec();
                field_path.push(field_name.to_string());
                infos.extend(self.collect_deferred_paths(
                    entity_field.selection_set.selections.as_deref(),
                    &field_path,
                ));
            }
        }

        // Process inline fragments
        for fragment in direct_selections.inline_fragments.values() {
            if let Some(defer_condition) = fragment.type_info().defer_condition() {
                let selection_set_name = SelectionSetNameGenerator::generated_selection_set_name(
                    fragment.type_info(),
                    None,
                    crate::templates::rendering_helpers::selection_set_name_generator::NameFormat::OmittingRoot,
                    &self.config.pluralizer,
                );

                infos.push(DeferredPathTypeInfo {
                    path: path.to_vec(),
                    defer_condition: defer_condition.clone(),
                    type_name: format!("Data.{}", selection_set_name),
                });
            }

            infos.extend(self.collect_deferred_paths(
                fragment.selection_set.selections.as_deref(),
                path,
            ));
        }

        // Process named fragments
        for fragment in direct_selections.named_fragments.values() {
            if let Some(defer_condition) = fragment.type_info.defer_condition() {
                let frag_name = as_fragment_name(&first_uppercased(&fragment.fragment.definition.name));
                infos.push(DeferredPathTypeInfo {
                    path: path.to_vec(),
                    defer_condition: defer_condition.clone(),
                    type_name: frag_name,
                });
            }

            infos.extend(self.collect_deferred_paths(
                fragment.fragment.root_field.selection_set.selections.as_deref(),
                path,
            ));
        }

        infos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deferred_path_type_info_key() {
        let info = DeferredPathTypeInfo {
            path: vec!["query".to_string(), "animal".to_string()],
            defer_condition: DeferCondition {
                label: "deferredLabel".to_string(),
                variable: None,
            },
            type_name: "Data.AsAnimal".to_string(),
        };
        let key = info.path_defer_condition_key();
        assert_eq!(key.0, vec!["query".to_string(), "animal".to_string()]);
        assert_eq!(key.1, "deferredLabel");
    }
}
