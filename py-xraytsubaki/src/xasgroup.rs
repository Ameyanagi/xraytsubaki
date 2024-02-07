use pyo3::prelude::*;
use xraytsubaki::prelude::*;

#[pyclass]
#[repr(transparent)]
#[derive(Clone)]
pub struct PyXASGroup {
    pub xasgroup: XASGroup,
}

#[pymethods]
#[allow(clippy::should_implement_trait)]
impl PyXASGroup {
    #[new]
    pub fn new() -> PyResult<Self> {
        Ok(PyXASGroup {
            xasgroup: XASGroup::new(),
        })
    }
}
