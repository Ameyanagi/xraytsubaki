//! Thin Wasm bindings: stage execution and defaults live in rexafs.
use wasm_bindgen::prelude::*;
fn error(error: rexafs::Error) -> JsValue {
    js_sys::Error::new(&error.to_string()).into()
}

#[wasm_bindgen(js_name = PrePostEdge)]
pub struct WasmPrePostEdge {
    inner: rexafs::PrePostEdge,
}
#[wasm_bindgen(js_class = PrePostEdge)]
impl WasmPrePostEdge {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: rexafs::PrePostEdge::new(),
        }
    }
    #[wasm_bindgen(getter)]
    pub fn pre_edge_start(&self) -> Option<f64> {
        self.inner.pre_edge_start
    }
    #[wasm_bindgen(setter)]
    pub fn set_pre_edge_start(&mut self, value: Option<f64>) {
        self.inner.pre_edge_start = value;
    }
    #[wasm_bindgen(getter)]
    pub fn pre_edge_end(&self) -> Option<f64> {
        self.inner.pre_edge_end
    }
    #[wasm_bindgen(setter)]
    pub fn set_pre_edge_end(&mut self, value: Option<f64>) {
        self.inner.pre_edge_end = value;
    }
    #[wasm_bindgen(getter)]
    pub fn norm_start(&self) -> Option<f64> {
        self.inner.norm_start
    }
    #[wasm_bindgen(setter)]
    pub fn set_norm_start(&mut self, value: Option<f64>) {
        self.inner.norm_start = value;
    }
    #[wasm_bindgen(getter)]
    pub fn norm_end(&self) -> Option<f64> {
        self.inner.norm_end
    }
    #[wasm_bindgen(setter)]
    pub fn set_norm_end(&mut self, value: Option<f64>) {
        self.inner.norm_end = value;
    }
    #[wasm_bindgen(getter)]
    pub fn norm_polyorder(&self) -> Option<i32> {
        self.inner.norm_polyorder
    }
    #[wasm_bindgen(setter)]
    pub fn set_norm_polyorder(&mut self, value: Option<i32>) {
        self.inner.norm_polyorder = value;
    }
    #[wasm_bindgen(getter)]
    pub fn n_victoreen(&self) -> Option<i32> {
        self.inner.n_victoreen
    }
    #[wasm_bindgen(setter)]
    pub fn set_n_victoreen(&mut self, value: Option<i32>) {
        self.inner.n_victoreen = value;
    }
    #[wasm_bindgen(getter)]
    pub fn e0(&self) -> Option<f64> {
        self.inner.e0
    }
    #[wasm_bindgen(setter)]
    pub fn set_e0(&mut self, value: Option<f64>) {
        self.inner.e0 = value;
    }
    #[wasm_bindgen(getter)]
    pub fn edge_step(&self) -> Option<f64> {
        self.inner.edge_step
    }
    #[wasm_bindgen(setter)]
    pub fn set_edge_step(&mut self, value: Option<f64>) {
        self.inner.edge_step = value;
    }
}

#[wasm_bindgen(js_name = AUTOBK)]
pub struct WasmAUTOBK {
    inner: rexafs::AUTOBK,
}
#[wasm_bindgen(js_class = AUTOBK)]
impl WasmAUTOBK {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: rexafs::AUTOBK::new(),
        }
    }
    #[wasm_bindgen(getter)]
    pub fn ek0(&self) -> Option<f64> {
        self.inner.ek0
    }
    #[wasm_bindgen(setter)]
    pub fn set_ek0(&mut self, value: Option<f64>) {
        self.inner.ek0 = value;
    }
    #[wasm_bindgen(getter)]
    pub fn rbkg(&self) -> Option<f64> {
        self.inner.rbkg
    }
    #[wasm_bindgen(setter)]
    pub fn set_rbkg(&mut self, value: Option<f64>) {
        self.inner.rbkg = value;
    }
    #[wasm_bindgen(getter)]
    pub fn nknots(&self) -> Option<i32> {
        self.inner.nknots
    }
    #[wasm_bindgen(setter)]
    pub fn set_nknots(&mut self, value: Option<i32>) {
        self.inner.nknots = value;
    }
    #[wasm_bindgen(getter)]
    pub fn kmin(&self) -> Option<f64> {
        self.inner.kmin
    }
    #[wasm_bindgen(setter)]
    pub fn set_kmin(&mut self, value: Option<f64>) {
        self.inner.kmin = value;
    }
    #[wasm_bindgen(getter)]
    pub fn kmax(&self) -> Option<f64> {
        self.inner.kmax
    }
    #[wasm_bindgen(setter)]
    pub fn set_kmax(&mut self, value: Option<f64>) {
        self.inner.kmax = value;
    }
    #[wasm_bindgen(getter)]
    pub fn kstep(&self) -> Option<f64> {
        self.inner.kstep
    }
    #[wasm_bindgen(setter)]
    pub fn set_kstep(&mut self, value: Option<f64>) {
        self.inner.kstep = value;
    }
    #[wasm_bindgen(getter)]
    pub fn nclamp(&self) -> Option<i32> {
        self.inner.nclamp
    }
    #[wasm_bindgen(setter)]
    pub fn set_nclamp(&mut self, value: Option<i32>) {
        self.inner.nclamp = value;
    }
    #[wasm_bindgen(getter)]
    pub fn clamp_lo(&self) -> Option<i32> {
        self.inner.clamp_lo
    }
    #[wasm_bindgen(setter)]
    pub fn set_clamp_lo(&mut self, value: Option<i32>) {
        self.inner.clamp_lo = value;
    }
    #[wasm_bindgen(getter)]
    pub fn clamp_hi(&self) -> Option<i32> {
        self.inner.clamp_hi
    }
    #[wasm_bindgen(setter)]
    pub fn set_clamp_hi(&mut self, value: Option<i32>) {
        self.inner.clamp_hi = value;
    }
    #[wasm_bindgen(getter)]
    pub fn clamp_lambda(&self) -> Option<f64> {
        self.inner.clamp_lambda
    }
    #[wasm_bindgen(setter)]
    pub fn set_clamp_lambda(&mut self, value: Option<f64>) {
        self.inner.clamp_lambda = value;
    }
    #[wasm_bindgen(getter)]
    pub fn nfft(&self) -> Option<i32> {
        self.inner.nfft
    }
    #[wasm_bindgen(setter)]
    pub fn set_nfft(&mut self, value: Option<i32>) {
        self.inner.nfft = value;
    }
    #[wasm_bindgen(getter)]
    pub fn kweight(&self) -> Option<i32> {
        self.inner.kweight
    }
    #[wasm_bindgen(setter)]
    pub fn set_kweight(&mut self, value: Option<i32>) {
        self.inner.kweight = value;
    }
    #[wasm_bindgen(getter)]
    pub fn dk(&self) -> Option<f64> {
        self.inner.dk
    }
    #[wasm_bindgen(setter)]
    pub fn set_dk(&mut self, value: Option<f64>) {
        self.inner.dk = value;
    }
    #[wasm_bindgen(getter)]
    pub fn linear_regularization(&self) -> Option<f64> {
        self.inner.linear_regularization
    }
    #[wasm_bindgen(setter)]
    pub fn set_linear_regularization(&mut self, value: Option<f64>) {
        self.inner.linear_regularization = value;
    }
    #[wasm_bindgen(getter)]
    pub fn linear_condition_limit(&self) -> Option<f64> {
        self.inner.linear_condition_limit
    }
    #[wasm_bindgen(setter)]
    pub fn set_linear_condition_limit(&mut self, value: Option<f64>) {
        self.inner.linear_condition_limit = value;
    }
    #[wasm_bindgen(getter)]
    pub fn linear_residual_ratio_limit(&self) -> Option<f64> {
        self.inner.linear_residual_ratio_limit
    }
    #[wasm_bindgen(setter)]
    pub fn set_linear_residual_ratio_limit(&mut self, value: Option<f64>) {
        self.inner.linear_residual_ratio_limit = value;
    }
    #[wasm_bindgen(getter)]
    pub fn linear_fallback_to_lm(&self) -> Option<bool> {
        self.inner.linear_fallback_to_lm
    }
    #[wasm_bindgen(setter)]
    pub fn set_linear_fallback_to_lm(&mut self, value: Option<bool>) {
        self.inner.linear_fallback_to_lm = value;
    }
    #[wasm_bindgen(getter)]
    pub fn linear_workspace_cache(&self) -> Option<bool> {
        self.inner.linear_workspace_cache
    }
    #[wasm_bindgen(setter)]
    pub fn set_linear_workspace_cache(&mut self, value: Option<bool>) {
        self.inner.linear_workspace_cache = value;
    }
    #[wasm_bindgen(getter)]
    pub fn window(&self) -> Option<String> {
        Some(format!("{:?}", self.inner.window))
    }
    #[wasm_bindgen(setter)]
    pub fn set_window(&mut self, value: Option<String>) -> Result<(), JsValue> {
        let parsed = match value.as_deref() {
            None => None,
            Some("Hanning") => Some(rexafs::prelude::FTWindow::Hanning),
            Some("Parzen") => Some(rexafs::prelude::FTWindow::Parzen),
            Some("Welch") => Some(rexafs::prelude::FTWindow::Welch),
            Some("Gaussian") => Some(rexafs::prelude::FTWindow::Gaussian),
            Some("Sine") => Some(rexafs::prelude::FTWindow::Sine),
            Some("KaiserBessel") => Some(rexafs::prelude::FTWindow::KaiserBessel),
            Some("FHanning") => Some(rexafs::prelude::FTWindow::FHanning),
            Some(value) => {
                return Err(js_sys::Error::new(&format!("unknown FTWindow: {value}")).into())
            }
        };
        self.inner.window = parsed.unwrap_or_default();
        Ok(())
    }
    #[wasm_bindgen(getter)]
    pub fn solver(&self) -> Option<String> {
        self.inner.solver.map(|v| format!("{v:?}"))
    }
    #[wasm_bindgen(setter)]
    pub fn set_solver(&mut self, value: Option<String>) -> Result<(), JsValue> {
        let parsed = match value.as_deref() {
            None => None,
            Some("TrustRegionDogLeg") => Some(rexafs::prelude::AUTOBKSolver::TrustRegionDogLeg),
            Some("LegacyLm") => Some(rexafs::prelude::AUTOBKSolver::LegacyLm),
            Some("LinearDirect") => Some(rexafs::prelude::AUTOBKSolver::LinearDirect),
            Some(value) => {
                return Err(js_sys::Error::new(&format!("unknown AUTOBKSolver: {value}")).into())
            }
        };
        self.inner.solver = parsed;
        Ok(())
    }
    #[wasm_bindgen(getter)]
    pub fn linear_fallback_solver(&self) -> Option<String> {
        self.inner.linear_fallback_solver.map(|v| format!("{v:?}"))
    }
    #[wasm_bindgen(setter)]
    pub fn set_linear_fallback_solver(&mut self, value: Option<String>) -> Result<(), JsValue> {
        let parsed = match value.as_deref() {
            None => None,
            Some("TrustRegionDogLeg") => Some(rexafs::prelude::AUTOBKSolver::TrustRegionDogLeg),
            Some("LegacyLm") => Some(rexafs::prelude::AUTOBKSolver::LegacyLm),
            Some("LinearDirect") => Some(rexafs::prelude::AUTOBKSolver::LinearDirect),
            Some(value) => {
                return Err(js_sys::Error::new(&format!("unknown AUTOBKSolver: {value}")).into())
            }
        };
        self.inner.linear_fallback_solver = parsed;
        Ok(())
    }
    #[wasm_bindgen(getter)]
    pub fn clamp_scale_policy(&self) -> Option<String> {
        self.inner.clamp_scale_policy.map(|v| format!("{v:?}"))
    }
    #[wasm_bindgen(setter)]
    pub fn set_clamp_scale_policy(&mut self, value: Option<String>) -> Result<(), JsValue> {
        let parsed = match value.as_deref() {
            None => None,
            Some("FixedPenalty") => Some(rexafs::prelude::AUTOBKClampScalePolicy::FixedPenalty),
            Some("Fixed") => Some(rexafs::prelude::AUTOBKClampScalePolicy::Fixed),
            Some("TwoPass") => Some(rexafs::prelude::AUTOBKClampScalePolicy::TwoPass),
            Some(value) => {
                return Err(
                    js_sys::Error::new(&format!("unknown AUTOBKClampScalePolicy: {value}")).into(),
                )
            }
        };
        self.inner.clamp_scale_policy = parsed;
        Ok(())
    }
}

#[wasm_bindgen(js_name = XrayFFTF)]
pub struct WasmXrayFFTF {
    inner: rexafs::XrayFFTF,
}
#[wasm_bindgen(js_class = XrayFFTF)]
impl WasmXrayFFTF {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: rexafs::XrayFFTF::new(),
        }
    }
    #[wasm_bindgen(getter)]
    pub fn rmax_out(&self) -> Option<f64> {
        self.inner.rmax_out
    }
    #[wasm_bindgen(setter)]
    pub fn set_rmax_out(&mut self, value: Option<f64>) {
        self.inner.rmax_out = value;
    }
    #[wasm_bindgen(getter)]
    pub fn dk(&self) -> Option<f64> {
        self.inner.dk
    }
    #[wasm_bindgen(setter)]
    pub fn set_dk(&mut self, value: Option<f64>) {
        self.inner.dk = value;
    }
    #[wasm_bindgen(getter)]
    pub fn dk2(&self) -> Option<f64> {
        self.inner.dk2
    }
    #[wasm_bindgen(setter)]
    pub fn set_dk2(&mut self, value: Option<f64>) {
        self.inner.dk2 = value;
    }
    #[wasm_bindgen(getter)]
    pub fn kmin(&self) -> Option<f64> {
        self.inner.kmin
    }
    #[wasm_bindgen(setter)]
    pub fn set_kmin(&mut self, value: Option<f64>) {
        self.inner.kmin = value;
    }
    #[wasm_bindgen(getter)]
    pub fn kmax(&self) -> Option<f64> {
        self.inner.kmax
    }
    #[wasm_bindgen(setter)]
    pub fn set_kmax(&mut self, value: Option<f64>) {
        self.inner.kmax = value;
    }
    #[wasm_bindgen(getter)]
    pub fn kweight(&self) -> Option<f64> {
        self.inner.kweight
    }
    #[wasm_bindgen(setter)]
    pub fn set_kweight(&mut self, value: Option<f64>) {
        self.inner.kweight = value;
    }
    #[wasm_bindgen(getter)]
    pub fn nfft(&self) -> Option<usize> {
        self.inner.nfft
    }
    #[wasm_bindgen(setter)]
    pub fn set_nfft(&mut self, value: Option<usize>) {
        self.inner.nfft = value;
    }
    #[wasm_bindgen(getter)]
    pub fn kstep(&self) -> Option<f64> {
        self.inner.kstep
    }
    #[wasm_bindgen(setter)]
    pub fn set_kstep(&mut self, value: Option<f64>) {
        self.inner.kstep = value;
    }
    #[wasm_bindgen(getter)]
    pub fn window(&self) -> Option<String> {
        self.inner.window.map(|v| format!("{v:?}"))
    }
    #[wasm_bindgen(setter)]
    pub fn set_window(&mut self, value: Option<String>) -> Result<(), JsValue> {
        let parsed = match value.as_deref() {
            None => None,
            Some("Hanning") => Some(rexafs::prelude::FTWindow::Hanning),
            Some("Parzen") => Some(rexafs::prelude::FTWindow::Parzen),
            Some("Welch") => Some(rexafs::prelude::FTWindow::Welch),
            Some("Gaussian") => Some(rexafs::prelude::FTWindow::Gaussian),
            Some("Sine") => Some(rexafs::prelude::FTWindow::Sine),
            Some("KaiserBessel") => Some(rexafs::prelude::FTWindow::KaiserBessel),
            Some("FHanning") => Some(rexafs::prelude::FTWindow::FHanning),
            Some(value) => {
                return Err(js_sys::Error::new(&format!("unknown FTWindow: {value}")).into())
            }
        };
        self.inner.window = parsed;
        Ok(())
    }
}

#[wasm_bindgen(js_name = NormalizationMethod)]
pub struct WasmNormalizationMethod {
    inner: rexafs::NormalizationMethod,
}
#[wasm_bindgen(js_class = NormalizationMethod)]
impl WasmNormalizationMethod {
    #[wasm_bindgen(js_name = PrePostEdge)]
    pub fn configured(parameters: &WasmPrePostEdge) -> Self {
        Self {
            inner: rexafs::NormalizationMethod::PrePostEdge(parameters.inner.clone()),
        }
    }
    pub fn new_prepostedge() -> Self {
        Self {
            inner: rexafs::NormalizationMethod::new_prepostedge(),
        }
    }
    pub fn new_mback() -> Self {
        Self {
            inner: rexafs::NormalizationMethod::new_mback(),
        }
    }
}

#[wasm_bindgen(js_name = BackgroundMethod)]
pub struct WasmBackgroundMethod {
    inner: rexafs::BackgroundMethod,
}
#[wasm_bindgen(js_class = BackgroundMethod)]
impl WasmBackgroundMethod {
    #[wasm_bindgen(js_name = AUTOBK)]
    pub fn configured(parameters: &WasmAUTOBK) -> Self {
        Self {
            inner: rexafs::BackgroundMethod::AUTOBK(parameters.inner.clone()),
        }
    }
    pub fn new_autobk() -> Self {
        Self {
            inner: rexafs::BackgroundMethod::new_autobk(),
        }
    }
    pub fn new_ilpbkg() -> Self {
        Self {
            inner: rexafs::BackgroundMethod::new_ilpbkg(),
        }
    }
}

#[wasm_bindgen(js_name = Spectrum)]
pub struct WasmSpectrum {
    inner: rexafs::Spectrum,
}
#[wasm_bindgen(js_class = Spectrum)]
impl WasmSpectrum {
    #[wasm_bindgen(constructor)]
    pub fn new(energy: &[f64], mu: &[f64]) -> Result<Self, JsValue> {
        Ok(Self {
            inner: rexafs::Spectrum::from_arrays(energy, mu).map_err(error)?,
        })
    }
    pub fn from_arrays(energy: &[f64], mu: &[f64]) -> Result<Self, JsValue> {
        Self::new(energy, mu)
    }
    pub fn set_spectrum(&mut self, energy: &[f64], mu: &[f64]) -> Result<(), JsValue> {
        let input = rexafs::Spectrum::from_arrays(energy, mu).map_err(error)?;
        self.inner
            .set_spectrum(input.energy.unwrap(), input.mu.unwrap());
        Ok(())
    }
    pub fn set_e0(&mut self, e0: f64) {
        self.inner.set_e0(e0);
    }
    pub fn e0(&self) -> Option<f64> {
        self.inner.e0()
    }
    pub fn invalidate_derived(&mut self) {
        self.inner.invalidate_derived();
    }
    pub fn set_fft(&mut self, parameters: &WasmXrayFFTF) {
        self.inner.set_fft(parameters.inner.clone());
    }
    pub fn set_normalization_method(
        &mut self,
        method: &WasmNormalizationMethod,
    ) -> Result<(), JsValue> {
        self.inner
            .set_normalization_method(Some(method.inner.clone()))
            .map(|_| ())
            .map_err(error)
    }
    pub fn set_background_method(&mut self, method: &WasmBackgroundMethod) -> Result<(), JsValue> {
        self.inner
            .set_background_method(Some(method.inner.clone()))
            .map(|_| ())
            .map_err(error)
    }
    pub fn find_e0(&mut self) -> Result<(), JsValue> {
        self.inner.find_e0().map(|_| ()).map_err(error)
    }
    pub fn normalize(&mut self) -> Result<(), JsValue> {
        self.inner.normalize().map(|_| ()).map_err(error)
    }
    pub fn calc_background(&mut self) -> Result<(), JsValue> {
        self.inner.calc_background().map(|_| ()).map_err(error)
    }
    pub fn fft(&mut self) -> Result<(), JsValue> {
        self.inner.fft().map(|_| ()).map_err(error)
    }
    pub fn ifft(&mut self) -> Result<(), JsValue> {
        self.inner.ifft().map(|_| ()).map_err(error)
    }
    pub fn k(&self) -> Option<js_sys::Float64Array> {
        self.inner.k().map(|v| js_sys::Float64Array::from(v))
    }
    pub fn chi(&self) -> Option<js_sys::Float64Array> {
        self.inner.chi().map(|v| js_sys::Float64Array::from(v))
    }
    pub fn norm(&self) -> Option<js_sys::Float64Array> {
        self.inner
            .norm()
            .map(|v| js_sys::Float64Array::from(v.as_slice()))
    }
    pub fn flat(&self) -> Option<js_sys::Float64Array> {
        self.inner
            .flat()
            .map(|v| js_sys::Float64Array::from(v.as_slice()))
    }
    pub fn pre_edge(&self) -> Option<js_sys::Float64Array> {
        self.inner
            .pre_edge()
            .map(|v| js_sys::Float64Array::from(v.as_slice()))
    }
    pub fn post_edge(&self) -> Option<js_sys::Float64Array> {
        self.inner
            .post_edge()
            .map(|v| js_sys::Float64Array::from(v.as_slice()))
    }
    pub fn r(&self) -> Option<js_sys::Float64Array> {
        self.inner
            .r()
            .map(|v| js_sys::Float64Array::from(v.as_slice()))
    }
    pub fn chir_mag(&self) -> Option<js_sys::Float64Array> {
        self.inner
            .chir_mag()
            .map(|v| js_sys::Float64Array::from(v.as_slice()))
    }
    pub fn chir_real(&self) -> Option<js_sys::Float64Array> {
        self.inner
            .chir_real()
            .map(|v| js_sys::Float64Array::from(v.as_slice()))
    }
    pub fn chir_imag(&self) -> Option<js_sys::Float64Array> {
        self.inner
            .chir_imag()
            .map(|v| js_sys::Float64Array::from(v.as_slice()))
    }
    pub fn q(&self) -> Option<js_sys::Float64Array> {
        self.inner
            .q()
            .map(|v| js_sys::Float64Array::from(v.as_slice()))
    }
    pub fn chiq(&self) -> Option<js_sys::Float64Array> {
        self.inner
            .chiq()
            .map(|v| js_sys::Float64Array::from(v.as_slice()))
    }
}
