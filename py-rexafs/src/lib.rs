//! Thin Python bindings: all stage execution and defaults live in rexafs.
use numpy::{PyArray1, PyReadonlyArray1};
use pyo3::prelude::*;
mod fluorescence;
mod mback;
mod metrics;
mod peaks;
mod wavelet;

type PySpectrumArrays<'py> = (Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>);

fn error(error: rexafs::Error) -> PyErr {
    match error {
        rexafs::Error::Data(_) | rexafs::Error::Normalization(_) => {
            pyo3::exceptions::PyValueError::new_err(error.to_string())
        }
        _ => pyo3::exceptions::PyRuntimeError::new_err(error.to_string()),
    }
}
fn arrays(
    py: Python<'_>,
    energy: &Bound<'_, PyAny>,
    mu: &Bound<'_, PyAny>,
) -> PyResult<rexafs::Spectrum> {
    let asarray = py.import("numpy")?.getattr("asarray")?;
    let energy = asarray.call1((energy, "float64"))?;
    let mu = asarray.call1((mu, "float64"))?;
    let energy = energy
        .extract::<PyReadonlyArray1<'_, f64>>()
        .map_err(|_| pyo3::exceptions::PyValueError::new_err("energy must be one-dimensional"))?;
    let mu = mu
        .extract::<PyReadonlyArray1<'_, f64>>()
        .map_err(|_| pyo3::exceptions::PyValueError::new_err("mu must be one-dimensional"))?;
    let energy = energy.as_array().iter().copied().collect::<Vec<_>>();
    let mu = mu.as_array().iter().copied().collect::<Vec<_>>();
    rexafs::Spectrum::from_arrays(&energy, &mu).map_err(error)
}

/// Configure the baselines and edge step used to normalize absorption.
///
/// A line is fitted before the absorption edge and a polynomial after it.
/// The normalized signal is (mu - pre_edge) / edge_step; mu and both baselines
/// have the same absorption units, so the result is dimensionless. The edge
/// step is the fitted baseline difference near E0 unless you override it.
/// Fit limits are energy offsets from E0 in eV, not absolute energies.
///
/// All fields default to None and are resolved from the spectrum when processing.
/// These automatic choices match Rust PrePostEdge::new(); Rust's Default trait
/// uses fixed ranges instead. Start with automatic settings and inspect the
/// baselines before choosing narrower ranges or a higher polynomial degree.
/// Settings are copied into a method and spectrum; edit and assign them again
/// to apply a later change.
///
/// Example: p = PrePostEdge(); p.pre_edge_end = -30.0. Assign p to the
/// spectrum's normalization stage, then call normalize() or a later stage.
///
/// For the measurement and normalization convention, see
/// [Newville, Fundamentals of XAFS, sections 4 and 5](https://docs.xrayabsorption.org/tutorials/XAFS_Fundamentals.pdf).
/// Automatic range selection and the numerical safeguards are rexafs choices;
/// see [processing theory](https://rexafs.com/docs/science/processing/).
///
/// Resolved fit ranges, polynomial degree and Victoreen exponent are retained
/// inside the spectrum. An automatically estimated edge step is recalculated
/// after normalization results are invalidated.
/// Processing does not replace None fields in your original settings object.
/// Reassign fresh or reset settings when you want retained automatic choices
/// recalculated after changing the data or an earlier stage.
#[pyclass(name = "PrePostEdge", module = "rexafs", skip_from_py_object)]
#[derive(Clone)]
struct PyPrePostEdge {
    inner: rexafs::PrePostEdge,
}
#[pymethods]
impl PyPrePostEdge {
    /// Create automatic pre/post-edge settings, with every field initially None.
    ///
    /// Set only the fields your data require, then assign the settings to the
    /// spectrum's normalization stage. Fit ranges and degrees are
    /// resolved when normalization runs; creating settings does not process data.
    ///
    /// Keyword arguments were added in 0.2.5. Published
    /// 0.2.4 settings use construction without arguments followed by field
    /// assignment. Python type conversion can raise TypeError, and an integer
    /// outside the native field's representable range can raise OverflowError
    /// before any numerical processing.
    ///
    /// Normalization fit choices are validated when a stage runs. To see the
    /// effect of a change, inspect pre_edge(), post_edge(), norm() and flat()
    /// after assigning the settings and calling normalize().
    #[new]
    #[pyo3(
        text_signature = "(*, pre_edge_start=None, pre_edge_end=None, norm_start=None, norm_end=None, norm_polyorder=None, n_victoreen=None, e0=None, edge_step=None)"
    )]
    #[pyo3(signature = (*, pre_edge_start=None, pre_edge_end=None, norm_start=None, norm_end=None, norm_polyorder=None, n_victoreen=None, e0=None, edge_step=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        pre_edge_start: Option<f64>,
        pre_edge_end: Option<f64>,
        norm_start: Option<f64>,
        norm_end: Option<f64>,
        norm_polyorder: Option<i32>,
        n_victoreen: Option<i32>,
        e0: Option<f64>,
        edge_step: Option<f64>,
    ) -> PyResult<Self> {
        let mut result = Self {
            inner: rexafs::PrePostEdge::new(),
        };
        result.set_pre_edge_start(pre_edge_start);
        result.set_pre_edge_end(pre_edge_end);
        result.set_norm_start(norm_start);
        result.set_norm_end(norm_end);
        result.set_norm_polyorder(norm_polyorder);
        result.set_n_victoreen(n_victoreen);
        result.set_e0(e0);
        result.set_edge_step(edge_step);
        Ok(result)
    }
    /// Start of the pre-edge fit, as an offset from E0 in eV.
    ///
    /// Default None estimates a lower bound near the second energy sample, rounded
    /// on a 2 or 5 eV grid and clipped to the measured range. Usually this offset
    /// is negative. Choose a range below the edge rise. Since 0.2.15 an explicit
    /// start before the first measured energy is raised to it when normalize() runs
    /// (the stored value is kept), and a start at or above pre_edge_end is an error
    /// naming pre_edge_start instead of being swapped.
    #[getter]
    fn pre_edge_start(&self) -> Option<f64> {
        self.inner.pre_edge_start
    }
    #[setter]
    fn set_pre_edge_start(&mut self, value: Option<f64>) {
        self.inner.pre_edge_start = value;
    }
    /// End of the pre-edge fit, as an offset from E0 in eV.
    ///
    /// Default None uses 5 * round(pre_edge_start / 15), approximately one third
    /// of the lower offset. A more negative value excludes more of the edge rise
    /// but leaves fewer baseline samples. Keep at least two usable pre-edge points.
    #[getter]
    fn pre_edge_end(&self) -> Option<f64> {
        self.inner.pre_edge_end
    }
    #[setter]
    fn set_pre_edge_end(&mut self, value: Option<f64>) {
        self.inner.pre_edge_end = value;
    }
    /// Start of the post-edge polynomial fit, as an offset from E0 in eV.
    ///
    /// Default None uses 5 * round(norm_end / 15), capped at 25 eV, then ensures
    /// at least a 10 eV separation from norm_end. Select a region beyond the edge
    /// rise; very short scans can make this automatic choice unsuitable. Since
    /// 0.2.15, when a start lies beyond the (data-limited) norm_end, normalize() uses
    /// min(25, 5 * round(norm_end / 15)) eV instead of failing; the setting is kept.
    #[getter]
    fn norm_start(&self) -> Option<f64> {
        self.inner.norm_start
    }
    #[setter]
    fn set_norm_start(&mut self, value: Option<f64>) {
        self.inner.norm_start = value;
    }
    /// End of the post-edge polynomial fit, as an offset from E0 in eV.
    ///
    /// Default None rounds the available upper energy offset to a 5 eV grid and
    /// clips it to the data limit. Reducing this bound excludes high-energy data
    /// from the normalization fit; it does not trim the spectrum itself. Since
    /// 0.2.15 an explicit value beyond the last measured energy is lowered to it
    /// when normalize() runs; the stored setting keeps the requested value.
    #[getter]
    fn norm_end(&self) -> Option<f64> {
        self.inner.norm_end
    }
    #[setter]
    fn set_norm_end(&mut self, value: Option<f64>) {
        self.inner.norm_end = value;
    }
    /// Degree of the polynomial fitted to the pre-edge-subtracted post-edge data.
    ///
    /// Default None chooses degree 0 for a fit span below 50 eV, 1 below 350 eV,
    /// and 2 otherwise. Explicit degrees are clamped to 0 through 5. Higher degrees
    /// can follow baseline curvature but also fit noise or oscillations; begin
    /// with the automatic choice. The fit needs more samples than its degree.
    #[getter]
    fn norm_polyorder(&self) -> Option<i32> {
        self.inner.norm_polyorder
    }
    #[setter]
    fn set_norm_polyorder(&mut self, value: Option<i32>) {
        self.inner.norm_polyorder = value;
    }
    /// Energy exponent used to fit the pre-edge baseline. Default None resolves to 0.
    ///
    /// For exponent n, the code fits mu(E) * E**n with a line, then divides that
    /// line by E**n to recover the baseline in mu units; E is in eV. With n=0
    /// this is an ordinary straight-line baseline. A nonzero exponent changes
    /// its curvature; it is a baseline model, not a correction for self-absorption.
    #[getter]
    fn n_victoreen(&self) -> Option<i32> {
        self.inner.n_victoreen
    }
    #[setter]
    fn set_n_victoreen(&mut self, value: Option<i32>) {
        self.inner.n_victoreen = value;
    }
    /// Absorption edge energy E0 in eV. Default: None for automatic detection.
    ///
    /// E0 defines the origin of the fit-range offsets and the conversion from
    /// energy to photoelectron k. Assigning these settings can replace a spectrum's
    /// previous E0. Spectrum processing rejects an explicit E0 that is non-finite
    /// or not strictly inside the measured energy range with ValueError; it does
    /// not silently replace such an invalid override with an automatic value. The
    /// lower-level Rust baseline fitter has its own redetection safeguards near the
    /// end of the scan, so inspect the resolved Spectrum.e0(). An automatic
    /// derivative estimate is not an energy calibration.
    #[getter]
    fn e0(&self) -> Option<f64> {
        self.inner.e0
    }
    #[setter]
    fn set_e0(&mut self, value: Option<f64>) {
        self.inner.e0 = value;
    }
    /// Override the absorption edge step, in the same units as mu.
    ///
    /// Default None estimates post_edge - pre_edge at the sample nearest E0.
    /// Normalization divides by this value, so changing it rescales norm and chi.
    /// Use a finite positive value if a separately determined step is available.
    /// The implementation floors finite steps below 1e-12 to 1e-12; that safeguard
    /// does not make a zero or negative experimental edge step meaningful.
    #[getter]
    fn edge_step(&self) -> Option<f64> {
        self.inner.edge_step
    }
    #[setter]
    fn set_edge_step(&mut self, value: Option<f64>) {
        self.inner.edge_step = value;
    }
}

/// Fit a smooth atomic background and extract the EXAFS oscillation chi(k).
///
/// AUTOBK fits a cubic spline in k to suppress low-R Fourier content of the
/// background-subtracted signal. Subtracting that background and dividing
/// by the normalization edge step gives dimensionless chi. rbkg controls the
/// low-R region treated as background; a value that reaches the first
/// structural shell can remove the signal you want to measure.
///
/// Recommended starting values are rbkg=1.0 angstrom, kstep=0.05 inverse
/// angstroms, kweight=1, window="Hanning", solver="LinearDirect", and
/// clamp_scale_policy="FixedPenalty" with clamp_lambda=0.001. The fixed
/// penalty discourages large endpoint oscillations; its strength is a
/// rexafs convention, not a universal statistical regularization parameter.
/// None resolves optional fields when processing, as described per field.
///
/// Example: p = AUTOBK(); p.rbkg = 1.2. Assign p to the spectrum's background
/// stage, then call calc_background() or a later stage.
/// Settings are copied; assigning them again is required after later edits.
/// Invalid ranges, an incompatible solver, or a failed solve raise RuntimeError
/// during processing. A solved background still needs scientific inspection.
///
/// Original AUTOBK method: [Newville et al. (1993)](https://doi.org/10.1103/PhysRevB.47.14126).
/// The fixed endpoint penalty and linear solution are rexafs-specific choices;
/// see the [implemented AUTOBK objective](https://rexafs.com/docs/science/autobk/).
///
/// Resolved scalar defaults and ek0 are retained inside the spectrum.
/// Automatic kmax and nknots remain unset in the settings and are calculated
/// locally from the current input on each background call.
/// Processing does not replace None fields in your original settings object.
/// Reassign fresh or reset settings when you want retained automatic choices
/// recalculated after changing the data or an earlier stage.
#[pyclass(name = "AUTOBK", module = "rexafs", skip_from_py_object)]
#[derive(Clone)]
struct PyAUTOBK {
    inner: rexafs::AUTOBK,
}
#[pymethods]
impl PyAUTOBK {
    /// Create AUTOBK settings with the recommended Rust defaults.
    ///
    /// Begin with rbkg=1.0 angstrom and the LinearDirect/FixedPenalty pair.
    /// Edit only the fields your data require, then assign the settings to the
    /// spectrum's background stage. Construction does not fit a spectrum;
    /// numeric range and solver compatibility checks occur during processing.
    ///
    /// Keyword arguments were added in 0.2.5. Published
    /// 0.2.4 settings use construction without arguments followed by field
    /// assignment. Python type conversion can raise TypeError, and an integer
    /// outside the native field's representable range can raise OverflowError
    /// before any numerical processing.
    #[new]
    #[pyo3(
        text_signature = "(*, ek0=None, rbkg=1.0, nknots=None, kmin=0.0, kmax=None, kstep=0.05, nclamp=3, clamp_lo=0, clamp_hi=1, clamp_lambda=0.001, nfft=2048, kweight=1, dk=0.1, linear_regularization=0.0001, linear_condition_limit=100000000.0, linear_residual_ratio_limit=1.05, linear_fallback_to_lm=True, linear_workspace_cache=True, window='Hanning', solver='LinearDirect', linear_fallback_solver='TrustRegionDogLeg', clamp_scale_policy='FixedPenalty')"
    )]
    #[pyo3(signature = (*, ek0=None, rbkg=Some(1.0), nknots=None, kmin=Some(0.0), kmax=None, kstep=Some(0.05), nclamp=Some(3), clamp_lo=Some(0), clamp_hi=Some(1), clamp_lambda=Some(0.001), nfft=Some(2048), kweight=Some(1), dk=Some(0.1), linear_regularization=Some(0.0001), linear_condition_limit=Some(100000000.0), linear_residual_ratio_limit=Some(1.05), linear_fallback_to_lm=Some(true), linear_workspace_cache=Some(true), window=Some("Hanning"), solver=Some("LinearDirect"), linear_fallback_solver=Some("TrustRegionDogLeg"), clamp_scale_policy=Some("FixedPenalty")))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        ek0: Option<f64>,
        rbkg: Option<f64>,
        nknots: Option<i32>,
        kmin: Option<f64>,
        kmax: Option<f64>,
        kstep: Option<f64>,
        nclamp: Option<i32>,
        clamp_lo: Option<i32>,
        clamp_hi: Option<i32>,
        clamp_lambda: Option<f64>,
        nfft: Option<i32>,
        kweight: Option<i32>,
        dk: Option<f64>,
        linear_regularization: Option<f64>,
        linear_condition_limit: Option<f64>,
        linear_residual_ratio_limit: Option<f64>,
        linear_fallback_to_lm: Option<bool>,
        linear_workspace_cache: Option<bool>,
        window: Option<&str>,
        solver: Option<&str>,
        linear_fallback_solver: Option<&str>,
        clamp_scale_policy: Option<&str>,
    ) -> PyResult<Self> {
        let mut result = Self {
            inner: rexafs::AUTOBK::new(),
        };
        result.set_ek0(ek0);
        result.set_rbkg(rbkg);
        result.set_nknots(nknots);
        result.set_kmin(kmin);
        result.set_kmax(kmax);
        result.set_kstep(kstep);
        result.set_nclamp(nclamp);
        result.set_clamp_lo(clamp_lo);
        result.set_clamp_hi(clamp_hi);
        result.set_clamp_lambda(clamp_lambda);
        result.set_nfft(nfft);
        result.set_kweight(kweight);
        result.set_dk(dk);
        result.set_linear_regularization(linear_regularization);
        result.set_linear_condition_limit(linear_condition_limit);
        result.set_linear_residual_ratio_limit(linear_residual_ratio_limit);
        result.set_linear_fallback_to_lm(linear_fallback_to_lm);
        result.set_linear_workspace_cache(linear_workspace_cache);
        result.set_window(window.map(str::to_owned))?;
        result.set_solver(solver.map(str::to_owned))?;
        result.set_linear_fallback_solver(linear_fallback_solver.map(str::to_owned))?;
        result.set_clamp_scale_policy(clamp_scale_policy.map(str::to_owned))?;
        Ok(result)
    }
    /// Edge energy used to convert energy to k, in eV.
    ///
    /// Default None uses the normalization E0. Above the edge, k is approximately
    /// sqrt((E - E0) / 3.81), with E in eV and k in inverse
    /// angstroms; the conversion constant has units eV * angstrom**2.
    /// Prefer Spectrum.set_e0() when changing the edge for all stages
    /// together; an independent background edge can differ from normalization.
    #[getter]
    fn ek0(&self) -> Option<f64> {
        self.inner.ek0
    }
    #[setter]
    fn set_ek0(&mut self, value: Option<f64>) {
        self.inner.ek0 = value;
    }
    /// Positive background cutoff in angstroms. Default: 1.0; None resolves to 1.0.
    ///
    /// AUTOBK suppresses low-R Fourier residuals up to a cutoff derived from this
    /// value, the k range, and the discrete R grid; it is not an exact continuous
    /// boundary. Increasing rbkg allows a more flexible spline and can remove
    /// real short-distance structure. Start near 1.0 and keep it below the first
    /// physical shell of interest.
    #[getter]
    fn rbkg(&self) -> Option<f64> {
        self.inner.rbkg
    }
    #[setter]
    fn set_rbkg(&mut self, value: Option<f64>) {
        self.inner.rbkg = value;
    }
    /// Requested number of spline coefficients/anchor points.
    ///
    /// Default None uses 1 + floor(2 * rbkg * (kmax - kmin) / pi), clamped to
    /// 5 through 128; explicit values are clamped to the same range. rbkg is in
    /// angstroms and k limits in inverse angstroms, so the count is dimensionless.
    /// More coefficients increase background flexibility. Leave this automatic
    /// unless testing a justified spline model.
    #[getter]
    fn nknots(&self) -> Option<i32> {
        self.inner.nknots
    }
    #[setter]
    fn set_nknots(&mut self, value: Option<i32>) {
        self.inner.nknots = value;
    }
    /// Lower k bound of the background fit and Fourier window, in inverse
    /// angstroms.
    ///
    /// Default: 0.0. None also resolves to 0.0. Raising it excludes low-k data from
    /// the Fourier objective and changes the spline geometry. It must be below the
    /// effective kmax. The returned k() array still starts at zero.
    #[getter]
    fn kmin(&self) -> Option<f64> {
        self.inner.kmin
    }
    #[setter]
    fn set_kmin(&mut self, value: Option<f64>) {
        self.inner.kmin = value;
    }
    /// Upper k bound of the background fit, in inverse angstroms.
    ///
    /// Default None uses the available data limit; an explicit larger value is
    /// clipped to that limit. Lower it to exclude a noisy high-k tail. This also
    /// changes the spline count and the extent of the returned k()/chi() arrays;
    /// it is independent of the later XrayFFTF.kmax setting.
    #[getter]
    fn kmax(&self) -> Option<f64> {
        self.inner.kmax
    }
    #[setter]
    fn set_kmax(&mut self, value: Option<f64>) {
        self.inner.kmax = value;
    }
    /// Spacing of the uniform output k grid, in inverse angstroms.
    ///
    /// Default: 0.05. None also resolves to 0.05. Values must be finite and
    /// positive. The grid begins at zero and is used by Spectrum.k() and chi().
    /// Smaller steps produce more interpolated samples without adding measured
    /// information. This usually supplies the automatic spacing of the later
    /// forward transform.
    ///
    /// For FixedPenalty, the internal objective FFT uses a fixed amplitude
    /// reference of 0.05 / sqrt(pi); kstep still sets the physical R spacing
    /// and low-R cutoff. Legacy clamp policies use kstep / sqrt(pi) instead.
    /// This internal convention is separate from the public forward transform.
    /// If that transform has already run, assign fresh XrayFFTF settings when
    /// changing this step so its automatic spacing is resolved again.
    #[getter]
    fn kstep(&self) -> Option<f64> {
        self.inner.kstep
    }
    #[setter]
    fn set_kstep(&mut self, value: Option<f64>) {
        self.inner.kstep = value;
    }
    /// Number of endpoint samples included at each enabled end of the clamp.
    ///
    /// Default: 3. None also resolves to 3; 0 disables endpoint penalties.
    /// FixedPenalty uses the first and last nclamp samples, limited to the
    /// available length, and requires a nonnegative value. Increasing the count
    /// constrains a wider endpoint region. clamp_lo/clamp_hi select which ends
    /// contribute.
    #[getter]
    fn nclamp(&self) -> Option<i32> {
        self.inner.nclamp
    }
    #[setter]
    fn set_nclamp(&mut self, value: Option<i32>) {
        self.inner.nclamp = value;
    }
    /// Relative weight of the low-k endpoint penalty. Default: 0. None also
    /// resolves to 0.
    ///
    /// Zero disables this end. For FixedPenalty, the absolute integer weight
    /// multiplies the endpoint residual before squaring; doubling it gives four
    /// times the squared contribution for a fixed residual. Start at zero so the
    /// near-edge oscillation is not forced toward zero.
    #[getter]
    fn clamp_lo(&self) -> Option<i32> {
        self.inner.clamp_lo
    }
    #[setter]
    fn set_clamp_lo(&mut self, value: Option<i32>) {
        self.inner.clamp_lo = value;
    }
    /// Relative weight of the high-k endpoint penalty. Default: 1. None also
    /// resolves to 1.
    ///
    /// Zero disables this end. For FixedPenalty, the absolute integer weight
    /// multiplies endpoint residuals before squaring. Larger values discourage
    /// large high-k endpoint oscillations more strongly but may bias a real
    /// oscillation; clamp_lambda controls the overall penalty strength.
    #[getter]
    fn clamp_hi(&self) -> Option<i32> {
        self.inner.clamp_hi
    }
    #[setter]
    fn set_clamp_hi(&mut self, value: Option<i32>) {
        self.inner.clamp_hi = value;
    }
    /// Strength of the FixedPenalty mean-square endpoint term. Default: 0.001.
    ///
    /// None resolves to 0.001; 0 disables both endpoint penalties regardless of
    /// their weights. Values must be finite and nonnegative. Increasing this
    /// value favors smaller endpoint oscillations over the low-R objective.
    /// Its numerical meaning depends on the implemented Fourier scaling and
    /// weights; it is not an uncertainty estimate. Unused by Fixed and TwoPass.
    ///
    /// FixedPenalty evaluates its low-R residual with an internal FFT factor
    /// of 0.05 / sqrt(pi). Changing the background k weight or window changes
    /// the numerical balance against the endpoint penalty, even with unchanged
    /// lambda. See the [implemented objective](https://rexafs.com/docs/science/autobk/).
    #[getter]
    fn clamp_lambda(&self) -> Option<f64> {
        self.inner.clamp_lambda
    }
    #[setter]
    fn set_clamp_lambda(&mut self, value: Option<f64>) {
        self.inner.clamp_lambda = value;
    }
    /// Number of samples in the FFT used inside background removal.
    ///
    /// Default: 2048. None also resolves to 2048. Use a positive length large
    /// enough to contain the output k grid: the underlying FFT otherwise keeps only
    /// its first nfft samples. A larger length makes the R grid finer through zero
    /// padding; it does not improve experimental resolution. This setting is
    /// independent of the later XrayFFTF.nfft.
    #[getter]
    fn nfft(&self) -> Option<i32> {
        self.inner.nfft
    }
    #[setter]
    fn set_nfft(&mut self, value: Option<i32>) {
        self.inner.nfft = value;
    }
    /// Integer exponent of k in the background Fourier objective. Default: 1.
    ///
    /// None resolves to 1. Larger nonnegative weights emphasize higher-k
    /// oscillations and their noise while the spline is fitted. Use nonnegative
    /// values on the zero-origin grid; negative powers are singular at k=0.
    /// This weight does not remain in the returned chi() and is independent
    /// of the exponent used by the later forward transform.
    #[getter]
    fn kweight(&self) -> Option<i32> {
        self.inner.kweight
    }
    #[setter]
    fn set_kweight(&mut self, value: Option<i32>) {
        self.inner.kweight = value;
    }
    /// Window taper parameter for the background objective. Default: 0.1.
    ///
    /// None resolves to 0.1. For the default Hanning window this sets the taper
    /// width at each k bound, in inverse angstroms. A broader taper softens
    /// truncation but reduces the strongly weighted range. Other window families
    /// interpret the parameter differently; KaiserBessel also uses it as a
    /// shape parameter. Inspect the window when changing families.
    #[getter]
    fn dk(&self) -> Option<f64> {
        self.inner.dk
    }
    #[setter]
    fn set_dk(&mut self, value: Option<f64>) {
        self.inner.dk = value;
    }
    /// Ridge strength for legacy Fixed/TwoPass direct solves. Default: 0.0001.
    ///
    /// None resolves to 0.0001. It adds a scaled diagonal regularization to the
    /// legacy normal equations, trading closeness to the unregularized solution
    /// for stability. FixedPenalty does not use this parameter; changing it
    /// does not regularize the recommended column-scaled SVD solve.
    #[getter]
    fn linear_regularization(&self) -> Option<f64> {
        self.inner.linear_regularization
    }
    #[setter]
    fn set_linear_regularization(&mut self, value: Option<f64>) {
        self.inner.linear_regularization = value;
    }
    /// Largest accepted condition measure for a direct solve. Default: 1e8.
    ///
    /// None resolves to 1e8. FixedPenalty requires a finite value at least 1
    /// and checks the largest/smallest singular-value ratio of its column-scaled
    /// design matrix. Legacy solves use a different condition proxy. Lowering
    /// the limit rejects more unstable systems; raising it can accept sensitive
    /// solutions. FixedPenalty failures raise RuntimeError without a fallback.
    #[getter]
    fn linear_condition_limit(&self) -> Option<f64> {
        self.inner.linear_condition_limit
    }
    #[setter]
    fn set_linear_condition_limit(&mut self, value: Option<f64>) {
        self.inner.linear_condition_limit = value;
    }
    /// Maximum solved/base residual-norm ratio for legacy direct solves.
    ///
    /// Default: 1.05. None also resolves to 1.05; legacy processing clamps it to at
    /// least 1. Exceeding the limit rejects the direct solution and may trigger the
    /// configured fallback. FixedPenalty does not use this legacy acceptance test;
    /// its rank, condition and stationarity checks are separate.
    #[getter]
    fn linear_residual_ratio_limit(&self) -> Option<f64> {
        self.inner.linear_residual_ratio_limit
    }
    #[setter]
    fn set_linear_residual_ratio_limit(&mut self, value: Option<f64>) {
        self.inner.linear_residual_ratio_limit = value;
    }
    /// Allow a rejected legacy direct solve to use linear_fallback_solver.
    ///
    /// Default: True. None also resolves to True. Despite its historical name, the
    /// selected fallback need not be Levenberg-Marquardt. False makes a rejected
    /// legacy direct solve raise an error. FixedPenalty never falls back: it
    /// returns a failed solve as RuntimeError regardless of this setting.
    #[getter]
    fn linear_fallback_to_lm(&self) -> Option<bool> {
        self.inner.linear_fallback_to_lm
    }
    #[setter]
    fn set_linear_fallback_to_lm(&mut self, value: Option<bool>) {
        self.inner.linear_fallback_to_lm = value;
    }
    /// Reuse compatible background-solver geometry and factorization. Default: True.
    ///
    /// None resolves to True. FixedPenalty caches spline/FFT design matrices,
    /// column scaling and singular-value decomposition factors for matching
    /// geometry; each spectrum still supplies its own data and receives a new
    /// solution. False disables this reuse, mainly for comparisons or profiling;
    /// the cache does not reuse a previous spectrum's chi().
    #[getter]
    fn linear_workspace_cache(&self) -> Option<bool> {
        self.inner.linear_workspace_cache
    }
    #[setter]
    fn set_linear_workspace_cache(&mut self, value: Option<bool>) {
        self.inner.linear_workspace_cache = value;
    }
    /// Fourier window used inside the background objective. Default: "Hanning".
    ///
    /// None selects Hanning. A window reduces artifacts from abrupt k truncation;
    /// its shape changes the objective and can change the extracted background.
    /// Accepted case-sensitive names are Hanning, Parzen, Welch, Gaussian,
    /// Sine, KaiserBessel and FHanning. See the
    /// [window reference](https://xraypy.github.io/xraylarch/xafs_fourier.html#ftwindow)
    /// for the shape-dependent parameter conventions.
    /// An unsupported name raises ValueError when assigned.
    #[getter]
    fn window(&self) -> Option<String> {
        Some(format!("{:?}", self.inner.window))
    }
    #[setter]
    fn set_window(&mut self, value: Option<String>) -> PyResult<()> {
        let parsed = match value.as_deref() {
            None => None,
            Some("Hanning") => Some(rexafs::prelude::FTWindow::Hanning),
            Some("Parzen") => Some(rexafs::prelude::FTWindow::Parzen),
            Some("Welch") => Some(rexafs::prelude::FTWindow::Welch),
            Some("Gaussian") => Some(rexafs::prelude::FTWindow::Gaussian),
            Some("Sine") => Some(rexafs::prelude::FTWindow::Sine),
            Some("KaiserBessel") => Some(rexafs::prelude::FTWindow::KaiserBessel),
            Some("FHanning") => Some(rexafs::prelude::FTWindow::FHanning),
            Some(value) => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown FTWindow: {value}"
                )))
            }
        };
        self.inner.window = parsed.unwrap_or_default();
        Ok(())
    }
    /// Solver used for the spline coefficients. Recommended default: "LinearDirect".
    ///
    /// None resolves to LinearDirect, which is required by FixedPenalty.
    /// TrustRegionDogLeg and LegacyLm are iterative solvers for legacy objectives;
    /// select Fixed or TwoPass explicitly to use them. TrustRegionDogLeg requires
    /// the trust-region Rust feature, included in Python packages. Unknown names
    /// raise ValueError; incompatible solver/objective pairs fail during processing.
    #[getter]
    fn solver(&self) -> Option<String> {
        self.inner.solver.map(|v| format!("{v:?}"))
    }
    #[setter]
    fn set_solver(&mut self, value: Option<String>) -> PyResult<()> {
        let parsed = match value.as_deref() {
            None => None,
            Some("TrustRegionDogLeg") => Some(rexafs::prelude::AUTOBKSolver::TrustRegionDogLeg),
            Some("LegacyLm") => Some(rexafs::prelude::AUTOBKSolver::LegacyLm),
            Some("LinearDirect") => Some(rexafs::prelude::AUTOBKSolver::LinearDirect),
            Some(value) => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown AUTOBKSolver: {value}"
                )))
            }
        };
        self.inner.solver = parsed;
        Ok(())
    }
    /// Solver used after a rejected legacy LinearDirect solve.
    ///
    /// The Python constructor default is "TrustRegionDogLeg". Explicit None
    /// normally resolves to "LegacyLm" when fallback is enabled; it is therefore
    /// different from leaving the constructor argument unchanged. LinearDirect
    /// cannot be its own fallback. FixedPenalty ignores this setting and never
    /// falls back. Unknown names raise ValueError when assigned.
    #[getter]
    fn linear_fallback_solver(&self) -> Option<String> {
        self.inner.linear_fallback_solver.map(|v| format!("{v:?}"))
    }
    #[setter]
    fn set_linear_fallback_solver(&mut self, value: Option<String>) -> PyResult<()> {
        let parsed = match value.as_deref() {
            None => None,
            Some("TrustRegionDogLeg") => Some(rexafs::prelude::AUTOBKSolver::TrustRegionDogLeg),
            Some("LegacyLm") => Some(rexafs::prelude::AUTOBKSolver::LegacyLm),
            Some("LinearDirect") => Some(rexafs::prelude::AUTOBKSolver::LinearDirect),
            Some(value) => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown AUTOBKSolver: {value}"
                )))
            }
        };
        self.inner.linear_fallback_solver = parsed;
        Ok(())
    }
    /// Endpoint model for background removal. Recommended default: "FixedPenalty".
    ///
    /// None resolves to FixedPenalty, which requires LinearDirect and uses
    /// clamp_lambda as a fixed mean-square penalty strength. Fixed and TwoPass
    /// retain older residual-dependent clamp models; their results need not
    /// match the recommended objective. See the [AUTOBK objective](https://rexafs.com/docs/science/autobk/) for
    /// the distinction. Unknown names raise ValueError when assigned.
    #[getter]
    fn clamp_scale_policy(&self) -> Option<String> {
        self.inner.clamp_scale_policy.map(|v| format!("{v:?}"))
    }
    #[setter]
    fn set_clamp_scale_policy(&mut self, value: Option<String>) -> PyResult<()> {
        let parsed = match value.as_deref() {
            None => None,
            Some("FixedPenalty") => Some(rexafs::prelude::AUTOBKClampScalePolicy::FixedPenalty),
            Some("Fixed") => Some(rexafs::prelude::AUTOBKClampScalePolicy::Fixed),
            Some("TwoPass") => Some(rexafs::prelude::AUTOBKClampScalePolicy::TwoPass),
            Some(value) => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown AUTOBKClampScalePolicy: {value}"
                )))
            }
        };
        self.inner.clamp_scale_policy = parsed;
        Ok(())
    }
}

/// Configure the weighted Fourier transform from photoelectron k to distance R.
///
/// The code forms g = chi * k**w * W, with integer w=kweight and real window
/// W, then computes chi_R = (kstep / sqrt(pi)) * rfft(g, n=nfft) using an
/// unnormalized, negative-exponent FFT. There is no additional 1/nfft factor,
/// window-area normalization or phase rotation. On a zero-origin uniform
/// grid, R_m = pi * m / (nfft * kstep), where m is the frequency-bin index.
/// For dimensionless chi, chi_R has units angstrom**(-(w + 1)); R is in
/// angstroms. Zero padding refines the R sampling but adds no measured data.
///
/// Recommended defaults are kmin=2, kmax=15 inverse angstroms, kweight=2,
/// window="KaiserBessel", dk=1, nfft=2048 and grid="Input". Automatic kstep
/// uses the background grid, usually 0.05 inverse angstroms. Settings are
/// copied into a spectrum, so reassign them after editing.
///
/// Example: p = XrayFFTF(); p.kmax = 12.0; spectrum.set_fft(p).fft().
/// Use spectrum.kwin_k() with kwin() to inspect the window. Scattering phase
/// shifts mean that an uncorrected Fourier peak is not directly a bond length.
///
/// Implementation convention: [NumPy's DFT definition](https://numpy.org/doc/stable/reference/routines.fft.html#implementation-details).
/// Physical interpretation: [Rehr and Albers (2000)](https://doi.org/10.1103/RevModPhys.72.621).
/// See [processing theory](https://rexafs.com/docs/science/processing/) for the
/// equation and links to the implementing Rust functions.
///
/// Resolved automatic k limits, k spacing and numeric defaults are retained
/// inside the spectrum. An unset window still selects Hanning when used.
/// Processing does not replace None fields in your original settings object.
/// Reassign fresh or reset settings when you want retained automatic choices
/// recalculated after changing the data or an earlier stage.
#[pyclass(name = "XrayFFTF", module = "rexafs", skip_from_py_object)]
#[derive(Clone)]
struct PyXrayFFTF {
    inner: rexafs::XrayFFTF,
}
#[pymethods]
impl PyXrayFFTF {
    /// Create forward-transform settings with the recommended Rust defaults.
    ///
    /// Start with k=2 to 15 inverse angstroms, kweight=2, a KaiserBessel window
    /// and nfft=2048. Choose a useful k range for your measured data, then call
    /// spectrum.set_fft(parameters).fft(). Construction does not run a transform;
    /// numeric validation occurs when fft() processes the data.
    ///
    /// Keyword arguments were added in 0.2.5. Published
    /// 0.2.4 settings use construction without arguments followed by field
    /// assignment. Python type conversion can raise TypeError, and an integer
    /// outside the native field's representable range can raise OverflowError
    /// before any numerical processing.
    #[new]
    #[pyo3(
        text_signature = "(*, grid='Input', rmax_out=10.0, dk=1.0, dk2=None, kmin=2.0, kmax=15.0, kweight=2.0, nfft=2048, kstep=None, window='KaiserBessel')"
    )]
    #[pyo3(signature = (*, grid="Input", rmax_out=Some(10.0), dk=Some(1.0), dk2=None, kmin=Some(2.0), kmax=Some(15.0), kweight=Some(2.0), nfft=Some(2048), kstep=None, window=Some("KaiserBessel")))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        grid: &str,
        rmax_out: Option<f64>,
        dk: Option<f64>,
        dk2: Option<f64>,
        kmin: Option<f64>,
        kmax: Option<f64>,
        kweight: Option<f64>,
        nfft: Option<usize>,
        kstep: Option<f64>,
        window: Option<&str>,
    ) -> PyResult<Self> {
        let mut result = Self {
            inner: rexafs::XrayFFTF::new(),
        };
        result.set_grid(grid)?;
        result.set_rmax_out(rmax_out);
        result.set_dk(dk);
        result.set_dk2(dk2);
        result.set_kmin(kmin);
        result.set_kmax(kmax);
        result.set_kweight(kweight);
        result.set_nfft(nfft);
        result.set_kstep(kstep);
        result.set_window(window.map(str::to_owned))?;
        Ok(result)
    }
    /// Sampling/window convention. Default: "Input".
    ///
    /// Input keeps the existing background samples and does not resample them
    /// when kstep changes. Larch linearly interpolates onto a zero-origin grid
    /// with the requested kstep and extends the window domain through the upper
    /// taper. Both preserve Spectrum.k()/chi(); kwin_k() returns the matching
    /// window axis. Unknown names raise ValueError when assigned.
    #[getter]
    fn grid(&self) -> String {
        format!("{:?}", self.inner.grid)
    }
    #[setter]
    fn set_grid(&mut self, value: &str) -> PyResult<()> {
        self.inner.grid = match value {
            "Input" => rexafs::FFTGrid::Input,
            "Larch" => rexafs::FFTGrid::Larch,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "FFT grid must be Input or Larch",
                ))
            }
        };
        Ok(())
    }
    /// Largest R shown by r() and chir_real/imag/mag(), in angstroms.
    ///
    /// Default: 10.0. None also resolves to 10.0. It must be finite and
    /// nonnegative. This limits returned display arrays; the complete internal
    /// forward transform is retained for inverse filtering. Increasing it does not
    /// improve spatial resolution or apply a structural shell filter.
    ///
    /// Keep at least two returned R samples if you plan to call ifft(): its
    /// grid validation uses r() even though filtering uses the full stored
    /// Fourier coefficients. For example, rmax_out=0 permits a forward result
    /// but makes a subsequent inverse fail with RuntimeError.
    #[getter]
    fn rmax_out(&self) -> Option<f64> {
        self.inner.rmax_out
    }
    #[setter]
    fn set_rmax_out(&mut self, value: Option<f64>) {
        self.inner.rmax_out = value;
    }
    /// Low-k window taper parameter. Default: 1.0. None also resolves to 1.0.
    ///
    /// For Hanning-like windows it describes a width in inverse angstroms.
    /// KaiserBessel also uses this numeric value to set the Bessel-function shape,
    /// so it is not a universally comparable taper width. Larger tapers generally
    /// soften truncation at the cost of a broader R response. Use kwin_k()/kwin()
    /// to inspect the actual window.
    ///
    /// FHanning uses a fractional taper parameter. For Gaussian, dk is the
    /// standard-deviation scale in inverse angstroms and the window has tails
    /// beyond the nominal bounds. It is not a low-end-only width for those
    /// families. See the [window reference](https://xraypy.github.io/xraylarch/xafs_fourier.html#ftwindow)
    /// for their distinct conventions.
    #[getter]
    fn dk(&self) -> Option<f64> {
        self.inner.dk
    }
    #[setter]
    fn set_dk(&mut self, value: Option<f64>) {
        self.inner.dk = value;
    }
    /// High-k window taper parameter. Default None uses dk.
    ///
    /// It controls the upper-end geometry in inverse angstroms for the
    /// width-based windows. Set it separately for an asymmetric taper.
    /// Window families interpret taper parameters differently; KaiserBessel's
    /// Bessel-function shape is controlled by dk, not an independent dk2 shape.
    ///
    /// For Gaussian, dk2 affects the window domain and center but is not a
    /// second standard deviation. FHanning interprets it as a fractional taper
    /// parameter. See the [window reference](https://xraypy.github.io/xraylarch/xafs_fourier.html#ftwindow)
    /// before comparing settings between families.
    #[getter]
    fn dk2(&self) -> Option<f64> {
        self.inner.dk2
    }
    #[setter]
    fn set_dk2(&mut self, value: Option<f64>) {
        self.inner.dk2 = value;
    }
    /// Lower Fourier-window limit, in inverse angstroms. Constructor default: 2.0.
    ///
    /// Explicit None uses the first background k sample, usually zero. Raising
    /// the limit suppresses low-k contributions but shortens the effective
    /// transform range. The implementation requires finite bounds with kmin
    /// below kmax; negative lower bounds are accepted, but the window is clipped
    /// to its sampled domain. Use a nonnegative bound for the physical k range.
    /// The taper can extend below the nominal limit.
    #[getter]
    fn kmin(&self) -> Option<f64> {
        self.inner.kmin
    }
    #[setter]
    fn set_kmin(&mut self, value: Option<f64>) {
        self.inner.kmin = value;
    }
    /// Upper Fourier-window limit, in inverse angstroms. Constructor default: 15.0.
    ///
    /// Explicit None uses the last background k sample. Lower it to exclude
    /// a noisy high-k tail; this changes the transform without recomputing chi.
    /// The taper can extend above the nominal bound. Input and Larch use
    /// different window domains when the taper reaches beyond measured data.
    #[getter]
    fn kmax(&self) -> Option<f64> {
        self.inner.kmax
    }
    #[setter]
    fn set_kmax(&mut self, value: Option<f64>) {
        self.inner.kmax = value;
    }
    /// Exponent applied as k**w before the transform. Default: 2.0. None also
    /// resolves to 2.0.
    ///
    /// Finite nonnegative values are floored to an integer w. A higher weight
    /// emphasizes high-k oscillations and noise. It changes both amplitude and
    /// units: dimensionless chi gives chi(R) in angstrom**(-(w + 1)). The
    /// background chi() remains unweighted.
    #[getter]
    fn kweight(&self) -> Option<f64> {
        self.inner.kweight
    }
    #[setter]
    fn set_kweight(&mut self, value: Option<f64>) {
        self.inner.kweight = value;
    }
    /// Total FFT length, including zero padding. Default: 2048. None also resolves
    /// to 2048.
    ///
    /// It must be at least 2. Choose a length that contains the prepared data:
    /// Input keeps only the first nfft samples if the data are longer; Larch
    /// requires its extended window grid to fit. Increasing nfft gives a finer R
    /// grid with spacing pi / (nfft * kstep), but does not add structural
    /// information or introduce an extra amplitude normalization.
    #[getter]
    fn nfft(&self) -> Option<usize> {
        self.inner.nfft
    }
    #[setter]
    fn set_nfft(&mut self, value: Option<usize>) {
        self.inner.nfft = value;
    }
    /// k spacing used for FFT scaling and the R axis, in inverse angstroms.
    ///
    /// Default None uses the difference between the first two input k values;
    /// the default AUTOBK grid gives 0.05. An explicit value must be finite and
    /// positive. Input does not resample, so keep this equal to its actual grid
    /// spacing. Use grid="Larch" when requesting resampling at a different step.
    /// The forward amplitude multiplier is kstep / sqrt(pi).
    ///
    /// Once resolved, the spectrum retains this spacing on later fft() calls.
    /// Changing the background k grid does not automatically reset it. Reassign
    /// an XrayFFTF with kstep=None to infer the new spacing; the original
    /// settings object remains unchanged by processing.
    #[getter]
    fn kstep(&self) -> Option<f64> {
        self.inner.kstep
    }
    #[setter]
    fn set_kstep(&mut self, value: Option<f64>) {
        self.inner.kstep = value;
    }
    /// Forward Fourier-window shape. Constructor default: "KaiserBessel".
    ///
    /// Explicit None selects Hanning, which differs from leaving the default
    /// unchanged. The window reduces truncation ringing and broadens the R
    /// response; it is not normalized by its area. Accepted case-sensitive names are Hanning, Parzen, Welch, Gaussian,
    /// Sine, KaiserBessel and FHanning. See the
    /// [window reference](https://xraypy.github.io/xraylarch/xafs_fourier.html#ftwindow)
    /// for the shape-dependent parameter conventions. Unknown names raise ValueError when assigned.
    #[getter]
    fn window(&self) -> Option<String> {
        self.inner.window.map(|v| format!("{v:?}"))
    }
    #[setter]
    fn set_window(&mut self, value: Option<String>) -> PyResult<()> {
        let parsed = match value.as_deref() {
            None => None,
            Some("Hanning") => Some(rexafs::prelude::FTWindow::Hanning),
            Some("Parzen") => Some(rexafs::prelude::FTWindow::Parzen),
            Some("Welch") => Some(rexafs::prelude::FTWindow::Welch),
            Some("Gaussian") => Some(rexafs::prelude::FTWindow::Gaussian),
            Some("Sine") => Some(rexafs::prelude::FTWindow::Sine),
            Some("KaiserBessel") => Some(rexafs::prelude::FTWindow::KaiserBessel),
            Some("FHanning") => Some(rexafs::prelude::FTWindow::FHanning),
            Some(value) => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown FTWindow: {value}"
                )))
            }
        };
        self.inner.window = parsed;
        Ok(())
    }
}

/// Configure a real inverse Fourier transform with an R-space window.
///
/// The code weights positive-R bins by R**rweight and a real window, then
/// reconstructs their conjugate negative-frequency partners to obtain a real
/// signal on q. q has the same physical meaning as k, in inverse angstroms;
/// the different name identifies a back-transformed, potentially filtered
/// signal. The forward k weighting and window remain in that signal.
/// This real inverse differs from Larch's complex, one-sided back-transform.
///
/// Defaults are rmin=0, rmax=20 angstroms, rweight=0, a KaiserBessel window,
/// dr=1 angstrom, qmax_out=10 inverse angstroms, nfft=2048, and automatic
/// kstep. Choose an R interval appropriate to your shells for deliberate
/// filtering. The uncorrected R axis includes scattering phase shifts.
///
/// Example: p = XrayFFTR(); p.rmin = 1.0; p.rmax = 3.0;
/// spectrum.set_ifft(p).ifft(). Settings are copied; reassign after edits.
/// These Python settings and set_ifft() were added in 0.2.5.
///
/// See the [implemented inverse convention](https://rexafs.com/docs/science/processing/)
/// for the scaling, and [Larch's Fourier guide](https://xraypy.github.io/xraylarch/xafs_fourier.html)
/// for windowing concepts rather than an assertion of identical inverse output.
///
/// Resolved automatic R limits, q spacing and numeric defaults are retained
/// inside the spectrum. Unset dr2 and window continue to select dr and
/// Hanning when the window is calculated.
/// Processing does not replace None fields in your original settings object.
/// Reassign fresh or reset settings when you want retained automatic choices
/// recalculated after changing the data or an earlier stage.
#[pyclass(name = "XrayFFTR", module = "rexafs", skip_from_py_object)]
#[derive(Clone)]
struct PyXrayFFTR {
    inner: rexafs::XrayFFTR,
}
#[pymethods]
impl PyXrayFFTR {
    /// Create inverse-transform settings with the recommended Rust defaults.
    ///
    /// Set rmin/rmax to select an R region and use rweight=0 unless extra R
    /// weighting is intended. Assign with spectrum.set_ifft(parameters).ifft().
    /// The returned signal retains forward weighting and windowing; construction
    /// alone does not filter a spectrum.
    ///
    /// This settings class was added in 0.2.5; it is
    /// not exported by the published 0.2.4 package. Python type conversion can raise TypeError, and an integer
    /// outside the native field's representable range can raise OverflowError
    /// before any numerical processing.
    #[new]
    #[pyo3(
        text_signature = "(*, qmax_out=10.0, dr=1.0, dr2=None, rmin=0.0, rmax=20.0, rweight=0.0, nfft=2048, kstep=None, window='KaiserBessel')"
    )]
    #[pyo3(signature = (*, qmax_out=Some(10.0), dr=Some(1.0), dr2=None, rmin=Some(0.0), rmax=Some(20.0), rweight=Some(0.0), nfft=Some(2048), kstep=None, window=Some("KaiserBessel")))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        qmax_out: Option<f64>,
        dr: Option<f64>,
        dr2: Option<f64>,
        rmin: Option<f64>,
        rmax: Option<f64>,
        rweight: Option<f64>,
        nfft: Option<usize>,
        kstep: Option<f64>,
        window: Option<&str>,
    ) -> PyResult<Self> {
        let mut result = Self {
            inner: rexafs::XrayFFTR::new(),
        };
        result.set_qmax_out(qmax_out);
        result.set_dr(dr);
        result.set_dr2(dr2);
        result.set_rmin(rmin);
        result.set_rmax(rmax);
        result.set_rweight(rweight);
        result.set_nfft(nfft);
        result.set_kstep(kstep);
        result.set_window(window.map(str::to_owned))?;
        Ok(result)
    }
    /// Largest returned q value, in inverse angstroms. Default: 10.0. None also
    /// resolves to 10.0.
    ///
    /// The q()/chiq() arrays stop at this bound or the available inverse samples,
    /// whichever comes first. It must be finite and nonnegative. This limits
    /// returned samples and does not change the R-space filter.
    #[getter]
    fn qmax_out(&self) -> Option<f64> {
        self.inner.qmax_out
    }
    #[setter]
    fn set_qmax_out(&mut self, value: Option<f64>) {
        self.inner.qmax_out = value;
    }
    /// Low-R window taper parameter. Default: 1.0. None also resolves to 1.0.
    ///
    /// For width-based windows the unit is angstroms. KaiserBessel also uses this
    /// numeric value as its Bessel-function shape parameter. A wider taper smooths
    /// the selected R boundary but mixes a broader range of distances into the
    /// filtered signal.
    ///
    /// FHanning instead uses a fractional taper parameter. Gaussian uses dr
    /// as its standard-deviation scale in angstroms and has nonzero tails
    /// beyond the nominal R interval.
    #[getter]
    fn dr(&self) -> Option<f64> {
        self.inner.dr
    }
    #[setter]
    fn set_dr(&mut self, value: Option<f64>) {
        self.inner.dr = value;
    }
    /// High-R window taper parameter. Default None uses dr.
    ///
    /// Use a separate value for asymmetric R-window geometry. The unit is
    /// angstroms for width-based windows; KaiserBessel's Bessel-function shape
    /// uses dr, so dr2 does not define an independent high-end shape.
    ///
    /// Gaussian uses dr for its standard deviation; dr2 affects the domain
    /// and center rather than providing a second Gaussian width. FHanning
    /// uses a fractional taper parameter.
    #[getter]
    fn dr2(&self) -> Option<f64> {
        self.inner.dr2
    }
    #[setter]
    fn set_dr2(&mut self, value: Option<f64>) {
        self.inner.dr2 = value;
    }
    /// Lower R-window limit, in angstroms. Constructor default: 0.0.
    ///
    /// Explicit None uses the first input R sample, which is zero. A larger
    /// value removes lower-R contributions from the back-transform. The bound
    /// must be finite, nonnegative and below rmax; the taper can extend below it.
    /// R peaks have not been corrected for scattering phase shifts.
    #[getter]
    fn rmin(&self) -> Option<f64> {
        self.inner.rmin
    }
    #[setter]
    fn set_rmin(&mut self, value: Option<f64>) {
        self.inner.rmin = value;
    }
    /// Upper R-window limit, in angstroms. Constructor default: 20.0.
    ///
    /// Choose a shell range such as 1 to 3 angstroms only when appropriate for
    /// your data. Explicit None uses the last reported input R sample, which
    /// depends on forward rmax_out; an explicit bound instead acts on the full
    /// internal Fourier bins. The taper can extend above the nominal bound.
    #[getter]
    fn rmax(&self) -> Option<f64> {
        self.inner.rmax
    }
    #[setter]
    fn set_rmax(&mut self, value: Option<f64>) {
        self.inner.rmax = value;
    }
    /// Exponent applied as R**v before the inverse transform. Default: 0.0.
    ///
    /// None resolves to 0.0; finite nonnegative values are floored to an integer
    /// v. Leave it at zero for ordinary R filtering. A positive value emphasizes
    /// higher-R contributions and changes the units of chiq(): with forward
    /// kweight w the units are angstrom**(v - w).
    #[getter]
    fn rweight(&self) -> Option<f64> {
        self.inner.rweight
    }
    #[setter]
    fn set_rweight(&mut self, value: Option<f64>) {
        self.inner.rweight = value;
    }
    /// Inverse transform length. Default: 2048. None also resolves to 2048;
    /// minimum: 2.
    ///
    /// Leave kstep automatic when changing this value. With fixed input R spacing,
    /// a larger nfft gives a finer q grid by zero padding Fourier bins; a smaller
    /// value discards bins beyond its representable range. Neither operation adds
    /// experimental information.
    #[getter]
    fn nfft(&self) -> Option<usize> {
        self.inner.nfft
    }
    #[setter]
    fn set_nfft(&mut self, value: Option<usize>) {
        self.inner.nfft = value;
    }
    /// Output q spacing, in inverse angstroms.
    ///
    /// Default None uses pi / (nfft * delta_R), where delta_R is the difference
    /// between the first two R samples in angstroms. An explicit positive value
    /// must agree with that spacing or processing raises RuntimeError. Keep it
    /// automatic when changing nfft so the physical Fourier grid stays consistent.
    ///
    /// Automatic spacing is retained inside the spectrum after the first
    /// inverse. If the forward R grid changes, assign fresh inverse settings
    /// with kstep=None before calling ifft() again. The earlier resolved value
    /// otherwise remains subject to the same consistency check.
    #[getter]
    fn kstep(&self) -> Option<f64> {
        self.inner.kstep
    }
    #[setter]
    fn set_kstep(&mut self, value: Option<f64>) {
        self.inner.kstep = value;
    }
    /// R-space Fourier-window shape. Constructor default: "KaiserBessel".
    ///
    /// Explicit None selects Hanning, unlike leaving the default unchanged.
    /// The window selects and tapers R contributions before the real inverse;
    /// it cannot undo the weighting or information lost in the forward window.
    /// Accepted case-sensitive names are Hanning, Parzen, Welch, Gaussian,
    /// Sine, KaiserBessel and FHanning. See the
    /// [window reference](https://xraypy.github.io/xraylarch/xafs_fourier.html#ftwindow)
    /// for the shape-dependent parameter conventions. Unknown names raise ValueError when assigned.
    #[getter]
    fn window(&self) -> Option<String> {
        self.inner.window.map(|v| format!("{v:?}"))
    }
    #[setter]
    fn set_window(&mut self, value: Option<String>) -> PyResult<()> {
        let parsed = match value.as_deref() {
            None => None,
            Some("Hanning") => Some(rexafs::prelude::FTWindow::Hanning),
            Some("Parzen") => Some(rexafs::prelude::FTWindow::Parzen),
            Some("Welch") => Some(rexafs::prelude::FTWindow::Welch),
            Some("Gaussian") => Some(rexafs::prelude::FTWindow::Gaussian),
            Some("Sine") => Some(rexafs::prelude::FTWindow::Sine),
            Some("KaiserBessel") => Some(rexafs::prelude::FTWindow::KaiserBessel),
            Some("FHanning") => Some(rexafs::prelude::FTWindow::FHanning),
            Some(value) => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown FTWindow: {value}"
                )))
            }
        };
        self.inner.window = parsed;
        Ok(())
    }
}

/// Select the normalization algorithm and own a copy of its settings.
///
/// Use NormalizationMethod.PrePostEdge(parameters) for configured pre/post-edge
/// normalization or new_prepostedge() for automatic settings, then pass the
/// result to Spectrum.set_normalization_method(). Creating a method does not
/// process data. The no-argument MBack selector has no absorber/edge and cannot
/// normalize. The MBACK algorithm was unimplemented through version 0.2.9.
#[pyclass(name = "NormalizationMethod", module = "rexafs", skip_from_py_object)]
#[derive(Clone)]
struct PyNormalizationMethod {
    inner: rexafs::NormalizationMethod,
}
#[pymethods]
impl PyNormalizationMethod {
    /// Copy PrePostEdge parameters into a normalization method.
    ///
    /// Assign the result with spectrum.set_normalization_method(method), then
    /// call normalize() or a later stage. The original parameters remain
    /// independent: edits do not change an already created method or spectrum.
    #[staticmethod]
    #[pyo3(name = "PrePostEdge")]
    fn configured(parameters: &PyPrePostEdge) -> Self {
        Self {
            inner: rexafs::NormalizationMethod::PrePostEdge(parameters.inner.clone()),
        }
    }
    /// Create a normalization method with automatic pre/post-edge settings.
    ///
    /// E0, fitting ranges, polynomial degree and edge step are inferred when
    /// processing runs. Equivalent to NormalizationMethod.PrePostEdge(PrePostEdge());
    /// assign the method to a spectrum to use it. No data are processed here.
    #[staticmethod]
    fn new_prepostedge() -> Self {
        Self {
            inner: rexafs::NormalizationMethod::new_prepostedge(),
        }
    }
    /// Create the historical empty MBack normalization selector.
    ///
    /// Selecting it preserves the requested algorithm, but normalize() and
    /// dependent stages raise ValueError rather than substitute another method.
    /// Use new_prepostedge() for automatic polynomial normalization. Through
    /// version 0.2.9 the MBACK algorithm was unimplemented; this historical
    /// no-argument selector remains unusable for normalization.
    #[staticmethod]
    fn new_mback() -> Self {
        Self {
            inner: rexafs::NormalizationMethod::new_mback(),
        }
    }
}

/// Select a background algorithm and own a copy of its settings.
///
/// Use BackgroundMethod.AUTOBK(parameters) to configure the spline background
/// or new_autobk() for the recommended defaults, then pass the result to
/// Spectrum.set_background_method(). Creating a method does not process
/// data. ILPBkg is a named placeholder and is not implemented.
#[pyclass(name = "BackgroundMethod", module = "rexafs", skip_from_py_object)]
#[derive(Clone)]
struct PyBackgroundMethod {
    inner: rexafs::BackgroundMethod,
}
#[pymethods]
impl PyBackgroundMethod {
    /// Copy AUTOBK parameters into a background method.
    ///
    /// Assign the result with spectrum.set_background_method(method), then call
    /// calc_background() or a later stage. Edits to the original parameters do
    /// not change an already created method or spectrum.
    #[staticmethod]
    #[pyo3(name = "AUTOBK")]
    fn configured(parameters: &PyAUTOBK) -> Self {
        Self {
            inner: rexafs::BackgroundMethod::AUTOBK(parameters.inner.clone()),
        }
    }
    /// Create a background method using the recommended AUTOBK defaults.
    ///
    /// This selects rbkg=1.0 angstrom, LinearDirect and FixedPenalty with
    /// clamp_lambda=0.001. Assign it to a spectrum before processing; no
    /// background is fitted by this factory.
    #[staticmethod]
    fn new_autobk() -> Self {
        Self {
            inner: rexafs::BackgroundMethod::new_autobk(),
        }
    }
    /// Create the unimplemented ILPBkg background placeholder.
    ///
    /// Selecting it preserves the requested algorithm, but calc_background()
    /// and dependent stages raise RuntimeError rather than substitute AUTOBK.
    /// Use new_autobk() for the implemented background workflow.
    #[staticmethod]
    fn new_ilpbkg() -> Self {
        Self {
            inner: rexafs::BackgroundMethod::new_ilpbkg(),
        }
    }
}

/// Own a measured absorption spectrum and its calculated processing stages.
///
/// Energy is in eV; absorption mu may be an absorption coefficient or a
/// consistently scaled measurement such as transmission optical depth.
/// Input arrays are copied to Rust-owned float64 storage. Returned NumPy
/// arrays are independent copies, so editing them does not change this
/// spectrum. Each result getter returns None until its stage has succeeded.
///
/// Start with Spectrum(energy, mu).fft() to run automatic normalization,
/// AUTOBK background removal and the k-to-R transform. Explicit processing
/// calls recompute their stage and return this same object; missing
/// prerequisites run automatically with the selected settings. Calculation
/// methods release Python's global interpreter lock while Rust runs.
///
/// Changing input data or settings clears dependent results. A failed stage
/// does not substitute another algorithm. Invalid input and normalization
/// errors raise ValueError; background and Fourier errors raise RuntimeError.
/// See [processing theory](https://rexafs.com/docs/science/processing/) for
/// equations, interpretation and limitations. Groups and structural fitting
/// are not currently exposed by this Python Spectrum API.
#[pyclass(name = "Spectrum", module = "rexafs", skip_from_py_object)]
struct PySpectrum {
    inner: rexafs::Spectrum,
}
#[pymethods]
impl PySpectrum {
    /// Correct into an independent unnormalized Spectrum (since 0.2.10). Internal
    /// conventional normalization runs automatically; the source stays unchanged.
    /// Unknown acquisition provenance is explicitly interpreted as fluorescence.
    /// Known transmission, prepared norm/flat and repeated correction raise ValueError.
    /// Select a line and measured surface angles in FluorescenceCorrection. Releases
    /// the GIL. Call normalize() for separate final polynomial/MBACK normalization.
    /// History survives edits; this XANES-only branch rejects background/FFT/wavelets.
    fn correct_fluorescence(
        &self,
        py: Python<'_>,
        model: &fluorescence::PyFluorescenceCorrection,
    ) -> PyResult<Self> {
        let inner = py
            .detach(|| self.inner.correct_fluorescence(&model.inner))
            .map_err(fluorescence::invalid)?;
        Ok(Self { inner })
    }
    /// Owned historical correction record, or None. Later edits/normalization do
    /// not change its original inputs or remove its XANES-only processing restriction.
    fn fluorescence_correction(&self) -> Option<fluorescence::PyFluorescenceCorrectionResult> {
        self.inner
            .fluorescence_correction()
            .cloned()
            .map(|inner| fluorescence::PyFluorescenceCorrectionResult { inner })
    }
    /// Acquisition interpretation: unknown, transmission or fluorescence. Unknown
    /// means missing evidence, not an automatically recognized fluorescence signal.
    fn absorption_mode(&self) -> &'static str {
        fluorescence::mode_name(self.inner.absorption_mode())
    }
    /// Explicitly revise acquisition interpretation and return this Spectrum.
    /// Arrays/caches are unchanged. A correction record and its restrictions survive.
    fn set_absorption_mode<'py>(
        mut slf: PyRefMut<'py, Self>,
        mode: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.set_absorption_mode(fluorescence::mode(mode)?);
        Ok(slf)
    }
    /// Calculate a Cauchy wavelet map on a private copy (since 0.2.10).
    /// Use spectrum.wavelet(Wavelet((2, 12))). The k interval is in inverse angstroms.
    /// Missing normalization/AUTOBK run automatically; existing chi is reused.
    /// Inputs, settings and cached results remain unchanged. Releases the GIL.
    /// Returns an owned WaveletMap with copied NumPy arrays (rows=R, columns=k).
    /// Incomplete support, invalid grids and unqualified corrected XANES inputs
    /// raise ValueError. R is not phase-corrected; colors do not imply concentration.
    fn wavelet(
        &self,
        py: Python<'_>,
        model: &wavelet::PyWavelet,
    ) -> PyResult<wavelet::PyWaveletMap> {
        let inner = py
            .detach(|| self.inner.wavelet(&model.inner))
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(wavelet::PyWaveletMap { inner })
    }
    /// Fit a composite XANES model, preparing missing normalization on a private copy.
    ///
    /// Available since 0.2.10. Example: spectrum.fit_peaks(PeakFit((-20, 40)).gaussian("p1", 5, 2, 3)).
    /// Defaults come from the model: Norm, E0-relative eV, 200 iterations. The source
    /// arrays, settings, caches and initial model remain unchanged. Returns an owned
    /// PeakFitResult with data/model/residual arrays on retained native points.
    /// Inspect termination and warnings; a returned result can be nonconverged.
    /// Invalid definitions, insufficient coverage or failed preparation raise ValueError.
    /// Rust calculation releases the GIL. No smoothing or interpolation occurs.
    ///
    /// Optional errors contain positive independent standard deviations in the selected
    /// signal representation, one per ORIGINAL native point, including excluded points.
    /// Raw detector errors are not propagated through normalization. Without errors,
    /// covariance uses residual-based variance; it is withheld at active bounds,
    /// deficient rank or nonconvergence. These are conditional local uncertainties.
    #[pyo3(signature=(model, *, errors=None))]
    fn fit_peaks(
        &self,
        py: Python<'_>,
        model: &peaks::PyPeakFit,
        errors: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<peaks::PyPeakFitResult> {
        let errors = errors.map(|e| metrics::errors(py, e)).transpose()?;
        let inner = py
            .detach(|| model.inner.fit_with_errors(&self.inner, errors.as_deref()))
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(peaks::PyPeakFitResult { inner })
    }
    /// Measure a point or region, preparing missing stages on a private copy.
    ///
    /// Available since 0.2.10. Recommended: spectrum.measure("mean", (-20, 30)). Defaults
    /// to normalized mu and E0-relative energy offsets in eV; select space="flat"
    /// explicitly. point accepts a scalar; mean/integral/maximum accept two
    /// increasing bounds. k (inverse angstroms) and R (angstroms, not phase
    /// corrected) default to absolute coordinates. kweight defaults to zero and
    /// applies only to chi. No extrapolation or display sampling occurs.
    ///
    /// Returns an owned MeasurementResult. Mean is integral divided by interval
    /// width, not the arithmetic sample mean. Integral uses piecewise-linear
    /// trapezoids with interpolated boundaries. Only required preparation runs;
    /// arrays, settings and cached results on this spectrum remain unchanged.
    /// Rust calculation releases the GIL. Invalid inputs, unavailable preparation
    /// and missing coverage raise ValueError.
    ///
    /// Optional errors are independent standard deviations of the SELECTED
    /// representation on its native grid, in its signal units. They must already
    /// describe that processed signal: raw-count errors are not propagated through
    /// normalization or Fourier transforms. Point/mean/integral support them;
    /// maximum rejects them. Axis, E0 and settings are treated as exact. No
    /// correlations, confidence intervals or errors are inferred when omitted.
    #[pyo3(signature = (operation, coordinates, *, space="norm", origin=None, kweight=0, errors=None))]
    // Keep the public keyword-only scientific options directly visible to Python editors.
    #[allow(clippy::too_many_arguments)]
    fn measure(
        &self,
        py: Python<'_>,
        operation: &str,
        coordinates: &Bound<'_, PyAny>,
        space: &str,
        origin: Option<&str>,
        kweight: u8,
        errors: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<metrics::PyMeasurementResult> {
        let definition = metrics::definition(operation, coordinates, space, origin, kweight)?;
        let errors = errors.map(|e| metrics::errors(py, e)).transpose()?;
        let result = py
            .detach(|| match errors {
                Some(errors) => self.inner.measure_with_errors(&definition, &errors),
                None => self.inner.measure(&definition),
            })
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(metrics::PyMeasurementResult { inner: result })
    }
    /// Create a spectrum by copying energy and mu into float64 storage.
    ///
    /// energy contains X-ray energies in eV; mu contains the corresponding
    /// absorption values in consistent units. Lists, NumPy arrays and strided
    /// views are accepted. Inputs must be one-dimensional, finite, equal-length,
    /// have at least two samples, and have strictly increasing energy.
    /// Invalid shapes or data raise ValueError. No processing runs here;
    /// call fft() for the default pipeline or normalize() for just normalization.
    ///
    /// The two-sample minimum only permits storage. Automatic edge detection
    /// requires at least three samples, and baseline/spline fitting needs enough
    /// points on the appropriate sides of the edge. Supply real numeric input;
    /// NumPy conversion errors for unsupported objects propagate to the caller.
    #[new]
    fn new(py: Python<'_>, energy: &Bound<'_, PyAny>, mu: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: arrays(py, energy, mu)?,
        })
    }
    /// Create a spectrum from energy in eV and corresponding absorption mu.
    ///
    /// Equivalent to Spectrum(energy, mu): it accepts array-like inputs and
    /// copies them to owned float64 storage. Inputs must be finite, one-dimensional,
    /// equal-length, with at least two samples and strictly increasing energy.
    /// Invalid data raise ValueError. Derived results are initially unavailable.
    ///
    /// As with the constructor, two samples are enough to create the object
    /// but not enough for automatic edge detection or a useful EXAFS pipeline.
    /// Array conversion follows NumPy's float64 conversion rules.
    #[staticmethod]
    fn from_arrays(
        py: Python<'_>,
        energy: &Bound<'_, PyAny>,
        mu: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Self::new(py, energy, mu)
    }
    /// Replace measured energy/mu, copy the inputs and clear E0 and derived results.
    ///
    /// Input units and validation match the constructor: energy in eV, finite
    /// matching one-dimensional arrays with strictly increasing energy and at
    /// least two samples. Invalid input raises ValueError before replacing data.
    /// Stage settings are retained, but old calculated edge values and arrays
    /// are discarded. Returns this spectrum; call a processing stage to recompute.
    ///
    /// Unlike the QAS reader, this method rejects unordered input rather than
    /// sorting it. Previously resolved automatic fit ranges and FFT spacings
    /// are retained with the other settings. Reassign automatic settings if
    /// the new scan needs those choices inferred again.
    fn set_spectrum<'py>(
        mut slf: PyRefMut<'py, Self>,
        energy: &Bound<'py, PyAny>,
        mu: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let input = arrays(slf.py(), energy, mu)?;
        slf.inner
            .set_spectrum(input.energy.unwrap(), input.mu.unwrap());
        Ok(slf)
    }
    /// Set the absorption edge energy in eV and clear all dependent results.
    ///
    /// The value is propagated to configured normalization and AUTOBK settings,
    /// so subsequent energy-to-k conversion uses the new edge. Use a finite
    /// value strictly inside the measured energy range; validation occurs when
    /// processing runs, not at this setter. Returns this spectrum without
    /// performing normalization or recalibrating the input energy axis.
    fn set_e0(mut slf: PyRefMut<'_, Self>, e0: f64) -> PyRefMut<'_, Self> {
        slf.inner.set_e0(e0);
        slf
    }
    /// Return the detected or assigned edge energy E0 in eV, or None if unset.
    ///
    /// This getter does not run edge detection. Call find_e0(), normalize() or
    /// a later processing stage to resolve an automatic edge. E0 sets the
    /// energy origin for k conversion; it is not an independent calibration
    /// measurement or a fitted structural energy shift.
    fn e0(&self) -> Option<f64> {
        self.inner.e0()
    }
    /// Clear normalization, background and Fourier results, keeping stage settings.
    ///
    /// The measured inputs and spectrum E0 are retained, as are explicit
    /// settings needed for recomputation. Getters for cleared arrays return
    /// None until their stages run again. Ordinary setters already invalidate
    /// the affected results; use this method when you need a full recomputation.
    /// Returns this spectrum without running any calculations.
    ///
    /// Resolved fit ranges and FFT spacings are retained along with explicit
    /// parameters. They are not restored to their original None values; reassign
    /// fresh settings if you want automatic ranges or spacings inferred again.
    fn invalidate_derived(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.inner.invalidate_derived();
        slf
    }
    /// Copy inverse settings and clear q()/chiq(), preserving forward results.
    ///
    /// Use XrayFFTR to choose the R window and output q range. Editing the
    /// original settings later does not change this spectrum; assign again
    /// to apply changes. Returns this spectrum without filtering. Call ifft()
    /// to compute the new result. This binding was added in 0.2.5.
    fn set_ifft<'py>(mut slf: PyRefMut<'py, Self>, parameters: &PyXrayFFTR) -> PyRefMut<'py, Self> {
        slf.inner.set_ifft(parameters.inner.clone());
        slf
    }
    /// Copy forward settings and clear Fourier and inverse results.
    ///
    /// Normalization and the background k()/chi() arrays are retained. Use
    /// XrayFFTF to choose k weights, the window and grid convention. Editing
    /// the original settings later has no effect until you assign them again.
    /// Returns this spectrum without transforming; call fft() to recompute.
    ///
    /// If ifft() has already resolved its automatic kstep and this change
    /// alters the R-grid spacing, reassign inverse settings with kstep=None
    /// before the next inverse. This setter clears results, not the resolved
    /// parameters of the inverse stage.
    fn set_fft<'py>(mut slf: PyRefMut<'py, Self>, parameters: &PyXrayFFTF) -> PyRefMut<'py, Self> {
        slf.inner.set_fft(parameters.inner.clone());
        slf
    }
    /// Copy the selected normalization method and clear normalization and later results.
    ///
    /// Since 0.2.5, PrePostEdge settings are accepted directly; omitted/None restores
    /// automatic normalization. Published 0.2.4 uses
    /// NormalizationMethod.PrePostEdge(parameters) or new_prepostedge().
    /// An explicit E0 in those parameters becomes the spectrum E0; otherwise
    /// an existing spectrum E0 is retained. Edits to the original settings do
    /// not propagate: assign again to apply them. Returns this spectrum without
    /// processing data; call normalize() or a later stage to recompute.
    #[pyo3(signature = (method=None))]
    fn set_normalization_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        method: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let selected = match method {
            None => None,
            Some(value) => {
                if let Ok(parameters) = value.extract::<PyRef<'_, PyPrePostEdge>>() {
                    Some(rexafs::NormalizationMethod::PrePostEdge(
                        parameters.inner.clone(),
                    ))
                } else if let Ok(parameters) = value.extract::<PyRef<'_, mback::PyMBack>>() {
                    Some(rexafs::NormalizationMethod::MBack(parameters.inner.clone()))
                } else {
                    Some(
                        value
                            .extract::<PyRef<'_, PyNormalizationMethod>>()?
                            .inner
                            .clone(),
                    )
                }
            }
        };
        slf.inner
            .set_normalization_method(selected)
            .map_err(error)?;
        Ok(slf)
    }
    /// Copy the selected background method and clear background and Fourier results.
    ///
    /// Since 0.2.5, AUTOBK settings are accepted directly; omitted/None restores
    /// default AUTOBK. Published 0.2.4 uses BackgroundMethod.AUTOBK(parameters)
    /// or BackgroundMethod.new_autobk() for recommended defaults. Existing
    /// normalization results are retained. Later edits to the original settings
    /// do not propagate: assign them again to apply changes. Returns this
    /// spectrum without fitting; call calc_background() or a later stage.
    ///
    /// If changing kstep after fft() has already run, also reassign XrayFFTF
    /// settings with kstep=None. Clearing the Fourier results does not reset
    /// its previously resolved automatic spacing.
    #[pyo3(signature = (method=None))]
    fn set_background_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        method: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let selected = match method {
            None => None,
            Some(value) => {
                if let Ok(parameters) = value.extract::<PyRef<'_, PyAUTOBK>>() {
                    Some(rexafs::BackgroundMethod::AUTOBK(parameters.inner.clone()))
                } else {
                    Some(
                        value
                            .extract::<PyRef<'_, PyBackgroundMethod>>()?
                            .inner
                            .clone(),
                    )
                }
            }
        };
        slf.inner.set_background_method(selected).map_err(error)?;
        Ok(slf)
    }
    /// Estimate the absorption edge from the energy derivative of mu.
    ///
    /// The detector seeks a strong rise with neighboring high-derivative
    /// samples and refines its search around the candidate edge. It returns
    /// this spectrum, stores E0 in eV and clears normalization and all later
    /// results. Inspect the result for noisy spectra or multiple edges;
    /// automatic detection is not energy calibration. Invalid data raise ValueError.
    ///
    /// At least three energy/mu samples are required, even though the
    /// constructor can store two. An insufficient scan raises ValueError.
    fn find_e0(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        let py = slf.py();
        let inner = &mut slf.inner;
        py.detach(|| inner.find_e0().map(|_| ())).map_err(error)?;
        Ok(slf)
    }
    /// Fit pre/post-edge baselines and compute dimensionless normalized absorption.
    ///
    /// The implemented method computes norm = (mu - pre_edge) / edge_step
    /// and flat with its fitted post-edge trend removed. Missing E0 and
    /// automatic parameters are resolved from the data. Call norm(), flat(),
    /// pre_edge() and post_edge() to retrieve independent result arrays.
    /// This recomputes normalization, clears background/Fourier results and
    /// returns this spectrum. Invalid ranges, failed fits and an empty MBack selector raise ValueError.
    fn normalize(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        let py = slf.py();
        let inner = &mut slf.inner;
        py.detach(|| inner.normalize().map(|_| ())).map_err(error)?;
        Ok(slf)
    }
    /// Fit the selected smooth background and calculate dimensionless chi(k).
    ///
    /// Missing normalization runs first. The default AUTOBK method suppresses
    /// low-R Fourier content with rbkg=1.0 angstrom and the fixed endpoint
    /// penalty; k() is a zero-origin grid with default step 0.05 inverse angstroms.
    /// The returned chi() is unweighted. This recomputes the background, clears
    /// forward/inverse results and returns this spectrum. Background failures
    /// or ILPBkg raise RuntimeError; prerequisite normalization can raise ValueError.
    fn calc_background(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        let py = slf.py();
        let inner = &mut slf.inner;
        py.detach(|| inner.calc_background().map(|_| ()))
            .map_err(error)?;
        Ok(slf)
    }
    /// Copy the latest full MBACK result, or None if absent/invalidated. Its arrays
    /// and diagnostics remain independent after further spectrum processing.
    fn mback_result(&self) -> Option<mback::PyMbackResult> {
        match self.inner.normalization.as_ref()? {
            rexafs::NormalizationMethod::MBack(m) => {
                m.result.as_ref().map(|r| mback::PyMbackResult {
                    inner: (**r).clone(),
                })
            }
            _ => None,
        }
    }
    /// Compute the weighted k-to-R Fourier transform, running missing prerequisites.
    ///
    /// Defaults are k=2 to 15 inverse angstroms, kweight=2, a KaiserBessel
    /// window and nfft=2048. The amplitude multiplier is kstep / sqrt(pi)
    /// after an unnormalized negative-exponent FFT; there is no extra 1/nfft.
    /// For dimensionless chi the default output units are inverse cubic
    /// angstroms. XrayFFTF documents the full convention and sampling choices.
    ///
    /// Use r() with chir_real(), chir_imag() or chir_mag(), and kwin_k() with
    /// kwin(). This recomputes the forward transform, clears inverse results
    /// and returns this spectrum. Background and FFT failures raise RuntimeError;
    /// prerequisite normalization can raise ValueError.
    fn fft(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        let py = slf.py();
        let inner = &mut slf.inner;
        py.detach(|| inner.fft().map(|_| ())).map_err(error)?;
        Ok(slf)
    }
    /// Back-transform the stored Fourier signal to a real signal on q().
    ///
    /// Missing forward stages run first. The inverse applies an R window and
    /// reconstructs a real signal using conjugate negative-frequency bins.
    /// The forward k weighting and window remain: chiq() is generally not
    /// the original unweighted chi(k). With default forward kweight=2 and
    /// inverse rweight=0, chiq() has units inverse square angstroms.
    /// This recomputes the inverse and returns this spectrum. Invalid inverse
    /// grids or settings raise RuntimeError; prerequisite errors propagate.
    ///
    /// At least two entries must be present in the displayed r() array;
    /// an overly small forward rmax_out can therefore prevent inversion.
    /// After changing the forward R spacing, reassign inverse settings with
    /// automatic kstep to resolve the new grid instead of retaining an old
    /// resolved spacing. Configurable inverse settings were added in 0.2.5;
    /// use a fresh spectrum in 0.2.4 after changing the forward grid.
    fn ifft(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        let py = slf.py();
        let inner = &mut slf.inner;
        py.detach(|| inner.ifft().map(|_| ())).map_err(error)?;
        Ok(slf)
    }
    /// Uniform background k axis in inverse angstroms, paired with chi().
    ///
    /// It begins at zero with AUTOBK.kstep spacing. It remains the background
    /// grid even when the forward transform uses grid="Larch".
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// calc_background() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn k<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner.k().map(|v| PyArray1::from_vec(py, v.to_vec()))
    }
    /// Unweighted, dimensionless EXAFS oscillation on k().
    ///
    /// chi is (mu - smooth background) / edge_step, resampled on the background
    /// k grid. Neither the AUTOBK objective weight nor the later forward FFT
    /// k weight is stored in this array.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// calc_background() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn chi<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner.chi().map(|v| PyArray1::from_vec(py, v.to_vec()))
    }
    /// Dimensionless normalized absorption on the original energy grid.
    ///
    /// norm = (mu - pre_edge) / edge_step, with the baseline and edge step in
    /// the same units as mu. Use this for a normalized edge; flat() additionally
    /// removes the fitted post-edge trend.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// normalize() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn norm<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .norm()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    /// Dimensionless normalized absorption with the fitted post-edge trend removed.
    ///
    /// Above the sample nearest E0, the fitted baseline difference divided
    /// by edge_step is subtracted and its value at E0 added back. Below that
    /// sample, flat equals norm. Values use the original energy grid; this
    /// flattening is separate from AUTOBK background subtraction.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// normalize() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn flat<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .flat()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    /// Fitted pre-edge baseline on the original energy grid, in mu units.
    ///
    /// The line fitted to mu * E**n_victoreen is divided by E**n_victoreen.
    /// It is extrapolated across the scan so normalization can subtract it.
    /// Inspect this baseline against the measured pre-edge region.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// normalize() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn pre_edge<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .pre_edge()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    /// Fitted post-edge baseline on the original energy grid, in mu units.
    ///
    /// This is the fitted post-edge polynomial plus the pre-edge baseline.
    /// Its difference from pre_edge near E0 estimates the normalization edge
    /// step. It is not the AUTOBK smooth atomic background.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// normalize() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn post_edge<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .post_edge()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    /// Forward-transform R axis in angstroms, paired with chir_real/imag/mag().
    ///
    /// For FFT length N and k spacing delta_k, adjacent bins are separated
    /// by pi / (N * delta_k); rmax_out limits the returned range. Zero padding
    /// only refines sampling. Scattering phase shifts mean that an uncorrected
    /// R peak is not directly a bond length.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// fft() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn r<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .r()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    /// Dimensionless forward Fourier-window values, paired with kwin_k().
    ///
    /// These are the window W before multiplying by chi and k**kweight.
    /// Use the matching kwin_k() axis: with grid="Larch" the window can extend
    /// beyond, and have a different length from, the background k() grid.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// fft() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn kwin<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .kwin()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    /// k axis of the forward Fourier window, in inverse angstroms.
    ///
    /// Pair it with kwin(). For grid="Input" it matches the background grid;
    /// for grid="Larch" it uses the resampled, extended window domain. The
    /// background k()/chi() arrays themselves are unchanged by this choice.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// fft() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn kwin_k<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .kwin_k()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    /// Magnitude of the complex forward transform, paired with r().
    ///
    /// It equals sqrt(chir_real()**2 + chir_imag()**2). For dimensionless chi
    /// and integer forward kweight w, units are angstrom**(-(w + 1)); the
    /// default w=2 gives inverse cubic angstroms. Magnitude discards phase
    /// information and is not a probability density.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// fft() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn chir_mag<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .chir_mag()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    /// Real component of the complex forward transform, paired with r().
    ///
    /// The convention uses a negative-exponent FFT followed by kstep/sqrt(pi),
    /// without additional FFT-length normalization. For dimensionless chi and
    /// integer kweight w, units are angstrom**(-(w + 1)); the default w=2
    /// gives inverse cubic angstroms.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// fft() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn chir_real<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .chir_real()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    /// Imaginary component of the complex forward transform, paired with r().
    ///
    /// Its sign follows the negative-exponent forward FFT, without an extra
    /// phase rotation. For dimensionless chi and integer kweight w, units are
    /// angstrom**(-(w + 1)); the default w=2 gives inverse cubic angstroms.
    /// Keep the complex components when phase matters.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// fft() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn chir_imag<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .chir_imag()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    /// Back-transform q axis in inverse angstroms, paired with chiq().
    ///
    /// q describes the same physical variable as k, but labels a reconstructed
    /// signal after R filtering. Its spacing is determined by the inverse FFT
    /// length and the input R spacing; qmax_out limits the returned range.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// ifft() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn q<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .q()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    /// Real R-filtered signal on q(), retaining the forward weighting and window.
    ///
    /// This is generally not the original unweighted chi(k). For dimensionless
    /// input chi, integer forward kweight w and inverse rweight v, its units
    /// are angstrom**(v - w); defaults w=2 and v=0 give inverse square
    /// angstroms. A selected R window removes contributions outside its taper.
    ///
    /// Returns an independent NumPy float64 array copy, or None before
    /// ifft() (or a dependent stage) succeeds or after invalidation.
    /// Reading this result does not run processing; editing the copy does
    /// not change the spectrum.
    fn chiq<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .chiq()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
}
/// Read a QAS transmission text file through the native reader.
///
/// `path` is a filename string. The first three whitespace-separated columns
/// contain energy in eV and incident/transmitted intensities; mu is their
/// natural log ratio. Energy and mu are sorted together when needed, while
/// duplicate rows remain. The reader does not validate positive intensities.
/// It returns an owned, unprocessed spectrum; file/parse errors become
/// RuntimeError and processing performs later numerical validation.
/// Public callers should use rexafs.io.read_qas_transmission, which also
/// converts pathlib.Path and provides the measurement explanation/citation.
/// Transmission convention: [Newville, Fundamentals of XAFS, section 4](https://docs.xrayabsorption.org/tutorials/XAFS_Fundamentals.pdf).
#[pyfunction]
fn read_qas_transmission(path: &str) -> PyResult<PySpectrum> {
    Ok(PySpectrum {
        inner: rexafs::io::read_qas_transmission(path).map_err(|e| error(e.into()))?,
    })
}
/// Native storage for the rexafs.io.Measurement facade introduced in 0.2.10.
/// Reading preserves source arrays; selecting a mapping performs only detector
/// arithmetic and conversion to eV. No processing or input mutation occurs.
#[pyclass(name = "Measurement")]
struct PyMeasurement {
    inner: rexafs::io::Measurement,
}
#[pymethods]
impl PyMeasurement {
    #[new]
    fn new(bytes: &[u8]) -> PyResult<Self> {
        Ok(Self {
            inner: rexafs::io::parse_measurement(bytes)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
        })
    }
    fn select_datasets(&mut self, paths: Vec<String>) -> PyResult<usize> {
        let scan = self
            .inner
            .dataset_scan(&paths)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let index = self.inner.scans.len();
        self.inner.scans.push(scan);
        Ok(index)
    }
    fn document_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }
    #[pyo3(signature = (scan=0, mapping_json=None))]
    fn arrays<'py>(
        &self,
        py: Python<'py>,
        scan: usize,
        mapping_json: Option<&str>,
    ) -> PyResult<PySpectrumArrays<'py>> {
        let mapping = mapping_json
            .map(serde_json::from_str::<rexafs::io::SpectrumSelection>)
            .transpose()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let s =
            self.inner.scans.get(scan).ok_or_else(|| {
                pyo3::exceptions::PyIndexError::new_err("Scan index out of range")
            })?;
        let mapping = mapping
            .as_ref()
            .map(|m| m.resolve(s))
            .transpose()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let (energy, mu) = s
            .arrays(mapping.as_ref())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok((PyArray1::from_vec(py, energy), PyArray1::from_vec(py, mu)))
    }
    #[pyo3(signature = (scan=0, mapping_json=None))]
    fn spectrum(&self, scan: usize, mapping_json: Option<&str>) -> PyResult<PySpectrum> {
        let mapping = mapping_json
            .map(serde_json::from_str::<rexafs::io::SpectrumSelection>)
            .transpose()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let s =
            self.inner.scans.get(scan).ok_or_else(|| {
                pyo3::exceptions::PyIndexError::new_err("Scan index out of range")
            })?;
        let mapping = mapping
            .as_ref()
            .map(|m| m.resolve(s))
            .transpose()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(PySpectrum {
            inner: s
                .to_spectrum(mapping.as_ref())
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
        })
    }
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPrePostEdge>()?;
    m.add_class::<PyAUTOBK>()?;
    m.add_class::<PyXrayFFTF>()?;
    m.add_class::<PyXrayFFTR>()?;
    m.add_class::<PyNormalizationMethod>()?;
    m.add_class::<PyBackgroundMethod>()?;
    m.add_class::<PySpectrum>()?;
    m.add_class::<metrics::PyMeasurementResult>()?;
    m.add_class::<wavelet::PyWavelet>()?;
    m.add_class::<fluorescence::PyFluorescenceCorrection>()?;
    m.add_class::<fluorescence::PyFluorescenceCorrectionResult>()?;
    m.add_class::<wavelet::PyWaveletMap>()?;
    m.add_class::<wavelet::PyWaveletRegionValue>()?;
    m.add_class::<mback::PyMBack>()?;
    m.add_class::<mback::PyMbackErfc>()?;
    m.add_class::<mback::PyMbackResult>()?;
    m.add_class::<peaks::PyPeakFit>()?;
    m.add_class::<peaks::PyPeakFitResult>()?;
    m.add_class::<peaks::PyPeakContribution>()?;
    m.add_class::<peaks::PyPeakOutcome>()?;
    m.add_class::<PyMeasurement>()?;
    m.add_function(wrap_pyfunction!(read_qas_transmission, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
