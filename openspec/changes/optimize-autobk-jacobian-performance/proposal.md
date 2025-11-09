# Optimize AUTOBK Jacobian Performance

## Why

The AUTOBK background removal algorithm uses Levenberg-Marquardt optimization with analytical Jacobian computation. Performance analysis revealed that **Jacobian computation accounts for ~88% of optimization time**, processing ~2.5M operations per iteration across 50-100 FFT calls.

**Critical Discovery**: The spline basis Jacobian is **independent of coefficient values** and can be precomputed once before optimization begins. The `splev_jacobian` function only uses the coefficient vector for array sizing, not computation. The actual basis functions computed by `fpbspl()` depend only on knot vector, order, and evaluation points—all **fixed during optimization**.

**Current Performance Impact**:
- Per-iteration cost: 2.5M operations (300K for basis + 2.2M for FFT)
- 300 memory allocations per iteration (5-6 per column × 50 columns)
- Vector clones on every Jacobian call
- Sequential FFT processing (no batching)

**Real-World Impact**:
- Single spectrum: 18 seconds (typical 15-20 LM iterations)
- 100 spectra batch: 30 minutes
- Research workflows significantly slowed by background removal bottleneck

## What Changes

### Phase 1: Quick Wins (2-3 Days, 25-35% Speedup)

1. **Precompute Spline Basis** ⭐ HIGHEST PRIORITY
   - Add `precomputed_basis: DMatrix<f64>` field to `AUTOBKSpline` struct
   - Compute once in constructor (line ~353)
   - Replace runtime computation in `residual_jacobian` (lines 728-734)
   - **Expected**: 10-15% speedup, 800KB memory per spectrum
   - **Risk**: Very Low

2. **Pre-allocate Clamping Buffers**
   - Eliminate 300 allocations per iteration in column loop (lines 737-764)
   - Pre-allocate with final size, use direct writes
   - **Expected**: 10-15% speedup
   - **Risk**: Low

3. **Eliminate Vector Clones**
   - Remove clones in `splev_jacobian` call (now unnecessary with precomputation)
   - **Expected**: Included in precomputation benefit
   - **Risk**: Very Low

### Phase 2: Advanced Optimizations (1-2 Weeks, 45-60% Total)

4. **FFT Plan Caching with RefCell**
   - Cache FFT plans using `RefCell<Option<Arc<dyn RealToComplex<f64>>>>`
   - Reuse plans across column FFTs
   - **Expected**: 5-15% additional speedup
   - **Risk**: Medium (requires RefCell borrow checking)

5. **Memory Layout Optimization**
   - Transpose precomputed basis for contiguous memory access
   - Reduce cache misses during column iteration
   - **Expected**: 5-10% additional speedup
   - **Risk**: Medium (needs profiling validation)

### Phase 3: Experimental (Research Phase, 2-4 Weeks)

6. **True Batch 2D FFT**
   - Investigate batched FFT with RustFFT or FFTW
   - **Expected**: 40-60% improvement for FFT portion
   - **Risk**: High (requires library changes)

7. **Quasi-Newton Jacobian Updates (Broyden's Method)**
   - Full Jacobian every N iterations, rank-1 updates between
   - **Expected**: 50-70% reduction in Jacobian calls
   - **Risk**: Very High (changes mathematical algorithm)

## Impact

### Performance Improvements

**Phase 1 Target**:
- Jacobian computation: -30 to -40%
- Full optimization: -25 to -35%
- Memory allocations: -50%
- **Real-world**: 100 spectra batch: 30 min → 20-22 min

**Phase 2 Target**:
- Jacobian computation: -45 to -60%
- Full optimization: -40 to -50%
- Cache efficiency: +20%
- **Real-world**: 100 spectra batch: 30 min → 15-18 min

**Phase 3 Potential**:
- Full optimization: up to -70% with advanced techniques
- **Real-world**: 100 spectra batch: 30 min → 9-12 min

### Code Quality

- Cleaner, more maintainable implementation
- Better documented performance characteristics
- Comprehensive test coverage
- Performance regression protection

### API Compatibility

- **Breaking Changes**: None
- **Internal Only**: All optimizations are implementation details
- **Behavioral Changes**: None (same algorithm, same results)

### Risks and Mitigation

**Technical Risks**:
1. **Precomputation Correctness** (LOW): Extensive numerical tests, easy rollback
2. **RefCell Panics** (MEDIUM): Debug assertions, extensive testing, Mutex alternative
3. **Memory Overhead** (LOW): 800KB per spectrum acceptable, document in release notes
4. **Convergence Changes** (VERY LOW): Same algorithm, verify with comparison tests

**Deployment Risks**:
5. **Platform Compatibility** (VERY LOW): Pure Rust, test on CI (Linux, macOS, Windows)

## Success Metrics

| Metric | Tool | Phase 1 Target | Phase 2 Target |
|--------|------|----------------|----------------|
| Jacobian time | cargo bench | -30% | -50% |
| Full optimization time | cargo bench | -25% | -40% |
| Memory allocations | heaptrack/dhat | -50% | -60% |
| Cache misses | perf stat | - | -20% |
| Numerical accuracy | unit tests | <1e-10 relative error | <1e-10 relative error |

## References

- `/home/ryuichi/rust/xafs/xraytsubaki/PERFORMANCE_ANALYSIS.md`: Initial bottleneck analysis
- `/home/ryuichi/rust/xafs/xraytsubaki/OPTIMIZATION_SOLUTION.md`: Complete implementation guide
- `crates/xraytsubaki/src/xafs/background.rs:697-767`: Current Jacobian implementation
- `crates/xraytsubaki/src/xafs/mathutils.rs:494-539`: splev_jacobian analysis
