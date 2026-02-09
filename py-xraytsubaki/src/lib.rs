use pyo3::prelude::*;
use pyo3::types::PyDict;
use xraytsubaki::prelude::*;
use xraytsubaki::xafs::io;
use xraytsubaki::xafs::xasgroup::BatchProcessError;
use xraytsubaki::xafs::XAFSError;

pub mod xasgroup;
pub mod xasspectrum;

fn error_category(error: &XAFSError) -> &'static str {
    match error {
        XAFSError::Data(_) => "data",
        XAFSError::Normalization(_) => "normalization",
        XAFSError::Background(_) => "background",
        XAFSError::FFT(_) => "fft",
        XAFSError::IO(_) => "io",
        XAFSError::Math(_) => "math",
        XAFSError::Fitting(_) => "fitting",
        XAFSError::NotEnoughData => "data",
        XAFSError::NotEnoughDataForXFTF => "fft",
        XAFSError::NotEnoughDataForXFTR => "fft",
        XAFSError::GroupIndexOutOfRange => "group",
        XAFSError::GroupIsEmpty => "group",
    }
}

#[pyfunction]
fn run_batch_qas_trans(paths: Vec<String>) -> PyResult<(usize, Vec<(usize, String, String)>)> {
    let mut group = XASGroup::new();
    let mut load_errors: Vec<(usize, String, String)> = Vec::new();

    for (index, path) in paths.iter().enumerate() {
        match io::load_spectrum_QAS_trans(path) {
            Ok(spectrum) => {
                group.add_spectrum(spectrum);
            }
            Err(err) => load_errors.push((index, "io".to_string(), err.to_string())),
        }
    }

    if !load_errors.is_empty() {
        return Ok((0, load_errors));
    }

    let mut failures: Vec<(usize, String, String)> = Vec::new();
    let mut collect_failures = |batch_error: BatchProcessError| {
        failures.extend(batch_error.errors.into_iter().map(|error| {
            (
                error.index,
                error_category(&error.source).to_string(),
                error.source.to_string(),
            )
        }));
    };
    if let Err(error) = group.find_e0() {
        collect_failures(error);
    } else if let Err(error) = group.normalize() {
        collect_failures(error);
    } else if let Err(error) = group.calc_background() {
        collect_failures(error);
    } else if let Err(error) = group.fft() {
        collect_failures(error);
    }

    Ok((group.len(), failures))
}

#[pyfunction]
fn run_pipeline_arrays<'py>(
    py: Python<'py>,
    energy: numpy::PyReadonlyArray1<'py, f64>,
    mu: numpy::PyReadonlyArray1<'py, f64>,
) -> PyResult<&'py PyDict> {
    // numpy::PyReadonlyArray1 borrows Python memory without copy on input.
    let energy_slice = energy.as_slice()?;
    let mu_slice = mu.as_slice()?;

    let mut spectrum = XASSpectrum::new();
    spectrum.set_spectrum(energy_slice.to_vec(), mu_slice.to_vec());
    spectrum
        .find_e0()
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    spectrum
        .normalize()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    spectrum
        .calc_background()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    spectrum
        .fft()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

    let out = PyDict::new(py);
    out.set_item("e0", spectrum.get_e0())?;
    if let Some(k) = spectrum.get_k() {
        out.set_item("k", numpy::PyArray1::from_slice(py, k.as_slice()))?;
    }
    if let Some(chi) = spectrum.get_chi() {
        out.set_item("chi", numpy::PyArray1::from_slice(py, chi.as_slice()))?;
    }
    if let Some(chir_mag) = spectrum.get_chir_mag() {
        out.set_item(
            "chir_mag",
            numpy::PyArray1::from_slice(py, chir_mag.as_slice()),
        )?;
    }

    Ok(out)
}

/// A Python module implemented in Rust.
#[pymodule]
fn py_xraytsubaki(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_batch_qas_trans, m)?)?;
    m.add_function(wrap_pyfunction!(run_pipeline_arrays, m)?)?;
    Ok(())
}
