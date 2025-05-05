#!/usr/bin/env python3
"""
Run all tests for the pyxraytsubaki module.
"""

import sys
import os
import subprocess
import time

def run_test_file(test_file):
    """Run a single test file and return success status."""
    start_time = time.time()
    print(f"Running {test_file}...")
    
    result = subprocess.run([sys.executable, test_file], capture_output=True, text=True)
    
    duration = time.time() - start_time
    
    if result.returncode == 0:
        print(f"✓ {test_file} passed in {duration:.2f}s")
        return True, result.stdout
    else:
        print(f"✗ {test_file} FAILED in {duration:.2f}s")
        print("\nOutput:")
        print(result.stdout)
        print("\nError:")
        print(result.stderr)
        return False, result.stdout + "\n" + result.stderr

def main():
    """Run all test files in the current directory."""
    current_dir = os.path.dirname(os.path.abspath(__file__))
    
    # Find all test files
    test_files = [f for f in os.listdir(current_dir) 
                 if f.startswith("test_") and f.endswith(".py") and f != os.path.basename(__file__)]
    
    test_files.sort()  # Run tests in alphabetical order
    
    print(f"Found {len(test_files)} test files to run:\n")
    for f in test_files:
        print(f"- {f}")
    print("\n" + "="*50 + "\n")
    
    # Run all tests
    results = []
    passed = 0
    failed = 0
    
    for test_file in test_files:
        test_path = os.path.join(current_dir, test_file)
        success, output = run_test_file(test_path)
        
        if success:
            passed += 1
        else:
            failed += 1
        
        results.append((test_file, success, output))
        print("-"*50 + "\n")
    
    # Print summary
    print("\n" + "="*50)
    print("TEST SUMMARY")
    print("="*50)
    
    for test_file, success, _ in results:
        status = "PASS" if success else "FAIL"
        print(f"{status}: {test_file}")
    
    print("\n" + "="*50)
    print(f"Tests passed: {passed}")
    print(f"Tests failed: {failed}")
    print(f"Total tests: {passed + failed}")
    print("="*50)
    
    return failed == 0

if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)