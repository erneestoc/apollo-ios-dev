#!/bin/bash
# Performance benchmark comparing Swift vs Rust codegen.
#
# Runs hyperfine benchmarks across multiple schema sizes and prints
# a markdown results table with speedup ratios.
#
# Usage: scripts/bench-codegen.sh [--skip-build]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$REPO_ROOT/scripts/lib/codegen-test-utils.sh"

SKIP_BUILD=false
for arg in "$@"; do
  case "$arg" in
    --skip-build) SKIP_BUILD=true ;;
  esac
done

# ── Check for hyperfine ─────────────────────────────────────────────────────
if ! command -v hyperfine &>/dev/null; then
  echo -e "${YELLOW}hyperfine is not installed.${NC}"
  echo ""
  echo "Install it with:"
  echo "  brew install hyperfine"
  echo ""
  read -rp "Install now? [y/N] " answer
  if [[ "$answer" =~ ^[Yy]$ ]]; then
    brew install hyperfine
  else
    echo "Aborting. hyperfine is required for benchmarking."
    exit 1
  fi
fi

# ── Build both CLIs in release mode ─────────────────────────────────────────
if [ "$SKIP_BUILD" = false ]; then
  echo -e "${CYAN}=== Building CLIs ===${NC}"
  build_rust_cli
  build_swift_cli
fi

SWIFT_CLI="$SWIFT_CLI_DIR/.build/release/apollo-ios-cli"
if [ ! -f "$SWIFT_CLI" ]; then
  echo -e "${RED}Swift CLI not found at $SWIFT_CLI${NC}"
  echo "Run: cd $SWIFT_CLI_DIR && swift build --product apollo-ios-cli -c release"
  exit 1
fi

if [ ! -f "$RUST_CLI" ]; then
  echo -e "${RED}Rust CLI not found at $RUST_CLI${NC}"
  echo "Run: cd $REPO_ROOT/apollo-codegen-rs && cargo build --release -p apollo-codegen-cli"
  exit 1
fi

# ── Prepare benchmark staging area ──────────────────────────────────────────
BENCH_DIR=$(mktemp -d)
RESULTS_DIR=$(mktemp -d)
trap "rm -rf '$BENCH_DIR' '$RESULTS_DIR'" EXIT

echo -e "${CYAN}Benchmark staging directory: $BENCH_DIR${NC}"
echo -e "${CYAN}Results directory: $RESULTS_DIR${NC}"
echo ""

# ── Helper: create a benchmark config ───────────────────────────────────────
# Args: $1=bench_name $2=schema_namespace $3=schema_path $4=operations_glob
create_bench_config() {
  local bench_name="$1"
  local namespace="$2"
  local schema_path="$3"
  local ops_glob="$4"
  local bench_work_dir="$BENCH_DIR/$bench_name"
  local output_dir="$bench_work_dir/output"

  mkdir -p "$bench_work_dir" "$output_dir"

  cat > "$bench_work_dir/apollo-codegen-config.json" <<EOF
{
  "schemaNamespace": "$namespace",
  "input": {
    "schemaSearchPaths": ["$schema_path"],
    "operationSearchPaths": ["$ops_glob"]
  },
  "output": {
    "schemaTypes": {
      "path": "$output_dir/$namespace",
      "moduleType": {
        "swiftPackageManager": {}
      }
    },
    "operations": {
      "inSchemaModule": {}
    },
    "testMocks": {
      "none": {}
    }
  },
  "options": {
    "pruneGeneratedFiles": true
  }
}
EOF

  echo "$bench_work_dir"
}

# ── Setup benchmarks ────────────────────────────────────────────────────────
declare -a BENCH_NAMES
declare -A BENCH_DIRS

# bench-small: AnimalKingdomAPI (14 operations, simple schema)
BENCH_DIRS[small]=$(create_bench_config \
  "small" \
  "AnimalKingdomAPI" \
  "$REPO_ROOT/Sources/AnimalKingdomAPI/animalkingdom-graphql/AnimalSchema.graphqls" \
  "$REPO_ROOT/Sources/AnimalKingdomAPI/animalkingdom-graphql/*.graphql")
BENCH_NAMES+=("small")

# bench-medium: StarWarsAPI (18 operations)
BENCH_DIRS[medium]=$(create_bench_config \
  "medium" \
  "StarWarsAPI" \
  "$REPO_ROOT/Sources/StarWarsAPI/starwars-graphql/schema.graphqls" \
  "$REPO_ROOT/Sources/StarWarsAPI/starwars-graphql/**/*.graphql")
BENCH_NAMES+=("medium")

# bench-large: GitHubAPI (3 operations, 38K-line schema, 232 output files)
BENCH_DIRS[large]=$(create_bench_config \
  "large" \
  "GitHubAPI" \
  "$REPO_ROOT/Sources/GitHubAPI/graphql/schema.graphqls" \
  "$REPO_ROOT/Sources/GitHubAPI/graphql/**/*.graphql")
BENCH_NAMES+=("large")

# bench-huge: Pre-generated huge schema
HUGE_SCHEMA_DIR="$REPO_ROOT/apollo-codegen-rs/benches/huge_schema"
if [ ! -f "$HUGE_SCHEMA_DIR/schema.graphqls" ]; then
  echo -e "${CYAN}Generating huge benchmark schema...${NC}"
  mkdir -p "$HUGE_SCHEMA_DIR"
  python3 "$REPO_ROOT/scripts/lib/generate-huge-schema.py" "$HUGE_SCHEMA_DIR"
  echo ""
fi

BENCH_DIRS[huge]=$(create_bench_config \
  "huge" \
  "HugeSchemaAPI" \
  "$HUGE_SCHEMA_DIR/schema.graphqls" \
  "$HUGE_SCHEMA_DIR/operations.graphql")
BENCH_NAMES+=("huge")

# ── Run benchmarks ──────────────────────────────────────────────────────────
echo -e "${CYAN}=== Running Benchmarks ===${NC}"
echo ""

for name in "${BENCH_NAMES[@]}"; do
  bench_dir="${BENCH_DIRS[$name]}"
  config="$bench_dir/apollo-codegen-config.json"

  echo -e "${CYAN}--- bench-$name ---${NC}"

  # Clean output dir between runs
  output_dir="$bench_dir/output"

  hyperfine \
    --warmup 2 \
    --min-runs 5 \
    --export-json "$RESULTS_DIR/bench-$name.json" \
    --prepare "rm -rf '$output_dir'; mkdir -p '$output_dir'" \
    --command-name "swift" \
    "$SWIFT_CLI generate -p '$config'" \
    --prepare "rm -rf '$output_dir'; mkdir -p '$output_dir'" \
    --command-name "rust" \
    "$RUST_CLI generate --path '$config'" \
    2>&1

  echo ""
done

# ── Parse results and print markdown table ──────────────────────────────────
echo -e "${CYAN}=== Results ===${NC}"
echo ""

# Use Python to parse hyperfine JSON and format the table
python3 - "$RESULTS_DIR" "${BENCH_NAMES[@]}" <<'PYEOF'
import json
import os
import sys

results_dir = sys.argv[1]
bench_names = sys.argv[2:]

rows = []
for name in bench_names:
    json_path = os.path.join(results_dir, f"bench-{name}.json")
    if not os.path.exists(json_path):
        rows.append((name, "N/A", "N/A", "N/A"))
        continue

    with open(json_path) as f:
        data = json.load(f)

    results = data.get("results", [])
    swift_result = None
    rust_result = None

    for r in results:
        cmd_name = r.get("command", "")
        if "swift" in cmd_name.lower() or r.get("command_name", "") == "swift":
            swift_result = r
        elif "rust" in cmd_name.lower() or r.get("command_name", "") == "rust":
            rust_result = r

    if not swift_result or not rust_result:
        # Fallback: first is swift, second is rust
        if len(results) >= 2:
            swift_result = results[0]
            rust_result = results[1]
        else:
            rows.append((name, "N/A", "N/A", "N/A"))
            continue

    swift_mean = swift_result["mean"]
    swift_stddev = swift_result["stddev"]
    rust_mean = rust_result["mean"]
    rust_stddev = rust_result["stddev"]

    speedup = swift_mean / rust_mean if rust_mean > 0 else float("inf")

    swift_str = f"{swift_mean:.2f}s +/- {swift_stddev:.3f}s"
    rust_str = f"{rust_mean:.2f}s +/- {rust_stddev:.3f}s"
    speedup_str = f"{speedup:.1f}x"

    rows.append((name, swift_str, rust_str, speedup_str))

# Print markdown table
print("| Benchmark | Swift (mean) | Rust (mean) | Speedup |")
print("|-----------|-------------|-------------|---------|")
for name, swift_str, rust_str, speedup_str in rows:
    print(f"| {name:<9} | {swift_str:<11} | {rust_str:<11} | {speedup_str:<7} |")

PYEOF

echo ""
echo -e "${GREEN}Benchmark complete.${NC}"
echo "Raw results saved to: $RESULTS_DIR/bench-*.json"
