use pyo3::prelude::*;
use pyo3::types::PyDict;
use numpy::{PyReadonlyArray1};
use crate::functions::{pre_edge, autobk, xftf, xftr};

/// Fluent API builder for pre_edge function
#[pyclass]
pub struct PreEdgeBuilder {
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
pub struct AutobkBuilder {
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
pub struct XftfBuilder {
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
pub struct XftrBuilder {
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