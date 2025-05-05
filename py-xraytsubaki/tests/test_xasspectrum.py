#!/usr/bin/env python3
"""
Tests for the XASSpectrum class in the Python bindings.
These tests define the expected interface and behavior for handling XAS spectra.
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

class TestXASSpectrum(unittest.TestCase):
    """Test the XASSpectrum class interface."""
    
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
        
    def test_create_spectrum(self):
        """Test creating a spectrum object."""
        # Basic initialization
        spectrum = xt.XASSpectrum()
        self.assertIsNotNone(spectrum)
        
        # Initialize with data
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu)
        self.assertIsNotNone(spectrum)
        
        # Check data was stored correctly
        np.testing.assert_allclose(spectrum.energy, self.energy)
        np.testing.assert_allclose(spectrum.mu, self.mu)
        
        # Test fluent API
        spectrum_fluent = xt.XASSpectrum().energy(self.energy).mu(self.mu)
        np.testing.assert_allclose(spectrum_fluent.energy, self.energy)
        np.testing.assert_allclose(spectrum_fluent.mu, self.mu)
    
    def test_spectrum_normalization(self):
        """Test spectrum normalization methods."""
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu)
        
        # Test normalization with default parameters
        spectrum.normalize()
        
        # After normalization, these attributes should exist
        self.assertTrue(hasattr(spectrum, 'norm'))
        self.assertTrue(hasattr(spectrum, 'e0'))
        self.assertTrue(hasattr(spectrum, 'edge_step'))
        self.assertAlmostEqual(spectrum.e0, 17500, delta=20)
        
        # Post-edge should be around 1.0
        post_idx = np.where(spectrum.energy > spectrum.e0 + 100)[0]
        norm_avg = np.mean(spectrum.norm[post_idx])
        self.assertAlmostEqual(norm_avg, 1.0, delta=0.1)
        
        # Test with explicit parameters
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu)
        spectrum.normalize(
            e0=17500,
            pre1=-200,
            pre2=-30,
            norm1=100,
            norm2=600,
            nnorm=2
        )
        self.assertAlmostEqual(spectrum.e0, 17500, delta=1)
        
        # Test fluent API
        spectrum_fluent = (
            xt.XASSpectrum(energy=self.energy, mu=self.mu)
            .normalize()
            .pre_range(-200, -30)
            .norm_range(100, 600)
            .nnorm(2)
            .run()
        )
        self.assertAlmostEqual(spectrum_fluent.edge_step, spectrum.edge_step, delta=0.001)
    
    def test_background_removal(self):
        """Test background removal from a spectrum."""
        # First normalize
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu)
        spectrum.normalize()
        
        # Then remove background
        spectrum.autobk(rbkg=1.0, kweight=2, kmin=0, kmax=15)
        
        # After background removal, these attributes should exist
        self.assertTrue(hasattr(spectrum, 'k'))
        self.assertTrue(hasattr(spectrum, 'chi'))
        self.assertTrue(hasattr(spectrum, 'bkg'))
        
        # k should start near 0 and end around kmax
        self.assertLess(spectrum.k[0], 0.1)
        self.assertGreater(spectrum.k[-1], 14.0)
        
        # chi should have oscillations
        chi_amplitude = np.max(np.abs(spectrum.chi))
        self.assertGreater(chi_amplitude, 0.01)
        
        # Test fluent API
        spectrum_fluent = (
            xt.XASSpectrum(energy=self.energy, mu=self.mu)
            .normalize()
            .autobk()
            .rbkg(1.0)
            .k_range(0, 15)
            .kweight(2)
            .run()
        )
        np.testing.assert_allclose(spectrum_fluent.k, spectrum.k, rtol=1e-5)
        np.testing.assert_allclose(spectrum_fluent.chi, spectrum.chi, rtol=1e-5)
    
    def test_forward_transform(self):
        """Test forward Fourier transform of a spectrum."""
        # Prepare spectrum with background removal
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu)
        spectrum.normalize()
        spectrum.autobk()
        
        # Perform forward FT
        spectrum.xftf(kmin=2, kmax=12, dk=1, window='hanning', kweight=2)
        
        # After FT, these attributes should exist
        self.assertTrue(hasattr(spectrum, 'r'))
        self.assertTrue(hasattr(spectrum, 'chir'))
        self.assertTrue(hasattr(spectrum, 'chir_mag'))
        
        # r should start at 0 and extend to reasonable values
        self.assertLess(spectrum.r[0], 0.1)
        self.assertGreater(spectrum.r[-1], 6.0)
        
        # Test fluent API
        spectrum_fluent = (
            xt.XASSpectrum(energy=self.energy, mu=self.mu)
            .normalize()
            .autobk()
            .xftf()
            .k_range(2, 12)
            .dk(1)
            .window('hanning')
            .kweight(2)
            .run()
        )
        np.testing.assert_allclose(spectrum_fluent.r, spectrum.r, rtol=1e-5)
        np.testing.assert_allclose(spectrum_fluent.chir_mag, spectrum.chir_mag, rtol=1e-5)
    
    def test_reverse_transform(self):
        """Test reverse Fourier transform of a spectrum."""
        # Prepare spectrum with forward FT
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu)
        spectrum.normalize()
        spectrum.autobk()
        spectrum.xftf(kmin=2, kmax=12)
        
        # Perform reverse FT
        spectrum.xftr(rmin=1, rmax=3, dr=0.1, window='hanning')
        
        # After reverse FT, these attributes should exist
        self.assertTrue(hasattr(spectrum, 'q'))
        self.assertTrue(hasattr(spectrum, 'chiq'))
        
        # q should cover a reasonable range
        self.assertLess(spectrum.q[0], 0.1)
        self.assertGreater(spectrum.q[-1], 10.0)
        
        # Test fluent API
        spectrum_fluent = (
            xt.XASSpectrum(energy=self.energy, mu=self.mu)
            .normalize()
            .autobk()
            .xftf()
            .xftr()
            .r_range(1, 3)
            .dr(0.1)
            .window('hanning')
            .run()
        )
        np.testing.assert_allclose(spectrum_fluent.q, spectrum.q, rtol=1e-5)
        np.testing.assert_allclose(spectrum_fluent.chiq, spectrum.chiq, rtol=1e-5)
    
    def test_file_io(self):
        """Test reading and writing spectrum data to files."""
        # Create a spectrum
        spectrum = xt.XASSpectrum(energy=self.energy, mu=self.mu)
        spectrum.normalize()
        spectrum.autobk()
        
        # Write to a temp file
        with tempfile.NamedTemporaryFile(suffix='.json') as tmp:
            spectrum.save(tmp.name)
            
            # Read back
            spectrum2 = xt.XASSpectrum.read(tmp.name)
            
            # Check data is the same
            np.testing.assert_allclose(spectrum2.energy, spectrum.energy)
            np.testing.assert_allclose(spectrum2.mu, spectrum.mu)
            np.testing.assert_allclose(spectrum2.norm, spectrum.norm)
            np.testing.assert_allclose(spectrum2.chi, spectrum.chi)
        
        # Test other formats (BSON)
        with tempfile.NamedTemporaryFile(suffix='.bson') as tmp:
            spectrum.save(tmp.name)
            spectrum2 = xt.XASSpectrum.read(tmp.name)
            np.testing.assert_allclose(spectrum2.energy, spectrum.energy)
    
    def test_spectrum_group(self):
        """Test creating and manipulating spectrum groups."""
        # Create two spectra
        spectrum1 = xt.XASSpectrum(energy=self.energy, mu=self.mu)
        spectrum1.name = "sample1"
        
        spectrum2 = xt.XASSpectrum(energy=self.energy, mu=self.mu * 0.9)
        spectrum2.name = "sample2"
        
        # Create a group
        group = xt.XASGroup()
        
        # Add spectra to group
        group.add_spectrum(spectrum1)
        group.add_spectrum(spectrum2)
        
        # Check spectra were added
        self.assertEqual(len(group), 2)
        self.assertEqual(group[0].name, "sample1")
        self.assertEqual(group[1].name, "sample2")
        
        # Access by name
        self.assertEqual(group["sample1"].name, "sample1")
        
        # Process all spectra in group
        group.normalize_all()
        
        # Check all were normalized
        self.assertTrue(hasattr(group[0], 'norm'))
        self.assertTrue(hasattr(group[1], 'norm'))
        
        # Similarly for other operations
        group.autobk_all()
        group.xftf_all(kmin=2, kmax=12)
        
        # Check results
        self.assertTrue(hasattr(group[0], 'chi'))
        self.assertTrue(hasattr(group[0], 'chir'))
        
        # Test fluent API
        group_fluent = (
            xt.XASGroup()
            .add(spectrum1)
            .add(spectrum2)
            .normalize_all()
            .autobk_all()
            .xftf_all()
            .k_range(2, 12)
            .run()
        )
        self.assertEqual(len(group_fluent), 2)
        np.testing.assert_allclose(group_fluent[0].chir_mag, group[0].chir_mag, rtol=1e-5)


if __name__ == '__main__':
    unittest.main()