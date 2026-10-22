# What fit statistics and uncertainties mean

This guide describes the current Rust/desktop EXAFS fitting implementation.
Python and TypeScript currently expose spectrum processing, not these fitting
classes. Processing produces chi(k); fitting adjusts a scattering-path model
to that signal. A converged optimizer has satisfied numerical stopping rules.
It has not established that the chosen structural model is unique or correct.

## From a scattering path to χ(k)

A path file supplies a reference half-path length $R_0$, multiplicity $N_p$,
and tabulated scattering amplitude, phase, and electron propagation terms.
For single scattering, $R_0$ is the absorber–scatterer distance; for multiple
scattering, it is half the full closed path length. Changing a path parameter
adjusts this reference calculation; it does not recalculate the electronic
structure or relax atomic coordinates. The scattering interpretation follows
[Rehr and Albers (2000)](https://doi.org/10.1103/RevModPhys.72.621); the exact
expression below is traced to [path_model.rs](../crates/rexafs/src/xafs/fitting/path_model.rs).

| Path parameter | Meaning and units | Loaded-path default |
|---|---|---|
| `degen` | Dimensionless path multiplicity $N_p$ | Value in the path file |
| `s02` | Dimensionless amplitude reduction $S_0^2$ | 1 |
| `e0` | Relative threshold shift $\Delta E_0$ in eV | 0 |
| `ei` | Imaginary energy correction $E_i$ in eV | 0 |
| `deltar` | Half-path-length change $\Delta R$ in Å | 0 |
| `sigma2` | Second distance cumulant $\sigma^2$ in Å² | 0 |
| `third` | Third distance cumulant $C_3$ in Å³ | 0 |
| `fourth` | Fourth distance cumulant $C_4$ in Å⁴ | 0 |

These are reference-model defaults, not measured values or universal fit bounds.
In particular, the fitted `e0` correction is distinct from the absolute edge
energy used during background removal. Increasing positive $\sigma^2$ usually
damps high-k structure; $\Delta R$ changes the oscillation phase and geometric
amplitude. $N_p$ and $S_0^2$ multiply each other and cannot be independently
identified from that product alone.

Away from its small-denominator guards, rexafs defines

$$
q=\operatorname{sgn}(k^2-\alpha\Delta E_0)
  \sqrt{|k^2-\alpha\Delta E_0|},
\qquad
p_c=\sqrt{\left(p_{\mathrm{re}}(q)+i/\lambda(q)\right)^2+i\alpha E_i}.
$$

Here $k$, the shifted real wave number $q$, and the complex momentum $p_c$
have units Å⁻¹; $\alpha=2m_e/\hbar^2$ is `ETOK` in Å⁻²/eV, with the required
unit conversion included. $p_{\mathrm{re}}$ is the tabulated real momentum,
$\lambda$ is the mean free path in Å, and $i^2=-1$. The square root of the
complex expression uses the principal branch. The model's dimensionless
complex contribution is

$$
\mathcal X(k)=
\frac{N_pS_0^2 F(q)}{q(R_0+\Delta R)^2}
\exp\!\left[
-2R_0\operatorname{Im}p_c
-2p_c^2\left(\sigma^2-\frac{p_c^2C_4}{3}\right)
+i\left\{2qR_0+\phi(q)
+2p_c\left(\Delta R-\frac{2\sigma^2}{R_0}
-\frac{2p_c^2C_3}{3}\right)\right\}
\right],
\qquad \chi(k)=\operatorname{Im}\mathcal X(k).
$$

$F=\mathtt{mag\_feff}\,\mathtt{red\_fact}$ is the reduced amplitude in Å;
$\phi=\mathtt{real\_phc}+\mathtt{pha\_feff}$ is the total phase in radians.
Every term in the exponential is dimensionless. The first real term attenuates
the electron's propagation, the cumulant term describes distance disorder,
and the imaginary term determines the oscillation phase. A fit sums $\chi(k)$
over enabled paths before applying fit weights and windows. The cumulant
expansion is a truncated description of disorder, not an arbitrary distance
distribution; large corrections can invalidate the reference-path model.

The code interpolates FEFF columns with cubic splines and extrapolates the
end polynomial pieces outside tabulated coverage. Use the available FEFF k
range when possible. Near zero, numerical guards regularize the energy,
momentum, mean-free-path, and distance denominators. The first complex sample
is replaced by $2\mathcal X_1-\mathcal X_2$, even on a grid that starts above
zero. These details are part of the implementation, rather than additional
physical terms in the equation. `FeffFlavor::Feff85L` selects the supported
file format, including compatible output from current FEFF10/ReFEFF runners;
the separate `FeffFlavor::Feff10` parser option is currently unsupported.

## Residuals and shared parameters

For dataset $d$, let $\boldsymbol\theta$ be the vector of independent fitted
parameters and let $\mathbf r_d(\boldsymbol\theta)$ be its real residual vector.
The residual includes the selected fit space, windows, k-weights and noise
scales. Complex R-space residuals include real and imaginary components
in that vector. Each element is a scaled data–model discrepancy. The optimizer
minimizes

$$
S(\boldsymbol\theta)=\sum_d\|\mathbf r_d(\boldsymbol\theta)\|_2^2.
$$

The squared Euclidean norm means the sum of the squared entries. A global
parameter appears in several datasets' models; a local parameter appears only
where assigned. Concatenating the residuals couples datasets through their
shared parameters. Independent batch fits instead solve separate problems.
The implementation is in
[solver.rs](../crates/rexafs/src/xafs/fitting/solver.rs) and
[transform.rs](../crates/rexafs/src/xafs/fitting/transform.rs); the
[multiple-spectrum guide](joint-fitting.md) describes the user controls.

## Fit-space and noise conventions

`FitSpace::R` is the default: the fit concatenates real and imaginary differences
after k- and R-windowing. `K` uses $k^w\chi(k)$ without the k-window. `Q` uses
the real part of an R-filtered back-transform. Its analytic half-spectrum
convention is implemented by `larch_chiq`; it differs from the processing
library's real, Hermitian `ifft()`. A magnitude-only R-space comparison is not
the objective used by `FitSpace::R`.

The default `FeffFitTransform` has k = 0–20 Å⁻¹, R = 1–3 Å, $w=2$,
2048 FFT points, and k spacing 0.05 Å⁻¹. Fitting accepts a nonnegative real
weight directly; the processing FFT floors its weight to an integer. The fit
expects prepared uniform k samples and does not resample them. Set the ranges
to the usable measured interval. Multiple weights add residual blocks but do
not multiply the independent-information estimate.

The Rust fit API uses a supplied per-weight `epsilon_ks` entry, then scalar
`epsilon_k`, then **1.0** as its k-space residual divisor. It does not estimate
noise automatically. Calling `estimate_noise()` is a separate operation.
For weight $w$, the R-space residual divisor is

$$
\epsilon_R=\frac{\epsilon_k}{2}
\sqrt{\frac{\delta k\left(k_{\max}^{a}-k_{\min}^{a}\right)}{\pi a}},
\qquad a=2w+1.
$$

$\delta k$, $k_{\min}$, and $k_{\max}$ use Å⁻¹; $a$ is dimensionless.
$\epsilon_k$ is the numerical scale supplied to the k residual and
$\epsilon_R$ is the corresponding numerical divisor used for R and Q.
These are implementation conventions; the code does not check that a supplied
scale is a calibrated uncertainty with units appropriate to every weighted
representation. Positive floors of $10^{-12}$ prevent zero divisors, but do
not make arbitrary scales a measurement-noise model. Set `kstep` explicitly
on a nondefault grid: when it is `None`, transform spacing is inferred from
the input, while this noise conversion still assumes 0.05 Å⁻¹.

`estimate_noise()` instead measures the root mean square per real/imaginary
component in the high-R interval 15–30 Å, corrected by the mean k-window.
Writing that result as $\widehat\epsilon_R$, its returned k-space scale is

$$
\widehat\epsilon_k=\widehat\epsilon_R
\sqrt{\frac{2\pi a}{\delta k\left(k_{\max}^{a}-k_{\min}^{a}\right)}}.
$$

The hat distinguishes this estimated quantity from the residual divisor.
The two conversions are **not inverses**: inserting $\widehat\epsilon_k$ into
the residual formula gives $\widehat\epsilon_R/\sqrt2$, before numerical guards.
This documents the current code's scaling; it is not an extra renormalization
of the measured spectrum. High-R noise estimation assumes the selected region
contains noise rather than structural signal, detector artifacts, or transform
leakage. Inspect that assumption before interpreting reduced chi-square.

## Independent information and reported chi-square

Denser k/R grids do not create more independent experimental information.
rexafs uses the following bandwidth estimate for each dataset:

$$
N_{\mathrm{idp},d}=1+\frac{2\Delta k_d\Delta R_d}{\pi},
\qquad N_{\mathrm{idp}}=\sum_dN_{\mathrm{idp},d}.
$$

$\Delta k_d=k_{\max,d}-k_{\min,d}$ is the selected k span in Å⁻¹, and
$\Delta R_d=R_{\max,d}-R_{\min,d}$ is the selected R span in Å. Their product
and $N_{\mathrm{idp}}$ are dimensionless. The additive **1** is the convention
implemented by `compute_n_idp`; it is not a claim that every published counting
convention uses that offset. The underlying information-counting issue is
examined by [Stern, *Physical Review B* 48, 9825–9827 (1993)](https://doi.org/10.1103/PhysRevB.48.9825).

Let $M$ be the number of scalar entries in the combined residual and $p$ the
number of independently varying parameters. For $M>0$ and $N_{\mathrm{idp}}>p$,
rexafs reports

$$
\chi^2_{\mathrm{reported}}=S\frac{N_{\mathrm{idp}}}{M},
\qquad
\chi^2_{\mathrm{reduced}}=
\frac{\chi^2_{\mathrm{reported}}}{N_{\mathrm{idp}}-p}.
$$

These are EXAFS reporting conventions, not simply the optimizer's raw sum $S$.
The code floors the denominator at $10^{-12}$ to avoid division by zero.
That numerical guard does not make a model with too many parameters
statistically identifiable. Noise estimates, overlapping windows, repeated
measurements and model inadequacy affect interpretation. See
[Larch's EXAFS fitting reference](https://xraypy.github.io/xraylarch/xafs_feffit.html)
for related reporting conventions; the equations above are traced to rexafs.

Reduced chi-square is a goodness-of-fit measure only when each residual is
divided by the measurement uncertainty $\epsilon$; a model that matches the
data within that noise then gives a value near 1. IFEFFIT and Larch estimate
$\epsilon$ from the high-R part of $|\chi(R)|$, where little structural
signal is expected ([Newville, *J. Synchrotron Rad.* 8, 322–324 (2001)](https://doi.org/10.1107/S0909049500016964);
[Larch `feffit`](https://xraypy.github.io/xraylarch/xafs_feffit.html)).
The desktop application does this only when **ε from noise** is on. When it is
off, the residual uses the unit divisor above, and the reduced chi-square
reflects the arbitrary scale of the weighted $\chi$ rather than the fit
quality. The desktop fit results, fit history, Markdown fit report, publication
methods and batch-fit table therefore show **n/a (noise not estimated)**, and
the batch CSV leaves that cell empty. This is a rexafs display choice: the
stored value and the fit itself are unchanged. Turn on **ε from noise** and
refit to report it.

## Covariance and standard errors

The Jacobian has entries
$J_{ij}=\partial r_i/\partial\theta_j$: each column describes how residuals
change when one parameter changes. It has $M$ rows and $p$ columns. rexafs
approximates parameter covariance at the fitted point by

$$
C\approx\frac{S}{N_{\mathrm{idp}}-p}(J^TJ)^{-1},
\qquad
\operatorname{stderr}(\theta_j)=\sqrt{C_{jj}}.
$$

This is a local linear approximation, with the same denominator guard as above.
The implementation scales the inverse normal matrix by **raw** $S$, not by
$\chi^2_{\mathrm{reported}}$. Covariance element $C_{ij}$ has units equal to
the product of the units of parameters $i$ and $j$; a standard error has the
same units as its parameter. It is not automatically a 95% confidence interval.
The [least-squares derivation](../supportinginfo/uncertainty.md) explains the
Jacobian approximation and its assumptions. SciPy likewise distinguishes an
inverse-curvature estimate from variance-scaled covariance in its
[`leastsq` documentation](https://docs.scipy.org/doc/scipy/reference/generated/scipy.optimize.leastsq.html).

For nonzero variances, the correlation coefficient is

$$
\rho_{ij}=\frac{C_{ij}}{\sqrt{C_{ii}C_{jj}}}.
$$

It is dimensionless. A value near +1 or −1 means that the two parameter
estimates can compensate for each other locally. In rexafs, rows and columns
follow `varying_names`. Covariance can be absent when the inverse cannot be
formed; a finite matrix can still be poorly conditioned. Bounds, nonlinear
parameter dependence and systematic errors can make local symmetric errors
misleading. See [likelihood and limitations](../supportinginfo/uncertainty2.md).

## R-factor

In the same transformed/scaled residual representation, let $\mathbf d$ be
the data vector and $\mathbf m$ the model vector. rexafs uses

$$
R_{\mathrm{factor}}=
\frac{\|\mathbf d-\mathbf m\|_2^2}{\|\mathbf d\|_2^2}.
$$

The combined value sums the dataset numerators and denominators before taking
the ratio. It measures a relative squared discrepancy, not a probability that
the model is correct. The code returns zero for a numerically zero denominator;
that special value must not be interpreted as a good fit to an absent signal.
Compare values only with compatible fit spaces, weighting and data selection.

## What to inspect before interpreting a fit

Inspect residual structure, fit ranges, solver status, parameter bounds and
correlations as well as the displayed statistics. Test whether reasonable
processing or model choices change the conclusions. Include the scattering
backend, paths, parameter constraints and uncertainty convention when reporting
results. A small numerical residual does not account for an incorrect phase,
missing paths, sample heterogeneity or detector effects.
