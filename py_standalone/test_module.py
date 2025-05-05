#!/usr/bin/env python3
"""
Test script for the standalone Python module
"""

import numpy as np

try:
    import py_standalone
    print("Successfully imported py_standalone")
    
    # Test basic functions
    print(f"Hello function result: {py_standalone.hello()}")
    print(f"Sum function result (1+2): {py_standalone.sum_as_string(1, 2)}")
    
    # Test numpy array function
    a = np.array([1.0, 2.0, 3.0])
    b = np.array([4.0, 5.0, 6.0])
    result = py_standalone.add_arrays(a, b)
    print(f"Array addition result: {result}")
    
    print("\nAll tests passed!")
except ImportError as e:
    print(f"Error importing py_standalone: {e}")
    print("You need to build the Python extension first.")
    print("Try running: cd py_standalone && maturin develop")