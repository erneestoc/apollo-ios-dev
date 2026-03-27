//! Schema configuration template.
//!
//! Generates SchemaConfiguration.swift - an editable file.

pub fn render(
    access_modifier: &str,
    api_target_name: &str,
    is_embedded: bool,
) -> String {
    // In embedded mode, SchemaConfiguration doesn't have 'public' on the enum
    // because it's accessed within the module
    let enum_am = if is_embedded { "" } else { access_modifier };
    format!(
        "\
// @generated
// This file was automatically generated and can be edited to
// provide custom configuration for a generated GraphQL schema.
//
// Any changes to this file will not be overwritten by future
// code generation execution.

import {api}

{eam}enum SchemaConfiguration: {api}.SchemaConfiguration {{
  {am}static func cacheKeyInfo(for type: {api}.Object, object: {api}.ObjectData) -> CacheKeyInfo? {{
    // Implement this function to configure cache key resolution for your schema types.
    return nil
  }}
}}\n",
        api = api_target_name,
        am = access_modifier,
        eam = enum_am,
    )
}
