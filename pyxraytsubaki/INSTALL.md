# Installing pyxraytsubaki

This document provides instructions for installing the `pyxraytsubaki` Python package.

## Option 1: Quick Install Using the Build Script

We provide a build script that automates the installation process:

```bash
# Make the script executable if it's not already
chmod +x BUILD_PYXRAYTSUBAKI.sh

# Run the build script
./BUILD_PYXRAYTSUBAKI.sh
```

This script will:
1. Create a standalone build directory outside of the workspace
2. Set up a Python virtual environment using uv
3. Build the Python extension with maturin
4. Create a wheel for distribution
5. Test the installation

After running the script, you can activate the virtual environment with:
```bash
source /tmp/pyxraytsubaki_build/.venv/bin/activate
```

## Option 2: Manual Installation

If you prefer to perform the installation manually, follow these steps:

### Prerequisites

- Python 3.8 or later
- Rust and Cargo
- uv package manager
- maturin

### Step 1: Set up a Virtual Environment

```bash
# Create a directory for the build
mkdir -p /tmp/pyxraytsubaki_build
cd /tmp/pyxraytsubaki_build

# Create a virtual environment with uv
uv venv
source .venv/bin/activate

# Install dependencies
uv add maturin numpy
```

### Step 2: Prepare Source Files

```bash
# Create the necessary directories
mkdir -p src python/pyxraytsubaki

# Copy source files from the repository
cp -r /path/to/xraytsubaki/pyxraytsubaki/src/* src/
cp -r /path/to/xraytsubaki/pyxraytsubaki/python/* python/
cp /path/to/xraytsubaki/pyxraytsubaki/Cargo.toml .
cp /path/to/xraytsubaki/pyxraytsubaki/pyproject.toml .
cp /path/to/xraytsubaki/pyxraytsubaki/README.md .

# Update dependency paths in Cargo.toml
sed -i 's|path = "../crates/xraytsubaki"|path = "/path/to/xraytsubaki/crates/xraytsubaki"|g' Cargo.toml
```

### Step 3: Build and Install

```bash
# Build the Python extension
maturin develop --release

# Test the installation
uv run python -c "import pyxraytsubaki; print(f'Successfully imported pyxraytsubaki version {pyxraytsubaki.__version__}')"
```

## Option 3: Install from Wheel (Once Available)

In the future, we plan to distribute pre-built wheels on PyPI:

```bash
# Create a virtual environment
uv venv
source .venv/bin/activate

# Install from PyPI (once available)
uv add pyxraytsubaki
```

Or install from a local wheel:

```bash
uv pip install --no-deps /path/to/wheel/pyxraytsubaki-0.1.0-*.whl
```

## Troubleshooting

If you encounter issues during the installation:

1. **Workspace Conflicts**: If you're building from within the original repository, the Cargo workspace configuration may cause conflicts. Use the build script or the manual installation method to create a standalone build environment.

2. **Rust Dependencies**: Ensure you have a compatible Rust version (1.62 or later is recommended).

3. **Python Version**: This package requires Python 3.8 or later.

4. **Build Tools**: Make sure you have the necessary build tools for your platform (e.g., build-essential on Ubuntu).

For additional help, please open an issue on the GitHub repository.