//! Configuration types mirroring Swift's ApolloCodegenConfiguration.

use serde::Deserialize;
use serde::de;

/// Root configuration for Apollo iOS code generation.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApolloCodegenConfiguration {
    pub schema_namespace: String,
    pub input: FileInput,
    pub output: FileOutput,
    #[serde(default)]
    pub options: OutputOptions,
    #[serde(default)]
    pub experimental_features: ExperimentalFeatures,
    #[serde(default)]
    pub schema_download: Option<SchemaDownloadConfiguration>,
    #[serde(default)]
    pub operation_manifest: Option<OperationManifestConfiguration>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInput {
    pub schema_search_paths: Vec<String>,
    pub operation_search_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOutput {
    pub schema_types: SchemaTypesFileOutput,
    pub operations: OperationsFileOutput,
    #[serde(default)]
    pub test_mocks: TestMockFileOutput,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaTypesFileOutput {
    pub path: String,
    pub module_type: SchemaModuleType,
}

/// Module type: `{"swiftPackageManager": {}}`, `{"embeddedInTarget": {...}}`, or `{"other": {}}`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SchemaModuleType {
    #[serde(rename_all = "camelCase")]
    SwiftPackageManager(SwiftPackageManagerConfig),
    #[serde(rename_all = "camelCase")]
    EmbeddedInTarget(EmbeddedInTargetConfig),
    Other(serde_json::Value),
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwiftPackageManagerConfig {
    #[serde(default)]
    pub target_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedInTargetConfig {
    pub name: String,
    #[serde(default)]
    pub access_modifier: AccessModifier,
}

/// Operations output: `{"inSchemaModule": {}}`, `{"absolute": {...}}`, or `{"relative": {...}}`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationsFileOutput {
    InSchemaModule(serde_json::Value),
    #[serde(rename_all = "camelCase")]
    Absolute(AbsoluteOperationsConfig),
    #[serde(rename_all = "camelCase")]
    Relative(RelativeOperationsConfig),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbsoluteOperationsConfig {
    pub path: String,
    #[serde(default = "default_public")]
    pub access_modifier: AccessModifier,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelativeOperationsConfig {
    pub subpath: Option<String>,
    #[serde(default = "default_public")]
    pub access_modifier: AccessModifier,
}

/// Test mocks: `{"none": {}}`, `{"absolute": {...}}`, or `{"swiftPackage": {...}}`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestMockFileOutput {
    None(serde_json::Value),
    #[serde(rename_all = "camelCase")]
    Absolute(AbsoluteTestMocksConfig),
    #[serde(rename_all = "camelCase")]
    SwiftPackage(SwiftPackageTestMocksConfig),
}

impl Default for TestMockFileOutput {
    fn default() -> Self {
        TestMockFileOutput::None(serde_json::Value::Null)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbsoluteTestMocksConfig {
    pub path: String,
    #[serde(default = "default_public")]
    pub access_modifier: AccessModifier,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwiftPackageTestMocksConfig {
    pub target_name: Option<String>,
}

// --- Options ---

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputOptions {
    #[serde(default)]
    pub additional_inflection_rules: Vec<InflectionRule>,
    #[serde(default)]
    pub query_string_literal_format: QueryStringLiteralFormat,
    #[serde(default)]
    pub deprecated_enum_cases: Composition,
    #[serde(default)]
    pub schema_documentation: SchemaDocumentation,
    #[serde(default)]
    pub selection_set_initializers: SelectionSetInitializers,
    #[serde(default)]
    pub operation_document_format: OperationDocumentFormat,
    #[serde(default)]
    pub cocoapods_compatible_import_statements: bool,
    #[serde(default)]
    pub warnings_on_deprecated_usage: Composition,
    #[serde(default = "default_true_bool")]
    pub prune_generated_files: bool,
    #[serde(default)]
    pub schema_customization: SchemaCustomization,
    #[serde(default)]
    pub reduce_generated_schema_types: bool,
    /// When true, generated operation and local cache mutation classes are marked `final`.
    /// Maps to JSON key `markOperationDefinitionsAsFinal`.
    #[serde(default)]
    pub mark_operation_definitions_as_final: bool,
    #[serde(default)]
    pub conversion_strategies: ConversionStrategies,
    // Legacy field, ignored
    #[serde(default)]
    pub apqs: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentalFeatures {
    #[serde(default)]
    pub field_merging: FieldMerging,
    #[serde(default)]
    pub legacy_safelisting_compatible_operations: bool,
    #[serde(default)]
    pub client_controlled_nullability: bool,
}

/// Field merging behavior.
///
/// In the Swift codegen (1.15.1), this is an `OptionSet` serialized as an array of strings:
/// - `["all"]` → merge all fields (ancestors + siblings + namedFragments)
/// - `["ancestors"]`, `["siblings"]`, `["namedFragments"]` → individual strategies
/// - `["ancestors", "siblings"]` → combined strategies
/// - `[]` → no merging
///
/// The default is `["all"]` (i.e., all merging enabled).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMerging {
    pub ancestors: bool,
    pub siblings: bool,
    pub named_fragments: bool,
}

impl FieldMerging {
    pub fn all() -> Self {
        Self { ancestors: true, siblings: true, named_fragments: true }
    }

    pub fn none() -> Self {
        Self { ancestors: false, siblings: false, named_fragments: false }
    }

    pub fn is_all(&self) -> bool {
        self.ancestors && self.siblings && self.named_fragments
    }

    pub fn is_none(&self) -> bool {
        !self.ancestors && !self.siblings && !self.named_fragments
    }
}

impl Default for FieldMerging {
    fn default() -> Self {
        Self::all()
    }
}

impl<'de> de::Deserialize<'de> for FieldMerging {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let values: Vec<String> = Vec::deserialize(deserializer)?;
        let mut result = FieldMerging::none();
        for v in &values {
            match v.as_str() {
                "all" => return Ok(FieldMerging::all()),
                "ancestors" => result.ancestors = true,
                "siblings" => result.siblings = true,
                "namedFragments" => result.named_fragments = true,
                other => {
                    return Err(de::Error::custom(format!(
                        "unknown fieldMerging value: {}",
                        other
                    )));
                }
            }
        }
        Ok(result)
    }
}

// --- Operation Manifest ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationManifestConfiguration {
    #[serde(default = "default_true_bool")]
    pub generate_manifest_on_codegen: bool,
    pub path: String,
    #[serde(default)]
    pub version: OperationManifestVersion,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationManifestVersion {
    #[default]
    PersistedQueries,
    #[serde(alias = "legacyAPQ", rename = "legacy")]
    Legacy,
}

// --- Common Types ---

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AccessModifier {
    Public,
    #[default]
    Internal,
}

fn default_public() -> AccessModifier {
    AccessModifier::Public
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QueryStringLiteralFormat {
    #[default]
    SingleLine,
    Multiline,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SchemaDocumentation {
    #[default]
    Include,
    Exclude,
}

fn default_true_bool() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Composition {
    #[default]
    Include,
    Exclude,
}

// --- Selection Set Initializers ---

/// Can be either a struct with named fields or an array of strings.
#[derive(Debug, Clone, Default)]
pub struct SelectionSetInitializers {
    pub operations: bool,
    pub named_fragments: bool,
    pub local_cache_mutations: bool,
}

impl<'de> de::Deserialize<'de> for SelectionSetInitializers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct StructForm {
            #[serde(default)]
            operations: bool,
            #[serde(default)]
            named_fragments: bool,
            #[serde(default)]
            local_cache_mutations: bool,
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Object(_) => {
                let s: StructForm =
                    serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(SelectionSetInitializers {
                    operations: s.operations,
                    named_fragments: s.named_fragments,
                    local_cache_mutations: s.local_cache_mutations,
                })
            }
            _ => Ok(SelectionSetInitializers::default()),
        }
    }
}

// --- Operation Document Format ---

/// Serialized as an array of strings: `["definition"]` or `["definition", "operationId"]`
#[derive(Debug, Clone)]
pub struct OperationDocumentFormat {
    pub definition: bool,
    pub operation_identifier: bool,
}

impl Default for OperationDocumentFormat {
    fn default() -> Self {
        Self {
            definition: true,
            operation_identifier: false,
        }
    }
}

impl<'de> de::Deserialize<'de> for OperationDocumentFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let values: Vec<String> = Vec::deserialize(deserializer)?;
        let mut result = OperationDocumentFormat {
            definition: false,
            operation_identifier: false,
        };
        for v in &values {
            match v.as_str() {
                "definition" => result.definition = true,
                "operationId" => result.operation_identifier = true,
                other => {
                    return Err(de::Error::custom(format!(
                        "unknown operationDocumentFormat value: {}",
                        other
                    )));
                }
            }
        }
        Ok(result)
    }
}

// --- Schema Customization ---

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCustomization {
    #[serde(default)]
    pub custom_type_names: indexmap::IndexMap<String, CustomizationType>,
}

/// Custom type name: either a simple string or a detailed configuration with
/// `"enum"` or `"inputObject"` key.
#[derive(Debug, Clone)]
pub enum CustomizationType {
    /// Simple rename: `"OldName": "NewName"`
    Type(String),
    /// Enum customization: `"EnumName": {"enum": {"name": "...", "cases": {...}}}`
    Enum {
        name: Option<String>,
        cases: Option<indexmap::IndexMap<String, String>>,
    },
    /// Input object customization: `"InputName": {"inputObject": {"name": "...", "fields": {...}}}`
    InputObject {
        name: Option<String>,
        fields: Option<indexmap::IndexMap<String, String>>,
    },
}

impl<'de> de::Deserialize<'de> for CustomizationType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;

        match value {
            serde_json::Value::String(s) => Ok(CustomizationType::Type(s)),
            serde_json::Value::Object(map) => {
                if let Some(enum_val) = map.get("enum") {
                    #[derive(Deserialize)]
                    struct EnumConfig {
                        name: Option<String>,
                        cases: Option<indexmap::IndexMap<String, String>>,
                    }
                    let config: EnumConfig =
                        serde_json::from_value(enum_val.clone()).map_err(de::Error::custom)?;
                    Ok(CustomizationType::Enum {
                        name: config.name,
                        cases: config.cases,
                    })
                } else if let Some(input_val) = map.get("inputObject") {
                    #[derive(Deserialize)]
                    struct InputConfig {
                        name: Option<String>,
                        fields: Option<indexmap::IndexMap<String, String>>,
                    }
                    let config: InputConfig =
                        serde_json::from_value(input_val.clone()).map_err(de::Error::custom)?;
                    Ok(CustomizationType::InputObject {
                        name: config.name,
                        fields: config.fields,
                    })
                } else if let Some(type_val) = map.get("type") {
                    if let Some(s) = type_val.as_str() {
                        Ok(CustomizationType::Type(s.to_string()))
                    } else {
                        Err(de::Error::custom("unexpected type value"))
                    }
                } else {
                    Err(de::Error::custom(
                        "expected 'enum', 'inputObject', or 'type' key",
                    ))
                }
            }
            _ => Err(de::Error::custom(
                "expected string or object for custom type name",
            )),
        }
    }
}

// --- Conversion Strategies ---

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionStrategies {
    #[serde(default)]
    pub enum_cases: EnumCaseConversionStrategy,
    #[serde(default)]
    pub input_objects: InputObjectConversionStrategy,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EnumCaseConversionStrategy {
    None,
    #[default]
    CamelCase,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputObjectConversionStrategy {
    None,
    #[default]
    CamelCase,
}

// --- Inflection Rules ---

/// Custom inflection rules for pluralization/singularization.
///
/// Mirrors Swift's `InflectionRule` enum with Codable serialization.
/// JSON format uses externally-tagged enum encoding:
/// - `{"pluralization": {"singularRegex": "...", "replacementRegex": "..."}}`
/// - `{"singularization": {"pluralRegex": "...", "replacementRegex": "..."}}`
/// - `{"irregular": {"singular": "...", "plural": "..."}}`
/// - `{"uncountable": {"word": "..."}}`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InflectionRule {
    /// A pluralization rule using regex.
    #[serde(rename_all = "camelCase")]
    Pluralization {
        singular_regex: String,
        replacement_regex: String,
    },
    /// A singularization rule using regex.
    #[serde(rename_all = "camelCase")]
    Singularization {
        plural_regex: String,
        replacement_regex: String,
    },
    /// An irregular word pair (e.g., "person" / "people").
    Irregular {
        singular: String,
        plural: String,
    },
    /// A word that is the same in singular and plural form (e.g., "fish").
    Uncountable {
        word: String,
    },
}

// --- Schema Download Configuration ---

/// Configuration for downloading a GraphQL schema.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDownloadConfiguration {
    pub download_method: SchemaDownloadMethod,
    #[serde(default = "default_download_timeout")]
    pub download_timeout: f64,
    #[serde(default, deserialize_with = "deserialize_headers")]
    pub headers: Vec<HTTPHeader>,
    pub output_path: String,
}

fn default_download_timeout() -> f64 {
    30.0
}

/// An HTTP header for schema download requests.
#[derive(Debug, Clone, Deserialize)]
pub struct HTTPHeader {
    pub key: String,
    pub value: String,
}

/// Deserialize headers from either an array of `{key, value}` objects or a
/// flat `{key: value}` dictionary.
fn deserialize_headers<'de, D>(deserializer: D) -> Result<Vec<HTTPHeader>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Array(_) => {
            let headers: Vec<HTTPHeader> =
                serde_json::from_value(value).map_err(de::Error::custom)?;
            Ok(headers)
        }
        serde_json::Value::Object(map) => {
            let mut headers: Vec<HTTPHeader> = map
                .into_iter()
                .map(|(k, v)| HTTPHeader {
                    key: k,
                    value: v.as_str().unwrap_or_default().to_string(),
                })
                .collect();
            headers.sort_by(|a, b| a.key.cmp(&b.key));
            Ok(headers)
        }
        serde_json::Value::Null => Ok(Vec::new()),
        _ => Err(de::Error::custom("expected array, object, or null for headers")),
    }
}

impl SchemaDownloadConfiguration {
    /// The output format based on the download method.
    pub fn output_format(&self) -> SchemaDownloadOutputFormat {
        match &self.download_method {
            SchemaDownloadMethod::ApolloRegistry(_) => SchemaDownloadOutputFormat::SDL,
            SchemaDownloadMethod::Introspection(settings) => settings.output_format,
        }
    }
}

/// How to download the schema.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SchemaDownloadMethod {
    Introspection(IntrospectionSettings),
    ApolloRegistry(ApolloRegistrySettings),
}

/// Settings for downloading a schema via GraphQL introspection.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntrospectionSettings {
    #[serde(rename = "endpointURL")]
    pub endpoint_url: String,
    #[serde(default = "default_http_method")]
    pub http_method: SchemaDownloadHTTPMethod,
    #[serde(default)]
    pub output_format: SchemaDownloadOutputFormat,
    #[serde(default)]
    pub include_deprecated_input_values: bool,
}

fn default_http_method() -> SchemaDownloadHTTPMethod {
    SchemaDownloadHTTPMethod::POST { header_name: None, header_value: None }
}

/// HTTP method used for introspection requests.
#[derive(Debug, Clone, Deserialize)]
pub enum SchemaDownloadHTTPMethod {
    POST {
        #[serde(default, rename = "headerName")]
        header_name: Option<String>,
        #[serde(default, rename = "headerValue")]
        header_value: Option<String>,
    },
    GET {
        #[serde(default = "default_query_parameter_name", rename = "queryParameterName")]
        query_parameter_name: String,
    },
}

fn default_query_parameter_name() -> String {
    "query".to_string()
}

/// Output format for introspection download.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum SchemaDownloadOutputFormat {
    SDL,
    JSON,
}

impl Default for SchemaDownloadOutputFormat {
    fn default() -> Self {
        SchemaDownloadOutputFormat::SDL
    }
}

/// Settings for downloading a schema from the Apollo Registry.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApolloRegistrySettings {
    pub key: String,
    #[serde(rename = "graphID")]
    pub graph_id: String,
    #[serde(default = "default_variant")]
    pub variant: String,
}

fn default_variant() -> String {
    "current".to_string()
}

// --- Parsing ---

impl ApolloCodegenConfiguration {
    /// Parse a configuration from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Parse a configuration from a JSON file.
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&contents)?)
    }
}
