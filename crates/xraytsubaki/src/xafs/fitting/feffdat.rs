use std::fs;
use std::path::Path;

use nalgebra::DVector;

use super::errors::FittingError;
use super::types::{FeffDat, FeffFlavor};

fn split_at_char_boundary_prefix(line: &str, byte_limit: usize) -> (&str, &str) {
    if byte_limit >= line.len() {
        return (line, "");
    }

    let split_index = line
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= byte_limit)
        .last()
        .unwrap_or(0);
    line.split_at(split_index)
}

pub fn parse_feff_path_file<P: AsRef<Path>>(
    path: P,
    flavor: FeffFlavor,
) -> Result<FeffDat, FittingError> {
    match flavor {
        FeffFlavor::Feff85L => parse_feff85l_dat(path.as_ref()),
        FeffFlavor::Feff10 => Err(FittingError::UnsupportedFeffFlavor {
            flavor: FeffFlavor::Feff10,
        }),
    }
}

fn parse_feff85l_dat(path: &Path) -> Result<FeffDat, FittingError> {
    let content = fs::read_to_string(path).map_err(|error| FittingError::ParseFailed {
        path: path.display().to_string(),
        reason: error.to_string(),
    })?;

    enum Mode {
        Header,
        Path,
        Arrays,
    }

    let mut mode = Mode::Header;
    let mut title: Option<String> = None;
    let mut version = String::new();
    let mut shell: Option<String> = None;
    let mut absorber: Option<String> = None;

    let mut nleg: Option<usize> = None;
    let mut degen: Option<f64> = None;
    let mut reff: Option<f64> = None;
    let mut geometry: Vec<String> = Vec::new();

    let mut cols: Vec<[f64; 7]> = Vec::new();

    for (line_number, raw_line) in content.lines().enumerate() {
        let mut line = raw_line.trim();
        if line.starts_with('#') {
            line = line.trim_start_matches('#').trim();
        }
        if line.is_empty() {
            continue;
        }

        if title.is_none() {
            // FEFF files traditionally contain title/version in line 1.
            if line.len() > 64 {
                let (title_part, version_part) = split_at_char_boundary_prefix(line, 64);
                title = Some(title_part.trim().to_string());
                version = version_part.trim().to_string();
            } else {
                title = Some(line.to_string());
            }
            continue;
        }

        if line.starts_with('k') && line.contains("real[p]@#") {
            mode = Mode::Arrays;
            continue;
        }

        let has_path_separator = line
            .char_indices()
            .nth(2)
            .map(|(index, _)| line[index..].contains("----"))
            .unwrap_or(false);
        if line.len() > 8 && has_path_separator {
            mode = Mode::Path;
            continue;
        }

        match mode {
            Mode::Header => {
                if line.starts_with("Abs") && line.contains("shell") {
                    let words: Vec<&str> = line.split_whitespace().collect();
                    if let Some(last) = words.last() {
                        shell = Some((*last).to_string());
                    }
                    if words.len() >= 2 {
                        absorber = Some(words[0].to_string());
                    }
                }
            }
            Mode::Path => {
                let words: Vec<&str> = line.split_whitespace().collect();
                if words.is_empty() {
                    continue;
                }

                if nleg.is_none() {
                    if words.len() < 5 {
                        return Err(FittingError::ParseFailed {
                            path: path.display().to_string(),
                            reason: format!(
                                "line {}: expected at least 5 numeric values in path header",
                                line_number + 1
                            ),
                        });
                    }
                    let parsed = words
                        .iter()
                        .take(5)
                        .map(|item| {
                            item.parse::<f64>().map_err(|_| FittingError::ParseFailed {
                                path: path.display().to_string(),
                                reason: format!(
                                    "line {}: invalid path header numeric value '{}'",
                                    line_number + 1,
                                    item
                                ),
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    nleg = Some(parsed[0] as usize);
                    degen = Some(parsed[1]);
                    reff = Some(parsed[2]);
                } else {
                    let label = words
                        .get(5)
                        .map(|value| (*value).to_string())
                        .unwrap_or_else(|| format!("atom_{}", geometry.len()));
                    geometry.push(label);
                }
            }
            Mode::Arrays => {
                let words: Vec<&str> = line.split_whitespace().collect();
                if words.len() != 7 {
                    continue;
                }
                let values = words
                    .iter()
                    .map(|item| {
                        item.parse::<f64>().map_err(|_| FittingError::ParseFailed {
                            path: path.display().to_string(),
                            reason: format!(
                                "line {}: expected 7 floating values for array row",
                                line_number + 1
                            ),
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let row: [f64; 7] = values.try_into().map_err(|_| FittingError::ParseFailed {
                    path: path.display().to_string(),
                    reason: format!(
                        "line {}: failed to convert parsed row to 7-column array",
                        line_number + 1
                    ),
                })?;

                if row.iter().any(|value| !value.is_finite()) {
                    return Err(FittingError::InvalidFeffData {
                        reason: format!("line {}: non-finite FEFF array value", line_number + 1),
                    });
                }

                cols.push(row);
            }
        }
    }

    if cols.len() < 3 {
        return Err(FittingError::InvalidFeffData {
            reason: "FEFF array section must contain at least 3 rows".to_string(),
        });
    }

    let nleg = nleg.ok_or_else(|| FittingError::InvalidFeffData {
        reason: "missing path header (nleg/degen/reff)".to_string(),
    })?;
    let degen = degen.ok_or_else(|| FittingError::InvalidFeffData {
        reason: "missing path degeneracy".to_string(),
    })?;
    let reff = reff.ok_or_else(|| FittingError::InvalidFeffData {
        reason: "missing path reff".to_string(),
    })?;

    if reff <= 0.0 {
        return Err(FittingError::InvalidFeffData {
            reason: format!("reff must be positive, got {reff}"),
        });
    }

    let k = DVector::from_iterator(cols.len(), cols.iter().map(|row| row[0]));
    for index in 1..k.len() {
        if k[index] < k[index - 1] {
            return Err(FittingError::InvalidFeffData {
                reason: format!("k grid must be monotonic: k[{index}] < k[{}]", index - 1),
            });
        }
    }

    let real_phc = DVector::from_iterator(cols.len(), cols.iter().map(|row| row[1]));
    let mag_feff = DVector::from_iterator(cols.len(), cols.iter().map(|row| row[2]));
    let pha_feff = DVector::from_iterator(cols.len(), cols.iter().map(|row| row[3]));
    let red_fact = DVector::from_iterator(cols.len(), cols.iter().map(|row| row[4]));
    let lam = DVector::from_iterator(cols.len(), cols.iter().map(|row| row[5]));
    let rep = DVector::from_iterator(cols.len(), cols.iter().map(|row| row[6]));

    let pha = &real_phc + &pha_feff;
    let amp = mag_feff.component_mul(&red_fact);

    Ok(FeffDat {
        filename: path.display().to_string(),
        title: title.unwrap_or_default(),
        version,
        absorber,
        shell,
        reff,
        degen,
        nleg,
        k,
        real_phc,
        mag_feff,
        pha_feff,
        red_fact,
        lam,
        rep,
        pha,
        amp,
        geometry,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::tests::TOP_DIR;

    #[test]
    fn test_parse_feff85l_file_success() {
        let path = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let parsed = parse_feff_path_file(path, FeffFlavor::Feff85L).unwrap();

        assert!(parsed.reff > 0.0);
        assert!(parsed.degen > 0.0);
        assert!(parsed.nleg >= 2);
        assert_eq!(parsed.k.len(), parsed.pha.len());
        assert!(parsed.k.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn test_parse_feff10_reports_not_supported() {
        let path = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let err = parse_feff_path_file(path, FeffFlavor::Feff10).unwrap_err();

        assert!(matches!(err, FittingError::UnsupportedFeffFlavor { .. }));
    }

    #[test]
    fn test_parse_rejects_invalid_array_rows() {
        let unique = format!(
            "xfeff-invalid-{}-{}.dat",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let tmp = std::env::temp_dir().join(unique);
        let invalid = "Title\n# comment\n  ----\n 2 12.0 2.5 1.0 0.0\nk real[p]@#\n0.0 1.0 2.0\n";
        fs::write(&tmp, invalid).unwrap();

        let err = parse_feff_path_file(&tmp, FeffFlavor::Feff85L).unwrap_err();
        let _ = fs::remove_file(tmp);
        assert!(matches!(err, FittingError::InvalidFeffData { .. }));
    }

    #[test]
    fn test_parse_multibyte_title_and_path_separator_without_panic() {
        let unique = format!(
            "xfeff-multibyte-{}-{}.dat",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let tmp = std::env::temp_dir().join(unique);
        let title = format!("{} version", "あ".repeat(30));
        let content = format!(
            "{title}\nあい---- path\n2 12.0 2.5 1.0 0.0\nk real[p]@#\n0.0 1.0 1.0 1.0 1.0 1.0 1.0\n1.0 1.0 1.0 1.0 1.0 1.0 1.0\n2.0 1.0 1.0 1.0 1.0 1.0 1.0\n"
        );
        fs::write(&tmp, content).unwrap();

        let parsed = parse_feff_path_file(&tmp, FeffFlavor::Feff85L).unwrap();
        let _ = fs::remove_file(tmp);
        assert_eq!(parsed.k.len(), 3);
        assert_eq!(parsed.nleg, 2);
    }
}
