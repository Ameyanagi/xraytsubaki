# Fixed-λ AUTOBK

New analyses use `LinearDirect` with `clamp_scale_policy = FixedPenalty` and
`clamp_lambda = 0.001`. This implements the weak fixed penalty tested in the
[clamp study](benchmarks/2026-09-07-clamp-study/README.md). The study's training
minimum was λ=0, with weak penalties practically tied; 0.001 is a conservative
weak-penalty choice, not a universally optimal value.

For spline coefficients **c**, let χ(c) be the edge-step-normalized spectrum,
minus an optional normalized χ standard **for the fitting objective only**.
Let **h(c)** contain the real and imaginary low-R Fourier residuals. The objective is

```text
J(c) = ||h(c)||² / m
       + λ × [w_lo² Σ χ(first n points)² + w_hi² Σ χ(last n points)²] / N_active

m        = number of real/imaginary low-R residual entries
n        = min(nclamp, number of χ(k) points)
N_active = n × (number of nonzero endpoint weights)
```

A zero weight excludes that end from both the sum and its denominator. If both
weights are zero, `nclamp = 0`, or `clamp_lambda = 0`, the endpoint term is absent.
The weights are the absolute values of `clamp_lo` and `clamp_hi`.

Defaults are `nclamp = 3`, `clamp_lo = 0`, and `clamp_hi = 1`: no low-end penalty,
and λ times the mean χ² of the final three points, including the last point.
At kstep=0.05 and kmax=12 these are k=11.90, 11.95, and 12.00 Å⁻¹. If enabled,
the first three points are k=0, 0.05, and 0.10 Å⁻¹. These are endpoints of the
output k grid; kmin controls the transform window and spline domain.

Doubling λ doubles the endpoint contribution to the objective. Doubling an
endpoint weight multiplies that end's contribution by four. Enabling both ends
shares the total penalty across twice as many endpoint points. There is no
initial-residual scale, update during a fit, second pass, or nonlinear fallback.

The low-R transform uses the k-weight and window selected for AUTOBK, with the
reference FFT amplitude factor `0.05 / sqrt(pi)` used by Larch's AUTOBK residual.
The selected kstep and nfft determine the physical R grid and cutoff. The fixed
reference amplitude also makes the reported λ directly comparable with the
prototype at both tested k steps; changing the window or k-weight changes the
objective and can change the effective balance with the endpoint term.

## Implementation

Energy-to-k conversion uses the CODATA 2022 electron mass, `9.1093837139e-31 kg`,
with the exact SI Planck constant and elementary charge. This gives
`E - E0 = 3.809982110968585 × k²` (eV and Å⁻¹), matching the current SciPy/Larch
reference. Both Rust array backends and Athena export share the same constants.
See [NIST CODATA 2022](https://physics.nist.gov/cuu/pdf/wall_2022.pdf).

The coefficient count uses `1 + floor(2 rbkg (kmax-kmin) / pi)`, bounded to 5–128,
with optional explicit nknots. The low-R cutoff uses floor. Cubic not-a-knot
interpolation and polynomial extrapolation reproduce the Larch model used in
the study. Raw-data interpolation uses O(n) memory/time rather than a dense
n-by-n solve. When possible, resampling the spline basis is eliminated using
the fact that its interior knots are a subset of the raw interpolant's knots;
otherwise each basis column is resampled explicitly in O(n).

The endpoint residual rows are multiplied by `sqrt(λ m / N_active)`, then the
single linear coefficient least-squares system is solved with column-scaled
SVD. Rank, conditioning, finite values, and stationarity are checked. A failure
returns an error; it never changes λ or inserts regularization to force a result.
Compatible spline/FFT geometry can be cached, with exact geometry comparison.
The solution is invariant to consistent scaling of μ and the edge step.

```python
bkg = rexafs.AUTOBK()
bkg.clamp_lambda = 0.001  # default; 0 disables the endpoint penalty
bkg.clamp_lo = 0
bkg.clamp_hi = 1
bkg.nclamp = 3
```

```rust
let mut bkg = rexafs::prelude::AUTOBK::new();
bkg.clamp_lambda = Some(0.001);
bkg.clamp_lo = Some(0);
bkg.clamp_hi = Some(1);
```

The JavaScript AUTOBK object exposes the same `clamp_lambda` property. In the
desktop app, select **Clamps & window → clamp model → Fixed λ**, then edit
**clamp λ**. An empty λ field uses 0.001.

## Compatibility

`Fixed` and `TwoPass` retain the historical direct-solver scale models and their
regularization/fallback behavior. They are distinct from `FixedPenalty`.
Iterative legacy solvers require a legacy clamp policy; the API rejects a
`FixedPenalty`/iterative-solver combination. The desktop selector switches the
paired solver/model setting to keep that combination valid.

Older saved projects with no clamp-model field retain the legacy `Fixed` model.
New projects explicitly save `FixedPenalty`, even when other defaults are omitted.
The parameter fingerprint, scoped copying, overrides, and undo history include
the model and λ. The separate legacy ndarray compatibility implementation retains
its previous behavior. The existing legacy dynamic-clamp Jacobian issue is not
addressed by this change; the new fixed objective does not use that Jacobian.
