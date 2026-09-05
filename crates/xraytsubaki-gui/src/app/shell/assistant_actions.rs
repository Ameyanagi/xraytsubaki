//! Semantic assistant actions: explicit spectrum targets and reversible model edits.
use super::{BkgView, EQuantity, FitView, Stage, TfView, fit_workspace::FitStep, journal::UndoOp};
use crate::{
    app::StudioApp,
    fitting::{FitRanges, FitVarSpec},
    joint_fitting::JointConfig,
};
use gpui::Context;
use serde_json::{Value, json};

#[derive(Clone)]
pub(crate) struct ModelSettings {
    pub paths: Vec<crate::fitting::FitPathSpec>,
    pub ranges: FitRanges,
    pub variables: Vec<FitVarSpec>,
    pub joint: JointConfig,
}
impl StudioApp {
    pub(crate) fn assistant_navigate(
        &mut self,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        // Validate navigation before changing the active spectrum.
        let stage = match args["stage"]
            .as_str()
            .unwrap_or(self.stage.name())
            .to_ascii_lowercase()
            .as_str()
        {
            "data" => Stage::Data,
            "normalize" => Stage::Normalize,
            "background" => Stage::Background,
            "transform" => Stage::Transform,
            "fit" => Stage::Fit,
            "series" => Stage::Series,
            "publish" => Stage::Publish,
            _ => return Err("Unknown stage".into()),
        };
        let step = match args["fit_step"].as_str() {
            None => None,
            Some("structure") => Some(FitStep::Structure),
            Some("calculate") => Some(FitStep::Calculate),
            Some("paths") => Some(FitStep::Paths),
            Some("model") => Some(FitStep::Model),
            Some("results") => Some(FitStep::Results),
            _ => return Err("Unknown fit step".into()),
        };
        let view = args["plot"].as_str();
        if view.is_some_and(|v| {
            !matches!(
                v,
                "k" | "r" | "q" | "k+r" | "mu" | "normalized_mu" | "flat_mu"
            )
        }) {
            return Err("Unknown plot".into());
        }
        let dataset = args["dataset_id"].as_u64().map(|v| v as usize);
        if let Some(id) = dataset {
            if !self.joint.config.datasets.iter().any(|d| d.id == id) {
                return Err("Unknown fit spectrum id".into());
            }
        }
        if let Some(file) = args["spectrum"].as_str() {
            let ix = (0..self.catalog.len())
                .find(|&ix| self.catalog.path(ix).to_string_lossy() == file)
                .ok_or("Spectrum must already be in the open catalog")?;
            self.select_entry(ix, cx);
        }
        self.set_stage(stage, cx);
        if let Some(step) = step {
            self.set_fit_step(step, cx);
        }
        if let Some(id) = dataset {
            self.joint.selected = Some((id, None));
            if let Some(result) = &self.joint.result_config {
                if let Some(index) = result.datasets.iter().position(|d| d.id == id) {
                    self.joint.result_index = index;
                    self.rebuild_fit_plots(cx);
                }
            }
        }
        if let Some(view) = view {
            if stage == Stage::Fit {
                let v = match view {
                    "k" => FitView::K,
                    "r" => FitView::R,
                    "q" => FitView::Q,
                    _ => FitView::Both,
                };
                self.stage_view.fit_view = v;
                self.fit_preview.view = v;
                self.rebuild_fit_preview_plots(cx, true);
                self.rebuild_fit_plots(cx);
            } else {
                match view {
                    "mu" => self.stage_view.e_quantity = EQuantity::Mu,
                    "normalized_mu" => self.stage_view.e_quantity = EQuantity::Norm,
                    "flat_mu" => self.stage_view.e_quantity = EQuantity::Flat,
                    "k" => {
                        self.stage_view.tf_view = TfView::K;
                        self.stage_view.bkg_view = BkgView::K;
                    }
                    "r" => self.stage_view.tf_view = TfView::R,
                    "q" => self.stage_view.tf_view = TfView::Q,
                    _ => self.stage_view.tf_view = TfView::Both,
                };
                self.stage_view_changed(cx);
            }
        }
        self.status = format!("Assistant · {}", stage.name()).into();
        cx.notify();
        Ok(
            json!({"stage":stage.name(),"fit_step":format!("{:?}",self.stage_view.fit_step),"plot":view,"current_spectrum":self.current_path,"dataset_id":dataset,"loading":self.load_running}),
        )
    }
    fn model_settings(&self) -> ModelSettings {
        ModelSettings {
            paths: self.fit_paths.iter().map(|p| p.spec.clone()).collect(),
            ranges: self.fit_ranges.clone(),
            variables: self.fit_vars.iter().map(|v| v.spec.clone()).collect(),
            joint: self.joint.config.clone(),
        }
    }
    pub(crate) fn restore_model_settings(&mut self, s: &ModelSettings, cx: &mut Context<Self>) {
        self.fit_paths.clear();
        for spec in &s.paths {
            self.add_path_row(spec.clone(), cx);
        }
        self.refresh_path_infos();
        self.fit_vars.clear();
        for spec in &s.variables {
            self.ensure_fit_var(&spec.name, spec.value, cx);
        }
        self.fit_ranges = s.ranges.clone();
        self.joint.config = s.joint.clone();
        self.joint.fields.clear();
        for spec in &s.variables {
            if let Some(v) = self.fit_vars.iter_mut().find(|v| v.spec.name == spec.name) {
                v.spec = spec.clone();
                v.field.update(cx, |f, cx| {
                    f.set_text(
                        spec.expr.clone().unwrap_or_else(|| spec.value.to_string()),
                        cx,
                    )
                });
                v.min_field.update(cx, |f, cx| {
                    f.set_text(spec.min.map(|n| n.to_string()).unwrap_or_default(), cx)
                });
                v.max_field.update(cx, |f, cx| {
                    f.set_text(spec.max.map(|n| n.to_string()).unwrap_or_default(), cx)
                });
            }
        }
        self.sync_range_fields(cx);
        self.fit_template_dirty = true;
        self.fit_model_changed(cx);
        cx.notify();
    }
    pub(crate) fn assistant_calculation_state(&self, cx: &gpui::App) -> Value {
        json!({"searching":self.structure.search_running,"loading":self.structure.fetch_running,"calculating":self.feff_running,"error":self.structure.search_error,"status":self.status.to_string(),
         "results":self.structure.hits.iter().map(|h|json!({"id":h.id,"formula":h.formula,"name":h.name,"space_group":h.space_group,"source":h.source.label()})).collect::<Vec<_>>(),
         "selected":self.structure.summary.as_ref().map(|s|json!({"id":s.hit.id,"formula":s.hit.formula,"name":s.hit.name,"lattice":s.lattice,"sites":s.sites.iter().map(|s|json!({"label":s.label,"element":s.symbol,"site_index":s.site_index,"multiplicity":s.multiplicity})).collect::<Vec<_>>()})),
         "absorber":self.structure.absorber,"absorber_site":self.structure.absorber_site,"edge":self.structure.edge,"cluster_radius_angstrom":self.structure.radius.read(cx).text(),"backend":crate::feffgen::backend_name(self.structure.backend),"workspace":self.feff_workspace,"paths":self.fit_path_infos})
    }
    pub(crate) fn assistant_select_paths(
        &mut self,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let files: Vec<std::path::PathBuf> =
            serde_json::from_value(args["files"].clone()).map_err(|e| e.to_string())?;
        if files.is_empty() {
            return Err("Choose at least one path".into());
        }
        let indices: Option<Vec<_>> = files
            .iter()
            .map(|file| self.fit_paths.iter().position(|p| &p.spec.file == file))
            .collect();
        let indices = indices.ok_or("Select existing path files from state")?;
        let id = args["dataset_id"].as_u64().map(|n| n as usize);
        if self.joint.config.enabled && id.is_none() {
            return Err("Specify dataset_id for path assignment in a multiple-spectrum fit".into());
        }
        if !self.joint.config.enabled && id.is_some() {
            return Err("dataset_id is only used for a multiple-spectrum fit".into());
        }
        if self.fit_running || self.feff_running {
            return Err("Wait for the running fit or path calculation".into());
        }
        if let Some(id) = id {
            if !self.joint.config.datasets.iter().any(|d| d.id == id) {
                return Err("Unknown dataset_id".into());
            }
        }
        let before = self.model_settings();
        if let Some(id) = id {
            let d = self
                .joint
                .config
                .datasets
                .iter_mut()
                .find(|d| d.id == id)
                .unwrap();
            d.paths = files.clone();
            d.expressions.retain(|f, _| files.contains(f));
            self.joint.selected = Some((id, files.first().cloned()));
        } else {
            for (i, p) in self.fit_paths.iter_mut().enumerate() {
                p.spec.enabled = indices.contains(&i);
            }
            self.paths_selection_changed(cx);
        }
        if args["rebuild_parameters"] == true {
            self.apply_fit_template(cx);
        }
        let after = self.model_settings();
        self.record(
            "Assistant: select paths",
            Some(UndoOp::FitModel { before, after }),
        );
        self.set_stage(Stage::Fit, cx);
        self.set_fit_step(FitStep::Paths, cx);
        self.fit_model_changed(cx);
        cx.notify();
        Ok(json!({"selected_paths":files,"dataset_id":id}))
    }
    pub(crate) fn assistant_fit_ranges(
        &mut self,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        if self.fit_running {
            return Err("Wait for the running fit".into());
        }
        let id = args["dataset_id"].as_u64().map(|n| n as usize);
        let old = if self.joint.config.enabled {
            let id = id.ok_or("Specify dataset_id for a multiple-spectrum fit")?;
            self.joint
                .config
                .datasets
                .iter()
                .find(|d| d.id == id)
                .ok_or("Unknown dataset_id")?
                .ranges
                .as_ref()
                .unwrap_or(&self.fit_ranges)
        } else {
            if id.is_some() {
                return Err("dataset_id is only used for a multiple-spectrum fit".into());
            }
            &self.fit_ranges
        };
        let ranges = proposed_ranges(old, &args["ranges"])?;
        let before = self.model_settings();
        if let Some(id) = id {
            self.joint
                .config
                .datasets
                .iter_mut()
                .find(|d| d.id == id)
                .unwrap()
                .ranges = Some(ranges.clone());
            self.joint.selected = Some((id, None));
        } else {
            self.fit_ranges = ranges.clone();
        }
        let after = self.model_settings();
        self.record(
            "Assistant: fit ranges",
            Some(UndoOp::FitModel { before, after }),
        );
        self.set_stage(Stage::Fit, cx);
        self.set_fit_step(FitStep::Model, cx);
        self.sync_range_fields(cx);
        self.select_fit_space_view(ranges.fitspace, cx);
        self.fit_model_changed(cx);
        cx.notify();
        Ok(json!({"applied":true,"dataset_id":id,"ranges":ranges}))
    }
    pub(crate) fn assistant_fit_parameter(
        &mut self,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        if self.fit_running {
            return Err("Wait for the running fit".into());
        }
        let name = args["name"].as_str().ok_or("Missing parameter name")?;
        let value = args["value"]
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or("value must be finite")?;
        let spec = self
            .fit_vars
            .iter()
            .find(|v| v.spec.name == name)
            .ok_or("Use an existing parameter name from state")?
            .spec
            .clone();
        if spec.expr.is_some() {
            return Err(
                "This parameter is constrained by an expression; edit its independent variables"
                    .into(),
            );
        }
        if spec.min.is_some_and(|v| value < v) || spec.max.is_some_and(|v| value > v) {
            return Err("Value is outside this parameter's bounds".into());
        }
        let id = args["dataset_id"].as_u64().map(|n| n as usize);
        if let Some(id) = id {
            if !self.joint.config.enabled || !self.joint.config.datasets.iter().any(|d| d.id == id)
            {
                return Err("Unknown multiple-fit dataset_id".into());
            }
        }
        if id.is_none()
            && self.joint.config.enabled
            && !self.joint.config.datasets.is_empty()
            && self
                .joint
                .config
                .datasets
                .iter()
                .all(|d| self.joint.config.is_local(d.id, name))
        {
            return Err("This parameter is local in every spectrum. Specify dataset_id to change its value.".into());
        }
        let before = self.model_settings();
        if let Some(id) = id {
            if !self.joint.config.is_local(id, name) {
                return Err(
                    "This is a global parameter. Omit dataset_id to change its shared value."
                        .into(),
                );
            }
            self.joint
                .config
                .values
                .entry(id)
                .or_default()
                .insert(name.into(), value);
            self.joint.selected = Some((id, None));
        } else {
            let v = self
                .fit_vars
                .iter_mut()
                .find(|v| v.spec.name == name)
                .unwrap();
            v.spec.value = value;
            v.field
                .update(cx, |f, cx| f.set_text(value.to_string(), cx));
        }
        let after = self.model_settings();
        self.record(
            format!("Assistant: {name} = {value}"),
            Some(UndoOp::FitModel { before, after }),
        );
        self.set_stage(Stage::Fit, cx);
        self.set_fit_step(FitStep::Model, cx);
        self.fit_model_changed(cx);
        cx.notify();
        Ok(
            json!({"applied":true,"name":name,"value":value,"dataset_id":id,"scope":if id.is_some(){"local"}else{"global"}}),
        )
    }
}
fn proposed_ranges(old: &FitRanges, patch: &Value) -> Result<FitRanges, String> {
    let changes = patch.as_object().ok_or("ranges must be an object")?;
    let mut v = serde_json::to_value(old).map_err(|e| e.to_string())?;
    for (k, x) in changes {
        if !matches!(
            k.as_str(),
            "kmin"
                | "kmax"
                | "rmin"
                | "rmax"
                | "kweight"
                | "kweights"
                | "follow_transform"
                | "fitspace"
                | "noise"
        ) {
            return Err(format!("Unknown fit range field: {k}"));
        }
        v[k] = x.clone();
    }
    let r: FitRanges = serde_json::from_value(v).map_err(|e| e.to_string())?;
    if ![r.kmin, r.kmax, r.rmin, r.rmax, r.kweight]
        .iter()
        .all(|v| v.is_finite() && *v >= 0.)
        || r.kmin >= r.kmax
        || r.rmin >= r.rmax
        || r.effective_kweights()
            .iter()
            .any(|v| !v.is_finite() || *v < 0.)
    {
        return Err("Invalid k/R range or k-weight".into());
    }
    Ok(r)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fit_range_patch_preserves_individual_weights() {
        let r = FitRanges {
            follow_transform: false,
            kweights: vec![3.],
            ..Default::default()
        };
        let p = proposed_ranges(&r, &json!({"rmin":1.4,"rmax":3.6})).unwrap();
        assert_eq!(p.kweights, vec![3.]);
        assert!(!p.follow_transform);
        assert!(proposed_ranges(&r, &json!({"kmin":20.})).is_err());
        assert!(proposed_ranges(&r, &json!({"unexpected":1})).is_err());
    }
}
