//! Named compositions shared by the analysis-folder and supporting-information exports.
use super::{SpectrumInput, figures, python_export};
use crate::group_identity::GroupId;
use figures::composer::{CompositeSample, compose_prepared, prepare_panels};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path, sync::Arc};

/// Session inputs captured before export, independently of the saved project's
/// source scope. Uncached samples are processed on the export worker.
#[derive(Default)]
pub(crate) struct ComposerInput {
    /// Current spectrum and selected fit dataset, as used by the Publish editor.
    pub current: Vec<Arc<figures::FigureData>>,
    /// Sources needed by all named recipes, including inactive recipes.
    pub samples: Vec<SampleInput>,
}

/// One workspace sample's durable identity, appearance and processing inputs.
pub(crate) struct SampleInput {
    /// Durable workspace group ID retained in the recipe.
    pub id: GroupId,
    /// Workspace plot colour as an opaque lowercase `#rrggbb` value.
    pub color: String,
    /// Current label, quantity, effective parameters and any cached spectrum.
    pub spectrum: SpectrumInput,
}

/// Write every renderable recipe after the single-figure assets. Recipe and
/// rendering failures become notices; required file-write failures abort.
/// Captions and matplotlib data use the same prepared panels as the images.
pub(super) fn write(
    input: &ComposerInput,
    settings: &figures::FigureSettings,
    destination: &Path,
    assets: &mut Vec<Value>,
    notices: &mut Vec<String>,
) -> Result<(), String> {
    let samples: Vec<_> = input
        .samples
        .iter()
        .map(|source| {
            let spectrum = &source.spectrum;
            // These cached arrays were captured at the current workspace
            // revision. Keep them even if the original source is now offline.
            let displayed = match (&spectrum.source_error, &spectrum.data) {
                (None, Some(data)) => Ok(data.clone()),
                _ => spectrum.for_display(),
            };
            let figures = match displayed {
                Ok(sp) => figures::quantity_figures(
                    sp,
                    &spectrum.label(),
                    spectrum.group.as_ref().map(|group| group.quantity),
                )
                .into_iter()
                .map(Arc::new)
                .collect(),
                Err(error) => {
                    notices.push(format!(
                        "Composer sample {} ({}): {error}",
                        spectrum.label(),
                        source.id.as_str()
                    ));
                    Vec::new()
                }
            };
            CompositeSample {
                id: source.id.clone(),
                label: spectrum.label(),
                color: source.color.clone(),
                figures,
            }
        })
        .collect();
    let folder = destination.join("figures/composed");
    let mut stems = BTreeSet::new();
    for (index, named) in settings.recipes().iter().enumerate() {
        // Distinct recipe names can have the same sanitized file stem.
        let base = named.file_stem();
        let stem = (1usize..)
            .map(|n| {
                if n == 1 {
                    base.clone()
                } else {
                    format!("{base}-{n}")
                }
            })
            .find(|stem| stems.insert(stem.clone()))
            .unwrap();
        let rendered = (|| {
            let prepared = prepare_panels(&named.recipe, &input.current, &samples, settings)?;
            let figure = compose_prepared(&named.recipe, &prepared, None)?;
            let png = figure.png()?;
            let pdf = figure.pdf()?;
            Ok::<_, String>((prepared, figure, png, pdf))
        })();
        let (prepared, figure, png, pdf) = match rendered {
            Ok(rendered) => rendered,
            Err(error) => {
                notices.push(format!("Composed figure {}: {error}", named.name));
                continue;
            }
        };
        fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        for (suffix, bytes) in [
            (".png", png.as_slice()),
            (".svg", figure.svg.as_bytes()),
            (".pdf", pdf.as_slice()),
            ("-caption.txt", figure.caption.as_bytes()),
        ] {
            fs::write(folder.join(format!("{stem}{suffix}")), bytes).map_err(|e| e.to_string())?;
        }
        for missing in &figure.missing {
            notices.push(format!(
                "Composed figure {}: skipped {missing}.",
                named.name
            ));
        }
        let mut asset = json!({
            "file": format!("figures/composed/{stem}.png"),
            "svg": format!("figures/composed/{stem}.svg"),
            "pdf": format!("figures/composed/{stem}.pdf"),
            "caption_file": format!("figures/composed/{stem}-caption.txt"),
            "kind": "composed",
            "recipe_name": named.name,
            "recipe_index": index,
            "number": assets.len() + 1,
            "caption": figure.caption,
        });
        match python_export::write_composite_python_export(
            &named.name,
            &named.recipe,
            &prepared,
            &folder.join(format!("{stem}-matplotlib")),
        ) {
            Ok(_) => {
                asset["python"] = json!(format!(
                    "figures/composed/{stem}-matplotlib/{}",
                    python_export::SCRIPT_NAME
                ));
            }
            Err(error) => notices.push(format!(
                "Composed figure {}: matplotlib export unavailable: {error}",
                named.name
            )),
        }
        assets.push(asset);
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::params::{DerivedSpectrum, Quantity};
    use crate::publication::figures::composer::{CompositePanel, CompositeRecipe};
    use crate::publication::{Snapshot, SpectrumInput};

    /// A selected difference, uncached and cached recipe-only groups, two
    /// renderable colliding names, an invalid recipe and a wholly missing source.
    pub(in crate::publication) fn snapshot() -> Snapshot {
        let current = DerivedSpectrum {
            id: 1,
            label: "Current difference".into(),
            quantity: Quantity::NormalizedDifference,
            energy: vec![7100., 7101., 7102.],
            mu: vec![-0.1, 0., 0.2],
            ..Default::default()
        };
        let other = DerivedSpectrum {
            id: 2,
            label: "Recipe-only difference".into(),
            mu: vec![0.2, -0.3, 0.4],
            ..current.clone()
        };
        let mut settings = figures::FigureSettings::default();
        settings.figures.insert(
            "mu-energy".into(),
            figures::FigureOptions {
                caption: Some("Difference <caption> & selected curves.".into()),
                width: Some(4.),
                height: Some(3.),
                dpi: Some(72.),
                ..Default::default()
            },
        );
        let recipe = CompositeRecipe {
            panels: vec!["mu-energy".into(), "fit-k".into()],
            width_mm: 101.6,
            panel_height_mm: Some(76.2),
            dpi: 72.,
            ..Default::default()
        };
        settings.rename_recipe("A/B").unwrap();
        *settings.active_composite_mut() = recipe.clone();
        settings.new_recipe();
        settings.rename_recipe("Bad <recipe>").unwrap();
        *settings.active_composite_mut() = CompositeRecipe {
            columns: 0,
            ..recipe.clone()
        };
        settings.new_recipe();
        settings.rename_recipe("Missing source").unwrap();
        *settings.active_composite_mut() = CompositeRecipe {
            samples: vec![GroupId::legacy_result(99)],
            panels: vec!["mu-energy".into()],
            ..recipe.clone()
        };
        settings.new_recipe();
        settings.rename_recipe("A-B").unwrap();
        *settings.active_composite_mut() = CompositeRecipe {
            // A panel override must resolve independently of inherited samples.
            samples: vec![GroupId::legacy_result(99)],
            panels: vec![CompositePanel {
                samples: Some(vec![GroupId::legacy_result(2), GroupId::legacy_result(3)]),
                ..CompositePanel::from("mu-energy")
            }],
            ..recipe
        };
        let spectrum = SpectrumInput {
            label: current.label.clone(),
            group: Some(current.clone()),
            ..Default::default()
        };
        let current_figures = figures::quantity_figures(
            spectrum.for_display().unwrap(),
            &current.label,
            Some(current.quantity),
        )
        .into_iter()
        .map(Arc::new)
        .collect();
        let mut cached = DerivedSpectrum {
            id: 3,
            label: "Cached session difference".into(),
            ..current.clone()
        };
        let cached_data = SpectrumInput {
            group: Some(cached.clone()),
            ..Default::default()
        }
        .for_display()
        .unwrap();
        cached.source = Some("/missing/rexafs-composer-cached.dat".into());
        let cached_sample = SampleInput {
            id: GroupId::legacy_result(3),
            color: "#0072b2".into(),
            spectrum: SpectrumInput {
                label: cached.label.clone(),
                group: Some(cached),
                data: Some(cached_data),
                ..Default::default()
            },
        };
        Snapshot {
            project: crate::project::ProjectFile {
                derived: vec![current],
                publication: settings,
                ..Default::default()
            },
            current: Default::default(),
            spectra: vec![spectrum],
            results: Default::default(),
            composer: ComposerInput {
                current: current_figures,
                samples: vec![
                    cached_sample,
                    SampleInput {
                        id: GroupId::legacy_result(2),
                        color: "#d55e00".into(),
                        spectrum: SpectrumInput {
                            label: other.label.clone(),
                            group: Some(other),
                            ..Default::default()
                        },
                    },
                ],
            },
            analysis: Value::Null,
            batch_csv: None,
            batch_stale: false,
            journal: vec![],
            screen: Value::Null,
        }
    }

    #[test]
    fn named_recipes_export_images_captions_and_session_samples_despite_failures() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("analysis");
        crate::publication::export(snapshot(), &folder).unwrap();
        let manifest: Value =
            serde_json::from_slice(&fs::read(folder.join("manifest.json")).unwrap()).unwrap();
        let assets = manifest["figures"].as_array().unwrap();
        assert_eq!(assets.len(), 3);
        assert_eq!(assets[0]["kind"], "mu-energy");
        assert_eq!(assets[1]["recipe_name"], "A/B");
        assert_eq!(assets[1]["recipe_index"], 0);
        assert_eq!(assets[2]["recipe_name"], "A-B");
        assert_eq!(assets[2]["recipe_index"], 3);
        for (index, stem) in ["a-b", "a-b-2"].into_iter().enumerate() {
            let asset = &assets[index + 1];
            assert_eq!(asset["number"], index + 2);
            let base = folder.join("figures/composed");
            let png = fs::read(base.join(format!("{stem}.png"))).unwrap();
            assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
            assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 288);
            let pdf = fs::read(base.join(format!("{stem}.pdf"))).unwrap();
            assert!(pdf.starts_with(b"%PDF-"));
            let svg = fs::read_to_string(base.join(format!("{stem}.svg"))).unwrap();
            assert!(svg.contains("<svg"));
            let caption = fs::read_to_string(base.join(format!("{stem}-caption.txt"))).unwrap();
            assert_eq!(caption, asset["caption"].as_str().unwrap());
            assert!(caption.starts_with("(a) Difference <caption> & selected curves."));
            assert!(folder.join(asset["python"].as_str().unwrap()).is_file());
        }
        let script =
            fs::read_to_string(folder.join(assets[2]["python"].as_str().unwrap())).unwrap();
        assert!(script.contains("Recipe-only difference"));
        assert!(script.contains("Cached session difference"));
        assert!(script.contains("#d55e00"));
        let csv =
            fs::read_to_string(folder.join("figures/composed/a-b-2-matplotlib/a-series-01.csv"))
                .unwrap();
        let values: Vec<(f64, f64)> = csv::Reader::from_reader(csv.as_bytes())
            .deserialize()
            .map(Result::unwrap)
            .collect();
        assert_eq!(values, vec![(7100., 0.2), (7101., -0.3), (7102., 0.4)]);
        let notices = manifest["notices"].as_array().unwrap();
        assert!(
            notices
                .iter()
                .any(|n| n.as_str().unwrap().contains("A/B: skipped fit-k"))
        );
        assert!(notices.iter().any(|n| {
            n.as_str()
                .unwrap()
                .contains("Bad <recipe>: Use 1–4 columns")
        }));
        assert!(notices.iter().any(|n| {
            n.as_str()
                .unwrap()
                .contains("Missing source: Add at least one panel")
        }));
        assert!(!folder.join("figures/composed/bad-recipe.png").exists());
        let report = fs::read_to_string(folder.join("report.html")).unwrap();
        assert!(
            report.find("spectrum-001-mu-energy.svg").unwrap()
                < report.find("<h2>Composed figures</h2>").unwrap()
        );
        assert!(report.contains("A/B. (a) Difference &lt;caption&gt; &amp; selected curves."));
        assert!(report.contains("A-B. (a) Difference &lt;caption&gt; &amp; selected curves."));
        assert!(report.contains("Bad &lt;recipe&gt;: Use 1–4 columns"));
        let captions = fs::read_to_string(folder.join("captions.md")).unwrap();
        assert!(captions.contains("**Figure 2.** A/B. (a) Difference <caption>"));
        assert!(captions.contains("**Figure 3.** A-B. (a) Difference <caption>"));
        let restored = crate::project::tests::load(&folder.join("project.rxs")).unwrap();
        assert_eq!(restored.derived.len(), 1);
        assert_eq!(restored.derived[0].id, 1);
    }
}
