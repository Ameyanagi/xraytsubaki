#!/usr/bin/env python3
"""
Tests for the basic XAS functions exposed in the Python bindings.
These tests define the expected interface and behavior.
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

class TestXASFunctions(unittest.TestCase):
    """Test basic XAS functions like normalization, background removal, etc."""
    
    def setUp(self):
        """Create sample data for the tests."""
        # Create energy grid
        self.energy = np.linspace(17000, 18000, 1000)
        
        # Create a synthetic XAFS spectrum
        e0 = 17500
        pre_edge = 1.0 + 0.01 * (self.energy - 17000)
        edge_step = 1.0
        
        # Step function at e0 with some broadening
        edge = edge_step * 0.5 * (1 + np.tanh((self.energy - e0) / 10.0))
        
        # Add some oscillations after the edge
        k = np.sqrt((self.energy[self.energy > e0] - e0) / 3.81)
        oscillations = np.zeros_like(self.energy)
        oscillations[self.energy > e0] = 0.3 * np.sin(5.0 * k) * np.exp(-0.05 * k**2)
        
        # Combine to make the final mu
        self.mu = pre_edge + edge + oscillations
        
    def test_find_e0(self):
        """Test finding the absorption edge energy."""
        # Define the expected method signature and behavior
        e0 = xt.find_e0(self.energy, self.mu)
        
        # Check that e0 is close to the expected value
        self.assertAlmostEqual(e0, 17500, delta=20)
        
        # Test the fluent API with method chaining
        e0_fluent = xt.find_e0(energy=self.energy, mu=self.mu)
        self.assertAlmostEqual(e0_fluent, 17500, delta=20)
    
    def test_pre_edge(self):
        """Test pre-edge subtraction functionality."""
        # Define the expected method signature
        result = xt.pre_edge(
            energy=self.energy, 
            mu=self.mu,
            e0=None,  # If None, should be determined automatically
            pre1=-200, 
            pre2=-30,
            norm1=100, 
            norm2=600,
            nnorm=2
        )
        
        # Check that the result contains all the expected outputs
        self.assertIn('e0', result)
        self.assertIn('edge_step', result)
        self.assertIn('norm', result)
        self.assertIn('pre_edge', result)
        self.assertIn('post_edge', result)
        
        # Check values are reasonable
        self.assertAlmostEqual(result['e0'], 17500, delta=20)
        self.assertAlmostEqual(result['edge_step'], 1.0, delta=0.1)
        
        # Normalized spectrum should be around 1.0 after the edge
        post_idx = np.where(self.energy > result['e0'] + 100)[0]
        norm_avg = np.mean(result['norm'][post_idx])
        self.assertAlmostEqual(norm_avg, 1.0, delta=0.1)
        
        # Test fluent API style
        result_fluent = (
            xt.pre_edge()
            .energy(self.energy)
            .mu(self.mu)
            .pre_range(-200, -30)
            .norm_range(100, 600)
            .nnorm(2)
            .run()
        )
        self.assertAlmostEqual(result_fluent['edge_step'], result['edge_step'], delta=0.001)
    
    def test_autobk(self):
        """Test background removal functionality."""
        # First normalize the data
        norm_result = xt.pre_edge(energy=self.energy, mu=self.mu)
        
        # Define the expected method signature for autobk
        result = xt.autobk(
            energy=self.energy,
            mu=norm_result['norm'],
            rbkg=1.0,
            e0=norm_result['e0'],
            kmin=0.0,
            kmax=15.0,
            kweight=2.0,
            dk=0.05,
            window='hanning'
        )
        
        # Check that the result contains all the expected outputs
        self.assertIn('chi', result)
        self.assertIn('k', result)
        self.assertIn('kraw', result)
        self.assertIn('background', result)
        
        # k should start near 0 and end around kmax
        self.assertLess(result['k'][0], 0.1)
        self.assertGreater(result['k'][-1], 14.0)
        
        # background should be smooth (have smaller oscillations)
        chi_amplitude = np.max(np.abs(result['chi']))
        self.assertGreater(chi_amplitude, 0.01)  # chi should have oscillations
        
        # Test fluent API
        result_fluent = (
            xt.autobk()
            .energy(self.energy)
            .mu(norm_result['norm'])
            .e0(norm_result['e0'])
            .rbkg(1.0)
            .k_range(0.0, 15.0)
            .kweight(2.0)
            .dk(0.05)
            .window('hanning')
            .run()
        )
        # Check that the results are the same
        np.testing.assert_allclose(result_fluent['k'], result['k'], rtol=1e-5)
        np.testing.assert_allclose(result_fluent['chi'], result['chi'], rtol=1e-5)
    
    def test_xftf(self):
        """Test forward Fourier transform functionality."""
        # First get normalized data with background removed
        norm_result = xt.pre_edge(energy=self.energy, mu=self.mu)
        bkg_result = xt.autobk(
            energy=self.energy, 
            mu=norm_result['norm'], 
            e0=norm_result['e0']
        )
        
        # Define the expected method signature for xftf
        result = xt.xftf(
            k=bkg_result['k'],
            chi=bkg_result['chi'],
            kmin=2.0,
            kmax=12.0,
            dk=1.0,
            window='hanning',
            kweight=2.0,
            nfft=2048
        )
        
        # Check that the result contains all the expected outputs
        self.assertIn('r', result)
        self.assertIn('chir', result)
        self.assertIn('chir_mag', result)
        self.assertIn('chir_re', result)
        self.assertIn('chir_im', result)
        
        # r should start at 0 and extend to reasonable values
        self.assertLess(result['r'][0], 0.1)
        self.assertGreater(result['r'][-1], 6.0)
        
        # chir_mag should have peaks
        max_mag = np.max(result['chir_mag'])
        self.assertGreater(max_mag, 0.1)
        
        # Test fluent API
        result_fluent = (
            xt.xftf()
            .k(bkg_result['k'])
            .chi(bkg_result['chi'])
            .k_range(2.0, 12.0)
            .dk(1.0)
            .window('hanning')
            .kweight(2.0)
            .nfft(2048)
            .run()
        )
        # Check that the results are the same
        np.testing.assert_allclose(result_fluent['r'], result['r'], rtol=1e-5)
        np.testing.assert_allclose(result_fluent['chir_mag'], result['chir_mag'], rtol=1e-5)
    
    def test_xftr(self):
        """Test reverse Fourier transform functionality."""
        # First get chi(k) and chi(R)
        norm_result = xt.pre_edge(energy=self.energy, mu=self.mu)
        bkg_result = xt.autobk(
            energy=self.energy, 
            mu=norm_result['norm'], 
            e0=norm_result['e0']
        )
        ft_result = xt.xftf(
            k=bkg_result['k'],
            chi=bkg_result['chi'],
            kmin=2.0,
            kmax=12.0
        )
        
        # Define the expected method signature for xftr
        result = xt.xftr(
            r=ft_result['r'],
            chir=ft_result['chir'],
            rmin=1.0,
            rmax=3.0,
            dr=0.1,
            window='hanning',
            kmax_out=15.0
        )
        
        # Check that the result contains all the expected outputs
        self.assertIn('k', result)
        self.assertIn('chiq', result)
        self.assertIn('chiq_re', result)
        self.assertIn('chiq_im', result)
        
        # k should cover the expected range
        self.assertLess(result['k'][0], 0.1)
        self.assertGreater(result['k'][-1], 14.0)
        
        # Test fluent API
        result_fluent = (
            xt.xftr()
            .r(ft_result['r'])
            .chir(ft_result['chir'])
            .r_range(1.0, 3.0)
            .dr(0.1)
            .window('hanning')
            .kmax_out(15.0)
            .run()
        )
        # Check that the results are the same
        np.testing.assert_allclose(result_fluent['k'], result['k'], rtol=1e-5)
        np.testing.assert_allclose(result_fluent['chiq'], result['chiq'], rtol=1e-5)


if __name__ == '__main__':
    unittest.main()