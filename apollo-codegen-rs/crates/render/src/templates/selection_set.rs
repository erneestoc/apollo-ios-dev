//! Selection set template.
//!
//! Generates the Swift `SelectionSet` struct with fields, inline fragments,
//! fragment containers, and initializers. Used by both operation and fragment templates.

use crate::naming;

/// Configuration for rendering a selection set.
#[derive(Debug)]
pub struct SelectionSetConfig<'a> {
    /// The struct name (e.g. "Data", "AllAnimal", "AsDog").
    pub struct_name: &'a str,
    /// The schema namespace (e.g. "AnimalKingdomAPI").
    pub schema_namespace: &'a str,
    /// Parent type info for `__parentType`.
    pub parent_type: ParentTypeRef<'a>,
    /// Whether this is a root selection set (Data struct of operation, or fragment root).
    pub is_root: bool,
    /// Whether this is an inline fragment (has RootEntityType typealias).
    pub is_inline_fragment: bool,
    /// Protocol conformances: "SelectionSet", "InlineFragment", etc.
    pub conformance: SelectionSetConformance<'a>,
    /// The RootEntityType path for inline fragments (e.g. "DogQuery.Data.AllAnimal").
    pub root_entity_type: Option<&'a str>,
    /// Merged sources for CompositeInlineFragment types.
    pub merged_sources: Vec<&'a str>,
    /// Direct field selections (the __selections array).
    pub selections: Vec<SelectionItem<'a>>,
    /// Field accessor definitions.
    pub field_accessors: Vec<FieldAccessor<'a>>,
    /// Inline fragment accessors (e.g. "asDog: AsDog?").
    pub inline_fragment_accessors: Vec<InlineFragmentAccessor<'a>>,
    /// Named fragment spreads for the Fragments container.
    pub fragment_spreads: Vec<FragmentSpreadAccessor<'a>>,
    /// Initializer fields.
    pub initializer: Option<InitializerConfig<'a>>,
    /// Nested selection set structs.
    pub nested_types: Vec<NestedSelectionSet<'a>>,
    /// Type aliases (e.g. "public typealias Height = HeightInMeters.Height").
    pub type_aliases: Vec<TypeAliasConfig<'a>>,
    /// Index into nested_types where type aliases should be rendered.
    /// Type aliases are rendered after nested_types[0..index] and before nested_types[index..].
    pub type_alias_insert_index: usize,
    /// Indentation level (number of spaces).
    pub indent: usize,
    /// The access modifier (e.g. "public ").
    pub access_modifier: &'a str,
    /// Whether this is a mutable selection set (for local cache mutations).
    /// When true, uses `var __data` instead of `let __data`, get/set field accessors,
    /// and MutableSelectionSet/MutableInlineFragment conformances.
    pub is_mutable: bool,
    /// The API target name for fully-qualified type references (default: "ApolloAPI").
    pub api_target_name: &'a str,
    /// Deprecated argument warnings: (field_name, arg_name, reason).
    pub deprecated_arg_warnings: Vec<(&'a str, &'a str, &'a str)>,
}

/// Parent type reference, mapping to Objects, Interfaces, or Unions namespace.
#[derive(Debug, Clone)]
pub enum ParentTypeRef<'a> {
    Object(&'a str),
    Interface(&'a str),
    Union(&'a str),
}

impl<'a> ParentTypeRef<'a> {
    pub fn render(&self, schema_namespace: &str) -> String {
        match self {
            ParentTypeRef::Object(name) => {
                format!("{}.Objects.{}", schema_namespace, naming::first_uppercased(name))
            }
            ParentTypeRef::Interface(name) => {
                format!("{}.Interfaces.{}", schema_namespace, naming::first_uppercased(name))
            }
            ParentTypeRef::Union(name) => {
                format!("{}.Unions.{}", schema_namespace, naming::first_uppercased(name))
            }
        }
    }
}

/// Protocol conformances for the selection set struct.
#[derive(Debug, Clone)]
pub enum SelectionSetConformance<'a> {
    /// Regular selection set: `SchemaNamespace.SelectionSet`
    SelectionSet,
    /// Fragment: `SchemaNamespace.SelectionSet, Fragment`
    Fragment,
    /// Inline fragment: `SchemaNamespace.InlineFragment`
    InlineFragment,
    /// Composite inline fragment: `SchemaNamespace.InlineFragment, ApolloAPI.CompositeInlineFragment`
    CompositeInlineFragment,
    /// Mutable selection set (local cache mutation): `SchemaNamespace.MutableSelectionSet`
    MutableSelectionSet,
    /// Mutable fragment: `SchemaNamespace.MutableSelectionSet, Fragment`
    MutableFragment,
    /// Mutable inline fragment (local cache mutation): `SchemaNamespace.MutableInlineFragment`
    MutableInlineFragment,
    /// Custom conformance string.
    Custom(&'a str),
}

/// An item in the __selections array.
#[derive(Debug, Clone)]
pub enum SelectionItem<'a> {
    /// `.field("name", Type.self)` or `.field("name", Type.self, arguments: [...])`
    Field(FieldSelectionItem<'a>),
    /// `.inlineFragment(AsTypeName.self)`
    InlineFragment(&'a str),
    /// `.fragment(FragmentName.self)`
    Fragment(&'a str),
    /// `.include(if: "var", .field(...))` or `.include(if: !"var", .field(...))`
    ConditionalField(InclusionConditionRef<'a>, FieldSelectionItem<'a>),
    /// `.include(if: "var", .inlineFragment(...))` or `.include(if: !"var", .inlineFragment(...))`
    ConditionalInlineFragment(InclusionConditionRef<'a>, &'a str),
    /// `.include(if: "var", [.field(...), .field(...)])` - grouped conditional fields
    ConditionalFieldGroup(InclusionConditionRef<'a>, Vec<FieldSelectionItem<'a>>),
}

/// How multiple inclusion conditions are combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionOperator {
    And,
    Or,
}

/// A single condition entry for rendering.
#[derive(Debug, Clone)]
pub struct ConditionEntry<'a> {
    pub variable: &'a str,
    pub is_inverted: bool,
}

/// A reference to an inclusion condition for rendering.
/// Supports compound conditions (e.g., `!"skipName" && "includeName"`).
#[derive(Debug, Clone)]
pub struct InclusionConditionRef<'a> {
    pub conditions: Vec<ConditionEntry<'a>>,
    pub operator: ConditionOperator,
}

/// A field in the __selections array.
#[derive(Debug, Clone)]
pub struct FieldSelectionItem<'a> {
    pub name: &'a str,
    pub alias: Option<&'a str>,
    pub swift_type: &'a str,
    pub arguments: Option<&'a str>,
}

/// A field accessor property.
#[derive(Debug, Clone)]
pub struct FieldAccessor<'a> {
    pub name: &'a str,
    pub swift_type: &'a str,
    pub description: Option<&'a str>,
}

/// An inline fragment accessor (asFoo pattern).
#[derive(Debug, Clone)]
pub struct InlineFragmentAccessor<'a> {
    pub property_name: &'a str,
    pub type_name: &'a str,
}

/// A fragment spread accessor in the Fragments container.
#[derive(Debug, Clone)]
pub struct FragmentSpreadAccessor<'a> {
    pub property_name: &'a str,
    pub fragment_type: &'a str,
    /// Whether this fragment accessor is optional (e.g., due to @skip/@include conditions).
    pub is_optional: bool,
}

/// Configuration for the memberwise initializer.
#[derive(Debug, Clone)]
pub struct InitializerConfig<'a> {
    /// Parameters for the init.
    pub parameters: Vec<InitParam<'a>>,
    /// Data dict entries in the init body.
    pub data_entries: Vec<DataEntry<'a>>,
    /// fulfilledFragments entries.
    pub fulfilled_fragments: Vec<&'a str>,
    /// The typename value for __typename in data dict.
    pub typename_value: TypenameValue<'a>,
}

/// A parameter for the initializer.
#[derive(Debug, Clone)]
pub struct InitParam<'a> {
    pub name: &'a str,
    pub swift_type: &'a str,
    pub default_value: Option<&'a str>,
}

/// An entry in the data dict.
#[derive(Debug, Clone)]
pub struct DataEntry<'a> {
    pub key: &'a str,
    pub value: DataEntryValue<'a>,
}

/// The value for a data dict entry.
#[derive(Debug, Clone)]
pub enum DataEntryValue<'a> {
    /// A plain variable reference: `key`
    Variable(&'a str),
    /// An entity field: `key._fieldData`
    FieldData(&'a str),
    /// A fixed typename: `SchemaNamespace.Objects.TypeName.typename`
    Typename(&'a str),
}

/// Typename value in the initializer.
#[derive(Debug, Clone)]
pub enum TypenameValue<'a> {
    /// A parameter (__typename passed in).
    Parameter,
    /// A fixed typename object reference (e.g. "AnimalKingdomAPI.Objects.Dog.typename").
    Fixed(&'a str),
}

/// A type alias declaration.
#[derive(Debug, Clone)]
pub struct TypeAliasConfig<'a> {
    pub name: &'a str,
    pub target: &'a str,
}

/// A nested selection set to render inside the parent.
#[derive(Debug)]
pub struct NestedSelectionSet<'a> {
    /// Doc comment (e.g. "/// Height")
    pub doc_comment: &'a str,
    /// Parent type comment (e.g. "/// Parent Type: `Height`")
    pub parent_type_comment: &'a str,
    /// The full config for the nested struct.
    pub config: SelectionSetConfig<'a>,
}

/// Render a complete selection set struct.
pub fn render(config: &SelectionSetConfig) -> String {
    let indent = " ".repeat(config.indent);
    let inner_indent = format!("{}  ", indent);
    let mut result = String::new();

    // Struct declaration
    let conformance = render_conformance(config);
    result.push_str(&format!(
        "{}{}struct {}: {} {{\n",
        indent, config.access_modifier, config.struct_name, conformance
    ));

    // __data and init
    let data_keyword = if config.is_mutable { "var" } else { "let" };
    result.push_str(&format!("{}{}{} __data: DataDict\n", inner_indent, config.access_modifier, data_keyword));
    result.push_str(&format!(
        "{}{}init(_dataDict: DataDict) {{ __data = _dataDict }}\n",
        inner_indent, config.access_modifier
    ));

    // RootEntityType typealias (for inline fragments)
    if let Some(root_entity) = config.root_entity_type {
        result.push('\n');
        result.push_str(&format!(
            "{}{}typealias RootEntityType = {}\n",
            inner_indent, config.access_modifier, root_entity
        ));
        // No blank line between RootEntityType and __parentType
    } else {
        result.push('\n');
    }

    // __parentType
    result.push_str(&format!(
        "{}{}static var __parentType: any {}.ParentType {{ {} }}\n",
        inner_indent,
        config.access_modifier,
        config.api_target_name,
        config.parent_type.render(config.schema_namespace)
    ));

    // __mergedSources (for CompositeInlineFragment)
    // Always uses "public" access — it's a protocol requirement from CompositeInlineFragment
    if !config.merged_sources.is_empty() {
        let item_indent = format!("{}  ", inner_indent);
        result.push_str(&format!(
            "{}public static var __mergedSources: [any {}.SelectionSet.Type] {{ [\n",
            inner_indent, config.api_target_name
        ));
        for (i, source) in config.merged_sources.iter().enumerate() {
            let comma = if i < config.merged_sources.len() - 1 { "," } else { "" };
            result.push_str(&format!(
                "{}{}.self{}\n",
                item_indent, source, comma
            ));
        }
        result.push_str(&format!("{}] }}\n", inner_indent));
    }

    // #warning directives for deprecated arguments
    // Rendered before __selections, matching Swift's SelectionSetTemplate
    for (field_name, arg_name, reason) in &config.deprecated_arg_warnings {
        // Escape special characters in the reason text
        let escaped = reason
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\'', "\\'")
            .replace('\t', "\\t")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\0', "\\0");
        result.push_str(&format!(
            "{}#warning(\"Argument '{}' of field '{}' is deprecated. Reason: '{}'\")\n",
            inner_indent, arg_name, field_name, escaped
        ));
    }

    // __selections
    if !config.selections.is_empty() {
        let item_indent = format!("{}  ", inner_indent);
        result.push_str(&format!(
            "{}{}static var __selections: [{}.Selection] {{ [\n",
            inner_indent, config.access_modifier, config.api_target_name
        ));
        // All items get trailing comma (Swift trailing comma convention)
        for sel in config.selections.iter() {
            match sel {
                SelectionItem::Field(f) => {
                    result.push_str(&format!(
                        "{}{},\n",
                        item_indent, render_field_selection(f, &item_indent)
                    ));
                }
                SelectionItem::InlineFragment(name) => {
                    result.push_str(&format!(
                        "{}.inlineFragment({}.self),\n",
                        item_indent, name
                    ));
                }
                SelectionItem::Fragment(name) => {
                    result.push_str(&format!(
                        "{}.fragment({}.self),\n",
                        item_indent, name
                    ));
                }
                SelectionItem::ConditionalField(cond, f) => {
                    let cond_str = render_inclusion_condition(cond);
                    result.push_str(&format!(
                        "{}.include(if: {}, {}),\n",
                        item_indent, cond_str, render_field_selection(f, &item_indent)
                    ));
                }
                SelectionItem::ConditionalInlineFragment(cond, name) => {
                    let cond_str = render_inclusion_condition(cond);
                    result.push_str(&format!(
                        "{}.include(if: {}, .inlineFragment({}.self)),\n",
                        item_indent, cond_str, name
                    ));
                }
                SelectionItem::ConditionalFieldGroup(cond, fields) => {
                    let cond_str = render_inclusion_condition(cond);
                    result.push_str(&format!(
                        "{}.include(if: {}, [\n",
                        item_indent, cond_str
                    ));
                    let group_indent = format!("{}  ", item_indent);
                    for f in fields {
                        result.push_str(&format!(
                            "{}{},\n",
                            group_indent, render_field_selection(f, &group_indent)
                        ));
                    }
                    result.push_str(&format!("{}]),\n", item_indent));
                }
            }
        }
        result.push_str(&format!("{}] }}\n", inner_indent));
    }

    // Field accessors
    if !config.field_accessors.is_empty() {
        result.push('\n');
        for accessor in &config.field_accessors {
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
            let prop_name = naming::first_lowercased(accessor.name);
            if config.is_mutable {
                result.push_str(&format!(
                    "{}{}var {}: {} {{\n",
                    inner_indent,
                    config.access_modifier,
                    naming::escape_swift_name(&prop_name),
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
                let prop_name = naming::first_lowercased(accessor.name);
                result.push_str(&format!(
                    "{}{}var {}: {} {{ __data[\"{}\"] }}\n",
                    inner_indent,
                    config.access_modifier,
                    naming::escape_swift_name(&prop_name),
                    accessor.swift_type,
                    accessor.name
                ));
            }
        }
    }

    // Inline fragment accessors
    if !config.inline_fragment_accessors.is_empty() {
        result.push('\n');
        for accessor in &config.inline_fragment_accessors {
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
    if !config.fragment_spreads.is_empty() {
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
        for spread in &config.fragment_spreads {
            let optional_suffix = if spread.is_optional { "?" } else { "" };
            result.push_str(&format!(
                "{}{}var {}: {}{} {{ _toFragment() }}\n",
                frag_inner,
                config.access_modifier,
                spread.property_name,
                spread.fragment_type,
                optional_suffix
            ));
        }
        result.push_str(&format!("{}}}\n", inner_indent));
    }

    // Initializer
    if let Some(init_config) = &config.initializer {
        result.push('\n');
        render_initializer(&mut result, init_config, &inner_indent, config.access_modifier);
    }

    // Nested selection set structs (entity types), up to insert index
    let insert_idx = config.type_alias_insert_index.min(config.nested_types.len());
    for nested in &config.nested_types[..insert_idx] {
        result.push('\n');
        result.push_str(&format!(
            "{}{}\n",
            inner_indent, nested.doc_comment
        ));
        result.push_str(&format!(
            "{}{}\n",
            inner_indent, nested.parent_type_comment
        ));
        result.push_str(&render(&nested.config));
    }

    // Type aliases (between entity types and inline fragment types)
    for alias in &config.type_aliases {
        result.push('\n');
        result.push_str(&format!(
            "{}{}typealias {} = {}\n",
            inner_indent, config.access_modifier, alias.name, alias.target
        ));
    }

    // Remaining nested selection set structs (inline fragment types)
    for nested in &config.nested_types[insert_idx..] {
        result.push('\n');
        result.push_str(&format!(
            "{}{}\n",
            inner_indent, nested.doc_comment
        ));
        result.push_str(&format!(
            "{}{}\n",
            inner_indent, nested.parent_type_comment
        ));
        result.push_str(&render(&nested.config));
    }

    // Close struct
    result.push_str(&format!("{}}}\n", indent));

    result
}

fn render_conformance(config: &SelectionSetConfig) -> String {
    match &config.conformance {
        SelectionSetConformance::SelectionSet => {
            format!("{}.SelectionSet", config.schema_namespace)
        }
        SelectionSetConformance::Fragment => {
            format!("{}.SelectionSet, Fragment", config.schema_namespace)
        }
        SelectionSetConformance::InlineFragment => {
            format!("{}.InlineFragment", config.schema_namespace)
        }
        SelectionSetConformance::CompositeInlineFragment => {
            format!(
                "{}.InlineFragment, {}.CompositeInlineFragment",
                config.schema_namespace, config.api_target_name
            )
        }
        SelectionSetConformance::MutableSelectionSet => {
            format!("{}.MutableSelectionSet", config.schema_namespace)
        }
        SelectionSetConformance::MutableFragment => {
            format!("{}.MutableSelectionSet, Fragment", config.schema_namespace)
        }
        SelectionSetConformance::MutableInlineFragment => {
            format!("{}.MutableInlineFragment", config.schema_namespace)
        }
        SelectionSetConformance::Custom(s) => s.to_string(),
    }
}

/// Render a `.field(...)` selection item string with proper indentation.
fn render_field_selection(f: &FieldSelectionItem, indent: &str) -> String {
    let alias_part = if let Some(alias) = f.alias {
        format!(", alias: \"{}\"", alias)
    } else {
        String::new()
    };
    if let Some(args) = f.arguments {
        // Multi-line arguments need proper indentation relative to parent
        if args.contains('\n') {
            // Arguments format is "[\nentry1,\nentry2\n]"
            // Track bracket nesting depth to properly indent nested object values.
            // depth 1 = inside outermost [], depth 2 = inside nested [], etc.
            let base_indent = format!("{}  ", indent);
            let mut indented = String::new();
            let mut bracket_depth: usize = 0;

            for (i, line) in args.lines().enumerate() {
                if i > 0 {
                    indented.push('\n');
                }
                if i == 0 {
                    // Opening "[" stays on same line
                    indented.push_str(line);
                    bracket_depth += 1;
                } else {
                    let trimmed = line.trim();
                    // Check if this line is a closing bracket
                    if trimmed == "]" || trimmed == "])" {
                        bracket_depth = bracket_depth.saturating_sub(1);
                        if bracket_depth == 0 {
                            // Outermost closing "]" at parent indent level
                            indented.push_str(indent);
                        } else {
                            // Inner closing "]" at entry level for that depth
                            let extra = "  ".repeat(bracket_depth - 1);
                            indented.push_str(&base_indent);
                            indented.push_str(&extra);
                        }
                        indented.push_str(trimmed);
                    } else {
                        // Content line: indent at base + extra for depth
                        // depth 1 → base_indent (no extra)
                        // depth 2 → base_indent + "  " (2 extra)
                        let extra = "  ".repeat(bracket_depth.saturating_sub(1));
                        indented.push_str(&base_indent);
                        indented.push_str(&extra);
                        indented.push_str(trimmed);
                        // Check if this line ends with "[" (opens a nested value)
                        if trimmed.ends_with('[') {
                            bracket_depth += 1;
                        }
                    }
                }
            }
            format!(".field(\"{}\"{}, {}.self, arguments: {})", f.name, alias_part, f.swift_type, indented)
        } else {
            format!(".field(\"{}\"{}, {}.self, arguments: {})", f.name, alias_part, f.swift_type, args)
        }
    } else {
        format!(".field(\"{}\"{}, {}.self)", f.name, alias_part, f.swift_type)
    }
}

fn render_initializer(
    result: &mut String,
    config: &InitializerConfig,
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

    // Data entries - all get trailing comma, skip duplicates
    let mut seen_keys = std::collections::HashSet::new();
    for entry in config.data_entries.iter() {
        if !seen_keys.insert(&entry.key) { continue; } // skip duplicate keys
        match &entry.value {
            DataEntryValue::Variable(var_name) => {
                result.push_str(&format!(
                    "{}\"{}\": {},\n",
                    inner3, entry.key, var_name
                ));
            }
            DataEntryValue::FieldData(var_name) => {
                result.push_str(&format!(
                    "{}\"{}\": {}._fieldData,\n",
                    inner3, entry.key, var_name
                ));
            }
            DataEntryValue::Typename(type_ref) => {
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

/// Render an inclusion condition for `.include(if: ...)`.
pub fn render_inclusion_condition(cond: &InclusionConditionRef) -> String {
    if cond.conditions.len() == 1 {
        let c = &cond.conditions[0];
        if c.is_inverted {
            format!("!\"{}\"", c.variable)
        } else {
            format!("\"{}\"", c.variable)
        }
    } else {
        let op_str = match cond.operator {
            ConditionOperator::And => " && ",
            ConditionOperator::Or => " || ",
        };
        let parts: Vec<String> = cond.conditions.iter().map(|c| {
            if c.is_inverted {
                format!("!\"{}\"", c.variable)
            } else {
                format!("\"{}\"", c.variable)
            }
        }).collect();
        parts.join(op_str)
    }
}
