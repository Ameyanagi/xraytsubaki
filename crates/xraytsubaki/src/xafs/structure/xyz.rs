//! XYZ files (`N` / comment / `El x y z` lines) as ready-made, non-periodic
//! clusters: the absorber is chosen by index or element and the atoms are
//! re-centred on it, skipping the crystal → cluster step.

use std::path::Path;

use super::cluster::{Cluster, ClusterAtom, Potential};
use super::element::Element;
use super::StructureError;

/// One atom of an XYZ file.
#[derive(Debug, Clone, PartialEq)]
pub struct XyzAtom {
    pub symbol: String,
    pub z: u8,
    pub cart: [f64; 3],
}

/// A parsed XYZ file.
#[derive(Debug, Clone, PartialEq)]
pub struct Xyz {
    pub comment: String,
    pub atoms: Vec<XyzAtom>,
}

/// Which atom of an XYZ file is the absorber.
#[derive(Debug, Clone, PartialEq)]
pub enum XyzAbsorber {
    /// 0-based atom index.
    Index(usize),
    /// First atom of this element (`Fe`).
    Element(String),
    /// The atom closest to the centroid of the given element.
    CentralOf(String),
}

/// Parse XYZ text. Tolerates a missing count line and extra columns.
pub fn parse_xyz(text: &str) -> Result<Xyz, StructureError> {
    let mut lines = text.lines().peekable();
    let mut comment = String::new();
    let mut expected: Option<usize> = None;
    if let Some(first) = lines.peek() {
        if let Ok(n) = first.trim().parse::<usize>() {
            expected = Some(n);
            lines.next();
            comment = lines.next().unwrap_or("").trim().to_string();
        }
    }
    let mut atoms = Vec::new();
    for (lineno, line) in lines.enumerate() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.is_empty() || words[0].starts_with('#') {
            continue;
        }
        if words.len() < 4 {
            return Err(StructureError::CifParse {
                line: lineno + 1,
                message: format!("XYZ line needs `El x y z`, got {line:?}"),
            });
        }
        let element = Element::from_label(words[0])
            .or_else(|| words[0].parse::<u8>().ok().and_then(Element::from_z));
        let Some(element) = element else {
            return Err(StructureError::UnknownElement {
                label: words[0].to_string(),
            });
        };
        let coord = |w: &str| {
            w.parse::<f64>().map_err(|_| StructureError::CifParse {
                line: lineno + 1,
                message: format!("bad coordinate {w:?}"),
            })
        };
        atoms.push(XyzAtom {
            symbol: element.symbol.to_string(),
            z: element.z,
            cart: [coord(words[1])?, coord(words[2])?, coord(words[3])?],
        });
        if expected.is_some_and(|n| atoms.len() == n) {
            break;
        }
    }
    if atoms.is_empty() {
        return Err(StructureError::CifNoStructure {
            reason: "XYZ file has no atoms".into(),
        });
    }
    Ok(Xyz { comment, atoms })
}

/// Read and parse an XYZ file.
pub fn read_xyz<P: AsRef<Path>>(path: P) -> Result<Xyz, StructureError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|source| StructureError::Io {
        path: path.display().to_string(),
        source,
    })?;
    parse_xyz(&text)
}

impl Xyz {
    /// Elements present, in order of first appearance.
    pub fn elements(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for a in &self.atoms {
            if !out.contains(&a.symbol) {
                out.push(a.symbol.clone());
            }
        }
        out
    }

    fn absorber_index(&self, sel: &XyzAbsorber) -> Result<usize, StructureError> {
        match sel {
            XyzAbsorber::Index(i) if *i < self.atoms.len() => Ok(*i),
            XyzAbsorber::Index(i) => Err(StructureError::AbsorberNotFound {
                reason: format!("atom index {i} out of range ({} atoms)", self.atoms.len()),
            }),
            XyzAbsorber::Element(sym) => self
                .atoms
                .iter()
                .position(|a| a.symbol.eq_ignore_ascii_case(sym))
                .ok_or_else(|| StructureError::AbsorberNotFound {
                    reason: format!("no {sym} atom in the XYZ file"),
                }),
            XyzAbsorber::CentralOf(sym) => {
                let members: Vec<usize> = self
                    .atoms
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| a.symbol.eq_ignore_ascii_case(sym))
                    .map(|(i, _)| i)
                    .collect();
                if members.is_empty() {
                    return Err(StructureError::AbsorberNotFound {
                        reason: format!("no {sym} atom in the XYZ file"),
                    });
                }
                let n = self.atoms.len() as f64;
                let mut c = [0.0; 3];
                for a in &self.atoms {
                    for (k, v) in c.iter_mut().zip(a.cart) {
                        *k += v / n;
                    }
                }
                Ok(members
                    .into_iter()
                    .min_by(|&i, &j| {
                        dist(self.atoms[i].cart, c).total_cmp(&dist(self.atoms[j].cart, c))
                    })
                    .unwrap())
            }
        }
    }

    /// A cluster centred on the absorber, truncated to `radius` (Å) when
    /// given. FEFF potentials: 0 = absorber, then one per element by first
    /// appearance in distance order.
    pub fn to_cluster(
        &self,
        sel: &XyzAbsorber,
        radius: Option<f64>,
    ) -> Result<Cluster, StructureError> {
        let ai = self.absorber_index(sel)?;
        let origin = self.atoms[ai].cart;
        let mut atoms: Vec<ClusterAtom> = self
            .atoms
            .iter()
            .enumerate()
            .filter_map(|(i, a)| {
                let cart = [
                    a.cart[0] - origin[0],
                    a.cart[1] - origin[1],
                    a.cart[2] - origin[2],
                ];
                let distance = dist(cart, [0.0; 3]);
                if radius.is_some_and(|r| distance > r) && i != ai {
                    return None;
                }
                Some(ClusterAtom {
                    cart,
                    distance,
                    symbol: a.symbol.clone(),
                    z: a.z,
                    site_index: i,
                    image: [0, 0, 0],
                    ipot: u16::MAX,
                    label: format!("{}{}", a.symbol, i + 1),
                })
            })
            .collect();
        atoms.sort_by(|a, b| {
            a.distance
                .total_cmp(&b.distance)
                .then(a.site_index.cmp(&b.site_index))
        });
        if let Some(pos) = atoms.iter().position(|a| a.site_index == ai) {
            let abs = atoms.remove(pos);
            atoms.insert(0, abs);
        }
        let absorber = &self.atoms[ai];
        let mut potentials = vec![Potential {
            ipot: 0,
            symbol: absorber.symbol.clone(),
            z: absorber.z,
            count: 1,
        }];
        atoms[0].ipot = 0;
        for atom in atoms.iter_mut().skip(1) {
            let ipot = match potentials.iter_mut().skip(1).find(|p| p.z == atom.z) {
                Some(p) => {
                    p.count += 1;
                    p.ipot
                }
                None => {
                    let ipot = potentials.len() as u16;
                    potentials.push(Potential {
                        ipot,
                        symbol: atom.symbol.clone(),
                        z: atom.z,
                        count: 1,
                    });
                    ipot
                }
            };
            atom.ipot = ipot;
        }
        let max_r = atoms.iter().map(|a| a.distance).fold(0.0, f64::max);
        let formula = {
            let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
            for a in &self.atoms {
                *counts.entry(a.symbol.clone()).or_default() += 1;
            }
            counts
                .into_iter()
                .map(|(s, n)| if n == 1 { s } else { format!("{s}{n}") })
                .collect::<Vec<_>>()
                .join("")
        };
        Ok(Cluster {
            absorber_site: ai,
            atoms,
            potentials,
            radius: radius.unwrap_or(max_r),
            warnings: Vec::new(),
            structure_title: if self.comment.is_empty() {
                "XYZ cluster".into()
            } else {
                self.comment.clone()
            },
            formula,
            space_group: None,
        })
    }
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str =
        "5\nRu5 cluster\nRu 0 0 0\nRu 2.7 0 0\nRu -2.7 0 0\nO 0 2.0 0\nRu 0 0 9.0\n";

    #[test]
    fn parses_and_centres_on_the_absorber() {
        let xyz = parse_xyz(SAMPLE).unwrap();
        assert_eq!(xyz.atoms.len(), 5);
        assert_eq!(xyz.elements(), vec!["Ru".to_string(), "O".to_string()]);
        let c = xyz
            .to_cluster(&XyzAbsorber::Element("Ru".into()), Some(5.0))
            .unwrap();
        assert_eq!(c.atoms.len(), 4, "far Ru is outside 5 Å");
        assert_eq!(c.absorber().symbol, "Ru");
        assert_eq!(c.atoms[1].symbol, "O");
        assert!((c.atoms[1].distance - 2.0).abs() < 1e-12);
        assert_eq!(c.potentials.len(), 3);
        assert_eq!(c.potentials[1].symbol, "O");
        assert_eq!(c.potentials[2].count, 2);
        let c2 = xyz.to_cluster(&XyzAbsorber::Index(1), None).unwrap();
        assert!((c2.atoms[0].cart[0]).abs() < 1e-12);
        assert_eq!(c2.atoms.len(), 5);
    }

    #[test]
    fn tolerates_missing_header_and_rejects_junk() {
        let xyz = parse_xyz("Fe 0 0 0\nS 1 1 1\n").unwrap();
        assert_eq!(xyz.atoms.len(), 2);
        assert!(parse_xyz("Xx 0 0 0").is_err());
        assert!(parse_xyz("2\nc\nFe 0 0\n").is_err());
    }
}
