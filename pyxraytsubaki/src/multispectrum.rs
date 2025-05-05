use std::collections::HashMap;

use numpy::{IntoPyArray, PyArray1, PyReadonlyArray1};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use xraytsubaki::prelude::*;
use xraytsubaki::xafs::multispectrum::{ConstrainedParameter, ConstrainedParameters, ParameterConstraint, MultiSpectrumDataset, MultiSpectrumFitter, MultiSpectrumFitResult};
use xraytsubaki::xafs::xafsutils::FTWindow;

use crate::fitting::{PyFittingDataset, PySimplePath};

/// Python wrapper for ParameterConstraint
#[pyclass(name = "ParameterConstraint")]
#[derive(Clone)]
pub struct PyParameterConstraint {
    pub constraint: ParameterConstraint,
}

#[pymethods]
impl PyParameterConstraint {
    /// Create a direct reference constraint
    #[staticmethod]
    fn reference(reference: &str) -> Self {
        Self {
            constraint: ParameterConstraint::Reference(reference.to_string()),
        }
    }
    
    /// Create a scaling constraint
    #[staticmethod]
    fn scale(reference: &str, factor: f64) -> Self {
        Self {
            constraint: ParameterConstraint::Scale {
                reference: reference.to_string(),
                factor,
            },
        }
    }
    
    /// Create an offset constraint
    #[staticmethod]
    fn offset(reference: &str, offset: f64) -> Self {
        Self {
            constraint: ParameterConstraint::Offset {
                reference: reference.to_string(),
                offset,
            },
        }
    }
    
    /// Create a formula constraint
    #[staticmethod]
    fn formula(formula: &str) -> Self {
        Self {
            constraint: ParameterConstraint::Formula(formula.to_string()),
        }
    }
    
    /// Return string representation
    fn __str__(&self) -> PyResult<String> {
        match &self.constraint {
            ParameterConstraint::Reference(reference) => {
                Ok(format!("ParameterConstraint(reference='{}')", reference))
            },
            ParameterConstraint::Scale { reference, factor } => {
                Ok(format!("ParameterConstraint(scale='{}', factor={})", reference, factor))
            },
            ParameterConstraint::Offset { reference, offset } => {
                Ok(format!("ParameterConstraint(offset='{}', offset={})", reference, offset))
            },
            ParameterConstraint::Formula(formula) => {
                Ok(format!("ParameterConstraint(formula='{}')", formula))
            },
        }
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        self.__str__()
    }
}

/// Python wrapper for ConstrainedParameter
#[pyclass(name = "ConstrainedParameter")]
#[derive(Clone)]
pub struct PyConstrainedParameter {
    pub parameter: ConstrainedParameter,
}

#[pymethods]
impl PyConstrainedParameter {
    #[new]
    #[pyo3(signature = (name, value, vary=true, min=None, max=None, constraint=None))]
    fn new(
        name: &str,
        value: f64,
        vary: bool,
        min: Option<f64>,
        max: Option<f64>,
        constraint: Option<&PyParameterConstraint>,
    ) -> Self {
        let mut param = ConstrainedParameter::new(name, value);
        param.param.vary = vary;
        param.param.min = min;
        param.param.max = max;
        
        if let Some(constraint_obj) = constraint {
            param.constraint = Some(constraint_obj.constraint.clone());
        }
        
        Self { parameter: param }
    }
    
    /// Get the parameter name
    #[getter]
    fn get_name(&self) -> String {
        self.parameter.param.name.clone()
    }
    
    /// Get the parameter value
    #[getter]
    fn get_value(&self) -> f64 {
        self.parameter.param.value
    }
    
    /// Set the parameter value
    #[setter]
    fn set_value(&mut self, value: f64) {
        self.parameter.param.value = value;
    }
    
    /// Get whether the parameter varies during fitting
    #[getter]
    fn get_vary(&self) -> bool {
        self.parameter.param.vary
    }
    
    /// Set whether the parameter varies during fitting
    #[setter]
    fn set_vary(&mut self, vary: bool) {
        self.parameter.param.vary = vary;
    }
    
    /// Get the minimum allowed value
    #[getter]
    fn get_min(&self) -> Option<f64> {
        self.parameter.param.min
    }
    
    /// Set the minimum allowed value
    #[setter]
    fn set_min(&mut self, min: Option<f64>) {
        self.parameter.param.min = min;
    }
    
    /// Get the maximum allowed value
    #[getter]
    fn get_max(&self) -> Option<f64> {
        self.parameter.param.max
    }
    
    /// Set the maximum allowed value
    #[setter]
    fn set_max(&mut self, max: Option<f64>) {
        self.parameter.param.max = max;
    }
    
    /// Get the parameter constraint
    #[getter]
    fn get_constraint(&self) -> Option<PyParameterConstraint> {
        self.parameter.constraint.as_ref().map(|c| PyParameterConstraint {
            constraint: c.clone(),
        })
    }
    
    /// Set the parameter to scale from another parameter
    fn scale_from(&mut self, reference: &str, factor: f64) -> PyResult<&mut Self> {
        self.parameter.constraint = Some(ParameterConstraint::Scale {
            reference: reference.to_string(),
            factor,
        });
        self.parameter.param.vary = false;
        Ok(self)
    }
    
    /// Set the parameter to be offset from another parameter
    fn offset_from(&mut self, reference: &str, offset: f64) -> PyResult<&mut Self> {
        self.parameter.constraint = Some(ParameterConstraint::Offset {
            reference: reference.to_string(),
            offset,
        });
        self.parameter.param.vary = false;
        Ok(self)
    }
    
    /// Set the parameter to directly reference another parameter
    fn reference_from(&mut self, reference: &str) -> PyResult<&mut Self> {
        self.parameter.constraint = Some(ParameterConstraint::Reference(reference.to_string()));
        self.parameter.param.vary = false;
        Ok(self)
    }
    
    /// Return string representation
    fn __str__(&self) -> PyResult<String> {
        let mut result = format!(
            "ConstrainedParameter({}, value={:.6})",
            self.parameter.param.name,
            self.parameter.param.value
        );
        
        if !self.parameter.param.vary {
            result.push_str(", vary=False");
        }
        
        if let Some(min) = self.parameter.param.min {
            result.push_str(&format!(", min={:.6}", min));
        }
        
        if let Some(max) = self.parameter.param.max {
            result.push_str(&format!(", max={:.6}", max));
        }
        
        if let Some(constraint) = &self.parameter.constraint {
            match constraint {
                ParameterConstraint::Reference(reference) => {
                    result.push_str(&format!(", reference='{}'", reference));
                },
                ParameterConstraint::Scale { reference, factor } => {
                    result.push_str(&format!(", scale='{}', factor={}", reference, factor));
                },
                ParameterConstraint::Offset { reference, offset } => {
                    result.push_str(&format!(", offset='{}', offset={}", reference, offset));
                },
                ParameterConstraint::Formula(formula) => {
                    result.push_str(&format!(", formula='{}'", formula));
                },
            }
        }
        
        Ok(result)
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        self.__str__()
    }
}

/// Python wrapper for ConstrainedParameters
#[pyclass(name = "ConstrainedParameters")]
#[derive(Clone)]
pub struct PyConstrainedParameters {
    pub parameters: ConstrainedParameters,
}

#[pymethods]
impl PyConstrainedParameters {
    #[new]
    fn new() -> Self {
        Self {
            parameters: ConstrainedParameters::new(),
        }
    }
    
    /// Add a parameter to the collection
    #[pyo3(signature = (name, value, vary=true, min=None, max=None, constraint=None))]
    fn add(
        &mut self,
        name: &str,
        value: f64,
        vary: bool,
        min: Option<f64>,
        max: Option<f64>,
        constraint: Option<&PyParameterConstraint>,
    ) -> PyResult<&mut Self> {
        let mut param = ConstrainedParameter::new(name, value);
        param.param.vary = vary;
        param.param.min = min;
        param.param.max = max;
        
        if let Some(constraint_obj) = constraint {
            param.constraint = Some(constraint_obj.constraint.clone());
        }
        
        self.parameters.add_parameter(param);
        Ok(self)
    }
    
    /// Get a parameter by name
    fn __getitem__(&self, name: &str) -> PyResult<PyConstrainedParameter> {
        if let Some(param) = self.parameters.get(name) {
            Ok(PyConstrainedParameter {
                parameter: param.clone(),
            })
        } else {
            Err(PyValueError::new_err(format!("No parameter named '{}'", name)))
        }
    }
    
    /// Set a parameter value
    fn __setitem__(&mut self, name: &str, value: f64) -> PyResult<()> {
        if let Some(param) = self.parameters.get_mut(name) {
            param.param.value = value;
            Ok(())
        } else {
            Err(PyValueError::new_err(format!("No parameter named '{}'", name)))
        }
    }
    
    /// Apply constraints to update dependent parameters
    fn apply_constraints(&mut self) -> PyResult<&mut Self> {
        match self.parameters.apply_constraints() {
            Ok(_) => Ok(self),
            Err(err) => Err(PyValueError::new_err(format!("Error applying constraints: {:?}", err))),
        }
    }
    
    /// Fluent API for constraining a parameter
    fn constrain(slf: Py<Self>, py: Python, name: &str) -> PyResult<PyConstraintBuilder> {
        let params = slf.borrow(py);
        if params.parameters.get(name).is_none() {
            return Err(PyValueError::new_err(format!("No parameter named '{}'", name)));
        }
        
        Ok(PyConstraintBuilder {
            parameters: slf,
            target_name: name.to_string(),
        })
    }
    
    /// Get all parameter names
    fn keys(&self) -> Vec<String> {
        self.parameters.names()
    }
    
    /// Return string representation
    fn __str__(&self) -> PyResult<String> {
        let mut result = "ConstrainedParameters:\n".to_string();
        
        for name in self.parameters.names() {
            if let Some(param) = self.parameters.get(&name) {
                result.push_str(&format!("  {}: {:.6}", name, param.param.value));
                
                if !param.param.vary {
                    result.push_str(" (fixed)");
                }
                
                if let Some(constraint) = &param.constraint {
                    match constraint {
                        ParameterConstraint::Reference(reference) => {
                            result.push_str(&format!(", reference='{}'", reference));
                        },
                        ParameterConstraint::Scale { reference, factor } => {
                            result.push_str(&format!(", scale='{}', factor={}", reference, factor));
                        },
                        ParameterConstraint::Offset { reference, offset } => {
                            result.push_str(&format!(", offset='{}', offset={}", reference, offset));
                        },
                        ParameterConstraint::Formula(formula) => {
                            result.push_str(&format!(", formula='{}'", formula));
                        },
                    }
                }
                
                result.push('\n');
            }
        }
        
        Ok(result)
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("ConstrainedParameters(n_params={})", self.parameters.names().len()))
    }
}

/// Helper class for fluent constraint API
#[pyclass]
pub struct PyConstraintBuilder {
    parameters: Py<PyConstrainedParameters>,
    target_name: String,
}

#[pymethods]
impl PyConstraintBuilder {
    /// Set a direct reference constraint
    fn reference_from(&mut self, py: Python, reference: &str) -> PyResult<Py<PyConstrainedParameters>> {
        let mut params = self.parameters.borrow_mut(py);
        if let Some(param) = params.parameters.get_mut(&self.target_name) {
            param.constraint = Some(ParameterConstraint::Reference(reference.to_string()));
            param.param.vary = false;
            Ok(self.parameters.clone())
        } else {
            Err(PyValueError::new_err(format!("No parameter named '{}'", self.target_name)))
        }
    }
    
    /// Set a scaling constraint
    fn scale_from(&mut self, py: Python, reference: &str, factor_param: &str) -> PyResult<Py<PyConstrainedParameters>> {
        let mut params = self.parameters.borrow_mut(py);
        
        // Get the factor value first
        let factor = {
            let factor_param_obj = params.parameters.get(factor_param)
                .ok_or_else(|| PyValueError::new_err(format!("No parameter named '{}'", factor_param)))?;
            factor_param_obj.param.value
        };
        
        // Now set the constraint
        if let Some(param) = params.parameters.get_mut(&self.target_name) {
            param.constraint = Some(ParameterConstraint::Scale {
                reference: reference.to_string(),
                factor,
            });
            param.param.vary = false;
            Ok(self.parameters.clone())
        } else {
            Err(PyValueError::new_err(format!("No parameter named '{}'", self.target_name)))
        }
    }
    
    /// Set an offset constraint
    fn offset_from(&mut self, py: Python, reference: &str, offset_param: &str) -> PyResult<Py<PyConstrainedParameters>> {
        let mut params = self.parameters.borrow_mut(py);
        
        // Get the offset value first
        let offset = {
            let offset_param_obj = params.parameters.get(offset_param)
                .ok_or_else(|| PyValueError::new_err(format!("No parameter named '{}'", offset_param)))?;
            offset_param_obj.param.value
        };
        
        // Now set the constraint
        if let Some(param) = params.parameters.get_mut(&self.target_name) {
            param.constraint = Some(ParameterConstraint::Offset {
                reference: reference.to_string(),
                offset,
            });
            param.param.vary = false;
            Ok(self.parameters.clone())
        } else {
            Err(PyValueError::new_err(format!("No parameter named '{}'", self.target_name)))
        }
    }
}

/// Python wrapper for MultiSpectrumDataset
#[pyclass(name = "MultiSpectrumDataset")]
#[derive(Clone)]
pub struct PyMultiSpectrumDataset {
    pub dataset: MultiSpectrumDataset,
}

#[pymethods]
impl PyMultiSpectrumDataset {
    #[new]
    fn new() -> Self {
        Self {
            dataset: MultiSpectrumDataset::new(),
        }
    }
    
    /// Add a dataset to the collection
    fn add_dataset(&mut self, dataset: &PyFittingDataset) -> PyResult<&mut Self> {
        self.dataset.add_dataset(dataset.dataset.clone());
        Ok(self)
    }
    
    /// Set constrained parameters
    fn params(&mut self, params: PyConstrainedParameters) -> PyResult<&mut Self> {
        self.dataset.params = params.parameters.clone();
        Ok(self)
    }
    
    /// Calculate model chi(k) for all spectra with current parameters
    fn calc_all_models<'py>(&self, py: Python<'py>) -> PyResult<Vec<&'py PyArray1<f64>>> {
        match self.dataset.calc_all_models() {
            Ok(models) => {
                let mut py_models = Vec::with_capacity(models.len());
                for model in models {
                    py_models.push(model.into_pyarray(py));
                }
                Ok(py_models)
            },
            Err(err) => Err(PyValueError::new_err(format!("Error calculating models: {:?}", err))),
        }
    }
    
    /// Return string representation
    fn __str__(&self) -> PyResult<String> {
        Ok(format!(
            "MultiSpectrumDataset(n_datasets={}, n_params={})",
            self.dataset.datasets.len(),
            self.dataset.params.names().len()
        ))
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        self.__str__()
    }
}

/// Python wrapper for MultiSpectrumFitResult
#[pyclass(name = "MultiSpectrumFitResult")]
#[derive(Clone)]
pub struct PyMultiSpectrumFitResult {
    pub result: MultiSpectrumFitResult,
}

#[pymethods]
impl PyMultiSpectrumFitResult {
    /// Get the optimized parameters
    #[getter]
    fn get_params(&self) -> PyConstrainedParameters {
        PyConstrainedParameters {
            parameters: self.result.params.clone(),
        }
    }
    
    /// Get the model chi(k) for each spectrum
    #[getter]
    fn get_model_chis<'py>(&self, py: Python<'py>) -> Vec<&'py PyArray1<f64>> {
        self.result.model_chis
            .iter()
            .map(|model| model.clone().into_pyarray(py))
            .collect()
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
    
    /// Get the R-factors for each spectrum
    #[getter]
    fn get_r_factors(&self) -> Vec<f64> {
        self.result.r_factors.clone()
    }
    
    /// Return string representation with fit statistics
    fn __str__(&self) -> PyResult<String> {
        let mut result = "MultiSpectrumFitResult:\n".to_string();
        
        result.push_str(&format!("  ndata = {}\n", self.result.ndata));
        result.push_str(&format!("  nvarys = {}\n", self.result.nvarys));
        result.push_str(&format!("  nfree = {}\n", self.result.nfree));
        result.push_str(&format!("  chi-square = {:.8}\n", self.result.chisqr));
        result.push_str(&format!("  reduced chi-square = {:.8}\n", self.result.redchi));
        
        result.push_str("\nR-factors for each spectrum:\n");
        for (i, r_factor) in self.result.r_factors.iter().enumerate() {
            result.push_str(&format!("  Spectrum {}: {:.8}\n", i + 1, r_factor));
        }
        
        result.push_str("\nParameters:\n");
        
        for name in self.result.params.names() {
            if let Some(param) = self.result.params.get(&name) {
                result.push_str(&format!("  {}: {:.6}", name, param.param.value));
                
                if !param.param.vary {
                    result.push_str(" (fixed)");
                }
                
                if let Some(stderr) = param.param.stderr {
                    result.push_str(&format!(" ± {:.6}", stderr));
                }
                
                if let Some(constraint) = &param.constraint {
                    match constraint {
                        ParameterConstraint::Reference(reference) => {
                            result.push_str(&format!(", reference='{}'", reference));
                        },
                        ParameterConstraint::Scale { reference, factor } => {
                            result.push_str(&format!(", scale='{}', factor={}", reference, factor));
                        },
                        ParameterConstraint::Offset { reference, offset } => {
                            result.push_str(&format!(", offset='{}', offset={}", reference, offset));
                        },
                        ParameterConstraint::Formula(formula) => {
                            result.push_str(&format!(", formula='{}'", formula));
                        },
                    }
                }
                
                result.push('\n');
            }
        }
        
        Ok(result)
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("MultiSpectrumFitResult(redchi={:.6}, n_spectra={})",
                 self.result.redchi, self.result.r_factors.len()))
    }
}

/// Python wrapper for MultiSpectrumFitter
#[pyclass(name = "MultiSpectrumFitter")]
pub struct PyMultiSpectrumFitter {
    dataset: PyMultiSpectrumDataset,
}

#[pymethods]
impl PyMultiSpectrumFitter {
    #[new]
    fn new(dataset: PyMultiSpectrumDataset) -> Self {
        Self { dataset }
    }
    
    /// Perform the fit
    fn fit(&self) -> PyResult<PyMultiSpectrumFitResult> {
        let fitter = MultiSpectrumFitter::new(&self.dataset.dataset);
        
        match fitter.fit() {
            Ok(result) => Ok(PyMultiSpectrumFitResult { result }),
            Err(err) => Err(PyValueError::new_err(format!("Fit error: {:?}", err))),
        }
    }
    
    /// Return string representation
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("MultiSpectrumFitter(dataset={})", self.dataset.__str__()?))
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("MultiSpectrumFitter(n_datasets={})", self.dataset.dataset.datasets.len()))
    }
}