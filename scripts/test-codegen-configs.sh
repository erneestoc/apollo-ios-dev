#!/bin/bash
# Test codegen configurations: runs both Swift and Rust CLIs against each of the 7
# test configurations in Tests/TestCodeGenConfigurations/ and compares the generated
# .swift files byte-for-byte.
#
# Usage:
#   ./scripts/test-codegen-configs.sh            # run all configs
#   ./scripts/test-codegen-configs.sh ConfigName  # run a single config
#   VERBOSE=1 ./scripts/test-codegen-configs.sh   # show diffs on failure

source "$(dirname "$0")/lib/codegen-test-utils.sh"

CONFIGS_DIR="$REPO_ROOT/Tests/TestCodeGenConfigurations"
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

  # Create isolated working copies so each CLI writes to its own output tree
  swift_work="$WORK_DIR/$config_name/swift"
  rust_work="$WORK_DIR/$config_name/rust"
  mkdir -p "$swift_work" "$rust_work"

  # Copy the full config directory into both work dirs so relative paths resolve
  cp -a "$config_src/." "$swift_work/"
  cp -a "$config_src/." "$rust_work/"

  # The config files use relative paths for input (e.g. ../../../Sources/...).
  # We need those relative paths to still resolve from the work dir.
  # For configs that reference ../../../Sources, we create the equivalent symlink.
  # We compute the relative offset from the config dir to the repo root.
  # Since most configs are at Tests/TestCodeGenConfigurations/<name>/,
  # the offset is ../../../ (3 levels up to REPO_ROOT).
  #
  # We handle this by symlinking the repo Sources directory so that
  # ../../../Sources resolves correctly from the work dir.

  # Determine how many parent refs the config uses
  # Create a parent structure that makes relative paths work
  for work in "$swift_work" "$rust_work"; do
    # Create ../../.. relative to work dir, pointing to REPO_ROOT
    # The work dir is at $WORK_DIR/$config_name/{swift,rust}/
    # The config references paths relative to its own directory.
    # Original config dir: $REPO_ROOT/Tests/TestCodeGenConfigurations/$config_name/
    # So ../../../ from original = $REPO_ROOT
    # We need the same relative path from work dir to also reach $REPO_ROOT resources.

    # Create the parent chain: work/../../.. should point somewhere that has Sources/
    # Instead of fighting with relative paths, just symlink the Sources dir
    # at the relative location the config expects.
    #
    # Most configs use ../../../Sources/... so we need work/../../../Sources -> REPO_ROOT/Sources
    parent3="$(cd "$work" && cd ../../.. 2>/dev/null && pwd)" || true

    # Ensure the target of ../../../Sources exists by symlinking
    mkdir -p "$work/../../../" 2>/dev/null || true
    target_dir="$(cd "$work/../../.." && pwd)"

    # Only symlink if the Sources dir doesn't already exist at that level
    if [ ! -e "$target_dir/Sources" ]; then
      ln -sf "$REPO_ROOT/Sources" "$target_dir/Sources"
    fi
  done

  # ---- Run Swift codegen ----
  swift_config="$swift_work/apollo-codegen-config.json"
  echo -e "  ${CYAN}Running Swift codegen for $config_name...${NC}"
  if ! (cd "$SWIFT_CLI_DIR" && swift run -c release apollo-ios-cli generate -p "$swift_config" 2>&1) > "$WORK_DIR/$config_name/swift.log" 2>&1; then
    echo -e "  ${YELLOW}WARN${NC}: Swift codegen failed for $config_name (see $WORK_DIR/$config_name/swift.log)"
  fi

  # ---- Run Rust codegen ----
  rust_config="$rust_work/apollo-codegen-config.json"
  echo -e "  ${CYAN}Running Rust codegen for $config_name...${NC}"
  if ! "$RUST_CLI" generate --path "$rust_config" > "$WORK_DIR/$config_name/rust.log" 2>&1; then
    echo -e "  ${YELLOW}WARN${NC}: Rust codegen failed for $config_name (see $WORK_DIR/$config_name/rust.log)"
  fi

  # ---- Compare generated .swift files ----
  # Both CLIs write output relative to the config file's directory, so the
  # generated files should appear somewhere under $swift_work and $rust_work.
  compare_generated "$rust_work" "$swift_work" "$config_name" || true
done

echo ""
print_summary
