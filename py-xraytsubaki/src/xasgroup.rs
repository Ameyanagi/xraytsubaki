use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::types::{PyDict, PyList};
use xraytsubaki::prelude::*;
use xraytsubaki::xafs::normalization;
use xraytsubaki::xafs::background;
use xraytsubaki::xafs::xrayfft;

use crate::xasspectrum::PyXASSpectrum;

/// Python wrapper for XASGroup
#[pyclass(name = "XASGroup")]
#[derive(Clone)]
pub struct PyXASGroup {
    pub xasgroup: XASGroup,
}

#[pymethods]
impl PyXASGroup {
    #[new]
    pub fn new() -> PyResult<Self> {
        Ok(PyXASGroup {
            xasgroup: XASGroup::new(),
        })
    }
    
    /// Add a spectrum to the group
    #[pyo3(text_signature = "(spectrum)")]
    fn add_spectrum(&mut self, spectrum: &PyXASSpectrum) -> PyResult<&mut Self> {
        self.xasgroup.add_spectrum(spectrum.xasspectrum.clone());
        Ok(self)
    }
    
    /// Fluent API for adding a spectrum
    fn add(&mut self, spectrum: &PyXASSpectrum) -> PyResult<&mut Self> {
        self.add_spectrum(spectrum)
    }
    
    /// Add multiple spectra to the group
    #[pyo3(text_signature = "(spectra)")]
    fn add_spectra(&mut self, spectra: Vec<&PyXASSpectrum>) -> PyResult<&mut Self> {
        for spectrum in spectra {
            self.xasgroup.add_spectrum(spectrum.xasspectrum.clone());
        }
        Ok(self)
    }
    
    /// Get the number of spectra in the group
    fn __len__(&self) -> PyResult<usize> {
        Ok(self.xasgroup.spectra.len())
    }
    
    /// Get a spectrum by index or name
    fn __getitem__(&self, py: Python, index: &PyAny) -> PyResult<PyXASSpectrum> {
        if let Ok(idx) = index.extract::<usize>() {
            // Access by index
            if idx < self.xasgroup.spectra.len() {
                return Ok(PyXASSpectrum {
                    xasspectrum: self.xasgroup.spectra[idx].clone(),
                });
            } else {
                return Err(PyValueError::new_err(format!("Index {} out of range", idx)));
            }
        } else if let Ok(name) = index.extract::<&str>() {
            // Access by name
            for spectrum in &self.xasgroup.spectra {
                if spectrum.name == name {
                    return Ok(PyXASSpectrum {
                        xasspectrum: spectrum.clone(),
                    });
                }
            }
            return Err(PyValueError::new_err(format!("No spectrum with name '{}'", name)));
        }
        
        Err(PyValueError::new_err("Index must be an integer or string"))
    }
    
    /// Return string representation
    fn __str__(&self) -> PyResult<String> {
        let mut result = format!("XASGroup with {} spectra:", self.xasgroup.spectra.len());
        
        for (i, spectrum) in self.xasgroup.spectra.iter().enumerate() {
            result.push_str(&format!("\n  {}: {}", i, spectrum.name));
        }
        
        Ok(result)
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("XASGroup(n_spectra={})", self.xasgroup.spectra.len()))
    }
    
    /// Normalize all spectra in the group
    #[pyo3(signature = (e0=None, pre1=None, pre2=None, norm1=None, norm2=None, nnorm=None))]
    fn normalize_all(
        &mut self,
        e0: Option<f64>,
        pre1: Option<f64>,
        pre2: Option<f64>,
        norm1: Option<f64>,
        norm2: Option<f64>,
        nnorm: Option<i32>,
    ) -> PyResult<&mut Self> {
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
        
        // Apply normalization to all spectra
        for spectrum in &mut self.xasgroup.spectra {
            match spectrum.normalize(&params) {
                Ok(_) => {},
                Err(err) => return Err(PyValueError::new_err(
                    format!("Normalization error for spectrum '{}': {:?}", spectrum.name, err)
                )),
            }
        }
        
        Ok(self)
    }
    
    /// Perform background removal on all spectra in the group
    #[pyo3(signature = (rbkg=None, e0=None, kmin=None, kmax=None, kweight=None, dk=None, window=None))]
    fn autobk_all(
        &mut self,
        rbkg: Option<f64>,
        e0: Option<f64>,
        kmin: Option<f64>,
        kmax: Option<f64>,
        kweight: Option<f64>,
        dk: Option<f64>,
        window: Option<&str>,
    ) -> PyResult<&mut Self> {
        // Create background parameters
        let mut params = background::BackgroundParameters::new();
        
        // Set parameters if provided
        if let Some(rbkg_value) = rbkg {
            params.rbkg = Some(rbkg_value);
        }
        
        if let Some(e0_value) = e0 {
            params.e0 = Some(e0_value);
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
        
        // Apply background removal to all spectra
        for spectrum in &mut self.xasgroup.spectra {
            match spectrum.autobk(&params) {
                Ok(_) => {},
                Err(err) => return Err(PyValueError::new_err(
                    format!("Background removal error for spectrum '{}': {:?}", spectrum.name, err)
                )),
            }
        }
        
        Ok(self)
    }
    
    /// Perform forward Fourier transform on all spectra in the group
    #[pyo3(signature = (kmin=None, kmax=None, dk=None, window=None, kweight=None, nfft=None))]
    fn xftf_all(
        &mut self,
        kmin: Option<f64>,
        kmax: Option<f64>,
        dk: Option<f64>,
        window: Option<&str>,
        kweight: Option<f64>,
        nfft: Option<i32>,
    ) -> PyResult<&mut Self> {
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
        
        if let Some(kweight_value) = kweight {
            params.kweight = Some(kweight_value);
        }
        
        if let Some(nfft_value) = nfft {
            params.nfft = Some(nfft_value);
        }
        
        // Apply forward FT to all spectra
        for spectrum in &mut self.xasgroup.spectra {
            match spectrum.xftf(&params) {
                Ok(_) => {},
                Err(err) => return Err(PyValueError::new_err(
                    format!("Forward Fourier transform error for spectrum '{}': {:?}", spectrum.name, err)
                )),
            }
        }
        
        Ok(self)
    }
    
    /// Perform reverse Fourier transform on all spectra in the group
    #[pyo3(signature = (rmin=None, rmax=None, dr=None, window=None, kmax_out=None))]
    fn xftr_all(
        &mut self,
        rmin: Option<f64>,
        rmax: Option<f64>,
        dr: Option<f64>,
        window: Option<&str>,
        kmax_out: Option<f64>,
    ) -> PyResult<&mut Self> {
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
        
        if let Some(kmax_out_value) = kmax_out {
            params.kmax_out = Some(kmax_out_value);
        }
        
        // Apply reverse FT to all spectra
        for spectrum in &mut self.xasgroup.spectra {
            match spectrum.xftr(&params) {
                Ok(_) => {},
                Err(err) => return Err(PyValueError::new_err(
                    format!("Reverse Fourier transform error for spectrum '{}': {:?}", spectrum.name, err)
                )),
            }
        }
        
        Ok(self)
    }
    
    /// Execute the current operation (used in the fluent API)
    fn run(&mut self) -> PyResult<&mut Self> {
        // This is a placeholder for the fluent API
        // In a real implementation, we would apply any pending operations
        Ok(self)
    }
}
