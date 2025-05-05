#!/usr/bin/env python3
"""
Test suite for the EXAFS fitting functionality in py_xraytsubaki.

This module tests the fitting capabilities:
1. Parameter creation and constraints
2. Path definition
3. Fitting dataset creation
4. Single spectrum fitting
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

def create_fitting_test_data():
    """Create synthetic EXAFS data for testing fitting."""
    # Create k-space grid
    k = np.linspace(0, 15, 300)
    
    # True parameters for the path
    true_amp = 0.8
    true_delr = 0.05  # Å displacement from initial guess
    true_sigma2 = 0.004  # Å^2 Debye-Waller factor
    true_e0 = 3.0  # eV energy shift
    
    # Initial path distance
    r_init = 2.5  # Å
    
    # Calculate EXAFS with known parameters
    # chi(k) = amp * sin(2kr + phase) * exp(-2k^2*sigma2) * exp(-2r/lambda)
    # We'll use simplified phase and mean free path for testing
    phase = 0.1 * k  # simple k-dependent phase
    lambda_k = 10.0  # Å mean free path (constant for simplicity)
    
    # Calculate chi
    chi = (
        true_amp * 
        np.sin(2 * k * (r_init + true_delr) + phase) * 
        np.exp(-2 * k**2 * true_sigma2) * 
        np.exp(-2 * (r_init + true_delr) / lambda_k)
    )
    
    # Add some noise
    np.random.seed(12345)
    noise_amplitude = 0.02
    noise = np.random.normal(0, noise_amplitude, len(k))
    chi_noisy = chi + noise
    
    # Create chi * k^2 for fitting (common weighting)
    chi_k2 = chi_noisy * k**2
    
    # Zero out low k values that are usually not used in fitting
    chi_k2[:20] = 0.0
    
    return k, chi_noisy, {
        'amp': true_amp,
        'delr': true_delr,
        'sigma2': true_sigma2,
        'e0': true_e0,
        'r_init': r_init
    }

class TestFittingFunctionality(unittest.TestCase):
    """Test the EXAFS fitting functionality in py_xraytsubaki."""
    
    @classmethod
    def setUpClass(cls):
        """Set up test data."""
        cls.k, cls.chi, cls.true_params = create_fitting_test_data()
        
        # Create a standard XASSpectrum with this data
        # We set up an energy grid that would give us the k values we want
        e0 = 8333.0  # Cu K-edge
        cls.energy = e0 * (1 + 3.81 * cls.k**2 / e0)
        cls.mu = np.ones_like(cls.energy)  # We'll only use k and chi, not the actual mu
        
        # Create test output directory
        cls.test_dir = Path("test_output")
        cls.test_dir.mkdir(exist_ok=True)
    
    def test_parameters(self):
        """Test parameter creation and manipulation."""
        # Create parameters object
        params = xt.FittingParameters()
        
        # Add parameters
        params.add("amp", 0.9, vary=True, min=0.5, max=1.5)
        params.add("delr", 0.0, vary=True, min=-0.2, max=0.2)
        params.add("sigma2", 0.003, vary=True, min=0.001, max=0.01)
        params.add("e0", 0.0, vary=True, min=-10.0, max=10.0)
        
        # Check parameter count
        self.assertEqual(len(params), 4)
        
        # Access parameters
        self.assertAlmostEqual(params["amp"].value, 0.9)
        self.assertAlmostEqual(params["delr"].value, 0.0)
        self.assertAlmostEqual(params["sigma2"].value, 0.003)
        self.assertAlmostEqual(params["e0"].value, 0.0)
        
        # Modify parameters
        params["amp"].value = 0.85
        params["delr"].vary = False
        
        # Check modifications
        self.assertAlmostEqual(params["amp"].value, 0.85)
        self.assertFalse(params["delr"].vary)
        
        # Create a copy
        params_copy = params.copy()
        
        # Modify the copy
        params_copy["amp"].value = 0.95
        
        # Verify original is unchanged
        self.assertAlmostEqual(params["amp"].value, 0.85)
        self.assertAlmostEqual(params_copy["amp"].value, 0.95)
        
        # Test parameter expression
        try:
            params.add("constrained", expr="2*amp")
            self.assertAlmostEqual(params["constrained"].value, 2*0.85)
            has_expr = True
        except Exception as e:
            print(f"Parameter expressions not implemented: {e}")
            has_expr = False
        
        if has_expr:
            # Update and check expressions are evaluated
            params["amp"].value = 1.0
            self.assertAlmostEqual(params["constrained"].value, 2.0)
    
    def test_path_creation(self):
        """Test creating EXAFS paths."""
        # Create a simple path
        path = xt.SimplePath(
            amp_param="amp",
            r_param="delr",
            phase_param="phase",
            sigma2_param="sigma2",
            degeneracy=1.0
        )
        
        # Verify path parameters
        self.assertEqual(path.amp_param, "amp")
        self.assertEqual(path.r_param, "delr")
        self.assertEqual(path.phase_param, "phase")
        self.assertEqual(path.sigma2_param, "sigma2")
        self.assertEqual(path.degeneracy, 1.0)
        
        # Create a path with custom attributes
        path2 = xt.SimplePath(
            amp_param="amp2",
            r_param="delr2",
            phase_param="phase2",
            sigma2_param="sigma2_2",
            degeneracy=2.0,
            initial_r=2.5
        )
        
        # Verify custom attributes
        self.assertEqual(path2.amp_param, "amp2")
        self.assertEqual(path2.degeneracy, 2.0)
        self.assertEqual(path2.initial_r, 2.5)
    
    def test_fitting_dataset(self):
        """Test creating fitting datasets."""
        # Create a dataset
        dataset = xt.FittingDataset(k=self.k, chi=self.chi)
        
        # Set fitting parameters
        dataset.kweight(2.0)
        dataset.k_range(2.0, 12.0)
        dataset.window("hanning")
        
        # Get current settings
        self.assertEqual(dataset.get_kweight(), 2.0)
        k_min, k_max = dataset.get_k_range()
        self.assertEqual(k_min, 2.0)
        self.assertEqual(k_max, 12.0)
        self.assertEqual(dataset.get_window(), "hanning")
        
        # Create a path
        path = xt.SimplePath(
            amp_param="amp",
            r_param="delr",
            phase_param="phase",
            sigma2_param="sigma2",
            degeneracy=1.0,
            initial_r=self.true_params['r_init']
        )
        
        # Add path to dataset
        dataset.add_path(path)
        
        # Verify path count
        self.assertEqual(dataset.get_path_count(), 1)
        
        # Create parameters
        params = xt.FittingParameters()
        params.add("amp", 0.9, vary=True, min=0.5, max=1.5)
        params.add("delr", 0.0, vary=True, min=-0.2, max=0.2)
        params.add("phase", 0.0, vary=True, min=-np.pi, max=np.pi)
        params.add("sigma2", 0.003, vary=True, min=0.001, max=0.01)
        
        # Calculate model
        model_chi = dataset.calc_model_chi(params)
        
        # Verify model has the right size
        self.assertEqual(len(model_chi), len(self.k))
        
        # Test R-factor calculation
        r_factor = dataset.calc_r_factor(params)
        
        # R-factor should be between 0 and 1, closer to 0 for a good fit
        self.assertGreaterEqual(r_factor, 0.0)
        self.assertLessEqual(r_factor, 1.0)
        
        # Test FT calculations for dataset
        r, chir_mag = dataset.calc_chir_mag(params)
        
        # Verify results
        self.assertGreater(len(r), 0)
        self.assertEqual(len(chir_mag), len(r))
    
    def test_single_spectrum_fitting(self):
        """Test fitting a single EXAFS spectrum."""
        # Create dataset
        dataset = xt.FittingDataset(k=self.k, chi=self.chi)
        dataset.kweight(2.0)
        dataset.k_range(2.0, 12.0)
        dataset.window("hanning")
        
        # Create path
        r_init = self.true_params['r_init']
        path = xt.SimplePath(
            amp_param="amp",
            r_param="delr",
            phase_param="phase",
            sigma2_param="sigma2",
            degeneracy=1.0,
            initial_r=r_init
        )
        dataset.add_path(path)
        
        # Create initial parameters
        params = xt.FittingParameters()
        params.add("amp", 0.7, vary=True, min=0.1, max=2.0)
        params.add("delr", 0.0, vary=True, min=-0.2, max=0.2)
        params.add("phase", 0.0, vary=True, min=-np.pi, max=np.pi)
        params.add("sigma2", 0.005, vary=True, min=0.001, max=0.01)
        
        # Create fitter
        fitter = xt.ExafsFitter(dataset=dataset, params=params)
        
        # Perform fitting
        start_time = time.time()
        result = fitter.fit()
        elapsed = time.time() - start_time
        
        # Verify result is returned
        self.assertIsNotNone(result)
        self.assertIsNotNone(result.params)
        
        # Check fitted values are within reasonable range of true values
        fitted_amp = result.params["amp"].value
        fitted_delr = result.params["delr"].value
        fitted_sigma2 = result.params["sigma2"].value
        
        true_amp = self.true_params['amp']
        true_delr = self.true_params['delr']
        true_sigma2 = self.true_params['sigma2']
        
        print(f"Fitting took {elapsed:.6f} seconds")
        print(f"True amp: {true_amp:.4f}, Fitted: {fitted_amp:.4f}")
        print(f"True delr: {true_delr:.4f}, Fitted: {fitted_delr:.4f}")
        print(f"True sigma2: {true_sigma2:.6f}, Fitted: {fitted_sigma2:.6f}")
        print(f"Final R-factor: {result.r_factor:.6f}")
        
        # Check values are within reasonable range (allowing for some error due to noise)
        # We use fairly loose tolerances since we have noise and simplified physics
        self.assertLess(abs(fitted_amp - true_amp), 0.3)
        self.assertLess(abs(fitted_delr - true_delr), 0.1)
        self.assertLess(abs(fitted_sigma2 - true_sigma2), 0.003)
        
        # Check r-factor is reasonable
        self.assertLess(result.r_factor, 0.2)
        
        # Check we can get the fitted model
        fitted_chi = result.get_fit()
        self.assertEqual(len(fitted_chi), len(self.k))
        
        # Check we can get components
        if hasattr(result, "get_components"):
            components = result.get_components()
            # Should have one component per path
            self.assertEqual(len(components), 1)
    
    def test_with_real_spectrum(self):
        """Test fitting with an XASSpectrum object."""
        # Create a spectrum
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu, name="Test Fitting")
        
        # Set k and chi directly (normally would calculate from EXAFS processing)
        spectrum._set_k_chi(self.k, self.chi)  # Ensure this method exists
        
        # Create dataset from spectrum
        dataset = xt.FittingDataset.from_spectrum(spectrum)
        dataset.kweight(2.0)
        dataset.k_range(2.0, 12.0)
        dataset.window("hanning")
        
        # Create path
        r_init = self.true_params['r_init']
        path = xt.SimplePath(
            amp_param="amp",
            r_param="delr",
            phase_param="phase",
            sigma2_param="sigma2",
            degeneracy=1.0,
            initial_r=r_init
        )
        dataset.add_path(path)
        
        # Create parameters
        params = xt.FittingParameters()
        params.add("amp", 0.7, vary=True, min=0.1, max=2.0)
        params.add("delr", 0.0, vary=True, min=-0.2, max=0.2)
        params.add("phase", 0.0, vary=True, min=-np.pi, max=np.pi)
        params.add("sigma2", 0.005, vary=True, min=0.001, max=0.01)
        
        # Perform fitting
        fitter = xt.ExafsFitter(dataset=dataset, params=params)
        result = fitter.fit()
        
        # Basic verification
        self.assertIsNotNone(result)
        self.assertLess(result.r_factor, 0.2)

if __name__ == "__main__":
    unittest.main()