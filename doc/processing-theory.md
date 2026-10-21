# How spectrum processing works

This guide explains the calculations behind `Spectrum.normalize()`,
`calc_background()`, `fft()` and `ifft()`. The equations below describe the
current default Rust backend used by the desktop, Python and Wasm bindings.
The optional ndarray backend has some different historical conventions; see
[FFT compatibility](fft-grid-compatibility.md). `XrayFFTR` and `set_ifft()` in
the bindings are additions after 0.2.4.

X-ray absorption spectroscopy (XAS) measures absorption as a function of photon
energy. Extended X-ray absorption fine structure (EXAFS) is the oscillatory
part above an absorption edge, caused by scattering of the emitted
photoelectron. The physical theory is reviewed by
[Rehr and Albers, *Reviews of Modern Physics* 72, 621–654 (2000)](https://doi.org/10.1103/RevModPhys.72.621).

## 1. Construct the absorption spectrum

For a homogeneous transmission sample, the Beer–Lambert relation gives

$$
I_t(E)=I_0(E)\exp[-\mu_{\mathrm{phys}}(E)t],
\qquad
\mu_{\mathrm{API}}(E)=\ln\!\frac{I_0(E)}{I_t(E)}.
$$

Here, $E$ is photon energy in electronvolts (eV); $I_0$ and $I_t$ are incident
and transmitted intensities in matching units; $t$ is sample thickness; and
$\mu_{\mathrm{phys}}$ is an absorption coefficient with units of inverse
length. The quantity called `mu` by the transmission reader is the dimensionless
optical depth $\mu_{\mathrm{phys}}t$, rather than an absolute absorption
coefficient. The logarithm is natural, and both intensities must be positive.
The physical measurement conventions are introduced in
[Newville, *Fundamentals of XAFS*](https://docs.xrayabsorption.org/tutorials/XAFS_Fundamentals.pdf).

The [QAS reader](../crates/rexafs/src/xafs/io/mod.rs) reads energy, $I_0$ and
$I_t$ from the first three columns and evaluates this logarithm. A general
`Spectrum(energy, mu)` accepts already constructed absorption, including other
measurement modes. It does not infer detector corrections from the arrays.
Normalization removes an overall absorption scale; it does not correct every
experimental distortion.

## 2. Find the edge and normalize the absorption

The edge energy $E_0$ defines the energy origin. Automatic edge finding uses
the absorption derivative with checks against spurious features. It is an
estimate, not an independent energy calibration. Inspect it for noisy data,
multiple edges or narrow scans, and use `set_e0()` when an explicit value is
required. An explicit E0 used through `Spectrum` must be finite and strictly
inside the measured energy range; normalization reports an error otherwise.
The lower-level Rust `PrePostEdge::fill_parameter` has a different contract:
it replaces nonfinite E0 values or values outside the first through penultimate
energy samples with an automatic estimate. See the [edge-finding implementation](../crates/rexafs/src/xafs/xafsutils.rs).

The pre-edge model is

$$
p(E)=(a+bE)E^{-v},\qquad v=\mathtt{n\_victoreen}.
$$

The coefficients $a$ and $b$ are obtained by fitting a line to $\mu(E)E^v$
in the selected pre-edge interval. Their units depend on $v$ so that $p(E)$
has the same units as the input absorption. With the automatic $v=0$, this is
an ordinary straight-line background.

A polynomial $P(E)$ is fitted to $\mu(E)-p(E)$ in the post-edge interval.
The returned `post_edge()` is $p(E)+P(E)$. Let $E_*$ be the measured energy
nearest $E_0$. The estimated edge step is $P(E_*)$, unless `edge_step` supplies
an explicit value. Using the resolved step $\Delta\mu$, the outputs are

$$
\mu_{\mathrm{norm}}(E)=\frac{\mu(E)-p(E)}{\Delta\mu},
$$

$$
\mu_{\mathrm{flat}}(E)=
\begin{cases}
\mu_{\mathrm{norm}}(E), & E<E_*,\\
\mu_{\mathrm{norm}}(E)-\dfrac{P(E)-P(E_*)}{\Delta\mu}, & E\ge E_*.
\end{cases}
$$

Normalization subtracts the pre-edge background and expresses absorption in
edge-step units. Flattening additionally removes the fitted post-edge trend
while preserving its value at the edge sample. These outputs are dimensionless.
The implementation requires a finite step and floors it at $10^{-12}$; a
near-zero or negative estimated jump therefore still needs scientific review.
Flattening is a separate output, not the input substituted into AUTOBK.

`PrePostEdge()` in Python/TypeScript and `PrePostEdge::new()` in Rust select
fit ranges from available data. Rust `PrePostEdge::default()` instead starts
with fixed offsets of −200 to −30 eV and 150 to 2000 eV, and degree 2.
Choose `new()` when you want the automatic behavior described here. Its automatic polynomial
degree is 0, 1 or 2 for post-edge spans below 50 eV, below 350 eV, or at least
350 eV. Explicit degrees are limited to 0–5. All four fit-window endpoints are
**offsets from $E_0$ in eV**, not absolute energies. The exact formulas and range
handling are in [normalization.rs](../crates/rexafs/src/xafs/normalization.rs).
The general method follows pre-edge subtraction and edge-step normalization;
see [Larch's normalization reference](https://xraypy.github.io/xraylarch/xafs_preedge.html).
The equations here describe rexafs and should not be read as a claim that every
Larch default or flattening detail is identical.

**Short scans (unreleased; planned for 0.2.15).** A fit window may reach beyond
the measured energies, for example the desktop defaults of −200 to −30 eV and
150 eV upward on a scan that starts 15 eV before $E_0$ or ends 120 eV after it.
Normalization then limits the window to the data instead of failing. The stored
settings keep the requested values, so a longer scan with the same settings is
unaffected. Let $\Delta E_{\min}$ and $\Delta E_{\max}$ be the first and last
measured energies minus $E_0$, in eV. The rules follow Larch `pre_edge`
([xraylarch `pre_edge.py`, lines 232–259](https://github.com/xraypy/xraylarch/blob/247278cefbb16eda04c91f79fc1b43df000fa498/larch/xafs/pre_edge.py#L232-L259)),
which applies the first two to explicit values as well as automatic ones:

1. The pre-edge start becomes $\max(\text{start}, \Delta E_{\min})$.
2. The post-edge end becomes $\min(\text{end}, \Delta E_{\max})$.
3. If the whole pre-edge window lies below the data, its end becomes half the
   limited start, and the pre-edge is a constant equal to the mean absorption in
   that window. This fallback is a rexafs choice. It borrows Larch's automatic
   `pre2 = pre1/2` and Larch's constant pre-edge for short windows, but Larch
   would instead swap such explicit bounds, and it switches to a constant
   whenever the window spans fewer than three samples. rexafs uses a constant
   only in this wholly-below-data case; a short window inside the data stays a
   line, as in earlier releases. A line fitted to a few eV just below the edge
   would be extrapolated far into the post-edge region and distort the
   normalized spectrum.
4. If the post-edge start lies at or beyond the limited end, it becomes Larch's
   automatic value $\min(25, 5\,\mathrm{round}(\text{end}/15))$ eV. The start is
   always kept at least 10 eV below the end; this margin is rexafs's existing
   rule, whereas the cited Larch revision keeps 2 eV.
5. The post-edge polynomial degree is capped at 1 when the window holds fewer
   than five points, as in Larch (`pre_edge.py`, lines 297–301), because a
   higher-degree polynomial through a few points amplifies noise when it is
   extrapolated back to $E_0$.

A reversed range, where the start is at or above the end inside the data, is an
error naming the setting instead of being swapped, as is a window that still
holds fewer than two measured points; so is a non-finite endpoint.
`Spectrum::range_adjustments()` (Rust) recomputes, from the current settings and
energy grid, each limited setting with its requested and used values. The desktop application
shows the used value under an explicitly set field in amber and adds one warning
per group to the Problems drawer; an automatic field shows the used value in its
placeholder. These limits only make the calculation possible. A normalization
fitted to a few eV of pre-edge or post-edge data is less reliable than one fitted
to a long scan, so inspect `pre_edge()`, `post_edge()` and the edge step.

The unreleased full [MBACK normalization](mback-normalization.md) provides a
separate atomic-reference method with an explicit absorber and edge, positive
fitted scale, strict fit regions and retained reference identity. Its matched
scattering curve `fpp` is distinct from its dimensionless `norm` and `flat`.
Conventional polynomial normalization remains the default.

## 3. Convert energy to wave number and remove the smooth background

For energies above the edge, the nonrelativistic photoelectron relation is

$$
k=\frac{\sqrt{2m_e(E-E_0)}}{\hbar},
\qquad
k\,[\mathrm{\AA}^{-1}]
=\sqrt{\frac{(E-E_0)\,[\mathrm{eV}]}{3.809982110968585}}.
$$

Here, $m_e$ is the electron mass and $\hbar$ is the reduced Planck constant;
the first expression uses coherent SI units. The numerical expression includes
the eV and angstrom conversions. rexafs uses the
[NIST CODATA 2022 constants](https://physics.nist.gov/cuu/pdf/wall_2022.pdf).
This real-valued expression is for $E\ge E_0$; the background pipeline selects
the relevant post-edge data and produces a uniform k grid.

AUTOBK represents the smooth absorption background by a cubic spline **in k**,
evaluates it on the energy grid as $\mu_0(E)$, and forms

$$
\chi(k)=\frac{\mu(E(k))-\mu_0(E(k))}{\Delta\mu}.
$$

Both absorption terms use the original input scale; $\Delta\mu$ is the
normalization edge step, so $\chi$ is dimensionless. A smooth background
contributes mainly at low Fourier distance R. AUTOBK adjusts its spline to
reduce that low-R contribution while limiting the spline's flexibility.
This is the method introduced by
[Newville et al., *Physical Review B* 47, 14126–14131 (1993)](https://doi.org/10.1103/PhysRevB.47.14126);
[Larch's AUTOBK reference](https://xraypy.github.io/xraylarch/xafs_autobk.html)
explains the role of the background cutoff.

The recommended starting cutoff is `rbkg=1.0` Å. Increasing it gives the
background fit access to a wider R region and can remove real structural
signal. It is not a nearest-neighbor distance estimate. Background weighting
(`kweight=1`, `window="Hanning"`) is separate from the public forward-transform
weighting (`kweight=2`, `window="KaiserBessel"`).

The spline range runs from `kmin` to `kmax` in Å⁻¹. An explicit `kmax` above
the largest measured wave number $k_{\max}=\sqrt{(E_{\max}-E_0)/3.81\ \mathrm{eV\,Å^2}}$
is capped at $k_{\max}$, as in Larch AUTOBK. The stored setting keeps the
requested value, and since 0.2.15 (unreleased) `range_adjustments()` reports
the cap. A `kmin` at or beyond the usable end is an error naming `kmin`. The
Cauchy wavelet transform similarly limits its selected k range to the measured
k range; the map keeps the requested definition, and `WaveletMap::k_support()`
and the map warnings report the interval used. A range wholly outside the data
is an error.

rexafs's default **FixedPenalty** objective adds a weak, fixed endpoint term
and solves one linear least-squares problem. The value `clamp_lambda=0.001` is
an empirical project default, not a physical constant or a result proved by the
1993 paper. The [fixed-penalty guide](autobk-fixed-penalty.md) defines the exact
objective, row scaling, solver and validation. Its use of cached geometry and
singular-value decomposition is visible in
[fixed.rs](../crates/rexafs/src/xafs/background/fixed.rs).

## 4. Transform from k to R

**The forward amplitude factor is $\delta k/\sqrt{\pi}$**, where $\delta k$ is
the k step (`kstep`), applied after an unnormalized forward FFT. There is no additional division by `nfft` or
`sqrt(nfft)`, and no extra factor of $i$. This is the convention implemented by
[xftf_fast_nalgebra](../crates/rexafs/src/xafs/xrayfft.rs).

To see where the sampling interval enters, write the continuous convention as

$$
\widetilde\chi(R)=\frac{1}{\sqrt{\pi}}
\int_0^\infty \chi(k)k^w W_k(k)\exp(-2ikR)\,dk.
$$

Here $k$ is photoelectron wave number in Å⁻¹, $R$ is Fourier distance in Å,
$w$ is the nonnegative integer k-weight, $W_k$ is a dimensionless window and
$i^2=-1$. The kernel uses $2kR$, as in the EXAFS scattering oscillation; an
uncorrected Fourier peak is still affected by the scattering phase and is not
directly a bond length. See [Rehr and Albers](https://doi.org/10.1103/RevModPhys.72.621)
for that physical distinction. The $1/\sqrt{\pi}$ prefactor specifies rexafs's
amplitude convention; it is not a measured quantity or a window correction.

On the standard zero-origin uniform grid $k_j=j\delta k$, the integral is
approximated by a sum with sample width $\delta k$. rexafs constructs
$g_j=\chi(k_j)k_j^w W_k(k_j)$ and evaluates

$$
\widetilde\chi_m=\frac{\delta k}{\sqrt{\pi}}
\sum_{j=0}^{N-1}g_j\exp\!\left(-\frac{2\pi i jm}{N}\right),
\qquad R_m=\frac{\pi m}{N\delta k}.
$$

$N$ is `nfft`, $\delta k$ is `kstep` in Å⁻¹, $j$ indexes input samples and
$m$ indexes Fourier bins. The two exponent forms agree because
$2k_jR_m=2\pi jm/N$. Samples beyond the prepared data are zero. In Input mode,
if the data are longer than $N$, only the first $N$ samples are transformed;
choose `nfft` large enough to retain all intended data.

For an already weighted and windowed real array `g`, the equivalent NumPy
calculation is:

```python
import numpy as np

chi_r = (kstep / np.sqrt(np.pi)) * np.fft.rfft(g, n=nfft, norm="backward")
r = np.arange(chi_r.size) * np.pi / (nfft * kstep)
```

NumPy's `norm="backward"` means the forward transform is unscaled; its $1/N$
factor belongs to the inverse transform. See the
[NumPy FFT normalization reference](https://numpy.org/doc/stable/reference/routines.fft.html#normalization).
At `kstep=0.05`, rexafs's explicit multiplier is approximately
**0.02820947918**, independent of `nfft`. The same multiplier and exponent sign
appear in [Larch's `xftf_fast` implementation](https://github.com/xraypy/xraylarch/blob/860d8a690c81eefb0e61dee4ca3703ef4b67e93d/larch/xafs/xafsft.py#L298).
Use that implementation when comparing conventions: a textbook transform or a
manual equation with a different sign or normalization is not interchangeable.
There is no normalization by the window area, peak height or sum of weights.

For dimensionless $\chi$, $\widetilde\chi$ has units
$\mathrm{\AA}^{-(w+1)}$; with the default $w=2$, these are Å⁻³.
`r()` reports R in Å, and `chir_real()`, `chir_imag()` and `chir_mag()` report
the real part, imaginary part and magnitude of $\widetilde\chi$. The real FFT
retains bins $0$ through $\lfloor N/2\rfloor$, including the Nyquist bin for
even $N$. The spectrum getters show only the range selected by `rmax_out`
(default 10 Å); the full coefficients remain available for inverse filtering.

Increasing k-weight emphasizes higher-k data, including its noise. Fractional
`kweight` settings are floored to an integer. The window reduces ringing caused
by sharp truncation. Window parameters are shape-dependent: `dk` describes taper
geometry but also controls Kaiser–Bessel shape; Gaussian `dk` controls its width,
and fractional Hanning uses fractional taper parameters. Equal `dk` does not
make different window families equivalent. See the
[window implementation](../crates/rexafs/src/xafs/xafsutils.rs) and
[Larch's window reference](https://xraypy.github.io/xraylarch/xafs_fourier.html#ftwindow-generating-fourier-transform-windows).

`XrayFFTF` defaults to `nfft=2048` and automatic `kstep`, which uses the first
input spacing. The usual `kstep=0.05` Å⁻¹ comes from AUTOBK's default output
grid. Together these give adjacent R samples approximately **0.03068 Å** apart.
Resolved settings are retained after calculation: automatic does not mean
continuously linked to a later background grid. Assign a fresh `XrayFFTF` (or
clear its `kstep` in the lower-level Rust API) when the input spacing changes.
Increasing `nfft` by adding zeros makes this display grid finer while preserving
amplitudes at shared R samples. It does not add experimental information or
resolve arbitrarily close shells.

`grid="Input"` keeps the prepared background grid. For a physical R-axis
interpretation, that grid should be uniform, start at zero and agree with
`kstep`; the low-level Input API accepts other increasing grids without
resampling or an origin-phase correction. `grid="Larch"` resamples from zero
and changes window construction. It does not change the returned background
`k()` or `chi()`. Use `kwin_k()` with `kwin()`. The
[compatibility guide](fft-grid-compatibility.md) specifies those conventions.

## 5. Filter in R and transform back to q

`XrayFFTR` selects an R window $W_R$ and an R-weight $u=\mathtt{rweight}$.
Fractional nonnegative R-weights are floored to an integer.
For an unchanged transform length, define

$$
H_m=\widetilde\chi_m W_R(R_m)R_m^u,
\qquad
\chi_q(j)=\frac{\sqrt{\pi}}{N\delta k}
\sum_{m=0}^{N-1}H_m\exp\!\left(\frac{2\pi i jm}{N}\right).
$$

$W_R$ is dimensionless, $u$ is a nonnegative exponent, and $H_m$ is the
windowed, R-weighted Fourier coefficient. For dimensionless input chi, the
returned signal has units $\mathrm{\AA}^{u-w}$; at the default $u=0$ it
retains the forward k-weighted signal's units.

The full sum uses the conjugate-symmetric extension of the real signal's
positive-frequency coefficients. rexafs therefore returns a **real** array
from `chiq()`. Here $q_j=j\delta k$ has units Å⁻¹. With R-weight zero and an
all-pass R window, the scale factors cancel and the result recovers the
forward-weighted/windowed signal $g_j$, not generally the original unweighted
$\chi(k_j)$. R filtering discards information; it does not undo normalization,
background removal or the forward window.

Choose `rmin` and `rmax` for the shell region of interest, and use `dr`/`dr2`
to control its window. `qmax_out` limits the returned q range. If inverse
`nfft` differs from the forward length, the implementation resizes the real
Fourier spectrum and infers the consistent q spacing from R; leave `kstep`
automatic unless you deliberately supply a compatible value. Once inferred,
that value is retained; assign fresh `XrayFFTR` settings or reset its kstep to
None when changing the inverse length or input R spacing. See
[inverse_fft.rs](../crates/rexafs/src/xafs/inverse_fft.rs).

## What processing does not establish

These stages produce a normalized spectrum and its Fourier representations.
Structural interpretation requires a scattering model and an assessment of
fit quality, parameter correlations and systematic errors. The
[fitting-statistics guide](fitting-statistics.md) explains those distinctions.
For practical code and parameter defaults, use the [API guide](api.md),
[Python guide](../py-rexafs/README.md) or [TypeScript guide](../js-rexafs/README.md).
