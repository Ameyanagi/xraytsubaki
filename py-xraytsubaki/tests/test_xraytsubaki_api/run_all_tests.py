#!/usr/bin/env python3
"""
Test runner for all XRayTsubaki Python API tests.
"""

import os
import sys
import unittest
import time

# Import all test modules
from test_xraylarch_api import TestXraytsubakiLarchAPI
from test_xasspectrum_class import TestXASSpectrumClass
from test_xasgroup_class import TestXASGroupClass
from test_fitting import TestFittingFunctionality
from test_multispectrum_fitting import TestMultispectrumFitting

def run_tests():
    """Run all tests and report results."""
    start_time = time.time()
    
    # Create test suite
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()
    
    # Add all test classes
    suite.addTests(loader.loadTestsFromTestCase(TestXraytsubakiLarchAPI))
    suite.addTests(loader.loadTestsFromTestCase(TestXASSpectrumClass))
    suite.addTests(loader.loadTestsFromTestCase(TestXASGroupClass))
    suite.addTests(loader.loadTestsFromTestCase(TestFittingFunctionality))
    suite.addTests(loader.loadTestsFromTestCase(TestMultispectrumFitting))
    
    # Run tests
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)
    
    elapsed = time.time() - start_time
    
    # Print summary
    print("\n" + "="*80)
    print(f"Test Summary:")
    print(f"  Ran {result.testsRun} tests in {elapsed:.2f} seconds")
    print(f"  Failures: {len(result.failures)}")
    print(f"  Errors: {len(result.errors)}")
    print(f"  Skipped: {len(result.skipped)}")
    
    # Return exit code based on test results
    return 0 if result.wasSuccessful() else 1

if __name__ == "__main__":
    sys.exit(run_tests())