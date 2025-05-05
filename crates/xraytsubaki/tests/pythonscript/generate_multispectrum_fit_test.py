#!/usr/bin/env python
"""
Generate test data for multi-spectrum EXAFS fitting with xraylarch

This script demonstrates how to simultaneously fit multiple XAS spectra
with linked parameters, as might be useful for analyzing a series of 
samples or the same sample measured at different temperatures.
"""

import os
import numpy as np
from larch import Group
from larch.io import read_ascii
from larch.xafs import pre_edge, autobk
from larch.fitting import param, minimize, fit_report

current_dir = os.path.dirname(os.path.abspath(__file__))
test_files_dir = os.path.join(current_dir, "../testfiles")
fit_results_dir = os.path.join(test_files_dir, "fit_results")

# Create output directory if it doesn't exist
if not os.path.exists(fit_results_dir):
    os.makedirs(fit_results_dir)

def create_synthetic_dataset(n_spectra=3):
    """Create synthetic datasets representing multiple measurements with small parameter changes"""
    # k grid for all spectra
    k = np.linspace(2, 15, 300)
    
    # Common parameters for all spectra
    common_freq = 1.5  # related to R (distance)
    common_phase = 0.3
    
    # Parameters that will vary across spectra
    amplitudes = [0.8, 0.75, 0.7]  # decreasing amplitude (e.g., thermal effect)
    damping_factors = [0.04, 0.045, 0.05]  # increasing disorder
    
    # Create dataset
    all_data = []
    for i in range(n_spectra):
        amp = amplitudes[i]
        damp = damping_factors[i]
        
        # Generate synthetic chi(k) data
        chi = amp * np.sin(2 * common_freq * k + common_phase) * np.exp(-damp * k**2)
        noise = np.random.normal(0, 0.02, len(k))  # Add some random noise
        chi_noisy = chi + noise
        
        all_data.append((k, chi_noisy))
        
        # Save individual datasets
        dataset = np.column_stack((k, chi, chi_noisy))
        np.savetxt(os.path.join(fit_results_dir, f'synthetic_spectrum_{i+1}.dat'), 
                   dataset, header="k chi chi_noisy")
    
    # Save "true" parameters for later comparison
    params_data = np.array([
        amplitudes,
        [common_freq] * n_spectra,
        [common_phase] * n_spectra,
        damping_factors
    ]).T
    
    np.savetxt(os.path.join(fit_results_dir, 'multi_spectra_true_params.dat'),
               params_data, header="amp freq phase damp")
    
    return all_data, params_data

def fit_multi_spectra(spectra_data):
    """Fit multiple spectra simultaneously with shared parameters"""
    # Function to calculate chi(k) for one spectrum
    def calc_model(params, k, spectrum_idx):
        """Model function for a specific spectrum"""
        # Get the base parameter names
        amp_param = f"amp_{spectrum_idx+1}"
        damp_param = f"damp_{spectrum_idx+1}"
        
        # Common parameters across all spectra
        freq = params['freq'].value
        phase = params['phase'].value
        
        # Spectrum-specific parameters
        amp = params[amp_param].value
        damp = params[damp_param].value
        
        # Calculate the model
        return amp * np.sin(2 * freq * k + phase) * np.exp(-damp * k**2)
    
    # Residual function for all spectra
    def residual(params, k_array, data_array, indices):
        """Calculate residuals for all spectra"""
        resid = []
        
        for idx, (k, data) in enumerate(zip(k_array, data_array)):
            model = calc_model(params, k, idx)
            # Apply k-weighting (k^2)
            kw = k**2
            resid.extend((data - model) * kw)
        
        return np.array(resid)
    
    # Get data
    k_array = [k for k, _ in spectra_data]
    data_array = [data for _, data in spectra_data]
    indices = list(range(len(spectra_data)))
    
    # Create parameter group
    params = Group()
    
    # Parameters shared across all spectra
    params.freq = param(name='freq', value=1.4, vary=True, min=0.5, max=3.0)
    params.phase = param(name='phase', value=0.1, vary=True, min=-np.pi, max=np.pi)
    
    # Spectrum-specific parameters
    for i in range(len(spectra_data)):
        setattr(params, f"amp_{i+1}", param(name=f"amp_{i+1}", value=0.7, vary=True, min=0.1, max=2.0))
        setattr(params, f"damp_{i+1}", param(name=f"damp_{i+1}", value=0.04, vary=True, min=0.001, max=0.2))
    
    # Run the fit
    result = minimize(residual, params, args=(k_array, data_array, indices))
    
    # Save fitted parameters
    n_spectra = len(spectra_data)
    fit_params = np.zeros((n_spectra, 4))
    
    for i in range(n_spectra):
        amp_name = f"amp_{i+1}"
        damp_name = f"damp_{i+1}"
        fit_params[i, 0] = result.params[amp_name].value
        fit_params[i, 1] = result.params['freq'].value
        fit_params[i, 2] = result.params['phase'].value
        fit_params[i, 3] = result.params[damp_name].value
    
    np.savetxt(os.path.join(fit_results_dir, 'multi_spectra_fit_params.dat'),
               fit_params, header="amp freq phase damp")
    
    # Save fit results for each spectrum
    for i in range(n_spectra):
        k = k_array[i]
        data = data_array[i]
        model = calc_model(result.params, k, i)
        
        fit_data = np.column_stack((k, data, model))
        np.savetxt(os.path.join(fit_results_dir, f'multi_spectra_fit_result_{i+1}.dat'),
                   fit_data, header="k data_chi model_chi")
    
    # Print fit report
    print(fit_report(result))
    
    return result

def fit_spectra_with_constraints():
    """
    Demonstrate parameter constraints across multiple spectra,
    such as:
    
    1. Fixed amplitude ratios between spectra
    2. Delta-e0 constraints (energy shift differences)
    3. Delta-sigma2 constraints (differences in Debye-Waller factors)
    """
    # Create synthetic dataset
    spectra_data, true_params = create_synthetic_dataset(n_spectra=3)
    
    # Function to calculate chi(k) for one spectrum
    def calc_model(params, k, spectrum_idx):
        """Model function for a specific spectrum"""
        # Get the base parameter names for this spectrum
        amp_param = f"amp_{spectrum_idx+1}" if spectrum_idx > 0 else "amp_1"
        damp_param = f"damp_{spectrum_idx+1}" if spectrum_idx > 0 else "damp_1"
        
        # Common parameters across all spectra
        freq = params['freq'].value
        phase = params['phase'].value
        
        # Spectrum-specific parameters with constraints
        # For amplitude, spectrum 2 and 3 are scaled relative to spectrum 1
        if spectrum_idx == 0:
            amp = params[amp_param].value
        else:
            amp_scale = params[f"amp_scale_{spectrum_idx+1}"].value
            amp = params["amp_1"].value * amp_scale
        
        # For damping, spectrum 2 and 3 have delta_sigma2 added to spectrum 1
        if spectrum_idx == 0:
            damp = params[damp_param].value
        else:
            delta_damp = params[f"delta_damp_{spectrum_idx+1}"].value
            damp = params["damp_1"].value + delta_damp
        
        # Calculate the model
        return amp * np.sin(2 * freq * k + phase) * np.exp(-damp * k**2)
    
    # Residual function for all spectra
    def residual(params, k_array, data_array, indices):
        """Calculate residuals for all spectra"""
        resid = []
        
        for idx, (k, data) in enumerate(zip(k_array, data_array)):
            model = calc_model(params, k, idx)
            # Apply k-weighting (k^2)
            kw = k**2
            resid.extend((data - model) * kw)
        
        return np.array(resid)
    
    # Get data
    k_array = [k for k, _ in spectra_data]
    data_array = [data for _, data in spectra_data]
    indices = list(range(len(spectra_data)))
    
    # Create parameter group with constraints
    params = Group()
    
    # Parameters shared across all spectra
    params.freq = param(name='freq', value=1.4, vary=True, min=0.5, max=3.0)
    params.phase = param(name='phase', value=0.1, vary=True, min=-np.pi, max=np.pi)
    
    # Base parameters for spectrum 1
    params.amp_1 = param(name='amp_1', value=0.8, vary=True, min=0.1, max=2.0)
    params.damp_1 = param(name='damp_1', value=0.04, vary=True, min=0.001, max=0.2)
    
    # Constraint parameters for spectra 2 and 3
    # Amplitude scale factors
    params.amp_scale_2 = param(name='amp_scale_2', value=0.9, vary=True, min=0.5, max=1.5)
    params.amp_scale_3 = param(name='amp_scale_3', value=0.8, vary=True, min=0.5, max=1.5)
    
    # Delta damping factors (delta_sigma2)
    params.delta_damp_2 = param(name='delta_damp_2', value=0.005, vary=True, min=0.0, max=0.05)
    params.delta_damp_3 = param(name='delta_damp_3', value=0.01, vary=True, min=0.0, max=0.05)
    
    # Run the fit
    result = minimize(residual, params, args=(k_array, data_array, indices))
    
    # Save fitted parameters in a format that shows the constraints
    fit_params = np.zeros((1, 10))
    
    # Base parameters
    fit_params[0, 0] = result.params['freq'].value
    fit_params[0, 1] = result.params['phase'].value
    fit_params[0, 2] = result.params['amp_1'].value
    fit_params[0, 3] = result.params['damp_1'].value
    
    # Constraint parameters
    fit_params[0, 4] = result.params['amp_scale_2'].value
    fit_params[0, 5] = result.params['amp_scale_3'].value
    fit_params[0, 6] = result.params['delta_damp_2'].value
    fit_params[0, 7] = result.params['delta_damp_3'].value
    
    # Derived effective parameters for spectra 2 and 3
    fit_params[0, 8] = result.params['amp_1'].value * result.params['amp_scale_2'].value
    fit_params[0, 9] = result.params['amp_1'].value * result.params['amp_scale_3'].value
    
    header = "freq phase amp_1 damp_1 amp_scale_2 amp_scale_3 delta_damp_2 delta_damp_3 effective_amp_2 effective_amp_3"
    np.savetxt(os.path.join(fit_results_dir, 'multi_spectra_constrained_params.dat'),
               fit_params, header=header)
    
    # Save fit results for each spectrum
    for i in range(len(spectra_data)):
        k = k_array[i]
        data = data_array[i]
        model = calc_model(result.params, k, i)
        
        fit_data = np.column_stack((k, data, model))
        np.savetxt(os.path.join(fit_results_dir, f'multi_spectra_constrained_fit_{i+1}.dat'),
                   fit_data, header="k data_chi model_chi")
    
    # Save separate effective parameter table that's easier to compare with true values
    effective_params = np.zeros((3, 4))
    
    # Spectrum 1
    effective_params[0, 0] = result.params['amp_1'].value
    effective_params[0, 1] = result.params['freq'].value
    effective_params[0, 2] = result.params['phase'].value
    effective_params[0, 3] = result.params['damp_1'].value
    
    # Spectrum 2
    effective_params[1, 0] = result.params['amp_1'].value * result.params['amp_scale_2'].value
    effective_params[1, 1] = result.params['freq'].value
    effective_params[1, 2] = result.params['phase'].value
    effective_params[1, 3] = result.params['damp_1'].value + result.params['delta_damp_2'].value
    
    # Spectrum 3
    effective_params[2, 0] = result.params['amp_1'].value * result.params['amp_scale_3'].value
    effective_params[2, 1] = result.params['freq'].value
    effective_params[2, 2] = result.params['phase'].value
    effective_params[2, 3] = result.params['damp_1'].value + result.params['delta_damp_3'].value
    
    np.savetxt(os.path.join(fit_results_dir, 'multi_spectra_effective_params.dat'),
               effective_params, header="amp freq phase damp")
    
    # Print fit report
    print(fit_report(result))
    
    return result

if __name__ == "__main__":
    print("Generating multi-spectrum test data...")
    spectra_data, true_params = create_synthetic_dataset(n_spectra=3)
    
    print("\nFitting multiple spectra independently...")
    fit_result = fit_multi_spectra(spectra_data)
    
    print("\nFitting multiple spectra with constraints...")
    constrained_result = fit_spectra_with_constraints()
    
    print(f"\nTest data saved to {fit_results_dir}")