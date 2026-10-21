//! Python wavelet definitions and owned native-grid results.
use numpy::{IntoPyArray, PyArray1, PyArray2};
use pyo3::{exceptions::PyValueError, prelude::*};
fn invalid(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}
fn dictionary<'py>(py: Python<'py>, json: String) -> PyResult<Bound<'py, PyAny>> {
    py.import("json")?.getattr("loads")?.call1((json,))
}
/// Cauchy wavelet settings (since 0.2.10). Use Wavelet((2, 12)) with spectrum.wavelet(model).
///
/// k_range is measured support in inverse angstroms. Defaults: weight 2, order 100,
/// kstep=0.05 inverse angstroms, rmax=6 angstroms, no taper and automatic FFT/R sampling.
/// Larger order narrows frequency response and broadens localization in k. R is not
/// phase-corrected. The cauchy_v1 profile uses a fixed order independent of R extent.
/// Construction copies settings; calculation checks finite values and budgets. Since
/// 0.2.15 endpoints beyond the measured k range are limited to it; the map keeps the
/// requested settings, its warnings report the range used, and a range wholly outside
/// the data raises.
/// Advanced radii replace the generated R grid; nfft never silently truncates input.
#[pyclass(name = "Wavelet", module = "rexafs", frozen, skip_from_py_object)]
#[derive(Clone)]
pub struct PyWavelet {
    pub inner: rexafs::Wavelet,
}
#[pymethods]
impl PyWavelet {
    /// k_range is inclusive. taper is the width of half-cosine ramps inside its bounds;
    /// zero means no taper. rstep=None chooses pi/(FFT length*kstep). radii, when given,
    /// are positive increasing angstrom coordinates. All input settings are copied.
    #[new]
    #[pyo3(signature=(k_range,*,kweight=2,order=100,kstep=0.05,rmax=6.0,rstep=None,taper=0.0,nfft=None,radii=None))]
    #[allow(clippy::too_many_arguments)] // Scientific options are keyword-only.
    fn new(
        k_range: (f64, f64),
        kweight: u8,
        order: usize,
        kstep: f64,
        rmax: f64,
        rstep: Option<f64>,
        taper: f64,
        nfft: Option<usize>,
        radii: Option<Vec<f64>>,
    ) -> PyResult<Self> {
        if !taper.is_finite() || taper < 0. {
            return Err(invalid("taper must be finite and nonnegative"));
        }
        let mut inner = rexafs::Wavelet::new(k_range.0..=k_range.1)
            .kweight(kweight)
            .order(order)
            .kstep(kstep)
            .rmax(rmax);
        inner.rstep = rstep;
        inner.nfft = nfft;
        inner.radii = radii;
        if taper > 0. {
            inner = inner.taper(taper);
        }
        Ok(Self { inner })
    }
    /// Transform original unweighted chi(k). Copies matching finite one-dimensional
    /// arrays and releases the GIL. k is nonnegative, increasing and in inverse angstroms;
    /// chi is dimensionless. Linear resampling is explicit; only selected measured
    /// support contributes. No extrapolation or display sampling affects the map.
    /// Returns owned complex arrays (rows=R, columns=k), with original inputs retained.
    /// Invalid settings, missing coverage or excessive resource requests raise ValueError.
    fn calculate(
        &self,
        py: Python<'_>,
        k: &Bound<'_, PyAny>,
        chi: &Bound<'_, PyAny>,
    ) -> PyResult<PyWaveletMap> {
        let k = crate::metrics::array(py, k, "k")?;
        let chi = crate::metrics::array(py, chi, "chi")?;
        let inner = py
            .detach(|| self.inner.calculate(&k, &chi))
            .map_err(invalid)?;
        Ok(PyWaveletMap { inner })
    }
    /// Validate the grid and report k_points, r_points, nfft, cells and estimated bytes.
    /// The estimate excludes input copies, FFT scratch and serialization overhead.
    fn estimate<'py>(&self, py: Python<'py>, k: &Bound<'_, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let k = crate::metrics::array(py, k, "k")?;
        dictionary(
            py,
            serde_json::to_string(&self.inner.estimate(&k).map_err(invalid)?).map_err(invalid)?,
        )
    }
    /// Native settings JSON, without input arrays. All automatic choices are preserved.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(invalid)
    }
    /// Restore native settings. Calculation validates scientific values and budgets.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json).map_err(invalid)?,
        })
    }
}
/// Owned Cauchy map (since 0.2.10). Array properties are independent NumPy copies.
///
/// shape is (R rows, k columns). real, imaginary and magnitude have that shape;
/// k/r are physical coordinates. W has units of k**weight * chi under cauchy_v1,
/// distinct from ordinary Fourier-transform scaling. Colors do not define a metric.
/// to_json retains full numerical data and preparation provenance, not a texture.
#[pyclass(name = "WaveletMap", module = "rexafs", frozen, skip_from_py_object)]
#[derive(Clone)]
pub struct PyWaveletMap {
    pub inner: rexafs::WaveletMap,
}
impl PyWaveletMap {
    fn matrix<'py>(
        &self,
        py: Python<'py>,
        values: Vec<f64>,
    ) -> PyResult<Bound<'py, PyArray2<f64>>> {
        let array = numpy::ndarray::Array2::from_shape_vec(
            (self.inner.r().len(), self.inner.k().len()),
            values,
        )
        .map_err(invalid)?;
        Ok(array.into_pyarray(py))
    }
}
#[pymethods]
impl PyWaveletMap {
    /// Matrix dimensions in (R, k) order, including explicit padded k columns.
    #[getter]
    fn shape(&self) -> (usize, usize) {
        (self.inner.r().len(), self.inner.k().len())
    }
    /// Independent k coordinates, in inverse angstroms.
    #[getter]
    fn k<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_vec(py, self.inner.k().to_vec())
    }
    /// Independent R coordinates, in angstroms; not phase-corrected bond lengths.
    #[getter]
    fn r<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_vec(py, self.inner.r().to_vec())
    }
    /// Original measured k coordinates, before resampling.
    #[getter]
    fn input_k<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_vec(py, self.inner.input_k().to_vec())
    }
    /// Original dimensionless unweighted chi; unchanged.
    #[getter]
    fn input_chi<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_vec(py, self.inner.input_chi().to_vec())
    }
    /// Resampled unweighted chi, zero outside selected support.
    #[getter]
    fn prepared_chi<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_vec(py, self.inner.prepared_chi().to_vec())
    }
    /// Support/taper multipliers applied before k weighting.
    #[getter]
    fn window<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_vec(py, self.inner.window().to_vec())
    }
    /// Boolean measured-support mask; False denotes padding, not measured zero.
    #[getter]
    fn support<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<bool>> {
        PyArray1::from_vec(py, self.inner.support().to_vec())
    }
    /// Independent real matrix, rows=R and columns=k.
    #[getter]
    fn real<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f64>>> {
        self.matrix(py, self.inner.real().to_vec())
    }
    /// Independent imaginary matrix, rows=R and columns=k.
    #[getter]
    fn imaginary<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f64>>> {
        self.matrix(py, self.inner.imaginary().to_vec())
    }
    /// Independent magnitude matrix; no display normalization or resampling.
    #[getter]
    fn magnitude<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f64>>> {
        self.matrix(py, self.inner.magnitude())
    }
    /// Independent phase matrix in radians. NaN masks zero amplitude and samples below
    /// relative_floor times the map maximum (default 1%); fraction must be in [0,1].
    /// Masking is a display choice and does not change the native map or region metric.
    #[pyo3(signature=(relative_floor=0.01))]
    fn phase<'py>(
        &self,
        py: Python<'py>,
        relative_floor: f64,
    ) -> PyResult<Bound<'py, PyArray2<f64>>> {
        self.matrix(
            py,
            self.inner
                .phase(relative_floor)
                .map_err(invalid)?
                .into_iter()
                .map(|p| p.unwrap_or(f64::NAN))
                .collect(),
        )
    }
    /// Native magnitude versus k at a covered R coordinate (angstroms).
    fn slice_at_r<'py>(&self, py: Python<'py>, r: f64) -> PyResult<Bound<'py, PyArray1<f64>>> {
        Ok(PyArray1::from_vec(
            py,
            self.inner.slice_at_r(r).map_err(invalid)?,
        ))
    }
    /// Native magnitude versus R at a covered k coordinate (inverse angstroms).
    fn slice_at_k<'py>(&self, py: Python<'py>, k: f64) -> PyResult<Bound<'py, PyArray1<f64>>> {
        Ok(PyArray1::from_vec(
            py,
            self.inner.slice_at_k(k).map_err(invalid)?,
        ))
    }
    /// Integrate native bilinear magnitude over a fully covered k/R rectangle.
    /// k_range uses inverse angstroms; r_range uses angstroms. Returns value, exact
    /// bounds, units and method, with no experimental uncertainty. Display sampling
    /// never participates. Invalid or uncovered bounds raise ValueError.
    fn integral(
        &self,
        py: Python<'_>,
        k_range: (f64, f64),
        r_range: (f64, f64),
    ) -> PyResult<PyWaveletRegionValue> {
        let inner = py
            .detach(|| {
                self.inner
                    .integral(k_range.0..=k_range.1, r_range.0..=r_range.1)
            })
            .map_err(invalid)?;
        Ok(PyWaveletRegionValue { inner })
    }
    /// Area-weighted mean of native bilinear magnitude (since 0.2.10), not a mean
    /// of grid cells. k_range is inverse angstroms; r_range is angstroms. Fully
    /// covered, finite, increasing bounds are required. Returns units and method;
    /// no uncertainty is inferred. Releases the Python GIL.
    fn mean(
        &self,
        py: Python<'_>,
        k_range: (f64, f64),
        r_range: (f64, f64),
    ) -> PyResult<PyWaveletRegionValue> {
        let inner = py
            .detach(|| {
                self.inner
                    .mean(k_range.0..=k_range.1, r_range.0..=r_range.1)
            })
            .map_err(invalid)?;
        Ok(PyWaveletRegionValue { inner })
    }
    /// Maximum of the native bilinear magnitude surface, including interpolated
    /// rectangle boundaries (since 0.2.10). k_range is inverse angstroms; r_range
    /// is angstroms. Invalid or uncovered bounds raise ValueError. Returns units
    /// and method without an inferred error bar; releases the Python GIL.
    fn maximum(
        &self,
        py: Python<'_>,
        k_range: (f64, f64),
        r_range: (f64, f64),
    ) -> PyResult<PyWaveletRegionValue> {
        let inner = py
            .detach(|| {
                self.inner
                    .maximum(k_range.0..=k_range.1, r_range.0..=r_range.1)
            })
            .map_err(invalid)?;
        Ok(PyWaveletRegionValue { inner })
    }
    /// Independent definition; automatic grid choices and explicit coordinates retained.
    #[getter]
    fn definition(&self) -> PyWavelet {
        PyWavelet {
            inner: self.inner.settings().clone(),
        }
    }
    /// Original spectrum preparation metadata, or None for an array calculation.
    #[getter]
    fn preparation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        dictionary(
            py,
            serde_json::to_string(&self.inner.preparation()).map_err(invalid)?,
        )
    }
    /// Interpretation and finite-support diagnostics, not confidence intervals.
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.inner.warnings().to_vec()
    }
    /// Complete native map JSON with original inputs, complex arrays and provenance.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(invalid)
    }
    /// Restore a native map; checks method, dimensions, axes, finite values and budgets.
    /// Validation does not independently prove an external producer's numerical result.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json).map_err(invalid)?,
        })
    }
}
/// Immutable native wavelet region statistic, with exact bounds and convention.
/// Units equal k**weight * chi because dk (inverse angstroms) times dR (angstroms)
/// cancel. This is a descriptive transform metric, not a chemical concentration.
#[pyclass(
    name = "WaveletRegionValue",
    module = "rexafs",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyWaveletRegionValue {
    inner: rexafs::WaveletRegionValue,
}
#[pymethods]
impl PyWaveletRegionValue {
    /// Native region statistic; no experimental uncertainty is supplied.
    #[getter]
    fn value(&self) -> f64 {
        self.inner.value
    }
    /// Exact inclusive k bounds in inverse angstroms.
    #[getter]
    fn k_range(&self) -> (f64, f64) {
        (self.inner.k_range[0], self.inner.k_range[1])
    }
    /// Exact inclusive R bounds in angstroms.
    #[getter]
    fn r_range(&self) -> (f64, f64) {
        (self.inner.r_range[0], self.inner.r_range[1])
    }
    /// Integral units including the selected k weight.
    #[getter]
    fn unit(&self) -> String {
        self.inner.unit.clone()
    }
    /// Numerical convention: bilinear_magnitude_v1 (integral),
    /// bilinear_magnitude_mean_v1 or bilinear_magnitude_maximum_v1.
    #[getter]
    fn method(&self) -> String {
        self.inner.method.clone()
    }
    /// Full result JSON including value, bounds, units and method.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(invalid)
    }
}
