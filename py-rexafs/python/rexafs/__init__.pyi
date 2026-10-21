"""Typed Rust spectrum processing. Energy: eV; k/q: inverse angstroms; R: angstroms."""

from typing import Literal, TypeAlias, TypedDict, overload
from collections.abc import Sequence

import numpy as np
from numpy.typing import ArrayLike, NDArray

from . import io as io

__version__: str
"""Version of the installed Python package, for example "0.2.4". Include this value when reporting results or requesting help; a source build can contain changes beyond the published package with the same version."""

AbsorptionMode: TypeAlias = Literal["unknown", "transmission", "fluorescence"]
"""Acquisition interpretation (since 0.2.10). Unknown means missing evidence;
it does not establish fluorescence. Changing it never removes correction history."""

FFTGrid: TypeAlias = Literal["Input", "Larch"]
"""Sampling conventions for the forward transform: "Input" or "Larch".

"Input" preserves the background k grid. "Larch" linearly resamples onto a
zero-origin grid and extends the window domain to cover its upper taper.
Neither changes Spectrum.k() or Spectrum.chi(); use kwin_k() with kwin().
See [FFT grid compatibility](https://rexafs.com/docs/science/fourier-compatibility/)."""
FTWindow: TypeAlias = Literal[
    "Hanning", "Parzen", "Welch", "Gaussian", "Sine", "KaiserBessel", "FHanning"
]
"""Names of the supported real Fourier windows.

Hanning and FHanning use cosine tapers; Parzen is linear, Welch is parabolic,
Gaussian is bell-shaped, Sine uses a sine profile, and KaiserBessel uses a
modified Bessel function. Windows reduce truncation ringing but broaden peaks.
Taper parameters are shape-dependent: KaiserBessel also uses dk/dr to control
its shape, so equal parameter values do not make the windows equivalent.
Use the default for each processing stage as a starting point and inspect
the resulting window. See [Larch's window reference](https://xraypy.github.io/xraylarch/xafs_fourier.html#ftwindow).

Names are case-sensitive. FHanning interprets its taper parameters as
fractions of the selected interval rather than ordinary absolute widths.
Gaussian uses dk/dr as its standard-deviation scale and has nonzero tails
outside the nominal interval; dk2/dr2 affects the domain geometry rather
than defining a separate Gaussian width. Inspect the generated window
when choosing any family; there is no area normalization."""
AUTOBKSolver: TypeAlias = Literal["TrustRegionDogLeg", "LegacyLm", "LinearDirect"]
"""Names of the AUTOBK spline solvers.

"LinearDirect" solves a linear least-squares problem and is required by the
recommended "FixedPenalty" objective. "TrustRegionDogLeg" and "LegacyLm"
are iterative alternatives for the legacy "Fixed" or "TwoPass" objectives.
TrustRegionDogLeg requires the trust-region Rust feature, included in Python
packages. Changing the solver does not select a compatible objective for you."""
AUTOBKClampScalePolicy: TypeAlias = Literal["FixedPenalty", "Fixed", "TwoPass"]
"""Names of the AUTOBK endpoint-penalty models.

"FixedPenalty" is recommended: a fixed mean-square penalty discourages large
endpoint oscillations and uses LinearDirect. "Fixed" and "TwoPass" retain
legacy residual-dependent clamp scaling; TwoPass updates that scale after
an initial direct solve. The models solve different objectives and need not
produce identical backgrounds. See the [AUTOBK objective](https://rexafs.com/docs/science/autobk/)."""

class PrePostEdge:
    """Configure the baselines and edge step used to normalize absorption.

    A line is fitted before the absorption edge and a polynomial after it.
    The normalized signal is (mu - pre_edge) / edge_step; mu and both baselines
    have the same absorption units, so the result is dimensionless. The edge
    step is the fitted baseline difference near E0 unless you override it.
    Fit limits are energy offsets from E0 in eV, not absolute energies.

    All fields default to None and are resolved from the spectrum when processing.
    These automatic choices match Rust PrePostEdge::new(); Rust's Default trait
    uses fixed ranges instead. Start with automatic settings and inspect the
    baselines before choosing narrower ranges or a higher polynomial degree.
    Settings are copied into a method and spectrum; edit and assign them again
    to apply a later change.

    Example: p = PrePostEdge(); p.pre_edge_end = -30.0. Assign p to the
    spectrum's normalization stage, then call normalize() or a later stage.

    For the measurement and normalization convention, see
    [Newville, Fundamentals of XAFS, sections 4 and 5](https://docs.xrayabsorption.org/tutorials/XAFS_Fundamentals.pdf).
    Automatic range selection and the numerical safeguards are rexafs choices;
    see [processing theory](https://rexafs.com/docs/science/processing/).

    Resolved fit ranges, polynomial degree and Victoreen exponent are retained
    inside the spectrum. An automatically estimated edge step is recalculated
    after normalization results are invalidated.
    Processing does not replace None fields in your original settings object.
    Reassign fresh or reset settings when you want retained automatic choices
    recalculated after changing the data or an earlier stage."""
    def __init__(
        self,
        *,
        pre_edge_start: float | None = None,
        pre_edge_end: float | None = None,
        norm_start: float | None = None,
        norm_end: float | None = None,
        norm_polyorder: int | None = None,
        n_victoreen: int | None = None,
        e0: float | None = None,
        edge_step: float | None = None,
    ) -> None:
        """Create automatic pre/post-edge settings, with every field initially None.

        Set only the fields your data require, then assign the settings to the
        spectrum's normalization stage. Fit ranges and degrees are
        resolved when normalization runs; creating settings does not process data.

        Keyword arguments were added in 0.2.5. Published
        0.2.4 settings use construction without arguments followed by field
        assignment. Python type conversion can raise TypeError, and an integer
        outside the native field's representable range can raise OverflowError
        before any numerical processing.

        Normalization fit choices are validated when a stage runs. To see the
        effect of a change, inspect pre_edge(), post_edge(), norm() and flat()
        after assigning the settings and calling normalize()."""
    pre_edge_start: float | None
    """Start of the pre-edge fit, as an offset from E0 in eV.

    Default None estimates a lower bound near the second energy sample, rounded
    on a 2 or 5 eV grid and clipped to the measured range. Usually this offset
    is negative. Choose a range below the edge rise. Since 0.2.15 an explicit
    start before the first measured energy is raised to it when normalize() runs
    (the stored value is kept), and a start at or above pre_edge_end is an error
    naming pre_edge_start instead of being swapped."""
    pre_edge_end: float | None
    """End of the pre-edge fit, as an offset from E0 in eV.

    Default None uses 5 * round(pre_edge_start / 15), approximately one third
    of the lower offset. A more negative value excludes more of the edge rise
    but leaves fewer baseline samples. Keep at least two usable pre-edge points."""
    norm_start: float | None
    """Start of the post-edge polynomial fit, as an offset from E0 in eV.

    Default None uses 5 * round(norm_end / 15), capped at 25 eV, then ensures
    at least a 10 eV separation from norm_end. Select a region beyond the edge
    rise; very short scans can make this automatic choice unsuitable. Since
    0.2.15, when a start lies beyond the (data-limited) norm_end, normalize() uses
    min(25, 5 * round(norm_end / 15)) eV instead of failing; the setting is kept."""
    norm_end: float | None
    """End of the post-edge polynomial fit, as an offset from E0 in eV.

    Default None rounds the available upper energy offset to a 5 eV grid and
    clips it to the data limit. Reducing this bound excludes high-energy data
    from the normalization fit; it does not trim the spectrum itself. Since
    0.2.15 an explicit value beyond the last measured energy is lowered to it
    when normalize() runs; the stored setting keeps the requested value."""
    norm_polyorder: int | None
    """Degree of the polynomial fitted to the pre-edge-subtracted post-edge data.

    Default None chooses degree 0 for a fit span below 50 eV, 1 below 350 eV,
    and 2 otherwise. Explicit degrees are clamped to 0 through 5. Higher degrees
    can follow baseline curvature but also fit noise or oscillations; begin
    with the automatic choice. The fit needs more samples than its degree."""
    n_victoreen: int | None
    """Energy exponent used to fit the pre-edge baseline. Default None resolves to 0.

    For exponent n, the code fits mu(E) * E**n with a line, then divides that
    line by E**n to recover the baseline in mu units; E is in eV. With n=0
    this is an ordinary straight-line baseline. A nonzero exponent changes
    its curvature; it is a baseline model, not a correction for self-absorption."""
    e0: float | None
    """Absorption edge energy E0 in eV. Default: None for automatic detection.

    E0 defines the origin of the fit-range offsets and the conversion from
    energy to photoelectron k. Assigning these settings can replace a spectrum's
    previous E0. Spectrum processing rejects an explicit E0 that is non-finite
    or not strictly inside the measured energy range with ValueError; it does
    not silently replace such an invalid override with an automatic value. The
    lower-level Rust baseline fitter has its own redetection safeguards near the
    end of the scan, so inspect the resolved Spectrum.e0(). An automatic
    derivative estimate is not an energy calibration."""
    edge_step: float | None
    """Override the absorption edge step, in the same units as mu.

    Default None estimates post_edge - pre_edge at the sample nearest E0.
    Normalization divides by this value, so changing it rescales norm and chi.
    Use a finite positive value if a separately determined step is available.
    The implementation floors finite steps below 1e-12 to 1e-12; that safeguard
    does not make a zero or negative experimental edge step meaningful."""

class AUTOBK:
    """Fit a smooth atomic background and extract the EXAFS oscillation chi(k).

    AUTOBK fits a cubic spline in k to suppress low-R Fourier content of the
    background-subtracted signal. Subtracting that background and dividing
    by the normalization edge step gives dimensionless chi. rbkg controls the
    low-R region treated as background; a value that reaches the first
    structural shell can remove the signal you want to measure.

    Recommended starting values are rbkg=1.0 angstrom, kstep=0.05 inverse
    angstroms, kweight=1, window="Hanning", solver="LinearDirect", and
    clamp_scale_policy="FixedPenalty" with clamp_lambda=0.001. The fixed
    penalty discourages large endpoint oscillations; its strength is a
    rexafs convention, not a universal statistical regularization parameter.
    None resolves optional fields when processing, as described per field.

    Example: p = AUTOBK(); p.rbkg = 1.2. Assign p to the spectrum's background
    stage, then call calc_background() or a later stage.
    Settings are copied; assigning them again is required after later edits.
    Invalid ranges, an incompatible solver, or a failed solve raise RuntimeError
    during processing. A solved background still needs scientific inspection.

    Original AUTOBK method: [Newville et al. (1993)](https://doi.org/10.1103/PhysRevB.47.14126).
    The fixed endpoint penalty and linear solution are rexafs-specific choices;
    see the [implemented AUTOBK objective](https://rexafs.com/docs/science/autobk/).

    Resolved scalar defaults and ek0 are retained inside the spectrum.
    Automatic kmax and nknots remain unset in the settings and are calculated
    locally from the current input on each background call.
    Processing does not replace None fields in your original settings object.
    Reassign fresh or reset settings when you want retained automatic choices
    recalculated after changing the data or an earlier stage."""
    def __init__(
        self,
        *,
        ek0: float | None = None,
        rbkg: float | None = 1.0,
        nknots: int | None = None,
        kmin: float | None = 0.0,
        kmax: float | None = None,
        kstep: float | None = 0.05,
        nclamp: int | None = 3,
        clamp_lo: int | None = 0,
        clamp_hi: int | None = 1,
        clamp_lambda: float | None = 0.001,
        nfft: int | None = 2048,
        kweight: int | None = 1,
        dk: float | None = 0.1,
        linear_regularization: float | None = 0.0001,
        linear_condition_limit: float | None = 100000000.0,
        linear_residual_ratio_limit: float | None = 1.05,
        linear_fallback_to_lm: bool | None = True,
        linear_workspace_cache: bool | None = True,
        window: FTWindow | None = "Hanning",
        solver: AUTOBKSolver | None = "LinearDirect",
        linear_fallback_solver: AUTOBKSolver | None = "TrustRegionDogLeg",
        clamp_scale_policy: AUTOBKClampScalePolicy | None = "FixedPenalty",
    ) -> None:
        """Create AUTOBK settings with the recommended Rust defaults.

        Begin with rbkg=1.0 angstrom and the LinearDirect/FixedPenalty pair.
        Edit only the fields your data require, then assign the settings to the
        spectrum's background stage. Construction does not fit a spectrum;
        numeric range and solver compatibility checks occur during processing.

        Keyword arguments were added in 0.2.5. Published
        0.2.4 settings use construction without arguments followed by field
        assignment. Python type conversion can raise TypeError, and an integer
        outside the native field's representable range can raise OverflowError
        before any numerical processing."""
    ek0: float | None
    """Edge energy used to convert energy to k, in eV.

    Default None uses the normalization E0. Above the edge, k is approximately
    sqrt((E - E0) / 3.81), with E in eV and k in inverse
    angstroms; the conversion constant has units eV * angstrom**2.
    Prefer Spectrum.set_e0() when changing the edge for all stages
    together; an independent background edge can differ from normalization."""
    rbkg: float | None
    """Positive background cutoff in angstroms. Default: 1.0; None resolves to 1.0.

    AUTOBK suppresses low-R Fourier residuals up to a cutoff derived from this
    value, the k range, and the discrete R grid; it is not an exact continuous
    boundary. Increasing rbkg allows a more flexible spline and can remove
    real short-distance structure. Start near 1.0 and keep it below the first
    physical shell of interest."""
    nknots: int | None
    """Requested number of spline coefficients/anchor points.

    Default None uses 1 + floor(2 * rbkg * (kmax - kmin) / pi), clamped to
    5 through 128; explicit values are clamped to the same range. rbkg is in
    angstroms and k limits in inverse angstroms, so the count is dimensionless.
    More coefficients increase background flexibility. Leave this automatic
    unless testing a justified spline model."""
    kmin: float | None
    """Lower k bound of the background fit and Fourier window, in inverse
    angstroms.

    Default: 0.0. None also resolves to 0.0. Raising it excludes low-k data from
    the Fourier objective and changes the spline geometry. It must be below the
    effective kmax. The returned k() array still starts at zero."""
    kmax: float | None
    """Upper k bound of the background fit, in inverse angstroms.

    Default None uses the available data limit; an explicit larger value is
    clipped to that limit. Lower it to exclude a noisy high-k tail. This also
    changes the spline count and the extent of the returned k()/chi() arrays;
    it is independent of the later XrayFFTF.kmax setting."""
    kstep: float | None
    """Spacing of the uniform output k grid, in inverse angstroms.

    Default: 0.05. None also resolves to 0.05. Values must be finite and
    positive. The grid begins at zero and is used by Spectrum.k() and chi().
    Smaller steps produce more interpolated samples without adding measured
    information. This usually supplies the automatic spacing of the later
    forward transform.

    For FixedPenalty, the internal objective FFT uses a fixed amplitude
    reference of 0.05 / sqrt(pi); kstep still sets the physical R spacing
    and low-R cutoff. Legacy clamp policies use kstep / sqrt(pi) instead.
    This internal convention is separate from the public forward transform.
    If that transform has already run, assign fresh XrayFFTF settings when
    changing this step so its automatic spacing is resolved again."""
    nclamp: int | None
    """Number of endpoint samples included at each enabled end of the clamp.

    Default: 3. None also resolves to 3; 0 disables endpoint penalties.
    FixedPenalty uses the first and last nclamp samples, limited to the
    available length, and requires a nonnegative value. Increasing the count
    constrains a wider endpoint region. clamp_lo/clamp_hi select which ends
    contribute."""
    clamp_lo: int | None
    """Relative weight of the low-k endpoint penalty. Default: 0. None also
    resolves to 0.

    Zero disables this end. For FixedPenalty, the absolute integer weight
    multiplies the endpoint residual before squaring; doubling it gives four
    times the squared contribution for a fixed residual. Start at zero so the
    near-edge oscillation is not forced toward zero."""
    clamp_hi: int | None
    """Relative weight of the high-k endpoint penalty. Default: 1. None also
    resolves to 1.

    Zero disables this end. For FixedPenalty, the absolute integer weight
    multiplies endpoint residuals before squaring. Larger values discourage
    large high-k endpoint oscillations more strongly but may bias a real
    oscillation; clamp_lambda controls the overall penalty strength."""
    clamp_lambda: float | None
    """Strength of the FixedPenalty mean-square endpoint term. Default: 0.001.

    None resolves to 0.001; 0 disables both endpoint penalties regardless of
    their weights. Values must be finite and nonnegative. Increasing this
    value favors smaller endpoint oscillations over the low-R objective.
    Its numerical meaning depends on the implemented Fourier scaling and
    weights; it is not an uncertainty estimate. Unused by Fixed and TwoPass.

    FixedPenalty evaluates its low-R residual with an internal FFT factor
    of 0.05 / sqrt(pi). Changing the background k weight or window changes
    the numerical balance against the endpoint penalty, even with unchanged
    lambda. See the [implemented objective](https://rexafs.com/docs/science/autobk/)."""
    nfft: int | None
    """Number of samples in the FFT used inside background removal.

    Default: 2048. None also resolves to 2048. Use a positive length large
    enough to contain the output k grid: the underlying FFT otherwise keeps only
    its first nfft samples. A larger length makes the R grid finer through zero
    padding; it does not improve experimental resolution. This setting is
    independent of the later XrayFFTF.nfft."""
    kweight: int | None
    """Integer exponent of k in the background Fourier objective. Default: 1.

    None resolves to 1. Larger nonnegative weights emphasize higher-k
    oscillations and their noise while the spline is fitted. Use nonnegative
    values on the zero-origin grid; negative powers are singular at k=0.
    This weight does not remain in the returned chi() and is independent
    of the exponent used by the later forward transform."""
    dk: float | None
    """Window taper parameter for the background objective. Default: 0.1.

    None resolves to 0.1. For the default Hanning window this sets the taper
    width at each k bound, in inverse angstroms. A broader taper softens
    truncation but reduces the strongly weighted range. Other window families
    interpret the parameter differently; KaiserBessel also uses it as a
    shape parameter. Inspect the window when changing families."""
    linear_regularization: float | None
    """Ridge strength for legacy Fixed/TwoPass direct solves. Default: 0.0001.

    None resolves to 0.0001. It adds a scaled diagonal regularization to the
    legacy normal equations, trading closeness to the unregularized solution
    for stability. FixedPenalty does not use this parameter; changing it
    does not regularize the recommended column-scaled SVD solve."""
    linear_condition_limit: float | None
    """Largest accepted condition measure for a direct solve. Default: 1e8.

    None resolves to 1e8. FixedPenalty requires a finite value at least 1
    and checks the largest/smallest singular-value ratio of its column-scaled
    design matrix. Legacy solves use a different condition proxy. Lowering
    the limit rejects more unstable systems; raising it can accept sensitive
    solutions. FixedPenalty failures raise RuntimeError without a fallback."""
    linear_residual_ratio_limit: float | None
    """Maximum solved/base residual-norm ratio for legacy direct solves.

    Default: 1.05. None also resolves to 1.05; legacy processing clamps it to at
    least 1. Exceeding the limit rejects the direct solution and may trigger the
    configured fallback. FixedPenalty does not use this legacy acceptance test;
    its rank, condition and stationarity checks are separate."""
    linear_fallback_to_lm: bool | None
    """Allow a rejected legacy direct solve to use linear_fallback_solver.

    Default: True. None also resolves to True. Despite its historical name, the
    selected fallback need not be Levenberg-Marquardt. False makes a rejected
    legacy direct solve raise an error. FixedPenalty never falls back: it
    returns a failed solve as RuntimeError regardless of this setting."""
    linear_workspace_cache: bool | None
    """Reuse compatible background-solver geometry and factorization. Default: True.

    None resolves to True. FixedPenalty caches spline/FFT design matrices,
    column scaling and singular-value decomposition factors for matching
    geometry; each spectrum still supplies its own data and receives a new
    solution. False disables this reuse, mainly for comparisons or profiling;
    the cache does not reuse a previous spectrum's chi()."""
    window: FTWindow | None
    """Fourier window used inside the background objective. Default: "Hanning".

    None selects Hanning. A window reduces artifacts from abrupt k truncation;
    its shape changes the objective and can change the extracted background.
    Accepted case-sensitive names are Hanning, Parzen, Welch, Gaussian,
    Sine, KaiserBessel and FHanning. See the
    [window reference](https://xraypy.github.io/xraylarch/xafs_fourier.html#ftwindow)
    for the shape-dependent parameter conventions.
    An unsupported name raises ValueError when assigned."""
    solver: AUTOBKSolver | None
    """Solver used for the spline coefficients. Recommended default: "LinearDirect".

    None resolves to LinearDirect, which is required by FixedPenalty.
    TrustRegionDogLeg and LegacyLm are iterative solvers for legacy objectives;
    select Fixed or TwoPass explicitly to use them. TrustRegionDogLeg requires
    the trust-region Rust feature, included in Python packages. Unknown names
    raise ValueError; incompatible solver/objective pairs fail during processing."""
    linear_fallback_solver: AUTOBKSolver | None
    """Solver used after a rejected legacy LinearDirect solve.

    The Python constructor default is "TrustRegionDogLeg". Explicit None
    normally resolves to "LegacyLm" when fallback is enabled; it is therefore
    different from leaving the constructor argument unchanged. LinearDirect
    cannot be its own fallback. FixedPenalty ignores this setting and never
    falls back. Unknown names raise ValueError when assigned."""
    clamp_scale_policy: AUTOBKClampScalePolicy | None
    """Endpoint model for background removal. Recommended default: "FixedPenalty".

    None resolves to FixedPenalty, which requires LinearDirect and uses
    clamp_lambda as a fixed mean-square penalty strength. Fixed and TwoPass
    retain older residual-dependent clamp models; their results need not
    match the recommended objective. See the [AUTOBK objective](https://rexafs.com/docs/science/autobk/) for
    the distinction. Unknown names raise ValueError when assigned."""

class XrayFFTF:
    """Configure the weighted Fourier transform from photoelectron k to distance R.

    The code forms g = chi * k**w * W, with integer w=kweight and real window
    W, then computes chi_R = (kstep / sqrt(pi)) * rfft(g, n=nfft) using an
    unnormalized, negative-exponent FFT. There is no additional 1/nfft factor,
    window-area normalization or phase rotation. On a zero-origin uniform
    grid, R_m = pi * m / (nfft * kstep), where m is the frequency-bin index.
    For dimensionless chi, chi_R has units angstrom**(-(w + 1)); R is in
    angstroms. Zero padding refines the R sampling but adds no measured data.

    Recommended defaults are kmin=2, kmax=15 inverse angstroms, kweight=2,
    window="KaiserBessel", dk=1, nfft=2048 and grid="Input". Automatic kstep
    uses the background grid, usually 0.05 inverse angstroms. Settings are
    copied into a spectrum, so reassign them after editing.

    Example: p = XrayFFTF(); p.kmax = 12.0; spectrum.set_fft(p).fft().
    Use spectrum.kwin_k() with kwin() to inspect the window. Scattering phase
    shifts mean that an uncorrected Fourier peak is not directly a bond length.

    Implementation convention: [NumPy's DFT definition](https://numpy.org/doc/stable/reference/routines.fft.html#implementation-details).
    Physical interpretation: [Rehr and Albers (2000)](https://doi.org/10.1103/RevModPhys.72.621).
    See [processing theory](https://rexafs.com/docs/science/processing/) for the
    equation and links to the implementing Rust functions.

    Resolved automatic k limits, k spacing and numeric defaults are retained
    inside the spectrum. An unset window still selects Hanning when used.
    Processing does not replace None fields in your original settings object.
    Reassign fresh or reset settings when you want retained automatic choices
    recalculated after changing the data or an earlier stage."""
    def __init__(
        self,
        *,
        grid: FFTGrid = "Input",
        rmax_out: float | None = 10.0,
        dk: float | None = 1.0,
        dk2: float | None = None,
        kmin: float | None = 2.0,
        kmax: float | None = 15.0,
        kweight: float | None = 2.0,
        nfft: int | None = 2048,
        kstep: float | None = None,
        window: FTWindow | None = "KaiserBessel",
    ) -> None:
        """Create forward-transform settings with the recommended Rust defaults.

        Start with k=2 to 15 inverse angstroms, kweight=2, a KaiserBessel window
        and nfft=2048. Choose a useful k range for your measured data, then call
        spectrum.set_fft(parameters).fft(). Construction does not run a transform;
        numeric validation occurs when fft() processes the data.

        Keyword arguments were added in 0.2.5. Published
        0.2.4 settings use construction without arguments followed by field
        assignment. Python type conversion can raise TypeError, and an integer
        outside the native field's representable range can raise OverflowError
        before any numerical processing."""
    grid: FFTGrid
    """Sampling/window convention. Default: "Input".

    Input keeps the existing background samples and does not resample them
    when kstep changes. Larch linearly interpolates onto a zero-origin grid
    with the requested kstep and extends the window domain through the upper
    taper. Both preserve Spectrum.k()/chi(); kwin_k() returns the matching
    window axis. Unknown names raise ValueError when assigned."""
    rmax_out: float | None
    """Largest R shown by r() and chir_real/imag/mag(), in angstroms.

    Default: 10.0. None also resolves to 10.0. It must be finite and
    nonnegative. This limits returned display arrays; the complete internal
    forward transform is retained for inverse filtering. Increasing it does not
    improve spatial resolution or apply a structural shell filter.

    Keep at least two returned R samples if you plan to call ifft(): its
    grid validation uses r() even though filtering uses the full stored
    Fourier coefficients. For example, rmax_out=0 permits a forward result
    but makes a subsequent inverse fail with RuntimeError."""
    dk: float | None
    """Low-k window taper parameter. Default: 1.0. None also resolves to 1.0.

    For Hanning-like windows it describes a width in inverse angstroms.
    KaiserBessel also uses this numeric value to set the Bessel-function shape,
    so it is not a universally comparable taper width. Larger tapers generally
    soften truncation at the cost of a broader R response. Use kwin_k()/kwin()
    to inspect the actual window.

    FHanning uses a fractional taper parameter. For Gaussian, dk is the
    standard-deviation scale in inverse angstroms and the window has tails
    beyond the nominal bounds. It is not a low-end-only width for those
    families. See the [window reference](https://xraypy.github.io/xraylarch/xafs_fourier.html#ftwindow)
    for their distinct conventions."""
    dk2: float | None
    """High-k window taper parameter. Default None uses dk.

    It controls the upper-end geometry in inverse angstroms for the
    width-based windows. Set it separately for an asymmetric taper.
    Window families interpret taper parameters differently; KaiserBessel's
    Bessel-function shape is controlled by dk, not an independent dk2 shape.

    For Gaussian, dk2 affects the window domain and center but is not a
    second standard deviation. FHanning interprets it as a fractional taper
    parameter. See the [window reference](https://xraypy.github.io/xraylarch/xafs_fourier.html#ftwindow)
    before comparing settings between families."""
    kmin: float | None
    """Lower Fourier-window limit, in inverse angstroms. Constructor default: 2.0.

    Explicit None uses the first background k sample, usually zero. Raising
    the limit suppresses low-k contributions but shortens the effective
    transform range. The implementation requires finite bounds with kmin
    below kmax; negative lower bounds are accepted, but the window is clipped
    to its sampled domain. Use a nonnegative bound for the physical k range.
    The taper can extend below the nominal limit."""
    kmax: float | None
    """Upper Fourier-window limit, in inverse angstroms. Constructor default: 15.0.

    Explicit None uses the last background k sample. Lower it to exclude
    a noisy high-k tail; this changes the transform without recomputing chi.
    The taper can extend above the nominal bound. Input and Larch use
    different window domains when the taper reaches beyond measured data."""
    kweight: float | None
    """Exponent applied as k**w before the transform. Default: 2.0. None also
    resolves to 2.0.

    Finite nonnegative values are floored to an integer w. A higher weight
    emphasizes high-k oscillations and noise. It changes both amplitude and
    units: dimensionless chi gives chi(R) in angstrom**(-(w + 1)). The
    background chi() remains unweighted."""
    nfft: int | None
    """Total FFT length, including zero padding. Default: 2048. None also resolves
    to 2048.

    It must be at least 2. Choose a length that contains the prepared data:
    Input keeps only the first nfft samples if the data are longer; Larch
    requires its extended window grid to fit. Increasing nfft gives a finer R
    grid with spacing pi / (nfft * kstep), but does not add structural
    information or introduce an extra amplitude normalization."""
    kstep: float | None
    """k spacing used for FFT scaling and the R axis, in inverse angstroms.

    Default None uses the difference between the first two input k values;
    the default AUTOBK grid gives 0.05. An explicit value must be finite and
    positive. Input does not resample, so keep this equal to its actual grid
    spacing. Use grid="Larch" when requesting resampling at a different step.
    The forward amplitude multiplier is kstep / sqrt(pi).

    Once resolved, the spectrum retains this spacing on later fft() calls.
    Changing the background k grid does not automatically reset it. Reassign
    an XrayFFTF with kstep=None to infer the new spacing; the original
    settings object remains unchanged by processing."""
    window: FTWindow | None
    """Forward Fourier-window shape. Constructor default: "KaiserBessel".

    Explicit None selects Hanning, which differs from leaving the default
    unchanged. The window reduces truncation ringing and broadens the R
    response; it is not normalized by its area. Accepted case-sensitive names are Hanning, Parzen, Welch, Gaussian,
    Sine, KaiserBessel and FHanning. See the
    [window reference](https://xraypy.github.io/xraylarch/xafs_fourier.html#ftwindow)
    for the shape-dependent parameter conventions. Unknown names raise ValueError when assigned."""

class XrayFFTR:
    """Configure a real inverse Fourier transform with an R-space window.

    The code weights positive-R bins by R**rweight and a real window, then
    reconstructs their conjugate negative-frequency partners to obtain a real
    signal on q. q has the same physical meaning as k, in inverse angstroms;
    the different name identifies a back-transformed, potentially filtered
    signal. The forward k weighting and window remain in that signal.
    This real inverse differs from Larch's complex, one-sided back-transform.

    Defaults are rmin=0, rmax=20 angstroms, rweight=0, a KaiserBessel window,
    dr=1 angstrom, qmax_out=10 inverse angstroms, nfft=2048, and automatic
    kstep. Choose an R interval appropriate to your shells for deliberate
    filtering. The uncorrected R axis includes scattering phase shifts.

    Example: p = XrayFFTR(); p.rmin = 1.0; p.rmax = 3.0;
    spectrum.set_ifft(p).ifft(). Settings are copied; reassign after edits.
    These Python settings and set_ifft() were added in 0.2.5.

    See the [implemented inverse convention](https://rexafs.com/docs/science/processing/)
    for the scaling, and [Larch's Fourier guide](https://xraypy.github.io/xraylarch/xafs_fourier.html)
    for windowing concepts rather than an assertion of identical inverse output.

    Resolved automatic R limits, q spacing and numeric defaults are retained
    inside the spectrum. Unset dr2 and window continue to select dr and
    Hanning when the window is calculated.
    Processing does not replace None fields in your original settings object.
    Reassign fresh or reset settings when you want retained automatic choices
    recalculated after changing the data or an earlier stage."""
    def __init__(
        self,
        *,
        qmax_out: float | None = 10.0,
        dr: float | None = 1.0,
        dr2: float | None = None,
        rmin: float | None = 0.0,
        rmax: float | None = 20.0,
        rweight: float | None = 0.0,
        nfft: int | None = 2048,
        kstep: float | None = None,
        window: FTWindow | None = "KaiserBessel",
    ) -> None:
        """Create inverse-transform settings with the recommended Rust defaults.

        Set rmin/rmax to select an R region and use rweight=0 unless extra R
        weighting is intended. Assign with spectrum.set_ifft(parameters).ifft().
        The returned signal retains forward weighting and windowing; construction
        alone does not filter a spectrum.

        This settings class was added in 0.2.5; it is
        not exported by the published 0.2.4 package. Python type conversion can raise TypeError, and an integer
        outside the native field's representable range can raise OverflowError
        before any numerical processing."""
    qmax_out: float | None
    """Largest returned q value, in inverse angstroms. Default: 10.0. None also
    resolves to 10.0.

    The q()/chiq() arrays stop at this bound or the available inverse samples,
    whichever comes first. It must be finite and nonnegative. This limits
    returned samples and does not change the R-space filter."""
    dr: float | None
    """Low-R window taper parameter. Default: 1.0. None also resolves to 1.0.

    For width-based windows the unit is angstroms. KaiserBessel also uses this
    numeric value as its Bessel-function shape parameter. A wider taper smooths
    the selected R boundary but mixes a broader range of distances into the
    filtered signal.

    FHanning instead uses a fractional taper parameter. Gaussian uses dr
    as its standard-deviation scale in angstroms and has nonzero tails
    beyond the nominal R interval."""
    dr2: float | None
    """High-R window taper parameter. Default None uses dr.

    Use a separate value for asymmetric R-window geometry. The unit is
    angstroms for width-based windows; KaiserBessel's Bessel-function shape
    uses dr, so dr2 does not define an independent high-end shape.

    Gaussian uses dr for its standard deviation; dr2 affects the domain
    and center rather than providing a second Gaussian width. FHanning
    uses a fractional taper parameter."""
    rmin: float | None
    """Lower R-window limit, in angstroms. Constructor default: 0.0.

    Explicit None uses the first input R sample, which is zero. A larger
    value removes lower-R contributions from the back-transform. The bound
    must be finite, nonnegative and below rmax; the taper can extend below it.
    R peaks have not been corrected for scattering phase shifts."""
    rmax: float | None
    """Upper R-window limit, in angstroms. Constructor default: 20.0.

    Choose a shell range such as 1 to 3 angstroms only when appropriate for
    your data. Explicit None uses the last reported input R sample, which
    depends on forward rmax_out; an explicit bound instead acts on the full
    internal Fourier bins. The taper can extend above the nominal bound."""
    rweight: float | None
    """Exponent applied as R**v before the inverse transform. Default: 0.0.

    None resolves to 0.0; finite nonnegative values are floored to an integer
    v. Leave it at zero for ordinary R filtering. A positive value emphasizes
    higher-R contributions and changes the units of chiq(): with forward
    kweight w the units are angstrom**(v - w)."""
    nfft: int | None
    """Inverse transform length. Default: 2048. None also resolves to 2048;
    minimum: 2.

    Leave kstep automatic when changing this value. With fixed input R spacing,
    a larger nfft gives a finer q grid by zero padding Fourier bins; a smaller
    value discards bins beyond its representable range. Neither operation adds
    experimental information."""
    kstep: float | None
    """Output q spacing, in inverse angstroms.

    Default None uses pi / (nfft * delta_R), where delta_R is the difference
    between the first two R samples in angstroms. An explicit positive value
    must agree with that spacing or processing raises RuntimeError. Keep it
    automatic when changing nfft so the physical Fourier grid stays consistent.

    Automatic spacing is retained inside the spectrum after the first
    inverse. If the forward R grid changes, assign fresh inverse settings
    with kstep=None before calling ifft() again. The earlier resolved value
    otherwise remains subject to the same consistency check."""
    window: FTWindow | None
    """R-space Fourier-window shape. Constructor default: "KaiserBessel".

    Explicit None selects Hanning, unlike leaving the default unchanged.
    The window selects and tapers R contributions before the real inverse;
    it cannot undo the weighting or information lost in the forward window.
    Accepted case-sensitive names are Hanning, Parzen, Welch, Gaussian,
    Sine, KaiserBessel and FHanning. See the
    [window reference](https://xraypy.github.io/xraylarch/xafs_fourier.html#ftwindow)
    for the shape-dependent parameter conventions. Unknown names raise ValueError when assigned."""

class NormalizationMethod:
    """Select the normalization algorithm and own a copy of its settings.

    Use NormalizationMethod.PrePostEdge(parameters) for configured pre/post-edge
    normalization or new_prepostedge() for automatic settings, then pass the
    result to Spectrum.set_normalization_method(). Creating a method does not
    process data. The no-argument MBack selector has no absorber/edge and cannot
    normalize. The MBACK algorithm was unimplemented through version 0.2.9."""
    @staticmethod
    def PrePostEdge(parameters: PrePostEdge) -> NormalizationMethod:
        """Copy PrePostEdge parameters into a normalization method.

        Assign the result with spectrum.set_normalization_method(method), then
        call normalize() or a later stage. The original parameters remain
        independent: edits do not change an already created method or spectrum."""
    @staticmethod
    def new_prepostedge() -> NormalizationMethod:
        """Create a normalization method with automatic pre/post-edge settings.

        E0, fitting ranges, polynomial degree and edge step are inferred when
        processing runs. Equivalent to NormalizationMethod.PrePostEdge(PrePostEdge());
        assign the method to a spectrum to use it. No data are processed here."""
    @staticmethod
    def new_mback() -> NormalizationMethod:
        """Create the historical empty MBack normalization selector.

        Selecting it preserves the requested algorithm, but normalize() and
        dependent stages raise ValueError rather than substitute another method.
        Use new_prepostedge() for automatic polynomial normalization. Through
        version 0.2.9 the MBACK algorithm was unimplemented; this historical
        no-argument selector remains unusable for normalization."""

class BackgroundMethod:
    """Select a background algorithm and own a copy of its settings.

    Use BackgroundMethod.AUTOBK(parameters) to configure the spline background
    or new_autobk() for the recommended defaults, then pass the result to
    Spectrum.set_background_method(). Creating a method does not process
    data. ILPBkg is a named placeholder and is not implemented."""
    @staticmethod
    def AUTOBK(parameters: AUTOBK) -> BackgroundMethod:
        """Copy AUTOBK parameters into a background method.

        Assign the result with spectrum.set_background_method(method), then call
        calc_background() or a later stage. Edits to the original parameters do
        not change an already created method or spectrum."""
    @staticmethod
    def new_autobk() -> BackgroundMethod:
        """Create a background method using the recommended AUTOBK defaults.

        This selects rbkg=1.0 angstrom, LinearDirect and FixedPenalty with
        clamp_lambda=0.001. Assign it to a spectrum before processing; no
        background is fitted by this factory."""
    @staticmethod
    def new_ilpbkg() -> BackgroundMethod:
        """Create the unimplemented ILPBkg background placeholder.

        Selecting it preserves the requested algorithm, but calc_background()
        and dependent stages raise RuntimeError rather than substitute AUTOBK.
        Use new_autobk() for the implemented background workflow."""

class MeasurementResult:
    """Owned result from Spectrum.measure (since 0.2.10). No processing state is mutated."""
    @property
    def value(self) -> float:
        """Finite scalar in unit."""
    @property
    def unit(self) -> str:
        """Signal unit for point/mean/maximum; signal times axis unit for integral."""
    @property
    def range(self) -> tuple[float, float]:
        """Resolved absolute bounds: eV, inverse angstroms, or angstroms."""
    @property
    def position(self) -> float | None:
        """Absolute point/maximum position, otherwise None."""
    @property
    def standard_error(self) -> float | None:
        """Independent propagated standard error when supplied; not a confidence interval."""
    @property
    def e0_ev(self) -> float | None:
        """Resolved absorption edge in eV, or None when unnecessary."""
    def to_json(self) -> str:
        """Serialize the result and full native measurement definition without changing inputs."""

class Spectrum:
    """Own a measured absorption spectrum and its calculated processing stages.

    Energy is in eV; absorption mu may be an absorption coefficient or a
    consistently scaled measurement such as transmission optical depth.
    Input arrays are copied to Rust-owned float64 storage. Returned NumPy
    arrays are independent copies, so editing them does not change this
    spectrum. Each result getter returns None until its stage has succeeded.

    Start with Spectrum(energy, mu).fft() to run automatic normalization,
    AUTOBK background removal and the k-to-R transform. Explicit processing
    calls recompute their stage and return this same object; missing
    prerequisites run automatically with the selected settings. Calculation
    methods release Python's global interpreter lock while Rust runs.

    Changing input data or settings clears dependent results. A failed stage
    does not substitute another algorithm. Invalid input and normalization
    errors raise ValueError; background and Fourier errors raise RuntimeError.
    See [processing theory](https://rexafs.com/docs/science/processing/) for
    equations, interpretation and limitations. Groups and structural fitting
    are not currently exposed by this Python Spectrum API."""
    def correct_fluorescence(self, model: FluorescenceCorrection) -> Spectrum:
        """Correct into an independent unnormalized Spectrum (since 0.2.10).
        Internal conventional normalization runs automatically; the source stays
        unchanged. Unknown acquisition provenance is explicitly interpreted as
        fluorescence. Known transmission, prepared norm/flat and repeated correction
        raise ValueError. Supply line and measured surface angles in the model.
        Releases the GIL. Call normalize() for separate final polynomial/MBACK
        normalization. History survives edits; this XANES-only branch rejects
        background/FFT/wavelets. Corrected-array uncertainty is unavailable."""
        ...
    def fluorescence_correction(self) -> FluorescenceCorrectionResult | None:
        """Owned historical correction record, or None. Later edits/normalization
        never rewrite its original inputs or remove the XANES-only restriction."""
        ...
    def absorption_mode(self) -> AbsorptionMode:
        """Acquisition interpretation: unknown, transmission or fluorescence."""
        ...
    def set_absorption_mode(self, mode: AbsorptionMode) -> Spectrum:
        """Explicitly revise acquisition interpretation and return this Spectrum.
        Arrays/caches stay unchanged. Correction history and restrictions survive."""
        ...
    def wavelet(self, model: Wavelet) -> WaveletMap:
        """Since 0.2.10: spectrum.wavelet(Wavelet((2, 12))) prepares missing
        normalization/AUTOBK on a copy, reusing existing chi. The interval uses
        inverse angstroms. Arrays/settings/caches are unchanged; Rust releases
        the GIL. Result matrices are owned (R rows, k columns). Invalid coverage,
        grids and unqualified corrected XANES input raise ValueError. R is not
        phase-corrected; color intensity is not a concentration."""
        ...
    def fit_peaks(self, model: PeakFit, *, errors: NDArray[np.float64] | Sequence[float] | None = None) -> PeakFitResult:
        """Fit a composite XANES model, preparing missing normalization on a copy (since 0.2.10).

        Example: spectrum.fit_peaks(PeakFit((-20, 40)).gaussian("p1", 5, 2, 3)).
        Defaults are Norm, E0-relative eV and 200 iterations. Source arrays, settings,
        caches and initial model remain unchanged; Rust releases the GIL. Results
        contain data/model/residual arrays on the retained native points. No smoothing
        or interpolation occurs. Invalid models, coverage or preparation raise ValueError.
        A result can be nonconverged: inspect termination, warnings and uncertainty_unavailable.

        Optional errors are positive independent standard deviations in the SELECTED
        signal representation on the original native grid, including excluded points.
        Raw detector errors are not propagated through normalization. Without errors,
        covariance uses residual-based variance. Active bounds, deficient rank and
        nonconvergence withhold conditional local uncertainty; this is not model confidence.
        """
    @overload
    def measure(self, operation: Literal["point"], coordinates: float, *,
                space: Literal["mu", "norm", "flat", "chi", "fourier"] = "norm",
                origin: Literal["e0", "absolute"] | None = None, kweight: int = 0,
                errors: NDArray[np.float64] | Sequence[float] | None = None) -> MeasurementResult:
        """Measure one interpolated point on a private copy (since 0.2.10).

        Defaults to normalized mu at an E0 offset in eV. Flat is explicit. k/R
        default to absolute inverse-angstrom/angstrom coordinates. Missing stages
        run on a copy; inputs/settings/caches remain unchanged, and missing coverage
        raises ValueError. Optional errors describe independent standard deviations
        on the selected signal grid, not raw counts propagated through processing.
        No confidence interval is inferred. See the region overload for details."""
    @overload
    def measure(self, operation: Literal["mean", "integral", "maximum"], coordinates: tuple[float, float], *,
                space: Literal["mu", "norm", "flat", "chi", "fourier"] = "norm",
                origin: Literal["e0", "absolute"] | None = None, kweight: int = 0,
                errors: NDArray[np.float64] | Sequence[float] | None = None) -> MeasurementResult:
        """Measure a region on a private copy (since 0.2.10).

        Recommended: spectrum.measure("mean", (-20, 30)). Defaults to normalized
        mu and E0-relative energy offsets in eV; select space="flat" explicitly.
        k is in inverse angstroms and R in angstroms, without phase correction;
        these require absolute coordinates (selected automatically when origin=None).
        kweight defaults to zero and applies only to chi. Bounds must increase.
        Mean is the piecewise-linear integral divided by interval width, not an
        arithmetic sample mean. Integral uses trapezoids and interpolated endpoints.
        Missing stages run automatically using this spectrum's settings, with the
        GIL released. Arrays, settings and cached results remain unchanged. No
        extrapolation or display sampling occurs. Invalid input, missing coverage,
        or unavailable preparation raises ValueError. Results own their values.

        errors supplies independent standard deviations of the SELECTED signal on
        its native grid, in its signal units, finite and nonnegative. Raw-count
        errors are not propagated through normalization or transforms. Point,
        integral and mean support this model; maximum rejects it. Axis, E0 and
        settings are exact. No correlations or confidence intervals are inferred;
        standard_error is None when errors are omitted.
        """
    def __init__(self, energy: ArrayLike, mu: ArrayLike) -> None:
        """Create a spectrum by copying energy and mu into float64 storage.

        energy contains X-ray energies in eV; mu contains the corresponding
        absorption values in consistent units. Lists, NumPy arrays and strided
        views are accepted. Inputs must be one-dimensional, finite, equal-length,
        have at least two samples, and have strictly increasing energy.
        Invalid shapes or data raise ValueError. No processing runs here;
        call fft() for the default pipeline or normalize() for just normalization.

        The two-sample minimum only permits storage. Automatic edge detection
        requires at least three samples, and baseline/spline fitting needs enough
        points on the appropriate sides of the edge. Supply real numeric input;
        NumPy conversion errors for unsupported objects propagate to the caller."""
    @staticmethod
    def from_arrays(energy: ArrayLike, mu: ArrayLike) -> Spectrum:
        """Create a spectrum from energy in eV and corresponding absorption mu.

        Equivalent to Spectrum(energy, mu): it accepts array-like inputs and
        copies them to owned float64 storage. Inputs must be finite, one-dimensional,
        equal-length, with at least two samples and strictly increasing energy.
        Invalid data raise ValueError. Derived results are initially unavailable.

        As with the constructor, two samples are enough to create the object
        but not enough for automatic edge detection or a useful EXAFS pipeline.
        Array conversion follows NumPy's float64 conversion rules."""
    def set_spectrum(self, energy: ArrayLike, mu: ArrayLike) -> Spectrum:
        """Replace measured energy/mu, copy the inputs and clear E0 and derived results.

        Input units and validation match the constructor: energy in eV, finite
        matching one-dimensional arrays with strictly increasing energy and at
        least two samples. Invalid input raises ValueError before replacing data.
        Stage settings are retained, but old calculated edge values and arrays
        are discarded. Returns this spectrum; call a processing stage to recompute.

        Unlike the QAS reader, this method rejects unordered input rather than
        sorting it. Previously resolved automatic fit ranges and FFT spacings
        are retained with the other settings. Reassign automatic settings if
        the new scan needs those choices inferred again."""
    def set_e0(self, e0: float) -> Spectrum:
        """Set the absorption edge energy in eV and clear all dependent results.

        The value is propagated to configured normalization and AUTOBK settings,
        so subsequent energy-to-k conversion uses the new edge. Use a finite
        value strictly inside the measured energy range; validation occurs when
        processing runs, not at this setter. Returns this spectrum without
        performing normalization or recalibrating the input energy axis."""
    def set_normalization_method(
        self, method: PrePostEdge | MBack | NormalizationMethod | None = None
    ) -> Spectrum:
        """Copy the selected normalization method and clear normalization and later results.

        Since 0.2.5, PrePostEdge settings are accepted directly; omitted/None restores
        automatic normalization. Published 0.2.4 uses
        NormalizationMethod.PrePostEdge(parameters) or new_prepostedge().
        An explicit E0 in those parameters becomes the spectrum E0; otherwise
        an existing spectrum E0 is retained. Edits to the original settings do
        not propagate: assign again to apply them. Returns this spectrum without
        processing data; call normalize() or a later stage to recompute."""
    def set_background_method(
        self, method: AUTOBK | BackgroundMethod | None = None
    ) -> Spectrum:
        """Copy the selected background method and clear background and Fourier results.

        Since 0.2.5, AUTOBK settings are accepted directly; omitted/None restores
        default AUTOBK. Published 0.2.4 uses BackgroundMethod.AUTOBK(parameters)
        or BackgroundMethod.new_autobk() for recommended defaults. Existing
        normalization results are retained. Later edits to the original settings
        do not propagate: assign them again to apply changes. Returns this
        spectrum without fitting; call calc_background() or a later stage.

        If changing kstep after fft() has already run, also reassign XrayFFTF
        settings with kstep=None. Clearing the Fourier results does not reset
        its previously resolved automatic spacing."""
    def set_ifft(self, parameters: XrayFFTR) -> Spectrum:
        """Copy inverse settings and clear q()/chiq(), preserving forward results.

        Use XrayFFTR to choose the R window and output q range. Editing the
        original settings later does not change this spectrum; assign again
        to apply changes. Returns this spectrum without filtering. Call ifft()
        to compute the new result. This binding was added in 0.2.5."""
    def set_fft(self, parameters: XrayFFTF) -> Spectrum:
        """Copy forward settings and clear Fourier and inverse results.

        Normalization and the background k()/chi() arrays are retained. Use
        XrayFFTF to choose k weights, the window and grid convention. Editing
        the original settings later has no effect until you assign them again.
        Returns this spectrum without transforming; call fft() to recompute.

        If ifft() has already resolved its automatic kstep and this change
        alters the R-grid spacing, reassign inverse settings with kstep=None
        before the next inverse. This setter clears results, not the resolved
        parameters of the inverse stage."""
    def e0(self) -> float | None:
        """Return the detected or assigned edge energy E0 in eV, or None if unset.

        This getter does not run edge detection. Call find_e0(), normalize() or
        a later processing stage to resolve an automatic edge. E0 sets the
        energy origin for k conversion; it is not an independent calibration
        measurement or a fitted structural energy shift."""
    def find_e0(self) -> Spectrum:
        """Estimate the absorption edge from the energy derivative of mu.

        The detector seeks a strong rise with neighboring high-derivative
        samples and refines its search around the candidate edge. It returns
        this spectrum, stores E0 in eV and clears normalization and all later
        results. Inspect the result for noisy spectra or multiple edges;
        automatic detection is not energy calibration. Invalid data raise ValueError.

        At least three energy/mu samples are required, even though the
        constructor can store two. An insufficient scan raises ValueError."""
    def normalize(self) -> Spectrum:
        """Fit pre/post-edge baselines and compute dimensionless normalized absorption.

        The implemented method computes norm = (mu - pre_edge) / edge_step
        and flat with its fitted post-edge trend removed. Missing E0 and
        automatic parameters are resolved from the data. Call norm(), flat(),
        pre_edge() and post_edge() to retrieve independent result arrays.
        This recomputes normalization, clears background/Fourier results and
        returns this spectrum. Invalid ranges, failed fits and an empty MBack selector raise ValueError."""
    def mback_result(self) -> MbackResult | None:
        """Copy the latest full MBACK result, or None when absent/invalidated.

        Arrays and diagnostics remain independent after further processing."""
        ...

    def calc_background(self) -> Spectrum:
        """Fit the selected smooth background and calculate dimensionless chi(k).

        Missing normalization runs first. The default AUTOBK method suppresses
        low-R Fourier content with rbkg=1.0 angstrom and the fixed endpoint
        penalty; k() is a zero-origin grid with default step 0.05 inverse angstroms.
        The returned chi() is unweighted. This recomputes the background, clears
        forward/inverse results and returns this spectrum. Background failures
        or ILPBkg raise RuntimeError; prerequisite normalization can raise ValueError."""
    def fft(self) -> Spectrum:
        """Compute the weighted k-to-R Fourier transform, running missing prerequisites.

        Defaults are k=2 to 15 inverse angstroms, kweight=2, a KaiserBessel
        window and nfft=2048. The amplitude multiplier is kstep / sqrt(pi)
        after an unnormalized negative-exponent FFT; there is no extra 1/nfft.
        For dimensionless chi the default output units are inverse cubic
        angstroms. XrayFFTF documents the full convention and sampling choices.

        Use r() with chir_real(), chir_imag() or chir_mag(), and kwin_k() with
        kwin(). This recomputes the forward transform, clears inverse results
        and returns this spectrum. Background and FFT failures raise RuntimeError;
        prerequisite normalization can raise ValueError."""
    def ifft(self) -> Spectrum:
        """Back-transform the stored Fourier signal to a real signal on q().

        Missing forward stages run first. The inverse applies an R window and
        reconstructs a real signal using conjugate negative-frequency bins.
        The forward k weighting and window remain: chiq() is generally not
        the original unweighted chi(k). With default forward kweight=2 and
        inverse rweight=0, chiq() has units inverse square angstroms.
        This recomputes the inverse and returns this spectrum. Invalid inverse
        grids or settings raise RuntimeError; prerequisite errors propagate.

        At least two entries must be present in the displayed r() array;
        an overly small forward rmax_out can therefore prevent inversion.
        After changing the forward R spacing, reassign inverse settings with
        automatic kstep to resolve the new grid instead of retaining an old
        resolved spacing. Configurable inverse settings were added in 0.2.5;
        use a fresh spectrum in 0.2.4 after changing the forward grid."""
    def invalidate_derived(self) -> Spectrum:
        """Clear normalization, background and Fourier results, keeping stage settings.

        The measured inputs and spectrum E0 are retained, as are explicit
        settings needed for recomputation. Getters for cleared arrays return
        None until their stages run again. Ordinary setters already invalidate
        the affected results; use this method when you need a full recomputation.
        Returns this spectrum without running any calculations.

        Resolved fit ranges and FFT spacings are retained along with explicit
        parameters. They are not restored to their original None values; reassign
        fresh settings if you want automatic ranges or spacings inferred again."""
    def k(self) -> NDArray[np.float64] | None:
        """Uniform background k axis in inverse angstroms, paired with chi().

        It begins at zero with AUTOBK.kstep spacing. It remains the background
        grid even when the forward transform uses grid="Larch".

        Returns an independent NumPy float64 array copy, or None before
        calc_background() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def chi(self) -> NDArray[np.float64] | None:
        """Unweighted, dimensionless EXAFS oscillation on k().

        chi is (mu - smooth background) / edge_step, resampled on the background
        k grid. Neither the AUTOBK objective weight nor the later forward FFT
        k weight is stored in this array.

        Returns an independent NumPy float64 array copy, or None before
        calc_background() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def norm(self) -> NDArray[np.float64] | None:
        """Dimensionless normalized absorption on the original energy grid.

        norm = (mu - pre_edge) / edge_step, with the baseline and edge step in
        the same units as mu. Use this for a normalized edge; flat() additionally
        removes the fitted post-edge trend.

        Returns an independent NumPy float64 array copy, or None before
        normalize() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def flat(self) -> NDArray[np.float64] | None:
        """Dimensionless normalized absorption with the fitted post-edge trend removed.

        Above the sample nearest E0, the fitted baseline difference divided
        by edge_step is subtracted and its value at E0 added back. Below that
        sample, flat equals norm. Values use the original energy grid; this
        flattening is separate from AUTOBK background subtraction.

        Returns an independent NumPy float64 array copy, or None before
        normalize() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def pre_edge(self) -> NDArray[np.float64] | None:
        """Fitted pre-edge baseline on the original energy grid, in mu units.

        The line fitted to mu * E**n_victoreen is divided by E**n_victoreen.
        It is extrapolated across the scan so normalization can subtract it.
        Inspect this baseline against the measured pre-edge region.

        Returns an independent NumPy float64 array copy, or None before
        normalize() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def post_edge(self) -> NDArray[np.float64] | None:
        """Fitted post-edge baseline on the original energy grid, in mu units.

        This is the fitted post-edge polynomial plus the pre-edge baseline.
        Its difference from pre_edge near E0 estimates the normalization edge
        step. It is not the AUTOBK smooth atomic background.

        Returns an independent NumPy float64 array copy, or None before
        normalize() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def r(self) -> NDArray[np.float64] | None:
        """Forward-transform R axis in angstroms, paired with chir_real/imag/mag().

        For FFT length N and k spacing delta_k, adjacent bins are separated
        by pi / (N * delta_k); rmax_out limits the returned range. Zero padding
        only refines sampling. Scattering phase shifts mean that an uncorrected
        R peak is not directly a bond length.

        Returns an independent NumPy float64 array copy, or None before
        fft() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def kwin(self) -> NDArray[np.float64] | None:
        """Dimensionless forward Fourier-window values, paired with kwin_k().

        These are the window W before multiplying by chi and k**kweight.
        Use the matching kwin_k() axis: with grid="Larch" the window can extend
        beyond, and have a different length from, the background k() grid.

        Returns an independent NumPy float64 array copy, or None before
        fft() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def kwin_k(self) -> NDArray[np.float64] | None:
        """k axis of the forward Fourier window, in inverse angstroms.

        Pair it with kwin(). For grid="Input" it matches the background grid;
        for grid="Larch" it uses the resampled, extended window domain. The
        background k()/chi() arrays themselves are unchanged by this choice.

        Returns an independent NumPy float64 array copy, or None before
        fft() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def chir_mag(self) -> NDArray[np.float64] | None:
        """Magnitude of the complex forward transform, paired with r().

        It equals sqrt(chir_real()**2 + chir_imag()**2). For dimensionless chi
        and integer forward kweight w, units are angstrom**(-(w + 1)); the
        default w=2 gives inverse cubic angstroms. Magnitude discards phase
        information and is not a probability density.

        Returns an independent NumPy float64 array copy, or None before
        fft() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def chir_real(self) -> NDArray[np.float64] | None:
        """Real component of the complex forward transform, paired with r().

        The convention uses a negative-exponent FFT followed by kstep/sqrt(pi),
        without additional FFT-length normalization. For dimensionless chi and
        integer kweight w, units are angstrom**(-(w + 1)); the default w=2
        gives inverse cubic angstroms.

        Returns an independent NumPy float64 array copy, or None before
        fft() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def chir_imag(self) -> NDArray[np.float64] | None:
        """Imaginary component of the complex forward transform, paired with r().

        Its sign follows the negative-exponent forward FFT, without an extra
        phase rotation. For dimensionless chi and integer kweight w, units are
        angstrom**(-(w + 1)); the default w=2 gives inverse cubic angstroms.
        Keep the complex components when phase matters.

        Returns an independent NumPy float64 array copy, or None before
        fft() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def q(self) -> NDArray[np.float64] | None:
        """Back-transform q axis in inverse angstroms, paired with chiq().

        q describes the same physical variable as k, but labels a reconstructed
        signal after R filtering. Its spacing is determined by the inverse FFT
        length and the input R spacing; qmax_out limits the returned range.

        Returns an independent NumPy float64 array copy, or None before
        ifft() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""
    def chiq(self) -> NDArray[np.float64] | None:
        """Real R-filtered signal on q(), retaining the forward weighting and window.

        This is generally not the original unweighted chi(k). For dimensionless
        input chi, integer forward kweight w and inverse rweight v, its units
        are angstrom**(v - w); defaults w=2 and v=0 give inverse square
        angstroms. A selected R window removes contributions outside its taper.

        Returns an independent NumPy float64 array copy, or None before
        ifft() (or a dependent stage) succeeds or after invalidation.
        Reading this result does not run processing; editing the copy does
        not change the spectrum."""

class PeakFit:
    """Immutable composite XANES peak definition (since 0.2.10).

    PeakFit((-20, 40)).gaussian("p1", 5, 2, 3).linear_baseline(0, 0)
    starts with Norm and E0-relative eV. Each builder returns a NEW model.
    Peak arguments are center, whole-axis area (signal units times eV), and
    FWHM (eV). Default bounds keep centers in the fit range, areas nonnegative,
    and widths positive. Missing normalization runs on a copy; inputs stay intact.
    No smoothing or automatic chemical/component-count assignment is performed.
    Inspect termination and warnings; covariance is conditional on the chosen model.
    """
    def __init__(self, range: tuple[float, float]) -> None:
        """Create an empty Norm model over inclusive E0-relative eV. Add components before fitting."""
    def flat(self) -> PeakFit:
        """Use dimensionless flattened mu; prerequisites run on a copy. Returns a new model."""
    def raw_mu(self) -> PeakFit:
        """Use the original mapped absorption signal and its units. Returns a new model."""
    def absolute(self) -> PeakFit:
        """Interpret ranges, centers and baseline references as absolute eV. Returns a new model."""
    def reference(self, energy_ev: float) -> PeakFit:
        """Use offsets from this fixed reference energy in eV. Returns a new model."""
    def gaussian(self, name: str, center: float, area: float, fwhm: float) -> PeakFit:
        """Add a Gaussian: center/FWHM in eV, whole-axis area in signal units times eV. Returns a new model."""
    def lorentzian(self, name: str, center: float, area: float, fwhm: float) -> PeakFit:
        """Add a Lorentzian with whole-axis area and FWHM in eV. Returns a new model."""
    def pseudo_voigt(self, name: str, center: float, area: float, fwhm: float, fraction: float) -> PeakFit:
        """Add a common-FWHM mixture; fraction is the Lorentzian share from zero to one. Returns a new model."""
    def voigt(self, name: str, center: float, area: float, gaussian_fwhm: float, lorentzian_fwhm: float) -> PeakFit:
        """Add a true Voigt with independent Gaussian/Lorentzian FWHM in eV. Returns a new model."""
    def erf_step(self, name: str, center: float, height: float, scale: float) -> PeakFit:
        """Add height*(1+erf((E-center)/scale))/2; scale is positive eV. Returns a new model."""
    def arctan_step(self, name: str, center: float, height: float, scale: float) -> PeakFit:
        """Add height*(1/2+atan((E-center)/scale)/pi); scale is positive eV. Returns a new model."""
    def constant_baseline(self, offset: float = 0) -> PeakFit:
        """Add a fitted constant named baseline, in signal units. Returns a new model."""
    def linear_baseline(self, offset: float = 0, slope: float = 0) -> PeakFit:
        """Add baseline = offset+slope*E_offset; slope is signal units/eV. Returns a new model."""
    def exclude(self, range: tuple[float, float]) -> PeakFit:
        """Exclude an inclusive interval in model coordinates. Masked gaps are not integrated."""
    def parameter(self, name: str, value: float, *, vary: bool = True,
                  bounds: tuple[float | None, float | None] = (None, None),
                  expression: str | None = None) -> PeakFit:
        """Replace an EXISTING parameter, returning a new model.

        Names use component_parameter, for example p1_center, p1_area, p1_width.
        Bounds default to unbounded; pass them explicitly to retain restrictions.
        An expression is a restricted tie, not executable code; it overrides vary.
        Dependencies, physical domains and finite values are checked at fit/evaluation.
        Unknown names raise ValueError immediately. Use vary=False to fix a value.
        """
    def as_baseline(self, name: str) -> PeakFit:
        """Make a named peak part of the baseline, excluding it from the area-weighted center.
        Shapes are unchanged; steps must remain edges. Returns a new model."""
    def solver(self, *, max_iterations: int = 200, tolerance: float = 1e-10) -> PeakFit:
        """Set positive iteration/tolerance limits on a new model; validated when fitting."""
    def evaluate(self, energy: NDArray[np.float64] | Sequence[float], *, e0: float | None = None) -> NDArray[np.float64]:
        """Evaluate without fitting or masking at absolute energy in eV.
        E0-relative models require e0 in eV. Returns a new NumPy array;
        invalid definitions and nonfinite arrays raise ValueError."""
    def initialize_baseline(self, spectrum: Spectrum, peak_intervals: Sequence[tuple[float, float]]) -> PeakFit:
        """Initialize only baseline-role variables outside the given peak intervals.
        Intervals use model coordinates; final masks and input spectra stay unchanged.
        Returns a new starting model for a joint final fit. Rust releases the GIL."""
    def fit_batch(self, spectra: Sequence[Spectrum]) -> list[PeakFitOutcome]:
        """Independent unweighted fits from this frozen start, one outcome per input.
        Bad frames keep an error row and do not stop later frames. Inputs are unchanged.
        Rust releases the GIL. Use spectrum.fit_peaks for supplied point errors."""
    def to_json(self) -> str:
        """Serialize the complete initial definition, constraints and masks."""
    @staticmethod
    def from_json(json: str) -> PeakFit:
        """Restore and validate a complete definition; invalid input raises ValueError."""

class PeakFitOutcome:
    """One independent batch outcome in input order (since 0.2.10).
    Exactly one of result/error is present. Nonconvergence is retained as a result."""
    @property
    def index(self) -> int:
        """Zero-based input index, also retained on failure."""
    @property
    def result(self) -> PeakFitResult | None:
        """Owned numerical result; inspect its termination and warnings."""
    @property
    def error(self) -> str | None:
        """Failure reason, otherwise None."""

class PeakFitResult:
    """Owned native fit (since 0.2.10). Array getters return independent copies.
    Result energies/centers are absolute eV; parameter values retain model coordinates.
    Covariance/errors are conditional on the model/noise, not model-selection confidence."""
    @property
    def definition(self) -> PeakFit:
        """Independent copy of the initial definition."""
    def fitted_model(self) -> PeakFit:
        """Copy fitted values for explicit reuse without changing the initial definition."""
    @property
    def parameters(self) -> dict[str, float]:
        """Named final values; parameter centers use the chosen model coordinates."""
    @property
    def parameter_errors(self) -> dict[str, float | None]:
        """Named conditional local errors; None means unavailable or not independently estimated."""
    @property
    def components(self) -> list[PeakContribution]:
        """Component curves and summaries in model order; independent copies."""
    def to_json(self) -> str:
        """Full native result with initial/final constraints, masks and diagnostics."""
    @property
    def origin_ev(self) -> float:
        """Resolved energy origin in eV, added to parameter centers/reference energies."""
    @property
    def energy(self) -> NDArray[np.float64]:
        """Absolute energy in eV, only the native points used by this fit."""
    @property
    def source_indices(self) -> list[int]:
        """Original zero-based point indices; preserves masks and sampling provenance."""
    @property
    def data(self) -> NDArray[np.float64]:
        """Selected representation's measured values, in its signal units."""
    @property
    def model(self) -> NDArray[np.float64]:
        """Joint baseline + peaks + steps, in the same signal units."""
    @property
    def residual(self) -> NDArray[np.float64]:
        """Unweighted data minus model, in signal units (also for weighted fits)."""
    @property
    def standard_deviation(self) -> list[float] | None:
        """Supplied selected-space standard deviations on the fitted points, if any."""
    @property
    def objective(self) -> float:
        """Sum of squared residuals, divided by supplied standard deviations if present."""
    @property
    def points(self) -> int:
        """Number of fitted native data points (not EXAFS independent-point estimates)."""
    @property
    def free_parameters(self) -> int:
        """Number of independent varying parameters; expression ties are excluded."""
    @property
    def degrees_of_freedom(self) -> int:
        """points − free_parameters. Fits with fewer points than variables are rejected."""
    @property
    def jacobian_rank(self) -> int:
        """Weighted numerical Jacobian rank under a 1e-10 relative singular-value cutoff."""
    @property
    def covariance_names(self) -> list[str]:
        """Sorted independent parameter names defining covariance/correlation axes."""
    @property
    def covariance(self) -> list[list[float]] | None:
        """Local covariance; absolute-error scaling when standard deviations were given,
        otherwise multiplied by objective/degrees_of_freedom."""
    @property
    def correlation(self) -> list[list[float]] | None:
        """Dimensionless correlations corresponding to covariance_names. Absent when
        any conditional variance is zero; the warning explains that case."""
    @property
    def uncertainty_unavailable(self) -> str | None:
        """Why covariance/standard errors were withheld, rather than replaced by zero."""
    @property
    def peak_center_ev(self) -> float | None:
        """Model peak-area-weighted center in absolute eV, excluding baseline/steps."""
    @property
    def peak_center_standard_error_ev(self) -> float | None:
        """Conditional error in that center, using full parameter covariance."""
    @property
    def termination(self) -> Literal["FixedModel", "Converged", "NotConverged", "Cancelled"]:
        """Explicit numerical termination category."""
    @property
    def termination_detail(self) -> str:
        """Solver-specific termination detail, retained verbatim for diagnosis."""
    @property
    def evaluations(self) -> int:
        """Number of residual-vector evaluations during optimization, including numerical derivatives."""
    @property
    def warnings(self) -> list[str]:
        """Active bounds and other model/uncertainty limitations."""

class PeakContribution:
    """One component curve and derived metrics (since 0.2.10). Curve getters return copies."""
    @property
    def name(self) -> str:
        """Stable component identity from the initial definition."""
    @property
    def role(self) -> Literal["Peak", "Baseline", "Edge"]:
        """Scientific role, independent of mathematical shape."""
    @property
    def shape(self) -> Literal["Gaussian", "Lorentzian", "PseudoVoigt", "Voigt", "ErfStep", "ArctanStep", "Constant", "Linear"]:
        """Mathematical shape used for evaluation."""
    @property
    def curve(self) -> NDArray[np.float64]:
        """Component values at the result's absolute-energy points, in signal units."""
    @property
    def center_ev(self) -> float | None:
        """Peak/step center in absolute eV; None for polynomial baselines."""
    @property
    def area(self) -> float | None:
        """Whole-axis model area in signal units × eV; None for steps/polynomials."""
    @property
    def height(self) -> float | None:
        """Peak contribution at its center, excluding all other components."""
    @property
    def fwhm_ev(self) -> float | None:
        """Peak FWHM in eV; true Voigt uses a numerical half-height root."""
    @property
    def center_standard_error_ev(self) -> float | None:
        """Conditional errors propagated with the full joint covariance; absent when
        local uncertainty is unavailable or the quantity does not apply."""
    @property
    def area_standard_error(self) -> float | None:
        """Conditional whole-axis area error, in signal units × eV."""
    @property
    def height_standard_error(self) -> float | None:
        """Conditional peak-height error, in signal units."""
    @property
    def fwhm_standard_error_ev(self) -> float | None:
        """Conditional FWHM error, in eV, including both true-Voigt width parameters."""
    @property
    def sampled_integral(self) -> float:
        """Trapezoidal component integral over included native-grid segments only.
        Masked gaps are not bridged; this is not its whole-axis analytic area."""


class MbackErfc:
    """Optional smooth fluorescence background for MBACK (since 0.2.10).

    The line must originate at the selected absorber edge. width=(low, high)
    gives positive eV bounds; amplitude=(low, high) gives finite f2-unit bounds.
    family=False selects one exact line such as Ka1; True selects a within-shell
    family such as Ka. This is not an over-absorption correction. Settings are copied."""
    def __init__(self, line: str, *, width: tuple[float, float], amplitude: tuple[float, float], family: bool = False) -> None:
        """Select an emission and explicit increasing width/amplitude bounds."""
        ...

class MBack:
    """Full Chantler MBACK normalization (since 0.2.10). Example:
    MBack("Cu", "K", pre_edge=(-200, -50), post_edge=(100, 800)).

    Ranges are eV offsets from E0. Degree defaults to 2; erfc is disabled. E0=None
    uses the derivative detector. Automatic ranges use the outer 80% of measured
    pre/post spans with neighboring-edge limits; inspect resolved result ranges.
    The offline atomic table is loaded automatically and never energy shifted.
    fit(energy, mu) leaves both inputs/model unchanged and returns owned norm/flat
    arrays and full diagnostics. set_normalization_method(model) copies settings
    into a Spectrum; normalize() invalidates its dependent background/FFT results.
    Invalid ranges, unsupported data, nonidentifiability and nonpositive scale/step
    raise ValueError. No experimental uncertainty is inferred from fit weights."""
    def __init__(self, element: str, edge: str, *, e0: float | None = None, pre_edge: tuple[float, float] | None = None, post_edge: tuple[float, float] | None = None, degree: int = 2, erfc: MbackErfc | None = None) -> None:
        """Create immutable settings; erfc requires an explicit MbackErfc object."""
        ...
    def fit(self, energy: NDArray[np.float64] | Sequence[float], mu: NDArray[np.float64] | Sequence[float]) -> MbackResult:
        """Fit finite 1D arrays (energy eV, raw absorption), leaving inputs unchanged.

        Copies input buffers and releases the GIL. Returns separate dimensionless
        norm/flat and matched fpp in electron units. Invalid scientific inputs raise ValueError."""
        ...
    def to_json(self) -> str:
        """Serialize native settings without adding input arrays."""
        ...
    @staticmethod
    def from_json(json: str) -> MBack:
        """Restore native settings; fit validates scientific values and reference identity."""
        ...

class MbackResult:
    """Owned full-MBACK output (since 0.2.10). Arrays are returned as independent copies.

    norm=(scale*mu-pre_curve)/Delta is dimensionless; fpp=scale*mu-background
    remains in f2 units. flat separately removes the auxiliary post-edge trend.
    objective uses balanced pre/post sample counts, not inverse measurement
    variances. Inspect warnings/condition and resolved ranges. No covariance or
    experimental confidence interval is implied. to_json retains all provenance."""
    def to_json(self) -> str:
        """Complete result JSON, including reference identity, settings, weights and curves."""
        ...
    @property
    def definition(self) -> MBack:
        """Immutable replay definition pinned to the original table.

        Requested automatic E0/ranges remain automatic; explicit settings remain explicit."""
        ...
    @property
    def reference(self) -> AtomicReference:
        """Independent dictionary with provider, data checksum and table identity."""
        ...
    @property
    def reference_json(self) -> str:
        """Reference identity JSON, including the actual data checksum."""
        ...
    @property
    def pre_edge(self) -> tuple[float, float]:
        """Resolved inclusive pre-edge offsets in eV from E0."""
        ...
    @property
    def post_edge(self) -> tuple[float, float]:
        """Resolved inclusive post-edge offsets in eV from E0."""
        ...
    @property
    def fit_indices(self) -> list[int]:
        """Original zero-based indices included in the objective."""
        ...
    @property
    def warnings(self) -> list[str]:
        """Nonfatal boundary/conditioning diagnostics. Empty does not establish physical validity."""
        ...
    @property
    def erfc_width(self) -> float | None:
        """Positive erfc width in eV, or None when disabled."""
        ...
    @property
    def e0(self) -> float:
        """Resolved fixed energy origin in eV."""
        ...
    @property
    def edge_step(self) -> float:
        """Positive fitted absorption step in input mu units."""
        ...
    @property
    def scale(self) -> float:
        """Positive conversion from input absorption to f2."""
        ...
    @property
    def objective(self) -> float:
        """Sum of squared region-balanced residuals, in squared f2 units."""
        ...
    @property
    def condition(self) -> float:
        """Condition number of the column-scaled final Jacobian."""
        ...
    @property
    def rank(self) -> int:
        """Rank of the final Jacobian, including erfc width when enabled."""
        ...
    @property
    def evaluations(self) -> int:
        """Number of linear solves used by the fit."""
        ...
    @property
    def erfc_amplitude(self) -> float:
        """Fitted erfc amplitude in f2 units; zero when disabled."""
        ...
    @property
    def energy_scale(self) -> float:
        """Polynomial coordinate scale in eV."""
        ...
    @property
    def energy(self) -> NDArray[np.float64]:
        """Original energy grid in eV. Returns a copy."""
        ...
    @property
    def f2(self) -> NDArray[np.float64]:
        """Unshifted atomic scattering factor, in electron units. Returns a copy."""
        ...
    @property
    def fpp(self) -> NDArray[np.float64]:
        """Matched scale*mu-background, in electron units; distinct from norm. Returns a copy."""
        ...
    @property
    def norm(self) -> NDArray[np.float64]:
        """Dimensionless normalized absorption. Returns a copy."""
        ...
    @property
    def flat(self) -> NDArray[np.float64]:
        """Dimensionless flattened absorption using the auxiliary post-edge trend. Returns a copy."""
        ...
    @property
    def background(self) -> NDArray[np.float64]:
        """Fitted smooth background in f2 units. Returns a copy."""
        ...
    @property
    def pre_curve(self) -> NDArray[np.float64]:
        """Auxiliary pre-edge line on f2+background, in f2 units. Returns a copy."""
        ...
    @property
    def post_curve(self) -> NDArray[np.float64]:
        """Auxiliary post-edge curve on f2+background, in f2 units. Returns a copy."""
        ...
    @property
    def residual(self) -> NDArray[np.float64]:
        """Unweighted f2+background-scale*mu on every energy point. Returns a copy."""
        ...
    @property
    def coefficients(self) -> NDArray[np.float64]:
        """Increasing polynomial powers of (energy-e0)/energy_scale, in f2 units. Returns a copy."""
        ...
    @property
    def weights(self) -> NDArray[np.float64]:
        """1/sqrt(region sample count), in fit_indices order. Returns a copy."""
        ...

class AtomicDataIdentity(TypedDict):
    """Exact offline provider, database version and decoded-data checksum."""
    provider: str
    """Named provider/interpolation implementation version."""
    data_version: str
    """Upstream database version."""
    data_sha256: str
    """SHA-256 of the actual decoded data."""

class AtomicReference(TypedDict):
    """Exact atomic dataset and numerical table identity."""
    data: AtomicDataIdentity
    """Actual loaded dataset identity."""
    table: Literal["ChantlerF2LogLogV1", "ElamTotalV1", "ElamTransitionsV1"]
    """Table and interpolation/contribution profile."""

class FluorescenceInternalNormalization(TypedDict):
    """Internal fit of original mu, distinct from final normalization (since 0.2.10).
    All arrays are independent Python lists on the original energy grid."""
    e0: float
    """Measured edge energy in eV."""
    pre_edge: list[float]
    """Resolved pre-edge eV offsets from E0, [start, end]."""
    post_edge: list[float]
    """Resolved post-edge eV offsets from E0, [start, end]."""
    degree: int
    """Internal post-edge polynomial degree; pre-edge is linear."""
    edge_step: float
    """Positive fitted jump in original absorption units, before numerical flooring."""
    pre_curve: list[float]
    """Pre-edge line in original absorption units."""
    post_curve: list[float]
    """Pre-edge line plus post-edge polynomial, in original absorption units."""
    norm: list[float]
    """Dimensionless internal n0 used in alpha+1-n0."""

class FluorescenceCorrection:
    """Optically thick, homogeneous-sample XANES correction (since 0.2.10).

    FluorescenceCorrection("CuO", "Cu", "K", line="Ka1", angles=(45,45))
    requires the complete sample formula, absorber, edge, detected emission and
    measured incident/exit angles. Angles are degrees FROM THE SAMPLE SURFACE,
    each in (0,90]; geometry is never inferred. family=True selects an unresolved
    within-shell family such as Ka. Internal conventional normalization runs
    automatically (degree 1, no Victoreen term). Final normalization is separate.
    The fluo_elam_v1 model is not qualified for EXAFS or finite-thickness samples.
    See https://xraypy.github.io/xraylarch/xafs_preedge.html#over-absorption-corrections."""
    def __init__(self, formula: str, element: str, edge: str, *, line: str,
                 angles: tuple[float, float], family: bool = False, e0: float | None = None,
                 pre_edge: tuple[float, float] | None = None,
                 post_edge: tuple[float, float] | None = None, degree: int = 1) -> None:
        """Copy settings. e0=None detects the edge. Internal pre/post ranges are
        inclusive eV offsets from E0; None uses available low endpoint to -30 eV
        and +100 eV to available high endpoint. Complete coverage is required.
        degree=1 is the internal post-edge degree (0–5). No final normalization runs."""
        ...
    def apply(self, energy: NDArray[np.float64] | Sequence[float], mu: NDArray[np.float64] | Sequence[float]) -> FluorescenceCorrectionResult:
        """Copy original unnormalized fluorescence arrays and release the GIL.
        Energy must be positive, strictly increasing eV; arrays are matching finite
        one-dimensional values. Output keeps original grid/units. Invalid geometry,
        composition, coverage, fitted step or singular denominator raise ValueError;
        nothing is clipped. Array uncertainty is unavailable. Prefer
        Spectrum.correct_fluorescence to preserve restrictions in further processing."""
        ...
    def to_json(self) -> str:
        """Native settings JSON, including any pinned atomic identity."""
        ...
    @staticmethod
    def from_json(json: str) -> FluorescenceCorrection:
        """Restore settings; calculation checks scientific values and data availability."""
        ...

class FluorescenceCorrectionResult:
    """Historical correction (since 0.2.10), with independent arrays and dictionaries.
    Later spectrum edits do not rewrite this record. Inspect amplification/warnings;
    a finite result does not prove physical validity. No uncertainty is claimed."""
    @property
    def energy(self) -> NDArray[np.float64]:
        """Original measured energy in eV, without resampling."""
        ...
    @property
    def original_mu(self) -> NDArray[np.float64]:
        """Original uncorrected absorption, in supplied units."""
        ...
    @property
    def corrected_mu(self) -> NDArray[np.float64]:
        """Corrected absorption, same grid/units; final normalization is separate."""
        ...
    @property
    def factor(self) -> NDArray[np.float64]:
        """Dimensionless alpha/denominator, without clipping."""
        ...
    @property
    def denominator(self) -> NDArray[np.float64]:
        """Dimensionless alpha+1-internal_norm, without clipping."""
        ...
    @property
    def method(self) -> str:
        """Named convention, fluo_elam_v1."""
        ...
    @property
    def input_mode(self) -> AbsorptionMode:
        """Original acquisition interpretation; unknown records a caller assumption."""
        ...
    @property
    def alpha(self) -> float:
        """Dimensionless attenuation/geometry constant."""
        ...
    @property
    def geometry_ratio(self) -> float:
        """sin(incidence)/sin(exit) using measured surface angles; dimensionless."""
        ...
    @property
    def minimum_denominator(self) -> float:
        """Smallest dimensionless denominator on the whole input grid."""
        ...
    @property
    def maximum_amplification(self) -> float:
        """Largest dimensionless factor; high values amplify noise."""
        ...
    @property
    def singularity_threshold(self) -> float:
        """Numerical rejection limit, 64*epsilon*max(1, alpha+1)."""
        ...
    @property
    def warnings(self) -> list[str]:
        """Domain, interpretation and numerical diagnostics; not confidence intervals."""
        ...
    @property
    def definition(self) -> FluorescenceCorrection:
        """Independent settings pinned to resolved ranges/E0 and atomic data."""
        ...
    @property
    def internal(self) -> FluorescenceInternalNormalization:
        """Internal conventional fit of original mu, with independent Python lists."""
        ...
    @property
    def atomic(self) -> dict[str, object]:
        """Edge, emission and compound attenuation records: energies (eV), mass
        fractions, cm²/g values, table identities and checksums."""
        ...
    def to_json(self) -> str:
        """Full native record with original inputs, assumptions and atomic evidence."""
        ...

class WaveletSize(TypedDict):
    """Checked dimensions and buffer estimate; input copies/scratch/serialization add overhead."""
    k_points: int
    """Number of prepared k columns, including padding."""
    r_points: int
    """Number of positive R rows."""
    nfft: int
    """Internal FFT length, in samples."""
    cells: int
    """Number of complex map cells."""
    bytes: int
    """Estimated scientific buffer bytes; input copies and scratch add overhead."""

class Wavelet:
    """Cauchy settings, available since 0.2.10. Use spectrum.wavelet(Wavelet((2, 12))).

    k_range is measured support in inverse angstroms. Defaults: weight 2, order
    100, kstep 0.05, R maximum 6 angstroms, no taper and automatic FFT/R sampling.
    Missing normalization/AUTOBK run on a copy; existing chi is reused. Larger
    order narrows frequency response and broadens localization in k. R is not
    phase-corrected. Fixed order is independent of R extent (cauchy_v1).
    Calculations release the GIL and return owned results; invalid definitions,
    intervals wholly outside the measured k range and excessive allocations
    raise ValueError. Since 0.2.15 endpoints beyond the measured k range are
    limited to it; the map keeps the requested settings and its warnings
    report the range used."""
    def __init__(self, k_range: tuple[float, float], *, kweight: int = 2,
                 order: int = 100, kstep: float = 0.05, rmax: float = 6.0,
                 rstep: float | None = None, taper: float = 0.0,
                 nfft: int | None = None, radii: Sequence[float] | None = None) -> None:
        """Copy settings. taper is half-cosine width inside support (inverse angstroms);
        zero means none. radii replaces generated positive increasing R coordinates.
        nfft must be a power of two at least twice the prepared grid length."""
        ...
    def calculate(self, k: NDArray[np.float64] | Sequence[float], chi: NDArray[np.float64] | Sequence[float]) -> WaveletMap:
        """Copy original unweighted chi(k), release the GIL and calculate a native map.
        k is finite, nonnegative, increasing and in inverse angstroms; chi is finite
        and dimensionless. Linear resampling never extrapolates measured support.
        Original inputs are retained; no display sampling alters the result."""
        ...
    def estimate(self, k: NDArray[np.float64] | Sequence[float]) -> WaveletSize:
        """Validate dimensions and estimate scientific buffer bytes before calculation."""
        ...
    def to_json(self) -> str:
        """Native settings JSON, with automatic choices preserved and no input arrays."""
        ...
    @staticmethod
    def from_json(json: str) -> Wavelet:
        """Restore native settings; calculation validates scientific values and budgets."""
        ...

class WaveletMap:
    """Owned native Cauchy map (since 0.2.10), with independent NumPy array properties.

    Matrices have shape (R rows, k columns), including explicit k padding. W has
    units of k**weight * chi, distinct from ordinary Fourier scaling. No color
    normalization changes the scientific data; to_json retains full provenance."""
    @property
    def shape(self) -> tuple[int, int]:
        """Matrix dimensions: R rows, k columns."""
        ...
    @property
    def k(self) -> NDArray[np.float64]:
        """Independent inverse-angstrom coordinates, including padded columns."""
        ...
    @property
    def r(self) -> NDArray[np.float64]:
        """Independent angstrom coordinates; not phase-corrected distances."""
        ...
    @property
    def input_k(self) -> NDArray[np.float64]:
        """Original measured k before resampling."""
        ...
    @property
    def input_chi(self) -> NDArray[np.float64]:
        """Original unweighted dimensionless chi, unchanged."""
        ...
    @property
    def prepared_chi(self) -> NDArray[np.float64]:
        """Resampled unweighted chi; values outside support are padding zeros."""
        ...
    @property
    def window(self) -> NDArray[np.float64]:
        """Support/taper multipliers applied before k weighting."""
        ...
    @property
    def support(self) -> NDArray[np.bool_]:
        """True for selected measured support; False for padding."""
        ...
    @property
    def real(self) -> NDArray[np.float64]:
        """Independent real matrix, rows=R and columns=k."""
        ...
    @property
    def imaginary(self) -> NDArray[np.float64]:
        """Independent imaginary matrix, rows=R and columns=k."""
        ...
    @property
    def magnitude(self) -> NDArray[np.float64]:
        """Independent full-native-grid magnitude matrix."""
        ...
    def phase(self, relative_floor: float = 0.01) -> NDArray[np.float64]:
        """Radians; NaN masks zero amplitude and values below a fraction of the map
        maximum (default 1%). Fraction is in [0,1]. Native data stay unchanged."""
        ...
    def slice_at_r(self, r: float) -> NDArray[np.float64]:
        """Native magnitude versus k at a covered R coordinate (angstroms)."""
        ...
    def slice_at_k(self, k: float) -> NDArray[np.float64]:
        """Native magnitude versus R at a covered k coordinate (inverse angstroms)."""
        ...
    def integral(self, k_range: tuple[float, float], r_range: tuple[float, float]) -> WaveletRegionValue:
        """Integrate native bilinear magnitude over a fully covered rectangle.
        k_range uses inverse angstroms and r_range angstroms. Returns exact bounds,
        value, units and method without experimental uncertainty. Releases the GIL;
        display sampling never participates. Invalid coverage raises ValueError."""
        ...
    def mean(self, k_range: tuple[float, float], r_range: tuple[float, float]) -> WaveletRegionValue:
        """Area-weighted mean of native bilinear magnitude (since 0.2.10).
        k is inverse angstroms; R is angstroms. Requires increasing, fully covered
        ranges. Returns units and method without uncertainty; releases the GIL."""
        ...
    def maximum(self, k_range: tuple[float, float], r_range: tuple[float, float]) -> WaveletRegionValue:
        """Maximum native bilinear magnitude, including rectangle boundaries
        (since 0.2.10). k is inverse angstroms; R is angstroms. Invalid coverage
        raises ValueError. Returns units/method without uncertainty; releases the GIL."""
        ...
    @property
    def definition(self) -> Wavelet:
        """Independent transform definition, retaining automatic and explicit choices."""
        ...
    @property
    def preparation(self) -> dict[str, object] | None:
        """Original spectrum preparation metadata; None for direct array calculations."""
        ...
    @property
    def warnings(self) -> list[str]:
        """Interpretation/boundary diagnostics, not confidence intervals."""
        ...
    def to_json(self) -> str:
        """Complete native map, original inputs and processing provenance."""
        ...
    @staticmethod
    def from_json(json: str) -> WaveletMap:
        """Restore checked method, dimensions, axes, finite values and budgets.
        Validation does not independently prove external numerical results."""
        ...

class WaveletRegionValue:
    """Immutable native magnitude statistic. dk times dR cancels for integrals; units equal
    k**weight * chi. This is a descriptive transform metric, not concentration."""
    @property
    def value(self) -> float:
        """Native region statistic, without an experimental uncertainty estimate."""
        ...
    @property
    def k_range(self) -> tuple[float, float]:
        """Exact inclusive k bounds in inverse angstroms."""
        ...
    @property
    def r_range(self) -> tuple[float, float]:
        """Exact inclusive R bounds in angstroms."""
        ...
    @property
    def unit(self) -> str:
        """Units of k**weight * chi for all three region statistics."""
        ...
    @property
    def method(self) -> str:
        """Method: bilinear_magnitude_v1 (integral), bilinear_magnitude_mean_v1
        or bilinear_magnitude_maximum_v1."""
        ...
    def to_json(self) -> str:
        """Value, exact bounds, units and method as JSON."""
        ...
