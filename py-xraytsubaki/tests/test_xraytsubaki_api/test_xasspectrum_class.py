#!/usr/bin/env python3
"""
Test suite for the XRayTsubaki native API in py_xraytsubaki.

This module tests the object-oriented API with XASSpectrum and XASGroup classes.
The tests include:
1. Creating and manipulating XASSpectrum objects
2. Normalization methods
3. Background removal
4. Fourier transforms
5. File I/O
6. Fluent API pattern
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

class TestXASSpectrumClass(unittest.TestCase):
    """Test the XASSpectrum class in the py_xraytsubaki package."""
    
    @classmethod
    def setUpClass(cls):
        """Set up the test class by creating test data."""
        cls.energy, cls.mu, cls.true_e0 = create_test_spectrum()
        
        # Create test directory for saving files
        cls.test_dir = Path("test_output")
        cls.test_dir.mkdir(exist_ok=True)
    
    def test_creation(self):
        """Test creating XASSpectrum objects."""
        # Test creating with arrays
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu, name="Test Spectrum")
        
        # Verify attributes
        self.assertEqual(spectrum.name, "Test Spectrum")
        self.assertIsNotNone(spectrum.energy)
        self.assertIsNotNone(spectrum.mu)
        
        # Verify arrays match input
        np.testing.assert_allclose(spectrum.energy, self.energy)
        np.testing.assert_allclose(spectrum.mu, self.mu)
        
        # Test string representation
        str_rep = str(spectrum)
        self.assertIn("Test Spectrum", str_rep)
        
        # Test empty creation and setting arrays later
        empty_spectrum = xt.XASSpectrum(name="Empty")
        self.assertEqual(empty_spectrum.name, "Empty")
        self.assertIsNone(empty_spectrum.energy)
        
        # Set arrays
        empty_spectrum.energy = self.energy
        empty_spectrum.mu = self.mu
        
        # Verify arrays
        np.testing.assert_allclose(empty_spectrum.energy, self.energy)
        np.testing.assert_allclose(empty_spectrum.mu, self.mu)
        
        # Test fluent API for setting arrays
        fluent_spectrum = xt.XASSpectrum(name="Fluent")
        fluent_spectrum.energy(self.energy).mu(self.mu)
        
        # Verify arrays
        np.testing.assert_allclose(fluent_spectrum.energy, self.energy)
        np.testing.assert_allclose(fluent_spectrum.mu, self.mu)
    
    def test_normalization(self):
        """Test normalization of XASSpectrum objects."""
        # Create spectrum
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu, name="Normalization Test")
        
        # Test normalization with default parameters
        start_time = time.time()
        spectrum.normalize()
        elapsed = time.time() - start_time
        
        # Verify normalization results
        self.assertIsNotNone(spectrum.e0)
        self.assertIsNotNone(spectrum.edge_step)
        self.assertIsNotNone(spectrum.norm)
        self.assertIsNotNone(spectrum.pre_edge)
        self.assertIsNotNone(spectrum.post_edge)
        
        # Check e0 is reasonable
        self.assertLess(abs(spectrum.e0 - self.true_e0), 10.0)
        
        # Check edge_step is close to 1 (our synthetic data has edge_step=1.0)
        self.assertGreater(spectrum.edge_step, 0.9)
        self.assertLess(spectrum.edge_step, 1.1)
        
        # Verify normalized spectrum is between 0 and ~1
        self.assertGreater(np.min(spectrum.norm), -0.1)
        self.assertLess(np.max(spectrum.norm), 1.2)
        
        print(f"normalize() took {elapsed:.6f} seconds")
        print(f"E0: {spectrum.e0:.2f}, Edge step: {spectrum.edge_step:.4f}")
        
        # Test normalization with custom parameters
        custom_spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu)
        custom_spectrum.normalize(pre1=-200, pre2=-30, norm1=100, norm2=600)
        
        # Basic verification
        self.assertIsNotNone(custom_spectrum.norm)
    
    def test_background_removal(self):
        """Test background removal on XASSpectrum objects."""
        # Create and normalize spectrum
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu, name="Background Test")
        spectrum.normalize()
        
        # Test background removal with default parameters
        start_time = time.time()
        spectrum.autobk()
        elapsed = time.time() - start_time
        
        # Verify background removal results
        self.assertIsNotNone(spectrum.k)
        self.assertIsNotNone(spectrum.chi)
        self.assertIsNotNone(spectrum.bkg)
        
        # Check arrays have correct length
        k_length = len(spectrum.k)
        self.assertGreater(k_length, 0)
        self.assertEqual(len(spectrum.chi), k_length)
        
        # Check k range starts near 0
        self.assertLess(spectrum.k[0], 0.5)
        
        # Verify chi has reasonable amplitude (not too large or small)
        chi_max = np.max(np.abs(spectrum.chi))
        self.assertGreater(chi_max, 0.01)
        self.assertLess(chi_max, 10.0)
        
        print(f"autobk() took {elapsed:.6f} seconds")
        print(f"k range: {spectrum.k[0]:.2f} to {spectrum.k[-1]:.2f} Å^-1")
        
        # Test with custom parameters
        custom_spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu)
        custom_spectrum.normalize()
        custom_spectrum.autobk(rbkg=1.0, kmin=0, kmax=15, kweight=2, window='hanning')
        
        # Basic verification
        self.assertIsNotNone(custom_spectrum.chi)
    
    def test_fourier_transform(self):
        """Test Fourier transform on XASSpectrum objects."""
        # Create, normalize, and background-subtract spectrum
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu, name="FT Test")
        spectrum.normalize()
        spectrum.autobk()
        
        # Test forward FT with default parameters
        start_time = time.time()
        spectrum.xftf()
        elapsed = time.time() - start_time
        
        # Verify FT results
        self.assertIsNotNone(spectrum.r)
        self.assertIsNotNone(spectrum.chir)
        self.assertIsNotNone(spectrum.chir_mag)
        self.assertIsNotNone(spectrum.chir_re)
        self.assertIsNotNone(spectrum.chir_im)
        
        # Check arrays have correct length
        r_length = len(spectrum.r)
        self.assertGreater(r_length, 0)
        
        # Check r range starts near 0
        self.assertLess(spectrum.r[0], 0.1)
        
        # Verify chir_mag has reasonable amplitude
        chir_max = np.max(spectrum.chir_mag)
        self.assertGreater(chir_max, 0.01)
        
        # Our test data has two main contributions at ~2.5Å and ~4.2Å
        # Find the peaks in chir_mag
        peak_indices = []
        for i in range(1, len(spectrum.r)-1):
            if (spectrum.chir_mag[i] > spectrum.chir_mag[i-1] and 
                spectrum.chir_mag[i] > spectrum.chir_mag[i+1] and
                spectrum.chir_mag[i] > 0.1*chir_max):
                peak_indices.append(i)
        
        # Check we found at least 2 peaks
        self.assertGreaterEqual(len(peak_indices), 2)
        
        # Get r values of the peaks
        peak_r_values = [spectrum.r[i] for i in peak_indices]
        
        # Since EXAFS phase shifts move peaks, we look for peaks roughly at our synthetic data values
        self.assertTrue(any(abs(r - 2.0) < 1.0 for r in peak_r_values))  # First shell near 2Å
        self.assertTrue(any(abs(r - 4.0) < 1.0 for r in peak_r_values))  # Second shell near 4Å
        
        print(f"xftf() took {elapsed:.6f} seconds")
        print(f"r range: {spectrum.r[0]:.2f} to {spectrum.r[-1]:.2f} Å")
        print(f"Found peaks at r = {', '.join(f'{r:.2f}' for r in peak_r_values)} Å")
        
        # Test reverse FT
        start_time = time.time()
        spectrum.xftr(rmin=1.0, rmax=3.0)
        elapsed = time.time() - start_time
        
        # Verify reverse FT results
        self.assertIsNotNone(spectrum.q)
        self.assertIsNotNone(spectrum.chiq)
        
        print(f"xftr() took {elapsed:.6f} seconds")
        print(f"q range: {spectrum.q[0]:.2f} to {spectrum.q[-1]:.2f} Å^-1")
        
        # Test with custom parameters
        custom_spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu)
        custom_spectrum.normalize()
        custom_spectrum.autobk()
        custom_spectrum.xftf(kmin=2, kmax=12, dk=1, window='hanning', kweight=2)
        
        # Basic verification
        self.assertIsNotNone(custom_spectrum.chir_mag)
    
    def test_file_io(self):
        """Test file I/O for XASSpectrum objects."""
        # Create and process spectrum
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu, name="IO Test")
        spectrum.normalize()
        spectrum.autobk()
        spectrum.xftf()
        
        # Save to JSON
        json_path = self.test_dir / "test_spectrum.json"
        spectrum.save(str(json_path))
        
        # Verify file exists
        self.assertTrue(json_path.exists())
        
        # Read from JSON
        loaded_spectrum = xt.XASSpectrum.read(str(json_path))
        
        # Verify loaded spectrum
        self.assertEqual(loaded_spectrum.name, "IO Test")
        self.assertIsNotNone(loaded_spectrum.energy)
        self.assertIsNotNone(loaded_spectrum.mu)
        self.assertIsNotNone(loaded_spectrum.norm)
        self.assertIsNotNone(loaded_spectrum.k)
        self.assertIsNotNone(loaded_spectrum.chi)
        self.assertIsNotNone(loaded_spectrum.r)
        self.assertIsNotNone(loaded_spectrum.chir_mag)
        
        # Compare key values
        self.assertAlmostEqual(loaded_spectrum.e0, spectrum.e0, places=4)
        self.assertAlmostEqual(loaded_spectrum.edge_step, spectrum.edge_step, places=4)
        
        # Compare arrays (not exact due to floating point)
        np.testing.assert_allclose(loaded_spectrum.energy, spectrum.energy, rtol=1e-5)
        np.testing.assert_allclose(loaded_spectrum.mu, spectrum.mu, rtol=1e-5)
        np.testing.assert_allclose(loaded_spectrum.norm, spectrum.norm, rtol=1e-5)
        np.testing.assert_allclose(loaded_spectrum.k, spectrum.k, rtol=1e-5)
        np.testing.assert_allclose(loaded_spectrum.chi, spectrum.chi, rtol=1e-5)
        
        print(f"Successfully saved and loaded spectrum from {json_path}")
        
        # Test BSON format if supported
        try:
            bson_path = self.test_dir / "test_spectrum.bson"
            spectrum.save(str(bson_path))
            
            # Verify file exists
            self.assertTrue(bson_path.exists())
            
            # Read from BSON
            bson_loaded = xt.XASSpectrum.read(str(bson_path))
            
            # Basic verification
            self.assertEqual(bson_loaded.name, "IO Test")
            self.assertIsNotNone(bson_loaded.energy)
            
            print(f"Successfully saved and loaded spectrum from {bson_path}")
        except Exception as e:
            print(f"BSON format test skipped: {e}")
    
    def test_fluent_api(self):
        """Test the fluent API pattern."""
        # Test complete workflow with method chaining
        start_time = time.time()
        
        spectrum = (
            xt.XASSpectrum(energy=self.energy, mu=self.mu, name="Fluent API Test")
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
        
        # Test r-range filtering
        spectrum = (
            spectrum
            .xftr()
            .r_range(1.0, 3.0)
            .dr(0.2)
            .window('hanning')
            .run()
        )
        
        elapsed = time.time() - start_time
        
        # Verify we have results from all steps
        self.assertIsNotNone(spectrum.e0)
        self.assertIsNotNone(spectrum.norm)
        self.assertIsNotNone(spectrum.chi)
        self.assertIsNotNone(spectrum.chir_mag)
        self.assertIsNotNone(spectrum.chiq)
        
        print(f"Fluent API workflow took {elapsed:.6f} seconds")
        
        # Test partial workflow (only normalization)
        norm_only = (
            xt.XASSpectrum(energy=self.energy, mu=self.mu)
            .normalize()
            .pre_range(-200, -30)
            .norm_range(100, 600)
            .run()
        )
        
        self.assertIsNotNone(norm_only.norm)
        self.assertIsNone(norm_only.chi)  # Background not removed yet

if __name__ == "__main__":
    unittest.main()