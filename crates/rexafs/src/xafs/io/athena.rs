//! Athena project file (`.prj`) import and export.
//!
//! An Athena project file is a gzip-compressed Perl-ish text file written by
//! Demeter/Athena. The layout is:
//!
//! ```text
//! # Athena project file -- Demeter version 0.9.26
//! # This file created at 2023-10-03T15:57:44
//! # Using Demeter 0.9.26 with perl 5.024000 ...
//!
//! $old_group = 'jimwk';
//! @args = ('key','value',...);
//! @x = ('21912.25',...);
//! @y = ('-0.056',...);
//! @i0 = (...);          # optional
//! @signal = (...);      # optional
//! @stddev = (...);      # optional (merged groups)
//! [record]   # create object and set arrays in ifeffit
//!
//! @journal = ('line', ...);
//!
//! 1;
//!
//! # Local Variables:
//! # truncate-lines: t
//! # End:
//! ```
//!
//! The reference implementation used for parsing and writing semantics is
//! Larch's `larch.io.athena_project` (`parse_perlathena` / `AthenaProject.save`).
//!
//! Round-trip fidelity: every `@args` key/value pair is kept verbatim in
//! [`AthenaGroup::args`] (in file order). The typed [`AthenaParams`] view is
//! derived from those args on read, and written back on top of them on write
//! (typed values win, original key order and quoting style are preserved,
//! unchanged values keep their original text, new keys are appended).

use std::fmt;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use nalgebra::DVector;

use crate::xafs::background::{BackgroundMethod, AUTOBK};
use crate::xafs::errors::{DataError, IOError};
use crate::xafs::normalization::{NormalizationMethod, PrePostEdge};
use crate::xafs::xafsutils::FTWindow;
use crate::xafs::xasspectrum::XASSpectrum;
use crate::xafs::xrayfft::{XrayFFTF, XrayFFTR};
use crate::xafs::XAFSError;

/// Demeter version written in the header of files we create.
pub const DEFAULT_DEMETER_VERSION: &str = "0.9.26";

/// `k = sqrt(ETOK * (E - E0))`, same constant as Larch/Ifeffit.
const ETOK: f64 = 0.262_468_291_7;

// ---------------------------------------------------------------------------
// Raw values
// ---------------------------------------------------------------------------

/// A raw value from an Athena `@args` list.
///
/// Athena writes most values as single-quoted strings, some as bare numbers
/// and a few (e.g. `titles`) as Perl array references. Keeping the flavour
/// lets us write the file back exactly as it was read.
#[derive(Debug, Clone, PartialEq)]
pub enum AthenaValue {
    /// `'text'` (escapes already resolved)
    Quoted(String),
    /// bare token such as `0`, `1`, `24`
    Bare(String),
    /// `[...]` array reference
    List(Vec<AthenaValue>),
}

impl AthenaValue {
    /// Scalar text of the value (`None` for lists).
    pub fn as_str(&self) -> Option<&str> {
        match self {
            AthenaValue::Quoted(s) | AthenaValue::Bare(s) => Some(s),
            AthenaValue::List(_) => None,
        }
    }

    /// Parse the scalar as a float (`None` if not a number).
    pub fn as_f64(&self) -> Option<f64> {
        self.as_str()?.trim().parse::<f64>().ok()
    }

    /// Quoted value from any string-like.
    pub fn quoted<S: Into<String>>(s: S) -> Self {
        AthenaValue::Quoted(s.into())
    }

    /// Bare value from any string-like.
    pub fn bare<S: Into<String>>(s: S) -> Self {
        AthenaValue::Bare(s.into())
    }
}

impl fmt::Display for AthenaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AthenaValue::Quoted(s) => {
                f.write_str("'")?;
                for c in s.chars() {
                    match c {
                        '\\' => f.write_str("\\\\")?,
                        '\'' => f.write_str("\\'")?,
                        c => write!(f, "{c}")?,
                    }
                }
                f.write_str("'")
            }
            AthenaValue::Bare(s) => f.write_str(s),
            AthenaValue::List(items) => {
                f.write_str("[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
        }
    }
}

/// Behaviour shared by the typed parameter kinds.
trait ParamValue: Sized {
    fn from_value(v: &AthenaValue) -> Option<Self>;
    fn to_text(&self) -> String;
    fn matches(&self, v: &AthenaValue) -> bool;
}

impl ParamValue for f64 {
    fn from_value(v: &AthenaValue) -> Option<Self> {
        v.as_f64()
    }
    fn to_text(&self) -> String {
        fmt_num(*self)
    }
    fn matches(&self, v: &AthenaValue) -> bool {
        v.as_f64() == Some(*self)
    }
}

/// Athena spells spline clamps either as numbers or as names.
fn clamp_from_name(s: &str) -> Option<i32> {
    match s.trim().to_ascii_lowercase().as_str() {
        "none" | "" => Some(0),
        "slight" => Some(3),
        "weak" => Some(6),
        "medium" => Some(12),
        "strong" => Some(24),
        "rigid" => Some(96),
        _ => None,
    }
}

impl ParamValue for i32 {
    fn from_value(v: &AthenaValue) -> Option<Self> {
        let s = v.as_str()?;
        if let Ok(f) = s.trim().parse::<f64>() {
            return Some(f.round() as i32);
        }
        clamp_from_name(s)
    }
    fn to_text(&self) -> String {
        format!("{self}")
    }
    fn matches(&self, v: &AthenaValue) -> bool {
        Self::from_value(v) == Some(*self)
    }
}

impl ParamValue for bool {
    fn from_value(v: &AthenaValue) -> Option<Self> {
        let s = v.as_str()?.trim();
        if let Ok(f) = s.parse::<f64>() {
            return Some(f != 0.0);
        }
        match s.to_ascii_lowercase().as_str() {
            "" | "no" | "false" | "none" => Some(false),
            _ => Some(true),
        }
    }
    fn to_text(&self) -> String {
        if *self { "1" } else { "0" }.to_string()
    }
    fn matches(&self, v: &AthenaValue) -> bool {
        Self::from_value(v) == Some(*self)
    }
}

impl ParamValue for String {
    fn from_value(v: &AthenaValue) -> Option<Self> {
        v.as_str().map(str::to_string)
    }
    fn to_text(&self) -> String {
        self.clone()
    }
    fn matches(&self, v: &AthenaValue) -> bool {
        v.as_str() == Some(self.as_str())
    }
}

/// Shortest round-trip text for a float, switching to exponent notation for
/// very small or very large magnitudes like Perl's `%g` does.
pub fn fmt_num(v: f64) -> String {
    let a = v.abs();
    if v != 0.0 && v.is_finite() && !(1e-4..1e15).contains(&a) {
        format!("{v:e}")
    } else {
        format!("{v}")
    }
}

fn lookup<'a>(args: &'a [(String, AthenaValue)], key: &str) -> Option<&'a AthenaValue> {
    args.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Write a typed value into the args list: keep the original text if it
/// already denotes the same value, otherwise replace it (keeping the quoting
/// style); append as a quoted value if the key is new.
fn apply<T: ParamValue>(args: &mut Vec<(String, AthenaValue)>, key: &str, value: &T) {
    match args.iter_mut().find(|(k, _)| k == key) {
        Some((_, existing)) => {
            if !value.matches(existing) {
                *existing = match existing {
                    AthenaValue::Bare(_) => AthenaValue::Bare(value.to_text()),
                    _ => AthenaValue::Quoted(value.to_text()),
                };
            }
        }
        None => args.push((key.to_string(), AthenaValue::Quoted(value.to_text()))),
    }
}

// ---------------------------------------------------------------------------
// Typed parameters
// ---------------------------------------------------------------------------

macro_rules! athena_params {
    ($( $(#[$doc:meta])* $field:ident : $ty:ty => $key:literal ),* $(,)?) => {
        /// Typed view of the Athena per-group parameters we understand.
        ///
        /// Every field is optional: `None` means the key was absent from the
        /// file (or is unknown for a freshly created group). Unknown keys are
        /// kept verbatim in [`AthenaGroup::args`].
        #[derive(Debug, Clone, Default, PartialEq)]
        pub struct AthenaParams {
            $( $(#[$doc])* pub $field: Option<$ty>, )*
        }

        impl AthenaParams {
            /// Athena key for each typed field, in declaration order.
            pub const KEYS: &'static [&'static str] = &[ $( $key ),* ];

            /// Build the typed view from a raw args list.
            pub fn from_args(args: &[(String, AthenaValue)]) -> Self {
                Self {
                    $( $field: lookup(args, $key).and_then(<$ty as ParamValue>::from_value), )*
                }
            }

            /// Write every `Some` field back into `args`.
            pub fn apply_to_args(&self, args: &mut Vec<(String, AthenaValue)>) {
                $( if let Some(v) = &self.$field { apply(args, $key, v); } )*
            }
        }
    };
}

athena_params! {
    /// `bkg_e0`: edge energy (eV)
    e0: f64 => "bkg_e0",
    /// `bkg_pre1`: pre-edge range start, relative to e0 (eV)
    pre1: f64 => "bkg_pre1",
    /// `bkg_pre2`: pre-edge range end, relative to e0 (eV)
    pre2: f64 => "bkg_pre2",
    /// `bkg_nor1`: normalization range start, relative to e0 (eV)
    nor1: f64 => "bkg_nor1",
    /// `bkg_nor2`: normalization range end, relative to e0 (eV)
    nor2: f64 => "bkg_nor2",
    /// `bkg_nnorm`: number of terms of the post-edge polynomial (3 = quadratic)
    nnorm: i32 => "bkg_nnorm",
    /// `bkg_nvict`: Victoreen exponent (Larch extension, not a Demeter attribute)
    nvict: i32 => "bkg_nvict",
    /// `bkg_step`: edge step
    step: f64 => "bkg_step",
    /// `bkg_fitted_step`: edge step from the normalization fit
    fitted_step: f64 => "bkg_fitted_step",
    /// `bkg_fixstep`: whether the edge step is held fixed
    fixstep: bool => "bkg_fixstep",
    /// `bkg_rbkg`: AUTOBK R_bkg (Å)
    rbkg: f64 => "bkg_rbkg",
    /// `bkg_kw`: AUTOBK k-weight
    bkg_kw: i32 => "bkg_kw",
    /// `bkg_spl1`: AUTOBK spline range start in k (Å⁻¹)
    spl1: f64 => "bkg_spl1",
    /// `bkg_spl2`: AUTOBK spline range end in k (Å⁻¹)
    spl2: f64 => "bkg_spl2",
    /// `bkg_clamp1`: low-k spline clamp strength (0/3/6/12/24/96)
    clamp1: i32 => "bkg_clamp1",
    /// `bkg_clamp2`: high-k spline clamp strength (0/3/6/12/24/96)
    clamp2: i32 => "bkg_clamp2",
    /// `bkg_nclamp`: number of clamped points
    nclamp: i32 => "bkg_nclamp",
    /// `bkg_kwindow`: AUTOBK Fourier window name
    bkg_kwindow: String => "bkg_kwindow",
    /// `bkg_dk`: AUTOBK window sill width
    bkg_dk: f64 => "bkg_dk",
    /// `bkg_eshift`: energy shift (eV)
    eshift: f64 => "bkg_eshift",
    /// `bkg_flatten`: whether to flatten the normalized spectrum
    flatten: bool => "bkg_flatten",
    /// `fft_kmin`: forward FT k range start (Å⁻¹)
    fft_kmin: f64 => "fft_kmin",
    /// `fft_kmax`: forward FT k range end (Å⁻¹)
    fft_kmax: f64 => "fft_kmax",
    /// `fft_dk`: forward FT window sill width
    fft_dk: f64 => "fft_dk",
    /// `fft_kwindow`: forward FT window name
    fft_kwindow: String => "fft_kwindow",
    /// `fft_kw`: forward FT k-weight (absent in older Athena files)
    fft_kw: f64 => "fft_kw",
    /// `fft_pc`: phase correction flag
    fft_pc: bool => "fft_pc",
    /// `rmax_out`: maximum R of the output chi(R)
    rmax_out: f64 => "rmax_out",
    /// `bft_rmin`: backward FT R range start (Å)
    bft_rmin: f64 => "bft_rmin",
    /// `bft_rmax`: backward FT R range end (Å)
    bft_rmax: f64 => "bft_rmax",
    /// `bft_dr`: backward FT window sill width
    bft_dr: f64 => "bft_dr",
    /// `bft_rwindow`: backward FT window name
    bft_rwindow: String => "bft_rwindow",
    /// `importance`: merge weight
    importance: f64 => "importance",
    /// `mark`: group is marked in the Athena group list
    mark: bool => "mark",
    /// `frozen`: group parameters are frozen
    frozen: bool => "frozen",
    /// `plot_yoffset`: plot y offset
    plot_yoffset: f64 => "plot_yoffset",
    /// `plot_scale`: plot scale factor
    plot_scale: f64 => "plot_scale",
    /// `recordtype`: e.g. `mu(E)`, `chi(k)`
    recordtype: String => "recordtype",
    /// `datatype`: e.g. `xmu`, `chi`
    datatype: String => "datatype",
}

// ---------------------------------------------------------------------------
// Window names
// ---------------------------------------------------------------------------

/// Map an Athena window name to our [`FTWindow`].
pub fn window_from_name(name: &str) -> Option<FTWindow> {
    match name.trim().to_ascii_lowercase().as_str() {
        "hanning" => Some(FTWindow::Hanning),
        "parzen" => Some(FTWindow::Parzen),
        "welch" => Some(FTWindow::Welch),
        "gaussian" => Some(FTWindow::Gaussian),
        "sine" => Some(FTWindow::Sine),
        "kaiser-bessel" | "kaiser" | "kaiserbessel" | "kb" => Some(FTWindow::KaiserBessel),
        "fhanning" => Some(FTWindow::FHanning),
        _ => None,
    }
}

/// Athena window name for an [`FTWindow`].
pub fn window_name(window: FTWindow) -> &'static str {
    match window {
        FTWindow::Hanning => "hanning",
        FTWindow::Parzen => "parzen",
        FTWindow::Welch => "welch",
        FTWindow::Gaussian => "gaussian",
        FTWindow::Sine => "sine",
        FTWindow::KaiserBessel => "kaiser-bessel",
        FTWindow::FHanning => "fhanning",
    }
}

// ---------------------------------------------------------------------------
// Group
// ---------------------------------------------------------------------------

/// One data record (group) of an Athena project.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AthenaGroup {
    /// Athena hash key (`$old_group`), typically 5 lowercase letters.
    pub tag: String,
    /// Human readable label shown in Athena's group list (`label` arg).
    pub label: String,
    /// `@x`: energy (eV) for `mu(E)` records, k (Å⁻¹) for `chi(k)` records.
    pub x: Vec<f64>,
    /// `@y`: mu(E) or chi(k).
    pub y: Vec<f64>,
    /// `@i0`: incident intensity column, if present.
    pub i0: Option<Vec<f64>>,
    /// `@signal`: signal column, if present.
    pub signal: Option<Vec<f64>>,
    /// `@stddev`: standard deviation, present for merged groups.
    pub stddev: Option<Vec<f64>>,
    /// Typed view of the parameters (wins over `args` on write).
    pub params: AthenaParams,
    /// Complete raw `@args` list in file order.
    pub args: Vec<(String, AthenaValue)>,
    /// Statements inside the record we do not model (e.g. `@xdi = ...`), verbatim.
    pub extra: Vec<String>,
}

impl AthenaGroup {
    /// Raw value of an arg.
    pub fn arg(&self, key: &str) -> Option<&AthenaValue> {
        lookup(&self.args, key)
    }

    /// Scalar text of an arg.
    pub fn arg_str(&self, key: &str) -> Option<&str> {
        self.arg(key).and_then(AthenaValue::as_str)
    }

    /// Numeric value of an arg.
    pub fn arg_f64(&self, key: &str) -> Option<f64> {
        self.arg(key).and_then(AthenaValue::as_f64)
    }

    /// Set (or append) a raw arg.
    pub fn set_arg<S: Into<String>>(&mut self, key: S, value: AthenaValue) -> &mut Self {
        let key = key.into();
        match self.args.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => *v = value,
            None => self.args.push((key, value)),
        }
        self
    }

    /// Whether the record stores chi(k) rather than mu(E).
    pub fn is_chi(&self) -> bool {
        let flag = |k: &str| self.arg(k).and_then(bool::from_value).unwrap_or(false);
        flag("is_chi")
            || self.params.datatype.as_deref() == Some("chi")
            || self
                .params
                .recordtype
                .as_deref()
                .map(|r| r.starts_with("chi"))
                .unwrap_or(false)
    }

    /// The args as they will be written: raw args with the typed values,
    /// `label` and `tag` applied on top.
    pub fn merged_args(&self) -> Vec<(String, AthenaValue)> {
        let mut args = self.args.clone();
        self.params.apply_to_args(&mut args);
        // `bkg_nvict` is not a Demeter attribute: only emit when meaningful.
        if self.params.nvict == Some(0) && lookup(&self.args, "bkg_nvict").is_none() {
            args.retain(|(k, _)| k != "bkg_nvict");
        }
        apply(&mut args, "label", &self.label);
        if lookup(&args, "tag").is_none() {
            apply(&mut args, "tag", &self.tag);
        }
        args
    }

    /// Convert this record into an [`XASSpectrum`] configured with the
    /// Athena normalization, background and Fourier transform parameters.
    ///
    /// `mu(E)` records set the raw energy/mu arrays; `chi(k)` records set
    /// `k`/`chi` directly. The Athena energy shift (`bkg_eshift`) is stored
    /// in the parameters but, like Larch, not applied to the energy array.
    pub fn to_spectrum(&self) -> Result<XASSpectrum, XAFSError> {
        if self.x.len() != self.y.len() {
            return Err(DataError::LengthMismatch {
                energy_len: self.x.len(),
                mu_len: self.y.len(),
            }
            .into());
        }
        let p = &self.params;
        let mut spectrum = XASSpectrum::new();
        spectrum.set_name(self.label.clone());

        if self.is_chi() {
            spectrum.k = Some(DVector::from_vec(self.x.clone()));
            spectrum.chi = Some(DVector::from_vec(self.y.clone()));
        } else {
            spectrum.set_spectrum(self.x.clone(), self.y.clone());
        }
        if let Some(e0) = p.e0 {
            spectrum.set_e0(e0);
        }

        let d = PrePostEdge::default();
        let norm = PrePostEdge {
            e0: p.e0,
            pre_edge_start: p.pre1.or(d.pre_edge_start),
            pre_edge_end: p.pre2.or(d.pre_edge_end),
            norm_start: p.nor1.or(d.norm_start),
            norm_end: p.nor2.or(d.norm_end),
            norm_polyorder: p.nnorm.map(|n| (n - 1).max(0)).or(d.norm_polyorder),
            n_victoreen: p.nvict.or(d.n_victoreen),
            edge_step: if p.fixstep == Some(true) {
                p.step
            } else {
                None
            },
            ..d
        };
        spectrum.normalization = Some(NormalizationMethod::PrePostEdge(norm));

        let d = AUTOBK::default();
        let autobk = AUTOBK {
            ek0: p.e0,
            rbkg: p.rbkg.or(d.rbkg),
            kmin: p.spl1.or(d.kmin),
            kmax: p.spl2.or(d.kmax),
            kweight: p.bkg_kw.or(d.kweight),
            dk: p.bkg_dk.or(d.dk),
            nclamp: p.nclamp.or(d.nclamp),
            clamp_lo: p.clamp1.or(d.clamp_lo),
            clamp_hi: p.clamp2.or(d.clamp_hi),
            window: p
                .bkg_kwindow
                .as_deref()
                .and_then(window_from_name)
                .unwrap_or(d.window),
            ..d
        };
        spectrum.background = Some(BackgroundMethod::AUTOBK(autobk));

        let d = XrayFFTF::default();
        let xftf = XrayFFTF {
            kmin: p.fft_kmin.or(d.kmin),
            kmax: p.fft_kmax.or(d.kmax),
            dk: p.fft_dk.or(d.dk),
            kweight: p.fft_kw.or(d.kweight),
            rmax_out: p.rmax_out.or(d.rmax_out),
            window: p
                .fft_kwindow
                .as_deref()
                .and_then(window_from_name)
                .or(d.window),
            ..d
        };
        spectrum.xftf = Some(xftf);

        let d = XrayFFTR::default();
        let xftr = XrayFFTR {
            rmin: p.bft_rmin.or(d.rmin),
            rmax: p.bft_rmax.or(d.rmax),
            dr: p.bft_dr.or(d.dr),
            window: p
                .bft_rwindow
                .as_deref()
                .and_then(window_from_name)
                .or(d.window),
            ..d
        };
        spectrum.xftr = Some(xftr);

        Ok(spectrum)
    }

    /// Build an Athena record from an [`XASSpectrum`].
    ///
    /// The raw energy/mu arrays become `@x`/`@y`; normalization, AUTOBK and
    /// FFT settings are translated to Athena keys; everything else gets
    /// Athena defaults so that Athena and Larch can open the file.
    pub fn from_spectrum(spectrum: &XASSpectrum, tag: &str) -> Result<AthenaGroup, IOError> {
        let energy = spectrum
            .raw_energy
            .as_ref()
            .or(spectrum.energy.as_ref())
            .ok_or_else(|| IOError::AthenaExport {
                reason: "spectrum has no energy array".to_string(),
            })?;
        let mu = spectrum
            .raw_mu
            .as_ref()
            .or(spectrum.mu.as_ref())
            .ok_or_else(|| IOError::AthenaExport {
                reason: "spectrum has no mu array".to_string(),
            })?;
        if energy.len() != mu.len() {
            return Err(IOError::AthenaExport {
                reason: format!("energy has {} points but mu has {}", energy.len(), mu.len()),
            });
        }

        let label = spectrum.name.clone().unwrap_or_else(|| tag.to_string());
        let x: Vec<f64> = energy.iter().copied().collect();
        let y: Vec<f64> = mu.iter().copied().collect();
        let (xmin, xmax) = x
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &v| {
                (lo.min(v), hi.max(v))
            });

        let e0 = spectrum
            .e0
            .or_else(|| spectrum.normalization.as_ref().and_then(|n| n.get_e0()));
        let edge_step = spectrum
            .normalization
            .as_ref()
            .and_then(|n| n.get_edge_step());
        let ppe = match &spectrum.normalization {
            Some(NormalizationMethod::PrePostEdge(ppe)) => ppe.clone(),
            _ => PrePostEdge::default(),
        };
        // Only translate what the spectrum actually carries; anything absent
        // falls through to the Athena defaults in the args table below.
        let autobk = match &spectrum.background {
            Some(BackgroundMethod::AUTOBK(a)) => Some(a),
            _ => None,
        };
        let xftf = spectrum.xftf.as_ref();
        let xftr = spectrum.xftr.as_ref();

        let emax = e0.map(|e0| (xmax - e0).max(0.0)).unwrap_or(0.0);
        let kmax_data = (ETOK * emax).sqrt();

        let params = AthenaParams {
            e0,
            pre1: ppe.pre_edge_start,
            pre2: ppe.pre_edge_end,
            nor1: ppe.norm_start,
            nor2: ppe.norm_end,
            nnorm: ppe.norm_polyorder.map(|o| o + 1),
            nvict: ppe.n_victoreen,
            step: edge_step,
            fitted_step: edge_step,
            fixstep: Some(false),
            rbkg: autobk.and_then(|a| a.rbkg),
            bkg_kw: autobk.and_then(|a| a.kweight),
            spl1: autobk.and_then(|a| a.kmin),
            spl2: autobk.map(|a| a.kmax.unwrap_or(kmax_data)),
            clamp1: autobk.and_then(|a| a.clamp_lo),
            clamp2: autobk.and_then(|a| a.clamp_hi),
            nclamp: autobk.and_then(|a| a.nclamp),
            bkg_kwindow: autobk.map(|a| window_name(a.window).to_string()),
            bkg_dk: autobk.and_then(|a| a.dk),
            eshift: Some(0.0),
            flatten: Some(true),
            fft_kmin: xftf.and_then(|f| f.kmin),
            fft_kmax: xftf.and_then(|f| f.kmax),
            fft_dk: xftf.and_then(|f| f.dk),
            fft_kwindow: xftf
                .and_then(|f| f.window)
                .map(|w| window_name(w).to_string()),
            fft_kw: xftf.and_then(|f| f.kweight),
            fft_pc: Some(false),
            rmax_out: xftf.and_then(|f| f.rmax_out),
            bft_rmin: xftr.and_then(|f| f.rmin),
            bft_rmax: xftr.and_then(|f| f.rmax),
            bft_dr: xftr.and_then(|f| f.dr),
            bft_rwindow: xftr
                .and_then(|f| f.window)
                .map(|w| window_name(w).to_string()),
            importance: Some(1.0),
            mark: Some(false),
            frozen: Some(false),
            plot_yoffset: Some(0.0),
            plot_scale: Some(1.0),
            recordtype: Some("mu(E)".to_string()),
            datatype: Some("xmu".to_string()),
        };

        let q = |s: &str| AthenaValue::Quoted(s.to_string());
        let b = |s: &str| AthenaValue::Bare(s.to_string());
        let args: Vec<(String, AthenaValue)> = [
            ("tag", q(tag)),
            ("datagroup", q(tag)),
            ("label", q(label.as_str())),
            ("npts", b(&x.len().to_string())),
            ("xmin", q(&format!("{xmin}"))),
            ("xmax", q(&format!("{xmax}"))),
            ("recordtype", q("mu(E)")),
            ("datatype", q("xmu")),
            ("is_xmu", b("1")),
            ("is_chi", b("0")),
            ("is_col", b("0")),
            ("is_nor", b("0")),
            ("is_merge", q("")),
            ("is_kev", q("0")),
            ("is_fit", b("0")),
            ("is_pixel", b("0")),
            ("is_special", b("0")),
            ("importance", b("1")),
            ("mark", b("0")),
            ("marked", b("0")),
            ("frozen", b("0")),
            ("plot_scale", b("1")),
            ("plot_yoffset", b("0")),
            ("plotkey", q("")),
            ("plotspaces", q("any")),
            ("bkg_algorithm", q("autobk")),
            ("bkg_e0", q("0")),
            ("bkg_e0_fraction", q("0.5")),
            ("bkg_former_e0", b("0")),
            ("bkg_tie_e0", b("0")),
            ("bkg_eshift", b("0")),
            ("bkg_delta_eshift", b("0")),
            ("bkg_pre1", q("-200")),
            ("bkg_pre2", q("-30")),
            ("bkg_nor1", q("150")),
            ("bkg_nor2", q("2000")),
            ("bkg_nnorm", q("3")),
            ("bkg_step", q("0")),
            ("bkg_fitted_step", q("0")),
            ("bkg_fixstep", b("0")),
            ("bkg_flatten", q("1")),
            ("bkg_rbkg", b("1")),
            ("bkg_kw", b("1")),
            ("bkg_spl1", q("0")),
            ("bkg_spl2", q(&format!("{kmax_data}"))),
            ("bkg_spl1e", q("0")),
            ("bkg_spl2e", q(&format!("{emax}"))),
            ("bkg_clamp1", b("0")),
            ("bkg_clamp2", b("24")),
            ("bkg_nclamp", b("5")),
            ("bkg_kwindow", q("hanning")),
            ("bkg_dk", b("1")),
            ("bkg_cl", b("0")),
            ("bkg_stan", q("None")),
            ("bkg_nc0", q("0")),
            ("bkg_nc1", q("0")),
            ("bkg_nc2", q("0")),
            ("bkg_nc3", b("0")),
            ("bkg_slope", q("0")),
            ("bkg_int", q("0")),
            ("fft_edge", q("k")),
            ("fft_kmin", q("2")),
            ("fft_kmax", q("15")),
            ("fft_dk", q("1")),
            ("fft_kw", q("2")),
            ("fft_kwindow", q("kaiser-bessel")),
            ("fft_pc", b("0")),
            ("fft_pctype", q("central")),
            ("fft_pcpathgroup", q("")),
            ("rmax_out", q("10")),
            ("bft_rmin", q("1")),
            ("bft_rmax", q("3")),
            ("bft_dr", q("0.0")),
            ("bft_rwindow", q("hanning")),
            ("update_data", b("0")),
            ("update_columns", b("0")),
            ("update_norm", b("1")),
            ("update_bkg", b("1")),
            ("update_fft", b("1")),
            ("update_bft", b("1")),
            ("from_athena", b("0")),
            ("from_yaml", b("0")),
            ("generated", b("0")),
            ("rebinned", b("0")),
            ("quenched", b("0")),
            ("quickmerge", b("0")),
            ("read_as_raw", b("0")),
            ("tying", b("0")),
            ("unreadable", b("0")),
            ("collided", b("0")),
            ("forcekey", b("0")),
            ("display", b("0")),
            ("merge_weight", b("1")),
            ("multiplier", q("1")),
            ("i0_scale", q("1")),
            ("signal_scale", q("1")),
            ("ln", q("0")),
            ("inv", q("0")),
            ("energy", q("$1")),
            ("numerator", q("$2")),
            ("denominator", q("1")),
            ("energy_string", q("")),
            ("i0_string", q("")),
            ("signal_string", q("")),
            ("xmu_string", q("")),
            ("chi_string", q("")),
            ("chi_column", q("")),
            ("columns", q("")),
            ("titles", AthenaValue::List(vec![q("data from rexafs")])),
            ("file", q(label.as_str())),
            ("source", q(label.as_str())),
            ("provenance", q("rexafs")),
            ("prjrecord", q("")),
            ("referencegroup", q("")),
            ("beamline", q("")),
            ("beamline_identified", b("0")),
            ("daq", q("")),
            ("xdifile", q("")),
            ("xdi_will_be_cloned", b("0")),
            ("annotation", q("")),
            ("trouble", q("")),
            ("epsk", q("")),
            ("epsr", q("")),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

        let mut group = AthenaGroup {
            tag: tag.to_string(),
            label,
            x,
            y,
            i0: None,
            signal: None,
            stddev: None,
            params,
            args,
            extra: Vec::new(),
        };
        group.args = group.merged_args();
        Ok(group)
    }
}

// ---------------------------------------------------------------------------
// Project
// ---------------------------------------------------------------------------

/// An Athena project: header, data groups and journal.
#[derive(Debug, Clone, PartialEq)]
pub struct AthenaProject {
    /// Demeter version from the first header line (`0.9.26`).
    pub version: String,
    /// Header comment lines as read (without the leading `# `), informational.
    pub header: Vec<String>,
    /// Data records in file order.
    pub groups: Vec<AthenaGroup>,
    /// Journal lines.
    pub journal: Vec<String>,
    /// Top-level statements we do not model (`@indicator`, `@plot_features`, ...), verbatim.
    pub extra: Vec<String>,
}

impl Default for AthenaProject {
    fn default() -> Self {
        Self {
            version: DEFAULT_DEMETER_VERSION.to_string(),
            header: Vec::new(),
            groups: Vec::new(),
            journal: Vec::new(),
            extra: Vec::new(),
        }
    }
}

impl AthenaProject {
    /// Empty project.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read a project file (gzip-compressed or plain text).
    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self, IOError> {
        let path = path.as_ref();
        let mut file = std::fs::File::open(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => IOError::FileNotFound {
                path: path.display().to_string(),
            },
            kind => IOError::ReadFailed {
                path: path.display().to_string(),
                kind,
            },
        })?;
        Self::read_from(&mut file).map_err(|e| match e {
            IOError::ReadFailed { kind, .. } => IOError::ReadFailed {
                path: path.display().to_string(),
                kind,
            },
            other => other,
        })
    }

    /// Read a project from any reader (gzip-compressed or plain text).
    pub fn read_from<R: Read + ?Sized>(reader: &mut R) -> Result<Self, IOError> {
        let read_err = |e: std::io::Error| IOError::ReadFailed {
            path: "<reader>".to_string(),
            kind: e.kind(),
        };
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).map_err(read_err)?;
        let text = if bytes.starts_with(&[0x1f, 0x8b]) {
            let mut text = String::new();
            GzDecoder::new(bytes.as_slice())
                .read_to_string(&mut text)
                .map_err(|e| IOError::CompressionError {
                    message: e.to_string(),
                })?;
            text
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        };
        Self::from_text(&text)
    }

    /// Parse the (decompressed) project text.
    pub fn from_text(text: &str) -> Result<Self, IOError> {
        Parser::new(text).parse()
    }

    /// Write the project, gzip-compressed, to `path`.
    pub fn write<P: AsRef<Path>>(&self, path: P) -> Result<(), IOError> {
        let path = path.as_ref();
        let write_err = |e: std::io::Error| IOError::WriteFailed {
            path: path.display().to_string(),
            kind: e.kind(),
        };
        let mut file = std::fs::File::create(path).map_err(write_err)?;
        self.write_to(&mut file).map_err(|e| match e {
            IOError::WriteFailed { kind, .. } => IOError::WriteFailed {
                path: path.display().to_string(),
                kind,
            },
            other => other,
        })
    }

    /// Write the project, gzip-compressed, to any writer.
    pub fn write_to<W: Write + ?Sized>(&self, writer: &mut W) -> Result<(), IOError> {
        let write_err = |e: std::io::Error| IOError::WriteFailed {
            path: "<writer>".to_string(),
            kind: e.kind(),
        };
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(self.to_text().as_bytes())
            .map_err(write_err)?;
        let bytes = encoder.finish().map_err(|e| IOError::CompressionError {
            message: e.to_string(),
        })?;
        writer.write_all(&bytes).map_err(write_err)?;
        writer.flush().map_err(write_err)
    }

    /// Render the (uncompressed) project text.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# Athena project file -- Demeter version {}\n",
            self.version
        ));
        out.push_str(&format!("# This file created at {}\n", iso_timestamp_utc()));
        out.push_str(&format!(
            "# Using rexafs {} on {}\n",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS
        ));
        out.push('\n');

        for group in &self.groups {
            out.push_str(&format!(
                "$old_group = {};\n",
                AthenaValue::Quoted(group.tag.clone())
            ));
            out.push_str("@args = (");
            for (i, (k, v)) in group.merged_args().iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&format!("{},{}", AthenaValue::Quoted(k.clone()), v));
            }
            out.push_str(");\n");
            push_array(&mut out, "x", &group.x);
            push_array(&mut out, "y", &group.y);
            if let Some(i0) = &group.i0 {
                push_array(&mut out, "i0", i0);
            }
            if let Some(signal) = &group.signal {
                push_array(&mut out, "signal", signal);
            }
            if let Some(stddev) = &group.stddev {
                push_array(&mut out, "stddev", stddev);
            }
            for statement in &group.extra {
                out.push_str(statement);
                out.push('\n');
            }
            out.push_str("[record]   # create object and set arrays in ifeffit\n\n");
        }

        for statement in &self.extra {
            out.push_str(statement);
            out.push('\n');
        }
        out.push_str("@journal = (");
        for (i, line) in self.journal.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&AthenaValue::Quoted(line.clone()).to_string());
        }
        out.push_str(");\n\n1;\n\n\n# Local Variables:\n# truncate-lines: t\n# End:\n");
        out
    }

    /// Build a project from spectra, one group per spectrum.
    pub fn from_spectra(spectra: &[XASSpectrum]) -> Result<Self, IOError> {
        let mut project = Self::new();
        for (index, spectrum) in spectra.iter().enumerate() {
            let tag = project.unique_tag(index);
            project
                .groups
                .push(AthenaGroup::from_spectrum(spectrum, &tag)?);
        }
        Ok(project)
    }

    /// Convert every group to an [`XASSpectrum`].
    pub fn to_spectra(&self) -> Result<Vec<XASSpectrum>, XAFSError> {
        self.groups.iter().map(AthenaGroup::to_spectrum).collect()
    }

    /// Group lookup by label.
    pub fn group_by_label(&self, label: &str) -> Option<&AthenaGroup> {
        self.groups.iter().find(|g| g.label == label)
    }

    /// A 5-letter Athena-style hash key not yet used in this project.
    fn unique_tag(&self, seed: usize) -> String {
        let mut state = (seed as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(iso_seed());
        loop {
            let mut tag = String::with_capacity(5);
            for _ in 0..5 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                tag.push((b'a' + (state % 26) as u8) as char);
            }
            if !self.groups.iter().any(|g| g.tag == tag) {
                return tag;
            }
        }
    }
}

fn push_array(out: &mut String, name: &str, values: &[f64]) {
    out.push('@');
    out.push_str(name);
    out.push_str(" = (");
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('\'');
        out.push_str(&fmt_num(*v));
        out.push('\'');
    }
    out.push_str(");\n");
}

fn iso_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x1234_5678)
        | 1
}

/// `YYYY-MM-DDTHH:MM:SS` in UTC without pulling in a date crate.
fn iso_timestamp_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mo <= 2);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}")
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    text: &'a str,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self { text }
    }

    fn err(line: usize, message: impl Into<String>) -> IOError {
        IOError::AthenaParse {
            line,
            message: message.into(),
        }
    }

    fn parse(self) -> Result<AthenaProject, IOError> {
        let first = self.text.lines().next().unwrap_or("");
        if !first.contains("Athena project file -- ") {
            return Err(IOError::NotAthenaProject {
                reason: "missing 'Athena project file' header".to_string(),
            });
        }
        let version = first
            .split("version")
            .nth(1)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| DEFAULT_DEMETER_VERSION.to_string());

        let mut project = AthenaProject {
            version,
            ..Default::default()
        };
        let mut current: Option<AthenaGroup> = None;
        let mut pending: Option<(usize, String)> = None;
        let mut in_header = true;

        for (idx, raw_line) in self.text.lines().enumerate() {
            let line_no = idx + 1;
            let line = raw_line.trim_end_matches('\r');

            if let Some((start, buf)) = pending.as_mut() {
                buf.push('\n');
                buf.push_str(line);
                if statement_complete(buf) {
                    let (start, buf) = (*start, std::mem::take(buf));
                    pending = None;
                    self.handle_statement(&buf, start, &mut project, &mut current)?;
                }
                continue;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(comment) = trimmed.strip_prefix('#') {
                if in_header {
                    project.header.push(comment.trim().to_string());
                }
                continue;
            }
            in_header = false;
            if trimmed.starts_with("[record]") {
                match current.take() {
                    Some(group) => project.groups.push(group),
                    None => return Err(Self::err(line_no, "[record] without $old_group")),
                }
                continue;
            }
            if statement_complete(trimmed) {
                self.handle_statement(trimmed, line_no, &mut project, &mut current)?;
            } else {
                pending = Some((line_no, trimmed.to_string()));
            }
        }
        if let Some((line_no, _)) = pending {
            return Err(Self::err(line_no, "unterminated statement"));
        }
        if let Some(group) = current.take() {
            project.groups.push(group);
        }
        Ok(project)
    }

    fn handle_statement(
        &self,
        statement: &str,
        line_no: usize,
        project: &mut AthenaProject,
        current: &mut Option<AthenaGroup>,
    ) -> Result<(), IOError> {
        let body = statement.trim().trim_end_matches(';').trim();
        if body == "1" {
            return Ok(());
        }
        let Some((lhs, rhs)) = body.split_once('=') else {
            return Err(Self::err(
                line_no,
                format!("unrecognised statement: {statement}"),
            ));
        };
        let key = lhs.trim().trim_start_matches(['$', '@', '%']).trim();
        let rhs = rhs.trim();

        match key {
            "old_group" => {
                if let Some(group) = current.take() {
                    project.groups.push(group);
                }
                let tag = match parse_scalar(rhs, line_no)? {
                    AthenaValue::Quoted(s) | AthenaValue::Bare(s) => s,
                    AthenaValue::List(_) => {
                        return Err(Self::err(line_no, "$old_group must be a string"))
                    }
                };
                *current = Some(AthenaGroup {
                    tag,
                    ..Default::default()
                });
            }
            "args" => {
                let group = current
                    .as_mut()
                    .ok_or_else(|| Self::err(line_no, "@args outside of a group"))?;
                let items = parse_list(rhs, line_no)?;
                if items.len() % 2 != 0 {
                    return Err(Self::err(line_no, "@args has an odd number of items"));
                }
                let mut args = Vec::with_capacity(items.len() / 2);
                let mut iter = items.into_iter();
                while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
                    let key = k
                        .as_str()
                        .map(str::to_string)
                        .ok_or_else(|| Self::err(line_no, "@args key must be a string"))?;
                    args.push((key, v));
                }
                group.params = AthenaParams::from_args(&args);
                if let Some(label) = lookup(&args, "label").and_then(AthenaValue::as_str) {
                    group.label = label.to_string();
                } else {
                    group.label = group.tag.clone();
                }
                group.args = args;
            }
            "x" | "y" | "i0" | "signal" | "stddev" => {
                let group = current
                    .as_mut()
                    .ok_or_else(|| Self::err(line_no, format!("@{key} outside of a group")))?;
                let values = parse_list(rhs, line_no)?
                    .iter()
                    .map(|v| {
                        v.as_f64().ok_or_else(|| {
                            Self::err(line_no, format!("non-numeric value in @{key}: {v}"))
                        })
                    })
                    .collect::<Result<Vec<f64>, _>>()?;
                match key {
                    "x" => group.x = values,
                    "y" => group.y = values,
                    "i0" => group.i0 = Some(values),
                    "signal" => group.signal = Some(values),
                    _ => group.stddev = Some(values),
                }
            }
            "journal" => {
                project.journal = parse_list(rhs, line_no)?
                    .into_iter()
                    .map(|v| match v {
                        AthenaValue::Quoted(s) | AthenaValue::Bare(s) => s,
                        list @ AthenaValue::List(_) => list.to_string(),
                    })
                    .collect();
            }
            _ => match current.as_mut() {
                Some(group) => group.extra.push(statement.trim().to_string()),
                None => project.extra.push(statement.trim().to_string()),
            },
        }
        Ok(())
    }
}

/// True when `s` ends with a `;` that is outside of any quoted string.
fn statement_complete(s: &str) -> bool {
    let mut in_quote = false;
    let mut escaped = false;
    let mut last_semicolon = false;
    for c in s.chars() {
        if in_quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '\'' {
                in_quote = false;
            }
            continue;
        }
        match c {
            '\'' => {
                in_quote = true;
                last_semicolon = false;
            }
            ';' => last_semicolon = true,
            c if c.is_whitespace() => {}
            _ => last_semicolon = false,
        }
    }
    !in_quote && last_semicolon
}

/// Parse a single scalar value (`'text'` or bare token).
fn parse_scalar(text: &str, line_no: usize) -> Result<AthenaValue, IOError> {
    let mut chars = text.chars().peekable();
    let mut lexer = Lexer {
        chars: &mut chars,
        line_no,
    };
    let value = lexer.value()?;
    match value {
        Some(v) => Ok(v),
        None => Err(Parser::err(line_no, "expected a value")),
    }
}

/// Parse a Perl list literal `( a, b, ... )`.
fn parse_list(text: &str, line_no: usize) -> Result<Vec<AthenaValue>, IOError> {
    let text = text.trim();
    let inner = text
        .strip_prefix('(')
        .and_then(|t| t.strip_suffix(')'))
        .ok_or_else(|| Parser::err(line_no, "expected a parenthesised list"))?;
    let mut chars = inner.chars().peekable();
    let mut lexer = Lexer {
        chars: &mut chars,
        line_no,
    };
    lexer.sequence(None)
}

struct Lexer<'a, I: Iterator<Item = char>> {
    chars: &'a mut std::iter::Peekable<I>,
    line_no: usize,
}

impl<I: Iterator<Item = char>> Lexer<'_, I> {
    fn skip_separators(&mut self) {
        while matches!(self.chars.peek(), Some(c) if c.is_whitespace() || *c == ',') {
            self.chars.next();
        }
    }

    /// Values until end of input (or until `close`).
    fn sequence(&mut self, close: Option<char>) -> Result<Vec<AthenaValue>, IOError> {
        let mut items = Vec::new();
        loop {
            self.skip_separators();
            match self.chars.peek() {
                None => {
                    return match close {
                        None => Ok(items),
                        Some(c) => Err(Parser::err(self.line_no, format!("missing '{c}'"))),
                    }
                }
                Some(&c) if Some(c) == close => {
                    self.chars.next();
                    return Ok(items);
                }
                _ => match self.value()? {
                    Some(v) => items.push(v),
                    None => return Ok(items),
                },
            }
        }
    }

    fn value(&mut self) -> Result<Option<AthenaValue>, IOError> {
        self.skip_separators();
        let Some(&c) = self.chars.peek() else {
            return Ok(None);
        };
        match c {
            '\'' => {
                self.chars.next();
                self.quoted().map(Some)
            }
            '[' => {
                self.chars.next();
                self.sequence(Some(']')).map(|v| Some(AthenaValue::List(v)))
            }
            '(' => {
                self.chars.next();
                self.sequence(Some(')')).map(|v| Some(AthenaValue::List(v)))
            }
            _ => {
                let mut token = String::new();
                while let Some(&c) = self.chars.peek() {
                    if c == ',' || c == ']' || c == ')' || c.is_whitespace() {
                        break;
                    }
                    token.push(c);
                    self.chars.next();
                }
                if token.is_empty() {
                    return Err(Parser::err(
                        self.line_no,
                        format!("unexpected character '{c}'"),
                    ));
                }
                Ok(Some(AthenaValue::Bare(token)))
            }
        }
    }

    /// Body of a single-quoted Perl string (opening quote already consumed).
    fn quoted(&mut self) -> Result<AthenaValue, IOError> {
        let mut out = String::new();
        loop {
            match self.chars.next() {
                None => return Err(Parser::err(self.line_no, "unterminated string")),
                Some('\'') => break,
                Some('\\') => match self.chars.peek() {
                    Some('\'') | Some('\\') => {
                        out.push(self.chars.next().unwrap_or('\\'));
                    }
                    Some('x') => {
                        // Perl writes non-ASCII as \x{hex}; decode like Larch does.
                        self.chars.next();
                        if self.chars.peek() == Some(&'{') {
                            self.chars.next();
                            let mut hex = String::new();
                            while let Some(&c) = self.chars.peek() {
                                self.chars.next();
                                if c == '}' {
                                    break;
                                }
                                hex.push(c);
                            }
                            match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                                Some(ch) => out.push(ch),
                                None => out.push_str(&format!("\\x{{{hex}}}")),
                            }
                        } else {
                            out.push_str("\\x");
                        }
                    }
                    _ => out.push('\\'),
                },
                Some(c) => out.push(c),
            }
        }
        Ok(AthenaValue::Quoted(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn athena_value_roundtrip_escapes() {
        let v = AthenaValue::Quoted(r"\\Mac\Home it's".to_string());
        let text = v.to_string();
        assert_eq!(text, r"'\\\\Mac\\Home it\'s'");
        let parsed = parse_scalar(&text, 1).unwrap();
        assert_eq!(parsed, v);
    }

    #[test]
    fn athena_parse_list_with_nested_array_ref() {
        let items = parse_list("('titles',[],'a',1,'b',['x','y'])", 1).unwrap();
        assert_eq!(items.len(), 6);
        assert_eq!(items[1], AthenaValue::List(vec![]));
        assert_eq!(items[3], AthenaValue::Bare("1".into()));
        assert_eq!(
            items[5],
            AthenaValue::List(vec![AthenaValue::quoted("x"), AthenaValue::quoted("y")])
        );
    }

    #[test]
    fn athena_statement_completion_respects_quotes() {
        assert!(statement_complete("@x = ('a;b');"));
        assert!(!statement_complete("@x = ('a;"));
        assert!(!statement_complete("@x = ('a')"));
    }

    #[test]
    fn athena_clamp_names() {
        assert_eq!(i32::from_value(&AthenaValue::quoted("Strong")), Some(24));
        assert_eq!(i32::from_value(&AthenaValue::quoted("None")), Some(0));
        assert_eq!(i32::from_value(&AthenaValue::bare("24")), Some(24));
    }

    #[test]
    fn athena_apply_keeps_original_text_when_equal() {
        let mut args = vec![
            ("bkg_spl1".to_string(), AthenaValue::quoted("0.000")),
            ("bkg_rbkg".to_string(), AthenaValue::bare("1")),
        ];
        apply(&mut args, "bkg_spl1", &0.0_f64);
        apply(&mut args, "bkg_rbkg", &1.5_f64);
        apply(&mut args, "fft_kw", &2.0_f64);
        assert_eq!(args[0].1, AthenaValue::quoted("0.000"));
        assert_eq!(args[1].1, AthenaValue::bare("1.5"));
        assert_eq!(args[2], ("fft_kw".to_string(), AthenaValue::quoted("2")));
    }

    #[test]
    fn athena_timestamp_shape() {
        let ts = iso_timestamp_utc();
        assert_eq!(ts.len(), 19);
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[10..11], "T");
    }
}
