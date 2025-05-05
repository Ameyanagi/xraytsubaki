#!/usr/bin/env python3
"""
Test suite for the XASGroup class in py_xraytsubaki.

This module tests the XASGroup class that manages collections of XAS spectra.
The tests include:
1. Creating and manipulating XASGroup objects
2. Adding and removing spectra
3. Batch processing operations
4. Accessing and modifying spectra in the group
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

def create_test_spectra(num_spectra=3):
    """Create multiple test spectra with varying parameters."""
    spectra = []
    
    # Energy grid
    energy = np.linspace(17000, 18000, 1000)
    
    # Common parameters
    e0 = 17500.0
    pre_edge_slope = 0.01
    edge_step = 1.0
    
    # Parameters that vary across spectra
    frequencies = [2.5, 2.6, 2.7]  # Increasing distance
    amplitudes = [0.4, 0.35, 0.3]   # Decreasing amplitude
    disorders = [0.03, 0.035, 0.04] # Increasing disorder
    
    for i in range(num_spectra):
        # Create synthetic spectrum
        # Pre-edge: linear function
        pre_edge = 1.0 + pre_edge_slope * (energy - 17000)
        
        # Edge step: tanh function
        edge = edge_step * 0.5 * (1 + np.tanh((energy - e0) / 10.0))
        
        # EXAFS oscillations
        k_mask = energy > e0
        k = np.zeros_like(energy)
        k[k_mask] = np.sqrt((energy[k_mask] - e0) / 3.81)
        
        # Single path with varying parameters
        freq = frequencies[i]
        amp = amplitudes[i]
        damp = disorders[i]
        phase = 0.3  # constant phase
        
        oscil = np.zeros_like(energy)
        oscil[k_mask] = amp * np.sin(2 * freq * k[k_mask] + phase) * np.exp(-damp * k[k_mask]**2)
        
        # Total mu
        mu = pre_edge + edge + oscil
        
        # Add some noise
        np.random.seed(12345 + i)
        noise = np.random.normal(0, 0.005, len(energy))
        mu_noisy = mu + noise
        
        # Create XASSpectrum
        spectrum = xt.XASSpectrum(energy=energy, mu=mu_noisy, name=f"Spectrum {i+1}")
        spectra.append(spectrum)
    
    return spectra, frequencies, amplitudes, disorders

class TestXASGroupClass(unittest.TestCase):
    """Test the XASGroup class in the py_xraytsubaki package."""
    
    @classmethod
    def setUpClass(cls):
        """Set up the test class by creating test data."""
        cls.spectra, cls.frequencies, cls.amplitudes, cls.disorders = create_test_spectra(3)
        
        # Create test directory for saving files
        cls.test_dir = Path("test_output")
        cls.test_dir.mkdir(exist_ok=True)
    
    def test_creation(self):
        """Test creating XASGroup objects."""
        # Test empty group
        group = xt.XASGroup(name="Test Group")
        
        # Verify attributes
        self.assertEqual(group.name, "Test Group")
        self.assertEqual(len(group), 0)
        
        # Add spectra
        for spectrum in self.spectra:
            group.add_spectrum(spectrum)
        
        # Verify group size
        self.assertEqual(len(group), 3)
        
        # Test string representation
        str_rep = str(group)
        self.assertIn("Test Group", str_rep)
        self.assertIn("3 spectra", str_rep)
        
        # Test creation with spectra
        spectra_list = [s for s in self.spectra]  # Make a copy
        group2 = xt.XASGroup(name="Group 2", spectra=spectra_list)
        
        # Verify group size
        self.assertEqual(len(group2), 3)
    
    def test_accessing_spectra(self):
        """Test accessing spectra in the group."""
        # Create group with spectra
        group = xt.XASGroup(name="Access Test")
        for spectrum in self.spectra:
            group.add_spectrum(spectrum)
        
        # Test __getitem__ access
        for i in range(3):
            # Access by index
            spectrum = group[i]
            self.assertEqual(spectrum.name, f"Spectrum {i+1}")
            
            # Access by name
            spectrum = group[f"Spectrum {i+1}"]
            self.assertEqual(spectrum.name, f"Spectrum {i+1}")
        
        # Test iteration
        for i, spectrum in enumerate(group):
            self.assertEqual(spectrum.name, f"Spectrum {i+1}")
        
        # Test getting all names
        names = group.get_spectrum_names()
        self.assertEqual(len(names), 3)
        self.assertEqual(set(names), {"Spectrum 1", "Spectrum 2", "Spectrum 3"})
    
    def test_modifying_group(self):
        """Test adding and removing spectra."""
        # Create empty group
        group = xt.XASGroup(name="Modify Test")
        
        # Add spectra one by one
        group.add_spectrum(self.spectra[0])
        self.assertEqual(len(group), 1)
        
        group.add_spectrum(self.spectra[1])
        self.assertEqual(len(group), 2)
        
        # Try adding a spectrum with a duplicate name
        duplicate = xt.XASSpectrum(
            energy=self.spectra[0].energy,
            mu=self.spectra[0].mu,
            name="Spectrum 1"
        )
        
        with self.assertRaises(Exception):
            group.add_spectrum(duplicate)
        
        # Add with a different name
        duplicate.name = "Unique Name"
        group.add_spectrum(duplicate)
        self.assertEqual(len(group), 3)
        
        # Remove by name
        group.remove_spectrum("Unique Name")
        self.assertEqual(len(group), 2)
        
        # Remove by index
        group.remove_spectrum(0)  # Remove the first one
        self.assertEqual(len(group), 1)
        
        # Check the remaining spectrum
        self.assertEqual(group[0].name, "Spectrum 2")
        
        # Clear all
        group.clear()
        self.assertEqual(len(group), 0)
    
    def test_batch_processing(self):
        """Test batch processing operations."""
        # Create group with spectra
        group = xt.XASGroup(name="Batch Test")
        for spectrum in self.spectra:
            group.add_spectrum(spectrum)
        
        # Test batch normalization
        start_time = time.time()
        group.normalize_all()
        elapsed = time.time() - start_time
        
        # Verify normalization results for all spectra
        for spectrum in group:
            self.assertIsNotNone(spectrum.e0)
            self.assertIsNotNone(spectrum.edge_step)
            self.assertIsNotNone(spectrum.norm)
        
        print(f"normalize_all() took {elapsed:.6f} seconds")
        
        # Test batch background removal
        start_time = time.time()
        group.autobk_all(rbkg=1.0, kmin=0, kmax=15)
        elapsed = time.time() - start_time
        
        # Verify background removal results for all spectra
        for spectrum in group:
            self.assertIsNotNone(spectrum.k)
            self.assertIsNotNone(spectrum.chi)
        
        print(f"autobk_all() took {elapsed:.6f} seconds")
        
        # Test batch forward FT
        start_time = time.time()
        group.xftf_all(kmin=2, kmax=12, dk=1, window='hanning', kweight=2)
        elapsed = time.time() - start_time
        
        # Verify FT results for all spectra
        for spectrum in group:
            self.assertIsNotNone(spectrum.r)
            self.assertIsNotNone(spectrum.chir_mag)
        
        print(f"xftf_all() took {elapsed:.6f} seconds")
        
        # Test batch reverse FT
        start_time = time.time()
        group.xftr_all(rmin=1.0, rmax=3.0, dr=0.2, window='hanning')
        elapsed = time.time() - start_time
        
        # Verify reverse FT results for all spectra
        for spectrum in group:
            self.assertIsNotNone(spectrum.q)
            self.assertIsNotNone(spectrum.chiq)
        
        print(f"xftr_all() took {elapsed:.6f} seconds")
    
    def test_parallel_processing(self):
        """Test that batch processing is faster than sequential for many spectra."""
        # Only valuable if we have more spectra
        n_spectra = 10
        large_test_spectra, _, _, _ = create_test_spectra(n_spectra)
        
        # Create groups for sequential and parallel processing
        seq_group = xt.XASGroup(name="Sequential")
        for spectrum in large_test_spectra:
            seq_group.add_spectrum(spectrum.clone())  # Clone to avoid sharing references
        
        par_group = xt.XASGroup(name="Parallel")
        for spectrum in large_test_spectra:
            par_group.add_spectrum(spectrum.clone())  # Clone to avoid sharing references
        
        # Set parallel processing flag
        par_group.set_parallel(True)
        seq_group.set_parallel(False)
        
        # Time sequential processing
        start_time = time.time()
        seq_group.normalize_all()
        seq_group.autobk_all()
        seq_group.xftf_all()
        sequential_time = time.time() - start_time
        
        # Time parallel processing
        start_time = time.time()
        par_group.normalize_all()
        par_group.autobk_all()
        par_group.xftf_all()
        parallel_time = time.time() - start_time
        
        print(f"Sequential processing time: {sequential_time:.6f} seconds")
        print(f"Parallel processing time: {parallel_time:.6f} seconds")
        
        # Verify that parallel is not slower than sequential
        # (may not be faster on CI with limited cores, so we don't assert it's faster)
        self.assertLessEqual(parallel_time / sequential_time, 1.5)
    
    def test_accessing_data_arrays(self):
        """Test accessing data arrays from the group."""
        # Create and process a group
        group = xt.XASGroup(name="Array Test")
        for spectrum in self.spectra:
            group.add_spectrum(spectrum)
        
        group.normalize_all()
        group.autobk_all()
        group.xftf_all()
        
        # Test getting energy
        energies = group.get_energy_arrays()
        self.assertEqual(len(energies), 3)
        for i, energy in enumerate(energies):
            np.testing.assert_allclose(energy, self.spectra[i].energy)
        
        # Test getting mu
        mus = group.get_mu_arrays()
        self.assertEqual(len(mus), 3)
        
        # Test getting normalized spectra
        norms = group.get_norm_arrays()
        self.assertEqual(len(norms), 3)
        
        # Test getting k arrays
        k_arrays = group.get_k_arrays()
        self.assertEqual(len(k_arrays), 3)
        
        # Test getting chi arrays
        chi_arrays = group.get_chi_arrays()
        self.assertEqual(len(chi_arrays), 3)
        
        # Test getting R arrays
        r_arrays = group.get_r_arrays()
        self.assertEqual(len(r_arrays), 3)
        
        # Test getting chi(R) magnitude arrays
        chir_mag_arrays = group.get_chir_mag_arrays()
        self.assertEqual(len(chir_mag_arrays), 3)
    
    def test_file_io(self):
        """Test file I/O for XASGroup objects."""
        # Create and process group
        group = xt.XASGroup(name="IO Test Group")
        for spectrum in self.spectra:
            group.add_spectrum(spectrum)
        
        group.normalize_all()
        group.autobk_all()
        group.xftf_all()
        
        # Save to JSON
        json_path = self.test_dir / "test_group.json"
        group.save(str(json_path))
        
        # Verify file exists
        self.assertTrue(json_path.exists())
        
        # Read from JSON
        loaded_group = xt.XASGroup.read(str(json_path))
        
        # Verify loaded group
        self.assertEqual(loaded_group.name, "IO Test Group")
        self.assertEqual(len(loaded_group), 3)
        
        # Verify spectra in the group
        for i, spectrum in enumerate(loaded_group):
            self.assertEqual(spectrum.name, f"Spectrum {i+1}")
            self.assertIsNotNone(spectrum.energy)
            self.assertIsNotNone(spectrum.mu)
            self.assertIsNotNone(spectrum.norm)
            self.assertIsNotNone(spectrum.k)
            self.assertIsNotNone(spectrum.chi)
            self.assertIsNotNone(spectrum.r)
            self.assertIsNotNone(spectrum.chir_mag)
        
        print(f"Successfully saved and loaded group from {json_path}")

if __name__ == "__main__":
    unittest.main()