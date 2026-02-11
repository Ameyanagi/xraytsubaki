#[cfg(not(feature = "plotting"))]
fn main() {
    eprintln!(
        "Enable plotting support: cargo run -p xraytsubaki --features plotting --example plot_demo"
    );
}

#[cfg(feature = "plotting")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::path::Path;

    use nalgebra::DVector;
    use xraytsubaki::prelude::*;
    use xraytsubaki::xafs::io::load_spectrum_QAS_trans;

    fn output_dir() -> String {
        format!("{}/target/plot_demo", env!("CARGO_MANIFEST_DIR"))
    }

    fn ensure_output_dir(path: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !Path::new(path).exists() {
            fs::create_dir_all(path)?;
        }
        Ok(())
    }

    fn chir_mag(re: &DVector<f64>, im: &DVector<f64>) -> DVector<f64> {
        let len = re.len().min(im.len());
        DVector::from_iterator(
            len,
            re.iter()
                .take(len)
                .zip(im.iter().take(len))
                .map(|(re, im)| (re * re + im * im).sqrt()),
        )
    }

    let input_path = format!("{}/tests/testfiles/Ru_QAS.dat", env!("CARGO_MANIFEST_DIR"));
    let mut spectrum = load_spectrum_QAS_trans(&input_path)?;

    let out = output_dir();
    ensure_output_dir(&out)?;

    // XASSpectrum examples
    spectrum
        .plot()
        .mu()
        .title("Single-spectrum flattened mu(E)")
        .save_png(format!("{out}/spectrum_mu.png"))?;

    spectrum
        .plot()
        .mu()
        .norm()
        .k()
        .r()
        .title("XAS overview")
        .save_png(format!("{out}/spectrum_overview.png"))?;

    // XASGroup examples
    let mut group = XASGroup::new();
    group.add_spectrum(spectrum.clone());

    let mut spectrum_scaled = spectrum.clone();
    if let Some(mu) = spectrum_scaled.mu.take() {
        spectrum_scaled.mu = Some(mu.map(|v| v * 1.03));
    }
    if let Some(raw_mu) = spectrum_scaled.raw_mu.take() {
        spectrum_scaled.raw_mu = Some(raw_mu.map(|v| v * 1.03));
    }
    spectrum_scaled.set_name("scaled-1.03");

    let mut spectrum_shifted = spectrum.clone();
    if let Some(mu) = spectrum_shifted.mu.take() {
        spectrum_shifted.mu = Some(mu.map(|v| v + 0.03));
    }
    if let Some(raw_mu) = spectrum_shifted.raw_mu.take() {
        spectrum_shifted.raw_mu = Some(raw_mu.map(|v| v + 0.03));
    }
    spectrum_shifted.set_name("shifted+0.03");

    group.add_spectrum(spectrum_scaled);
    group.add_spectrum(spectrum_shifted);

    group
        .plot()
        .mu()
        .title("Group overlay")
        .save_png(format!("{out}/group_overlay.png"))?;

    group
        .plot()
        .mu()
        .stacked(0.2)
        .title("Group stacked")
        .save_png(format!("{out}/group_stacked.png"))?;

    group
        .plot()
        .mu()
        .select(&[0, 2])
        .title("Group selected")
        .save_png(format!("{out}/group_selected.png"))?;

    // FeffFitResult examples (synthetic model from spectrum-derived vectors)
    spectrum.normalize()?.calc_background()?.fft()?;

    let k = spectrum.get_k().ok_or("missing k after background")?;
    let data_chi = spectrum.get_chi().ok_or("missing chi after background")?;
    let kwin = spectrum.get_kwin().ok_or("missing k-window after fft")?;
    let model_chi = data_chi.map(|value| value * 0.95);

    let r = spectrum.get_r().ok_or("missing r after fft")?;
    let data_chir_re = spectrum
        .get_chir_real()
        .ok_or("missing chi_r real after fft")?;
    let data_chir_im = spectrum
        .get_chir_imag()
        .ok_or("missing chi_r imag after fft")?;
    let model_chir_re = data_chir_re.map(|value| value * 0.95);
    let model_chir_im = data_chir_im.map(|value| value * 0.95);
    let model_chir_mag = chir_mag(&model_chir_re, &model_chir_im);

    let mut fit = FeffFitResult::default();
    fit.k = k.clone();
    fit.data_chi = data_chi.clone();
    fit.model_chi = model_chi.clone();
    fit.r = r.clone();
    fit.data_chir_re = data_chir_re.clone();
    fit.data_chir_im = data_chir_im.clone();
    fit.model_chir_re = model_chir_re.clone();
    fit.model_chir_im = model_chir_im.clone();
    fit.model_chir_mag = model_chir_mag.clone();
    fit.path_contributions.push(PathContribution {
        label: "path-1".to_string(),
        chi: model_chi.clone(),
        chir_re: model_chir_re.clone(),
        chir_im: model_chir_im.clone(),
        chir_mag: model_chir_mag.clone(),
    });

    let mut dataset = DatasetResult::default();
    dataset.k = k;
    dataset.data_chi = data_chi;
    dataset.model_chi = model_chi;
    dataset.kweight = 2.0;
    dataset.kwin = kwin;
    dataset.kmin = Some(2.0);
    dataset.kmax = Some(14.0);
    dataset.r = r;
    dataset.rmin = Some(1.0);
    dataset.rmax = Some(3.0);
    dataset.data_chir_re = data_chir_re;
    dataset.data_chir_im = data_chir_im;
    dataset.model_chir_re = model_chir_re;
    dataset.model_chir_im = model_chir_im;
    dataset.model_chir_mag = model_chir_mag;
    dataset.path_contributions = fit.path_contributions.clone();

    fit.datasets.push(dataset);
    fit.sync_primary_dataset_fields();

    fit.plot()
        .k()
        .dataset(0)
        .paths(true)
        .title("Fit: k-space")
        .save_png(format!("{out}/fit_k.png"))?;

    fit.plot()
        .k()
        .dataset(0)
        .window(true)
        .paths(true)
        .title("Fit: k-space + window")
        .save_png(format!("{out}/fit_k_window.png"))?;

    fit.plot()
        .r()
        .dataset(0)
        .title("Fit: r-space")
        .save_png(format!("{out}/fit_r.png"))?;

    fit.plot()
        .r()
        .dataset(0)
        .window_box(true)
        .title("Fit: r-space + window range")
        .save_png(format!("{out}/fit_r_window.png"))?;

    println!("saved demo plots to {out}");
    Ok(())
}
