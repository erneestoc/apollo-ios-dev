use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::access_modifier::AccessModifier;

/// The local path structure for the generated test mock object files.
///
/// Mirrors Swift's `ApolloCodegenConfiguration.TestMockFileOutput` enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestMockFileOutput {
  /// Test mocks will not be generated. This is the default value.
  None,
  /// Generated test mock files will be located in the specified `path`.
  /// `access_modifier` defaults to `Public`.
  Absolute {
    path: String,
    access_modifier: AccessModifier,
  },
  /// Generated test mock files will be included in a target defined in the generated
  /// `Package.swift` file. `target_name` defaults to `None`.
  SwiftPackage { target_name: Option<String> },
}

impl<'de> Deserialize<'de> for TestMockFileOutput {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    struct TestMockFileOutputVisitor;

    impl<'de> Visitor<'de> for TestMockFileOutputVisitor {
      type Value = TestMockFileOutput;

      fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(
          "a TestMockFileOutput object with one key: none, absolute, or swiftPackage",
        )
      }

      fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let key: String = map
          .next_key()?
          .ok_or_else(|| de::Error::custom("Invalid number of keys found, expected one."))?;

        match key.as_str() {
          "none" => {
            let _: serde_json::Value = map.next_value()?;
            Ok(TestMockFileOutput::None)
          }
          "absolute" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Inner {
              path: String,
              #[serde(rename = "accessModifier", default = "crate::config::access_modifier::default_public")]
              access_modifier: AccessModifier,
            }
            let inner: Inner = map.next_value()?;
            Ok(TestMockFileOutput::Absolute {
              path: inner.path,
              access_modifier: inner.access_modifier,
            })
          }
          "swiftPackage" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Inner {
              #[serde(rename = "targetName")]
              target_name: Option<String>,
            }
            let inner: Inner = map.next_value()?;
            Ok(TestMockFileOutput::SwiftPackage {
              target_name: inner.target_name,
            })
          }
          other => Err(de::Error::unknown_variant(
            other,
            &["none", "absolute", "swiftPackage"],
          )),
        }
      }
    }

    deserializer.deserialize_map(TestMockFileOutputVisitor)
  }
}

impl Serialize for TestMockFileOutput {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    match self {
      TestMockFileOutput::None => {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("none", &serde_json::Map::new())?;
        map.end()
      }
      TestMockFileOutput::Absolute {
        path,
        access_modifier,
      } => {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Inner<'a> {
          path: &'a str,
          access_modifier: &'a AccessModifier,
        }
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(
          "absolute",
          &Inner {
            path,
            access_modifier,
          },
        )?;
        map.end()
      }
      TestMockFileOutput::SwiftPackage { target_name } => {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Inner<'a> {
          target_name: &'a Option<String>,
        }
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("swiftPackage", &Inner { target_name })?;
        map.end()
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_test_mock_none_roundtrip() {
    let json = r#"{"none":{}}"#;
    let parsed: TestMockFileOutput = serde_json::from_str(json).unwrap();
    assert_eq!(parsed, TestMockFileOutput::None);
    let serialized = serde_json::to_string(&parsed).unwrap();
    assert_eq!(serialized, json);
  }

  #[test]
  fn test_test_mock_absolute_roundtrip() {
    let json = r#"{"absolute":{"path":"x","accessModifier":"internal"}}"#;
    let parsed: TestMockFileOutput = serde_json::from_str(json).unwrap();
    assert_eq!(
      parsed,
      TestMockFileOutput::Absolute {
        path: "x".to_string(),
        access_modifier: AccessModifier::Internal,
      }
    );
    let serialized = serde_json::to_string(&parsed).unwrap();
    assert_eq!(serialized, json);
  }

  #[test]
  fn test_test_mock_swift_package_roundtrip() {
    let json = r#"{"swiftPackage":{"targetName":"SchemaTestMocks"}}"#;
    let parsed: TestMockFileOutput = serde_json::from_str(json).unwrap();
    assert_eq!(
      parsed,
      TestMockFileOutput::SwiftPackage {
        target_name: Some("SchemaTestMocks".to_string()),
      }
    );
    let serialized = serde_json::to_string(&parsed).unwrap();
    assert_eq!(serialized, json);
  }

  #[test]
  fn test_test_mock_swift_package_no_target_name() {
    let json = r#"{"swiftPackage":{"targetName":null}}"#;
    let parsed: TestMockFileOutput = serde_json::from_str(json).unwrap();
    assert_eq!(
      parsed,
      TestMockFileOutput::SwiftPackage {
        target_name: None,
      }
    );
  }
}
