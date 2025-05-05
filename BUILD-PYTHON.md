# Building Python Bindings for XRayTsubaki

This document provides instructions for building and testing the Python bindings for the XRayTsubaki library.

## Current Status

The Python bindings for XRayTsubaki are implemented in the `py-xraytsubaki` directory using PyO3. The current build setup has a few issues that need to be addressed:

1. The workspace structure is complex, with nested directories and workspace members
2. The Python extension module is not being built correctly with the existing build configuration
3. There are conflicts between the root `pyproject.toml` and the one in the `py-xraytsubaki` directory

## Steps to Build Python Bindings

### Prerequisites

1. Python 3.8+ with NumPy installed
2. Rust toolchain (cargo, rustc)
3. Maturin for building PyO3 extensions

### Recommended Build Process

1. **Create a Python virtual environment with uv**:
   ```bash
   uv venv
   source .venv/bin/activate
   uv add maturin numpy
   ```

2. **Fix pyproject.toml conflicts**:
   The root `pyproject.toml` should be updated to avoid conflicts with the one in `py-xraytsubaki`:
   - Ensure the root `pyproject.toml` includes a [build-system] section
   - Consider renaming or moving one of the conflicting files

3. **Build the extension directly with maturin**:
   ```bash
   cd py-xraytsubaki
   maturin develop
   ```

   If there are still issues, try building with specific options:
   ```bash
   maturin develop --release --strip
   ```

4. **Verify installation**:
   ```python
   import py_xraytsubaki
   # or
   import xraytsubaki  # depending on how the module is named
   ```

## Testing

Once the Python extension is built successfully, test the functionality with:

```bash
uv run python simple_test.py
```

The test script creates synthetic XAS data and tests the basic functionality of the library.

## Troubleshooting

If you encounter build issues:

1. Check for shared library output:
   ```bash
   find . -name "*.so" | grep -v "deps" | grep -v "venv"
   ```

2. Look for PyO3 compile errors in the build output

3. Try building with different PyO3 features:
   ```bash
   maturin develop --features "pyo3/extension-module"
   ```

4. Make sure the library name in `Cargo.toml` matches the expected import name

## Next Steps

1. Fix the build system to properly build the Python extension
2. Ensure all Python tests pass
3. Document the Python API
4. Create examples showing how to use the Python bindings