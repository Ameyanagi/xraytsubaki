# XAS Data Structures Specification (Delta)

## ADDED Requirements

### Requirement: nalgebra DVector for Spectral Data
The system SHALL use `nalgebra::DVector<f64>` as the primary data structure for all one-dimensional spectral data arrays.

#### Scenario: XASSpectrum with DVector fields
- **GIVEN** a new XASSpectrum instance
- **WHEN** setting energy and mu data with `DVector<f64>`
- **THEN** the spectrum stores data as `raw_energy: Option<DVector<f64>>` and `raw_mu: Option<DVector<f64>>`

#### Scenario: DVector creation from Vec
- **GIVEN** energy and mu data as `Vec<f64>`
- **WHEN** creating spectrum with `set_spectrum(energy, mu)`
- **THEN** the system converts Vec to DVector automatically via Into trait

#### Scenario: DVector interpolation
- **GIVEN** a spectrum with raw data
- **WHEN** interpolating to new energy grid
- **THEN** the system uses DVector operations for cubic spline interpolation

### Requirement: nalgebra Version 0.34.1
The system SHALL use nalgebra version 0.34.1 or later for all linear algebra operations.

#### Scenario: Workspace dependency specification
- **GIVEN** workspace Cargo.toml
- **WHEN** specifying nalgebra dependency
- **THEN** version SHALL be "0.34.1" with serde features enabled

#### Scenario: BLAS backend support
- **GIVEN** optional performance optimization
- **WHEN** compiling with BLAS backend
- **THEN** the system MAY enable openblas-system or netlib features

### Requirement: Type-Safe Vector Operations
The system SHALL provide type-safe operations for DVector manipulation without runtime panics for valid data.

#### Scenario: Element-wise operations
- **GIVEN** two DVectors of equal length
- **WHEN** performing element-wise multiplication (chi * k^kweight)
- **THEN** the operation SHALL return a new DVector with correct values

#### Scenario: Vector slicing and indexing
- **GIVEN** a DVector with N elements
- **WHEN** accessing element at valid index i < N
- **THEN** the system SHALL return &f64 reference safely

#### Scenario: Vector sorting with argsort
- **GIVEN** an unsorted energy DVector
- **WHEN** calling argsort equivalent
- **THEN** the system SHALL return indices DVector for sorted order

### Requirement: Conversion Trait Support
The system SHALL provide conversion traits between common Rust types and DVector.

#### Scenario: Vec to DVector conversion
- **GIVEN** `Vec<f64>` data
- **WHEN** using Into<DVector<f64>> trait
- **THEN** the conversion SHALL succeed without copying if possible

#### Scenario: Array to DVector conversion
- **GIVEN** fixed-size array `[f64; N]`
- **WHEN** converting to DVector
- **THEN** the system SHALL accept Into trait conversion

#### Scenario: Iterator to DVector collection
- **GIVEN** iterator over f64 values
- **WHEN** collecting into DVector
- **THEN** the system SHALL support FromIterator trait

## MODIFIED Requirements

### Requirement: XASSpectrum Core Structure
The XASSpectrum struct SHALL store all spectral data using `nalgebra::DVector<f64>` instead of `ndarray::Array1<f64>`.

**Changed from**: ndarray::ArrayBase<OwnedRepr<f64>, Ix1>
**Changed to**: nalgebra::DVector<f64>

#### Scenario: XASSpectrum field types
- **GIVEN** XASSpectrum struct definition
- **WHEN** inspecting field types
- **THEN** all vector fields SHALL be `Option<DVector<f64>>`:
  - raw_energy: Option<DVector<f64>>
  - raw_mu: Option<DVector<f64>>
  - energy: Option<DVector<f64>>
  - mu: Option<DVector<f64>>
  - k: Option<DVector<f64>>
  - chi: Option<DVector<f64>>
  - chi_kweighted: Option<DVector<f64>>
  - chi_r: Option<DVector<f64>>
  - chi_r_mag: Option<DVector<f64>>
  - chi_r_re: Option<DVector<f64>>
  - chi_r_im: Option<DVector<f64>>
  - q: Option<DVector<f64>>

#### Scenario: set_spectrum method signature
- **GIVEN** generic type parameters T, M
- **WHEN** calling set_spectrum::<T, M>(energy: T, mu: M)
- **THEN** T and M SHALL implement Into<DVector<f64>>

#### Scenario: Spectrum data access
- **GIVEN** XASSpectrum with populated data
- **WHEN** calling get_k() or get_chi()
- **THEN** methods SHALL return Option<DVector<f64>>

### Requirement: Background Removal Data Types
The AUTOBK background removal algorithm SHALL use DVector for all internal calculations and results.

**Changed from**: ndarray Array1<f64> for chi, k, bkg arrays
**Changed to**: nalgebra DVector<f64>

#### Scenario: AUTOBK result fields
- **GIVEN** AUTOBK struct after calc_background
- **WHEN** accessing result fields
- **THEN** chi, k, bkg, chie SHALL be Option<DVector<f64>>

#### Scenario: Spline coefficient storage
- **GIVEN** AUTOBK internal spline calculations
- **WHEN** storing spline coefficients
- **THEN** coefficients SHALL be DVector<f64>

### Requirement: Normalization Data Types
The pre-edge and post-edge normalization SHALL use DVector for fitted curves and normalized data.

**Changed from**: ndarray Array1<f64> for norm, flat, pre_edge, post_edge
**Changed to**: nalgebra DVector<f64>

#### Scenario: PrePostEdge result fields
- **GIVEN** PrePostEdge normalization result
- **WHEN** accessing norm, flat, pre_edge, post_edge
- **THEN** all fields SHALL be Option<DVector<f64>>

#### Scenario: Normalization getter methods
- **GIVEN** normalization trait implementation
- **WHEN** calling get_norm() or get_flat()
- **THEN** methods SHALL return Option<&DVector<f64>>

### Requirement: Mathematical Utility Functions
All mathematical utility functions SHALL accept and return DVector types.

**Changed from**: ndarray Array1<f64> parameters and returns
**Changed to**: nalgebra DVector<f64>

#### Scenario: find_e0 function signature
- **GIVEN** energy and mu as DVector<f64>
- **WHEN** calling find_e0(energy, mu)
- **THEN** function SHALL return Result<f64, Box<dyn Error>>

#### Scenario: find_energy_step function
- **GIVEN** energy DVector<f64>
- **WHEN** calling find_energy_step(energy, frac_ignore, nave, None)
- **THEN** function SHALL return f64 step size

#### Scenario: interpolation utilities
- **GIVEN** knots and values as DVector<f64>
- **WHEN** interpolating at new points
- **THEN** function SHALL return DVector<f64> with interpolated values

## REMOVED Requirements

### Requirement: ndarray Array1 Support
**Reason**: Consolidating on nalgebra as single linear algebra library
**Migration**: Replace all Array1<f64> with DVector<f64>

The system SHALL NOT use `ndarray::Array1` or `ndarray::ArrayBase` types for spectral data.

#### Scenario: Dependency removal validation
- **GIVEN** crate Cargo.toml
- **WHEN** listing dependencies
- **THEN** ndarray SHALL NOT appear in dependencies or dev-dependencies

#### Scenario: Import statement validation
- **GIVEN** all source files in xafs module
- **WHEN** searching for "use ndarray"
- **THEN** zero matches SHALL be found

### Requirement: ToNalgebra Conversion Trait
**Reason**: No longer needed when using nalgebra natively
**Migration**: Remove nshare.rs ToNalgebra trait or repurpose for external conversions only

The ToNalgebra trait for converting ndarray to nalgebra types SHALL be removed or deprecated.

#### Scenario: nshare.rs trait removal
- **GIVEN** nshare.rs module
- **WHEN** using nalgebra natively
- **THEN** ToNalgebra trait SHALL be deprecated or removed

#### Scenario: Test code cleanup
- **GIVEN** test files using ToNalgebra
- **WHEN** migrating tests
- **THEN** ToNalgebra calls SHALL be replaced with native DVector operations
