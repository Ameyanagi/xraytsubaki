//! Processing-range endpoints that were limited to the measured data (since 0.2.15).
//!
//! Normalization, AUTOBK background removal and the Cauchy wavelet transform
//! accept fit or transform ranges that can extend beyond a short scan. Rather
//! than failing, rexafs limits such an endpoint to the measured data when the
//! calculation runs. This follows Larch `pre_edge`, which applies
//! `pre1 = max(pre1, emin - e0)` and `norm2 = min(norm2, emax - e0)` to explicit
//! values as well as automatic ones
//! ([xraylarch `pre_edge.py`, commit 247278c, lines 232–259](https://github.com/xraypy/xraylarch/blob/247278cefbb16eda04c91f79fc1b43df000fa498/larch/xafs/pre_edge.py#L232-L259)),
//! and AUTOBK's cap of `kmax` at the measured k range. The stored settings are
//! never rewritten, so the same settings applied to a longer scan use the
//! requested values unchanged. A reversed range, or one that still holds too
//! few measured points, is an error that names the setting.

/// One setting whose requested value was replaced by a data-limited value.
///
/// Energies are offsets from E0 in eV; background k values are in Å⁻¹; the
/// polynomial degree is dimensionless. `requested` is the configured (or
/// automatically resolved) value and `used` is the value applied in the
/// calculation. Use this to tell users that a short scan changed a range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeAdjustment {
    /// Setting name as used by the Rust API: `pre_edge_start`, `pre_edge_end`,
    /// `norm_start`, `norm_end`, `norm_polyorder` (normalization) or `kmax`
    /// (AUTOBK background).
    pub field: &'static str,
    /// Requested value in the setting's units.
    pub requested: f64,
    /// Value used in the calculation, in the same units.
    pub used: f64,
}
