#!/bin/bash
# test-codegen-permutations.sh
#
# Generate ~50 codegen config permutations for AnimalKingdomAPI, then run both
# Swift and Rust codegens against each one and compare the output.
#
# Usage:
#   ./scripts/test-codegen-permutations.sh            # run all
#   ./scripts/test-codegen-permutations.sh -v          # verbose diffs
#   ./scripts/test-codegen-permutations.sh -j 4        # 4 parallel jobs (default: serial)
#   ./scripts/test-codegen-permutations.sh -k          # keep temp dirs on failure

source "$(dirname "$0")/lib/codegen-test-utils.sh"

VERBOSE=${VERBOSE:-0}
KEEP_TMP=0
JOBS=1

while getopts 'vkj:' OPTION; do
  case "$OPTION" in
    v) VERBOSE=1 ;;
    k) KEEP_TMP=1 ;;
    j) JOBS="$OPTARG" ;;
    ?)
      echo "Usage: $(basename "$0") [-v] [-k] [-j N]" >&2
      exit 1
      ;;
  esac
done
shift "$((OPTIND - 1))"

export VERBOSE

# ---------------------------------------------------------------------------
# Create workspace
# ---------------------------------------------------------------------------
WORK_DIR=$(mktemp -d)
CONFIGS_DIR="$WORK_DIR/configs"
RESULTS_FILE="$WORK_DIR/results.txt"
mkdir -p "$CONFIGS_DIR"

cleanup() {
  if [ "$KEEP_TMP" = "0" ]; then
    rm -rf "$WORK_DIR"
  else
    echo -e "${YELLOW}Temp directory preserved: $WORK_DIR${NC}"
  fi
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# Step 1: Generate configs
# ---------------------------------------------------------------------------
echo -e "${CYAN}Generating permutation configs...${NC}"
python3 "$REPO_ROOT/scripts/lib/generate-permutation-configs.py" "$CONFIGS_DIR"

CONFIG_FILES=("$CONFIGS_DIR"/config-*.json)
NUM_CONFIGS=${#CONFIG_FILES[@]}
echo -e "${CYAN}Generated $NUM_CONFIGS configurations.${NC}"
echo ""

# ---------------------------------------------------------------------------
# Step 2: Build CLIs
# ---------------------------------------------------------------------------
build_rust_cli
build_swift_cli
echo ""

# ---------------------------------------------------------------------------
# Step 3: Run each config through both codegens and compare
# ---------------------------------------------------------------------------
run_one_config() {
  local config_file="$1"
  local idx="$2"
  local config_name
  config_name=$(basename "$config_file" .json)

  local test_dir="$WORK_DIR/run-$config_name"
  local rust_out="$test_dir/rust-out"
  local swift_out="$test_dir/swift-out"
  mkdir -p "$rust_out" "$swift_out"

  # The config's output paths are relative, so we rewrite them for each run.
  # Rust output
  local rust_config="$test_dir/config-rust.json"
  python3 -c "
import json, sys
cfg = json.load(open('$config_file'))
cfg['output']['schemaTypes']['path'] = '$rust_out/Generated'
ops = cfg['output']['operations']
if 'absolute' in ops:
    ops['absolute']['path'] = '$rust_out/Ops'
mocks = cfg['output']['testMocks']
if 'absolute' in mocks:
    mocks['absolute']['path'] = '$rust_out/Mocks'
json.dump(cfg, open('$rust_config', 'w'), indent=2)
"

  # Swift output
  local swift_config="$test_dir/config-swift.json"
  python3 -c "
import json, sys
cfg = json.load(open('$config_file'))
cfg['output']['schemaTypes']['path'] = '$swift_out/Generated'
ops = cfg['output']['operations']
if 'absolute' in ops:
    ops['absolute']['path'] = '$swift_out/Ops'
mocks = cfg['output']['testMocks']
if 'absolute' in mocks:
    mocks['absolute']['path'] = '$swift_out/Mocks'
json.dump(cfg, open('$swift_config', 'w'), indent=2)
"

  # Run Rust codegen
  local rust_ok=true
  if ! run_rust_codegen "$rust_config" 2>"$test_dir/rust-stderr.txt"; then
    rust_ok=false
  fi

  # Run Swift codegen
  local swift_ok=true
  if ! run_swift_codegen "$swift_config" 2>"$test_dir/swift-stderr.txt"; then
    swift_ok=false
  fi

  # Compare results
  if [ "$rust_ok" = false ] && [ "$swift_ok" = false ]; then
    echo -e "  ${YELLOW}SKIP${NC}: $config_name (both codegens failed)"
    echo "SKIP" >> "$RESULTS_FILE"
    return 0
  elif [ "$rust_ok" = false ]; then
    echo -e "  ${RED}FAIL${NC}: $config_name (Rust codegen failed, Swift succeeded)"
    if [ "$VERBOSE" = "1" ]; then
      echo "  Rust stderr:"
      head -5 "$test_dir/rust-stderr.txt" | sed 's/^/    /'
    fi
    echo "FAIL" >> "$RESULTS_FILE"
    return 1
  elif [ "$swift_ok" = false ]; then
    echo -e "  ${RED}FAIL${NC}: $config_name (Swift codegen failed, Rust succeeded)"
    if [ "$VERBOSE" = "1" ]; then
      echo "  Swift stderr:"
      head -5 "$test_dir/swift-stderr.txt" | sed 's/^/    /'
    fi
    echo "FAIL" >> "$RESULTS_FILE"
    return 1
  fi

  # Both succeeded — compare output
  if compare_generated "$rust_out" "$swift_out" "$config_name"; then
    echo "PASS" >> "$RESULTS_FILE"
  else
    echo "FAIL" >> "$RESULTS_FILE"
  fi
}

echo -e "${CYAN}Running $NUM_CONFIGS permutation tests...${NC}"
echo ""

for i in "${!CONFIG_FILES[@]}"; do
  config="${CONFIG_FILES[$i]}"
  echo -e "${CYAN}[$((i + 1))/$NUM_CONFIGS]${NC} $(basename "$config" .json)"
  run_one_config "$config" "$i" || true
done

# ---------------------------------------------------------------------------
# Step 4: Summary
# ---------------------------------------------------------------------------
echo ""
echo "================================"

TOTAL=$(wc -l < "$RESULTS_FILE" 2>/dev/null || echo 0)
TOTAL=$(echo "$TOTAL" | tr -d ' ')
PASSES=$(grep -c "^PASS$" "$RESULTS_FILE" 2>/dev/null || echo 0)
FAILS=$(grep -c "^FAIL$" "$RESULTS_FILE" 2>/dev/null || echo 0)
SKIPS=$(grep -c "^SKIP$" "$RESULTS_FILE" 2>/dev/null || echo 0)

if [ "$FAILS" -eq 0 ]; then
  echo -e "${GREEN}ALL $PASSES/$TOTAL PERMUTATIONS PASSED${NC} ($SKIPS skipped)"
else
  echo -e "${RED}$FAILS/$TOTAL PERMUTATIONS FAILED${NC} ($PASSES passed, $SKIPS skipped)"
fi
echo "================================"

exit "$FAILS"
