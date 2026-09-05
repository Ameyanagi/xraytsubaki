//! A library browsing hint, never an automatic change to an existing model.
use xraytsubaki::xafs::{io::xdi::XdiHeader, structure::Element};

#[derive(Debug, PartialEq)]
pub(crate) struct SpectrumInterest {
    pub element: String,
    pub edge: Option<String>,
    pub estimated: bool,
}

impl SpectrumInterest {
    pub fn infer(header: Option<&XdiHeader>, e0: Option<f64>) -> Option<Self> {
        if let Some(header) = header
            && let Some(element) = header
                .get("Element.symbol")
                .and_then(|s| Element::from_symbol(s.trim()))
        {
            return Some(Self {
                element: element.symbol.into(),
                edge: header
                    .get("Element.edge")
                    .map(|s| s.trim().to_ascii_uppercase()),
                estimated: false,
            });
        }
        let e0 = e0.filter(|e| e.is_finite() && *e > 0.)?;
        let guess = xraydb::XrayDb::try_new()
            .ok()?
            .guess_edge(e0, None, Some(100.))?;
        Some(Self {
            element: guess.element,
            edge: Some(guess.edge),
            estimated: true,
        })
    }

    pub fn label(&self) -> String {
        format!(
            "{}{} · {}",
            self.element,
            self.edge
                .as_ref()
                .map(|s| format!(" {s}"))
                .unwrap_or_default(),
            if self.estimated {
                "estimated from E₀"
            } else {
                "XDI metadata"
            }
        )
    }
}

pub(crate) fn contains_element(hit: &crate::structure::StructureHit, element: &str) -> bool {
    hit.core
        .as_ref()
        .is_some_and(|h| h.elements.iter().any(|s| s.eq_ignore_ascii_case(element)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_filter_uses_elements_not_formula_substrings() {
        use crate::structure::{BuiltinProvider, StructureProvider};
        let hits = BuiltinProvider.search("").unwrap();
        let copper: Vec<_> = hits.iter().filter(|h| contains_element(h, "Cu")).collect();
        assert!(copper.iter().any(|h| h.formula == "Cu"));
        assert!(copper.iter().any(|h| h.formula == "CuO"));
        assert!(copper.iter().any(|h| h.formula == "Cu2O"));
        assert!(!copper.iter().any(|h| h.formula == "Ni"));
        assert!(
            !hits
                .iter()
                .filter(|h| h.formula == "Ni")
                .any(|h| contains_element(h, "N"))
        );
    }
    #[test]
    fn energy_hint_and_metadata_priority() {
        assert_eq!(
            SpectrumInterest::infer(None, Some(8977.5)).unwrap().element,
            "Cu"
        );
        assert_eq!(
            SpectrumInterest::infer(None, Some(8332.)).unwrap().element,
            "Ni"
        );
        let mut h = XdiHeader {
            version: "1.0".into(),
            applications: vec![],
            metadata: Default::default(),
            comments: vec![],
            columns: vec![],
            warnings: vec![],
        };
        h.metadata.insert("element.symbol".into(), " ni ".into());
        h.metadata.insert("element.edge".into(), "K".into());
        let hint = SpectrumInterest::infer(Some(&h), Some(8979.)).unwrap();
        assert_eq!(hint.element, "Ni");
        assert!(!hint.estimated);
        for e0 in [None, Some(f64::NAN), Some(-1.), Some(1e7)] {
            assert!(SpectrumInterest::infer(None, e0).is_none());
        }
    }
}
