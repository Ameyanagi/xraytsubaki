//! SI defining constants and the CODATA 2022 electron mass.
//!
//! Reference: <https://physics.nist.gov/cuu/pdf/wall_2022.pdf>.
//! Shared by both array backends and Athena export so energy/k conversions
//! cannot silently use different revisions of the physical constants.
#![allow(non_upper_case_globals)]

/// Planck constant (J s), exact in the SI.
pub const h: f64 = 6.62607015e-34;
/// Reduced Planck constant (J s).
pub const hbar: f64 = h / (2.0 * std::f64::consts::PI);
/// Electron mass (kg), CODATA 2022; standard uncertainty 2.8e-40 kg.
pub const m_e: f64 = 9.1093837139e-31;
/// Elementary charge (C), exact in the SI.
pub const e: f64 = 1.602176634e-19;
/// E - E0 (eV) = KTOE × k², with k in Å⁻¹.
pub const KTOE: f64 = 1.0e20 * hbar * hbar / (2.0 * m_e * e);
/// k² (Å⁻²) = ETOK × (E - E0), with energy in eV.
pub const ETOK: f64 = 1.0 / KTOE;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversion_matches_codata_2022_scipy_reference() {
        // Independently evaluated with scipy.constants (CODATA 2022).
        assert!((KTOE - 3.809982110968585).abs() < 1e-15);
        assert!((ETOK - 0.26246842396479836).abs() < 1e-16);
    }
}
