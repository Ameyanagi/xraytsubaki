#!/usr/bin/env python3
"""
Simple test script to verify Python/Rust interop with numpy only.
"""

import os
import sys
import numpy as np
from pathlib import Path

# Create test data directory
test_dir = Path("test_data")
test_dir.mkdir(exist_ok=True)

# Create synthetic XAS spectrum
def create_test_data():
    print("Creating synthetic XAS spectrum...")
    
    # Energy grid
    energy = np.linspace(17000, 18000, 1000)
    
    # Parameters
    e0 = 17500
    pre_edge_slope = 0.01
    edge_step = 1.0
    
    # Synthetic XAFS spectrum
    # Pre-edge: linear function
    pre_edge = 1.0 + pre_edge_slope * (energy - 17000)
    
    # Edge step: tanh function
    edge = edge_step * 0.5 * (1 + np.tanh((energy - e0) / 10.0))
    
    # EXAFS oscillations
    # Convert E to k
    k_mask = energy > e0
    k = np.zeros_like(energy)
    k[k_mask] = np.sqrt((energy[k_mask] - e0) / 3.81)
    
    # Multiple paths
    amp1, freq1, phase1, damp1 = 0.4, 1.5, 0.3, 0.04
    amp2, freq2, phase2, damp2 = 0.2, 2.8, 0.6, 0.03
    
    oscil = np.zeros_like(energy)
    oscil[k_mask] = (
        amp1 * np.sin(2 * freq1 * k[k_mask] + phase1) * np.exp(-damp1 * k[k_mask]**2) +
        amp2 * np.sin(2 * freq2 * k[k_mask] + phase2) * np.exp(-damp2 * k[k_mask]**2)
    )
    
    # Total mu
    mu = pre_edge + edge + oscil
    
    # Add some noise
    np.random.seed(12345)
    noise = np.random.normal(0, 0.01, len(energy))
    mu_noisy = mu + noise
    
    # Save spectrum
    np.savetxt(
        test_dir / "synthetic_xas.dat",
        np.column_stack((energy, mu_noisy)),
        header="# energy mu",
        fmt="%.6f %.6f"
    )
    
    print(f"Created synthetic spectrum with E0 = {e0} eV")
    return energy, mu_noisy, e0

# Try to import the Python bindings
try:
    # First, check if the module exists by trying to get its spec
    import importlib.util
    spec = importlib.util.find_spec("py_xraytsubaki")
    if spec is None:
        print("Python module 'py_xraytsubaki' not found.")
        print("You need to build the Python bindings first.")
        sys.exit(1)
    else:
        print("Module 'py_xraytsubaki' exists, trying to import...")
        import py_xraytsubaki as xt
        print("Successfully imported py_xraytsubaki")
except ImportError as e:
    print(f"Error importing py_xraytsubaki: {e}")
    print("You might need to build the Python bindings first.")
    sys.exit(1)

def main():
    # Create test data
    energy, mu, e0 = create_test_data()
    
    print("\nTest data created successfully. Module imports work, but we need to build and install the extension properly.")
    print("Next steps would be to use maturin to build the extension and then run the full test suite.")

if __name__ == "__main__":
    main()