#!/usr/bin/env python3
"""
Test suite for the xraylarch-compatible API in py_xraytsubaki.

This module tests the functional API that's designed to be compatible with xraylarch.
The tests include:
1. Basic spectrum loading and manipulation
2. find_e0
3. pre_edge normalization
4. autobk background removal
5. xftf (forward Fourier transform)
6. xftr (reverse Fourier transform)
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

# Create test data if not exists
def create_test_spectrum():
    """Create a synthetic XAS spectrum for testing."""
    # Energy grid
    energy = np.linspace(17000, 18000, 1000)
    
    # Parameters
    e0 = 17500.0
    pre_edge_slope = 0.01
    edge_step = 1.0
    
    # Create synthetic XAFS spectrum
    # Pre-edge: linear function
    pre_edge = 1.0 + pre_edge_slope * (energy - 17000)
    
    # Edge step: tanh function
    edge = edge_step * 0.5 * (1 + np.tanh((energy - e0) / 10.0))
    
    # EXAFS oscillations
    # Convert E to k
    k_mask = energy > e0
    k = np.zeros_like(energy)
    k[k_mask] = np.sqrt((energy[k_mask] - e0) / 3.81)
    
    # Multiple paths with known parameters for testing
    amp1, freq1, phase1, damp1 = 0.4, 2.5, 0.3, 0.04  # First shell
    amp2, freq2, phase2, damp2 = 0.2, 4.2, 0.6, 0.03  # Second shell
    
    # Create oscillations
    oscil = np.zeros_like(energy)
    oscil[k_mask] = (
        amp1 * np.sin(2 * freq1 * k[k_mask] + phase1) * np.exp(-damp1 * k[k_mask]**2) +
        amp2 * np.sin(2 * freq2 * k[k_mask] + phase2) * np.exp(-damp2 * k[k_mask]**2)
    )
    
    # Total mu
    mu = pre_edge + edge + oscil
    
    # Add some noise
    np.random.seed(12345)
    noise = np.random.normal(0, 0.005, len(energy))
    mu_noisy = mu + noise
    
    return energy, mu_noisy, e0

class TestXraytsubakiLarchAPI(unittest.TestCase):
    """Test the xraylarch-compatible API in the py_xraytsubaki package."""
    
    @classmethod
    def setUpClass(cls):
        """Set up the test class by creating test data."""
        cls.energy, cls.mu, cls.true_e0 = create_test_spectrum()
        
        # Create test directory for saving files
        cls.test_dir = Path("test_output")
        cls.test_dir.mkdir(exist_ok=True)
    
    def test_find_e0(self):
        """Test find_e0 function."""
        # Test with default parameters
        start_time = time.time()
        e0 = xt.find_e0(self.energy, self.mu)
        elapsed = time.time() - start_time
        
        # Verify e0 is close to the true value (within 10 eV)
        self.assertIsNotNone(e0)
        self.assertLess(abs(e0 - self.true_e0), 10.0)
        
        print(f"find_e0 took {elapsed:.6f} seconds")
        print(f"True E0: {self.true_e0:.2f}, Found E0: {e0:.2f}")
    
    def test_pre_edge(self):
        """Test pre_edge function for normalization."""
        # Test with default parameters
        start_time = time.time()
        result = xt.pre_edge(self.energy, self.mu)
        elapsed = time.time() - start_time
        
        # Verify output structure and values
        self.assertIn('e0', result)
        self.assertIn('edge_step', result)
        self.assertIn('norm', result)
        self.assertIn('pre_edge', result)
        self.assertIn('post_edge', result)
        
        # Check e0 is reasonable
        self.assertLess(abs(result['e0'] - self.true_e0), 10.0)
        
        # Check edge_step is close to 1 (our synthetic data has edge_step=1.0)
        self.assertGreater(result['edge_step'], 0.9)
        self.assertLess(result['edge_step'], 1.1)
        
        # Check arrays have correct length
        self.assertEqual(len(result['norm']), len(self.energy))
        self.assertEqual(len(result['pre_edge']), len(self.energy))
        self.assertEqual(len(result['post_edge']), len(self.energy))
        
        # Verify normalized spectrum is between 0 and ~1
        self.assertGreater(np.min(result['norm']), -0.1)
        self.assertLess(np.max(result['norm']), 1.2)
        
        print(f"pre_edge took {elapsed:.6f} seconds")
        print(f"E0: {result['e0']:.2f}, Edge step: {result['edge_step']:.4f}")
        
        # Test with custom parameters
        custom_result = xt.pre_edge(
            self.energy, self.mu, 
            pre1=-200, pre2=-30, 
            norm1=100, norm2=600
        )
        
        # Basic verification
        self.assertIn('norm', custom_result)
        
        # Test that builder pattern works
        builder_result = (
            xt.PreEdgeBuilder()
            .energy(self.energy)
            .mu(self.mu)
            .pre_range(-200, -30)
            .norm_range(100, 600)
            .run(xt._internals.get_python())
        )
        
        self.assertIn('norm', builder_result)
    
    def test_autobk(self):
        """Test autobk function for background removal."""
        # First normalize
        pre_edge_result = xt.pre_edge(self.energy, self.mu)
        
        # Test autobk with default parameters
        start_time = time.time()
        result = xt.autobk(self.energy, pre_edge_result['norm'], pre_edge_result['e0'])
        elapsed = time.time() - start_time
        
        # Verify output structure
        self.assertIn('k', result)
        self.assertIn('chi', result)
        self.assertIn('kraw', result)
        self.assertIn('background', result)
        
        # Check arrays have correct length
        k_length = len(result['k'])
        self.assertGreater(k_length, 0)
        self.assertEqual(len(result['chi']), k_length)
        
        # Check k range starts near 0
        self.assertLess(result['k'][0], 0.5)
        
        # Verify chi has reasonable amplitude (not too large or small)
        chi_max = np.max(np.abs(result['chi']))
        self.assertGreater(chi_max, 0.01)
        self.assertLess(chi_max, 10.0)
        
        print(f"autobk took {elapsed:.6f} seconds")
        print(f"k range: {result['k'][0]:.2f} to {result['k'][-1]:.2f} Å^-1")
        
        # Test with custom parameters
        custom_result = xt.autobk(
            self.energy, pre_edge_result['norm'], pre_edge_result['e0'],
            rbkg=1.0, kmin=0, kmax=15, kweight=2, window='hanning'
        )
        
        # Basic verification
        self.assertIn('chi', custom_result)
        
        # Test that builder pattern works
        builder_result = (
            xt.AutobkBuilder()
            .energy(self.energy)
            .mu(pre_edge_result['norm'])
            .e0(pre_edge_result['e0'])
            .rbkg(1.0)
            .k_range(0, 15)
            .kweight(2)
            .window('hanning')
            .run(xt._internals.get_python())
        )
        
        self.assertIn('chi', builder_result)
    
    def test_xftf(self):
        """Test xftf function for forward Fourier transform."""
        # First normalize and remove background
        pre_edge_result = xt.pre_edge(self.energy, self.mu)
        autobk_result = xt.autobk(self.energy, pre_edge_result['norm'], pre_edge_result['e0'])
        
        # Test xftf with default parameters
        start_time = time.time()
        result = xt.xftf(autobk_result['k'], autobk_result['chi'])
        elapsed = time.time() - start_time
        
        # Verify output structure
        self.assertIn('r', result)
        self.assertIn('chir', result)
        self.assertIn('chir_mag', result)
        self.assertIn('chir_re', result)
        self.assertIn('chir_im', result)
        
        # Check arrays have correct length
        r_length = len(result['r'])
        self.assertGreater(r_length, 0)
        
        # Check r range starts near 0
        self.assertLess(result['r'][0], 0.1)
        
        # Verify chir_mag has reasonable amplitude
        chir_max = np.max(result['chir_mag'])
        self.assertGreater(chir_max, 0.01)
        
        # Our test data has two main contributions at ~2.5Å and ~4.2Å
        # Find the peaks in chir_mag
        peak_indices = []
        for i in range(1, len(result['r'])-1):
            if (result['chir_mag'][i] > result['chir_mag'][i-1] and 
                result['chir_mag'][i] > result['chir_mag'][i+1] and
                result['chir_mag'][i] > 0.1*chir_max):
                peak_indices.append(i)
        
        # Check we found at least 2 peaks
        self.assertGreaterEqual(len(peak_indices), 2)
        
        # Get r values of the peaks
        peak_r_values = [result['r'][i] for i in peak_indices]
        
        # Since EXAFS phase shifts move peaks, we look for peaks roughly at our synthetic data values
        self.assertTrue(any(abs(r - 2.0) < 1.0 for r in peak_r_values))  # First shell near 2Å
        self.assertTrue(any(abs(r - 4.0) < 1.0 for r in peak_r_values))  # Second shell near 4Å
        
        print(f"xftf took {elapsed:.6f} seconds")
        print(f"r range: {result['r'][0]:.2f} to {result['r'][-1]:.2f} Å")
        print(f"Found peaks at r = {', '.join(f'{r:.2f}' for r in peak_r_values)} Å")
        
        # Test with custom parameters
        custom_result = xt.xftf(
            autobk_result['k'], autobk_result['chi'],
            kmin=2, kmax=12, dk=1, window='hanning', kweight=2
        )
        
        # Basic verification
        self.assertIn('chir_mag', custom_result)
        
        # Test that builder pattern works
        builder_result = (
            xt.XftfBuilder()
            .k(autobk_result['k'])
            .chi(autobk_result['chi'])
            .k_range(2, 12)
            .dk(1)
            .window('hanning')
            .kweight(2)
            .run(xt._internals.get_python())
        )
        
        self.assertIn('chir_mag', builder_result)
    
    def test_xftr(self):
        """Test xftr function for reverse Fourier transform."""
        # First normalize, remove background, and forward FT
        pre_edge_result = xt.pre_edge(self.energy, self.mu)
        autobk_result = xt.autobk(self.energy, pre_edge_result['norm'], pre_edge_result['e0'])
        xftf_result = xt.xftf(
            autobk_result['k'], autobk_result['chi'],
            kmin=2, kmax=12, dk=1, window='hanning', kweight=2
        )
        
        # Test xftr with default parameters
        start_time = time.time()
        result = xt.xftr(xftf_result['r'], xftf_result['chir'])
        elapsed = time.time() - start_time
        
        # Verify output structure
        self.assertIn('k', result)
        self.assertIn('chiq', result)
        self.assertIn('chiq_re', result)
        self.assertIn('chiq_im', result)
        
        # Check arrays have correct length
        k_length = len(result['k'])
        self.assertGreater(k_length, 0)
        
        # Verify filtered chiq has reasonable amplitude
        chiq_max = np.max(np.abs(result['chiq']))
        self.assertGreater(chiq_max, 0.01)
        
        print(f"xftr took {elapsed:.6f} seconds")
        print(f"k range after reverse FT: {result['k'][0]:.2f} to {result['k'][-1]:.2f} Å^-1")
        
        # Test with custom parameters for r-range filtering
        custom_result = xt.xftr(
            xftf_result['r'], xftf_result['chir'],
            rmin=1.0, rmax=3.0, dr=0.2, window='hanning'
        )
        
        # Basic verification
        self.assertIn('chiq', custom_result)
        
        # Test that builder pattern works
        builder_result = (
            xt.XftrBuilder()
            .r(xftf_result['r'])
            .chir(xftf_result['chir'])
            .r_range(1.0, 3.0)
            .dr(0.2)
            .window('hanning')
            .run(xt._internals.get_python())
        )
        
        self.assertIn('chiq', builder_result)
    
    def test_function_chaining(self):
        """Test chaining of functions in xraylarch-compatible style."""
        # A common workflow would be:
        # 1. Find E0
        # 2. Normalize
        # 3. Background removal
        # 4. Forward FT
        # 5. Reverse FT with filtering
        
        start_time = time.time()
        
        # 1. Find E0
        e0 = xt.find_e0(self.energy, self.mu)
        
        # 2. Normalize
        normalization = xt.pre_edge(
            self.energy, self.mu, e0=e0,
            pre1=-200, pre2=-30, norm1=100, norm2=600
        )
        
        # 3. Background removal
        bkg_result = xt.autobk(
            self.energy, normalization['norm'], e0=e0,
            rbkg=1.0, kmin=0, kmax=15, kweight=2, window='hanning'
        )
        
        # 4. Forward FT
        ft_result = xt.xftf(
            bkg_result['k'], bkg_result['chi'],
            kmin=2, kmax=12, dk=1, window='hanning', kweight=2
        )
        
        # 5. Reverse FT with filtering
        ift_result = xt.xftr(
            ft_result['r'], ft_result['chir'],
            rmin=1.0, rmax=3.0, dr=0.2, window='hanning'
        )
        
        elapsed = time.time() - start_time
        
        # Verify we have results from all steps
        self.assertIsNotNone(e0)
        self.assertIn('norm', normalization)
        self.assertIn('chi', bkg_result)
        self.assertIn('chir_mag', ft_result)
        self.assertIn('chiq', ift_result)
        
        print(f"Complete workflow took {elapsed:.6f} seconds")

if __name__ == "__main__":
    unittest.main()