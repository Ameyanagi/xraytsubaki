use xraytsubaki::prelude::*;

#[pyclass]
#[repr(transparent)]
#[derive(Clone)]
pub struct PyXASGroup {
    pub xasgroup: XASGroup,
}
