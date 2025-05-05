use pyo3::prelude::*;
use pyo3::types::PyDict;
use numpy::{PyArray1, PyReadonlyArray1, IntoPyArray};
use xraytsubaki::xafs::{normalization, background, xafsutils, xrayfft};

// Import specific types
use xraytsubaki::xafs::normalization::NormalizationParameters;
use xraytsubaki::xafs::background::BackgroundParameters;
use xraytsubaki::xafs::xrayfft::{FTParameters, IFTParameters};

pub mod xasgroup;
pub mod xasspectrum;
pub mod fitting;
pub mod multispectrum;

use crate::xasgroup::PyXASGroup;
use crate::xasspectrum::PyXASSpectrum;
use crate::fitting::{PyFittingParameter, PyFittingParameters, PySimplePath, PyFittingDataset, PyFitResult, PyExafsFitter};
use crate::multispectrum::{PyParameterConstraint, PyConstrainedParameter, PyConstrainedParameters, PyMultiSpectrumDataset, PyMultiSpectrumFitResult, PyMultiSpectrumFitter};

/// Find the absorption edge energy (E0) in an XAS spectrum
#[pyfunction]
#[pyo3(signature = (energy, mu))]
fn find_e0(energy: PyReadonlyArray1<f64>, mu: PyReadonlyArray1<f64>) -> PyResult<f64> {
    let energy_arr = energy.as_array();
    let mu_arr = mu.as_array();
    
    // Convert to owned arrays before calling find_e0
    let energy_owned = energy_arr.to_owned();
    let mu_owned = mu_arr.to_owned();
    
    match xafsutils::find_e0(energy_owned, mu_owned) {
        Ok(e0) => Ok(e0),
        Err(err) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            format!("Error finding E0: {:?}", err)
        )),
    }
}

/// Perform pre-edge subtraction and normalization
#[pyfunction]
#[pyo3(signature = (energy, mu, e0=None, pre1=None, pre2=None, norm1=None, norm2=None, nnorm=None))]
fn pre_edge(
    py: Python,
    energy: PyReadonlyArray1<f64>,
    mu: PyReadonlyArray1<f64>,
    e0: Option<f64>,
    pre1: Option<f64>,
    pre2: Option<f64>,
    norm1: Option<f64>,
    norm2: Option<f64>,
    nnorm: Option<i32>,
) -> PyResult<Py<PyDict>> {
    let energy_arr = energy.as_array();
    let mu_arr = mu.as_array();
    
    // Create normalization parameters
    let mut params = normalization::NormalizationParameters::new();
    
    // Set parameters if provided
    if let Some(e0_value) = e0 {
        params.e0 = Some(e0_value);
    }
    
    if let Some(pre1_value) = pre1 {
        params.pre_edge_start = Some(pre1_value);
    }
    
    if let Some(pre2_value) = pre2 {
        params.pre_edge_end = Some(pre2_value);
    }
    
    if let Some(norm1_value) = norm1 {
        params.norm_start = Some(norm1_value);
    }
    
    if let Some(norm2_value) = norm2 {
        params.norm_end = Some(norm2_value);
    }
    
    if let Some(nnorm_value) = nnorm {
        params.norm_polyorder = Some(nnorm_value);
    }
    
    // Perform pre-edge subtraction
    match normalization::pre_edge_scipy(energy_arr, mu_arr, &params) {
        Ok(result) => {
            // Create Python dictionary to return results
            let dict = PyDict::new(py);
            
            // Add results to dictionary
            dict.set_item("e0", result.e0)?;
            dict.set_item("edge_step", result.edge_step)?;
            dict.set_item("norm", result.norm.into_pyarray(py))?;
            dict.set_item("pre_edge", result.pre_edge.into_pyarray(py))?;
            dict.set_item("post_edge", result.post_edge.into_pyarray(py))?;
            
            Ok(dict.into())
        },
        Err(err) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            format!("Pre-edge subtraction error: {:?}", err)
        )),
    }
}

/// Extract EXAFS signal by removing background
#[pyfunction]
#[pyo3(signature = (energy, mu, e0=None, rbkg=None, kmin=None, kmax=None, kweight=None, dk=None, window=None))]
fn autobk(
    py: Python,
    energy: PyReadonlyArray1<f64>,
    mu: PyReadonlyArray1<f64>,
    e0: Option<f64>,
    rbkg: Option<f64>,
    kmin: Option<f64>,
    kmax: Option<f64>,
    kweight: Option<f64>,
    dk: Option<f64>,
    window: Option<&str>,
) -> PyResult<Py<PyDict>> {
    let energy_arr = energy.as_array();
    let mu_arr = mu.as_array();
    
    // Create background parameters
    let mut params = background::BackgroundParameters::new();
    
    // Set parameters if provided
    if let Some(e0_value) = e0 {
        params.e0 = Some(e0_value);
    }
    
    if let Some(rbkg_value) = rbkg {
        params.rbkg = Some(rbkg_value);
    }
    
    if let Some(kmin_value) = kmin {
        params.kmin = Some(kmin_value);
    }
    
    if let Some(kmax_value) = kmax {
        params.kmax = Some(kmax_value);
    }
    
    if let Some(kweight_value) = kweight {
        params.kweight = Some(kweight_value);
    }
    
    if let Some(dk_value) = dk {
        params.dk = Some(dk_value);
    }
    
    if let Some(window_str) = window {
        params.window = Some(match window_str.to_lowercase().as_str() {
            "hanning" => xafsutils::FTWindow::Hanning,
            "sine" => xafsutils::FTWindow::Sine,
            "kaiser-bessel" | "kaiserbessel" => xafsutils::FTWindow::KaiserBessel,
            "gaussian" => xafsutils::FTWindow::Gaussian,
            "parzen" => xafsutils::FTWindow::Parzen,
            "welch" => xafsutils::FTWindow::Welch,
            _ => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                format!("Unknown window type: {}", window_str)
            )),
        });
    }
    
    // Perform background removal
    match background::autobk(energy_arr, mu_arr, &params) {
        Ok(result) => {
            // Create Python dictionary to return results
            let dict = PyDict::new(py);
            
            // Add results to dictionary
            dict.set_item("k", result.k.into_pyarray(py))?;
            dict.set_item("chi", result.chi.into_pyarray(py))?;
            dict.set_item("kraw", result.kraw.into_pyarray(py))?;
            dict.set_item("background", result.background.into_pyarray(py))?;
            
            Ok(dict.into())
        },
        Err(err) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            format!("Background removal error: {:?}", err)
        )),
    }
}

/// Perform forward Fourier transform of EXAFS data
#[pyfunction]
#[pyo3(signature = (k, chi, kmin=None, kmax=None, dk=None, window=None, kweight=None, nfft=None))]
fn xftf(
    py: Python,
    k: PyReadonlyArray1<f64>,
    chi: PyReadonlyArray1<f64>,
    kmin: Option<f64>,
    kmax: Option<f64>,
    dk: Option<f64>,
    window: Option<&str>,
    kweight: Option<f64>,
    nfft: Option<i32>,
) -> PyResult<Py<PyDict>> {
    let k_arr = k.as_array();
    let chi_arr = chi.as_array();
    
    // Create FT parameters
    let mut params = xrayfft::FTParameters::new();
    
    // Set parameters if provided
    if let Some(kmin_value) = kmin {
        params.kmin = Some(kmin_value);
    }
    
    if let Some(kmax_value) = kmax {
        params.kmax = Some(kmax_value);
    }
    
    if let Some(dk_value) = dk {
        params.dk = Some(dk_value);
    }
    
    if let Some(window_str) = window {
        params.window = Some(match window_str.to_lowercase().as_str() {
            "hanning" => xafsutils::FTWindow::Hanning,
            "sine" => xafsutils::FTWindow::Sine,
            "kaiser-bessel" | "kaiserbessel" => xafsutils::FTWindow::KaiserBessel,
            "gaussian" => xafsutils::FTWindow::Gaussian,
            "parzen" => xafsutils::FTWindow::Parzen,
            "welch" => xafsutils::FTWindow::Welch,
            _ => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                format!("Unknown window type: {}", window_str)
            )),
        });
    }
    
    if let Some(kweight_value) = kweight {
        params.kweight = Some(kweight_value);
    }
    
    if let Some(nfft_value) = nfft {
        params.nfft = Some(nfft_value);
    }
    
    // Perform forward FT
    match xrayfft::xftf(k_arr, chi_arr, &params) {
        Ok(result) => {
            // Create Python dictionary to return results
            let dict = PyDict::new(py);
            
            // Add results to dictionary
            dict.set_item("r", result.r.into_pyarray(py))?;
            dict.set_item("chir", result.chir.into_pyarray(py))?; // Complex array
            dict.set_item("chir_mag", result.chir_mag.into_pyarray(py))?;
            dict.set_item("chir_re", result.chir_re.into_pyarray(py))?;
            dict.set_item("chir_im", result.chir_im.into_pyarray(py))?;
            
            Ok(dict.into())
        },
        Err(err) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            format!("Forward Fourier transform error: {:?}", err)
        )),
    }
}

/// Perform reverse Fourier transform of EXAFS data
#[pyfunction]
#[pyo3(signature = (r, chir, rmin=None, rmax=None, dr=None, window=None, kmax_out=None))]
fn xftr(
    py: Python,
    r: PyReadonlyArray1<f64>,
    chir: PyReadonlyArray1<f64>,
    rmin: Option<f64>,
    rmax: Option<f64>,
    dr: Option<f64>,
    window: Option<&str>,
    kmax_out: Option<f64>,
) -> PyResult<Py<PyDict>> {
    let r_arr = r.as_array();
    let chir_arr = chir.as_array();
    
    // Create reverse FT parameters
    let mut params = xrayfft::IFTParameters::new();
    
    // Set parameters if provided
    if let Some(rmin_value) = rmin {
        params.rmin = Some(rmin_value);
    }
    
    if let Some(rmax_value) = rmax {
        params.rmax = Some(rmax_value);
    }
    
    if let Some(dr_value) = dr {
        params.dr = Some(dr_value);
    }
    
    if let Some(window_str) = window {
        params.window = Some(match window_str.to_lowercase().as_str() {
            "hanning" => xafsutils::FTWindow::Hanning,
            "sine" => xafsutils::FTWindow::Sine,
            "kaiser-bessel" | "kaiserbessel" => xafsutils::FTWindow::KaiserBessel,
            "gaussian" => xafsutils::FTWindow::Gaussian,
            "parzen" => xafsutils::FTWindow::Parzen,
            "welch" => xafsutils::FTWindow::Welch,
            _ => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                format!("Unknown window type: {}", window_str)
            )),
        });
    }
    
    if let Some(kmax_out_value) = kmax_out {
        params.kmax_out = Some(kmax_out_value);
    }
    
    // Perform reverse FT
    match xrayfft::xftr(r_arr, chir_arr, &params) {
        Ok(result) => {
            // Create Python dictionary to return results
            let dict = PyDict::new(py);
            
            // Add results to dictionary
            dict.set_item("k", result.k.into_pyarray(py))?;
            dict.set_item("chiq", result.chiq.into_pyarray(py))?;
            dict.set_item("chiq_re", result.chiq_re.into_pyarray(py))?;
            dict.set_item("chiq_im", result.chiq_im.into_pyarray(py))?;
            
            Ok(dict.into())
        },
        Err(err) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            format!("Reverse Fourier transform error: {:?}", err)
        )),
    }
}

/// Fluent API builder for pre_edge function
#[pyclass]
struct PreEdgeBuilder {
    energy: Option<PyReadonlyArray1<f64>>,
    mu: Option<PyReadonlyArray1<f64>>,
    e0: Option<f64>,
    pre1: Option<f64>,
    pre2: Option<f64>,
    norm1: Option<f64>,
    norm2: Option<f64>,
    nnorm: Option<i32>,
}

#[pymethods]
impl PreEdgeBuilder {
    #[new]
    fn new() -> Self {
        Self {
            energy: None,
            mu: None,
            e0: None,
            pre1: None,
            pre2: None,
            norm1: None,
            norm2: None,
            nnorm: None,
        }
    }
    
    fn energy(&mut self, energy: PyReadonlyArray1<f64>) -> PyResult<&Self> {
        self.energy = Some(energy);
        Ok(self)
    }
    
    fn mu(&mut self, mu: PyReadonlyArray1<f64>) -> PyResult<&Self> {
        self.mu = Some(mu);
        Ok(self)
    }
    
    fn e0(&mut self, e0: f64) -> PyResult<&Self> {
        self.e0 = Some(e0);
        Ok(self)
    }
    
    fn pre_range(&mut self, pre1: f64, pre2: f64) -> PyResult<&Self> {
        self.pre1 = Some(pre1);
        self.pre2 = Some(pre2);
        Ok(self)
    }
    
    fn norm_range(&mut self, norm1: f64, norm2: f64) -> PyResult<&Self> {
        self.norm1 = Some(norm1);
        self.norm2 = Some(norm2);
        Ok(self)
    }
    
    fn nnorm(&mut self, nnorm: i32) -> PyResult<&Self> {
        self.nnorm = Some(nnorm);
        Ok(self)
    }
    
    fn run(&self, py: Python) -> PyResult<Py<PyDict>> {
        if self.energy.is_none() || self.mu.is_none() {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "energy and mu must be set"
            ));
        }
        
        pre_edge(
            py,
            self.energy.as_ref().unwrap().clone(),
            self.mu.as_ref().unwrap().clone(),
            self.e0,
            self.pre1,
            self.pre2,
            self.norm1,
            self.norm2,
            self.nnorm,
        )
    }
}

/// Fluent API builder for autobk function
#[pyclass]
struct AutobkBuilder {
    energy: Option<PyReadonlyArray1<f64>>,
    mu: Option<PyReadonlyArray1<f64>>,
    e0: Option<f64>,
    rbkg: Option<f64>,
    kmin: Option<f64>,
    kmax: Option<f64>,
    kweight: Option<f64>,
    dk: Option<f64>,
    window: Option<String>,
}

#[pymethods]
impl AutobkBuilder {
    #[new]
    fn new() -> Self {
        Self {
            energy: None,
            mu: None,
            e0: None,
            rbkg: None,
            kmin: None,
            kmax: None,
            kweight: None,
            dk: None,
            window: None,
        }
    }
    
    fn energy(&mut self, energy: PyReadonlyArray1<f64>) -> PyResult<&Self> {
        self.energy = Some(energy);
        Ok(self)
    }
    
    fn mu(&mut self, mu: PyReadonlyArray1<f64>) -> PyResult<&Self> {
        self.mu = Some(mu);
        Ok(self)
    }
    
    fn e0(&mut self, e0: f64) -> PyResult<&Self> {
        self.e0 = Some(e0);
        Ok(self)
    }
    
    fn rbkg(&mut self, rbkg: f64) -> PyResult<&Self> {
        self.rbkg = Some(rbkg);
        Ok(self)
    }
    
    fn k_range(&mut self, kmin: f64, kmax: f64) -> PyResult<&Self> {
        self.kmin = Some(kmin);
        self.kmax = Some(kmax);
        Ok(self)
    }
    
    fn kweight(&mut self, kweight: f64) -> PyResult<&Self> {
        self.kweight = Some(kweight);
        Ok(self)
    }
    
    fn dk(&mut self, dk: f64) -> PyResult<&Self> {
        self.dk = Some(dk);
        Ok(self)
    }
    
    fn window(&mut self, window: &str) -> PyResult<&Self> {
        self.window = Some(window.to_string());
        Ok(self)
    }
    
    fn run(&self, py: Python) -> PyResult<Py<PyDict>> {
        if self.energy.is_none() || self.mu.is_none() {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "energy and mu must be set"
            ));
        }
        
        autobk(
            py,
            self.energy.as_ref().unwrap().clone(),
            self.mu.as_ref().unwrap().clone(),
            self.e0,
            self.rbkg,
            self.kmin,
            self.kmax,
            self.kweight,
            self.dk,
            self.window.as_deref(),
        )
    }
}

/// Fluent API builder for xftf function
#[pyclass]
struct XftfBuilder {
    k: Option<PyReadonlyArray1<f64>>,
    chi: Option<PyReadonlyArray1<f64>>,
    kmin: Option<f64>,
    kmax: Option<f64>,
    dk: Option<f64>,
    window: Option<String>,
    kweight: Option<f64>,
    nfft: Option<i32>,
}

#[pymethods]
impl XftfBuilder {
    #[new]
    fn new() -> Self {
        Self {
            k: None,
            chi: None,
            kmin: None,
            kmax: None,
            dk: None,
            window: None,
            kweight: None,
            nfft: None,
        }
    }
    
    fn k(&mut self, k: PyReadonlyArray1<f64>) -> PyResult<&Self> {
        self.k = Some(k);
        Ok(self)
    }
    
    fn chi(&mut self, chi: PyReadonlyArray1<f64>) -> PyResult<&Self> {
        self.chi = Some(chi);
        Ok(self)
    }
    
    fn k_range(&mut self, kmin: f64, kmax: f64) -> PyResult<&Self> {
        self.kmin = Some(kmin);
        self.kmax = Some(kmax);
        Ok(self)
    }
    
    fn dk(&mut self, dk: f64) -> PyResult<&Self> {
        self.dk = Some(dk);
        Ok(self)
    }
    
    fn window(&mut self, window: &str) -> PyResult<&Self> {
        self.window = Some(window.to_string());
        Ok(self)
    }
    
    fn kweight(&mut self, kweight: f64) -> PyResult<&Self> {
        self.kweight = Some(kweight);
        Ok(self)
    }
    
    fn nfft(&mut self, nfft: i32) -> PyResult<&Self> {
        self.nfft = Some(nfft);
        Ok(self)
    }
    
    fn run(&self, py: Python) -> PyResult<Py<PyDict>> {
        if self.k.is_none() || self.chi.is_none() {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "k and chi must be set"
            ));
        }
        
        xftf(
            py,
            self.k.as_ref().unwrap().clone(),
            self.chi.as_ref().unwrap().clone(),
            self.kmin,
            self.kmax,
            self.dk,
            self.window.as_deref(),
            self.kweight,
            self.nfft,
        )
    }
}

/// Fluent API builder for xftr function
#[pyclass]
struct XftrBuilder {
    r: Option<PyReadonlyArray1<f64>>,
    chir: Option<PyReadonlyArray1<f64>>,
    rmin: Option<f64>,
    rmax: Option<f64>,
    dr: Option<f64>,
    window: Option<String>,
    kmax_out: Option<f64>,
}

#[pymethods]
impl XftrBuilder {
    #[new]
    fn new() -> Self {
        Self {
            r: None,
            chir: None,
            rmin: None,
            rmax: None,
            dr: None,
            window: None,
            kmax_out: None,
        }
    }
    
    fn r(&mut self, r: PyReadonlyArray1<f64>) -> PyResult<&Self> {
        self.r = Some(r);
        Ok(self)
    }
    
    fn chir(&mut self, chir: PyReadonlyArray1<f64>) -> PyResult<&Self> {
        self.chir = Some(chir);
        Ok(self)
    }
    
    fn r_range(&mut self, rmin: f64, rmax: f64) -> PyResult<&Self> {
        self.rmin = Some(rmin);
        self.rmax = Some(rmax);
        Ok(self)
    }
    
    fn dr(&mut self, dr: f64) -> PyResult<&Self> {
        self.dr = Some(dr);
        Ok(self)
    }
    
    fn window(&mut self, window: &str) -> PyResult<&Self> {
        self.window = Some(window.to_string());
        Ok(self)
    }
    
    fn kmax_out(&mut self, kmax_out: f64) -> PyResult<&Self> {
        self.kmax_out = Some(kmax_out);
        Ok(self)
    }
    
    fn run(&self, py: Python) -> PyResult<Py<PyDict>> {
        if self.r.is_none() || self.chir.is_none() {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "r and chir must be set"
            ));
        }
        
        xftr(
            py,
            self.r.as_ref().unwrap().clone(),
            self.chir.as_ref().unwrap().clone(),
            self.rmin,
            self.rmax,
            self.dr,
            self.window.as_deref(),
            self.kmax_out,
        )
    }
}

/// A Python module implemented in Rust.
#[pymodule]
#[pyo3(name = "pyxraytsubaki")]
fn init_module(_py: Python, m: &PyModule) -> PyResult<()> {
    // Add the classes
    m.add_class::<PyXASSpectrum>()?;
    m.add_class::<PyXASGroup>()?;
    
    // Add fitting classes
    m.add_class::<PyFittingParameter>()?;
    m.add_class::<PyFittingParameters>()?;
    m.add_class::<PySimplePath>()?;
    m.add_class::<PyFittingDataset>()?;
    m.add_class::<PyFitResult>()?;
    m.add_class::<PyExafsFitter>()?;
    
    // Add multi-spectrum fitting classes
    m.add_class::<PyParameterConstraint>()?;
    m.add_class::<PyConstrainedParameter>()?;
    m.add_class::<PyConstrainedParameters>()?;
    m.add_class::<PyMultiSpectrumDataset>()?;
    m.add_class::<PyMultiSpectrumFitResult>()?;
    m.add_class::<PyMultiSpectrumFitter>()?;
    
    // Add the builder classes for fluent API
    m.add_class::<PreEdgeBuilder>()?;
    m.add_class::<AutobkBuilder>()?;
    m.add_class::<XftfBuilder>()?;
    m.add_class::<XftrBuilder>()?;
    
    // Add the functions
    m.add_function(wrap_pyfunction!(find_e0, m)?)?;
    m.add_function(wrap_pyfunction!(pre_edge, m)?)?;
    m.add_function(wrap_pyfunction!(autobk, m)?)?;
    m.add_function(wrap_pyfunction!(xftf, m)?)?;
    m.add_function(wrap_pyfunction!(xftr, m)?)?;
    
    Ok(())
}
