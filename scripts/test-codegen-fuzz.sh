#!/bin/bash
# test-codegen-fuzz.sh
#
# Use apollo-smith to generate random GraphQL schemas and operations, then run
# both Swift and Rust codegens on each and compare the output.
#
# Usage:
#   ./scripts/test-codegen-fuzz.sh                         # defaults: 20 schemas, medium
#   ./scripts/test-codegen-fuzz.sh -n 50 -s 42             # 50 schemas, seed 42
#   ./scripts/test-codegen-fuzz.sh -c large -v             # large complexity, verbose
#   ./scripts/test-codegen-fuzz.sh -k                      # keep temp dirs on failure

source "$(dirname "$0")/lib/codegen-test-utils.sh"

VERBOSE=${VERBOSE:-0}
KEEP_TMP=0
COUNT=20
SEED=0
COMPLEXITY="medium"

while getopts 'vkn:s:c:' OPTION; do
  case "$OPTION" in
    v) VERBOSE=1 ;;
    k) KEEP_TMP=1 ;;
    n) COUNT="$OPTARG" ;;
    s) SEED="$OPTARG" ;;
    c) COMPLEXITY="$OPTARG" ;;
    ?)
      echo "Usage: $(basename "$0") [-v] [-k] [-n count] [-s seed] [-c small|medium|large|huge]" >&2
      exit 1
      ;;
  esac
done
shift "$((OPTIND - 1))"

export VERBOSE

FUZZ_BIN="$REPO_ROOT/apollo-codegen-rs/target/release/apollo-codegen-fuzz"

# ---------------------------------------------------------------------------
# Create workspace
# ---------------------------------------------------------------------------
WORK_DIR=$(mktemp -d)
FUZZ_SCHEMAS_DIR="$WORK_DIR/schemas"
RESULTS_FILE="$WORK_DIR/results.txt"
mkdir -p "$FUZZ_SCHEMAS_DIR"

cleanup() {
  if [ "$KEEP_TMP" = "0" ]; then
    rm -rf "$WORK_DIR"
  else
    echo -e "${YELLOW}Temp directory preserved: $WORK_DIR${NC}"
  fi
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# Step 1: Build the fuzz binary
# ---------------------------------------------------------------------------
echo -e "${CYAN}Building fuzz test binary...${NC}"
(cd "$REPO_ROOT/apollo-codegen-rs" && cargo build --release -p apollo-codegen-fuzz-test 2>&1 | tail -3)
if [ ! -f "$FUZZ_BIN" ]; then
  echo -e "${RED}Failed to build fuzz binary at $FUZZ_BIN${NC}" >&2
  exit 1
fi
echo ""

# ---------------------------------------------------------------------------
# Step 2: Generate random schemas
# ---------------------------------------------------------------------------
echo -e "${CYAN}Generating $COUNT random schemas (seed=$SEED, complexity=$COMPLEXITY)...${NC}"
"$FUZZ_BIN" \
  --count "$COUNT" \
  --seed "$SEED" \
  --complexity "$COMPLEXITY" \
  --output-dir "$FUZZ_SCHEMAS_DIR" > /dev/null

# Discover generated case directories
CASE_DIRS=("$FUZZ_SCHEMAS_DIR"/case-*)
NUM_CASES=${#CASE_DIRS[@]}
echo -e "${CYAN}Generated $NUM_CASES test cases.${NC}"
echo ""

# ---------------------------------------------------------------------------
# Step 3: Build CLIs
# ---------------------------------------------------------------------------
build_rust_cli
build_swift_cli
echo ""

# ---------------------------------------------------------------------------
# Step 4: Run each case through both codegens
# ---------------------------------------------------------------------------
echo -e "${CYAN}Running $NUM_CASES fuzz test cases...${NC}"
echo ""

for i in "${!CASE_DIRS[@]}"; do
  case_dir="${CASE_DIRS[$i]}"
  case_name=$(basename "$case_dir")
  config_file="$case_dir/config.json"

  echo -e "${CYAN}[$((i + 1))/$NUM_CASES]${NC} $case_name"

  if [ ! -f "$config_file" ]; then
    echo -e "  ${YELLOW}SKIP${NC}: no config.json found"
    echo "SKIP" >> "$RESULTS_FILE"
    continue
  fi

  # Create separate output directories for each codegen
  rust_out="$case_dir/rust-out"
  swift_out="$case_dir/swift-out"
  mkdir -p "$rust_out/Generated" "$swift_out/Generated"

  # Rewrite config for Rust output
  rust_config="$case_dir/config-rust.json"
  python3 -c "
import json
cfg = json.load(open('$config_file'))
cfg['output']['schemaTypes']['path'] = '$rust_out/Generated'
json.dump(cfg, open('$rust_config', 'w'), indent=2)
"

  # Rewrite config for Swift output
  swift_config="$case_dir/config-swift.json"
  python3 -c "
import json
cfg = json.load(open('$config_file'))
cfg['output']['schemaTypes']['path'] = '$swift_out/Generated'
json.dump(cfg, open('$swift_config', 'w'), indent=2)
"

  # Run Rust codegen
  rust_ok=true
  if ! run_rust_codegen "$rust_config" 2>"$case_dir/rust-stderr.txt"; then
    rust_ok=false
  fi

  # Run Swift codegen
  swift_ok=true
  if ! run_swift_codegen "$swift_config" 2>"$case_dir/swift-stderr.txt"; then
    swift_ok=false
  fi

  # Evaluate results
  if [ "$rust_ok" = false ] && [ "$swift_ok" = false ]; then
    echo -e "  ${YELLOW}SKIP${NC}: both codegens failed (likely invalid schema)"
    echo "SKIP" >> "$RESULTS_FILE"
  elif [ "$rust_ok" = false ]; then
    echo -e "  ${RED}FAIL${NC}: Rust codegen failed, Swift succeeded"
    if [ "$VERBOSE" = "1" ]; then
      echo "  Rust stderr:"
      head -5 "$case_dir/rust-stderr.txt" | sed 's/^/    /'
    fi
    echo "FAIL" >> "$RESULTS_FILE"
  elif [ "$swift_ok" = false ]; then
    echo -e "  ${RED}FAIL${NC}: Swift codegen failed, Rust succeeded"
    if [ "$VERBOSE" = "1" ]; then
      echo "  Swift stderr:"
      head -5 "$case_dir/swift-stderr.txt" | sed 's/^/    /'
    fi
    echo "FAIL" >> "$RESULTS_FILE"
  else
    # Both succeeded — compare output
    if compare_generated "$rust_out" "$swift_out" "$case_name"; then
      echo "PASS" >> "$RESULTS_FILE"
    else
      echo "FAIL" >> "$RESULTS_FILE"
    fi
  fi
done

# ---------------------------------------------------------------------------
# Step 5: Summary
# ---------------------------------------------------------------------------
echo ""
echo "================================"

TOTAL=$(wc -l < "$RESULTS_FILE" 2>/dev/null || echo 0)
TOTAL=$(echo "$TOTAL" | tr -d ' ')
PASSES=$(grep -c "^PASS$" "$RESULTS_FILE" 2>/dev/null || echo 0)
FAILS=$(grep -c "^FAIL$" "$RESULTS_FILE" 2>/dev/null || echo 0)
SKIPS=$(grep -c "^SKIP$" "$RESULTS_FILE" 2>/dev/null || echo 0)

if [ "$FAILS" -eq 0 ]; then
  echo -e "${GREEN}FUZZ: ALL $PASSES/$TOTAL CASES PASSED${NC} ($SKIPS skipped)"
else
  echo -e "${RED}FUZZ: $FAILS/$TOTAL CASES FAILED${NC} ($PASSES passed, $SKIPS skipped)"
fi
echo "Seed: $SEED  Complexity: $COMPLEXITY"
echo "================================"

exit "$FAILS"
