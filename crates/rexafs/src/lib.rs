//! Rust-powered X-ray absorption analysis.
//!
//! Start with [`Spectrum`]: calling `fft()` computes missing prerequisite stages.
//! Advanced APIs live in [`fitting`], [`structure`] and [`io`].
//! Developed under the codename xraytsubaki.

pub mod parser;
#[cfg(feature = "plotting")]
pub mod plot;
pub mod prelude;
pub mod xafs;

pub use xafs::background::{BackgroundMethod, AUTOBK};
pub use xafs::normalization::{NormalizationMethod, PrePostEdge};
pub use xafs::xasgroup::XASGroup as Group;
pub use xafs::xasspectrum::XASSpectrum as Spectrum;
pub use xafs::xrayfft::XrayFFTF;
pub use xafs::{analysis, fitting, io, structure, tools, Result, XAFSError as Error};
