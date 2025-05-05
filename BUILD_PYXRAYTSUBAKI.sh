#!/bin/bash
# Build script for pyxraytsubaki using uv and maturin
# This script creates a standalone build environment outside of the workspace
# to avoid conflicts with Cargo workspace configuration.

set -e  # Exit immediately if a command exits with a non-zero status

# Create a new directory outside the workspace
echo "Creating a standalone build directory..."
BUILD_DIR="/tmp/pyxraytsubaki_build"
mkdir -p $BUILD_DIR/src
mkdir -p $BUILD_DIR/python/pyxraytsubaki

# Copy source files
echo "Copying source files..."
cp -r pyxraytsubaki/src/* $BUILD_DIR/src/
cp -r pyxraytsubaki/python/* $BUILD_DIR/python/
cp pyxraytsubaki/Cargo.toml $BUILD_DIR/
cp pyxraytsubaki/pyproject.toml $BUILD_DIR/
cp pyxraytsubaki/README.md $BUILD_DIR/

# Edit Cargo.toml to use absolute paths
echo "Updating dependency paths..."
REPO_PATH=$(pwd)
sed -i "s|path = \"../crates/xraytsubaki\"|path = \"$REPO_PATH/crates/xraytsubaki\"|g" $BUILD_DIR/Cargo.toml

# Create a virtual environment with uv
echo "Creating Python virtual environment with uv..."
cd $BUILD_DIR
uv venv
source .venv/bin/activate

# Enable PyO3 forward compatibility for newer Python versions
export PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1

# Install dependencies with uv
echo "Installing dependencies with uv..."
uv add maturin numpy

# Build with maturin in release mode with optimizations
echo "Building the Python extension..."
maturin develop --release

# Test the package
echo "Testing the Python package..."
uv run python -c "import pyxraytsubaki; print(f'Successfully imported pyxraytsubaki version {pyxraytsubaki.__version__}')"

# Create a wheel for distribution (optional)
echo "Creating a wheel for distribution..."
maturin build --release

echo "Build completed successfully!"
echo "The Python package is available in the virtual environment at $BUILD_DIR/.venv"
echo "To use it, run: source $BUILD_DIR/.venv/bin/activate"
echo "The wheel is available at $BUILD_DIR/target/wheels/"