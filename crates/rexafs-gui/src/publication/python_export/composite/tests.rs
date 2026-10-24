use super::*;
use crate::group_identity::GroupId;
use crate::publication::figures::composer::{
    CompositePanel, CompositeSample, InsetKind, PanelInset, SampleColours, compose_prepared,
    prepare_panels,
};
use crate::publication::figures::{FigureSettings, LegendPlacement};
use std::sync::Arc;

/// Synthetic values keep display/export checks independent of processing.
fn figure(key: &'static str, label: &str, y: Vec<f64>) -> FigureData {
    FigureData {
        key,
        label: label.into(),
        xlabel: "Energy (eV)".into(),
        ylabel: "μ(E) (dimensionless)".into(),
        math_ylabel: "_μ_(_E_) (dimensionless)".into(),
        series: vec![FigureSeries {
            key: "mu".into(),
            label: label.into(),
            x: vec![7100., 7101., 7102., 7103., 7104.],
            y,
            dashed: false,
            optional: false,
        }],
        normalized_series: None,
        e0: Some(7101.),
        guides: vec![7101.],
        default_xlim: Some((7100., 7104.)),
        residual: None,
    }
}

fn sample(id: u64, label: &str, color: &str, y: Vec<f64>) -> CompositeSample {
    let mut data = figure("xanes", label, y);
    data.normalized_series = Some(data.series.clone());
    CompositeSample {
        id: GroupId::legacy_result(id),
        is_current: false,
        label: label.into(),
        color: color.into(),
        figures: vec![Arc::new(data)],
    }
}

fn series_records(script: &str) -> Vec<serde_json::Value> {
    script
        .lines()
        .filter(|line| line.contains("\"file\": "))
        .map(|line| serde_json::from_str(line.trim().trim_end_matches(',')).unwrap())
        .collect()
}

/// Read the first setting in each main panel, before its inset or residual.
fn main_panel_flags(script: &str, key: &str) -> Vec<bool> {
    let prefix = format!("\"{key}\": ");
    let panels = script
        .split_once("PANELS = [")
        .unwrap()
        .1
        .split_once("\n]\n")
        .unwrap()
        .0;
    panels
        .split("\n    {\n")
        .skip(1)
        .map(|panel| {
            match panel
                .lines()
                .find_map(|line| line.trim().strip_prefix(&prefix))
                .unwrap()
                .trim_end_matches(',')
            {
                "True" => true,
                "False" => false,
                value => panic!("unexpected Python boolean {value:?}"),
            }
        })
        .collect()
}

/// Three available panels, a missing fit, two samples, stacking and an inset.
fn fixture() -> (CompositeRecipe, PreparedComposition) {
    let mut first = figure(
        "xanes",
        "A, foil $1\ncontinued",
        vec![1e-20, 1., 2., 1., 0.],
    );
    let mut second = figure("xanes", "B", vec![0.5, 1.5, 2.5, 1.5, 0.5]);
    first.normalized_series = Some(first.series.clone());
    second.normalized_series = Some(second.series.clone());
    let samples: Vec<_> = [first, second]
        .into_iter()
        .enumerate()
        .map(|(index, data)| CompositeSample {
            id: GroupId::legacy_result(index as u64 + 1),
            is_current: false,
            label: data.label.clone(),
            color: if index == 0 { "#0072b2" } else { "#d55e00" }.into(),
            figures: vec![Arc::new(data)],
        })
        .collect();
    let recipe = CompositeRecipe {
        panels: vec![
            CompositePanel {
                offset_step: 0.3,
                reverse: true,
                label_mode: CurveLabelMode::CurveEnd,
                inset: Some(PanelInset {
                    energy_range: (-1., 3.),
                    placement: InsetPlacement::UpperLeft,
                    ..PanelInset::new(InsetKind::Edge)
                }),
                ..CompositePanel::from("xanes")
            },
            "fit-q".into(),
            CompositePanel {
                samples: Some(Vec::new()),
                ..CompositePanel::from("chi-k")
            },
            "fit-residual-k".into(),
        ],
        samples: samples.iter().map(|sample| sample.id.clone()).collect(),
        columns: 2,
        width_mm: 177.8,
        panel_height_mm: Some(50.8),
        dpi: 300.,
        font_size: 9.,
        line_width: Some(1.25),
        series_colors: [("result:1/mu".into(), "#4f2b7c".into())].into(),
        sample_colours: SampleColours::Workspace,
        ..Default::default()
    };
    let chi = figure("chi-k", "χ(k)", vec![0., 1., -1., 1., 0.]);
    let mut fit = figure("fit-residual-k", "Fit", vec![0., 1., 2., 1., 0.]);
    let mut fitted = fit.series[0].clone();
    fitted.key = "fit".into();
    fitted.label = "Fit curve".into();
    fitted.dashed = true;
    fit.series.push(fitted);
    fit.ylabel = "k²χ(k) (Å⁻²)".into();
    fit.math_ylabel = "_k_#super[2]_χ_(_k_) (Å#super[−2])".into();
    let mut residual = fit.series[0].clone();
    residual.key = "residual".into();
    residual.y = vec![-0.1, 0., 0.1, 0., -0.1];
    fit.residual = Some(vec![residual]);
    let mut settings = FigureSettings::default();
    settings.figures = [(
        "chi-k".into(),
        FigureOptions {
            xlabel: Some("$k$ ($\"Å\"^(-1)$)".into()),
            ylabel: Some("$k^2 chi(k)$".into()),
            xmin: Some(7100.5),
            xmax: Some(7103.5),
            ymin: Some(-2.),
            ymax: Some(2.),
            guides: true,
            grid: Some(false),
            legend_position: LegendPlacement::LowerLeft,
            ..Default::default()
        },
    )]
    .into();
    let prepared = prepare_panels(&recipe, &[chi, fit], &samples, &settings).unwrap();
    (recipe, prepared)
}

#[test]
fn grid_script_preserves_style_offsets_insets_and_missing_notices() {
    let (recipe, prepared) = fixture();
    let files = composite_python_export("SI Figure \"S4\"\n$", &recipe, &prepared).unwrap();
    let script = &files[0].1;
    for expected in [
        "ROWS = 2",
        "COLS = 2",
        "WIDTH_MM = 177.8",
        "HEIGHT_MM = 101.6",
        "DPI = 300.0",
        "FONT_SIZE = 9.0",
        r#""font.family": ["Arial", "DejaVu Sans"],"#,
        "LINE_WIDTH = 1.25",
        "from matplotlib.ticker import AutoMinorLocator, NullLocator",
        "TICK_DIRECTION = \"in\"",
        "TICKS_TOP = True",
        "TICKS_RIGHT = True",
        "MINOR_SUBDIVISIONS = 5",
        "LEGEND_FONT_SIZE = FONT_SIZE - 1",
        "LEGEND_FRAME = False",
        "LEGEND_LOCATION = \"best\"",
        "LEGEND_HANDLE_LENGTH = 1.4",
        "LEGEND_LABEL_SPACING = 0.3",
        "LEGEND_HANDLE_TEXT_PAD = 0.5",
        "ax.tick_params(direction=TICK_DIRECTION, top=TICKS_TOP, right=TICKS_RIGHT, which=\"both\")",
        "ax.xaxis.set_minor_locator(AutoMinorLocator(MINOR_SUBDIVISIONS))",
        "ax.yaxis.set_minor_locator(AutoMinorLocator(MINOR_SUBDIVISIONS))",
        "ax.legend(loc=LEGEND_LOCATION, frameon=LEGEND_FRAME, fontsize=LEGEND_FONT_SIZE,",
        "handlelength=LEGEND_HANDLE_LENGTH, labelspacing=LEGEND_LABEL_SPACING,",
        "handletextpad=LEGEND_HANDLE_TEXT_PAD)",
        "plt.subplots(ROWS, COLS, figsize=(WIDTH_MM / 25.4, HEIGHT_MM / 25.4)",
        "ax.inset_axes(inset[\"bounds\"])",
        "\"bounds\": (0.03, 0.60, 0.43, 0.36)",
        "\"label\": \"(a)\"",
        "\"label\": \"(b)\"",
        "\"label\": \"(c)\"",
        "\"curve_end\": True",
        "y = y + series[\"offset\"]",
        "\"offset\": 0.3",
        "\"color\": \"#4f2b7c\"",
        "\"color\": \"#d55e00\"",
        "\"linestyle\": \"--\"",
        "\"xlim\": (7100.5, 7103.5)",
        "\"ylim\": (-2.0, 2.0)",
        "\"grid\": False",
        "\"guides\": [7101.0]",
        r#"r"$k^2\,\chi(k)$""#,
        "SKIPPED = [\"fit-q\"]",
        "print(f\"Skipped: {notice}\")",
        "RESIDUAL_HEIGHT_RATIOS = (3, 1)",
        "RESIDUAL_GAP = 0.08",
        "subgridspec(2, 1, height_ratios=RESIDUAL_HEIGHT_RATIOS, hspace=RESIDUAL_GAP)",
        "INSET_HEADROOM = 0.2",
        "INSET_TAG_BBOX = dict(facecolor=\"white\", edgecolor=\"none\", pad=1)",
        "INSET_TICK_BBOX = dict(facecolor=\"white\", edgecolor=\"none\", pad=0.5)",
        "inset_ax.set_ylim(low, high + INSET_HEADROOM * (high - low))",
        "bbox=INSET_TAG_BBOX)",
        "tick.set_bbox(INSET_TICK_BBOX)",
        "inset_ax.set_yticks([])",
        "inset_ax.yaxis.set_minor_locator(NullLocator())",
        "for extension in (\"pdf\", \"svg\", \"png\")",
        "dpi=DPI",
    ] {
        assert!(
            script.contains(expected),
            "missing {expected:?} in\n{script}"
        );
    }
    assert!(!script.contains("\"label\": \"(d)\""));
    assert!(script.contains("RECIPE_NAME = \"SI Figure \\\"S4\\\"\\n$\""));
    assert_eq!(
        files
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        [
            SCRIPT_NAME,
            "a-series-01.csv",
            "a-series-02.csv",
            "a-inset-series-01.csv",
            "a-inset-series-02.csv",
            "b-series-01.csv",
            "c-series-01.csv",
            "c-series-02.csv",
            "c-residual-series-01.csv",
        ]
    );
    // Retain an optional local smoke-test artifact without requiring Python
    // or its packages in the Rust test suite.
    if let Some(root) = std::env::var_os("REXAFS_COMPOSITE_PYTHON_EXPORT_DIR") {
        fs::create_dir_all(&root).unwrap();
        write_composite_python_export(
            "SI Figure S4",
            &recipe,
            &prepared,
            &Path::new(&root).join("journal-grid"),
        )
        .unwrap();
    }
}

#[test]
fn csv_values_precede_main_and_inset_stacking_exactly() {
    let (recipe, prepared) = fixture();
    let files = composite_python_export("Figure 1", &recipe, &prepared).unwrap();
    assert_eq!(prepared.panels[0].data.series[0].y[0], 0.3);
    assert_eq!(
        prepared.panels[0].offsets[0].as_ref().unwrap().unshifted_y[0],
        1e-20
    );
    for name in ["a-series-01.csv", "a-inset-series-01.csv"] {
        let csv = &files.iter().find(|(file, _)| file == name).unwrap().1;
        let mut reader = csv::Reader::from_reader(csv.as_bytes());
        assert_eq!(reader.headers().unwrap().len(), 2);
        assert_eq!(
            &reader.headers().unwrap()[0],
            "A, foil $1\ncontinued: Energy (eV)"
        );
        let values: Vec<(f64, f64)> = reader.deserialize().map(Result::unwrap).collect();
        assert_eq!(
            values,
            [
                (7100., 1e-20),
                (7101., 1.),
                (7102., 2.),
                (7103., 1.),
                (7104., 0.)
            ]
        );
    }
}

#[test]
fn complete_figure_labels_match_native_with_hidden_clipped_and_repeated_samples() {
    let samples: Vec<_> = (1..=3)
        .map(|id| {
            let label = format!("Synthetic {id:02} · Cu₂O cafe\u{301} 100.0% · CuO 0.0% · Cu 0.0%");
            CompositeSample {
                id: GroupId::legacy_result(id),
                is_current: false,
                label: label.clone(),
                color: "#0072b2".into(),
                figures: vec![Arc::new(figure(
                    "xanes",
                    &label,
                    vec![if id == 1 { 100. } else { id as f64 / 3. }; 5],
                ))],
            }
        })
        .collect();
    let mut recipe = CompositeRecipe {
        samples: samples.iter().map(|sample| sample.id.clone()).collect(),
        panels: vec![
            CompositePanel {
                label_mode: CurveLabelMode::CurveEnd,
                .."xanes".into()
            },
            "xanes".into(),
        ],
        dpi: 100.,
        panel_height_mm: Some(50.8),
        figure_labels: [
            (samples[0].id.clone(), "Frame 1".into()),
            (samples[1].id.clone(), "Frame 2\nCu₂O cafe\u{301}".into()),
            (samples[2].id.clone(), "Frame 3".into()),
        ]
        .into(),
        ..Default::default()
    };
    // Hide the last curve, clip the first one, and repeat the middle ID.
    // Only source indices 1 and 3 receive native direct labels.
    recipe.samples.push(samples[1].id.clone());
    let mut settings = FigureSettings::default();
    settings.figures.insert(
        "xanes".into(),
        FigureOptions {
            ymin: Some(0.),
            ymax: Some(2.),
            hidden: ["result:3/mu".into()].into(),
            ..Default::default()
        },
    );
    let empty: Vec<FigureData> = Vec::new();
    let prepared = prepare_panels(&recipe, &empty, &samples, &settings).unwrap();
    let files = composite_python_export("Figure labels", &recipe, &prepared).unwrap();
    let script = &files[0].1;
    let native = compose_prepared(&recipe, &prepared, None).unwrap();
    let records = series_records(script);
    assert_eq!(records.len(), 6);
    assert_eq!(native.svg.matches("class=\"curve-end-label\"").count(), 2);
    for label in native.svg.split("<text class=\"curve-end-label\"").skip(1) {
        let label = label.split("</text>").next().unwrap();
        for text in ["Frame 2", "Cu₂O cafe\u{301}"] {
            assert!(label.contains(&format!(">{text}</tspan>")));
        }
    }
    for (index, record) in records.iter().enumerate() {
        let expected = if index % 3 == 0 {
            "Frame 1"
        } else {
            "Frame 2\nCu₂O cafe\u{301}"
        };
        assert_eq!(record["label"], expected);
        assert!(record.get("full_label").is_none());
        let csv = &files
            .iter()
            .find(|(name, _)| name == record["file"].as_str().unwrap())
            .unwrap()
            .1;
        let mut reader = csv::Reader::from_reader(csv.as_bytes());
        assert_eq!(
            reader.headers().unwrap()[0],
            format!("{expected}: Energy (eV)")
        );
    }
    assert!(!script.contains("full_label"));
    assert!(!script.contains('…'));
    assert!(script.contains("label=series[\"label\"]"));

    // Complete workspace names are validated, rather than silently truncated.
    // Hidden and fully clipped names do not create native annotations.
    recipe.figure_labels.clear();
    for width_mm in [177.8, 66.04] {
        recipe.width_mm = width_mm;
        let prepared = prepare_panels(&recipe, &empty, &samples, &settings).unwrap();
        let native_error = compose_prepared(&recipe, &prepared, None).err().unwrap();
        assert!(
            native_error.contains("Shorten the figure labels")
                || native_error.contains("0.5 in of plot width"),
            "{native_error}"
        );
        assert_eq!(
            composite_python_export("Long names", &recipe, &prepared).unwrap_err(),
            native_error
        );
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("long-labels");
        assert_eq!(
            write_composite_python_export("Long names", &recipe, &prepared, &destination)
                .unwrap_err(),
            native_error
        );
        assert!(!destination.exists());
    }
}

#[test]
fn curve_end_labels_require_the_same_vertical_space_as_native_output() {
    let samples: Vec<_> = (1..=8)
        .map(|id| sample(id, &format!("S{id}"), "#0072b2", vec![id as f64 / 10.; 5]))
        .collect();
    let recipe = CompositeRecipe {
        panels: vec![CompositePanel {
            label_mode: CurveLabelMode::CurveEnd,
            .."xanes".into()
        }],
        samples: samples.iter().map(|sample| sample.id.clone()).collect(),
        panel_height_mm: Some(25.4),
        font_size: 12.,
        dpi: 100.,
        ..Default::default()
    };
    let empty: Vec<FigureData> = Vec::new();
    let prepared = prepare_panels(&recipe, &empty, &samples, &FigureSettings::default()).unwrap();
    let native_error = compose_prepared(&recipe, &prepared, None).err().unwrap();
    assert!(native_error.contains("taller"), "{native_error}");
    assert_eq!(
        composite_python_export("Too many labels", &recipe, &prepared).unwrap_err(),
        native_error
    );
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("crowded-labels");
    assert_eq!(
        write_composite_python_export("Too many labels", &recipe, &prepared, &destination)
            .unwrap_err(),
        native_error
    );
    assert!(!destination.exists());
}

#[test]
fn difference_inset_exports_prepared_quantity_and_independent_offsets() {
    let (mut recipe, _) = fixture();
    recipe.panels[0].inset = Some(PanelInset {
        kind: InsetKind::Difference,
        follow_offset: false,
        energy_range: (-1., 3.),
        placement: InsetPlacement::CenterRight,
    });
    let first = figure("xanes", "A", vec![0., 1., 2., 1., 0.]);
    let second = figure("xanes", "B", vec![0.5, 1.5, 3., 1.5, 0.5]);
    let samples: Vec<_> = [first, second]
        .into_iter()
        .enumerate()
        .map(|(index, mut data)| {
            data.normalized_series = Some(data.series.clone());
            CompositeSample {
                id: GroupId::legacy_result(index as u64 + 1),
                is_current: false,
                label: data.label.clone(),
                color: if index == 0 { "#0072b2" } else { "#d55e00" }.into(),
                figures: vec![Arc::new(data)],
            }
        })
        .collect();
    let empty: Vec<FigureData> = Vec::new();
    let prepared = prepare_panels(&recipe, &empty, &samples, &FigureSettings::default()).unwrap();
    let files = composite_python_export("Difference", &recipe, &prepared).unwrap();
    let script = &files[0].1;
    assert!(script.contains("\"quantity\": \"Δμ\""));
    assert!(script.contains("\"bounds\": (0.54, 0.32, 0.43, 0.36)"));
    assert!(script.contains("\"offset\": 0.3"));
    assert!(
        prepared.panels[0]
            .inset
            .as_ref()
            .unwrap()
            .offsets
            .iter()
            .all(Option::is_none)
    );
    let csv = &files
        .iter()
        .find(|(name, _)| name == "a-inset-series-02.csv")
        .unwrap()
        .1;
    let mut reader = csv::Reader::from_reader(csv.as_bytes());
    assert_eq!(&reader.headers().unwrap()[1], "B: Δμ");
    let values: Vec<(f64, f64)> = reader.deserialize().map(Result::unwrap).collect();
    assert_eq!(
        values.iter().map(|point| point.1).collect::<Vec<_>>(),
        [0.5, 0.5, 1., 0.5, 0.5]
    );

    if let Some(root) = std::env::var_os("REXAFS_COMPOSITE_PYTHON_EXPORT_DIR") {
        // Smooth synthetic edges make both the direct labels and legend useful
        // in a rendered smoke check; these values are not measurement data.
        let mut visual_samples = samples.clone();
        for (index, source) in visual_samples.iter_mut().enumerate() {
            let data = Arc::make_mut(&mut source.figures[0]);
            let x: Vec<_> = (0..=200).map(|point| 7092. + point as f64 * 0.16).collect();
            let y: Vec<_> = x
                .iter()
                .map(|energy| {
                    let edge = 1. / (1. + (-(energy - 7101. - index as f64 * 0.4)).exp());
                    let peak =
                        (0.12 + index as f64 * 0.08) * (-((energy - 7105.) / 1.5).powi(2)).exp();
                    edge + peak
                })
                .collect();
            data.series[0].x = x;
            data.series[0].y = y;
            data.normalized_series = Some(data.series.clone());
            data.default_xlim = Some((7092., 7124.));
        }
        let mut visual_recipe = recipe.clone();
        visual_recipe.panels = vec![
            CompositePanel {
                inset: Some(PanelInset {
                    energy_range: (-3., 12.),
                    ..visual_recipe.panels[0].inset.clone().unwrap()
                }),
                ..visual_recipe.panels[0].clone()
            },
            CompositePanel {
                offset_step: 0.3,
                reverse: true,
                .."xanes".into()
            },
        ];
        visual_recipe.figure_labels = [
            (visual_samples[0].id.clone(), "Reference".into()),
            (visual_samples[1].id.clone(), "Comparison".into()),
        ]
        .into();
        visual_recipe.panel_height_mm = Some(65.);
        let visual_prepared = prepare_panels(
            &visual_recipe,
            &empty,
            &visual_samples,
            &FigureSettings::default(),
        )
        .unwrap();
        fs::create_dir_all(&root).unwrap();
        write_composite_python_export(
            "Synthetic stacked difference inset",
            &visual_recipe,
            &visual_prepared,
            &Path::new(&root).join("difference-inset"),
        )
        .unwrap();
    }
}

#[test]
fn main_and_inset_exports_preserve_resolved_palettes_and_colour_precedence() {
    let samples = [
        sample(1, "Workspace A", "#abcdef", vec![0., 1., 2., 1., 0.]),
        sample(2, "Workspace B", "#123456", vec![0.5, 1.5, 3., 1.5, 0.5]),
    ];
    let empty: Vec<FigureData> = Vec::new();
    for (mode, palette) in [
        (SampleColours::Ordered, ["#440154", "#95d63f"]),
        (SampleColours::OkabeIto, ["#000000", "#e69f00"]),
        (SampleColours::Workspace, ["#abcdef", "#123456"]),
    ] {
        for kind in InsetKind::ALL {
            let mut recipe = CompositeRecipe {
                samples: samples.iter().map(|sample| sample.id.clone()).collect(),
                figure_labels: [
                    (samples[0].id.clone(), "Figure A".into()),
                    (samples[1].id.clone(), "Figure B".into()),
                ]
                .into(),
                sample_colours: mode,
                panels: vec![CompositePanel {
                    inset: Some(PanelInset {
                        energy_range: (-1., 3.),
                        ..PanelInset::new(kind)
                    }),
                    .."xanes".into()
                }],
                dpi: 100.,
                ..Default::default()
            };
            let mut settings = FigureSettings::default();
            for overrides in [false, true] {
                if overrides {
                    settings.figures.insert(
                        "xanes".into(),
                        FigureOptions {
                            series_colors: [
                                ("mu".into(), "#111111".into()),
                                ("result:1/mu".into(), "#222222".into()),
                                ("result:2/mu".into(), "#333333".into()),
                            ]
                            .into(),
                            ..Default::default()
                        },
                    );
                    recipe
                        .series_colors
                        .insert("result:2/mu".into(), "#444444".into());
                }
                let expected = if overrides {
                    ["#222222", "#444444"]
                } else {
                    palette
                };
                let prepared = prepare_panels(&recipe, &empty, &samples, &settings).unwrap();
                let files =
                    composite_python_export("Resolved colours", &recipe, &prepared).unwrap();
                let records = series_records(&files[0].1);
                assert_eq!(records.len(), 4);
                for prefix in ["a-series", "a-inset-series"] {
                    for (index, expected) in expected.into_iter().enumerate() {
                        let file = format!("{prefix}-{:02}.csv", index + 1);
                        let record = records
                            .iter()
                            .find(|record| record["file"] == file)
                            .unwrap();
                        assert_eq!(record["color"], expected, "{mode:?}, {kind:?}, {overrides}");
                        assert_eq!(
                            record["label"],
                            format!("Figure {}", if index == 0 { "A" } else { "B" })
                        );
                    }
                }
                assert!(
                    prepared.panels[0]
                        .options
                        .series_colors
                        .values()
                        .any(|colour| colour == expected[0])
                );
            }
        }
    }
}

#[test]
fn recipe_grid_overrides_figure_grid_while_insets_always_hide_it() {
    let samples = [
        sample(1, "A", "#0072b2", vec![0., 1., 2., 1., 0.]),
        sample(2, "B", "#d55e00", vec![0.5, 1.5, 3., 1.5, 0.5]),
    ];
    let empty: Vec<FigureData> = Vec::new();
    for (recipe_grid, figure_grid) in [(None, true), (Some(false), true), (Some(true), false)] {
        let mut recipe = CompositeRecipe {
            panels: vec![CompositePanel {
                inset: Some(PanelInset::new(InsetKind::Edge)),
                .."xanes".into()
            }],
            samples: samples.iter().map(|sample| sample.id.clone()).collect(),
            dpi: 100.,
            ..Default::default()
        };
        if let Some(grid) = recipe_grid {
            recipe.grid = grid;
        }
        let mut settings = FigureSettings::default();
        settings.figures.insert(
            "xanes".into(),
            FigureOptions {
                grid: Some(figure_grid),
                ..Default::default()
            },
        );
        let prepared = prepare_panels(&recipe, &empty, &samples, &settings).unwrap();
        assert_eq!(prepared.panels[0].options.grid, Some(recipe.grid));
        assert_eq!(
            prepared.panels[0].inset.as_ref().unwrap().options.grid,
            Some(false)
        );
        let files = composite_python_export("Grid choice", &recipe, &prepared).unwrap();
        assert_eq!(main_panel_flags(&files[0].1, "grid"), [recipe.grid]);
        let inset = files[0].1.split("\"inset\": {").nth(1).unwrap();
        assert!(
            inset
                .split("\"series\": [")
                .next()
                .unwrap()
                .contains("\"grid\": False")
        );
    }
}

#[test]
fn exported_legend_visibility_matches_prepared_panels_and_keeps_single_and_fit_legends() {
    let samples = [
        sample(1, "A", "#0072b2", vec![0., 1., 2., 1., 0.]),
        sample(2, "B", "#d55e00", vec![0.5, 1.5, 3., 1.5, 0.5]),
    ];
    let current = [figure("fit-q", "Current fit", vec![0., 1., 2., 1., 0.])];
    let mut recipe = CompositeRecipe {
        panels: vec![
            CompositePanel {
                samples: Some(vec![samples[0].id.clone()]),
                .."xanes".into()
            },
            "fit-q".into(),
            CompositePanel {
                label_mode: CurveLabelMode::CurveEnd,
                .."xanes".into()
            },
            CompositePanel {
                inset: Some(PanelInset::new(InsetKind::Edge)),
                .."xanes".into()
            },
            "xanes".into(),
        ],
        samples: samples.iter().map(|sample| sample.id.clone()).collect(),
        dpi: 100.,
        ..Default::default()
    };
    for (every_panel, expected) in [
        (false, [true, true, false, true, false]),
        (true, [true, true, false, true, true]),
    ] {
        recipe.legend_every_panel = every_panel;
        let prepared =
            prepare_panels(&recipe, &current, &samples, &FigureSettings::default()).unwrap();
        assert_eq!(
            prepared
                .panels
                .iter()
                .map(|panel| panel.options.legend)
                .collect::<Vec<_>>(),
            expected
        );
        assert!(!prepared.panels[3].inset.as_ref().unwrap().options.legend);
        let files = composite_python_export("Legend choice", &recipe, &prepared).unwrap();
        assert_eq!(main_panel_flags(&files[0].1, "legend"), expected);
        let inset = files[0].1.split("\"inset\": {").nth(1).unwrap();
        assert!(
            inset
                .split("\"series\": [")
                .next()
                .unwrap()
                .contains("\"legend\": False")
        );
    }
    let mut settings = FigureSettings::default();
    settings.figures.insert(
        "xanes".into(),
        FigureOptions {
            legend: false,
            ..Default::default()
        },
    );
    let prepared = prepare_panels(&recipe, &current, &samples, &settings).unwrap();
    let files = composite_python_export("Hidden sample legends", &recipe, &prepared).unwrap();
    assert_eq!(
        main_panel_flags(&files[0].1, "legend"),
        [false, true, false, false, false]
    );
}

#[test]
fn empty_and_hidden_panels_are_skipped_and_all_missing_is_an_error() {
    let (mut recipe, mut prepared) = fixture();
    prepared.panels[0]
        .options
        .hidden
        .extend(["result:1/mu".into(), "result:2/mu".into()]);
    prepared.panels[1].data.series[0].x.clear();
    prepared.panels[1].data.series[0].y.clear();
    let files = composite_python_export("Available fit", &recipe, &prepared).unwrap();
    assert!(files[0].1.contains("ROWS = 1\nCOLS = 1"));
    assert!(files[0].1.contains("xanes: no visible curves with data"));
    assert!(files[0].1.contains("chi-k: no visible curves with data"));
    assert!(files[0].1.contains("\"label\": \"(a)\""));
    assert!(!files[0].1.contains("\"label\": \"(b)\""));
    prepared.panels.clear();
    assert!(
        composite_python_export("Empty", &recipe, &prepared)
            .unwrap_err()
            .contains("Missing: fit-q")
    );
    recipe.width_mm = f64::NAN;
    assert!(composite_python_export("Invalid", &recipe, &fixture().1).is_err());
}

#[test]
fn repeated_sample_ids_keep_independent_offsets_after_visibility_filtering() {
    let source = figure("xanes", "Repeated", vec![0., 1., 2., 1., 0.]);
    let sample = CompositeSample {
        id: GroupId::legacy_result(1),
        is_current: false,
        label: "Repeated".into(),
        color: "#0072b2".into(),
        figures: vec![Arc::new(source)],
    };
    let recipe = CompositeRecipe {
        panels: vec![CompositePanel {
            offset_step: 0.3,
            ..CompositePanel::from("xanes")
        }],
        samples: vec![sample.id.clone(), sample.id.clone()],
        ..Default::default()
    };
    let empty: Vec<FigureData> = Vec::new();
    let prepared = prepare_panels(&recipe, &empty, &[sample], &FigureSettings::default()).unwrap();
    let files = composite_python_export("Repeated", &recipe, &prepared).unwrap();
    let series: Vec<_> = files[0]
        .1
        .lines()
        .filter(|line| line.contains("\"file\": "))
        .collect();
    assert_eq!(series.len(), 2);
    assert!(series[0].contains("\"offset\": 0.0"));
    assert!(series[1].contains("\"offset\": 0.3"));
    assert_eq!(files[1].1, files[2].1);

    let (recipe, mut prepared) = fixture();
    prepared.panels[0]
        .options
        .hidden
        .insert("result:1/mu".into());
    let files = composite_python_export("Hidden first", &recipe, &prepared).unwrap();
    let first = files[0]
        .1
        .lines()
        .find(|line| line.contains("\"file\": "))
        .unwrap();
    assert!(first.contains("\"offset\": 0.0"));
    assert!(first.contains("#d55e00"));
}

#[test]
fn invalid_data_fail_before_writing_and_existing_exports_are_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("Figure 1");
    let (recipe, mut prepared) = fixture();
    prepared.panels[0].data.series[0].x.pop();
    assert!(write_composite_python_export("Figure 1", &recipe, &prepared, &folder).is_err());
    assert!(!folder.exists());
    let (_, prepared) = fixture();
    let path = write_composite_python_export("Figure 1", &recipe, &prepared, &folder).unwrap();
    fs::write(&path, "user edits").unwrap();
    assert!(write_composite_python_export("Figure 1", &recipe, &prepared, &folder).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "user edits");
}
