use pyo3::prelude::*;

// Module declarations
pub mod utils;
pub mod functions;
pub mod builders;
pub mod xasgroup;
pub mod xasspectrum;
pub mod fitting;
pub mod multispectrum;

// Imports from functions module
use crate::functions::{find_e0, pre_edge, autobk, xftf, xftr};

// Imports from builders module
use crate::builders::{PreEdgeBuilder, AutobkBuilder, XftfBuilder, XftrBuilder};

// Imports from xasgroup and xasspectrum modules
use crate::xasgroup::PyXASGroup;
use crate::xasspectrum::PyXASSpectrum;

// Imports from fitting module
use crate::fitting::{
    PyFittingParameter, 
    PyFittingParameters, 
    PySimplePath, 
    PyFittingDataset, 
    PyFitResult, 
    PyExafsFitter
};

// Imports from multispectrum module
use crate::multispectrum::{
    PyParameterConstraint, 
    PyConstrainedParameter, 
    PyConstrainedParameters, 
    PyMultiSpectrumDataset, 
    PyMultiSpectrumFitResult, 
    PyMultiSpectrumFitter
};

/// A Python module implemented in Rust.
#[pymodule]
#[pyo3(name = "py_xraytsubaki")]
fn init_module(py: Python, m: &PyModule) -> PyResult<()> {
    // Add the XAS classes
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
    
    // Add the standalone functions
    m.add_function(wrap_pyfunction!(find_e0, m)?)?;
    m.add_function(wrap_pyfunction!(pre_edge, m)?)?;
    m.add_function(wrap_pyfunction!(autobk, m)?)?;
    m.add_function(wrap_pyfunction!(xftf, m)?)?;
    m.add_function(wrap_pyfunction!(xftr, m)?)?;
    
    Ok(())
}