//! Assistant access to the same LCF/PCA forms used in the Data workspace.
//! Group identities and current cache fingerprints are checked before changing
//! the selection or running a calculation. No raw arrays cross the tool boundary.

use super::tools::{LcfSpaceChoice, Tool, ToolField};
use crate::{app::StudioApp, group_identity::GroupId};
use gpui::Context;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AnalysisRequest {
    pub method: String,
    pub target_group_id: GroupId,
    pub reference_group_ids: Vec<GroupId>,
    #[serde(default = "normalized")]
    pub space: String,
    /// Energy-space bounds are offsets from E0 in eV, matching the GUI.
    /// Chi-space bounds are absolute k in inverse angstroms.
    pub range: Option<(f64, f64)>,
    #[serde(default = "yes")]
    pub sum_to_one: bool,
    #[serde(default)]
    pub fit_energy_shifts: bool,
    pub components: Option<usize>,
    #[serde(default)]
    pub center: bool,
}
fn normalized() -> String {
    "norm".into()
}
fn yes() -> bool {
    true
}

impl AnalysisRequest {
    fn validate(&self) -> Result<(Tool, LcfSpaceChoice), String> {
        let tool = match self.method.as_str() {
            "lcf" => Tool::Lcf,
            "pca" => Tool::Pca,
            _ => return Err("Choose lcf or pca".into()),
        };
        let space = match self.space.as_str() {
            "norm" => LcfSpaceChoice::Norm,
            "flat" => LcfSpaceChoice::Flat,
            "derivative" => LcfSpaceChoice::Deriv,
            "chi" => LcfSpaceChoice::Chi,
            _ => return Err("Choose norm, flat, derivative or chi".into()),
        };
        let unique: BTreeSet<_> = self.reference_group_ids.iter().collect();
        if unique.len() < 2
            || unique.len() != self.reference_group_ids.len()
            || unique.contains(&self.target_group_id)
        {
            return Err(
                "Choose at least two distinct references, excluding the target group".into(),
            );
        }
        if self
            .range
            .is_some_and(|(lo, hi)| !lo.is_finite() || !hi.is_finite() || lo >= hi)
        {
            return Err("Range bounds must be finite and increasing".into());
        }
        if tool == Tool::Pca && self.components.is_none_or(|n| n == 0 || n > unique.len()) {
            return Err(
                "PCA components must be between one and the number of training groups".into(),
            );
        }
        if tool == Tool::Lcf && (self.components.is_some() || self.center) {
            return Err("components and center apply only to PCA".into());
        }
        if tool == Tool::Pca && self.fit_energy_shifts {
            return Err("PCA does not fit energy shifts".into());
        }
        Ok((tool, space))
    }
}

impl StudioApp {
    pub(super) fn assistant_run_analysis(
        &mut self,
        args: &Value,
        inspected: &[(super::tools::ToolTarget, super::Stage)],
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let request: AnalysisRequest =
            serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
        let (tool, space) = request.validate()?;
        if self.load_running || self.recompute_dirty || self.running_job_count() > 0 {
            return Err("Wait for current processing and analysis jobs to finish".into());
        }
        let target = self.assistant_group_index(&request.target_group_id)?;
        if self.current_group_index() != Some(target) {
            return Err("Navigate to target_group_id and inspect its processing first".into());
        }
        let references: BTreeSet<_> = request
            .reference_group_ids
            .iter()
            .map(|id| self.assistant_group_index(id))
            .collect::<Result<_, _>>()?;
        if references.contains(&crate::app::NO_ENTRY) {
            return Err(
                "Import the standalone reference into Groups before using it in LCF/PCA".into(),
            );
        }
        for &ix in references.iter().chain(std::iter::once(&target)) {
            let operand = self.tool_target(ix).ok_or("Group is unavailable")?;
            // Process shows normalized μ(E) and χ(k) together, so one
            // inspection covers both energy-space and χ analyses.
            if !inspected.contains(&(operand.clone(), super::Stage::Process)) {
                return Err(format!(
                    "Navigate to {} and call xray_get_plots in Process before analysis",
                    operand.label
                ));
            }
            if self
                .cache
                .peek(&(ix, self.effective_fingerprint(ix)))
                .is_none()
            {
                return Err(format!(
                    "Navigate to {} and inspect its current processing before analysis",
                    self.entry_label(ix)
                ));
            }
        }
        // Every requested operand is ready: the GUI's marked-spectra collector
        // must not silently reduce the reference set by skipping missing caches.
        self.selection = references;
        self.open_tool(tool, cx);
        self.tools.lcf_space = space;
        self.tools.pca_center = request.center;
        self.tools.lcf_sum_to_one = request.sum_to_one;
        self.tools.lcf_e0_shift = request.fit_energy_shifts;
        self.tools.lcf_all_combinations = false;
        self.tools.lcf_range = request.range;
        if let Some(n) = request.components {
            self.tools.pca_components = n;
        }
        for (field, value) in [
            (ToolField::RangeLo, request.range.map(|r| r.0)),
            (ToolField::RangeHi, request.range.map(|r| r.1)),
            (
                ToolField::Components,
                Some(self.tools.pca_components as f64),
            ),
        ] {
            if let Some((_, entity)) = self.tools.fields.iter().find(|(f, _)| *f == field) {
                entity.update(cx, |field, cx| field.set_value(value, cx));
            }
        }
        let message = self.run_analysis_tool(tool, cx)?;
        let sources = if tool == Tool::Lcf {
            &self.analysis.lcf_sources
        } else {
            &self.analysis.pca_sources
        };
        Ok(json!({"completed": true, "method": request.method,
            "target_group_id": request.target_group_id,
            "reference_group_ids": sources.iter().skip(1).map(|s| &s.group_id).collect::<Vec<_>>(),
            "reference_order": "Coefficient indices refer to this resolved reference order",
            "message": message, "result": self.assistant_analysis_summary()}))
    }

    /// Report scalar results and component coefficients without copying spectral
    /// arrays, PCA loadings or training matrices into the conversation.
    pub(super) fn assistant_analysis_summary(&self) -> Value {
        json!({
            "lcf": self.analysis.lcf.as_ref().map(|r| json!({
                "inputs": self.analysis.lcf_sources.iter().map(|s| s.operation_input()).collect::<Vec<_>>(),
                "stale": !self.analysis_sources_current(&self.analysis.lcf_sources),
                "space": r.space, "weights": r.weights, "r_factor": r.r_factor,
                "sum_of_weights": r.sum_of_weights, "n_data": r.n_data, "n_vary": r.n_vary,
                "range": [r.x.as_slice().first(), r.x.as_slice().last()]})),
            "pca": self.analysis.pca.as_ref().map(|m| json!({
                "inputs": self.analysis.pca_sources.iter().map(|s| s.operation_input()).collect::<Vec<_>>(),
                "stale": !self.analysis_sources_current(&self.analysis.pca_sources),
                "space": m.space, "centered": m.centered, "training_groups": m.labels,
                "eigenvalues": m.eigenvalues, "variance_explained": m.variance_explained,
                "cumulative_variance": m.cumulative_variance, "indicator": m.ind,
                "points": m.x.len(), "range": [m.x.as_slice().first(), m.x.as_slice().last()]})),
            "pca_target": self.analysis.pca_fit.as_ref().map(|f| json!({
                "components": f.n_components, "weights": f.weights, "r_factor": f.r_factor})),
            "note": "Uncentered PCA fractions describe squared signal, not chemical concentrations. Component count alone does not identify chemical species. Review overlays and residuals."
        })
    }

    fn analysis_sources_current(&self, sources: &[super::tools::ToolTarget]) -> bool {
        !sources.is_empty()
            && sources
                .iter()
                .all(|s| self.tool_target(s.ix).as_ref() == Some(s))
    }

    /// Return the displayed analysis overlay only for its unchanged target and
    /// reference set. A spectrum selected later must not inherit an old result.
    pub(super) fn assistant_analysis_plot(&self) -> Option<(&'static str, ruviz::prelude::Plot)> {
        let (space, sources) = match self.analysis.shown? {
            Tool::Lcf => (
                self.analysis.lcf.as_ref()?.space,
                &self.analysis.lcf_sources,
            ),
            Tool::Pca => (
                self.analysis.pca.as_ref()?.space,
                &self.analysis.pca_sources,
            ),
            _ => return None,
        };
        if !self.analysis_sources_current(sources)
            || self.current_tool_target().as_ref() != sources.first()
        {
            return None;
        }
        let (x, y) = match space {
            rexafs::prelude::AnalysisSpace::Norm => ("Energy (eV)", "normalized μ(E)".into()),
            rexafs::prelude::AnalysisSpace::Flat => ("Energy (eV)", "flattened μ(E)".into()),
            rexafs::prelude::AnalysisSpace::Deriv => ("Energy (eV)", "dμ/dE (eV⁻¹)".into()),
            rexafs::prelude::AnalysisSpace::Chi { kweight } => (
                crate::plotting::K_AXIS,
                crate::plotting::chik_label(kweight),
            ),
        };
        let theme = crate::theme::Theme::light();
        match self.analysis.shown? {
            Tool::Lcf => Some((
                "LCF overlay and residual",
                crate::plotting::build_lcf_plot(self.analysis.lcf.as_ref()?, x, &y, &theme),
            )),
            Tool::Pca => Some((
                "PCA target and residual",
                crate::plotting::build_pca_plot(self.analysis.pca_fit.as_ref()?, x, &y, &theme),
            )),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_component_mixtures_survive_gui_processing_lcf_and_pca() {
        use crate::params::{DerivedSpectrum, PipelineParams};
        use rexafs::prelude::{LcfConfig, PcaConfig, lcf, pca_train};
        let energy: Vec<f64> = (0..1231).map(|i| 8770. + i as f64).collect();
        let params = PipelineParams {
            e0: Some(8980.),
            edge_step: Some(1.),
            pre_edge_start: Some(-190.),
            pre_edge_end: Some(-40.),
            norm_start: Some(150.),
            norm_end: Some(900.),
            norm_polyorder: Some(1),
            fft_kmin: Some(3.),
            fft_kmax: Some(12.),
            ..Default::default()
        };
        // Three Cu-edge-like shapes with a common step and distinct Gaussian
        // features. These are controlled signals, not chemical reference spectra.
        let raw: Vec<Vec<f64>> = [(8987., 0.45, 4.), (9005., 0.65, 6.), (9027., 0.4, 8.)]
            .into_iter()
            .map(|(center, amplitude, width)| {
                energy
                    .iter()
                    .map(|&e| {
                        0.2 + 0.00003 * (e - 8980.)
                            + 1. / (1. + (-(e - 8980.) / 2.).exp())
                            + amplitude * (-((e - center) / width).powi(2)).exp()
                    })
                    .collect()
            })
            .collect();
        let process = |mu: Vec<f64>| {
            DerivedSpectrum {
                energy: energy.clone(),
                mu,
                ..Default::default()
            }
            .process(&params)
            .unwrap()
        };
        let references: Vec<_> = raw.iter().cloned().map(process).collect();
        let mix = |weights: [f64; 3], noise: bool| {
            process(
                (0..energy.len())
                    .map(|i| {
                        (0..3).map(|j| weights[j] * raw[j][i]).sum::<f64>()
                            + if noise {
                                (i as f64 * 1.234).sin() * 1e-5
                            } else {
                                0.
                            }
                    })
                    .collect(),
            )
        };
        let target = mix([0.2, 0.3, 0.5], true);
        let range = Some((-20., 80.));
        let fit = lcf(
            &target,
            &references,
            &LcfConfig {
                range,
                ..Default::default()
            },
        )
        .unwrap();
        for (component, expected) in fit.weights.iter().zip([0.2, 0.3, 0.5]) {
            assert!(
                (component.weight - expected).abs() < 5e-4,
                "{:?}",
                fit.weights
            );
        }
        let training: Vec<_> = [
            [0.8, 0.1, 0.1],
            [0.1, 0.8, 0.1],
            [0.1, 0.1, 0.8],
            [0.3, 0.5, 0.2],
            [0.4, 0.2, 0.4],
        ]
        .into_iter()
        .map(|w| mix(w, false))
        .collect();
        let model = pca_train(
            &training,
            &PcaConfig {
                range,
                center: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(model.cumulative_variance[2] > 1. - 1e-10);
        let reconstructed = model.target_transform(&target, 3).unwrap();
        assert!(reconstructed.r_factor < 1e-8, "{}", reconstructed.r_factor);
        assert!(
            model.target_transform(&target, 2).unwrap().r_factor > reconstructed.r_factor * 100.
        );
    }

    fn request() -> Value {
        json!({"method":"pca","target_group_id":"result:1",
            "reference_group_ids":["result:2","result:3"],"components":2,"range":[-20.,80.]})
    }
    #[test]
    fn analysis_rejects_ambiguous_operands_and_invalid_rank_before_gui_changes() {
        let valid: AnalysisRequest = serde_json::from_value(request()).unwrap();
        assert!(valid.validate().is_ok());
        for (key, value) in [
            ("reference_group_ids", json!(["result:2", "result:2"])),
            ("reference_group_ids", json!(["result:1", "result:2"])),
            ("components", json!(3)),
            ("components", json!(0)),
            ("range", json!([80., -20.])),
            ("space", json!("unknown")),
            ("fit_energy_shifts", json!(true)),
        ] {
            let mut invalid = request();
            invalid[key] = value;
            assert!(
                serde_json::from_value::<AnalysisRequest>(invalid)
                    .unwrap()
                    .validate()
                    .is_err(),
                "{key}"
            );
        }
        let mut invalid = request();
        invalid["raw_data"] = json!([1, 2]);
        assert!(serde_json::from_value::<AnalysisRequest>(invalid).is_err());
    }
}
