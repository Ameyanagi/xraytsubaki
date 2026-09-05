//! XAS Data Interchange 1.x reader.
//!
//! Based on the XraySpectroscopy XDI specification and metadata dictionary:
//! <https://github.com/XraySpectroscopy/XAS-Data-Interchange/tree/master/specification>.
//! This is an importer, not a complete metadata-dictionary validator. It keeps
//! unknown metadata and comments, reports missing descriptive fields as warnings,
//! and rejects ambiguous axes or damaged numeric tables instead of guessing.

use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XdiColumn {
    pub label: String,
    pub units: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XdiHeader {
    pub version: String,
    /// Ordered application/version tokens from the first line.
    pub applications: Vec<String>,
    /// Case-folded `family.field` keys; duplicate fields use their last value.
    pub metadata: BTreeMap<String, String>,
    /// User comments, including empty lines and interior whitespace.
    pub comments: Vec<String>,
    pub columns: Vec<XdiColumn>,
    pub warnings: Vec<String>,
}

impl XdiHeader {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.metadata
            .get(&key.to_ascii_lowercase())
            .map(String::as_str)
    }

    pub fn column_index(&self, label: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.label.eq_ignore_ascii_case(label))
    }

    /// Convert a raw abscissa value to eV without changing the stored table.
    /// Angle axes use first-order Bragg diffraction and Mono.d_spacing in Å.
    pub fn energy_ev(&self, column: usize, value: f64) -> Result<f64, XdiError> {
        let c = self
            .columns
            .get(column)
            .ok_or_else(|| XdiError::new(0, "energy column out of range"))?;
        let units = c.units.as_deref().unwrap_or("").to_ascii_lowercase();
        let energy = match c.label.to_ascii_lowercase().as_str() {
            "energy" => match units.as_str() {
                "ev" => value,
                "kev" => value * 1000.0,
                _ => {
                    return Err(XdiError::new(
                        0,
                        format!("unsupported energy units '{units}'; expected eV or keV"),
                    ));
                }
            },
            "angle" => {
                let radians = match units.as_str() {
                    "deg" | "degree" | "degrees" => value.to_radians(),
                    "rad" | "radian" | "radians" => value,
                    _ => {
                        return Err(XdiError::new(
                            0,
                            format!(
                                "unsupported angle units '{units}'; expected degrees or radians"
                            ),
                        ));
                    }
                };
                let d = self
                    .get("mono.d_spacing")
                    .and_then(|s| s.parse::<f64>().ok())
                    .filter(|d| d.is_finite() && *d > 0.)
                    .ok_or_else(|| {
                        XdiError::new(
                            0,
                            "angle axis requires a positive Mono.d_spacing in angstroms",
                        )
                    })?;
                if !(0.0..=std::f64::consts::FRAC_PI_2).contains(&radians) || radians == 0.0 {
                    return Err(XdiError::new(
                        0,
                        "monochromator angle must be greater than 0 and at most 90 degrees",
                    ));
                }
                // CODATA hc in eV Å, first-order Bragg law.
                12398.419843320026 / (2.0 * d * radians.sin())
            }
            label => {
                return Err(XdiError::new(
                    0,
                    format!("'{label}' is not an energy or angle axis; cannot import it as mu(E)"),
                ));
            }
        };
        if !energy.is_finite() || energy <= 0.0 {
            return Err(XdiError::new(0, "energy must be positive and finite"));
        }
        Ok(energy)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XdiFile {
    pub header: XdiHeader,
    /// Row-major table, with original values and units intact.
    pub data: Vec<Vec<f64>>,
}

/// The measured signal to turn into mu(E). Auto prefers a precomputed sample
/// signal, then sample intensities, then a reference signal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum XdiSignal {
    #[default]
    Auto,
    Transmission,
    Fluorescence,
    Reference,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("XDI{location}: {message}", location = if *line == 0 { String::new() } else { format!(" line {line}") })]
pub struct XdiError {
    pub line: usize,
    pub message: String,
}

impl XdiError {
    fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

/// Identify XDI by its version signature, independent of the filename suffix.
pub fn is_xdi(text: &str) -> bool {
    text.trim_start_matches('\u{feff}')
        .split(['\n', '\r'])
        .next()
        .and_then(|s| s.trim_start().strip_prefix('#'))
        .is_some_and(|s| s.trim_start().starts_with("XDI/"))
}

fn separator(s: &str, character: char) -> bool {
    s.len() >= 3 && s.chars().all(|c| c == character)
}

fn field_name(s: &str) -> bool {
    let Some((family, tag)) = s.split_once('.') else {
        return false;
    };
    let word = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    family.starts_with(|c: char| c.is_ascii_alphabetic()) && word(family) && word(tag)
}

impl XdiFile {
    pub fn read(path: impl AsRef<Path>) -> Result<Self, XdiError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|e| XdiError::new(0, format!("{}: {e}", path.display())))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self, XdiError> {
        // All three line endings named by the specification, plus a UTF-8 BOM.
        let normalized = text
            .trim_start_matches('\u{feff}')
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        let mut lines = normalized.lines().enumerate();
        let (_, first) = lines
            .next()
            .ok_or_else(|| XdiError::new(1, "missing XDI version line"))?;
        let mut tokens = first
            .trim_start()
            .strip_prefix('#')
            .unwrap_or("")
            .split_whitespace();
        let version = tokens
            .next()
            .and_then(|s| s.strip_prefix("XDI/"))
            .ok_or_else(|| XdiError::new(1, "expected '# XDI/1.0' version signature"))?;
        let parts: Vec<_> = version.split('.').collect();
        if !(2..=3).contains(&parts.len()) || parts.iter().any(|p| p.parse::<u32>().is_err()) {
            return Err(XdiError::new(1, "invalid XDI version"));
        }
        if parts[0].parse::<u32>().unwrap() != 1 {
            return Err(XdiError::new(
                1,
                format!("unsupported XDI version {version}; supported major version is 1"),
            ));
        }
        let mut header = XdiHeader {
            version: version.into(),
            applications: tokens.map(str::to_string).collect(),
            metadata: BTreeMap::new(),
            comments: Vec::new(),
            columns: Vec::new(),
            warnings: Vec::new(),
        };
        let mut in_comments = false;
        let mut ended_header = false;
        let mut labels: Option<Vec<String>> = None;
        let mut data: Vec<Vec<f64>> = Vec::new();
        for (index, line) in lines {
            let n = index + 1;
            let trimmed = line.trim();
            if !ended_header {
                let comment = line.trim_start().strip_prefix('#').ok_or_else(|| {
                    XdiError::new(n, "expected a header comment or '# ---' before the data")
                })?;
                let content = comment.trim();
                if separator(content, '-') {
                    ended_header = true;
                    continue;
                }
                if !in_comments && separator(content, '/') {
                    in_comments = true;
                    continue;
                }
                if in_comments {
                    header.comments.push(
                        comment
                            .strip_prefix(' ')
                            .unwrap_or(comment)
                            .trim_end()
                            .to_string(),
                    );
                } else if let Some((key, value)) =
                    content.split_once(':').filter(|(key, _)| field_name(key))
                {
                    header
                        .metadata
                        .insert(key.to_ascii_lowercase(), value.trim().to_string());
                } else {
                    header
                        .warnings
                        .push(format!("Line {n}: ignored malformed metadata field"));
                }
                continue;
            }
            if trimmed.is_empty() {
                continue;
            }
            if let Some(comment) = trimmed.strip_prefix('#') {
                if !data.is_empty() || labels.is_some() {
                    return Err(XdiError::new(
                        n,
                        "unexpected comment in numeric data section",
                    ));
                }
                labels = Some(comment.split_whitespace().map(str::to_string).collect());
                continue;
            }
            let row = trimmed
                .split_whitespace()
                .enumerate()
                .map(|(i, token)| {
                    token
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| {
                            XdiError::new(
                                n,
                                format!("column {}: invalid finite number '{token}'", i + 1),
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(first) = data.first() {
                if row.len() != first.len() {
                    return Err(XdiError::new(
                        n,
                        format!("expected {} columns, found {}", first.len(), row.len()),
                    ));
                }
            }
            data.push(row);
        }
        if !ended_header {
            return Err(XdiError::new(0, "missing '# ---' header-end separator"));
        }
        let width = data
            .first()
            .ok_or_else(|| XdiError::new(0, "no numeric data rows"))?
            .len();
        if let Some(labels) = &labels {
            if labels.len() != width {
                return Err(XdiError::new(
                    0,
                    "column-label count does not match the data table",
                ));
            }
        }
        for key in header.metadata.keys().filter(|k| k.starts_with("column.")) {
            let index = key[7..].parse::<usize>().ok();
            if !index.is_some_and(|i| (1..=width).contains(&i)) {
                return Err(XdiError::new(
                    0,
                    format!("{key} does not identify a data column"),
                ));
            }
        }
        for i in 0..width {
            let field = header.get(&format!("column.{}", i + 1));
            let (label, units) = if let Some(field) = field {
                let mut parts = field.split_whitespace();
                let label = parts
                    .next()
                    .ok_or_else(|| XdiError::new(0, format!("Column.{} has no label", i + 1)))?;
                let units = parts.collect::<Vec<_>>().join(" ");
                if labels
                    .as_ref()
                    .is_some_and(|ls| !ls[i].eq_ignore_ascii_case(label))
                {
                    return Err(XdiError::new(
                        0,
                        format!(
                            "Column.{} conflicts with the optional column-label line",
                            i + 1
                        ),
                    ));
                }
                (label.to_string(), (!units.is_empty()).then_some(units))
            } else if i == 0 {
                return Err(XdiError::new(
                    0,
                    "missing required Column.1 abscissa declaration",
                ));
            } else {
                (
                    labels
                        .as_ref()
                        .map(|ls| ls[i].clone())
                        .unwrap_or_else(|| format!("column{}", i + 1)),
                    None,
                )
            };
            header.columns.push(XdiColumn { label, units });
        }
        if header.columns[0].units.is_none() {
            return Err(XdiError::new(0, "Column.1 must declare abscissa units"));
        }
        for key in ["element.symbol", "element.edge"] {
            if header.get(key).is_none_or(str::is_empty) {
                header
                    .warnings
                    .push(format!("Missing required {key} metadata"));
            }
        }
        Ok(Self { header, data })
    }

    pub fn energy_ev(&self) -> Result<Vec<f64>, XdiError> {
        self.data
            .iter()
            .map(|row| self.header.energy_ev(0, row[0]))
            .collect()
    }

    /// Build a spectrum from standard XDI signal names. The original metadata,
    /// units and table remain available on this XdiFile. For arbitrary detector
    /// channels, use `data` with your own channel assignments instead.
    pub fn to_spectrum(&self, signal: XdiSignal) -> Result<super::XASSpectrum, XdiError> {
        let find = |names: &[&str]| names.iter().find_map(|name| self.header.column_index(name));
        let direct = |signal| match signal {
            XdiSignal::Transmission => find(&["mutrans", "normtrans"]),
            XdiSignal::Fluorescence => find(&["mufluor", "normfluor"]),
            XdiSignal::Reference => find(&["murefer", "normrefer"]),
            XdiSignal::Auto => None,
        };
        let ratio = |signal| match signal {
            XdiSignal::Transmission => find(&["i0"]).zip(find(&["itrans"])),
            XdiSignal::Fluorescence => find(&["ifluor"]).zip(find(&["i0"])),
            XdiSignal::Reference => find(&["itrans"]).zip(find(&["irefer"])),
            XdiSignal::Auto => None,
        };
        let signal = if signal == XdiSignal::Auto {
            [XdiSignal::Transmission, XdiSignal::Fluorescence]
                .into_iter()
                .find(|s| direct(*s).is_some())
                .or_else(|| {
                    [
                        XdiSignal::Transmission,
                        XdiSignal::Fluorescence,
                        XdiSignal::Reference,
                    ]
                    .into_iter()
                    .find(|s| ratio(*s).is_some() || direct(*s).is_some())
                })
                .ok_or_else(|| XdiError::new(0, "no standard XDI mu or intensity columns found"))?
        } else {
            signal
        };
        let mu = if let Some(column) = direct(signal) {
            self.data.iter().map(|row| row[column]).collect::<Vec<_>>()
        } else {
            let (numerator, denominator) = ratio(signal).ok_or_else(|| {
                XdiError::new(0, format!("missing columns for {signal:?} signal"))
            })?;
            self.data
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    let value = row[numerator] / row[denominator];
                    if row[denominator] <= 0.
                        || (signal != XdiSignal::Fluorescence && row[numerator] <= 0.)
                    {
                        return Err(XdiError::new(
                            0,
                            format!(
                                "data row {}: non-positive intensity in {signal:?} ratio",
                                i + 1
                            ),
                        ));
                    }
                    let mu = if signal == XdiSignal::Fluorescence {
                        value
                    } else {
                        value.ln()
                    };
                    if mu.is_finite() {
                        Ok(mu)
                    } else {
                        Err(XdiError::new(
                            0,
                            format!("data row {}: non-finite mu", i + 1),
                        ))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut spectrum = super::XASSpectrum::new();
        spectrum.set_spectrum(self.energy_ev()?, mu);
        spectrum.name = self.header.get("sample.name").map(str::to_string);
        Ok(spectrum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "# XDI/1.0 Beamline/2.0\n# Column.1: energy keV\n# Column.2: mutrans\n# Element.symbol: Cu\n# Element.edge: K\n";

    #[test]
    fn metadata_comments_units_and_optional_labels() {
        let text = format!(
            "{HEADER}# SAMPLE.NAME: old\n# sample.name: 铜箔\n# Beamline.extra: a: b\n# ///\n#  retain  spaces\n#\n# Column.1: angle radians\n# ---\n8.9 0.4\n9.0 0.5\n"
        );
        let xdi = XdiFile::parse(&text).unwrap();
        assert_eq!(xdi.header.get("Sample.name"), Some("铜箔"));
        assert_eq!(xdi.header.get("beamline.extra"), Some("a: b"));
        assert_eq!(xdi.header.applications, ["Beamline/2.0"]);
        assert_eq!(
            xdi.header.comments,
            [" retain  spaces", "", "Column.1: angle radians"]
        );
        assert_eq!(xdi.energy_ev().unwrap(), [8900., 9000.]);
        assert_eq!(xdi.data[0][0], 8.9);
        assert!(xdi.header.warnings.is_empty());
        let labeled = format!("{HEADER}# ---\n# energy mutrans\n8.9 .4\n9 .5\n");
        assert_eq!(XdiFile::parse(&labeled).unwrap().data, xdi.data);
        for text in [
            labeled.replace('\n', "\r"),
            labeled.replace('\n', "\r\n"),
            format!("\u{feff}{labeled}"),
        ] {
            assert!(is_xdi(&text));
            assert_eq!(XdiFile::parse(&text).unwrap().data, xdi.data);
        }
    }

    #[test]
    fn angle_conversion_and_unsupported_axes() {
        let mut xdi = XdiFile::parse(&format!(
            "{}# Mono.d_spacing: 3.1356\n# ---\n30 .4\n29 .5\n",
            HEADER.replace("energy keV", "angle deg")
        ))
        .unwrap();
        assert!((xdi.energy_ev().unwrap()[0] - 12398.419843320026 / 3.1356).abs() < 1e-8);
        xdi.header.columns[0].units = Some("radians".into());
        xdi.data[0][0] = std::f64::consts::FRAC_PI_6;
        assert!(
            (xdi.header.energy_ev(0, xdi.data[0][0]).unwrap() - 3954.0821033677844).abs() < 1e-6
        );
        xdi.header.metadata.remove("mono.d_spacing");
        assert!(xdi.energy_ev().unwrap_err().message.contains("d_spacing"));
        xdi.header.columns[0].label = "k".into();
        assert!(xdi
            .energy_ev()
            .unwrap_err()
            .message
            .contains("not an energy"));
        xdi.header.columns[0].label = "energy".into();
        assert!(xdi.energy_ev().unwrap_err().message.contains("units"));
    }

    #[test]
    fn damaged_data_is_never_silently_skipped_or_truncated() {
        for (body, message) in [
            ("8.9 .4\n9 .5 12", "expected 2 columns"),
            ("8.9 .4\n9", "expected 2 columns"),
            ("8.9 .4\n9 bad", "invalid finite number"),
            ("8.9 .4\n9 NaN", "invalid finite number"),
            ("8.9 .4\n# comment", "unexpected comment"),
            ("# energy itrans\n8.9 .4", "conflicts"),
        ] {
            let err = XdiFile::parse(&format!("{HEADER}# ---\n{body}")).unwrap_err();
            assert!(err.message.contains(message), "{err}");
        }
        assert!(XdiFile::parse(&HEADER.replace("XDI/1.0", "XDI/2.0"))
            .unwrap_err()
            .message
            .contains("unsupported"));
        assert!(XdiFile::parse(HEADER)
            .unwrap_err()
            .message
            .contains("header-end"));
        assert!(XdiFile::parse(&format!("{HEADER}# Column.3: i0\n# ---\n8.9 .4")).is_err());
    }

    #[test]
    fn measured_nickel_foil() {
        let file = XdiFile::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/testfiles/xraylarch_d867/xafsdata/ni_metal_rt.xdi"),
        )
        .unwrap();
        assert_eq!(
            file.header.get("sample.prep"),
            Some("standard foil (Joe Wong boxed set)")
        );
        assert_eq!(file.header.column_index("mutrans"), Some(1));
        assert!(file.data.len() > 300);
        assert_eq!(file.energy_ev().unwrap()[0], file.data[0][0]);
        assert!(file.header.warnings.is_empty());
        let spectrum = file.to_spectrum(XdiSignal::Auto).unwrap();
        assert_eq!(spectrum.name.as_deref(), Some("Ni metal foil"));
        assert_eq!(spectrum.raw_mu.unwrap()[0], -1.1873423);
        assert!(file.to_spectrum(XdiSignal::Reference).is_err());
    }

    #[test]
    fn detector_signals_and_angle_sort_together() {
        let file = XdiFile::parse("# XDI/1.0\n# Column.1: angle degrees\n# Column.2: i0\n# Column.3: itrans\n# Column.4: ifluor\n# Column.5: irefer\n# Mono.d_spacing: 3.1356\n# ---\n29 20 10 4 2\n30 10 5 1 2.5\n").unwrap();
        for (signal, expected) in [
            (XdiSignal::Auto, 2.0_f64.ln()),
            (XdiSignal::Transmission, 2.0_f64.ln()),
            (XdiSignal::Fluorescence, 0.1),
            (XdiSignal::Reference, 2.0_f64.ln()),
        ] {
            let sp = file.to_spectrum(signal).unwrap();
            assert!((sp.raw_mu.unwrap()[0] - expected).abs() < 1e-12);
            let energy = sp.raw_energy.unwrap();
            assert!(energy[0] < energy[1]);
        }
        let mut invalid = file.clone();
        invalid.data[0][1] = 0.0;
        assert!(invalid
            .to_spectrum(XdiSignal::Auto)
            .unwrap_err()
            .message
            .contains("non-positive"));
    }
}
