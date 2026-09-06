use numpy::{PyArray1, PyReadonlyArray1};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use rexafs::{Error, ProcessOptions};

fn error_category(error: &Error) -> &'static str {
    match error {
        Error::Data(_) | Error::NotEnoughData => "data",
        Error::Normalization(_) => "normalization",
        Error::Background(_) => "background",
        Error::FFT(_) | Error::NotEnoughDataForXFTF | Error::NotEnoughDataForXFTR => "fft",
        Error::IO(_) => "io",
        Error::Math(_) => "math",
        Error::Fitting(_) => "fitting",
        Error::GroupIndexOutOfRange | Error::GroupIsEmpty => "group",
    }
}

#[pyfunction]
#[pyo3(signature = (energy, mu, e0=None))]
fn process<'py>(
    py: Python<'py>,
    energy: PyReadonlyArray1<'py, f64>,
    mu: PyReadonlyArray1<'py, f64>,
    e0: Option<f64>,
) -> PyResult<Bound<'py, PyDict>> {
    // Own inputs before releasing the GIL; another Python thread may mutate arrays.
    let energy = energy.as_array().iter().copied().collect::<Vec<_>>();
    let mu = mu.as_array().iter().copied().collect::<Vec<_>>();
    let result = py
        .detach(|| rexafs::process_with_options(&energy, &mu, ProcessOptions { e0 }))
        .map_err(|error| match error {
            Error::Data(_) | Error::Normalization(_) => {
                pyo3::exceptions::PyValueError::new_err(error.to_string())
            }
            _ => pyo3::exceptions::PyRuntimeError::new_err(error.to_string()),
        })?;
    let out = PyDict::new(py);
    out.set_item("e0", result.e0)?;
    for (name, values) in [
        ("k", result.k),
        ("chi", result.chi),
        ("r", result.r),
        ("chir_mag", result.chir_mag),
        ("chir_re", result.chir_re),
        ("chir_im", result.chir_im),
    ] {
        out.set_item(name, PyArray1::from_vec(py, values))?;
    }
    Ok(out)
}

type BatchFailures = Vec<(usize, String, String)>;

#[pyfunction]
fn run_batch_qas_trans(py: Python<'_>, paths: Vec<String>) -> (usize, BatchFailures) {
    py.detach(|| {
        let mut processed = 0;
        let mut errors = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            let result = rexafs::io::read_qas_transmission(path)
                .map_err(Error::from)
                .and_then(|mut spectrum| {
                    spectrum.find_e0()?.normalize()?.calc_background()?.fft()?;
                    Ok(())
                });
            match result {
                Ok(()) => processed += 1,
                Err(error) => {
                    errors.push((index, error_category(&error).into(), error.to_string()))
                }
            }
        }
        (processed, errors)
    })
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(process, m)?)?;
    m.add_function(wrap_pyfunction!(run_batch_qas_trans, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
