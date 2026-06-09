//! Root view: workspace shell per doc/gui-ux-design.md.
//!
//! M1 scope: lazy catalog — "Open Folder" starts a background scan that
//! streams batches into a virtualized file list; clicking an entry parses and
//! processes it on the background executor (generation-counted so stale
//! results are dropped) with an LRU cache of processed spectra. The Explore
//! center is the 2x2 quadrant grid from M0.

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use gpui::{
    ClickEvent, Context, Entity, IntoElement, ParentElement, PathPromptOptions, Render,
    SharedString, Styled, Window, div, prelude::*, px, uniform_list,
};
use lru::LruCache;
use ruviz_gpui::{RuvizPlot, plot_builder};
use xraytsubaki::prelude::XASSpectrum;

use crate::catalog::{Catalog, ScanEvent, start_scan};
use crate::params::{PipelineParams, process_file};
use crate::plotting::build_quadrants;
use crate::theme::{Theme, ThemeMode};
use crate::widgets::numeric_field::{FieldEvent, NumericField};

/// Processed spectra kept in RAM. ~100-300 KB each, so 1024 ≈ a few hundred MB
/// worst case; browsing a million-file catalog stays bounded.
const PROCESSED_CACHE_CAPACITY: usize = 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Workspace {
    Explore,
    Operando,
    Fit,
}

/// Which pipeline parameter a context-panel field edits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ParamKey {
    E0,
    PreEdgeStart,
    PreEdgeEnd,
    NormStart,
    NormEnd,
    Rbkg,
    BkgKmin,
    BkgKmax,
    FftKmin,
    FftKmax,
    FftDk,
    FftKweight,
}

/// Cache slot for the spectrum loaded outside the catalog (default file).
const NO_ENTRY: usize = usize::MAX;

pub struct StudioApp {
    theme: Theme,
    workspace: Workspace,
    catalog: Catalog,
    selected: Option<usize>,
    /// Bumped on every selection; async load results from older generations
    /// are discarded.
    generation: u64,
    /// Bumped on every parameter edit; the debounced recompute only fires for
    /// the latest epoch.
    recompute_epoch: u64,
    params: PipelineParams,
    param_fields: Vec<(ParamKey, Entity<NumericField>)>,
    /// Keyed by (catalog index, params fingerprint).
    cache: LruCache<(usize, u64), Arc<XASSpectrum>>,
    current_path: PathBuf,
    spectrum: Option<Arc<XASSpectrum>>,
    spectrum_label: SharedString,
    quadrants: Vec<(SharedString, Entity<RuvizPlot>)>,
    maximized: Option<usize>,
    status: SharedString,
}

fn default_data_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../xraytsubaki/tests/testfiles/Ru_QAS.dat")
}

fn spectrum_status(label: &SharedString, sp: &XASSpectrum) -> SharedString {
    format!(
        "{} · {} points · E0 {:.1} eV",
        label,
        sp.energy.as_ref().map(|e| e.len()).unwrap_or(0),
        sp.get_e0().unwrap_or(f64::NAN),
    )
    .into()
}

impl StudioApp {
    pub fn new_with_open(
        initial_open: Option<PathBuf>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial_dir = initial_open.as_ref().filter(|p| p.is_dir()).cloned();
        let path = match initial_dir {
            Some(_) => default_data_file(),
            None => initial_open.unwrap_or_else(default_data_file),
        };
        let label: SharedString = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string())
            .into();

        let theme = Theme::dark();
        let params = PipelineParams::default();
        let param_fields = Self::build_param_fields(theme, cx);

        let mut app = Self {
            theme,
            workspace: Workspace::Explore,
            catalog: Catalog::default(),
            selected: None,
            generation: 0,
            recompute_epoch: 0,
            params,
            param_fields,
            cache: LruCache::new(NonZeroUsize::new(PROCESSED_CACHE_CAPACITY).unwrap()),
            current_path: path.clone(),
            spectrum: None,
            spectrum_label: label.clone(),
            quadrants: Vec::new(),
            maximized: None,
            status: "loading...".into(),
        };

        match process_file(&path, &app.params) {
            Ok(sp) => {
                app.set_processed(label, Arc::new(sp), cx);
            }
            Err(e) => {
                app.status = format!("failed to load {}: {e}", path.display()).into();
            }
        }
        if let Some(dir) = initial_dir {
            app.scan_folder(dir, cx);
        }
        app
    }

    /// Context-panel fields, in pipeline order. Placeholders show the value
    /// used when the field is on "auto".
    fn build_param_fields(
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Vec<(ParamKey, Entity<NumericField>)> {
        let specs: [(ParamKey, &str, &str); 12] = [
            (ParamKey::E0, "E0 (eV)", "auto"),
            (ParamKey::PreEdgeStart, "pre-edge start", "auto (-200)"),
            (ParamKey::PreEdgeEnd, "pre-edge end", "auto (-30)"),
            (ParamKey::NormStart, "norm start", "auto (150)"),
            (ParamKey::NormEnd, "norm end", "auto (2000)"),
            (ParamKey::Rbkg, "rbkg (Å)", "auto (1.0)"),
            (ParamKey::BkgKmin, "k min", "auto (0)"),
            (ParamKey::BkgKmax, "k max", "auto (full)"),
            (ParamKey::FftKmin, "k min", "auto (0)"),
            (ParamKey::FftKmax, "k max", "auto (20)"),
            (ParamKey::FftDk, "dk", "auto (1)"),
            (ParamKey::FftKweight, "k-weight", "auto (2)"),
        ];
        specs
            .into_iter()
            .map(|(key, label, placeholder)| {
                let field = cx.new(|cx| NumericField::new(label, placeholder, None, theme, cx));
                cx.subscribe(&field, move |this: &mut Self, _field, event, cx| {
                    let FieldEvent::Changed(value) = event;
                    this.apply_param(key, *value, cx);
                })
                .detach();
                (key, field)
            })
            .collect()
    }

    fn apply_param(&mut self, key: ParamKey, value: Option<f64>, cx: &mut Context<Self>) {
        let p = &mut self.params;
        match key {
            ParamKey::E0 => p.e0 = value,
            ParamKey::PreEdgeStart => p.pre_edge_start = value,
            ParamKey::PreEdgeEnd => p.pre_edge_end = value,
            ParamKey::NormStart => p.norm_start = value,
            ParamKey::NormEnd => p.norm_end = value,
            ParamKey::Rbkg => p.rbkg = value,
            ParamKey::BkgKmin => p.bkg_kmin = value,
            ParamKey::BkgKmax => p.bkg_kmax = value,
            ParamKey::FftKmin => p.fft_kmin = value,
            ParamKey::FftKmax => p.fft_kmax = value,
            ParamKey::FftDk => p.fft_dk = value,
            ParamKey::FftKweight => p.fft_kweight = value,
        }
        self.schedule_recompute(cx);
    }

    /// Debounced (~200 ms) recompute of the current spectrum after parameter
    /// edits; only the latest epoch fires.
    fn schedule_recompute(&mut self, cx: &mut Context<Self>) {
        self.recompute_epoch += 1;
        let epoch = self.recompute_epoch;
        let timer = cx.background_executor().timer(Duration::from_millis(200));
        cx.spawn(async move |this, cx| {
            timer.await;
            this.update(cx, |app, cx| {
                if app.recompute_epoch == epoch {
                    app.reprocess_current(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn reprocess_current(&mut self, cx: &mut Context<Self>) {
        let ix = self.selected.unwrap_or(NO_ENTRY);
        let label = self.spectrum_label.clone();
        let path = self.current_path.clone();
        self.load_spectrum(ix, path, label, cx);
    }

    /// Common load path: serve from the (entry, params) cache or process on
    /// the background executor; stale generations are dropped.
    fn load_spectrum(
        &mut self,
        ix: usize,
        path: PathBuf,
        label: SharedString,
        cx: &mut Context<Self>,
    ) {
        self.generation += 1;
        let generation = self.generation;
        let key = (ix, self.params.fingerprint());

        if let Some(sp) = self.cache.get(&key) {
            let sp = sp.clone();
            self.set_processed(label, sp, cx);
            cx.notify();
            return;
        }

        self.status = format!("processing {label} ...").into();
        cx.notify();
        let params = self.params;
        let load = cx
            .background_executor()
            .spawn(async move { process_file(&path, &params) });
        cx.spawn(async move |this, cx| {
            let result = load.await;
            this.update(cx, |app, cx| {
                if app.generation != generation {
                    return; // a newer selection/edit superseded this load
                }
                match result {
                    Ok(sp) => {
                        let sp = Arc::new(sp);
                        app.cache.put(key, sp.clone());
                        app.set_processed(label, sp, cx);
                    }
                    Err(e) => {
                        app.status = format!("failed to process {label}: {e}").into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_processed(&mut self, label: SharedString, sp: Arc<XASSpectrum>, cx: &mut Context<Self>) {
        self.status = spectrum_status(&label, &sp);
        self.spectrum_label = label;
        // Surface the auto-determined E0 in the field placeholder.
        if self.params.e0.is_none()
            && let Some(e0) = sp.get_e0()
            && let Some((_, field)) = self
                .param_fields
                .iter()
                .find(|(key, _)| *key == ParamKey::E0)
        {
            field.update(cx, |f, cx| {
                f.set_placeholder(format!("auto ({e0:.1})"), cx)
            });
        }
        self.spectrum = Some(sp);
        self.rebuild_plots(cx);
    }

    fn rebuild_plots(&mut self, cx: &mut Context<Self>) {
        let Some(sp) = &self.spectrum else {
            return;
        };
        let plots = build_quadrants(sp, &self.theme);
        let titled = [
            ("mu(E)", plots.mu_e),
            ("normalized", plots.norm),
            ("chi(k)", plots.chi_k),
            ("|chi(R)|", plots.chi_r),
        ];
        if self.quadrants.is_empty() {
            self.quadrants = titled
                .into_iter()
                .map(|(title, plot)| {
                    let entity = plot_builder(plot).interactive().build(cx);
                    (SharedString::from(title), entity)
                })
                .collect();
        } else {
            for ((_, entity), (_, plot)) in self.quadrants.iter().zip(titled.into_iter()) {
                entity.update(cx, |rp, cx| rp.set_plot(plot, cx));
            }
        }
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = self.theme.toggled();
        let theme = self.theme;
        for (_, field) in &self.param_fields {
            field.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        self.rebuild_plots(cx);
        cx.notify();
    }

    // ---- catalog -----------------------------------------------------------

    fn open_folder(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                for root in paths {
                    this.update(cx, |app, cx| app.scan_folder(root, cx)).ok();
                }
            }
        })
        .detach();
    }

    fn scan_folder(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.catalog.scanning = true;
        self.status = format!("scanning {} ...", root.display()).into();
        let mut rx = start_scan(root);
        cx.spawn(async move |this, cx| {
            while let Some(event) = rx.next().await {
                let done = matches!(event, ScanEvent::Done { .. } | ScanEvent::Error(_));
                let update = this.update(cx, |app, cx| {
                    match event {
                        ScanEvent::Batch(batch) => {
                            let first = app.catalog.is_empty();
                            app.catalog.extend(batch);
                            // Show something as soon as the index has anything.
                            if first && app.selected.is_none() {
                                app.select_entry(0, cx);
                            }
                        }
                        ScanEvent::Done { total } => {
                            app.catalog.scanning = false;
                            app.status = format!("indexed {total} files").into();
                        }
                        ScanEvent::Error(e) => {
                            app.catalog.scanning = false;
                            app.status = format!("scan failed: {e}").into();
                        }
                    }
                    cx.notify();
                });
                if done || update.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn select_entry(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix >= self.catalog.len() {
            return;
        }
        self.selected = Some(ix);
        let label: SharedString = self.catalog.name(ix).to_string().into();
        self.spectrum_label = label.clone();
        let path = self.catalog.path(ix);
        self.current_path = path.clone();
        self.load_spectrum(ix, path, label, cx);
    }

    // ---- views -------------------------------------------------------------

    fn rail_button(
        &self,
        id: &'static str,
        label: &'static str,
        ws: Workspace,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let active = self.workspace == ws;
        div()
            .id(id)
            .w(px(40.))
            .h(px(40.))
            .my_1()
            .rounded_md()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .when(active, |d| d.bg(t.raised).text_color(t.accent))
            .when(!active, |d| d.text_color(t.text_muted))
            .hover(|d| d.bg(t.raised))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                this.workspace = ws;
                cx.notify();
            }))
            .child(label)
    }

    fn icon_rail(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        div()
            .w(px(48.))
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .pt_2()
            .bg(t.surface)
            .border_r_1()
            .border_color(t.border)
            .child(self.rail_button("ws-explore", "Ex", Workspace::Explore, cx))
            .child(self.rail_button("ws-operando", "Op", Workspace::Operando, cx))
            .child(self.rail_button("ws-fit", "Ft", Workspace::Fit, cx))
    }

    fn file_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let entity = cx.entity();
        let selected = self.selected;
        uniform_list(
            "catalog-files",
            self.catalog.len(),
            move |range, _window, app| {
                let mut rows = Vec::with_capacity(range.len());
                for ix in range {
                    let name: SharedString = entity.read(app).catalog.name(ix).to_string().into();
                    let is_selected = selected == Some(ix);
                    let entity = entity.clone();
                    rows.push(
                        div()
                            .id(ix)
                            .h(px(24.))
                            .px_3()
                            .flex()
                            .items_center()
                            .text_sm()
                            .overflow_hidden()
                            .when(is_selected, |d| d.bg(t.raised).text_color(t.accent))
                            .when(!is_selected, |d| d.text_color(t.text))
                            .hover(|d| d.bg(t.raised))
                            .cursor_pointer()
                            .on_click(move |_: &ClickEvent, _window, app| {
                                entity.update(app, |this, cx| this.select_entry(ix, cx));
                            })
                            .child(name),
                    );
                }
                rows
            },
        )
        .flex_1()
    }

    fn data_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let footer: SharedString = if self.catalog.is_empty() && !self.catalog.scanning {
            "no folder open".into()
        } else {
            format!(
                "{} files{}",
                self.catalog.len(),
                if self.catalog.scanning {
                    " · scanning..."
                } else {
                    ""
                }
            )
            .into()
        };
        div()
            .w(px(220.))
            .h_full()
            .flex()
            .flex_col()
            .bg(t.surface)
            .border_r_1()
            .border_color(t.border)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_xs().text_color(t.text_muted).child("DATA"))
                    .child(
                        div()
                            .id("open-folder")
                            .px_2()
                            .rounded_sm()
                            .text_xs()
                            .text_color(t.accent)
                            .cursor_pointer()
                            .hover(|d| d.bg(t.raised))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.open_folder(cx);
                            }))
                            .child("Open Folder..."),
                    ),
            )
            .child(if self.catalog.is_empty() {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .text_sm()
                            .text_color(t.accent)
                            .bg(t.raised)
                            .child(self.spectrum_label.clone()),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(self.file_list(cx))
                    .into_any_element()
            })
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(if self.catalog.scanning {
                        t.warn
                    } else {
                        t.text_muted
                    })
                    .border_t_1()
                    .border_color(t.border)
                    .child(footer),
            )
    }

    fn quadrant(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let (title, plot) = &self.quadrants[index];
        let maximized = self.maximized == Some(index);
        div()
            .flex_1()
            .m_1()
            .flex()
            .flex_col()
            .rounded_md()
            .bg(t.raised)
            .border_1()
            .border_color(t.border)
            .child(
                div()
                    .id(SharedString::from(format!("quad-{index}")))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(if maximized { t.accent } else { t.text_muted })
                    .cursor_pointer()
                    .hover(|d| d.text_color(t.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        this.maximized = if this.maximized == Some(index) {
                            None
                        } else {
                            Some(index)
                        };
                        cx.notify();
                    }))
                    .child(title.clone()),
            )
            .child(div().flex_1().p_1().child(plot.clone()))
    }

    fn explore_center(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        if self.quadrants.len() != 4 {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(t.text_muted)
                .child(self.status.clone());
        }
        if let Some(index) = self.maximized {
            return div().flex_1().flex().child(self.quadrant(index, cx));
        }
        div()
            .flex_1()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .child(self.quadrant(0, cx))
                    .child(self.quadrant(1, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .child(self.quadrant(2, cx))
                    .child(self.quadrant(3, cx)),
            )
    }

    fn placeholder_center(&self, label: &'static str) -> impl IntoElement + use<> {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_color(self.theme.text_muted)
            .child(label)
    }

    fn section_header(&self, label: &'static str) -> impl IntoElement + use<> {
        div()
            .px_3()
            .pt_3()
            .pb_1()
            .text_xs()
            .text_color(self.theme.accent)
            .child(label)
    }

    fn context_panel(&self) -> impl IntoElement + use<> {
        let t = self.theme;
        let field = |key: ParamKey| {
            self.param_fields
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, f)| f.clone())
        };
        let mut sections = div()
            .id("params-scroll")
            .flex_1()
            .flex()
            .flex_col()
            .overflow_y_scroll();
        sections = sections.child(self.section_header("Normalization"));
        for key in [
            ParamKey::E0,
            ParamKey::PreEdgeStart,
            ParamKey::PreEdgeEnd,
            ParamKey::NormStart,
            ParamKey::NormEnd,
        ] {
            if let Some(f) = field(key) {
                sections = sections.child(f);
            }
        }
        sections = sections.child(self.section_header("Background (AUTOBK)"));
        for key in [ParamKey::Rbkg, ParamKey::BkgKmin, ParamKey::BkgKmax] {
            if let Some(f) = field(key) {
                sections = sections.child(f);
            }
        }
        sections = sections.child(self.section_header("FFT"));
        for key in [
            ParamKey::FftKmin,
            ParamKey::FftKmax,
            ParamKey::FftDk,
            ParamKey::FftKweight,
        ] {
            if let Some(f) = field(key) {
                sections = sections.child(f);
            }
        }
        sections = sections.child(
            div()
                .px_3()
                .py_2()
                .text_xs()
                .text_color(t.text_muted)
                .child("Enter commits · empty = auto"),
        );

        div()
            .w(px(260.))
            .h_full()
            .flex()
            .flex_col()
            .bg(t.surface)
            .border_l_1()
            .border_color(t.border)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child("PARAMETERS"),
            )
            .child(sections)
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let theme_label = match self.theme.mode {
            ThemeMode::Dark => "dark",
            ThemeMode::Light => "light",
        };
        div()
            .h(px(28.))
            .w_full()
            .flex()
            .items_center()
            .px_3()
            .gap_3()
            .bg(t.surface)
            .border_t_1()
            .border_color(t.border)
            .text_xs()
            .text_color(t.text_muted)
            .child(div().flex_1().child(self.status.clone()))
            .child(
                div()
                    .id("theme-toggle")
                    .px_2()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|d| d.bg(t.raised).text_color(t.text))
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.toggle_theme(cx);
                    }))
                    .child(format!("theme: {theme_label}")),
            )
    }
}

impl Render for StudioApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let center = match self.workspace {
            Workspace::Explore => self.explore_center(cx).into_any_element(),
            Workspace::Operando => self
                .placeholder_center("Operando workspace — heatmap + time scrubbing land in M3")
                .into_any_element(),
            Workspace::Fit => self
                .placeholder_center("Fit workspace — FEFF model + batch fitting land in M4/M5")
                .into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.bg)
            .text_color(t.text)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .child(self.icon_rail(cx))
                    .child(self.data_panel(cx))
                    .child(div().flex_1().flex().flex_col().child(center))
                    .child(self.context_panel()),
            )
            .child(self.status_bar(cx))
    }
}
