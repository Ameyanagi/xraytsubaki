#!/usr/bin/env python3
"""
Simple test to verify the module can be imported
"""

try:
    import xraytsubaki
    print("Successfully imported xraytsubaki")
    print(f"Version: {xraytsubaki.__version__ if hasattr(xraytsubaki, '__version__') else 'unknown'}")
    print(f"Available modules/functions: {dir(xraytsubaki)}")
except ImportError as e:
    print(f"Error importing xraytsubaki: {e}")
    print("You might need to build the Python bindings first.")

try:
    import py_xraytsubaki
    print("\nSuccessfully imported py_xraytsubaki")
    print(f"Available modules/functions: {dir(py_xraytsubaki)}")
except ImportError as e:
    print(f"\nError importing py_xraytsubaki: {e}")
    print("Alternative module name might not be available.")