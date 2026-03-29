#!/bin/bash
# Regression test suite: runs reproduction schemas discovered during parity work.
# These are specific schemas that triggered bugs in the Rust codegen.
#
# Uses the complex-fuzz generator cases 9-14 which exercise:
# - fulfilledFragments under/over-inclusion (cases 9, 10, 11)
# - 3-level fragment chaining (case 12)
# - Overlapping entity fields with fragment spreads (case 13)
# - Inline input object literal formatting (case 14)

set -uo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib/codegen-test-utils.sh"

build_swift_cli
build_rust_cli

WORK_DIR=$(create_temp_dir)

echo ""
echo "Generating regression test schemas..."
python3 "$REPO_ROOT/scripts/lib/generate-complex-fuzz.py" "$WORK_DIR/cases"

echo ""
echo "Running regression tests (cases 9-14)..."
echo ""

for case_dir in "$WORK_DIR/cases"/case-{0009,0010,0011,0012,0013,0014}; do
  [ -d "$case_dir" ] || continue
  case_name=$(basename "$case_dir")

  config_file="$case_dir/config.json"
  [ -f "$config_file" ] || { echo -e "  ${YELLOW}SKIP${NC}: $case_name (no config)"; continue; }

  # Create output directories
  SWIFT_DIR="$case_dir/swift_out"
  RUST_DIR="$case_dir/rust_out"
  mkdir -p "$SWIFT_DIR" "$RUST_DIR"

  # Rewrite output paths for each CLI
  SWIFT_CONFIG="$case_dir/swift-config.json"
  RUST_CONFIG="$case_dir/rust-config.json"
  python3 -c "
import json
with open('$config_file') as f: c = json.load(f)
c['output']['schemaTypes']['path'] = '$SWIFT_DIR'
with open('$SWIFT_CONFIG', 'w') as f: json.dump(c, f, indent=2)
"
  python3 -c "
import json
with open('$config_file') as f: c = json.load(f)
c['output']['schemaTypes']['path'] = '$RUST_DIR'
with open('$RUST_CONFIG', 'w') as f: json.dump(c, f, indent=2)
"

  echo -e "  ${CYAN}Running Swift codegen for $case_name...${NC}"
  run_swift_codegen "$SWIFT_CONFIG"

  echo -e "  ${CYAN}Running Rust codegen for $case_name...${NC}"
  run_rust_codegen "$RUST_CONFIG"

  compare_generated "$RUST_DIR" "$SWIFT_DIR" "$case_name" || true
done

echo ""
echo "================================"
if [ "$FAIL_COUNT" -eq 0 ]; then
  echo -e "${GREEN}ALL $TOTAL_COUNT REGRESSION TESTS PASSED${NC}"
else
  echo -e "${RED}$FAIL_COUNT/$TOTAL_COUNT REGRESSION TESTS FAILED${NC}"
fi
echo "================================"

[ "$FAIL_COUNT" -eq 0 ] || exit 1
