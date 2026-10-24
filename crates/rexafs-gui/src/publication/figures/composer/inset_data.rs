//! Resolve inset display arrays before rendering, without changing spectra.
use super::inset::PreparedInset;
use super::offsets::format_offset;
use super::{
    CompositePanel, CompositeRecipe, CompositeSample, DisplayOffset, InsetKind, PreparedPanel,
};
use crate::publication::figures::{FigureData, FigureOptions, FigureSeries};
use std::borrow::Borrow;

/// Prepare independent inset limits and display curves. The first selected
/// sample remains the energy and difference reference even when offsets are
/// reversed; missing reference data is an error rather than a new reference.
pub(super) fn prepare(
    panel: &CompositePanel,
    recipe: &CompositeRecipe,
    prepared: &PreparedPanel,
    current: &[impl Borrow<FigureData>],
    samples: &[CompositeSample],
) -> Result<Option<PreparedInset>, String> {
    let Some(inset) = &panel.inset else {
        return Ok(None);
    };
    let error = |message: &str| format!("Panel {} inset: {message}", panel.key);
    if !panel.supports_inset() {
        return Err(error("only energy-domain μ(E) panels support insets."));
    }
    let (low, high) = inset.energy_range;
    if !low.is_finite() || !high.is_finite() || low >= high {
        return Err(error(
            "enter finite energy limits with minimum < maximum, relative to E₀ in eV.",
        ));
    }
    let ids = panel.samples.as_deref().unwrap_or(&recipe.samples);
    if inset.kind == InsetKind::Difference && ids.len() < 2 {
        return Err(error(
            "Difference needs at least two samples with normalized μ(E) data.",
        ));
    }
    let mut sources = Vec::new();
    if ids.is_empty() {
        let figures: Vec<_> = current.iter().map(Borrow::borrow).collect();
        sources.push((0, None, figures));
    } else {
        for (position, id) in ids.iter().enumerate() {
            if let Some(sample) = samples.iter().find(|sample| sample.id == *id) {
                sources.push((
                    position,
                    Some(sample),
                    sample.figures.iter().map(AsRef::as_ref).collect(),
                ));
            } else if position == 0 {
                return Err(error("the first selected sample is unavailable."));
            }
        }
    }
    let mut curves = Vec::new();
    let mut normalized = Vec::new();
    let mut positions = Vec::new();
    let mut e0 = None;
    for (position, sample, figures) in sources {
        let source = figures
            .iter()
            .find(|data| data.key == panel.key)
            .filter(|data| {
                data.series(&prepared.options)
                    .first()
                    .is_some_and(|curve| !curve.x.is_empty())
            });
        let Some(source) = source else {
            if position == 0 {
                return Err(error(
                    "the first selected sample has no data for this panel.",
                ));
            }
            continue;
        };
        if position == 0 {
            e0 = source.e0.filter(|value| value.is_finite());
        }
        let decorate = |mut curve: FigureSeries| {
            if ids.len() > 1
                && let Some(sample) = sample
            {
                curve.key = format!(
                    "{}/{}",
                    sample.id.as_str(),
                    source.series(&prepared.options)[0].key
                );
                curve.label = sample.label.clone();
                curve.optional = false;
            }
            curve
        };
        let main = source.series(&prepared.options);
        if inset.kind == InsetKind::Difference {
            let norm = source
                .normalized_series
                .as_ref()
                .or_else(|| {
                    figures
                        .iter()
                        .find(|data| matches!(data.key, "xanes" | "flattened-mu"))
                        .and_then(|data| data.normalized_series.as_ref())
                })
                .and_then(|series| series.first())
                .filter(|curve| !curve.x.is_empty())
                .ok_or_else(|| {
                    error("every compared sample needs processed normalized μ(E) data.")
                })?;
            let mut curve = decorate(norm.clone());
            // Difference curves identify the same sample as the panel's main
            // curve, including its colour key and line style on raw panels.
            curve.dashed = main[0].dashed;
            normalized.push(curve);
        }
        let selected = if ids.len() > 1 { &main[..1] } else { main };
        for curve in selected {
            curves.push(decorate(curve.clone()));
            positions.push(position);
        }
    }
    if inset.kind == InsetKind::Difference && normalized.len() < 2 {
        return Err(error(
            "Difference needs at least two samples with normalized μ(E) data.",
        ));
    }
    let e0 = e0.ok_or_else(|| error("the first sample needs a finite processed E₀."))?;
    let range = (e0 + low, e0 + high);
    if !range.0.is_finite() || !range.1.is_finite() || range.0 >= range.1 {
        return Err(error(
            "the energy range is too large or too narrow at this E₀.",
        ));
    }
    let main_range = prepared
        .data
        .xlim(&prepared.options)
        .or_else(|| extent(curves.iter().flat_map(|curve| curve.x.iter().copied())));
    let main_span = main_range.map_or(0., |range| single_span(&curves, &prepared.options, range));
    if inset.kind == InsetKind::Difference {
        let reference = normalized[0].clone();
        for curve in &mut normalized {
            validate_grid(curve).map_err(error)?;
        }
        // Evaluate the reference on each sample's grid without extrapolation.
        // The reference itself is retained as a zero curve in panel order.
        for curve in &mut normalized {
            let points: Vec<_> = curve
                .x
                .iter()
                .zip(&curve.y)
                .filter_map(|(&x, &y)| interpolate(&reference, x).map(|baseline| (x, y - baseline)))
                .collect();
            curve.x = points.iter().map(|point| point.0).collect();
            curve.y = points.iter().map(|point| point.1).collect();
            if curve_span(curve, range).is_none() {
                return Err(error(
                    "the samples need overlapping energy data in the inset range.",
                ));
            }
        }
        curves = normalized;
    }
    let mut options = FigureOptions {
        xmin: Some(range.0),
        xmax: Some(range.1),
        ymin: None,
        ymax: None,
        normalized: false,
        legend: false,
        guides: false,
        title: None,
        xlabel: Some(String::new()),
        ylabel: Some(String::new()),
        ..prepared.options.clone()
    };
    let inset_span = single_span(&curves, &options, range);
    let scale = if inset.follow_offset && main_span > 0. && inset_span > 0. {
        inset_span / main_span
    } else {
        0.
    };
    let mut offsets = Vec::new();
    for (curve, position) in curves.iter_mut().zip(positions) {
        let offset_index = if ids.len() <= 1 {
            0
        } else if panel.reverse {
            ids.len() - 1 - position
        } else {
            position
        };
        let offset = panel.offset_step * offset_index as f64 * scale;
        if !offset.is_finite() {
            return Err(error("the scaled display offset is too large."));
        }
        offsets.push((offset != 0.).then(|| DisplayOffset {
            value: offset,
            unshifted_y: curve.y.clone(),
        }));
        for y in &mut curve.y {
            let shifted = *y + offset;
            if y.is_finite() && !shifted.is_finite() {
                return Err(error("the scaled display offset is too large."));
            }
            *y = shifted;
        }
    }
    let quantity = if inset.kind == InsetKind::Difference {
        "Δμ"
    } else {
        "μ(E)"
    };
    let mut data = prepared.data.clone();
    data.series = curves;
    data.normalized_series = None;
    data.residual = None;
    data.guides.clear();
    data.default_xlim = Some(range);
    data.ylabel = quantity.into();
    data.math_ylabel = quantity.into();
    let (min, max) = data
        .visible_ylim(&options)
        .ok_or_else(|| error("no visible curves have data in the requested energy range."))?;
    options.ymin = Some(min);
    options.ymax = Some(max);
    let description = match inset.kind {
        InsetKind::PreEdge => "pre-edge region",
        InsetKind::Edge => "edge and white line",
        InsetKind::Difference => "normalized μ(E) difference from the first sample (Δμ)",
    };
    let mut caption = format!(
        "Inset: {description}, {}–{} eV.",
        format_offset(range.0, 0.1),
        format_offset(range.1, 0.1)
    );
    if ids.len() > 1 && panel.offset_step != 0. && scale > 0. {
        caption.push_str(" Inset curves are offset proportionally to the main panel.");
    }
    Ok(Some(PreparedInset {
        data,
        options,
        placement: inset.placement,
        quantity,
        caption,
        offsets,
    }))
}

/// Largest individual unshifted curve span within the visible energy window.
/// Excludes inter-sample baseline differences and automatic axis padding.
fn single_span(curves: &[FigureSeries], options: &FigureOptions, range: (f64, f64)) -> f64 {
    curves
        .iter()
        .filter(|curve| curve.visible(options))
        .filter_map(|curve| curve_span(curve, range))
        .map(|(min, max)| max - min)
        .fold(0., f64::max)
}

fn extent(values: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    values
        .filter(|value| value.is_finite())
        .fold(None, |range, value| {
            Some(range.map_or((value, value), |(min, max)| {
                (min.min(value), max.max(value))
            }))
        })
}

/// Include linearly interpolated boundaries so sparse, overlapping grids have
/// the same scaling as the visible line segments, without extrapolation.
fn curve_span(curve: &FigureSeries, range: (f64, f64)) -> Option<(f64, f64)> {
    let points = curve
        .x
        .iter()
        .zip(&curve.y)
        .filter(|(x, _)| (range.0..=range.1).contains(x))
        .map(|(_, y)| *y);
    extent(
        points.chain(
            [range.0, range.1]
                .into_iter()
                .filter_map(|x| interpolate(curve, x)),
        ),
    )
}

fn validate_grid(curve: &FigureSeries) -> Result<(), &'static str> {
    if curve.x.len() != curve.y.len()
        || curve.x.len() < 2
        || curve
            .x
            .iter()
            .chain(&curve.y)
            .any(|value| !value.is_finite())
        || curve.x.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err("difference curves need finite values on strictly increasing energy grids.");
    }
    Ok(())
}

/// Linear interpolation on a processed, increasing energy grid; points beyond
/// measured support are omitted rather than extrapolated.
fn interpolate(curve: &FigureSeries, x: f64) -> Option<f64> {
    let index = curve.x.partition_point(|value| *value < x);
    if curve.x.get(index) == Some(&x) {
        return curve
            .y
            .get(index)
            .copied()
            .filter(|value| value.is_finite());
    }
    if index == 0 || index >= curve.x.len() || curve.y.len() != curve.x.len() {
        return None;
    }
    let t = (x - curve.x[index - 1]) / (curve.x[index] - curve.x[index - 1]);
    let y = curve.y[index - 1] + t * (curve.y[index] - curve.y[index - 1]);
    y.is_finite().then_some(y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group_identity::GroupId;
    use crate::publication::figures::composer::{PanelInset, prepare_panels};
    use crate::publication::figures::{FigureSettings, spectrum_figures};
    use rexafs::prelude::{AnalysisSpace, XASSpectrum};
    use std::sync::Arc;

    fn prepared_norm_sample(
        id: u64,
        energy: &[f64],
        values: &[f64],
        e0: f64,
    ) -> (Arc<XASSpectrum>, CompositeSample) {
        let spectrum =
            Arc::new(XASSpectrum::from_prepared(energy, values, AnalysisSpace::Norm, e0).unwrap());
        assert_eq!(spectrum.norm().unwrap().as_slice(), values);
        assert!(spectrum.flat().is_none());
        let label = format!("Sample {id}");
        let figures = spectrum_figures(spectrum.clone(), &label);
        assert!(
            figures
                .iter()
                .all(|data| !matches!(data.key, "xanes" | "flattened-mu"))
        );
        let sample = CompositeSample {
            id: GroupId::legacy_result(id),
            label,
            color: "#0072b2".into(),
            figures: figures.into_iter().map(Arc::new).collect(),
        };
        (spectrum, sample)
    }

    fn difference_recipe(samples: &[CompositeSample]) -> CompositeRecipe {
        CompositeRecipe {
            samples: samples.iter().map(|sample| sample.id.clone()).collect(),
            panels: vec![CompositePanel {
                inset: Some(PanelInset {
                    follow_offset: false,
                    ..PanelInset::new(InsetKind::Difference)
                }),
                .."mu-energy".into()
            }],
            ..Default::default()
        }
    }

    fn raw_data_mut(sample: &mut CompositeSample) -> &mut FigureData {
        Arc::make_mut(
            sample
                .figures
                .iter_mut()
                .find(|data| data.key == "mu-energy")
                .unwrap(),
        )
    }

    #[test]
    fn inset_captions_round_energy_to_tenths_and_describe_only_active_offsets() {
        let current: Vec<FigureData> = Vec::new();
        for (e0, kind, description, energy_range) in [
            (
                8981.214,
                InsetKind::PreEdge,
                "pre-edge region",
                "8975.2–8992.2",
            ),
            (
                8981.214,
                InsetKind::Edge,
                "edge and white line",
                "8979.2–9004.2",
            ),
            (
                8981.214,
                InsetKind::Difference,
                "normalized μ(E) difference from the first sample (Δμ)",
                "8975.2–9012.2",
            ),
            (8981., InsetKind::PreEdge, "pre-edge region", "8975–8992"),
            (8981.96, InsetKind::PreEdge, "pre-edge region", "8976–8993"),
        ] {
            let energy = [8970., 8980., 8990., 9000., 9020.];
            let (_, a) = prepared_norm_sample(1, &energy, &[0., 0.2, 0.5, 1., 1.2], e0);
            let (_, b) = prepared_norm_sample(2, &energy, &[0., 0.4, 1., 2., 2.4], e0);
            let samples = [a, b];
            let mut recipe = difference_recipe(&samples);
            for (follow_offset, offset_step, suffix) in [
                (
                    true,
                    0.3,
                    " Inset curves are offset proportionally to the main panel.",
                ),
                (false, 0.3, ""),
                (true, 0., ""),
            ] {
                recipe.panels[0].offset_step = offset_step;
                recipe.panels[0].inset = Some(PanelInset {
                    follow_offset,
                    ..PanelInset::new(kind)
                });
                let prepared =
                    prepare_panels(&recipe, &current, &samples, &FigureSettings::default())
                        .unwrap();
                let inset = prepared.panels[0].inset.as_ref().unwrap();
                assert_eq!(
                    inset.caption,
                    format!("Inset: {description}, {energy_range} eV.{suffix}")
                );
                // Caption rounding leaves the actual energy limits intact.
                let (low, high) = kind.default_range();
                assert_eq!(inset.data.xlim(&inset.options), Some((e0 + low, e0 + high)));
            }
        }
    }

    #[test]
    fn prepared_norm_inputs_support_raw_panel_differences_without_flattened_data() {
        let (reference, a) = prepared_norm_sample(
            1,
            &[990., 995., 1000., 1005., 1010.],
            &[0., 0.5, 1., 1.5, 2.],
            1000.,
        );
        let (other, b) = prepared_norm_sample(
            2,
            &[992.5, 997.5, 1002.5, 1007.5, 1012.5],
            &[0.75, 1.75, 2.75, 3.75, 4.75],
            1001.,
        );
        let spectra = [reference, other];
        let before: Vec<_> = spectra
            .iter()
            .map(|spectrum| {
                (
                    spectrum.energy.clone(),
                    spectrum.mu.clone(),
                    spectrum.norm(),
                )
            })
            .collect();
        let samples = [a, b];
        let options = FigureOptions {
            normalized: true,
            ..Default::default()
        };
        for sample in &samples {
            let raw = sample
                .figures
                .iter()
                .find(|data| data.key == "mu-energy")
                .unwrap();
            assert!(raw.normalized_series.is_some());
            // The arrays happen to agree for prepared input; pointer identity
            // checks that the raw panel still selects its original curve list.
            assert!(std::ptr::eq(raw.series(&options), raw.series.as_slice()));
            assert_eq!(raw.ylabel(&options), "μ(E) (arb. units)");
            assert_eq!(raw.math_ylabel(&options), raw.math_ylabel);
        }
        let mut settings = FigureSettings::default();
        settings.figures.insert("mu-energy".into(), options);
        let mut recipe = difference_recipe(&samples);
        recipe.panels[0].reverse = true;
        recipe.panels[0].offset_step = 10.;
        let current: Vec<FigureData> = Vec::new();
        let prepared = prepare_panels(&recipe, &current, &samples, &settings).unwrap();
        let panel = &prepared.panels[0];
        assert_eq!(panel.data.series[0].y, [10., 10.5, 11., 11.5, 12.]);
        let inset = panel.inset.as_ref().unwrap();
        assert_eq!(inset.data.xlim(&inset.options), Some((994., 1031.)));
        assert_eq!(inset.data.series[0].y, [0., 0., 0., 0., 0.]);
        assert_eq!(inset.data.series[1].x, [992.5, 997.5, 1002.5, 1007.5]);
        assert_eq!(inset.data.series[1].y, [0.5, 1., 1.5, 2.]);
        for (spectrum, expected) in spectra.iter().zip(before) {
            assert_eq!(
                (
                    spectrum.energy.clone(),
                    spectrum.mu.clone(),
                    spectrum.norm(),
                ),
                expected
            );
            assert!(spectrum.flat().is_none());
        }
    }

    #[test]
    fn inset_offset_ratio_uses_individual_spans_at_interpolated_crop_boundaries() {
        let (_, a) = prepared_norm_sample(1, &[0., 10.], &[0., 20.], 5.);
        let (_, b) = prepared_norm_sample(2, &[0., 10.], &[1000., 1040.], 5.);
        let samples = [a, b];
        let mut recipe = difference_recipe(&samples);
        recipe.panels[0].offset_step = 3.;
        recipe.panels[0].inset = Some(PanelInset {
            energy_range: (-1., 1.),
            ..PanelInset::new(InsetKind::PreEdge)
        });
        let mut settings = FigureSettings::default();
        settings.figures.insert(
            "mu-energy".into(),
            FigureOptions {
                xmin: Some(2.),
                xmax: Some(8.),
                ..Default::default()
            },
        );
        let current: Vec<FigureData> = Vec::new();
        let prepared = prepare_panels(&recipe, &current, &samples, &settings).unwrap();
        let panel = &prepared.panels[0];
        assert_eq!(panel.data.series[1].y, [1003., 1043.]);
        let inset = panel.inset.as_ref().unwrap();
        // No measured point lies inside either cropped range. Interpolated
        // endpoint spans are 24 in the main panel and 8 in the inset, so the
        // display step is 3 × 8 / 24 = 1 despite the 1000-unit baseline gap.
        assert_eq!(inset.data.xlim(&inset.options), Some((4., 6.)));
        assert_eq!(inset.data.series[0].y, [0., 20.]);
        assert_eq!(inset.data.series[1].y, [1001., 1041.]);
        assert_eq!(
            samples[1]
                .figures
                .iter()
                .find(|data| data.key == "mu-energy")
                .unwrap()
                .series[0]
                .y,
            [1000., 1040.]
        );
    }

    #[test]
    fn difference_rejects_invalid_normalized_grids_in_reference_or_compared_sample() {
        let (_, a) = prepared_norm_sample(1, &[0., 5., 10.], &[0., 1., 2.], 5.);
        let (_, b) = prepared_norm_sample(2, &[0., 5., 10.], &[0., 1.5, 3.], 5.);
        let current: Vec<FigureData> = Vec::new();
        let invalid = [
            (vec![0., 5., 10.], vec![0., 1.]),
            (vec![5.], vec![1.]),
            (vec![0., f64::NAN, 10.], vec![0., 1., 2.]),
            (vec![0., 5., 10.], vec![0., f64::INFINITY, 2.]),
            (vec![0., 5., 5.], vec![0., 1., 2.]),
            (vec![0., 10., 5.], vec![0., 1., 2.]),
        ];
        for position in 0..2 {
            for (x, y) in &invalid {
                let mut samples = [a.clone(), b.clone()];
                let curve = &mut raw_data_mut(&mut samples[position])
                    .normalized_series
                    .as_mut()
                    .unwrap()[0];
                curve.x = x.clone();
                curve.y = y.clone();
                let error = prepare_panels(
                    &difference_recipe(&samples),
                    &current,
                    &samples,
                    &FigureSettings::default(),
                )
                .err()
                .unwrap();
                assert!(
                    error.contains(
                        "difference curves need finite values on strictly increasing energy grids"
                    ),
                    "position {position}, x={x:?}, y={y:?}: {error}"
                );
            }
        }
    }
}
