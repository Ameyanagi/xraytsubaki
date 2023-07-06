use ndarray::Array1;

/// XASGroup is a struct that contains all the data and parameters for a single XAS spectrum.
///
/// # Examples
///
/// TODO: Add examples
pub struct XASGroup {
    pub name: Option<String>,
    pub raw_energy: Option<Array1<f64>>,
    pub raw_mu: Option<Array1<f64>>,
    pub energy: Option<Array1<f64>>,
    pub mu: Option<Array1<f64>>,
    pub e0: Option<f64>,
    pub norm: Option<Array1<f64>>,
    pub flat: Option<Array1<f64>>,
    pub k: Option<Array1<f64>>,
    pub chi: Option<Array1<f64>>,
    pub chi_kweight: Option<Array1<f64>>,
    pub chi_r: Option<Array1<f64>>,
    pub chi_r_mag: Option<Array1<f64>>,
    pub chi_r_re: Option<Array1<f64>>,
    pub chi_r_im: Option<Array1<f64>>,
    pub q: Option<Array1<f64>>,
    pub normalization: Normalization,
    pub background: Background,
    pub xftf: XrayForwardFFT,
    pub xftr: XrayReverseFFT,
}

impl Default for XASGroup {
    fn default() -> Self {
        todo!("Implement Default for XASGroup")
    }
}

impl XASGroup {
    pub fn new() -> XASGroup {
        XASGroup::default()
    }

    pub fn set_name<S: Into<String>>(&mut self, name: S) {
        self.name = Some(name.into());
    }

    pub fn set_spectrum<E: Into<Array1<f64>>, M: Into<Array1<f64>>>(&mut self, energy: E, mu: M) {
        self.raw_energy = Some(energy.into());
        self.raw_mu = Some(mu.into());
    }

    pub fn set_e0<S: Into<f64>>(&mut self, e0: S) {
        self.e0 = Some(e0.into());
    }
}

enum Normalization {
    PrePostEdge(PrePostEdge),
    MBack(MBack),
    None,
}

struct PrePostEdge {
    pub pre_edge_start: Option<f64>,
    pub pre_edge_end: Option<f64>,
    pub norm_start: Option<f64>,
    pub norm_end: Option<f64>,
    pub norm_polyorder: Option<i32>,
    pub n_victoreen: Option<i32>,
}

impl Default for PrePostEdge {
    fn default() -> Self {
        PrePostEdge {
            pre_edge_start: Some(-200.0),
            pre_edge_end: Some(-30.0),
            norm_start: Some(150.0),
            norm_end: Some(2000.0),
            norm_polyorder: Some(2),
            n_victoreen: Some(0),
        }
    }
}

impl PrePostEdge {
    pub fn new() -> PrePostEdge {
        PrePostEdge::default()
    }
}

struct Mback {}

enum Background {
    AUTOBK(AUTOBK),
    ILPBkg(ILPBkg),
    None,
}

struct AUTOBK {
    pub rbkg: Option<f64>,
    pub nknots: Option<i32>,
    pub kmin: Option<f64>,
    pub kmax: Option<f64>,
    pub kstep: Option<f64>,
    pub nclamp: Option<i32>,
    pub clamp_lo: Option<i32>,
    pub clamp_hi: Option<i32>,
}

impl Default for AUTOBK {
    fn default() -> Self {
        AUTOBK {
            rbkg: Some(1.0),
            nknots: None,
            kmin: Some(0.0),
            kmax: None,
            kstep: Some(0.05),
            nclamp: Some(3),
            clamp_lo: Some(0),
            clamp_hi: Some(1),
        }
    }
}

impl AUTOBK {
    pub fn new() -> AUTOBK {
        AUTOBK::default()
    }
}
struct ILPBkg {}

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

struct XrayForwardFFT {
    pub rmax_out: Option<f64>,
    pub window: Option<FFTWindow>,
    pub dk: Option<f64>,
    pub dk2: Option<f64>,
    pub kmin: Option<f64>,
    pub kmax: Option<f64>,
    pub kweight: Option<f64>,
    pub nfft: Option<i32>,
}

impl Default for FFTParams {
    fn default() -> Self {
        FFTParams {
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
struct XrayReverseFFT {}

impl FFTParams {
    pub fn new() -> FFTParams {
        FFTParams::default()
    }

    pub fn set_window<T>(&mut self, window: T) {
        todo!();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IFFTParams {
    pub rmin: Option<f64>,
    pub rmax: Option<f64>,
    pub dr: Option<f64>,
    pub dr2: Option<f64>,
    pub rweight: Option<f64>,
    pub qmax_out: Option<f64>,
    pub window: Option<FFTWindow>,
}

impl Default for IFFTParams {
    fn default() -> Self {
        IFFTParams {
            rmin: Some(0.0),
            rmax: Some(20.0),
            dr: Some(1.0),
            dr2: None,
            rweight: Some(0.0),
            qmax_out: Some(30.0),
            window: Some(FFTWindow::default()),
        }
    }
}

impl IFFTParams {
    pub fn new() -> IFFTParams {
        IFFTParams::default()
    }

    pub fn set_window<T>(&mut self, window: T) {
        todo!();
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_xafs_group_name_from_string() {
        let mut xafs_group = XASGroup::new();
        xafs_group.set_name("test".to_string());
        assert_eq!(xafs_group.name, Some("test".to_string()));
    }

    #[test]
    fn test_xafs_group_name_from_str() {
        let mut xafs_group = XASGroup::new();
        xafs_group.set_name("test");
        assert_eq!(xafs_group.name, Some("test".to_string()));

        let name = String::from("test");

        let mut xafs_group = XASGroup::new();
        xafs_group.set_name(name.clone());
        assert_eq!(xafs_group.name, Some("test".to_string()));

        println!("name: {}", name);
    }

    #[test]
    fn test_xafs_group_spectrum_from_vec() {
        let energy: Vec<f64> = vec![1.0, 2.0, 3.0];
        let mu: Array1<f64> = Array1::from_vec(vec![4.0, 5.0, 6.0]);
        let mut xafs_group = XASGroup::new();
        xafs_group.set_spectrum(energy, mu);
        assert_eq!(
            xafs_group.raw_energy,
            Some(Array1::from_vec(vec![1.0, 2.0, 3.0]))
        );
        assert_eq!(
            xafs_group.raw_mu,
            Some(Array1::from_vec(vec![4.0, 5.0, 6.0]))
        );
    }
}
