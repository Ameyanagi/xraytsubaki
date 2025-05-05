use pyo3::prelude::*;

/// A simple demo module to verify PyO3 is working
#[pymodule]
fn py_xraytsubaki(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(hello_world, m)?)?;
    m.add_function(wrap_pyfunction!(sum_as_string, m)?)?;
    Ok(())
}

#[pyfunction]
fn hello_world() -> PyResult<String> {
    Ok("Hello from XRayTsubaki Rust extension!".to_string())
}

#[pyfunction]
fn sum_as_string(a: i64, b: i64) -> PyResult<String> {
    Ok(format!("{}", a + b))
}