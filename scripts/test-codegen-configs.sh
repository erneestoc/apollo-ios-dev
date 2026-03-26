#!/bin/bash
# Test codegen configurations: runs both Swift and Rust CLIs against each of the 7
# test configurations in Tests/TestCodeGenConfigurations/ and compares the generated
# .swift files byte-for-byte.
#
# Instead of copying configs to temp dirs (which breaks relative paths), this script
# creates modified configs with absolute input paths and redirected output paths
# pointing to temp directories.
#
# Usage:
#   ./scripts/test-codegen-configs.sh            # run all configs
#   ./scripts/test-codegen-configs.sh ConfigName  # run a single config
#   VERBOSE=1 ./scripts/test-codegen-configs.sh   # show diffs on failure

source "$(dirname "$0")/lib/codegen-test-utils.sh"

CONFIGS_DIR="$REPO_ROOT/Tests/TestCodeGenConfigurations"
REWRITE_SCRIPT="$REPO_ROOT/scripts/lib/rewrite-config-paths.py"
ALL_CONFIGS=(
  SwiftPackageManager
  EmbeddedInTarget-InSchemaModule
  EmbeddedInTarget-RelativeAbsolute
  Other-CustomTarget
  Other-CocoaPods
  SPMInXcodeProject
  CodegenXCFramework
)

# Allow running a single config via CLI arg
if [ $# -gt 0 ]; then
  ALL_CONFIGS=("$@")
fi

# ---- Build both CLIs once ----
build_swift_cli
build_rust_cli

# ---- Create top-level temp dir (cleaned up on exit) ----
WORK_DIR=$(create_temp_dir)

echo ""
echo "Comparing codegen output for ${#ALL_CONFIGS[@]} configurations..."
echo ""

for config_name in "${ALL_CONFIGS[@]}"; do
  config_src="$CONFIGS_DIR/$config_name"
  config_file="$config_src/apollo-codegen-config.json"

  if [ ! -f "$config_file" ]; then
    echo -e "  ${YELLOW}SKIP${NC}: $config_name (no apollo-codegen-config.json)"
    continue
  fi

  # Create separate output directories for Swift and Rust
  swift_out="$WORK_DIR/$config_name/swift-out"
  rust_out="$WORK_DIR/$config_name/rust-out"
  mkdir -p "$swift_out" "$rust_out"

  # Create rewritten configs with absolute input paths and output redirected to temp dirs
  swift_config="$WORK_DIR/$config_name/swift-config.json"
  rust_config="$WORK_DIR/$config_name/rust-config.json"

  python3 "$REWRITE_SCRIPT" "$config_file" "$swift_config" "$swift_out"
  python3 "$REWRITE_SCRIPT" "$config_file" "$rust_config" "$rust_out"

  # ---- Run Swift codegen ----
  echo -e "  ${CYAN}Running Swift codegen for $config_name...${NC}"
  if ! (cd "$SWIFT_CLI_DIR" && swift run -c release apollo-ios-cli generate -p "$swift_config" --ignore-version-mismatch 2>&1) > "$WORK_DIR/$config_name/swift.log" 2>&1; then
    echo -e "  ${YELLOW}WARN${NC}: Swift codegen failed for $config_name (see $WORK_DIR/$config_name/swift.log)"
  fi

  # ---- Run Rust codegen ----
  echo -e "  ${CYAN}Running Rust codegen for $config_name...${NC}"
  if ! "$RUST_CLI" generate --path "$rust_config" > "$WORK_DIR/$config_name/rust.log" 2>&1; then
    echo -e "  ${YELLOW}WARN${NC}: Rust codegen failed for $config_name (see $WORK_DIR/$config_name/rust.log)"
  fi

  # ---- Compare generated .swift files ----
  # Both CLIs wrote output into their respective temp dirs ($rust_out and $swift_out).
  compare_generated "$rust_out" "$swift_out" "$config_name" || true
done

echo ""
print_summary
