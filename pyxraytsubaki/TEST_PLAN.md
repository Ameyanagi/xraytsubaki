# Test Plan for pyxraytsubaki

This document outlines the test plan for the Python bindings of XRayTsubaki.

## Prerequisites

Before running the tests, make sure you have:

1. Built the Python extension using either:
   - Quick build: `./FIXED_BUILD_SCRIPT.sh`
   - Full build: `./BUILD_PYXRAYTSUBAKI.sh`
   
2. Activated the virtual environment:
   ```bash
   source /tmp/pyxraytsubaki_build/.venv/bin/activate  # For full build
   # or
   source /tmp/py_xraytsubaki_minimal/.venv/bin/activate  # For minimal build
   ```

## Test Modules

### 1. Basic Module Functionality

- Test import of the module
- Verify version string
- Test numpy array interoperability

### 2. XASSpectrum Class Tests

- Creation of XASSpectrum objects with energy and mu data
- Normalization
- Background removal (AUTOBK)
- Forward Fourier transform
- Reverse Fourier transform
- File saving and loading
- Fluent API functionality

### 3. XASGroup Class Tests

- Creation of XASGroup objects
- Adding spectra to the group
- Group operations
- Iteration over group members

### 4. Fitting Module Tests

- Creation of fitting parameters
- Parameter constraints
- Simple path models
- Fitting dataset creation
- Model calculation
- Levenberg-Marquardt optimization

### 5. Multi-spectrum Fitting Tests

- Parameter constraints between multiple spectra
- Simultaneous fitting of multiple datasets
- Scaling and offset constraints

## Test Data Generation

The following synthetic test data will be created:

1. Single XAS spectrum with:
   - Known edge position (E0)
   - Known normalization parameters
   - Known EXAFS oscillations

2. Multiple XAS spectra with:
   - Common structural parameters (distance, phase)
   - Varying amplitude and disorder parameters

## Test Scripts

The tests are organized into the following scripts:

1. `test_import.py` - Basic import and functionality tests
2. `test_xasspectrum.py` - XASSpectrum class tests
3. `test_xasgroup.py` - XASGroup class tests
4. `test_fitting.py` - Fitting module tests
5. `test_multispectrum.py` - Multi-spectrum fitting tests

## Executing the Tests

To run the tests, execute the following from the root directory:

```bash
# Make sure you're in the virtual environment with the built extension
source /tmp/pyxraytsubaki_build/.venv/bin/activate

# Navigate to the test directory
cd /home/ryuichi/rust/xraytsubaki/pyxraytsubaki/tests

# Run individual test modules
python test_import.py
python test_xasspectrum.py
python test_xasgroup.py
python test_fitting.py
python test_multispectrum.py

# Or run all tests
python run_all_tests.py
```

## Expected Results

Each test script will output:
- "PASS" or "FAIL" for each test
- A summary of passed/failed tests
- Detailed error messages for any failures

All tests should pass, indicating that the Python bindings match the functionality of the Rust library.