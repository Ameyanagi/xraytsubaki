//! Process and fit X-ray absorption spectroscopy (XAS) data in Rust.
//!
//! Start with [`Spectrum`], an owned spectrum with mutable processing settings.
//! [`Spectrum::fft`] calculates missing normalization and background stages;
//! [`Spectrum::ifft`] filters the Fourier result back to a real signal.
//! No calculation runs when a result getter is called. Getters return `None`
//! until the relevant stage succeeds, or after its results are invalidated.
//!
//! # A checked processing pipeline
//!
//! ```no_run
//! use rexafs::{PrePostEdge, Spectrum, XrayFFTF, AUTOBK};
//!
//! fn analyze(energy_ev: &[f64], mu: &[f64]) -> rexafs::Result<Spectrum> {
//!     let mut spectrum = Spectrum::from_arrays(energy_ev, mu)?;
//!     spectrum.set_normalization_method(PrePostEdge::new())?;
//!     spectrum.set_background_method(AUTOBK::new())?;
//!     let mut transform = XrayFFTF::default();
//!     transform.kweight = Some(2.0);
//!     spectrum.set_fft(transform).fft()?;
//!     // k() and chi() borrow; chir_mag() returns an owned vector.
//!     println!("E0 = {:?} eV", spectrum.e0());
//!     Ok(spectrum)
//! }
//! ```
//!
//! Energy is in eV, the photoelectron wave number `k` is in Å⁻¹, and Fourier
//! distance `R` is in Å. [`Spectrum::from_arrays`] copies finite, equally sized
//! slices with at least two points and strictly increasing energy. Later stages
//! also need sufficient pre-edge, post-edge and EXAFS coverage. Use the typed
//! [`Error`] to report a failed stage; successful earlier stages can remain stored.
//!
//! # Find the API you need
//!
//! | Entry point | Purpose |
//! | --- | --- |
//! | [`Spectrum`], [`Group`] | One spectrum or a collection, with automatic prerequisite stages. |
//! | [`PrePostEdge`], [`AUTOBK`], [`XrayFFTF`], [`XrayFFTR`] | Defaults and configurable normalization, background and Fourier operations. |
//! | [`tools`] | Calibration, alignment, deglitching, smoothing, rebinning and merging. |
//! | [`analysis`] | Linear combination fitting and principal component analysis. |
//! | [`io`] | QAS/XDI data and Athena project interchange. |
//! | [`structure`] | Structures, clusters, scattering paths and FEFF inputs. |
//! | [`fitting`] | FEFF path models and single, joint or independent fits. |
//! | [`rmc`] | Experimental reverse Monte Carlo refinement of atomic coordinates. |
//!
//! Rust setters take ownership of settings. Call `.clone()` before passing a
//! configuration if you need to retain it. Setters invalidate dependent results;
//! direct edits to legacy public fields require [`Spectrum::invalidate_derived`].
//!
//! # Cargo features
//!
//! `trust-region` is enabled by default and provides optional optimizer support.
//! `plotting` enables the ruviz plot builders. `refeff-runner` and `feff10-runner`
//! enable calculation backends; fitting existing FEFF path files needs neither.
//! `amcsd` enables a local structure catalog; `materials-project` and `cod` enable
//! online sources through the shared `http` feature. `ndarray-compat` selects the
//! historical ndarray implementation instead of the default nalgebra path.
//! Optional backends may need extra platform dependencies. Enabling every feature
//! also selects `ndarray-compat`, so it does not describe the default numerical backend.
//!
//! See the [user guide](https://rexafs.com/docs/libraries/rust/) and
//! [processing theory](https://rexafs.com/docs/science/processing/) for workflows
//! and interpretation. The crate was developed under the codename xraytsubaki.
//!
//! Scientific background: [Newville et al., AUTOBK (1993)](https://doi.org/10.1103/PhysRevB.47.14126)
//! and [Rehr and Albers, XAFS theory (2000)](https://doi.org/10.1103/RevModPhys.72.621).
//! The fixed endpoint penalty is a rexafs-specific choice; Fourier peaks are not automatically phase corrected.

pub mod atomic;
pub mod parser;
#[cfg(feature = "plotting")]
pub mod plot;
pub mod prelude;
pub mod xafs;

pub use xafs::analysis::peakfit::{PeakFit, PeakFitError, PeakFitResult};
pub use xafs::background::{BackgroundMethod, AUTOBK};
pub use xafs::fluorescence::{
    AbsorptionMode, FluorescenceCorrection, FluorescenceCorrectionResult, FluorescenceError,
};
pub use xafs::normalization::{MBack, NormalizationMethod, PrePostEdge};
pub use xafs::ranges::RangeAdjustment;
pub use xafs::xasgroup::XASGroup as Group;
pub use xafs::xasspectrum::XASSpectrum as Spectrum;
pub use xafs::xrayfft::{FFTGrid, XrayFFTF, XrayFFTR};
pub use xafs::{
    analysis, fitting, io, rmc, structure, tools, transform, Result, XAFSError as Error,
};

// Since 0.2.10, native Cauchy wavelet analysis.
pub use crate::xafs::wavelet::{
    Wavelet, WaveletError, WaveletMap, WaveletPreparation, WaveletRegionValue, WaveletSize,
    WaveletWindow,
};
