//! Symmetry operations (`x,-y+1/2,z`), the bundled space-group table
//! (spglib's 530 Hall settings) and asymmetric-unit expansion.

use std::io::Read;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::model::{Site, Species};
use super::StructureError;

/// One symmetry operation `x' = R·x + t` in fractional coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymOp {
    /// Integer rotation matrix (rows act on `[x, y, z]`).
    pub rot: [[i8; 3]; 3],
    /// Translation, each component in [0, 1).
    pub trans: [f64; 3],
}

impl SymOp {
    pub fn identity() -> Self {
        Self {
            rot: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
            trans: [0.0; 3],
        }
    }

    /// Parse a CIF/spglib operation string such as `-y, x-y, z+1/2`,
    /// `1/2+x,1/2-y,-z` or `0.5-x, y, z`. Letters may be upper case;
    /// separators are commas (or whitespace when there are exactly three
    /// terms without commas).
    pub fn parse(text: &str) -> Result<Self, StructureError> {
        let err = |reason: &str| StructureError::InvalidSymOp {
            op: text.to_string(),
            reason: reason.to_string(),
        };
        let cleaned = text.trim().trim_matches(|c| c == '\'' || c == '"');
        let parts: Vec<&str> = if cleaned.contains(',') {
            cleaned.split(',').collect()
        } else {
            cleaned.split_whitespace().collect()
        };
        if parts.len() != 3 {
            return Err(err("expected three comma-separated components"));
        }
        let mut rot = [[0i8; 3]; 3];
        let mut trans = [0.0f64; 3];
        for (row, part) in parts.iter().enumerate() {
            let chars: Vec<char> = part.chars().filter(|c| !c.is_whitespace()).collect();
            let mut i = 0;
            let mut sign = 1i32;
            while i < chars.len() {
                let c = chars[i];
                match c {
                    '+' => {
                        sign = 1;
                        i += 1;
                    }
                    '-' => {
                        sign = -1;
                        i += 1;
                    }
                    'x' | 'X' | 'y' | 'Y' | 'z' | 'Z' => {
                        let col = match c.to_ascii_lowercase() {
                            'x' => 0,
                            'y' => 1,
                            _ => 2,
                        };
                        rot[row][col] = rot[row][col].saturating_add(sign as i8);
                        sign = 1;
                        i += 1;
                    }
                    '0'..='9' | '.' => {
                        let start = i;
                        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                            i += 1;
                        }
                        let num: String = chars[start..i].iter().collect();
                        let mut value: f64 = num.parse().map_err(|_| err("bad number"))?;
                        if i < chars.len() && chars[i] == '/' {
                            i += 1;
                            let dstart = i;
                            while i < chars.len() && chars[i].is_ascii_digit() {
                                i += 1;
                            }
                            let den: String = chars[dstart..i].iter().collect();
                            let den: f64 = den.parse().map_err(|_| err("bad fraction"))?;
                            if den == 0.0 {
                                return Err(err("zero denominator"));
                            }
                            value /= den;
                        }
                        // A number directly followed by a variable is a
                        // coefficient (`2x` — not in space groups, but be safe).
                        if i < chars.len() && matches!(chars[i], 'x' | 'X' | 'y' | 'Y' | 'z' | 'Z')
                        {
                            let col = match chars[i].to_ascii_lowercase() {
                                'x' => 0,
                                'y' => 1,
                                _ => 2,
                            };
                            rot[row][col] =
                                rot[row][col].saturating_add((sign as f64 * value).round() as i8);
                            i += 1;
                        } else {
                            trans[row] += sign as f64 * value;
                        }
                        sign = 1;
                    }
                    '*' => {
                        i += 1;
                    }
                    _ => return Err(err(&format!("unexpected character '{c}'"))),
                }
            }
            if rot[row].iter().all(|v| *v == 0) {
                return Err(err("component has no x/y/z term"));
            }
            trans[row] = wrap_unit(trans[row]);
        }
        Ok(Self { rot, trans })
    }

    /// Apply to fractional coordinates (result not wrapped).
    pub fn apply(&self, f: [f64; 3]) -> [f64; 3] {
        let mut out = [0.0; 3];
        for (row, o) in out.iter_mut().enumerate() {
            *o = self.rot[row][0] as f64 * f[0]
                + self.rot[row][1] as f64 * f[1]
                + self.rot[row][2] as f64 * f[2]
                + self.trans[row];
        }
        out
    }

    /// Canonical string form (`-y,x-y,z+1/2`).
    pub fn to_xyz(&self) -> String {
        let names = ['x', 'y', 'z'];
        let mut parts = Vec::new();
        for row in 0..3 {
            let mut s = String::new();
            for (col, name) in names.iter().enumerate() {
                match self.rot[row][col] {
                    0 => {}
                    1 => {
                        if !s.is_empty() {
                            s.push('+');
                        }
                        s.push(*name);
                    }
                    -1 => {
                        s.push('-');
                        s.push(*name);
                    }
                    n => {
                        s.push_str(&format!("{n:+}"));
                        s.push(*name);
                    }
                }
            }
            let t = self.trans[row];
            if t.abs() > 1e-9 {
                s.push_str(&fraction_string(t));
            }
            parts.push(s);
        }
        parts.join(",")
    }
}

/// Wrap into [0, 1) with a tolerance so 0.9999999 → 0.
pub fn wrap_unit(v: f64) -> f64 {
    let mut w = v - v.floor();
    if w >= 1.0 - 1e-6 {
        w = 0.0;
    }
    if w.abs() < 1e-9 {
        w = 0.0;
    }
    w
}

fn fraction_string(t: f64) -> String {
    for den in [2u32, 3, 4, 6, 8, 12] {
        let num = (t * den as f64).round();
        if ((num / den as f64) - t).abs() < 1e-6 {
            return format!("{:+}/{den}", num as i64);
        }
    }
    format!("{t:+.4}")
}

/// One Hall setting of the bundled space-group table.
#[derive(Debug, Clone, Deserialize)]
pub struct SpaceGroupEntry {
    pub hall_number: u16,
    /// International Tables number.
    pub number: u16,
    pub hall: String,
    pub hm_short: String,
    pub hm_full: String,
    pub hm: String,
    pub choice: String,
    /// Operations as `x,y,z` strings.
    pub ops: Vec<String>,
}

impl SpaceGroupEntry {
    pub fn operations(&self) -> Vec<SymOp> {
        self.ops
            .iter()
            .filter_map(|s| SymOp::parse(s).ok())
            .collect()
    }
}

static TABLE: OnceLock<Vec<SpaceGroupEntry>> = OnceLock::new();

/// All 530 Hall settings (generated with spglib by
/// `scripts/generate_spacegroup_table.py`).
pub fn space_group_table() -> &'static [SpaceGroupEntry] {
    TABLE.get_or_init(|| {
        let bytes: &[u8] = include_bytes!("spacegroups.json.gz");
        let mut text = String::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_string(&mut text)
            .expect("bundled space-group table decompresses");
        serde_json::from_str(&text).expect("bundled space-group table parses")
    })
}

/// Normalise an H-M or Hall symbol for comparison: lower case, no spaces,
/// no underscores, setting suffix after `:` dropped, `\-3` → `-3`.
pub fn normalize_symbol(symbol: &str) -> String {
    let s = symbol.trim();
    let s = s.split(':').next().unwrap_or(s);
    s.chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != '\'' && *c != '"')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Resolve a space group from what a CIF offers: Hall symbol first, then
/// the H-M symbol (short or full spelling), then the IT number (its
/// standard setting = first table entry).
pub fn find_space_group(
    number: Option<u16>,
    hm_symbol: Option<&str>,
    hall: Option<&str>,
) -> Option<&'static SpaceGroupEntry> {
    let table = space_group_table();
    if let Some(hall) = hall.map(normalize_symbol).filter(|s| !s.is_empty()) {
        if let Some(e) = table.iter().find(|e| normalize_symbol(&e.hall) == hall) {
            return Some(e);
        }
    }
    if let Some(hm) = hm_symbol.map(normalize_symbol).filter(|s| !s.is_empty()) {
        let candidates = table.iter().filter(|e| {
            normalize_symbol(&e.hm_short) == hm
                || normalize_symbol(&e.hm_full) == hm
                || normalize_symbol(&e.hm) == hm
        });
        // Prefer the entry that also matches the IT number when given.
        let mut best: Option<&SpaceGroupEntry> = None;
        for e in candidates {
            if number.is_some_and(|n| n == e.number) {
                return Some(e);
            }
            if best.is_none() {
                best = Some(e);
            }
        }
        if best.is_some() {
            return best;
        }
        // Rhombohedral groups written with the hexagonal-axes suffix
        // (`R -3 m :H`) normalise to the short symbol already; also try the
        // symbol with the `1` fillers removed (`P 1 21/c 1` → `P21/c`).
        let compact: String = hm.chars().filter(|c| *c != '1').collect();
        if let Some(e) = table.iter().find(|e| {
            let short = normalize_symbol(&e.hm_short);
            short == compact || short.chars().filter(|c| *c != '1').collect::<String>() == compact
        }) {
            return Some(e);
        }
    }
    number.and_then(|n| table.iter().find(|e| e.number == n))
}

/// Expand asymmetric-unit sites with `ops` into the full cell. Positions
/// within `tol` (fractional, periodic) are merged; species landing on the
/// same position from different asymmetric sites are combined (partial
/// occupancy).
pub fn expand_sites(asym: &[Site], ops: &[SymOp], tol: f64) -> Vec<Site> {
    let ops: Vec<SymOp> = if ops.is_empty() {
        vec![SymOp::identity()]
    } else {
        ops.to_vec()
    };
    let mut out: Vec<Site> = Vec::new();
    for (ai, site) in asym.iter().enumerate() {
        let mut generated = 0u32;
        let first_index = out.len();
        for op in &ops {
            let raw = op.apply(site.frac);
            let pos = [wrap_unit(raw[0]), wrap_unit(raw[1]), wrap_unit(raw[2])];
            if let Some(existing) = out.iter_mut().find(|s| frac_close(s.frac, pos, tol)) {
                if existing.asym_index != Some(ai) {
                    // Another asymmetric site shares this position: merge species.
                    for sp in &site.species {
                        merge_species(&mut existing.species, sp);
                    }
                    if !existing.label.contains(&site.label) {
                        existing.label = format!("{}/{}", existing.label, site.label);
                    }
                }
                continue;
            }
            let mut new_site = site.clone();
            new_site.frac = pos;
            new_site.asym_index = Some(ai);
            out.push(new_site);
            generated += 1;
        }
        for s in out.iter_mut().skip(first_index) {
            if s.asym_index == Some(ai) {
                s.multiplicity = Some(generated);
            }
        }
    }
    out
}

fn merge_species(target: &mut Vec<Species>, sp: &Species) {
    if let Some(t) = target
        .iter_mut()
        .find(|t| t.symbol.eq_ignore_ascii_case(&sp.symbol))
    {
        t.occupancy = (t.occupancy + sp.occupancy).min(1.0);
    } else {
        target.push(sp.clone());
    }
}

/// Periodic closeness in fractional coordinates.
pub fn frac_close(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
    (0..3).all(|i| {
        let d = (a[i] - b[i]).abs();
        let d = d.min((1.0 - d).abs());
        d < tol
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_operation_strings() {
        let op = SymOp::parse("-y, x-y, z+1/2").unwrap();
        assert_eq!(op.rot, [[0, -1, 0], [1, -1, 0], [0, 0, 1]]);
        assert!((op.trans[2] - 0.5).abs() < 1e-12);
        let op2 = SymOp::parse("1/2-x, 0.5+y, -z").unwrap();
        assert_eq!(op2.rot, [[-1, 0, 0], [0, 1, 0], [0, 0, -1]]);
        assert!((op2.trans[0] - 0.5).abs() < 1e-12 && (op2.trans[1] - 0.5).abs() < 1e-12);
        assert_eq!(SymOp::parse("x,y,z").unwrap().to_xyz(), "x,y,z");
        assert_eq!(op.to_xyz(), "-y,x-y,z+1/2");
        assert!(SymOp::parse("x, y").is_err());
        assert!(SymOp::parse("x, y, q").is_err());
    }

    #[test]
    fn table_resolves_symbols() {
        let e = find_space_group(None, Some("P 63/m m c"), None).unwrap();
        assert_eq!(e.number, 194);
        assert_eq!(e.operations().len(), 24);
        let e = find_space_group(Some(14), Some("P 1 21/c 1"), None).unwrap();
        assert_eq!(e.number, 14);
        assert_eq!(normalize_symbol(&e.hm_full), "p121/c1");
        let e = find_space_group(None, Some("R -3 m :H"), None).unwrap();
        assert_eq!(e.number, 166);
        let e = find_space_group(None, None, Some("-P 6c 2c")).unwrap();
        assert_eq!(e.number, 194);
        let e = find_space_group(Some(225), None, None).unwrap();
        assert_eq!(e.operations().len(), 192);
        assert!(find_space_group(None, Some("nonsense"), None).is_none());
    }

    #[test]
    fn expands_hcp() {
        let ops = find_space_group(Some(194), None, None)
            .unwrap()
            .operations();
        let site = Site::new("Ru1", "Ru", [1.0 / 3.0, 2.0 / 3.0, 0.25]);
        let cell = expand_sites(&[site], &ops, 1e-3);
        assert_eq!(cell.len(), 2);
        assert_eq!(cell[0].multiplicity, Some(2));
    }
}
