import numpy as np
import py_xraytsubaki as xrt

# Test data
energy = np.linspace(17000, 18000, 1000)
mu = np.sin(np.linspace(0, 10, 1000)) + 1.0

# Test find_e0
print("Testing find_e0...")
e0 = xrt.find_e0(energy, mu)
print(f"E0: {e0}")

# Test pre_edge
print("\nTesting pre_edge...")
pre_edge_result = xrt.pre_edge(energy, mu)
print(f"Edge step: {pre_edge_result['edge_step']}")

# Test using builder pattern
print("\nTesting builder pattern...")
pre_edge_builder = xrt.PreEdgeBuilder().energy(energy).mu(mu)
pre_edge_result2 = pre_edge_builder.run()
print(f"Edge step (builder): {pre_edge_result2['edge_step']}")

# Test spectrum class
print("\nTesting PyXASSpectrum class...")
spectrum = xrt.PyXASSpectrum()
spectrum.set_energy_mu(energy, mu)
print(f"Energy points: {len(spectrum.energy())}")
print(f"Mu points: {len(spectrum.mu())}")

print("\nAll tests completed successfully!")