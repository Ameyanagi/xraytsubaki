# Design: AUTOBK Jacobian Performance Optimization

## Overview

This document describes the design decisions and architectural considerations for optimizing AUTOBK Jacobian computation performance. The optimization is **cross-cutting** because it affects numerical computation patterns, memory management, and FFT processing across the XAFS analysis pipeline.

## Core Mathematical Insight

### B-Spline Basis Independence

**Mathematical Foundation**:
```
Spline function: s(k) = Σ c_i * B_i(k)
Partial derivative: ∂s/∂c_i = B_i(k)
```

**Key Property**: The basis functions `B_i(k)` depend only on:
- Knot vector `t`
- Spline order `k`
- Evaluation points `x`

These are **all fixed** during Levenberg-Marquardt optimization iterations. The coefficient vector `c` only affects the linear combination, not the basis functions themselves.

**Implementation Evidence** (mathutils.rs:494-539):
```rust
pub fn splev_jacobian(t: Vec<f64>, c: Vec<f64>, k: usize, x: Vec<f64>, e: usize) -> DMatrix<f64> {
    // ...
    let h = rusty_fitpack::fpbspl::fpbspl(arg, &t, k, l);  // No 'c' used!
    // ...
    derivatives[i][ll - 1] = h[j - 1];  // Only basis functions
}
```

The parameter `c` is passed but only `c.len()` is used for array bounds. The actual computation uses only `t`, `k`, and `x`.

## Design Decisions

### 1. Precomputation Strategy

**Decision**: Compute basis Jacobian once in constructor, store in struct field

**Alternatives Considered**:
- **Lazy initialization**: Compute on first use
  - Rejected: Adds complexity with no benefit (always used immediately)
- **External caching**: Store in separate cache structure
  - Rejected: Adds indirection overhead and lifetime management complexity
- **Recompute each iteration**: Current approach
  - Rejected: Wastes 12% of computation time per iteration

**Rationale**:
- Simple implementation (add field, compute once, reference many times)
- Zero runtime overhead (just reference precomputed data)
- Memory cost acceptable (800KB per spectrum vs 30% speedup)
- No lifetime or ownership complexity

**Trade-offs**:
- Memory: +800KB per spectrum (rows × cols × 8 bytes)
- Initialization time: +50ms one-time cost
- Performance: -12% per iteration (pure win)

### 2. Memory Layout

**Decision**: Store as row-major DMatrix, optionally transpose for cache efficiency

**Alternatives Considered**:
- **Column-major storage**: Match iteration pattern
  - Rejected: nalgebra uses row-major by default, changing would affect entire codebase
- **Always transpose**: Force contiguous access
  - Rejected: Transpose has cost, not always beneficial
- **Separate storage format**: Custom layout structure
  - Rejected: Adds complexity, defeats nalgebra integration

**Rationale**:
- Phase 1: Accept row-major default, get 30% win immediately
- Phase 2: Profile and conditionally transpose if cache misses significant
- Keep nalgebra integration benefits (BLAS, compatibility)

**Trade-offs**:
- Cache efficiency: Potentially suboptimal (mitigated in Phase 2)
- Integration: Seamless nalgebra compatibility (strong benefit)
- Flexibility: Can optimize later based on measurements

### 3. FFT Plan Management

**Decision**: RefCell-based interior mutability for plan caching

**Alternatives Considered**:
- **Mutable struct** (`&mut self`):
  - Rejected: `LeastSquaresProblem` trait requires `&self` for Jacobian
- **Thread-local storage**:
  - Considered: Works but adds complexity
  - Accepted for Phase 3 if RefCell proves problematic
- **Unsafe cell**:
  - Rejected: No safety benefit over RefCell, harder to audit
- **No caching** (current):
  - Rejected: Wastes 5-15% on FFT setup overhead

**Rationale**:
- RefCell provides runtime borrow checking
- LM solver is single-threaded within one optimization
- Panic on borrow conflict indicates bug (caught in testing)
- Ergonomic with minimal unsafe code

**Trade-offs**:
- Runtime checking: Small overhead (negligible vs FFT cost)
- Panic risk: Low (single-threaded usage, caught in tests)
- Thread safety: Not thread-safe (acceptable, document clearly)

### 4. Allocation Elimination

**Decision**: Pre-allocate output buffer with final size, use direct writes

**Alternatives Considered**:
- **Vec with capacity reservation**: `Vec::with_capacity()`
  - Rejected: Still uses `extend()` which may reallocate
- **Small vector optimization**: Stack allocation for small buffers
  - Rejected: Buffers are always large (>100 elements)
- **Current approach**: Multiple allocations and extends
  - Rejected: 300 allocations per iteration is excessive

**Rationale**:
- Single allocation with exact final size
- Direct memory writes via `rows_mut()` slices
- Zero reallocation or copy overhead
- Simple and clear code

**Trade-offs**:
- Code complexity: Slightly more verbose (but clearer intent)
- Flexibility: Less flexible for variable sizes (not needed here)
- Performance: Clear win (50% allocation reduction)

### 5. Phased Implementation

**Decision**: Three-phase rollout (Quick Wins → Advanced → Experimental)

**Alternatives Considered**:
- **All at once**: Implement everything immediately
  - Rejected: High risk, difficult to validate, hard to rollback
- **Big bang rewrite**: Complete redesign
  - Rejected: Unnecessary, current algorithm is sound
- **Research first**: Experimental techniques before basics
  - Rejected: Defer high-value, low-risk optimizations

**Rationale**:
- Phase 1: Proven techniques, low risk, high value (30% gain)
- Phase 2: Moderate techniques, measured validation (20% additional)
- Phase 3: Research needed, algorithm changes (deferred to future)
- Each phase independently valuable
- Progressive validation reduces risk

## Cross-Cutting Concerns

### Memory Management Pattern

**Global Impact**: This optimization establishes a pattern for numerical code:
1. Identify invariant computations
2. Precompute in constructor/initialization
3. Store in struct fields with RefCell when needed
4. Reference in hot loops

**Reuse Opportunities**:
- Other spline operations in XAFS pipeline
- FFT-heavy computations in forward transform
- Numerical optimization elsewhere in codebase

### Performance Testing Infrastructure

**Need**: Systematic performance regression testing

**Implementation**:
```rust
#[bench]
fn bench_autobk_optimization(b: &mut Bencher) {
    let spectrum = load_standard_test_spectrum();
    b.iter(|| {
        black_box(spectrum.clone().calc_background().unwrap())
    });
}
```

**CI Integration**:
- Baseline measurement on main branch
- Compare PR against baseline
- Fail if regression > 5%
- Update baseline on merge

### Thread Safety Documentation

**Critical**: Document threading assumptions clearly

**Required Documentation**:
- `AUTOBKSpline`: "Not thread-safe due to RefCell. Use one instance per thread."
- `residual_jacobian`: "Must not be called concurrently on same instance."
- FFT cache: "Plan reuse assumes sequential calls within single optimization."

**Validation**:
- Add `#[deny(missing_docs)]` to relevant modules
- Document `Send`/`Sync` bounds explicitly
- Add test validating single-threaded LM usage

### Numerical Accuracy Validation

**Standard**: All optimizations must preserve numerical correctness

**Test Strategy**:
```rust
#[test]
fn test_optimization_preserves_accuracy() {
    let spectrum = load_test_spectrum();

    // Reference implementation (current code)
    let reference = reference_calc_background(&spectrum);

    // Optimized implementation
    let optimized = spectrum.calc_background().unwrap();

    // Strict numerical equality
    assert_relative_eq!(
        optimized.chi,
        reference.chi,
        epsilon = 1e-10
    );
}
```

**Validation Points**:
- After each phase implementation
- Before merging to main
- In CI pipeline
- With diverse test spectra

## API Stability Guarantees

### Public API: No Changes

**Guarantee**: All optimizations are internal implementation details

**Unchanged**:
- `XASSpectrum::calc_background()` signature
- `AUTOBKParams` structure
- Output data format
- Numerical results (within floating-point epsilon)

### Internal API: Managed Changes

**Changed** (internal only):
- `AUTOBKSpline` struct fields (private)
- `residual_jacobian` implementation (private)
- FFT utility functions (private)

**Compatibility**: No impact on external users or dependent crates

## Performance Characteristics

### Time Complexity

**Before**: O(iterations × (num_coefs × nfft × log(nfft)))
**After Phase 1**: O(nfft × log(nfft) + iterations × (num_coefs × nfft × log(nfft)))
**Improvement**: Precomputation amortizes over iterations (constant cost becomes one-time)

**After Phase 2**: Additional FFT plan reuse reduces setup overhead within each iteration

### Space Complexity

**Before**: O(num_points + num_coefs + nfft)
**After**: O(num_points × num_coefs + nfft)
**Additional**: 800KB per spectrum (precomputed basis matrix)

**Justification**:
- Memory is cheap vs computation time
- Modern systems have GB of RAM
- 1000 spectra = 800MB (acceptable for batch processing)

### Cache Behavior

**Current**: Column iteration causes strided access (cache misses)
**Phase 2**: Optional transpose for contiguous access (cache-friendly)
**Expected**: 20% cache miss reduction based on memory access patterns

## Risk Management

### Technical Risks

1. **Precomputation Correctness** (LOW)
   - Mitigation: Extensive numerical tests with epsilon < 1e-10
   - Rollback: Remove field, revert to runtime computation
   - Detection: Comparison tests with reference implementation

2. **RefCell Panics** (MEDIUM)
   - Mitigation: Debug assertions, extensive testing, single-threaded validation
   - Fallback: Use `Mutex` (small performance cost, guaranteed safety)
   - Detection: Runtime panics caught in testing

3. **Memory Overhead** (LOW)
   - Mitigation: Document requirement clearly, provide configuration option
   - Fallback: Make precomputation optional via feature flag
   - Detection: Memory profiling in tests

4. **Numerical Stability** (VERY LOW)
   - Mitigation: Same algorithm, same floating-point operations
   - Validation: Comprehensive accuracy tests
   - Detection: Automated comparison with reference

### Deployment Risks

1. **Platform Compatibility** (VERY LOW)
   - Mitigation: Pure Rust, no platform-specific code
   - Validation: CI on Linux, macOS, Windows
   - Detection: Cross-platform test suite

2. **Dependency Changes** (LOW)
   - Mitigation: Pin dependency versions, test upgrades separately
   - Impact: Only affects RustFFT integration (Phase 2)
   - Detection: Cargo.lock checks in CI

## Future Extensions

### Batched FFT Processing

**Phase 3 Research**: True 2D batched FFT

**Approach**:
1. Collect all column FFT inputs into 2D matrix
2. Use batched FFT API (RustFFT or FFTW)
3. Process all columns in single operation

**Expected**: 40-60% improvement for FFT portion

**Challenges**:
- RustFFT batch API availability
- Memory layout requirements
- FFTW C dependency management

### GPU Acceleration

**Long-term Possibility**: CUDA/OpenCL for FFT operations

**Rationale**:
- FFTs are embarrassingly parallel
- GPU memory bandwidth ideal for this workload
- 10-100× potential speedup

**Challenges**:
- Cross-platform compatibility
- Deployment complexity (CUDA drivers)
- Memory transfer overhead
- Development cost vs benefit

### Adaptive Jacobian Strategy

**Hybrid Approach**: Full Jacobian periodically, approximate updates between

**Options**:
1. **Broyden updates**: Rank-1 approximation
2. **Finite difference**: Selective numerical differentiation
3. **Adaptive frequency**: Full Jacobian when convergence slows

**Research Needed**:
- Convergence analysis
- Accuracy vs speed trade-offs
- Production validation

## References

- PERFORMANCE_ANALYSIS.md: Initial bottleneck identification
- OPTIMIZATION_SOLUTION.md: Complete implementation guide
- background.rs:697-767: Current Jacobian implementation
- mathutils.rs:494-539: Basis function computation
- LM algorithm: "Methods for Non-Linear Least Squares Problems" (Madsen et al.)
- B-spline theory: "A Practical Guide to Splines" (de Boor)
