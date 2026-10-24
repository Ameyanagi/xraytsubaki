//! One-click supporting-information (SI) bundle: a zip archive for journal
//! submission.
//!
//! The bundle reuses the analysis-folder export ([`super::export`]: report,
//! single figures as PNG/SVG/CSV, named compositions as PNG/SVG/PDF with
//! captions and matplotlib data, processed JSON arrays, `project.rxs`, captions
//! and methods) and adds:
//!
//! - `data/csv/`: plain CSV columns for each spectrum (energy, μ(E),
//!   normalized and flattened μ(E); k, χ(k), k^w·χ(k); R, |χ(R)|, Re and Im
//!   χ(R)) and, for each completed fit and dataset, measured and model χ(k)
//!   and |χ(R)|;
//! - `tables/`: the per-path fit parameter table of every recorded fit as
//!   LaTeX, CSV and Word (see [`super::fit_table`]);
//! - `manifest-sha256.txt`: the SHA-256 checksum of every other file in the
//!   archive, in the `sha256sum`/`shasum -a 256` format, so a reader can check
//!   the files with `shasum -a 256 -c manifest-sha256.txt`.
//!
//! The bundled `project.rxs` always embeds the original spectra and referenced
//! FEFF inputs (with checksums), whatever raw-data mode the open project uses,
//! so the archive opens on another computer without the source folders. The
//! export fails, rather than writing a project with dangling links, when a
//! source cannot be read or the inputs exceed the 1 GiB embedding limit.
//! Composer samples are captured from the current workspace independently of
//! project scope. Rendering does not add otherwise absent groups to the project;
//! static figures and matplotlib data retain their plotted curves.
//!
//! Everything is written into a temporary folder first; the zip is written
//! next to its destination and renamed into place only when complete.
use super::{Snapshot, fit_table};
use rexafs::prelude::{FeffFitResult, XASSpectrum};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Name of the checksum manifest at the bundle root.
pub(crate) const MANIFEST: &str = "manifest-sha256.txt";

/// Columns of possibly different lengths; shorter columns are padded with
/// blank cells. Values use 17 significant digits so every f64 round-trips.
fn columns_csv(columns: &[(&str, Vec<f64>)]) -> String {
    let mut out = columns
        .iter()
        .map(|(name, _)| crate::fitting::csv_field(name))
        .collect::<Vec<_>>()
        .join(",");
    out.push('\n');
    let rows = columns.iter().map(|(_, v)| v.len()).max().unwrap_or(0);
    for row in 0..rows {
        let cells: Vec<_> = columns
            .iter()
            .map(|(_, v)| v.get(row).map(|x| format!("{x:.16e}")).unwrap_or_default())
            .collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

fn vec(v: Option<nalgebra::DVector<f64>>) -> Vec<f64> {
    v.map(|v| v.as_slice().to_vec()).unwrap_or_default()
}

/// Energy-, k- and R-space CSV files of one processed spectrum. Arrays the
/// processing did not produce (for example χ(k) of a XANES-only scan) are
/// left out; a file is written only when its x column has data.
pub(crate) fn spectrum_csvs(sp: &XASSpectrum) -> Vec<(&'static str, String)> {
    let mut files = Vec::new();
    let energy = vec(sp.energy.clone());
    if !energy.is_empty() {
        files.push((
            "energy",
            columns_csv(&[
                ("energy (eV)", energy),
                ("mu(E)", vec(sp.mu.clone())),
                ("normalized mu(E)", vec(sp.norm())),
                ("flattened mu(E)", vec(sp.flat())),
            ]),
        ));
    }
    let k = sp.k().map(<[f64]>::to_vec).unwrap_or_default();
    if !k.is_empty() {
        let w = sp.kweight().copied().unwrap_or(2.);
        files.push((
            "chi-k",
            columns_csv(&[
                ("k (1/Å)", k),
                ("chi(k)", sp.chi().map(<[f64]>::to_vec).unwrap_or_default()),
                (&format!("k^{w} chi(k) (Å^-{w})"), vec(sp.chi_kweighted())),
            ]),
        ));
    }
    let r = vec(sp.r());
    if !r.is_empty() {
        files.push((
            "chi-r",
            columns_csv(&[
                ("R (Å)", r),
                ("|chi(R)|", vec(sp.chir_mag())),
                ("Re chi(R)", vec(sp.chir_real())),
                ("Im chi(R)", vec(sp.chir_imag())),
            ]),
        ));
    }
    files
}

/// Measured and model χ(k) and |χ(R)| of one fit dataset, unweighted and with
/// the fit's k-weight applied.
pub(crate) fn fit_csvs(view: &FeffFitResult) -> Vec<(&'static str, String)> {
    let w = view.kweight;
    let k: Vec<f64> = view.k.iter().copied().collect();
    let weighted = |chi: &nalgebra::DVector<f64>| -> Vec<f64> {
        chi.iter().zip(&k).map(|(c, k)| c * k.powf(w)).collect()
    };
    let magnitude: Vec<f64> = view
        .data_chir_re
        .iter()
        .zip(view.data_chir_im.iter())
        .map(|(re, im)| re.hypot(*im))
        .collect();
    vec![
        (
            "k",
            columns_csv(&[
                ("k (1/Å)", k.clone()),
                ("data chi(k)", view.data_chi.as_slice().to_vec()),
                ("model chi(k)", view.model_chi.as_slice().to_vec()),
                (&format!("data k^{w} chi(k)"), weighted(&view.data_chi)),
                (&format!("model k^{w} chi(k)"), weighted(&view.model_chi)),
                ("k window", view.kwin.as_slice().to_vec()),
            ]),
        ),
        (
            "r",
            columns_csv(&[
                ("R (Å)", view.r.as_slice().to_vec()),
                ("data |chi(R)|", magnitude),
                ("model |chi(R)|", view.model_chir_mag.as_slice().to_vec()),
            ]),
        ),
    ]
}

/// `<hex>  <relative path>` lines for every file under `root`, sorted by path.
pub(crate) fn sha256_manifest(root: &Path) -> Result<String, String> {
    let mut lines = Vec::new();
    for entry in walkdir::WalkDir::new(root).sort_by_file_name() {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        if relative == MANIFEST {
            continue;
        }
        let digest = Sha256::digest(fs::read(entry.path()).map_err(|e| e.to_string())?);
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        lines.push(format!("{hex}  {relative}\n"));
    }
    Ok(lines.concat())
}

fn zip_folder(root: &Path, prefix: &str, destination: &Path) -> Result<(), String> {
    let partial = destination.with_extension("zip.partial");
    let result = (|| {
        let file = fs::File::create(&partial).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        for entry in walkdir::WalkDir::new(root).sort_by_file_name() {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().is_file() {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            zip.start_file(format!("{prefix}/{relative}"), options)
                .map_err(|e| e.to_string())?;
            zip.write_all(&fs::read(entry.path()).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        }
        zip.finish()
            .map_err(|e| e.to_string())?
            .flush()
            .map_err(|e| e.to_string())?;
        fs::rename(&partial, destination).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}

const README: &str = "# Supporting information\n\nGenerated by rexafs. Contents:\n\n- `report.html`, `README.md`, `analysis.md`, `captions.md`, `methods.md`: the publication report and analysis record.\n- `figures/`: single figures as PNG, SVG and the CSV of their plotted curves. `figures/composed/` contains named compositions as PNG, SVG and PDF with combined captions and editable matplotlib scripts plus curve CSVs when available.\n- `data/csv/`: spectra and fit curves as CSV columns (17 significant digits); `data/*.json`: full processed objects.\n- `tables/`: per-path fit parameter tables (N, S₀², ΔE₀, R, σ²) as LaTeX (booktabs), CSV (unrounded and rounded values) and Word (.docx). The uncertainty is one standard error from the fit covariance, rounded to one significant figure (two if its leading digit is 1); the value is rounded to the same decimal place.\n- `project.rxs`: the rexafs project, self-contained: the original spectra and referenced FEFF inputs are embedded with SHA-256 checksums, so its saved source scope opens in rexafs on any computer without the source folders. Recipe-only workspace groups are not added to the project; static figures and matplotlib data retain their plotted curves. Recreating those recipes in rexafs requires their groups in the reopened workspace.\n- `manifest-sha256.txt`: SHA-256 checksums of every other file. Verify with `shasum -a 256 -c manifest-sha256.txt` (macOS) or `sha256sum -c manifest-sha256.txt` (Linux) in this folder.\n";

/// Write the supporting-information zip to `destination` (a `.zip` extension
/// is added when missing) and return its path. An existing file is replaced
/// only after the new archive is complete.
pub(crate) fn export_bundle(mut snapshot: Snapshot, destination: &Path) -> Result<PathBuf, String> {
    let mut destination = destination.to_path_buf();
    if destination
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("zip"))
    {
        let mut name = destination.file_name().unwrap_or_default().to_os_string();
        name.push(".zip");
        destination.set_file_name(name);
    }
    let prefix = destination
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "supporting-information".into());
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let root = temp.path().join("si");

    // A supporting-information archive must be self-contained.
    snapshot.project.data_storage = crate::project::DataStorage::Embedded;
    snapshot.resolve_spectra();
    let spectra: Vec<_> = snapshot.spectra.iter().map(|s| s.data.clone()).collect();
    let results = snapshot.results.clone();
    let fits = snapshot.project.fit_history.clone();
    super::export(snapshot, &root)?;

    let csv_dir = root.join("data").join("csv");
    fs::create_dir_all(&csv_dir).map_err(|e| e.to_string())?;
    for (i, data) in spectra.iter().enumerate() {
        let Some(sp) = data else { continue };
        for (kind, text) in spectrum_csvs(sp) {
            fs::write(
                csv_dir.join(format!("spectrum-{:03}-{kind}.csv", i + 1)),
                text,
            )
            .map_err(|e| e.to_string())?;
        }
    }
    for (id, result) in &results {
        for index in 0..result.datasets.len().max(1) {
            let view = crate::joint_fitting::result_view(result, index);
            if view.k.is_empty() {
                continue;
            }
            for (kind, text) in fit_csvs(&view) {
                fs::write(
                    csv_dir.join(format!("fit-{id}-spectrum-{}-{kind}.csv", index + 1)),
                    text,
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }
    let tables = root.join("tables");
    fs::create_dir_all(&tables).map_err(|e| e.to_string())?;
    for fit in fits.iter().filter(|f| !f.path_details.is_empty()) {
        fit_table::write_all(fit, &tables.join(format!("fit-{}-parameters", fit.id)))?;
    }
    fs::write(root.join("SUPPORTING-INFORMATION.md"), README).map_err(|e| e.to_string())?;
    fs::write(root.join(MANIFEST), sha256_manifest(&root)?).map_err(|e| e.to_string())?;
    zip_folder(&root, &prefix, &destination)?;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{params::PipelineParams, project::ProjectFile, publication::SpectrumInput};
    use serde_json::Value;
    use std::{collections::BTreeMap, io::Read};

    #[test]
    fn composed_bundle_checksums_cover_all_recipe_files_and_keep_project_scope() {
        let dir = tempfile::tempdir().unwrap();
        let path = export_bundle(
            super::super::composed::tests::snapshot(),
            &dir.path().join("recipes"),
        )
        .unwrap();
        let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        let mut read = |name: &str| {
            let mut bytes = Vec::new();
            archive
                .by_name(&format!("recipes/{name}"))
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            bytes
        };
        let manifest = String::from_utf8(read(MANIFEST)).unwrap();
        for line in manifest.lines() {
            let (hex, name) = line.split_once("  ").unwrap();
            let actual: String = Sha256::digest(read(name))
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(hex, actual, "{name}");
        }
        for stem in ["a-b", "a-b-2"] {
            for suffix in [
                ".png",
                ".svg",
                ".pdf",
                "-caption.txt",
                "-matplotlib/make_figure.py",
                "-matplotlib/a-series-01.csv",
            ] {
                let path = format!("figures/composed/{stem}{suffix}");
                assert!(
                    manifest
                        .lines()
                        .any(|line| line.ends_with(&format!("  {path}"))),
                    "{path}"
                );
            }
        }
        let report = String::from_utf8(read("report.html")).unwrap();
        assert!(report.contains("A-B. (a) Difference &lt;caption&gt; &amp; selected curves."));
        assert!(report.contains("Bad &lt;recipe&gt;: Use 1–4 columns"));
        let rxs = dir.path().join("project.rxs");
        fs::write(&rxs, read("project.rxs")).unwrap();
        let project = crate::project::tests::load(&rxs).unwrap();
        assert_eq!(project.data_storage, crate::project::DataStorage::Embedded);
        assert_eq!(project.derived.len(), 1);
        assert_eq!(project.derived[0].id, 1);
        assert_eq!(project.publication.recipes().len(), 4);
    }

    #[test]
    fn copper_bundle_has_data_csv_project_and_verified_manifest() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rexafs/tests/testfiles/xraylarch_d867/xafsdata/cu_150k.xmu");
        let original = file.clone();
        let snapshot = Snapshot {
            project: ProjectFile {
                spectrum_file: Some(file.clone()),
                data_storage: crate::project::DataStorage::Paths,
                ..Default::default()
            },
            current: file.clone(),
            spectra: vec![SpectrumInput {
                path: file,
                params: PipelineParams::default(),
                ..Default::default()
            }],
            results: BTreeMap::new(),
            analysis: Value::Null,
            batch_csv: None,
            batch_stale: false,
            journal: vec![],
            composer: Default::default(),
            screen: Value::Null,
        };
        let dir = tempfile::tempdir().unwrap();
        let zip_path = export_bundle(snapshot, &dir.path().join("cu-si")).unwrap();
        assert_eq!(zip_path, dir.path().join("cu-si.zip"));
        let mut archive = zip::ZipArchive::new(fs::File::open(&zip_path).unwrap()).unwrap();
        let mut read = |name: &str| {
            let mut bytes = Vec::new();
            archive
                .by_name(&format!("cu-si/{name}"))
                .unwrap_or_else(|e| panic!("{name}: {e}"))
                .read_to_end(&mut bytes)
                .unwrap();
            bytes
        };
        let manifest = String::from_utf8(read(MANIFEST)).unwrap();
        for line in manifest.lines() {
            let (hex, name) = line.split_once("  ").unwrap();
            let digest = Sha256::digest(read(name));
            let actual: String = digest.iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(hex, actual, "{name}");
        }
        for name in [
            "project.rxs",
            "report.html",
            "data/csv/spectrum-001-energy.csv",
            "data/csv/spectrum-001-chi-k.csv",
            "data/csv/spectrum-001-chi-r.csv",
            "figures/spectrum-001-chi-r.png",
        ] {
            assert!(
                manifest.lines().any(|l| l.ends_with(&format!("  {name}"))),
                "{name} missing from manifest"
            );
        }
        let energy = String::from_utf8(read("data/csv/spectrum-001-energy.csv")).unwrap();
        assert!(energy.starts_with("energy (eV),mu(E),normalized mu(E),flattened mu(E)\n"));
        assert!(energy.lines().count() > 100);
        let chir = String::from_utf8(read("data/csv/spectrum-001-chi-r.csv")).unwrap();
        assert!(chir.starts_with("R (Å),|chi(R)|,Re chi(R),Im chi(R)\n"));

        // The project is embedded even though the open project uses paths, and
        // it opens from a folder that holds nothing but the extracted .rxs.
        let report: Value = serde_json::from_slice(&read("manifest.json")).unwrap();
        assert_eq!(report["project_storage"], "embedded");
        let isolated = tempfile::tempdir().unwrap();
        let rxs = isolated.path().join("project.rxs");
        fs::write(&rxs, read("project.rxs")).unwrap();
        let restored = crate::project::tests::load(&rxs).unwrap();
        assert_eq!(restored.data_storage, crate::project::DataStorage::Embedded);
        let spectrum = restored.spectrum_file.unwrap();
        assert!(
            !spectrum.starts_with(env!("CARGO_MANIFEST_DIR")),
            "{} still points at the original source",
            spectrum.display()
        );
        assert_eq!(fs::read(&spectrum).unwrap(), fs::read(&original).unwrap());
    }

    #[test]
    fn columns_pad_shorter_arrays() {
        assert_eq!(
            columns_csv(&[("x", vec![1., 2.]), ("y", vec![3.])]),
            "x,y\n1.0000000000000000e0,3.0000000000000000e0\n2.0000000000000000e0,\n"
        );
    }
}
