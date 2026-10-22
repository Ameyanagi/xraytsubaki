//! Semantic assistant actions: explicit spectrum targets and reversible model edits.
use super::assistant_receipts::{
    Receipt, diff, navigation_target, scope_label, select_history_result,
};
use super::{BkgView, EQuantity, FitView, Stage, TfView, fit_workspace::FitStep, journal::UndoOp};
use crate::{
    app::StudioApp,
    fitting::{FitRanges, FitVarSpec},
    joint_fitting::JointConfig,
};
use gpui::Context;
use serde_json::{Value, json};

#[derive(Clone, serde::Serialize)]
pub(crate) struct ModelSettings {
    pub paths: Vec<crate::fitting::FitPathSpec>,
    pub ranges: FitRanges,
    pub variables: Vec<FitVarSpec>,
    pub joint: JointConfig,
}
fn spectrum_index(
    catalog: &crate::catalog::Catalog,
    registry: &crate::group_identity::GroupRegistry,
    file: &str,
) -> Result<usize, String> {
    let ix = (0..catalog.len())
        .find(|&ix| catalog.path(ix) == std::path::Path::new(file))
        .ok_or("Spectrum must already be in the open catalog")?;
    if registry.index_excluded(ix) {
        return Err("group was removed".into());
    }
    Ok(ix)
}

impl StudioApp {
    /// Resolve an opaque identity from the overview, never a model-invented path.
    pub(super) fn assistant_group_index(
        &self,
        id: &crate::group_identity::GroupId,
    ) -> Result<usize, String> {
        self.group_registry
            .index(id)
            .or_else(|| {
                self.standalone_source
                    .as_ref()
                    .filter(|(_, _, current)| current == id)
                    .map(|_| crate::app::NO_ENTRY)
            })
            .filter(|&ix| self.tool_target(ix).is_some())
            .ok_or_else(|| "Group is no longer available; retrieve the overview again".into())
    }

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
        if let Some(id) = args.get("group_id") {
            if args.get("spectrum").is_some() || dataset.is_some() {
                return Err("Use exactly one spectrum selector".into());
            }
            let id = serde_json::from_value(id.clone()).map_err(|e| e.to_string())?;
            let ix = self.assistant_group_index(&id)?;
            self.select_entry(ix, cx);
        } else if let Some(file) = args["spectrum"].as_str() {
            let catalog_index =
                (0..self.catalog.len()).find(|&ix| self.catalog.path(ix).to_string_lossy() == file);
            if let Some(ix) = navigation_target(file, &self.current_path, catalog_index)? {
                spectrum_index(&self.catalog, &self.group_registry, file)?;
                self.select_entry(ix, cx);
            }
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
            json!({"stage":stage.name(),"fit_step":format!("{:?}",self.stage_view.fit_step),"plot":view,"current_spectrum":self.current_path,"group_id":self.current_group_index().and_then(|ix| self.group_id(ix)),"dataset_id":dataset,"loading":self.load_running}),
        )
    }
    /// Display cached results without restoring the model used to obtain them.
    pub(super) fn assistant_show_result(
        &mut self,
        id: usize,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let entry = self
            .fit_history
            .iter()
            .find(|e| e.id == id)
            .ok_or("Fit history is no longer available")?;
        let result = self
            .fit_history_results
            .get(&id)
            .cloned()
            .ok_or("Fit plots are no longer available in this session")?;
        self.joint.result_config = entry.joint.clone();
        self.joint.result_index = 0;
        self.fit_result = Some(result);
        self.pending_fit_restore = None;
        select_history_result(&mut self.fit_provenance, &mut self.fit_history_selected, id);
        self.stage_view.fit_result_tab = 0;
        self.set_stage(Stage::Fit, cx);
        self.set_fit_step(FitStep::Results, cx);
        self.rebuild_fit_plots(cx);
        cx.notify();
        Ok(json!({"shown_fit":id}))
    }
    fn model_settings(&self) -> ModelSettings {
        ModelSettings {
            paths: self.fit_paths.iter().map(|p| p.spec.clone()).collect(),
            ranges: self.fit_ranges.clone(),
            variables: self.fit_vars.iter().map(|v| v.spec.clone()).collect(),
            joint: self.joint.config.clone(),
        }
    }

    /// Configure the existing model for one spectrum or an explicit group set.
    /// Existing dataset ranges, paths, expressions and local values are retained.
    /// A first joint setup may declare local names; later scope edits remain in
    /// the GUI so this operation cannot erase per-dataset scope overrides.
    pub(super) fn assistant_configure_fit(
        &mut self,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<(Value, Option<Receipt>), String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            mode: String,
            #[serde(default)]
            group_ids: Vec<crate::group_identity::GroupId>,
            local_parameters: Option<std::collections::BTreeSet<String>>,
        }
        let request: Request = serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
        if self.fit_running || self.batch_running || self.feff_running {
            return Err("Wait for the running calculation".into());
        }
        let before = self.model_settings();
        let mut after = before.clone();
        match request.mode.as_str() {
            "single" => {
                if !request.group_ids.is_empty() || request.local_parameters.is_some() {
                    return Err(
                        "Single mode uses the current group; omit group_ids and local_parameters"
                            .into(),
                    );
                }
                after.joint.enabled = false;
            }
            "multiple" => {
                let unique: std::collections::BTreeSet<_> = request.group_ids.iter().collect();
                if unique.len() < 2 || unique.len() != request.group_ids.len() {
                    return Err("Select at least two distinct group_ids".into());
                }
                if let Some(local) = request.local_parameters {
                    if !before.joint.datasets.is_empty() {
                        return Err("Existing local scopes are preserved. Omit local_parameters or edit scopes in Spectra & paths".into());
                    }
                    if local
                        .iter()
                        .any(|name| !before.variables.iter().any(|v| &v.name == name))
                    {
                        return Err("Use existing fit variable names for local_parameters".into());
                    }
                    after.joint.local = local;
                }
                let mut next_id = before
                    .joint
                    .datasets
                    .iter()
                    .map(|d| d.id)
                    .max()
                    .unwrap_or(0)
                    + 1;
                let paths: Vec<_> = before
                    .paths
                    .iter()
                    .filter(|p| p.enabled)
                    .map(|p| p.file.clone())
                    .collect();
                let mut datasets = Vec::new();
                for id in request.group_ids {
                    let ix = self.assistant_group_index(&id)?;
                    let target = self.tool_target(ix).ok_or("Group is unavailable")?;
                    if let Some(existing) = before
                        .joint
                        .datasets
                        .iter()
                        .find(|d| d.source_id.as_ref() == Some(&id))
                    {
                        datasets.push(existing.clone());
                    } else {
                        datasets.push(crate::joint_fitting::JointDataset {
                            id: next_id,
                            file: target.path,
                            group_id: target.derived_id,
                            source_id: Some(id),
                            label: target.label,
                            paths: paths.clone(),
                            ..Default::default()
                        });
                        next_id += 1;
                    }
                }
                after.joint.datasets = datasets;
                after.joint.enabled = true;
                crate::joint_fitting::prepare(&after.joint, &after.paths, &after.variables)
                    .map_err(|e| e.to_string())?;
            }
            _ => return Err("Choose single or multiple".into()),
        }
        let lines = diff(&json!(before.joint), &json!(after.joint));
        self.restore_model_settings(&after, cx);
        self.record(
            "Assistant: fit mode and spectra",
            Some(UndoOp::FitModel { before, after }),
        );
        self.joint.setup = true;
        self.set_stage(Stage::Fit, cx);
        self.set_fit_step(FitStep::Model, cx);
        Ok((
            json!({"applied":true,"joint":self.joint.config}),
            self.model_receipt(None, "model", lines, None, cx),
        ))
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
    pub(super) fn assistant_select_paths(
        &mut self,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<(Value, Option<Receipt>), String> {
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
        let lines = diff(&json!(before), &json!(after));
        self.record(
            "Assistant: select paths",
            Some(UndoOp::FitModel { before, after }),
        );
        self.set_stage(Stage::Fit, cx);
        self.set_fit_step(FitStep::Paths, cx);
        self.fit_model_changed(cx);
        cx.notify();
        Ok((
            json!({"selected_paths":files,"dataset_id":id}),
            self.model_receipt(
                id,
                "paths",
                lines,
                (args["rebuild_parameters"] == true && self.joint.config.enabled)
                    .then_some(self.joint.config.datasets.len()),
                cx,
            ),
        ))
    }
    pub(super) fn assistant_fit_ranges(
        &mut self,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<(Value, Option<Receipt>), String> {
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
        let lines = diff(&json!(old), &json!(ranges));
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
        Ok((
            json!({"applied":true,"dataset_id":id,"ranges":ranges}),
            self.model_receipt(id, "model", lines, None, cx),
        ))
    }
    pub(super) fn assistant_fit_parameter(
        &mut self,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<(Value, Option<Receipt>), String> {
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
        let old_value = id.map_or(spec.value, |id| self.joint.config.initial_value(id, &spec));
        let lines = diff(&json!({name:old_value}), &json!({name:value}));
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
        let shared = (id.is_none() && self.joint.config.enabled).then(|| {
            self.joint
                .config
                .datasets
                .iter()
                .filter(|d| !self.joint.config.is_local(d.id, name))
                .count()
        });
        let mut receipt = self.model_receipt(id, "model", lines, shared, cx);
        if let Some(receipt) = &mut receipt {
            receipt.finish_parameter_edit(self.fit_preview.loading);
        }
        Ok((
            json!({"applied":true,"name":name,"value":value,"dataset_id":id,"scope":if id.is_some(){"local"}else{"global"}}),
            receipt,
        ))
    }
    fn model_receipt(
        &mut self,
        id: Option<usize>,
        step: &str,
        lines: Vec<String>,
        shared: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Option<Receipt> {
        if lines.is_empty() {
            return None;
        }
        let file = id
            .and_then(|id| self.joint.config.datasets.iter().find(|d| d.id == id))
            .map_or(&self.current_path, |d| &d.file);
        let mut receipt = Receipt::change(
            &file.to_string_lossy(),
            Stage::Fit,
            lines,
            self.journal.receipt_revision,
        );
        receipt.navigation["fit_step"] = json!(step);
        if let Some(id) = id {
            receipt.navigation["dataset_id"] = json!(id);
        }
        receipt.scope = scope_label(shared);
        receipt.model = self.fit_model_fingerprint();
        // Range previews are the calculation triggered by model edits; parameter
        // values themselves are applied synchronously and do not run a fit.
        if step == "model" {
            self.ensure_fit_preview(cx);
        } else {
            receipt.state = "Recalculation complete".into();
        }
        Some(receipt)
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

    #[test]
    fn assistant_spectrum_lookup_reports_removed_group() {
        let mut catalog = crate::catalog::Catalog::default();
        catalog.extend(vec![crate::catalog::FileMeta {
            dir: "/data".into(),
            name: "a.dat".into(),
            size: 0,
        }]);
        let registry = crate::group_identity::GroupRegistry::default();
        let id = registry.register_source(
            Some(0),
            catalog.path(0),
            Default::default(),
            &Default::default(),
        );
        // Path components compare native and forward slashes consistently on
        // Windows without performing filesystem I/O or resolving other files.
        let native = catalog.path(0).to_string_lossy().into_owned();
        for file in [native.as_str(), "/data/a.dat"] {
            assert_eq!(spectrum_index(&catalog, &registry, file), Ok(0));
        }
        assert_eq!(
            spectrum_index(&catalog, &registry, "/data/b.dat"),
            Err("Spectrum must already be in the open catalog".into())
        );
        registry.set_excluded(&std::collections::BTreeSet::from([id]));
        for file in [native.as_str(), "/data/a.dat"] {
            assert_eq!(
                spectrum_index(&catalog, &registry, file),
                Err("group was removed".into())
            );
        }
    }

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
