import numpy as np
import sys
import os
import subprocess

# Build the module using cargo directly with the library crate-type
print("Building the module using cargo...")
subprocess.run(["cargo", "build", "--release"], cwd=os.path.dirname(__file__))

# Directly adding the .so file to the Python path
lib_dir = os.path.join(os.path.dirname(__file__), "target/release")
sys.path.append(lib_dir)

# Create a symlink from libpyxraytsubaki.so to pyxraytsubaki.so if it doesn't exist
lib_file = os.path.join(lib_dir, "libpyxraytsubaki.so")
so_file = os.path.join(lib_dir, "pyxraytsubaki.so")

if os.path.exists(lib_file) and not os.path.exists(so_file):
    print(f"Creating symlink from {lib_file} to {so_file}")
    try:
        os.symlink(os.path.basename(lib_file), so_file)
    except Exception as e:
        print(f"Error creating symlink: {e}")

try:
    import pyxraytsubaki
    print("Successfully imported pyxraytsubaki")
except Exception as e:
    print(f"Failed to import pyxraytsubaki: {e}")
    sys.exit(1)

# Test data
energy = np.linspace(17000, 18000, 1000)
mu = np.sin(np.linspace(0, 10, 1000)) + 1.0

# Test find_e0
print("\nTesting find_e0...")
try:
    e0 = pyxraytsubaki.find_e0(energy, mu)
    print(f"E0: {e0}")
except Exception as e:
    print(f"Error in find_e0: {e}")

# Test pre_edge
print("\nTesting pre_edge...")
try:
    pre_edge_result = pyxraytsubaki.pre_edge(energy, mu)
    print(f"Edge step: {pre_edge_result['edge_step']}")
except Exception as e:
    print(f"Error in pre_edge: {e}")

# Test using builder pattern
print("\nTesting builder pattern...")
try:
    pre_edge_builder = pyxraytsubaki.PreEdgeBuilder()
    pre_edge_builder.energy(energy)
    pre_edge_builder.mu(mu)
    pre_edge_result2 = pre_edge_builder.run()
    print(f"Edge step (builder): {pre_edge_result2['edge_step']}")
except Exception as e:
    print(f"Error in builder pattern: {e}")

# Test spectrum class
print("\nTesting PyXASSpectrum class...")
try:
    spectrum = pyxraytsubaki.PyXASSpectrum()
    spectrum.set_energy_mu(energy, mu)
    print(f"Energy points: {len(spectrum.energy())}")
    print(f"Mu points: {len(spectrum.mu())}")
except Exception as e:
    print(f"Error in PyXASSpectrum: {e}")

print("\nAll tests completed!")