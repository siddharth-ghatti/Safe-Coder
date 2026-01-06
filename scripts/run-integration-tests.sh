#!/bin/bash

# Integration test runner for safe-coder
# This script runs all integration tests with proper setup and reporting

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
CARGO_BUILD_PROFILE=${CARGO_BUILD_PROFILE:-debug}
RUST_LOG=${RUST_LOG:-warn}
TEST_THREADS=${TEST_THREADS:-1}  # Serial execution for integration tests

echo -e "${GREEN}🧪 Safe-Coder Integration Test Runner${NC}"
echo "================================="

# Build the project first
echo -e "${YELLOW}📦 Building safe-coder...${NC}"
if ! cargo build --profile="$CARGO_BUILD_PROFILE"; then
    echo -e "${RED}❌ Build failed!${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Build successful${NC}"

# Check for required tools
echo -e "${YELLOW}🔍 Checking dependencies...${NC}"

required_tools=("git")
for tool in "${required_tools[@]}"; do
    if ! command -v "$tool" &> /dev/null; then
        echo -e "${RED}❌ Required tool '$tool' not found${NC}"
        exit 1
    fi
done

echo -e "${GREEN}✅ Dependencies check passed${NC}"

# Run tests with proper configuration
echo -e "${YELLOW}🚀 Running integration tests...${NC}"

export RUST_LOG="$RUST_LOG"
export NO_COLOR=1  # Disable colored output for cleaner test logs

# Test modules to run
test_modules=(
    "cli_tests"
    "config_tests" 
    "llm_tests"
    "tools_tests"
    "git_tests"
    "session_tests"
    "orchestrator_tests"
)

# Run individual test modules
total_modules=${#test_modules[@]}
passed_modules=0
failed_modules=()

for module in "${test_modules[@]}"; do
    echo ""
    echo -e "${YELLOW}🧪 Running ${module}...${NC}"
    
    if cargo test \
        --test integration \
        "$module" \
        --test-threads="$TEST_THREADS" \
        -- --nocapture; then
        echo -e "${GREEN}✅ ${module} passed${NC}"
        ((passed_modules++))
    else
        echo -e "${RED}❌ ${module} failed${NC}"
        failed_modules+=("$module")
    fi
done

# Summary
echo ""
echo "================================="
echo -e "${GREEN}📊 Test Summary${NC}"
echo "================================="
echo "Total modules: $total_modules"
echo "Passed: $passed_modules"
echo "Failed: ${#failed_modules[@]}"

if [ ${#failed_modules[@]} -eq 0 ]; then
    echo ""
    echo -e "${GREEN}🎉 All integration tests passed!${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}💥 Failed modules:${NC}"
    for module in "${failed_modules[@]}"; do
        echo "  - $module"
    done
    echo ""
    echo -e "${RED}❌ Some integration tests failed${NC}"
    exit 1
fi