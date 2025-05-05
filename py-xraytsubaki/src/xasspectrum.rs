use std::mem;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use numpy::{IntoPyArray, PyArray1, PyReadonlyArray1};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};
use xraytsubaki::{prelude::*, xafs::background, xafs::normalization, xafs::xafsutils, xafs::xrayfft, xafs::io};

/// Python wrapper for XASSpectrum
#[pyclass(name = "XASSpectrum")]
#[derive(Clone)]
pub struct PyXASSpectrum {
    pub xasspectrum: XASSpectrum,
}

#[pymethods]
impl PyXASSpectrum {
    #[new]
    #[pyo3(signature = (energy = None, mu = None, name = None))]
    pub fn new(
        energy: Option<PyReadonlyArray1<f64>>,
        mu: Option<PyReadonlyArray1<f64>>,
        name: Option<&str>,
    ) -> PyResult<Self> {
        let mut xas_spectrum = XASSpectrum::new();

        if let Some(energy_array) = energy {
            xas_spectrum.raw_energy = Some(energy_array.as_array().to_owned());
        }
        
        if let Some(mu_array) = mu {
            xas_spectrum.raw_mu = Some(mu_array.as_array().to_owned());
        }
        
        if let Some(name_str) = name {
            xas_spectrum.name = name_str.to_string();
        }
        
        Ok(PyXASSpectrum {
            xasspectrum: xas_spectrum,
        })
    }
    
    /// Get the name of the spectrum
    #[getter]
    fn get_name(&self) -> String {
        self.xasspectrum.name.clone()
    }
    
    /// Set the name of the spectrum
    #[setter]
    fn set_name(&mut self, name: &str) {
        self.xasspectrum.name = name.to_string();
    }
    
    /// Get the energy array
    #[getter]
    fn get_energy<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.raw_energy {
            Some(energy) => Ok(Some(energy.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Set the energy array
    #[setter]
    fn set_energy(&mut self, energy: PyReadonlyArray1<f64>) -> PyResult<()> {
        self.xasspectrum.raw_energy = Some(energy.as_array().to_owned());
        Ok(())
    }
    
    /// Fluent API for setting energy
    fn energy(&mut self, energy: PyReadonlyArray1<f64>) -> PyResult<&mut Self> {
        self.set_energy(energy)?;
        Ok(self)
    }
    
    /// Get the mu (absorption) array
    #[getter]
    fn get_mu<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.raw_mu {
            Some(mu) => Ok(Some(mu.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Set the mu (absorption) array
    #[setter]
    fn set_mu(&mut self, mu: PyReadonlyArray1<f64>) -> PyResult<()> {
        self.xasspectrum.raw_mu = Some(mu.as_array().to_owned());
        Ok(())
    }
    
    /// Fluent API for setting mu
    fn mu(&mut self, mu: PyReadonlyArray1<f64>) -> PyResult<&mut Self> {
        self.set_mu(mu)?;
        Ok(self)
    }
    
    /// Get the normalized spectrum
    #[getter]
    fn get_norm<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.norm {
            Some(norm) => Ok(Some(norm.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Get the edge energy (E0)
    #[getter]
    fn get_e0(&self) -> PyResult<Option<f64>> {
        Ok(self.xasspectrum.e0)
    }
    
    /// Set the edge energy (E0)
    #[setter]
    fn set_e0(&mut self, e0: f64) -> PyResult<()> {
        self.xasspectrum.e0 = Some(e0);
        Ok(())
    }
    
    /// Get the edge step
    #[getter]
    fn get_edge_step(&self) -> PyResult<Option<f64>> {
        Ok(self.xasspectrum.edge_step)
    }
    
    /// Get the pre-edge line
    #[getter]
    fn get_pre_edge<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.pre_edge {
            Some(pre_edge) => Ok(Some(pre_edge.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Get the post-edge line
    #[getter]
    fn get_post_edge<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.post_edge {
            Some(post_edge) => Ok(Some(post_edge.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Get the k array
    #[getter]
    fn get_k<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.k {
            Some(k) => Ok(Some(k.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Get the chi array (background-subtracted EXAFS)
    #[getter]
    fn get_chi<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.chi {
            Some(chi) => Ok(Some(chi.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Get the background function
    #[getter]
    fn get_bkg<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.bkg {
            Some(bkg) => Ok(Some(bkg.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Get the R array from Fourier transform
    #[getter]
    fn get_r<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.r {
            Some(r) => Ok(Some(r.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Get the complex chi(R) from Fourier transform
    #[getter]
    fn get_chir<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.chir {
            Some(chir) => {
                // In Rust, we store complex numbers as interleaved real/imag pairs
                // For Python, we need to convert to a complex array
                Ok(Some(chir.to_owned().into_pyarray(py)))
            },
            None => Ok(None),
        }
    }
    
    /// Get the magnitude of chi(R)
    #[getter]
    fn get_chir_mag<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.chir_mag {
            Some(chir_mag) => Ok(Some(chir_mag.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Get the real part of chi(R)
    #[getter]
    fn get_chir_re<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.chir_re {
            Some(chir_re) => Ok(Some(chir_re.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Get the imaginary part of chi(R)
    #[getter]
    fn get_chir_im<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.chir_im {
            Some(chir_im) => Ok(Some(chir_im.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Get the q array from reverse Fourier transform
    #[getter]
    fn get_q<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.q {
            Some(q) => Ok(Some(q.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Get the chi(q) from reverse Fourier transform
    #[getter]
    fn get_chiq<'py>(&self, py: Python<'py>) -> PyResult<Option<&'py PyArray1<f64>>> {
        match &self.xasspectrum.chiq {
            Some(chiq) => Ok(Some(chiq.to_owned().into_pyarray(py))),
            None => Ok(None),
        }
    }
    
    /// Normalize the spectrum
    #[pyo3(signature = (e0=None, pre1=None, pre2=None, norm1=None, norm2=None, nnorm=None))]
    fn normalize(
        &mut self,
        e0: Option<f64>,
        pre1: Option<f64>,
        pre2: Option<f64>,
        norm1: Option<f64>,
        norm2: Option<f64>,
        nnorm: Option<i32>,
    ) -> PyResult<&mut Self> {
        // Check if we have energy and mu data
        if self.xasspectrum.raw_energy.is_none() || self.xasspectrum.raw_mu.is_none() {
            return Err(PyValueError::new_err("Energy and mu data must be set before normalization"));
        }
        
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
        
        // Perform normalization
        match self.xasspectrum.normalize(&params) {
            Ok(_) => Ok(self),
            Err(err) => Err(PyValueError::new_err(format!("Normalization error: {:?}", err))),
        }
    }
    
    /// Fluent API for setting pre-edge range
    fn pre_range(&mut self, pre1: f64, pre2: f64) -> PyResult<&mut Self> {
        self.xasspectrum.normalization_params.pre_edge_start = Some(pre1);
        self.xasspectrum.normalization_params.pre_edge_end = Some(pre2);
        Ok(self)
    }
    
    /// Fluent API for setting normalization range
    fn norm_range(&mut self, norm1: f64, norm2: f64) -> PyResult<&mut Self> {
        self.xasspectrum.normalization_params.norm_start = Some(norm1);
        self.xasspectrum.normalization_params.norm_end = Some(norm2);
        Ok(self)
    }
    
    /// Fluent API for setting normalization polynomial order
    fn nnorm(&mut self, nnorm: i32) -> PyResult<&mut Self> {
        self.xasspectrum.normalization_params.norm_polyorder = Some(nnorm);
        Ok(self)
    }
    
    /// Apply background removal to extract EXAFS signal
    #[pyo3(signature = (rbkg=None, e0=None, kmin=None, kmax=None, kweight=None, dk=None, window=None))]
    fn autobk(
        &mut self,
        rbkg: Option<f64>,
        e0: Option<f64>,
        kmin: Option<f64>,
        kmax: Option<f64>,
        kweight: Option<f64>,
        dk: Option<f64>,
        window: Option<&str>,
    ) -> PyResult<&mut Self> {
        // Check if normalization has been done
        if self.xasspectrum.norm.is_none() {
            return Err(PyValueError::new_err("Spectrum must be normalized before background removal"));
        }
        
        // Create background parameters
        let mut params = background::BackgroundParameters::new();
        
        // Set parameters if provided
        if let Some(rbkg_value) = rbkg {
            params.rbkg = Some(rbkg_value);
        }
        
        if let Some(e0_value) = e0 {
            params.e0 = Some(e0_value);
        } else {
            // Use already determined e0 if available
            params.e0 = self.xasspectrum.e0;
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
            params.window = Some(match window_str.to_lowercase().as_str() {
                "hanning" => xafsutils::FTWindow::Hanning,
                "sine" => xafsutils::FTWindow::Sine,
                "kaiser-bessel" | "kaiserbessel" => xafsutils::FTWindow::KaiserBessel,
                "gaussian" => xafsutils::FTWindow::Gaussian,
                "parzen" => xafsutils::FTWindow::Parzen,
                "welch" => xafsutils::FTWindow::Welch,
                _ => return Err(PyValueError::new_err(format!("Unknown window type: {}", window_str))),
            });
        }
        
        // Perform background removal
        match self.xasspectrum.autobk(&params) {
            Ok(_) => Ok(self),
            Err(err) => Err(PyValueError::new_err(format!("Background removal error: {:?}", err))),
        }
    }
    
    /// Fluent API for setting rbkg
    fn rbkg(&mut self, rbkg: f64) -> PyResult<&mut Self> {
        self.xasspectrum.background_params.rbkg = Some(rbkg);
        Ok(self)
    }
    
    /// Fluent API for setting k range
    fn k_range(&mut self, kmin: f64, kmax: f64) -> PyResult<&mut Self> {
        self.xasspectrum.background_params.kmin = Some(kmin);
        self.xasspectrum.background_params.kmax = Some(kmax);
        Ok(self)
    }
    
    /// Fluent API for setting kweight
    fn kweight(&mut self, kweight: f64) -> PyResult<&mut Self> {
        self.xasspectrum.background_params.kweight = Some(kweight);
        Ok(self)
    }
    
    /// Fluent API for setting window
    fn window(&mut self, window: &str) -> PyResult<&mut Self> {
        self.xasspectrum.background_params.window = Some(match window.to_lowercase().as_str() {
            "hanning" => xafsutils::FTWindow::Hanning,
            "sine" => xafsutils::FTWindow::Sine,
            "kaiser-bessel" | "kaiserbessel" => xafsutils::FTWindow::KaiserBessel,
            "gaussian" => xafsutils::FTWindow::Gaussian,
            "parzen" => xafsutils::FTWindow::Parzen,
            "welch" => xafsutils::FTWindow::Welch,
            _ => return Err(PyValueError::new_err(format!("Unknown window type: {}", window))),
        });
        Ok(self)
    }
    
    /// Perform forward Fourier transform to get chi(R)
    #[pyo3(signature = (kmin=None, kmax=None, dk=None, window=None, kweight=None, nfft=None))]
    fn xftf(
        &mut self,
        kmin: Option<f64>,
        kmax: Option<f64>,
        dk: Option<f64>,
        window: Option<&str>,
        kweight: Option<f64>,
        nfft: Option<i32>,
    ) -> PyResult<&mut Self> {
        // Check if chi data is available
        if self.xasspectrum.chi.is_none() || self.xasspectrum.k.is_none() {
            return Err(PyValueError::new_err("Background removal must be done before Fourier transform"));
        }
        
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
            params.window = Some(match window_str.to_lowercase().as_str() {
                "hanning" => xafsutils::FTWindow::Hanning,
                "sine" => xafsutils::FTWindow::Sine,
                "kaiser-bessel" | "kaiserbessel" => xafsutils::FTWindow::KaiserBessel,
                "gaussian" => xafsutils::FTWindow::Gaussian,
                "parzen" => xafsutils::FTWindow::Parzen,
                "welch" => xafsutils::FTWindow::Welch,
                _ => return Err(PyValueError::new_err(format!("Unknown window type: {}", window_str))),
            });
        }
        
        if let Some(kweight_value) = kweight {
            params.kweight = Some(kweight_value);
        }
        
        if let Some(nfft_value) = nfft {
            params.nfft = Some(nfft_value);
        }
        
        // Perform forward FT
        match self.xasspectrum.xftf(&params) {
            Ok(_) => Ok(self),
            Err(err) => Err(PyValueError::new_err(format!("Forward Fourier transform error: {:?}", err))),
        }
    }
    
    /// Fluent API for setting FT dk parameter
    fn dk(&mut self, dk: f64) -> PyResult<&mut Self> {
        self.xasspectrum.ft_params.dk = Some(dk);
        Ok(self)
    }
    
    /// Perform reverse Fourier transform to get filtered chi(k)
    #[pyo3(signature = (rmin=None, rmax=None, dr=None, window=None, kmax_out=None))]
    fn xftr(
        &mut self,
        rmin: Option<f64>,
        rmax: Option<f64>,
        dr: Option<f64>,
        window: Option<&str>,
        kmax_out: Option<f64>,
    ) -> PyResult<&mut Self> {
        // Check if chi(R) data is available
        if self.xasspectrum.chir.is_none() || self.xasspectrum.r.is_none() {
            return Err(PyValueError::new_err("Forward Fourier transform must be done before reverse transform"));
        }
        
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
            params.window = Some(match window_str.to_lowercase().as_str() {
                "hanning" => xafsutils::FTWindow::Hanning,
                "sine" => xafsutils::FTWindow::Sine,
                "kaiser-bessel" | "kaiserbessel" => xafsutils::FTWindow::KaiserBessel,
                "gaussian" => xafsutils::FTWindow::Gaussian,
                "parzen" => xafsutils::FTWindow::Parzen,
                "welch" => xafsutils::FTWindow::Welch,
                _ => return Err(PyValueError::new_err(format!("Unknown window type: {}", window_str))),
            });
        }
        
        if let Some(kmax_out_value) = kmax_out {
            params.kmax_out = Some(kmax_out_value);
        }
        
        // Perform reverse FT
        match self.xasspectrum.xftr(&params) {
            Ok(_) => Ok(self),
            Err(err) => Err(PyValueError::new_err(format!("Reverse Fourier transform error: {:?}", err))),
        }
    }
    
    /// Fluent API for setting R range
    fn r_range(&mut self, rmin: f64, rmax: f64) -> PyResult<&mut Self> {
        self.xasspectrum.ift_params.rmin = Some(rmin);
        self.xasspectrum.ift_params.rmax = Some(rmax);
        Ok(self)
    }
    
    /// Fluent API for setting dr parameter
    fn dr(&mut self, dr: f64) -> PyResult<&mut Self> {
        self.xasspectrum.ift_params.dr = Some(dr);
        Ok(self)
    }
    
    /// Execute the current operation (used in the fluent API)
    fn run(&mut self) -> PyResult<&mut Self> {
        // Apply any pending operations
        if self.xasspectrum.norm.is_none() && self.xasspectrum.raw_energy.is_some() && self.xasspectrum.raw_mu.is_some() {
            // Apply normalization if it hasn't been done yet
            match self.xasspectrum.normalize(&self.xasspectrum.normalization_params) {
                Ok(_) => {},
                Err(err) => return Err(PyValueError::new_err(format!("Normalization error: {:?}", err))),
            }
        }
        
        if self.xasspectrum.norm.is_some() && self.xasspectrum.chi.is_none() {
            // Apply background removal if normalization is done but not background
            match self.xasspectrum.autobk(&self.xasspectrum.background_params) {
                Ok(_) => {},
                Err(err) => return Err(PyValueError::new_err(format!("Background removal error: {:?}", err))),
            }
        }
        
        if self.xasspectrum.chi.is_some() && self.xasspectrum.chir.is_none() {
            // Apply forward FT if background is done but not FT
            match self.xasspectrum.xftf(&self.xasspectrum.ft_params) {
                Ok(_) => {},
                Err(err) => return Err(PyValueError::new_err(format!("Forward Fourier transform error: {:?}", err))),
            }
        }
        
        if self.xasspectrum.chir.is_some() && self.xasspectrum.chiq.is_none() {
            // Apply reverse FT if forward FT is done but not reverse
            match self.xasspectrum.xftr(&self.xasspectrum.ift_params) {
                Ok(_) => {},
                Err(err) => return Err(PyValueError::new_err(format!("Reverse Fourier transform error: {:?}", err))),
            }
        }
        
        Ok(self)
    }
    
    /// Save spectrum to a file
    fn save(&self, filename: &str) -> PyResult<()> {
        let path = Path::new(filename);
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        
        match extension.to_lowercase().as_str() {
            "json" => {
                match io::xafs_json::write_xas_json(&self.xasspectrum, filename) {
                    Ok(_) => Ok(()),
                    Err(err) => Err(PyValueError::new_err(format!("Error saving to JSON: {:?}", err))),
                }
            },
            "bson" => {
                match io::xafs_bson::write_xas_bson(&self.xasspectrum, filename) {
                    Ok(_) => Ok(()),
                    Err(err) => Err(PyValueError::new_err(format!("Error saving to BSON: {:?}", err))),
                }
            },
            _ => Err(PyValueError::new_err(format!("Unsupported file format: {}", extension))),
        }
    }
    
    /// Read spectrum from a file
    #[staticmethod]
    fn read(filename: &str) -> PyResult<Self> {
        let path = Path::new(filename);
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        
        let xas_spectrum = match extension.to_lowercase().as_str() {
            "json" => {
                match io::xafs_json::read_xas_json(filename) {
                    Ok(spectrum) => spectrum,
                    Err(err) => return Err(PyValueError::new_err(format!("Error reading JSON: {:?}", err))),
                }
            },
            "bson" => {
                match io::xafs_bson::read_xas_bson(filename) {
                    Ok(spectrum) => spectrum,
                    Err(err) => return Err(PyValueError::new_err(format!("Error reading BSON: {:?}", err))),
                }
            },
            _ => return Err(PyValueError::new_err(format!("Unsupported file format: {}", extension))),
        };
        
        Ok(PyXASSpectrum {
            xasspectrum: xas_spectrum,
        })
    }
    
    /// Return string representation
    fn __str__(&self) -> PyResult<String> {
        let mut result = format!("XASSpectrum: {}", self.xasspectrum.name);
        
        if let Some(e0) = self.xasspectrum.e0 {
            result.push_str(&format!("\n  E0: {:.2} eV", e0));
        }
        
        if let Some(edge_step) = self.xasspectrum.edge_step {
            result.push_str(&format!("\n  Edge step: {:.4}", edge_step));
        }
        
        if let Some(energy) = &self.xasspectrum.raw_energy {
            result.push_str(&format!("\n  Energy range: {:.2} to {:.2} eV", 
                                   energy.first().unwrap_or(&0.0), 
                                   energy.last().unwrap_or(&0.0)));
        }
        
        if let Some(k) = &self.xasspectrum.k {
            result.push_str(&format!("\n  k range: {:.2} to {:.2} Å⁻¹", 
                                   k.first().unwrap_or(&0.0), 
                                   k.last().unwrap_or(&0.0)));
        }
        
        if let Some(r) = &self.xasspectrum.r {
            result.push_str(&format!("\n  R range: {:.2} to {:.2} Å", 
                                   r.first().unwrap_or(&0.0), 
                                   r.last().unwrap_or(&0.0)));
        }
        
        Ok(result)
    }
    
    /// Return representation
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("XASSpectrum(name='{}')", self.xasspectrum.name))
    }
}
