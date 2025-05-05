"""
Test utilities for the pyxraytsubaki package.
"""

import os
import numpy as np

def create_synthetic_spectrum(e0=17500, n_points=1000):
    """Create a synthetic XAS spectrum for testing.
    
    Args:
        e0: The edge energy in eV
        n_points: Number of data points
        
    Returns:
        Tuple of (energy, mu) arrays
    """
    # Energy grid
    energy = np.linspace(e0 - 500, e0 + 500, n_points)
    
    # Parameters
    pre_edge_slope = 0.01
    edge_step = 1.0
    
    # Pre-edge: linear function
    pre_edge = 1.0 + pre_edge_slope * (energy - energy[0])
    
    # Edge step: tanh function
    edge = edge_step * 0.5 * (1 + np.tanh((energy - e0) / 10.0))
    
    # EXAFS oscillations
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
    
    return energy, mu_noisy