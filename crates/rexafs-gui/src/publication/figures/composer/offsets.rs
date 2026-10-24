//! Decimal display of the vertical displacements recorded in figure captions.

/// Format a display displacement at the precision of its configured step.
///
/// The step's shortest decimal representation determines the last displayed
/// decimal place, with at most 15 significant digits (the decimal precision of
/// `f64`). This removes multiplication noise without rounding small steps to
/// zero. Trailing fractional zeros and negative zero are omitted. Values below
/// `1e-6` or at least `1e12` use scientific notation to bound caption length.
/// Positive values have no leading `+`; negative values retain their sign.
pub(super) fn format_offset(value: f64, step: f64) -> String {
    if value == 0. {
        return "0".into();
    }
    if !value.is_finite() || !step.is_finite() || step == 0. {
        return value.to_string();
    }

    let (step_exponent, step_fraction_digits) = decimal_parts(step);
    let (value_exponent, _) = decimal_parts(value);
    let max_fraction_digits = f64::DIGITS as i32 - 1;
    let last_digit_exponent = (step_exponent
        - step_fraction_digits.min(max_fraction_digits as usize) as i32)
        .max(value_exponent - max_fraction_digits);

    if (-6..12).contains(&value_exponent) {
        let precision = (-last_digit_exponent).max(0) as usize;
        trim_fraction(format!("{value:.precision$}"))
    } else {
        let precision = (value_exponent - last_digit_exponent).max(0) as usize;
        let formatted = format!("{value:.precision$e}");
        let (mantissa, exponent) = formatted.split_once('e').unwrap();
        format!("{}e{exponent}", trim_fraction(mantissa.to_owned()))
    }
}

/// Return the exponent and number of fractional mantissa digits in the
/// shortest scientific decimal representation of a finite, nonzero value.
fn decimal_parts(value: f64) -> (i32, usize) {
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap();
    let fraction_digits = mantissa.split_once('.').map_or(0, |(_, tail)| tail.len());
    (exponent.parse().unwrap(), fraction_digits)
}

fn trim_fraction(mut value: String) -> String {
    if value.contains('.') {
        value.truncate(value.trim_end_matches('0').len());
        value.truncate(value.trim_end_matches('.').len());
    }
    if value == "-0" { "0".into() } else { value }
}

#[cfg(test)]
mod tests {
    use super::super::{
        CompositePanel, CompositeRecipe, CompositeSample, FigureData, FigureOptions,
        FigureSettings, GroupId, prepare_panels,
    };
    use super::format_offset;
    use crate::publication::figures::FigureSeries;
    use std::sync::Arc;

    #[test]
    fn offset_display_uses_step_precision_without_multiplication_noise() {
        for (step, count, expected) in [
            (0.3, 3., "0.9"),
            (1.5, 3., "4.5"),
            (0.25, 3., "0.75"),
            (0.25, 4., "1"),
            (0.3, 10., "3"),
            (0.1 + 0.2, 3., "0.9"),
            (-0.3, 3., "-0.9"),
            (0.3, -3., "-0.9"),
            (0.3, 0., "0"),
            (-0.3, 0., "0"),
        ] {
            assert_eq!(format_offset(step * count, step), expected);
        }
        assert_eq!(format_offset(0., 0.), "0");
        assert_eq!(format_offset(-0., 0.), "0");
    }

    #[test]
    fn extreme_steps_retain_their_scale_with_bounded_precision() {
        for (step, expected) in [
            (0.000_003, "0.000009"),
            (0.000_000_3, "9e-7"),
            (1.25e-100, "3.75e-100"),
            (-1.25e-100, "-3.75e-100"),
            (1.5e100, "4.5e100"),
            (3e300, "9e300"),
        ] {
            assert_eq!(format_offset(step * 3., step), expected);
        }
        assert_eq!(
            format_offset(f64::from_bits(1), f64::from_bits(1)),
            "5e-324"
        );
        assert_eq!(format_offset(f64::MAX, f64::MAX), "1.79769313486232e308");
        assert_eq!(format_offset(1e21, 1e20), "1e21");
    }

    #[test]
    fn prepared_captions_keep_saved_sample_order_and_clean_signed_offsets() {
        let samples: Vec<_> = ["A", "B", "C", "D"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| CompositeSample {
                id: GroupId::legacy_result(index as u64 + 1),
                label: label.into(),
                color: "#0072b2".into(),
                figures: vec![Arc::new(FigureData {
                    key: "xanes",
                    label: "Energy".into(),
                    xlabel: "Energy (eV)".into(),
                    ylabel: "Normalized absorption (dimensionless)".into(),
                    math_ylabel: "Normalized absorption".into(),
                    series: vec![FigureSeries {
                        key: "series-0".into(),
                        label: label.into(),
                        x: vec![0., 1.],
                        y: vec![0., 1.],
                        dashed: false,
                        optional: false,
                    }],
                    normalized_series: None,
                    guides: vec![],
                    default_xlim: Some((0., 1.)),
                    residual: None,
                })],
            })
            .collect();
        let source = samples[3].figures[0]
            .csv(&FigureOptions::default())
            .unwrap();
        let current: Vec<FigureData> = vec![];
        for (step, reverse, expected) in [
            (0.3, false, "A: +0, B: +0.3, C: +0.6, D: +0.9"),
            (0.3, true, "A: +0.9, B: +0.6, C: +0.3, D: +0"),
            (-0.3, false, "A: +0, B: -0.3, C: -0.6, D: -0.9"),
            (-0.3, true, "A: -0.9, B: -0.6, C: -0.3, D: +0"),
        ] {
            let recipe = CompositeRecipe {
                samples: samples.iter().map(|sample| sample.id.clone()).collect(),
                panels: vec![CompositePanel {
                    offset_step: step,
                    reverse,
                    .."xanes".into()
                }],
                ..Default::default()
            };
            let prepared =
                prepare_panels(&recipe, &current, &samples, &FigureSettings::default()).unwrap();
            assert_eq!(
                prepared.panels[0].offset_caption.as_deref(),
                Some(format!(
                    "Spectra offset vertically by {step} in vertical-axis units for clarity. Display offsets in sample order: {expected}."
                ).as_str())
            );
        }
        assert_eq!(
            samples[3].figures[0]
                .csv(&FigureOptions::default())
                .unwrap(),
            source
        );
    }
}
