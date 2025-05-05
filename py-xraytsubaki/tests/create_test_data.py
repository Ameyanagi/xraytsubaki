#!/usr/bin/env python3
"""
Create synthetic test data for testing the Python bindings.
"""

import os
import numpy as np
import matplotlib.pyplot as plt
from pathlib import Path

# Create test data directory
test_data_dir = Path(__file__).parent / "test_data"
test_data_dir.mkdir(exist_ok=True)

# Parameters for synthetic XAFS spectrum
def create_synthetic_spectrum():
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
        test_data_dir / "synthetic_xas.dat",
        np.column_stack((energy, mu_noisy)),
        header="# energy mu",
        fmt="%.6f %.6f"
    )
    
    # Create plot
    plt.figure(figsize=(10, 6))
    plt.plot(energy, mu_noisy, label="Spectrum with noise")
    plt.plot(energy, pre_edge, 'r--', label="Pre-edge")
    plt.plot(energy, pre_edge + edge, 'g--', label="Edge")
    plt.axvline(e0, color='k', linestyle=':', label=f"E0 = {e0} eV")
    plt.xlabel("Energy (eV)")
    plt.ylabel("Absorption (a.u.)")
    plt.legend()
    plt.tight_layout()
    plt.savefig(test_data_dir / "synthetic_xas.png")
    
    # Also save the true parameters for testing
    param_file = test_data_dir / "true_params.txt"
    with open(param_file, "w") as f:
        f.write(f"e0 = {e0}\n")
        f.write(f"edge_step = {edge_step}\n")
        f.write(f"amp1 = {amp1}\n")
        f.write(f"freq1 = {freq1}\n")
        f.write(f"phase1 = {phase1}\n")
        f.write(f"damp1 = {damp1}\n")
        f.write(f"amp2 = {amp2}\n")
        f.write(f"freq2 = {freq2}\n")
        f.write(f"phase2 = {phase2}\n")
        f.write(f"damp2 = {damp2}\n")
    
    return energy, mu_noisy, e0

def create_multi_spectrum_data():
    """Create multiple spectra with different parameters."""
    # Energy grid
    energy = np.linspace(17000, 18000, 1000)
    
    # Common parameters
    e0 = 17500
    pre_edge_slope = 0.01
    edge_step = 1.0
    freq = 1.5
    phase = 0.3
    
    # Parameters that vary across spectra (e.g., different temperatures)
    amplitudes = [0.8, 0.75, 0.7]  # decreasing amplitude
    dampings = [0.04, 0.045, 0.05]  # increasing disorder
    
    spectra = []
    
    for i in range(3):
        # Pre-edge and edge step (same for all spectra)
        pre_edge = 1.0 + pre_edge_slope * (energy - 17000)
        edge = edge_step * 0.5 * (1 + np.tanh((energy - e0) / 10.0))
        
        # EXAFS oscillations
        k_mask = energy > e0
        k = np.zeros_like(energy)
        k[k_mask] = np.sqrt((energy[k_mask] - e0) / 3.81)
        
        amp = amplitudes[i]
        damp = dampings[i]
        
        oscil = np.zeros_like(energy)
        oscil[k_mask] = amp * np.sin(2 * freq * k[k_mask] + phase) * np.exp(-damp * k[k_mask]**2)
        
        # Total mu
        mu = pre_edge + edge + oscil
        
        # Add some noise
        np.random.seed(12345 + i)
        noise = np.random.normal(0, 0.01, len(energy))
        mu_noisy = mu + noise
        
        spectra.append((energy, mu_noisy))
        
        # Save spectrum
        np.savetxt(
            test_data_dir / f"spectrum_{i+1}.dat",
            np.column_stack((energy, mu_noisy)),
            header="# energy mu",
            fmt="%.6f %.6f"
        )
    
    # Save the true parameters for testing
    param_file = test_data_dir / "multi_spectra_params.txt"
    with open(param_file, "w") as f:
        f.write(f"e0 = {e0}\n")
        f.write(f"edge_step = {edge_step}\n")
        f.write(f"freq = {freq}\n")
        f.write(f"phase = {phase}\n")
        for i, (amp, damp) in enumerate(zip(amplitudes, dampings)):
            f.write(f"amp{i+1} = {amp}\n")
            f.write(f"damp{i+1} = {damp}\n")
    
    # Plot all spectra
    plt.figure(figsize=(10, 6))
    for i, (_, mu) in enumerate(spectra):
        plt.plot(energy, mu, label=f"Spectrum {i+1}")
    plt.xlabel("Energy (eV)")
    plt.ylabel("Absorption (a.u.)")
    plt.legend()
    plt.tight_layout()
    plt.savefig(test_data_dir / "multi_spectra.png")
    
    return spectra

if __name__ == "__main__":
    print("Creating synthetic XAS spectrum...")
    energy, mu, e0 = create_synthetic_spectrum()
    print(f"Created synthetic spectrum with E0 = {e0} eV")
    
    print("Creating multi-spectrum dataset...")
    spectra = create_multi_spectrum_data()
    print(f"Created {len(spectra)} spectra with different parameters")
    
    print(f"Test data saved to {test_data_dir}")