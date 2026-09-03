//! Tolerant CIF 1.1 reader (the subset written by AMCSD, COD, the Materials
//! Project and pymatgen) and a P1 writer.

use std::collections::HashMap;
use std::path::Path;

use super::element::Element;
use super::lattice::Lattice;
use super::model::{Site, SpaceGroupInfo, Species, Structure};
use super::symmetry::{expand_sites, find_space_group, wrap_unit, SymOp};
use super::StructureError;

/// A `loop_` block: tag names and rows of values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CifLoop {
    pub tags: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl CifLoop {
    pub fn column(&self, tag: &str) -> Option<usize> {
        self.tags.iter().position(|t| t.eq_ignore_ascii_case(tag))
    }

    pub fn has(&self, tag: &str) -> bool {
        self.column(tag).is_some()
    }
}

/// One `data_` block.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CifBlock {
    pub name: String,
    /// Non-loop items, keys lower-cased.
    pub items: HashMap<String, String>,
    pub loops: Vec<CifLoop>,
}

impl CifBlock {
    pub fn get(&self, tag: &str) -> Option<&str> {
        self.items
            .get(&tag.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// First present of several alternative tags.
    pub fn get_any(&self, tags: &[&str]) -> Option<&str> {
        tags.iter().find_map(|t| self.get(t))
    }

    pub fn number(&self, tag: &str) -> Option<f64> {
        self.get(tag).and_then(parse_number)
    }

    pub fn loop_with(&self, tag: &str) -> Option<&CifLoop> {
        self.loops.iter().find(|l| l.has(tag))
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Data(String),
    Loop,
    Tag(String),
    Value(String),
}

fn tokenize(text: &str) -> Result<Vec<(usize, Token)>, StructureError> {
    let mut tokens = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut li = 0;
    while li < lines.len() {
        let line = lines[li];
        let lineno = li + 1;
        // Semicolon text field: starts a line with ';', ends at a line
        // starting with ';'.
        if let Some(first) = line.strip_prefix(';') {
            let mut buf = String::new();
            buf.push_str(first);
            li += 1;
            let mut closed = false;
            while li < lines.len() {
                if lines[li].starts_with(';') {
                    closed = true;
                    break;
                }
                buf.push('\n');
                buf.push_str(lines[li]);
                li += 1;
            }
            if !closed {
                return Err(StructureError::CifParse {
                    line: lineno,
                    message: "unterminated ';' text field".into(),
                });
            }
            tokens.push((lineno, Token::Value(buf.trim().to_string())));
            li += 1;
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c.is_whitespace() {
                i += 1;
                continue;
            }
            if c == '#' {
                break;
            }
            if c == '\'' || c == '"' {
                // Quote closes only when followed by whitespace or EOL.
                let quote = c;
                let mut j = i + 1;
                let mut value = String::new();
                loop {
                    if j >= chars.len() {
                        return Err(StructureError::CifParse {
                            line: lineno,
                            message: "unterminated quoted string".into(),
                        });
                    }
                    if chars[j] == quote && (j + 1 >= chars.len() || chars[j + 1].is_whitespace()) {
                        break;
                    }
                    value.push(chars[j]);
                    j += 1;
                }
                tokens.push((lineno, Token::Value(value)));
                i = j + 1;
                continue;
            }
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let lower = word.to_ascii_lowercase();
            if let Some(name) = lower.strip_prefix("data_") {
                tokens.push((lineno, Token::Data(word[5..].to_string())));
                let _ = name;
            } else if lower == "loop_" {
                tokens.push((lineno, Token::Loop));
            } else if lower.starts_with('_') {
                tokens.push((lineno, Token::Tag(lower)));
            } else if lower.starts_with("save_") || lower == "global_" || lower == "stop_" {
                // Dictionary/save frames: ignore.
            } else {
                tokens.push((lineno, Token::Value(word)));
            }
        }
        li += 1;
    }
    Ok(tokens)
}

/// Parse every data block of a CIF text.
pub fn parse_cif(text: &str) -> Result<Vec<CifBlock>, StructureError> {
    let tokens = tokenize(text)?;
    let mut blocks: Vec<CifBlock> = Vec::new();
    let mut i = 0;
    let ensure_block = |blocks: &mut Vec<CifBlock>| {
        if blocks.is_empty() {
            blocks.push(CifBlock {
                name: String::new(),
                ..Default::default()
            });
        }
    };
    while i < tokens.len() {
        let (line, tok) = &tokens[i];
        match tok {
            Token::Data(name) => {
                blocks.push(CifBlock {
                    name: name.clone(),
                    ..Default::default()
                });
                i += 1;
            }
            Token::Tag(tag) => {
                ensure_block(&mut blocks);
                let value = match tokens.get(i + 1) {
                    Some((_, Token::Value(v))) => v.clone(),
                    _ => {
                        return Err(StructureError::CifParse {
                            line: *line,
                            message: format!("tag {tag} has no value"),
                        })
                    }
                };
                blocks.last_mut().unwrap().items.insert(tag.clone(), value);
                i += 2;
            }
            Token::Loop => {
                ensure_block(&mut blocks);
                i += 1;
                let mut tags = Vec::new();
                while let Some((_, Token::Tag(t))) = tokens.get(i) {
                    tags.push(t.clone());
                    i += 1;
                }
                if tags.is_empty() {
                    return Err(StructureError::CifParse {
                        line: *line,
                        message: "loop_ without tags".into(),
                    });
                }
                let mut values = Vec::new();
                while let Some((_, Token::Value(v))) = tokens.get(i) {
                    values.push(v.clone());
                    i += 1;
                }
                let n = tags.len();
                let mut rows = Vec::new();
                for chunk in values.chunks(n) {
                    if chunk.len() == n {
                        rows.push(chunk.to_vec());
                    }
                }
                blocks
                    .last_mut()
                    .unwrap()
                    .loops
                    .push(CifLoop { tags, rows });
            }
            Token::Value(v) => {
                // Stray value (e.g. a loop row count mismatch); skip.
                let _ = v;
                i += 1;
            }
        }
    }
    Ok(blocks)
}

/// Parse a CIF numeric value: `1.234(5)` → 1.234, `?`/`.` → None.
pub fn parse_number(value: &str) -> Option<f64> {
    let v = value.trim();
    if v.is_empty() || v == "?" || v == "." {
        return None;
    }
    let v = v.split('(').next().unwrap_or(v);
    v.trim().parse::<f64>().ok()
}

/// Build a [`Structure`] from CIF text (first block with cell data).
pub fn structure_from_cif(text: &str) -> Result<Structure, StructureError> {
    let blocks = parse_cif(text)?;
    let block = blocks
        .iter()
        .find(|b| b.get("_cell_length_a").is_some())
        .ok_or_else(|| StructureError::CifNoStructure {
            reason: "no _cell_length_a in any data block".into(),
        })?;
    structure_from_block(block)
}

/// Build a [`Structure`] from one parsed block.
pub fn structure_from_block(block: &CifBlock) -> Result<Structure, StructureError> {
    let cell = |tag: &str| {
        block
            .number(tag)
            .ok_or_else(|| StructureError::CifNoStructure {
                reason: format!("missing or invalid {tag}"),
            })
    };
    let lattice = Lattice::from_parameters(
        cell("_cell_length_a")?,
        cell("_cell_length_b")?,
        cell("_cell_length_c")?,
        block.number("_cell_angle_alpha").unwrap_or(90.0),
        block.number("_cell_angle_beta").unwrap_or(90.0),
        block.number("_cell_angle_gamma").unwrap_or(90.0),
    )?;

    let atoms =
        block
            .loop_with("_atom_site_fract_x")
            .ok_or_else(|| StructureError::CifNoStructure {
                reason: "no _atom_site_fract_x loop".into(),
            })?;
    let col = |tag: &str| atoms.column(tag);
    let (cx, cy, cz) = (
        col("_atom_site_fract_x").unwrap(),
        col("_atom_site_fract_y").ok_or_else(|| StructureError::CifNoStructure {
            reason: "no _atom_site_fract_y".into(),
        })?,
        col("_atom_site_fract_z").ok_or_else(|| StructureError::CifNoStructure {
            reason: "no _atom_site_fract_z".into(),
        })?,
    );
    let c_label = col("_atom_site_label");
    let c_type = col("_atom_site_type_symbol");
    let c_occ = col("_atom_site_occupancy");
    let c_mult =
        col("_atom_site_symmetry_multiplicity").or(col("_atom_site_site_symmetry_multiplicity"));
    let c_wyck = col("_atom_site_wyckoff_symbol").or(col("_atom_site_wyckoff_label"));

    let mut warnings = Vec::new();
    let mut asym: Vec<Site> = Vec::new();
    for (ri, row) in atoms.rows.iter().enumerate() {
        let label = c_label
            .and_then(|c| row.get(c))
            .cloned()
            .or_else(|| c_type.and_then(|c| row.get(c)).cloned())
            .unwrap_or_else(|| format!("site{}", ri + 1));
        let type_symbol = c_type.and_then(|c| row.get(c)).map(String::as_str);
        let Some(element) = type_symbol
            .and_then(Element::from_label)
            .or_else(|| Element::from_label(&label))
        else {
            // Placeholder rows (`?` in old database exports) or unknown
            // symbols: skip the site rather than reject the whole file.
            warnings.push(format!(
                "site {label}: unknown element symbol {:?}, skipped",
                type_symbol.unwrap_or(&label)
            ));
            continue;
        };
        let oxidation = type_symbol.and_then(parse_oxidation);
        let (x, y, z) = match (
            row.get(cx).and_then(|v| parse_number(v)),
            row.get(cy).and_then(|v| parse_number(v)),
            row.get(cz).and_then(|v| parse_number(v)),
        ) {
            (Some(x), Some(y), Some(z)) => (x, y, z),
            _ => {
                warnings.push(format!("site {label}: missing coordinates, skipped"));
                continue;
            }
        };
        let occupancy = c_occ
            .and_then(|c| row.get(c))
            .and_then(|v| parse_number(v))
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        let mut species = Species::new(element.symbol, occupancy);
        species.oxidation = oxidation;
        let frac = [wrap_unit(x), wrap_unit(y), wrap_unit(z)];
        // Sites listed twice at the same position (disorder) merge here.
        if let Some(existing) = asym
            .iter_mut()
            .find(|s| super::symmetry::frac_close(s.frac, frac, 1e-3))
        {
            if existing
                .species
                .iter()
                .all(|s| !s.symbol.eq_ignore_ascii_case(&species.symbol))
            {
                existing.species.push(species);
                existing.label = format!("{}/{}", existing.label, label);
                continue;
            }
        }
        asym.push(Site {
            label,
            species: vec![species],
            frac,
            multiplicity: c_mult
                .and_then(|c| row.get(c))
                .and_then(|v| v.parse::<u32>().ok()),
            wyckoff: c_wyck.and_then(|c| row.get(c)).cloned(),
            asym_index: None,
        });
    }
    if asym.is_empty() {
        return Err(StructureError::CifNoStructure {
            reason: "atom site loop is empty".into(),
        });
    }

    // Symmetry: explicit operations win; otherwise resolve the group.
    let number = block
        .get_any(&["_space_group_it_number", "_symmetry_int_tables_number"])
        .and_then(|v| v.trim().parse::<u16>().ok());
    let hm = block
        .get_any(&[
            "_space_group_name_h-m_alt",
            "_symmetry_space_group_name_h-m",
            "_space_group_name_h-m",
        ])
        .map(str::to_string);
    let hall = block
        .get_any(&["_space_group_name_hall", "_symmetry_space_group_name_hall"])
        .map(str::to_string);
    let mut ops: Vec<SymOp> = Vec::new();
    if let Some(lp) = block
        .loop_with("_space_group_symop_operation_xyz")
        .or_else(|| block.loop_with("_symmetry_equiv_pos_as_xyz"))
    {
        let c = lp
            .column("_space_group_symop_operation_xyz")
            .or_else(|| lp.column("_symmetry_equiv_pos_as_xyz"))
            .unwrap();
        for row in &lp.rows {
            if let Some(v) = row.get(c) {
                match SymOp::parse(v) {
                    Ok(op) => ops.push(op),
                    Err(e) => warnings.push(e.to_string()),
                }
            }
        }
    }
    let mut resolved_number = number;
    if ops.is_empty() {
        match find_space_group(number, hm.as_deref(), hall.as_deref()) {
            Some(entry) => {
                ops = entry.operations();
                resolved_number = Some(entry.number);
            }
            None => {
                let is_p1 = hm
                    .as_deref()
                    .map(super::symmetry::normalize_symbol)
                    .is_some_and(|s| s == "p1");
                if !is_p1 && (number.is_some() || hm.is_some() || hall.is_some()) {
                    return Err(StructureError::UnknownSpaceGroup {
                        reason: format!("number {number:?}, H-M {hm:?}, Hall {hall:?}"),
                    });
                }
                warnings.push("no symmetry information; treated as P1".into());
                ops = vec![SymOp::identity()];
            }
        }
    }
    let sites = expand_sites(&asym, &ops, 1e-3);

    let mineral = block
        .get_any(&["_chemical_name_mineral", "_chemical_name_common"])
        .map(str::to_string)
        .filter(|s| !s.is_empty() && s != "?" && s != ".");
    let title = mineral
        .clone()
        .or_else(|| {
            block
                .get("_chemical_formula_structural")
                .map(str::to_string)
        })
        .or_else(|| block.get("_chemical_formula_sum").map(str::to_string))
        .filter(|s| !s.is_empty() && s != "?")
        .unwrap_or_else(|| {
            if block.name.is_empty() {
                "structure".into()
            } else {
                block.name.clone()
            }
        });

    Ok(Structure {
        title,
        source: String::new(),
        lattice,
        sites,
        asymmetric_sites: asym,
        space_group: SpaceGroupInfo {
            number: resolved_number,
            hm_symbol: hm,
            hall,
            operations: ops,
        },
        formula_sum: block
            .get("_chemical_formula_sum")
            .map(|s| s.trim().to_string()),
        mineral,
        warnings,
    })
}

fn parse_oxidation(type_symbol: &str) -> Option<f64> {
    let rest: String = type_symbol
        .trim()
        .chars()
        .skip_while(|c| c.is_ascii_alphabetic())
        .collect();
    if rest.is_empty() {
        return None;
    }
    let (digits, sign): (String, f64) = rest
        .strip_suffix('+')
        .map(|d| (d.to_string(), 1.0))
        .or_else(|| rest.strip_suffix('-').map(|d| (d.to_string(), -1.0)))
        .or_else(|| rest.strip_prefix('+').map(|d| (d.to_string(), 1.0)))
        .or_else(|| rest.strip_prefix('-').map(|d| (d.to_string(), -1.0)))?;
    let magnitude: f64 = if digits.is_empty() {
        1.0
    } else {
        digits.parse().ok()?
    };
    Some(sign * magnitude)
}

/// Read a `.cif` file.
pub fn read_cif<P: AsRef<Path>>(path: P) -> Result<Structure, StructureError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|source| StructureError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let mut s = structure_from_cif(&text)?;
    s.source = format!("cif:{}", path.display());
    if s.title == "structure" {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            s.title = stem.to_string();
        }
    }
    Ok(s)
}

/// Write the expanded cell as a P1 CIF (every site explicit).
pub fn structure_to_cif(s: &Structure) -> String {
    let mut out = String::new();
    let name: String = s
        .title
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    out.push_str("# written by xraytsubaki (expanded cell, P1)\n");
    out.push_str(&format!("data_{name}\n"));
    if let Some(m) = &s.mineral {
        out.push_str(&format!("_chemical_name_mineral '{m}'\n"));
    }
    out.push_str(&format!("_chemical_formula_sum '{}'\n", s.formula()));
    let l = &s.lattice;
    out.push_str(&format!(
        "_cell_length_a {:.6}\n_cell_length_b {:.6}\n_cell_length_c {:.6}\n_cell_angle_alpha {:.4}\n_cell_angle_beta {:.4}\n_cell_angle_gamma {:.4}\n",
        l.a, l.b, l.c, l.alpha, l.beta, l.gamma
    ));
    if let Some(hm) = &s.space_group.hm_symbol {
        out.push_str(&format!("# original space group: {hm}\n"));
    }
    out.push_str("_symmetry_space_group_name_H-M 'P 1'\n_symmetry_Int_Tables_number 1\nloop_\n_symmetry_equiv_pos_as_xyz\n'x, y, z'\nloop_\n_atom_site_label\n_atom_site_type_symbol\n_atom_site_fract_x\n_atom_site_fract_y\n_atom_site_fract_z\n_atom_site_occupancy\n");
    for (i, site) in s.sites.iter().enumerate() {
        for sp in &site.species {
            let label = if site.species.len() == 1 {
                format!("{}{}", sp.symbol, i + 1)
            } else {
                format!("{}{}_{}", sp.symbol, i + 1, sp.symbol)
            };
            out.push_str(&format!(
                "{label} {} {:.6} {:.6} {:.6} {:.4}\n",
                sp.symbol, site.frac[0], site.frac[1], site.frac[2], sp.occupancy
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_handles_quotes_and_text_fields() {
        let text = "data_x\n_a 'hello world'\n_b \"it's\"\n_c\n;\nline one\nline two\n;\nloop_\n_t1\n_t2\n1 2\n3 'four five'\n";
        let blocks = parse_cif(text).unwrap();
        let b = &blocks[0];
        assert_eq!(b.name, "x");
        assert_eq!(b.get("_a"), Some("hello world"));
        assert_eq!(b.get("_b"), Some("it's"));
        assert_eq!(b.get("_c"), Some("line one\nline two"));
        assert_eq!(
            b.loops[0].rows,
            vec![
                vec!["1".to_string(), "2".into()],
                vec!["3".into(), "four five".into()]
            ]
        );
        assert_eq!(parse_number("1.234(5)"), Some(1.234));
        assert_eq!(parse_number("?"), None);
        assert_eq!(parse_oxidation("Fe2+"), Some(2.0));
        assert_eq!(parse_oxidation("O2-"), Some(-2.0));
        assert_eq!(parse_oxidation("Fe"), None);
    }

    #[test]
    fn placeholder_sites_are_skipped_with_a_warning() {
        let text = "data_x\n_cell_length_a 5.0\n_cell_length_b 5.0\n_cell_length_c 5.0\n\
_symmetry_space_group_name_H-M 'P 1'\n\
loop_\n_atom_site_label\n_atom_site_fract_x\n_atom_site_fract_y\n_atom_site_fract_z\n\
? 0 0 0\nFe1 0.5 0.5 0.5\n";
        let s = structure_from_cif(text).unwrap();
        assert_eq!(s.sites.len(), 1);
        assert_eq!(s.sites[0].species[0].symbol, "Fe");
        assert!(s.warnings.iter().any(|w| w.contains("unknown element")));
        let all_placeholder = text.replace("Fe1 0.5 0.5 0.5\n", "");
        assert!(matches!(
            structure_from_cif(&all_placeholder),
            Err(StructureError::CifNoStructure { .. })
        ));
    }
}
