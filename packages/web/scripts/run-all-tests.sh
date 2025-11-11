#!/bin/bash

# Run All Tests Script
# This script runs all tests in the correct order and provides a summary

set -e

echo "🧪 Starting comprehensive test suite..."
echo "======================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Track results
RESULTS=()
FAILED_TESTS=()

# Function to run a test and track results
run_test() {
  local test_name="$1"
  local test_command="$2"
  local optional="${3:-false}"
  
  echo -e "${BLUE}Running ${test_name}...${NC}"
  
  if eval "$test_command"; then
    echo -e "${GREEN}✅ ${test_name} passed${NC}"
    RESULTS+=("✅ $test_name")
  else
    if [ "$optional" = "true" ]; then
      echo -e "${YELLOW}⚠️ ${test_name} failed (optional)${NC}"
      RESULTS+=("⚠️ $test_name (optional)")
    else
      echo -e "${RED}❌ ${test_name} failed${NC}"
      RESULTS+=("❌ $test_name")
      FAILED_TESTS+=("$test_name")
    fi
  fi
  
  echo ""
}

# 1. Code Quality Checks
echo "📝 Code Quality Checks"
echo "-----------------------"
run_test "Code Formatting" "pnpm fmt.check"
run_test "ESLint" "pnpm lint"
run_test "TypeScript Compilation" "pnpm build.types"

# 2. Unit Tests
echo "🔬 Unit Tests"
echo "-------------"
run_test "Unit Tests" "pnpm test:run"
run_test "Test Coverage" "pnpm test:coverage"

# 3. Build Tests
echo "🏗️ Build Tests"
echo "---------------"
run_test "Production Build" "pnpm build"

# 4. Security Tests
echo "🔒 Security Tests"
echo "----------------"
run_test "Security Audit" "pnpm audit --audit-level moderate" "true"

# 5. E2E Tests (if not in CI or if specifically requested)
if [ "$RUN_E2E" = "true" ] || [ -z "$CI" ]; then
  echo "🌐 End-to-End Tests"
  echo "-------------------"
  
  # Install Playwright browsers if needed
  if ! command -v playwright &> /dev/null; then
    echo "Installing Playwright browsers..."
    pnpm exec playwright install --with-deps
  fi
  
  run_test "E2E Tests" "pnpm test:e2e" "true"
fi

# Summary
echo "======================================="
echo "📊 Test Summary"
echo "======================================="

for result in "${RESULTS[@]}"; do
  echo -e "$result"
done

echo ""

if [ ${#FAILED_TESTS[@]} -eq 0 ]; then
  echo -e "${GREEN}🎉 All tests passed successfully!${NC}"
  exit 0
else
  echo -e "${RED}💥 ${#FAILED_TESTS[@]} test(s) failed:${NC}"
  for failed_test in "${FAILED_TESTS[@]}"; do
    echo -e "  ${RED}- $failed_test${NC}"
  done
  echo ""
  echo -e "${RED}Please fix the failing tests before proceeding.${NC}"
  exit 1
fi