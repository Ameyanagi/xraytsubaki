//! Structure model: species, sites, space-group info and the `Structure`
//! container (expanded conventional cell plus the asymmetric unit it came
//! from).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::element::Element;
use super::lattice::Lattice;
use super::symmetry::SymOp;

/// One chemical species occupying (part of) a site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Species {
    /// Element symbol (`Fe`), always a valid [`Element`].
    pub symbol: String,
    /// Site occupancy in 0…1.
    pub occupancy: f64,
    /// Oxidation state when the source gave one (`Fe2+` → 2).
    pub oxidation: Option<f64>,
}

impl Species {
    pub fn new(symbol: &str, occupancy: f64) -> Self {
        Self {
            symbol: symbol.to_string(),
            occupancy,
            oxidation: None,
        }
    }

    /// The element behind this species' symbol, or `None` when the symbol
    /// (or label such as `Ru1`, `Fe2+`) does not name a known element.
    pub fn element(&self) -> Option<&'static Element> {
        Element::from_symbol(&self.symbol).or_else(|| Element::from_label(&self.symbol))
    }
}

/// A crystallographic site of the (expanded) cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Site {
    /// Label from the source (`Ru1`, `O2`), or a generated one.
    pub label: String,
    /// Species on this site, occupancies summing to ≤ 1.
    pub species: Vec<Species>,
    /// Fractional coordinates, wrapped into [0, 1).
    pub frac: [f64; 3],
    /// Site multiplicity when known (number of equivalent positions).
    pub multiplicity: Option<u32>,
    pub wyckoff: Option<String>,
    /// Index of the asymmetric-unit site this one was generated from.
    pub asym_index: Option<usize>,
}

impl Site {
    pub fn new(label: &str, symbol: &str, frac: [f64; 3]) -> Self {
        Self {
            label: label.to_string(),
            species: vec![Species::new(symbol, 1.0)],
            frac,
            multiplicity: None,
            wyckoff: None,
            asym_index: None,
        }
    }

    /// Majority species (highest occupancy).
    pub fn majority(&self) -> Option<&Species> {
        self.species
            .iter()
            .max_by(|a, b| a.occupancy.total_cmp(&b.occupancy))
    }

    /// Majority element.
    pub fn element(&self) -> Option<&'static Element> {
        self.majority().and_then(Species::element)
    }

    /// `Fe` for a pure site, `(Fe0.7Ni0.3)` for a mixed one.
    pub fn species_string(&self) -> String {
        if self.species.len() == 1 && (self.species[0].occupancy - 1.0).abs() < 1e-6 {
            return self.species[0].symbol.clone();
        }
        let parts: Vec<String> = self
            .species
            .iter()
            .map(|s| format!("{}{}", s.symbol, trim_float(s.occupancy)))
            .collect();
        format!("({})", parts.join(""))
    }

    pub fn contains_element(&self, symbol: &str) -> bool {
        self.species
            .iter()
            .any(|s| s.symbol.eq_ignore_ascii_case(symbol))
    }

    pub fn total_occupancy(&self) -> f64 {
        self.species.iter().map(|s| s.occupancy).sum()
    }
}

/// Space-group metadata carried with a structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SpaceGroupInfo {
    /// International Tables number (1…230).
    pub number: Option<u16>,
    /// Hermann–Mauguin symbol as given by the source.
    pub hm_symbol: Option<String>,
    pub hall: Option<String>,
    /// The operations used to expand the asymmetric unit.
    pub operations: Vec<SymOp>,
}

/// A crystal structure: lattice + fully expanded sites, with provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Structure {
    /// Display name (mineral name, formula, or file stem).
    pub title: String,
    /// Where it came from (`cif:/path`, `amcsd:1234`, `mp:mp-33`).
    pub source: String,
    pub lattice: Lattice,
    /// All sites of the conventional cell.
    pub sites: Vec<Site>,
    /// The asymmetric unit as read from the source (may equal `sites`).
    pub asymmetric_sites: Vec<Site>,
    pub space_group: SpaceGroupInfo,
    pub formula_sum: Option<String>,
    pub mineral: Option<String>,
    /// Free-form notes and parser warnings.
    pub warnings: Vec<String>,
}

impl Structure {
    pub fn new(title: &str, lattice: Lattice, sites: Vec<Site>) -> Self {
        Self {
            title: title.to_string(),
            source: String::new(),
            lattice,
            asymmetric_sites: sites.clone(),
            sites,
            space_group: SpaceGroupInfo::default(),
            formula_sum: None,
            mineral: None,
            warnings: Vec::new(),
        }
    }

    pub fn num_sites(&self) -> usize {
        self.sites.len()
    }

    /// Cartesian coordinates of site `i` (Å).
    pub fn cart(&self, i: usize) -> [f64; 3] {
        self.lattice.to_cart(self.sites[i].frac)
    }

    /// Element symbols present, in order of first appearance.
    pub fn elements(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for site in &self.sites {
            for sp in &site.species {
                if !out.iter().any(|s| s == &sp.symbol) {
                    out.push(sp.symbol.clone());
                }
            }
        }
        out
    }

    /// Occupancy-weighted composition of the cell.
    pub fn composition(&self) -> BTreeMap<String, f64> {
        let mut comp = BTreeMap::new();
        for site in &self.sites {
            for sp in &site.species {
                *comp.entry(sp.symbol.clone()).or_insert(0.0) += sp.occupancy;
            }
        }
        comp
    }

    /// Reduced formula with integer counts (`RuO2`, `FeS2`), elements in
    /// order of first appearance (metals usually come first in CIFs).
    pub fn formula(&self) -> String {
        let comp = self.composition();
        if comp.is_empty() {
            return String::new();
        }
        let counts: Vec<f64> = comp.values().copied().collect();
        // Scale so the smallest count is 1, then find the smallest integer
        // multiplier (≤ 12) that makes every count integral.
        let min = counts
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min)
            .max(1e-9);
        let scaled: Vec<f64> = counts.iter().map(|c| c / min).collect();
        let mult = (1..=12)
            .find(|m| {
                scaled
                    .iter()
                    .all(|c| ((c * *m as f64).round() - c * *m as f64).abs() < 0.02)
            })
            .unwrap_or(1) as f64;
        let mut out = String::new();
        for symbol in self.elements() {
            let n = (comp[&symbol] / min * mult).round() as i64;
            out.push_str(&symbol);
            if n != 1 {
                out.push_str(&n.to_string());
            }
        }
        out
    }

    /// Indices of the sites whose majority species is `symbol`.
    pub fn sites_of(&self, symbol: &str) -> Vec<usize> {
        self.sites
            .iter()
            .enumerate()
            .filter(|(_, s)| s.contains_element(symbol))
            .map(|(i, _)| i)
            .collect()
    }

    /// Sites grouped by the asymmetric-unit site they descend from.
    pub fn equivalent_sites(&self, i: usize) -> Vec<usize> {
        match self.sites.get(i).and_then(|s| s.asym_index) {
            Some(asym) => self
                .sites
                .iter()
                .enumerate()
                .filter(|(_, s)| s.asym_index == Some(asym))
                .map(|(j, _)| j)
                .collect(),
            None => vec![i],
        }
    }
}

pub(super) fn trim_float(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".into()
    } else {
        s.to_string()
    }
}
