#!/bin/bash
# Master test runner for Rust codegen compatibility.
#
# Ties together all Rust codegen test scripts into a single entry point
# with quick, full, and bench modes.
#
# Usage: scripts/test-rust-codegen.sh [--quick|--full|--bench]
#
#   --quick  (default) Run config and API comparison tests
#   --full   Run all tests including permutations and fuzz
#   --bench  Run full suite plus performance benchmarks

set -euo pipefail

source "$(dirname "$0")/lib/codegen-test-utils.sh"

MODE="${1:---quick}"

echo "================================"
echo " Rust Codegen Test Runner"
echo " Mode: $MODE"
echo "================================"
echo ""

build_rust_cli

case "$MODE" in
  --quick)
    echo -e "${CYAN}=== Quick Test Suite ===${NC}"
    echo ""

    if [ -x "$REPO_ROOT/scripts/test-codegen-configs.sh" ]; then
      echo -e "${CYAN}--- Config Tests ---${NC}"
      "$REPO_ROOT/scripts/test-codegen-configs.sh"
      echo ""
    else
      echo -e "${YELLOW}Skipping test-codegen-configs.sh (not found or not executable)${NC}"
    fi

    if [ -x "$REPO_ROOT/scripts/test-codegen-apis.sh" ]; then
      echo -e "${CYAN}--- API Tests ---${NC}"
      "$REPO_ROOT/scripts/test-codegen-apis.sh"
      echo ""
    else
      echo -e "${YELLOW}Skipping test-codegen-apis.sh (not found or not executable)${NC}"
    fi
    ;;

  --full)
    echo -e "${CYAN}=== Full Test Suite ===${NC}"
    echo ""

    if [ -x "$REPO_ROOT/scripts/test-codegen-configs.sh" ]; then
      echo -e "${CYAN}--- Config Tests ---${NC}"
      "$REPO_ROOT/scripts/test-codegen-configs.sh"
      echo ""
    else
      echo -e "${YELLOW}Skipping test-codegen-configs.sh (not found or not executable)${NC}"
    fi

    if [ -x "$REPO_ROOT/scripts/test-codegen-apis.sh" ]; then
      echo -e "${CYAN}--- API Tests ---${NC}"
      "$REPO_ROOT/scripts/test-codegen-apis.sh"
      echo ""
    else
      echo -e "${YELLOW}Skipping test-codegen-apis.sh (not found or not executable)${NC}"
    fi

    if [ -x "$REPO_ROOT/scripts/test-codegen-permutations.sh" ]; then
      echo -e "${CYAN}--- Permutation Tests ---${NC}"
      "$REPO_ROOT/scripts/test-codegen-permutations.sh"
      echo ""
    else
      echo -e "${YELLOW}Skipping test-codegen-permutations.sh (not found or not executable)${NC}"
    fi

    if [ -x "$REPO_ROOT/scripts/test-codegen-fuzz.sh" ]; then
      echo -e "${CYAN}--- Fuzz Tests ---${NC}"
      "$REPO_ROOT/scripts/test-codegen-fuzz.sh"
      echo ""
    else
      echo -e "${YELLOW}Skipping test-codegen-fuzz.sh (not found or not executable)${NC}"
    fi
    ;;

  --bench)
    echo -e "${CYAN}=== Full Test Suite + Benchmarks ===${NC}"
    echo ""

    if [ -x "$REPO_ROOT/scripts/test-codegen-configs.sh" ]; then
      echo -e "${CYAN}--- Config Tests ---${NC}"
      "$REPO_ROOT/scripts/test-codegen-configs.sh"
      echo ""
    else
      echo -e "${YELLOW}Skipping test-codegen-configs.sh (not found or not executable)${NC}"
    fi

    if [ -x "$REPO_ROOT/scripts/test-codegen-apis.sh" ]; then
      echo -e "${CYAN}--- API Tests ---${NC}"
      "$REPO_ROOT/scripts/test-codegen-apis.sh"
      echo ""
    else
      echo -e "${YELLOW}Skipping test-codegen-apis.sh (not found or not executable)${NC}"
    fi

    if [ -x "$REPO_ROOT/scripts/test-codegen-permutations.sh" ]; then
      echo -e "${CYAN}--- Permutation Tests ---${NC}"
      "$REPO_ROOT/scripts/test-codegen-permutations.sh"
      echo ""
    else
      echo -e "${YELLOW}Skipping test-codegen-permutations.sh (not found or not executable)${NC}"
    fi

    if [ -x "$REPO_ROOT/scripts/test-codegen-fuzz.sh" ]; then
      echo -e "${CYAN}--- Fuzz Tests ---${NC}"
      "$REPO_ROOT/scripts/test-codegen-fuzz.sh"
      echo ""
    else
      echo -e "${YELLOW}Skipping test-codegen-fuzz.sh (not found or not executable)${NC}"
    fi

    echo -e "${CYAN}--- Benchmarks ---${NC}"
    "$REPO_ROOT/scripts/bench-codegen.sh" --skip-build
    echo ""
    ;;

  *)
    echo "Usage: $0 [--quick|--full|--bench]"
    echo ""
    echo "  --quick  (default) Run config and API comparison tests"
    echo "  --full   Run all tests including permutations and fuzz"
    echo "  --bench  Run full suite plus performance benchmarks"
    exit 1
    ;;
esac

print_summary
