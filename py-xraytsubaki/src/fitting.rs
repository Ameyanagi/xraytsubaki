use std::collections::HashMap;

use numpy::{IntoPyArray, PyArray1, PyReadonlyArray1};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use xraytsubaki::prelude::*;
use xraytsubaki::xafs::fitting::{self, FittingParameter, FittingParameters, PathModel, SimplePath, FittingDataset, FitResult, ExafsFitter};
use xraytsubaki::xafs::xafsutils::FTWindow;

/// Python wrapper for FittingParameter
#[pyclass(name = "FittingParameter")]
#[derive(Clone)]
pub struct PyFittingParameter {
    pub parameter: FittingParameter,
}

#[pymethods]
impl PyFittingParameter {
    #[new]
    #[pyo3(signature = (name, value, vary=true, min=None, max=None, expr=None))]
    fn new(
        name: &str,
        value: f64,
        vary: bool,
        min: Option<f64>,
        max: Option<f64>,
        expr: Option<&str>,
    ) -> Self {
        let mut param = FittingParameter::new(name, value);
        param.vary = vary;
        param.min = min;
        param.max = max;
        param.expr = expr.map(|s| s.to_string());
        
        Self { parameter: param }
    }
    
    /// Get the parameter name
    #[getter]
    fn get_name(&self) -> String {
        self.parameter.name.clone()
    }
    
    /// Get the parameter value
    #[getter]
    fn get_value(&self) -> f64 {
        self.parameter.value
    }
    
    /// Set the parameter value
    #[setter]
    fn set_value(&mut self, value: f64) {
        self.parameter.value = value;
    }
    
    /// Get whether the parameter varies during fitting
    #[getter]
    fn get_vary(&self) -> bool {
        self.parameter.vary
    }
    
    /// Set whether the parameter varies during fitting
    #[setter]
    fn set_vary(&mut self, vary: bool) {
        self.parameter.vary = vary;
    }
    
    /// Get the minimum allowed value
    #[getter]
    fn get_min(&self) -> Option<f64> {
        self.parameter.min
    }
    
    /// Set the minimum allowed value
    #[setter]
    fn set_min(&mut self, min: Option<f64>) {
        self.parameter.min = min;
    }
    
    /// Get the maximum allowed value
    #[getter]
    fn get_max(&self) -> Option<f64> {
        self.parameter.max
    }
    
    /// Set the maximum allowed value
    #[setter]
    fn set_max(&mut self, max: Option<f64>) {
        self.parameter.max = max;
    }
    
    /// Get the parameter expression
    #[getter]
    fn get_expr(&self) -> Option<String> {
        self.parameter.expr.clone()
    }
    
    /// Set the parameter expression
    #[setter]
    fn set_expr(&mut self, expr: Option<&str>) {
        self.parameter.expr = expr.map(|s| s.to_string());
    }
    
    /// Get the standard error (uncertainty) after fitting
    #[getter]
    fn get_stderr(&self) -> Option<f64> {
        self.parameter.stderr
    }
    
    /// Return string representation
    fn __str__(&self) -> PyResult<String> {
        let mut result = format!("Parameter({}, value={:.6})", self.parameter.name, self.parameter.value);
        
        if !self.parameter.vary {
            result.push_str(", vary=False");
        }
        
        if let Some(min) = self.parameter.min {
            result.push_str(&format!(", min={:.6}", min));
        }
        
        if let Some(max) = self.parameter.max {
            result.push_str(&format!(", max={:.6}", max));
        }
        
        if let Some(stderr) = self.parameter.stderr {
            result.push_str(&format!(", stderr={:.6}", stderr));
        }
        
        if let Some(expr) = &self.parameter.expr {
            result.push_str(&format!(", expr='{}'", expr));
        }
        
        Ok(result)
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        self.__str__()
    }
}

/// Python wrapper for FittingParameters
#[pyclass(name = "FittingParameters")]
#[derive(Clone)]
pub struct PyFittingParameters {
    pub parameters: FittingParameters,
}

#[pymethods]
impl PyFittingParameters {
    #[new]
    fn new() -> Self {
        Self {
            parameters: FittingParameters::new(),
        }
    }
    
    /// Add a parameter to the collection
    #[pyo3(signature = (name, value, vary=true, min=None, max=None, expr=None))]
    fn add(
        &mut self,
        name: &str,
        value: f64,
        vary: bool,
        min: Option<f64>,
        max: Option<f64>,
        expr: Option<&str>,
    ) -> PyResult<&mut Self> {
        let mut param = FittingParameter::new(name, value);
        param.vary = vary;
        param.min = min;
        param.max = max;
        param.expr = expr.map(|s| s.to_string());
        
        self.parameters.add_parameter(param);
        Ok(self)
    }
    
    /// Get a parameter by name
    fn __getitem__(&self, name: &str) -> PyResult<PyFittingParameter> {
        if let Some(param) = self.parameters.get(name) {
            Ok(PyFittingParameter {
                parameter: param.clone(),
            })
        } else {
            Err(PyValueError::new_err(format!("No parameter named '{}'", name)))
        }
    }
    
    /// Set a parameter value
    fn __setitem__(&mut self, name: &str, value: f64) -> PyResult<()> {
        if let Some(param) = self.parameters.get_mut(name) {
            param.value = value;
            Ok(())
        } else {
            Err(PyValueError::new_err(format!("No parameter named '{}'", name)))
        }
    }
    
    /// Get all parameter names
    fn keys(&self) -> Vec<String> {
        self.parameters.names()
    }
    
    /// Get parameters as a dictionary
    fn as_dict(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        
        for name in self.parameters.names() {
            if let Some(param) = self.parameters.get(&name) {
                dict.set_item(name, param.value)?;
            }
        }
        
        Ok(dict.into())
    }
    
    /// Return string representation
    fn __str__(&self) -> PyResult<String> {
        let mut result = "Parameters:\n".to_string();
        
        for name in self.parameters.names() {
            if let Some(param) = self.parameters.get(&name) {
                result.push_str(&format!("  {}: {:.6}", name, param.value));
                
                if !param.vary {
                    result.push_str(" (fixed)");
                }
                
                if let Some(stderr) = param.stderr {
                    result.push_str(&format!(" ± {:.6}", stderr));
                }
                
                result.push('\n');
            }
        }
        
        Ok(result)
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("FittingParameters(n_params={})", self.parameters.names().len()))
    }
}

/// Python wrapper for SimplePath
#[pyclass(name = "SimplePath")]
#[derive(Clone)]
pub struct PySimplePath {
    pub path: SimplePath,
}

#[pymethods]
impl PySimplePath {
    #[new]
    #[pyo3(signature = (amp_param, r_param, phase_param, sigma2_param, degeneracy=1.0))]
    fn new(
        amp_param: &str,
        r_param: &str,
        phase_param: &str,
        sigma2_param: &str,
        degeneracy: f64,
    ) -> Self {
        Self {
            path: SimplePath::new(amp_param, r_param, phase_param, sigma2_param, degeneracy),
        }
    }
    
    /// Calculate chi(k) for this path
    fn calc_chi<'py>(
        &self,
        py: Python<'py>,
        params: &PyFittingParameters,
        k: PyReadonlyArray1<f64>,
    ) -> PyResult<&'py PyArray1<f64>> {
        let k_arr = k.as_array();
        
        match self.path.calc_chi(&params.parameters, k_arr.as_slice().unwrap()) {
            Ok(chi) => Ok(chi.into_pyarray(py)),
            Err(err) => Err(PyValueError::new_err(format!("Error calculating chi: {:?}", err))),
        }
    }
    
    /// Return string representation
    fn __str__(&self) -> PyResult<String> {
        Ok(format!(
            "SimplePath(amp={}, r={}, phase={}, sigma2={}, degeneracy={})",
            self.path.amp_param,
            self.path.r_param,
            self.path.phase_param,
            self.path.sigma2_param,
            self.path.degeneracy
        ))
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        self.__str__()
    }
}

/// Python wrapper for FittingDataset
#[pyclass(name = "FittingDataset")]
#[derive(Clone)]
pub struct PyFittingDataset {
    pub dataset: FittingDataset,
}

#[pymethods]
impl PyFittingDataset {
    #[new]
    fn new(k: PyReadonlyArray1<f64>, chi: PyReadonlyArray1<f64>) -> Self {
        Self {
            dataset: FittingDataset::new(
                k.as_array().to_owned(),
                chi.as_array().to_owned(),
            ),
        }
    }
    
    /// Add a path to the dataset
    fn add_path(&mut self, path: &PySimplePath) -> PyResult<&mut Self> {
        self.dataset.add_path(path.path.clone());
        Ok(self)
    }
    
    /// Set k-weight for fitting
    fn kweight(&mut self, kweight: f64) -> PyResult<&mut Self> {
        self.dataset.set_kweight(kweight);
        Ok(self)
    }
    
    /// Set k-range for fitting
    fn k_range(&mut self, kmin: f64, kmax: f64) -> PyResult<&mut Self> {
        self.dataset.set_k_range(kmin, kmax);
        Ok(self)
    }
    
    /// Set window function for fitting
    fn window(&mut self, window: &str) -> PyResult<&mut Self> {
        let window_type = match window.to_lowercase().as_str() {
            "hanning" => FTWindow::Hanning,
            "sine" => FTWindow::Sine,
            "kaiser-bessel" | "kaiserbessel" => FTWindow::KaiserBessel,
            "gaussian" => FTWindow::Gaussian,
            "parzen" => FTWindow::Parzen,
            "welch" => FTWindow::Welch,
            _ => return Err(PyValueError::new_err(format!("Unknown window type: {}", window))),
        };
        
        self.dataset.set_window(window_type);
        Ok(self)
    }
    
    /// Calculate model chi(k) with the given parameters
    fn calc_model_chi<'py>(
        &self,
        py: Python<'py>,
        params: &PyFittingParameters,
    ) -> PyResult<&'py PyArray1<f64>> {
        match self.dataset.calc_model_chi(&params.parameters) {
            Ok(model_chi) => Ok(model_chi.into_pyarray(py)),
            Err(err) => Err(PyValueError::new_err(format!("Error calculating model: {:?}", err))),
        }
    }
    
    /// Return string representation
    fn __str__(&self) -> PyResult<String> {
        let k_range = if let Some((kmin, kmax)) = self.dataset.k_range {
            format!("{:.2} to {:.2}", kmin, kmax)
        } else {
            "not set".to_string()
        };
        
        let window = if let Some(window) = &self.dataset.window {
            format!("{:?}", window)
        } else {
            "not set".to_string()
        };
        
        Ok(format!(
            "FittingDataset(n_points={}, n_paths={}, k-weight={:.1}, k-range={}, window={})",
            self.dataset.k.len(),
            self.dataset.paths.len(),
            self.dataset.kweight,
            k_range,
            window
        ))
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        self.__str__()
    }
    
    /// Get the k array
    #[getter]
    fn get_k<'py>(&self, py: Python<'py>) -> &'py PyArray1<f64> {
        self.dataset.k.clone().into_pyarray(py)
    }
    
    /// Get the chi array
    #[getter]
    fn get_chi<'py>(&self, py: Python<'py>) -> &'py PyArray1<f64> {
        self.dataset.chi.clone().into_pyarray(py)
    }
    
    /// Get the k-weight value
    #[getter]
    fn get_kweight(&self) -> f64 {
        self.dataset.kweight
    }
}

/// Python wrapper for FitResult
#[pyclass(name = "FitResult")]
#[derive(Clone)]
pub struct PyFitResult {
    pub result: FitResult,
}

#[pymethods]
impl PyFitResult {
    /// Get the optimized parameters
    #[getter]
    fn get_params(&self) -> PyFittingParameters {
        PyFittingParameters {
            parameters: self.result.params.clone(),
        }
    }
    
    /// Get the model chi(k) calculated with optimized parameters
    #[getter]
    fn get_model_chi<'py>(&self, py: Python<'py>) -> &'py PyArray1<f64> {
        self.result.model_chi.clone().into_pyarray(py)
    }
    
    /// Get the number of data points
    #[getter]
    fn get_ndata(&self) -> usize {
        self.result.ndata
    }
    
    /// Get the number of varying parameters
    #[getter]
    fn get_nvarys(&self) -> usize {
        self.result.nvarys
    }
    
    /// Get the degrees of freedom
    #[getter]
    fn get_nfree(&self) -> usize {
        self.result.nfree
    }
    
    /// Get the chi-square of the fit
    #[getter]
    fn get_chisqr(&self) -> f64 {
        self.result.chisqr
    }
    
    /// Get the reduced chi-square of the fit
    #[getter]
    fn get_redchi(&self) -> f64 {
        self.result.redchi
    }
    
    /// Get the R-factor (goodness of fit)
    #[getter]
    fn get_r_factor(&self) -> f64 {
        self.result.r_factor
    }
    
    /// Return string representation with fit statistics
    fn __str__(&self) -> PyResult<String> {
        let mut result = "Fit Results:\n".to_string();
        
        result.push_str(&format!("  ndata = {}\n", self.result.ndata));
        result.push_str(&format!("  nvarys = {}\n", self.result.nvarys));
        result.push_str(&format!("  nfree = {}\n", self.result.nfree));
        result.push_str(&format!("  chi-square = {:.8}\n", self.result.chisqr));
        result.push_str(&format!("  reduced chi-square = {:.8}\n", self.result.redchi));
        result.push_str(&format!("  R-factor = {:.8}\n", self.result.r_factor));
        
        result.push_str("\nParameters:\n");
        
        for name in self.result.params.names() {
            if let Some(param) = self.result.params.get(&name) {
                result.push_str(&format!("  {}: {:.6}", name, param.value));
                
                if !param.vary {
                    result.push_str(" (fixed)");
                }
                
                if let Some(stderr) = param.stderr {
                    result.push_str(&format!(" ± {:.6}", stderr));
                }
                
                result.push('\n');
            }
        }
        
        Ok(result)
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("FitResult(redchi={:.6}, r_factor={:.6})",
                 self.result.redchi, self.result.r_factor))
    }
}

/// Python wrapper for ExafsFitter
#[pyclass(name = "ExafsFitter")]
pub struct PyExafsFitter {
    dataset: PyFittingDataset,
    params: PyFittingParameters,
}

#[pymethods]
impl PyExafsFitter {
    #[new]
    fn new(dataset: PyFittingDataset, params: Option<PyFittingParameters>) -> Self {
        Self {
            dataset,
            params: params.unwrap_or_else(|| PyFittingParameters::new()),
        }
    }
    
    /// Set the parameters for fitting
    fn params(&mut self, params: PyFittingParameters) -> PyResult<&mut Self> {
        self.params = params;
        Ok(self)
    }
    
    /// Perform the fit
    fn fit(&self) -> PyResult<PyFitResult> {
        let fitter = ExafsFitter::new(&self.dataset.dataset, self.params.parameters.clone());
        
        match fitter.fit() {
            Ok(result) => Ok(PyFitResult { result }),
            Err(err) => Err(PyValueError::new_err(format!("Fit error: {:?}", err))),
        }
    }
    
    /// Return string representation
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("ExafsFitter(dataset={}, n_params={})",
                 self.dataset.__str__()?,
                 self.params.parameters.names().len()))
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("ExafsFitter(n_params={})", self.params.parameters.names().len()))
    }
}