use xraytsubaki::prelude::*;
use std::path::Path;
use ndarray::{Array1, Array2};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Define paths
    let test_file = "crates/xraytsubaki/tests/testfiles/Ru_QAS.dat";
    let output_dir = "examples/plot_output";
    
    // Create output directory if it doesn't exist
    std::fs::create_dir_all(output_dir)?;
    
    // Load spectrum
    println!("Loading spectrum from {}", test_file);
    let mut spectrum = io::load_spectrum_QAS_trans(test_file)?;
    spectrum.set_name("Ru QAS");
    
    // Plot raw data
    let raw_plot_path = format!("{}/01_raw_data.png", output_dir);
    println!("Plotting raw data to {}", raw_plot_path);
    spectrum.plot_raw(&raw_plot_path)?;
    
    // Find E0 and plot
    println!("Finding E0");
    spectrum.find_e0()?;
    println!("E0 = {:.2} eV", spectrum.e0.unwrap());
    
    let e0_plot_path = format!("{}/02_e0_marked.png", output_dir);
    println!("Plotting with E0 marked to {}", e0_plot_path);
    spectrum.get_plot_builder()
        .title("XAS with E0 Marked")
        .x_label("Energy (eV)")
        .y_label("Absorption")
        .dimensions(1000, 600)
        .save(&e0_plot_path)?;
    
    // Normalize and plot
    println!("Normalizing spectrum");
    spectrum.normalize()?;
    
    let norm_plot_path = format!("{}/03_normalized.png", output_dir);
    println!("Plotting normalized data to {}", norm_plot_path);
    spectrum.plot_normalized(&norm_plot_path)?;
    
    // Pre-edge fit plot
    let preedge_plot_path = format!("{}/04_preedge_fit.png", output_dir);
    println!("Plotting pre-edge fit to {}", preedge_plot_path);
    spectrum.plot_preedge_fit(&preedge_plot_path)?;
    
    // Calculate background and extract EXAFS
    println!("Calculating background and extracting EXAFS");
    spectrum.calc_background()?;
    
    // Plot k-space EXAFS with different k-weightings
    for k_weight in [0, 1, 2, 3].iter() {
        let k_plot_path = format!("{}/05_k_space_k{}.png", output_dir, k_weight);
        println!("Plotting k-space data (k-weight={}) to {}", k_weight, k_plot_path);
        spectrum.plot_k_space(&k_plot_path, Some(*k_weight as f64))?;
    }
    
    // Perform Fourier transform
    println!("Performing Fourier transform");
    spectrum.fft()?;
    
    // Plot R-space magnitude
    let r_mag_plot_path = format!("{}/06_r_space_magnitude.png", output_dir);
    println!("Plotting R-space magnitude to {}", r_mag_plot_path);
    spectrum.plot_r_mag(&r_mag_plot_path)?;
    
    // Plot R-space components
    let r_comp_plot_path = format!("{}/07_r_space_components.png", output_dir);
    println!("Plotting R-space components to {}", r_comp_plot_path);
    spectrum.plot_r_components(&r_comp_plot_path)?;
    
    // Export an SVG version
    let svg_path = format!("{}/08_vector_plot.svg", output_dir);
    println!("Creating SVG plot to {}", svg_path);
    spectrum.get_plot_builder()
        .title("XAS Spectrum (SVG Format)")
        .x_label("Energy (eV)")
        .y_label("Normalized Absorption")
        .dimensions(1200, 800)
        .save(&svg_path)?;
    
    // Create a group demo
    println!("\nCreating a group of spectra");
    let mut group = XASGroup::new();
    
    // Add the original spectrum
    group.add_spectrum(spectrum.clone());
    
    // Add two variants of the spectrum with shifted data
    let mut spectrum2 = spectrum.clone();
    spectrum2.set_name("Ru QAS - Variant 1");
    if let Some(mu) = &mut spectrum2.mu {
        // Shift mu values by 10% up
        for i in 0..mu.len() {
            mu[i] *= 1.1;
        }
    }
    group.add_spectrum(spectrum2);
    
    let mut spectrum3 = spectrum.clone();
    spectrum3.set_name("Ru QAS - Variant 2");
    if let Some(mu) = &mut spectrum3.mu {
        // Shift mu values by 10% down
        for i in 0..mu.len() {
            mu[i] *= 0.9;
        }
    }
    group.add_spectrum(spectrum3);
    
    // Process group
    group.find_e0()?;
    group.normalize()?;
    group.calc_background()?;
    group.fft()?;
    
    // Plot group comparisons
    println!("Plotting group comparisons");
    
    // Plot raw spectra
    let group_raw_path = format!("{}/09_group_raw.png", output_dir);
    println!("Plotting raw group data to {}", group_raw_path);
    group.plot_raw(&group_raw_path)?;
    
    // Plot normalized spectra
    let group_norm_path = format!("{}/10_group_normalized.png", output_dir);
    println!("Plotting normalized group data to {}", group_norm_path);
    group.plot_normalized(&group_norm_path)?;
    
    // Plot derivative spectra
    let group_deriv_path = format!("{}/11_group_derivative.png", output_dir);
    println!("Plotting derivative group data to {}", group_deriv_path);
    group.plot_derivative(&group_deriv_path)?;
    
    // Plot k-space EXAFS
    let group_k_path = format!("{}/12_group_k_space.png", output_dir);
    println!("Plotting k-space group data to {}", group_k_path);
    group.plot_k_space(&group_k_path, Some(2.0))?;
    
    // Plot R-space magnitude
    let group_r_mag_path = format!("{}/13_group_r_mag.png", output_dir);
    println!("Plotting R-space magnitude group data to {}", group_r_mag_path);
    group.plot_r_mag(&group_r_mag_path)?;
    
    // Plot R-space components
    let group_r_comp_path = format!("{}/14_group_r_components.png", output_dir);
    println!("Plotting R-space components group data to {}", group_r_comp_path);
    group.plot_r_components(&group_r_comp_path)?;
    
    // Demo for fitting visualization
    println!("\nCreating fit visualization examples");
    
    // Create synthetic data for fitting demo
    let k = Array1::linspace(2.0, 14.0, 100);
    let data = k.mapv(|k_val| 0.5 * (k_val * 2.0).sin() * (-0.1 * k_val).exp() + 0.3 * (k_val * 4.0).sin() * (-0.2 * k_val).exp());
    
    // Create a fitting dataset
    let mut dataset = FittingDataset::default();
    dataset.set_k(k.clone());
    dataset.set_data(data.clone());
    
    // Create a mock fit result
    let mut fit_result = FitResult::default();
    
    // Set best fit (add small noise to data)
    let best_fit = data.mapv(|val| val + 0.01 * (rand::random::<f64>() - 0.5));
    fit_result.set_best_fit(best_fit);
    
    // Create two components (damped sine waves with different frequencies)
    let component1 = k.mapv(|k_val| 0.5 * (k_val * 2.0).sin() * (-0.1 * k_val).exp());
    let component2 = k.mapv(|k_val| 0.3 * (k_val * 4.0).sin() * (-0.2 * k_val).exp());
    fit_result.set_components(vec![component1, component2]);
    
    // Create a mock correlation matrix
    let correlation = Array2::from_shape_vec(
        (4, 4),
        vec![
            1.0,  0.3, -0.1,  0.2,
            0.3,  1.0,  0.6, -0.4,
           -0.1,  0.6,  1.0,  0.1,
            0.2, -0.4,  0.1,  1.0,
        ]
    ).unwrap();
    fit_result.set_correlation_matrix(correlation);
    
    // Set parameter names
    fit_result.set_param_names(vec![
        "N₁".to_string(), 
        "r₁".to_string(), 
        "σ²₁".to_string(),
        "E₀".to_string()
    ]);
    
    // Plot fit
    let fit_path = format!("{}/15_fit.png", output_dir);
    println!("Plotting fit to {}", fit_path);
    plot_fit(&dataset, &fit_result, &fit_path)?;
    
    // Plot fit components
    let components_path = format!("{}/16_fit_components.png", output_dir);
    println!("Plotting fit components to {}", components_path);
    plot_fit_components(&dataset, &fit_result, &components_path)?;
    
    // Plot parameter correlation matrix
    let correlation_path = format!("{}/17_parameter_correlation.png", output_dir);
    println!("Plotting parameter correlation matrix to {}", correlation_path);
    plot_parameter_correlation(&fit_result, &correlation_path)?;
    
    // Print summary
    println!("\nPlotting complete. Files saved to:");
    for entry in std::fs::read_dir(output_dir)? {
        let entry = entry?;
        println!("  {}", entry.path().display());
    }
    
    Ok(())
}