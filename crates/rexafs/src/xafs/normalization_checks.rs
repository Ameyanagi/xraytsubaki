//! Checks that flag an untrustworthy pre/post-edge normalization (since 0.2.15).
//!
//! Pre/post-edge normalization fits a polynomial to the post-edge window and
//! extrapolates it back to E0, where the edge step is the post-edge curve
//! minus the pre-edge line. A window with only a few points, or one that covers
//! only a few eV, fits the local EXAFS oscillation rather than the smooth
//! post-edge trend, and the extrapolation over tens of eV can give an edge step
//! that is far too large, near zero or negative. Normalization divides by that
//! step (a non-positive one is floored at 1e-12), so normalized μ(E), χ(k) and
//! χ(R) can be wrong by orders of magnitude while their plots still look like
//! ordinary curves.
//!
//! These checks only report; they never change settings or results, so
//! ordinary data are processed exactly as before. The thresholds are:
//!
//! - [`MIN_POST_EDGE_POINTS`] = 5 measured points. Larch `pre_edge` treats
//!   windows with fewer than five points as too short for a full polynomial and
//!   caps the degree at 1
//!   ([xraylarch `pre_edge.py`, commit 247278c, lines 297–301](https://github.com/xraypy/xraylarch/blob/247278cefbb16eda04c91f79fc1b43df000fa498/larch/xafs/pre_edge.py#L297-L301)).
//!   rexafs applies the same cap and additionally warns, because even a line
//!   through so few points is a poor estimate of the post-edge trend.
//! - [`MIN_POST_EDGE_SPAN_EV`] = 20 eV between the first and last fitted
//!   energies. This is a rexafs heuristic, not a published criterion: one EXAFS
//!   oscillation of a first shell at R ≈ 2.5 Å has a period Δk = π/R ≈ 1.3 Å⁻¹,
//!   which is about 20 eV at k = 2 Å⁻¹ (15 eV above E0) and grows further from
//!   the edge, so a narrower window cannot average over even one oscillation.
//! - [`MIN_EDGE_STEP_FRACTION`] = 10⁻³ of the measured μ span (maximum minus
//!   minimum). A rexafs heuristic: a real absorption edge is normally a large
//!   part of that span, so a step below a thousandth of it, zero or negative
//!   means the post-edge and pre-edge curves meet or cross at E0. Larch instead
//!   uses the absolute value of a negative step
//!   ([`pre_edge.py`, line 313](https://github.com/xraypy/xraylarch/blob/247278cefbb16eda04c91f79fc1b43df000fa498/larch/xafs/pre_edge.py#L313)),
//!   which silently flips the sign of the normalized spectrum; rexafs keeps
//!   its 1e-12 floor and reports the step instead.

use std::fmt;

/// Fewest measured points in a post-edge window that rexafs accepts without a
/// warning; the same count below which Larch `pre_edge` caps the degree at 1.
pub const MIN_POST_EDGE_POINTS: usize = 5;

/// Narrowest post-edge window, in eV between the first and last fitted
/// energies, that rexafs accepts without a warning (rexafs heuristic).
pub const MIN_POST_EDGE_SPAN_EV: f64 = 20.0;

/// Smallest edge step, as a fraction of the measured μ span, that rexafs
/// accepts without a warning (rexafs heuristic).
pub const MIN_EDGE_STEP_FRACTION: f64 = 1e-3;

/// One reason a pre/post-edge normalization should not be trusted.
///
/// Returned by [`PrePostEdge::normalization_warnings`](super::normalization::PrePostEdge::normalization_warnings)
/// and [`Spectrum::normalization_warnings`](crate::Spectrum::normalization_warnings).
/// `Display` gives a short English description for user interfaces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NormalizationWarning {
    /// The post-edge window holds fewer than [`MIN_POST_EDGE_POINTS`] measured points.
    FewPostEdgePoints {
        /// Number of measured points used in the post-edge fit.
        points: usize,
    },
    /// The fitted post-edge energies span less than [`MIN_POST_EDGE_SPAN_EV`].
    NarrowPostEdge {
        /// Last minus first fitted energy, in eV.
        span: f64,
    },
    /// The edge step is non-finite, zero, negative, or below
    /// [`MIN_EDGE_STEP_FRACTION`] of the μ span.
    SmallEdgeStep {
        /// Edge step in μ units before the 1e-12 floor (may be negative).
        edge_step: f64,
        /// Measured μ span (maximum minus minimum) in μ units.
        mu_span: f64,
    },
}

impl fmt::Display for NormalizationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::FewPostEdgePoints { points } => write!(
                f,
                "post-edge fit uses only {points} points (at least {MIN_POST_EDGE_POINTS} needed)"
            ),
            Self::NarrowPostEdge { span } => write!(
                f,
                "post-edge fit spans only {span:.1} eV (at least {MIN_POST_EDGE_SPAN_EV:.0} eV needed)"
            ),
            Self::SmallEdgeStep { edge_step, .. } if !edge_step.is_finite() => {
                write!(f, "edge step is not a finite number ({edge_step})")
            }
            Self::SmallEdgeStep { edge_step, .. } if edge_step < 0.0 => {
                write!(f, "edge step is negative ({edge_step:.3})")
            }
            Self::SmallEdgeStep { edge_step, .. } => {
                write!(f, "edge step ≈ 0 ({edge_step:.3e})")
            }
        }
    }
}

/// Apply the three checks to one post-edge fit.
///
/// `points` and `span` (eV) describe the measured energies used in the
/// post-edge fit; `edge_step` is the step in μ units before the 1e-12 floor;
/// `mu_span` is the measured μ maximum minus minimum. A non-finite `mu_span`
/// skips the relative step test but still flags a non-finite or non-positive step.
pub fn post_edge_warnings(
    points: usize,
    span: f64,
    edge_step: f64,
    mu_span: f64,
) -> Vec<NormalizationWarning> {
    let mut warnings = Vec::new();
    if points < MIN_POST_EDGE_POINTS {
        warnings.push(NormalizationWarning::FewPostEdgePoints { points });
    }
    if span < MIN_POST_EDGE_SPAN_EV {
        warnings.push(NormalizationWarning::NarrowPostEdge { span });
    }
    if let Some(warning) = edge_step_warning(edge_step, mu_span) {
        warnings.push(warning);
    }
    warnings
}

/// The edge-step check alone, for normalizations without a post-edge window
/// (for example MBACK); see [`post_edge_warnings`].
pub fn edge_step_warning(edge_step: f64, mu_span: f64) -> Option<NormalizationWarning> {
    let tiny = mu_span.is_finite() && edge_step < MIN_EDGE_STEP_FRACTION * mu_span;
    (!edge_step.is_finite() || edge_step <= 0.0 || tiny)
        .then_some(NormalizationWarning::SmallEdgeStep { edge_step, mu_span })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_windows_pass() {
        assert!(post_edge_warnings(400, 800.0, 1.2, 2.5).is_empty());
        // The shortest automatic window rexafs picks for a 40 eV scan.
        assert!(post_edge_warnings(50, 25.0, 0.9, 1.4).is_empty());
    }

    #[test]
    fn few_points_follow_larch_and_narrow_windows_are_flagged() {
        assert_eq!(
            post_edge_warnings(4, 30.0, 1.0, 2.0),
            [NormalizationWarning::FewPostEdgePoints { points: 4 }]
        );
        assert!(post_edge_warnings(5, 30.0, 1.0, 2.0).is_empty());
        assert_eq!(
            post_edge_warnings(6, 7.9, 1.0, 2.0),
            [NormalizationWarning::NarrowPostEdge { span: 7.9 }]
        );
    }

    #[test]
    fn negative_tiny_and_non_finite_steps_are_flagged() {
        for step in [-8.1, 0.0, 1e-12, 1e-4, f64::NAN] {
            assert!(edge_step_warning(step, 2.0).is_some(), "{step}");
        }
        assert_eq!(edge_step_warning(0.01, 2.0), None);
        // Without a usable μ span only non-positive steps are flagged.
        assert_eq!(edge_step_warning(1e-6, f64::NAN), None);
        assert!(edge_step_warning(-1.0, f64::NAN).is_some());
    }

    #[test]
    fn messages_name_the_problem() {
        let text = |w: NormalizationWarning| w.to_string();
        assert_eq!(
            text(NormalizationWarning::FewPostEdgePoints { points: 3 }),
            "post-edge fit uses only 3 points (at least 5 needed)"
        );
        assert_eq!(
            text(NormalizationWarning::NarrowPostEdge { span: 7.94 }),
            "post-edge fit spans only 7.9 eV (at least 20 eV needed)"
        );
        assert_eq!(
            text(NormalizationWarning::SmallEdgeStep {
                edge_step: -8.128,
                mu_span: 2.65
            }),
            "edge step is negative (-8.128)"
        );
    }
}
