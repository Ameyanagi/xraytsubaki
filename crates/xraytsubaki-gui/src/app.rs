//! Root view: workspace shell per doc/gui-ux-design.md.
//!
//! M1 scope: lazy catalog — "Open Folder" starts a background scan that
//! streams batches into a virtualized file list; clicking an entry parses and
//! processes it on the background executor (generation-counted so stale
//! results are dropped) with an LRU cache of processed spectra. The Explore
//! center is the 2x2 quadrant grid from M0.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use gpui::{
    ClickEvent, Context, Entity, FocusHandle, IntoElement, KeyBinding, ParentElement,
    PathPromptOptions, Render, SharedString, Styled, Window, actions, div, prelude::*, px,
    uniform_list,
};
use lru::LruCache;
use ruviz_gpui::{RuvizPlot, plot_builder};
use xraytsubaki::prelude::XASSpectrum;

use rayon::prelude::*;

use xraytsubaki::prelude::FeffFitResult;

use crate::catalog::{Catalog, ScanEvent, start_scan};
use crate::fitting::{BatchFitRow, FitPathSpec, FitRanges, FitVarSpec, PathMeta, batch_csv, path_meta, result_summary, run_fit};
use crate::params::{AUTOBK_SOLVERS, DerivedSpectrum, DetectionMode, FT_WINDOWS, PipelineParams, average_spectra, load_raw, parse_cols, process_arrays, process_file, read_columns, resample_chik};
use crate::project::{ProjectFile, PROJECT_VERSION};
use crate::plotting::{
    QuadTrace, TraceLayout, ViewOptions, build_fit_k, build_fit_r, build_frame_chik,
    build_heatmap, build_quadrants_multi, build_trend,
};
use crate::fitting::expr_identifiers;
use crate::theme::{Theme, ThemeMode};
use crate::widgets::numeric_field::{FieldEvent, NumericField};
use crate::widgets::text_input::{InputEvent, TextInput};

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
    ImpEnergyCol,
    ImpI0Col,
    ImpItCol,
    ImpIrCol,
    AlignTarget,
    E0,
    PreEdgeStart,
    PreEdgeEnd,
    NormStart,
    NormEnd,
    NormPolyorder,
    NVictoreen,
    Rbkg,
    BkgKmin,
    BkgKmax,
    BkgKstep,
    BkgNknots,
    BkgKweight,
    BkgClampLo,
    BkgClampHi,
    BkgDk,
    BkgNfft,
    FftKmin,
    FftKmax,
    FftDk,
    FftKweight,
    FftDk2,
    FftRmax,
    FftKstep,
    FftNfft,
}

/// Enum-valued parameters edited with cycling chips.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EnumParam {
    ImportMode,
    BkgWindow,
    BkgSolver,
    FftWindow,
}

const DETECTION_MODES: [DetectionMode; 3] = [
    DetectionMode::Transmission,
    DetectionMode::Fluorescence,
    DetectionMode::Reference,
];

actions!(
    studio,
    [
        NavUp,
        NavDown,
        NavExtendUp,
        NavExtendDown,
        ClearCompare,
        FramePrev,
        FrameNext,
        FrameJumpBack,
        FrameJumpFwd,
    ]
);

/// Key bindings for list navigation and operando scrubbing; register at
/// startup alongside the text-input bindings.
pub fn studio_keybindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("up", NavUp, Some("DataPanel")),
        KeyBinding::new("down", NavDown, Some("DataPanel")),
        KeyBinding::new("shift-up", NavExtendUp, Some("DataPanel")),
        KeyBinding::new("shift-down", NavExtendDown, Some("DataPanel")),
        KeyBinding::new("escape", ClearCompare, Some("DataPanel")),
        KeyBinding::new("left", FramePrev, Some("Operando")),
        KeyBinding::new("right", FrameNext, Some("Operando")),
        KeyBinding::new("shift-left", FrameJumpBack, Some("Operando")),
        KeyBinding::new("shift-right", FrameJumpFwd, Some("Operando")),
    ]
}

/// Overlays beyond this many traces are unreadable; larger selections are
/// thinned evenly (the Operando heatmap is the full-set view).
const MAX_OVERLAY: usize = 12;

/// Cache slot for the spectrum loaded outside the catalog (default file).
const NO_ENTRY: usize = usize::MAX;

/// Derived (merged) spectra get virtual indices above this base so the
/// selection/cache/compare machinery treats them like catalog entries.
const DERIVED_BASE: usize = usize::MAX / 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DataTab {
    Files,
    Scans,
}

/// Frames sampled for the operando overview (heatmap stays bounded no matter
/// how large the scan is).
const MAX_FRAMES: usize = 192;
const K_GRID_BINS: usize = 256;
const K_GRID_MAX: f64 = 15.0;

/// Downsampled overview of one scan, valid for one params fingerprint.
struct OperandoData {
    scan: usize,
    fingerprint: u64,
    sample_ixs: Vec<usize>,
    grid: Vec<f64>,
    matrix: Vec<Vec<f64>>,
    e0s: Vec<f64>,
    kweight: f64,
}

struct OperandoPlots {
    heatmap: Entity<RuvizPlot>,
    chik: Entity<RuvizPlot>,
    trend: Entity<RuvizPlot>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RangeKey {
    Kmin,
    Kmax,
    Rmin,
    Rmax,
    Kweight,
}

/// Completed batch fit over the operando frame sample.
struct BatchFitData {
    scan: usize,
    fingerprint: u64,
    rows: Vec<BatchFitRow>,
    varying_names: Vec<String>,
    /// Index into varying_names selected for the operando trend plot.
    trend_param: usize,
}

/// A fit variable row: spec + its editable value/expression field.
struct FitVar {
    spec: FitVarSpec,
    field: Entity<TextInput>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PathParam {
    S02,
    E0,
    Sigma2,
    DeltaR,
}

/// An imported FEFF path with editable parameter-expression cells.
struct FitPathRow {
    spec: FitPathSpec,
    meta: Option<PathMeta>,
    fields: Vec<(PathParam, Entity<TextInput>)>,
    /// Collapsed rows show one metadata line; expanding reveals the
    /// parameter-expression cells.
    expanded: bool,
}

/// Crystal-form fields for Atoms-lite feff.inp generation.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FeffFormKey {
    Element,
    Element2,
    Structure,
    LatticeA,
    LatticeC,
    Edge,
    Rmax,
}

pub struct StudioApp {
    theme: Theme,
    workspace: Workspace,
    catalog: Catalog,
    source_dir: Option<PathBuf>,
    /// Active spectrum (drives params/fit/status).
    selected: Option<usize>,
    /// Compare set; the active spectrum is implicitly included.
    selection: BTreeSet<usize>,
    /// Merged/averaged spectra (virtual indices DERIVED_BASE + i).
    derived: Vec<DerivedSpectrum>,
    compare_gen: u64,
    view: ViewOptions,
    view_offset_field: Option<Entity<NumericField>>,
    filter_input: Option<Entity<TextInput>>,
    filter_text: String,
    data_focus: FocusHandle,
    operando_focus: FocusHandle,
    /// Per-section "advanced parameters" fold state (Norm, Bkg, FFT, Import).
    adv_open: [bool; 4],
    roi_input: Option<Entity<TextInput>>,
    import_preview: SharedString,
    /// Which enum parameter's option list is expanded.
    open_enum: Option<EnumParam>,
    /// Catalog indices passing the filter (ascending); None = no filter.
    filtered: Option<Arc<Vec<usize>>>,
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
    data_tab: DataTab,
    active_scan: Option<usize>,
    operando: Option<OperandoData>,
    operando_plots: Option<OperandoPlots>,
    operando_gen: u64,
    time_pos: usize,
    fit_paths: Vec<FitPathRow>,
    fit_vars: Vec<FitVar>,
    fit_range_fields: Vec<(RangeKey, Entity<NumericField>)>,
    fit_ranges: FitRanges,
    fit_result: Option<Arc<FeffFitResult>>,
    fit_plots: Option<(Entity<RuvizPlot>, Entity<RuvizPlot>)>,
    fit_gen: u64,
    fit_running: bool,
    feff_workspace: Option<PathBuf>,
    feff_running: bool,
    feff_gen: u64,
    feff_form: Vec<(FeffFormKey, Entity<TextInput>)>,
    /// Batch-fit rows for (scan, params fingerprint) + progress counter.
    batch_fit: Option<BatchFitData>,
    batch_running: bool,
    batch_progress: (usize, usize),
    batch_gen: u64,
    status: SharedString,
}

/// Evenly sample `all` (sorted) down to `cap`, always keeping first, last,
/// and `keep` when present.
fn thin_even(all: &[usize], cap: usize, keep: Option<usize>) -> Vec<usize> {
    if all.len() <= cap {
        return all.to_vec();
    }
    let total = all.len();
    let mut thinned: Vec<usize> = (0..cap).map(|i| all[i * (total - 1) / (cap - 1)]).collect();
    if let Some(keep) = keep
        && all.contains(&keep)
        && !thinned.contains(&keep)
    {
        thinned[cap / 2] = keep;
        thinned.sort_unstable();
    }
    thinned.dedup();
    thinned
}

#[cfg(test)]
mod thin_tests {
    use super::thin_even;

    #[test]
    fn small_sets_pass_through() {
        assert_eq!(thin_even(&[1, 5, 9], 12, Some(5)), vec![1, 5, 9]);
    }

    #[test]
    fn large_sets_sample_evenly_and_keep_ends_and_active() {
        let all: Vec<usize> = (0..200).collect();
        let out = thin_even(&all, 12, Some(7));
        assert_eq!(out.len(), 12);
        assert_eq!(*out.first().unwrap(), 0);
        assert_eq!(*out.last().unwrap(), 199);
        assert!(out.contains(&7));
        assert!(out.windows(2).all(|w| w[0] < w[1]));
    }
}

/// Case-insensitive name filter with `*` wildcards; without `*` it is a
/// substring match. Segments must appear in order; ends anchor unless the
/// pattern starts/ends with `*`.
fn filter_match(name: &str, pattern: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let pattern = pattern.to_ascii_lowercase();
    if pattern.is_empty() {
        return true;
    }
    if !pattern.contains('*') {
        return name.contains(&pattern);
    }
    let anchored_start = !pattern.starts_with('*');
    let anchored_end = !pattern.ends_with('*');
    let segments: Vec<&str> = pattern.split('*').filter(|s| !s.is_empty()).collect();
    let Some((&last, head)) = segments.split_last() else {
        return true; // pattern was only '*'s
    };
    let in_order = if anchored_end { head } else { &segments[..] };

    let mut pos = 0usize;
    for (i, seg) in in_order.iter().enumerate() {
        if i == 0 && anchored_start {
            if !name.starts_with(seg) {
                return false;
            }
            pos = seg.len();
        } else if let Some(found) = name[pos..].find(seg) {
            pos += found + seg.len();
        } else {
            return false;
        }
    }
    if anchored_end {
        if !name.ends_with(last) {
            return false;
        }
        // the suffix segment must not overlap text consumed by earlier segments
        if name.len() - last.len() < pos {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod filter_tests {
    use super::filter_match;

    #[test]
    fn substring_and_globs() {
        assert!(filter_match("frame_0042.dat", "0042"));
        assert!(filter_match("frame_0042.dat", "FRAME"));
        assert!(!filter_match("frame_0042.dat", "0043"));
        assert!(filter_match("frame_0042.dat", "frame*.dat"));
        assert!(filter_match("frame_0042.dat", "*42.dat"));
        assert!(!filter_match("frame_0042.dat", "*43.dat"));
        assert!(filter_match("frame_0042.dat", "fr*00*dat"));
        assert!(!filter_match("frame_0042.dat", "x*"));
        assert!(filter_match("anything", ""));
    }
}

fn default_data_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../xraytsubaki/tests/testfiles/Ru_QAS.dat")
}

fn default_for(param: PathParam) -> f64 {
    match param {
        PathParam::S02 => 0.9,
        PathParam::E0 => 0.0,
        PathParam::Sigma2 => 0.003,
        PathParam::DeltaR => 0.0,
    }
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
            source_dir: None,
            selected: None,
            selection: BTreeSet::new(),
            derived: Vec::new(),
            compare_gen: 0,
            view: ViewOptions::default(),
            view_offset_field: None,
            filter_input: None,
            filter_text: String::new(),
            filtered: None,
            data_focus: cx.focus_handle(),
            operando_focus: cx.focus_handle(),
            adv_open: [false; 4],
            roi_input: None,
            import_preview: "".into(),
            open_enum: None,
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
            data_tab: DataTab::Files,
            active_scan: None,
            operando: None,
            operando_plots: None,
            operando_gen: 0,
            time_pos: 0,
            fit_paths: Vec::new(),
            fit_vars: Vec::new(),
            fit_range_fields: Vec::new(),
            fit_ranges: FitRanges::default(),
            fit_result: None,
            fit_plots: None,
            fit_gen: 0,
            fit_running: false,
            feff_workspace: None,
            feff_running: false,
            feff_gen: 0,
            feff_form: Vec::new(),
            batch_fit: None,
            batch_running: false,
            batch_progress: (0, 0),
            batch_gen: 0,
            status: "loading...".into(),
        };
        app.fit_range_fields = Self::build_range_fields(theme, app.fit_ranges, cx);
        let offset_field = cx.new(|cx| {
            NumericField::new("offset", "", Some(app.view.offset_frac), theme, cx)
        });
        cx.subscribe(&offset_field, |this: &mut Self, _f, event, cx| {
            let FieldEvent::Changed(value) = event;
            if let Some(v) = value {
                this.view.offset_frac = v.clamp(0.05, 5.0);
                this.rebuild_plots(cx);
                cx.notify();
            }
        })
        .detach();
        app.view_offset_field = Some(offset_field);
        let filter_input = cx.new(|cx| TextInput::new("filter… (* glob)", "", theme, cx));
        cx.subscribe(&filter_input, |this: &mut Self, _f, event, cx| {
            let InputEvent::Committed(text) = event;
            this.filter_text = text.trim().to_string();
            this.apply_filter(cx);
        })
        .detach();
        app.filter_input = Some(filter_input);
        let roi_input = cx.new(|cx| TextInput::new("e.g. 4 or 4-7", "4", theme, cx));
        cx.subscribe(&roi_input, |this: &mut Self, input, event, cx| {
            let InputEvent::Committed(text) = event;
            match parse_cols(text) {
                Some(cols) => {
                    if this.params.import.fluor_cols != cols {
                        this.params.import.fluor_cols = cols;
                        this.schedule_recompute(cx);
                    }
                }
                None => {
                    // revert to current value
                    let text = this
                        .params
                        .import
                        .fluor_cols
                        .iter()
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join(",");
                    input.update(cx, |i, cx| i.set_text(text, cx));
                }
            }
            cx.notify();
        })
        .detach();
        app.roi_input = Some(roi_input);
        app.feff_form = [
            (FeffFormKey::Element, "element", "Cu"),
            (FeffFormKey::Element2, "element 2", ""),
            (FeffFormKey::Structure, "fcc/bcc/hcp/...", "fcc"),
            (FeffFormKey::LatticeA, "a (Å)", "3.615"),
            (FeffFormKey::LatticeC, "c (Å, hcp)", ""),
            (FeffFormKey::Edge, "edge", "K"),
            (FeffFormKey::Rmax, "rmax (Å)", "5.0"),
        ]
        .into_iter()
        .map(|(key, placeholder, initial)| {
            (key, cx.new(|cx| TextInput::new(placeholder, initial, theme, cx)))
        })
        .collect();

        app.update_import_preview();
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
        let specs: [(ParamKey, &str, &str); 30] = [
            (ParamKey::ImpEnergyCol, "energy col", "0"),
            (ParamKey::ImpI0Col, "I0 col", "1"),
            (ParamKey::ImpItCol, "It col", "2"),
            (ParamKey::ImpIrCol, "Ir col", "3"),
            (ParamKey::AlignTarget, "ref E0 target", "e.g. 22117"),
            (ParamKey::E0, "E0 (eV)", "auto"),
            (ParamKey::PreEdgeStart, "pre-edge start", "auto (-200)"),
            (ParamKey::PreEdgeEnd, "pre-edge end", "auto (-30)"),
            (ParamKey::NormStart, "norm start", "auto (150)"),
            (ParamKey::NormEnd, "norm end", "auto (2000)"),
            (ParamKey::NormPolyorder, "poly order", "auto (2)"),
            (ParamKey::NVictoreen, "victoreen n", "auto (0)"),
            (ParamKey::Rbkg, "rbkg (Å)", "auto (1.0)"),
            (ParamKey::BkgKmin, "k min", "auto (0)"),
            (ParamKey::BkgKmax, "k max", "auto (full)"),
            (ParamKey::BkgKstep, "k step", "auto (0.05)"),
            (ParamKey::BkgNknots, "spline knots", "auto"),
            (ParamKey::BkgKweight, "bkg k-weight", "auto (1)"),
            (ParamKey::BkgClampLo, "clamp lo", "auto (0)"),
            (ParamKey::BkgClampHi, "clamp hi", "auto (1)"),
            (ParamKey::BkgDk, "window dk", "auto (0.1)"),
            (ParamKey::BkgNfft, "nfft", "auto (2048)"),
            (ParamKey::FftKmin, "k min", "auto (2)"),
            (ParamKey::FftKmax, "k max", "auto (15)"),
            (ParamKey::FftDk, "dk", "auto (1)"),
            (ParamKey::FftKweight, "k-weight", "auto (2)"),
            (ParamKey::FftDk2, "dk2", "auto"),
            (ParamKey::FftRmax, "R max out", "auto (10)"),
            (ParamKey::FftKstep, "k step", "auto"),
            (ParamKey::FftNfft, "nfft", "auto (2048)"),
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
        let int = value.map(|v| v.round() as i32);
        let col = value.map(|v| v.round().max(0.0) as usize);
        match key {
            ParamKey::ImpEnergyCol => p.import.energy_col = col.unwrap_or(0),
            ParamKey::ImpI0Col => p.import.i0_col = col.unwrap_or(1),
            ParamKey::ImpItCol => p.import.it_col = col.unwrap_or(2),
            ParamKey::ImpIrCol => p.import.ir_col = col.unwrap_or(3),
            ParamKey::AlignTarget => p.align_target = value,
            ParamKey::E0 => p.e0 = value,
            ParamKey::PreEdgeStart => p.pre_edge_start = value,
            ParamKey::PreEdgeEnd => p.pre_edge_end = value,
            ParamKey::NormStart => p.norm_start = value,
            ParamKey::NormEnd => p.norm_end = value,
            ParamKey::NormPolyorder => p.norm_polyorder = int,
            ParamKey::NVictoreen => p.n_victoreen = int,
            ParamKey::Rbkg => p.rbkg = value,
            ParamKey::BkgKmin => p.bkg_kmin = value,
            ParamKey::BkgKmax => p.bkg_kmax = value,
            ParamKey::BkgKstep => p.bkg_kstep = value,
            ParamKey::BkgNknots => p.bkg_nknots = int,
            ParamKey::BkgKweight => p.bkg_kweight = int,
            ParamKey::BkgClampLo => p.bkg_clamp_lo = int,
            ParamKey::BkgClampHi => p.bkg_clamp_hi = int,
            ParamKey::BkgDk => p.bkg_dk = value,
            ParamKey::BkgNfft => p.bkg_nfft = int,
            ParamKey::FftKmin => p.fft_kmin = value,
            ParamKey::FftKmax => p.fft_kmax = value,
            ParamKey::FftDk => p.fft_dk = value,
            ParamKey::FftKweight => p.fft_kweight = value,
            ParamKey::FftDk2 => p.fft_dk2 = value,
            ParamKey::FftRmax => p.fft_rmax = value,
            ParamKey::FftKstep => p.fft_kstep = value,
            ParamKey::FftNfft => p.fft_nfft = int,
        }
        self.schedule_recompute(cx);
    }

    /// Option labels for an enum parameter: index 0 = auto, then variants.
    fn enum_options(which: EnumParam) -> Vec<String> {
        let (auto_label, variants): (&str, Vec<String>) = match which {
            EnumParam::ImportMode => {
                return DETECTION_MODES.iter().map(|m| format!("{m:?}")).collect();
            }
            EnumParam::BkgWindow => (
                "auto (Hanning)",
                FT_WINDOWS.iter().map(|w| format!("{w:?}")).collect(),
            ),
            EnumParam::BkgSolver => (
                "auto (LinearDirect)",
                AUTOBK_SOLVERS.iter().map(|v| format!("{v:?}")).collect(),
            ),
            EnumParam::FftWindow => (
                "auto (KaiserBessel)",
                FT_WINDOWS.iter().map(|w| format!("{w:?}")).collect(),
            ),
        };
        let mut out = vec![auto_label.to_string()];
        out.extend(variants);
        out
    }

    /// Apply a selection from the option list (0 = auto).
    fn set_enum_param(&mut self, which: EnumParam, index: usize, cx: &mut Context<Self>) {
        let variant = index.checked_sub(1);
        match which {
            EnumParam::ImportMode => {
                self.params.import.mode = DETECTION_MODES[index.min(2)];
            }
            EnumParam::BkgWindow => {
                self.params.bkg_window = variant.map(|i| FT_WINDOWS[i]);
            }
            EnumParam::BkgSolver => {
                self.params.bkg_solver = variant.map(|i| AUTOBK_SOLVERS[i]);
            }
            EnumParam::FftWindow => {
                self.params.fft_window = variant.map(|i| FT_WINDOWS[i]);
            }
        }
        self.open_enum = None;
        self.schedule_recompute(cx);
        cx.notify();
    }

    fn enum_selected_index(&self, which: EnumParam) -> usize {
        match which {
            EnumParam::ImportMode => DETECTION_MODES
                .iter()
                .position(|m| *m == self.params.import.mode)
                .unwrap_or(0),
            EnumParam::BkgWindow => self
                .params
                .bkg_window
                .and_then(|w| FT_WINDOWS.iter().position(|x| *x == w))
                .map(|i| i + 1)
                .unwrap_or(0),
            EnumParam::BkgSolver => self
                .params
                .bkg_solver
                .and_then(|v| AUTOBK_SOLVERS.iter().position(|x| *x == v))
                .map(|i| i + 1)
                .unwrap_or(0),
            EnumParam::FftWindow => self
                .params
                .fft_window
                .and_then(|w| FT_WINDOWS.iter().position(|x| *x == w))
                .map(|i| i + 1)
                .unwrap_or(0),
        }
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
        // Parameter edits also invalidate the operando overview + overlay.
        self.ensure_operando(cx);
        self.ensure_compare_loaded(cx);
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
        let params = self.params.clone();
        let derived = (ix >= DERIVED_BASE)
            .then(|| self.derived.get(ix - DERIVED_BASE).cloned())
            .flatten();
        let load = cx.background_executor().spawn(async move {
            match derived {
                Some(d) => process_arrays(d.energy, d.mu, &params),
                None => process_file(&path, &params),
            }
        });
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

    // ---- operando ----------------------------------------------------------

    fn open_scan(&mut self, scan_ix: usize, cx: &mut Context<Self>) {
        self.active_scan = Some(scan_ix);
        self.workspace = Workspace::Operando;
        self.ensure_operando(cx);
        cx.notify();
    }

    /// (Re)build the downsampled scan overview if the scan or parameters
    /// changed. Frames are processed in parallel on rayon.
    fn ensure_operando(&mut self, cx: &mut Context<Self>) {
        let Some(scan_ix) = self.active_scan else {
            return;
        };
        let fingerprint = self.params.fingerprint();
        if self
            .operando
            .as_ref()
            .is_some_and(|o| o.scan == scan_ix && o.fingerprint == fingerprint)
        {
            return;
        }
        let Some(scan) = self.catalog.scans.get(scan_ix) else {
            return;
        };

        self.operando_gen += 1;
        let generation = self.operando_gen;
        // Even sampling across the scan; first and last frames included.
        let sample_ixs: Vec<usize> = if scan.len <= MAX_FRAMES {
            (scan.start..scan.start + scan.len).collect()
        } else {
            (0..MAX_FRAMES)
                .map(|i| scan.start + i * (scan.len - 1) / (MAX_FRAMES - 1))
                .collect()
        };
        let paths: Vec<PathBuf> = sample_ixs.iter().map(|&ix| self.catalog.path(ix)).collect();
        let params = self.params.clone();
        let grid: Vec<f64> = (0..K_GRID_BINS)
            .map(|i| i as f64 * K_GRID_MAX / (K_GRID_BINS - 1) as f64)
            .collect();
        self.status = format!(
            "building scan overview ({} of {} frames) ...",
            paths.len(),
            scan.len
        )
        .into();

        let job_grid = grid.clone();
        let job = cx.background_executor().spawn(async move {
            paths
                .par_iter()
                .map(|path| {
                    process_file(path, &params)
                        .ok()
                        .and_then(|sp| {
                            let row = resample_chik(&sp, &job_grid)?;
                            Some((row, sp.get_e0().unwrap_or(f64::NAN)))
                        })
                        .unwrap_or_else(|| (vec![0.0; job_grid.len()], f64::NAN))
                })
                .collect::<Vec<(Vec<f64>, f64)>>()
        });
        cx.spawn(async move |this, cx| {
            let rows = job.await;
            this.update(cx, |app, cx| {
                if app.operando_gen != generation {
                    return;
                }
                let (matrix, e0s): (Vec<Vec<f64>>, Vec<f64>) = rows.into_iter().unzip();
                let kweight = app.params.fft_kweight.unwrap_or(2.0);
                app.operando = Some(OperandoData {
                    scan: scan_ix,
                    fingerprint,
                    sample_ixs,
                    grid,
                    matrix,
                    e0s,
                    kweight,
                });
                app.time_pos = 0;
                app.rebuild_operando_plots(cx);
                app.status = "scan overview ready".into();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn rebuild_operando_plots(&mut self, cx: &mut Context<Self>) {
        let Some(data) = &self.operando else {
            return;
        };
        let heatmap = build_heatmap(&data.matrix, K_GRID_MAX, &self.theme);
        let row = data.matrix.get(self.time_pos).cloned().unwrap_or_default();
        let chik = build_frame_chik(&data.grid, &row, data.kweight, &self.theme);
        let (trend_values, trend_label) = self.trend_series();
        let trend = build_trend(&trend_values, self.time_pos, &trend_label, &self.theme);
        match &self.operando_plots {
            Some(plots) => {
                plots.heatmap.update(cx, |rp, cx| rp.set_plot(heatmap, cx));
                plots.chik.update(cx, |rp, cx| rp.set_plot(chik, cx));
                plots.trend.update(cx, |rp, cx| rp.set_plot(trend, cx));
            }
            None => {
                self.operando_plots = Some(OperandoPlots {
                    heatmap: plot_builder(heatmap).interactive().build(cx),
                    chik: plot_builder(chik).interactive().build(cx),
                    trend: plot_builder(trend).interactive().build(cx),
                });
            }
        }
    }

    fn step_time(&mut self, delta: isize, cx: &mut Context<Self>) {
        let next = self.time_pos as isize + delta;
        self.set_time_pos(next.max(0) as usize, cx);
    }

    fn set_time_pos(&mut self, pos: usize, cx: &mut Context<Self>) {
        let Some(data) = &self.operando else {
            return;
        };
        let pos = pos.min(data.matrix.len().saturating_sub(1));
        if pos == self.time_pos {
            return;
        }
        self.time_pos = pos;
        let ix = data.sample_ixs.get(pos).copied();
        let row = data.matrix.get(pos).cloned().unwrap_or_default();
        let chik = build_frame_chik(&data.grid, &row, data.kweight, &self.theme);
        let (trend_values, trend_label) = self.trend_series();
        let trend = build_trend(&trend_values, pos, &trend_label, &self.theme);
        if let Some(plots) = &self.operando_plots {
            plots.chik.update(cx, |rp, cx| rp.set_plot(chik, cx));
            plots.trend.update(cx, |rp, cx| rp.set_plot(trend, cx));
        }
        if let Some(ix) = ix {
            self.status = format!(
                "frame {}/{} · {}",
                pos + 1,
                data.matrix.len(),
                self.catalog.name(ix)
            )
            .into();
        }
        cx.notify();
    }

    /// Selection (plus active) thinned evenly to MAX_OVERLAY, active always
    /// kept. Returns (indices, total_before_thinning).
    fn compare_indices(&self) -> (Vec<usize>, usize) {
        let mut set: BTreeSet<usize> = self.selection.clone();
        if let Some(active) = self.selected {
            set.insert(active);
        }
        let all: Vec<usize> = set.into_iter().collect();
        let total = all.len();
        (thin_even(&all, MAX_OVERLAY, self.selected), total)
    }

    /// Process any compare-set members missing from the cache (rayon batch),
    /// then rebuild the overlay.
    fn ensure_compare_loaded(&mut self, cx: &mut Context<Self>) {
        let fingerprint = self.params.fingerprint();
        let (indices, _) = self.compare_indices();
        let missing: Vec<(usize, Result<PathBuf, DerivedSpectrum>)> = indices
            .iter()
            .filter(|&&ix| ix != NO_ENTRY && !self.cache.contains(&(ix, fingerprint)))
            .filter_map(|&ix| {
                if ix >= DERIVED_BASE {
                    self.derived
                        .get(ix - DERIVED_BASE)
                        .map(|d| (ix, Err(d.clone())))
                } else {
                    Some((ix, Ok(self.catalog.path(ix))))
                }
            })
            .collect();
        if missing.is_empty() {
            self.rebuild_plots(cx);
            cx.notify();
            return;
        }
        self.compare_gen += 1;
        let generation = self.compare_gen;
        self.status = format!("processing {} spectra for overlay ...", missing.len()).into();
        cx.notify();
        let params = self.params.clone();
        let job = cx.background_executor().spawn(async move {
            missing
                .par_iter()
                .map(|(ix, source)| {
                    let result = match source {
                        Ok(path) => process_file(path, &params),
                        Err(d) => process_arrays(d.energy.clone(), d.mu.clone(), &params),
                    };
                    (*ix, result)
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let results = job.await;
            this.update(cx, |app, cx| {
                if app.compare_gen != generation {
                    return;
                }
                let mut failed = 0usize;
                for (ix, result) in results {
                    match result {
                        Ok(sp) => {
                            app.cache.put((ix, fingerprint), Arc::new(sp));
                        }
                        Err(_) => failed += 1,
                    }
                }
                let (_, total) = app.compare_indices();
                app.status = if failed > 0 {
                    format!("overlay ready · {failed} failed").into()
                } else {
                    format!("overlay of {total} spectra ready").into()
                };
                app.rebuild_plots(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn rebuild_plots(&mut self, cx: &mut Context<Self>) {
        let fingerprint = self.params.fingerprint();
        let (indices, total) = self.compare_indices();
        let mut traces: Vec<QuadTrace> = Vec::new();
        for ix in indices {
            if ix == NO_ENTRY {
                continue;
            }
            let label = self.entry_label(ix);
            if let Some(sp) = self.cache.get(&(ix, fingerprint)) {
                traces.push(QuadTrace {
                    label,
                    sp: sp.clone(),
                    active: Some(ix) == self.selected,
                });
            }
        }
        // No catalog selection (default file) or nothing cached yet: fall
        // back to the active spectrum object.
        if traces.is_empty() {
            let Some(sp) = &self.spectrum else {
                return;
            };
            traces.push(QuadTrace {
                label: self.spectrum_label.to_string(),
                sp: sp.clone(),
                active: true,
            });
        }
        if total > MAX_OVERLAY {
            self.status = format!(
                "showing {} of {total} selected — the Operando heatmap shows the full set",
                traces.len()
            )
            .into();
        }
        let plots = build_quadrants_multi(&traces, &self.view, &self.theme);
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
        for (_, field) in &self.fit_range_fields {
            field.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        for var in &self.fit_vars {
            var.field.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        for row in &self.fit_paths {
            for (_, field) in &row.fields {
                field.update(cx, |f, cx| f.set_theme(theme, cx));
            }
        }
        for (_, field) in &self.feff_form {
            field.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        if let Some(field) = &self.view_offset_field {
            field.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        if let Some(input) = &self.roi_input {
            input.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        self.rebuild_plots(cx);
        self.rebuild_operando_plots(cx);
        self.rebuild_fit_plots(cx);
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
        self.source_dir = Some(root.clone());
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
                            if !app.filter_text.is_empty() {
                                app.apply_filter(cx);
                            }
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

    /// Modifier-aware list click: plain = activate (clears compare set),
    /// shift = extend range from the active row, cmd = toggle membership.
    fn click_entry(&mut self, ix: usize, modifiers: gpui::Modifiers, cx: &mut Context<Self>) {
        if modifiers.shift
            && let Some(anchor) = self.selected
            && anchor < DERIVED_BASE
            && ix < DERIVED_BASE
        {
            match &self.filtered {
                None => {
                    let (lo, hi) = (anchor.min(ix), anchor.max(ix));
                    self.selection.extend(lo..=hi);
                }
                Some(filtered) => {
                    // range over the *visible* rows
                    if let (Ok(a), Ok(b)) =
                        (filtered.binary_search(&anchor), filtered.binary_search(&ix))
                    {
                        let (lo, hi) = (a.min(b), a.max(b));
                        self.selection.extend(filtered[lo..=hi].iter().copied());
                    } else {
                        self.selection.insert(ix);
                    }
                }
            }
            self.ensure_compare_loaded(cx);
        } else if modifiers.platform {
            if !self.selection.remove(&ix) {
                self.selection.insert(ix);
            }
            self.ensure_compare_loaded(cx);
        } else {
            self.selection.clear();
            self.select_entry(ix, cx);
        }
        cx.notify();
    }

    /// Neighbor of the active row within the visible (filtered) list.
    fn visible_neighbor(&self, delta: isize) -> Option<usize> {
        let active = self.selected?;
        match &self.filtered {
            None => {
                let next = active as isize + delta;
                (next >= 0 && (next as usize) < self.catalog.len()).then_some(next as usize)
            }
            Some(filtered) => {
                let pos = filtered.binary_search(&active).ok()? as isize + delta;
                (pos >= 0).then(|| filtered.get(pos as usize).copied())?
            }
        }
    }

    fn nav_move(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let Some(next) = self.visible_neighbor(delta) else {
            return;
        };
        if extend {
            if let Some(active) = self.selected {
                self.selection.insert(active);
            }
            self.selection.insert(next);
        } else {
            self.selection.clear();
        }
        self.select_entry(next, cx);
        if extend {
            self.ensure_compare_loaded(cx);
        }
        cx.notify();
    }

    /// Add a whole scan to the compare set (shift/cmd-click on a scan row).
    fn select_scan_range(&mut self, scan_ix: usize, cx: &mut Context<Self>) {
        if let Some(scan) = self.catalog.scans.get(scan_ix) {
            self.selection.extend(scan.start..scan.start + scan.len);
            self.ensure_compare_loaded(cx);
            cx.notify();
        }
    }

    fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if !self.selection.is_empty() {
            self.selection.clear();
            self.rebuild_plots(cx);
            cx.notify();
        }
    }

    fn apply_filter(&mut self, cx: &mut Context<Self>) {
        if self.filter_text.is_empty() {
            self.filtered = None;
        } else {
            let pattern = self.filter_text.clone();
            let matches: Vec<usize> = (0..self.catalog.len())
                .filter(|&ix| filter_match(self.catalog.name(ix), &pattern))
                .collect();
            self.filtered = Some(Arc::new(matches));
        }
        cx.notify();
    }

    /// Add the scan containing the active spectrum to the compare set.
    fn select_active_scan(&mut self, cx: &mut Context<Self>) {
        let Some(active) = self.selected else {
            return;
        };
        if let Some(scan_ix) = self
            .catalog
            .scans
            .iter()
            .position(|s| (s.start..s.start + s.len).contains(&active))
        {
            self.select_scan_range(scan_ix, cx);
        }
    }

    /// Keep every 10th member of the current selection.
    fn thin_selection(&mut self, cx: &mut Context<Self>) {
        if self.selection.len() <= 1 {
            return;
        }
        let kept: BTreeSet<usize> = self
            .selection
            .iter()
            .copied()
            .step_by(10)
            .collect();
        self.selection = kept;
        self.ensure_compare_loaded(cx);
        cx.notify();
    }

    /// Average the selected catalog spectra into a derived spectrum.
    fn merge_selection(&mut self, cx: &mut Context<Self>) {
        let files: Vec<usize> = self
            .selection
            .iter()
            .copied()
            .filter(|&ix| ix < DERIVED_BASE)
            .collect();
        if files.len() < 2 {
            self.status = "select at least 2 spectra to merge".into();
            cx.notify();
            return;
        }
        let label = format!(
            "avg of {} ({}..{})",
            files.len(),
            self.catalog.name(files[0]),
            self.catalog.name(*files.last().unwrap())
        );
        let paths: Vec<PathBuf> = files.iter().map(|&ix| self.catalog.path(ix)).collect();
        let params = self.params.clone();
        self.status = format!("merging {} spectra ...", files.len()).into();
        cx.notify();
        let job = cx.background_executor().spawn(async move {
            let inputs: Result<Vec<(Vec<f64>, Vec<f64>)>, String> =
                paths.par_iter().map(|p| load_raw(p, &params)).collect();
            inputs.and_then(|inputs| average_spectra(&inputs))
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                match result {
                    Ok((energy, mu)) => {
                        app.derived.push(DerivedSpectrum {
                            label: label.clone(),
                            energy,
                            mu,
                        });
                        let ix = DERIVED_BASE + app.derived.len() - 1;
                        app.status = format!("merged → {label}").into();
                        app.selection.clear();
                        app.select_entry(ix, cx);
                    }
                    Err(e) => {
                        app.status = format!("merge failed: {e}").into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn remove_derived(&mut self, i: usize, cx: &mut Context<Self>) {
        if i >= self.derived.len() {
            return;
        }
        self.derived.remove(i);
        // Virtual indices shift; drop selections/cache touching derived.
        self.selection.retain(|&ix| ix < DERIVED_BASE);
        if self.selected.is_some_and(|ix| ix >= DERIVED_BASE) {
            self.selected = None;
        }
        self.cache.clear();
        self.rebuild_plots(cx);
        cx.notify();
    }

    /// Add all filter results to the compare set.
    fn select_filter_results(&mut self, cx: &mut Context<Self>) {
        if let Some(filtered) = &self.filtered {
            self.selection.extend(filtered.iter().copied());
            self.ensure_compare_loaded(cx);
            cx.notify();
        }
    }

    fn entry_label(&self, ix: usize) -> String {
        if ix >= DERIVED_BASE {
            self.derived
                .get(ix - DERIVED_BASE)
                .map(|d| d.label.clone())
                .unwrap_or_else(|| "merged".into())
        } else {
            self.catalog.name(ix).to_string()
        }
    }

    fn select_entry(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix >= DERIVED_BASE {
            if ix - DERIVED_BASE >= self.derived.len() {
                return;
            }
            self.selected = Some(ix);
            let label: SharedString = self.entry_label(ix).into();
            self.spectrum_label = label.clone();
            self.load_spectrum(ix, PathBuf::new(), label, cx);
            return;
        }
        if ix >= self.catalog.len() {
            return;
        }
        self.selected = Some(ix);
        let label: SharedString = self.catalog.name(ix).to_string().into();
        self.spectrum_label = label.clone();
        let path = self.catalog.path(ix);
        self.current_path = path.clone();
        self.update_import_preview();
        self.load_spectrum(ix, path, label, cx);
    }

    fn update_import_preview(&mut self) {
        self.import_preview = match read_columns(&self.current_path) {
            Ok(rows) => {
                let first = rows[0]
                    .iter()
                    .take(6)
                    .map(|v| format!("{v:.4}"))
                    .collect::<Vec<_>>()
                    .join("  ");
                format!("{} columns · row 0: {first}", rows[0].len()).into()
            }
            Err(_) => "no preview".into(),
        };
    }

    // ---- fitting -----------------------------------------------------------

    fn build_range_fields(
        theme: Theme,
        ranges: FitRanges,
        cx: &mut Context<Self>,
    ) -> Vec<(RangeKey, Entity<NumericField>)> {
        let specs: [(RangeKey, &str, f64); 5] = [
            (RangeKey::Kmin, "k min", ranges.kmin),
            (RangeKey::Kmax, "k max", ranges.kmax),
            (RangeKey::Rmin, "R min", ranges.rmin),
            (RangeKey::Rmax, "R max", ranges.rmax),
            (RangeKey::Kweight, "k-weight", ranges.kweight),
        ];
        specs
            .into_iter()
            .map(|(key, label, value)| {
                let field =
                    cx.new(|cx| NumericField::new(label, "", Some(value), theme, cx));
                cx.subscribe(&field, move |this: &mut Self, _field, event, cx| {
                    let FieldEvent::Changed(value) = event;
                    if let Some(v) = value {
                        let r = &mut this.fit_ranges;
                        match key {
                            RangeKey::Kmin => r.kmin = *v,
                            RangeKey::Kmax => r.kmax = *v,
                            RangeKey::Rmin => r.rmin = *v,
                            RangeKey::Rmax => r.rmax = *v,
                            RangeKey::Kweight => r.kweight = *v,
                        }
                        cx.notify();
                    }
                })
                .detach();
                (key, field)
            })
            .collect()
    }

    fn ensure_fit_var(&mut self, name: &str, default: f64, cx: &mut Context<Self>) {
        if name.is_empty() || self.fit_vars.iter().any(|v| v.spec.name == name) {
            return;
        }
        let theme = self.theme;
        let field = cx.new(|cx| TextInput::new("value or expr", format!("{default}"), theme, cx));
        let var_name = name.to_string();
        cx.subscribe(&field, move |this: &mut Self, _field, event, cx| {
            let InputEvent::Committed(text) = event;
            let text = text.trim().to_string();
            this.set_var_text(&var_name.clone(), &text, cx);
        })
        .detach();
        self.fit_vars.push(FitVar {
            spec: FitVarSpec {
                name: name.to_string(),
                value: default,
                vary: true,
                expr: None,
            },
            field,
        });
    }

    /// A variable field committed: a number sets the value; anything else is
    /// a derived expression. New identifiers become variables automatically.
    fn set_var_text(&mut self, name: &str, text: &str, cx: &mut Context<Self>) {
        if text.is_empty() {
            return;
        }
        match text.parse::<f64>() {
            Ok(v) if v.is_finite() => {
                if let Some(var) = self.fit_vars.iter_mut().find(|x| x.spec.name == name) {
                    var.spec.value = v;
                    var.spec.expr = None;
                }
            }
            _ => {
                for ident in expr_identifiers(text) {
                    if ident != name {
                        self.ensure_fit_var(&ident, 0.0, cx);
                    }
                }
                if let Some(var) = self.fit_vars.iter_mut().find(|x| x.spec.name == name) {
                    var.spec.expr = Some(text.to_string());
                }
            }
        }
        cx.notify();
    }

    /// A path parameter cell committed: store the expression and auto-create
    /// any new variables it references.
    fn set_path_param(
        &mut self,
        path_ix: usize,
        param: PathParam,
        text: &str,
        cx: &mut Context<Self>,
    ) {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        if text.parse::<f64>().is_err() {
            for ident in expr_identifiers(&text) {
                self.ensure_fit_var(&ident, default_for(param), cx);
            }
        }
        if let Some(row) = self.fit_paths.get_mut(path_ix) {
            let spec = &mut row.spec;
            match param {
                PathParam::S02 => spec.s02 = text,
                PathParam::E0 => spec.e0 = text,
                PathParam::Sigma2 => spec.sigma2 = text,
                PathParam::DeltaR => spec.deltar = text,
            }
        }
        cx.notify();
    }

    fn add_fit_path_dialog(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                this.update(cx, |app, cx| {
                    for file in paths {
                        app.push_fit_path(file, cx);
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Standard parameterization: shared amp/de0, per-path sigma2/deltar.
    /// Every parameter cell accepts a number or an expression.
    fn push_fit_path(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        let i = self.fit_paths.len() + 1;
        let label = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.display().to_string());
        let sigma2 = format!("sig2_{i}");
        let deltar = format!("dr_{i}");
        self.ensure_fit_var("amp", 0.9, cx);
        self.ensure_fit_var("de0", 0.0, cx);
        self.ensure_fit_var(&sigma2, 0.003, cx);
        self.ensure_fit_var(&deltar, 0.0, cx);
        let spec = FitPathSpec {
            file,
            label,
            s02: "amp".into(),
            e0: "de0".into(),
            sigma2,
            deltar,
            enabled: true,
        };
        self.add_path_row(spec, cx);
    }

    /// Materialize a path spec into a row with editable cells.
    fn add_path_row(&mut self, spec: FitPathSpec, cx: &mut Context<Self>) {
        let meta = path_meta(&spec.file);
        let theme = self.theme;
        let path_ix = self.fit_paths.len();
        let fields = [
            (PathParam::S02, spec.s02.clone()),
            (PathParam::E0, spec.e0.clone()),
            (PathParam::Sigma2, spec.sigma2.clone()),
            (PathParam::DeltaR, spec.deltar.clone()),
        ]
        .into_iter()
        .map(|(param, initial)| {
            let field = cx.new(|cx| TextInput::new("expr", initial, theme, cx));
            cx.subscribe(&field, move |this: &mut Self, _field, event, cx| {
                let InputEvent::Committed(text) = event;
                this.set_path_param(path_ix, param, text, cx);
            })
            .detach();
            (param, field)
        })
        .collect();
        self.fit_paths.push(FitPathRow { spec, meta, fields, expanded: false });
    }

    fn run_fit_now(&mut self, cx: &mut Context<Self>) {
        if self.fit_running {
            return;
        }
        let Some(sp) = &self.spectrum else {
            self.status = "no spectrum loaded".into();
            cx.notify();
            return;
        };
        let (Some(k), Some(chi)) = (sp.get_k(), sp.get_chi()) else {
            self.status = "current spectrum has no chi(k) — check background".into();
            cx.notify();
            return;
        };
        self.fit_gen += 1;
        let generation = self.fit_gen;
        self.fit_running = true;
        self.status = "fitting ...".into();
        cx.notify();
        let paths: Vec<FitPathSpec> = self.fit_paths.iter().map(|r| r.spec.clone()).collect();
        let vars: Vec<FitVarSpec> = self.fit_vars.iter().map(|v| v.spec.clone()).collect();
        let ranges = self.fit_ranges;
        let job = cx
            .background_executor()
            .spawn(async move { run_fit(k, chi, &paths, &vars, ranges) });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                if app.fit_gen != generation {
                    return;
                }
                app.fit_running = false;
                match result {
                    Ok(result) => {
                        app.status = format!(
                            "fit done · R-factor {:.5} · red. chi² {:.3e}",
                            result.r_factor, result.reduced_chi_square
                        )
                        .into();
                        let result = Arc::new(result);
                        app.fit_result = Some(result.clone());
                        app.rebuild_fit_plots(cx);
                        // Reflect fitted values back into the variable fields.
                        for var in &mut app.fit_vars {
                            if var.spec.expr.is_some() {
                                continue;
                            }
                            if let Some(fitted) = result.variables.get(&var.spec.name) {
                                var.spec.value = fitted.value;
                                let text = format!("{:.5}", fitted.value);
                                var.field.update(cx, |f, cx| f.set_text(text, cx));
                            }
                        }
                    }
                    Err(e) => {
                        app.status = format!("fit failed: {e}").into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn rebuild_fit_plots(&mut self, cx: &mut Context<Self>) {
        let Some(result) = &self.fit_result else {
            return;
        };
        let k_plot = build_fit_k(result, &self.theme);
        let r_plot = build_fit_r(result, &self.theme);
        match &self.fit_plots {
            Some((k_entity, r_entity)) => {
                k_entity.update(cx, |rp, cx| rp.set_plot(k_plot, cx));
                r_entity.update(cx, |rp, cx| rp.set_plot(r_plot, cx));
            }
            None => {
                self.fit_plots = Some((
                    plot_builder(k_plot).interactive().build(cx),
                    plot_builder(r_plot).interactive().build(cx),
                ));
            }
        }
    }

    // ---- batch fitting -------------------------------------------------------

    /// Fit every sampled frame of the active scan with the current model, in
    /// parallel on rayon, streaming progress to the status bar.
    fn run_batch_fit(&mut self, cx: &mut Context<Self>) {
        if self.batch_running {
            return;
        }
        let Some(data) = &self.operando else {
            self.status = "open a scan in Operando first".into();
            cx.notify();
            return;
        };
        if self.fit_paths.is_empty() {
            self.status = "no FEFF paths in the model".into();
            cx.notify();
            return;
        }
        let scan = data.scan;
        let fingerprint = data.fingerprint;
        let frames: Vec<(usize, usize, PathBuf)> = data
            .sample_ixs
            .iter()
            .enumerate()
            .map(|(pos, &ix)| (pos, ix, self.catalog.path(ix)))
            .collect();
        let total = frames.len();
        self.batch_gen += 1;
        let generation = self.batch_gen;
        self.batch_running = true;
        self.batch_progress = (0, total);
        self.status = format!("batch fit 0/{total} ...").into();
        cx.notify();

        let params = self.params.clone();
        let paths: Vec<FitPathSpec> = self.fit_paths.iter().map(|r| r.spec.clone()).collect();
        let vars: Vec<FitVarSpec> = self.fit_vars.iter().map(|v| v.spec.clone()).collect();
        let ranges = self.fit_ranges;
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();

        let job = cx.background_executor().spawn(async move {
            let rows: Vec<Option<BatchFitRow>> = frames
                .par_iter()
                .map(|(pos, ix, path)| {
                    let row = (|| {
                        let sp = process_file(path, &params).ok()?;
                        let (k, chi) = (sp.get_k()?, sp.get_chi()?);
                        let result = run_fit(k, chi, &paths, &vars, ranges).ok()?;
                        Some(BatchFitRow::from_result(*pos, *ix, &result))
                    })();
                    let _ = tx.unbounded_send(());
                    row
                })
                .collect();
            rows
        });

        // Progress drain: one tick per completed frame.
        cx.spawn(async move |this, cx| {
            while rx.next().await.is_some() {
                let stop = this
                    .update(cx, |app, cx| {
                        if app.batch_gen != generation {
                            return true;
                        }
                        app.batch_progress.0 += 1;
                        let (done, total) = app.batch_progress;
                        app.status = format!("batch fit {done}/{total} ...").into();
                        cx.notify();
                        false
                    })
                    .unwrap_or(true);
                if stop {
                    break;
                }
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            let rows = job.await;
            this.update(cx, |app, cx| {
                if app.batch_gen != generation {
                    return;
                }
                app.batch_running = false;
                let ok_rows: Vec<BatchFitRow> = rows.into_iter().flatten().collect();
                let failed = total - ok_rows.len();
                let varying_names: Vec<String> = ok_rows
                    .first()
                    .map(|r| r.values.iter().map(|(n, _, _)| n.clone()).collect())
                    .unwrap_or_default();
                app.status = format!(
                    "batch fit done · {} ok · {failed} failed",
                    ok_rows.len()
                )
                .into();
                app.batch_fit = Some(BatchFitData {
                    scan,
                    fingerprint,
                    rows: ok_rows,
                    varying_names,
                    trend_param: 0,
                });
                app.rebuild_operando_plots(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The operando trend series: fitted parameter when a batch fit exists
    /// for the current scan/params, otherwise E0.
    fn trend_series(&self) -> (Vec<f64>, String) {
        if let (Some(bf), Some(data)) = (&self.batch_fit, &self.operando)
            && bf.scan == data.scan
            && bf.fingerprint == data.fingerprint
            && let Some(name) = bf.varying_names.get(bf.trend_param)
        {
            let mut values = vec![f64::NAN; data.matrix.len()];
            for row in &bf.rows {
                if let (Some(slot), Some(v)) = (values.get_mut(row.frame), row.value_of(name)) {
                    *slot = v;
                }
            }
            // Fill gaps so the line stays drawable.
            let mut last = values.iter().copied().find(|v| v.is_finite()).unwrap_or(0.0);
            for v in values.iter_mut() {
                if v.is_finite() {
                    last = *v;
                } else {
                    *v = last;
                }
            }
            return (values, name.clone());
        }
        let values = self
            .operando
            .as_ref()
            .map(|d| d.e0s.clone())
            .unwrap_or_default();
        (values, "E0 (eV)".to_string())
    }

    fn cycle_trend_param(&mut self, cx: &mut Context<Self>) {
        if let Some(bf) = &mut self.batch_fit
            && !bf.varying_names.is_empty()
        {
            bf.trend_param = (bf.trend_param + 1) % bf.varying_names.len();
            self.rebuild_operando_plots(cx);
            cx.notify();
        }
    }

    fn export_batch_csv(&mut self, cx: &mut Context<Self>) {
        let Some(bf) = &self.batch_fit else {
            return;
        };
        let files: Vec<String> = self
            .operando
            .as_ref()
            .map(|d| {
                d.sample_ixs
                    .iter()
                    .map(|&ix| self.catalog.name(ix).to_string())
                    .collect()
            })
            .unwrap_or_default();
        let csv = batch_csv(&bf.rows, &bf.varying_names, &files);
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let rx = cx.prompt_for_new_path(std::path::Path::new(&home), Some("batch_fit.csv"));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                let message = match std::fs::write(&path, csv) {
                    Ok(()) => format!("exported {}", path.display()),
                    Err(e) => format!("export failed: {e}"),
                };
                this.update(cx, |app, cx| {
                    app.status = message.into();
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    // ---- FEFF10 generation --------------------------------------------------

    /// Read the crystal form and generate a feff.inp workspace (Atoms-lite).
    fn generate_feff_inp(&mut self, cx: &mut Context<Self>) {
        let text = |key: FeffFormKey| -> String {
            self.feff_form
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, f)| f.read(cx).text().trim().to_string())
                .unwrap_or_default()
        };
        let opt = |s: String| if s.is_empty() { None } else { Some(s) };
        let spec = crate::feffgen::CrystalSpec {
            element: text(FeffFormKey::Element),
            element2: opt(text(FeffFormKey::Element2)),
            structure: text(FeffFormKey::Structure),
            a: text(FeffFormKey::LatticeA).parse().unwrap_or(0.0),
            c: text(FeffFormKey::LatticeC).parse().ok(),
            edge: {
                let e = text(FeffFormKey::Edge);
                if e.is_empty() { "K".into() } else { e }
            },
            rmax: text(FeffFormKey::Rmax).parse().unwrap_or(5.0),
        };
        match crate::feffgen::new_workspace_from_spec(&spec) {
            Ok(dir) => {
                self.status = format!(
                    "generated {}/feff.inp — Run FEFF10 (or edit it first)",
                    dir.display()
                )
                .into();
                self.feff_workspace = Some(dir);
            }
            Err(e) => self.status = format!("feff.inp generation failed: {e}").into(),
        }
        cx.notify();
    }

    /// Create a template feff.inp workspace and open it in the system editor.
    fn new_feff_inp(&mut self, cx: &mut Context<Self>) {
        match crate::feffgen::new_workspace() {
            Ok(dir) => {
                let inp = dir.join("feff.inp");
                let _ = std::process::Command::new("open").arg("-t").arg(&inp).spawn();
                self.status = format!("feff.inp template at {} — edit, then Run FEFF10", inp.display()).into();
                self.feff_workspace = Some(dir);
            }
            Err(e) => self.status = format!("failed to create feff workspace: {e}").into(),
        }
        cx.notify();
    }

    fn choose_feff_inp(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(file) = paths.first()
                && let Some(dir) = file.parent()
            {
                let dir = dir.to_path_buf();
                this.update(cx, |app, cx| {
                    app.status = format!("FEFF workspace: {}", dir.display()).into();
                    app.feff_workspace = Some(dir);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Run FEFF10 on the workspace's feff.inp and import the generated paths.
    fn run_feff10_now(&mut self, cx: &mut Context<Self>) {
        if self.feff_running {
            return;
        }
        let Some(workspace) = self.feff_workspace.clone() else {
            self.status = "no FEFF workspace — New feff.inp... or Choose feff.inp...".into();
            cx.notify();
            return;
        };
        self.feff_gen += 1;
        let generation = self.feff_gen;
        self.feff_running = true;
        self.status = "running FEFF10 ...".into();
        cx.notify();
        let job = cx
            .background_executor()
            .spawn(async move { crate::feffgen::run_feff10_subprocess(&workspace) });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                if app.feff_gen != generation {
                    return;
                }
                app.feff_running = false;
                match result {
                    Ok(mut paths) => {
                        // Athena-style ordering: nearest shells first.
                        paths.sort_by(|a, b| {
                            let ra = path_meta(a).map(|m| m.reff).unwrap_or(f64::MAX);
                            let rb = path_meta(b).map(|m| m.reff).unwrap_or(f64::MAX);
                            ra.total_cmp(&rb)
                        });
                        let n = paths.len();
                        for (i, file) in paths.into_iter().enumerate() {
                            app.push_fit_path(file, cx);
                            // Long path lists: keep only the first few enabled.
                            if i >= 3 && let Some(row) = app.fit_paths.last_mut() {
                                row.spec.enabled = false;
                            }
                        }
                        app.status = format!("FEFF10 done — imported {n} paths").into();
                    }
                    Err(e) => {
                        app.status = format!("FEFF10 failed: {e}").into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---- project persistence -------------------------------------------------

    fn project_file(&self) -> ProjectFile {
        ProjectFile {
            version: PROJECT_VERSION,
            source_dir: self.source_dir.clone(),
            params: self.params.clone(),
            fit_paths: self.fit_paths.iter().map(|r| r.spec.clone()).collect(),
            fit_vars: self.fit_vars.iter().map(|v| v.spec.clone()).collect(),
            fit_ranges: self.fit_ranges,
            feff_workspace: self.feff_workspace.clone(),
            derived: self.derived.clone(),
        }
    }

    fn save_project(&mut self, cx: &mut Context<Self>) {
        let project = self.project_file();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let rx = cx.prompt_for_new_path(std::path::Path::new(&home), Some("project.xtproj"));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                let message = match crate::project::save(&path, &project) {
                    Ok(()) => format!("saved {}", path.display()),
                    Err(e) => format!("save failed: {e}"),
                };
                this.update(cx, |app, cx| {
                    app.status = message.into();
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn open_project(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.first()
            {
                match crate::project::load(path) {
                    Ok(project) => {
                        this.update(cx, |app, cx| app.apply_project(project, cx)).ok();
                    }
                    Err(e) => {
                        this.update(cx, |app, cx| {
                            app.status = format!("open failed: {e}").into();
                            cx.notify();
                        })
                        .ok();
                    }
                }
            }
        })
        .detach();
    }

    fn apply_project(&mut self, project: ProjectFile, cx: &mut Context<Self>) {
        // Pipeline params + field texts.
        self.params = project.params;
        let value_of = |key: ParamKey, p: &PipelineParams| match key {
            ParamKey::ImpEnergyCol => Some(p.import.energy_col as f64),
            ParamKey::ImpI0Col => Some(p.import.i0_col as f64),
            ParamKey::ImpItCol => Some(p.import.it_col as f64),
            ParamKey::ImpIrCol => Some(p.import.ir_col as f64),
            ParamKey::AlignTarget => p.align_target,
            ParamKey::E0 => p.e0,
            ParamKey::PreEdgeStart => p.pre_edge_start,
            ParamKey::PreEdgeEnd => p.pre_edge_end,
            ParamKey::NormStart => p.norm_start,
            ParamKey::NormEnd => p.norm_end,
            ParamKey::NormPolyorder => p.norm_polyorder.map(|v| v as f64),
            ParamKey::NVictoreen => p.n_victoreen.map(|v| v as f64),
            ParamKey::Rbkg => p.rbkg,
            ParamKey::BkgKmin => p.bkg_kmin,
            ParamKey::BkgKmax => p.bkg_kmax,
            ParamKey::BkgKstep => p.bkg_kstep,
            ParamKey::BkgNknots => p.bkg_nknots.map(|v| v as f64),
            ParamKey::BkgKweight => p.bkg_kweight.map(|v| v as f64),
            ParamKey::BkgClampLo => p.bkg_clamp_lo.map(|v| v as f64),
            ParamKey::BkgClampHi => p.bkg_clamp_hi.map(|v| v as f64),
            ParamKey::BkgDk => p.bkg_dk,
            ParamKey::BkgNfft => p.bkg_nfft.map(|v| v as f64),
            ParamKey::FftKmin => p.fft_kmin,
            ParamKey::FftKmax => p.fft_kmax,
            ParamKey::FftDk => p.fft_dk,
            ParamKey::FftKweight => p.fft_kweight,
            ParamKey::FftDk2 => p.fft_dk2,
            ParamKey::FftRmax => p.fft_rmax,
            ParamKey::FftKstep => p.fft_kstep,
            ParamKey::FftNfft => p.fft_nfft.map(|v| v as f64),
        };
        let params = self.params.clone();
        for (key, field) in &self.param_fields {
            let value = value_of(*key, &params);
            field.update(cx, |f, cx| f.set_value(value, cx));
        }

        // Fit model: rebuild rows, variables, ranges.
        self.fit_paths.clear();
        self.fit_vars.clear();
        self.fit_result = None;
        self.fit_plots = None;
        self.batch_fit = None;
        for spec in project.fit_paths {
            self.add_path_row(spec, cx);
        }
        // add_path_row's expression scan creates vars with defaults; restore
        // the saved values/flags on top.
        for saved in project.fit_vars {
            self.ensure_fit_var(&saved.name, saved.value, cx);
            if let Some(var) = self.fit_vars.iter_mut().find(|v| v.spec.name == saved.name) {
                var.spec = saved.clone();
                let text = match &saved.expr {
                    Some(expr) => expr.clone(),
                    None => format!("{}", saved.value),
                };
                var.field.update(cx, |f, cx| f.set_text(text, cx));
            }
        }
        self.fit_ranges = project.fit_ranges;
        let ranges = self.fit_ranges;
        for (key, field) in &self.fit_range_fields {
            let value = match key {
                RangeKey::Kmin => ranges.kmin,
                RangeKey::Kmax => ranges.kmax,
                RangeKey::Rmin => ranges.rmin,
                RangeKey::Rmax => ranges.rmax,
                RangeKey::Kweight => ranges.kweight,
            };
            field.update(cx, |f, cx| f.set_value(Some(value), cx));
        }
        self.feff_workspace = project.feff_workspace;
        self.derived = project.derived;

        // Reopen the data source.
        self.operando = None;
        self.operando_plots = None;
        self.active_scan = None;
        if let Some(dir) = project.source_dir.clone() {
            self.catalog = Catalog::default();
            self.selected = None;
            self.scan_folder(dir, cx);
        }
        self.status = "project loaded".into();
        self.schedule_recompute(cx);
        cx.notify();
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
        let active = self.selected;
        let selection = self.selection.clone();
        let filtered = self.filtered.clone();
        let count = filtered
            .as_ref()
            .map(|f| f.len())
            .unwrap_or(self.catalog.len());
        uniform_list(
            "catalog-files",
            count,
            move |range, _window, app| {
                let mut rows = Vec::with_capacity(range.len());
                for row in range {
                    let ix = match &filtered {
                        Some(f) => f[row],
                        None => row,
                    };
                    let name: SharedString = entity.read(app).catalog.name(ix).to_string().into();
                    let is_active = active == Some(ix);
                    let in_set = selection.contains(&ix);
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
                            .when(is_active, |d| d.bg(t.raised).text_color(t.accent))
                            .when(!is_active && in_set, |d| d.bg(t.raised).text_color(t.text))
                            .when(!is_active && !in_set, |d| d.text_color(t.text))
                            .hover(|d| d.bg(t.raised))
                            .cursor_pointer()
                            .on_click(move |ev: &ClickEvent, window, app| {
                                let modifiers = ev.modifiers();
                                let focus = entity.read(app).data_focus.clone();
                                window.focus(&focus, app);
                                entity.update(app, |this, cx| {
                                    this.click_entry(ix, modifiers, cx)
                                });
                            })
                            .child(name),
                    );
                }
                rows
            },
        )
        .flex_1()
    }

    fn scan_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let entity = cx.entity();
        let active = self.active_scan;
        uniform_list(
            "catalog-scans",
            self.catalog.scans.len(),
            move |range, _window, app| {
                let mut rows = Vec::with_capacity(range.len());
                for ix in range {
                    let (label, count): (SharedString, usize) = {
                        let scan = &entity.read(app).catalog.scans[ix];
                        (format!("{} · {}", scan.label, scan.len).into(), scan.len)
                    };
                    let _ = count;
                    let is_active = active == Some(ix);
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
                            .when(is_active, |d| d.bg(t.raised).text_color(t.accent))
                            .when(!is_active, |d| d.text_color(t.text))
                            .hover(|d| d.bg(t.raised))
                            .cursor_pointer()
                            .on_click(move |ev: &ClickEvent, _window, app| {
                                let modifiers = ev.modifiers();
                                entity.update(app, |this, cx| {
                                    if modifiers.shift || modifiers.platform {
                                        this.select_scan_range(ix, cx);
                                    } else {
                                        this.open_scan(ix, cx);
                                    }
                                });
                            })
                            .child(label),
                    );
                }
                rows
            },
        )
        .flex_1()
    }

    fn data_tab_button(
        &self,
        id: &'static str,
        label: &'static str,
        tab: DataTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let active = self.data_tab == tab;
        div()
            .id(id)
            .flex_1()
            .py_1()
            .flex()
            .justify_center()
            .text_xs()
            .cursor_pointer()
            .when(active, |d| {
                d.text_color(t.accent).border_b_2().border_color(t.accent)
            })
            .when(!active, |d| d.text_color(t.text_muted))
            .hover(|d| d.bg(t.raised))
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                this.data_tab = tab;
                cx.notify();
            }))
            .child(label)
    }

    fn data_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let footer: SharedString = if self.catalog.is_empty() && !self.catalog.scanning {
            "no folder open".into()
        } else if let Some(filtered) = &self.filtered {
            format!("{} of {} files", filtered.len(), self.catalog.len()).into()
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
            .id("data-panel")
            .key_context("DataPanel")
            .track_focus(&self.data_focus)
            .on_action(cx.listener(|this: &mut Self, _: &NavUp, _window, cx| {
                this.nav_move(-1, false, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &NavDown, _window, cx| {
                this.nav_move(1, false, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &NavExtendUp, _window, cx| {
                this.nav_move(-1, true, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &NavExtendDown, _window, cx| {
                this.nav_move(1, true, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &ClearCompare, _window, cx| {
                this.clear_selection(cx);
            }))
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
            .child(
                div().px_2().pb_1().children(self.filter_input.clone()),
            )
            .child(
                div()
                    .flex()
                    .border_b_1()
                    .border_color(t.border)
                    .child(self.data_tab_button("tab-files", "Files", DataTab::Files, cx))
                    .child(self.data_tab_button("tab-scans", "Scans", DataTab::Scans, cx)),
            )
            .child(if self.catalog.is_empty() {
                div().into_any_element()
            } else {
                let cmd = |id: &'static str,
                           label: &'static str,
                           enabled: bool,
                           action: fn(&mut Self, &mut Context<Self>)| {
                    div()
                        .id(id)
                        .px_1()
                        .rounded_sm()
                        .text_xs()
                        .text_color(if enabled { t.accent } else { t.text_muted })
                        .when(enabled, |d| d.cursor_pointer().hover(|d| d.bg(t.raised)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                            action(this, cx);
                        }))
                        .child(label)
                };
                div()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(t.border)
                    .child(cmd("sel-scan", "scan", self.selected.is_some(), |this, cx| {
                        this.select_active_scan(cx)
                    }))
                    .child(cmd(
                        "sel-tenth",
                        "1/10th",
                        self.selection.len() > 1,
                        |this, cx| this.thin_selection(cx),
                    ))
                    .child(cmd(
                        "sel-merge",
                        "merge",
                        self.selection.iter().filter(|&&ix| ix < DERIVED_BASE).count() >= 2,
                        |this, cx| this.merge_selection(cx),
                    ))
                    .child(cmd(
                        "sel-filter",
                        "filter→sel",
                        self.filtered.is_some(),
                        |this, cx| this.select_filter_results(cx),
                    ))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(if self.selection.is_empty() {
                                t.text_muted
                            } else {
                                t.accent
                            })
                            .child(SharedString::from(format!("{}", self.selection.len()))),
                    )
                    .child(cmd(
                        "sel-clear",
                        "clear",
                        !self.selection.is_empty(),
                        |this, cx| this.clear_selection(cx),
                    ))
                    .into_any_element()
            })
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
                let list = match self.data_tab {
                    DataTab::Files => self.file_list(cx).into_any_element(),
                    DataTab::Scans => self.scan_list(cx).into_any_element(),
                };
                let mut column = div().flex_1().min_h_0().flex().flex_col();
                if !self.derived.is_empty() {
                    let mut block = div().flex().flex_col().border_b_1().border_color(t.border);
                    for (i, d) in self.derived.iter().enumerate() {
                        let ix = DERIVED_BASE + i;
                        let is_active = self.selected == Some(ix);
                        let in_set = self.selection.contains(&ix);
                        let label: SharedString = d.label.clone().into();
                        block = block.child(
                            div()
                                .h(px(24.))
                                .px_3()
                                .flex()
                                .items_center()
                                .gap_1()
                                .when(is_active, |d| d.bg(t.raised))
                                .hover(|d| d.bg(t.raised))
                                .child(
                                    div()
                                        .id(("derived", i))
                                        .flex_1()
                                        .overflow_hidden()
                                        .text_sm()
                                        .text_color(if is_active {
                                            t.accent
                                        } else if in_set {
                                            t.text
                                        } else {
                                            t.warn
                                        })
                                        .cursor_pointer()
                                        .on_click(cx.listener(
                                            move |this, ev: &ClickEvent, _window, cx| {
                                                this.click_entry(ix, ev.modifiers(), cx);
                                            },
                                        ))
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .id(("derived-x", i))
                                        .px_1()
                                        .text_xs()
                                        .text_color(t.text_muted)
                                        .cursor_pointer()
                                        .hover(|d| d.text_color(t.error))
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _window, cx| {
                                                this.remove_derived(i, cx);
                                            },
                                        ))
                                        .child("✕"),
                                ),
                        );
                    }
                    column = column.child(block);
                }
                column.child(list).into_any_element()
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
            .child({
                let mut header = div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pr_2()
                    .child(
                        div()
                            .id(SharedString::from(format!("quad-{index}")))
                            .flex_1()
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
                    );
                // Per-plot diagnostics live on the quadrant they affect.
                if index == 0 {
                    header = header
                        .child(self.view_chip("view-pre", "pre", self.view.show_pre, cx, |this, cx| {
                            this.view.show_pre = !this.view.show_pre;
                            this.rebuild_plots(cx);
                            cx.notify();
                        }))
                        .child(self.view_chip(
                            "view-post",
                            "post",
                            self.view.show_post,
                            cx,
                            |this, cx| {
                                this.view.show_post = !this.view.show_post;
                                this.rebuild_plots(cx);
                                cx.notify();
                            },
                        ))
                        .child(self.view_chip("view-e0", "E0", self.view.show_e0, cx, |this, cx| {
                            this.view.show_e0 = !this.view.show_e0;
                            this.rebuild_plots(cx);
                            cx.notify();
                        }))
                        .child(self.view_chip(
                            "view-ranges",
                            "ranges",
                            self.view.show_ranges,
                            cx,
                            |this, cx| {
                                this.view.show_ranges = !this.view.show_ranges;
                                this.rebuild_plots(cx);
                                cx.notify();
                            },
                        ));
                } else if index == 2 {
                    header = header
                        .child(self.view_chip(
                            "view-krange",
                            "FT range",
                            self.view.show_krange,
                            cx,
                            |this, cx| {
                                this.view.show_krange = !this.view.show_krange;
                                this.rebuild_plots(cx);
                                cx.notify();
                            },
                        ))
                        .child(self.view_chip(
                            "view-kwin",
                            "FT window",
                            self.view.show_kwin,
                            cx,
                            |this, cx| {
                                this.view.show_kwin = !this.view.show_kwin;
                                this.rebuild_plots(cx);
                                cx.notify();
                            },
                        ));
                } else if index == 1 {
                    header = header
                        .child(self.view_chip(
                            "view-flat",
                            if self.view.flat { "flat" } else { "norm" },
                            !self.view.flat,
                            cx,
                            |this, cx| {
                                this.view.flat = !this.view.flat;
                                this.rebuild_plots(cx);
                                cx.notify();
                            },
                        ))
                        .child(self.view_chip(
                            "view-e0-norm",
                            "E0",
                            self.view.show_e0,
                            cx,
                            |this, cx| {
                                this.view.show_e0 = !this.view.show_e0;
                                this.rebuild_plots(cx);
                                cx.notify();
                            },
                        ));
                }
                header
            })
            .child(div().flex_1().p_1().child(plot.clone()))
    }

    fn view_chip(
        &self,
        id: &'static str,
        label: &'static str,
        on: bool,
        cx: &mut Context<Self>,
        action: fn(&mut Self, &mut Context<Self>),
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        div()
            .id(id)
            .px_2()
            .rounded_sm()
            .text_xs()
            .cursor_pointer()
            .text_color(if on { t.accent } else { t.text_muted })
            .hover(|d| d.bg(t.raised))
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                action(this, cx);
            }))
            .child(label)
    }

    fn view_options_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let waterfall = self.view.layout == TraceLayout::Waterfall;
        let mut row = div()
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .text_color(t.text_muted)
            .child(self.view_chip(
                "view-layout",
                if waterfall { "waterfall" } else { "overlay" },
                waterfall,
                cx,
                |this, cx| {
                    this.view.layout = match this.view.layout {
                        TraceLayout::Overlay => TraceLayout::Waterfall,
                        TraceLayout::Waterfall => TraceLayout::Overlay,
                    };
                    this.rebuild_plots(cx);
                    cx.notify();
                },
            ));
        if waterfall && let Some(field) = &self.view_offset_field {
            row = row.child(div().w(px(72.)).child(field.clone()));
        }
        row = row
            .child(self.view_chip("view-legend", "legend", self.view.legend, cx, |this, cx| {
                this.view.legend = !this.view.legend;
                this.rebuild_plots(cx);
                cx.notify();
            }))
            .child(self.view_chip("view-grid", "grid", self.view.grid, cx, |this, cx| {
                this.view.grid = !this.view.grid;
                this.rebuild_plots(cx);
                cx.notify();
            }));
        row
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
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .child(self.view_options_row(cx))
                .child(div().flex_1().flex().child(self.quadrant(index, cx)));
        }
        div()
            .flex_1()
            .flex()
            .flex_col()
            .child(self.view_options_row(cx))
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

    /// Scrub strip: clickable segments mapping linearly onto frames — the
    /// "heatmap is the scrollbar" control until plot-click navigation lands.
    fn time_scrubber(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        const SEGMENTS: usize = 96;
        let t = self.theme;
        let frames = self
            .operando
            .as_ref()
            .map(|d| d.matrix.len())
            .unwrap_or(0)
            .max(1);
        let active_seg = if frames > 1 {
            self.time_pos * (SEGMENTS - 1) / (frames - 1)
        } else {
            0
        };
        let mut strip = div().flex().w_full().h(px(16.)).gap(px(1.)).px_1();
        for seg in 0..SEGMENTS {
            let frame = if SEGMENTS > 1 {
                seg * (frames - 1) / (SEGMENTS - 1)
            } else {
                0
            };
            strip = strip.child(
                div()
                    .id(("scrub", seg))
                    .flex_1()
                    .h_full()
                    .rounded_xs()
                    .bg(if seg == active_seg { t.accent } else { t.raised })
                    .hover(|d| d.bg(t.text_muted))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        this.set_time_pos(frame, cx);
                    })),
            );
        }
        strip
    }

    fn operando_center(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let Some(plots) = &self.operando_plots else {
            let hint: SharedString = if self.active_scan.is_some() {
                self.status.clone()
            } else if self.catalog.scans.is_empty() {
                "Open a folder, then pick a scan in the Scans tab".into()
            } else {
                "Pick a scan in the Scans tab".into()
            };
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(t.text_muted)
                .child(hint)
                .into_any_element();
        };
        let frames = self.operando.as_ref().map(|d| d.matrix.len()).unwrap_or(0);
        let frame_label: SharedString = format!("frame {} / {frames}", self.time_pos + 1).into();
        div()
            .id("operando-center")
            .key_context("Operando")
            .track_focus(&self.operando_focus)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _ev, window, cx| {
                    let handle = this.operando_focus.clone();
                    window.focus(&handle, cx);
                }),
            )
            .on_action(cx.listener(|this: &mut Self, _: &FramePrev, _window, cx| {
                this.step_time(-1, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &FrameNext, _window, cx| {
                this.step_time(1, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &FrameJumpBack, _window, cx| {
                this.step_time(-10, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &FrameJumpFwd, _window, cx| {
                this.step_time(10, cx);
            }))
            .flex_1()
            .flex()
            .child(
                // Left: heatmap overview + scrubber.
                div()
                    .flex_1()
                    .min_w_0()
                    .m_1()
                    .flex()
                    .flex_col()
                    .rounded_md()
                    .bg(t.raised)
                    .border_1()
                    .border_color(t.border)
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_xs()
                            .text_color(t.text_muted)
                            .child("scan overview · k²χ(k) vs time"),
                    )
                    .child(div().flex_1().p_1().child(plots.heatmap.clone()))
                    .child(self.time_scrubber(cx))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_xs()
                            .text_color(t.text_muted)
                            .child(frame_label),
                    ),
            )
            .child(
                // Right: frame chi(k) + E0 trend.
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_1()
                            .m_1()
                            .rounded_md()
                            .bg(t.raised)
                            .border_1()
                            .border_color(t.border)
                            .p_1()
                            .child(plots.chik.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .m_1()
                            .rounded_md()
                            .bg(t.raised)
                            .border_1()
                            .border_color(t.border)
                            .p_1()
                            .child(plots.trend.clone()),
                    ),
            )
            .into_any_element()
    }

    fn fit_center(&self, _cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let Some((k_plot, r_plot)) = &self.fit_plots else {
            let hint: SharedString = if self.fit_paths.is_empty() {
                "Add a FEFF path (.dat) in the panel, then Run Fit".into()
            } else if self.fit_running {
                "fitting ...".into()
            } else {
                "Run Fit to see data vs model".into()
            };
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(t.text_muted)
                .child(hint)
                .into_any_element();
        };
        let card = |title: &'static str, plot: Entity<RuvizPlot>| {
            div()
                .flex_1()
                .min_h_0()
                .m_1()
                .flex()
                .flex_col()
                .rounded_md()
                .bg(t.raised)
                .border_1()
                .border_color(t.border)
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child(title),
                )
                .child(div().flex_1().p_1().child(plot))
        };
        div()
            .flex_1()
            .flex()
            .flex_col()
            .child(card("fit in k-space", k_plot.clone()))
            .child(card("fit in R-space", r_plot.clone()))
            .into_any_element()
    }

    fn fit_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut panel = div()
            .id("fit-scroll")
            .on_scroll_wheel(cx.listener(|_t, ev: &gpui::ScrollWheelEvent, _w, _cx| {
                eprintln!("[scroll-dbg] fit-scroll got wheel: {:?}", ev.delta);
            }))
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_y_scroll();

        // Paths.
        panel = panel.child(
            div()
                .px_3()
                .pt_3()
                .pb_1()
                .flex()
                .justify_between()
                .child(div().text_xs().text_color(t.accent).child("FEFF paths"))
                .child(
                    div()
                        .id("add-path")
                        .px_2()
                        .rounded_sm()
                        .text_xs()
                        .text_color(t.accent)
                        .cursor_pointer()
                        .hover(|d| d.bg(t.raised))
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.add_fit_path_dialog(cx);
                        }))
                        .child("Add Path..."),
                ),
        );
        if self.fit_paths.is_empty() {
            panel = panel.child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child("no paths imported"),
            );
        }
        for (i, row) in self.fit_paths.iter().enumerate() {
            let label: SharedString = row.spec.label.clone().into();
            let enabled = row.spec.enabled;
            let expanded = row.expanded;
            let meta_line: SharedString = match &row.meta {
                Some(m) => format!("R {:.3} Å · deg {:.0} · {} legs", m.reff, m.degen, m.nleg).into(),
                None => "".into(),
            };
            let mut path_card = div().px_2().flex().flex_col().child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        // enable/disable toggle
                        div()
                            .id(("fit-path-en", i))
                            .px_1()
                            .text_sm()
                            .cursor_pointer()
                            .text_color(if enabled { t.success } else { t.text_muted })
                            .hover(|d| d.bg(t.raised))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                if let Some(p) = this.fit_paths.get_mut(i) {
                                    p.spec.enabled = !p.spec.enabled;
                                    cx.notify();
                                }
                            }))
                            .child(if enabled { "✓" } else { "✗" }),
                    )
                    .child(
                        // header: click to expand/collapse the editor
                        div()
                            .id(("fit-path", i))
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .overflow_hidden()
                            .cursor_pointer()
                            .hover(|d| d.bg(t.raised))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                if let Some(p) = this.fit_paths.get_mut(i) {
                                    p.expanded = !p.expanded;
                                    cx.notify();
                                }
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(if enabled { t.text } else { t.text_muted })
                                    .child(label),
                            )
                            .child(div().flex_1())
                            .child(div().text_xs().text_color(t.text_muted).child(meta_line)),
                    ),
            );
            if expanded {
                for (param, field) in &row.fields {
                    let cell_label = match param {
                        PathParam::S02 => "s02",
                        PathParam::E0 => "e0",
                        PathParam::Sigma2 => "σ²",
                        PathParam::DeltaR => "Δr",
                    };
                    path_card = path_card.child(
                        div()
                            .pl_4()
                            .py_0p5()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w(px(28.))
                                    .text_xs()
                                    .text_color(t.text_muted)
                                    .child(cell_label),
                            )
                            .child(div().flex_1().child(field.clone())),
                    );
                }
            }
            panel = panel.child(path_card);
        }

        // FEFF10 generation.
        panel = panel.child(self.section_header("FEFF10"));
        let feff_button = |id: &'static str, label: SharedString| {
            div()
                .id(id)
                .px_2()
                .py_0p5()
                .rounded_sm()
                .text_xs()
                .text_color(t.accent)
                .cursor_pointer()
                .hover(|d| d.bg(t.raised))
                .child(label)
        };
        // Crystal form (Atoms-lite).
        for (key, field) in &self.feff_form {
            let label = match key {
                FeffFormKey::Element => "element",
                FeffFormKey::Element2 => "element 2",
                FeffFormKey::Structure => "structure",
                FeffFormKey::LatticeA => "a (Å)",
                FeffFormKey::LatticeC => "c (Å)",
                FeffFormKey::Edge => "edge",
                FeffFormKey::Rmax => "rmax (Å)",
            };
            panel = panel.child(
                div()
                    .px_3()
                    .py_0p5()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(72.)).text_xs().text_color(t.text_muted).child(label))
                    .child(div().flex_1().child(field.clone())),
            );
        }
        panel = panel.child(
            div().px_3().py_1().child(
                div()
                    .id("feff-generate")
                    .w_full()
                    .py_1()
                    .rounded_md()
                    .flex()
                    .justify_center()
                    .text_xs()
                    .bg(t.raised)
                    .text_color(t.accent)
                    .cursor_pointer()
                    .hover(|d| d.bg(t.border))
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.generate_feff_inp(cx);
                    }))
                    .child("Generate feff.inp from structure"),
            ),
        );
        panel = panel.child(
            div()
                .px_3()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(feff_button("feff-new", "New feff.inp...".into()).on_click(
                            cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.new_feff_inp(cx);
                            }),
                        ))
                        .child(feff_button("feff-choose", "Choose feff.inp...".into()).on_click(
                            cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.choose_feff_inp(cx);
                            }),
                        ))
                        .child(
                            feff_button(
                                "feff-run",
                                if self.feff_running {
                                    "running...".into()
                                } else {
                                    "Run FEFF10".into()
                                },
                            )
                            .on_click(cx.listener(
                                |this, _: &ClickEvent, _window, cx| {
                                    this.run_feff10_now(cx);
                                },
                            )),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child(SharedString::from(match &self.feff_workspace {
                            Some(ws) => format!("workspace: {}", ws.display()),
                            None => "no workspace".to_string(),
                        })),
                ),
        );

        // Variables.
        panel = panel.child(self.section_header("Variables"));
        for var in &self.fit_vars {
            let name: SharedString = var.spec.name.clone().into();
            let vary = var.spec.vary;
            let is_expr = var.spec.expr.is_some();
            let var_name = var.spec.name.clone();
            let badge: &'static str = if is_expr {
                "expr"
            } else if vary {
                "vary"
            } else {
                "fixed"
            };
            panel = panel.child(
                div()
                    .px_3()
                    .py_0p5()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(56.)).text_sm().text_color(t.text_muted).child(name))
                    .child(div().flex_1().child(var.field.clone()))
                    .child(
                        div()
                            .id(SharedString::from(format!("vary-{var_name}")))
                            .px_1()
                            .rounded_sm()
                            .text_xs()
                            .cursor_pointer()
                            .text_color(if is_expr {
                                t.warn
                            } else if vary {
                                t.accent
                            } else {
                                t.text_muted
                            })
                            .hover(|d| d.bg(t.raised))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                if let Some(v) = this
                                    .fit_vars
                                    .iter_mut()
                                    .find(|v| v.spec.name == var_name)
                                {
                                    if v.spec.expr.is_some() {
                                        // expr -> back to a plain varying value
                                        v.spec.expr = None;
                                        v.spec.vary = true;
                                    } else {
                                        v.spec.vary = !v.spec.vary;
                                    }
                                    cx.notify();
                                }
                            }))
                            .child(badge),
                    ),
            );
        }

        // Ranges + run.
        panel = panel.child(self.section_header("Fit ranges"));
        for (_, field) in &self.fit_range_fields {
            panel = panel.child(field.clone());
        }
        panel = panel.child(
            div().px_3().py_2().child(
                div()
                    .id("run-fit")
                    .w_full()
                    .py_1()
                    .rounded_md()
                    .flex()
                    .justify_center()
                    .text_sm()
                    .bg(t.accent)
                    .text_color(t.bg)
                    .cursor_pointer()
                    .hover(|d| d.bg(t.text))
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.run_fit_now(cx);
                    }))
                    .child(if self.fit_running { "fitting ..." } else { "Run Fit" }),
            ),
        );

        // Batch over the active scan.
        panel = panel.child(self.section_header("Batch"));
        let batch_label: SharedString = if self.batch_running {
            let (done, total) = self.batch_progress;
            format!("fitting {done}/{total} ...").into()
        } else if let Some(data) = &self.operando {
            format!("Batch fit scan ({} frames)", data.sample_ixs.len()).into()
        } else {
            "Batch fit scan (open a scan first)".into()
        };
        panel = panel.child(
            div().px_3().pb_1().flex().flex_col().gap_1().child(
                div()
                    .id("batch-fit")
                    .w_full()
                    .py_1()
                    .rounded_md()
                    .flex()
                    .justify_center()
                    .text_xs()
                    .bg(t.raised)
                    .text_color(t.accent)
                    .cursor_pointer()
                    .hover(|d| d.bg(t.border))
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.run_batch_fit(cx);
                    }))
                    .child(batch_label),
            ),
        );
        if let Some(bf) = &self.batch_fit {
            let summary: SharedString =
                format!("{} frames fitted · trend: {}", bf.rows.len(),
                    bf.varying_names.get(bf.trend_param).cloned().unwrap_or_default()).into();
            panel = panel.child(
                div()
                    .px_3()
                    .pb_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().text_xs().text_color(t.text_muted).child(summary))
                    .child(
                        div()
                            .id("trend-cycle")
                            .px_1()
                            .rounded_sm()
                            .text_xs()
                            .text_color(t.accent)
                            .cursor_pointer()
                            .hover(|d| d.bg(t.raised))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.cycle_trend_param(cx);
                            }))
                            .child("next param"),
                    )
                    .child(
                        div()
                            .id("batch-csv")
                            .px_1()
                            .rounded_sm()
                            .text_xs()
                            .text_color(t.accent)
                            .cursor_pointer()
                            .hover(|d| d.bg(t.raised))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.export_batch_csv(cx);
                            }))
                            .child("CSV..."),
                    ),
            );
        }

        // Results.
        if let Some(result) = &self.fit_result {
            panel = panel.child(self.section_header("Results"));
            for line in result_summary(result) {
                panel = panel.child(
                    div()
                        .px_3()
                        .py_0p5()
                        .text_xs()
                        .text_color(t.text)
                        .child(SharedString::from(line)),
                );
            }
        }

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
                    .child("FIT MODEL"),
            )
            .child(panel)
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

    fn context_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_y_scroll();

        // ---- Import (configure once, applies to the whole catalog) ----
        {
            let open = self.adv_open[3];
            sections = sections.child(
                div()
                    .px_3()
                    .pt_3()
                    .pb_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_xs().text_color(t.accent).child("Import"))
                    .child(
                        div()
                            .id("adv-import")
                            .px_1()
                            .rounded_sm()
                            .text_xs()
                            .text_color(if open { t.accent } else { t.text_muted })
                            .cursor_pointer()
                            .hover(|d| d.bg(t.raised))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.adv_open[3] = !this.adv_open[3];
                                cx.notify();
                            }))
                            .child(if open { "▾ less" } else { "▸ columns" }),
                    ),
            );
            // mode selector (always visible)
            let options = Self::enum_options(EnumParam::ImportMode);
            let selected = self.enum_selected_index(EnumParam::ImportMode);
            let expanded = self.open_enum == Some(EnumParam::ImportMode);
            let current: SharedString = options[selected].clone().into();
            sections = sections.child(
                div()
                    .px_3()
                    .py_0p5()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().text_sm().text_color(t.text_muted).child("mode"))
                    .child(
                        div()
                            .id("enum-import-mode")
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .text_xs()
                            .bg(t.bg)
                            .border_1()
                            .border_color(if expanded { t.accent } else { t.border })
                            .text_color(t.text)
                            .cursor_pointer()
                            .hover(|d| d.border_color(t.accent))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.open_enum = if this.open_enum == Some(EnumParam::ImportMode) {
                                    None
                                } else {
                                    Some(EnumParam::ImportMode)
                                };
                                cx.notify();
                            }))
                            .child(format!("{current} ▾")),
                    ),
            );
            if expanded {
                let mut list = div()
                    .mx_3()
                    .mb_1()
                    .rounded_sm()
                    .border_1()
                    .border_color(t.border)
                    .bg(t.bg)
                    .flex()
                    .flex_col();
                for (i, option) in options.iter().enumerate() {
                    let option: SharedString = option.clone().into();
                    let is_sel = i == selected;
                    list = list.child(
                        div()
                            .id(SharedString::from(format!("enum-opt-import-{i}")))
                            .px_2()
                            .py_0p5()
                            .text_xs()
                            .cursor_pointer()
                            .when(is_sel, |d| d.bg(t.raised).text_color(t.accent))
                            .when(!is_sel, |d| d.text_color(t.text))
                            .hover(|d| d.bg(t.raised))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                this.set_enum_param(EnumParam::ImportMode, i, cx);
                            }))
                            .child(option),
                    );
                }
                sections = sections.child(list);
            }
            // preview line (always visible)
            sections = sections.child(
                div()
                    .px_3()
                    .pb_1()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child(self.import_preview.clone()),
            );
            if open {
                for key in [
                    ParamKey::ImpEnergyCol,
                    ParamKey::ImpI0Col,
                    ParamKey::ImpItCol,
                    ParamKey::ImpIrCol,
                ] {
                    if let Some(f) = field(key) {
                        sections = sections.child(f);
                    }
                }
                if let Some(roi) = &self.roi_input {
                    sections = sections.child(
                        div()
                            .px_3()
                            .py_0p5()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(t.text_muted)
                                    .child("ROI cols"),
                            )
                            .child(div().w(px(96.)).child(roi.clone())),
                    );
                }
                let align_on = self.params.align_to_ref;
                sections = sections.child(
                    div()
                        .px_3()
                        .py_0p5()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("align-toggle")
                                .px_1()
                                .rounded_sm()
                                .text_xs()
                                .cursor_pointer()
                                .text_color(if align_on { t.accent } else { t.text_muted })
                                .hover(|d| d.bg(t.raised))
                                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                    this.params.align_to_ref = !this.params.align_to_ref;
                                    this.schedule_recompute(cx);
                                    cx.notify();
                                }))
                                .child(if align_on { "✓ align to ref" } else { "align to ref" }),
                        )
                        .child(div().flex_1()),
                );
                if let Some(f) = field(ParamKey::AlignTarget) {
                    sections = sections.child(f);
                }
            }
        }

        // (title, basic keys, advanced keys, fold index)
        let groups: [(&'static str, &[ParamKey], &[ParamKey], usize); 3] = [
            (
                "Normalization",
                &[
                    ParamKey::E0,
                    ParamKey::PreEdgeStart,
                    ParamKey::PreEdgeEnd,
                    ParamKey::NormStart,
                    ParamKey::NormEnd,
                ],
                &[ParamKey::NormPolyorder, ParamKey::NVictoreen],
                0,
            ),
            (
                "Background (AUTOBK)",
                &[ParamKey::Rbkg, ParamKey::BkgKmin, ParamKey::BkgKmax],
                &[
                    ParamKey::BkgKstep,
                    ParamKey::BkgNknots,
                    ParamKey::BkgKweight,
                    ParamKey::BkgClampLo,
                    ParamKey::BkgClampHi,
                    ParamKey::BkgDk,
                    ParamKey::BkgNfft,
                ],
                1,
            ),
            (
                "FFT",
                &[
                    ParamKey::FftKmin,
                    ParamKey::FftKmax,
                    ParamKey::FftDk,
                    ParamKey::FftKweight,
                ],
                &[ParamKey::FftDk2, ParamKey::FftRmax, ParamKey::FftKstep, ParamKey::FftNfft],
                2,
            ),
        ];
        for (title, basics, advanced, fold) in groups {
            let open = self.adv_open[fold];
            sections = sections.child(
                div()
                    .px_3()
                    .pt_3()
                    .pb_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_xs().text_color(t.accent).child(title))
                    .child(
                        div()
                            .id(SharedString::from(format!("adv-{fold}")))
                            .px_1()
                            .rounded_sm()
                            .text_xs()
                            .text_color(if open { t.accent } else { t.text_muted })
                            .cursor_pointer()
                            .hover(|d| d.bg(t.raised))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                this.adv_open[fold] = !this.adv_open[fold];
                                cx.notify();
                            }))
                            .child(if open { "▾ less" } else { "▸ more" }),
                    ),
            );
            for &key in basics {
                if let Some(f) = field(key) {
                    sections = sections.child(f);
                }
            }
            if open {
                for &key in advanced {
                    if let Some(f) = field(key) {
                        sections = sections.child(f);
                    }
                }
                let enum_rows: &[(&'static str, EnumParam)] = match fold {
                    1 => &[
                        ("window", EnumParam::BkgWindow),
                        ("solver", EnumParam::BkgSolver),
                    ],
                    2 => &[("window", EnumParam::FftWindow)],
                    _ => &[],
                };
                for &(label, which) in enum_rows {
                    let options = Self::enum_options(which);
                    let selected = self.enum_selected_index(which);
                    let expanded = self.open_enum == Some(which);
                    let current: SharedString = options[selected].clone().into();
                    sections = sections.child(
                        div()
                            .px_3()
                            .py_0p5()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(t.text_muted)
                                    .child(label),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("enum-{fold}-{label}")))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .text_xs()
                                    .bg(t.bg)
                                    .border_1()
                                    .border_color(if expanded { t.accent } else { t.border })
                                    .text_color(t.text)
                                    .cursor_pointer()
                                    .hover(|d| d.border_color(t.accent))
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, _window, cx| {
                                            this.open_enum = if this.open_enum == Some(which) {
                                                None
                                            } else {
                                                Some(which)
                                            };
                                            cx.notify();
                                        },
                                    ))
                                    .child(format!("{current} ▾")),
                            ),
                    );
                    if expanded {
                        let mut list = div()
                            .mx_3()
                            .mb_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(t.border)
                            .bg(t.bg)
                            .flex()
                            .flex_col();
                        for (i, option) in options.iter().enumerate() {
                            let option: SharedString = option.clone().into();
                            let is_sel = i == selected;
                            list = list.child(
                                div()
                                    .id(SharedString::from(format!("enum-opt-{fold}-{label}-{i}")))
                                    .px_2()
                                    .py_0p5()
                                    .text_xs()
                                    .cursor_pointer()
                                    .when(is_sel, |d| d.bg(t.raised).text_color(t.accent))
                                    .when(!is_sel, |d| d.text_color(t.text))
                                    .hover(|d| d.bg(t.raised))
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, _window, cx| {
                                            this.set_enum_param(which, i, cx);
                                        },
                                    ))
                                    .child(option),
                            );
                        }
                        sections = sections.child(list);
                    }
                }
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
                    .id("open-project")
                    .px_2()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|d| d.bg(t.raised).text_color(t.text))
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.open_project(cx);
                    }))
                    .child("open project"),
            )
            .child(
                div()
                    .id("save-project")
                    .px_2()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|d| d.bg(t.raised).text_color(t.text))
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.save_project(cx);
                    }))
                    .child("save project"),
            )
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
            Workspace::Operando => self.operando_center(cx).into_any_element(),
            Workspace::Fit => self.fit_center(cx).into_any_element(),
        };
        let context_panel = match self.workspace {
            Workspace::Fit => self.fit_panel(cx).into_any_element(),
            _ => self.context_panel(cx).into_any_element(),
        };
        div()
            .id("root")
            .on_scroll_wheel(cx.listener(|_t, ev: &gpui::ScrollWheelEvent, _w, _cx| {
                eprintln!("[scroll-dbg] window got wheel: {:?}", ev.delta);
            }))
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
                    .child(context_panel),
            )
            .child(self.status_bar(cx))
    }
}
