//! Wavelet workspace: immutable scientific maps, bounded current residency and
//! independent display controls. Original groups and pipeline settings stay intact.
use super::*;
use crate::wavelet_history::WaveletSavedRegion;
use crate::{
    group_identity::GroupId,
    params::{PipelineParams, RequiredStage},
    wavelet_history::{self as history, WaveletArchive, WaveletReceipt, WaveletRecord},
};
use gpui::{AppContext, Entity};
use rexafs::Wavelet;
use ruviz_gpui::{PlotPointerEvent, PlotPointerEventKind, RuvizPlot};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
mod controls;
mod layout;
pub(crate) mod plots;
pub(crate) const PLOT_WAVELET_K: usize = 500;
pub(crate) const PLOT_WAVELET_R: usize = 501;

#[derive(Default)]
pub(crate) struct WaveletState {
    pub open: bool,
    pub archive: WaveletArchive,
    group: Option<GroupId>,
    source_settings: Option<PipelineParams>,
    fields: Vec<Entity<crate::widgets::numeric_field::NumericField>>,
    pub(super) record: Option<Arc<WaveletRecord>>,
    receipt: Option<WaveletReceipt>,
    pub plot_k: Option<Entity<RuvizPlot>>,
    pub plot_r: Option<Entity<RuvizPlot>>,
    pub(super) plot_map: Option<Entity<RuvizPlot>>,
    subscription: Option<gpui::Subscription>,
    view_links: Vec<ruviz::core::InteractiveChangeSubscription>,
    layout_size: Option<(u32, u32)>,
    stop: Option<Arc<AtomicBool>>,
    generation: u64,
    busy: bool,
    message: String,
    cursor: [f64; 2],
    slices: bool,
    view: usize,
    palette: Option<crate::app::series_display::HeatmapPalette>,
    reversed: bool,
    colors_open: bool,
    dirty: bool,
    automatic_receipts: std::collections::HashMap<GroupId, String>,
    menu_position: gpui::Point<gpui::Pixels>,
    menu_focus: Option<gpui::FocusHandle>,
    lock_scale: Option<(f64, f64)>,
    lock_basis: Option<(PipelineParams, Wavelet)>,
    advanced: bool,
}
impl WaveletState {
    /// Hide this view without discarding its map, region or running calculation.
    pub fn hide(&mut self) {
        self.open = false;
        self.colors_open = false;
    }

    pub fn cancel(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        self.generation += 1;
        self.busy = false;
    }
}
fn prepare_wavelet_input(
    input: &crate::series_measurements::FrameInput,
    settings: &PipelineParams,
) -> Result<rexafs::Spectrum, String> {
    match &input.derived {
        Some(d) if d.quantity == crate::params::Quantity::ChiK && !d.quantity_unconfirmed => {
            d.for_display(settings)
        }
        Some(d) => d.prepare(settings, RequiredStage::Background),
        None => {
            let (e, m) = crate::params::load_raw(&input.path, settings)?;
            crate::params::prepare_arrays(e, m, settings, RequiredStage::Background)
        }
    }
}
impl StudioApp {
    pub(crate) fn wavelet_matches_current(&self) -> bool {
        self.wavelet.group == self.current_group_index().and_then(|i| self.group_id(i))
            && self.wavelet.source_settings.as_ref() == Some(self.ui_params())
    }
    /// Redraw the cached map, marginals and slices with the current theme.
    pub(crate) fn restyle_wavelet(&mut self, cx: &mut Context<Self>) {
        if self.wavelet.record.is_some() {
            // Also rebuilds the slices.
            self.rebuild_wavelet_plots(cx);
        }
    }
    pub(crate) fn open_wavelet(&mut self, cx: &mut Context<Self>) {
        self.ui.sections.insert("Wavelet settings");
        self.set_stage(super::Stage::Transform, cx);
        self.fluorescence.close();
        self.peaks.open = false;
        if self.wavelet_matches_current() && self.wavelet.fields.len() == 8 {
            self.wavelet.open = true;
            if self.wavelet.dirty || self.wavelet.record.is_none() {
                self.schedule_wavelet(cx);
            }
            cx.notify();
            return;
        }
        let previous_definition = self.wavelet_definition(cx).ok();
        self.wavelet.cancel();
        self.wavelet.open = true;
        self.wavelet.colors_open = false;
        self.wavelet.group = self.current_group_index().and_then(|i| self.group_id(i));
        self.wavelet.source_settings = Some(self.ui_params().clone());
        self.wavelet.record = None;
        self.wavelet.receipt = None;
        self.wavelet.view_links.clear();
        self.wavelet.plot_map = None;
        self.wavelet.plot_k = None;
        self.wavelet.plot_r = None;
        self.wavelet.message = "Updating wavelet…".into();
        let recent = self
            .wavelet
            .archive
            .entries
            .iter()
            .rev()
            .find(|r| {
                Some(&r.group) == self.wavelet.group.as_ref() && r.settings == *self.ui_params()
            })
            .cloned();
        let available = self
            .spectrum
            .as_ref()
            .and_then(|s| s.k())
            .and_then(|k| k.last())
            .copied()
            .unwrap_or(12.);
        let definition = previous_definition
            .or_else(|| recent.as_ref().map(|r| r.definition.clone()))
            .unwrap_or_else(|| Wavelet::new(2. ..=(available.min(12.) * 100.).floor() / 100.));
        self.set_wavelet_fields(&definition, cx);
        if let Some(receipt) = recent.filter(|r| r.definition == definition) {
            self.wavelet.dirty = false;
            self.load_wavelet(receipt, cx);
        } else {
            self.schedule_wavelet(cx);
        }
        cx.notify();
    }
    pub(crate) fn ensure_wavelet_fields(&mut self, cx: &mut Context<Self>) {
        if self.wavelet.fields.is_empty() {
            self.set_wavelet_fields(&Wavelet::new(2. ..=12.), cx);
        }
    }
    pub(crate) fn set_wavelet_fields(&mut self, definition: &Wavelet, cx: &mut Context<Self>) {
        self.wavelet.fields.clear();
        let taper = match definition.window {
            rexafs::WaveletWindow::None => 0.,
            rexafs::WaveletWindow::Cosine { width } => width,
        };
        use crate::widgets::numeric_field::{FieldEvent, FieldKind, NumericField};
        let values = [
            Some(definition.k_range[0]),
            Some(definition.k_range[1]),
            Some(definition.kweight as f64),
            Some(definition.rmax),
            Some(definition.order as f64),
            Some(definition.kstep),
            definition.rstep,
            Some(taper),
        ];
        for (i, (label, value)) in [
            "Wavelet k min (Å⁻¹)",
            "Wavelet k max (Å⁻¹)",
            "k-weight",
            "R max (Å)",
            "Cauchy order",
            "k step (Å⁻¹)",
            "R step (Å)",
            "Taper (Å⁻¹)",
        ]
        .into_iter()
        .zip(values)
        .enumerate()
        {
            let kind = if i == 2 || i == 4 {
                FieldKind::Integer {
                    min: Some(if i == 4 { 1 } else { 0 }),
                }
            } else {
                FieldKind::Float
            };
            let description = match i {
                4 => Some(
                    "Order of the Cauchy wavelet. A larger order narrows the frequency response and broadens the localization in k.",
                ),
                6 => Some("Spacing of the R rows. Blank chooses the step automatically."),
                7 => Some("Width of the cosine taper at both k limits. 0 applies no taper."),
                _ => None,
            };
            let field = cx.new(|cx| {
                let field = NumericField::new(
                    label,
                    if i == 6 { "auto" } else { "required" },
                    value,
                    kind,
                    self.theme,
                    cx,
                )
                .with_step(match i {
                    0 | 1 | 3 | 7 => 0.1,
                    5 | 6 => 0.01,
                    _ => 1.,
                });
                match description {
                    Some(text) => field.with_description(text),
                    None => field,
                }
            });
            cx.subscribe(&field, |app, _, event, cx| {
                if matches!(event, FieldEvent::Changed(_)) {
                    app.schedule_wavelet(cx);
                }
            })
            .detach();
            self.wavelet.fields.push(field);
        }
    }
    pub(crate) fn wavelet_definition(&self, cx: &Context<Self>) -> Result<Wavelet, String> {
        if self.wavelet.fields.len() != 8 {
            return Err("Set the wavelet parameters in Transform first".into());
        }
        let number = |i: usize| {
            self.wavelet.fields[i]
                .read(cx)
                .value()
                .ok_or_else(|| "Enter wavelet settings".to_owned())
        };
        let weight = number(2)?;
        let order = number(4)?;
        if weight.fract() != 0.
            || !(0. ..=6.).contains(&weight)
            || order.fract() != 0.
            || !(1. ..=4096.).contains(&order)
        {
            return Err("Weight must be 0–6; order must be 1–4096 (whole numbers)".into());
        }
        let mut definition = Wavelet::new(number(0)?..=number(1)?)
            .kweight(weight as u8)
            .rmax(number(3)?)
            .order(order as usize)
            .kstep(number(5)?);
        if let Some(step) = self.wavelet.fields[6].read(cx).value() {
            definition = definition.rstep(step);
        }
        let taper = number(7)?;
        if taper < 0. {
            return Err("Taper must be zero or positive".into());
        }
        if taper > 0. {
            definition = definition.taper(taper);
        }
        definition
            .estimate(&definition.k_range)
            .map_err(|e| e.to_string())?;
        Ok(definition)
    }
    /// Coalesce committed edits and steppers; stale jobs cannot replace newer maps.
    fn schedule_wavelet(&mut self, cx: &mut Context<Self>) {
        self.wavelet.cancel();
        self.wavelet.dirty = true;
        if let Err(error) = self.wavelet_definition(cx) {
            self.wavelet.message = format!("{error} · previous map unchanged");
            cx.notify();
            return;
        }
        self.wavelet.message = "Updating wavelet…".into();
        if self.wavelet.open {
            let generation = self.wavelet.generation;
            let project = self.project_generation;
            let timer = cx
                .background_executor()
                .timer(std::time::Duration::from_millis(200));
            cx.spawn(async move |this, cx| {
                timer.await;
                this.update(cx, |app, cx| {
                    if app.project_generation == project
                        && app.wavelet.generation == generation
                        && app.wavelet.open
                        && app.stage == Stage::Transform
                    {
                        app.calculate_wavelet(cx);
                    }
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }
    fn calculate_wavelet(&mut self, cx: &mut Context<Self>) {
        if self.wavelet.busy {
            return;
        }
        let definition = match self.wavelet_definition(cx) {
            Ok(d) => d,
            Err(e) => {
                self.wavelet.message = e;
                cx.notify();
                return;
            }
        };
        self.wavelet.group = self.current_group_index().and_then(|i| self.group_id(i));
        self.wavelet.source_settings = Some(self.ui_params().clone());
        let Some(ix) = self.current_group_index() else {
            return;
        };
        let Some(group) = self.group_id(ix) else {
            return;
        };
        let input = self.measurement_input(&group, self.entry_label(ix));
        let settings = self.ui_params().clone();
        let before = settings.clone();
        let project = self.project_generation;
        self.wavelet.cancel();
        let generation = self.wavelet.generation;
        let stop = Arc::new(AtomicBool::new(false));
        self.wavelet.stop = Some(stop.clone());
        self.wavelet.busy = true;
        self.wavelet.message = "Calculating full map…".into();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut sp = prepare_wavelet_input(&input,&settings)?;
                    let map = sp
                        .wavelet_with_cancel(&definition, || stop.load(Ordering::Relaxed))
                        .map_err(|e| e.to_string())?;
                    if map.r().len()<2 {return Err("The desktop map needs at least two R rows; increase R maximum or reduce R step".into());}
                    let mut fft_settings = settings.clone();
                    // The map's k range may be limited to the measured data.
                    let [kmin, kmax] = map.settings().k_range;
                    fft_settings.fft_kmin = Some(kmin);
                    fft_settings.fft_kmax = Some(kmax);
                    fft_settings.fft_kweight = Some(definition.kweight as f64);
                    let fourier_error = match crate::params::forward_settings(&fft_settings) {
                        Ok(f) => sp.set_fft(f).fft().err().map(|e| e.to_string()),
                        Err(e) => Some(e),
                    };
                    let fourier = if fourier_error.is_none() {
                        sp.xftf
                    } else {
                        None
                    };
                    let record = WaveletRecord {
                        schema: 1,
                        group: input.group.clone(),
                        label: input.label,
                        settings,
                        map,
                        fourier,
                        fourier_error,
                    };
                    if stop.load(Ordering::Relaxed) {
                        return Err("Wavelet calculation cancelled".into());
                    }
                    let receipt = history::retain(&history::root()?, &record)?;
                    Ok::<_, String>((record, receipt))
                })
                .await;
            this.update(cx, |app, cx| {
                if app.project_generation != project || app.wavelet.generation != generation {
                    return;
                }
                app.wavelet.busy = false;
                app.wavelet.stop = None;
                if app
                    .current_group_index()
                    .and_then(|i| app.group_id(i))
                    .as_ref()
                    != Some(&group)
                    || app.ui_params() != &before
                {
                    app.schedule_wavelet(cx);
                    cx.notify();
                    return;
                }
                match result {
                    Err(e) => app.wavelet.message = format!("{e} · previous map unchanged"),
                    Ok((record, receipt)) => {
                        app.wavelet.archive.insert_preview(receipt.clone(), &mut app.wavelet.automatic_receipts);
                        app.wavelet.dirty = false;
                        app.show_wavelet(record, receipt, cx);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn load_wavelet(&mut self, receipt: WaveletReceipt, cx: &mut Context<Self>) {
        let project = self.project_generation;
        let generation = self.wavelet.generation;
        self.wavelet.busy = true;
        cx.spawn(async move |this, cx| {
            let copy = receipt.clone();
            let result = cx
                .background_spawn(async move { history::read(&copy) })
                .await;
            this.update(cx, |app, cx| {
                if app.project_generation != project || app.wavelet.generation != generation {
                    return;
                }
                app.wavelet.busy = false;
                match result {
                    Ok(record) => app.show_wavelet(record, receipt, cx),
                    Err(e) => app.wavelet.message = e,
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    fn show_wavelet(
        &mut self,
        record: WaveletRecord,
        receipt: WaveletReceipt,
        cx: &mut Context<Self>,
    ) {
        if record.map.r().len() < 2 {
            self.wavelet.message =
                "The desktop map needs at least two R rows; increase R maximum or reduce R step"
                    .into();
            return;
        }
        if self
            .wavelet
            .lock_basis
            .as_ref()
            .is_some_and(|(settings, definition)| {
                settings != &record.settings || definition != record.map.settings()
            })
        {
            self.wavelet.lock_scale = None;
            self.wavelet.lock_basis = None;
        }
        let map = &record.map;
        let k = map.settings().k_range;
        // Amber note on k min/max when the map used a data-limited k range.
        for (i, used) in k.into_iter().enumerate() {
            if let Some(field) = self.wavelet.fields.get(i) {
                let requested = field.read(cx).value();
                let note = requested
                    .is_some_and(|v| (v - used).abs() > 1e-9)
                    .then(|| format!("Used {used:.2} Å⁻¹: limited by the measured data").into());
                field.update(cx, |f, cx| f.set_adjusted(note, cx));
            }
        }
        let r = [map.r()[0], *map.r().last().unwrap()];
        self.wavelet.cursor = [0.5 * (k[0] + k[1]), 0.5 * (r[0] + r[1])];
        self.wavelet.message.clear();
        self.wavelet.view_links.clear();
        self.wavelet.plot_map = None;
        self.wavelet.plot_k = None;
        self.wavelet.plot_r = None;
        self.wavelet.subscription = None;
        self.wavelet.record = Some(Arc::new(record));
        self.wavelet.receipt = Some(receipt);
        self.rebuild_wavelet_plots(cx);
    }
    pub(super) fn export_wavelet(&mut self, cx: &mut Context<Self>) {
        let Some(record) = self.wavelet.record.clone() else {
            return;
        };
        let regions = self
            .wavelet
            .archive
            .regions
            .iter()
            .filter(|r| {
                self.wavelet
                    .receipt
                    .as_ref()
                    .is_some_and(|p| p.digest == r.map_digest)
            })
            .cloned()
            .collect::<Vec<_>>();
        let request = cx.prompt_for_new_path(&std::env::temp_dir(), Some("wavelet.json"));
        let project = self.project_generation;
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = request.await else {
                return;
            };
            let result = cx
                .background_spawn(async move {
                    let mut f = tempfile::NamedTempFile::new_in(
                        path.parent().ok_or("Export directory unavailable")?,
                    )
                    .map_err(|e| e.to_string())?;
                    #[derive(serde::Serialize)]
                    struct Export<'a> {
                        record: &'a WaveletRecord,
                        regions: &'a [WaveletSavedRegion],
                    }
                    serde_json::to_writer(
                        &mut f,
                        &Export {
                            record: record.as_ref(),
                            regions: &regions,
                        },
                    )
                    .map_err(|e| e.to_string())?;
                    f.as_file().sync_all().map_err(|e| e.to_string())?;
                    f.persist(&path).map_err(|e| e.to_string())?;
                    Ok::<_, String>(())
                })
                .await;
            this.update(cx, |app, cx| {
                if app.project_generation == project {
                    app.status = match result {
                        Ok(()) => "Exported full map, original χ and provenance".into(),
                        Err(e) => e.into(),
                    };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_wavelet_jobs_cannot_publish_a_previous_generation() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut state = WaveletState {
            busy: true,
            stop: Some(stop.clone()),
            generation: 7,
            ..Default::default()
        };
        let running_generation = state.generation;
        state.cancel();
        assert!(stop.load(Ordering::Relaxed));
        assert!(!state.busy);
        assert!(state.stop.is_none());
        assert_ne!(state.generation, running_generation);
        let pending_generation = state.generation;
        state.cancel();
        assert_ne!(state.generation, pending_generation);
    }

    #[test]
    fn typed_chi_reuses_original_arrays_and_unknown_quantities_still_fail() {
        let k = (0..101).map(|i| i as f64 * 0.1).collect::<Vec<_>>();
        let chi = k.iter().map(|x| (4. * x).sin()).collect::<Vec<_>>();
        let group = crate::params::DerivedSpectrum {
            energy: k.clone(),
            mu: chi.clone(),
            quantity: crate::params::Quantity::ChiK,
            ..Default::default()
        };
        let mut input = crate::series_measurements::FrameInput {
            group: GroupId::new_result(),
            label: "Typed χ".into(),
            path: Default::default(),
            derived: Some(Arc::new(group.clone())),
            settings: Default::default(),
            recipe: None,
        };
        let sp = prepare_wavelet_input(&input, &input.settings).unwrap();
        assert!(sp.normalization.is_none());
        let map = sp.wavelet(&Wavelet::new(1. ..=9.)).unwrap();
        assert_eq!(map.input_k(), k);
        assert_eq!(map.input_chi(), chi);
        let mut unknown = group;
        unknown.quantity_unconfirmed = true;
        input.derived = Some(Arc::new(unknown));
        assert!(prepare_wavelet_input(&input, &input.settings).is_err());
        let zero = Wavelet::new(1. ..=9.)
            .calculate(&k, &vec![0.; k.len()])
            .unwrap();
        assert!(
            plots::texture(&zero, 3)
                .0
                .iter()
                .flatten()
                .all(|v| v.is_nan())
        );
        assert_eq!(plots::scale(&zero, 0), (0., 1.));
    }
}
