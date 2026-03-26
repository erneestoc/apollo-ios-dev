#!/bin/bash
# Shared utilities for codegen comparison testing.
# Source this file from test scripts: source "$(dirname "$0")/lib/codegen-test-utils.sh"

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUST_CLI="$REPO_ROOT/apollo-codegen-rs/target/release/apollo-ios-cli-rs"
SWIFT_CLI_DIR="$REPO_ROOT/apollo-ios-codegen"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

PASS_COUNT=0
FAIL_COUNT=0
TOTAL_COUNT=0

# Build the Rust CLI in release mode (once)
build_rust_cli() {
  if [ ! -f "$RUST_CLI" ] || [ "$RUST_CLI" -ot "$REPO_ROOT/apollo-codegen-rs/Cargo.toml" ]; then
    echo -e "${CYAN}Building Rust CLI (release)...${NC}"
    (cd "$REPO_ROOT/apollo-codegen-rs" && cargo build --release -p apollo-codegen-cli 2>&1 | tail -1)
  fi
}

# Build the Swift CLI (once)
build_swift_cli() {
  echo -e "${CYAN}Building Swift CLI...${NC}"
  (cd "$SWIFT_CLI_DIR" && swift build -c release 2>&1 | tail -1)
}

# Run Rust codegen with a config file
# Args: $1=config_path, $2=output_dir (optional, uses config's relative paths if not set)
run_rust_codegen() {
  local config="$1"
  "$RUST_CLI" generate --path "$config" 2>/dev/null
}

# Run Swift codegen with a config file
# Args: $1=config_path
run_swift_codegen() {
  local config="$1"
  (cd "$SWIFT_CLI_DIR" && swift run -c release apollo-ios-cli generate -p "$config" 2>/dev/null)
}

# Compare two directories of generated files byte-for-byte
# Args: $1=dir_a (rust), $2=dir_b (swift), $3=label
# Returns: 0 if all match, 1 if any differ
compare_generated() {
  local dir_a="$1"
  local dir_b="$2"
  local label="$3"

  local match=0
  local differ=0
  local missing_in_a=0
  local missing_in_b=0

  # Check all files in dir_b (golden/swift output)
  while IFS= read -r -d '' f; do
    local rel="${f#$dir_b/}"
    local file_a="$dir_a/$rel"
    if [ -f "$file_a" ]; then
      if diff -q "$file_a" "$f" > /dev/null 2>&1; then
        match=$((match + 1))
      else
        differ=$((differ + 1))
        if [ "${VERBOSE:-0}" = "1" ]; then
          echo -e "  ${RED}DIFF${NC}: $rel"
          diff "$file_a" "$f" | head -5
        fi
      fi
    else
      missing_in_a=$((missing_in_a + 1))
      [ "${VERBOSE:-0}" = "1" ] && echo -e "  ${RED}MISSING in Rust${NC}: $rel"
    fi
  done < <(find "$dir_b" -name "*.swift" -print0 | sort -z)

  # Check for extra files in dir_a
  while IFS= read -r -d '' f; do
    local rel="${f#$dir_a/}"
    local file_b="$dir_b/$rel"
    if [ ! -f "$file_b" ]; then
      missing_in_b=$((missing_in_b + 1))
      [ "${VERBOSE:-0}" = "1" ] && echo -e "  ${YELLOW}EXTRA in Rust${NC}: $rel"
    fi
  done < <(find "$dir_a" -name "*.swift" -print0 | sort -z)

  local total=$((match + differ + missing_in_a))
  TOTAL_COUNT=$((TOTAL_COUNT + 1))

  if [ $differ -eq 0 ] && [ $missing_in_a -eq 0 ] && [ $missing_in_b -eq 0 ]; then
    echo -e "  ${GREEN}PASS${NC}: $label ($match/$total files match)"
    PASS_COUNT=$((PASS_COUNT + 1))
    return 0
  else
    echo -e "  ${RED}FAIL${NC}: $label ($match/$total match, $differ differ, $missing_in_a missing, $missing_in_b extra)"
    FAIL_COUNT=$((FAIL_COUNT + 1))
    return 1
  fi
}

# Print summary
print_summary() {
  echo ""
  echo "================================"
  if [ $FAIL_COUNT -eq 0 ]; then
    echo -e "${GREEN}ALL $TOTAL_COUNT TESTS PASSED${NC}"
  else
    echo -e "${RED}$FAIL_COUNT/$TOTAL_COUNT TESTS FAILED${NC}"
  fi
  echo "================================"
  return $FAIL_COUNT
}

# Create a temp directory that gets cleaned up on exit
create_temp_dir() {
  local tmp
  tmp=$(mktemp -d)
  trap "rm -rf '$tmp'" EXIT
  echo "$tmp"
}
