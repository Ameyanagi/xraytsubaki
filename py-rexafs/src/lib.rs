//! Thin Python bindings: all stage execution and defaults live in rexafs.
use numpy::{PyArray1, PyReadonlyArray1};
use pyo3::prelude::*;

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

#[pyclass(name = "PrePostEdge", module = "rexafs", skip_from_py_object)]
#[derive(Clone)]
struct PyPrePostEdge {
    inner: rexafs::PrePostEdge,
}
#[pymethods]
impl PyPrePostEdge {
    #[new]
    fn new() -> Self {
        Self {
            inner: rexafs::PrePostEdge::new(),
        }
    }
    #[getter]
    fn pre_edge_start(&self) -> Option<f64> {
        self.inner.pre_edge_start
    }
    #[setter]
    fn set_pre_edge_start(&mut self, value: Option<f64>) {
        self.inner.pre_edge_start = value;
    }
    #[getter]
    fn pre_edge_end(&self) -> Option<f64> {
        self.inner.pre_edge_end
    }
    #[setter]
    fn set_pre_edge_end(&mut self, value: Option<f64>) {
        self.inner.pre_edge_end = value;
    }
    #[getter]
    fn norm_start(&self) -> Option<f64> {
        self.inner.norm_start
    }
    #[setter]
    fn set_norm_start(&mut self, value: Option<f64>) {
        self.inner.norm_start = value;
    }
    #[getter]
    fn norm_end(&self) -> Option<f64> {
        self.inner.norm_end
    }
    #[setter]
    fn set_norm_end(&mut self, value: Option<f64>) {
        self.inner.norm_end = value;
    }
    #[getter]
    fn norm_polyorder(&self) -> Option<i32> {
        self.inner.norm_polyorder
    }
    #[setter]
    fn set_norm_polyorder(&mut self, value: Option<i32>) {
        self.inner.norm_polyorder = value;
    }
    #[getter]
    fn n_victoreen(&self) -> Option<i32> {
        self.inner.n_victoreen
    }
    #[setter]
    fn set_n_victoreen(&mut self, value: Option<i32>) {
        self.inner.n_victoreen = value;
    }
    #[getter]
    fn e0(&self) -> Option<f64> {
        self.inner.e0
    }
    #[setter]
    fn set_e0(&mut self, value: Option<f64>) {
        self.inner.e0 = value;
    }
    #[getter]
    fn edge_step(&self) -> Option<f64> {
        self.inner.edge_step
    }
    #[setter]
    fn set_edge_step(&mut self, value: Option<f64>) {
        self.inner.edge_step = value;
    }
}

#[pyclass(name = "AUTOBK", module = "rexafs", skip_from_py_object)]
#[derive(Clone)]
struct PyAUTOBK {
    inner: rexafs::AUTOBK,
}
#[pymethods]
impl PyAUTOBK {
    #[new]
    fn new() -> Self {
        Self {
            inner: rexafs::AUTOBK::new(),
        }
    }
    #[getter]
    fn ek0(&self) -> Option<f64> {
        self.inner.ek0
    }
    #[setter]
    fn set_ek0(&mut self, value: Option<f64>) {
        self.inner.ek0 = value;
    }
    #[getter]
    fn rbkg(&self) -> Option<f64> {
        self.inner.rbkg
    }
    #[setter]
    fn set_rbkg(&mut self, value: Option<f64>) {
        self.inner.rbkg = value;
    }
    #[getter]
    fn nknots(&self) -> Option<i32> {
        self.inner.nknots
    }
    #[setter]
    fn set_nknots(&mut self, value: Option<i32>) {
        self.inner.nknots = value;
    }
    #[getter]
    fn kmin(&self) -> Option<f64> {
        self.inner.kmin
    }
    #[setter]
    fn set_kmin(&mut self, value: Option<f64>) {
        self.inner.kmin = value;
    }
    #[getter]
    fn kmax(&self) -> Option<f64> {
        self.inner.kmax
    }
    #[setter]
    fn set_kmax(&mut self, value: Option<f64>) {
        self.inner.kmax = value;
    }
    #[getter]
    fn kstep(&self) -> Option<f64> {
        self.inner.kstep
    }
    #[setter]
    fn set_kstep(&mut self, value: Option<f64>) {
        self.inner.kstep = value;
    }
    #[getter]
    fn nclamp(&self) -> Option<i32> {
        self.inner.nclamp
    }
    #[setter]
    fn set_nclamp(&mut self, value: Option<i32>) {
        self.inner.nclamp = value;
    }
    #[getter]
    fn clamp_lo(&self) -> Option<i32> {
        self.inner.clamp_lo
    }
    #[setter]
    fn set_clamp_lo(&mut self, value: Option<i32>) {
        self.inner.clamp_lo = value;
    }
    #[getter]
    fn clamp_hi(&self) -> Option<i32> {
        self.inner.clamp_hi
    }
    #[setter]
    fn set_clamp_hi(&mut self, value: Option<i32>) {
        self.inner.clamp_hi = value;
    }
    #[getter]
    fn clamp_lambda(&self) -> Option<f64> {
        self.inner.clamp_lambda
    }
    #[setter]
    fn set_clamp_lambda(&mut self, value: Option<f64>) {
        self.inner.clamp_lambda = value;
    }
    #[getter]
    fn nfft(&self) -> Option<i32> {
        self.inner.nfft
    }
    #[setter]
    fn set_nfft(&mut self, value: Option<i32>) {
        self.inner.nfft = value;
    }
    #[getter]
    fn kweight(&self) -> Option<i32> {
        self.inner.kweight
    }
    #[setter]
    fn set_kweight(&mut self, value: Option<i32>) {
        self.inner.kweight = value;
    }
    #[getter]
    fn dk(&self) -> Option<f64> {
        self.inner.dk
    }
    #[setter]
    fn set_dk(&mut self, value: Option<f64>) {
        self.inner.dk = value;
    }
    #[getter]
    fn linear_regularization(&self) -> Option<f64> {
        self.inner.linear_regularization
    }
    #[setter]
    fn set_linear_regularization(&mut self, value: Option<f64>) {
        self.inner.linear_regularization = value;
    }
    #[getter]
    fn linear_condition_limit(&self) -> Option<f64> {
        self.inner.linear_condition_limit
    }
    #[setter]
    fn set_linear_condition_limit(&mut self, value: Option<f64>) {
        self.inner.linear_condition_limit = value;
    }
    #[getter]
    fn linear_residual_ratio_limit(&self) -> Option<f64> {
        self.inner.linear_residual_ratio_limit
    }
    #[setter]
    fn set_linear_residual_ratio_limit(&mut self, value: Option<f64>) {
        self.inner.linear_residual_ratio_limit = value;
    }
    #[getter]
    fn linear_fallback_to_lm(&self) -> Option<bool> {
        self.inner.linear_fallback_to_lm
    }
    #[setter]
    fn set_linear_fallback_to_lm(&mut self, value: Option<bool>) {
        self.inner.linear_fallback_to_lm = value;
    }
    #[getter]
    fn linear_workspace_cache(&self) -> Option<bool> {
        self.inner.linear_workspace_cache
    }
    #[setter]
    fn set_linear_workspace_cache(&mut self, value: Option<bool>) {
        self.inner.linear_workspace_cache = value;
    }
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

#[pyclass(name = "XrayFFTF", module = "rexafs", skip_from_py_object)]
#[derive(Clone)]
struct PyXrayFFTF {
    inner: rexafs::XrayFFTF,
}
#[pymethods]
impl PyXrayFFTF {
    #[new]
    fn new() -> Self {
        Self {
            inner: rexafs::XrayFFTF::new(),
        }
    }
    #[getter]
    fn rmax_out(&self) -> Option<f64> {
        self.inner.rmax_out
    }
    #[setter]
    fn set_rmax_out(&mut self, value: Option<f64>) {
        self.inner.rmax_out = value;
    }
    #[getter]
    fn dk(&self) -> Option<f64> {
        self.inner.dk
    }
    #[setter]
    fn set_dk(&mut self, value: Option<f64>) {
        self.inner.dk = value;
    }
    #[getter]
    fn dk2(&self) -> Option<f64> {
        self.inner.dk2
    }
    #[setter]
    fn set_dk2(&mut self, value: Option<f64>) {
        self.inner.dk2 = value;
    }
    #[getter]
    fn kmin(&self) -> Option<f64> {
        self.inner.kmin
    }
    #[setter]
    fn set_kmin(&mut self, value: Option<f64>) {
        self.inner.kmin = value;
    }
    #[getter]
    fn kmax(&self) -> Option<f64> {
        self.inner.kmax
    }
    #[setter]
    fn set_kmax(&mut self, value: Option<f64>) {
        self.inner.kmax = value;
    }
    #[getter]
    fn kweight(&self) -> Option<f64> {
        self.inner.kweight
    }
    #[setter]
    fn set_kweight(&mut self, value: Option<f64>) {
        self.inner.kweight = value;
    }
    #[getter]
    fn nfft(&self) -> Option<usize> {
        self.inner.nfft
    }
    #[setter]
    fn set_nfft(&mut self, value: Option<usize>) {
        self.inner.nfft = value;
    }
    #[getter]
    fn kstep(&self) -> Option<f64> {
        self.inner.kstep
    }
    #[setter]
    fn set_kstep(&mut self, value: Option<f64>) {
        self.inner.kstep = value;
    }
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

#[pyclass(name = "NormalizationMethod", module = "rexafs", skip_from_py_object)]
#[derive(Clone)]
struct PyNormalizationMethod {
    inner: rexafs::NormalizationMethod,
}
#[pymethods]
impl PyNormalizationMethod {
    #[staticmethod]
    #[pyo3(name = "PrePostEdge")]
    fn configured(parameters: &PyPrePostEdge) -> Self {
        Self {
            inner: rexafs::NormalizationMethod::PrePostEdge(parameters.inner.clone()),
        }
    }
    #[staticmethod]
    fn new_prepostedge() -> Self {
        Self {
            inner: rexafs::NormalizationMethod::new_prepostedge(),
        }
    }
    #[staticmethod]
    fn new_mback() -> Self {
        Self {
            inner: rexafs::NormalizationMethod::new_mback(),
        }
    }
}

#[pyclass(name = "BackgroundMethod", module = "rexafs", skip_from_py_object)]
#[derive(Clone)]
struct PyBackgroundMethod {
    inner: rexafs::BackgroundMethod,
}
#[pymethods]
impl PyBackgroundMethod {
    #[staticmethod]
    #[pyo3(name = "AUTOBK")]
    fn configured(parameters: &PyAUTOBK) -> Self {
        Self {
            inner: rexafs::BackgroundMethod::AUTOBK(parameters.inner.clone()),
        }
    }
    #[staticmethod]
    fn new_autobk() -> Self {
        Self {
            inner: rexafs::BackgroundMethod::new_autobk(),
        }
    }
    #[staticmethod]
    fn new_ilpbkg() -> Self {
        Self {
            inner: rexafs::BackgroundMethod::new_ilpbkg(),
        }
    }
}

#[pyclass(name = "Spectrum", module = "rexafs", skip_from_py_object)]
struct PySpectrum {
    inner: rexafs::Spectrum,
}
#[pymethods]
impl PySpectrum {
    #[new]
    fn new(py: Python<'_>, energy: &Bound<'_, PyAny>, mu: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: arrays(py, energy, mu)?,
        })
    }
    #[staticmethod]
    fn from_arrays(
        py: Python<'_>,
        energy: &Bound<'_, PyAny>,
        mu: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Self::new(py, energy, mu)
    }
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
    fn set_e0(mut slf: PyRefMut<'_, Self>, e0: f64) -> PyRefMut<'_, Self> {
        slf.inner.set_e0(e0);
        slf
    }
    fn e0(&self) -> Option<f64> {
        self.inner.e0()
    }
    fn invalidate_derived(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.inner.invalidate_derived();
        slf
    }
    fn set_fft<'py>(mut slf: PyRefMut<'py, Self>, parameters: &PyXrayFFTF) -> PyRefMut<'py, Self> {
        slf.inner.set_fft(parameters.inner.clone());
        slf
    }
    #[pyo3(signature = (method=None))]
    fn set_normalization_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        method: Option<&PyNormalizationMethod>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner
            .set_normalization_method(method.map(|m| m.inner.clone()))
            .map_err(error)?;
        Ok(slf)
    }
    #[pyo3(signature = (method=None))]
    fn set_background_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        method: Option<&PyBackgroundMethod>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner
            .set_background_method(method.map(|m| m.inner.clone()))
            .map_err(error)?;
        Ok(slf)
    }
    fn find_e0(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        let py = slf.py();
        let inner = &mut slf.inner;
        py.detach(|| inner.find_e0().map(|_| ())).map_err(error)?;
        Ok(slf)
    }
    fn normalize(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        let py = slf.py();
        let inner = &mut slf.inner;
        py.detach(|| inner.normalize().map(|_| ())).map_err(error)?;
        Ok(slf)
    }
    fn calc_background(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        let py = slf.py();
        let inner = &mut slf.inner;
        py.detach(|| inner.calc_background().map(|_| ()))
            .map_err(error)?;
        Ok(slf)
    }
    fn fft(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        let py = slf.py();
        let inner = &mut slf.inner;
        py.detach(|| inner.fft().map(|_| ())).map_err(error)?;
        Ok(slf)
    }
    fn ifft(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        let py = slf.py();
        let inner = &mut slf.inner;
        py.detach(|| inner.ifft().map(|_| ())).map_err(error)?;
        Ok(slf)
    }
    fn k<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner.k().map(|v| PyArray1::from_vec(py, v.to_vec()))
    }
    fn chi<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner.chi().map(|v| PyArray1::from_vec(py, v.to_vec()))
    }
    fn norm<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .norm()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    fn flat<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .flat()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    fn pre_edge<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .pre_edge()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    fn post_edge<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .post_edge()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    fn r<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .r()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    fn chir_mag<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .chir_mag()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    fn chir_real<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .chir_real()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    fn chir_imag<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .chir_imag()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    fn q<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .q()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
    fn chiq<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner
            .chiq()
            .map(|v| PyArray1::from_vec(py, v.as_slice().to_vec()))
    }
}
#[pyfunction]
fn read_qas_transmission(path: &str) -> PyResult<PySpectrum> {
    Ok(PySpectrum {
        inner: rexafs::io::read_qas_transmission(path).map_err(|e| error(e.into()))?,
    })
}
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPrePostEdge>()?;
    m.add_class::<PyAUTOBK>()?;
    m.add_class::<PyXrayFFTF>()?;
    m.add_class::<PyNormalizationMethod>()?;
    m.add_class::<PyBackgroundMethod>()?;
    m.add_class::<PySpectrum>()?;
    m.add_function(wrap_pyfunction!(read_qas_transmission, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
