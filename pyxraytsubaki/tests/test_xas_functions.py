#!/usr/bin/env python3
"""
Tests for XAS data processing functions in pyxraytsubaki.
This tests the functional API that mimics the xraylarch interface.
"""

import sys
import os
import numpy as np
import time

def create_test_spectrum():
    """Create a synthetic XAS spectrum for testing."""
    # Energy grid
    energy = np.linspace(17000, 18000, 1000)
    
    # Parameters
    e0 = 17500
    pre_edge_slope = 0.01
    edge_step = 1.0
    
    # Synthetic XAFS spectrum
    # Pre-edge: linear function
    pre_edge = 1.0 + pre_edge_slope * (energy - 17000)
    
    # Edge step: tanh function
    edge = edge_step * 0.5 * (1 + np.tanh((energy - e0) / 10.0))
    
    # EXAFS oscillations
    # Convert E to k
    k_mask = energy > e0
    k = np.zeros_like(energy)
    k[k_mask] = np.sqrt((energy[k_mask] - e0) / 3.81)
    
    # Multiple paths
    amp1, freq1, phase1, damp1 = 0.4, 1.5, 0.3, 0.04
    amp2, freq2, phase2, damp2 = 0.2, 2.8, 0.6, 0.03
    
    oscil = np.zeros_like(energy)
    oscil[k_mask] = (
        amp1 * np.sin(2 * freq1 * k[k_mask] + phase1) * np.exp(-damp1 * k[k_mask]**2) +
        amp2 * np.sin(2 * freq2 * k[k_mask] + phase2) * np.exp(-damp2 * k[k_mask]**2)
    )
    
    # Total mu
    mu = pre_edge + edge + oscil
    
    # Add some noise
    np.random.seed(12345)
    noise = np.random.normal(0, 0.01, len(energy))
    mu_noisy = mu + noise
    
    return energy, mu_noisy, e0

def test_find_e0():
    """Test find_e0 function."""
    try:
        import pyxraytsubaki as xt
        
        # Create test data
        energy, mu, true_e0 = create_test_spectrum()
        
        # Test find_e0 function
        start_time = time.time()
        e0 = xt.find_e0(energy, mu)
        elapsed = time.time() - start_time
        
        print(f"find_e0 test: calculated E0 = {e0:.2f} eV, expected ~{true_e0:.2f} eV")
        print(f"Execution time: {elapsed:.6f} seconds")
        
        # Check if E0 is close to the true value
        tolerance = 50  # eV - allow some deviation since edge finding is approximate
        assert abs(e0 - true_e0) < tolerance, f"E0 error too large: {abs(e0 - true_e0):.2f} eV"
        
        print("✓ find_e0 test passed")
        return True
    except Exception as e:
        print(f"✗ find_e0 test failed: {e}")
        return False

def test_pre_edge():
    """Test pre_edge function."""
    try:
        import pyxraytsubaki as xt
        
        # Create test data
        energy, mu, e0 = create_test_spectrum()
        
        # Test pre_edge function
        start_time = time.time()
        
        # Default parameters
        result = xt.pre_edge(energy, mu)
        elapsed = time.time() - start_time
        
        # Check if required keys are in the result
        required_keys = ['e0', 'edge_step', 'norm', 'pre_edge', 'post_edge']
        for key in required_keys:
            assert key in result, f"Missing key in pre_edge result: {key}"
        
        print(f"pre_edge test: E0 = {result['e0']:.2f} eV, edge step = {result['edge_step']:.4f}")
        print(f"Execution time: {elapsed:.6f} seconds")
        
        # Verify dimensions
        assert len(result['norm']) == len(energy), "Normalized spectrum has wrong size"
        assert len(result['pre_edge']) == len(energy), "Pre-edge line has wrong size"
        assert len(result['post_edge']) == len(energy), "Post-edge line has wrong size"
        
        # Test with custom parameters
        custom_result = xt.pre_edge(energy, mu, e0=e0, pre1=-150, pre2=-30, norm1=100, norm2=500)
        
        # Verify the custom parameters produced different results
        assert np.any(result['norm'] != custom_result['norm']), "Custom parameters had no effect"
        
        print("✓ pre_edge test passed")
        return True
    except Exception as e:
        print(f"✗ pre_edge test failed: {e}")
        return False

def test_autobk():
    """Test autobk function."""
    try:
        import pyxraytsubaki as xt
        
        # Create test data
        energy, mu, e0 = create_test_spectrum()
        
        # First perform normalization
        norm_result = xt.pre_edge(energy, mu, e0=e0)
        
        # Test autobk function
        start_time = time.time()
        
        # Default parameters
        result = xt.autobk(energy, norm_result['norm'], e0=e0)
        elapsed = time.time() - start_time
        
        # Check if required keys are in the result
        required_keys = ['k', 'chi', 'kraw', 'background']
        for key in required_keys:
            assert key in result, f"Missing key in autobk result: {key}"
        
        # Check k-range
        k_min, k_max = min(result['k']), max(result['k'])
        print(f"autobk test: k range = {k_min:.2f} to {k_max:.2f} Å⁻¹")
        print(f"Execution time: {elapsed:.6f} seconds")
        
        # Test with custom parameters
        custom_result = xt.autobk(energy, norm_result['norm'], e0=e0, rbkg=1.2, kmin=0, kmax=15, kweight=2)
        
        # Verify dimensions
        assert len(result['k']) == len(result['chi']), "k and chi arrays have different lengths"
        assert len(result['kraw']) == len(result['background']), "kraw and background arrays have different lengths"
        
        print("✓ autobk test passed")
        return True
    except Exception as e:
        print(f"✗ autobk test failed: {e}")
        return False

def test_xftf():
    """Test xftf (forward Fourier transform) function."""
    try:
        import pyxraytsubaki as xt
        
        # Create test data
        energy, mu, e0 = create_test_spectrum()
        
        # First perform normalization and background removal
        norm_result = xt.pre_edge(energy, mu, e0=e0)
        bkg_result = xt.autobk(energy, norm_result['norm'], e0=e0)
        
        # Test xftf function
        start_time = time.time()
        
        # Default parameters
        result = xt.xftf(bkg_result['k'], bkg_result['chi'])
        elapsed = time.time() - start_time
        
        # Check if required keys are in the result
        required_keys = ['r', 'chir', 'chir_mag', 'chir_re', 'chir_im']
        for key in required_keys:
            assert key in result, f"Missing key in xftf result: {key}"
        
        # Check r-range
        r_min, r_max = min(result['r']), max(result['r'])
        print(f"xftf test: r range = {r_min:.2f} to {r_max:.2f} Å")
        print(f"Execution time: {elapsed:.6f} seconds")
        
        # Test with custom parameters
        custom_result = xt.xftf(bkg_result['k'], bkg_result['chi'], 
                              kmin=2, kmax=12, dk=1, window='hanning', kweight=2)
        
        # Verify dimensions
        assert len(result['r']) == len(result['chir_mag']), "r and chir_mag arrays have different lengths"
        assert len(result['chir_re']) == len(result['chir_im']), "chir_re and chir_im arrays have different lengths"
        
        print("✓ xftf test passed")
        return True
    except Exception as e:
        print(f"✗ xftf test failed: {e}")
        return False

def test_xftr():
    """Test xftr (reverse Fourier transform) function."""
    try:
        import pyxraytsubaki as xt
        
        # Create test data
        energy, mu, e0 = create_test_spectrum()
        
        # Perform the full processing chain
        norm_result = xt.pre_edge(energy, mu, e0=e0)
        bkg_result = xt.autobk(energy, norm_result['norm'], e0=e0)
        ft_result = xt.xftf(bkg_result['k'], bkg_result['chi'], kmin=2, kmax=12, dk=1, window='hanning', kweight=2)
        
        # Test xftr function
        start_time = time.time()
        
        # Default parameters - Use real part of chir for simplicity
        result = xt.xftr(ft_result['r'], ft_result['chir_re'])
        elapsed = time.time() - start_time
        
        # Check if required keys are in the result
        required_keys = ['k', 'chiq', 'chiq_re', 'chiq_im']
        for key in required_keys:
            assert key in result, f"Missing key in xftr result: {key}"
        
        # Check k-range
        k_min, k_max = min(result['k']), max(result['k'])
        print(f"xftr test: back-transformed k range = {k_min:.2f} to {k_max:.2f} Å⁻¹")
        print(f"Execution time: {elapsed:.6f} seconds")
        
        # Test with custom parameters
        custom_result = xt.xftr(ft_result['r'], ft_result['chir_re'], 
                              rmin=1, rmax=3, dr=0.1, window='hanning')
        
        # Verify dimensions
        assert len(result['k']) == len(result['chiq']), "k and chiq arrays have different lengths"
        assert len(result['chiq_re']) == len(result['chiq_im']), "chiq_re and chiq_im arrays have different lengths"
        
        print("✓ xftr test passed")
        return True
    except Exception as e:
        print(f"✗ xftr test failed: {e}")
        return False

def run_tests():
    """Run all functional API tests."""
    print("Testing functional API (xraylarch-compatible interface)...")
    print("-" * 60)
    
    tests = [
        test_find_e0,
        test_pre_edge,
        test_autobk,
        test_xftf,
        test_xftr
    ]
    
    results = []
    for test in tests:
        results.append(test())
        print("-" * 60)
    
    # Print summary
    print("\nTest Summary:")
    print(f"Passed: {results.count(True)}/{len(tests)}")
    print(f"Failed: {results.count(False)}/{len(tests)}")
    
    return all(results)

if __name__ == "__main__":
    success = run_tests()
    sys.exit(0 if success else 1)