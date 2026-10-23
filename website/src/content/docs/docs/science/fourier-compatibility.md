---
title: "Fourier and Larch compatibility"
description: "Compare sampling conventions and legacy AUTOBK behavior."
audience: user
---

Version 0.2.4 adds an explicit Fourier sampling grid and corrects the legacy
AUTOBK derivative, addressing [#20](https://github.com/Ameyanagi/rexafs/issues/20).
The default [fixed-λ AUTOBK objective](/docs/science/autobk/) is unchanged.

## Legacy iterative AUTOBK

The historical clamp residual for an endpoint sample $i$ is
$w\,s(\mathbf c)\,\chi_i(\mathbf c)$, where $w$ is its fixed endpoint weight,
$\chi_i(\mathbf c)$ is the endpoint residual, and the scale is

$$
s(\mathbf c)=1+\frac{100}{L}\sum_{j=1}^{L}h_j(\mathbf c)^2 ,
$$

with $\mathbf h(\mathbf c)$ the $L$ real and imaginary low-R residual entries.
Its derivative with respect to one spline coefficient $c_k$ is

$$
\frac{\partial}{\partial c_k}\bigl[w\,s\,\chi_i\bigr]
=w\left(s\,\frac{\partial\chi_i}{\partial c_k}
+\chi_i\,\frac{\partial s}{\partial c_k}\right),
\qquad
\frac{\partial s}{\partial c_k}
=\frac{200}{L}\sum_{j=1}^{L}h_j\,\frac{\partial h_j}{\partial c_k}.
$$

Applying the expression to every coefficient builds the Jacobian. The second
equation follows by differentiating the sum of squared entries of $\mathbf h$;
the first follows from the product rule. Omitting
$\chi_i\,\partial s/\partial c_k$ differentiates a frozen scale instead of the
scale actually used by the residual. These are
derivatives of the [implemented legacy objective](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs/src/xafs/background.rs),
not an additional physical model. For normalization and Fourier conventions,
see [processing theory](/docs/science/processing/).

Both array backends include the previously omitted second term. The FFT head
and spectrum used by that term are computed once per Jacobian. The frozen-scale
linear design matrix still uses zero scale derivative. Residual weights, scale,
slices (including the legacy high-end exclusion of the last sample), and default
fixed-λ objective are unchanged. A failed/nonfinite legacy LM solve returns
an error instead of returning its last coefficients as a successful result.

Central differences cover 64 combinations of clamp length, endpoint weights,
empty/nonempty FFT heads and dynamic/frozen scale in each backend, including an
optional χ standard. Both cached and uncached spline-basis paths are exercised.
Relative Jacobian error must be below `1e-7`.

The regression spectrum also compares LM with a numerical Jacobian. Both
converge in 45 residual evaluations to objective `3.299013380277`, with χ relative
L2 difference below `1e-6`. Repeating the old omitted-term derivative takes 48
evaluations and stops at objective `3.767250202953`; its χ differs by 11.22%.
These are synthetic diagnostic results, not a claim of measured-data accuracy.

Recomputing a project that explicitly uses `LegacyLm`, legacy dog-leg, or a
fallback to them can therefore change its result. Keep the previous release
when reproducing that earlier numerical behavior. The current fixed-λ default
and legacy frozen-scale direct solves do not use this dynamic Jacobian.

## Backend scope

The default nalgebra backend implements the recommended FixedPenalty AUTOBK
objective. The optional Rust `ndarray-compat` feature selects a historical
backend with only Fixed/TwoPass clamp policies and a rounded, rather than
floored, automatic spline coefficient count. It does not expose `clamp_lambda`.
Matching the public FFT grid does not remove these background differences.
Desktop, Python and Wasm use the default backend.

Both public forward FFT implementations use the actual $\delta k/\sqrt{\pi}$
amplitude factor for the k step $\delta k$ after an unnormalized
negative-exponent FFT. FixedPenalty AUTOBK instead uses the fixed internal
reference multiplier $0.05/\sqrt{\pi}$; its selected k step still controls the
R axis. Legacy AUTOBK policies use the
actual kstep for both scale and R axis. These are separate objective conventions.

## Explicit output FFT grid

The default Rust backend, desktop, Python and JavaScript retain **Input grid**.
It weights and windows the supplied k samples without resampling. Projects and
serialized FFT settings that omit the new field retain this behavior.

Choose **Process › Transform → Advanced → sampling grid → Larch grid** for the extended
window domain used by XrayLarch. Equivalent API settings are:

```python
ft = rexafs.XrayFFTF()
ft.grid = "Larch"  # "Input" restores the historical default
spectrum.set_fft(ft).fft()
k_for_window, window = spectrum.kwin_k(), spectrum.kwin()
```

```javascript
const ft = new XrayFFTF();
ft.grid = "Larch";
spectrum.set_fft(ft).fft();
```

```rust
let ft = rexafs::XrayFFTF {
    grid: rexafs::FFTGrid::Larch,
    ..Default::default()
};
spectrum.set_fft(ft).fft()?;
```

Larch mode linearly resamples χ from k=0 at the selected FFT step, holding the
nearest endpoint value outside the supplied range. It constructs the window
through `max(last measured k, kmax + dk2)` using Larch's integer grid counts,
then truncates the weighted spectrum and window to the measured range before
zero-padding. It requires finite, increasing, nonnegative k and enough NFFT
points for the extended window. The background's k/χ arrays remain unchanged.
Use `kwin_k()` for the window axis when FFT and background steps differ; desktop
plots and thumbnails do this automatically.

The optional `ndarray-compat` backend already constructed an extended grid and
retains that default. Its output transform now applies the window it previously
calculated but accidentally omitted from the FFT multiplication. This bug fix
changes ndarray output transforms. Select `FFTGrid::Input` explicitly when
comparing its input-domain behavior with the default backend.

The choice is saved, participates in parameter copying, overrides, undo and cache
invalidation, and does not alter AUTOBK's internal transform or FEFF fitting's
separate transform model. Compatibility does not imply equivalence of different
background objectives or general FEFF fit results. rexafs continues to expose
the real FFT's Nyquist bin; Larch's public output convention omits that final bin.
