#![allow(dead_code)]
#![allow(unused_imports)]

use nalgebra::DVector;
use polyfit_rs::polyfit_rs::polyfit;
use serde::{Deserialize, Serialize};

use super::errors::{DataError, NormalizationError};
use super::mathutils::{self, MathUtils};
use super::ranges::RangeAdjustment;
use super::xafsutils;

/// Sample-index windows `[start, end)` and degree used by one normalization;
/// the pre-edge flag selects a constant instead of a line.
struct FitWindows {
    pre: (usize, usize, bool),
    post: (usize, usize),
    polyorder: usize,
    adjustments: Vec<RangeAdjustment>,
}

/// Common interface for absorption normalization and its cached results.
///
/// Energy is in eV; normalized and flattened outputs are dimensionless.
/// Setters on these standalone objects do not clear cached arrays; recalculate
/// after changing settings, or use Spectrum setters to manage invalidation.
pub trait Normalization {
    /// Normalize absorption using the selected method and refresh cached outputs.
    /// Energy is in eV. Use matching finite arrays with sufficient fit points;
    /// method-specific failures are returned as `NormalizationError`.
    /// The low-level PrePostEdge implementation zips its inputs and filters
    /// nonfinite pairs before fitting: unequal lengths can discard the longer
    /// tail, and result arrays then describe only retained pairs. Spectrum
    /// validates finite matching arrays before this step; prefer that contract.
    fn normalize(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
    ) -> Result<&mut Self, NormalizationError>;

    /// Borrow dimensionless edge-step-normalized absorption, or `None` before calculation.
    fn get_norm(&self) -> Option<&DVector<f64>>;
    /// Borrow the flattened normalized absorption, or `None` before calculation.
    fn get_flat(&self) -> Option<&DVector<f64>>;
    /// Read the absorption jump in input mu units, or `None` while automatic/unresolved.
    fn get_edge_step(&self) -> Option<f64>;
    /// Read edge energy in eV, or `None` while automatic/unresolved.
    fn get_e0(&self) -> Option<f64>;
    /// Set edge energy in eV or `None` for automatic detection; recalculate results afterward.
    fn set_e0(&mut self, e0: Option<f64>) -> &mut Self;
    /// Set the jump in mu units or `None` for estimation; recalculate results afterward.
    fn set_edge_step(&mut self, edge_step: Option<f64>) -> &mut Self;
}

/// Absorption normalization algorithm and its settings/results.
///
/// Pre/post-edge normalization remains the default; full Chantler MBACK is
/// available with explicit absorber/edge settings. `new()` uses automatic ranges while `default()`
/// uses `PrePostEdge::default()` with fixed initial ranges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NormalizationMethod {
    /// Implemented pre/post-edge subtraction and edge-step normalization.
    PrePostEdge(PrePostEdge),
    /// Full Chantler MBACK; requires absorber and edge (since 0.2.10).
    MBack(MBack),
}

impl From<MBack> for NormalizationMethod {
    fn from(parameters: MBack) -> Self {
        Self::MBack(parameters)
    }
}
impl From<MBack> for Option<NormalizationMethod> {
    fn from(parameters: MBack) -> Self {
        Some(NormalizationMethod::MBack(parameters))
    }
}

impl From<PrePostEdge> for NormalizationMethod {
    fn from(parameters: PrePostEdge) -> Self {
        Self::PrePostEdge(parameters)
    }
}

// Allow direct settings while preserving existing optional-enum setter calls.
impl From<PrePostEdge> for Option<NormalizationMethod> {
    fn from(parameters: PrePostEdge) -> Self {
        Some(parameters.into())
    }
}

impl Default for NormalizationMethod {
    fn default() -> Self {
        NormalizationMethod::PrePostEdge(PrePostEdge::default())
    }
}

impl NormalizationMethod {
    /// Select pre/post-edge normalization with automatic settings.
    pub fn new() -> NormalizationMethod {
        NormalizationMethod::PrePostEdge(PrePostEdge::new())
    }

    /// Select pre/post-edge normalization with automatic settings.
    pub fn new_prepostedge() -> NormalizationMethod {
        NormalizationMethod::PrePostEdge(PrePostEdge::new())
    }

    /// Create historical empty MBACK settings. Normalization requires an explicit
    /// absorber and edge; prefer `MBack::for_edge(element, edge).into()`.
    pub fn new_mback() -> NormalizationMethod {
        NormalizationMethod::MBack(MBack::new())
    }

    /// Resolve the selected algorithm's automatic settings without fitting.
    ///
    /// For PrePostEdge, use matching finite energy/mu arrays with at least two
    /// nondecreasing energy samples in eV; this resolves ranges and low-level E0
    /// automatically. Spectrum validates an explicit E0 more strictly before
    /// normalizing. MBACK resolves its checked ranges during fitting; this variant performs no work.
    pub fn fill_parameter(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
    ) -> Result<&mut Self, NormalizationError> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => {
                pre_post_edge.fill_parameter(energy, mu)?;
            }
            NormalizationMethod::MBack(mback) => {
                mback.fill_parameter();
            }
        }

        Ok(self)
    }

    /// Run the selected normalization method and replace its cached results.
    ///
    /// Input energy is in eV and mu has a consistent absorption scale.
    /// Insufficient fit points or failed fits return `NormalizationError`.
    /// The low-level PrePostEdge path zips and filters input pairs before
    /// validation, so unequal tails/nonfinite pairs may be discarded; Spectrum
    /// instead rejects such inputs before dispatch. MBACK also rejects invalid arrays,
    /// unsupported atomic data and nonidentifiable fits. Borrowed inputs are unchanged.
    pub fn normalize(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
    ) -> Result<&mut Self, NormalizationError> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => {
                pre_post_edge.normalize(energy, mu)?;
            }
            NormalizationMethod::MBack(mback) => {
                mback.normalize(energy, mu)?;
            }
        }

        Ok(self)
    }

    /// Read the stored edge energy in eV without detecting or validating it.
    /// `None` means no value is stored. Both algorithms resolve a missing E₀ during
    /// normalization; neither detector shifts the atomic reference table.
    pub fn get_e0(&self) -> Option<f64> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => pre_post_edge.get_e0(),
            NormalizationMethod::MBack(mback) => mback.get_e0(),
        }
    }

    /// Read the stored absorption jump in input mu units without calculating it.
    /// `None` means no value is stored. PrePostEdge estimates a missing jump during
    /// normalization; MBACK calculates its step from the matched atomic scale.
    pub fn get_edge_step(&self) -> Option<f64> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => pre_post_edge.get_edge_step(),
            NormalizationMethod::MBack(mback) => mback.get_edge_step(),
        }
    }

    /// Borrow the stored dimensionless flattened absorption without calculating.
    /// PrePostEdge removes its fitted post-edge trend above E0. Returns `None`
    /// when no successful result is stored. MBACK uses its auxiliary fitted trend.
    pub fn get_flat(&self) -> Option<&DVector<f64>> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => pre_post_edge.get_flat(),
            NormalizationMethod::MBack(mback) => mback.get_flat(),
        }
    }

    /// Borrow the stored dimensionless normalized absorption without calculating.
    /// PrePostEdge produces `(mu - pre_edge) / edge_step`. Returns `None` when no
    /// array is stored. MBACK uses `(s*mu-P)/Delta` from its saved auxiliary model.
    pub fn get_norm(&self) -> Option<&DVector<f64>> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => pre_post_edge.get_norm(),
            NormalizationMethod::MBack(mback) => mback.get_norm(),
        }
    }

    /// Store edge energy in eV, or `None` to request PrePostEdge detection.
    /// An explicit value used through Spectrum must be finite and strictly inside
    /// the input energy range. This standalone setter does not validate, recalculate
    /// or clear arrays; use Spectrum setters to invalidate dependent results.
    pub fn set_e0(&mut self, e0: Option<f64>) -> &mut Self {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => {
                pre_post_edge.set_e0(e0);
            }
            NormalizationMethod::MBack(mback) => {
                mback.set_e0(e0);
            }
        }

        self
    }

    /// Store the absorption jump in mu units, or `None` for PrePostEdge estimation.
    /// This standalone setter does not validate, recalculate or clear stored arrays.
    /// For MBACK this is only historical output storage: the next fit determines
    /// its step from atomic data and does not use this value as an override.
    pub fn set_edge_step(&mut self, edge_step: Option<f64>) -> &mut Self {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => {
                pre_post_edge.set_edge_step(edge_step);
            }
            NormalizationMethod::MBack(mback) => {
                mback.set_edge_step(edge_step);
            }
        }

        self
    }
}

/// Pre-edge subtraction, edge-step normalization and post-edge flattening.
///
/// Use `new()` for data-dependent fit ranges and polynomial degree, as in the
/// Python/TypeScript constructors. `default()` instead uses fixed offsets
/// −200/−30/150/2000 eV and degree 2. Endpoints are offsets from E0, not absolute
/// energies. Results are cached in this object after `normalize`.
///
/// The method fits a line to `mu * E.powi(n_victoreen)`, removes that background,
/// and fits a post-edge polynomial to estimate the edge step. See
/// [Newville, Fundamentals of XAFS](https://docs.xrayabsorption.org/tutorials/XAFS_Fundamentals.pdf).
/// The precise automatic ranges and flattening convention are rexafs choices.
///
/// For photon energy E in eV and input absorption mu(E), the fitted models are
/// `p(E) = (a + b*E) * E^(-v)` and a polynomial P(E) fitted to `mu(E)-p(E)`.
/// Here v is `n_victoreen`; coefficients have the units required to match mu.
/// `pre_edge` is p(E), and `post_edge` is p(E)+P(E). If E* is the measured
/// energy nearest E0, the automatic jump is P(E*), in mu units. With the
/// resolved jump D, the dimensionless outputs are
///
/// ```text
/// norm(E) = (mu(E) - p(E)) / D
/// flat(E) = norm(E)                              for E < E*
///         = norm(E) - (P(E) - P(E*)) / D          for E >= E*
/// ```
///
/// The jump must be finite and is floored at 1e-12. This numerical floor is not
/// evidence of a usable absorption edge. Flattening is a separate display
/// output; AUTOBK uses the original absorption and resolved edge step.
/// Automatic values filled into this object are retained. Set a field back to
/// None, or construct fresh settings, to request its automatic resolution again.
///
/// Short scans (since 0.2.15): when a fit window extends beyond the measured
/// energies, normalization limits it to the data instead of failing, following
/// Larch `pre_edge`. The pre-edge start is raised to the first energy and the
/// post-edge end lowered to the last. A pre-edge window lying wholly below the
/// data becomes `[start, start / 2]` with a constant (mean) pre-edge; a post-edge
/// start beyond the data becomes `min(25, 5 * round(end / 15))` eV; and the
/// post-edge degree is capped at 1 when the window has fewer than five points.
/// The stored settings keep the requested values, so longer scans are not
/// affected; [`PrePostEdge::range_adjustments`] lists what was changed. A
/// reversed pair (start at or above end, inside the data) or a window with fewer
/// than two measured points returns [`NormalizationError::InvalidFitRange`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrePostEdge {
    /// Lower pre-edge fit offset from E0 in eV. `new()` leaves this automatic;
    /// `default()` uses −200. Automatic selection follows the available pre-edge data.
    pub pre_edge_start: Option<f64>,
    /// Upper pre-edge fit offset from E0 in eV. `new()` is automatic; `default()`
    /// uses −30. Choose a range below edge structure with enough points for a line.
    pub pre_edge_end: Option<f64>,
    /// Lower post-edge fit offset from E0 in eV. `new()` is automatic; `default()`
    /// uses 150. Automatic selection depends on the resolved upper endpoint.
    pub norm_start: Option<f64>,
    /// Upper post-edge fit offset from E0 in eV. `new()` uses the available range;
    /// `default()` uses 2000. Fit intervals are evaluated on available samples.
    pub norm_end: Option<f64>,
    /// Degree of the post-edge polynomial. Automatic degrees are 0, 1, or 2 for
    /// spans below 50 eV, below 350 eV, or at least 350 eV; explicit values are
    /// clamped to 0–5. `new()` is automatic and `default()` uses 2.
    pub norm_polyorder: Option<i32>,
    /// Exponent v in the pre-edge model `(a + b*E) * E.powi(-v)`.
    /// The line is fitted to `mu * E.powi(v)`; automatic v is 0 (ordinary line).
    pub n_victoreen: Option<i32>,
    /// Absorption edge energy in eV; `None` requests automatic detection.
    /// When using Spectrum, an explicit E0 must be finite and strictly inside the
    /// energy range or normalization returns `E0OutOfRange`. The lower-level
    /// `PrePostEdge::fill_parameter` instead replaces a nonfinite value or one
    /// outside the first through penultimate input energies with a derivative estimate.
    pub e0: Option<f64>,
    /// Absorption jump in the same units as mu. `None` uses post-edge minus
    /// pre-edge at the energy sample nearest E0. A finite result is floored at
    /// 1e-12; inspect nonpositive or tiny jumps rather than trusting that floor.
    pub edge_step: Option<f64>,
    /// Fitted pre-edge background on the energy grid, in input mu units.
    /// `None` until normalization runs.
    pub pre_edge: Option<DVector<f64>>,
    /// Pre-edge background plus the post-edge polynomial, in input mu units.
    /// `None` until normalization runs.
    pub post_edge: Option<DVector<f64>>,
    /// Dimensionless `(mu - pre_edge) / edge_step` on the energy grid.
    /// `None` until normalization runs.
    pub norm: Option<DVector<f64>>,
    /// Dimensionless normalized absorption with the post-edge trend removed above
    /// the sample nearest E0. Values below that sample equal `norm`. This is a
    /// separate display output and is not substituted into AUTOBK.
    pub flat: Option<DVector<f64>>,
    /// Fitted line coefficients `[a, b]` for `mu * E.powi(n_victoreen)`.
    /// Coefficient units depend on the exponent and the use of E in eV.
    pub pre_coefficients: Option<Vec<f64>>,
    /// Post-edge polynomial coefficients in increasing powers of absolute E in eV.
    /// Their weighted sum has input mu units; they are not powers of E − E0.
    pub norm_coefficients: Option<Vec<f64>>,
}

impl Default for PrePostEdge {
    fn default() -> Self {
        PrePostEdge {
            pre_edge_start: Some(-200.0),
            pre_edge_end: Some(-30.0),
            norm_start: Some(150.0),
            norm_end: Some(2000.0),
            norm_polyorder: Some(2),
            n_victoreen: Some(0),
            e0: None,
            edge_step: None,
            pre_edge: None,
            post_edge: None,
            norm: None,
            flat: None,
            norm_coefficients: None,
            pre_coefficients: None,
        }
    }
}

impl PrePostEdge {
    const MAX_NORM_POLYORDER: i32 = 5;

    /// Create automatic normalization settings.
    ///
    /// Unlike `default()`, every configurable field starts as `None`; the next
    /// calculation resolves fit ranges, degree, edge energy and edge step from data.
    pub fn new() -> PrePostEdge {
        PrePostEdge {
            pre_edge_start: None,
            pre_edge_end: None,
            norm_start: None,
            norm_end: None,
            norm_polyorder: None,
            n_victoreen: None,
            e0: None,
            edge_step: None,
            pre_edge: None,
            post_edge: None,
            norm: None,
            flat: None,
            norm_coefficients: None,
            pre_coefficients: None,
        }
    }

    /// Resolve automatic settings from equal-length finite energy/mu arrays.
    ///
    /// Requires at least two samples and nondecreasing energy in eV. This mutates
    /// settings but does not fit the backgrounds or refresh result arrays.
    /// Resolved values remain stored; reset a field to None or create a fresh
    /// object to request automatic selection again. Explicit range endpoints are
    /// stored unchanged; limiting them to the data happens during normalization.
    pub fn fill_parameter(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
    ) -> Result<&mut Self, NormalizationError> {
        Self::validate_inputs(energy, mu)?;

        let mut e0 = self.e0.unwrap_or(f64::NAN);
        if !e0.is_finite() || e0 > energy[energy.len() - 2] || e0 < energy[0] {
            e0 = xafsutils::find_e0(energy, mu)?;
            self.e0 = Some(e0);
        }

        let energy_slice = energy.as_slice();
        let ie0 = mathutils::index_nearest_sorted(energy_slice, &e0)?;
        let e0 = energy[ie0];

        let n_victoreen = self.n_victoreen.unwrap_or(0);
        self.n_victoreen = Some(n_victoreen);

        let mut pre_edge_start = self.pre_edge_start.unwrap_or_else(|| {
            let estimate = if ie0 > 20 {
                5.0 * ((energy[1] - e0) / 5.0).round()
            } else {
                2.0 * ((energy[1] - e0) / 2.0).round()
            };
            estimate.max(energy.min() - e0)
        });

        let mut pre_edge_end = self
            .pre_edge_end
            .unwrap_or_else(|| 5.0 * (pre_edge_start / 15.0).round());

        // Explicit values are never swapped or rewritten: normalization limits
        // endpoints beyond the data and reports a reversed pair as an error
        // (since 0.2.15). Only fully automatic pairs keep the legacy reordering.
        let explicit_pre = self.pre_edge_start.is_some() || self.pre_edge_end.is_some();
        if pre_edge_start > pre_edge_end && !explicit_pre {
            std::mem::swap(&mut pre_edge_start, &mut pre_edge_end);
        }

        let explicit_norm = self.norm_start.is_some() || self.norm_end.is_some();
        let mut norm_end = self.norm_end.unwrap_or_else(|| {
            let estimate = 5.0 * ((energy.max() - e0) / 5.0).round();
            let estimate = if estimate < 0.0 {
                energy.max() - e0 - estimate
            } else {
                estimate
            };
            estimate.min(energy.max() - e0)
        });

        let mut norm_start = self
            .norm_start
            .unwrap_or_else(|| (5.0 * (norm_end / 15.0).round()).min(25.0));

        if !explicit_norm {
            if norm_start > norm_end + 5.0 {
                std::mem::swap(&mut norm_start, &mut norm_end);
            }
            norm_start = norm_start.min(norm_end - 10.0);
        }

        let mut norm_polyorder = self.norm_polyorder.unwrap_or_else(|| {
            let diff = norm_end - norm_start;
            if diff < 50.0 {
                0
            } else if diff < 350.0 {
                1
            } else {
                2
            }
        });
        norm_polyorder = norm_polyorder.clamp(0, PrePostEdge::MAX_NORM_POLYORDER);

        self.pre_edge_start = Some(pre_edge_start);
        self.pre_edge_end = Some(pre_edge_end);
        self.norm_start = Some(norm_start);
        self.norm_end = Some(norm_end);
        self.norm_polyorder = Some(norm_polyorder);

        Ok(self)
    }

    /// Fit windows used by [`Normalization::normalize`] after limiting the
    /// resolved settings to `energy` (sorted, finite, in eV). Requires the
    /// settings filled by [`Self::fill_parameter`]. Stored settings are unchanged.
    ///
    /// The rules follow Larch `pre_edge` (see [`super::ranges`]):
    /// - the pre-edge start is raised to the first measured energy;
    /// - if the whole pre-edge window lies below the data, its end becomes
    ///   `start / 2` and the pre-edge is a constant (the mean absorption). This
    ///   fallback is a rexafs choice: Larch swaps such explicit bounds and may fit
    ///   from the first sample, and it selects a constant pre-edge whenever the
    ///   window spans fewer than three samples, whereas rexafs does so only when
    ///   the window lies wholly below the data (a two-point window inside the data
    ///   stays linear, as before);
    /// - a data-limited pre-edge window with one point keeps two points, as Larch does;
    /// - the post-edge end is lowered to the last measured energy;
    /// - if the post-edge start lies at or beyond that end, it becomes the
    ///   automatic value `min(25, 5 * round(end / 15))` eV, and it always stays
    ///   at least 10 eV below the end;
    /// - the polynomial degree is capped at 1 when the window has fewer than five points.
    ///
    /// A reversed range, or a window that still holds fewer than two measured
    /// points, returns [`NormalizationError::InvalidFitRange`] naming the setting.
    fn fit_windows(&self, energy: &[f64]) -> Result<FitWindows, NormalizationError> {
        let missing = |field: &str| -> NormalizationError {
            DataError::MissingData {
                field: field.to_string(),
            }
            .into()
        };
        let e0 = self.e0.ok_or_else(|| missing("e0"))?;
        let pre_start = self
            .pre_edge_start
            .ok_or_else(|| missing("pre_edge_start"))?;
        let pre_end = self.pre_edge_end.ok_or_else(|| missing("pre_edge_end"))?;
        let norm_start = self.norm_start.ok_or_else(|| missing("norm_start"))?;
        let norm_end = self.norm_end.ok_or_else(|| missing("norm_end"))?;
        let polyorder = self
            .norm_polyorder
            .ok_or_else(|| missing("norm_polyorder"))?;
        let n = energy.len();
        let lo = energy[0] - e0;
        let hi = energy[n - 1] - e0;
        let invalid = |field: &str, reason: String| NormalizationError::InvalidFitRange {
            field: field.to_string(),
            reason,
        };
        // Check before clamping: f64::min/max would silently replace a NaN.
        for (field, value) in [
            ("pre_edge_start", pre_start),
            ("pre_edge_end", pre_end),
            ("norm_start", norm_start),
            ("norm_end", norm_end),
        ] {
            if !value.is_finite() {
                return Err(invalid(field, format!("{value} eV must be finite")));
            }
        }
        let mut adjustments = Vec::new();

        // Pre-edge: start limited to the data (Larch: pre1 = max(pre1, emin - e0)).
        if pre_start >= pre_end && pre_end > lo {
            return Err(invalid(
                "pre_edge_start",
                format!("{pre_start} eV must be below pre_edge_end ({pre_end} eV)"),
            ));
        }
        let start = pre_start.max(lo);
        let mut end = pre_end;
        // A window entirely below the data falls back to Larch's automatic end
        // pre2 = pre1 / 2 (pre_edge.py, line 234) and a constant pre-edge, as
        // Larch uses for short windows (lines 243–245): a line fitted to a few
        // eV just below the edge would be extrapolated far into the post-edge.
        let constant = end <= start;
        if constant {
            end = 0.5 * start;
        }
        let p1 = mathutils::index_of_sorted(energy, &(start + e0))?;
        let mut p2 = mathutils::index_nearest_sorted(energy, &(end + e0))?;
        let limited = start != pre_start || end != pre_end;
        if p2 < p1 + 2 {
            if !limited || p1 + 2 >= n {
                return Err(invalid(
                    "pre_edge_end",
                    format!(
                        "the pre-edge range {start:.1} to {end:.1} eV holds fewer than two measured points; \
                         the scan starts {lo:.1} eV from E0"
                    ),
                ));
            }
            p2 = p1 + 2;
            end = energy[p2] - e0;
        }
        for (field, requested, used) in [
            ("pre_edge_start", pre_start, start),
            ("pre_edge_end", pre_end, end),
        ] {
            if used != requested {
                adjustments.push(RangeAdjustment {
                    field,
                    requested,
                    used,
                });
            }
        }
        let pre = (p1, p2, constant);

        // Post-edge: end limited to the data (Larch: norm2 = min(norm2, emax - e0)).
        // A small tolerance treats an end computed from the last energy as "at the end".
        let tolerance = 1e-6 * hi.abs().max(1.0);
        let at_end = norm_end >= hi - tolerance;
        if norm_start >= norm_end && !at_end {
            return Err(invalid(
                "norm_start",
                format!("{norm_start} eV must be below norm_end ({norm_end} eV)"),
            ));
        }
        let end = norm_end.min(hi);
        let mut start = norm_start;
        if start >= end {
            start = (5.0 * (end / 15.0).round()).min(25.0);
        }
        start = start.min(end - 10.0);
        if end < norm_end && start < 0.0 {
            return Err(invalid(
                "norm_end",
                format!(
                    "the scan ends {hi:.1} eV above E0, too close to the edge for a post-edge fit"
                ),
            ));
        }
        let p1 = mathutils::index_of_sorted(energy, &(start + e0))?;
        let p2 = mathutils::index_nearest_sorted(energy, &(end + e0))?;
        if p2 < p1 + 2 {
            return Err(invalid(
                "norm_start",
                format!(
                    "the post-edge range {start:.1} to {end:.1} eV holds fewer than two measured points; \
                     the scan ends {hi:.1} eV above E0"
                ),
            ));
        }
        // Larch caps the degree at 1 for windows of fewer than five points
        // (pre_edge.py, lines 297–301); the window always holds at least two.
        let order = if p2 - p1 < 5 {
            polyorder.clamp(0, 1) as usize
        } else {
            polyorder.max(0) as usize
        };
        for (field, requested, used) in [
            ("norm_start", norm_start, start),
            ("norm_end", norm_end, end),
            ("norm_polyorder", polyorder as f64, order as f64),
        ] {
            if used != requested && !(field == "norm_end" && used >= requested - tolerance) {
                adjustments.push(RangeAdjustment {
                    field,
                    requested,
                    used,
                });
            }
        }
        Ok(FitWindows {
            pre,
            post: (p1, p2),
            polyorder: order,
            adjustments,
        })
    }

    /// Settings that normalization limits to the measured data (since 0.2.15).
    ///
    /// Recomputes the limits from the current settings and the supplied `energy`
    /// grid in eV (normally the grid that was normalized); it does not record the
    /// last calculation. After `fill_parameter` or `normalize` the settings are
    /// resolved, so this can be called before fitting too. Returns an empty list
    /// when every range fits inside the data, when settings are still unresolved,
    /// or when the ranges are invalid. Fits nothing and changes nothing.
    pub fn range_adjustments(&self, energy: &DVector<f64>) -> Vec<RangeAdjustment> {
        if energy.is_empty() {
            return Vec::new();
        }
        self.fit_windows(energy.as_slice())
            .map(|windows| windows.adjustments)
            .unwrap_or_default()
    }

    fn validate_inputs(energy: &DVector<f64>, mu: &DVector<f64>) -> Result<(), NormalizationError> {
        if energy.len() != mu.len() {
            return Err(DataError::LengthMismatch {
                energy_len: energy.len(),
                mu_len: mu.len(),
            }
            .into());
        }
        if energy.len() < 2 {
            return Err(DataError::InsufficientData {
                min: 2,
                actual: energy.len(),
            }
            .into());
        }

        let non_finite = energy
            .iter()
            .zip(mu.iter())
            .enumerate()
            .filter_map(|(index, (e, m))| (!e.is_finite() || !m.is_finite()).then_some(index))
            .collect::<Vec<_>>();
        if !non_finite.is_empty() {
            return Err(DataError::NonFiniteValues {
                indices: non_finite,
            }
            .into());
        }

        for index in 1..energy.len() {
            let prev = energy[index - 1];
            let curr = energy[index];
            if curr < prev {
                return Err(DataError::NonMonotonicEnergy { index, prev, curr }.into());
            }
        }

        Ok(())
    }

    /// Read the stored value without recalculating.
    ///
    /// Lower pre-edge fit offset from E0 in eV. `new()` leaves this automatic;
    /// `default()` uses −200. Automatic selection follows the available pre-edge data.
    pub fn get_pre_edge_start(&self) -> Option<f64> {
        self.pre_edge_start
    }

    /// Read the stored value without recalculating.
    ///
    /// Upper pre-edge fit offset from E0 in eV. `new()` is automatic; `default()`
    /// uses −30. Choose a range below edge structure with enough points for a line.
    pub fn get_pre_edge_end(&self) -> Option<f64> {
        self.pre_edge_end
    }

    /// Read the stored value without recalculating.
    ///
    /// Lower post-edge fit offset from E0 in eV. `new()` is automatic; `default()`
    /// uses 150. Automatic selection depends on the resolved upper endpoint.
    pub fn get_norm_start(&self) -> Option<f64> {
        self.norm_start
    }

    /// Read the stored value without recalculating.
    ///
    /// Upper post-edge fit offset from E0 in eV. `new()` uses the available range;
    /// `default()` uses 2000. Fit intervals are evaluated on available samples.
    pub fn get_norm_end(&self) -> Option<f64> {
        self.norm_end
    }

    /// Read the stored value without recalculating.
    ///
    /// Degree of the post-edge polynomial. Automatic degrees are 0, 1, or 2 for
    /// spans below 50 eV, below 350 eV, or at least 350 eV; explicit values are
    /// clamped to 0–5. `new()` is automatic and `default()` uses 2.
    pub fn get_norm_polyorder(&self) -> Option<i32> {
        self.norm_polyorder
    }

    /// Read the stored value without recalculating.
    ///
    /// Exponent v in the pre-edge model `(a + b*E) * E.powi(-v)`.
    /// The line is fitted to `mu * E.powi(v)`; automatic v is 0 (ordinary line).
    pub fn get_n_victoreen(&self) -> Option<i32> {
        self.n_victoreen
    }

    /// Read the stored value without recalculating.
    ///
    /// Fitted pre-edge background on the energy grid, in input mu units.
    /// `None` until normalization runs.
    pub fn get_pre_edge(&self) -> Option<&DVector<f64>> {
        self.pre_edge.as_ref()
    }

    /// Read the stored value without recalculating.
    ///
    /// Pre-edge background plus the post-edge polynomial, in input mu units.
    /// `None` until normalization runs.
    pub fn get_post_edge(&self) -> Option<&DVector<f64>> {
        self.post_edge.as_ref()
    }

    /// Read the stored value without recalculating.
    ///
    /// Post-edge polynomial coefficients in increasing powers of absolute E in eV.
    /// Their weighted sum has input mu units; they are not powers of E − E0.
    pub fn get_norm_coefficients(&self) -> Option<&Vec<f64>> {
        self.norm_coefficients.as_ref()
    }

    /// Read the stored value without recalculating.
    ///
    /// Fitted line coefficients `[a, b]` for `mu * E.powi(n_victoreen)`.
    /// Coefficient units depend on the exponent and the use of E in eV.
    pub fn get_pre_coefficients(&self) -> Option<&Vec<f64>> {
        self.pre_coefficients.as_ref()
    }
}

impl Normalization for PrePostEdge {
    fn normalize(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
    ) -> Result<&mut Self, NormalizationError> {
        let (energy, mu): (Vec<f64>, Vec<f64>) = energy
            .iter()
            .zip(mu.iter())
            .filter(|(e, m)| e.is_finite() && m.is_finite())
            .map(|(e, m)| (*e, *m))
            .unzip();
        let energy = DVector::from_vec(energy);
        let mu = DVector::from_vec(mu);
        Self::validate_inputs(&energy, &mu)?;

        self.fill_parameter(&energy, &mu)?;

        let e0 = self.e0.ok_or_else(|| DataError::MissingData {
            field: "e0".to_string(),
        })?;
        let nvict = self.n_victoreen.unwrap_or(0);

        let energy_slice = energy.as_slice();
        let windows = self.fit_windows(energy_slice)?;
        let (p1, p2, constant) = windows.pre;
        let nvict = if constant { 0 } else { nvict };

        let mut energy_x = Vec::with_capacity(p2 - p1);
        let mut mu_x = Vec::with_capacity(p2 - p1);
        for i in p1..p2 {
            let e = energy[i];
            let y = mu[i] * e.powi(nvict);
            if e.is_finite() && y.is_finite() {
                energy_x.push(e);
                mu_x.push(y);
            }
        }
        if energy_x.len() < 2 {
            return Err(NormalizationError::PreEdgeFitFailed {
                start: energy[p1],
                end: energy[p2 - 1],
            });
        }

        let pre_coefficients: Vec<f64> = if constant {
            vec![mu_x.iter().sum::<f64>() / mu_x.len() as f64, 0.0]
        } else {
            polyfit(&energy_x, &mu_x, 1)?
        };

        let mut pre_edge = DVector::zeros(energy.len());
        for i in 0..energy.len() {
            pre_edge[i] =
                (energy[i] * pre_coefficients[1] + pre_coefficients[0]) * energy[i].powi(-nvict);
        }

        let (p1, p2) = windows.post;
        let norm_polyorder = windows.polyorder;
        let mut presub = Vec::with_capacity(p2 - p1);
        for i in p1..p2 {
            presub.push(mu[i] - pre_edge[i]);
        }
        let post_coefficients = polyfit(&energy_slice[p1..p2], &presub, norm_polyorder)?;

        let mut post_edge = pre_edge.clone();
        for (i, c) in post_coefficients.iter().enumerate() {
            for j in 0..post_edge.len() {
                post_edge[j] += energy[j].powi(i as i32) * *c;
            }
        }

        let ie0 = mathutils::index_nearest_sorted(energy_slice, &e0)?;
        let edge_step = self.edge_step.unwrap_or(post_edge[ie0] - pre_edge[ie0]);
        if !edge_step.is_finite() {
            return Err(NormalizationError::EdgeStepTooSmall {
                edge_step,
                min: 1.0e-12,
            });
        }
        let edge_step = edge_step.max(1.0e-12);

        let norm = (&mu - &pre_edge) / edge_step;
        let flat_residue = (&post_edge - &pre_edge) / edge_step;

        let mut flat = &norm - &flat_residue;
        let residue_offset = flat_residue[ie0];
        for i in 0..flat.len() {
            flat[i] += residue_offset;
        }
        for i in 0..ie0 {
            flat[i] = norm[i];
        }

        self.edge_step = Some(edge_step);
        self.pre_edge = Some(pre_edge);
        self.post_edge = Some(post_edge);
        self.norm = Some(norm);
        self.flat = Some(flat);
        self.norm_coefficients = Some(post_coefficients);
        self.pre_coefficients = Some(pre_coefficients);

        Ok(self)
    }

    fn get_e0(&self) -> Option<f64> {
        self.e0
    }

    fn get_edge_step(&self) -> Option<f64> {
        self.edge_step
    }

    fn get_flat(&self) -> Option<&DVector<f64>> {
        self.flat.as_ref()
    }

    fn get_norm(&self) -> Option<&DVector<f64>> {
        self.norm.as_ref()
    }

    fn set_e0(&mut self, e0: Option<f64>) -> &mut Self {
        self.e0 = e0;
        self
    }

    fn set_edge_step(&mut self, edge_step: Option<f64>) -> &mut Self {
        self.edge_step = edge_step;
        self
    }
}

/// Full Chantler MBACK model and cached results (since 0.2.10).
pub use super::mback::MBack;

#[cfg(test)]
mod tests {
    use crate::xafs::io;
    use data_reader::reader::load_txt_f64;

    use super::*;
    use crate::xafs::tests::PARAM_LOADTXT;
    use crate::xafs::tests::TOP_DIR;
    use crate::xafs::tests::{TEST_TOL, TEST_TOL_LESS_ACC};
    use approx::assert_abs_diff_eq;
    const ACCEPTABLE_MU_DIFF: f64 = 1e-6;

    #[test]
    fn test_pre_post_edge_fill_parameter() {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let xafs_test_group = io::load_spectrum_QAS_trans(&path).unwrap();

        let mut pre_post_edge = PrePostEdge::new();
        let energy = xafs_test_group.energy.clone().unwrap();
        let mu = xafs_test_group.mu.clone().unwrap();

        let _ = pre_post_edge.fill_parameter(&energy, &mu);

        let expected = PrePostEdge {
            pre_edge_start: Some(-200.0),
            pre_edge_end: Some(-65.0),
            norm_start: Some(25.0),
            norm_end: Some(944.5331719999995),
            norm_polyorder: Some(2),
            n_victoreen: None,
            e0: Some(22118.8),
            edge_step: None,
            pre_edge: None,
            post_edge: None,
            norm: None,
            flat: None,
            norm_coefficients: None,
            pre_coefficients: None,
        };

        assert_abs_diff_eq!(
            pre_post_edge.e0.unwrap(),
            expected.e0.unwrap(),
            epsilon = TEST_TOL
        );

        assert_abs_diff_eq!(
            pre_post_edge.pre_edge_start.unwrap(),
            expected.pre_edge_start.unwrap(),
            epsilon = TEST_TOL
        );

        assert_abs_diff_eq!(
            pre_post_edge.pre_edge_end.unwrap(),
            expected.pre_edge_end.unwrap(),
            epsilon = TEST_TOL
        );

        assert_abs_diff_eq!(
            pre_post_edge.norm_start.unwrap(),
            expected.norm_start.unwrap(),
            epsilon = TEST_TOL
        );

        assert_abs_diff_eq!(
            pre_post_edge.norm_end.unwrap(),
            expected.norm_end.unwrap(),
            epsilon = TEST_TOL
        );

        assert_abs_diff_eq!(
            pre_post_edge.norm_polyorder.unwrap() as f64,
            expected.norm_polyorder.unwrap() as f64,
            epsilon = TEST_TOL
        );
    }

    #[test]
    fn test_normalization() {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let xafs_test_group = io::load_spectrum_QAS_trans(&path).unwrap();
        let energy = xafs_test_group.energy.clone().unwrap();
        let mu = xafs_test_group.mu.clone().unwrap();

        let mut pre_post_edge = PrePostEdge::new();
        let _ = pre_post_edge.fill_parameter(&energy, &mu);
        let _ = pre_post_edge.normalize(&energy, &mu);

        assert_abs_diff_eq!(
            pre_post_edge.edge_step.unwrap(),
            0.862815921384477,
            epsilon = TEST_TOL_LESS_ACC
        );

        pre_post_edge
            .pre_coefficients
            .as_ref()
            .unwrap()
            .iter()
            .zip([-0.05298882571982536, -1.9039451808611713e-7].iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, b, epsilon = TEST_TOL_LESS_ACC));

        pre_post_edge
            .norm_coefficients
            .as_ref()
            .unwrap()
            .iter()
            .zip(
                [
                    8.985714230146124,
                    -0.0005540674890038064,
                    8.446567273641622e-9,
                ]
                .iter(),
            )
            .for_each(|(a, b)| assert_abs_diff_eq!(a, b, epsilon = TEST_TOL_LESS_ACC.sqrt()));

        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_pre_post_edge_expected.dat";
        let reference_dat = load_txt_f64(&path, &PARAM_LOADTXT).unwrap();

        let reference_norm = reference_dat.get_col(4);
        let reference_flat = reference_dat.get_col(5);

        pre_post_edge
            .norm
            .as_ref()
            .unwrap()
            .iter()
            .zip(reference_norm.iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, b, epsilon = TEST_TOL_LESS_ACC));

        pre_post_edge
            .flat
            .as_ref()
            .unwrap()
            .iter()
            .zip(reference_flat.iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, b, epsilon = TEST_TOL_LESS_ACC));

        let larch_norm_path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_preedge_larch.txt";
        let larch_norm = load_txt_f64(&larch_norm_path, &PARAM_LOADTXT).unwrap();

        let norm_expected = larch_norm.get_col(1);

        let expected = PrePostEdge {
            pre_edge_start: Some(-200.0),
            pre_edge_end: Some(-65.0),
            norm_start: Some(25.0),
            norm_end: Some(945.0),
            norm_polyorder: Some(2),
            n_victoreen: None,
            e0: Some(22118.8),
            edge_step: Some(0.8614006777730155),
            pre_edge: None,
            post_edge: None,
            norm: None,
            flat: None,
            norm_coefficients: Some(vec![
                8.985714130708697,
                -0.0005540674801681585,
                8.446567483044725e-09,
            ]),
            pre_coefficients: Some(vec![-5.29888257e-02, -1.90394518e-07]),
        };

        assert_abs_diff_eq!(
            pre_post_edge.e0.unwrap(),
            expected.e0.unwrap(),
            epsilon = TEST_TOL
        );

        pre_post_edge
            .norm
            .as_ref()
            .unwrap()
            .iter()
            .zip(norm_expected.iter())
            .for_each(|(a, b)| {
                assert_abs_diff_eq!(a, b, epsilon = ACCEPTABLE_MU_DIFF);
            });
    }

    /// Synthetic Ni-like edge at 8333 eV: a sloped line plus a smooth unit step.
    fn edge(first: f64, last: f64) -> (DVector<f64>, DVector<f64>) {
        let count = (last - first) as usize + 1;
        let energy = DVector::from_iterator(count, (0..count).map(|i| first + i as f64));
        let mu =
            energy.map(|e| 0.2 - 1e-5 * (e - 8333.0) + 1.0 / (1.0 + (-(e - 8333.0) / 2.0).exp()));
        (energy, mu)
    }

    fn normalized(
        first: f64,
        last: f64,
        settings: PrePostEdge,
    ) -> Result<PrePostEdge, NormalizationError> {
        let (energy, mu) = edge(first, last);
        let mut settings = settings;
        settings.e0 = Some(8333.0);
        settings.normalize(&energy, &mu)?;
        Ok(settings)
    }

    #[test]
    fn short_post_edge_is_limited_to_the_data_and_reported() {
        // The scan ends 119 eV above E0, below the default 150 eV fit minimum.
        let result = normalized(8133.0, 8452.0, PrePostEdge::default()).unwrap();
        let norm = result.get_norm().unwrap();
        assert_abs_diff_eq!(norm[norm.len() - 1], 1.0, epsilon = 0.02);
        // Requested settings are kept for longer scans.
        assert_eq!(result.norm_start, Some(150.0));
        assert_eq!(result.norm_end, Some(2000.0));
        let (energy, _) = edge(8133.0, 8452.0);
        let fields: Vec<_> = result
            .range_adjustments(&energy)
            .iter()
            .map(|a| (a.field, a.used))
            .collect();
        assert_eq!(fields, [("norm_start", 25.0), ("norm_end", 119.0)]);
    }

    #[test]
    fn short_pre_edge_is_limited_to_the_data_and_reported() {
        // The scan starts 15 eV before E0, above the default −30 eV pre-edge end.
        let result = normalized(8318.0, 8800.0, PrePostEdge::default()).unwrap();
        let norm = result.get_norm().unwrap();
        assert_abs_diff_eq!(norm[norm.len() - 1], 1.0, epsilon = 0.02);
        assert_eq!(result.pre_edge_start, Some(-200.0));
        let (energy, _) = edge(8318.0, 8800.0);
        let adjustments = result.range_adjustments(&energy);
        assert_eq!(adjustments[0].field, "pre_edge_start");
        assert_eq!(adjustments[0].used, -15.0);
        assert_eq!(adjustments[1].field, "pre_edge_end");
        assert_eq!(adjustments[1].used, -7.5);
    }

    #[test]
    fn ranges_inside_the_data_are_not_reported() {
        let settings = PrePostEdge {
            norm_end: Some(400.0),
            ..PrePostEdge::default()
        };
        let result = normalized(8133.0, 8800.0, settings).unwrap();
        let (energy, _) = edge(8133.0, 8800.0);
        assert!(result.range_adjustments(&energy).is_empty());
    }

    #[test]
    fn reversed_or_empty_ranges_name_the_setting() {
        let field = |settings: PrePostEdge| match normalized(8100.0, 8800.0, settings) {
            Err(NormalizationError::InvalidFitRange { field, .. }) => field,
            other => panic!("expected InvalidFitRange, got {other:?}"),
        };
        let reversed_pre = PrePostEdge {
            pre_edge_start: Some(-30.0),
            pre_edge_end: Some(-200.0),
            ..PrePostEdge::default()
        };
        assert_eq!(field(reversed_pre), "pre_edge_start");

        let reversed_post = PrePostEdge {
            norm_start: Some(300.0),
            norm_end: Some(150.0),
            ..PrePostEdge::default()
        };
        assert_eq!(field(reversed_post), "norm_start");

        let one_point = PrePostEdge {
            pre_edge_start: Some(-100.0),
            pre_edge_end: Some(-99.6),
            ..PrePostEdge::default()
        };
        assert_eq!(field(one_point), "pre_edge_end");
    }

    #[test]
    fn short_post_edge_windows_cap_the_degree_at_one_like_larch() {
        // 1 eV steps through the edge, then 10 eV steps: 150–190 eV holds four points.
        let energy: Vec<f64> = (8133..=8340)
            .map(f64::from)
            .chain((1..=26).map(|i| 8340.0 + 10.0 * f64::from(i)))
            .collect();
        let energy = DVector::from_vec(energy);
        let mu = energy.map(|e| 0.2 + 1.0 / (1.0 + (-(e - 8333.0) / 2.0).exp()));
        let mut settings = PrePostEdge {
            e0: Some(8333.0),
            norm_start: Some(150.0),
            norm_end: Some(190.0),
            norm_polyorder: Some(3),
            ..PrePostEdge::default()
        };
        settings.normalize(&energy, &mu).unwrap();
        assert_eq!(settings.get_norm_coefficients().unwrap().len(), 2);
        let adjustments = settings.range_adjustments(&energy);
        assert_eq!(
            (
                adjustments[0].field,
                adjustments[0].requested,
                adjustments[0].used
            ),
            ("norm_polyorder", 3.0, 1.0)
        );
        assert_eq!(settings.norm_polyorder, Some(3));
    }

    #[test]
    fn non_finite_endpoints_are_rejected_before_clamping() {
        for (field, settings) in [
            (
                "norm_end",
                PrePostEdge {
                    norm_end: Some(f64::NAN),
                    ..PrePostEdge::default()
                },
            ),
            (
                "pre_edge_start",
                PrePostEdge {
                    pre_edge_start: Some(f64::NEG_INFINITY),
                    ..PrePostEdge::default()
                },
            ),
        ] {
            match normalized(8100.0, 8800.0, settings) {
                Err(NormalizationError::InvalidFitRange { field: name, .. }) => {
                    assert_eq!(name, field)
                }
                other => panic!("expected InvalidFitRange, got {other:?}"),
            }
        }
    }
}
