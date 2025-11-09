# Project Context

## Purpose
xraytsubaki is a high-performance X-ray Absorption Spectroscopy (XAS) data analysis tool written in Rust. The project implements core functionalities of [xraylarch](https://xraypy.github.io/xraylarch/) with a focus on:

- **Speed**: Processing thousands of XAS spectra in seconds (10x+ faster than Python alternatives)
- **Parallel Processing**: Leveraging Rayon for multi-core processing (e.g., 10,000 spectra in 7.5s on 10-core M1)
- **Cross-Language Integration**: Building a generalized library compatible with Python, JavaScript, and native applications
- **Modern GUI**: Supporting native GUI applications via frameworks like Tauri and Dioxus

**Name Origin**: Inspired by [tsubaki (Camellia japonica)](https://en.wikipedia.org/wiki/Camellia_japonica)

## Tech Stack

### Core Technologies
- **Language**: Rust (2021 edition)
- **Workspace**: Cargo workspace with multiple crates
- **Parallelization**: Rayon for multi-threaded processing
- **Python Bindings**: PyO3 with maturin (planned in py-xraytsubaki)

### Key Dependencies
- **Scientific Computing**:
  - `ndarray` (0.15.6) - N-dimensional arrays with serde support
  - `nalgebra` (0.32.4) - Linear algebra
  - `num-complex` (0.4.5) - Complex number support
  - `levenberg-marquardt` (0.13.1) - Optimization algorithms
  - `polyfit-rs` (0.2.1) - Polynomial fitting
  - `rusty-fitpack` (0.1.2) - Spline fitting

- **Signal Processing**:
  - `easyfft` (0.4.1) - FFT operations
  - `fftconvolve` (0.1.1) - Convolution operations
  - `errorfunctions` (0.2.0) - Error function calculations
  - `enterpolation` (0.2.1) - Interpolation with linear features

- **Data I/O**:
  - `data_reader` (0.5.0) - Data file reading
  - `serde` (1.0.197) - Serialization/deserialization
  - `serde_json` (1.0.114) - JSON support
  - `bson` (2.9.0) - BSON format support
  - `serde_arrow` (0.10.0) - Apache Arrow integration
  - `flate2` (1.0.28) - Compression support

- **Parsing & Utilities**:
  - `pest` (2.7.7) - Parser generator
  - `itertools` (0.12.0) - Iterator utilities
  - `lazy_static` (1.4.0) - Lazy static values
  - `derivative` (2.2.0) - Custom derive macros

- **Development**:
  - `criterion` (0.5.1) - Benchmarking framework with HTML reports
  - `pprof` (0.13) - Profiling with flamegraph support
  - `approx` (0.5.1) - Approximate floating-point comparisons

## Project Conventions

### Code Style
- **Edition**: Rust 2021
- **Formatting**: Standard Rustfmt conventions
- **Naming**:
  - `snake_case` for modules, functions, variables
  - `PascalCase` for types, structs, enums
  - `SCREAMING_SNAKE_CASE` for constants
- **Module Organization**: Domain-driven structure (xafs, parser, prelude)
- **Imports**: Prefer explicit imports; use prelude module for commonly used items
- **Linting**: Allow `dead_code` and `unused_imports` in debug mode for development flexibility
- **Documentation**: Use `//!` for module-level docs, `///` for item-level docs

### Architecture Patterns
- **Workspace Organization**:
  - `crates/xraytsubaki/` - Core library implementation
  - `crates/xasio/` - I/O utilities
  - `py-xraytsubaki/` - Python bindings (in development)
  - `xraytsubaki-gui/` - GUI application (planned)

- **Module Structure**:
  - `xafs/` - Core XAS analysis functionality
    - `xasgroup.rs` / `xasgroup_nalgebra.rs` - Main group data structure
    - `xasparameters.rs` - Analysis parameters
    - `xasspectrum.rs` / `xasspectrum_nalgebra.rs` - Spectrum representation
    - `background.rs` / `background_nalgebra.rs` - Background removal (AUTOBK)
    - `normalization.rs` / `normalization_nalgebra.rs` - Pre/post-edge normalization
    - `xrayfft.rs` / `xrayfft_nalgebra.rs` - FFT/IFFT operations
    - `fitting.rs` / `fitting_nalgebra.rs` - Curve fitting utilities
    - `io/` - Serialization (JSON, BSON)
    - `mathutils.rs`, `xafsutils.rs`, `lmutils.rs` - Utility functions
  - `parser/` - Data parsing functionality
  - `plot/` - Visualization support
  - `prelude/` - Common exports

- **Design Patterns**:
  - **Trait-based abstractions**: `MathUtils`, `Normalization`, `XAFSUtils` traits
  - **Error handling**: Custom `XAFSError` enum with `Error` trait implementation
  - **Dual implementations**: Both standard and nalgebra-based versions for flexibility
  - **Optimization Strategy**:
    - Analytical Jacobian for AUTOBK minimization
    - Rayon-based parallelization for batch processing
    - Zero-copy operations where possible

### Testing Strategy
- **Unit Tests**: Embedded in `xafs/tests.rs` and `mod tests` blocks within source files
- **Test Configuration**: `#[cfg(test)]` gating
- **Test Constants**:
  - `TEST_TOL: f64 = 1e-12` - High-precision tests
  - `TEST_TOL_LESS_ACC: f64 = 1e-8` - Lower precision tests
- **Test Data**: Test files loaded via `data_reader` with standardized `ReaderParams`, using `CARGO_MANIFEST_DIR` for relative paths
- **Benchmarks**: Criterion-based benchmarks with HTML reports
  - Single-threaded benchmark: `xas_group_benchmark_single`
  - Parallel benchmark: `xas_group_benchmark_parallel` (active)
- **Profiling**: pprof with flamegraph for performance analysis
- **Debug Symbols**: Enabled in bench profile for detailed profiling
- **Floating Point**: Use `approx` crate for numerical comparisons

### Git Workflow
- **Main Branch**: `main`
- **Recent Activity**: Focus on refactoring commits
- **License**: Dual-licensed MIT OR Apache-2.0
- **Repository**: https://github.com/Ameyanagi/xraytsubaki
- **CI/CD**: GitHub Actions workflow (`.github/workflows/rust.yml`)
- **Commit Style**: Descriptive messages focusing on "why" over "what"

## Domain Context

### X-ray Absorption Spectroscopy (XAS)
- **EXAFS**: Extended X-ray Absorption Fine Structure analysis
- **Core Operations**:
  1. **Energy calibration** (`find_e0`) - Edge energy determination / identify absorption edge
  2. **Normalization** - Pre-edge and post-edge background fitting
  3. **AUTOBK** - Automated background removal using spline fitting with Levenberg-Marquardt optimization
  4. **EXAFS extraction** - Extract χ(k) oscillations
  5. **Fourier Transform** - FFT/IFFT transforms for k-space to R-space conversion

### Scientific Computing Requirements
- **Numerical Precision**: Floating-point operations require approximate comparisons (`TEST_TOL = 1e-12` for critical calculations)
- **Performance**: Real-time or near-real-time processing of large datasets (1000+ spectra)
- **Reproducibility**: Results must match established tools (xraylarch) for validation
- **Data Formats**: Support for multiple serialization formats (JSON, BSON, Arrow)
- **Key Performance Requirements**:
  - Process 1000+ spectra in seconds (not minutes)
  - Parallel processing across CPU cores
  - Analytical Jacobian optimization for AUTOBK minimization

### Target Use Cases
- **In-situ Measurements**: Processing thousands of spectra from time-resolved experiments
- **Batch Processing**: High-throughput analysis of spectroscopy datasets
- **Backend Integration**: API for existing Python tools (xraylarch enhancement)
- **Standalone Applications**: Native GUI tools for researchers

**Reference Implementation**: [xraylarch](https://xraypy.github.io/xraylarch/) - Python-based XAS analysis suite

## Important Constraints

### Performance Requirements
- **Target**: 10x+ speedup over Python/NumPy/xraylarch baseline
- **Benchmark**: M1 MacBook Pro (10 cores) processes 10,000 spectra in ~7.5 seconds vs. 145 seconds for NumPy+xraylarch
- **Parallelism**: Must leverage Rayon for multi-core processing

### Scientific Accuracy
- **Compatibility**: Must maintain API compatibility with xraylarch where applicable
- **Precision**: Scientific accuracy required for numerical algorithms (`TEST_TOL = 1e-12`)
- Must maintain compatibility with xraylarch results
- AUTOBK optimization requires analytical Jacobian for performance

### Cross-Platform Support
- Support Rust native, Python bindings, potential WebAssembly
- Python compatibility: >=3.8
- Must support Linux, macOS, Windows
- Future targets: JavaScript/TypeScript, GUI frameworks (Tauri, Dioxus)

### Development Constraints
- **Workspace Resolver**: Using resolver = "2" for dependency resolution
- **Default Members**: crates/* only (examples excluded)
- **Feature Flags**: Selective feature enabling (e.g., ndarray approx/serde features)
- **Memory**: Efficient memory usage for large dataset processing

### Data Formats
- Input: Text-based data files (via `data_reader`)
- Serialization: JSON and BSON support for XAS data structures
- Must handle large datasets efficiently

### Future Development Priorities
1. EXAFS helper functions (rebinning, etc.)
2. Complete Python wrapper (py-xraytsubaki)
3. GUI application using Dioxus (xraytsubaki-gui)
4. WebAssembly version for web applications

## External Dependencies

### Referenced Projects
- **xraylarch**: Primary reference implementation (Python-based)
  - Source of algorithm specifications and validation data
  - Target for API compatibility where beneficial
  - URL: https://xraypy.github.io/xraylarch/
- Synchrotron beamline data formats (various text-based formats)

### Rust Ecosystem
- **ndarray ecosystem** - NumPy-equivalent array operations
- **nalgebra** - Alternative linear algebra backend
- **Rayon** - Work-stealing parallelism
- **FFT libraries** - Fast Fourier Transform implementations

### Python Ecosystem (for bindings)
- **maturin** (>=1.4, <2.0) - Rust→Python build toolchain
- **PyO3** - Rust bindings for Python interpreter / cross-language usage
- Target deployment: PyPI package `xraytsubaki`

### Planned Integrations
- **Tauri**: Native GUI application framework (alternative)
- **Dioxus**: Rust-native GUI framework (planned)
- **WebAssembly**: Browser-based analysis tools (future)
- GUI frameworks: Tauri, Dioxus
- Additional language bindings beyond Python

### Data Exchange Formats
- **Apache Arrow**: High-performance columnar data (via serde_arrow)
- **JSON**: Human-readable serialization
- **BSON**: Binary JSON for efficiency
