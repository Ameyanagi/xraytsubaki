//! Visible labels and hover explanations for processing-inspector parameters.
//!
//! All label and tooltip text for the Normalize, Background and Transform
//! inspectors lives in this one table so it can be reviewed in one place.
//! A label is short, sentence case and ends with its unit in parentheses
//! (the numeric field moves a trailing "(unit)" into its unit column). A
//! tooltip says in one or two sentences what the parameter does, its unit,
//! the Auto (default) behaviour and the matching Larch or Athena name.
//! "rexafs-specific" marks settings without a Larch or Athena counterpart.
//!
//! Sources: defaults are the desktop values documented on
//! [`crate::params::PipelineParams`]; the pre-edge, AUTOBK and Fourier
//! conventions are described in `doc/processing-theory.md` and
//! `doc/autobk-fixed-penalty.md`. AUTOBK is the method of Newville et al.,
//! Phys. Rev. B 47, 14126 (1993). Larch names are the keyword arguments of
//! `pre_edge`, `autobk`, `xftf` and `xftr`.

use super::{EnumParam, ParamKey};

/// Visible label and optional hover explanation of one inspector parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParamText {
    pub label: &'static str,
    pub tip: Option<&'static str>,
}

const fn text(label: &'static str, tip: &'static str) -> ParamText {
    ParamText {
        label,
        tip: Some(tip),
    }
}

/// Label and tooltip of a numeric inspector field.
pub(crate) fn param_text(key: ParamKey) -> ParamText {
    use ParamKey::*;
    match key {
        // Data stage: outside the scope of this table; labels only.
        ImpEnergyCol => ParamText {
            label: "energy col",
            tip: None,
        },
        ImpI0Col => ParamText {
            label: "I0 col",
            tip: None,
        },
        ImpItCol => ParamText {
            label: "It col",
            tip: None,
        },
        ImpIrCol => ParamText {
            label: "Ir col",
            tip: None,
        },
        AlignTarget => ParamText {
            label: "ref E0 target",
            tip: None,
        },
        EnergyOffset => ParamText {
            label: "offset (eV)",
            tip: None,
        },

        // Normalize.
        E0 => text(
            "E₀ (eV)",
            "Edge energy that sets the origin of k and of every pre-edge and post-edge range. \
             Auto estimates it from the derivative of μ(E). Larch/Athena: e0.",
        ),
        EdgeStep => text(
            "Edge step",
            "Jump in μ(E) at E₀ that normalized μ(E) and χ(k) are divided by, in the units of μ. \
             Auto evaluates the post-edge minus pre-edge curves at E₀. Larch: step; Athena: edge step.",
        ),
        PreEdgeStart => text(
            "Pre-edge start (eV)",
            "Lower end of the pre-edge line fit, in eV relative to E₀. \
             Auto uses −200 eV. Larch: pre1; Athena: pre-edge range.",
        ),
        PreEdgeEnd => text(
            "Pre-edge end (eV)",
            "Upper end of the pre-edge line fit, in eV relative to E₀. \
             Auto uses −30 eV. Larch: pre2; Athena: pre-edge range.",
        ),
        NormStart => text(
            "Post-edge start (eV)",
            "Lower end of the post-edge polynomial fit, in eV relative to E₀. \
             Auto uses +150 eV. Larch: norm1; Athena: normalization range.",
        ),
        NormEnd => text(
            "Post-edge end (eV)",
            "Upper end of the post-edge polynomial fit, in eV relative to E₀. \
             Auto uses the end of the measured spectrum. Larch: norm2; Athena: normalization range.",
        ),
        NormPolyorder => text(
            "Polynomial order",
            "Degree of the post-edge polynomial (0 constant, 1 linear, 2 quadratic). \
             Auto uses 2. Larch: nnorm; Athena: normalization order.",
        ),
        NVictoreen => text(
            "Victoreen exponent n",
            "The pre-edge line is fitted to μ(E)·Eⁿ, so the background falls as E⁻ⁿ; \
             n = 0 (Auto) fits a plain straight line. Larch: nvict.",
        ),

        // Background (AUTOBK; Newville et al. 1993).
        Rbkg => text(
            "Rbkg (Å)",
            "AUTOBK cutoff distance: the spline background is adjusted to remove χ(R) below Rbkg \
             and sets the knot count. Auto uses 1.0 Å; larger values can remove real first-shell \
             signal. Larch/Athena: rbkg.",
        ),
        BkgKmin => text(
            "Spline k min (Å⁻¹)",
            "Lower wave-number limit of the AUTOBK spline fit and its window. \
             Auto uses 0 Å⁻¹. Larch: kmin (autobk); Athena: spline range in k.",
        ),
        BkgKmax => text(
            "Spline k max (Å⁻¹)",
            "Upper wave-number limit of the AUTOBK spline fit and its window. \
             Auto uses the end of the data. Larch: kmax (autobk); Athena: spline range in k.",
        ),
        BkgKstep => text(
            "k grid step (Å⁻¹)",
            "Spacing of the uniform k grid on which χ(k) is returned; it also sets the R grid \
             of the AUTOBK transform. Auto uses 0.05 Å⁻¹. Larch: kstep (autobk).",
        ),
        BkgNknots => text(
            "Spline knots",
            "Number of spline knots. Auto uses 1 + 2·Rbkg·(k max − k min)/π, rounded down \
             and limited to 5–128. Larch: nknots.",
        ),
        BkgKweight => text(
            "Spline k-weight",
            "Power of k applied to χ(k) inside the AUTOBK fit; independent of the Fourier \
             transform k-weight unless linked. Auto uses 1. Larch: kweight (autobk); Athena: \
             background k-weight.",
        ),
        BkgClampLo => text(
            "Low-k clamp",
            "Weight of the penalty that holds χ(k) near zero over the first clamp points. \
             Auto uses 0 (off). Larch: clamp_lo; Athena: spline clamps (low).",
        ),
        BkgClampHi => text(
            "High-k clamp",
            "Weight of the penalty that holds χ(k) near zero over the last clamp points. \
             Auto uses 1; 0 turns it off. Larch: clamp_hi; Athena: spline clamps (high).",
        ),
        BkgNclamp => text(
            "Clamp points",
            "Number of k samples at each clamped end. Auto uses 3; 0 disables clamping. \
             Larch: nclamp.",
        ),
        BkgClampLambda => text(
            "Clamp λ",
            "Strength of the fixed endpoint penalty in the Fixed λ clamp model; 0 disables it. \
             Auto uses 0.001. rexafs-specific (see the fixed-penalty AUTOBK guide).",
        ),
        BkgDk => text(
            "Window dk (Å⁻¹)",
            "Taper width at both ends of the AUTOBK k window. Auto uses 0.1 Å⁻¹. Larch: dk (autobk).",
        ),
        BkgNfft => text(
            "FFT length",
            "Number of points in the AUTOBK Fourier transform; must cover the whole k grid. \
             Auto uses 2048. Larch: nfft (autobk).",
        ),
        BkgEk0 => text(
            "k origin E₀ (eV)",
            "Energy taken as k = 0 when converting E to k for the background. \
             Auto uses the normalization E₀. Larch: ek0.",
        ),
        BkgRegularization => text(
            "Legacy ridge",
            "Ridge magnitude added by the legacy linear solver; unused by Fixed λ. \
             Auto uses 0.0001. rexafs-specific.",
        ),
        BkgCondition => text(
            "Condition limit",
            "Largest accepted ratio of largest to smallest singular value of the scaled linear \
             system; Fixed λ reports an error above it. Auto uses 1e8. rexafs-specific.",
        ),
        BkgResidualRatio => text(
            "Residual ratio limit",
            "Legacy models accept a direct solve only if its residual norm is at most this \
             multiple of the starting residual; unused by Fixed λ. Auto uses 1.05. rexafs-specific.",
        ),

        // Transform: forward k → R.
        FftKmin => text(
            "FT k min (Å⁻¹)",
            "Lower k limit of the forward Fourier-transform window. \
             Auto uses 2 Å⁻¹. Larch: kmin (xftf); Athena: k-range.",
        ),
        FftKmax => text(
            "FT k max (Å⁻¹)",
            "Upper k limit of the forward Fourier-transform window; choose the end of the \
             useful signal. Auto uses 15 Å⁻¹. Larch: kmax (xftf); Athena: k-range.",
        ),
        FftDk => text(
            "Window dk (Å⁻¹)",
            "Taper width at the k-window edges; for Kaiser–Bessel it also sets the window shape. \
             Auto uses 1. Larch/Athena: dk.",
        ),
        FftKweight => text(
            "FT k-weight",
            "Power w in χ(k)·kʷ before the transform; non-integer values are rounded down. \
             Auto uses 2. Larch: kweight (xftf); Athena: k-weight.",
        ),
        FftDk2 => text(
            "Window dk2 (Å⁻¹)",
            "Taper width at the high-k window edge only. Auto uses dk. Larch: dk2.",
        ),
        FftRmax => text(
            "Output R max (Å)",
            "Largest R kept in the χ(R) output and plot; it is not a fitting range. \
             Auto uses 10 Å. Larch: rmax_out.",
        ),
        FftKstep => text(
            "FT k step (Å⁻¹)",
            "k spacing assumed by the forward transform; it sets the R spacing and amplitude. \
             Auto uses the χ(k) grid spacing. Larch: kstep (xftf).",
        ),
        FftNfft => text(
            "FFT length",
            "Number of points in the forward FFT; more zero-padding gives a finer R grid but \
             no extra resolution. Auto uses 2048. Larch: nfft (xftf).",
        ),

        // Transform: back R → q.
        BftRmin => text(
            "R min (Å)",
            "Lower R limit of the back-transform window. Auto uses 0 Å. \
             Larch: rmin (xftr); Athena: R-range.",
        ),
        BftRmax => text(
            "R max (Å)",
            "Upper R limit of the back-transform window. Auto uses 20 Å. \
             Larch: rmax (xftr); Athena: R-range.",
        ),
        BftDr => text(
            "Window dR (Å)",
            "Taper width at the R-window edges. Auto uses 1. Larch/Athena: dr.",
        ),
        BftDr2 => text(
            "Window dR2 (Å)",
            "Taper width at the high-R window edge only. Auto uses dR. Larch: dr2.",
        ),
        BftRweight => text(
            "R-weight",
            "Power of R applied to χ(R) before the back transform. Auto uses 0. Larch: rw.",
        ),
        BftQmax => text(
            "q max (Å⁻¹)",
            "Largest q kept in the χ(q) output; changes the extent, not the spacing. \
             Auto uses 10 Å⁻¹. Larch: qmax_out.",
        ),
        BftKstep => text(
            "q step (Å⁻¹)",
            "q spacing of χ(q). Auto derives it from the R spacing and the inverse FFT length; \
             an explicit value must agree with them. Larch: kstep (xftr).",
        ),
        BftNfft => text(
            "Inverse FFT length",
            "Number of points in the back transform; it changes the automatic q step. \
             Auto uses 2048. Larch: nfft (xftr).",
        ),
    }
}

/// Label and tooltip of a choice (drop-down) inspector row.
pub(crate) fn enum_text(which: EnumParam) -> ParamText {
    use EnumParam::*;
    match which {
        ImportMode => ParamText {
            label: "Import mode",
            tip: None,
        },
        BkgWindow => text(
            "Window",
            "Window applied to χ(k) in the AUTOBK objective. Auto uses Hanning. Larch: win (autobk).",
        ),
        BkgSolver => text(
            "Method",
            "Optimizer for the spline coefficients. Auto uses the direct linear solve. \
             rexafs-specific.",
        ),
        BkgClampPolicy => text(
            "Clamp model",
            "How endpoint clamps enter the AUTOBK objective. Fixed λ (default) adds a constant \
             penalty; the legacy models reproduce earlier rexafs results. rexafs-specific.",
        ),
        BkgFallback => text(
            "Fallback",
            "Retry a failed legacy direct solve with an iterative solver. Auto: on. rexafs-specific.",
        ),
        BkgCache => text(
            "Matrix cache",
            "Reuse compatible AUTOBK matrices between spectra; results are unchanged. \
             Auto: on. rexafs-specific.",
        ),
        BkgFallbackSolver => text(
            "Fallback solver",
            "Iterative solver used by the legacy fallback. Auto uses dog-leg. rexafs-specific.",
        ),
        FftWindow => text(
            "Window",
            "Shape of the forward k window. Auto uses Kaiser–Bessel. Larch: window (xftf); \
             Athena: window type.",
        ),
        FftGrid => text(
            "Sampling grid",
            "Input transforms χ(k) on its own grid; Larch resamples to the Larch grid for \
             compatibility. rexafs-specific.",
        ),
        BftWindow => text(
            "Window",
            "Shape of the back-transform R window. Auto uses Kaiser–Bessel. Larch: window (xftr); \
             Athena: window type.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSPECTOR_KEYS: &[ParamKey] = &[
        ParamKey::E0,
        ParamKey::EdgeStep,
        ParamKey::PreEdgeStart,
        ParamKey::PreEdgeEnd,
        ParamKey::NormStart,
        ParamKey::NormEnd,
        ParamKey::NormPolyorder,
        ParamKey::NVictoreen,
        ParamKey::Rbkg,
        ParamKey::BkgKmin,
        ParamKey::BkgKmax,
        ParamKey::BkgKstep,
        ParamKey::BkgNknots,
        ParamKey::BkgKweight,
        ParamKey::BkgClampLo,
        ParamKey::BkgClampHi,
        ParamKey::BkgNclamp,
        ParamKey::BkgClampLambda,
        ParamKey::BkgDk,
        ParamKey::BkgNfft,
        ParamKey::BkgEk0,
        ParamKey::BkgRegularization,
        ParamKey::BkgCondition,
        ParamKey::BkgResidualRatio,
        ParamKey::FftKmin,
        ParamKey::FftKmax,
        ParamKey::FftDk,
        ParamKey::FftKweight,
        ParamKey::FftDk2,
        ParamKey::FftRmax,
        ParamKey::FftKstep,
        ParamKey::FftNfft,
        ParamKey::BftRmin,
        ParamKey::BftRmax,
        ParamKey::BftDr,
        ParamKey::BftDr2,
        ParamKey::BftRweight,
        ParamKey::BftQmax,
        ParamKey::BftKstep,
        ParamKey::BftNfft,
    ];

    #[test]
    fn processing_fields_have_sentence_case_labels_and_complete_tooltips() {
        for &key in INSPECTOR_KEYS {
            let ParamText { label, tip } = param_text(key);
            let first = label.chars().next().unwrap();
            assert!(
                !first.is_lowercase() || label.starts_with('q') || label.starts_with('k'),
                "{key:?}: label {label:?} should start in upper case"
            );
            let tip = tip.unwrap_or_else(|| panic!("{key:?} has no tooltip"));
            assert!(tip.contains("Auto"), "{key:?}: tooltip must state Auto");
            assert!(
                tip.contains("Larch") || tip.contains("rexafs-specific"),
                "{key:?}: tooltip must name the Larch equivalent or say it has none"
            );
        }
    }

    #[test]
    fn units_stay_in_the_unit_column() {
        // NumericField moves a trailing "(unit)" of up to six characters
        // into its unit column; longer text would stay in the label.
        for &key in INSPECTOR_KEYS {
            let label = param_text(key).label;
            if let Some(open) = label.rfind(" (") {
                let unit = &label[open + 2..label.len() - 1];
                assert!(unit.chars().count() <= 6, "{key:?}: unit {unit:?} too long");
            }
            assert!(
                label.chars().count() <= 22,
                "{key:?}: label {label:?} too long"
            );
        }
    }
}
