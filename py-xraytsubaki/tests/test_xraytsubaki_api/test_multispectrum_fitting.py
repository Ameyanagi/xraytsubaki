#!/usr/bin/env python3
"""
Test suite for multi-spectrum EXAFS fitting in py_xraytsubaki.

This module tests:
1. Parameter constraints
2. Multi-spectrum dataset creation
3. Fitting multiple spectra with shared parameters
"""

import os
import sys
import time
import numpy as np
import matplotlib.pyplot as plt
from pathlib import Path
import unittest

# Try to import the Python bindings
try:
    import py_xraytsubaki as xt
    print("Successfully imported py_xraytsubaki")
except ImportError as e:
    print(f"Error importing py_xraytsubaki: {e}")
    print("You might need to build the Python bindings first.")
    sys.exit(1)

def create_multispectrum_test_data(num_spectra=3):
    """Create synthetic EXAFS data for multiple spectra with related parameters."""
    # Create k-space grid
    k = np.linspace(0, 15, 300)
    
    # True parameters
    # Base values for spectrum 1
    base_amp = 0.8
    base_r = 2.5  # Å
    base_sigma2 = 0.004  # Å^2
    
    # Systematic changes for spectra 2 and 3
    amp_scales = [1.0, 0.9, 0.8]  # Decreasing amplitude
    r_shifts = [0.00, 0.02, 0.04]  # Increasing distance
    sigma2_shifts = [0.000, 0.001, 0.002]  # Increasing disorder
    
    spectra = []
    params = []
    
    for i in range(num_spectra):
        # Calculate parameters for this spectrum
        amp = base_amp * amp_scales[i]
        r = base_r + r_shifts[i]
        sigma2 = base_sigma2 + sigma2_shifts[i]
        
        # Store true params for testing
        params.append({
            'amp': amp,
            'r': r,
            'sigma2': sigma2
        })
        
        # Calculate EXAFS with these parameters
        # chi(k) = amp * sin(2kr + phase) * exp(-2k^2*sigma2) * exp(-2r/lambda)
        phase = 0.1 * k  # simple k-dependent phase
        lambda_k = 10.0  # Å mean free path (constant for simplicity)
        
        chi = (
            amp * 
            np.sin(2 * k * r + phase) * 
            np.exp(-2 * k**2 * sigma2) * 
            np.exp(-2 * r / lambda_k)
        )
        
        # Add some noise
        np.random.seed(12345 + i)
        noise_amplitude = 0.02
        noise = np.random.normal(0, noise_amplitude, len(k))
        chi_noisy = chi + noise
        
        # Create energies that would correspond to these k values
        e0 = 8333.0  # Cu K-edge
        energy = e0 * (1 + 3.81 * k**2 / e0)
        mu = np.ones_like(energy)  # Dummy mu values
        
        # Create spectrum object
        spectrum = xt.XASSpectrum(energy=energy, mu=mu, name=f"Spectrum {i+1}")
        spectrum._set_k_chi(k, chi_noisy)  # Set k and chi directly
        
        spectra.append(spectrum)
    
    return spectra, params, {
        'base_amp': base_amp,
        'base_r': base_r,
        'base_sigma2': base_sigma2,
        'amp_scales': amp_scales,
        'r_shifts': r_shifts,
        'sigma2_shifts': sigma2_shifts
    }

class TestMultispectrumFitting(unittest.TestCase):
    """Test the multi-spectrum EXAFS fitting functionality in py_xraytsubaki."""
    
    @classmethod
    def setUpClass(cls):
        """Set up test data."""
        cls.spectra, cls.individual_params, cls.relation_params = create_multispectrum_test_data(3)
        
        # Create test output directory
        cls.test_dir = Path("test_output")
        cls.test_dir.mkdir(exist_ok=True)
    
    def test_parameter_constraints(self):
        """Test parameter constraint creation and application."""
        # Create constrained parameters
        params = xt.ConstrainedParameters()
        
        # Add base parameters
        params.add("amp_1", 0.8, vary=True, min=0.5, max=1.5)
        params.add("sigma2_1", 0.004, vary=True, min=0.001, max=0.01)
        
        # Add parameter scales for spectrum 2 and 3
        params.add("amp_scale_2", 0.9, vary=True, min=0.5, max=1.5)
        params.add("amp_scale_3", 0.8, vary=True, min=0.5, max=1.5)
        
        # Add sigma2 offsets
        params.add("dsigma2_2", 0.001, vary=True, min=0.0, max=0.005)
        params.add("dsigma2_3", 0.002, vary=True, min=0.0, max=0.005)
        
        # Add constraints for amp_2 and amp_3
        constraint1 = xt.ParameterConstraint.scale("amp_1", "amp_scale_2")
        params.add("amp_2", 0.0, constraint=constraint1)
        
        constraint2 = xt.ParameterConstraint.scale("amp_1", "amp_scale_3")
        params.add("amp_3", 0.0, constraint=constraint2)
        
        # Add constraints for sigma2_2 and sigma2_3
        constraint3 = xt.ParameterConstraint.offset("sigma2_1", "dsigma2_2")
        params.add("sigma2_2", 0.0, constraint=constraint3)
        
        constraint4 = xt.ParameterConstraint.offset("sigma2_1", "dsigma2_3")
        params.add("sigma2_3", 0.0, constraint=constraint4)
        
        # Apply constraints
        params.apply_constraints()
        
        # Verify constrained values
        self.assertAlmostEqual(params["amp_2"].value, 0.8 * 0.9)
        self.assertAlmostEqual(params["amp_3"].value, 0.8 * 0.8)
        self.assertAlmostEqual(params["sigma2_2"].value, 0.004 + 0.001)
        self.assertAlmostEqual(params["sigma2_3"].value, 0.004 + 0.002)
        
        # Test updating base values updates constrained values
        params["amp_1"].value = 1.0
        params["sigma2_1"].value = 0.005
        params.apply_constraints()
        
        self.assertAlmostEqual(params["amp_2"].value, 1.0 * 0.9)
        self.assertAlmostEqual(params["amp_3"].value, 1.0 * 0.8)
        self.assertAlmostEqual(params["sigma2_2"].value, 0.005 + 0.001)
        self.assertAlmostEqual(params["sigma2_3"].value, 0.005 + 0.002)
        
        # Test updating constraint values
        params["amp_scale_2"].value = 0.95
        params["dsigma2_3"].value = 0.003
        params.apply_constraints()
        
        self.assertAlmostEqual(params["amp_2"].value, 1.0 * 0.95)
        self.assertAlmostEqual(params["sigma2_3"].value, 0.005 + 0.003)
        
        # Test other constraint types
        ratio_constraint = xt.ParameterConstraint.ratio("amp_1", 0.5)
        params.add("half_amp", 0.0, constraint=ratio_constraint)
        
        sum_constraint = xt.ParameterConstraint.sum(["amp_1", "amp_2"], 0.5)
        params.add("weighted_sum", 0.0, constraint=sum_constraint)
        
        params.apply_constraints()
        
        self.assertAlmostEqual(params["half_amp"].value, 1.0 * 0.5)
        self.assertAlmostEqual(params["weighted_sum"].value, 1.0 + (1.0 * 0.95) + 0.5)
    
    def test_multispectrum_dataset(self):
        """Test creating multi-spectrum datasets."""
        # Create individual datasets
        datasets = []
        for i, spectrum in enumerate(self.spectra):
            dataset = xt.FittingDataset.from_spectrum(spectrum)
            dataset.kweight(2.0)
            dataset.k_range(2.0, 12.0)
            dataset.window("hanning")
            
            # Create path with spectrum-specific parameter names
            path = xt.SimplePath(
                amp_param=f"amp_{i+1}",
                r_param=f"r_{i+1}",
                sigma2_param=f"sigma2_{i+1}",
                degeneracy=1.0,
                initial_r=self.relation_params['base_r']
            )
            
            dataset.add_path(path)
            datasets.append(dataset)
        
        # Create multi-spectrum dataset
        multi_dataset = xt.MultiSpectrumDataset()
        
        # Add individual datasets
        for dataset in datasets:
            multi_dataset.add_dataset(dataset)
        
        # Verify dataset count
        self.assertEqual(multi_dataset.get_dataset_count(), 3)
        
        # Create parameters with constraints
        params = xt.ConstrainedParameters()
        
        # Base parameters for spectrum 1
        params.add("amp_1", 0.7, vary=True, min=0.1, max=2.0)
        params.add("r_1", self.relation_params['base_r'], vary=True, min=2.0, max=3.0)
        params.add("sigma2_1", 0.003, vary=True, min=0.001, max=0.01)
        
        # Constraint parameters
        params.add("amp_scale_2", 0.9, vary=True, min=0.5, max=1.5)
        params.add("amp_scale_3", 0.8, vary=True, min=0.5, max=1.5)
        
        params.add("dr_2", 0.02, vary=True, min=0.0, max=0.1)
        params.add("dr_3", 0.04, vary=True, min=0.0, max=0.1)
        
        params.add("dsigma2_2", 0.001, vary=True, min=0.0, max=0.005)
        params.add("dsigma2_3", 0.002, vary=True, min=0.0, max=0.005)
        
        # Set up constraints
        amp2_constraint = xt.ParameterConstraint.scale("amp_1", "amp_scale_2")
        amp3_constraint = xt.ParameterConstraint.scale("amp_1", "amp_scale_3")
        
        r2_constraint = xt.ParameterConstraint.offset("r_1", "dr_2")
        r3_constraint = xt.ParameterConstraint.offset("r_1", "dr_3")
        
        sigma2_2_constraint = xt.ParameterConstraint.offset("sigma2_1", "dsigma2_2")
        sigma2_3_constraint = xt.ParameterConstraint.offset("sigma2_1", "dsigma2_3")
        
        params.add("amp_2", 0.0, constraint=amp2_constraint)
        params.add("amp_3", 0.0, constraint=amp3_constraint)
        
        params.add("r_2", 0.0, constraint=r2_constraint)
        params.add("r_3", 0.0, constraint=r3_constraint)
        
        params.add("sigma2_2", 0.0, constraint=sigma2_2_constraint)
        params.add("sigma2_3", 0.0, constraint=sigma2_3_constraint)
        
        # Apply constraints
        params.apply_constraints()
        
        # Set parameters for multi-dataset
        multi_dataset.params(params)
        
        # Calculate R-factor
        r_factor = multi_dataset.calc_r_factor()
        
        # Verify reasonable R-factor
        self.assertGreaterEqual(r_factor, 0.0)
        self.assertLessEqual(r_factor, 1.0)
        
        # Calculate individual R-factors
        r_factors = multi_dataset.calc_all_r_factors()
        
        # Verify we have R-factors for each dataset
        self.assertEqual(len(r_factors), 3)
        
        # Test model calculation
        models = multi_dataset.calc_all_models()
        
        # Verify we have models for each dataset
        self.assertEqual(len(models), 3)
        self.assertEqual(len(models[0]), len(self.spectra[0].k))
    
    def test_multispectrum_fitting(self):
        """Test fitting multiple spectra simultaneously."""
        # Create individual datasets
        datasets = []
        for i, spectrum in enumerate(self.spectra):
            dataset = xt.FittingDataset.from_spectrum(spectrum)
            dataset.kweight(2.0)
            dataset.k_range(3.0, 12.0)
            dataset.window("hanning")
            
            # Create path with spectrum-specific parameter names
            path = xt.SimplePath(
                amp_param=f"amp_{i+1}",
                r_param=f"r_{i+1}",
                sigma2_param=f"sigma2_{i+1}",
                degeneracy=1.0,
                initial_r=self.relation_params['base_r']
            )
            
            dataset.add_path(path)
            datasets.append(dataset)
        
        # Create multi-spectrum dataset
        multi_dataset = xt.MultiSpectrumDataset()
        
        # Add individual datasets
        for dataset in datasets:
            multi_dataset.add_dataset(dataset)
        
        # Create parameters with constraints (initial guesses different from true values)
        params = xt.ConstrainedParameters()
        
        # Base parameters for spectrum 1
        params.add("amp_1", 0.7, vary=True, min=0.1, max=2.0)
        params.add("r_1", 2.45, vary=True, min=2.0, max=3.0)
        params.add("sigma2_1", 0.003, vary=True, min=0.001, max=0.01)
        
        # Constraint parameters
        params.add("amp_scale_2", 0.95, vary=True, min=0.5, max=1.5)
        params.add("amp_scale_3", 0.85, vary=True, min=0.5, max=1.5)
        
        params.add("dr_2", 0.01, vary=True, min=0.0, max=0.1)
        params.add("dr_3", 0.03, vary=True, min=0.0, max=0.1)
        
        params.add("dsigma2_2", 0.0005, vary=True, min=0.0, max=0.005)
        params.add("dsigma2_3", 0.0015, vary=True, min=0.0, max=0.005)
        
        # Set up constraints
        amp2_constraint = xt.ParameterConstraint.scale("amp_1", "amp_scale_2")
        amp3_constraint = xt.ParameterConstraint.scale("amp_1", "amp_scale_3")
        
        r2_constraint = xt.ParameterConstraint.offset("r_1", "dr_2")
        r3_constraint = xt.ParameterConstraint.offset("r_1", "dr_3")
        
        sigma2_2_constraint = xt.ParameterConstraint.offset("sigma2_1", "dsigma2_2")
        sigma2_3_constraint = xt.ParameterConstraint.offset("sigma2_1", "dsigma2_3")
        
        params.add("amp_2", 0.0, constraint=amp2_constraint)
        params.add("amp_3", 0.0, constraint=amp3_constraint)
        
        params.add("r_2", 0.0, constraint=r2_constraint)
        params.add("r_3", 0.0, constraint=r3_constraint)
        
        params.add("sigma2_2", 0.0, constraint=sigma2_2_constraint)
        params.add("sigma2_3", 0.0, constraint=sigma2_3_constraint)
        
        # Apply constraints
        params.apply_constraints()
        
        # Set parameters for multi-dataset
        multi_dataset.params(params)
        
        # Create fitter
        fitter = xt.MultiSpectrumFitter(dataset=multi_dataset)
        
        # Perform fitting
        start_time = time.time()
        result = fitter.fit()
        elapsed = time.time() - start_time
        
        # Verify result
        self.assertIsNotNone(result)
        self.assertIsNotNone(result.params)
        
        # Check fitted values
        fitted_amp_1 = result.params["amp_1"].value
        fitted_r_1 = result.params["r_1"].value
        fitted_sigma2_1 = result.params["sigma2_1"].value
        
        fitted_amp_scale_2 = result.params["amp_scale_2"].value
        fitted_amp_scale_3 = result.params["amp_scale_3"].value
        
        fitted_dr_2 = result.params["dr_2"].value
        fitted_dr_3 = result.params["dr_3"].value
        
        fitted_dsigma2_2 = result.params["dsigma2_2"].value
        fitted_dsigma2_3 = result.params["dsigma2_3"].value
        
        # Print results
        print(f"Multi-spectrum fitting took {elapsed:.6f} seconds")
        print(f"Base amplitude: {fitted_amp_1:.4f} (true: {self.relation_params['base_amp']:.4f})")
        print(f"Base r: {fitted_r_1:.4f} (true: {self.relation_params['base_r']:.4f})")
        print(f"Base sigma2: {fitted_sigma2_1:.6f} (true: {self.relation_params['base_sigma2']:.6f})")
        print(f"Amp scale 2: {fitted_amp_scale_2:.4f} (true: {self.relation_params['amp_scales'][1]:.4f})")
        print(f"Amp scale 3: {fitted_amp_scale_3:.4f} (true: {self.relation_params['amp_scales'][2]:.4f})")
        print(f"Delta r 2: {fitted_dr_2:.4f} (true: {self.relation_params['r_shifts'][1]:.4f})")
        print(f"Delta r 3: {fitted_dr_3:.4f} (true: {self.relation_params['r_shifts'][2]:.4f})")
        print(f"Delta sigma2 2: {fitted_dsigma2_2:.6f} (true: {self.relation_params['sigma2_shifts'][1]:.6f})")
        print(f"Delta sigma2 3: {fitted_dsigma2_3:.6f} (true: {self.relation_params['sigma2_shifts'][2]:.6f})")
        
        # Check values are close to true values (with relaxed tolerances due to noise)
        self.assertLess(abs(fitted_amp_1 - self.relation_params['base_amp']), 0.2)
        self.assertLess(abs(fitted_r_1 - self.relation_params['base_r']), 0.1)
        self.assertLess(abs(fitted_sigma2_1 - self.relation_params['base_sigma2']), 0.002)
        
        self.assertLess(abs(fitted_amp_scale_2 - self.relation_params['amp_scales'][1]), 0.15)
        self.assertLess(abs(fitted_amp_scale_3 - self.relation_params['amp_scales'][2]), 0.15)
        
        self.assertLess(abs(fitted_dr_2 - self.relation_params['r_shifts'][1]), 0.02)
        self.assertLess(abs(fitted_dr_3 - self.relation_params['r_shifts'][2]), 0.02)
        
        self.assertLess(abs(fitted_dsigma2_2 - self.relation_params['sigma2_shifts'][1]), 0.002)
        self.assertLess(abs(fitted_dsigma2_3 - self.relation_params['sigma2_shifts'][2]), 0.002)
        
        # Check R-factors
        self.assertLess(result.r_factor, 0.2)  # Overall R-factor
        for r_factor in result.r_factors:
            self.assertLess(r_factor, 0.3)  # Individual R-factors
        
        # Get fitted models
        models = result.get_fits()
        self.assertEqual(len(models), 3)
        
        # Check models match k-space
        for i, model in enumerate(models):
            self.assertEqual(len(model), len(self.spectra[i].k))

if __name__ == "__main__":
    unittest.main()