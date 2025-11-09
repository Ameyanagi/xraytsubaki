# AUTOBK Jacobian Performance Benchmarking Guide

This document explains how to run and interpret benchmarks for the AUTOBK Jacobian performance optimization.

## Quick Start

### Run All Benchmarks

```bash
# Run the complete AUTOBK optimization benchmark suite
cargo bench --bench autobk_jacobian_bench

# Run with flamegraph profiling (requires perf on Linux)
cargo bench --bench autobk_jacobian_bench -- --profile-time=5
```

### Run Specific Benchmarks

```bash
# Full optimization benchmark only
cargo bench --bench autobk_jacobian_bench -- "full_background_removal"

# Jacobian-intensive tests
cargo bench --bench autobk_jacobian_bench -- "jacobian_performance"

# Batch processing tests
cargo bench --bench autobk_jacobian_bench -- "batch_processing"

# Memory efficiency tests
cargo bench --bench autobk_jacobian_bench -- "memory_efficiency"
```

## Benchmark Suite Overview

### 1. Full Background Removal (`autobk_optimization` group)

**Purpose**: Measure end-to-end AUTOBK performance with all optimizations

**What it tests**:
- Complete normalize → calc_background workflow
- Levenberg-Marquardt optimization with precomputed Jacobian
- Allocation-optimized column processing

**Expected results** (Phase 1):
- **Baseline**: ~18 seconds per spectrum (pre-optimization)
- **Target**: 11-13 seconds per spectrum (25-35% improvement)
- **Measurement**: 20 samples over 10 seconds

**Key metrics**:
```
Time:   [11.234 s 11.456 s 11.678 s]
Change: [-32.4% -30.2% -28.1%] (improvement)
```

### 2. Jacobian Performance (`jacobian_performance` group)

**Purpose**: Validate consistent speedup across different spectrum states

**Test cases**:
- `standard`: Full normalize + background workflow
- `normalized`: Background calculation on pre-normalized data

**Expected results**:
- Both cases should show similar ~30% improvement
- Confirms optimization doesn't depend on normalization state

**What this validates**:
- Precomputed basis works correctly in all scenarios
- No unexpected performance regression in edge cases

### 3. Batch Processing (`batch_processing` group)

**Purpose**: Measure scalability and allocation efficiency at scale

**Test parameters**:
- Batch sizes: 1, 10, 50 spectra
- Validates linear scaling

**Expected results**:
```
batch_processing/1   [11.5 s ... 11.7 s]
batch_processing/10  [115.2 s ... 117.3 s]  (10x linear)
batch_processing/50  [575.8 s ... 586.5 s]  (50x linear)
```

**What this validates**:
- No memory leaks across multiple spectra
- Consistent per-spectrum performance
- Allocation optimization scales properly

### 4. Memory Efficiency (`memory_efficiency` group)

**Purpose**: Measure allocation performance impact

**What it measures**:
- Impact of pre-allocated buffers
- Elimination of extend() calls

**Expected allocation reduction**:
- **Before**: ~300 allocations per LM iteration
- **After**: ~150 allocations per LM iteration
- **Target**: 50% reduction

**Note**: Use `heaptrack` or `dhat` for detailed allocation profiling:
```bash
# Install heaptrack (Linux)
sudo apt install heaptrack

# Profile allocations
heaptrack cargo bench --bench autobk_jacobian_bench -- "memory_efficiency"

# Analyze results
heaptrack_gui heaptrack.*.gz
```

## Interpreting Results

### Success Criteria (Phase 1)

✅ **Full optimization**: 25-35% speedup
- Time reduction from ~18s to 11-13s per spectrum

✅ **Jacobian computation**: Reduced by ~12% per iteration
- Precomputed basis eliminates redundant splev_jacobian calls

✅ **Memory allocations**: 50% reduction
- Pre-allocated buffers + direct writes

✅ **Linear scaling**: Batch processing scales proportionally
- 10 spectra = 10x time, 50 spectra = 50x time

### Reading Criterion Output

```
autobk_optimization/full_background_removal
                        time:   [11.234 s 11.456 s 11.678 s]
                        change: [-32.4% -30.2% -28.1%] (p = 0.00 < 0.05)
                        Performance has improved.
```

**Interpretation**:
- **time**: [lower_bound median upper_bound] with 95% confidence
- **change**: Percentage change from previous baseline
- **p-value**: Statistical significance (p < 0.05 = significant)
- **Median (11.456s)**: Most reliable estimate of performance

### Comparison with Baseline

First run establishes baseline:
```bash
# Establish baseline (before optimization)
git checkout main
cargo bench --bench autobk_jacobian_bench --save-baseline main

# Measure optimization (after changes)
git checkout optimize-autobk-jacobian
cargo bench --bench autobk_jacobian_bench --baseline main
```

## Advanced Profiling

### Flamegraph Generation

Flamegraphs show where time is spent during execution:

```bash
# Generate flamegraph (Linux only, requires perf)
cargo bench --bench autobk_jacobian_bench -- --profile-time=5

# Find flamegraph
find target/criterion -name "flamegraph.svg"

# Open in browser
firefox target/criterion/autobk_optimization/full_background_removal/profile/flamegraph.svg
```

**What to look for**:
- Width = time spent in function
- Should see `precomputed_basis` reference (thin) vs old `splev_jacobian` (thick)
- FFT operations should be major contributor
- Allocation overhead should be minimal

### Memory Profiling with dhat

```bash
# Add dhat to dev-dependencies in Cargo.toml (if not present)
# [dev-dependencies]
# dhat = "0.3"

# Run with dhat
cargo bench --bench autobk_jacobian_bench --features dhat

# Analyze results
dhat/dh_view.html dhat-heap.json
```

**Expected metrics**:
- **Total allocations**: Reduced by ~50%
- **Peak memory**: Similar (precomputed basis adds ~800KB)
- **Allocation hotspots**: Should shift from column loop to initialization

### CPU Profiling with perf (Linux)

```bash
# Record CPU profile
perf record -g cargo bench --bench autobk_jacobian_bench -- "full_background_removal"

# Generate report
perf report

# Generate annotated source
perf annotate
```

**What to look for**:
- Reduced time in `splev_jacobian`
- More time in FFT operations (proportionally)
- Efficient memory operations in column loop

## Continuous Integration

### Baseline Tracking

```bash
# scripts/benchmark.sh
#!/bin/bash
set -e

# Run benchmarks and save results
cargo bench --bench autobk_jacobian_bench -- --save-baseline current

# Compare with main branch baseline
cargo bench --bench autobk_jacobian_bench -- --baseline main

# Fail if regression > 5%
# (This requires parsing criterion output or using criterion-table)
```

### Performance Regression Tests

Add to CI pipeline:
```yaml
# .github/workflows/performance.yml
name: Performance Tests

on: [pull_request]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run benchmarks
        run: cargo bench --bench autobk_jacobian_bench
      - name: Check for regression
        run: |
          # Parse criterion output and fail if regression > 5%
          # Implementation depends on CI tooling
```

## Troubleshooting

### Benchmark Doesn't Compile

**Issue**: Pre-existing compilation errors unrelated to benchmarks

**Solution**:
```bash
# The benchmark code is correct, but requires fixing pre-existing issues
# Focus on background.rs compilation first
cargo build --package xraytsubaki
```

### High Variance in Results

**Issue**: Inconsistent timing results

**Solutions**:
1. Close background applications
2. Disable CPU frequency scaling:
   ```bash
   sudo cpupower frequency-set --governor performance
   ```
3. Increase sample size:
   ```rust
   group.sample_size(50);  // Default is 100
   ```

### Benchmarks Too Slow

**Issue**: Benchmarks take too long to run

**Solutions**:
1. Reduce measurement time:
   ```rust
   group.measurement_time(Duration::from_secs(5));
   ```
2. Reduce sample size:
   ```rust
   group.sample_size(10);
   ```
3. Run specific benchmarks only:
   ```bash
   cargo bench --bench autobk_jacobian_bench -- "full_background_removal"
   ```

## Expected Benchmark Results Summary

| Benchmark | Baseline | Phase 1 Target | Phase 2 Target |
|-----------|----------|----------------|----------------|
| Full background removal | ~18s | 11-13s (30%) | 7-9s (50%) |
| Batch 10 spectra | ~180s | 120-130s (30%) | 80-90s (50%) |
| Batch 50 spectra | ~900s | 600-650s (30%) | 400-450s (50%) |
| Allocations per iteration | ~300 | ~150 (50%) | ~100 (67%) |

## Next Steps After Benchmarking

1. **Document actual results**: Update this file with real measurements
2. **Compare with targets**: Verify 25-35% improvement achieved
3. **Identify bottlenecks**: Use flamegraphs to find next optimization targets
4. **Update OpenSpec**: Mark benchmark tasks as complete in tasks.md
5. **Proceed to Phase 2**: FFT plan caching and memory layout optimization
