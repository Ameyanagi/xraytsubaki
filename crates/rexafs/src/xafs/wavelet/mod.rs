//! Cauchy wavelet maps of EXAFS χ(k), using the `cauchy_v1` convention (since 0.2.10).
//!
//! [`Wavelet::new`] selects measured k support; [`crate::Spectrum::wavelet`]
//! prepares missing normalization/AUTOBK on a copy. The discrete positive-frequency
//! Cauchy filter peaks at angular frequency 2R. Order is fixed independently of
//! displayed R extent; cropping never changes a retained map. A wavelet feature
//! is not a phase-corrected bond length or a concentration measurement.
//! Scientific motivation: [Muñoz, Argoul and Farges (2003)](https://doi.org/10.2138/am-2003-0423).
mod calculate;
mod map;
#[cfg(test)]
mod tests;
pub use map::{WaveletMap, WaveletPreparation, WaveletRegionValue};
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

/// Invalid inputs/settings or an explicitly cancelled calculation.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WaveletError {
    /// Input, coverage, grid, memory or finite-value validation failed.
    #[error("Wavelet: {0}")]
    Invalid(String),
    /// No partial map is advertised as a complete result.
    #[error("Wavelet calculation cancelled")]
    Cancelled,
    /// Normalization or AUTOBK could not prepare the input.
    #[error("Wavelet preparation: {0}")]
    Preparation(String),
}
type Result<T> = std::result::Result<T, WaveletError>;
fn invalid(message: impl std::fmt::Display) -> WaveletError {
    WaveletError::Invalid(message.to_string())
}

/// Explicit support taper. It multiplies k-weighted χ before the transform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum WaveletWindow {
    /// Hard measured-support boundaries; default for reference comparisons.
    #[default]
    None,
    /// Half-cosine ramps inside both support boundaries, with this width in Å⁻¹.
    /// The two ramps may not overlap. This is an explicit cosine taper, not the
    /// historical XAFS FFT window's half-width/shape convention.
    Cosine {
        /// Width of each half-cosine ramp in Å⁻¹; the two ramps must not overlap.
        width: f64,
    },
}

/// A Cauchy transform definition. Defaults: weight 2, order 100, R up to 6 Å,
/// k step 0.05 Å⁻¹, no taper, and a power-of-two FFT at least twice the grid length.
///
/// ```no_run
/// # use rexafs::{Spectrum, Wavelet};
/// # fn example(spectrum: &Spectrum) -> Result<(), Box<dyn std::error::Error>> {
/// let map = spectrum.wavelet(&Wavelet::new(2.0..=12.0))?;
/// let magnitude = map.magnitude(); // rows are R; columns are k
/// let region = map.integral(4.0..=10.0, 1.0..=3.0)?;
/// # Ok(()) }
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wavelet {
    /// Selected support in Å⁻¹, increasing. Since 0.2.15 endpoints beyond the
    /// measured k range are limited to it during calculation; the map keeps this
    /// requested value in `settings()`, reports the used interval through
    /// `WaveletMap::k_support` and says so in its warnings. A range wholly outside
    /// the measured k range is an error.
    pub k_range: [f64; 2],
    /// Integer k exponent, 0–6, default 2; amplifies high-k contributions/noise.
    pub kweight: u8,
    /// Positive Cauchy order, 1–4096, default 100. Larger orders narrow frequency
    /// response and broaden localization in k; they do not add information.
    pub order: usize,
    /// Uniform output k spacing in Å⁻¹, default 0.05. Linear resampling is explicit
    /// and retained; it does not increase experimental resolution.
    pub kstep: f64,
    /// Maximum generated R in Å, default 6. Only used without explicit radii.
    pub rmax: f64,
    /// Optional numerical R spacing in Å; default π/(FFT length*kstep).
    /// This is sampling, not independent spatial resolution.
    pub rstep: Option<f64>,
    /// Advanced positive, strictly increasing R coordinates (Å). When present,
    /// these replace the generated grid, for exact reference/comparison alignment.
    pub radii: Option<Vec<f64>>,
    /// Optional power-of-two FFT length, at least twice the prepared k grid length.
    /// None chooses the smallest such length; never truncates measured samples.
    pub nfft: Option<usize>,
    /// Explicit support taper; None by default.
    pub window: WaveletWindow,
}
impl Wavelet {
    /// Select a k interval in Å⁻¹, with documented defaults. Construction is cheap;
    /// calculation limits the interval to the measured k range and validates data,
    /// coverage and resource limits.
    pub fn new(k_range: RangeInclusive<f64>) -> Self {
        Self {
            k_range: [*k_range.start(), *k_range.end()],
            kweight: 2,
            order: 100,
            kstep: 0.05,
            rmax: 6.,
            rstep: None,
            radii: None,
            nfft: None,
            window: WaveletWindow::None,
        }
    }
    /// Set integer k weight, 0–6. Does not mutate an existing map.
    pub fn kweight(mut self, value: u8) -> Self {
        self.kweight = value;
        self
    }
    /// Set the Cauchy order, independently of the R grid/display extent.
    pub fn order(mut self, value: usize) -> Self {
        self.order = value;
        self
    }
    /// Set uniform k sampling in Å⁻¹. No extrapolation of measured χ occurs.
    pub fn kstep(mut self, value: f64) -> Self {
        self.kstep = value;
        self
    }
    /// Generate R values up to this positive radius in Å; clears explicit radii.
    pub fn rmax(mut self, value: f64) -> Self {
        self.rmax = value;
        self.radii = None;
        self
    }
    /// Set numerical R sampling in Å; clears explicit radii.
    pub fn rstep(mut self, value: f64) -> Self {
        self.rstep = Some(value);
        self.radii = None;
        self
    }
    /// Use exact, positive increasing radii in Å for reproducible comparisons.
    pub fn radii(mut self, values: Vec<f64>) -> Self {
        self.radii = Some(values);
        self
    }
    /// Set a power-of-two FFT length. Padding refines sampling, not resolution.
    pub fn nfft(mut self, value: usize) -> Self {
        self.nfft = Some(value);
        self
    }
    /// Apply half-cosine ramps of `width` Å⁻¹ inside both measured support endpoints.
    pub fn taper(mut self, width: f64) -> Self {
        self.window = WaveletWindow::Cosine { width };
        self
    }
    /// Transform borrowed unweighted χ(k); returns an owned, immutable scientific map.
    ///
    /// k is in Å⁻¹, finite, nonnegative and strictly increasing; χ is finite and
    /// dimensionless. Irregular samples are linearly resampled onto a zero-origin
    /// grid. Values outside selected measured support are zero padding, recorded
    /// by the support mask. All original samples are retained as provenance.
    /// The selected interval is first limited to the measured k range (since 0.2.15).
    /// Requires at least two samples, an interval overlapping the data, at most
    /// 32768 k grid points/R rows, 262144 FFT samples, 4 million complex output
    /// cells and 64 million R-rows-times-FFT-samples of filtering work. The largest
    /// R must be below the Nyquist radius π/(2*kstep).
    /// No image downsampling, color mapping or normalization of map amplitude occurs.
    pub fn calculate(&self, k: &[f64], chi: &[f64]) -> Result<WaveletMap> {
        self.calculate_with_cancel(k, chi, || false)
    }
    /// Same calculation, with cooperative cancellation before preparation and
    /// between R rows. A true callback returns Cancelled and discards partial output.
    pub fn calculate_with_cancel(
        &self,
        k: &[f64],
        chi: &[f64],
        cancelled: impl Fn() -> bool,
    ) -> Result<WaveletMap> {
        calculate::calculate(self, k, chi, &cancelled)
    }
    /// Validate the grid and estimate scientific output storage before allocation.
    /// Bytes cover complex values, axes and prepared χ/window/mask arrays; FFT
    /// scratch, original-input copies and serialization add bounded overhead.
    pub fn estimate(&self, k: &[f64]) -> Result<WaveletSize> {
        Ok(calculate::layout(self, k)?.size)
    }
}

/// Checked dimensions and estimated output bytes; no transform is computed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaveletSize {
    /// Number of k columns, including explicitly padded lower-k support.
    pub k_points: usize,
    /// Number of strictly positive R rows.
    pub r_points: usize,
    /// Power-of-two transform length.
    pub nfft: usize,
    /// Complex output cells (R rows times k columns).
    pub cells: usize,
    /// Estimated in-memory output buffers, excluding input/FFT/serialization overhead.
    pub bytes: usize,
}

impl crate::Spectrum {
    /// Calculate a Cauchy wavelet map, preparing missing normalization and AUTOBK
    /// on a private copy. Source arrays, parameters and cached results stay unchanged.
    /// Uses existing unweighted χ/k when available. This does not run the ordinary
    /// Fourier transform or borrow its display window. Settings select k support,
    /// weight, order and sampling explicitly. Returns an owned map; no GUI is needed.
    /// The native fluorescence-corrected XANES branch is rejected as unqualified
    /// for EXAFS. Preparation failure returns its diagnostic without a partial map.
    pub fn wavelet(&self, settings: &Wavelet) -> Result<WaveletMap> {
        self.wavelet_with_cancel(settings, || false)
    }
    /// Cooperative cancellation is checked before/after prerequisite processing
    /// and between transform rows; an in-progress AUTOBK solve is not interrupted.
    pub fn wavelet_with_cancel(
        &self,
        settings: &Wavelet,
        cancelled: impl Fn() -> bool,
    ) -> Result<WaveletMap> {
        self.ensure_exafs_allowed()
            .map_err(|e| WaveletError::Preparation(e.to_string()))?;
        if cancelled() {
            return Err(WaveletError::Cancelled);
        }
        let mut prepared;
        let spectrum = if self.k().is_some() && self.chi().is_some() {
            self
        } else {
            prepared = self.clone();
            prepared
                .calc_background()
                .map_err(|e| WaveletError::Preparation(e.to_string()))?;
            &prepared
        };
        let mut map = settings.calculate_with_cancel(
            spectrum.k().ok_or_else(|| invalid("missing prepared k"))?,
            spectrum
                .chi()
                .ok_or_else(|| invalid("missing prepared chi"))?,
            cancelled,
        )?;
        map.data.preparation = Some(WaveletPreparation::from_spectrum(spectrum));
        Ok(map)
    }
}
