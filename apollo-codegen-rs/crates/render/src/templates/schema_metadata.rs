//! Schema metadata template.
//!
//! Generates SchemaMetadata.graphql.swift containing:
//! - SelectionSet, InlineFragment, MutableSelectionSet, MutableInlineFragment protocols
//! - SchemaMetadata enum with objectType(forTypename:)
//! - Empty namespace enums (Objects, Interfaces, Unions)

use super::header;

pub fn render(
    schema_namespace: &str,
    object_types: &[(String, String)], // (graphql_typename, swift_name)
    access_modifier: &str,
    api_target_name: &str,
    is_embedded: bool,
) -> String {
    let ns = crate::naming::first_uppercased(schema_namespace);

    if is_embedded {
        render_embedded(&ns, object_types, access_modifier, api_target_name)
    } else {
        render_module(&ns, object_types, access_modifier, api_target_name)
    }
}

/// Render schema metadata for SwiftPackageManager/Other module types.
fn render_module(
    ns: &str,
    object_types: &[(String, String)],
    access_modifier: &str,
    api_target_name: &str,
) -> String {
    let mut result = String::new();
    result.push_str(header::HEADER);
    result.push_str("\n\n");
    result.push_str(&format!("import {}\n\n", api_target_name));

    // Protocol definitions
    result.push_str(&format!(
        "{access_modifier}protocol SelectionSet: {api}.SelectionSet & {api}.RootSelectionSet\n\
         where Schema == {ns}.SchemaMetadata {{}}\n\n\
         {access_modifier}protocol InlineFragment: {api}.SelectionSet & {api}.InlineFragment\n\
         where Schema == {ns}.SchemaMetadata {{}}\n\n\
         {access_modifier}protocol MutableSelectionSet: {api}.MutableRootSelectionSet\n\
         where Schema == {ns}.SchemaMetadata {{}}\n\n\
         {access_modifier}protocol MutableInlineFragment: {api}.MutableSelectionSet & {api}.InlineFragment\n\
         where Schema == {ns}.SchemaMetadata {{}}\n\n",
        access_modifier = access_modifier,
        api = api_target_name,
        ns = ns,
    ));

    // SchemaMetadata enum
    result.push_str(&format!(
        "{am}enum SchemaMetadata: {api}.SchemaMetadata {{\n\
         {s}{am}static let configuration: any {api}.SchemaConfiguration.Type = SchemaConfiguration.self\n\
         \n\
         {s}{am}static func objectType(forTypename typename: String) -> {api}.Object? {{\n\
         {s}{s}switch typename {{\n",
        am = access_modifier,
        api = api_target_name,
        s = "  ",
    ));

    for (typename, swift_name) in object_types {
        result.push_str(&format!(
            "    case \"{}\": return {}.Objects.{}\n",
            typename,
            ns,
            crate::naming::first_uppercased(swift_name),
        ));
    }

    result.push_str("    default: return nil\n");
    result.push_str("    }\n");
    result.push_str("  }\n");
    result.push_str("}\n\n");

    // Namespace enums
    result.push_str(&format!("{}enum Objects {{}}\n", access_modifier));
    result.push_str(&format!("{}enum Interfaces {{}}\n", access_modifier));
    result.push_str(&format!("{}enum Unions {{}}\n", access_modifier));

    result
}

/// Render schema metadata for embeddedInTarget module type.
///
/// In embedded mode:
/// - Protocols get `{Namespace}_` prefix to avoid conflicts
/// - Everything is wrapped in `extension {Namespace} { }`
/// - Inner declarations drop `public`
fn render_embedded(
    ns: &str,
    object_types: &[(String, String)],
    access_modifier: &str,
    api_target_name: &str,
) -> String {
    let mut result = String::new();
    result.push_str(header::HEADER);
    result.push_str("\n\n");
    result.push_str(&format!("import {}\n\n", api_target_name));

    // Protocol definitions with namespace prefix
    result.push_str(&format!(
        "{am}protocol {ns}_SelectionSet: {api}.SelectionSet & {api}.RootSelectionSet\n\
         where Schema == {ns}.SchemaMetadata {{}}\n\n\
         {am}protocol {ns}_InlineFragment: {api}.SelectionSet & {api}.InlineFragment\n\
         where Schema == {ns}.SchemaMetadata {{}}\n\n\
         {am}protocol {ns}_MutableSelectionSet: {api}.MutableRootSelectionSet\n\
         where Schema == {ns}.SchemaMetadata {{}}\n\n\
         {am}protocol {ns}_MutableInlineFragment: {api}.MutableSelectionSet & {api}.InlineFragment\n\
         where Schema == {ns}.SchemaMetadata {{}}\n\n",
        am = access_modifier,
        api = api_target_name,
        ns = ns,
    ));

    // Extension with typealiases, SchemaMetadata, and namespace enums
    result.push_str(&format!("{}extension {} {{\n", access_modifier, ns));
    result.push_str(&format!("  typealias SelectionSet = {}_SelectionSet\n\n", ns));
    result.push_str(&format!("  typealias InlineFragment = {}_InlineFragment\n\n", ns));
    result.push_str(&format!("  typealias MutableSelectionSet = {}_MutableSelectionSet\n\n", ns));
    result.push_str(&format!("  typealias MutableInlineFragment = {}_MutableInlineFragment\n\n", ns));

    // SchemaMetadata enum (inside extension, no public)
    result.push_str(&format!(
        "  enum SchemaMetadata: {api}.SchemaMetadata {{\n\
         {s}{s}{am}static let configuration: any {api}.SchemaConfiguration.Type = SchemaConfiguration.self\n\
         \n\
         {s}{s}{am}static func objectType(forTypename typename: String) -> {api}.Object? {{\n\
         {s}{s}{s}switch typename {{\n",
        am = access_modifier,
        api = api_target_name,
        s = "  ",
    ));

    for (typename, swift_name) in object_types {
        result.push_str(&format!(
            "      case \"{}\": return {}.Objects.{}\n",
            typename,
            ns,
            crate::naming::first_uppercased(swift_name),
        ));
    }

    result.push_str("      default: return nil\n");
    result.push_str("      }\n");
    result.push_str("    }\n");
    result.push_str("  }\n\n");

    // Namespace enums (inside extension, no public)
    result.push_str("  enum Objects {}\n");
    result.push_str("  enum Interfaces {}\n");
    result.push_str("  enum Unions {}\n\n");

    result.push('}');

    result
}
