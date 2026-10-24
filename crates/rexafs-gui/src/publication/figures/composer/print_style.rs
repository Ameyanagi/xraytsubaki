//! Regression checks for the composed-figure print defaults and saved labels.
use super::tests::sample;
use super::*;
use crate::publication::figures::{render_figure, spectrum_figures};
use std::path::Path;

#[test]
fn figure_labels_and_print_settings_round_trip_and_legacy_recipes_load() {
    let recipe = CompositeRecipe {
        figure_labels: BTreeMap::from([
            (GroupId::legacy_result(1), "Frame 1".into()),
            (GroupId::legacy_result(2), String::new()),
            (GroupId::legacy_result(99), "Retained missing sample".into()),
        ]),
        sample_colours: SampleColours::OkabeIto,
        grid: true,
        legend_every_panel: true,
        ..Default::default()
    };
    let json = serde_json::to_value(&recipe).unwrap();
    assert_eq!(json["figure_labels"]["result:1"], "Frame 1");
    assert_eq!(
        serde_json::from_value::<CompositeRecipe>(json).unwrap(),
        recipe
    );
    for legacy in [r#"{}"#, r#"{"panels":["xanes"]}"#] {
        let recipe: CompositeRecipe = serde_json::from_str(legacy).unwrap();
        assert!(recipe.figure_labels.is_empty());
        assert_eq!(recipe.sample_colours, SampleColours::Ordered);
        assert!(!recipe.grid);
        assert!(!recipe.legend_every_panel);
    }
}

#[test]
fn caption_uses_figure_labels_and_identifies_full_names_once() {
    let samples = [
        sample(1, "Synthetic 01 · CuO 100.0%", "#abcdef"),
        sample(2, "Workspace B", "#123456"),
    ];
    let recipe = CompositeRecipe {
        samples: samples.iter().map(|s| s.id.clone()).collect(),
        panels: vec![
            CompositePanel {
                offset_step: 0.3,
                .."xanes".into()
            };
            2
        ],
        figure_labels: BTreeMap::from([
            (samples[0].id.clone(), "Frame 1".into()),
            (samples[1].id.clone(), "".into()),
        ]),
        dpi: 72.,
        ..Default::default()
    };
    let current: Vec<FigureData> = vec![];
    let composed = compose_samples(
        &recipe,
        &current,
        &samples,
        &FigureSettings::default(),
        None,
    )
    .unwrap();
    assert!(
        composed
            .caption
            .contains("Frame 1 (solid), Workspace B (solid)")
    );
    assert!(composed.caption.contains("Frame 1: +0, Workspace B: +0.3"));
    assert!(
        composed
            .caption
            .ends_with("Sample labels: Frame 1 (Synthetic 01 · CuO 100.0%).")
    );
    assert_eq!(composed.caption.matches(&samples[0].label).count(), 1);
    assert!(!composed.caption.contains("Workspace B (Workspace B)"));
    assert!(
        composed
            .svg
            .contains(&super::super::super::report::html(&composed.caption))
    );
}

#[test]
fn custom_label_applies_to_current_and_explicit_single_sample_without_recolouring() {
    let mut current = sample(1, "Workspace current", "#abcdef");
    current.is_current = true;
    let mut recipe = CompositeRecipe {
        panels: vec!["xanes".into()],
        figure_labels: BTreeMap::from([(current.id.clone(), "Frame 1".into())]),
        ..Default::default()
    };
    for ids in [vec![], vec![current.id.clone()]] {
        recipe.samples = ids;
        let prepared = prepare_panels(
            &recipe,
            &current.figures,
            std::slice::from_ref(&current),
            &FigureSettings::default(),
        )
        .unwrap();
        let panel = &prepared.panels[0];
        assert_eq!(panel.data.series[0].label, "Frame 1");
        assert_eq!(
            panel.data.normalized_series.as_ref().unwrap()[0].label,
            "Frame 1"
        );
        assert_eq!(panel.data.series[1].label, "Helper");
        assert!(panel.options.series_colors.is_empty());
        assert_eq!(
            prepared.sample_names,
            [("Frame 1".into(), "Workspace current".into())]
        );
    }
}

#[test]
fn palette_modes_follow_saved_positions_and_explicit_colours_win() {
    let samples = [sample(1, "A", "#abcdef"), sample(2, "B", "#123456")];
    let current: Vec<FigureData> = vec![];
    let mut recipe = CompositeRecipe {
        samples: samples.iter().map(|s| s.id.clone()).collect(),
        panels: vec!["xanes".into()],
        ..Default::default()
    };
    let settings = FigureSettings::default();
    for (mode, expected) in [
        (SampleColours::Ordered, ["#440154", "#95d63f"]),
        (SampleColours::OkabeIto, ["#000000", "#e69f00"]),
        (SampleColours::Workspace, ["#abcdef", "#123456"]),
    ] {
        recipe.sample_colours = mode;
        let prepared = prepare_panels(&recipe, &current, &samples, &settings).unwrap();
        let colors = &prepared.panels[0].options.series_colors;
        assert_eq!(colors["result:1/series-0"], expected[0], "{mode:?}");
        assert_eq!(colors["result:2/series-0"], expected[1], "{mode:?}");
        let mut override_settings = settings.clone();
        override_settings.figures.insert(
            "xanes".into(),
            FigureOptions {
                series_colors: BTreeMap::from([
                    ("series-0".into(), "#111111".into()),
                    ("result:1/series-0".into(), "#222222".into()),
                    ("result:2/series-0".into(), "#333333".into()),
                ]),
                ..Default::default()
            },
        );
        recipe
            .series_colors
            .insert("result:2/series-0".into(), "#444444".into());
        let prepared = prepare_panels(&recipe, &current, &samples, &override_settings).unwrap();
        assert_eq!(
            prepared.panels[0].options.series_colors["result:1/series-0"],
            "#222222"
        );
        assert_eq!(
            prepared.panels[0].options.series_colors["result:2/series-0"],
            "#444444"
        );
        recipe.series_colors.clear();
    }
    assert_eq!(SampleColours::OkabeIto.color(8, 9, ""), "#000000");
    // A missing earlier source does not move the later source into its palette slot.
    recipe.sample_colours = SampleColours::Ordered;
    let prepared = prepare_panels(&recipe, &current, &samples[1..], &settings).unwrap();
    assert_eq!(
        prepared.panels[0].options.series_colors["result:2/series-0"],
        "#95d63f"
    );
}

#[test]
fn composed_grid_overrides_figure_settings_and_legend_appears_once() {
    let samples = [sample(1, "A", "#abcdef"), sample(2, "B", "#123456")];
    let mut recipe = CompositeRecipe {
        samples: samples.iter().map(|s| s.id.clone()).collect(),
        panels: vec![
            CompositePanel {
                label_mode: CurveLabelMode::CurveEnd,
                .."xanes".into()
            },
            "xanes".into(),
            "xanes".into(),
        ],
        ..Default::default()
    };
    let current: Vec<FigureData> = vec![];
    let mut settings = FigureSettings::default();
    settings.figures.insert(
        "xanes".into(),
        FigureOptions {
            grid: Some(true),
            ..Default::default()
        },
    );
    let prepared = prepare_panels(&recipe, &current, &samples, &settings).unwrap();
    assert!(
        prepared
            .panels
            .iter()
            .all(|panel| panel.options.grid == Some(false))
    );
    assert_eq!(
        prepared
            .panels
            .iter()
            .map(|p| p.options.legend)
            .collect::<Vec<_>>(),
        [false, true, false]
    );
    recipe.legend_every_panel = true;
    recipe.grid = true;
    settings.figures.get_mut("xanes").unwrap().grid = Some(false);
    let prepared = prepare_panels(&recipe, &current, &samples, &settings).unwrap();
    assert!(
        prepared
            .panels
            .iter()
            .all(|panel| panel.options.grid == Some(true))
    );
    assert_eq!(
        prepared
            .panels
            .iter()
            .map(|p| p.options.legend)
            .collect::<Vec<_>>(),
        [false, true, true]
    );
    settings.figures.get_mut("xanes").unwrap().legend = false;
    let prepared = prepare_panels(&recipe, &current, &samples, &settings).unwrap();
    assert!(prepared.panels.iter().all(|panel| !panel.options.legend));
}

#[test]
fn composing_does_not_change_single_figure_output_or_settings() {
    let samples = [sample(1, "A", "#abcdef"), sample(2, "B", "#123456")];
    let settings = FigureSettings::default();
    let before_settings = serde_json::to_value(&settings).unwrap();
    let data = &samples[0].figures[0];
    let options = settings.options(data.key);
    let before = render_figure(data, &options).unwrap();
    let recipe = CompositeRecipe {
        samples: samples.iter().map(|s| s.id.clone()).collect(),
        panels: vec!["xanes".into()],
        dpi: 72.,
        ..Default::default()
    };
    compose_samples(&recipe, &samples[0].figures, &samples, &settings, None).unwrap();
    let after = render_figure(data, &settings.options(data.key)).unwrap();
    assert_eq!(before.svg, after.svg);
    assert_eq!(before.png, after.png);
    assert_eq!(before_settings, serde_json::to_value(&settings).unwrap());
}

#[test]
fn measured_multi_sample_print_figure_renders_with_both_label_modes() {
    // Three measured copper spectra; no synthetic displacement is applied until
    // the separate curve-end render. Attribution stays with the fixture files.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rexafs/tests");
    let sources = [
        (
            "fixtures/xas/samples/nsls/x11a/xasdatalibrary/cu_metal_10K.xdi",
            "Cu 10 K",
        ),
        ("testfiles/xraylarch_d867/xafsdata/cu_150k.xmu", "Cu 150 K"),
        (
            "fixtures/xas/samples/aps/13-id-c/xasdatalibrary/cu_metal_rt.xdi",
            "Cu RT",
        ),
    ];
    let samples: Vec<_> = sources
        .iter()
        .enumerate()
        .map(|(i, (file, short))| {
            let spectrum = crate::params::process_file(
                &root.join(file),
                &crate::params::PipelineParams::default(),
            )
            .unwrap();
            let full = format!("Measured copper reference · {short}");
            CompositeSample {
                id: GroupId::legacy_result(i as u64 + 1),
                is_current: false,
                label: full.clone(),
                color: "#abcdef".into(),
                figures: spectrum_figures(Arc::new(spectrum), &full)
                    .into_iter()
                    .map(Arc::new)
                    .collect(),
            }
        })
        .collect();
    let mut recipe = CompositeRecipe {
        samples: samples.iter().map(|s| s.id.clone()).collect(),
        figure_labels: samples
            .iter()
            .zip(sources)
            .map(|(s, (_, short))| (s.id.clone(), short.into()))
            .collect(),
        panels: vec!["xanes".into(), "chi-r".into()],
        dpi: 200.,
        line_width: Some(1.),
        ..Default::default()
    };
    let mut settings = FigureSettings::default();
    settings.figures.insert(
        "xanes".into(),
        FigureOptions {
            xmin: Some(8960.),
            xmax: Some(9060.),
            ..Default::default()
        },
    );
    settings.figures.insert(
        "chi-r".into(),
        FigureOptions {
            xmin: Some(0.),
            xmax: Some(6.),
            ..Default::default()
        },
    );
    let current: Vec<FigureData> = vec![];
    for ends in [false, true] {
        if ends {
            for (panel, step) in recipe.panels.iter_mut().zip([0.3, 1.5]) {
                panel.label_mode = CurveLabelMode::CurveEnd;
                panel.offset_step = step;
            }
        }
        let rendered = compose_samples(&recipe, &current, &samples, &settings, None).unwrap();
        assert_eq!(rendered.image.width, 1400);
        for sample in &samples {
            assert_eq!(rendered.caption.matches(&sample.label).count(), 1);
        }
        if let Some(dir) = std::env::var_os("REXAFS_COMPOSER_SHOTS") {
            let dir = Path::new(&dir);
            std::fs::create_dir_all(dir).unwrap();
            let name = if ends {
                "composed-check-ends"
            } else {
                "composed-check"
            };
            std::fs::write(dir.join(format!("{name}.png")), rendered.png().unwrap()).unwrap();
            std::fs::write(dir.join(format!("{name}.svg")), &rendered.svg).unwrap();
        }
    }
}

#[test]
fn subset_and_reordered_panels_keep_sample_colours_and_missing_panels_do_not_repeat_legends() {
    let mut a = sample(1, "A", "#abcdef");
    a.figures.push(Arc::new(FigureData {
        key: "chi-r",
        ..a.figures[0].as_ref().clone()
    }));
    let samples = [a, sample(2, "B", "#123456"), sample(3, "C", "#234567")];
    let current: Vec<FigureData> = vec![];
    for mode in [SampleColours::Ordered, SampleColours::OkabeIto] {
        let recipe = CompositeRecipe {
            samples: samples.iter().map(|s| s.id.clone()).collect(),
            sample_colours: mode,
            panels: vec![
                "chi-r".into(),
                "xanes".into(),
                CompositePanel {
                    samples: Some(vec![samples[2].id.clone(), samples[1].id.clone()]),
                    .."xanes".into()
                },
                "chi-r".into(),
            ],
            ..Default::default()
        };
        let prepared =
            prepare_panels(&recipe, &current, &samples, &FigureSettings::default()).unwrap();
        assert_eq!(
            prepared
                .panels
                .iter()
                .map(|p| p.options.legend)
                .collect::<Vec<_>>(),
            [false, true, false, false]
        );
        for id in [2, 3] {
            let key = format!("result:{id}/series-0");
            assert_eq!(
                prepared.panels[1].options.series_colors[&key],
                prepared.panels[2].options.series_colors[&key]
            );
        }
    }
}

#[test]
fn captions_record_an_alias_only_when_its_curve_is_visible() {
    let samples = [
        sample(1, "Long full name", "#abcdef"),
        sample(2, "B", "#123456"),
    ];
    let recipe = CompositeRecipe {
        samples: samples.iter().map(|s| s.id.clone()).collect(),
        panels: vec![CompositePanel {
            offset_step: 0.3,
            .."xanes".into()
        }],
        figure_labels: BTreeMap::from([(samples[0].id.clone(), "A".into())]),
        ..Default::default()
    };
    let mut settings = FigureSettings::default();
    settings.figures.insert(
        "xanes".into(),
        FigureOptions {
            hidden: BTreeSet::from(["result:1/series-0".into()]),
            ..Default::default()
        },
    );
    let current: Vec<FigureData> = vec![];
    let prepared = prepare_panels(&recipe, &current, &samples, &settings).unwrap();
    assert!(prepared.sample_names.is_empty());
    assert!(
        prepared.panels[0]
            .offset_caption
            .as_ref()
            .unwrap()
            .ends_with("Display offsets in sample order: B: +0.3.")
    );
}
