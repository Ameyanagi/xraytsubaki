# Project Context

## Purpose
xraytsubaki is a high-performance X-ray Absorption Spectroscopy (XAS) data analysis tool written in Rust that implements core functionalities from xraylarch. The primary goal is to dramatically accelerate processing of large XAS datasets (1000+ spectra) through Rust's performance capabilities and parallel processing with Rayon (~10x speedup over Python-based tools).

**Key Objectives**:
- Provide phenomenally fast core API for XAS analysis
- Enable cross-language support (Python, JavaScript) via Rust's ecosystem
- Support native GUI applications using modern frameworks like Tauri
- Maintain compatibility with xraylarch workflows while offering superior performance

## Tech Stack

### Core Technologies
- **Rust** (2021 edition) - Primary implementation language
- **Cargo** 1.91.0 - Build system and package manager
- **rustfmt** 1.8.0 - Code formatting

### Key Dependencies
- **ndarray** - N-dimensional array support with serde serialization
- **rayon** - Data parallelism for processing multiple spectra
- **levenberg-marquardt** - Non-linear optimization for AUTOBK
- **nalgebra** - Linear algebra operations
- **easyfft** - Fast Fourier Transform with serde support
- **serde** / **serde_json** / **bson** - Serialization formats
- **data_reader** - Data file I/O
- **itertools** - Iterator utilities
- **polyfit-rs** - Polynomial fitting
- **rusty-fitpack** - Spline fitting
- **pest** - Parser generator

### Python Bindings
- **maturin** (>=1.4, <2.0) - Build backend for Python bindings
- **PyO3** - Rust-Python interop layer
- Targets Python >=3.8

### Development Tools
- **criterion** - Benchmarking framework with HTML reports
- **pprof** - Profiling with flamegraph support

## Project Conventions

### Code Style
- **Formatting**: Use `rustfmt` for all Rust code (standard Rust formatting)
- **Naming Conventions**:
  - `snake_case` for functions, variables, modules
  - `PascalCase` for types, structs, enums
  - `SCREAMING_SNAKE_CASE` for constants
- **Linting**: Allow `dead_code` and `unused_imports` in debug mode for development flexibility
- **Documentation**: Use `//!` for module-level docs, `///` for item-level docs

### Architecture Patterns

**Workspace Structure**:
```
xraytsubaki/
├── crates/
│   ├── xraytsubaki/     # Core library
│   └── xasio/           # I/O utilities
├── py-xraytsubaki/      # Python bindings (maturin)
└── examples/            # Usage examples
```

**Core Modules** (in `crates/xraytsubaki/src/`):
- `xafs/` - EXAFS analysis algorithms
  - `background.rs` / `background_nalgebra.rs` - Background subtraction (AUTOBK)
  - `normalization.rs` / `normalization_nalgebra.rs` - Pre/post-edge normalization
  - `xrayfft.rs` / `xrayfft_nalgebra.rs` - FFT/IFFT operations
  - `xasgroup.rs` / `xasgroup_nalgebra.rs` - Multi-spectrum containers
  - `xasspectrum.rs` / `xasspectrum_nalgebra.rs` - Single spectrum data structures
  - `fitting.rs` / `fitting_nalgebra.rs` - Curve fitting utilities
  - `io/` - BSON/JSON serialization
- `parser/` - Data parsing utilities
- `plot/` - Visualization support
- `prelude.rs` - Common exports

**Design Patterns**:
- **Trait-based abstractions**: `MathUtils`, `Normalization`, `XAFSUtils` traits
- **Error handling**: Custom `XAFSError` enum with `Error` trait implementation
- **Parallelization**: Rayon-based parallel processing for batch operations
- **Dual implementations**: Both standard and nalgebra-based versions for flexibility

### Testing Strategy

**Test Organization**:
- Unit tests: Located in `mod tests` blocks within source files
- Integration tests: `crates/xraytsubaki/src/xafs/tests.rs`
- Test configuration: `#[cfg(test)]` gating

**Test Constants**:
```rust
const TEST_TOL: f64 = 1e-12;           // High-precision tests
const TEST_TOL_LESS_ACC: f64 = 1e-8;   // Lower precision tests
```

**Test Data**:
- Test files loaded via `data_reader` with standardized `ReaderParams`
- Use `CARGO_MANIFEST_DIR` for relative test data paths

**Benchmarking**:
- Criterion-based benchmarks in `benches/`
- Profile with `pprof` and flamegraphs
- Focus on parallel vs. sequential performance comparisons

### Git Workflow
- **Main Branch**: `main` (protected)
- **Commit Style**: Descriptive messages focusing on "why" over "what"
- **Current Status**: Active development with recent refactoring work
- **Licensing**: Dual licensed under MIT OR Apache-2.0

## Domain Context

### X-ray Absorption Spectroscopy (XAS/EXAFS)
XAS is a synchrotron-based technique for probing local atomic structure and electronic states. The analysis pipeline involves:

1. **Energy calibration** (`find_e0`) - Identify absorption edge
2. **Normalization** - Pre-edge and post-edge background fitting
3. **AUTOBK** - Automated background removal using spline fitting with Levenberg-Marquardt optimization
4. **EXAFS extraction** - Extract χ(k) oscillations
5. **Fourier Transform** - Convert k-space to R-space (FFT/IFFT)

**Key Performance Requirements**:
- Process 1000+ spectra in seconds (not minutes)
- Parallel processing across CPU cores
- Analytical Jacobian optimization for AUTOBK minimization

**Reference Implementation**: [xraylarch](https://xraypy.github.io/xraylarch/) - Python-based XAS analysis suite

## Important Constraints

### Performance Requirements
- **Target**: 10x speedup over Python/NumPy/xraylarch baseline
- **Benchmark**: M1 MacBook Pro (10 cores) processes 10,000 spectra in ~7.5 seconds vs. 145 seconds for NumPy+xraylarch
- **Parallelism**: Must leverage Rayon for multi-core processing

### Scientific Accuracy
- Numerical precision: `TEST_TOL = 1e-12` for critical calculations
- Must maintain compatibility with xraylarch results
- AUTOBK optimization requires analytical Jacobian for performance

### Cross-Platform Support
- Support Rust native, Python bindings, potential WebAssembly
- Python compatibility: >=3.8
- Future targets: JavaScript/TypeScript, GUI frameworks (Tauri, Dioxus)

### Data Formats
- Input: Text-based data files (via `data_reader`)
- Serialization: JSON and BSON support for XAS data structures
- Must handle large datasets efficiently

## External Dependencies

### Scientific Libraries
- **xraylarch** (reference implementation) - Python-based XAS analysis
- Synchrotron beamline data formats (various text-based formats)

### Rust Ecosystem
- **ndarray ecosystem** - NumPy-equivalent array operations
- **nalgebra** - Alternative linear algebra backend
- **Rayon** - Work-stealing parallelism
- **FFT libraries** - Fast Fourier Transform implementations

### Python Ecosystem (for bindings)
- **maturin** - Rust→Python build toolchain
- **PyO3** - Rust bindings for Python interpreter
- Target deployment: PyPI package `xraytsubaki`

### Future Integrations (Planned)
- GUI frameworks: Tauri, Dioxus
- WebAssembly for browser-based analysis
- Additional language bindings beyond Python
