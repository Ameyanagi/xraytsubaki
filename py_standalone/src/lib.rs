use pyo3::prelude::*;
use numpy::{PyArray1, PyReadonlyArray1};

#[pyfunction]
fn hello() -> String {
    "Hello from Rust!".to_string()
}

#[pyfunction]
fn sum_as_string(a: i64, b: i64) -> String {
    format!("{}", a + b)
}

/// Sample function showing numpy interop
#[pyfunction]
fn add_arrays(py: Python, a: PyReadonlyArray1<f64>, b: PyReadonlyArray1<f64>) -> PyResult<Py<PyArray1<f64>>> {
    let a_array = a.as_array();
    let b_array = b.as_array();
    
    if a_array.shape() != b_array.shape() {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Arrays must have the same shape"));
    }
    
    let mut result = vec![0.0; a_array.len()];
    for i in 0..a_array.len() {
        result[i] = a_array[i] + b_array[i];
    }
    
    Ok(PyArray1::from_vec(py, result).into())
}

/// A Python module implemented in Rust
#[pymodule]
fn py_standalone(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(hello, m)?)?;
    m.add_function(wrap_pyfunction!(sum_as_string, m)?)?;
    m.add_function(wrap_pyfunction!(add_arrays, m)?)?;
    Ok(())
}