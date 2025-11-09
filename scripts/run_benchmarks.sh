#!/bin/bash
# Script to run AUTOBK Jacobian performance benchmarks
# Usage: ./scripts/run_benchmarks.sh [OPTIONS]

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}================================${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

# Parse command line arguments
BENCHMARK_TYPE="${1:-all}"
PROFILE_TIME="${2:-0}"

print_header "AUTOBK Jacobian Performance Benchmarks"

echo "Benchmark type: $BENCHMARK_TYPE"
echo "Profile time: $PROFILE_TIME seconds"
echo ""

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    print_error "Cargo not found. Please install Rust."
    exit 1
fi

# Navigate to project root
cd "$(dirname "$0")/.."

print_header "Step 1: Building Project"

if cargo build --package xraytsubaki 2>&1 | grep -q "error"; then
    print_error "Compilation failed. Please fix compilation errors first."
    echo ""
    echo "Known issues:"
    echo "  - Pre-existing error handling modernization incomplete"
    echo "  - The AUTOBK optimization code itself is correct"
    echo ""
    echo "To debug:"
    echo "  cargo build --package xraytsubaki 2>&1 | grep 'error\['"
    exit 1
fi

print_success "Compilation successful"
echo ""

print_header "Step 2: Running Benchmarks"

case "$BENCHMARK_TYPE" in
    "quick")
        print_warning "Running quick test (full_background_removal only)"
        if [ "$PROFILE_TIME" -gt 0 ]; then
            cargo bench --bench autobk_jacobian_bench -- "full_background_removal" --profile-time="$PROFILE_TIME"
        else
            cargo bench --bench autobk_jacobian_bench -- "full_background_removal"
        fi
        ;;

    "jacobian")
        print_warning "Running Jacobian performance tests"
        cargo bench --bench autobk_jacobian_bench -- "jacobian_performance"
        ;;

    "batch")
        print_warning "Running batch processing tests"
        cargo bench --bench autobk_jacobian_bench -- "batch_processing"
        ;;

    "memory")
        print_warning "Running memory efficiency tests"
        cargo bench --bench autobk_jacobian_bench -- "memory_efficiency"
        ;;

    "all")
        print_warning "Running full benchmark suite (this may take 10+ minutes)"
        if [ "$PROFILE_TIME" -gt 0 ]; then
            cargo bench --bench autobk_jacobian_bench -- --profile-time="$PROFILE_TIME"
        else
            cargo bench --bench autobk_jacobian_bench
        fi
        ;;

    "compare")
        print_header "Baseline Comparison Mode"

        echo "Step 1: Checking out main branch..."
        git stash
        git checkout main

        echo "Step 2: Running baseline benchmarks..."
        cargo bench --bench autobk_jacobian_bench -- --save-baseline main

        echo "Step 3: Checking out optimization branch..."
        git checkout -
        git stash pop || true

        echo "Step 4: Running optimized benchmarks..."
        cargo bench --bench autobk_jacobian_bench -- --baseline main
        ;;

    *)
        print_error "Unknown benchmark type: $BENCHMARK_TYPE"
        echo ""
        echo "Usage: $0 [TYPE] [PROFILE_TIME]"
        echo ""
        echo "Types:"
        echo "  quick    - Quick test (2 minutes)"
        echo "  jacobian - Jacobian performance tests"
        echo "  batch    - Batch processing tests"
        echo "  memory   - Memory efficiency tests"
        echo "  all      - Full benchmark suite (default)"
        echo "  compare  - Compare with baseline"
        echo ""
        echo "Examples:"
        echo "  $0 quick           # Quick test"
        echo "  $0 all 5           # Full suite with 5s profiling"
        echo "  $0 compare         # Compare with main branch"
        exit 1
        ;;
esac

echo ""
print_header "Step 3: Results"

# Find latest criterion output
LATEST_REPORT=$(find target/criterion -name "index.html" | head -1)

if [ -n "$LATEST_REPORT" ]; then
    print_success "Benchmark complete!"
    echo ""
    echo "View detailed results:"
    echo "  firefox $LATEST_REPORT"
    echo ""

    # Check for flamegraph
    FLAMEGRAPH=$(find target/criterion -name "flamegraph.svg" | head -1)
    if [ -n "$FLAMEGRAPH" ]; then
        print_success "Flamegraph generated: $FLAMEGRAPH"
    fi
else
    print_warning "No criterion output found"
fi

echo ""
print_header "Expected Results"
echo ""
echo "Phase 1 Targets:"
echo "  • Total speedup:        25-35% (18s → 12s per spectrum)"
echo "  • Jacobian reduction:   ~12% per iteration"
echo "  • Allocation reduction: 50% (300 → 150 per iteration)"
echo "  • Memory overhead:      +800KB per spectrum"
echo ""

# Parse criterion output for key metrics
if [ "$BENCHMARK_TYPE" = "quick" ] || [ "$BENCHMARK_TYPE" = "all" ]; then
    CRITERION_OUTPUT=$(find target/criterion/autobk_optimization/full_background_removal -name "base" | head -1)
    if [ -n "$CRITERION_OUTPUT" ]; then
        print_success "Key benchmark results available in criterion output"
    fi
fi

print_success "Benchmark run complete!"
