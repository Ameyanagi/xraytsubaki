#!/usr/bin/env python3
"""
Basic import and functionality tests for the pyxraytsubaki module.
"""

import sys
import os
import numpy as np

def run_tests():
    """Run all import tests."""
    tests_passed = 0
    tests_failed = 0
    
    # Test 1: Basic import
    print("Test 1: Import pyxraytsubaki module")
    try:
        import pyxraytsubaki
        print("✓ Successfully imported pyxraytsubaki")
        tests_passed += 1
    except ImportError as e:
        print(f"✗ Failed to import pyxraytsubaki: {e}")
        print("  Make sure you've built the module and are in the correct virtual environment.")
        tests_failed += 1
        return tests_passed, tests_failed
    
    # Test 2: Check version
    print("\nTest 2: Check version")
    try:
        version = pyxraytsubaki.__version__
        print(f"✓ Version: {version}")
        if not isinstance(version, str) or not version:
            raise ValueError("Version is not a valid string")
        tests_passed += 1
    except (AttributeError, ValueError) as e:
        print(f"✗ Failed to get version: {e}")
        tests_failed += 1
    
    # Test 3: Import XASSpectrum class
    print("\nTest 3: Import XASSpectrum class")
    try:
        from pyxraytsubaki import XASSpectrum
        print("✓ Successfully imported XASSpectrum class")
        tests_passed += 1
    except ImportError as e:
        print(f"✗ Failed to import XASSpectrum class: {e}")
        tests_failed += 1
    
    # Test 4: Import XASGroup class
    print("\nTest 4: Import XASGroup class")
    try:
        from pyxraytsubaki import XASGroup
        print("✓ Successfully imported XASGroup class")
        tests_passed += 1
    except ImportError as e:
        print(f"✗ Failed to import XASGroup class: {e}")
        tests_failed += 1
    
    # Test 5: Import fitting classes
    print("\nTest 5: Import fitting classes")
    try:
        from pyxraytsubaki import FittingParameters, SimplePath, FittingDataset, ExafsFitter
        print("✓ Successfully imported fitting classes")
        tests_passed += 1
    except ImportError as e:
        print(f"✗ Failed to import fitting classes: {e}")
        tests_failed += 1
    
    # Test 6: Import multi-spectrum fitting classes
    print("\nTest 6: Import multi-spectrum fitting classes")
    try:
        from pyxraytsubaki import ConstrainedParameters, MultiSpectrumDataset, MultiSpectrumFitter
        print("✓ Successfully imported multi-spectrum fitting classes")
        tests_passed += 1
    except ImportError as e:
        print(f"✗ Failed to import multi-spectrum fitting classes: {e}")
        tests_failed += 1
    
    # Test 7: Import direct functions
    print("\nTest 7: Import direct functions")
    try:
        from pyxraytsubaki import find_e0, pre_edge, autobk, xftf, xftr
        print("✓ Successfully imported direct functions")
        tests_passed += 1
    except ImportError as e:
        print(f"✗ Failed to import direct functions: {e}")
        tests_failed += 1
    
    # Test 8: NumPy integration
    print("\nTest 8: Check NumPy integration")
    try:
        # Create simple arrays
        energy = np.linspace(17000, 18000, 100)
        mu = np.sin(energy / 1000) + 1.0
        
        # Test with find_e0 function which accepts NumPy arrays
        e0 = pyxraytsubaki.find_e0(energy, mu)
        print(f"✓ Successfully called find_e0 function, result: {e0:.2f} eV")
        tests_passed += 1
    except Exception as e:
        print(f"✗ Failed to test NumPy integration: {e}")
        tests_failed += 1
    
    return tests_passed, tests_failed

if __name__ == "__main__":
    print("Running basic import tests for pyxraytsubaki...\n")
    
    try:
        tests_passed, tests_failed = run_tests()
        
        print("\n" + "="*50)
        print(f"Tests passed: {tests_passed}")
        print(f"Tests failed: {tests_failed}")
        print(f"Total tests: {tests_passed + tests_failed}")
        print("="*50)
        
        if tests_failed > 0:
            sys.exit(1)
        
    except Exception as e:
        print(f"Error running tests: {e}")
        sys.exit(1)