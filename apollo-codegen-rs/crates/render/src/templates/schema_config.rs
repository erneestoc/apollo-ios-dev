//! Schema configuration template.
//!
//! Generates SchemaConfiguration.swift - an editable file.

pub fn render(
    access_modifier: &str,
    api_target_name: &str,
) -> String {
    format!(
        "\
// @generated
// This file was automatically generated and can be edited to
// provide custom configuration for a generated GraphQL schema.
//
// Any changes to this file will not be overwritten by future
// code generation execution.

import {api}

{am}enum SchemaConfiguration: {api}.SchemaConfiguration {{
  {am}static func cacheKeyInfo(
    for type: {api}.Object,
    object: {api}.ObjectData
  ) -> {api}.CacheKeyInfo? {{
    // Implement this function to configure cache key resolution for your schema types.
    return nil
  }}
}}\n",
        api = api_target_name,
        am = access_modifier,
    )
}
