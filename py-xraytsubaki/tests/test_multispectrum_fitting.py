#!/usr/bin/env python3
"""
Tests for the multi-spectrum EXAFS fitting module in the Python bindings.
These tests define the expected interface and behavior for multi-spectrum fitting.
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

class TestMultiSpectrumFitting(unittest.TestCase):
    """Test the multi-spectrum EXAFS fitting functionality."""
    
    def setUp(self):
        """Create sample data for testing multiple spectra."""
        # Create synthetic EXAFS data for multiple spectra
        # This mimics temperature-dependent measurements where amplitude decreases
        # and disorder (sigma2) increases with temperature
        self.k = np.linspace(0, 15, 300)
        
        # Common parameters for all spectra
        self.true_r = 2.5  # Å
        self.true_phase = 0.3
        
        # Parameters that vary across spectra (e.g., different temperatures)
        self.true_amps = [0.8, 0.75, 0.7]  # Decreasing amplitude
        self.true_sigma2s = [0.004, 0.0045, 0.005]  # Increasing disorder
        
        # Create chi(k) for each spectrum
        self.chis = []
        self.chi_noisys = []
        
        np.random.seed(12345)
        for i in range(3):
            amp = self.true_amps[i]
            sigma2 = self.true_sigma2s[i]
            
            chi = (
                amp * 
                np.sin(2.0 * self.k * self.true_r + self.true_phase) * 
                np.exp(-sigma2 * self.k**2)
            )
            
            # Add some noise
            noise = np.random.normal(0, 0.02, len(self.k))
            chi_noisy = chi + noise
            
            self.chis.append(chi)
            self.chi_noisys.append(chi_noisy)
    
    def test_constrained_parameters(self):
        """Test parameter constraints for multi-spectrum fitting."""
        # Create a constrained parameter system
        params = xt.ConstrainedParameters()
        
        # Add base parameters
        params.add("amp_1", value=0.8)
        params.add("sigma2_1", value=0.004)
        params.add("r", value=2.5)
        params.add("phase", value=0.3)
        
        # Add constraint parameters
        params.add("amp_scale_2", value=0.94)  # amp_2 = amp_1 * 0.94
        params.add("amp_scale_3", value=0.88)  # amp_3 = amp_1 * 0.88
        params.add("delta_sigma2_2", value=0.0005)  # sigma2_2 = sigma2_1 + 0.0005
        params.add("delta_sigma2_3", value=0.001)   # sigma2_3 = sigma2_1 + 0.001
        
        # Add constrained parameters
        params.add("amp_2", value=0.0, constraint=xt.ParameterConstraint.scale("amp_1", "amp_scale_2"))
        params.add("amp_3", value=0.0, constraint=xt.ParameterConstraint.scale("amp_1", "amp_scale_3"))
        params.add("sigma2_2", value=0.0, constraint=xt.ParameterConstraint.offset("sigma2_1", "delta_sigma2_2"))
        params.add("sigma2_3", value=0.0, constraint=xt.ParameterConstraint.offset("sigma2_1", "delta_sigma2_3"))
        
        # Apply constraints
        params.apply_constraints()
        
        # Check constrained values
        self.assertAlmostEqual(params["amp_2"].value, 0.8 * 0.94)
        self.assertAlmostEqual(params["amp_3"].value, 0.8 * 0.88)
        self.assertAlmostEqual(params["sigma2_2"].value, 0.004 + 0.0005)
        self.assertAlmostEqual(params["sigma2_3"].value, 0.004 + 0.001)
        
        # Change a base parameter and check that constraints propagate
        params["amp_1"].value = 0.9
        params.apply_constraints()
        
        self.assertAlmostEqual(params["amp_2"].value, 0.9 * 0.94)
        self.assertAlmostEqual(params["amp_3"].value, 0.9 * 0.88)
        
        # Test fluent API
        params_fluent = (
            xt.ConstrainedParameters()
            .add("amp_1", value=0.8)
            .add("sigma2_1", value=0.004)
            .add("amp_scale_2", value=0.94)
            .add("amp_2", value=0.0)
            .constrain("amp_2").scale_from("amp_1", "amp_scale_2")
            .apply_constraints()
        )
        self.assertAlmostEqual(params_fluent["amp_2"].value, 0.8 * 0.94)
    
    def test_multi_spectrum_dataset(self):
        """Test creating and manipulating multi-spectrum datasets."""
        # Create individual datasets
        datasets = []
        for i in range(3):
            dataset = xt.FittingDataset(k=self.k, chi=self.chi_noisys[i])
            dataset.kweight = 2.0
            dataset.k_range = (2.0, 12.0)
            dataset.window = "hanning"
            
            # Add a path with spectrum-specific parameter names
            path = xt.SimplePath(
                amp_param=f"amp_{i+1}",
                r_param="r",
                phase_param="phase",
                sigma2_param=f"sigma2_{i+1}",
                degeneracy=1.0
            )
            dataset.add_path(path)
            datasets.append(dataset)
        
        # Create multi-spectrum dataset
        multi_dataset = xt.MultiSpectrumDataset()
        
        # Add individual datasets
        for dataset in datasets:
            multi_dataset.add_dataset(dataset)
        
        # Check properties
        self.assertEqual(len(multi_dataset.datasets), 3)
        
        # Create constrained parameters
        params = xt.ConstrainedParameters()
        
        # Add base parameters
        params.add("amp_1", value=0.7, vary=True, min=0.0, max=2.0)
        params.add("sigma2_1", value=0.003, vary=True, min=0.0, max=0.05)
        params.add("r", value=2.3, vary=True, min=1.0, max=5.0)
        params.add("phase", value=0.0, vary=True, min=-np.pi, max=np.pi)
        
        # Add constraint parameters
        params.add("amp_scale_2", value=0.94, vary=True, min=0.5, max=1.5)
        params.add("amp_scale_3", value=0.88, vary=True, min=0.5, max=1.5)
        params.add("delta_sigma2_2", value=0.0005, vary=True, min=0.0, max=0.01)
        params.add("delta_sigma2_3", value=0.001, vary=True, min=0.0, max=0.01)
        
        # Add constrained parameters
        params.add("amp_2", value=0.0, constraint=xt.ParameterConstraint.scale("amp_1", "amp_scale_2"))
        params.add("amp_3", value=0.0, constraint=xt.ParameterConstraint.scale("amp_1", "amp_scale_3"))
        params.add("sigma2_2", value=0.0, constraint=xt.ParameterConstraint.offset("sigma2_1", "delta_sigma2_2"))
        params.add("sigma2_3", value=0.0, constraint=xt.ParameterConstraint.offset("sigma2_1", "delta_sigma2_3"))
        
        # Apply constraints
        params.apply_constraints()
        
        # Set parameters for the dataset
        multi_dataset.params = params
        
        # Calculate models
        models = multi_dataset.calc_all_models()
        
        # Check results
        self.assertEqual(len(models), 3)
        for i in range(3):
            self.assertEqual(len(models[i]), len(self.k))
        
        # Test fluent API
        multi_dataset_fluent = (
            xt.MultiSpectrumDataset()
            .add_dataset(datasets[0])
            .add_dataset(datasets[1])
            .add_dataset(datasets[2])
            .params(params)
        )
        self.assertEqual(len(multi_dataset_fluent.datasets), 3)
    
    def test_multi_spectrum_fitter(self):
        """Test multi-spectrum fitting with constrained parameters."""
        # Create individual datasets
        datasets = []
        for i in range(3):
            dataset = xt.FittingDataset(k=self.k, chi=self.chi_noisys[i])
            dataset.kweight = 2.0
            dataset.k_range = (2.0, 12.0)
            dataset.window = "hanning"
            
            # Add a path with spectrum-specific parameter names
            path = xt.SimplePath(
                amp_param=f"amp_{i+1}",
                r_param="r",
                phase_param="phase",
                sigma2_param=f"sigma2_{i+1}",
                degeneracy=1.0
            )
            dataset.add_path(path)
            datasets.append(dataset)
        
        # Create multi-spectrum dataset
        multi_dataset = xt.MultiSpectrumDataset()
        for dataset in datasets:
            multi_dataset.add_dataset(dataset)
        
        # Create constrained parameters
        params = xt.ConstrainedParameters()
        
        # Add base parameters
        params.add("amp_1", value=0.7, vary=True, min=0.0, max=2.0)
        params.add("sigma2_1", value=0.003, vary=True, min=0.0, max=0.05)
        params.add("r", value=2.3, vary=True, min=1.0, max=5.0)
        params.add("phase", value=0.0, vary=True, min=-np.pi, max=np.pi)
        
        # Add constraint parameters
        params.add("amp_scale_2", value=0.94, vary=True, min=0.5, max=1.5)
        params.add("amp_scale_3", value=0.88, vary=True, min=0.5, max=1.5)
        params.add("delta_sigma2_2", value=0.0005, vary=True, min=0.0, max=0.01)
        params.add("delta_sigma2_3", value=0.001, vary=True, min=0.0, max=0.01)
        
        # Add constrained parameters
        params.add("amp_2", value=0.0, constraint=xt.ParameterConstraint.scale("amp_1", "amp_scale_2"))
        params.add("amp_3", value=0.0, constraint=xt.ParameterConstraint.scale("amp_1", "amp_scale_3"))
        params.add("sigma2_2", value=0.0, constraint=xt.ParameterConstraint.offset("sigma2_1", "delta_sigma2_2"))
        params.add("sigma2_3", value=0.0, constraint=xt.ParameterConstraint.offset("sigma2_1", "delta_sigma2_3"))
        
        # Apply constraints
        params.apply_constraints()
        
        # Set parameters for the dataset
        multi_dataset.params = params
        
        # Create fitter
        fitter = xt.MultiSpectrumFitter(dataset=multi_dataset)
        
        # Run the fit
        result = fitter.fit()
        
        # Check properties of the result
        self.assertTrue(hasattr(result, 'params'))
        self.assertTrue(hasattr(result, 'chisqr'))
        self.assertTrue(hasattr(result, 'redchi'))
        self.assertTrue(hasattr(result, 'r_factors'))
        self.assertTrue(hasattr(result, 'model_chis'))
        
        # Check final parameters are close to true values
        self.assertAlmostEqual(result.params["amp_1"].value, self.true_amps[0], delta=0.05)
        self.assertAlmostEqual(result.params["r"].value, self.true_r, delta=0.05)
        self.assertAlmostEqual(result.params["phase"].value, self.true_phase, delta=0.1)
        self.assertAlmostEqual(result.params["sigma2_1"].value, self.true_sigma2s[0], delta=0.001)
        
        # Check constraint parameter values
        self.assertAlmostEqual(result.params["amp_scale_2"].value, 
                              self.true_amps[1] / self.true_amps[0], delta=0.05)
        self.assertAlmostEqual(result.params["delta_sigma2_3"].value, 
                              self.true_sigma2s[2] - self.true_sigma2s[0], delta=0.001)
        
        # Check constrained parameter values
        self.assertAlmostEqual(result.params["amp_2"].value, self.true_amps[1], delta=0.05)
        self.assertAlmostEqual(result.params["sigma2_3"].value, self.true_sigma2s[2], delta=0.001)
        
        # Check model chi values
        for i in range(3):
            self.assertEqual(len(result.model_chis[i]), len(self.k))
        
        # Test fluent API
        result_fluent = (
            xt.MultiSpectrumFitter(dataset=multi_dataset)
            .fit()
        )
        self.assertAlmostEqual(result_fluent.params["amp_1"].value, 
                              result.params["amp_1"].value, delta=0.001)
        
    def test_multi_spectrum_with_spectrum_class(self):
        """Test integration with XASSpectrum class."""
        # Create sample XAS spectra
        spectra = []
        for i in range(3):
            # Create energy grid (all spectra have same grid in this example)
            energy = np.linspace(17000, 18000, 1000)
            
            # Create a synthetic XAFS spectrum
            e0 = 17500
            pre_edge = 1.0 + 0.01 * (energy - 17000)
            edge_step = 1.0
            
            # Step function at e0 with some broadening
            edge = edge_step * 0.5 * (1 + np.tanh((energy - e0) / 10.0))
            
            # Add some oscillations that change with "temperature"
            amp = self.true_amps[i]
            sigma2 = self.true_sigma2s[i]
            
            k_vals = np.zeros_like(energy)
            idx = energy > e0
            k_vals[idx] = np.sqrt((energy[idx] - e0) / 3.81)
            
            oscillations = np.zeros_like(energy)
            oscillations[idx] = amp * np.sin(2.0 * self.true_r * k_vals[idx] + self.true_phase) * np.exp(-sigma2 * k_vals[idx]**2)
            
            # Combine to make the final mu
            mu = pre_edge + edge + oscillations
            
            # Create spectrum
            spectrum = xt.XASSpectrum(energy=energy, mu=mu)
            spectrum.name = f"sample_{i+1}"
            
            # Process spectrum
            spectrum.normalize()
            spectrum.autobk()
            
            spectra.append(spectrum)
        
        # Create a group
        group = xt.XASGroup()
        for spectrum in spectra:
            group.add_spectrum(spectrum)
        
        # Fit all spectra together with constraints
        result = group.fit_spectra(
            kmin=2.0,
            kmax=12.0,
            kweight=2.0,
            paths=[{
                'type': 'simple',
                'amp_param': 'amp_{spectrum_index}',
                'r_param': 'r',
                'phase_param': 'phase',
                'sigma2_param': 'sigma2_{spectrum_index}',
                'degeneracy': 1.0
            }],
            constraints=[
                {'param': 'amp_2', 'type': 'scale', 'reference': 'amp_1', 'factor_param': 'amp_scale_2'},
                {'param': 'amp_3', 'type': 'scale', 'reference': 'amp_1', 'factor_param': 'amp_scale_3'},
                {'param': 'sigma2_2', 'type': 'offset', 'reference': 'sigma2_1', 'offset_param': 'delta_sigma2_2'},
                {'param': 'sigma2_3', 'type': 'offset', 'reference': 'sigma2_1', 'offset_param': 'delta_sigma2_3'}
            ],
            initial_params={
                'amp_1': {'value': 0.7, 'min': 0.0, 'max': 2.0},
                'sigma2_1': {'value': 0.003, 'min': 0.0, 'max': 0.05},
                'r': {'value': 2.3, 'min': 1.0, 'max': 5.0},
                'phase': {'value': 0.0, 'min': -np.pi, 'max': np.pi},
                'amp_scale_2': {'value': 0.94, 'min': 0.5, 'max': 1.5},
                'amp_scale_3': {'value': 0.88, 'min': 0.5, 'max': 1.5},
                'delta_sigma2_2': {'value': 0.0005, 'min': 0.0, 'max': 0.01},
                'delta_sigma2_3': {'value': 0.001, 'min': 0.0, 'max': 0.01}
            }
        )
        
        # Check result
        self.assertIsNotNone(result)
        self.assertTrue(hasattr(result, 'params'))
        self.assertTrue(hasattr(result, 'chisqr'))
        
        # Check final parameters are close to true values
        self.assertAlmostEqual(result.params["amp_1"].value, self.true_amps[0], delta=0.05)
        self.assertAlmostEqual(result.params["r"].value, self.true_r, delta=0.05)
        
        # Check that the fit results were stored back to the spectra
        for i, spectrum in enumerate(spectra):
            self.assertTrue(hasattr(spectrum, 'fit_result'))
            self.assertTrue(hasattr(spectrum, 'model_chi'))
            
        # Test fluent API
        result_fluent = (
            xt.XASGroup()
            .add_spectra(spectra)
            .fit_spectra()
            .krange(2.0, 12.0)
            .kweight(2.0)
            .paths([{
                'type': 'simple',
                'amp_param': 'amp_{spectrum_index}',
                'r_param': 'r',
                'phase_param': 'phase',
                'sigma2_param': 'sigma2_{spectrum_index}',
                'degeneracy': 1.0
            }])
            .add_constraint('amp_2', 'scale', 'amp_1', 'amp_scale_2')
            .add_constraint('amp_3', 'scale', 'amp_1', 'amp_scale_3')
            .add_constraint('sigma2_2', 'offset', 'sigma2_1', 'delta_sigma2_2')
            .add_constraint('sigma2_3', 'offset', 'sigma2_1', 'delta_sigma2_3')
            .initial_params({
                'amp_1': {'value': 0.7, 'min': 0.0, 'max': 2.0},
                'sigma2_1': {'value': 0.003, 'min': 0.0, 'max': 0.05},
                'r': {'value': 2.3, 'min': 1.0, 'max': 5.0},
                'phase': {'value': 0.0, 'min': -np.pi, 'max': np.pi},
                'amp_scale_2': {'value': 0.94, 'min': 0.5, 'max': 1.5},
                'amp_scale_3': {'value': 0.88, 'min': 0.5, 'max': 1.5},
                'delta_sigma2_2': {'value': 0.0005, 'min': 0.0, 'max': 0.01},
                'delta_sigma2_3': {'value': 0.001, 'min': 0.0, 'max': 0.01}
            })
            .run()
        )
        self.assertAlmostEqual(result_fluent.params["amp_1"].value, 
                              result.params["amp_1"].value, delta=0.001)


if __name__ == '__main__':
    unittest.main()