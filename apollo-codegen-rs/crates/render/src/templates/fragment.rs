//! Fragment definition template.
//!
//! Generates files like:
//! ```swift
//! // @generated
//! // This file was automatically generated and should not be edited.
//!
//! @_exported import ApolloAPI
//!
//! public struct HeightInMeters: AnimalKingdomAPI.SelectionSet, Fragment {
//!   public static var fragmentDefinition: StaticString { ... }
//!   ...
//! }
//! ```

use super::header;
use super::selection_set::{self, SelectionSetConfig};

/// Configuration for rendering a fragment file.
#[derive(Debug)]
pub struct FragmentConfig<'a> {
    /// Fragment name (e.g. "HeightInMeters").
    pub name: &'a str,
    /// The fragment definition source string.
    pub fragment_definition: &'a str,
    /// Schema namespace (e.g. "AnimalKingdomAPI").
    pub schema_namespace: &'a str,
    /// Access modifier (e.g. "public ").
    pub access_modifier: &'a str,
    /// The root selection set config.
    pub selection_set: SelectionSetConfig<'a>,
    /// Whether this is a mutable fragment (for local cache mutations).
    pub is_mutable: bool,
    /// How to format the query string literal (default: SingleLine).
    pub query_string_format: super::operation::QueryStringFormat,
    /// The API target name for import statements (default: "ApolloAPI").
    pub api_target_name: &'a str,
    /// Whether to include the fragmentDefinition property (default: true).
    /// When operationDocumentFormat doesn't include "definition", this should be false.
    pub include_definition: bool,
}

/// Render a complete fragment file.
pub fn render(config: &FragmentConfig) -> String {
    let mut result = String::new();

    // Header
    result.push_str(header::HEADER);
    result.push_str("\n\n");
    result.push_str(&format!("@_exported import {}\n\n", config.api_target_name));

    // Render the selection set struct (the fragment itself)
    let body = render_fragment_body(config);
    result.push_str(&body);

    result
}

fn render_fragment_body(config: &FragmentConfig) -> String {
    let ss = &config.selection_set;
    let indent = " ".repeat(ss.indent);
    let inner_indent = format!("{}  ", indent);
    let mut result = String::new();

    // Struct declaration with Fragment conformance
    let conformance = if config.is_mutable {
        format!("{}.MutableSelectionSet, Fragment", config.schema_namespace)
    } else {
        format!("{}.SelectionSet, Fragment", config.schema_namespace)
    };
    result.push_str(&format!(
        "{}{}struct {}: {} {{\n",
        indent, config.access_modifier, config.name, conformance
    ));

    // fragmentDefinition (only when definition is included in operationDocumentFormat)
    if config.include_definition {
        result.push_str(&format!(
            "{}{}static var fragmentDefinition: StaticString {{\n",
            inner_indent, config.access_modifier
        ));
        // Fragment definitions always use single-line format (matching Swift CLI behavior
        // which ignores queryStringLiteralFormat for fragmentDefinition)
        result.push_str(&format!(
            "{}  #\"{}\"#\n",
            inner_indent, config.fragment_definition
        ));
        result.push_str(&format!("{}}}\n", inner_indent));
        result.push('\n');
    }

    // __data and init
    let data_keyword = if config.is_mutable { "var" } else { "let" };
    result.push_str(&format!(
        "{}{}{} __data: DataDict\n",
        inner_indent, config.access_modifier, data_keyword
    ));
    result.push_str(&format!(
        "{}{}init(_dataDict: DataDict) {{ __data = _dataDict }}\n",
        inner_indent, config.access_modifier
    ));

    // __parentType
    result.push('\n');
    result.push_str(&format!(
        "{}{}static var __parentType: any {}.ParentType {{ {} }}\n",
        inner_indent,
        config.access_modifier,
        config.api_target_name,
        ss.parent_type.render(config.schema_namespace)
    ));

    // __mergedSources (for CompositeInlineFragment)
    if !ss.merged_sources.is_empty() {
        result.push_str(&format!(
            "{}{}static var __mergedSources: [any {}.SelectionSet.Type] {{ [\n",
            inner_indent, config.access_modifier, config.api_target_name
        ));
        for (i, source) in ss.merged_sources.iter().enumerate() {
            let comma = if i < ss.merged_sources.len() - 1 { "," } else { "" };
            result.push_str(&format!(
                "{}  {}.self{}\n",
                inner_indent, source, comma
            ));
        }
        result.push_str(&format!("{}] }}\n", inner_indent));
    }

    // __selections
    if !ss.selections.is_empty() {
        result.push_str(&render_selections(&ss.selections, &inner_indent, config.access_modifier, config.api_target_name));
    }

    // Field accessors
    if !ss.field_accessors.is_empty() {
        result.push('\n');
        for accessor in &ss.field_accessors {
            // Documentation comment
            if let Some(desc) = accessor.description {
                if !desc.is_empty() {
                    for line in desc.lines() {
                        if line.is_empty() {
                            result.push_str(&format!("{}///\n", inner_indent));
                        } else {
                            result.push_str(&format!("{}/// {}\n", inner_indent, line));
                        }
                    }
                }
            }
            if config.is_mutable {
                result.push_str(&format!(
                    "{}{}var {}: {} {{\n",
                    inner_indent,
                    config.access_modifier,
                    crate::naming::escape_swift_name(accessor.name),
                    accessor.swift_type,
                ));
                result.push_str(&format!(
                    "{}  get {{ __data[\"{}\"] }}\n",
                    inner_indent, accessor.name
                ));
                result.push_str(&format!(
                    "{}  set {{ __data[\"{}\"] = newValue }}\n",
                    inner_indent, accessor.name
                ));
                result.push_str(&format!("{}}}\n", inner_indent));
            } else {
                result.push_str(&format!(
                    "{}{}var {}: {} {{ __data[\"{}\"] }}\n",
                    inner_indent,
                    config.access_modifier,
                    crate::naming::escape_swift_name(accessor.name),
                    accessor.swift_type,
                    accessor.name
                ));
            }
        }
    }

    // Inline fragment accessors
    if !ss.inline_fragment_accessors.is_empty() {
        result.push('\n');
        for accessor in &ss.inline_fragment_accessors {
            if config.is_mutable {
                result.push_str(&format!(
                    "{}{}var {}: {}? {{\n",
                    inner_indent,
                    config.access_modifier,
                    accessor.property_name,
                    accessor.type_name
                ));
                result.push_str(&format!(
                    "{}  get {{ _asInlineFragment() }}\n",
                    inner_indent
                ));
                result.push_str(&format!(
                    "{}  set {{ if let newData = newValue?.__data._data {{ __data._data = newData }}}}\n",
                    inner_indent
                ));
                result.push_str(&format!("{}}}\n", inner_indent));
            } else {
                result.push_str(&format!(
                    "{}{}var {}: {}? {{ _asInlineFragment() }}\n",
                    inner_indent,
                    config.access_modifier,
                    accessor.property_name,
                    accessor.type_name
                ));
            }
        }
    }

    // Fragments container
    if !ss.fragment_spreads.is_empty() {
        result.push('\n');
        result.push_str(&format!(
            "{}{}struct Fragments: FragmentContainer {{\n",
            inner_indent, config.access_modifier
        ));
        let frag_inner = format!("{}  ", inner_indent);
        result.push_str(&format!(
            "{}{}let __data: DataDict\n",
            frag_inner, config.access_modifier
        ));
        result.push_str(&format!(
            "{}{}init(_dataDict: DataDict) {{ __data = _dataDict }}\n",
            frag_inner, config.access_modifier
        ));
        result.push('\n');
        for spread in &ss.fragment_spreads {
            result.push_str(&format!(
                "{}{}var {}: {} {{ _toFragment() }}\n",
                frag_inner,
                config.access_modifier,
                spread.property_name,
                spread.fragment_type
            ));
        }
        result.push_str(&format!("{}}}\n", inner_indent));
    }

    // Initializer
    if let Some(init_config) = &ss.initializer {
        result.push('\n');
        render_initializer(&mut result, init_config, &inner_indent, config.access_modifier);
    }

    // Nested selection set structs
    for nested in &ss.nested_types {
        result.push('\n');
        result.push_str(&format!(
            "{}{}\n",
            inner_indent, nested.doc_comment
        ));
        result.push_str(&format!(
            "{}{}\n",
            inner_indent, nested.parent_type_comment
        ));
        result.push_str(&selection_set::render(&nested.config));
    }

    // Type aliases
    for alias in &ss.type_aliases {
        result.push('\n');
        result.push_str(&format!(
            "{}{}typealias {} = {}\n",
            inner_indent, config.access_modifier, alias.name, alias.target
        ));
    }

    // Close struct
    result.push_str(&format!("{}}}\n", indent));

    result
}

fn render_selections(
    selections: &[selection_set::SelectionItem],
    indent: &str,
    access_modifier: &str,
    api_target_name: &str,
) -> String {
    let item_indent = format!("{}  ", indent);
    let mut result = String::new();
    result.push_str(&format!(
        "{}{}static var __selections: [{}.Selection] {{ [\n",
        indent, access_modifier, api_target_name
    ));
    for sel in selections.iter() {
        // All items get trailing comma (Swift trailing comma convention)
        match sel {
            selection_set::SelectionItem::Field(f) => {
                if let Some(args) = f.arguments {
                    result.push_str(&format!(
                        "{}.field(\"{}\", {}.self, arguments: {}),\n",
                        item_indent, f.name, f.swift_type, args
                    ));
                } else {
                    result.push_str(&format!(
                        "{}.field(\"{}\", {}.self),\n",
                        item_indent, f.name, f.swift_type
                    ));
                }
            }
            selection_set::SelectionItem::InlineFragment(name) => {
                result.push_str(&format!(
                    "{}.inlineFragment({}.self),\n",
                    item_indent, name
                ));
            }
            selection_set::SelectionItem::Fragment(name) => {
                result.push_str(&format!(
                    "{}.fragment({}.self),\n",
                    item_indent, name
                ));
            }
            selection_set::SelectionItem::ConditionalField(cond, f) => {
                let cond_str = selection_set::render_inclusion_condition(cond);
                if let Some(args) = f.arguments {
                    result.push_str(&format!(
                        "{}.include(if: {}, .field(\"{}\", {}.self, arguments: {})),\n",
                        item_indent, cond_str, f.name, f.swift_type, args
                    ));
                } else {
                    result.push_str(&format!(
                        "{}.include(if: {}, .field(\"{}\", {}.self)),\n",
                        item_indent, cond_str, f.name, f.swift_type
                    ));
                }
            }
            selection_set::SelectionItem::ConditionalInlineFragment(cond, name) => {
                let cond_str = selection_set::render_inclusion_condition(cond);
                result.push_str(&format!(
                    "{}.include(if: {}, .inlineFragment({}.self)),\n",
                    item_indent, cond_str, name
                ));
            }
            selection_set::SelectionItem::ConditionalFieldGroup(cond, fields) => {
                let cond_str = selection_set::render_inclusion_condition(cond);
                result.push_str(&format!(
                    "{}.include(if: {}, [\n",
                    item_indent, cond_str
                ));
                let group_indent = format!("{}  ", item_indent);
                for f in fields {
                    if let Some(args) = f.arguments {
                        result.push_str(&format!(
                            "{}.field(\"{}\", {}.self, arguments: {}),\n",
                            group_indent, f.name, f.swift_type, args
                        ));
                    } else {
                        result.push_str(&format!(
                            "{}.field(\"{}\", {}.self),\n",
                            group_indent, f.name, f.swift_type
                        ));
                    }
                }
                result.push_str(&format!("{}]),\n", item_indent));
            }
        }
    }
    result.push_str(&format!("{}] }}\n", indent));
    result
}

fn render_initializer(
    result: &mut String,
    config: &selection_set::InitializerConfig,
    indent: &str,
    access_modifier: &str,
) {
    let inner = format!("{}  ", indent);
    let inner2 = format!("{}  ", inner);
    let inner3 = format!("{}  ", inner2);

    // Opening line
    result.push_str(&format!("{}{}init(\n", indent, access_modifier));

    // Parameters
    for (i, param) in config.parameters.iter().enumerate() {
        let comma = if i < config.parameters.len() - 1 { "," } else { "" };
        if let Some(default) = param.default_value {
            result.push_str(&format!(
                "{}{}: {} = {}{}\n",
                inner, param.name, param.swift_type, default, comma
            ));
        } else {
            result.push_str(&format!(
                "{}{}: {}{}\n",
                inner, param.name, param.swift_type, comma
            ));
        }
    }

    result.push_str(&format!("{}) {{\n", indent));
    result.push_str(&format!("{}self.init(_dataDict: DataDict(\n", inner));
    result.push_str(&format!("{}data: [\n", inner2));

    // Data entries - all get trailing comma
    for entry in config.data_entries.iter() {
        match &entry.value {
            selection_set::DataEntryValue::Variable(var_name) => {
                result.push_str(&format!(
                    "{}\"{}\": {},\n",
                    inner3, entry.key, var_name
                ));
            }
            selection_set::DataEntryValue::FieldData(var_name) => {
                result.push_str(&format!(
                    "{}\"{}\": {}._fieldData,\n",
                    inner3, entry.key, var_name
                ));
            }
            selection_set::DataEntryValue::Typename(type_ref) => {
                result.push_str(&format!(
                    "{}\"{}\": {},\n",
                    inner3, entry.key, type_ref
                ));
            }
        }
    }

    result.push_str(&format!("{}],\n", inner2));
    result.push_str(&format!("{}fulfilledFragments: [\n", inner2));

    // fulfilledFragments - NO trailing comma on last
    for (i, frag) in config.fulfilled_fragments.iter().enumerate() {
        let comma = if i < config.fulfilled_fragments.len() - 1 { "," } else { "" };
        result.push_str(&format!(
            "{}ObjectIdentifier({}.self){}\n",
            inner3, frag, comma
        ));
    }

    result.push_str(&format!("{}]\n", inner2));
    result.push_str(&format!("{}))\n", inner));
    result.push_str(&format!("{}}}\n", indent));
}
