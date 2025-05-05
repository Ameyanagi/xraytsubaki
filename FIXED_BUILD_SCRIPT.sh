#!/bin/bash
# Quick build script with the fixes applied

set -e  # Exit immediately if a command exits with a non-zero status

echo "Setting up a simplified build for pyxraytsubaki..."

# Create a minimal project
BUILD_DIR="/tmp/py_xraytsubaki_minimal"
mkdir -p $BUILD_DIR/src
cd $BUILD_DIR

# Create Cargo.toml
cat > Cargo.toml << 'EOT'
[package]
name = "py_xraytsubaki"
version = "0.1.0"
edition = "2021"

[lib]
name = "py_xraytsubaki"
crate-type = ["cdylib"]

[dependencies]
numpy = "0.20.0"
pyo3 = { version = "0.20.2", features = ["extension-module"] }
EOT

# Create pyproject.toml
cat > pyproject.toml << 'EOT'
[build-system]
requires = ["maturin>=1.4,<2.0"]
build-backend = "maturin"

[project]
name = "py_xraytsubaki"
requires-python = ">=3.8"
classifiers = [
  "Programming Language :: Rust",
  "Programming Language :: Python :: Implementation :: CPython",
]
dynamic = ["version"]

[tool.maturin]
features = ["pyo3/extension-module"]
EOT

# Create a simple lib.rs
cat > src/lib.rs << 'EOT'
use pyo3::prelude::*;
use numpy::{PyArray1, PyReadonlyArray1};

#[pyfunction]
fn sum_arrays<'py>(py: Python<'py>, a: PyReadonlyArray1<f64>, b: PyReadonlyArray1<f64>) -> PyResult<&'py PyArray1<f64>> {
    let a_arr = a.as_array();
    let b_arr = b.as_array();
    
    if a_arr.shape() != b_arr.shape() {
        return Err(pyo3::exceptions::PyValueError::new_err("Arrays must have the same shape"));
    }
    
    let mut result = Vec::with_capacity(a_arr.len());
    for i in 0..a_arr.len() {
        result.push(a_arr[i] + b_arr[i]);
    }
    
    Ok(PyArray1::from_vec(py, result))
}

#[pyfunction]
fn hello_xraytsubaki() -> String {
    "Hello from XRayTsubaki Python bindings!".to_string()
}

#[pymodule]
fn py_xraytsubaki(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(sum_arrays, m)?)?;
    m.add_function(wrap_pyfunction!(hello_xraytsubaki, m)?)?;
    m.add("__version__", "0.1.0")?;
    Ok(())
}
EOT

# Create a virtual environment with uv
echo "Creating Python virtual environment with uv..."
uv venv
source .venv/bin/activate

# Enable PyO3 forward compatibility for newer Python versions
export PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1

# Install dependencies with uv
echo "Installing dependencies with uv..."
uv add maturin numpy

# Build with maturin
echo "Building the Python extension..."
maturin develop --release

# Test the package
echo "Testing the Python package..."
uv run python - <<EOT
import numpy as np
import py_xraytsubaki

print(f"Successfully imported py_xraytsubaki version {py_xraytsubaki.__version__}")
print(py_xraytsubaki.hello_xraytsubaki())

# Test array addition
a = np.array([1.0, 2.0, 3.0])
b = np.array([4.0, 5.0, 6.0])
result = py_xraytsubaki.sum_arrays(a, b)
print(f"Sum of arrays: {result}")
EOT

echo "Build completed successfully!"
echo "The Python package is available in the virtual environment at $BUILD_DIR/.venv"
echo "To use it, run: source $BUILD_DIR/.venv/bin/activate"