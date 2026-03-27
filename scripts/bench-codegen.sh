#!/bin/bash
# Performance benchmark comparing Swift vs Rust codegen.
#
# Uses hyperfine for accurate timing across multiple schema sizes.
#
# Usage:
#   scripts/bench-codegen.sh              # run all benchmarks
#   scripts/bench-codegen.sh --skip-build # skip building CLIs

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$REPO_ROOT/scripts/lib/codegen-test-utils.sh"

SKIP_BUILD=false
for arg in "$@"; do
  case "$arg" in
    --skip-build) SKIP_BUILD=true ;;
  esac
done

# Check for hyperfine
if ! command -v hyperfine &>/dev/null; then
  echo -e "${YELLOW}hyperfine is not installed. Install with: brew install hyperfine${NC}"
  exit 1
fi

# Build CLIs
if [ "$SKIP_BUILD" = false ]; then
  build_rust_cli
  build_swift_cli
fi

SWIFT_CLI="$SWIFT_CLI_DIR/.build/release/apollo-ios-cli"
if [ ! -f "$SWIFT_CLI" ]; then
  echo -e "${RED}Swift CLI not found. Building...${NC}"
  (cd "$SWIFT_CLI_DIR" && swift build --product apollo-ios-cli -c release 2>&1 | tail -1)
fi

echo ""
echo "================================"
echo " Codegen Performance Benchmarks"
echo "================================"
echo ""

BENCH_DIR=$(mktemp -d)
RESULTS_DIR=$(mktemp -d)
trap "rm -rf '$BENCH_DIR' '$RESULTS_DIR'" EXIT

# Helper: create a benchmark config
create_config() {
  local name="$1" namespace="$2" schema="$3" ops="$4"
  local dir="$BENCH_DIR/$name"
  local out="$dir/output"
  mkdir -p "$dir" "$out"

  cat > "$dir/config.json" <<EOF
{
  "schemaNamespace": "$namespace",
  "input": {
    "schemaSearchPaths": ["$schema"],
    "operationSearchPaths": ["$ops"]
  },
  "output": {
    "schemaTypes": { "path": "$out/$namespace", "moduleType": { "swiftPackageManager": {} } },
    "operations": { "inSchemaModule": {} },
    "testMocks": { "none": {} }
  },
  "options": { "pruneGeneratedFiles": true }
}
EOF
  echo "$dir"
}

# Setup schemas
SMALL=$(create_config "small" "AnimalKingdomAPI" \
  "$REPO_ROOT/Sources/AnimalKingdomAPI/animalkingdom-graphql/AnimalSchema.graphqls" \
  "$REPO_ROOT/Sources/AnimalKingdomAPI/animalkingdom-graphql/**/*.graphql")

MEDIUM=$(create_config "medium" "StarWarsAPI" \
  "$REPO_ROOT/Sources/StarWarsAPI/starwars-graphql/schema.graphqls" \
  "$REPO_ROOT/Sources/StarWarsAPI/starwars-graphql/**/*.graphql")

LARGE=$(create_config "large" "GitHubAPI" \
  "$REPO_ROOT/Sources/GitHubAPI/graphql/schema.graphqls" \
  "$REPO_ROOT/Sources/GitHubAPI/graphql/**/*.graphql")

HUGE_DIR="$REPO_ROOT/apollo-codegen-rs/benches/huge_schema"
HUGE=""
if [ -f "$HUGE_DIR/schema.graphqls" ]; then
  HUGE=$(create_config "huge" "HugeSchemaAPI" \
    "$HUGE_DIR/schema.graphqls" \
    "$HUGE_DIR/operations.graphql")
fi

# Run benchmarks
for entry in "small:$SMALL" "medium:$MEDIUM" "large:$LARGE" "huge:$HUGE"; do
  name="${entry%%:*}"
  dir="${entry#*:}"

  if [ -z "$dir" ]; then continue; fi

  config="$dir/config.json"
  output="$dir/output"

  echo -e "${CYAN}--- bench-$name ---${NC}"

  hyperfine \
    --warmup 2 \
    --min-runs 5 \
    --export-json "$RESULTS_DIR/bench-$name.json" \
    --prepare "rm -rf '$output'; mkdir -p '$output'" \
    -n "rust"  "$RUST_CLI generate --path '$config'" \
    --prepare "rm -rf '$output'; mkdir -p '$output'" \
    -n "swift" "$SWIFT_CLI generate -p '$config' --ignore-version-mismatch" \
    2>&1

  echo ""
done

# Print results table
echo -e "${CYAN}=== Results Summary ===${NC}"
echo ""

python3 - "$RESULTS_DIR" <<'PYEOF'
import json, os, sys, glob

results_dir = sys.argv[1]
rows = []

for f in sorted(glob.glob(os.path.join(results_dir, "bench-*.json"))):
    name = os.path.basename(f).replace("bench-", "").replace(".json", "")
    data = json.load(open(f))
    results = {r.get("command", r.get("command_name", "")): r for r in data.get("results", [])}

    # Find by command_name
    rust = swift = None
    for r in data.get("results", []):
        cn = r.get("command_name", "")
        if cn == "rust": rust = r
        elif cn == "swift": swift = r

    if not rust or not swift:
        rows.append((name, "N/A", "N/A", "N/A"))
        continue

    speedup = swift["mean"] / rust["mean"] if rust["mean"] > 0 else 0
    rows.append((
        name,
        f'{rust["mean"]*1000:.1f}ms',
        f'{swift["mean"]*1000:.1f}ms',
        f'{speedup:.1f}x'
    ))

print(f"{'Schema':<12} {'Rust':>10} {'Swift':>10} {'Speedup':>10}")
print(f"{'------':<12} {'----':>10} {'-----':>10} {'-------':>10}")
for name, rust_s, swift_s, speedup_s in rows:
    print(f"{name:<12} {rust_s:>10} {swift_s:>10} {speedup_s:>10}")

PYEOF

echo ""
echo "================================"
echo " Benchmark complete"
echo "================================"
