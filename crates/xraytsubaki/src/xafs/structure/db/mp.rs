//! Materials Project v2 REST client (`https://api.materialsproject.org`).
//! Only what a structure browser needs: search summaries by formula /
//! chemical system / elements and fetch one structure by `mp-id`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{StructureHit, StructureQuery, StructureSource};
use crate::xafs::structure::lattice::Lattice;
use crate::xafs::structure::model::{Site, SpaceGroupInfo, Species, Structure};
use crate::xafs::structure::symmetry::{wrap_unit, SymOp};
use crate::xafs::structure::StructureError;

pub const DEFAULT_BASE_URL: &str = "https://api.materialsproject.org";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialsProjectConfig {
    pub api_key: String,
    pub base_url: String,
    /// Request timeout in seconds.
    pub timeout_sec: u64,
}

impl MaterialsProjectConfig {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.trim().to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout_sec: 30,
        }
    }
}

/// Client. Cheap to clone.
#[derive(Debug, Clone)]
pub struct MaterialsProject {
    config: MaterialsProjectConfig,
}

const SUMMARY_FIELDS: &str =
    "material_id,formula_pretty,symmetry,structure,energy_above_hull,is_stable,nsites,volume,density";

impl MaterialsProject {
    pub fn new(config: MaterialsProjectConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &MaterialsProjectConfig {
        &self.config
    }

    fn get_json(&self, path_and_query: &str) -> Result<Value, StructureError> {
        if self.config.api_key.is_empty() {
            return Err(StructureError::Network {
                reason: "Materials Project API key is not set".into(),
            });
        }
        let url = format!(
            "{}/{}",
            self.config.base_url.trim_end_matches('/'),
            path_and_query
        );
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(
                self.config.timeout_sec,
            )))
            .build()
            .into();
        let mut resp = agent
            .get(&url)
            .header("X-API-KEY", &self.config.api_key)
            .header("accept", "application/json")
            .call()
            .map_err(|e| StructureError::Network {
                reason: format!("{url}: {e}"),
            })?;
        resp.body_mut()
            .read_json::<Value>()
            .map_err(|e| StructureError::Network {
                reason: format!("{url}: invalid JSON: {e}"),
            })
    }

    /// Build the summary query string for a [`StructureQuery`].
    pub fn summary_query(query: &StructureQuery) -> String {
        let mut params: Vec<String> = Vec::new();
        let text = query
            .text
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty());
        if let Some(text) = text {
            if text.starts_with("mp-") || text.starts_with("mvc-") {
                params.push(format!("material_ids={text}"));
            } else if text.contains('-')
                && text.chars().all(|c| c.is_ascii_alphabetic() || c == '-')
            {
                params.push(format!("chemsys={text}"));
            } else {
                params.push(format!(
                    "formula={}",
                    text.split_whitespace().collect::<String>()
                ));
            }
        }
        if !query.elements.is_empty() {
            if query.exact_elements {
                params.push(format!("chemsys={}", query.elements.join("-")));
            } else {
                params.push(format!("elements={}", query.elements.join(",")));
            }
        }
        if !query.exclude.is_empty() {
            params.push(format!("exclude_elements={}", query.exclude.join(",")));
        }
        let limit = if query.limit == 0 { 50 } else { query.limit };
        params.push(format!("_limit={limit}"));
        params.push(format!("_fields={SUMMARY_FIELDS}"));
        params.push("_sort_fields=energy_above_hull".into());
        format!("materials/summary/?{}", params.join("&"))
    }

    /// Fetch one material by id.
    pub fn material(&self, material_id: &str) -> Result<Structure, StructureError> {
        let v = self.get_json(&format!(
            "materials/summary/?material_ids={material_id}&_fields={SUMMARY_FIELDS}"
        ))?;
        let docs = summary_docs(&v)?;
        let doc = docs.first().ok_or_else(|| StructureError::Network {
            reason: format!("{material_id}: not found"),
        })?;
        structure_from_doc(doc)
    }
}

/// The `data` array of a summary response.
pub fn summary_docs(v: &Value) -> Result<Vec<Value>, StructureError> {
    match v.get("data") {
        Some(Value::Array(a)) => Ok(a.clone()),
        _ => Err(StructureError::Network {
            reason: v
                .get("detail")
                .map(|d| d.to_string())
                .unwrap_or_else(|| "response has no data array".into()),
        }),
    }
}

/// A search hit from one summary document.
pub fn hit_from_doc(doc: &Value) -> Option<StructureHit> {
    let id = doc.get("material_id")?.as_str()?.to_string();
    let formula = doc
        .get("formula_pretty")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let symmetry = doc.get("symmetry");
    let space_group = symmetry
        .and_then(|s| s.get("symbol"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut extra = BTreeMap::new();
    if let Some(n) = symmetry
        .and_then(|s| s.get("number"))
        .and_then(Value::as_i64)
    {
        extra.insert("spacegroup_number".into(), n.to_string());
    }
    if let Some(cs) = symmetry
        .and_then(|s| s.get("crystal_system"))
        .and_then(Value::as_str)
    {
        extra.insert("crystal_system".into(), cs.to_string());
    }
    if let Some(e) = doc.get("energy_above_hull").and_then(Value::as_f64) {
        extra.insert("energy_above_hull_eV".into(), format!("{e:.4}"));
    }
    if let Some(stable) = doc.get("is_stable").and_then(Value::as_bool) {
        extra.insert("stable".into(), stable.to_string());
    }
    let elements = doc
        .get("structure")
        .map(elements_of_structure_json)
        .unwrap_or_else(|| super::parse_formula(&formula).keys().cloned().collect());
    Some(StructureHit {
        id,
        source: "materials-project".into(),
        formula,
        name: None,
        space_group,
        elements,
        extra,
    })
}

fn elements_of_structure_json(structure: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(sites) = structure.get("sites").and_then(Value::as_array) {
        for site in sites {
            if let Some(species) = site.get("species").and_then(Value::as_array) {
                for sp in species {
                    if let Some(el) = sp.get("element").and_then(Value::as_str) {
                        if !out.iter().any(|e| e == el) {
                            out.push(el.to_string());
                        }
                    }
                }
            }
        }
    }
    out
}

/// Convert a summary document (with its pymatgen `structure`) to a
/// [`Structure`].
pub fn structure_from_doc(doc: &Value) -> Result<Structure, StructureError> {
    let id = doc
        .get("material_id")
        .and_then(Value::as_str)
        .unwrap_or("mp-?");
    let sjson = doc
        .get("structure")
        .ok_or_else(|| StructureError::Network {
            reason: format!("{id}: document has no structure"),
        })?;
    let mut s = structure_from_pymatgen_json(sjson)?;
    s.source = format!("mp:{id}");
    s.title = doc
        .get("formula_pretty")
        .and_then(Value::as_str)
        .map(|f| format!("{f} ({id})"))
        .unwrap_or_else(|| id.to_string());
    if let Some(sym) = doc.get("symmetry") {
        s.space_group.hm_symbol = sym
            .get("symbol")
            .and_then(Value::as_str)
            .map(str::to_string);
        s.space_group.number = sym.get("number").and_then(Value::as_u64).map(|n| n as u16);
    }
    Ok(s)
}

/// Convert a pymatgen `Structure.as_dict()` JSON value (lattice matrix +
/// sites with species/occu and abc) — the cell is already fully expanded.
pub fn structure_from_pymatgen_json(v: &Value) -> Result<Structure, StructureError> {
    let bad = |reason: &str| StructureError::Network {
        reason: format!("pymatgen structure JSON: {reason}"),
    };
    let matrix = v
        .get("lattice")
        .and_then(|l| l.get("matrix"))
        .and_then(Value::as_array)
        .ok_or_else(|| bad("missing lattice.matrix"))?;
    let mut m = [[0.0f64; 3]; 3];
    for (i, row) in matrix.iter().take(3).enumerate() {
        let row = row.as_array().ok_or_else(|| bad("lattice row"))?;
        for (j, x) in row.iter().take(3).enumerate() {
            m[i][j] = x.as_f64().ok_or_else(|| bad("lattice value"))?;
        }
    }
    let lattice = Lattice::from_matrix(m)?;
    let sites_json = v
        .get("sites")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("missing sites"))?;
    let mut sites = Vec::new();
    for (i, sj) in sites_json.iter().enumerate() {
        let frac: [f64; 3] = match sj.get("abc").and_then(Value::as_array) {
            Some(abc) if abc.len() == 3 => {
                let mut f = [0.0; 3];
                for (k, x) in abc.iter().enumerate() {
                    f[k] = wrap_unit(x.as_f64().ok_or_else(|| bad("abc value"))?);
                }
                f
            }
            _ => {
                let xyz = sj
                    .get("xyz")
                    .and_then(Value::as_array)
                    .ok_or_else(|| bad("site abc/xyz"))?;
                let mut c = [0.0; 3];
                for (k, x) in xyz.iter().take(3).enumerate() {
                    c[k] = x.as_f64().ok_or_else(|| bad("xyz value"))?;
                }
                let f = lattice.to_frac(c);
                [wrap_unit(f[0]), wrap_unit(f[1]), wrap_unit(f[2])]
            }
        };
        let mut species = Vec::new();
        if let Some(list) = sj.get("species").and_then(Value::as_array) {
            for sp in list {
                let el = sp
                    .get("element")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad("species element"))?;
                let occ = sp.get("occu").and_then(Value::as_f64).unwrap_or(1.0);
                let mut s = Species::new(el, occ);
                s.oxidation = sp.get("oxidation_state").and_then(Value::as_f64);
                species.push(s);
            }
        }
        if species.is_empty() {
            return Err(bad("site without species"));
        }
        let label = sj
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}{}", species[0].symbol, i + 1));
        sites.push(Site {
            label,
            species,
            frac,
            multiplicity: None,
            wyckoff: None,
            asym_index: Some(i),
        });
    }
    let mut s = Structure::new("materials-project", lattice, sites);
    s.space_group = SpaceGroupInfo {
        number: None,
        hm_symbol: None,
        hall: None,
        operations: vec![SymOp::identity()],
    };
    Ok(s)
}

impl StructureSource for MaterialsProject {
    fn name(&self) -> &str {
        "materials-project"
    }

    fn search(&self, query: &StructureQuery) -> Result<Vec<StructureHit>, StructureError> {
        let v = self.get_json(&Self::summary_query(query))?;
        let mut hits: Vec<StructureHit> =
            summary_docs(&v)?.iter().filter_map(hit_from_doc).collect();
        // Element-set constraints the API cannot express exactly.
        let mut q = query.clone();
        q.text = None;
        hits.retain(|h| q.matches(h));
        Ok(hits)
    }

    fn fetch(&self, hit: &StructureHit) -> Result<Structure, StructureError> {
        self.material(&hit.id)
    }
}
