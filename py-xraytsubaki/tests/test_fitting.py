#!/usr/bin/env python3
"""
Tests for the EXAFS fitting module in the Python bindings.
These tests define the expected interface and behavior for EXAFS fitting.
"""

import unittest
import numpy as np
import os
import tempfile
import sys

# Add the parent directory to the path so we can import py_xraytsubaki
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
try:
    import py_xraytsubaki as xt
except ImportError:
    # In case the package is not yet built, this test will fail
    raise unittest.SkipTest("py_xraytsubaki not available, skipping tests")

class TestFitting(unittest.TestCase):
    """Test the EXAFS fitting functionality."""
    
    def setUp(self):
        """Create sample data for testing."""
        # Create synthetic EXAFS data for fitting
        # First, create k grid
        self.k = np.linspace(0, 15, 300)
        
        # Create a simple EXAFS model
        # chi(k) = amp * sin(2*k*r + phase) * exp(-sigma2 * k^2)
        self.true_amp = 0.8
        self.true_r = 2.5  # Å
        self.true_phase = 0.3
        self.true_sigma2 = 0.005
        
        self.chi = (
            self.true_amp * 
            np.sin(2.0 * self.k * self.true_r + self.true_phase) * 
            np.exp(-self.true_sigma2 * self.k**2)
        )
        
        # Add some noise
        np.random.seed(12345)
        self.noise = np.random.normal(0, 0.02, len(self.k))
        self.chi_noisy = self.chi + self.noise
    
    def test_parameter_system(self):
        """Test the parameter system for EXAFS fitting."""
        # Create a parameter
        param = xt.FittingParameter(name="amp", value=0.7)
        self.assertEqual(param.name, "amp")
        self.assertAlmostEqual(param.value, 0.7)
        
        # Parameters should have vary, min, max properties
        self.assertTrue(param.vary)  # Default is True
        self.assertIsNone(param.min)
        self.assertIsNone(param.max)
        
        # Set constraints
        param.vary = False
        param.min = 0.0
        param.max = 2.0
        
        self.assertFalse(param.vary)
        self.assertEqual(param.min, 0.0)
        self.assertEqual(param.max, 2.0)
        
        # Create a parameter collection
        params = xt.FittingParameters()
        
        # Add parameters
        params.add("amp", value=0.7, vary=True, min=0.0, max=2.0)
        params.add("r", value=2.3, vary=True, min=1.0, max=5.0)
        params.add("phase", value=0.0, vary=True, min=-np.pi, max=np.pi)
        params.add("sigma2", value=0.003, vary=True, min=0.0, max=0.05)
        
        # Get parameter values
        self.assertEqual(params["amp"].value, 0.7)
        self.assertEqual(params["r"].value, 2.3)
        
        # Get parameter as dict
        param_dict = params.as_dict()
        self.assertEqual(param_dict["amp"], 0.7)
        
        # Test fluent API
        params_fluent = (
            xt.FittingParameters()
            .add("amp", value=0.7, vary=True, min=0.0, max=2.0)
            .add("r", value=2.3, vary=True, min=1.0, max=5.0)
        )
        self.assertEqual(params_fluent["amp"].value, 0.7)
    
    def test_path_model(self):
        """Test path model for EXAFS fitting."""
        # Create a simple path model
        path = xt.SimplePath(
            amp_param="amp",
            r_param="r",
            phase_param="phase",
            sigma2_param="sigma2",
            degeneracy=1.0
        )
        
        # Create parameters
        params = xt.FittingParameters()
        params.add("amp", value=0.8)
        params.add("r", value=2.5)
        params.add("phase", value=0.3)
        params.add("sigma2", value=0.005)
        
        # Calculate chi for the path
        k_test = np.linspace(2, 12, 100)
        chi_test = path.calc_chi(params, k_test)
        
        # Check result
        self.assertEqual(len(chi_test), len(k_test))
        
        # Calculate expected chi manually
        expected_chi = (
            0.8 * 
            np.sin(2.0 * k_test * 2.5 + 0.3) * 
            np.exp(-0.005 * k_test**2)
        )
        
        # Compare
        np.testing.assert_allclose(chi_test, expected_chi, rtol=1e-5)
    
    def test_feff_path(self):
        """Test FEFF path for EXAFS fitting."""
        # In a real implementation, this would load a FEFF path from a file
        # For testing, we'll mock it
        feff_path = xt.FeffPath(filename=None)
        feff_path.s02_param = "amp"
        feff_path.e0_param = "e0"
        feff_path.deltar_param = "deltar"
        feff_path.sigma2_param = "sigma2"
        
        # Set mock amp and phase arrays (normally from FEFF)
        # These are simplified for testing
        k_test = np.linspace(2, 12, 100)
        feff_path.k = k_test
        feff_path.amp = np.ones_like(k_test)
        feff_path.phase = 2.0 * k_test * 2.5
        
        # Create parameters
        params = xt.FittingParameters()
        params.add("amp", value=0.8)
        params.add("e0", value=0.0)
        params.add("deltar", value=0.0)
        params.add("sigma2", value=0.005)
        
        # Calculate chi for the path
        chi_test = feff_path.calc_chi(params, k_test)
        
        # Check result
        self.assertEqual(len(chi_test), len(k_test))
    
    def test_fitting_dataset(self):
        """Test creating and manipulating fitting datasets."""
        # Create a fitting dataset
        dataset = xt.FittingDataset(k=self.k, chi=self.chi_noisy)
        
        # Set properties
        dataset.kweight = 2.0
        dataset.k_range = (2.0, 12.0)
        dataset.window = "hanning"
        
        # Add a path
        path = xt.SimplePath(
            amp_param="amp",
            r_param="r",
            phase_param="phase",
            sigma2_param="sigma2",
            degeneracy=1.0
        )
        dataset.add_path(path)
        
        # Check dataset properties
        self.assertEqual(dataset.kweight, 2.0)
        self.assertEqual(dataset.k_range, (2.0, 12.0))
        self.assertEqual(dataset.window, "hanning")
        self.assertEqual(len(dataset.paths), 1)
        
        # Calculate model chi with parameters
        params = xt.FittingParameters()
        params.add("amp", value=0.8)
        params.add("r", value=2.5)
        params.add("phase", value=0.3)
        params.add("sigma2", value=0.005)
        
        model_chi = dataset.calc_model_chi(params)
        
        # Check result
        self.assertEqual(len(model_chi), len(self.k))
        
        # Test fluent API
        dataset_fluent = (
            xt.FittingDataset(k=self.k, chi=self.chi_noisy)
            .kweight(2.0)
            .k_range(2.0, 12.0)
            .window("hanning")
            .add_path(path)
        )
        self.assertEqual(dataset_fluent.kweight, 2.0)
    
    def test_exafs_fitter(self):
        """Test EXAFS fitting with Levenberg-Marquardt optimization."""
        # Create a dataset with one path
        dataset = xt.FittingDataset(k=self.k, chi=self.chi_noisy)
        dataset.kweight = 2.0
        dataset.k_range = (2.0, 12.0)
        dataset.window = "hanning"
        
        path = xt.SimplePath(
            amp_param="amp",
            r_param="r",
            phase_param="phase",
            sigma2_param="sigma2",
            degeneracy=1.0
        )
        dataset.add_path(path)
        
        # Create initial parameters (starting point for the fit)
        params = xt.FittingParameters()
        params.add("amp", value=0.7, vary=True, min=0.0, max=2.0)
        params.add("r", value=2.3, vary=True, min=1.0, max=5.0)
        params.add("phase", value=0.0, vary=True, min=-np.pi, max=np.pi)
        params.add("sigma2", value=0.003, vary=True, min=0.0, max=0.05)
        
        # Create a fitter
        fitter = xt.ExafsFitter(dataset=dataset, params=params)
        
        # Run the fit
        result = fitter.fit()
        
        # Check properties of the result
        self.assertTrue(hasattr(result, 'params'))
        self.assertTrue(hasattr(result, 'chisqr'))
        self.assertTrue(hasattr(result, 'redchi'))
        self.assertTrue(hasattr(result, 'r_factor'))
        self.assertTrue(hasattr(result, 'model_chi'))
        
        # Check final parameters are close to true values
        self.assertAlmostEqual(result.params["amp"].value, self.true_amp, delta=0.05)
        self.assertAlmostEqual(result.params["r"].value, self.true_r, delta=0.05)
        self.assertAlmostEqual(result.params["phase"].value, self.true_phase, delta=0.1)
        self.assertAlmostEqual(result.params["sigma2"].value, self.true_sigma2, delta=0.001)
        
        # Test fluent API
        fitter_fluent = (
            xt.ExafsFitter(dataset=dataset)
            .params(params)
            .fit()
        )
        self.assertAlmostEqual(fitter_fluent.params["amp"].value, 
                              result.params["amp"].value, delta=0.001)


if __name__ == '__main__':
    unittest.main()