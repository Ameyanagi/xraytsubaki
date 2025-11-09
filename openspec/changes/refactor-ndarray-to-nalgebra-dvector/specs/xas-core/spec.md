# XAS Core Data Structures and Algorithms - Delta Specification

## MODIFIED Requirements

### Requirement: Vector Data Type Standard
The system SHALL use `nalgebra::DVector<f64>` as the standard type for all 1D vector operations in XAS data processing, replacing `ndarray::ArrayBase<OwnedRepr<f64>, Ix1>`.

#### Scenario: XASSpectrum field types
- **GIVEN** a new XASSpectrum instance
- **WHEN** examining field types
- **THEN** all vector fields (energy, mu, k, chi, etc.) SHALL be `Option<DVector<f64>>`
- **AND** no ndarray types SHALL be present

#### Scenario: Vector creation from Vec
- **GIVEN** spectroscopy data as `Vec<f64>`
- **WHEN** creating DVector: `DVector::from_vec(data)`
- **THEN** vector SHALL be heap-allocated with runtime size
- **AND** shall support all linear algebra operations

### Requirement: Mathematical Utility Operations
The system SHALL implement the `MathUtils` trait for `DVector<f64>` to provide gaussian, lorentzian, and voigt distribution functions.

#### Scenario: Gaussian peak calculation
- **GIVEN** energy vector as DVector and peak parameters (center, sigma)
- **WHEN** calling `energy.gaussian(center, sigma)`
- **THEN** SHALL return DVector with gaussian distribution
- **AND** peak maximum SHALL occur at center
- **AND** width SHALL be determined by sigma

#### Scenario: Voigt profile calculation
- **GIVEN** energy DVector and parameters (center, sigma, gamma)
- **WHEN** calling `energy.voigt(center, sigma, gamma)`
- **THEN** SHALL return DVector with voigt profile (gaussian-lorentzian convolution)
- **AND** numerical accuracy SHALL be within 1e-12 tolerance

### Requirement: XAS Utility Functions
The system SHALL provide utility functions (find_e0, find_energy_step, smooth, interpolation) that operate on `DVector<f64>` inputs.

#### Scenario: Edge energy determination
- **GIVEN** energy and mu as DVector<f64>
- **WHEN** calling `find_e0(energy, mu)`
- **THEN** SHALL return f64 absorption edge energy
- **AND** result SHALL match xraylarch reference within tolerance
- **AND** shall use derivative-based detection

#### Scenario: Energy step calculation
- **GIVEN** energy DVector
- **WHEN** calling `find_energy_step(energy, frac_ignore, nave, None)`
- **THEN** SHALL return average energy spacing as f64
- **AND** SHALL ignore specified fraction of data points
- **AND** SHALL average over nave points

#### Scenario: Smoothing operation
- **GIVEN** x and y as DVector, smoothing parameters
- **WHEN** calling `smooth(x, y, sigma, conv_form)`
- **THEN** SHALL return smoothed DVector
- **AND** SHALL use specified convolution form (lorentzian/gaussian)
- **AND** output length SHALL match input length

### Requirement: Background Removal (AUTOBK)
The system SHALL implement AUTOBK algorithm using `DVector<f64>` for all vector operations including spline coefficients, knots, and chi calculations.

#### Scenario: AUTOBK spline fitting
- **GIVEN** energy and mu as DVector, normalization result
- **WHEN** calling `calc_background(energy, mu, normalization)`
- **THEN** SHALL fit spline background using Levenberg-Marquardt
- **AND** spline coefficients SHALL be DVector<f64>
- **AND** spline knots SHALL be DVector<f64>
- **AND** SHALL converge to minimize chi-square

#### Scenario: Background subtraction output
- **GIVEN** successfully fitted AUTOBK background
- **WHEN** retrieving results
- **THEN** `get_bkg()` SHALL return `Option<&DVector<f64>>` with background
- **AND** `get_chi()` SHALL return `Option<&DVector<f64>>` with extracted EXAFS
- **AND** `get_k()` SHALL return `Option<&DVector<f64>>` with k-space values
- **AND** chi values SHALL be accurate to 1e-12 relative to reference

### Requirement: Normalization Operations
The system SHALL implement pre-edge and post-edge normalization using `DVector<f64>` for all intermediate and output vectors.

#### Scenario: Pre-post-edge normalization
- **GIVEN** energy and mu as DVector, e0 value
- **WHEN** calling `normalize(energy, mu)` with PrePostEdge method
- **THEN** `get_pre_edge()` SHALL return Option<&DVector<f64>> with pre-edge fit
- **AND** `get_post_edge()` SHALL return Option<&DVector<f64>> with post-edge fit
- **AND** `get_norm()` SHALL return Option<&DVector<f64>> with normalized mu
- **AND** `get_flat()` SHALL return Option<&DVector<f64>> with flattened data
- **AND** normalized data SHALL match reference within TEST_TOL_LESS_ACC (1e-8)

### Requirement: FFT/IFFT Operations
The system SHALL perform forward and inverse Fourier transforms on `DVector<f64>` inputs for k-space to R-space conversion.

#### Scenario: Forward FFT (k-space to R-space)
- **GIVEN** k and chi as DVector<f64>
- **WHEN** calling `xftf(k, chi)` with window parameters
- **THEN** `get_r()` SHALL return Option<&DVector<f64>> with R-space grid
- **AND** `get_chir_mag()` SHALL return Option<&DVector<f64>> with magnitude
- **AND** `get_chir_real()` SHALL return Option<DVector<f64>> with real part
- **AND** `get_chir_imag()` SHALL return Option<DVector<f64>> with imaginary part
- **AND** Parseval's theorem SHALL be satisfied (energy conservation)

#### Scenario: Inverse FFT (R-space to k-space)
- **GIVEN** r and chi_r from forward FFT
- **WHEN** calling `xftr(r, chi_r)` with window parameters
- **THEN** `get_q()` SHALL return Option<&DVector<f64>> with back-transformed k-grid
- **AND** `get_chiq()` SHALL return Option<&DVector<f64>> with filtered chi
- **AND** round-trip FFT→IFFT SHALL preserve signal within numerical precision

### Requirement: Serialization Support
The system SHALL serialize and deserialize `DVector<f64>` fields to JSON and BSON formats using nalgebra's serde integration.

#### Scenario: JSON serialization
- **GIVEN** XASSpectrum with DVector fields populated
- **WHEN** serializing to JSON
- **THEN** each DVector SHALL serialize with structure: `{"data": {"v": [...]}, "nrows": N, "ncols": 1}`
- **AND** round-trip serialization SHALL preserve data exactly

#### Scenario: BSON serialization
- **GIVEN** XASSpectrum with DVector fields
- **WHEN** serializing to BSON
- **THEN** DVector SHALL use compact binary encoding
- **AND** deserialization SHALL reconstruct identical DVector

#### Scenario: Legacy format compatibility
- **GIVEN** data serialized with old ndarray format
- **WHEN** deserializing
- **THEN** system SHOULD support legacy format during transition period
- **AND** SHALL provide conversion utility to new format

### Requirement: Parallel Processing Compatibility
The system SHALL maintain Rayon-based parallel processing capability with DVector operations for batch XAS analysis.

#### Scenario: Parallel edge finding
- **GIVEN** XASGroup with 1000 spectra as DVector
- **WHEN** calling `find_e0_par()`
- **THEN** SHALL process all spectra in parallel using Rayon
- **AND** each spectrum's e0 SHALL be determined independently
- **AND** results SHALL match sequential processing exactly
- **AND** performance SHALL scale with CPU core count

#### Scenario: Parallel normalization
- **GIVEN** XASGroup with DVector spectra
- **WHEN** calling `normalize_par()`
- **THEN** SHALL normalize all spectra in parallel
- **AND** each normalization SHALL use DVector operations
- **AND** memory usage SHALL be reasonable (no excessive allocation)

### Requirement: Performance Preservation
The system SHALL maintain or exceed current performance benchmarks after DVector migration.

#### Scenario: 10,000 spectra processing
- **GIVEN** 10,000 XAS spectra as DVector data
- **WHEN** running full analysis pipeline (e0 → normalize → AUTOBK → FFT)
- **THEN** total processing time SHALL be ≤7.5 seconds on 10-core M1 MacBook Pro
- **AND** performance SHALL represent ≥10x speedup vs Python/NumPy baseline
- **AND** memory usage SHALL not exceed ndarray baseline by >10%

#### Scenario: AUTOBK performance
- **GIVEN** single spectrum requiring background removal
- **WHEN** running AUTOBK with DVector operations
- **THEN** convergence time SHALL be comparable to ndarray implementation (±5%)
- **AND** number of LM iterations SHALL be unchanged

## MODIFIED Requirements (Feature-Gated)

### Requirement: Optional ndarray Array1 Input Support
**Change Type**: Breaking - internal storage only, with optional input compatibility
**Migration**: Internal `ArrayBase<OwnedRepr<f64>, Ix1>` → `DVector<f64>`, inputs via `Into<DVector<f64>>`

The system SHALL use `DVector<f64>` for ALL internal vector storage, but MAY accept `Array1<f64>` inputs when the `ndarray-compat` feature is enabled.

#### Scenario: Default build (pure nalgebra)
- **GIVEN** xraytsubaki compiled without `ndarray-compat` feature
- **WHEN** creating spectrum with `set_spectrum(energy, mu)`
- **THEN** parameters SHALL accept `DVector<f64>` only
- **AND** no ndarray dependency SHALL be present in binary

#### Scenario: Feature-enabled build (ndarray compatibility)
- **GIVEN** xraytsubaki compiled with `ndarray-compat` feature
- **WHEN** creating spectrum with `set_spectrum(energy, mu)`
- **THEN** parameters SHALL accept both `DVector<f64>` and `Array1<f64>` via `Into<DVector<f64>>`
- **AND** Array1 inputs SHALL automatically convert to DVector internally
- **AND** ndarray dependency SHALL be present

### Requirement: Enhanced Conversion Traits
**Change Type**: Enhancement with feature gating
**Migration**: Enhance `nshare.rs` with feature-gated `From<Array1>` for `DVector`

The system SHALL provide bidirectional conversion between `Array1<f64>` and `DVector<f64>` when the `ndarray-compat` feature is enabled.

#### Scenario: Conversion with feature enabled
- **GIVEN** `ndarray-compat` feature enabled
- **WHEN** calling `DVector::from(array1_data)`
- **THEN** conversion SHALL succeed with heap allocation
- **AND** data SHALL be preserved exactly

#### Scenario: Conversion without feature (compilation check)
- **GIVEN** `ndarray-compat` feature NOT enabled
- **WHEN** attempting to compile code using `From<Array1>` for `DVector`
- **THEN** compilation SHALL fail with clear error message
- **AND** user SHALL be directed to enable feature flag

## RENAMED Requirements

None - No requirements renamed, only modified and removed.
