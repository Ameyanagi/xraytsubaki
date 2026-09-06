use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use nalgebra::DVector;
use rexafs::prelude::*;
use rexafs::xafs::fitting::transform::apply_r_transform;

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("testfiles")
        .join(name)
}

fn output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../doc/plots/feff_vs_larch_data")
}

fn load_two_column(path: &Path) -> Result<(DVector<f64>, DVector<f64>), Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    let mut x = Vec::new();
    let mut y = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let xv: f64 = parts.next().ok_or("missing x value in fixture")?.parse()?;
        let yv: f64 = parts.next().ok_or("missing y value in fixture")?.parse()?;
        x.push(xv);
        y.push(yv);
    }
    Ok((DVector::from_vec(x), DVector::from_vec(y)))
}

fn write_comparison_csv(
    path: &Path,
    k: &DVector<f64>,
    model: &DVector<f64>,
    larch: &DVector<f64>,
) -> Result<(), Box<dyn Error>> {
    if k.len() != model.len() || k.len() != larch.len() {
        return Err("length mismatch while writing comparison csv".into());
    }
    let mut out = String::from("k,model,larch,diff\n");
    for i in 0..k.len() {
        let diff = model[i] - larch[i];
        out.push_str(&format!(
            "{:.15e},{:.15e},{:.15e},{:.15e}\n",
            k[i], model[i], larch[i], diff
        ));
    }
    fs::write(path, out)?;
    Ok(())
}

fn reference_transform() -> FeffFitTransform {
    FeffFitTransform {
        kmin: 2.0,
        kmax: 14.0,
        rmin: 1.0,
        rmax: 3.0,
        window: FTWindow::Hanning,
        ..FeffFitTransform::default()
    }
}

fn write_rspace_comparison_csv(
    path: &Path,
    k: &DVector<f64>,
    model_chi: &DVector<f64>,
    larch_chi: &DVector<f64>,
) -> Result<(), Box<dyn Error>> {
    let transform = reference_transform();
    let model = apply_r_transform(k, model_chi, &transform)?;
    let larch = apply_r_transform(k, larch_chi, &transform)?;
    if model.r.len() != larch.r.len() || model.chir_mag.len() != larch.chir_mag.len() {
        return Err("R-space transform output mismatch".into());
    }
    let mut out = String::from("r,model,larch,diff\n");
    for i in 0..model.r.len() {
        let x = model.r[i];
        let mv = model.chir_mag[i];
        let lv = larch.chir_mag[i];
        let diff = mv - lv;
        out.push_str(&format!(
            "{:.15e},{:.15e},{:.15e},{:.15e}\n",
            x, mv, lv, diff
        ));
    }
    fs::write(path, out)?;
    Ok(())
}

fn sanitize_label(label: &str) -> String {
    let mut out = String::new();
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == '_' || ch == '-' {
            out.push('_');
        }
    }
    if out.is_empty() {
        "path".to_string()
    } else {
        out
    }
}

fn write_rspace_contrib_csv(
    path: &Path,
    k: &DVector<f64>,
    model_chi: &DVector<f64>,
    larch_chi: &DVector<f64>,
    contributions: &[(String, DVector<f64>)],
) -> Result<(), Box<dyn Error>> {
    let transform = reference_transform();
    let model = apply_r_transform(k, model_chi, &transform)?;
    let larch = apply_r_transform(k, larch_chi, &transform)?;
    if model.r.len() != larch.r.len() || model.chir_mag.len() != larch.chir_mag.len() {
        return Err("R-space transform output mismatch".into());
    }

    let mut contrib_r = Vec::with_capacity(contributions.len());
    for (label, chi) in contributions {
        let out = apply_r_transform(k, chi, &transform)?;
        if out.chir_mag.len() != model.r.len() {
            return Err("R-space contribution length mismatch".into());
        }
        contrib_r.push((label.clone(), out.chir_mag));
    }

    let mut labels = Vec::with_capacity(contrib_r.len());
    for (idx, (label, _)) in contrib_r.iter().enumerate() {
        let clean = sanitize_label(label);
        labels.push(format!("{clean}_{idx:02}"));
    }

    let mut out = String::from("r,total_model,total_larch,diff");
    for label in &labels {
        out.push_str(&format!(",{label}"));
    }
    out.push('\n');

    for i in 0..model.r.len() {
        out.push_str(&format!(
            "{:.15e},{:.15e},{:.15e},{:.15e}",
            model.r[i],
            model.chir_mag[i],
            larch.chir_mag[i],
            model.chir_mag[i] - larch.chir_mag[i]
        ));
        for (_, chir_mag) in &contrib_r {
            out.push_str(&format!(",{:.15e}", chir_mag[i]));
        }
        out.push('\n');
    }

    fs::write(path, out)?;
    Ok(())
}

fn larch_truth_variables() -> FitVariables {
    let mut vars = FitVariables::new();
    vars.insert("amp", FitVariable::new(0.92, false));
    vars.insert("de0", FitVariable::new(1.4, false));
    vars.insert("sig2", FitVariable::new(0.0031, false));
    vars.insert("dr", FitVariable::new(0.011, false));
    vars.insert("amp2", FitVariable::new(0.35, false));
    vars.insert("dr2", FitVariable::new(0.0025, false));
    vars
}

fn export_path_builder() -> Result<(), Box<dyn Error>> {
    let base_path = feffpath(
        fixture_path("feffcu01.dat")
            .to_str()
            .ok_or("invalid path")?,
        FeffFlavor::Feff85L,
    )?
    .set_s02("amp")
    .set_e0("de0")
    .set_deltar("dr");
    let path = base_path.set_sigma2("sig2");

    let (k, larch) = load_two_column(&fixture_path("feff_path_chi_larch_ref.txt"))?;
    let model = path2chi(&path, &larch_truth_variables(), &k)?;
    write_comparison_csv(
        &output_dir().join("01_path_builder.csv"),
        &k,
        &model,
        &larch,
    )?;
    write_rspace_comparison_csv(
        &output_dir().join("01_path_builder_rspace.csv"),
        &k,
        &model,
        &larch,
    )?;
    write_rspace_contrib_csv(
        &output_dir().join("01_path_builder_rspace_contrib.csv"),
        &k,
        &model,
        &larch,
        &[(path.label.clone(), model.clone())],
    )
}

fn export_multi_path_model() -> Result<(), Box<dyn Error>> {
    let path1 = feffpath(
        fixture_path("feffcu01.dat")
            .to_str()
            .ok_or("invalid path")?,
        FeffFlavor::Feff85L,
    )?
    .set_s02("amp")
    .set_e0("de0")
    .set_sigma2("sig2")
    .set_deltar("dr");
    let path2 = feffpath(
        fixture_path("feff0002.dat")
            .to_str()
            .ok_or("invalid path")?,
        FeffFlavor::Feff85L,
    )?
    .set_s02("amp2")
    .set_e0("de0")
    .set_sigma2("sig2")
    .set_deltar("dr2");

    let (k, larch) = load_two_column(&fixture_path("feff_ff2chi_larch_ref.txt"))?;
    let out = ff2chi(&[path1, path2], &larch_truth_variables(), &k)?;
    let model = out.chi.clone();
    write_comparison_csv(
        &output_dir().join("02_multi_path_model.csv"),
        &k,
        &model,
        &larch,
    )?;
    write_rspace_comparison_csv(
        &output_dir().join("02_multi_path_model_rspace.csv"),
        &k,
        &model,
        &larch,
    )?;
    write_rspace_contrib_csv(
        &output_dir().join("02_multi_path_model_rspace_contrib.csv"),
        &k,
        &model,
        &larch,
        &out.path_chi,
    )
}

fn export_single_dataset_fit() -> Result<(), Box<dyn Error>> {
    let path = feffpath(
        fixture_path("feffcu01.dat")
            .to_str()
            .ok_or("invalid path")?,
        FeffFlavor::Feff85L,
    )?
    .set_s02("amp")
    .set_e0("de0")
    .set_sigma2("sig2")
    .set_deltar("dr");
    let (k, larch) = load_two_column(&fixture_path("feff_fit_target_larch.txt"))?;

    let result = FeffFit::new()
        .data(&k, &larch)
        .add_path(path)
        .set_inits([("amp", 0.95), ("de0", 0.0), ("sig2", 0.002), ("dr", 0.0)])
        .set_bounds("sig2", 0.0, 0.02)
        .krange(2.0, 14.0)
        .rrange(1.0, 3.0)
        .fit()?;
    write_comparison_csv(
        &output_dir().join("03_single_dataset_fit.csv"),
        &k,
        &result.model_chi,
        &larch,
    )?;
    write_rspace_comparison_csv(
        &output_dir().join("03_single_dataset_fit_rspace.csv"),
        &k,
        &result.model_chi,
        &larch,
    )?;
    write_rspace_contrib_csv(
        &output_dir().join("03_single_dataset_fit_rspace_contrib.csv"),
        &k,
        &result.model_chi,
        &larch,
        &result
            .path_contributions
            .iter()
            .map(|p| (p.label.clone(), p.chi.clone()))
            .collect::<Vec<_>>(),
    )
}

fn export_clone_template() -> Result<(), Box<dyn Error>> {
    let path1 = feffpath(
        fixture_path("feffcu01.dat")
            .to_str()
            .ok_or("invalid path")?,
        FeffFlavor::Feff85L,
    )?
    .set_s02("amp")
    .set_e0("de0")
    .set_sigma2("sig2")
    .set_deltar("dr");
    let path2 = feffpath(
        fixture_path("feff0002.dat")
            .to_str()
            .ok_or("invalid path")?,
        FeffFlavor::Feff85L,
    )?
    .set_s02("amp2")
    .set_e0("de0")
    .set_sigma2("sig2")
    .set_deltar("dr2");

    let base = FeffFit::new()
        .params([
            Param::new("amp", 0.95),
            Param::new("de0", 0.0),
            Param::new("sig2", 0.002).bounds(0.0, 0.02),
            Param::new("dr", 0.0),
            Param::new("amp2", 0.2),
            Param::new("dr2", 0.0),
        ])
        .krange(2.0, 14.0)
        .rrange(1.0, 3.0);

    let (k1, larch1) = load_two_column(&fixture_path("feff_path_chi_larch_ref.txt"))?;
    let (k2, larch2) = load_two_column(&fixture_path("feff_ff2chi_larch_ref.txt"))?;

    let r1 = base
        .clone()
        .data(&k1, &larch1)
        .add_path(path1.clone())
        .fit()?;
    let r2 = base
        .clone()
        .data(&k2, &larch2)
        .add_path(path1)
        .add_path(path2)
        .fit()?;

    write_comparison_csv(
        &output_dir().join("04_clone_template_path.csv"),
        &k1,
        &r1.model_chi,
        &larch1,
    )?;
    write_rspace_comparison_csv(
        &output_dir().join("04_clone_template_path_rspace.csv"),
        &k1,
        &r1.model_chi,
        &larch1,
    )?;
    write_rspace_contrib_csv(
        &output_dir().join("04_clone_template_path_rspace_contrib.csv"),
        &k1,
        &r1.model_chi,
        &larch1,
        &r1.path_contributions
            .iter()
            .map(|p| (p.label.clone(), p.chi.clone()))
            .collect::<Vec<_>>(),
    )?;
    write_comparison_csv(
        &output_dir().join("05_clone_template_ff2chi.csv"),
        &k2,
        &r2.model_chi,
        &larch2,
    )?;
    write_rspace_comparison_csv(
        &output_dir().join("05_clone_template_ff2chi_rspace.csv"),
        &k2,
        &r2.model_chi,
        &larch2,
    )?;
    write_rspace_contrib_csv(
        &output_dir().join("05_clone_template_ff2chi_rspace_contrib.csv"),
        &k2,
        &r2.model_chi,
        &larch2,
        &r2.path_contributions
            .iter()
            .map(|p| (p.label.clone(), p.chi.clone()))
            .collect::<Vec<_>>(),
    )
}

fn export_multi_dataset() -> Result<(), Box<dyn Error>> {
    let ds1_path = feffpath(
        fixture_path("feffcu01.dat")
            .to_str()
            .ok_or("invalid path")?,
        FeffFlavor::Feff85L,
    )?
    .set_s02("amp")
    .set_e0("de0")
    .set_sigma2("sig2")
    .set_deltar("dr");
    let ds2_path1 = feffpath(
        fixture_path("feffcu01.dat")
            .to_str()
            .ok_or("invalid path")?,
        FeffFlavor::Feff85L,
    )?
    .set_s02("amp")
    .set_e0("de0")
    .set_sigma2("sig2")
    .set_deltar("dr");
    let ds2_path2 = feffpath(
        fixture_path("feff0002.dat")
            .to_str()
            .ok_or("invalid path")?,
        FeffFlavor::Feff85L,
    )?
    .set_s02("amp2")
    .set_e0("de0")
    .set_sigma2("sig2")
    .set_deltar("dr2");

    let (k1, larch1) = load_two_column(&fixture_path("feff_path_chi_larch_ref.txt"))?;
    let (k2, larch2) = load_two_column(&fixture_path("feff_ff2chi_larch_ref.txt"))?;

    let ds1 = FeffFitDataset::new()
        .data(&k1, &larch1)
        .add_path(ds1_path)
        .krange(2.0, 14.0)
        .rrange(1.0, 3.0);
    let ds2 = FeffFitDataset::new()
        .data(&k2, &larch2)
        .add_path(ds2_path1)
        .add_path(ds2_path2)
        .krange(2.0, 14.0)
        .rrange(1.0, 3.0);

    let result = FeffFit::new()
        .add_dataset(ds1)
        .add_dataset(ds2)
        .set_inits([
            ("amp", 0.95),
            ("de0", 0.0),
            ("sig2", 0.002),
            ("dr", 0.0),
            ("amp2", 0.2),
            ("dr2", 0.0),
        ])
        .set_bounds("sig2", 0.0, 0.02)
        .fit()?;

    write_comparison_csv(
        &output_dir().join("06_multi_dataset_ds1.csv"),
        &k1,
        &result.datasets[0].model_chi,
        &larch1,
    )?;
    write_rspace_comparison_csv(
        &output_dir().join("06_multi_dataset_ds1_rspace.csv"),
        &k1,
        &result.datasets[0].model_chi,
        &larch1,
    )?;
    write_rspace_contrib_csv(
        &output_dir().join("06_multi_dataset_ds1_rspace_contrib.csv"),
        &k1,
        &result.datasets[0].model_chi,
        &larch1,
        &result.datasets[0]
            .path_contributions
            .iter()
            .map(|p| (p.label.clone(), p.chi.clone()))
            .collect::<Vec<_>>(),
    )?;
    write_comparison_csv(
        &output_dir().join("07_multi_dataset_ds2.csv"),
        &k2,
        &result.datasets[1].model_chi,
        &larch2,
    )?;
    write_rspace_comparison_csv(
        &output_dir().join("07_multi_dataset_ds2_rspace.csv"),
        &k2,
        &result.datasets[1].model_chi,
        &larch2,
    )?;
    write_rspace_contrib_csv(
        &output_dir().join("07_multi_dataset_ds2_rspace_contrib.csv"),
        &k2,
        &result.datasets[1].model_chi,
        &larch2,
        &result.datasets[1]
            .path_contributions
            .iter()
            .map(|p| (p.label.clone(), p.chi.clone()))
            .collect::<Vec<_>>(),
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(output_dir())?;

    export_path_builder()?;
    export_multi_path_model()?;
    export_single_dataset_fit()?;
    export_clone_template()?;
    export_multi_dataset()?;

    println!(
        "Wrote comparison CSV files to {}",
        output_dir().canonicalize()?.display()
    );
    Ok(())
}
