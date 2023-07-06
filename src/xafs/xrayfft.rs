#[derive(Debug, Clone, PartialEq, Default)]
pub enum FFTWindow {
    #[default]
    Hanning,
    KaiserBessel,
    Parzen,
    Sine,
    Gaussian,
    Welch,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrayForwardFFT {
    pub rmax_out: Option<f64>,
    pub window: Option<FFTWindow>,
    pub dk: Option<f64>,
    pub dk2: Option<f64>,
    pub kmin: Option<f64>,
    pub kmax: Option<f64>,
    pub kweight: Option<f64>,
    pub nfft: Option<i32>,
}

impl Default for XrayForwardFFT {
    fn default() -> Self {
        XrayForwardFFT {
            rmax_out: Some(10.0),
            window: Some(FFTWindow::default()),
            dk: Some(0.05),
            dk2: None,
            kmin: Some(2.0),
            kmax: Some(15.0),
            kweight: Some(2.0),
            nfft: Some(2048),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct XrayReverseFFT {}
