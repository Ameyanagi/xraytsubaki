#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundMethod {
    AUTOBK(AUTOBK),
    ILPBkg(ILPBkg),
    None,
}

impl Default for BackgroundMethod {
    fn default() -> Self {
        BackgroundMethod::AUTOBK(AUTOBK::default())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AUTOBK {
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

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ILPBkg {}
