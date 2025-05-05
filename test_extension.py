#!/usr/bin/env python3
"""
Test the Python extension module we built with the FIXED_BUILD_SCRIPT.
"""

import sys
import numpy as np

# Set the correct Python path in the test script
sys.path.append('/tmp/py_xraytsubaki_minimal/.venv/lib/python3.13/site-packages')

try:
    import py_xraytsubaki
    print(f"Successfully imported py_xraytsubaki version {py_xraytsubaki.__version__}")
    
    # Test hello function
    print(f"Greeting: {py_xraytsubaki.hello_xraytsubaki()}")
    
    # Test array operations
    print("\nTesting array operations:")
    a = np.array([1.0, 2.0, 3.0, 4.0, 5.0])
    b = np.array([5.0, 4.0, 3.0, 2.0, 1.0])
    result = py_xraytsubaki.sum_arrays(a, b)
    print(f"a        = {a}")
    print(f"b        = {b}")
    print(f"a + b    = {result}")
    print(f"Expected = {a + b}")
    print(f"Result matches expected: {np.array_equal(result, a + b)}")
    
    # Test with larger arrays
    print("\nTesting with larger arrays:")
    large_a = np.random.random(1000)
    large_b = np.random.random(1000)
    large_result = py_xraytsubaki.sum_arrays(large_a, large_b)
    expected = large_a + large_b
    print(f"First 5 values of result:  {large_result[:5]}")
    print(f"First 5 values expected:   {expected[:5]}")
    print(f"Result matches expected:   {np.array_equal(large_result, expected)}")
    
    # Test error handling
    print("\nTesting error handling:")
    try:
        c = np.array([1.0, 2.0, 3.0])
        d = np.array([1.0, 2.0, 3.0, 4.0])
        py_xraytsubaki.sum_arrays(c, d)
        print("ERROR: Should have raised an exception for arrays of different sizes")
    except Exception as e:
        print(f"Correctly raised exception: {e}")
    
    print("\nAll tests completed successfully!")
    
except ImportError as e:
    print(f"Error importing py_xraytsubaki: {e}")
    print("Make sure you've run FIXED_BUILD_SCRIPT.sh first to build the extension.")
    sys.exit(1)
except Exception as e:
    print(f"Error during testing: {e}")
    sys.exit(1)