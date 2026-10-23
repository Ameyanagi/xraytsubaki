//! Thick-sample fluorescence correction: preview first, then retain a separate group.
use super::*;
use crate::plot_ranges::plot_builder;
use crate::{
    app::{DERIVED_BASE, StudioApp},
    fluorescence_history::{self as history, CorrectionReceipt, CorrectionRecord},
    params::{DerivedSpectrum, DetectionMode, Operation, Quantity},
    series_measurements::FrameInput,
    widgets::text_input::{InputEvent, TextInput},
};
use gpui::{AppContext, Entity};
use rexafs::{AbsorptionMode, FluorescenceCorrection, Spectrum};
use ruviz::prelude::{LegendPosition, Plot};
use ruviz_gpui::RuvizPlot;
use std::sync::Arc;

mod controls;

#[derive(Default)]
pub(crate) struct FluorescenceState {
    pub open: bool,
    target: Option<crate::app::shell::tools::ToolTarget>,
    fields: Vec<Entity<TextInput>>,
    generation: u64,
    busy: bool,
    advanced: bool,
    notes: bool,
    assumed: bool,
    historical: bool,
    history_index: usize,
    factor: bool,
    record: Option<Arc<CorrectionRecord>>,
    plot: Option<Entity<RuvizPlot>>,
    message: String,
}
impl FluorescenceState {
    pub fn close(&mut self) {
        self.open = false;
        self.invalidate();
    }
    fn invalidate(&mut self) {
        self.generation += 1;
        self.busy = false;
        self.record = None;
        self.plot = None;
        self.factor = false;
    }
}
fn prepare_input(input: &FrameInput) -> Result<Spectrum, String> {
    if let Some(d) = &input.derived {
        if !d.corrections.is_empty() {
            return Err("Already corrected. Select the original spectrum.".into());
        }
        if d.quantity_unconfirmed || d.quantity != Quantity::RawMu {
            return Err("Select uncorrected μ(E), before normalization or flattening.".into());
        }
    }
    let raw = crate::params::load_group_raw_with_diagnostics(
        &input.path,
        &input.settings,
        input.derived.as_deref(),
    )?;
    let mode = input
        .derived
        .as_ref()
        .map(|d| d.acquisition_mode())
        .filter(|m| *m != AbsorptionMode::Unknown)
        .unwrap_or(match raw.channel {
            DetectionMode::Transmission | DetectionMode::Reference => AbsorptionMode::Transmission,
            _ => AbsorptionMode::Unknown,
        });
    let mut sp = Spectrum::from_arrays(&raw.energy, &raw.mu).map_err(|e| e.to_string())?;
    sp.set_absorption_mode(mode);
    sp.e0 = input.settings.e0;
    Ok(sp)
}
fn corrected_group(record: &CorrectionRecord, receipt: CorrectionReceipt) -> DerivedSpectrum {
    let r = &record.result;
    let mut settings = record.settings.for_materialized(0.);
    settings.e0 = Some(r.internal.e0);
    // These settings are for the subsequent independent normalization, not the
    // conventional pre/post fit saved inside the correction result.
    settings.import.mode = DetectionMode::MuColumn;
    DerivedSpectrum {
        label: format!("{} · corrected", record.input.label),
        energy: r.energy.clone(),
        mu: r.corrected_mu.clone(),
        absorption_mode: AbsorptionMode::Fluorescence,
        corrections: vec![receipt.clone()],
        params: Some(settings),
        operation: Some(Operation {
            tool: "Fluorescence correction".into(),
            inputs: vec![record.input.clone()],
            applied_energy_shift_ev: 0.0,
            parameters: serde_json::json!({"method":r.method,"definition":r.definition(),
                "history_digest":receipt.digest,"domain":"XANES only",
                "final_normalization":"independent; configured in Normalize"}),
        }),
        ..Default::default()
    }
}
impl StudioApp {
    pub(crate) fn correction_sources(&self, ix: usize) -> Vec<CorrectionReceipt> {
        ix.checked_sub(DERIVED_BASE)
            .and_then(|i| self.derived.get(i))
            .map(|d| d.corrections.clone())
            .unwrap_or_default()
    }
    pub(crate) fn fluorescence_matches_current(&self) -> bool {
        self.fluorescence.target == self.current_tool_target()
    }
    pub(crate) fn open_fluorescence(&mut self, cx: &mut Context<Self>) {
        self.wavelet.cancel();
        self.wavelet.open = false;
        self.peaks.open = false;
        self.tools.open = None;
        self.fluorescence.close();
        self.fluorescence.open = true;
        self.fluorescence.target = self.current_tool_target();
        self.fluorescence.historical = false;
        self.fluorescence.assumed = false;
        self.fluorescence.message = "Confirm fluorescence input and enter sample geometry.".into();
        let declared = self
            .current_group_index()
            .and_then(|i| self.group_declared_edge(i));
        let values = [
            String::new(),
            declared
                .as_ref()
                .map(|d| d.element.clone())
                .unwrap_or_default(),
            declared.map(|d| d.edge).unwrap_or("K".into()),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            "1".into(),
        ];
        self.fluorescence.fields.clear();
        for (label, value) in [
            "Composition",
            "Absorber",
            "Edge",
            "Emission",
            "Incident angle",
            "Exit angle",
            "Correction E0",
            "Internal pre from",
            "Internal pre to",
            "Internal post from",
            "Internal post to",
            "Internal degree",
        ]
        .into_iter()
        .zip(values)
        {
            let field = cx.new(|cx| {
                let placeholder = match label {
                    "Composition" => "e.g. CuO",
                    "Absorber" => "e.g. Cu",
                    "Emission" => "Ka1 or family:Ka",
                    "Correction E0" | "Internal pre from" | "Internal pre to"
                    | "Internal post from" | "Internal post to" => "auto",
                    _ => "",
                };
                let mut f = TextInput::new(placeholder, value, self.theme, cx);
                f.set_accessible_name(label);
                f
            });
            cx.subscribe(&field, |app, _, e, cx| {
                if matches!(e, InputEvent::Edited(_)) {
                    app.fluorescence.invalidate();
                    app.fluorescence.message = "Preview the updated settings.".into();
                    cx.notify();
                }
            })
            .detach();
            self.fluorescence.fields.push(field);
        }
        if self
            .current_group_index()
            .is_some_and(|i| !self.correction_sources(i).is_empty())
        {
            self.load_fluorescence_history(0, cx);
        }
        cx.notify();
    }
    fn load_fluorescence_history(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(receipt) = self
            .current_group_index()
            .and_then(|i| self.correction_sources(i).get(index).cloned())
        else {
            return;
        };
        self.fluorescence.invalidate();
        self.fluorescence.historical = true;
        self.fluorescence.history_index = index;
        self.fluorescence.message = "Loading retained correction…".into();
        let generation = self.fluorescence.generation;
        let project = self.project_generation;
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move { history::read(&receipt) })
                .await;
            this.update(cx, |app, cx| {
                if project != app.project_generation || generation != app.fluorescence.generation {
                    return;
                }
                match r {
                    Ok(r) => {
                        app.fluorescence.message =
                            format!("Ancestor correction · {}", r.input.label);
                        app.fluorescence.record = Some(Arc::new(r));
                        app.rebuild_fluorescence_plot(cx);
                    }
                    Err(e) => app.fluorescence.message = e,
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn fluorescence_definition(
        &self,
        cx: &Context<Self>,
    ) -> Result<FluorescenceCorrection, String> {
        let f = |i: usize| {
            self.fluorescence.fields[i]
                .read(cx)
                .text()
                .trim()
                .to_owned()
        };
        let number = |i: usize| {
            f(i).parse::<f64>()
                .map_err(|_| "Enter measured incident and exit angles in degrees.".to_owned())
        };
        if f(0).is_empty() || f(1).is_empty() || f(2).is_empty() || f(3).is_empty() {
            return Err("Enter composition, absorber, edge and emission.".into());
        }
        let mut m = FluorescenceCorrection::new(f(0), f(1), f(2)).angles(number(4)?, number(5)?);
        m = if let Some(family) = f(3).strip_prefix("family:") {
            m.line_family(family)
        } else {
            m.line(f(3))
        };
        if !f(6).is_empty() {
            m.e0 = Some(number(6).map_err(|_| "Enter E₀ in eV.")?);
        }
        for (i, slot) in [(7, &mut m.pre_edge), (9, &mut m.post_edge)] {
            if !f(i).is_empty() || !f(i + 1).is_empty() {
                *slot = Some([
                    number(i).map_err(|_| "Enter both interval bounds, in eV from E₀.")?,
                    number(i + 1).map_err(|_| "Enter both interval bounds, in eV from E₀.")?,
                ]);
            }
        }
        m.degree = f(11)
            .parse()
            .map_err(|_| "Internal degree must be an integer from 0 to 5.")?;
        Ok(m)
    }
    fn preview_fluorescence(&mut self, cx: &mut Context<Self>) {
        if self.fluorescence.busy || self.fluorescence.historical {
            return;
        }
        let result = self.fluorescence_definition(cx).and_then(|m| {
            if !self.fluorescence.assumed {
                return Err("Confirm that this is uncorrected fluorescence μ(E).".into());
            }
            let target = self.current_tool_target().ok_or("Select a spectrum")?;
            let group = target
                .group_id
                .clone()
                .ok_or("Source identity unavailable")?;
            Ok((
                m,
                self.measurement_input(&group, target.label.clone()),
                target,
            ))
        });
        let (model, input, target) = match result {
            Ok(v) => v,
            Err(e) => {
                self.fluorescence.message = e;
                cx.notify();
                return;
            }
        };
        self.fluorescence.invalidate();
        self.fluorescence.busy = true;
        self.fluorescence.message = "Calculating correction…".into();
        let generation = self.fluorescence.generation;
        let project = self.project_generation;
        cx.spawn(async move |this, cx| {
            let operation = target.operation_input();
            let r = cx
                .background_spawn(async move {
                    let source_digest = input.revision()?.0;
                    let corrected = prepare_input(&input)?
                        .correct_fluorescence(&model)
                        .map_err(|e| e.to_string())?;
                    if input.revision()?.0 != source_digest {
                        return Err("Source changed during preview; try again.".into());
                    }
                    Ok::<_, String>(CorrectionRecord {
                        schema: 1,
                        input: operation,
                        source_digest,
                        settings: input.settings,
                        result: corrected
                            .fluorescence_correction()
                            .ok_or("Correction record missing")?
                            .clone(),
                    })
                })
                .await;
            this.update(cx, |app, cx| {
                if project != app.project_generation || generation != app.fluorescence.generation {
                    return;
                }
                app.fluorescence.busy = false;
                if app.current_tool_target() != Some(target) {
                    app.fluorescence.message = "Source changed; preview again.".into();
                    cx.notify();
                    return;
                }
                match r {
                    Err(e) => app.fluorescence.message = e,
                    Ok(r) => {
                        app.fluorescence.message = format!(
                            "Preview · maximum amplification {:.2}×",
                            r.result.maximum_amplification
                        );
                        app.fluorescence.record = Some(Arc::new(r));
                        app.rebuild_fluorescence_plot(cx);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn apply_fluorescence(&mut self, cx: &mut Context<Self>) {
        if self.fluorescence.busy
            || self.fluorescence.historical
            || !self.fluorescence_matches_current()
        {
            return;
        }
        let Some(record) = self.fluorescence.record.clone() else {
            return;
        };
        let Some(target) = self.fluorescence.target.clone() else {
            return;
        };
        let Some(group) = target.group_id.clone() else {
            return;
        };
        let input = self.measurement_input(&group, target.label.clone());
        let generation = self.fluorescence.generation;
        let project = self.project_generation;
        self.fluorescence.busy = true;
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move {
                    if input.revision()?.0 != record.source_digest {
                        return Err("Source changed; preview again.".into());
                    }
                    let receipt = history::retain(&history::root()?, &record)?;
                    Ok::<_, String>(corrected_group(&record, receipt))
                })
                .await;
            this.update(cx, |app, cx| {
                if project != app.project_generation || generation != app.fluorescence.generation {
                    return;
                }
                app.fluorescence.busy = false;
                if app.current_tool_target() != Some(target) {
                    app.fluorescence.message = "Source changed; preview again.".into();
                    cx.notify();
                    return;
                }
                match r {
                    Err(e) => app.fluorescence.message = e,
                    Ok(mut d) => {
                        d.id = app.next_group_id();
                        d.group_id = Some(crate::group_identity::GroupId::new_result());
                        d.declared_edge = app
                            .current_group_index()
                            .and_then(|i| app.group_declared_edge(i));
                        let index = app.derived.len();
                        app.record(
                            "Add fluorescence-corrected spectrum",
                            Some(super::journal::UndoOp::DerivedAdd {
                                index,
                                spectrum: d.clone(),
                            }),
                        );
                        app.derived.push(d);
                        app.rekey_after_catalog_change();
                        app.fluorescence.close();
                        app.select_entry(DERIVED_BASE + index, cx);
                        app.focus_process_section(super::ProcessSection::Normalize, cx);
                        app.status = "Corrected group added · choose final normalization".into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn rebuild_fluorescence_plot(&mut self, cx: &mut Context<Self>) {
        let Some(record) = &self.fluorescence.record else {
            return;
        };
        let r = &record.result;
        let mut p = Plot::new()
            .theme(self.theme.plot_theme())
            .xlabel("Energy (eV)");
        p = if self.fluorescence.factor {
            p.line(&r.energy, &r.factor)
                .label("Correction factor")
                .into()
        } else {
            p.line(&r.energy, &r.original_mu).label("Original").into()
        };
        if !self.fluorescence.factor {
            p = p.line(&r.energy, &r.corrected_mu).label("Corrected").into();
        }
        self.fluorescence.plot = Some(
            plot_builder(
                p.ylabel(if self.fluorescence.factor {
                    "Amplification"
                } else {
                    "μ(E)"
                })
                .legend_position(LegendPosition::UpperRight),
            )
            .interactive()
            .build(cx),
        );
    }
    fn export_fluorescence(&mut self, cx: &mut Context<Self>) {
        let Some(record) = self.fluorescence.record.clone() else {
            return;
        };
        let request =
            cx.prompt_for_new_path(&std::env::temp_dir(), Some("fluorescence-correction.json"));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = request.await else {
                return;
            };
            let result = cx
                .background_spawn(async move {
                    let parent = path.parent().ok_or("Export folder unavailable")?;
                    let mut f =
                        tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
                    serde_json::to_writer_pretty(&mut f, &*record).map_err(|e| e.to_string())?;
                    f.as_file().sync_all().map_err(|e| e.to_string())?;
                    f.persist(&path).map_err(|e| e.to_string())?;
                    Ok::<_, String>(())
                })
                .await;
            this.update(cx, |a, cx| {
                a.fluorescence.message = match result {
                    Ok(()) => "Correction history exported".into(),
                    Err(e) => e,
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests;
