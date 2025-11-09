# Performance Analysis: AUTOBK Minimizer

## Executive Summary

Analyzed the Levenberg-Marquardt optimization in the AUTOBK background removal algorithm. Identified **5 major performance bottlenecks** in the Jacobian computation, with estimated **30-60% potential speedup** through targeted optimizations.

**Critical finding**: The `residual_jacobian` method performs ~50-100 FFT operations per iteration (one per spline coefficient), which is the primary performance bottleneck.

## Methodology

1. Located minimizer code using pattern search for "minim|optim|levenberg|marquardt"
2. Analyzed Jacobian computation in `background.rs` (lines 697-767)
3. Examined FFT implementation in `xrayfft.rs` (lines 405-414)
4. Identified computational complexity and memory allocation patterns
5. Compared analytical vs numerical differentiation approaches

## Performance Bottlenecks Identified

### 1. **Individual FFT Per Jacobian Column** (CRITICAL)
**Location**: `background.rs:737-764`

**Current Implementation**:
```rust
let jacobian_columns = spline_jacobian
    .column_iter()
    .map(|chi_der| {
        let mut out: DVector<f64> = chi_der
            .component_mul(&self.ftwin)
            .xftf_fast(self.nfft, self.kstep)[..self.irbkg]
            .realimg();
        // ... clamping operations
    })
    .collect::<Vec<DVector<f64>>>();
```

**Problem**:
- Performs **one FFT per spline coefficient** (typically 50-100 coefficients)
- Each FFT is O(N log N) where N = nfft (default 2048)
- Total complexity: O(num_coefs × nfft × log(nfft))
- **Estimated impact**: 60-80% of Jacobian computation time

**Evidence**:
- Commented timing code (lines 797-819) suggests this was previously measured
- Default nfft=2048, typical spline has 50-100 knots
- 50 FFTs × 2048 points = ~102,400 FFT operations per iteration

### 2. **Unnecessary Vector Allocations in Loop**
**Location**: `background.rs:737-764`

**Problem**:
```rust
let jacobian_columns = spline_jacobian
    .column_iter()
    .map(|chi_der| {
        let mut out: DVector<f64> = chi_der  // Allocation 1
            .component_mul(&self.ftwin)       // Allocation 2
            .xftf_fast(self.nfft, self.kstep)[..self.irbkg]
            .realimg();                        // Allocation 3

        // More allocations for clamping
        out.extend(low_clamp.data.as_vec().to_owned());   // Allocation 4
        out.extend(high_clamp.data.as_vec().to_owned());  // Allocation 5
        out
    })
    .collect::<Vec<DVector<f64>>>();          // Final allocation
```

**Impact**:
- 5-6 allocations per column
- 50 columns × 6 allocations = ~300 allocations per iteration
- Each allocation involves heap memory access and potential cache misses
- **Estimated impact**: 10-15% of Jacobian computation time

### 3. **Redundant Clone Operations**
**Location**: `background.rs:704-711` (splev_jacobian call)

**Problem**:
```rust
let spline_jacobian = -splev_jacobian(
    self.knots.data.as_vec().clone(),      // Clone 1
    self.coefs.data.as_vec().clone(),      // Clone 2
    self.order,
    self.kout.data.as_vec().clone(),       // Clone 3
    3,
);
```

**Impact**:
- Clones 3 large vectors every Jacobian evaluation
- Typical sizes: knots ~100, coefs ~100, kout ~1000
- **Estimated impact**: 5-10% of Jacobian computation time

**Note**: May be required by `splev_jacobian` API, needs verification

### 4. **Sequential Column Processing**
**Location**: `background.rs:737-764`

**Problem**:
- Uses `.map()` which processes columns sequentially
- Each column is independent and could be computed in parallel
- Rayon parallel iterators not utilized

**Opportunity**:
- Replace `.map()` with `.par_iter()` from Rayon
- **Potential speedup**: 2-4× on multi-core systems (if FFTs are parallelizable)

**Caveat**: Need to verify that FFT implementation is thread-safe

### 5. **FFT Setup Overhead**
**Location**: `xrayfft.rs:405-414`

**Problem**:
```rust
pub fn xftf_fast_nalgebra(chi: &DVector<f64>, nfft: usize, kstep: f64) -> DynRealDft<f64> {
    let mut cchi = vec![0.0 as f64; nfft];          // Allocation every call
    cchi[..chi.len()].copy_from_slice(&chi.data.as_vec()[..]);

    let mut freq = cchi.real_fft();                 // FFT setup every call
    freq *= kstep / std::f64::consts::PI.sqrt();
    freq
}
```

**Impact**:
- FFT plan might be recreated for each call (depends on `real_fft()` implementation)
- Modern FFT libraries (FFTW) benefit from plan reuse
- **Estimated impact**: 5-15% if plan is not cached

## Computational Complexity Analysis

### Current Jacobian Method (Analytical)
```
Per Iteration Cost:
- splev_jacobian: O(num_coefs × num_points × order) ≈ 100 × 1000 × 3 = 300K ops
- FFT per column: O(num_coefs × nfft × log(nfft)) ≈ 100 × 2048 × 11 = 2.2M ops
- Clamping: O(num_coefs × nclamp) ≈ 100 × 20 = 2K ops
- Matrix assembly: O(num_coefs × residual_len) ≈ 100 × 100 = 10K ops

Total: ~2.5M operations per iteration
Dominant: FFT operations (88%)
```

### Alternative Numerical Method (Commented Out)
```rust
// let residual_vec = |coefs: &DVector<f64>| AUTOBKSpline::residual_vec(&self, &coefs);
// Some(self.coefs.jacobian(&residual_vec))
```

**Complexity**:
- Numerical differentiation: O(num_coefs × (1 forward + 1 backward evaluation))
- Each evaluation includes FFT: O(nfft × log(nfft))
- Total: O(num_coefs × nfft × log(nfft)) - **same order as analytical!**

**Why analytical is faster** (based on commented timing code):
- Analytical: Single FFT per column with pre-computed derivatives
- Numerical: 2 full residual evaluations per parameter (forward/backward)
- Each residual evaluation does: spline_eval + FFT + clamping
- Analytical avoids redundant spline evaluations

## Optimization Recommendations

### Priority 1: Batch FFT Processing (HIGH IMPACT)
**Estimated speedup**: 40-60%

**Strategy**: Restructure to compute all FFTs in a single batch operation

```rust
// Current: 100 separate FFT calls
for col in jacobian_columns {
    let fft_result = col.xftf_fast(nfft, kstep);
}

// Proposed: Single batched FFT
let all_inputs = combine_columns_into_matrix(jacobian_columns);
let all_ffts = batch_fft_2d(all_inputs, nfft, kstep);  // Process all at once
```

**Requirements**:
1. Check if underlying FFT library (`easyfft`) supports batched 2D FFTs
2. If not, consider switching to `rustfft` or `fftw` with batch support
3. Pre-allocate output matrix to avoid intermediate allocations

**Implementation Steps**:
1. Research FFT library capabilities
2. Create `batch_xftf_fast()` function
3. Restructure `residual_jacobian` to use batch operations
4. Benchmark against current implementation

### Priority 2: Pre-allocate and Reuse Buffers (MEDIUM IMPACT)
**Estimated speedup**: 10-20%

**Strategy**: Store reusable buffers in `AUTOBKSpline` struct

```rust
struct AUTOBKSpline {
    // ... existing fields ...

    // Optimization buffers
    fft_buffer: Option<DVector<f64>>,           // Reusable FFT input buffer
    jacobian_workspace: Option<DMatrix<f64>>,  // Pre-allocated Jacobian matrix
}

impl AUTOBKSpline {
    fn residual_jacobian_optimized(&mut self, coefs: &DVector<f64>) -> DMatrix<f64> {
        // Reuse pre-allocated buffers instead of allocating each time
        let jac = self.jacobian_workspace.get_or_insert_with(|| {
            DMatrix::zeros(self.residual_len(), coefs.len())
        });

        // ... computation using pre-allocated workspace
        jac.clone()  // Only clone final result
    }
}
```

**Challenges**:
- LeastSquaresProblem trait requires `&self`, not `&mut self`
- May need interior mutability (RefCell/Cell) or unsafe code
- Trade-off: thread-safety vs performance

### Priority 3: Eliminate Unnecessary Clones (LOW-MEDIUM IMPACT)
**Estimated speedup**: 5-10%

**Strategy**: Investigate `splev_jacobian` API to avoid clones

```rust
// Check if splev_jacobian can accept slices instead of owned vectors
// If rusty_fitpack API allows:
let spline_jacobian = -splev_jacobian(
    self.knots.data.as_slice(),  // No clone
    self.coefs.data.as_slice(),  // No clone
    self.order,
    self.kout.data.as_slice(),   // No clone
    3,
);
```

**Action**: Review `rusty_fitpack` crate API documentation

### Priority 4: Parallelize Column Processing (MEDIUM-HIGH IMPACT)
**Estimated speedup**: 2-4× on multi-core (if FFT allows)

**Strategy**: Use Rayon for parallel column processing

```rust
use rayon::prelude::*;

let jacobian_columns: Vec<DVector<f64>> = spline_jacobian
    .column_iter()
    .par_bridge()  // Enable parallel processing
    .map(|chi_der| {
        // ... same FFT and clamping operations
    })
    .collect();
```

**Considerations**:
1. Check if FFT library is thread-safe
2. Memory bandwidth may become bottleneck with parallel FFTs
3. Best for systems with 4+ cores
4. May conflict with outer-level Rayon parallelism in `xasgroup.rs`

**Risk**: AUTOBK already has Clone requirement for Rayon in outer loop (xasgroup.rs:199-201)
- May create nested parallelism issues
- Need to coordinate parallelism levels

### Priority 5: FFT Plan Caching (LOW-MEDIUM IMPACT)
**Estimated speedup**: 5-15%

**Strategy**: Reuse FFT plans across calls

```rust
use std::cell::RefCell;

struct FFTPlanCache {
    plan: RefCell<Option<RealFftPlan>>,
}

impl FFTPlanCache {
    fn get_or_create(&self, size: usize) -> &RealFftPlan {
        // Create plan once, reuse for all FFTs of same size
    }
}
```

**Dependencies**: Check if `easyfft` exposes plan caching API

## Testing Strategy

### Benchmark Setup
```rust
#[bench]
fn bench_jacobian_computation(b: &mut Bencher) {
    let spline = setup_typical_autobk_spline();

    b.iter(|| {
        spline.residual_jacobian(&spline.coefs)
    });
}

#[bench]
fn bench_jacobian_optimized(b: &mut Bencher) {
    let mut spline = setup_typical_autobk_spline();

    b.iter(|| {
        spline.residual_jacobian_optimized(&spline.coefs)
    });
}
```

### Correctness Validation
```rust
#[test]
fn test_jacobian_optimization_correctness() {
    let spline = setup_typical_autobk_spline();

    let jac_original = spline.residual_jacobian(&spline.coefs);
    let jac_optimized = spline.residual_jacobian_optimized(&spline.coefs);

    assert_relative_eq!(jac_original, jac_optimized, epsilon = 1e-12);
}
```

### Performance Metrics to Track
1. **Time per Jacobian evaluation**: Current baseline needed
2. **Memory allocations**: Use `heaptrack` or similar
3. **Cache efficiency**: `perf stat` on Linux
4. **Parallel scaling**: Test 1, 2, 4, 8 cores
5. **Full AUTOBK iteration time**: End-to-end benchmark

## Alternative Approaches (Long-term)

### 1. Hybrid Analytical-Numerical Jacobian
- Use analytical for first ~10 iterations (fast convergence)
- Switch to numerical with larger step size for final refinement
- Trade accuracy for speed in late iterations

### 2. Approximate Jacobian Updates
- Full Jacobian every N iterations
- Use Broyden or BFGS update formula for intermediate steps
- Common in quasi-Newton methods

### 3. GPU Acceleration
- FFTs are highly parallelizable on GPU
- Consider `cuFFT` bindings for Rust
- Significant engineering effort, but 10-100× speedup possible

### 4. Reduced-Rank Jacobian
- If many spline coefficients correlate, use SVD to reduce dimensionality
- Compute Jacobian for reduced parameter space
- Requires mathematical analysis of spline behavior

## Implementation Priority

**Phase 1** (Quick wins, 1-2 weeks):
1. ✅ Performance analysis (completed)
2. Benchmark current implementation
3. Eliminate clones in `splev_jacobian` call
4. Pre-allocate Jacobian workspace

**Phase 2** (Medium effort, 2-4 weeks):
1. Investigate batch FFT capabilities
2. Implement FFT plan caching
3. Benchmark improvements

**Phase 3** (High effort, 1-2 months):
1. Implement batch FFT processing
2. Add parallelization with proper coordination
3. Comprehensive benchmarking and tuning

**Phase 4** (Research, 2-3 months):
1. Explore hybrid Jacobian methods
2. Investigate GPU acceleration feasibility
3. Mathematical analysis for reduced-rank approaches

## Measurement Plan

### Before Optimization
```bash
# Run with timing instrumentation
cargo bench --bench autobk_optimization

# Profile with perf (Linux)
perf record -g cargo bench --bench autobk_optimization
perf report

# Check allocations
valgrind --tool=massif cargo bench --bench autobk_optimization
```

### Success Metrics
- **Target**: 30-50% reduction in Jacobian computation time
- **Stretch**: 50-70% with batch FFT and parallelization
- **Validation**: No regression in accuracy (< 1e-10 relative error)

## Risks and Mitigations

### Risk 1: Batch FFT Not Available
**Mitigation**: Implement custom batched wrapper or switch FFT library

### Risk 2: Parallelization Overhead
**Mitigation**: Make parallelization configurable, benchmark crossover point

### Risk 3: Memory Usage Increase
**Mitigation**: Profile memory, consider memory pooling for large batches

### Risk 4: Numerical Instability
**Mitigation**: Extensive testing, maintain numerical accuracy benchmarks

### Risk 5: API Constraints
**Mitigation**: Fork `rusty_fitpack` if necessary, contribute improvements upstream

## Conclusion

The AUTOBK minimizer's Jacobian computation has significant optimization potential, primarily in the FFT operations. The analytical approach is fundamentally sound, but implementation details create performance bottlenecks.

**Key Insight**: The commented-out timing code (lines 797-819) indicates the developers were aware of performance concerns and measured both approaches. The analytical method was chosen for good reason, but can be optimized further.

**Recommended Next Steps**:
1. Uncomment and run timing code to establish baseline
2. Implement Priority 1-3 optimizations (low-hanging fruit)
3. Benchmark and iterate
4. Consider Priority 4-5 based on results

**Expected Outcome**: 30-60% performance improvement with moderate effort, potential for 2-4× with aggressive parallelization and library optimization.
