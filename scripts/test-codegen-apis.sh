#!/bin/bash
# Test codegen APIs: runs both Swift and Rust CLIs against each of the 5 test APIs
# (AnimalKingdomAPI, StarWarsAPI, GitHubAPI, UploadAPI, SubscriptionAPI) and compares
# the generated .swift files byte-for-byte.
#
# Generates configs with absolute input paths, then creates per-CLI copies with
# output paths redirected to separate temp directories for clean comparison.
#
# Usage:
#   ./scripts/test-codegen-apis.sh              # run all APIs
#   ./scripts/test-codegen-apis.sh StarWarsAPI   # run a single API
#   VERBOSE=1 ./scripts/test-codegen-apis.sh     # show diffs on failure

source "$(dirname "$0")/lib/codegen-test-utils.sh"

ALL_APIS=(
  AnimalKingdomAPI
  StarWarsAPI
  GitHubAPI
  UploadAPI
  SubscriptionAPI
)

# Allow running a single API via CLI arg
if [ $# -gt 0 ]; then
  ALL_APIS=("$@")
fi

# ---- Build both CLIs once ----
build_swift_cli
build_rust_cli

# ---- Create top-level temp dir (cleaned up on exit) ----
WORK_DIR=$(create_temp_dir)

# ---- Generate base configs for all APIs ----
echo ""
echo "Generating API configs..."
python3 "$REPO_ROOT/scripts/lib/generate-api-configs.py" "$WORK_DIR/configs" "$REPO_ROOT"

echo ""
echo "Comparing codegen output for ${#ALL_APIS[@]} APIs..."
echo ""

for api_name in "${ALL_APIS[@]}"; do
  config_file="$WORK_DIR/configs/$api_name/apollo-codegen-config.json"

  if [ ! -f "$config_file" ]; then
    echo -e "  ${YELLOW}SKIP${NC}: $api_name (config not generated)"
    continue
  fi

  # Create separate output directories for Swift and Rust
  swift_out="$WORK_DIR/$api_name/swift-out"
  rust_out="$WORK_DIR/$api_name/rust-out"
  mkdir -p "$swift_out" "$rust_out"

  # The generated configs have absolute input paths and relative output paths
  # (e.g. ./AnimalKingdomAPI). Create per-CLI configs with output redirected to
  # each CLI's temp directory using the rewrite script.
  swift_config="$WORK_DIR/$api_name/swift-config.json"
  rust_config="$WORK_DIR/$api_name/rust-config.json"

  python3 "$REPO_ROOT/scripts/lib/rewrite-config-paths.py" "$config_file" "$swift_config" "$swift_out"
  python3 "$REPO_ROOT/scripts/lib/rewrite-config-paths.py" "$config_file" "$rust_config" "$rust_out"

  # ---- Run Swift codegen ----
  echo -e "  ${CYAN}Running Swift codegen for $api_name...${NC}"
  if ! (cd "$SWIFT_CLI_DIR" && swift run -c release apollo-ios-cli generate -p "$swift_config" --ignore-version-mismatch 2>&1) > "$WORK_DIR/$api_name/swift.log" 2>&1; then
    echo -e "  ${YELLOW}WARN${NC}: Swift codegen failed for $api_name (see $WORK_DIR/$api_name/swift.log)"
  fi

  # ---- Run Rust codegen ----
  echo -e "  ${CYAN}Running Rust codegen for $api_name...${NC}"
  if ! "$RUST_CLI" generate --path "$rust_config" > "$WORK_DIR/$api_name/rust.log" 2>&1; then
    echo -e "  ${YELLOW}WARN${NC}: Rust codegen failed for $api_name (see $WORK_DIR/$api_name/rust.log)"
  fi

  # ---- Compare generated .swift files ----
  compare_generated "$rust_out" "$swift_out" "$api_name" || true
done

echo ""
print_summary
