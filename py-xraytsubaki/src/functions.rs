use pyo3::prelude::*;
use pyo3::types::PyDict;
use numpy::{IntoPyArray, PyArray1, PyReadonlyArray1};
use xraytsubaki::prelude::*;
use xraytsubaki::xafs::{normalization, background, xafsutils, xrayfft};
use crate::utils::str_to_window;

/// Find the absorption edge energy (E0) in an XAS spectrum
#[pyfunction]
#[pyo3(signature = (energy, mu))]
pub fn find_e0(energy: PyReadonlyArray1<f64>, mu: PyReadonlyArray1<f64>) -> PyResult<f64> {
    let energy_arr = energy.as_array();
    let mu_arr = mu.as_array();
    
    match xafsutils::find_e0(energy_arr, mu_arr) {
        Ok(e0) => Ok(e0),
        Err(err) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            format!("Error finding E0: {:?}", err)
        )),
    }
}

/// Perform pre-edge subtraction and normalization
#[pyfunction]
#[pyo3(signature = (energy, mu, e0=None, pre1=None, pre2=None, norm1=None, norm2=None, nnorm=None))]
pub fn pre_edge(
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
pub fn autobk(
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
        params.window = Some(str_to_window(window_str)?);
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
pub fn xftf(
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
        params.window = Some(str_to_window(window_str)?);
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
pub fn xftr(
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
        params.window = Some(str_to_window(window_str)?);
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