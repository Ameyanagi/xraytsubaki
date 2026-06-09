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

use futures::StreamExt;
use gpui::{
    ClickEvent, Context, Entity, IntoElement, ParentElement, PathPromptOptions, Render,
    SharedString, Styled, Window, div, prelude::*, px, uniform_list,
};
use lru::LruCache;
use ruviz_gpui::{RuvizPlot, plot_builder};
use xraytsubaki::prelude::XASSpectrum;
use xraytsubaki::xafs::io;

use crate::catalog::{Catalog, ScanEvent, start_scan};
use crate::plotting::build_quadrants;
use crate::theme::{Theme, ThemeMode};

/// Processed spectra kept in RAM. ~100-300 KB each, so 1024 ≈ a few hundred MB
/// worst case; browsing a million-file catalog stays bounded.
const PROCESSED_CACHE_CAPACITY: usize = 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Workspace {
    Explore,
    Operando,
    Fit,
}

pub struct StudioApp {
    theme: Theme,
    workspace: Workspace,
    catalog: Catalog,
    selected: Option<usize>,
    /// Bumped on every selection; async load results from older generations
    /// are discarded.
    generation: u64,
    cache: LruCache<usize, Arc<XASSpectrum>>,
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

fn load_and_process(path: &PathBuf) -> Result<XASSpectrum, String> {
    let mut sp = io::load_spectrum_QAS_trans(path).map_err(|e| e.to_string())?;
    sp.find_e0().map_err(|e| e.to_string())?;
    sp.normalize().map_err(|e| e.to_string())?;
    sp.calc_background().map_err(|e| e.to_string())?;
    sp.fft().map_err(|e| e.to_string())?;
    Ok(sp)
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

        let mut app = Self {
            theme: Theme::dark(),
            workspace: Workspace::Explore,
            catalog: Catalog::default(),
            selected: None,
            generation: 0,
            cache: LruCache::new(NonZeroUsize::new(PROCESSED_CACHE_CAPACITY).unwrap()),
            spectrum: None,
            spectrum_label: label.clone(),
            quadrants: Vec::new(),
            maximized: None,
            status: "loading...".into(),
        };

        match load_and_process(&path) {
            Ok(sp) => {
                app.status = spectrum_status(&label, &sp);
                app.spectrum = Some(Arc::new(sp));
                app.rebuild_plots(cx);
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
        self.generation += 1;
        let generation = self.generation;
        let label: SharedString = self.catalog.name(ix).to_string().into();
        self.spectrum_label = label.clone();

        if let Some(sp) = self.cache.get(&ix) {
            self.spectrum = Some(sp.clone());
            self.status = spectrum_status(&label, sp);
            self.rebuild_plots(cx);
            cx.notify();
            return;
        }

        self.status = format!("loading {label} ...").into();
        cx.notify();
        let path = self.catalog.path(ix);
        let load = cx
            .background_executor()
            .spawn(async move { load_and_process(&path) });
        cx.spawn(async move |this, cx| {
            let result = load.await;
            this.update(cx, |app, cx| {
                if app.generation != generation {
                    return; // a newer selection superseded this load
                }
                match result {
                    Ok(sp) => {
                        let sp = Arc::new(sp);
                        app.cache.put(ix, sp.clone());
                        app.status = spectrum_status(&label, &sp);
                        app.spectrum = Some(sp);
                        app.rebuild_plots(cx);
                    }
                    Err(e) => {
                        app.status = format!("failed to load {label}: {e}").into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
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

    fn param_row(&self, name: &'static str, value: String) -> impl IntoElement + use<> {
        let t = self.theme;
        div()
            .px_3()
            .py_1()
            .flex()
            .justify_between()
            .text_sm()
            .child(div().text_color(t.text_muted).child(name))
            .child(div().text_color(t.text).child(value))
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
        let e0 = self
            .spectrum
            .as_ref()
            .and_then(|s| s.get_e0())
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "—".into());
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
            .child(self.section_header("Normalization"))
            .child(self.param_row("E0 (eV)", e0))
            .child(self.param_row("pre-edge", "auto".into()))
            .child(self.param_row("norm range", "auto".into()))
            .child(self.section_header("Background (AUTOBK)"))
            .child(self.param_row("rbkg", "1.0".into()))
            .child(self.param_row("k range", "auto".into()))
            .child(self.param_row("solver", "LinearDirect".into()))
            .child(self.section_header("FFT"))
            .child(self.param_row("k-weight", "2".into()))
            .child(self.param_row("window", "KaiserBessel".into()))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child("editing arrives in M2"),
            )
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
