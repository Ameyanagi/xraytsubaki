//! Numbered captions and semantic tables shared by the manuscript and HTML report.
use super::*;

pub(crate) const TABLE_CAPTIONS: [(&str, &str, &str); 3] = [
    (
        "processing",
        "Processing table",
        "Selected spectrum processing settings. Auto values are resolved separately for each spectrum; Used records the processed result when available. Full import/advanced settings are in analysis.md and resolved.md. Energy ranges are relative to E₀ where labeled.",
    ),
    (
        "parameters",
        "Fit parameters table",
        "Fitted parameter estimates. Standard errors are one standard error from the fit covariance, not total experimental or model uncertainty. Parameter units follow their definitions in the recorded model; unavailable errors are not zero.",
    ),
    (
        "paths",
        "Path results table",
        "Scattering-path parameters. Distance is R_eff + ΔR; for multiple scattering it is half the total path length. Values with ± include one propagated standard error where available. S₀² is dimensionless.",
    ),
];

pub(crate) struct Table {
    pub number: usize,
    pub kind: &'static str,
    pub caption: String,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}
impl Table {
    fn new(
        number: usize,
        kind: &'static str,
        source: String,
        settings: &figures::FigureSettings,
        headers: &[&str],
        rows: Vec<Vec<String>>,
    ) -> Self {
        let default = TABLE_CAPTIONS
            .iter()
            .find(|(key, _, _)| *key == kind)
            .unwrap()
            .2;
        let caption = format!(
            "{source}. {}",
            settings
                .table_captions
                .get(kind)
                .map(String::as_str)
                .unwrap_or(default)
        );
        Self {
            number,
            kind,
            caption,
            headers: headers.iter().map(|s| s.to_string()).collect(),
            rows,
        }
    }
    pub fn markdown(&self) -> String {
        let mut text = format!(
            "**Table {}.** {}\n\n| {} |\n|{}|\n",
            self.number,
            cell(&self.caption),
            self.headers
                .iter()
                .map(cell)
                .collect::<Vec<_>>()
                .join(" | "),
            vec!["---"; self.headers.len()].join("|")
        );
        for row in &self.rows {
            text.push_str(&format!(
                "| {} |\n",
                row.iter().map(cell).collect::<Vec<_>>().join(" | ")
            ));
        }
        text.push('\n');
        text
    }
    fn html(&self) -> String {
        let mut text = format!(
            "<table id=\"table-{}\"><caption><strong>Table {}.</strong> {}</caption><thead><tr>",
            self.number,
            self.number,
            html(&self.caption)
        );
        for header in &self.headers {
            text.push_str(&format!("<th scope=\"col\">{}</th>", html(header)));
        }
        text.push_str("</tr></thead><tbody>");
        for row in &self.rows {
            text.push_str("<tr>");
            for (index, value) in row.iter().enumerate() {
                if index == 0 {
                    text.push_str(&format!("<th scope=\"row\">{}</th>", html(value)));
                } else {
                    text.push_str(&format!("<td>{}</td>", html(value)));
                }
            }
            text.push_str("</tr>");
        }
        text.push_str("</tbody></table>");
        text
    }
}

impl Snapshot {
    pub(crate) fn tables(&self) -> Vec<Table> {
        let mut tables = Vec::new();
        let settings = &self.project.publication;
        for (index, spectrum) in self.spectra.iter().enumerate() {
            let values = serde_json::to_value(&spectrum.params).unwrap();
            let resolved = spectrum
                .data
                .as_ref()
                .map(|sp| resolved_settings(sp))
                .unwrap_or(Value::Null);
            let fields = [
                ("Absorption edge E₀", "e0", "/e0", "eV"),
                (
                    "Pre-edge start (relative to E₀)",
                    "pre_edge_start",
                    "/normalization/PrePostEdge/pre_edge_start",
                    "eV",
                ),
                (
                    "Pre-edge end (relative to E₀)",
                    "pre_edge_end",
                    "/normalization/PrePostEdge/pre_edge_end",
                    "eV",
                ),
                (
                    "Normalization start (relative to E₀)",
                    "norm_start",
                    "/normalization/PrePostEdge/norm_start",
                    "eV",
                ),
                (
                    "Normalization end (relative to E₀)",
                    "norm_end",
                    "/normalization/PrePostEdge/norm_end",
                    "eV",
                ),
                (
                    "Normalization polynomial order",
                    "norm_polyorder",
                    "/normalization/PrePostEdge/norm_polyorder",
                    "—",
                ),
                (
                    "Background cutoff R_bkg",
                    "rbkg",
                    "/background/AUTOBK/rbkg",
                    "Å",
                ),
                (
                    "Background k-weight",
                    "bkg_kweight",
                    "/background/AUTOBK/kweight",
                    "—",
                ),
                (
                    "Background window",
                    "bkg_window",
                    "/background/AUTOBK/window",
                    "—",
                ),
                ("Transform k minimum", "fft_kmin", "/xftf/kmin", "Å⁻¹"),
                ("Transform k maximum", "fft_kmax", "/xftf/kmax", "Å⁻¹"),
                ("Transform k-weight", "fft_kweight", "/xftf/kweight", "—"),
                ("Transform window", "fft_window", "/xftf/window", "—"),
                ("Transform window taper Δk", "fft_dk", "/xftf/dk", "Å⁻¹"),
                ("Back-transform R minimum", "bft_rmin", "/xftr/rmin", "Å"),
                ("Back-transform R maximum", "bft_rmax", "/xftr/rmax", "Å"),
                ("Back-transform window taper ΔR", "bft_dr", "/xftr/dr", "Å"),
            ];
            let display = |v: &Value, missing: &str| {
                if v.is_null() {
                    missing.into()
                } else if let Some(s) = v.as_str() {
                    s.replace("KaiserBessel", "Kaiser–Bessel")
                } else {
                    v.to_string()
                }
            };
            let rows = fields
                .into_iter()
                .map(|(label, key, pointer, unit)| {
                    vec![
                        label.into(),
                        unit.into(),
                        display(&values[key], "Auto"),
                        display(
                            resolved.pointer(pointer).unwrap_or(&Value::Null),
                            "Unavailable",
                        ),
                    ]
                })
                .collect();
            tables.push(Table::new(
                tables.len() + 1,
                "processing",
                format!(
                    "Spectrum {}: {}",
                    index + 1,
                    spectrum
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                ),
                settings,
                &["Setting", "Unit", "Requested", "Used"],
                rows,
            ));
        }
        for fit in &self.project.fit_history {
            let source = format!("Fit {}: {}", fit.id, fit.group);
            let rows = fit
                .values
                .iter()
                .map(|(name, v, se)| {
                    vec![
                        name.clone(),
                        format!("{v:.8}"),
                        se.map(|v| format!("{v:.8}"))
                            .unwrap_or("Unavailable".into()),
                    ]
                })
                .collect();
            tables.push(Table::new(
                tables.len() + 1,
                "parameters",
                source.clone(),
                settings,
                &["Parameter", "Value", "Standard error"],
                rows,
            ));
            let estimate = |v: &Option<crate::fit_details::Estimate>| {
                v.as_ref()
                    .map(|v| v.label(6))
                    .unwrap_or("Unavailable".into())
            };
            let rows = fit
                .path_details
                .iter()
                .map(|p| {
                    vec![
                        p.label.clone(),
                        p.reff
                            .map(|v| format!("{v:.6}"))
                            .unwrap_or("Unavailable".into()),
                        estimate(&p.distance),
                        estimate(&p.deltar),
                        estimate(&p.sigma2),
                        estimate(&p.s02),
                        estimate(&p.e0),
                    ]
                })
                .collect();
            tables.push(Table::new(
                tables.len() + 1,
                "paths",
                source,
                settings,
                &[
                    "Path",
                    "R_eff (Å)",
                    "Distance (Å)",
                    "ΔR (Å)",
                    "σ² (Å²)",
                    "S₀²",
                    "ΔE₀ (eV)",
                ],
                rows,
            ));
        }
        tables
    }
}

pub(crate) fn html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(crate) fn write(
    destination: &Path,
    assets: &[Value],
    tables: &[Table],
    notices: &[String],
) -> Result<(), String> {
    let mut captions = String::from(
        "# Figure and table captions\n\nCaptions describe the exported curves and recorded tables. Add sample identity, acquisition conditions and interpretation from the experimental record.\n\n",
    );
    let mut report = format!(
        r#"<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>rexafs publication report</title>
<style>body{{max-width:1100px;margin:2.5rem auto;padding:0 1.5rem;color:#18232d;background:white;font:16px/1.6 Georgia,serif}}h1,h2,nav{{font-family:system-ui,sans-serif}}a{{color:#155c83}}figure{{margin:2.5rem 0;break-inside:avoid}}figure img{{display:block;width:auto;max-width:100%;height:auto;margin:auto}}figcaption{{margin-top:1rem}}table{{border-collapse:collapse;width:100%;margin:2.5rem 0;font-size:14px;break-inside:avoid}}caption{{caption-side:top;text-align:left;margin-bottom:.75rem}}th,td{{padding:.5rem .75rem;border-bottom:1px solid #ccc;text-align:right;overflow-wrap:anywhere}}th:first-child,td:first-child{{text-align:left}}thead{{border-top:2px solid #333;border-bottom:2px solid #333}}tbody th{{font-weight:normal}}.scroll{{overflow-x:auto}}@media print{{body{{max-width:none;margin:0;font-size:10pt}}nav{{display:none}}a{{color:inherit;text-decoration:none}}figure img{{max-height:180mm}}.scroll{{overflow:visible}}thead{{display:table-header-group}}}}</style>
<h1>rexafs publication report</h1><p>Analysis prepared with rexafs {}.</p><nav><a href="analysis.md">Analysis record</a> · <a href="resolved.md">Resolved settings</a> · <a href="methods.md">Methods draft</a> · <a href="references.md">References</a> · <a href="captions.md">Copy captions</a></nav>
<p>Figures and tables retain the selected settings. Complete the experimental methods and captions with sample and acquisition details before submission. The numerical record and export notices accompany this report.</p><h2>Figures</h2>"#,
        env!("CARGO_PKG_VERSION")
    );
    for asset in assets {
        let n = asset["number"].as_u64().unwrap();
        let caption = asset["caption"].as_str().unwrap();
        captions.push_str(&format!("**Figure {n}.** {}\n\n", cell(caption)));
        report.push_str(&format!("<figure id=\"figure-{n}\"><img src=\"{}\" alt=\"{}\"><figcaption><strong>Figure {n}.</strong> {} <a href=\"{}\">PNG</a> · <a href=\"{}\">SVG</a></figcaption></figure>",html(asset["svg"].as_str().unwrap()),html(caption),html(caption),html(asset["file"].as_str().unwrap()),html(asset["svg"].as_str().unwrap())));
    }
    report.push_str("<h2>Tables</h2>");
    for table in tables {
        captions.push_str(&format!(
            "**Table {}.** {}\n\n",
            table.number,
            cell(&table.caption)
        ));
        report.push_str(&format!("<div class=\"scroll\">{}</div>", table.html()));
    }
    if !notices.is_empty() {
        report.push_str("<h2>Export notices</h2><ul>");
        for notice in notices {
            report.push_str(&format!("<li>{}</li>", html(notice)));
        }
        report.push_str("</ul>");
    }
    report.push_str("</html>");
    fs::write(destination.join("captions.md"), captions).map_err(|e| e.to_string())?;
    fs::write(destination.join("report.html"), report).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historical_fit_tables_have_numbered_captions_units_and_error_definitions() {
        let project = crate::project::load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/projects/rexafs-0.1.0-links.rxs"),
        )
        .unwrap();
        let snapshot = Snapshot {
            project,
            current: "Cu.xmu".into(),
            spectra: vec![SpectrumInput {
                path: "Cu.xmu".into(),
                params: PipelineParams::default(),
                data: None,
            }],
            results: BTreeMap::new(),
            analysis: Value::Null,
            batch_csv: None,
            batch_stale: false,
            journal: vec![],
            screen: Value::Null,
        };
        let tables = snapshot.tables();
        assert_eq!(tables.len(), 3);
        assert_eq!(tables[1].number, 2);
        assert!(tables[1].caption.contains("errors are one standard error"));
        assert!(tables[1].markdown().contains("0.02000000"));
        let paths = tables[2].html();
        assert!(paths.contains("<caption><strong>Table 3.</strong>"));
        assert!(paths.contains("σ² (Å²)"));
        assert!(paths.contains("half the total path length"));
        let record = snapshot.markdown();
        for n in 1..=3 {
            assert_eq!(record.matches(&format!("**Table {n}.**")).count(), 1);
        }
    }
}
