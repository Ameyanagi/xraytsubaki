//! Crystallography Open Database (COD) client.
//!
//! COD (<https://www.crystallography.net/cod/>) is an open-access collection
//! of crystal structures of organic, inorganic, metal-organic compounds and
//! minerals; its data are released to the public domain (CC0). This client
//! uses the documented REST result endpoint
//! (<https://wiki.crystallography.net/RESTful_API/>) for searches and the
//! `https://www.crystallography.net/cod/<id>.cif` pattern to fetch one entry.
//!
//! COD asks for polite use of the public server: requests carry a
//! descriptive `User-Agent`, run with a timeout, and are throttled to a
//! minimum spacing per client.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{StructureHit, StructureQuery, StructureSource};
use crate::xafs::structure::cif::structure_from_cif;
use crate::xafs::structure::model::Structure;
use crate::xafs::structure::StructureError;

pub const DEFAULT_BASE_URL: &str = "https://www.crystallography.net/cod";
/// Citation shown next to COD results.
pub const CITATION: &str = "Gražulis et al. (2012) Nucleic Acids Res. 40, D420–D427; \
    Vaitkus et al. (2021) J. Appl. Cryst. 54, 661–672";
/// Data licence as published by COD.
pub const LICENSE: &str = "COD data are in the public domain (CC0 1.0)";

const USER_AGENT: &str = concat!(
    "xraytsubaki/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Ameyanagi/xraytsubaki)"
);
/// Minimum spacing between requests from one process (polite use).
const MIN_REQUEST_GAP: Duration = Duration::from_millis(500);
/// Hits kept when the query gives no limit.
const DEFAULT_LIMIT: usize = 200;

static LAST_REQUEST: Mutex<Option<Instant>> = Mutex::new(None);

fn throttle() {
    let mut last = LAST_REQUEST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(t) = *last {
        let since = t.elapsed();
        if since < MIN_REQUEST_GAP {
            std::thread::sleep(MIN_REQUEST_GAP - since);
        }
    }
    *last = Some(Instant::now());
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodConfig {
    pub base_url: String,
    /// Request timeout in seconds.
    pub timeout_sec: u64,
}

impl Default for CodConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout_sec: 30,
        }
    }
}

/// Client. Cheap to clone; no credentials needed.
#[derive(Debug, Clone, Default)]
pub struct Cod {
    config: CodConfig,
}

impl Cod {
    pub fn new(config: CodConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &CodConfig {
        &self.config
    }

    fn agent(&self) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(self.config.timeout_sec)))
            .user_agent(USER_AGENT)
            .build()
            .into()
    }

    fn url(&self, path_and_query: &str) -> String {
        format!(
            "{}/{}",
            self.config.base_url.trim_end_matches('/'),
            path_and_query
        )
    }

    /// Build the `result?format=json&…` query string for a [`StructureQuery`].
    ///
    /// Free text goes to `text=` (metadata, bibliography and mineral names);
    /// a text that reads as a formula (`Fe S2`, `RuO2`) is sent as
    /// `formula=` in Hill notation instead, which is what COD indexes.
    /// Required elements map to `el1..el8`, exclusions to `nel1..nel4`, and
    /// an exact element set to `strictmin`/`strictmax`.
    pub fn result_query(query: &StructureQuery) -> String {
        let mut parts: Vec<String> = vec!["format=json".to_string()];
        if let Some(text) = query
            .text
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            let formula = super::parse_formula(text);
            if !formula.is_empty() && looks_like_formula(text) {
                parts.push(format!("formula={}", encode(&hill_formula(&formula))));
            } else if let Some(id) = text
                .strip_prefix("cod")
                .or_else(|| text.strip_prefix("COD"))
            {
                let id = id.trim_matches(|c: char| c == '-' || c == ' ');
                if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
                    parts.push(format!("id={id}"));
                } else {
                    parts.push(format!("text={}", encode(text)));
                }
            } else if text.chars().all(|c| c.is_ascii_digit()) && text.len() == 7 {
                parts.push(format!("id={text}"));
            } else {
                parts.push(format!("text={}", encode(text)));
            }
        }
        for (i, el) in query.elements.iter().take(8).enumerate() {
            parts.push(format!("el{}={}", i + 1, encode(el)));
        }
        for (i, el) in query.exclude.iter().take(4).enumerate() {
            parts.push(format!("nel{}={}", i + 1, encode(el)));
        }
        if query.exact_elements && !query.elements.is_empty() {
            let n = query.elements.len();
            parts.push(format!("strictmin={n}&strictmax={n}"));
        }
        format!("result?{}", parts.join("&"))
    }

    fn get_json(&self, path_and_query: &str) -> Result<Value, StructureError> {
        let url = self.url(path_and_query);
        throttle();
        let mut resp = self
            .agent()
            .get(&url)
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

    /// Download the CIF text of one COD entry.
    pub fn cif_text(&self, id: &str) -> Result<String, StructureError> {
        let id = normalise_id(id);
        let url = self.url(&format!("{id}.cif"));
        throttle();
        let mut resp = self
            .agent()
            .get(&url)
            .call()
            .map_err(|e| StructureError::Network {
                reason: format!("{url}: {e}"),
            })?;
        resp.body_mut()
            .read_to_string()
            .map_err(|e| StructureError::Network {
                reason: format!("{url}: {e}"),
            })
    }

    /// Fetch and parse one COD entry by id (`9000594` or `cod-9000594`).
    pub fn entry(&self, id: &str) -> Result<Structure, StructureError> {
        let text = self.cif_text(id)?;
        let mut structure = structure_from_cif(&text)?;
        if structure.source.is_empty() {
            structure.source = format!("COD {}", normalise_id(id));
        }
        Ok(structure)
    }
}

fn normalise_id(id: &str) -> String {
    id.trim()
        .trim_start_matches("cod-")
        .trim_start_matches("COD-")
        .trim_start_matches("cod")
        .trim_start_matches("COD")
        .trim()
        .to_string()
}

fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// A text is a formula when every token is `Element[count]` and at least one
/// token is a valid element symbol; words like "pyrite" or "iron oxide" are
/// not.
fn looks_like_formula(text: &str) -> bool {
    let toks: Vec<&str> = text.split_whitespace().collect();
    if toks.is_empty() {
        return false;
    }
    let formula = super::parse_formula(text);
    if formula.is_empty() {
        return false;
    }
    // Reject plain words (all lowercase, or longer than a formula token).
    !toks.iter().any(|t| {
        t.chars().all(|c| c.is_ascii_lowercase()) || t.chars().any(|c| !c.is_ascii_alphanumeric())
    })
}

/// Hill notation: C first, then H, then the rest alphabetically; without
/// carbon everything is alphabetical. Counts of 1 are omitted.
pub fn hill_formula(formula: &BTreeMap<String, f64>) -> String {
    let fmt_count = |n: f64| {
        if (n - 1.0).abs() < 1e-9 {
            String::new()
        } else if (n - n.round()).abs() < 1e-9 {
            format!("{}", n.round() as i64)
        } else {
            format!("{n}")
        }
    };
    let mut parts = Vec::new();
    let has_c = formula.contains_key("C");
    if has_c {
        parts.push(format!("C{}", fmt_count(formula["C"])));
        if let Some(h) = formula.get("H") {
            parts.push(format!("H{}", fmt_count(*h)));
        }
    }
    for (el, n) in formula {
        if has_c && (el == "C" || el == "H") {
            continue;
        }
        parts.push(format!("{el}{}", fmt_count(*n)));
    }
    parts.join(" ")
}

/// COD wraps formulas as `- Fe S2 -`; strip the markers.
fn clean_formula(s: &str) -> String {
    s.trim().trim_matches('-').trim().to_string()
}

/// Convert one JSON record of the `result?format=json` response.
pub fn hit_from_record(rec: &Value) -> Option<StructureHit> {
    let id = rec.get("file")?.as_str()?.trim().to_string();
    if id.is_empty() {
        return None;
    }
    let s = |k: &str| {
        rec.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    let formula = s("formula")
        .or_else(|| s("calcformula"))
        .map(|f| clean_formula(&f))
        .unwrap_or_default();
    let elements: Vec<String> = super::parse_formula(&formula).into_keys().collect();
    let name = s("mineral")
        .or_else(|| s("commonname"))
        .or_else(|| s("chemname"));
    let space_group = s("sg");
    let mut extra = BTreeMap::new();
    if let Some(n) = s("sgNumber") {
        extra.insert("space_group_number".into(), n);
    }
    if let (Some(a), Some(b), Some(c)) = (s("a"), s("b"), s("c")) {
        let angles = match (s("alpha"), s("beta"), s("gamma")) {
            (Some(al), Some(be), Some(ga)) => format!(" α {al} β {be} γ {ga}"),
            _ => String::new(),
        };
        extra.insert("cell".into(), format!("a {a} b {b} c {c}{angles}"));
    }
    if let Some(v) = s("vol") {
        extra.insert("volume".into(), v);
    }
    if let Some(z) = s("Z") {
        extra.insert("Z".into(), z);
    }
    let journal = match (s("journal"), s("year")) {
        (Some(j), Some(y)) => Some(format!("{j} ({y})")),
        (Some(j), None) => Some(j),
        (None, Some(y)) => Some(y),
        (None, None) => None,
    };
    if let Some(j) = journal {
        extra.insert("journal".into(), j);
    }
    if let Some(t) = s("title") {
        extra.insert("title".into(), t);
    }
    if let Some(a) = s("authors") {
        extra.insert("authors".into(), a);
    }
    if let Some(d) = s("doi") {
        extra.insert("doi".into(), d);
    }
    if let Some(f) = s("flags") {
        extra.insert("flags".into(), f);
    }
    Some(StructureHit {
        id,
        source: "cod".into(),
        formula,
        name,
        space_group,
        elements,
        extra,
    })
}

/// All records of a `result?format=json` response as hits.
pub fn hits_from_response(v: &Value) -> Result<Vec<StructureHit>, StructureError> {
    let arr = v.as_array().ok_or_else(|| StructureError::Network {
        reason: "COD: response is not a JSON array".into(),
    })?;
    Ok(arr.iter().filter_map(hit_from_record).collect())
}

impl StructureSource for Cod {
    fn name(&self) -> &str {
        "cod"
    }

    fn search(&self, query: &StructureQuery) -> Result<Vec<StructureHit>, StructureError> {
        if query
            .text
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
            && query.elements.is_empty()
        {
            return Err(StructureError::Network {
                reason: "COD: enter a formula, a mineral or compound name, elements, or a COD id"
                    .into(),
            });
        }
        let v = self.get_json(&Self::result_query(query))?;
        let mut hits = hits_from_response(&v)?;
        // Entries without coordinates cannot become clusters (COD flags
        // every usable entry with "has coordinates").
        hits.retain(|h| {
            h.extra
                .get("flags")
                .map(|f| f.contains("has coordinates"))
                .unwrap_or(false)
        });
        // Constraints the endpoint cannot express exactly (element sets).
        let mut q = query.clone();
        q.text = None;
        hits.retain(|h| q.matches(h));
        let limit = if query.limit == 0 {
            DEFAULT_LIMIT
        } else {
            query.limit
        };
        hits.truncate(limit);
        Ok(hits)
    }

    fn fetch(&self, hit: &StructureHit) -> Result<Structure, StructureError> {
        let mut structure = self.entry(&hit.id)?;
        if structure.mineral.is_none() {
            structure.mineral = hit.name.clone();
        }
        Ok(structure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/testfiles/cod_result_fes2.json"
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn hits_from_fixture() {
        let hits = hits_from_response(&fixture()).unwrap();
        assert_eq!(hits.len(), 6);
        let pyrite = hits
            .iter()
            .find(|h| h.id == "1544891")
            .expect("pyrite record");
        assert_eq!(pyrite.formula, "Fe S2");
        assert_eq!(pyrite.source, "cod");
        assert_eq!(pyrite.name.as_deref(), Some("pyrite at 1 atm"));
        assert_eq!(pyrite.space_group.as_deref(), Some("P a -3"));
        assert_eq!(pyrite.elements, vec!["Fe".to_string(), "S".to_string()]);
        assert_eq!(
            pyrite.extra.get("journal").map(String::as_str),
            Some("Mineralogical Journal (1986)")
        );
        assert_eq!(
            pyrite.extra.get("space_group_number").map(String::as_str),
            Some("205")
        );
        assert!(pyrite.extra["cell"].starts_with("a 5.417"));
        let marcasite = hits.iter().find(|h| h.id == "1011013").unwrap();
        assert_eq!(marcasite.name.as_deref(), Some("Marcasite"));
    }

    #[test]
    fn result_query_maps_text_formula_elements_and_ids() {
        let q = StructureQuery::text("pyrite");
        assert_eq!(Cod::result_query(&q), "result?format=json&text=pyrite");
        let q = StructureQuery::text("Fe S2");
        assert_eq!(Cod::result_query(&q), "result?format=json&formula=Fe+S2");
        let q = StructureQuery::text("RuO2");
        assert_eq!(Cod::result_query(&q), "result?format=json&formula=O2+Ru");
        let q = StructureQuery::text("9000594");
        assert_eq!(Cod::result_query(&q), "result?format=json&id=9000594");
        let q = StructureQuery::text("cod-9000594");
        assert_eq!(Cod::result_query(&q), "result?format=json&id=9000594");
        let q = StructureQuery::text("iron oxide");
        assert_eq!(Cod::result_query(&q), "result?format=json&text=iron+oxide");
        let q = StructureQuery {
            elements: vec!["Fe".into(), "S".into()],
            exclude: vec!["O".into()],
            exact_elements: true,
            ..Default::default()
        };
        assert_eq!(
            Cod::result_query(&q),
            "result?format=json&el1=Fe&el2=S&nel1=O&strictmin=2&strictmax=2"
        );
    }

    #[test]
    fn hill_formula_orders_carbon_first() {
        let mut f = BTreeMap::new();
        f.insert("O".to_string(), 2.0);
        f.insert("C".to_string(), 1.0);
        f.insert("H".to_string(), 4.0);
        f.insert("N".to_string(), 1.0);
        assert_eq!(hill_formula(&f), "C H4 N O2");
    }

    #[test]
    fn pyrite_cif_from_cod_parses() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/testfiles/cod_9000594_pyrite.cif"
        );
        let text = std::fs::read_to_string(path).unwrap();
        let s = structure_from_cif(&text).unwrap();
        assert_eq!(s.mineral.as_deref(), Some("Pyrite"));
        assert_eq!(s.space_group.number, Some(205));
        assert!((s.lattice.a - 5.417).abs() < 0.01, "a = {}", s.lattice.a);
        assert_eq!(s.sites.len(), 12, "Fe4 S8 conventional cell");
    }

    /// Live search + fetch against crystallography.net (network).
    #[test]
    #[ignore]
    fn live_search_pyrite() {
        let cod = Cod::default();
        let hits = cod.search(&StructureQuery::text("pyrite")).unwrap();
        assert!(!hits.is_empty());
        let hit = hits
            .iter()
            .find(|h| h.formula == "Fe S2")
            .unwrap_or(&hits[0]);
        let s = cod.fetch(hit).unwrap();
        assert!(!s.sites.is_empty());
        eprintln!(
            "{} hits; fetched {} ({} sites)",
            hits.len(),
            hit.id,
            s.sites.len()
        );
    }
}
