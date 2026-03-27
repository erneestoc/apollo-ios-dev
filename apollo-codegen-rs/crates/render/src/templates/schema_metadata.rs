//! Schema metadata template.
//!
//! Generates SchemaMetadata.graphql.swift containing:
//! - SelectionSet, InlineFragment, MutableSelectionSet, MutableInlineFragment protocols
//! - SchemaMetadata enum with objectType(forTypename:)
//! - Empty namespace enums (Objects, Interfaces, Unions)

use askama::Template;

/// Pre-computed object type entry for the switch statement.
struct ObjectTypeEntry {
    typename: String,
    swift_name: String,
}

#[derive(Template)]
#[template(path = "schema_metadata.swift.askama", escape = "none")]
struct SchemaMetadataTemplate<'a> {
    api_target_name: &'a str,
    access_modifier: &'a str,
    ns: String,
    is_embedded: bool,
    object_types: Vec<ObjectTypeEntry>,
}

pub fn render(
    schema_namespace: &str,
    object_types: &[(String, String)], // (graphql_typename, swift_name)
    access_modifier: &str,
    api_target_name: &str,
    is_embedded: bool,
) -> String {
    let ns = crate::naming::first_uppercased(schema_namespace);

    let entries: Vec<ObjectTypeEntry> = object_types
        .iter()
        .map(|(typename, swift_name)| ObjectTypeEntry {
            typename: typename.clone(),
            swift_name: crate::naming::first_uppercased(swift_name),
        })
        .collect();

    let template = SchemaMetadataTemplate {
        api_target_name,
        access_modifier,
        ns,
        is_embedded,
        object_types: entries,
    };

    let mut output = template
        .render()
        .expect("schema_metadata template render failed");
    if !is_embedded {
        // Module mode: ensure trailing newline (matches Swift codegen output)
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}
