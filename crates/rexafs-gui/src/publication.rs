//! A local, inspectable analysis record. The Assistant uses a separate summary.
pub(crate) mod assistant_context;
pub(crate) mod figures;
pub(crate) mod report;
use crate::{params::PipelineParams, plotting, project::ProjectFile, theme::Theme};
use rexafs::prelude::{FeffFitResult, XASSpectrum};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Default)]
pub(crate) struct SpectrumInput {
    pub source_error: Option<String>,
    pub label: String,
    pub group: Option<crate::params::DerivedSpectrum>,
    pub path: PathBuf,
    pub params: PipelineParams,
    pub data: Option<Arc<XASSpectrum>>,
}
impl SpectrumInput {
    pub fn for_display(&self) -> Result<Arc<XASSpectrum>, String> {
        if self.source_error.is_none()
            && let Some(group) = &self.group
        {
            return group.for_display(&self.params).map(Arc::new);
        }
        self.process()
    }

    pub fn label(&self) -> String {
        if self.label.is_empty() {
            self.path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        } else {
            self.label.clone()
        }
    }
    pub fn process(&self) -> Result<Arc<XASSpectrum>, String> {
        if let Some(error) = &self.source_error {
            return Err(error.clone());
        }
        if let Some(reason) = self
            .group
            .as_ref()
            .and_then(|g| g.processing_block_reason())
        {
            return Err(reason);
        }
        if let Some(data) = &self.data {
            return Ok(data.clone());
        }
        match &self.group {
            Some(group) => group.process(&self.params),
            None => crate::params::process_file(&self.path, &self.params),
        }
        .map(Arc::new)
    }
}
pub(crate) struct Snapshot {
    pub project: ProjectFile,
    pub current: PathBuf,
    pub spectra: Vec<SpectrumInput>,
    pub results: BTreeMap<usize, Arc<FeffFitResult>>,
    pub analysis: Value,
    pub batch_csv: Option<String>,
    pub batch_stale: bool,
    pub journal: Vec<String>,
    pub screen: Value,
}
fn cell(s: impl AsRef<str>) -> String {
    s.as_ref().replace('|', "\\|").replace('\n', " ")
}
/// Source comments are untrusted experimental metadata, bounded before parsing.
fn source_comments(path: &Path) -> Vec<String> {
    use std::io::{BufRead, BufReader, Read};
    let Ok(file) = fs::File::open(path) else {
        return vec![];
    };
    BufReader::new(file.take(32_768))
        .lines()
        .take(256)
        .map_while(Result::ok)
        .take_while(|line| {
            line.trim().is_empty() || line.trim_start().starts_with(['#', ';', '%', '!'])
        })
        .filter(|line| !line.trim().is_empty())
        .collect()
}
impl Snapshot {
    /// Process each spectrum that has no result yet, so the record's "Used"
    /// column shows the values processing applied: resolved Auto values and
    /// ranges limited to short scans. A spectrum that cannot be processed keeps
    /// no result and its "Used" cells read "Unavailable".
    pub(crate) fn resolve_spectra(&mut self) {
        for spectrum in &mut self.spectra {
            if spectrum.data.is_none() {
                spectrum.data = spectrum.for_display().ok();
            }
        }
    }
    pub(crate) fn context(&self) -> Value {
        // Fit curve arrays are exported once, as data/fit-<id>.json.
        let mut project = serde_json::to_value(&self.project).unwrap_or_default();
        if let Some(object) = project.as_object_mut() {
            object.remove("fit_results");
        }
        json!({
            "software":{"name":"rexafs","version":env!("CARGO_PKG_VERSION")},
            "current_spectrum":self.current, "screen":self.screen,
            "project":project,
            "effective_processing":self.spectra.iter().map(|s|json!({"file":s.path,"group":s.label(),"group_id":s.group.as_ref().map(|g|g.id),"requested":s.params,"source_comments":source_comments(&s.path)})).collect::<Vec<_>>(),
            "analysis":self.analysis, "batch_results_stale":self.batch_stale, "journal":self.journal,
            "semantics":{"null_processing_value":"Auto; resolved numbers are in processed-spectrum JSON when available", "fit_history":"Historical inputs and values; not necessarily the currently edited model", "path_distance":"R_eff + deltaR; for multiple scattering, half the total scattering path length", "stderr":"One standard error from the fit covariance, not total experimental uncertainty"}
        })
    }
    pub(crate) fn markdown(&self) -> String {
        let mut text = format!(
            "# rexafs analysis record\n\nVersion {}. Current spectrum: `{}`.\n\nThis is a snapshot of requested processing settings and recorded fit results. Auto values are resolved separately for each spectrum; the Used column gives the values processing applied, including ranges limited to the measured data. Current settings may differ from historical fit settings.\n\n",
            env!("CARGO_PKG_VERSION"),
            cell(self.current.display().to_string())
        );
        let tables = self.tables();
        let mut table = tables.iter();
        for (i, s) in self.spectra.iter().enumerate() {
            text.push_str(&format!(
                "## Spectrum {} — {}\n\nSource: `{}`\n\n{}\n",
                i + 1,
                cell(s.label()),
                cell(s.path.display().to_string()),
                table.next().unwrap().markdown()
            ));
            text.push_str(&format!(
                "All requested processing/import settings:\n\n```json\n{}\n```\n\n",
                serde_json::to_string_pretty(&s.params).unwrap()
            ));
            let comments = source_comments(&s.path);
            if !comments.is_empty() {
                text.push_str("Source comments (verbatim metadata):\n\n");
                for line in comments {
                    text.push_str(&format!("> {}\n", line));
                }
                text.push('\n');
            }
        }
        text.push_str("## Current fit model\n\n```json\n");
        text.push_str(&serde_json::to_string_pretty(&json!({"ranges":self.project.fit_ranges,"parameters":self.project.fit_vars,"paths":self.project.fit_paths,"multiple_spectra":self.project.joint})).unwrap());
        text.push_str("\n```\n\n## Fit history\n\n");
        if self.project.fit_history.is_empty() {
            text.push_str("No completed fits recorded.\n\n");
        }
        for fit in &self.project.fit_history {
            text.push_str(&format!("### Fit {} — {}\n\nR factor: {:.6}; reduced χ²: {}; χ²: {:.6}; N independent: {:.3}; varying parameters: {}.\n\n", fit.id, cell(&fit.group), fit.r_factor, fit.reduced_chi_square_text(), fit.chi_square, fit.n_idp, fit.n_vary));
            text.push_str(&table.next().unwrap().markdown());
            text.push_str(&table.next().unwrap().markdown());
            text.push_str("\nRecorded inputs, per-spectrum ranges, path expressions and solver diagnostics:\n\n```json\n");
            text.push_str(&serde_json::to_string_pretty(fit).unwrap());
            text.push_str("\n```\n\n");
        }
        text.push_str("## Additional analyses\n\n```json\n");
        text.push_str(&serde_json::to_string_pretty(&self.analysis).unwrap());
        text.push_str("\n```\n\n## Actions\n\n");
        for line in &self.journal {
            text.push_str(&format!("- {}\n", cell(line)));
        }
        text
    }
}
fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn save_figure(
    data: &figures::FigureData,
    settings: &figures::FigureSettings,
    path: &Path,
) -> Result<(), String> {
    let rendered = figures::render_figure(data, &settings.options(data.key))?;
    fs::write(path, rendered.png).map_err(|e| e.to_string())?;
    fs::write(path.with_extension("svg"), rendered.svg).map_err(|e| e.to_string())?;
    fs::write(
        path.with_extension("csv"),
        data.csv(&settings.options(data.key))?,
    )
    .map_err(|e| e.to_string())
}
pub(crate) fn spectrum_plots(
    sp: Arc<XASSpectrum>,
    label: &str,
) -> Vec<(&'static str, ruviz::prelude::Plot)> {
    let theme = Theme::light();
    let opts = plotting::ViewOptions {
        flat: false,
        show_re: true,
        show_im: true,
        show_bkg: true,
        show_pre: true,
        show_post: true,
        show_e0: true,
        show_ranges: true,
        show_kwin: true,
        ..Default::default()
    };
    let traces = [plotting::QuadTrace {
        color_index: 0,
        color: None,
        label: label.into(),
        sp,
        active: true,
    }];
    let specs = plotting::build_quadrant_specs(&traces, &opts, &theme, true);
    ["mu-energy", "normalized-mu", "chi-k", "chi-r", "chi-q"]
        .into_iter()
        .zip(specs)
        .map(|(name, mut spec)| {
            spec.legend_columns = Some(3);
            if let Some(series) = spec.series.first_mut() {
                series.label = Some(label.to_owned());
            }
            (name, spec.to_plot(&theme).0)
        })
        .collect()
}
pub(crate) fn resolved_settings(sp: &XASSpectrum) -> Value {
    fn scalars(v: Value) -> Value {
        match v {
            Value::Object(map) => Value::Object(
                map.into_iter()
                    .filter_map(|(k, v)| {
                        if v.is_array() {
                            None
                        } else {
                            Some((k, scalars(v)))
                        }
                    })
                    .collect(),
            ),
            _ => v,
        }
    }
    let values = serde_json::to_value(sp).unwrap_or(Value::Null);
    let mut out = serde_json::Map::new();
    for key in [
        "e0",
        "energy_shift",
        "normalization",
        "background",
        "xftf",
        "xftr",
    ] {
        out.insert(key.into(), scalars(values[key].clone()));
    }
    Value::Object(out)
}
/// Export a snapshot into a newly created analysis directory.
///
/// Writes the selected project storage mode, processed spectrum arrays, available
/// fit arrays, PNG/SVG/CSV figures, captions, methods and reference drafts.
/// Per-spectrum processing failures, failed figures and unavailable historical
/// arrays are retained as notices; a successful return can therefore describe an
/// incomplete analysis. Inspect `manifest.json`, the report and the README.
///
/// An existing destination is rejected. Directory/file write failures abort with
/// an error and can leave a partial directory, possibly before a manifest exists.
/// The folder is not transactional and is not removed on failure. Input files and
/// the live analysis are unchanged; retry into a new directory after resolving
/// the reported cause.
pub(crate) fn export(mut snapshot: Snapshot, destination: &Path) -> Result<PathBuf, String> {
    fs::create_dir(destination).map_err(|e| format!("{}: {e}", destination.display()))?;
    let figs = destination.join("figures");
    let data = destination.join("data");
    fs::create_dir(&figs).map_err(|e| e.to_string())?;
    fs::create_dir(&data).map_err(|e| e.to_string())?;
    write_json(&destination.join("state.json"), &snapshot.context())?;
    crate::project::save(&destination.join("project.rxs"), &snapshot.project)?;
    if let Some(series) = &snapshot.project.lcf_series_analysis {
        write_json(&data.join("lcf-series.json"), series)?;
        let quote = |v: &str| format!("\"{}\"", v.replace('"', "\"\""));
        let mut csv = format!(
            "frame,label,{},r_factor\n",
            series
                .standards
                .iter()
                .map(|s| quote(&s.label))
                .collect::<Vec<_>>()
                .join(",")
        );
        for (frame, values) in &series.rows {
            let label = series.inputs.get(frame).map_or("", |s| s.label.as_str());
            csv.push_str(&format!(
                "{},{},{}\n",
                frame + 1,
                quote(label),
                values
                    .iter()
                    .map(|v| format!("{v:.17e}"))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        fs::write(data.join("lcf-series.csv"), csv).map_err(|e| e.to_string())?;
    }
    let project_sources = match snapshot.project.data_storage {
        crate::project::DataStorage::Paths => {
            "The .rxs project links to source spectra and FEFF inputs using paths relative to its directory. Move the source folders with it, or select Raw: embedded before exporting a portable archive."
        }
        crate::project::DataStorage::Embedded => {
            "The .rxs project includes losslessly compressed original spectra and referenced FEFF inputs, with source metadata and checksums in its header."
        }
    };

    let mut assets = Vec::new();
    let mut errors = Vec::new();
    let mut resolved = String::from(
        "# Resolved processing\n\nRequested settings are in analysis.md. These outputs were recomputed with those settings at export time. Full resolved objects and arrays are in data/*.json.\n\n",
    );
    for (i, s) in snapshot.spectra.iter_mut().enumerate() {
        let id = format!("spectrum-{:03}", i + 1);
        // Export retained calculation evidence even when the selected downstream
        // processing cannot run on a short or edge-free interval.
        if let Some(group) = &s.group
            && let Some(operation) = &group.operation
            && matches!(
                operation.tool.as_str(),
                "MCR-ALS" | "Linear combination fit"
            )
        {
            write_json(&data.join(format!("{id}-provenance.json")), group)?;
            let mut text = format!(
                "# rexafs calculated spectrum; not a raw measurement\n# Quantity: {}\n# Retained calculated values; preserve scale by default. Optional baseline refits are recorded separately in processing settings.\n# Operation: {}\n# energy_eV calculated_signal\n",
                group.quantity.label(),
                serde_json::to_string(operation).map_err(|e| e.to_string())?
            );
            for (x, y) in group.energy.iter().zip(&group.mu) {
                text.push_str(&format!("{x:.17e} {y:.17e}\n"));
            }
            fs::write(data.join(format!("{id}-calculated.dat")), text)
                .map_err(|e| e.to_string())?;
        }
        let result = s.for_display();
        match result {
            Ok(sp) => {
                s.data = Some(sp.clone());
                write_json(&data.join(format!("{id}.json")), sp.as_ref())?;
                resolved.push_str(&format!(
                    "## {}\n\nE₀: {} eV. Transform k-weight: {}.\n\n",
                    cell(s.path.display().to_string()),
                    sp.e0()
                        .map(|v| format!("{v:.6}"))
                        .unwrap_or("Unavailable".into()),
                    sp.kweight()
                        .map(|v| v.to_string())
                        .unwrap_or("Unavailable".into())
                ));
                resolved.push_str(&format!(
                    "```json\n{}\n```\n\n",
                    serde_json::to_string_pretty(&resolved_settings(&sp)).unwrap()
                ));
                for figure in
                    figures::quantity_figures(sp, &s.label(), s.group.as_ref().map(|g| g.quantity))
                {
                    let filename = format!("{id}-{}.png", figure.key);
                    match save_figure(&figure,&snapshot.project.publication,&figs.join(&filename)) {Ok(())=>assets.push(json!({"file":format!("figures/{filename}"),"svg":format!("figures/{}",filename.replace(".png",".svg")),"csv":format!("figures/{}",filename.replace(".png",".csv")),"source":s.path,"group":s.label(),"group_id":s.group.as_ref().map(|g|g.id),"kind":figure.key,"number":assets.len()+1,"caption":format!("Spectrum {} ({}). {}",i+1,s.label(),figure.caption(&snapshot.project.publication.options(figure.key)))})),Err(e)=>errors.push(format!("{filename}: {e}"))}
                }
            }
            Err(e) => errors.push(format!("{}: {e}", s.path.display())),
        }
    }
    for (id, result) in &snapshot.results {
        write_json(&data.join(format!("fit-{id}.json")), result.as_ref())?;
        for index in 0..result.datasets.len().max(1) {
            let view = crate::joint_fitting::result_view(result, index);
            if view.k.is_empty() {
                continue;
            }
            for figure in figures::fit_figures(&view) {
                let filename = format!("fit-{id}-spectrum-{}-{}.png", index + 1, figure.key);
                match save_figure(&figure,&snapshot.project.publication,&figs.join(&filename)) {Ok(())=>assets.push(json!({"file":format!("figures/{filename}"),"svg":format!("figures/{}",filename.replace(".png",".svg")),"csv":format!("figures/{}",filename.replace(".png",".csv")),"fit_id":id,"dataset_index":index,"kind":figure.key,"number":assets.len()+1,"caption":format!("Fit {id}, dataset {}. {}",index+1,figure.caption(&snapshot.project.publication.options(figure.key)))})),Err(e)=>errors.push(format!("{filename}: {e}"))}
            }
        }
    }
    for fit in &snapshot.project.fit_history {
        if !snapshot.results.contains_key(&fit.id) {
            errors.push(format!("Fit {}: archived statistics and inputs exported; curve arrays are unavailable in this session.",fit.id));
        }
    }
    let tables = snapshot.tables();
    fs::write(destination.join("analysis.md"), snapshot.markdown()).map_err(|e| e.to_string())?;
    if snapshot.batch_stale {
        errors.push(
            "Batch results are stale: they do not reflect the current model/settings.".into(),
        );
    }
    if let Some(csv) = snapshot.batch_csv {
        fs::write(destination.join("batch-results.csv"), csv).map_err(|e| e.to_string())?;
    }
    fs::write(destination.join("resolved.md"), resolved).map_err(|e| e.to_string())?;
    fs::write(destination.join("methods.md"),"# Experimental and analysis methods — draft\n\n## Experiment\n\nSample composition, preparation, beamline, detector geometry, temperature and acquisition details: **supply from the experimental record**. They are not inferred from curve shape. Check file metadata before completing this section.\n\n## Analysis\n\nXAS spectra were processed with rexafs. Pre-edge subtraction and post-edge normalization were followed by AUTOBK spline background removal and Fourier transformation. Per-spectrum settings, resolved outputs and selected fit ranges are in analysis.md and resolved.md. FEFF path files, expressions, shared and local parameters, solver diagnostics and uncertainties are recorded for each completed fit. Select the relevant successful fit before describing conclusions.\n\nDistances for multiple-scattering paths denote half the total path length. Reported standard errors reflect the fit covariance; they do not include all experimental or model uncertainty. R-space spectra are not phase corrected.\n").map_err(|e|e.to_string())?;
    fs::write(destination.join("references.md"), REFERENCES).map_err(|e| e.to_string())?;
    fs::write(destination.join("references.bib"), BIBTEX).map_err(|e| e.to_string())?;
    write_json(
        &destination.join("manifest.json"),
        &json!({"figures":assets,"figure_settings":snapshot.project.publication,"tables":tables.iter().map(|t|json!({"number":t.number,"kind":t.kind,"caption":t.caption})).collect::<Vec<_>>(),"notices":errors,"batch_results_stale":snapshot.batch_stale,"project_storage":snapshot.project.data_storage,"project_sources":project_sources}),
    )?;
    report::write(destination, &assets, &tables, &errors)?;
    let mut index = String::from(
        "# Publication assets\n\n[Publication report](report.html) · [Captions](captions.md) · [Analysis and history](analysis.md) · [Resolved processing](resolved.md) · [Methods draft](methods.md) · [References](references.md) · [State JSON](state.json) · [Project](project.rxs)\n\nThis folder is a local export. Review the methods and reference choices before publication. Processed arrays are included in data/.\n\n",
    );
    index.push_str(project_sources);
    index.push_str("\n\n## Figures\n\n");
    for asset in &assets {
        if let Some(file) = asset["file"].as_str() {
            index.push_str(&format!(
                "![Figure {}]({file})\n\n**Figure {}.** {}\n\n",
                asset["number"],
                asset["number"],
                cell(asset["caption"].as_str().unwrap())
            ));
            if let Some(svg) = asset["svg"].as_str() {
                index.push_str(&format!("[Download vector SVG]({svg})\n\n"));
            }
        }
        if let Some(csv) = asset["csv"].as_str() {
            index.push_str(&format!("[Download curve data CSV]({csv})\n\n"));
        }
    }
    if !errors.is_empty() {
        index.push_str("## Export notices\n\n");
        for error in errors {
            index.push_str(&format!("- {}\n", cell(error)));
        }
    }
    fs::write(destination.join("README.md"), index).map_err(|e| e.to_string())?;
    Ok(destination.to_path_buf())
}
const REFERENCES: &str = "# References to review\n\n- rexafs: record the software version in state.json and the source revision used for the analysis.\n- M. Newville, *Larch: An Analysis Package for XAFS and Related Spectroscopies*, Journal of Physics: Conference Series 430, 012007 (2013). https://doi.org/10.1088/1742-6596/430/1/012007 — algorithm/reference implementation background; this export was generated by rexafs, not Larch.\n- M. Newville, P. Līviņš, Y. Yacoby, J. J. Rehr and E. A. Stern, *Near-edge x-ray-absorption fine structure of Pb: A comparison of theory and experiment*, Phys. Rev. B 47, 14126–14131 (1993). https://doi.org/10.1103/PhysRevB.47.14126 — AUTOBK background-removal reference; algorithm notes: https://xraypy.github.io/xraylarch/xafs_autobk.html\n- J. J. Rehr and R. C. Albers, *Theoretical Approaches to X-ray Absorption Fine Structure*, Rev. Mod. Phys. 72, 621 (2000). https://doi.org/10.1103/RevModPhys.72.621 — scattering theory.\n- Select the citation matching the FEFF backend/version recorded in the calculation inputs: https://feff.phys.washington.edu/feffproject-references.html. Imported path files may not identify their generator; verify provenance.\n- Add citations for the actual crystal structures, spectral reference data, beamline and sample sources used.\n";
const BIBTEX: &str = "@article{Newville1993, author={M. Newville and P. Livins and Y. Yacoby and J. J. Rehr and E. A. Stern}, title={Near-edge x-ray-absorption fine structure of {Pb}: A comparison of theory and experiment}, journal={Physical Review B}, volume={47}, pages={14126--14131}, year={1993}, doi={10.1103/PhysRevB.47.14126}}\n\n@article{Newville2013, author={Matthew Newville}, title={Larch: An Analysis Package for XAFS and Related Spectroscopies}, journal={Journal of Physics: Conference Series}, volume={430}, pages={012007}, year={2013}, doi={10.1088/1742-6596/430/1/012007}}\n\n@article{RehrAlbers2000, author={J. J. Rehr and R. C. Albers}, title={Theoretical Approaches to X-ray Absorption Fine Structure}, journal={Reviews of Modern Physics}, volume={72}, pages={621}, year={2000}, doi={10.1103/RevModPhys.72.621}}\n";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calculated_flat_groups_export_exact_arrays_and_provenance_headers() {
        let group = crate::params::DerivedSpectrum {
            label: "MCR component 1".into(),
            energy: vec![8950., 8951., 8952.],
            mu: vec![-0.01, 0.7, 1.01],
            quantity: crate::params::Quantity::FlattenedMu,
            operation: Some(crate::params::Operation {
                tool: "MCR-ALS".into(),
                parameters: json!({"space":"Flat","component_index":1,"seed":71}),
                inputs: vec![],
                applied_energy_shift_ev: 0.,
            }),
            ..Default::default()
        };
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("export");
        export(
            Snapshot {
                project: ProjectFile {
                    derived: vec![group.clone()],
                    lcf_series_analysis: Some(crate::project::LcfSeriesAnalysis {
                        config: rexafs::prelude::LcfConfig {
                            space: rexafs::prelude::AnalysisSpace::Flat,
                            ..Default::default()
                        },
                        inputs: [(
                            0,
                            crate::project::AnalysisInput {
                                corrections: Vec::new(),
                                group_id: None,
                                label: "sample, \"one\"".into(),
                                fingerprint: 7,
                            },
                        )]
                        .into(),
                        standards: vec![crate::project::AnalysisInput {
                            corrections: Vec::new(),
                            group_id: None,
                            label: "Cu foil".into(),
                            fingerprint: 9,
                        }],
                        rows: [(0, vec![1.0, 0.0])].into(),
                        errors: Default::default(),
                        complete: true,
                        cancelled: false,
                    }),
                    ..Default::default()
                },
                current: PathBuf::new(),
                spectra: vec![SpectrumInput {
                    group: Some(group.clone()),
                    label: group.label.clone(),
                    ..Default::default()
                }],
                results: BTreeMap::new(),
                analysis: Value::Null,
                batch_csv: None,
                batch_stale: false,
                journal: vec![],
                screen: Value::Null,
            },
            &output,
        )
        .unwrap();
        let csv = fs::read_to_string(output.join("data/lcf-series.csv")).unwrap();
        assert!(csv.starts_with("frame,label,\"Cu foil\",r_factor\n"));
        assert!(
            csv.contains("1,\"sample, \"\"one\"\"\",1.00000000000000000e0,0.00000000000000000e0")
        );
        let record: crate::project::LcfSeriesAnalysis =
            serde_json::from_slice(&fs::read(output.join("data/lcf-series.json")).unwrap())
                .unwrap();
        assert_eq!(record.config.space, rexafs::prelude::AnalysisSpace::Flat);
        assert_eq!(record.rows[&0], vec![1.0, 0.0]);
        let text = fs::read_to_string(output.join("data/spectrum-001-calculated.dat")).unwrap();
        let header = text
            .lines()
            .find_map(|line| line.strip_prefix("# Operation: "))
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(header).unwrap()["parameters"]["seed"],
            71
        );
        let rows: Vec<Vec<f64>> = text
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| {
                line.split_whitespace()
                    .map(|x| x.parse().unwrap())
                    .collect()
            })
            .collect();
        assert_eq!(
            rows,
            vec![vec![8950., -0.01], vec![8951., 0.7], vec![8952., 1.01]]
        );
        let saved: crate::params::DerivedSpectrum = serde_json::from_slice(
            &fs::read(output.join("data/spectrum-001-provenance.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(saved.mu, group.mu);
        assert_eq!(saved.quantity, crate::params::Quantity::FlattenedMu);
    }

    #[test]
    fn missing_channel_never_falls_back_to_primary_file() {
        let input = SpectrumInput {
            source_error: Some("Reference: source group is missing".into()),
            path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../rexafs/tests/testfiles/Ru_QAS.dat"),
            ..Default::default()
        };
        assert_eq!(
            input.process().err().as_deref(),
            Some("Reference: source group is missing")
        );
    }

    #[test]
    fn copper_export_has_resolved_settings_arrays_and_readable_pngs() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rexafs/tests/testfiles/xraylarch_d867/xafsdata/cu_150k.xmu");
        let folder =
            std::env::temp_dir().join(format!("xts-publication-qa-{}", std::process::id()));
        let mut project = ProjectFile {
            spectrum_file: Some(file.clone()),
            data_storage: crate::project::DataStorage::Embedded,
            ..Default::default()
        };
        project.publication.figures.insert(
            "chi-k".into(),
            figures::FigureOptions {
                caption: Some("Copper <reference> & EXAFS.".into()),
                width: Some(4.),
                height: Some(3.),
                dpi: Some(200.),
                ..Default::default()
            },
        );
        project.publication.table_captions.insert(
            "processing".into(),
            "Copper processing settings; Auto is resolved per spectrum.".into(),
        );
        let snapshot = Snapshot {
            project,
            current: file.clone(),
            spectra: vec![SpectrumInput {
                source_error: None,
                group: None,
                label: String::new(),
                path: file,
                params: PipelineParams::default(),
                data: None,
            }],
            results: BTreeMap::new(),
            analysis: Value::Null,
            batch_csv: None,
            batch_stale: false,
            journal: vec![],
            screen: Value::Null,
        };
        assert!(
            snapshot.context()["effective_processing"][0]["source_comments"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_str().unwrap().contains("foil"))
        );
        export(snapshot, &folder).unwrap();
        let manifest: Value =
            serde_json::from_slice(&fs::read(folder.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["figures"].as_array().unwrap().len(), 6);
        assert_eq!(manifest["project_storage"], "embedded");
        let restored = crate::project::tests::load(&folder.join("project.rxs")).unwrap();
        assert_eq!(restored.data_storage, crate::project::DataStorage::Embedded);
        assert_eq!(
            fs::read(restored.spectrum_file.unwrap()).unwrap(),
            fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../rexafs/tests/testfiles/xraylarch_d867/xafsdata/cu_150k.xmu")
            )
            .unwrap()
        );
        assert_eq!(manifest["tables"][0]["number"], 1);
        assert!(
            manifest["tables"][0]["caption"]
                .as_str()
                .unwrap()
                .contains("Copper processing settings")
        );
        let report = fs::read_to_string(folder.join("report.html")).unwrap();
        assert_eq!(report.matches("<figcaption>").count(), 6);
        assert_eq!(report.matches("<caption>").count(), 1);
        assert!(report.contains("Copper &lt;reference&gt; &amp; EXAFS."));
        assert!(report.contains("Absorption edge E₀"));
        assert!(
            report.contains("<td>8977.5</td>"),
            "table must contain resolved values, not only Auto requests"
        );
        assert!(!report.contains("Copper <reference>"));
        assert!(
            fs::read_to_string(folder.join("captions.md"))
                .unwrap()
                .contains("**Figure 3.**")
        );
        assert!(
            fs::read_to_string(folder.join("analysis.md"))
                .unwrap()
                .contains("**Table 1.**")
        );
        assert!(
            manifest["notices"].as_array().unwrap().is_empty(),
            "{manifest}"
        );
        let resolved = fs::read_to_string(folder.join("resolved.md")).unwrap();
        assert!(resolved.find("## ").unwrap() < resolved.find("```json").unwrap());
        let data: Value =
            serde_json::from_slice(&fs::read(folder.join("data/spectrum-001.json")).unwrap())
                .unwrap();
        assert!(data["energy"].is_array() || data["energy"].is_object());
        for asset in manifest["figures"].as_array().unwrap() {
            let csv = fs::read_to_string(folder.join(asset["csv"].as_str().unwrap())).unwrap();
            assert!(csv.lines().count() > 1);
            let bytes = fs::read(folder.join(asset["file"].as_str().unwrap())).unwrap();
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            let default = if asset["kind"] == "chi-k" {
                (800, 600)
            } else {
                ruviz::prelude::Plot::new()
                    .dpi(300)
                    .get_config()
                    .canvas_size()
            };
            assert_eq!(
                u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
                default.0
            );
            assert_eq!(
                u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
                default.1
            );
            let svg = fs::read_to_string(folder.join(asset["svg"].as_str().unwrap())).unwrap();
            assert!(svg.contains("<desc>"));
            assert!(!asset["caption"].as_str().unwrap().is_empty());
        }
        if crate::settings::env_var_os("KEEP_PUBLICATION_QA").is_some() {
            eprintln!("Publication QA assets: {}", folder.display());
        } else {
            fs::remove_dir_all(folder).unwrap();
        }
    }
    #[test]
    fn record_keeps_per_spectrum_settings_and_historical_model_separate() {
        let s = Snapshot {
            project: ProjectFile::default(),
            current: "Cu.xdi".into(),
            spectra: vec![
                SpectrumInput {
                    source_error: None,
                    group: None,
                    label: String::new(),
                    path: "Cu.xdi".into(),
                    params: PipelineParams {
                        fft_kweight: Some(1.),
                        ..Default::default()
                    },
                    data: None,
                },
                SpectrumInput {
                    source_error: None,
                    group: None,
                    label: String::new(),
                    path: "Ni.xdi".into(),
                    params: PipelineParams {
                        fft_kweight: Some(3.),
                        ..Default::default()
                    },
                    data: None,
                },
            ],
            results: BTreeMap::new(),
            analysis: Value::Null,
            batch_csv: None,
            batch_stale: false,
            journal: vec![],
            screen: Value::Null,
        };
        let c = s.context();
        assert_eq!(c["effective_processing"][0]["requested"]["fft_kweight"], 1.);
        assert_eq!(c["effective_processing"][1]["requested"]["fft_kweight"], 3.);
        assert!(s.markdown().contains("No completed fits recorded"));
    }
}
