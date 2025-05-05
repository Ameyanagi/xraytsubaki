#!/usr/bin/env python3
"""
Example script demonstrating how to use the Python extension module
once it's been built with the FIXED_BUILD_SCRIPT.sh script.
"""

import sys
import time
import numpy as np

# Set the correct Python path to the virtual environment where the extension was built
sys.path.append('/tmp/py_xraytsubaki_minimal/.venv/lib/python3.13/site-packages')

try:
    import py_xraytsubaki
    print(f"Successfully imported py_xraytsubaki version {py_xraytsubaki.__version__}")
    
    # Example 1: Basic Hello World
    greeting = py_xraytsubaki.hello_xraytsubaki()
    print(f"\nExample 1: {greeting}")
    
    # Example 2: Simple Array Addition
    print("\nExample 2: Adding two arrays")
    a = np.array([1.0, 2.0, 3.0, 4.0, 5.0])
    b = np.array([5.0, 4.0, 3.0, 2.0, 1.0])
    result = py_xraytsubaki.sum_arrays(a, b)
    print(f"a     = {a}")
    print(f"b     = {b}")
    print(f"a + b = {result}")
    
    # Example 3: Working with mathematical operations
    print("\nExample 3: Advanced array operations")
    x = np.linspace(0, 2*np.pi, 10)
    y1 = np.sin(x)
    y2 = np.cos(x)
    sum_result = py_xraytsubaki.sum_arrays(y1, y2)
    
    # Display the data
    print(f"x values:       {x[:5]}...")
    print(f"sin(x):         {y1[:5]}...")
    print(f"cos(x):         {y2[:5]}...")
    print(f"sin(x) + cos(x): {sum_result[:5]}...")
    
    # Verify the result
    numpy_sum = y1 + y2
    is_correct = np.allclose(sum_result, numpy_sum)
    print(f"Result matches NumPy: {is_correct}")
    
    # Performance test
    print("\nPerformance test with larger arrays:")
    size = 1000000
    a = np.random.random(size)
    b = np.random.random(size)
    
    start = time.time()
    rust_result = py_xraytsubaki.sum_arrays(a, b)
    rust_time = time.time() - start
    
    start = time.time()
    numpy_result = a + b
    numpy_time = time.time() - start
    
    print(f"Array size: {size:,} elements")
    print(f"Rust time:  {rust_time:.6f} seconds")
    print(f"NumPy time: {numpy_time:.6f} seconds")
    print(f"Results match: {np.allclose(rust_result, numpy_result)}")
    
    print("\nExample completed successfully!")
    
except ImportError as e:
    print(f"Error importing py_xraytsubaki: {e}")
    print("Make sure you've run FIXED_BUILD_SCRIPT.sh first to build the extension.")
    sys.exit(1)
except Exception as e:
    print(f"Error during example: {e}")
    sys.exit(1)