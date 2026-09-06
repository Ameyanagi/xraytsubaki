//! The portable array pipeline exposed by the rexafs npm package.
use wasm_bindgen::prelude::*;

/// Normalize, remove the background and Fourier transform one spectrum.
#[wasm_bindgen]
pub fn process(energy: &[f64], mu: &[f64], e0: Option<f64>) -> Result<JsValue, JsValue> {
    let result = rexafs::process_with_options(energy, mu, rexafs::ProcessOptions { e0 })
        .map_err(|error| js_sys::Error::new(&error.to_string()))?;
    let output = js_sys::Object::new();
    js_sys::Reflect::set(&output, &"e0".into(), &result.e0.into())?;
    for (name, values) in [
        ("k", result.k),
        ("chi", result.chi),
        ("r", result.r),
        ("chir_mag", result.chir_mag),
        ("chir_re", result.chir_re),
        ("chir_im", result.chir_im),
    ] {
        let array = js_sys::Float64Array::from(values.as_slice());
        js_sys::Reflect::set(&output, &name.into(), &array)?;
    }
    Ok(output.into())
}
