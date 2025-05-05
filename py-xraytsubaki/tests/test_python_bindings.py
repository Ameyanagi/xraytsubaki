#!/usr/bin/env python3
"""
Manual test script for the Python bindings.
This script manually tests the Python bindings by running through a typical workflow.
"""

import os
import sys
import numpy as np
import matplotlib.pyplot as plt
from pathlib import Path

# Add parent directory to sys.path
parent_dir = str(Path(__file__).parent.parent)
if parent_dir not in sys.path:
    sys.path.insert(0, parent_dir)

# Try to import the Python bindings
try:
    import py_xraytsubaki as xt
    print("Successfully imported py_xraytsubaki")
except ImportError as e:
    print(f"Error importing py_xraytsubaki: {e}")
    print("You might need to build the Python bindings first.")
    sys.exit(1)

# Test data directory
test_data_dir = Path(__file__).parent / "test_data"
if not test_data_dir.exists():
    print(f"Test data directory {test_data_dir} does not exist.")
    print("Please run create_test_data.py first.")
    sys.exit(1)

# Test XASSpectrum class
def test_xasspectrum():
    print("\n=== Testing XASSpectrum class ===")
    
    # Load synthetic spectrum
    data = np.loadtxt(test_data_dir / "synthetic_xas.dat")
    energy = data[:, 0]
    mu = data[:, 1]
    
    # Create XASSpectrum object
    try:
        spectrum = xt.XASSpectrum(energy=energy, mu=mu)
        print("Created XASSpectrum object successfully")
    except Exception as e:
        print(f"Error creating XASSpectrum object: {e}")
        return
    
    # Test normalization
    try:
        spectrum.normalize(pre1=-200, pre2=-30, norm1=100, norm2=600)
        print(f"Normalized spectrum: E0 = {spectrum.e0:.1f} eV, edge_step = {spectrum.edge_step:.4f}")
    except Exception as e:
        print(f"Error normalizing spectrum: {e}")
        return
    
    # Test background removal
    try:
        spectrum.autobk(rbkg=1.0, kmin=0, kmax=15)
        print(f"Removed background: k range = {spectrum.k[0]:.1f} to {spectrum.k[-1]:.1f} Å⁻¹")
    except Exception as e:
        print(f"Error removing background: {e}")
        return
    
    # Test Fourier transform
    try:
        spectrum.xftf(kmin=2, kmax=12, dk=1, window='hanning', kweight=2)
        print(f"Performed Fourier transform: R range = {spectrum.r[0]:.1f} to {spectrum.r[-1]:.1f} Å")
    except Exception as e:
        print(f"Error performing Fourier transform: {e}")
        return
    
    # Test saving to file
    try:
        output_file = test_data_dir / "test_save.json"
        spectrum.save(str(output_file))
        print(f"Saved spectrum to {output_file}")
    except Exception as e:
        print(f"Error saving spectrum: {e}")
    
    # Test fluent API
    try:
        spectrum_fluent = (
            xt.XASSpectrum(energy=energy, mu=mu)
            .normalize()
            .pre_range(-200, -30)
            .norm_range(100, 600)
            .autobk()
            .rbkg(1.0)
            .k_range(0, 15)
            .xftf()
            .k_range(2, 12)
            .kweight(2)
            .window('hanning')
            .run()
        )
        print("Fluent API works successfully")
    except Exception as e:
        print(f"Error using fluent API: {e}")
    
    return spectrum

# Test fitting module
def test_fitting(spectrum):
    print("\n=== Testing Fitting module ===")
    
    if spectrum is None:
        print("Spectrum is None, cannot test fitting")
        return
    
    if spectrum.k is None or spectrum.chi is None:
        print("Spectrum k or chi is None, cannot test fitting")
        return
    
    # Create parameters
    try:
        params = xt.FittingParameters()
        params.add("amp", 0.8, vary=True, min=0, max=2.0)
        params.add("r", 2.5, vary=True, min=1.0, max=5.0)
        params.add("phase", 0.0, vary=True, min=-np.pi, max=np.pi)
        params.add("sigma2", 0.004, vary=True, min=0.0, max=0.05)
        print("Created fitting parameters successfully")
    except Exception as e:
        print(f"Error creating fitting parameters: {e}")
        return
    
    # Create path
    try:
        path = xt.SimplePath(
            amp_param="amp",
            r_param="r",
            phase_param="phase",
            sigma2_param="sigma2"
        )
        print("Created path successfully")
    except Exception as e:
        print(f"Error creating path: {e}")
        return
    
    # Create dataset
    try:
        dataset = xt.FittingDataset(k=spectrum.k, chi=spectrum.chi)
        dataset.add_path(path)
        dataset.kweight(2.0)
        dataset.k_range(2.0, 12.0)
        dataset.window("hanning")
        print("Created fitting dataset successfully")
    except Exception as e:
        print(f"Error creating fitting dataset: {e}")
        return
    
    # Test model calculation
    try:
        model_chi = dataset.calc_model_chi(params)
        print("Calculated model chi successfully")
    except Exception as e:
        print(f"Error calculating model chi: {e}")
        return
    
    # Test fitting
    try:
        fitter = xt.ExafsFitter(dataset=dataset, params=params)
        result = fitter.fit()
        print("Performed fitting successfully")
        print(f"Amplitude = {result.params['amp'].value:.4f}")
        print(f"Distance = {result.params['r'].value:.4f} Å")
        print(f"Phase = {result.params['phase'].value:.4f}")
        print(f"Sigma2 = {result.params['sigma2'].value:.6f}")
        print(f"R-factor = {result.r_factor:.6f}")
    except Exception as e:
        print(f"Error performing fitting: {e}")
        return
    
    return result

# Test multi-spectrum fitting
def test_multispectrum_fitting():
    print("\n=== Testing Multi-Spectrum Fitting ===")
    
    # Load spectra
    spectra = []
    for i in range(3):
        try:
            data = np.loadtxt(test_data_dir / f"spectrum_{i+1}.dat")
            energy = data[:, 0]
            mu = data[:, 1]
            
            # Create and process spectrum
            spectrum = xt.XASSpectrum(energy=energy, mu=mu, name=f"Spectrum {i+1}")
            spectrum.normalize()
            spectrum.autobk()
            
            spectra.append(spectrum)
            print(f"Loaded and processed spectrum {i+1}")
        except Exception as e:
            print(f"Error loading spectrum {i+1}: {e}")
            return
    
    # Create fitting datasets
    datasets = []
    for i, spectrum in enumerate(spectra):
        try:
            # Create path with spectrum-specific parameter names
            path = xt.SimplePath(
                amp_param=f"amp_{i+1}",
                r_param="r",
                phase_param="phase",
                sigma2_param=f"sigma2_{i+1}",
                degeneracy=1.0
            )
            
            # Create dataset
            dataset = xt.FittingDataset(k=spectrum.k, chi=spectrum.chi)
            dataset.add_path(path)
            dataset.kweight(2.0)
            dataset.k_range(2.0, 12.0)
            
            datasets.append(dataset)
            print(f"Created fitting dataset for spectrum {i+1}")
        except Exception as e:
            print(f"Error creating dataset for spectrum {i+1}: {e}")
            return
    
    # Test ConstrainedParameters
    try:
        # Create parameters with constraints
        params = xt.ConstrainedParameters()
        
        # Base parameters for spectrum 1
        params.add("amp_1", 0.8, vary=True, min=0.0, max=2.0)
        params.add("sigma2_1", 0.004, vary=True, min=0.0, max=0.05)
        
        # Common parameters for all spectra
        params.add("r", 2.5, vary=True, min=1.0, max=5.0)
        params.add("phase", 0.0, vary=True, min=-np.pi, max=np.pi)
        
        # Constraint parameters
        params.add("amp_scale_2", 0.94, vary=True, min=0.5, max=1.5)
        params.add("amp_scale_3", 0.88, vary=True, min=0.5, max=1.5)
        params.add("delta_sigma2_2", 0.0005, vary=True, min=0.0, max=0.01)
        params.add("delta_sigma2_3", 0.001, vary=True, min=0.0, max=0.01)
        
        # Add constrained parameters
        constraint = xt.ParameterConstraint.scale("amp_1", 0.94)
        params.add("amp_2", 0.0, constraint=constraint)
        
        constraint = xt.ParameterConstraint.scale("amp_1", 0.88)
        params.add("amp_3", 0.0, constraint=constraint)
        
        constraint = xt.ParameterConstraint.offset("sigma2_1", 0.0005)
        params.add("sigma2_2", 0.0, constraint=constraint)
        
        constraint = xt.ParameterConstraint.offset("sigma2_1", 0.001)
        params.add("sigma2_3", 0.0, constraint=constraint)
        
        # Apply constraints
        params.apply_constraints()
        
        print("Created and applied parameter constraints successfully")
    except Exception as e:
        print(f"Error creating parameter constraints: {e}")
        return
    
    # Create multi-spectrum dataset
    try:
        multi_dataset = xt.MultiSpectrumDataset()
        for dataset in datasets:
            multi_dataset.add_dataset(dataset)
        multi_dataset.params(params)
        print("Created multi-spectrum dataset successfully")
    except Exception as e:
        print(f"Error creating multi-spectrum dataset: {e}")
        return
    
    # Test model calculation
    try:
        models = multi_dataset.calc_all_models()
        print("Calculated all models successfully")
    except Exception as e:
        print(f"Error calculating all models: {e}")
        return
    
    # Test fitting
    try:
        fitter = xt.MultiSpectrumFitter(dataset=multi_dataset)
        result = fitter.fit()
        print("Performed multi-spectrum fitting successfully")
        print(f"Base amplitude = {result.params['amp_1'].value:.4f}")
        print(f"Base sigma2 = {result.params['sigma2_1'].value:.6f}")
        print(f"Distance = {result.params['r'].value:.4f} Å")
        print(f"Amplitude scale 2 = {result.params['amp_scale_2'].value:.4f}")
        print(f"Amplitude scale 3 = {result.params['amp_scale_3'].value:.4f}")
        print(f"Delta sigma2 2 = {result.params['delta_sigma2_2'].value:.6f}")
        print(f"Delta sigma2 3 = {result.params['delta_sigma2_3'].value:.6f}")
        
        # Check the R-factors for each spectrum
        for i, r_factor in enumerate(result.r_factors):
            print(f"R-factor for spectrum {i+1} = {r_factor:.6f}")
    except Exception as e:
        print(f"Error performing multi-spectrum fitting: {e}")
        return
    
    return result

# Main function
def main():
    print("Testing Python bindings for XRayTsubaki")
    
    # Test XASSpectrum class
    spectrum = test_xasspectrum()
    
    # Test fitting module
    fit_result = test_fitting(spectrum)
    
    # Test multi-spectrum fitting
    multi_result = test_multispectrum_fitting()
    
    print("\nAll tests completed.")

if __name__ == "__main__":
    main()