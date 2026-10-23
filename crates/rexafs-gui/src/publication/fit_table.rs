//! Per-path EXAFS fit parameter table for manuscripts (LaTeX, Word and CSV).
//!
//! Each row is one scattering path of a recorded fit; the columns are the
//! coordination number N, the amplitude reduction factor S₀², the energy
//! shift ΔE₀ (eV), the fitted half path length R = R_eff + ΔR (Å) and the
//! mean-square relative displacement σ² (Å²). Values come from the physical
//! path values archived with each fit ([`crate::fit_details::PathFitDetails`]).
//!
//! # Rounding rule
//!
//! A fitted value with standard error u is rounded as follows (see
//! [`round_pair`]):
//!
//! 1. Round u to one significant figure, or to two when its leading digit is 1
//!    (0.0435 → 0.04, 0.0149 → 0.015). When rounding carries u up to a power of
//!    ten (0.0096 → 0.01), the result starts with 1 and keeps two figures (0.010).
//! 2. Round the value to the same decimal place as the rounded uncertainty
//!    (2.5523 ± 0.0149 → 2.552 ± 0.015).
//!
//! The one-or-two-figure rule follows J. R. Taylor, *An Introduction to Error
//! Analysis*, 2nd ed. (University Science Books, 1997), §2.2, rules (2.5) and
//! (2.9). It is a stricter form of the BIPM *Guide to the Expression of
//! Uncertainty in Measurement* (JCGM 100:2008, §7.2.6,
//! <https://doi.org/10.59161/JCGM100-2008E>), which asks for at most two
//! significant digits in the uncertainty and estimates rounded to be consistent
//! with it. The choice of Taylor's variant is a rexafs default.
//!
//! The uncertainty is one standard error from the fit covariance, propagated to
//! constrained path expressions (see [`crate::fit_details::estimate`]); it does
//! not include all experimental or model uncertainty. Fixed parameters are
//! marked "(fixed)" instead of "± 0". A value without a usable uncertainty
//! (missing, zero, infinite or NaN) is given to four significant figures and
//! flagged with a dagger (†) and a note. The CSV keeps the unrounded numbers.
use crate::fit_details::{Estimate, PathFitDetails};
use crate::fitting::FitHistoryEntry;
use std::io::Write;

/// One table cell after the rounding rule.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Rounded {
    /// Fitted value and its uncertainty, rounded to the same decimal place.
    Fitted { value: String, uncertainty: String },
    /// A parameter held constant during the fit.
    Fixed { value: String },
    /// A finite value whose uncertainty is missing, zero or not finite.
    NoUncertainty { value: String },
    /// No finite value (not recorded, or NaN).
    Unavailable,
}

/// Leading digit and decimal exponent of a positive finite number, read from
/// its shortest exact decimal form so binary rounding cannot misplace them.
fn leading(x: f64) -> (u32, i32) {
    let text = format!("{x:e}");
    let (mantissa, exponent) = text.split_once('e').expect("{:e} has an exponent");
    (
        mantissa
            .chars()
            .next()
            .and_then(|c| c.to_digit(10))
            .unwrap_or(0),
        exponent.parse().unwrap_or(0),
    )
}

/// Format `x` with `decimals` digits after the decimal point; a negative count
/// rounds to tens, hundreds, ... Negative zero is written as zero.
fn fixed_decimals(x: f64, decimals: i32) -> String {
    let text = if decimals >= 0 {
        format!("{:.*}", decimals as usize, x)
    } else {
        let scale = 10f64.powi(-decimals);
        format!("{:.0}", (x / scale).round() * scale)
    };
    match text.strip_prefix('-') {
        Some(rest) if rest.chars().all(|c| c == '0' || c == '.') => rest.to_string(),
        _ => text,
    }
}

/// A value without an uncertainty: four significant figures, trailing zeros
/// after the decimal point removed (12 → "12", 0.9 → "0.9").
pub(crate) fn plain(value: f64) -> String {
    if value == 0. {
        return "0".into();
    }
    let (_, exponent) = leading(value.abs());
    let text = fixed_decimals(value, (3 - exponent).max(0));
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    }
}

/// Apply the module's rounding rule to a value and its standard error.
pub(crate) fn round_pair(value: f64, uncertainty: Option<f64>) -> Rounded {
    if !value.is_finite() {
        return Rounded::Unavailable;
    }
    let Some(u) = uncertainty.filter(|u| u.is_finite() && *u > 0.) else {
        return Rounded::NoUncertainty {
            value: plain(value),
        };
    };
    let (digit, exponent) = leading(u);
    let figures = if digit == 1 { 2 } else { 1 };
    // Rounding at this place also handles a carry: 0.0096 → "0.010", which
    // keeps the two figures the rule asks for when the result starts with 1.
    let decimals = figures - 1 - exponent;
    Rounded::Fitted {
        value: fixed_decimals(value, decimals),
        uncertainty: fixed_decimals(u, decimals),
    }
}

fn round_estimate(estimate: Option<&Estimate>) -> Rounded {
    match estimate {
        None => Rounded::Unavailable,
        Some(e) if e.fixed && e.value.is_finite() => Rounded::Fixed {
            value: plain(e.value),
        },
        Some(e) => round_pair(e.value, e.stderr),
    }
}

/// Columns: CSV key, plain-text header, LaTeX header and the path value.
type Column = (
    &'static str,
    &'static str,
    &'static str,
    fn(&PathFitDetails) -> Option<&Estimate>,
);
const COLUMNS: [Column; 5] = [
    ("N", "N", "$N$", |p| p.effective_degen.as_ref()),
    ("S02", "S₀²", "$S_0^2$", |p| p.s02.as_ref()),
    ("dE0_eV", "ΔE₀ (eV)", "$\\Delta E_0$ (eV)", |p| {
        p.e0.as_ref()
    }),
    ("R_A", "R (Å)", "$R$ (\\AA)", |p| p.distance.as_ref()),
    ("sigma2_A2", "σ² (Å²)", "$\\sigma^2$ (\\AA$^2$)", |p| {
        p.sigma2.as_ref()
    }),
];

const UNAVAILABLE_NOTE: &str = "† Uncertainty unavailable (not estimated by the fit covariance, zero or not finite); value given to four significant figures.";

/// Caption: fit identity, quality statistics and the uncertainty convention.
pub(crate) fn caption(fit: &FitHistoryEntry) -> String {
    format!(
        "EXAFS fit parameters (fit {}, {}). R factor = {}; reduced χ² = {}; N_idp = {:.1}; N_vary = {}. R = R_eff + ΔR is half the total scattering path length. Uncertainties are one standard error from the fit covariance; the uncertainty is rounded to one significant figure (two if its leading digit is 1) and the value to the same decimal place.",
        fit.id,
        fit.group,
        plain(fit.r_factor),
        fit.reduced_chi_square_text(),
        fit.n_idp,
        fit.n_vary
    )
}

fn rows(fit: &FitHistoryEntry) -> impl Iterator<Item = (String, [Rounded; 5])> + '_ {
    fit.path_details
        .iter()
        .map(|p| (p.title(), COLUMNS.map(|(.., get)| round_estimate(get(p)))))
}

fn has_note(fit: &FitHistoryEntry) -> bool {
    rows(fit).any(|(_, cells)| {
        cells
            .iter()
            .any(|c| matches!(c, Rounded::NoUncertainty { .. }))
    })
}

fn text_cell(cell: &Rounded) -> String {
    match cell {
        Rounded::Fitted { value, uncertainty } => format!("{value} ± {uncertainty}"),
        Rounded::Fixed { value } => format!("{value} (fixed)"),
        Rounded::NoUncertainty { value } => format!("{value}†"),
        Rounded::Unavailable => "—".into(),
    }
}

/// Escape LaTeX special characters and map the Unicode used in path names.
fn latex_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '–' => out.push_str("--"),
            '—' => out.push_str("---"),
            'Å' => out.push_str("\\AA{}"),
            '·' => out.push_str("\\ensuremath{\\cdot}"),
            'χ' => out.push_str("\\ensuremath{\\chi}"),
            'Δ' => out.push_str("\\ensuremath{\\Delta}"),
            'σ' => out.push_str("\\ensuremath{\\sigma}"),
            '±' => out.push_str("\\ensuremath{\\pm}"),
            '²' => out.push_str("\\ensuremath{^2}"),
            '₀' => out.push_str("\\ensuremath{_0}"),
            '†' => out.push_str("\\ensuremath{^\\dagger}"),
            _ => out.push(c),
        }
    }
    out
}

fn latex_cell(cell: &Rounded) -> String {
    match cell {
        Rounded::Fitted { value, uncertainty } => format!("${value} \\pm {uncertainty}$"),
        Rounded::Fixed { value } => format!("${value}$ (fixed)"),
        Rounded::NoUncertainty { value } => format!("${value}^{{\\dagger}}$"),
        Rounded::Unavailable => "---".into(),
    }
}

/// A booktabs `table` environment; requires `\usepackage{booktabs}`.
pub(crate) fn latex(fit: &FitHistoryEntry) -> String {
    let mut out = format!(
        "% EXAFS fit parameter table generated by rexafs {}.\n% Requires \\usepackage{{booktabs}} in the preamble.\n\\begin{{table}}\n  \\centering\n  \\caption{{{}}}\n  \\label{{tab:exafs-fit-{}}}\n  \\begin{{tabular}}{{lccccc}}\n    \\toprule\n    Path",
        env!("CARGO_PKG_VERSION"),
        latex_text(&caption(fit)),
        fit.id
    );
    for (_, _, header, _) in COLUMNS {
        out.push_str(" & ");
        out.push_str(header);
    }
    out.push_str(" \\\\\n    \\midrule\n");
    for (path, cells) in rows(fit) {
        out.push_str("    ");
        out.push_str(&latex_text(&path));
        for cell in &cells {
            out.push_str(" & ");
            out.push_str(&latex_cell(cell));
        }
        out.push_str(" \\\\\n");
    }
    out.push_str("    \\bottomrule\n  \\end{tabular}\n");
    if has_note(fit) {
        out.push_str(&format!(
            "\n  \\par\\smallskip\\footnotesize {}\n",
            latex_text(UNAVAILABLE_NOTE)
        ));
    }
    out.push_str("\\end{table}\n");
    out
}

fn number(value: Option<f64>) -> String {
    value
        .filter(|v| v.is_finite())
        .map(|v| format!("{v:.16e}"))
        .unwrap_or_default()
}

/// Long-format CSV: one row per path and parameter, with unrounded value and
/// standard error (17 significant digits, enough to recover each f64), the rounded pair and a status of
/// `fitted`, `fixed`, `no_uncertainty` or `unavailable`.
pub(crate) fn csv(fit: &FitHistoryEntry) -> String {
    let quote = crate::fitting::csv_field;
    let mut out = String::from(
        "fit_id,group,path,parameter,unit,value,uncertainty,value_rounded,uncertainty_rounded,status\n",
    );
    for p in &fit.path_details {
        for (key, _, _, get) in COLUMNS {
            let estimate = get(p);
            let unit = match key {
                "dE0_eV" => "eV",
                "R_A" => "Å",
                "sigma2_A2" => "Å²",
                _ => "",
            };
            let parameter = key.split('_').next().unwrap_or(key);
            let (value, unc, status) = match round_estimate(estimate) {
                Rounded::Fitted { value, uncertainty } => (value, uncertainty, "fitted"),
                Rounded::Fixed { value } => (value, String::new(), "fixed"),
                Rounded::NoUncertainty { value } => (value, String::new(), "no_uncertainty"),
                Rounded::Unavailable => (String::new(), String::new(), "unavailable"),
            };
            out.push_str(&format!(
                "{},{},{},{parameter},{unit},{},{},{value},{unc},{status}\n",
                fit.id,
                quote(&fit.group),
                quote(&p.title()),
                number(estimate.map(|e| e.value)),
                number(estimate.filter(|e| !e.fixed).and_then(|e| e.stderr)),
            ));
        }
    }
    out
}

fn xml(text: &str) -> String {
    crate::publication::report::html(text)
}

fn docx_paragraph(text: &str, bold: bool, size_half_points: u32) -> String {
    format!(
        "<w:p><w:r><w:rPr>{}<w:sz w:val=\"{size_half_points}\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        if bold { "<w:b/>" } else { "" },
        xml(text)
    )
}

fn docx_cell(text: &str, header: bool) -> String {
    let border = if header {
        "<w:tcPr><w:tcBorders><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"0\" w:color=\"000000\"/></w:tcBorders></w:tcPr>"
    } else {
        ""
    };
    format!(
        "<w:tc>{border}<w:p><w:pPr><w:spacing w:before=\"40\" w:after=\"40\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:tc>",
        xml(text)
    )
}

/// WordprocessingML body of the table document (`word/document.xml`).
fn docx_document(fit: &FitHistoryEntry) -> String {
    let mut body = docx_paragraph(&format!("Table. {}", caption(fit)), false, 20);
    body.push_str(
        "<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/><w:tblBorders><w:top w:val=\"single\" w:sz=\"12\" w:space=\"0\" w:color=\"000000\"/><w:bottom w:val=\"single\" w:sz=\"12\" w:space=\"0\" w:color=\"000000\"/></w:tblBorders><w:tblCellMar><w:left w:w=\"80\" w:type=\"dxa\"/><w:right w:w=\"80\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tr>",
    );
    body.push_str(&docx_cell("Path", true));
    for (_, header, _, _) in COLUMNS {
        body.push_str(&docx_cell(header, true));
    }
    body.push_str("</w:tr>");
    for (path, cells) in rows(fit) {
        body.push_str("<w:tr>");
        body.push_str(&docx_cell(&path, false));
        for cell in &cells {
            body.push_str(&docx_cell(&text_cell(cell), false));
        }
        body.push_str("</w:tr>");
    }
    body.push_str("</w:tbl>");
    if has_note(fit) {
        body.push_str(&docx_paragraph(UNAVAILABLE_NOTE, false, 18));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"1440\" w:right=\"1080\" w:bottom=\"1440\" w:left=\"1080\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>"
    )
}

/// A minimal Office Open XML (.docx) document holding the table, written with
/// the `zip` crate already used by the app; Word, LibreOffice and Pages open it.
pub(crate) fn docx(fit: &FitHistoryEntry) -> Result<Vec<u8>, String> {
    const CONTENT_TYPES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>";
    const RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>";
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, content) in [
        ("[Content_Types].xml", CONTENT_TYPES.to_string()),
        ("_rels/.rels", RELS.to_string()),
        ("word/document.xml", docx_document(fit)),
    ] {
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
}

/// Write `<base>.tex`, `<base>.csv` and `<base>.docx` for one fit and return
/// the written paths.
pub(crate) fn write_all(
    fit: &FitHistoryEntry,
    base: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    let files = [
        ("tex", latex(fit).into_bytes()),
        ("csv", csv(fit).into_bytes()),
        ("docx", docx(fit)?),
    ];
    files
        .into_iter()
        .map(|(extension, bytes)| {
            let path = base.with_extension(extension);
            std::fs::write(&path, bytes)
                .map(|()| path.clone())
                .map_err(|e| format!("{}: {e}", path.display()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fitted(value: f64, u: f64) -> (String, String) {
        match round_pair(value, Some(u)) {
            Rounded::Fitted { value, uncertainty } => (value, uncertainty),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn uncertainty_keeps_one_figure_or_two_when_leading_digit_is_one() {
        assert_eq!(fitted(0.00812, 0.00292), ("0.008".into(), "0.003".into()));
        assert_eq!(fitted(0.9312, 0.0435), ("0.93".into(), "0.04".into()));
        assert_eq!(fitted(2.5523, 0.0149), ("2.552".into(), "0.015".into()));
        // Carry to a power of ten keeps two figures: 0.0096 -> 0.010.
        assert_eq!(fitted(1.23456, 0.0096), ("1.235".into(), "0.010".into()));
        // Uncertainties above 10 round the value to tens.
        assert_eq!(fitted(1234.5, 23.), ("1230".into(), "20".into()));
        assert_eq!(fitted(-3.46, 0.7), ("-3.5".into(), "0.7".into()));
        assert_eq!(fitted(-0.0004, 0.2), ("0.0".into(), "0.2".into()));
        assert_eq!(fitted(5.0, 1.0), ("5.0".into(), "1.0".into()));
    }

    #[test]
    fn missing_zero_and_nan_uncertainties_are_flagged_not_rounded() {
        for u in [None, Some(0.), Some(f64::NAN), Some(f64::INFINITY)] {
            assert_eq!(
                round_pair(2.55234, u),
                Rounded::NoUncertainty {
                    value: "2.552".into()
                }
            );
        }
        assert_eq!(round_pair(f64::NAN, Some(0.1)), Rounded::Unavailable);
        assert_eq!(plain(12.), "12");
        assert_eq!(plain(0.9), "0.9");
        assert_eq!(plain(0.0), "0");
        assert_eq!(plain(12345.6), "12346");
    }

    fn estimate(value: f64, stderr: Option<f64>, fixed: bool) -> Option<Estimate> {
        Some(Estimate {
            value,
            stderr,
            fixed,
        })
    }

    fn fit() -> FitHistoryEntry {
        let details: PathFitDetails = serde_json::from_value(serde_json::json!({
            "file": "feff0001.dat", "label": "feff0001.dat", "scatterers": "Cu–Cu",
            "reff": 2.5527, "nleg": 2, "degeneracy": 12.0,
            "distance": null, "deltar": null, "sigma2": null, "s02": null, "e0": null
        }))
        .unwrap();
        let details = PathFitDetails {
            effective_degen: estimate(12., Some(0.), true),
            s02: estimate(0.9312, Some(0.0435), false),
            e0: estimate(4.123, Some(0.648), false),
            distance: estimate(2.5434, Some(0.0149), false),
            sigma2: estimate(0.00866, Some(0.00292), false),
            ..details
        };
        let mut fit: FitHistoryEntry = serde_json::from_value(serde_json::json!({
            "id": 3, "group": "cu_150k.xmu", "r_factor": 0.01234, "reduced_chi_square": 152.3,
            "chi_square": 1500.0, "n_idp": 13.4, "n_vary": 4, "paths": [], "vars": [],
            "ranges": crate::fitting::FitRanges::default(), "values": []
        }))
        .unwrap();
        fit.path_details = vec![details];
        fit
    }

    #[test]
    fn latex_uses_booktabs_pm_and_marks_fixed_values() {
        let tex = latex(&fit());
        assert!(tex.contains("\\toprule") && tex.contains("\\midrule"));
        assert!(tex.contains("\\bottomrule"));
        assert!(tex.contains(
            "Cu--Cu 2.553 \\AA{} & $12$ (fixed) & $0.93 \\pm 0.04$ & $4.1 \\pm 0.6$ & $2.543 \\pm 0.015$ & $0.009 \\pm 0.003$ \\\\"
        ));
        assert!(!tex.contains("\\pm 0$"));
        assert!(!tex.contains("dagger"));
    }

    #[test]
    fn csv_keeps_unrounded_and_rounded_columns() {
        let text = csv(&fit());
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 6);
        assert!(lines[1].starts_with("3,cu_150k.xmu,Cu–Cu 2.553 Å,N,,"));
        assert!(lines[1].ends_with(",12,,fixed"));
        assert!(lines[2].contains(&format!(
            ",S02,,{:.16e},{:.16e},0.93,0.04,fitted",
            0.9312, 0.0435
        )));
        assert!(lines[5].contains(",sigma2,Å²,"));
        assert!(lines[5].ends_with(",0.009,0.003,fitted"));
    }

    #[test]
    fn docx_is_a_word_package_with_the_table() {
        let bytes = docx(&fit()).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut document = String::new();
        std::io::Read::read_to_string(
            &mut archive.by_name("word/document.xml").unwrap(),
            &mut document,
        )
        .unwrap();
        assert!(document.contains("<w:tbl>"));
        assert!(document.contains("0.93 ± 0.04"));
        assert!(document.contains("12 (fixed)"));
        assert!(archive.by_name("[Content_Types].xml").is_ok());
        let mut missing = fit();
        missing.path_details[0].s02 = estimate(0.9, None, false);
        assert!(latex(&missing).contains("$0.9^{\\dagger}$"));
        assert!(latex(&missing).contains("Uncertainty unavailable"));
    }
}
