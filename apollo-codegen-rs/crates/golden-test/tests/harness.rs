//! Integration tests verifying the golden file test harness works correctly.

use apollo_codegen_golden_test::*;

#[test]
fn golden_files_exist_for_all_apis() {
    for api in TEST_APIS {
        let files = load_golden_files(api);
        assert!(
            !files.is_empty(),
            "No golden files found for {}",
            api
        );
        println!("{}: {} golden files", api, files.len());
    }
}

#[test]
fn golden_file_counts() {
    let expected_counts = [
        ("AnimalKingdomAPI", 50, 70),  // range to allow for non-.graphql.swift files
        ("StarWarsAPI", 60, 80),
        ("GitHubAPI", 220, 240),
        ("UploadAPI", 5, 15),
        ("SubscriptionAPI", 3, 10),
    ];

    for (api, min, max) in expected_counts {
        let files = load_golden_files(api);
        assert!(
            files.len() >= min && files.len() <= max,
            "{}: expected {}-{} files, got {}",
            api,
            min,
            max,
            files.len()
        );
    }
}

#[test]
fn empty_output_reports_all_missing() {
    for api in TEST_APIS {
        let result = compare_api_empty(api);
        assert!(
            !result.missing.is_empty(),
            "{} should have missing files when comparing against empty output",
            api
        );
        assert_eq!(result.matches, 0);
        assert!(result.mismatches.is_empty());
        assert!(result.extra.is_empty());
        println!("{}", result.summary());
    }
}

#[test]
fn golden_files_match_themselves() {
    for api in TEST_APIS {
        let golden = load_golden_files(api);
        let result = compare_api(api, &golden);
        assert!(
            result.is_perfect_match(),
            "{} golden files should match themselves: {}",
            api,
            result.summary()
        );
        println!("{}", result.summary());
    }
}

#[test]
fn detects_content_mismatch() {
    let mut files = load_golden_files("SubscriptionAPI");
    // Modify one file to verify mismatch detection
    if let Some((_key, value)) = files.iter_mut().next() {
        value.push_str("\n// MODIFIED");
    }
    let result = compare_api("SubscriptionAPI", &files);
    assert_eq!(
        result.mismatches.len(),
        1,
        "Should detect exactly one mismatch"
    );
}

#[test]
fn detects_extra_files() {
    let mut files = load_golden_files("SubscriptionAPI");
    files.insert("Extra.graphql.swift".to_string(), "// extra".to_string());
    let result = compare_api("SubscriptionAPI", &files);
    assert_eq!(result.extra.len(), 1, "Should detect one extra file");
}

#[test]
fn all_generated_files_start_with_generated_comment() {
    for api in TEST_APIS {
        let files = load_golden_files(api);
        for (path, content) in &files {
            // Skip SchemaConfiguration.swift (user-editable) and Package.swift
            if path.contains("SchemaConfiguration.swift") || path.contains("Package.swift") {
                continue;
            }
            // Skip CustomScalar files that are not .graphql.swift
            if !path.ends_with(".graphql.swift") {
                continue;
            }
            assert!(
                content.starts_with("// @generated"),
                "{}/{} should start with '// @generated' but starts with: {:?}",
                api,
                path,
                &content[..content.len().min(50)]
            );
        }
    }
}
