#!/usr/bin/env python
"""
Generate test data for XAS fitting with xraylarch
This script creates a simplified version that focuses on XAS fitting
without using the more complex feffpath objects.
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


def create_test_data():
    """Create synthetic data for testing"""
    # Generate some synthetic EXAFS data for testing
    # This is just k, chi data - we won't use actual FEFF paths here
    k = np.linspace(2, 15, 500)
    
    # Create a simple damped sinusoidal oscillation to simulate chi(k)
    # Parameters: amplitude, frequency (related to distance), phase, damping
    amp = 0.8
    freq = 1.5  # Related to the path distance
    phase = 0.3
    damp = 0.05  # Related to Debye-Waller factor
    
    # Generate synthetic chi(k) data with some noise
    chi = amp * np.sin(2 * freq * k + phase) * np.exp(-damp * k**2)
    noise = np.random.normal(0, 0.05, len(k))
    chi_noisy = chi + noise
    
    # Save the synthetic data
    test_data = np.column_stack((k, chi, chi_noisy))
    np.savetxt(os.path.join(fit_results_dir, 'synthetic_k_chi.dat'), test_data,
               header="k chi chi_noisy")
    
    return k, chi, chi_noisy


def fit_with_damped_sine():
    """Fit a damped sine wave to the synthetic EXAFS-like data"""
    # Load synthetic data
    k, chi_true, chi_noisy = create_test_data()
    
    def sine_model(params, k):
        """Model: A damped sine wave"""
        amp = params['amp'].value
        freq = params['freq'].value
        phase = params['phase'].value
        damp = params['damp'].value
        return amp * np.sin(2 * freq * k + phase) * np.exp(-damp * k**2)
    
    def residual(params, k, data):
        """Calculate residual between model and data"""
        model = sine_model(params, k)
        return data - model
    
    # Create parameters for the fit
    params = Group()
    params.amp = param(name='amp', value=0.7, vary=True, min=0.1, max=2.0)
    params.freq = param(name='freq', value=1.4, vary=True, min=0.5, max=3.0)
    params.phase = param(name='phase', value=0.1, vary=True, min=-np.pi, max=np.pi)
    params.damp = param(name='damp', value=0.04, vary=True, min=0.001, max=0.2)
    
    # Run the fit
    result = minimize(residual, params, args=(k, chi_noisy))
    
    # Save the fit results
    # Use 0 for stderr as it might not be provided in all versions
    fit_params = np.array([
        result.params['amp'].value, 0.0,  # Zero for stderr
        result.params['freq'].value, 0.0,
        result.params['phase'].value, 0.0,
        result.params['damp'].value, 0.0
    ]).reshape(1, -1)
    
    np.savetxt(os.path.join(fit_results_dir, 'fit_params.dat'), fit_params,
               header="amp amp_err freq freq_err phase phase_err damp damp_err")
    
    # Generate the best fit model
    best_fit = sine_model(result.params, k)
    
    # Save the data and best fit
    fit_data = np.column_stack((k, chi_noisy, best_fit))
    np.savetxt(os.path.join(fit_results_dir, 'fit_result.dat'), fit_data,
               header="k data best_fit")
    
    # Calculate fit statistics
    ndata = len(k)
    nvarys = 4  # Number of parameters we're varying
    nfree = ndata - nvarys
    
    # Calculate the residuals
    residuals = residual(result.params, k, chi_noisy)
    chisqr = np.sum(residuals**2)
    redchi = chisqr / nfree
    
    # Save fit statistics
    with open(os.path.join(fit_results_dir, 'fit_stats.dat'), 'w') as f:
        f.write(f"# ndata {ndata}\n")
        f.write(f"# nvarys {nvarys}\n")
        f.write(f"# nfree {nfree}\n")
        f.write(f"# chisqr {chisqr}\n")
        f.write(f"# redchi {redchi}\n")
    
    # Print fit report for reference
    print(fit_report(result))
    
    return result


def fit_real_xas_data():
    """Fit a real XAS dataset with a simple model"""
    # Read Ru_QAS.dat data
    data_file = os.path.join(test_files_dir, "Ru_QAS.dat")
    data = read_ascii(data_file, labels='energy i0 itrans')
    data.mu = -np.log(data.itrans/data.i0)
    
    # Process data
    dat = Group(energy=data.energy, mu=data.mu)
    pre_edge(dat, e0=22117.0, pre1=-200, pre2=-75, norm1=150, norm2=600)
    autobk(dat, rbkg=1.1, kweight=2, kmin=0, kmax=15)
    
    # Extract k and chi data
    k = dat.k
    chi = dat.chi
    
    # Select a range for fitting
    start_idx = np.where(k >= 3.0)[0][0]
    end_idx = np.where(k <= 12.0)[0][-1]
    
    k_fit = k[start_idx:end_idx+1]
    chi_fit = chi[start_idx:end_idx+1]
    
    # Define a simpler model for real data: sum of damped sines
    def exafs_model(params, k):
        """Model: Sum of two damped sine waves"""
        amp1 = params['amp1'].value
        freq1 = params['freq1'].value  # related to R1
        phase1 = params['phase1'].value
        damp1 = params['damp1'].value  # related to sigma²₁
        
        amp2 = params['amp2'].value
        freq2 = params['freq2'].value  # related to R2
        phase2 = params['phase2'].value
        damp2 = params['damp2'].value  # related to sigma²₂
        
        # First shell contribution
        wave1 = amp1 * np.sin(2 * freq1 * k + phase1) * np.exp(-damp1 * k**2)
        # Second shell contribution
        wave2 = amp2 * np.sin(2 * freq2 * k + phase2) * np.exp(-damp2 * k**2)
        
        return wave1 + wave2
    
    def residual(params, k, data, weight=2):
        """Calculate residual between model and data, with k-weighting"""
        model = exafs_model(params, k)
        kw = k**weight  # Apply k-weighting
        return (data - model) * kw
    
    # Create parameters for the fit
    params = Group()
    # First shell - oxygen (around 2.0 Å)
    params.amp1 = param(name='amp1', value=0.5, vary=True, min=0.1, max=2.0)
    params.freq1 = param(name='freq1', value=1.0, vary=True, min=0.5, max=2.0)
    params.phase1 = param(name='phase1', value=0.0, vary=True, min=-np.pi, max=np.pi)
    params.damp1 = param(name='damp1', value=0.003, vary=True, min=0.001, max=0.02)
    
    # Second shell - ruthenium (around 3.5 Å)
    params.amp2 = param(name='amp2', value=0.3, vary=True, min=0.1, max=1.0)
    params.freq2 = param(name='freq2', value=1.7, vary=True, min=1.2, max=2.5)
    params.phase2 = param(name='phase2', value=0.0, vary=True, min=-np.pi, max=np.pi)
    params.damp2 = param(name='damp2', value=0.005, vary=True, min=0.001, max=0.02)
    
    # Run the fit
    result = minimize(residual, params, args=(k_fit, chi_fit))
    
    # Save the fit results
    # Use 0 for stderr as it might not be provided in all versions
    fit_params = np.array([
        result.params['amp1'].value, 0.0,  # Zero for stderr
        result.params['freq1'].value, 0.0,
        result.params['phase1'].value, 0.0,
        result.params['damp1'].value, 0.0,
        result.params['amp2'].value, 0.0,
        result.params['freq2'].value, 0.0,
        result.params['phase2'].value, 0.0,
        result.params['damp2'].value, 0.0
    ]).reshape(1, -1)
    
    np.savetxt(os.path.join(fit_results_dir, 'real_xas_fit_params.dat'), fit_params,
               header="amp1 amp1_err freq1 freq1_err phase1 phase1_err damp1 damp1_err " +
                      "amp2 amp2_err freq2 freq2_err phase2 phase2_err damp2 damp2_err")
    
    # Generate the best fit model
    best_fit = exafs_model(result.params, k_fit)
    
    # Calculate individual shell contributions
    shell1 = result.params['amp1'].value * np.sin(2 * result.params['freq1'].value * k_fit + result.params['phase1'].value) * np.exp(-result.params['damp1'].value * k_fit**2)
    shell2 = result.params['amp2'].value * np.sin(2 * result.params['freq2'].value * k_fit + result.params['phase2'].value) * np.exp(-result.params['damp2'].value * k_fit**2)
    
    # Save the data and best fit
    fit_data = np.column_stack((k_fit, chi_fit, best_fit, shell1, shell2))
    np.savetxt(os.path.join(fit_results_dir, 'real_xas_fit_result.dat'), fit_data,
               header="k data best_fit shell1 shell2")
    
    # Calculate fit statistics
    ndata = len(k_fit)
    nvarys = 8  # Number of parameters we're varying (2 shells × 4 params each)
    nfree = ndata - nvarys
    
    # Calculate the residuals
    residuals = residual(result.params, k_fit, chi_fit)
    chisqr = np.sum(residuals**2)
    redchi = chisqr / nfree
    
    # Save fit statistics
    with open(os.path.join(fit_results_dir, 'real_xas_fit_stats.dat'), 'w') as f:
        f.write(f"# ndata {ndata}\n")
        f.write(f"# nvarys {nvarys}\n")
        f.write(f"# nfree {nfree}\n")
        f.write(f"# chisqr {chisqr}\n")
        f.write(f"# redchi {redchi}\n")
    
    # Print fit report for reference
    print(fit_report(result))
    
    return result


if __name__ == "__main__":
    print("Generating synthetic data and fitting with damped sine...")
    fit_with_damped_sine()
    
    print("\nFitting real XAS data with a simple model...")
    fit_real_xas_data()
    
    print(f"\nTest data saved to {fit_results_dir}")