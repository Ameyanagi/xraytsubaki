//! Methods draft generated from the settings that processing and fitting used.
//!
//! The Analysis section is written from the resolved values of each exported
//! spectrum ([`used_settings`]: Auto values resolved, ranges limited to the
//! measured scan) and from each recorded fit: its transform, fit space,
//! independent-point count and the FEFF engine that produced its path files.
//! A step that was not performed gets no sentence: no fit, no fitting
//! paragraph; no background removal, no EXAFS sentences. Each method that is
//! named is cited, and only cited works are listed under References. The
//! Experiment section stays a placeholder because rexafs cannot infer sample
//! or beamline details from the data.
use super::*;
use crate::fitting::FitHistoryEntry;
use std::collections::BTreeSet;

/// A cited work: the inline author–year form and the full reference.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Citation {
    inline: &'static str,
    full: &'static str,
}

const AUTOBK: Citation = Citation {
    inline: "Newville et al., 1993",
    full: "M. Newville, P. Līviņš, Y. Yacoby, J. J. Rehr and E. A. Stern, *Near-edge x-ray-absorption fine structure of Pb: A comparison of theory and experiment*, Phys. Rev. B 47, 14126–14131 (1993). https://doi.org/10.1103/PhysRevB.47.14126",
};
const MBACK: Citation = Citation {
    inline: "Weng et al., 2005",
    full: "T.-C. Weng, G. S. Waldo and J. E. Penner-Hahn, *A method for normalization of X-ray absorption spectra*, J. Synchrotron Rad. 12, 506–510 (2005). https://doi.org/10.1107/S0909049504034193",
};
const FEFF10: Citation = Citation {
    inline: "Kas et al., 2021",
    full: "J. J. Kas, F. D. Vila, C. D. Pemmaraju, T. S. Tan and J. J. Rehr, *Advanced calculations of X-ray spectroscopies with FEFF10 and Corvus*, J. Synchrotron Rad. 28, 1801–1810 (2021). https://doi.org/10.1107/S1600577521008614",
};
const THEORY: Citation = Citation {
    inline: "Rehr and Albers, 2000",
    full: "J. J. Rehr and R. C. Albers, *Theoretical Approaches to X-ray Absorption Fine Structure*, Rev. Mod. Phys. 72, 621 (2000). https://doi.org/10.1103/RevModPhys.72.621",
};
const STERN: Citation = Citation {
    inline: "Stern, 1993",
    full: "E. A. Stern, *Number of relevant independent points in x-ray-absorption fine-structure spectra*, Phys. Rev. B 48, 9825–9827 (1993). https://doi.org/10.1103/PhysRevB.48.9825",
};

/// Resolved processing settings with the short-scan range limits applied,
/// i.e. the values processing used. The stored settings keep the requested
/// ranges; a short scan limits them only while processing
/// (`XASSpectrum::range_adjustments`).
pub(crate) fn used_settings(sp: &XASSpectrum) -> Value {
    let mut resolved = resolved_settings(sp);
    for adjustment in sp.range_adjustments() {
        for stage in ["/normalization/PrePostEdge/", "/background/AUTOBK/"] {
            if let Some(value) = resolved.pointer_mut(&format!("{stage}{}", adjustment.field)) {
                *value = Value::from(adjustment.used);
            }
        }
    }
    resolved
}

fn num(v: &Value, pointer: &str, unit: &str) -> Option<String> {
    v.pointer(pointer)
        .and_then(Value::as_f64)
        .map(|x| report::used_number(x, unit))
}

fn window(v: &Value, pointer: &str) -> Option<&'static str> {
    v.pointer(pointer)
        .cloned()
        .and_then(|w| serde_json::from_value(w).ok())
        .map(crate::fit_report::window_name)
}

/// Methods sentences for one processed spectrum, from [`used_settings`].
fn processing_text(v: &Value, cite: &mut BTreeSet<Citation>) -> String {
    let mut out = Vec::new();
    if let Some(e0) = num(v, "/e0", "eV") {
        out.push(format!("The edge energy E₀ was {e0} eV."));
    }
    let pre = "/normalization/PrePostEdge";
    if v.pointer(pre).is_some() {
        let range = |a: &str, b: &str| match (
            num(v, &format!("{pre}/{a}"), "eV"),
            num(v, &format!("{pre}/{b}"), "eV"),
        ) {
            (Some(a), Some(b)) => format!(" from {a} to {b} eV relative to E₀"),
            _ => String::new(),
        };
        let order = num(v, &format!("{pre}/norm_polyorder"), "—")
            .map(|n| format!(" of order {n}"))
            .unwrap_or_default();
        out.push(format!(
            "A pre-edge line fitted{} was subtracted, and the spectrum was normalized to the edge step with a post-edge polynomial{order} fitted{}.",
            range("pre_edge_start", "pre_edge_end"),
            range("norm_start", "norm_end"),
        ));
    } else if let Some(mback) = v.pointer("/normalization/MBack/options") {
        cite.insert(MBACK);
        let text = |key: &str| mback[key].as_str().unwrap_or("?").to_string();
        out.push(format!(
            "The spectrum was normalized with MBACK ({}), matching tabulated atomic cross sections for the {} {} edge with a background polynomial of degree {}.",
            MBACK.inline,
            text("element"),
            text("edge"),
            mback["degree"]
        ));
    }
    let bkg = "/background/AUTOBK";
    match v.pointer("/background") {
        Some(Value::Object(map)) if map.contains_key("AUTOBK") => {
            cite.insert(AUTOBK);
            let mut details = Vec::new();
            if let Some(r) = num(v, &format!("{bkg}/rbkg"), "Å") {
                details.push(format!("R_bkg = {r} Å"));
            }
            if let Some(k) = num(v, &format!("{bkg}/kweight"), "—") {
                details.push(format!("k-weight {k}"));
            }
            match (
                num(v, &format!("{bkg}/kmin"), "Å⁻¹"),
                num(v, &format!("{bkg}/kmax"), "Å⁻¹"),
            ) {
                (Some(a), Some(b)) => details.push(format!("k = {a}–{b} Å⁻¹")),
                (Some(a), None) => details.push(format!("k from {a} Å⁻¹ to the end of the data")),
                _ => {}
            }
            out.push(format!(
                "The smooth atomic background was removed with the AUTOBK spline method ({}){}, giving χ(k).",
                AUTOBK.inline,
                if details.is_empty() {
                    String::new()
                } else {
                    format!(" using {}", details.join(", "))
                }
            ));
            let ft = "/xftf";
            if let (Some(a), Some(b)) = (
                num(v, &format!("{ft}/kmin"), "Å⁻¹"),
                num(v, &format!("{ft}/kmax"), "Å⁻¹"),
            ) {
                let weight = num(v, &format!("{ft}/kweight"), "—")
                    .map(|w| format!(" (k-weight {w})"))
                    .unwrap_or_default();
                let win = window(v, &format!("{ft}/window"))
                    .map(|w| {
                        let dk = num(v, &format!("{ft}/dk"), "Å⁻¹")
                            .map(|d| format!(", dk = {d} Å⁻¹"))
                            .unwrap_or_default();
                        format!(" with a {w} window{dk}")
                    })
                    .unwrap_or_default();
                out.push(format!(
                    "The k-weighted χ(k){weight} was Fourier transformed over k = {a}–{b} Å⁻¹{win}; χ(R) is not phase corrected."
                ));
            }
        }
        Some(Value::Object(map)) => {
            if let Some(name) = map.keys().next() {
                out.push(format!("Background removal used the {name} method."));
            }
        }
        // `None`: background removal was skipped; no EXAFS was extracted.
        _ => {}
    }
    out.join(" ")
}

/// The FEFF engine that produced one path file: the `engine.txt` marker that
/// rexafs writes into each calculation workspace, else the generator named at
/// the end of the path file's first (title) line, e.g. "Feff XX 6.10".
fn path_engine(file: &Path) -> String {
    let dir = file.parent().unwrap_or(Path::new(""));
    if let Ok(engine) = fs::read_to_string(dir.join("engine.txt")) {
        let engine = engine.trim();
        if !engine.is_empty() {
            return engine.to_string();
        }
    }
    use std::io::{BufRead, BufReader, Read};
    let header = fs::File::open(file)
        .ok()
        .and_then(|f| BufReader::new(f.take(4096)).lines().next()?.ok());
    header
        .and_then(|line| {
            let lower = line.to_lowercase();
            lower.rfind("feff").map(|i| line[i..].trim().to_string())
        })
        .map(|tag| format!("imported path files (header: {tag})"))
        .unwrap_or_else(|| "imported path files of unrecorded origin".into())
}

fn engine_text(engine: &str, cite: &mut BTreeSet<Citation>) -> String {
    let version = env!("CARGO_PKG_VERSION");
    match engine {
        "ReFEFF" => {
            cite.insert(FEFF10);
            format!(
                "ReFEFF, the Rust port of FEFF10 bundled with rexafs {version} ({}; https://github.com/Ameyanagi/refeff)",
                FEFF10.inline
            )
        }
        "FEFF-RS / FEFF10" => {
            cite.insert(FEFF10);
            format!(
                "FEFF10 through the FEFF-RS wrapper bundled with rexafs {version} ({})",
                FEFF10.inline
            )
        }
        other => format!("{other}; cite the FEFF version that produced them"),
    }
}

fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// The fitting paragraph for one recorded fit.
fn fit_text(fit: &FitHistoryEntry, cite: &mut BTreeSet<Citation>) -> String {
    cite.insert(THEORY);
    cite.insert(STERN);
    let space = match fit.ranges.fitspace {
        crate::fitting::FitSpaceSpec::R => "R space (real and imaginary parts of χ(R))",
        crate::fitting::FitSpaceSpec::K => "k space",
        crate::fitting::FitSpaceSpec::Q => "q space (back-transformed χ(q))",
    };
    let (subject, files): (String, Vec<PathBuf>) = match &fit.joint {
        Some(joint) => (
            format!(
                "{} spectra ({}) were fitted simultaneously with shared and per-spectrum parameters",
                joint.datasets.len(),
                list(
                    &joint
                        .datasets
                        .iter()
                        .map(|d| d.label.clone())
                        .collect::<Vec<_>>()
                )
            ),
            joint
                .datasets
                .iter()
                .flat_map(|d| d.paths.iter().cloned())
                .collect(),
        ),
        None => (
            format!("The EXAFS of {} was fitted", fit.group),
            fit.paths
                .iter()
                .filter(|p| p.enabled)
                .map(|p| p.file.clone())
                .collect(),
        ),
    };
    let (transforms, reconstructed) = crate::fit_report::fit_transforms(fit);
    let describe = |t: &rexafs::xafs::fitting::FeffFitTransform| {
        format!(
            "k = {:.2}–{:.2} Å⁻¹ and R = {:.2}–{:.2} Å with k-weight(s) {} and a {}",
            t.kmin,
            t.kmax,
            t.rmin,
            t.rmax,
            crate::fit_report::kweights_text(t),
            crate::fit_report::window_text(t)
        )
    };
    let all_same = transforms.windows(2).all(|w| w[0] == w[1]);
    let ranges = if all_same || fit.joint.is_none() {
        transforms.first().map(describe).unwrap_or_default()
    } else {
        let labels: Vec<String> = fit
            .joint
            .iter()
            .flat_map(|j| j.datasets.iter().map(|d| d.label.clone()))
            .collect();
        transforms
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let label = labels
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("dataset {}", i + 1));
                format!("{} for {label}", describe(t))
            })
            .collect::<Vec<_>>()
            .join("; ")
    };
    let engines: BTreeSet<String> = files.iter().map(|f| path_engine(f)).collect();
    let engines: Vec<String> = engines.iter().map(|e| engine_text(e, cite)).collect();
    let mut text = format!(
        "{subject} in {space} over {ranges}{}. Theoretical scattering amplitudes and phase shifts ({}) ",
        if reconstructed {
            " (window not recorded; the release default at fit time)"
        } else {
            ""
        },
        THEORY.inline
    );
    if engines.is_empty() {
        text.push_str("were not recorded with this fit.");
    } else {
        text.push_str(&format!(
            "were calculated for {} scattering path{} with {}.",
            files.len(),
            if files.len() == 1 { "" } else { "s" },
            list(&engines)
        ));
    }
    text.push_str(&format!(
        " The fit varied {} parameters against N_idp = {:.1} independent points, estimated in rexafs as 1 + 2ΔkΔR/π (cf. {}).",
        fit.n_vary, fit.n_idp, STERN.inline
    ));
    if fit.noise_estimated() {
        text.push_str(
            " Residuals were weighted by the measurement noise estimated from high-R χ(R).",
        );
    } else {
        text.push_str(" Measurement noise was not estimated, so the residual uses unit weights and reduced χ² is not reported.");
    }
    text
}

/// methods.md: the generated Analysis paragraphs, a placeholder Experiment
/// section and the references cited in the text.
pub(crate) fn markdown(snapshot: &Snapshot) -> String {
    let mut cite = BTreeSet::new();
    let mut text = String::from(
        "# Experimental and analysis methods — draft\n\n## Experiment\n\nSample composition, preparation, beamline, detector geometry, temperature and acquisition details: **supply from the experimental record**. They are not inferred from curve shape. Check file metadata before completing this section.\n\n## Analysis\n\n",
    );
    text.push_str(&format!(
        "XAS spectra were processed with rexafs {} (https://github.com/Ameyanagi/rexafs). The values below are those processing applied, with automatic choices resolved for each spectrum.\n\n",
        env!("CARGO_PKG_VERSION")
    ));
    // Spectra processed identically share one paragraph.
    let mut paragraphs: Vec<(String, Vec<String>)> = Vec::new();
    for spectrum in &snapshot.spectra {
        let body = match &spectrum.data {
            Some(sp) => processing_text(&used_settings(sp), &mut cite),
            None => {
                "This spectrum could not be processed; its requested settings are in analysis.md."
                    .into()
            }
        };
        match paragraphs.iter_mut().find(|(b, _)| *b == body) {
            Some((_, labels)) => labels.push(spectrum.label()),
            None => paragraphs.push((body, vec![spectrum.label()])),
        }
    }
    for (body, labels) in &paragraphs {
        if paragraphs.len() > 1 || snapshot.spectra.len() > 1 {
            text.push_str(&format!("**{}.** ", list(labels)));
        }
        text.push_str(body);
        text.push_str("\n\n");
    }
    for fit in &snapshot.project.fit_history {
        if snapshot.project.fit_history.len() > 1 {
            text.push_str(&format!("**Fit {}.** ", fit.id));
        }
        text.push_str(&fit_text(fit, &mut cite));
        text.push_str("\n\n");
    }
    if !snapshot.project.fit_history.is_empty() {
        text.push_str("Distances for multiple-scattering paths denote half the total path length. Reported uncertainties are one standard error from the fit covariance; they do not include all experimental or model uncertainty. Statistics for each fit are in report.html and analysis.md.\n\n");
    }
    text.push_str("Per-spectrum requested and used settings are in analysis.md and resolved.md.\n\n## References\n\n");
    for citation in &cite {
        text.push_str(&format!("- {}\n", citation.full));
    }
    text.push_str("- Add citations for the crystal structures, reference spectra, beamline and samples used.\n");
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fitting::{FitPathSpec, FitRanges};

    fn spectrum(params: PipelineParams) -> SpectrumInput {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/projects/data/cu_150k.xmu");
        SpectrumInput {
            path,
            params,
            ..Default::default()
        }
    }

    fn snapshot(spectra: Vec<SpectrumInput>, fits: Vec<FitHistoryEntry>) -> Snapshot {
        let mut snapshot = Snapshot {
            project: ProjectFile {
                fit_history: fits,
                ..Default::default()
            },
            current: PathBuf::new(),
            spectra,
            results: BTreeMap::new(),
            analysis: Value::Null,
            batch_csv: None,
            batch_stale: false,
            journal: vec![],
            composer: Default::default(),
            screen: Value::Null,
        };
        snapshot.resolve_spectra();
        snapshot
    }

    /// A one-path fit whose path file sits in a workspace with an engine marker.
    fn fit(dir: &Path, engine: &str) -> FitHistoryEntry {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("engine.txt"), engine).unwrap();
        let file = dir.join("feff0001.dat");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/projects/feff/feff0001.dat"),
            &file,
        )
        .unwrap();
        let mut path = FitPathSpec::standard(file, 1);
        path.enabled = true;
        let result = rexafs::prelude::FeffFitResult {
            n_idp: 11.2,
            n_vary: 4,
            ..Default::default()
        };
        let ranges = FitRanges {
            kmin: 3.0,
            kmax: 14.0,
            rmin: 1.4,
            rmax: 3.0,
            ..Default::default()
        };
        FitHistoryEntry::from_result(1, "cu_150k.xmu".into(), vec![path], vec![], ranges, &result)
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rexafs-methods-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn autobk_processing_without_fit_has_no_fitting_sentence() {
        let s = snapshot(
            vec![spectrum(PipelineParams {
                rbkg: Some(1.1),
                norm_polyorder: Some(1),
                ..Default::default()
            })],
            vec![],
        );
        let md = markdown(&s);
        assert!(md.contains(&format!("rexafs {}", env!("CARGO_PKG_VERSION"))));
        assert!(
            md.contains("AUTOBK spline method (Newville et al., 1993) using R_bkg = 1.10 Å"),
            "{md}"
        );
        assert!(
            md.contains("post-edge polynomial of order 1 fitted from"),
            "{md}"
        );
        assert!(md.contains("Fourier transformed over k = "), "{md}");
        assert!(md.contains("Kaiser–Bessel window"), "{md}");
        assert!(md.contains("PhysRevB.47.14126"));
        // No fit: no fitting sentence, FEFF or N_idp citation.
        assert!(!md.contains("was fitted"), "{md}");
        assert!(!md.contains("FEFF10"), "{md}");
        assert!(!md.contains("PhysRevB.48.9825"), "{md}");
        // A single spectrum is not labelled.
        assert!(!md.contains("**cu_150k.xmu.**"), "{md}");
    }

    #[test]
    fn other_or_no_background_is_not_called_autobk() {
        let mut v = json!({
            "e0": 8979.0,
            "normalization": {"MBack": {"options": {"element": "Cu", "edge": "K", "degree": 2}}},
            "background": {"ILPBkg": {}},
            "xftf": {"kmin": 2.0, "kmax": 12.0}
        });
        let mut cite = BTreeSet::new();
        let text = processing_text(&v, &mut cite);
        assert!(text.contains("MBACK (Weng et al., 2005)"), "{text}");
        assert!(text.contains("Cu K edge"), "{text}");
        assert!(
            text.contains("Background removal used the ILPBkg method."),
            "{text}"
        );
        assert!(!text.contains("AUTOBK"), "{text}");
        assert!(!text.contains("Fourier"), "{text}");
        assert!(cite.contains(&MBACK) && !cite.contains(&AUTOBK));
        v["background"] = Value::String("None".into());
        let text = processing_text(&v, &mut BTreeSet::new());
        assert!(
            !text.contains("Background removal") && !text.contains("AUTOBK"),
            "{text}"
        );
        assert!(!text.contains("χ(k)"), "{text}");
    }

    #[test]
    fn single_fit_describes_ranges_engine_and_statistics_basis() {
        let dir = scratch("single");
        let s = snapshot(
            vec![spectrum(PipelineParams::default())],
            vec![fit(&dir, "ReFEFF")],
        );
        let md = markdown(&s);
        assert!(
            md.contains("The EXAFS of cu_150k.xmu was fitted in R space"),
            "{md}"
        );
        assert!(
            md.contains("k = 3.00–14.00 Å⁻¹ and R = 1.40–3.00 Å"),
            "{md}"
        );
        assert!(md.contains("k-weight(s) 2"), "{md}");
        assert!(md.contains("Kaiser–Bessel k window"), "{md}");
        assert!(
            md.contains("1 scattering path with ReFEFF, the Rust port of FEFF10"),
            "{md}"
        );
        assert!(
            md.contains("varied 4 parameters against N_idp = 11.2"),
            "{md}"
        );
        assert!(md.contains("reduced χ² is not reported"), "{md}");
        for doi in ["S1600577521008614", "RevModPhys.72.621", "PhysRevB.48.9825"] {
            assert!(md.contains(doi), "{doi}: {md}");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn joint_fit_names_datasets_and_their_ranges() {
        let dir = scratch("joint");
        let mut entry = fit(&dir.join("a"), "FEFF-RS / FEFF10");
        let path = entry.paths[0].file.clone();
        let datasets = ["Cu foil 150 K", "Cu foil 300 K"]
            .iter()
            .enumerate()
            .map(|(id, label)| crate::joint_fitting::JointDataset {
                id,
                label: label.to_string(),
                paths: vec![path.clone()],
                ..Default::default()
            })
            .collect();
        entry.joint = Some(crate::joint_fitting::JointConfig {
            enabled: true,
            datasets,
            ..Default::default()
        });
        let mut wide = entry.ranges.transform();
        wide.kmax = 15.0;
        entry.transforms = vec![entry.ranges.transform(), wide];
        let s = snapshot(vec![], vec![entry]);
        let md = markdown(&s);
        assert!(
            md.contains("2 spectra (Cu foil 150 K and Cu foil 300 K) were fitted simultaneously"),
            "{md}"
        );
        assert!(
            md.contains("k = 3.00–14.00 Å⁻¹") && md.contains("for Cu foil 150 K"),
            "{md}"
        );
        assert!(
            md.contains("k = 3.00–15.00 Å⁻¹") && md.contains("for Cu foil 300 K"),
            "{md}"
        );
        assert!(md.contains("FEFF10 through the FEFF-RS wrapper"), "{md}");
        let _ = fs::remove_dir_all(dir);
    }
}
