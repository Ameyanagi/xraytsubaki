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
- **Python Bindings**: PyO3 (planned in py-xraytsubaki)

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
  - Snake_case for modules, functions, variables
  - PascalCase for types, structs, enums
  - SCREAMING_SNAKE_CASE for constants
- **Module Organization**: Domain-driven structure (xafs, parser, prelude)
- **Imports**: Prefer explicit imports; use prelude module for commonly used items

### Architecture Patterns
- **Workspace Organization**:
  - `crates/xraytsubaki/` - Core library implementation
  - `py-xraytsubaki/` - Python bindings (in development)
  - `xraytsubaki-gui/` - GUI application (planned)

- **Module Structure**:
  - `xafs/` - Core XAS analysis functionality
    - `xasgroup.rs` - Main group data structure
    - `xasparameters.rs` - Analysis parameters
    - `xasspectrum.rs` - Spectrum representation
    - `background.rs` - Background removal (AUTOBK)
    - `normalization.rs` - Pre/post-edge normalization
    - `xrayfft.rs` - FFT/IFFT operations
    - `io/` - Serialization (JSON, BSON)
    - `mathutils.rs`, `xafsutils.rs`, `lmutils.rs` - Utility functions
  - `parser/` - Data parsing functionality
  - `prelude/` - Common exports

- **Optimization Strategy**:
  - Analytical Jacobian for AUTOBK minimization
  - Rayon-based parallelization for batch processing
  - Zero-copy operations where possible

### Testing Strategy
- **Unit Tests**: Embedded in `xafs/tests.rs`
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

## Domain Context

### X-ray Absorption Spectroscopy (XAS)
- **EXAFS**: Extended X-ray Absorption Fine Structure analysis
- **Core Operations**:
  - `find_e0`: Edge energy determination
  - Pre-edge and post-edge normalization
  - **AUTOBK**: Automated background removal using spline fitting with Levenberg-Marquardt optimization
  - FFT/IFFT: Fourier transforms for k-space to R-space conversion

### Scientific Computing Requirements
- **Numerical Precision**: Floating-point operations require approximate comparisons
- **Performance**: Real-time or near-real-time processing of large datasets (1000+ spectra)
- **Reproducibility**: Results must match established tools (xraylarch) for validation
- **Data Formats**: Support for multiple serialization formats (JSON, BSON, Arrow)

### Target Use Cases
- **In-situ Measurements**: Processing thousands of spectra from time-resolved experiments
- **Batch Processing**: High-throughput analysis of spectroscopy datasets
- **Backend Integration**: API for existing Python tools (xraylarch enhancement)
- **Standalone Applications**: Native GUI tools for researchers

## Important Constraints

### Technical Constraints
- **Compatibility**: Must maintain API compatibility with xraylarch where applicable
- **Performance Target**: 10x+ speedup over NumPy + xraylarch implementations
- **Precision**: Scientific accuracy required for numerical algorithms
- **Memory**: Efficient memory usage for large dataset processing

### Development Constraints
- **Workspace Resolver**: Using resolver = "2" for dependency resolution
- **Default Members**: crates/* only (examples excluded)
- **Feature Flags**: Selective feature enabling (e.g., ndarray approx/serde features)
- **Cross-Platform**: Must support Linux, macOS, Windows

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

### Planned Integrations
- **PyO3**: Python bindings for cross-language usage
- **Tauri**: Native GUI application framework (alternative)
- **Dioxus**: Rust-native GUI framework (planned)
- **WebAssembly**: Browser-based analysis tools (future)

### Data Exchange Formats
- **Apache Arrow**: High-performance columnar data (via serde_arrow)
- **JSON**: Human-readable serialization
- **BSON**: Binary JSON for efficiency
