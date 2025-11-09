# Implementation Tasks

## Phase 1: Quick Wins (Week 1)

### Day 1-2: Precompute Basis

- [ ] **Add precomputed_basis field**
  - File: `crates/xraytsubaki/src/xafs/background.rs:608`
  - Add `precomputed_basis: DMatrix<f64>` to `AUTOBKSpline` struct
  - Update struct documentation

- [ ] **Compute in constructor**
  - File: `crates/xraytsubaki/src/xafs/background.rs:~353`
  - Add computation after knot/coef initialization
  - Use dummy coefficient vector: `vec![0.0; num_coefs]`
  - Store result in struct field

- [ ] **Replace runtime call**
  - File: `crates/xraytsubaki/src/xafs/background.rs:728-734`
  - Replace `splev_jacobian()` call with `&self.precomputed_basis`
  - Remove vector clones (no longer needed)

- [ ] **Unit tests: verify identical results**
  - Create `test_precomputed_basis_correctness()`
  - Compare old vs new Jacobian computation
  - Assert relative equality: epsilon < 1e-12

- [ ] **Benchmark: measure speedup**
  - Create `bench_jacobian_precomputed()`
  - Compare with baseline measurement
  - Document actual speedup achieved

### Day 3: Eliminate Allocations

- [ ] **Implement pre-allocated buffer approach**
  - File: `crates/xraytsubaki/src/xafs/background.rs:737-764`
  - Pre-allocate output vector with final size
  - Use `rows_mut()` for direct writes
  - Remove `extend()` calls

- [ ] **Profile memory allocations**
  - Before: Run heaptrack/dhat on current implementation
  - After: Run profiling on optimized version
  - Document reduction in allocation count

- [ ] **Benchmark: measure speedup**
  - Create `bench_allocation_optimized()`
  - Measure performance improvement
  - Document results

### Day 4-5: Integration & Testing

- [ ] **Comprehensive numerical accuracy tests**
  - Test against reference implementation
  - Verify convergence properties unchanged
  - Test edge cases (small/large spectra, various parameters)

- [ ] **End-to-end benchmarks**
  - Full AUTOBK optimization timing
  - Compare with baseline
  - Verify 25-35% total speedup achieved

- [ ] **Code review and documentation**
  - Update inline comments
  - Document precomputation rationale
  - Add performance notes to struct docs

- [ ] **Commit and document**
  - Commit message: "perf: precompute spline basis and eliminate allocations"
  - Update CHANGELOG.md
  - Create performance regression test

## Phase 2: Advanced (Week 2-3)

### Week 2: FFT Plan Caching

- [ ] **Research RustFFT plan API**
  - Review RustFFT documentation
  - Understand plan caching patterns
  - Design RefCell-based cache structure

- [ ] **Implement RefCell-based caching**
  - Add `fft_cache: RefCell<Option<Arc<dyn RealToComplex<f64>>>>` field
  - Create plan on first use
  - Reuse for subsequent FFTs

- [ ] **Handle plan size validation**
  - Verify plan size matches nfft
  - Handle size mismatches gracefully
  - Add debug assertions

- [ ] **Test thread safety assumptions**
  - Verify single-threaded LM usage
  - Test with existing Rayon parallelism
  - Document threading constraints

- [ ] **Benchmark improvement**
  - Measure FFT overhead reduction
  - Target 5-15% additional speedup
  - Document results

### Week 3: Memory Layout

- [ ] **Profile current cache behavior**
  - Run `perf stat` for cache misses
  - Identify memory access patterns
  - Establish baseline metrics

- [ ] **Implement transpose approach**
  - Transpose `precomputed_basis` after computation
  - Update column iteration to row iteration
  - Ensure contiguous memory access

- [ ] **A/B test with current approach**
  - Benchmark both approaches
  - Measure cache miss reduction
  - Choose faster implementation

- [ ] **Validation and documentation**
  - Verify numerical correctness maintained
  - Document memory layout rationale
  - Update performance notes

## Phase 3: Validation (Week 4)

- [ ] **Comprehensive benchmark suite**
  - Multiple spectrum sizes (small, medium, large)
  - Various parameter combinations
  - Statistical significance testing

- [ ] **Numerical accuracy validation**
  - Epsilon < 1e-10 relative error
  - Test against reference data
  - Verify convergence properties

- [ ] **Memory profiling**
  - Confirm no memory leaks
  - Verify allocation reduction targets met
  - Document memory overhead (800KB/spectrum)

- [ ] **Cross-platform testing**
  - Linux (primary development platform)
  - macOS (CI validation)
  - Windows (CI validation)

- [ ] **Performance regression tests**
  - Create benchmark baseline
  - Add CI performance checks
  - Set acceptable performance thresholds

- [ ] **Documentation updates**
  - Update README with performance notes
  - Add optimization guide
  - Document memory vs speed trade-offs

- [ ] **Release preparation**
  - Update version number
  - Prepare release notes
  - Create migration guide (if needed)

## Phase 4: Experimental (Future Work)

### Batch 2D FFT Research (2-3 weeks)

- [ ] **Investigate RustFFT batch capabilities**
  - Review library documentation
  - Test batch FFT performance
  - Compare with current approach

- [ ] **Evaluate FFTW integration**
  - Assess C dependency trade-offs
  - Benchmark batched performance
  - Consider deployment complexity

- [ ] **Prototype implementation**
  - Create batch FFT wrapper
  - Test numerical correctness
  - Measure performance gains

- [ ] **Decision point**
  - Evaluate 40-60% speedup potential
  - Assess maintenance burden
  - Decide on integration

### Quasi-Newton Methods Research (2-4 weeks)

- [ ] **Research Broyden's method**
  - Review quasi-Newton literature
  - Understand convergence properties
  - Assess applicability to AUTOBK

- [ ] **Prototype implementation**
  - Implement rank-1 update formula
  - Test convergence behavior
  - Compare with full Jacobian

- [ ] **Validation research**
  - Verify mathematical correctness
  - Test on diverse spectra
  - Assess convergence reliability

- [ ] **Decision point**
  - Evaluate 50-70% reduction potential
  - Assess algorithmic risk
  - Decide on production readiness

## Milestones

- **M1: Phase 1 Complete** (End of Week 1)
  - Precomputation implemented and tested
  - Allocations eliminated
  - 25-35% speedup achieved and validated

- **M2: Phase 2 Complete** (End of Week 3)
  - FFT plan caching operational
  - Memory layout optimized
  - 45-60% total speedup achieved

- **M3: Phase 3 Complete** (End of Week 4)
  - All tests passing
  - Documentation complete
  - Ready for release

- **M4: Research Complete** (Future)
  - Experimental techniques evaluated
  - Production decisions made
  - Roadmap for next iteration established
