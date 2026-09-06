"""Regenerate independent spline values/derivatives with SciPy (no Rust imports).

Run with: uv run --python 3.12 --with scipy python <this-file>
"""
import json
from pathlib import Path

import numpy as np
import scipy
from scipy.interpolate import BSpline, splrep, splev

x = np.array([-2.0, -1.4, 0.1, 0.3, 1.8, 4.0, 4.1, 8.0])
y = np.array([0.5, -1.2, 3.1, 2.3, -0.7, 1.4, 0.3, 2.0])
query = np.array([-3.0, -2.0, -1.7, 0.2, 2.1, 4.05, 8.0, 9.0])
t, c, k = splrep(x, y, k=3, s=0)
count = len(t) - k - 1

def basis(points):
    # Each unit coefficient vector evaluates one column of dS/dc.
    matrix = BSpline(t, np.eye(count), k)(points)
    return np.pad(matrix, ((0, 0), (0, k + 1))).tolist()

record = dict(
    generator=f"SciPy {scipy.__version__}, splrep(k=3,s=0), splev, BSpline",
    x=x.tolist(), y=y.tolist(), query=query.tolist(),
    knots=t.tolist(), coefficients=c.tolist(),
    extrapolated=splev(query, (t, c, k), ext=0).tolist(),
    clamped=splev(query, (t, c, k), ext=3).tolist(),
    extrapolated_basis=basis(query),
    clamped_basis=basis(np.clip(query, x[0], x[-1])),
)
output = Path(__file__).resolve().parents[1] / "testfiles/spline_scipy_reference.json"
output.write_text(json.dumps(record, indent=2) + "\n")
print(output)
