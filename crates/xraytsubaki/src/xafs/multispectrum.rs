//! Multi-spectrum EXAFS fitting module
//! 
//! This module extends the basic EXAFS fitting functionality to support
//! fitting multiple spectra simultaneously with shared or linked parameters.

use std::collections::HashMap;

use nalgebra::{DMatrix, DVector, Dyn};
use ndarray::{Array1, ArrayBase, Ix1, OwnedRepr};

use crate::xafs::fitting::{FittingDataset, FittingError, FittingParameter, FittingParameters, FitResult, PathModel};
use crate::xafs::xafsutils::FTWindow;
use crate::xafs::mathutils::MathUtils;
use crate::xafs::nshare::{ToNalgebra, ToNdarray1};

/// Parameter constraint types for multi-spectrum fitting
#[derive(Debug, Clone)]
pub enum ParameterConstraint {
    /// Direct reference to another parameter (same value)
    Reference(String),
    /// Scaled relative to another parameter (value = factor * reference_value)
    Scale { reference: String, factor: f64 },
    /// Offset relative to another parameter (value = reference_value + offset)
    Offset { reference: String, offset: f64 },
    /// Formula based on other parameters (not implemented yet)
    Formula(String),
}

/// Extended parameter type with constraint support
#[derive(Debug, Clone)]
pub struct ConstrainedParameter {
    /// Base parameter
    pub param: FittingParameter,
    /// Optional constraint
    pub constraint: Option<ParameterConstraint>,
}

impl ConstrainedParameter {
    /// Create a new constrained parameter
    pub fn new(name: &str, value: f64) -> Self {
        Self {
            param: FittingParameter::new(name, value),
            constraint: None,
        }
    }

    /// Set parameter to be scaled relative to another parameter
    pub fn scale_from(mut self, reference: &str, factor: f64) -> Self {
        self.constraint = Some(ParameterConstraint::Scale {
            reference: reference.to_string(),
            factor,
        });
        self.param.vary = false; // Constrained parameters don't vary independently
        self
    }

    /// Set parameter to be offset from another parameter
    pub fn offset_from(mut self, reference: &str, offset: f64) -> Self {
        self.constraint = Some(ParameterConstraint::Offset {
            reference: reference.to_string(),
            offset,
        });
        self.param.vary = false; // Constrained parameters don't vary independently
        self
    }

    /// Set parameter to directly reference another parameter
    pub fn reference(mut self, reference: &str) -> Self {
        self.constraint = Some(ParameterConstraint::Reference(reference.to_string()));
        self.param.vary = false; // Constrained parameters don't vary independently
        self
    }

    /// Convert to a basic fitting parameter (for compatibility)
    pub fn to_fitting_parameter(&self) -> FittingParameter {
        self.param.clone()
    }

    /// Apply constraints based on current parameter values
    pub fn apply_constraint(&mut self, params: &HashMap<String, ConstrainedParameter>) -> Result<(), FittingError> {
        if let Some(constraint) = &self.constraint {
            match constraint {
                ParameterConstraint::Reference(reference) => {
                    if let Some(ref_param) = params.get(reference) {
                        self.param.value = ref_param.param.value;
                    } else {
                        return Err(FittingError::InvalidParameters);
                    }
                }
                ParameterConstraint::Scale { reference, factor } => {
                    if let Some(ref_param) = params.get(reference) {
                        self.param.value = ref_param.param.value * factor;
                    } else {
                        return Err(FittingError::InvalidParameters);
                    }
                }
                ParameterConstraint::Offset { reference, offset } => {
                    if let Some(ref_param) = params.get(reference) {
                        self.param.value = ref_param.param.value + offset;
                    } else {
                        return Err(FittingError::InvalidParameters);
                    }
                }
                ParameterConstraint::Formula(_) => {
                    // Not implemented yet
                    return Err(FittingError::InvalidParameters);
                }
            }
        }
        Ok(())
    }
}

/// Parameters collection with constraint support
#[derive(Debug, Clone)]
pub struct ConstrainedParameters {
    /// Parameters stored by name
    params: HashMap<String, ConstrainedParameter>,
}

impl ConstrainedParameters {
    /// Create a new empty parameter set
    pub fn new() -> Self {
        Self {
            params: HashMap::new(),
        }
    }

    /// Add a parameter to the set
    pub fn add_parameter(&mut self, param: ConstrainedParameter) -> &mut Self {
        self.params.insert(param.param.name.clone(), param);
        self
    }

    /// Get a parameter by name
    pub fn get(&self, name: &str) -> Option<&ConstrainedParameter> {
        self.params.get(name)
    }

    /// Get a mutable parameter by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut ConstrainedParameter> {
        self.params.get_mut(name)
    }

    /// Get all parameter names
    pub fn names(&self) -> Vec<String> {
        self.params.keys().cloned().collect()
    }

    /// Apply all constraints to update dependent parameters
    pub fn apply_constraints(&mut self) -> Result<(), FittingError> {
        // First, make a clone of the current parameters to use as reference
        // This prevents issues with circular references or order dependencies
        let params_clone = self.params.clone();
        
        // Apply constraints to all parameters
        for param in self.params.values_mut() {
            param.apply_constraint(&params_clone)?;
        }
        
        Ok(())
    }

    /// Convert to basic FittingParameters (for compatibility with existing code)
    pub fn to_fitting_parameters(&self) -> FittingParameters {
        let mut result = FittingParameters::new();
        
        for param in self.params.values() {
            result.add_parameter(param.param.clone());
        }
        
        result
    }

    /// Get all independently varying parameter names (those without constraints)
    pub fn varying_names(&self) -> Vec<String> {
        self.params
            .iter()
            .filter(|(_, p)| p.param.vary && p.constraint.is_none())
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get values of varying parameters as a vector
    pub fn varying_values(&self) -> DVector<f64> {
        let varying: Vec<f64> = self
            .params
            .iter()
            .filter(|(_, p)| p.param.vary && p.constraint.is_none())
            .map(|(_, p)| p.param.value)
            .collect();
        DVector::from_vec(varying)
    }

    /// Update values of varying parameters from a vector
    pub fn set_varying_values(&mut self, values: &DVector<f64>) -> Result<(), FittingError> {
        let varying_names = self.varying_names();
        assert_eq!(varying_names.len(), values.len());

        for (i, name) in varying_names.iter().enumerate() {
            if let Some(param) = self.params.get_mut(name) {
                param.param.value = values[i];
            }
        }
        
        // Apply constraints to update dependent parameters
        self.apply_constraints()?;
        
        Ok(())
    }
}

/// Multi-spectrum fitting dataset
#[derive(Debug)]
pub struct MultiSpectrumDataset {
    /// Collection of individual datasets
    pub datasets: Vec<FittingDataset>,
    /// Parameters with constraints
    pub params: ConstrainedParameters,
}

impl MultiSpectrumDataset {
    /// Create a new multi-spectrum dataset
    pub fn new() -> Self {
        Self {
            datasets: Vec::new(),
            params: ConstrainedParameters::new(),
        }
    }

    /// Add a dataset to the collection
    pub fn add_dataset(&mut self, dataset: FittingDataset) -> &mut Self {
        self.datasets.push(dataset);
        self
    }

    /// Calculate model chi(k) for all spectra with current parameters
    pub fn calc_all_models(&self) -> Result<Vec<Array1<f64>>, FittingError> {
        // Convert constrained parameters to basic parameters
        let fitting_params = self.params.to_fitting_parameters();
        
        // Calculate model for each dataset
        let mut results = Vec::with_capacity(self.datasets.len());
        
        for dataset in &self.datasets {
            let model = dataset.calc_model_chi(&fitting_params)?;
            results.push(model);
        }
        
        Ok(results)
    }

    /// Calculate residuals for all spectra
    pub fn calc_residuals(&self) -> Result<DVector<f64>, FittingError> {
        // Calculate models for all spectra
        let models = self.calc_all_models()?;
        
        // Collect all residuals in a single vector
        let mut all_residuals = Vec::new();
        
        for (i, dataset) in self.datasets.iter().enumerate() {
            let model = &models[i];
            
            // Filter by k-range if specified
            let (indices, kw) = if let Some((kmin, kmax)) = dataset.k_range {
                let idx: Vec<usize> = dataset.k
                    .iter()
                    .enumerate()
                    .filter(|(_, &k)| k >= kmin && k <= kmax)
                    .map(|(i, _)| i)
                    .collect();
                
                // Calculate k-weights for the selected indices
                let kw: Vec<f64> = idx.iter()
                    .map(|&i| dataset.k[i].powf(dataset.kweight))
                    .collect();
                
                (idx, kw)
            } else {
                // Use all data
                let idx: Vec<usize> = (0..dataset.k.len()).collect();
                let kw: Vec<f64> = dataset.k
                    .iter()
                    .map(|&k| k.powf(dataset.kweight))
                    .collect();
                
                (idx, kw)
            };
            
            // Apply window function if specified
            let window = if let Some(window_type) = &dataset.window {
                let mut window_values = vec![1.0; dataset.k.len()];
                
                if let Some((kmin, kmax)) = dataset.k_range {
                    // This is a simplified version similar to the one in fitting.rs
                    for (i, &k) in dataset.k.iter().enumerate() {
                        if k >= kmin && k <= kmax {
                            let x = (k - kmin) / (kmax - kmin);
                            window_values[i] = match window_type {
                                crate::xafs::xafsutils::FTWindow::Hanning => 0.5 * (1.0 - (2.0 * std::f64::consts::PI * x).cos()),
                                crate::xafs::xafsutils::FTWindow::Sine => (std::f64::consts::PI * x).sin(),
                                _ => 1.0, // Default to rectangle window
                            };
                        } else {
                            window_values[i] = 0.0;
                        }
                    }
                }
                
                window_values
            } else {
                vec![1.0; dataset.k.len()]
            };
            
            // Calculate weighted residuals: (data - model) * k^kweight * window
            let residuals: Vec<f64> = indices.iter()
                .enumerate()
                .map(|(j, &i)| {
                    (dataset.chi[i] - model[i]) * kw[j] * window[i]
                })
                .collect();
            
            // Add to the full residual vector
            all_residuals.extend(residuals);
        }
        
        Ok(DVector::from_vec(all_residuals))
    }
}

/// Multi-spectrum EXAFS fit result
#[derive(Debug)]
pub struct MultiSpectrumFitResult {
    /// Optimized parameters
    pub params: ConstrainedParameters,
    /// Model chi(k) for each spectrum
    pub model_chis: Vec<Array1<f64>>,
    /// Statistics for the overall fit
    pub ndata: usize,
    pub nvarys: usize,
    pub nfree: usize,
    pub chisqr: f64,
    pub redchi: f64,
    /// R-factors for each spectrum
    pub r_factors: Vec<f64>,
}

/// Multi-spectrum EXAFS fitter
pub struct MultiSpectrumFitter<'a> {
    /// Dataset to fit
    dataset: &'a MultiSpectrumDataset,
}

impl<'a> MultiSpectrumFitter<'a> {
    /// Create a new multi-spectrum fitter
    pub fn new(dataset: &'a MultiSpectrumDataset) -> Self {
        Self { dataset }
    }

    /// A simplified version of fitting for demonstration
    /// In a real implementation, we would implement a full optimization algorithm
    pub fn fit(&self) -> Result<MultiSpectrumFitResult, FittingError> {
        // Get parameter clone for calculations
        let params = self.dataset.params.clone();
        
        // Calculate models for all spectra
        let models = self.dataset.calc_all_models()?;
        
        // Calculate statistics
        let ndata: usize = self.dataset.datasets.iter()
            .map(|d| d.k.len())
            .sum();
        
        let nvarys = params.varying_names().len();
        let nfree = ndata.saturating_sub(nvarys);
        
        // Calculate residuals and chi-square
        let residuals = self.dataset.calc_residuals()?;
        let chisqr = residuals.dot(&residuals);
        let redchi = if nfree > 0 { chisqr / nfree as f64 } else { f64::NAN };
        
        // Calculate R-factor for each spectrum
        let mut r_factors = Vec::with_capacity(self.dataset.datasets.len());
        
        for (i, dataset) in self.dataset.datasets.iter().enumerate() {
            let model = &models[i];
            
            let r_factor = {
                let data_sum_sq: f64 = dataset.chi
                    .iter()
                    .zip(dataset.k.iter())
                    .map(|(&chi, &k)| {
                        chi.powi(2) * k.powf(2.0 * dataset.kweight)
                    })
                    .sum();
                
                let diff_sum_sq: f64 = dataset.chi
                    .iter()
                    .zip(model.iter())
                    .zip(dataset.k.iter())
                    .map(|((&data, &model), &k)| {
                        (data - model).powi(2) * k.powf(2.0 * dataset.kweight)
                    })
                    .sum();
                
                diff_sum_sq / data_sum_sq
            };
            
            r_factors.push(r_factor);
        }
        
        Ok(MultiSpectrumFitResult {
            params,
            model_chis: models,
            ndata,
            nvarys,
            nfree,
            chisqr,
            redchi,
            r_factors,
        })
    }
}

// The actual optimization-based implementation will be added in a future PR
// For now, this is a structure to demonstrate the design and enable testing

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::fitting::SimplePath;
    use crate::xafs::tests::TOP_DIR;
    use approx::assert_abs_diff_eq;
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use std::path::Path;
    
    // Helper function to load test data
    fn load_test_data(filename: &str) -> (Array1<f64>, Array1<f64>) {
        let filepath = format!("{}/tests/testfiles/fit_results/{}", TOP_DIR, filename);
        let file = File::open(Path::new(&filepath)).unwrap();
        let reader = BufReader::new(file);
        
        let mut k = Vec::new();
        let mut chi = Vec::new();
        
        for (i, line) in reader.lines().enumerate() {
            if i == 0 { continue; } // Skip header
            
            let line = line.unwrap();
            let values: Vec<f64> = line
                .split_whitespace()
                .map(|s| s.parse::<f64>().unwrap())
                .collect();
            
            if values.len() >= 2 {
                k.push(values[0]);
                chi.push(values[1]);
            }
        }
        
        (Array1::from(k), Array1::from(chi))
    }
    
    #[test]
    fn test_constrained_parameters() {
        // Create parameters with constraints
        let mut params = ConstrainedParameters::new();
        
        // Base parameter
        params.add_parameter(ConstrainedParameter::new("amp_1", 0.8));
        params.add_parameter(ConstrainedParameter::new("damp_1", 0.04));
        
        // Constrained parameters
        params.add_parameter(ConstrainedParameter::new("amp_2", 0.0)
            .scale_from("amp_1", 0.9)); // amp_2 = 0.9 * amp_1
        
        params.add_parameter(ConstrainedParameter::new("damp_2", 0.0)
            .offset_from("damp_1", 0.005)); // damp_2 = damp_1 + 0.005
        
        // Apply constraints
        params.apply_constraints().unwrap();
        
        // Check that constrained parameters were updated correctly
        assert_abs_diff_eq!(
            params.get("amp_2").unwrap().param.value,
            0.8 * 0.9,
            epsilon = 1e-10
        );
        
        assert_abs_diff_eq!(
            params.get("damp_2").unwrap().param.value,
            0.04 + 0.005,
            epsilon = 1e-10
        );
        
        // Change base parameter and check that constraints propagate
        if let Some(amp_1) = params.get_mut("amp_1") {
            amp_1.param.value = 1.0;
        }
        
        params.apply_constraints().unwrap();
        
        assert_abs_diff_eq!(
            params.get("amp_2").unwrap().param.value,
            1.0 * 0.9,
            epsilon = 1e-10
        );
    }
    
    #[test]
    fn test_multi_spectrum_simple() {
        // Load test data for multiple spectra
        let (k1, chi1) = load_test_data("multi_spectra_fit_result_1.dat");
        let (k2, chi2) = load_test_data("multi_spectra_fit_result_2.dat");
        let (k3, chi3) = load_test_data("multi_spectra_fit_result_3.dat");
        
        // Create individual datasets
        let mut dataset1 = FittingDataset::new(k1.clone(), chi1.clone());
        dataset1.set_kweight(2.0);
        dataset1.add_path(SimplePath::new("amp_1", "freq", "phase", "damp_1", 1.0));
        
        let mut dataset2 = FittingDataset::new(k2.clone(), chi2.clone());
        dataset2.set_kweight(2.0);
        dataset2.add_path(SimplePath::new("amp_2", "freq", "phase", "damp_2", 1.0));
        
        let mut dataset3 = FittingDataset::new(k3.clone(), chi3.clone());
        dataset3.set_kweight(2.0);
        dataset3.add_path(SimplePath::new("amp_3", "freq", "phase", "damp_3", 1.0));
        
        // Create multi-spectrum dataset
        let mut multi_dataset = MultiSpectrumDataset::new();
        multi_dataset.add_dataset(dataset1)
                     .add_dataset(dataset2)
                     .add_dataset(dataset3);
        
        // Create parameters
        let mut params = ConstrainedParameters::new();
        
        // Shared parameters
        params.add_parameter(ConstrainedParameter::new("freq", 1.5));
        params.add_parameter(ConstrainedParameter::new("phase", 0.3));
        
        // Spectrum-specific parameters
        params.add_parameter(ConstrainedParameter::new("amp_1", 0.8));
        params.add_parameter(ConstrainedParameter::new("damp_1", 0.04));
        
        params.add_parameter(ConstrainedParameter::new("amp_2", 0.75));
        params.add_parameter(ConstrainedParameter::new("damp_2", 0.045));
        
        params.add_parameter(ConstrainedParameter::new("amp_3", 0.7));
        params.add_parameter(ConstrainedParameter::new("damp_3", 0.05));
        
        // Set parameters
        multi_dataset.params = params;
        
        // Calculate models
        let models = multi_dataset.calc_all_models().unwrap();
        
        // Each spectrum should have its own model
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].len(), k1.len());
        assert_eq!(models[1].len(), k2.len());
        assert_eq!(models[2].len(), k3.len());
        
        // Models should be reasonably close to the data
        // (not an exact fit since we're not optimizing here)
        for (i, (model, data)) in models.iter().zip([&chi1, &chi2, &chi3].iter()).enumerate() {
            let mse = model
                .iter()
                .zip(data.iter())
                .map(|(model, data)| (model - data).powi(2))
                .sum::<f64>() / model.len() as f64;
            
            // The MSE should be reasonably small
            assert!(mse < 0.1, "MSE too large for spectrum {}: {}", i + 1, mse);
        }
    }
    
    #[test]
    fn test_multi_spectrum_with_constraints() {
        // Load test data for multiple spectra
        let (k1, chi1) = load_test_data("multi_spectra_constrained_fit_1.dat");
        let (k2, chi2) = load_test_data("multi_spectra_constrained_fit_2.dat");
        let (k3, chi3) = load_test_data("multi_spectra_constrained_fit_3.dat");
        
        // Create individual datasets
        let mut dataset1 = FittingDataset::new(k1.clone(), chi1.clone());
        dataset1.set_kweight(2.0);
        dataset1.add_path(SimplePath::new("amp_1", "freq", "phase", "damp_1", 1.0));
        
        let mut dataset2 = FittingDataset::new(k2.clone(), chi2.clone());
        dataset2.set_kweight(2.0);
        dataset2.add_path(SimplePath::new("amp_2", "freq", "phase", "damp_2", 1.0));
        
        let mut dataset3 = FittingDataset::new(k3.clone(), chi3.clone());
        dataset3.set_kweight(2.0);
        dataset3.add_path(SimplePath::new("amp_3", "freq", "phase", "damp_3", 1.0));
        
        // Create multi-spectrum dataset
        let mut multi_dataset = MultiSpectrumDataset::new();
        multi_dataset.add_dataset(dataset1)
                     .add_dataset(dataset2)
                     .add_dataset(dataset3);
        
        // Create parameters with constraints
        let mut params = ConstrainedParameters::new();
        
        // Shared parameters
        params.add_parameter(ConstrainedParameter::new("freq", 1.5));
        params.add_parameter(ConstrainedParameter::new("phase", 0.3));
        
        // Base parameters for spectrum 1
        params.add_parameter(ConstrainedParameter::new("amp_1", 0.8));
        params.add_parameter(ConstrainedParameter::new("damp_1", 0.04));
        
        // Constraint parameters
        params.add_parameter(ConstrainedParameter::new("amp_scale_2", 0.9));
        params.add_parameter(ConstrainedParameter::new("amp_scale_3", 0.85));
        params.add_parameter(ConstrainedParameter::new("delta_damp_2", 0.005));
        params.add_parameter(ConstrainedParameter::new("delta_damp_3", 0.01));
        
        // Constrained parameters for spectrum 2 and 3
        params.add_parameter(ConstrainedParameter::new("amp_2", 0.0)
            .scale_from("amp_1", 0.9));
        params.add_parameter(ConstrainedParameter::new("damp_2", 0.0)
            .offset_from("damp_1", 0.005));
        
        params.add_parameter(ConstrainedParameter::new("amp_3", 0.0)
            .scale_from("amp_1", 0.85));
        params.add_parameter(ConstrainedParameter::new("damp_3", 0.0)
            .offset_from("damp_1", 0.01));
        
        // Apply constraints
        params.apply_constraints().unwrap();
        
        // Set parameters
        multi_dataset.params = params;
        
        // Calculate models
        let models = multi_dataset.calc_all_models().unwrap();
        
        // Verify constrained parameters were applied correctly
        assert_abs_diff_eq!(
            multi_dataset.params.get("amp_2").unwrap().param.value,
            multi_dataset.params.get("amp_1").unwrap().param.value * 
            multi_dataset.params.get("amp_scale_2").unwrap().param.value,
            epsilon = 1e-10
        );
        
        assert_abs_diff_eq!(
            multi_dataset.params.get("damp_3").unwrap().param.value,
            multi_dataset.params.get("damp_1").unwrap().param.value + 
            multi_dataset.params.get("delta_damp_3").unwrap().param.value,
            epsilon = 1e-10
        );
        
        // Each spectrum should have its own model
        assert_eq!(models.len(), 3);
        
        // Models should be reasonably close to the data
        for (i, (model, data)) in models.iter().zip([&chi1, &chi2, &chi3].iter()).enumerate() {
            let mse = model
                .iter()
                .zip(data.iter())
                .map(|(model, data)| (model - data).powi(2))
                .sum::<f64>() / model.len() as f64;
            
            // The MSE should be reasonably small
            assert!(mse < 0.1, "MSE too large for spectrum {}: {}", i + 1, mse);
        }
    }
}