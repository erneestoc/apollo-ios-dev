//! Integration test: verify the CLI can run on the SwiftPackageManager config.
//! Tests the binary by running it as a subprocess.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn cli_generates_for_animal_kingdom() {
    // Build the CLI binary first
    let status = Command::new("cargo")
        .args(["build", "-p", "apollo-codegen-cli"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("Failed to build CLI");
    assert!(status.success(), "CLI build failed");

    // Create temp output directory
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = repo_root()
        .join("Tests/TestCodeGenConfigurations/SwiftPackageManager/apollo-codegen-config.json");

    // Run the CLI
    let output = Command::new(format!(
        "{}/target/debug/apollo-ios-cli-rs",
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .display()
    ))
    .args(["generate", "--path", &config_path.to_string_lossy(), "--verbose"])
    .output()
    .expect("Failed to run CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("CLI stderr: {}", stderr);

    assert!(
        output.status.success(),
        "CLI exited with error: {}",
        stderr,
    );

    assert!(
        stderr.contains("files written"),
        "CLI should report files written. Got: {}",
        stderr,
    );
}
