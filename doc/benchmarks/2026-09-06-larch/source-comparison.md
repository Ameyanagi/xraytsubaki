# AUTOBK reference choices: Larch, rexafs and IFEFFIT

The code comparison below describes the released rexafs 0.1.0 behavior. The
selected next change replaces rounding with floor for automatic spline parameter
count. The subsequent [known-background clamp study](../2026-09-07-clamp-study/README.md)
supports retaining a direct solve; no new production clamp default has been
selected. This comparison supports the remaining compatibility decisions.

The benchmark uses published rexafs 0.1.0 and XrayLarch 2026.3.1. Its main matrix
passes rbkg=1 and kmax=12 to both packages. Larch rbkg=1.05 appears only in the
separately labeled causal diagnostics, to match the internally selected counts;
it was not used for the published-package speed measurements.

## Spline parameter count

Exact Larch Python statement:

```python
nspl = 1 + int(2*rbkg*(kmax-kmin)/np.pi)
```

[Source, Larch 2026.3.1](https://github.com/xraypy/xraylarch/blob/860d8a690c81eefb0e61dee4ca3703ef4b67e93d/larch/xafs/autobk.py#L159).
For positive arguments, Python int truncates.

Exact rexafs Rust statement:

```rust
let mut nspl = 1 + (2.0 * rbkg * (kmax - kmin) / std::f64::consts::PI).round() as i32;
```

[Source, rexafs 0.1.0](https://github.com/Ameyanagi/rexafs/blob/v0.1.0/crates/rexafs/src/xafs/background.rs#L750).
Rexafs explicitly rounds before converting to an integer.

IFEFFIT Fortran statement (leading fixed-format indentation omitted):

```fortran
nsplin = 2 * int(rbkg * (qmax - qmin)/ pi) + 1
```

[Source, spline.f line 221](https://github.com/keechul/ifeffit/blob/306444e500cb3ecb1795fcbde9219369b003f1fa/src/lib/spline.f#L221).
IFEFFIT truncates before multiplying by two. The source also snaps the fit range
to measured energies before deriving qmin/qmax, so the table below is an
illustration at an exact effective range of 12, not output from running IFEFFIT.

| Effective rbkg=1, k range=12 | Larch | Rexafs | IFEFFIT |
|---|---:|---:|---:|
| Parameter count before bounds | 8 | 9 | 7 |
| Calculation | 1 + floor(7.6394) | 1 + round(7.6394) | 1 + 2 floor(3.8197) |

These differences change the fitted background's flexibility. They are not
required by Python, Rust or Fortran; the programs choose different formulas.

## Low-R residual selection

Larch's statement is:

```python
irbkg = int(1 + (nspl-1)*np.pi/(2*rgrid*(kmax-kmin)))
```

Rexafs uses the same expression with floating-point rounding:

```rust
let irbkg = (1.0 + (nspl - 1) as f64 * std::f64::consts::PI / (2.0 * rgrid * (kmax - kmin)))
    .round() as i32;
```

See the linked Larch and rexafs count code above. Both compute this cutoff before
applying an explicit nknots override, so setting nknots alone does not guarantee
that their low-R objectives match.

IFEFFIT uses a different rule based directly on rbkg:

```fortran
nrbkg = 2 * int(1d-2 + (rbkg /rgrid))+ 2
```

[Source, spline.f line 119](https://github.com/keechul/ifeffit/blob/306444e500cb3ecb1795fcbde9219369b003f1fa/src/lib/spline.f#L119).
Its nrbkg counts real residual components, whereas Larch/rexafs irbkg counts
complex FFT bins. With nfft=2048, kstep=0.05 and the values above, the corresponding
counts are 30 complex bins for Larch, 35 for rexafs, and 66 real components
(33 complex bins) for IFEFFIT. Those are different fit objectives.

## Clamp scale and endpoints

Larch's exact scale statement:

```python
scale = 0.1 + 10*(out*out).mean()
```

[Source, autobk.py line 42](https://github.com/xraypy/xraylarch/blob/860d8a690c81eefb0e61dee4ca3703ef4b67e93d/larch/xafs/autobk.py#L42).
It recalculates the scale and uses the final nclamp points of uniform-k residuals.
The residual is formed before edge-step division.

Rexafs' scale expression:

```rust
1.0 + 100.0 * out.dot(&out) / out.len() as f64
```

Its high-end selection:

```rust
let high_start = chi.len() - nclamp - 1;
let high_clamp = self.clamp_hi as f64 * scale * chi.view((high_start, 0), (nclamp, 1));
```

[Source, background.rs](https://github.com/Ameyanagi/rexafs/blob/v0.1.0/crates/rexafs/src/xafs/background.rs#L1096).
This selects one uniform-k point earlier. The default direct solver freezes the
initial scale, two-pass updates it once, and iterative solvers recalculate it.
At identical residuals its scale is ten times Larch's, hence a hundred times the
squared endpoint penalty. Actual fits have different residuals and models, so
that factor does not predict their final output error.

IFEFFIT's high-end clamp scale statement:

```fortran
fclamp = sclamp(2)*(1+int(tresid*100.d0)/nrbkg)/nclamp
```

[Source, splfun.f line 148](https://github.com/keechul/ifeffit/blob/306444e500cb3ecb1795fcbde9219369b003f1fa/src/lib/splfun.f#L148).
Here tresid is the low-R residual sum of squares for the no-standard case.
The int result and nrbkg are integers: their division is integer division, making
the multiplier change in steps. It is also divided by nclamp. IFEFFIT applies
clamps to endpoint points on the measured energy grid, using an already
edge-step-normalized residual (or functional normalization when selected).
Thus copying its constants into rexafs would not reproduce IFEFFIT's clamping.

## What a compatibility choice entails

| Target | What should be matched | Main use |
|---|---|---|
| Larch 2026.3.1 | Count/cutoff selection, residual interpolation, endpoint grid/slice, scale formula/update and normalization conventions | Reproducing the current comparison package's results |
| IFEFFIT 1.2.13 | Its full energy-grid spline/background and normalization pipeline, count/cutoff rules, interpolation and quantized clamps | Reproducing legacy IFEFFIT analyses |
| Explicit rexafs method | Document and validate the chosen objective; preserve existing-project behavior and expose the intended approximation | An independently specified method with measured accuracy and speed tradeoffs |

My recommendation is to use a versioned Larch compatibility target for a
reference mode, then optimize the same mathematical objective and measure both
speed and numerical agreement. A direct solve with a frozen dynamic clamp is an
approximation to a different objective and should be labeled accordingly.
Legacy IFEFFIT compatibility should be separately specified if reproducing those
analyses is a requirement. This is a recommendation for discussion, not an
implemented or selected default.

The missing dynamic-clamp Jacobian chain-rule term is a separate correctness
issue, confirmed in the [clamping investigation](clamping-0.1.0/README.md).
Fixing that derivative does not require choosing Larch versus IFEFFIT's model.
Existing saved projects must retain their processing semantics across any future
change of default; such a change needs versioned settings and regression fixtures.

## IFEFFIT source verification

The original newville/ifeffit GitHub repository currently returns 404. I downloaded
the [1.2.13 source archive retained by MacPorts](https://distfiles.macports.org/ifeffit/ifeffit-1.2.13.tar.gz)
and verified its SHA-256 against the
[MacPorts package definition](https://github.com/macports/macports-ports/blob/master/science/ifeffit/Portfile):

`79fa938643a1417c5b01be4b6196bd0ea6bf40685448ba98546c7989b0f48a48`

Its spline.f, splfun.f and consts.h are byte-identical to the archived GitHub
snapshot linked above (commit 306444e500cb3ecb1795fcbde9219369b003f1fa). Therefore
those links show the verified released code, despite the mirror's older date.
The archive root is newville-ifeffit-83b3455. This is source inspection; I have
not built or run IFEFFIT on the benchmark scans.
