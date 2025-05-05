//! Tests for plotting functionality
//!
//! This module contains tests for the plotting functionality.

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::xafs::io;
    use crate::xafs::tests::TOP_DIR;
    use crate::plot::fitting::{plot_parameter_correlation, plot_fit_components};
    use nalgebra::DMatrix;
    use plotters::prelude::*;
    use std::path::Path;
    
    #[test]
    fn test_plot_raw_xanes() {
        // Load a test spectrum
        let test_file = format!("{}/tests/testfiles/Ru_QAS.dat", TOP_DIR);
        let spectrum = match io::load_spectrum_QAS_trans(&test_file) {
            Ok(spectrum) => spectrum,
            Err(e) => {
                println!("Failed to load test spectrum: {}", e);
                return;
            }
        };
        
        // Create output directory if it doesn't exist
        let output_dir = format!("{}/tests/test_output", TOP_DIR);
        std::fs::create_dir_all(&output_dir).unwrap_or_else(|_| {
            println!("Failed to create output directory");
        });
        
        // Plot raw XANES
        let output_file = format!("{}/raw_xanes.png", output_dir);
        let result = spectrum.plot_raw(&output_file);
        
        // Check if plot was created successfully
        assert!(result.is_ok(), "Failed to create raw XANES plot: {:?}", result.err());
        assert!(Path::new(&output_file).exists(), "Plot file was not created");
    }
    
    #[test]
    fn test_plot_normalized_xanes() {
        // Load a test spectrum
        let test_file = format!("{}/tests/testfiles/Ru_QAS.dat", TOP_DIR);
        let mut spectrum = match io::load_spectrum_QAS_trans(&test_file) {
            Ok(spectrum) => spectrum,
            Err(e) => {
                println!("Failed to load test spectrum: {}", e);
                return;
            }
        };
        
        // Normalize the spectrum
        match spectrum.find_e0() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to find e0: {}", e);
                return;
            }
        }
        
        match spectrum.normalize() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to normalize spectrum: {}", e);
                return;
            }
        }
        
        // Create output directory if it doesn't exist
        let output_dir = format!("{}/tests/test_output", TOP_DIR);
        std::fs::create_dir_all(&output_dir).unwrap_or_else(|_| {
            println!("Failed to create output directory");
        });
        
        // Plot normalized XANES
        let output_file = format!("{}/normalized_xanes.png", output_dir);
        let result = spectrum.plot_normalized(&output_file);
        
        // Check if plot was created successfully
        assert!(result.is_ok(), "Failed to create normalized XANES plot: {:?}", result.err());
        assert!(Path::new(&output_file).exists(), "Plot file was not created");
    }
    
    #[test]
    fn test_plot_exafs_k() {
        // Load a test spectrum
        let test_file = format!("{}/tests/testfiles/Ru_QAS.dat", TOP_DIR);
        let mut spectrum = match io::load_spectrum_QAS_trans(&test_file) {
            Ok(spectrum) => spectrum,
            Err(e) => {
                println!("Failed to load test spectrum: {}", e);
                return;
            }
        };
        
        // Process the spectrum for EXAFS
        match spectrum.find_e0() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to find e0: {}", e);
                return;
            }
        }
        
        match spectrum.normalize() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to normalize spectrum: {}", e);
                return;
            }
        }
        
        match spectrum.calc_background() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to calculate background: {}", e);
                return;
            }
        }
        
        // Create output directory if it doesn't exist
        let output_dir = format!("{}/tests/test_output", TOP_DIR);
        std::fs::create_dir_all(&output_dir).unwrap_or_else(|_| {
            println!("Failed to create output directory");
        });
        
        // Plot k-space EXAFS with different k-weightings
        for k_weight in [0.0, 1.0, 2.0, 3.0].iter() {
            let output_file = format!("{}/exafs_k{}.png", output_dir, *k_weight as usize);
            let result = spectrum.plot_k_space(&output_file, Some(*k_weight));
            
            // Check if plot was created successfully
            assert!(result.is_ok(), "Failed to create k-space EXAFS plot with k-weight {}: {:?}", k_weight, result.err());
            assert!(Path::new(&output_file).exists(), "Plot file was not created");
        }
    }
    
    #[test]
    fn test_plot_exafs_r() {
        // Load a test spectrum
        let test_file = format!("{}/tests/testfiles/Ru_QAS.dat", TOP_DIR);
        let mut spectrum = match io::load_spectrum_QAS_trans(&test_file) {
            Ok(spectrum) => spectrum,
            Err(e) => {
                println!("Failed to load test spectrum: {}", e);
                return;
            }
        };
        
        // Process the spectrum for EXAFS
        match spectrum.find_e0() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to find e0: {}", e);
                return;
            }
        }
        
        match spectrum.normalize() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to normalize spectrum: {}", e);
                return;
            }
        }
        
        match spectrum.calc_background() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to calculate background: {}", e);
                return;
            }
        }
        
        match spectrum.fft() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to perform FFT: {}", e);
                return;
            }
        }
        
        // Create output directory if it doesn't exist
        let output_dir = format!("{}/tests/test_output", TOP_DIR);
        std::fs::create_dir_all(&output_dir).unwrap_or_else(|_| {
            println!("Failed to create output directory");
        });
        
        // Plot R-space magnitude
        let output_file = format!("{}/exafs_r_mag.png", output_dir);
        let result = spectrum.plot_r_mag(&output_file);
        
        // Check if plot was created successfully
        assert!(result.is_ok(), "Failed to create R-space magnitude plot: {:?}", result.err());
        assert!(Path::new(&output_file).exists(), "Plot file was not created");
        
        // Plot R-space components
        let output_file = format!("{}/exafs_r_components.png", output_dir);
        let result = spectrum.plot_r_components(&output_file);
        
        // Check if plot was created successfully
        assert!(result.is_ok(), "Failed to create R-space components plot: {:?}", result.err());
        assert!(Path::new(&output_file).exists(), "Plot file was not created");
    }
    
    #[test]
    fn test_plot_builder() {
        // Load a test spectrum
        let test_file = format!("{}/tests/testfiles/Ru_QAS.dat", TOP_DIR);
        let mut spectrum = match io::load_spectrum_QAS_trans(&test_file) {
            Ok(spectrum) => spectrum,
            Err(e) => {
                println!("Failed to load test spectrum: {}", e);
                return;
            }
        };
        
        // Process the spectrum
        match spectrum.find_e0() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to find e0: {}", e);
                return;
            }
        }
        
        // Create output directory if it doesn't exist
        let output_dir = format!("{}/tests/test_output", TOP_DIR);
        std::fs::create_dir_all(&output_dir).unwrap_or_else(|_| {
            println!("Failed to create output directory");
        });
        
        // Use plot builder with customization
        let output_file = format!("{}/custom_plot.png", output_dir);
        let result = spectrum.get_plot_builder()
            .title("Custom Plot Title")
            .x_label("Energy (eV)")
            .y_label("Absorption (a.u.)")
            .x_range(22000.0, 22300.0)
            .dimensions(1024, 768)
            .background_color(WHITE)
            .line_width(3)
            .save(&output_file);
        
        // Check if plot was created successfully
        assert!(result.is_ok(), "Failed to create custom plot: {:?}", result.err());
        assert!(Path::new(&output_file).exists(), "Plot file was not created");
        
        // Test SVG output
        let output_file = format!("{}/vector_plot.svg", output_dir);
        let result = spectrum.get_plot_builder().save(&output_file);
        
        // Check if plot was created successfully
        assert!(result.is_ok(), "Failed to create SVG plot: {:?}", result.err());
        assert!(Path::new(&output_file).exists(), "SVG plot file was not created");
    }
    
    #[test]
    fn test_xas_group_plotting() {
        // Create a group with multiple spectra
        let test_file = format!("{}/tests/testfiles/Ru_QAS.dat", TOP_DIR);
        let mut spectrum = match io::load_spectrum_QAS_trans(&test_file) {
            Ok(spectrum) => spectrum,
            Err(e) => {
                println!("Failed to load test spectrum: {}", e);
                return;
            }
        };
        
        // Create a modified version of the spectrum to simulate multiple spectra
        let mut spectrum2 = spectrum.clone();
        if let Some(mu) = &mut spectrum2.mu {
            for i in 0..mu.len() {
                mu[i] *= 1.1;
            }
        }
        
        let mut spectrum3 = spectrum.clone();
        if let Some(mu) = &mut spectrum3.mu {
            for i in 0..mu.len() {
                mu[i] *= 0.9;
            }
        }
        
        // Set names
        spectrum.set_name("Sample 1");
        spectrum2.set_name("Sample 2");
        spectrum3.set_name("Sample 3");
        
        // Create a group
        let mut group = crate::xafs::xasgroup::XASGroup::new();
        group.add_spectrum(spectrum);
        group.add_spectrum(spectrum2);
        group.add_spectrum(spectrum3);
        
        // Process all spectra
        match group.find_e0() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to find e0: {}", e);
                return;
            }
        }
        
        match group.normalize() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to normalize spectra: {}", e);
                return;
            }
        }
        
        // Create output directory if it doesn't exist
        let output_dir = format!("{}/tests/test_output", TOP_DIR);
        std::fs::create_dir_all(&output_dir).unwrap_or_else(|_| {
            println!("Failed to create output directory");
        });
        
        // Test group plotting functions
        // Plot raw spectra
        let raw_output_file = format!("{}/group_raw.png", output_dir);
        let result = group.plot_raw(&raw_output_file);
        assert!(result.is_ok(), "Failed to create raw group plot: {:?}", result.err());
        assert!(Path::new(&raw_output_file).exists(), "Raw group plot file was not created");
        
        // Plot normalized spectra
        let norm_output_file = format!("{}/group_normalized.png", output_dir);
        let result = group.plot_normalized(&norm_output_file);
        assert!(result.is_ok(), "Failed to create normalized group plot: {:?}", result.err());
        assert!(Path::new(&norm_output_file).exists(), "Normalized group plot file was not created");
        
        // Plot derivative
        let deriv_output_file = format!("{}/group_derivative.png", output_dir);
        let result = group.plot_derivative(&deriv_output_file);
        assert!(result.is_ok(), "Failed to create derivative group plot: {:?}", result.err());
        assert!(Path::new(&deriv_output_file).exists(), "Derivative group plot file was not created");
        
        // Process for EXAFS
        match group.calc_background() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to calculate background: {}", e);
                return;
            }
        }
        
        // Test k-space plotting
        let k_output_file = format!("{}/group_k_space.png", output_dir);
        let result = group.plot_k_space(&k_output_file, Some(2.0));
        assert!(result.is_ok(), "Failed to create k-space group plot: {:?}", result.err());
        assert!(Path::new(&k_output_file).exists(), "K-space group plot file was not created");
        
        // Process for R-space
        match group.fft() {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to perform FFT: {}", e);
                return;
            }
        }
        
        // Test R-magnitude plotting
        let r_mag_output_file = format!("{}/group_r_mag.png", output_dir);
        let result = group.plot_r_mag(&r_mag_output_file);
        assert!(result.is_ok(), "Failed to create R-magnitude group plot: {:?}", result.err());
        assert!(Path::new(&r_mag_output_file).exists(), "R-magnitude group plot file was not created");
        
        // Test R-components plotting
        let r_comp_output_file = format!("{}/group_r_components.png", output_dir);
        let result = group.plot_r_components(&r_comp_output_file);
        assert!(result.is_ok(), "Failed to create R-components group plot: {:?}", result.err());
        assert!(Path::new(&r_comp_output_file).exists(), "R-components group plot file was not created");
    }
    
    #[test]
    fn test_parameter_correlation_plotting() {
        // Create a mock FitResult with a correlation matrix
        use crate::xafs::fitting::FitResult;
        
        let mut fit_result = FitResult::default();
        
        // Create a mock correlation matrix (3x3)
        let correlation_data = vec![
            1.0, 0.5, -0.3,
            0.5, 1.0, 0.7,
            -0.3, 0.7, 1.0
        ];
        
        let correlation = DMatrix::from_vec(3, 3, correlation_data);
        
        // Add correlation matrix to fit result
        fit_result.set_correlation_matrix(correlation);
        
        // Set parameter names
        fit_result.set_param_names(vec!["N".to_string(), "r".to_string(), "σ²".to_string()]);
        
        // Create output directory if it doesn't exist
        let output_dir = format!("{}/tests/test_output", TOP_DIR);
        std::fs::create_dir_all(&output_dir).unwrap_or_else(|_| {
            println!("Failed to create output directory");
        });
        
        // Test parameter correlation plotting
        let output_file = format!("{}/parameter_correlation.png", output_dir);
        let result = plot_parameter_correlation(&fit_result, &output_file);
        
        assert!(result.is_ok(), "Failed to create parameter correlation plot: {:?}", result.err());
        assert!(Path::new(&output_file).exists(), "Parameter correlation plot file was not created");
    }
    
    #[test]
    fn test_fit_components_plotting() {
        // Create mock FittingDataset and FitResult for component plotting
        use crate::xafs::fitting::{FitResult, FittingDataset};
        use ndarray::Array1;
        
        // Create k data
        let k = Array1::linspace(2.0, 12.0, 100);
        
        // Create mock dataset
        let mut dataset = FittingDataset::default();
        dataset.set_k(k.clone());
        
        // Create mock data with sine waves
        let data = k.mapv(|k_val| 0.5 * (k_val * 2.0).sin() + 0.3 * (k_val * 4.0).sin());
        dataset.set_data(data.clone());
        
        // Create mock fit result
        let mut fit_result = FitResult::default();
        
        // Set best fit (identical to data for this test)
        fit_result.set_best_fit(data.clone());
        
        // Create two components (sine waves with different frequencies)
        let component1 = k.mapv(|k_val| 0.5 * (k_val * 2.0).sin());
        let component2 = k.mapv(|k_val| 0.3 * (k_val * 4.0).sin());
        
        // Set components
        fit_result.set_components(vec![component1, component2]);
        
        // Create output directory if it doesn't exist
        let output_dir = format!("{}/tests/test_output", TOP_DIR);
        std::fs::create_dir_all(&output_dir).unwrap_or_else(|_| {
            println!("Failed to create output directory");
        });
        
        // Test fit components plotting
        let output_file = format!("{}/fit_components.png", output_dir);
        let result = plot_fit_components(&dataset, &fit_result, &output_file);
        
        assert!(result.is_ok(), "Failed to create fit components plot: {:?}", result.err());
        assert!(Path::new(&output_file).exists(), "Fit components plot file was not created");
    }
}