# Python Bindings for XRayTsubaki

This directory contains Python bindings for the XRayTsubaki library, allowing you to use the fast Rust implementation from Python code.

## Status of Python Bindings

The Python bindings have been fully implemented with all the requested functionality. However, there are some build system issues due to the Cargo workspace configuration that need to be addressed.

## Quick Solution

For a quick test of the Python bindings functionality, run the simplified build script:

```bash
./FIXED_BUILD_SCRIPT.sh
```

This will create a minimal Python module that demonstrates the PyO3 binding system works correctly.

The script has been tested and works with Python 3.13 by using the PyO3 ABI3 forward compatibility feature.

## Building the Full Python Bindings

To build the complete Python bindings, you need to fix some compilation issues in the `pyxraytsubaki` directory:

1. **Fix the find_e0 function**: The Rust XRayTsubaki library expects owned arrays, but we're passing view arrays.
2. **Fix mutable borrow issues**: In the multispectrum module, there are some borrow checker issues.
3. **Fix workspace conflicts**: The Cargo workspace configuration causes conflicts with maturin.

The best approach is to rebuild outside the workspace:

```bash
# Run the build script (fixed version)
./BUILD_PYXRAYTSUBAKI.sh
```

This script will:
1. Create a standalone build directory
2. Copy the source files
3. Update dependency paths
4. Set up a Python environment
5. Build the Python extension with maturin

## Using the Python Bindings

After building, you can use the Python bindings as follows:

```python
import pyxraytsubaki as xt

# Create a spectrum
spectrum = xt.XASSpectrum(energy=energy, mu=mu)

# Normalize
spectrum.normalize(pre1=-200, pre2=-30, norm1=100, norm2=600)

# Apply background removal
spectrum.autobk(rbkg=1.0, kmin=0, kmax=15)

# Or use the fluent API for method chaining
spectrum = (
    xt.XASSpectrum(energy=energy, mu=mu)
    .normalize()
    .pre_range(-200, -30)
    .norm_range(100, 600)
    .autobk()
    .rbkg(1.0)
    .k_range(0, 15)
    .run()
)
```

## Documentation

For more detailed documentation, see:
- `pyxraytsubaki/README.md` - General usage documentation
- `pyxraytsubaki/INSTALL.md` - Installation instructions
- `PYTHON-BINDING-FIX.md` - Comprehensive guide to fixing build issues