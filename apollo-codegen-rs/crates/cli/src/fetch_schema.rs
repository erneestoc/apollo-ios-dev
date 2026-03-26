//! Schema download via GraphQL introspection or Apollo Registry.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use apollo_codegen_config::{
    ApolloRegistrySettings, HTTPHeader, IntrospectionSettings, SchemaDownloadConfiguration,
    SchemaDownloadHTTPMethod, SchemaDownloadMethod, SchemaDownloadOutputFormat,
};

/// Standard GraphQL introspection query.
fn introspection_query(include_deprecated_input_values: bool) -> String {
    let input_deprecation_args = if include_deprecated_input_values {
        "(includeDeprecated: true)"
    } else {
        ""
    };
    let input_value_deprecation_fields = if include_deprecated_input_values {
        "\n          isDeprecated\n          deprecationReason"
    } else {
        ""
    };

    format!(
        r#"query IntrospectionQuery {{
      __schema {{
        queryType {{ name }}
        mutationType {{ name }}
        subscriptionType {{ name }}
        types {{
          ...FullType
        }}
        directives {{
          name
          description
          locations
          args {{
            ...InputValue
          }}
        }}
      }}
    }}
    fragment FullType on __Type {{
      kind
      name
      description
      fields(includeDeprecated: true) {{
        name
        description
        args{input_deprecation_args} {{
          ...InputValue
        }}
        type {{
          ...TypeRef
        }}
        isDeprecated
        deprecationReason
      }}
      inputFields{input_deprecation_args} {{
        ...InputValue
      }}
      interfaces {{
        ...TypeRef
      }}
      enumValues(includeDeprecated: true) {{
        name
        description
        isDeprecated
        deprecationReason
      }}
      possibleTypes {{
        ...TypeRef
      }}
    }}
    fragment InputValue on __InputValue {{
      name
      description
      type {{ ...TypeRef }}
      defaultValue{input_value_deprecation_fields}
    }}
    fragment TypeRef on __Type {{
      kind
      name
      ofType {{
        kind
        name
        ofType {{
          kind
          name
          ofType {{
            kind
            name
            ofType {{
              kind
              name
              ofType {{
                kind
                name
                ofType {{
                  kind
                  name
                  ofType {{
                    kind
                    name
                  }}
                }}
              }}
            }}
          }}
        }}
      }}
    }}"#
    )
}

/// Fetch a GraphQL schema using the provided configuration.
pub fn fetch_schema(config: &SchemaDownloadConfiguration, root: &Path, verbose: bool) -> Result<()> {
    match &config.download_method {
        SchemaDownloadMethod::Introspection(settings) => {
            download_via_introspection(settings, config, root, verbose)
        }
        SchemaDownloadMethod::ApolloRegistry(settings) => {
            download_from_registry(settings, config, root, verbose)
        }
    }
}

/// Resolve the output path relative to the root directory.
fn resolve_output_path(output_path: &str, root: &Path) -> PathBuf {
    let path = Path::new(output_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

/// Ensure the parent directory of the output path exists.
fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    Ok(())
}

/// Build a reqwest client with custom headers and timeout.
fn build_client(
    headers: &[HTTPHeader],
    timeout_secs: f64,
    extra_headers: &[(&str, &str)],
) -> Result<reqwest::blocking::Client> {
    let mut header_map = reqwest::header::HeaderMap::new();
    header_map.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    for h in headers {
        let name = reqwest::header::HeaderName::from_bytes(h.key.as_bytes())
            .with_context(|| format!("Invalid header name: {}", h.key))?;
        let value = reqwest::header::HeaderValue::from_str(&h.value)
            .with_context(|| format!("Invalid header value for {}", h.key))?;
        header_map.insert(name, value);
    }

    for (k, v) in extra_headers {
        let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
            .with_context(|| format!("Invalid header name: {}", k))?;
        let value = reqwest::header::HeaderValue::from_str(v)
            .with_context(|| format!("Invalid header value for {}", k))?;
        header_map.insert(name, value);
    }

    reqwest::blocking::Client::builder()
        .default_headers(header_map)
        .timeout(Duration::from_secs_f64(timeout_secs))
        .build()
        .context("Failed to build HTTP client")
}

/// Download schema via GraphQL introspection.
fn download_via_introspection(
    settings: &IntrospectionSettings,
    config: &SchemaDownloadConfiguration,
    root: &Path,
    verbose: bool,
) -> Result<()> {
    if verbose {
        eprintln!(
            "Downloading schema via introspection from {}",
            settings.endpoint_url
        );
    }

    let query = introspection_query(settings.include_deprecated_input_values);
    let client = build_client(&config.headers, config.download_timeout, &[])?;

    let response_text = match &settings.http_method {
        SchemaDownloadHTTPMethod::POST { .. } => {
            let body = serde_json::json!({
                "operationName": "IntrospectionQuery",
                "query": query,
                "variables": {}
            });

            let resp = client
                .post(&settings.endpoint_url)
                .json(&body)
                .send()
                .with_context(|| {
                    format!("Failed to send introspection request to {}", settings.endpoint_url)
                })?;

            if !resp.status().is_success() {
                return Err(anyhow!(
                    "Introspection request failed with status {}: {}",
                    resp.status(),
                    resp.text().unwrap_or_default()
                ));
            }

            resp.text().context("Failed to read introspection response")?
        }
        SchemaDownloadHTTPMethod::GET {
            query_parameter_name,
        } => {
            let resp = client
                .get(&settings.endpoint_url)
                .query(&[(query_parameter_name.as_str(), query.as_str())])
                .send()
                .with_context(|| {
                    format!("Failed to send introspection GET request to {}", settings.endpoint_url)
                })?;

            if !resp.status().is_success() {
                return Err(anyhow!(
                    "Introspection request failed with status {}: {}",
                    resp.status(),
                    resp.text().unwrap_or_default()
                ));
            }

            resp.text().context("Failed to read introspection response")?
        }
    };

    let output_path = resolve_output_path(&config.output_path, root);
    ensure_parent_dir(&output_path)?;

    match config.output_format() {
        SchemaDownloadOutputFormat::JSON => {
            // Write the introspection JSON directly.
            // Parse and re-serialize to ensure it's valid and pretty-printed.
            let json_value: serde_json::Value = serde_json::from_str(&response_text)
                .context("Failed to parse introspection response as JSON")?;
            let pretty = serde_json::to_string_pretty(&json_value)
                .context("Failed to format introspection JSON")?;
            let mut file = std::fs::File::create(&output_path)
                .with_context(|| format!("Failed to create output file: {}", output_path.display()))?;
            file.write_all(pretty.as_bytes())?;
        }
        SchemaDownloadOutputFormat::SDL => {
            // Convert introspection JSON to SDL using apollo-compiler.
            let sdl = introspection_json_to_sdl(&response_text)?;
            let mut file = std::fs::File::create(&output_path)
                .with_context(|| format!("Failed to create output file: {}", output_path.display()))?;
            file.write_all(sdl.as_bytes())?;
        }
    }

    if verbose {
        eprintln!("Successfully downloaded schema to {}", output_path.display());
    }

    eprintln!("Schema downloaded to {}", output_path.display());
    Ok(())
}

/// Convert an introspection JSON response to SDL format.
fn introspection_json_to_sdl(response_text: &str) -> Result<String> {
    apollo_codegen_frontend::introspection::introspection_json_to_sdl(response_text)
        .map_err(|e| anyhow!("Failed to convert introspection JSON to SDL: {}", e))
}

/// Download schema from the Apollo Registry.
fn download_from_registry(
    settings: &ApolloRegistrySettings,
    config: &SchemaDownloadConfiguration,
    root: &Path,
    verbose: bool,
) -> Result<()> {
    if verbose {
        eprintln!(
            "Downloading schema from Apollo Registry (graph: {}, variant: {})",
            settings.graph_id, settings.variant
        );
    }

    let registry_endpoint = "https://graphql.api.apollographql.com/api/graphql";

    let query = r#"query DownloadSchema($graphID: ID!, $variant: String!) {
      service(id: $graphID) {
        variant(name: $variant) {
          activeSchemaPublish {
            schema {
              document
            }
          }
        }
      }
    }"#;

    let body = serde_json::json!({
        "operationName": "DownloadSchema",
        "query": query,
        "variables": {
            "graphID": settings.graph_id,
            "variant": settings.variant
        }
    });

    let extra_headers = [("x-api-key", settings.key.as_str())];
    let client = build_client(&config.headers, config.download_timeout, &extra_headers)?;

    let resp = client
        .post(registry_endpoint)
        .json(&body)
        .send()
        .context("Failed to send request to Apollo Registry")?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "Apollo Registry request failed with status {}: {}",
            resp.status(),
            resp.text().unwrap_or_default()
        ));
    }

    let response_text = resp
        .text()
        .context("Failed to read Apollo Registry response")?;

    let response: serde_json::Value =
        serde_json::from_str(&response_text).context("Failed to parse registry response JSON")?;

    // Extract SDL from: data.service.variant.activeSchemaPublish.schema.document
    let sdl = response
        .get("data")
        .and_then(|d| d.get("service"))
        .and_then(|s| s.get("variant"))
        .and_then(|v| v.get("activeSchemaPublish"))
        .and_then(|a| a.get("schema"))
        .and_then(|s| s.get("document"))
        .and_then(|d| d.as_str())
        .ok_or_else(|| anyhow!("Could not extract SDL schema from Apollo Registry response"))?;

    let output_path = resolve_output_path(&config.output_path, root);
    ensure_parent_dir(&output_path)?;

    let mut file = std::fs::File::create(&output_path)
        .with_context(|| format!("Failed to create output file: {}", output_path.display()))?;
    file.write_all(sdl.as_bytes())?;

    if verbose {
        eprintln!(
            "Successfully downloaded schema from registry to {}",
            output_path.display()
        );
    }

    eprintln!("Schema downloaded to {}", output_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_introspection_query_without_deprecated() {
        let query = introspection_query(false);
        assert!(query.contains("query IntrospectionQuery"));
        assert!(query.contains("__schema"));
        // The InputValue fragment should NOT contain isDeprecated when disabled
        let input_value_section = query.split("fragment InputValue on __InputValue").nth(1).unwrap();
        assert!(!input_value_section.contains("isDeprecated"));
        // inputFields should NOT have (includeDeprecated: true)
        assert!(!query.contains("inputFields(includeDeprecated: true)"));
    }

    #[test]
    fn test_introspection_query_with_deprecated() {
        let query = introspection_query(true);
        assert!(query.contains("query IntrospectionQuery"));
        // When enabled, args and inputFields should have (includeDeprecated: true)
        assert!(query.contains("(includeDeprecated: true)"));
        // The InputValue fragment should contain isDeprecated
        let input_value_section = query.split("fragment InputValue on __InputValue").nth(1).unwrap();
        assert!(input_value_section.contains("isDeprecated"));
        assert!(input_value_section.contains("deprecationReason"));
    }

    #[test]
    fn test_resolve_output_path_absolute() {
        let path = resolve_output_path("/tmp/schema.graphqls", Path::new("/some/root"));
        assert_eq!(path, PathBuf::from("/tmp/schema.graphqls"));
    }

    #[test]
    fn test_resolve_output_path_relative() {
        let path = resolve_output_path("./schema.graphqls", Path::new("/some/root"));
        assert_eq!(path, PathBuf::from("/some/root/./schema.graphqls"));
    }

    #[test]
    fn test_deserialize_introspection_config() {
        let json = r#"{
            "schemaDownload": {
                "downloadMethod": {
                    "introspection": {
                        "endpointURL": "http://localhost:4000/graphql",
                        "httpMethod": {
                            "POST": {
                                "headerName": null,
                                "headerValue": null
                            }
                        },
                        "includeDeprecatedInputValues": false,
                        "outputFormat": "SDL"
                    }
                },
                "downloadTimeout": 60,
                "headers": {
                    "Authorization": "Bearer token"
                },
                "outputPath": "./schema.graphqls"
            },
            "schemaNamespace": "TestSchema",
            "input": {
                "schemaSearchPaths": ["**/*.graphqls"],
                "operationSearchPaths": ["**/*.graphql"]
            },
            "output": {
                "schemaTypes": {
                    "path": "./Generated",
                    "moduleType": {
                        "swiftPackageManager": {}
                    }
                },
                "operations": {
                    "inSchemaModule": {}
                }
            }
        }"#;

        let config: apollo_codegen_config::ApolloCodegenConfiguration =
            serde_json::from_str(json).expect("Failed to parse config");

        let schema_download = config.schema_download.expect("Missing schemaDownload");
        assert_eq!(schema_download.download_timeout, 60.0);
        assert_eq!(schema_download.output_path, "./schema.graphqls");
        assert_eq!(schema_download.headers.len(), 1);
        assert_eq!(schema_download.headers[0].key, "Authorization");
        assert_eq!(schema_download.headers[0].value, "Bearer token");

        match &schema_download.download_method {
            SchemaDownloadMethod::Introspection(settings) => {
                assert_eq!(settings.endpoint_url, "http://localhost:4000/graphql");
                assert!(!settings.include_deprecated_input_values);
                assert_eq!(settings.output_format, SchemaDownloadOutputFormat::SDL);
                match &settings.http_method {
                    SchemaDownloadHTTPMethod::POST { .. } => {}
                    _ => panic!("Expected POST method"),
                }
            }
            _ => panic!("Expected introspection download method"),
        }
    }

    #[test]
    fn test_deserialize_registry_config() {
        let json = r#"{
            "schemaDownload": {
                "downloadMethod": {
                    "apolloRegistry": {
                        "graphID": "my-graph-id",
                        "variant": "current",
                        "key": "service:my-graph:key"
                    }
                },
                "outputPath": "./schema.graphqls"
            },
            "schemaNamespace": "TestSchema",
            "input": {
                "schemaSearchPaths": ["**/*.graphqls"],
                "operationSearchPaths": ["**/*.graphql"]
            },
            "output": {
                "schemaTypes": {
                    "path": "./Generated",
                    "moduleType": {
                        "swiftPackageManager": {}
                    }
                },
                "operations": {
                    "inSchemaModule": {}
                }
            }
        }"#;

        let config: apollo_codegen_config::ApolloCodegenConfiguration =
            serde_json::from_str(json).expect("Failed to parse config");

        let schema_download = config.schema_download.expect("Missing schemaDownload");
        assert_eq!(schema_download.download_timeout, 30.0); // default
        assert_eq!(schema_download.output_path, "./schema.graphqls");
        assert!(schema_download.headers.is_empty());

        match &schema_download.download_method {
            SchemaDownloadMethod::ApolloRegistry(settings) => {
                assert_eq!(settings.graph_id, "my-graph-id");
                assert_eq!(settings.variant, "current");
                assert_eq!(settings.key, "service:my-graph:key");
            }
            _ => panic!("Expected apollo registry download method"),
        }
    }

    #[test]
    fn test_deserialize_config_without_schema_download() {
        let json = r#"{
            "schemaNamespace": "TestSchema",
            "input": {
                "schemaSearchPaths": ["**/*.graphqls"],
                "operationSearchPaths": ["**/*.graphql"]
            },
            "output": {
                "schemaTypes": {
                    "path": "./Generated",
                    "moduleType": {
                        "swiftPackageManager": {}
                    }
                },
                "operations": {
                    "inSchemaModule": {}
                }
            }
        }"#;

        let config: apollo_codegen_config::ApolloCodegenConfiguration =
            serde_json::from_str(json).expect("Failed to parse config");
        assert!(config.schema_download.is_none());
    }

    #[test]
    fn test_deserialize_headers_as_array() {
        let json = r#"{
            "schemaDownload": {
                "downloadMethod": {
                    "introspection": {
                        "endpointURL": "http://localhost:4000/graphql"
                    }
                },
                "headers": [
                    {"key": "Authorization", "value": "Bearer token"},
                    {"key": "X-Custom", "value": "custom-value"}
                ],
                "outputPath": "./schema.graphqls"
            },
            "schemaNamespace": "TestSchema",
            "input": {
                "schemaSearchPaths": ["**/*.graphqls"],
                "operationSearchPaths": ["**/*.graphql"]
            },
            "output": {
                "schemaTypes": {
                    "path": "./Generated",
                    "moduleType": {
                        "swiftPackageManager": {}
                    }
                },
                "operations": {
                    "inSchemaModule": {}
                }
            }
        }"#;

        let config: apollo_codegen_config::ApolloCodegenConfiguration =
            serde_json::from_str(json).expect("Failed to parse config");

        let schema_download = config.schema_download.expect("Missing schemaDownload");
        assert_eq!(schema_download.headers.len(), 2);
        assert_eq!(schema_download.headers[0].key, "Authorization");
        assert_eq!(schema_download.headers[1].key, "X-Custom");
    }

    #[test]
    fn test_deserialize_get_method() {
        let json = r#"{
            "schemaDownload": {
                "downloadMethod": {
                    "introspection": {
                        "endpointURL": "http://localhost:4000/graphql",
                        "httpMethod": {
                            "GET": {
                                "queryParameterName": "query"
                            }
                        }
                    }
                },
                "outputPath": "./schema.graphqls"
            },
            "schemaNamespace": "TestSchema",
            "input": {
                "schemaSearchPaths": ["**/*.graphqls"],
                "operationSearchPaths": ["**/*.graphql"]
            },
            "output": {
                "schemaTypes": {
                    "path": "./Generated",
                    "moduleType": {
                        "swiftPackageManager": {}
                    }
                },
                "operations": {
                    "inSchemaModule": {}
                }
            }
        }"#;

        let config: apollo_codegen_config::ApolloCodegenConfiguration =
            serde_json::from_str(json).expect("Failed to parse config");

        let schema_download = config.schema_download.expect("Missing schemaDownload");
        match &schema_download.download_method {
            SchemaDownloadMethod::Introspection(settings) => {
                match &settings.http_method {
                    SchemaDownloadHTTPMethod::GET { query_parameter_name } => {
                        assert_eq!(query_parameter_name, "query");
                    }
                    _ => panic!("Expected GET method"),
                }
            }
            _ => panic!("Expected introspection download method"),
        }
    }
}
