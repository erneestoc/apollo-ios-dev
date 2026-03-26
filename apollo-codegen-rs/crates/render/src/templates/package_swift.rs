//! Swift Package Manager module template.
//!
//! Generates Package.swift for the schema types module.

pub fn render(
    schema_namespace: &str,
    test_mock_target: Option<(&str, &str)>, // (target_name, path)
) -> String {
    let ns = crate::naming::first_uppercased(schema_namespace);

    let mut result = String::new();
    result.push_str("// swift-tools-version:5.7\n\n");
    result.push_str("import PackageDescription\n\n");
    result.push_str("let package = Package(\n");
    result.push_str(&format!("  name: \"{}\",\n", ns));
    result.push_str("  platforms: [\n");
    result.push_str("    .iOS(.v12),\n");
    result.push_str("    .macOS(.v10_14),\n");
    result.push_str("    .tvOS(.v12),\n");
    result.push_str("    .watchOS(.v5),\n");
    result.push_str("  ],\n");
    result.push_str("  products: [\n");
    result.push_str(&format!(
        "    .library(name: \"{ns}\", targets: [\"{ns}\"]),\n",
        ns = ns
    ));
    if let Some((target_name, _)) = test_mock_target {
        result.push_str(&format!(
            "    .library(name: \"{tn}\", targets: [\"{tn}\"]),\n",
            tn = target_name
        ));
    }
    result.push_str("  ],\n");
    result.push_str("  dependencies: [\n");
    result.push_str(
        "    .package(url: \"https://github.com/apollographql/apollo-ios.git\", from: \"1.0.0\"),\n",
    );
    result.push_str("  ],\n");
    result.push_str("  targets: [\n");
    result.push_str("    .target(\n");
    result.push_str(&format!("      name: \"{}\",\n", ns));
    result.push_str("      dependencies: [\n");
    result.push_str(
        "        .product(name: \"ApolloAPI\", package: \"apollo-ios\"),\n",
    );
    result.push_str("      ],\n");
    result.push_str("      path: \"./Sources\"\n");
    result.push_str("    ),\n");
    if let Some((target_name, path)) = test_mock_target {
        result.push_str("    .target(\n");
        result.push_str(&format!("      name: \"{}\",\n", target_name));
        result.push_str("      dependencies: [\n");
        result.push_str(
            "        .product(name: \"ApolloTestSupport\", package: \"apollo-ios\"),\n",
        );
        result.push_str(&format!("        .target(name: \"{}\"),\n", ns));
        result.push_str("      ],\n");
        result.push_str(&format!("      path: \"{}\"\n", path));
        result.push_str("    ),\n");
    }
    result.push_str("  ]\n");
    result.push_str(")\n");

    result
}
