//! Built-in axis labels for publication figures, written as Typst text markup.
//!
//! ruviz sets the Typst text font to a sans-serif list (Arial first), but Typst
//! always draws math (`$…$`) in its serif New Computer Modern Math font, and
//! neither Typst nor ruviz ships a sans-serif math font. The built-in labels are
//! therefore Typst *text*: `_k_` is italic (emphasis) and `#super[−2]` is a
//! superscript, so `_k_#super[2]_χ_(_k_) (Å#super[−2])` renders k²χ(k) (Å⁻²)
//! entirely in Arial. Exponents use the Unicode minus sign U+2212 (−), which is
//! the typographic minus that math mode would otherwise insert. Labels typed by
//! users keep `$…$` as Typst math (see `layout::typst_label`).
//!
//! Typst markup reference: <https://typst.app/docs/reference/syntax/>.

/// The x-axis label for a figure key, or `None` for keys without a built-in one.
pub(super) fn x_label(key: &str) -> Option<&'static str> {
    Some(match key {
        "mu-energy" | "xanes" | "flattened-mu" => "_E_ (eV)",
        "chi-k" | "fit-k" | "residual-k" | "fit-residual-k" => "_k_ (Å#super[−1])",
        "chi-r" | "fit-r" | "residual-r" | "fit-residual-r" => "_R_ (Å)",
        "chi-q" | "fit-q" => "_q_ (Å#super[−1])",
        _ => return None,
    })
}

/// μ(E), the X-ray absorption coefficient as a function of photon energy.
pub(super) const MU_E: &str = "_μ_(_E_)";

/// A number in a label, with a typographic minus sign: `−2`, `2.5`.
fn number(value: f64) -> String {
    value.to_string().replace('-', "−")
}

/// Units of a k-weighted quantity: Å to the power −`power`, since k is in Å⁻¹.
pub(super) fn unit(power: f64) -> String {
    if power.abs() < 1e-9 {
        "(dimensionless)".into()
    } else {
        format!("(Å#super[{}])", number(-power))
    }
}

/// kʷχ(k) with its units; plain χ(k) for k-weight 0.
pub(super) fn chik(weight: f64) -> String {
    let chi = if weight.abs() < 1e-9 {
        "_χ_(_k_)".into()
    } else {
        format!("_k_#super[{}]_χ_(_k_)", number(weight))
    };
    format!("{chi} {}", unit(weight))
}

/// |χ(R)|, the Fourier-transform magnitude of kʷχ(k), in Å^−(w+1).
pub(super) fn chir(weight: f64) -> String {
    format!("|_χ_(_R_)| {}", unit(weight + 1.))
}

/// Re χ(q), the real part of the back-transformed χ.
pub(super) fn chiq(weight: f64) -> String {
    format!("Re _χ_(_q_) {}", unit(weight))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_labels_are_text_markup_not_math() {
        assert_eq!(x_label("chi-k"), Some("_k_ (Å#super[−1])"));
        assert_eq!(x_label("custom"), None);
        assert_eq!(chik(2.), "_k_#super[2]_χ_(_k_) (Å#super[−2])");
        assert_eq!(chik(0.), "_χ_(_k_) (dimensionless)");
        assert_eq!(chik(2.5), "_k_#super[2.5]_χ_(_k_) (Å#super[−2.5])");
        assert_eq!(chir(2.), "|_χ_(_R_)| (Å#super[−3])");
        assert_eq!(chiq(1.), "Re _χ_(_q_) (Å#super[−1])");
        for label in [chik(3.), chir(0.), chiq(2.), MU_E.into()] {
            assert!(!label.contains('$'), "{label}");
        }
    }
}
