//! Root view: workspace shell per doc/gui-ux-design.md.
//!
//! M1 scope: lazy catalog — "Open Folder" starts a background scan that
//! streams batches into a virtualized file list; clicking an entry parses and
//! processes it on the background executor (generation-counted so stale
//! results are dropped) with an LRU cache of processed spectra. The Explore
//! center is the 2x2 quadrant grid from M0.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures::StreamExt;
use gpui::{
    ClickEvent, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding, MouseDownEvent,
    ParentElement, PathPromptOptions, Render, ScrollStrategy, SharedString, Styled,
    UniformListScrollHandle, Window, actions, canvas, div, fill, point, prelude::*, px, size,
    uniform_list,
};
use lru::LruCache;
use ruviz_gpui::{RuvizPlot, plot_builder};
use xraytsubaki::prelude::XASSpectrum;

use rayon::prelude::*;

use xraytsubaki::prelude::FeffFitResult;

use crate::catalog::{Catalog, ScanEvent, index_cache_path, load_index, start_scan, write_index};
use crate::fitting::expr_identifiers;
use crate::fitting::{
    BatchFitRow, FitPathSpec, FitRanges, FitVarSpec, PathMeta, batch_csv, path_meta,
    result_summary, run_fit,
};
use crate::params::{
    AUTOBK_SOLVERS, DerivedSpectrum, DetectionMode, FT_WINDOWS, ImportPreview, PipelineParams,
    StreamingAverage, load_raw, parse_cols, preview_import, process_arrays, process_file,
    resample_chik,
};
use crate::plotting::{
    QuadTrace, TraceLayout, ViewOptions, build_fit_k, build_fit_r, build_fit_residual_k,
    build_fit_residual_r, build_frame_chik, build_heatmap, build_quadrants_multi, build_trend,
    chik_label, middle_truncate, trace_rgba,
};
use crate::project::{PROJECT_VERSION, ParamOverride, ProjectFile};
use crate::theme::{Theme, ThemeMode};
use crate::widgets::numeric_field::{FieldEvent, FieldKind, NumericField};
use crate::widgets::text_input::{InputEvent, TextInput};

/// Processed spectra kept in RAM. ~100-300 KB each, so 1024 ≈ a few hundred MB
/// worst case; browsing a million-file catalog stays bounded.
const PROCESSED_CACHE_CAPACITY: usize = 1024;
const JOB_ERROR_CAPACITY: usize = 200;

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

/// Import role edited by a detected-column picker.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ImportRole {
    Energy,
    I0,
    It,
    Ir,
    Fluor,
    Mu,
}

/// Context-panel section a parameter belongs to, for the per-section
/// global/override chips.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ParamSection {
    Import,
    Norm,
    Bkg,
    Fft,
}

/// Copy one section's fields from `src` over `dst`. Reset-to-global is
/// `copy_section(override, global, section)`; the single field list here
/// also powers `section_differs`.
fn copy_section(dst: &mut PipelineParams, src: &PipelineParams, section: ParamSection) {
    match section {
        ParamSection::Import => {
            dst.import = src.import.clone();
            dst.align_to_ref = src.align_to_ref;
            dst.align_target = src.align_target;
        }
        ParamSection::Norm => {
            dst.e0 = src.e0;
            dst.pre_edge_start = src.pre_edge_start;
            dst.pre_edge_end = src.pre_edge_end;
            dst.norm_start = src.norm_start;
            dst.norm_end = src.norm_end;
            dst.norm_polyorder = src.norm_polyorder;
            dst.n_victoreen = src.n_victoreen;
        }
        ParamSection::Bkg => {
            dst.rbkg = src.rbkg;
            dst.bkg_kmin = src.bkg_kmin;
            dst.bkg_kmax = src.bkg_kmax;
            dst.bkg_kstep = src.bkg_kstep;
            dst.bkg_nknots = src.bkg_nknots;
            dst.bkg_kweight = src.bkg_kweight;
            dst.bkg_clamp_lo = src.bkg_clamp_lo;
            dst.bkg_clamp_hi = src.bkg_clamp_hi;
            dst.bkg_window = src.bkg_window;
            dst.bkg_dk = src.bkg_dk;
            dst.bkg_solver = src.bkg_solver;
            dst.bkg_nfft = src.bkg_nfft;
        }
        ParamSection::Fft => {
            dst.fft_kmin = src.fft_kmin;
            dst.fft_kmax = src.fft_kmax;
            dst.fft_dk = src.fft_dk;
            dst.fft_kweight = src.fft_kweight;
            dst.fft_dk2 = src.fft_dk2;
            dst.fft_rmax = src.fft_rmax;
            dst.fft_window = src.fft_window;
            dst.fft_kstep = src.fft_kstep;
            dst.fft_nfft = src.fft_nfft;
        }
    }
}

/// True when the section's fields differ between the two param sets.
fn section_differs(a: &PipelineParams, b: &PipelineParams, section: ParamSection) -> bool {
    let mut probe = a.clone();
    copy_section(&mut probe, b, section);
    probe != *a
}

/// Combined validity stamp for a scan-wide computation (operando overview,
/// batch fit): the global fingerprint plus every per-spectrum override
/// landing inside the frame range, so editing one member's params
/// invalidates exactly the overviews it feeds.
fn scan_fingerprint(
    global: &PipelineParams,
    overrides: &BTreeMap<usize, PipelineParams>,
    start: usize,
    len: usize,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    global.fingerprint().hash(&mut hasher);
    for (ix, params) in overrides.range(start..start.saturating_add(len)) {
        ix.hash(&mut hasher);
        params.fingerprint().hash(&mut hasher);
    }
    hasher.finish()
}

/// Value a context-panel numeric field should display for a param set.
fn param_field_value(key: ParamKey, p: &PipelineParams) -> Option<f64> {
    match key {
        ParamKey::ImpEnergyCol => p.import.energy_col.map(|column| column as f64),
        ParamKey::ImpI0Col => p.import.i0_col.map(|column| column as f64),
        ParamKey::ImpItCol => p.import.it_col.map(|column| column as f64),
        ParamKey::ImpIrCol => p.import.ir_col.map(|column| column as f64),
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
    }
}

const DETECTION_MODES: [DetectionMode; 4] = [
    DetectionMode::Transmission,
    DetectionMode::Fluorescence,
    DetectionMode::Reference,
    DetectionMode::MuColumn,
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
        FrameFirst,
        FrameLast,
        WorkspaceExplore,
        WorkspaceOperando,
        WorkspaceFit,
        ToggleDataPanel,
        ToggleContextPanel,
        FocusFilter,
        Maximize1,
        Maximize2,
        Maximize3,
        Maximize4,
        RestoreGrid,
        ExploreEscape,
    ]
);

/// Application key bindings; register at startup alongside the text-input
/// bindings.
///
/// GPUI treats context-free bindings as if they matched the deepest focused
/// context. The shell bindings are therefore deliberately scoped to the root
/// `Studio`/workspace key context and exclude `TextInput` where a printable or
/// editing keystroke must stay with the focused editor.
pub fn studio_keybindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("up", NavUp, Some("DataPanel")),
        KeyBinding::new("down", NavDown, Some("DataPanel")),
        KeyBinding::new("shift-up", NavExtendUp, Some("DataPanel")),
        KeyBinding::new("shift-down", NavExtendDown, Some("DataPanel")),
        KeyBinding::new(
            "escape",
            ClearCompare,
            Some("DataPanel && !Explore && !TextInput"),
        ),
        KeyBinding::new("left", FramePrev, Some("Operando && !TextInput")),
        KeyBinding::new("right", FrameNext, Some("Operando && !TextInput")),
        KeyBinding::new("shift-left", FrameJumpBack, Some("Operando && !TextInput")),
        KeyBinding::new("shift-right", FrameJumpFwd, Some("Operando && !TextInput")),
        KeyBinding::new("home", FrameFirst, Some("Operando && !TextInput")),
        KeyBinding::new("end", FrameLast, Some("Operando && !TextInput")),
        KeyBinding::new("cmd-1", WorkspaceExplore, Some("Studio")),
        KeyBinding::new("cmd-2", WorkspaceOperando, Some("Studio")),
        KeyBinding::new("cmd-3", WorkspaceFit, Some("Studio")),
        KeyBinding::new("cmd-b", ToggleDataPanel, Some("Studio && !TextInput")),
        KeyBinding::new("cmd-j", ToggleContextPanel, Some("Studio && !TextInput")),
        KeyBinding::new("cmd-p", FocusFilter, Some("Studio && !TextInput")),
        KeyBinding::new("1", Maximize1, Some("Explore && !TextInput")),
        KeyBinding::new("2", Maximize2, Some("Explore && !TextInput")),
        KeyBinding::new("3", Maximize3, Some("Explore && !TextInput")),
        KeyBinding::new("4", Maximize4, Some("Explore && !TextInput")),
        KeyBinding::new("0", RestoreGrid, Some("Explore && !TextInput")),
        KeyBinding::new("escape", ExploreEscape, Some("Explore && !TextInput")),
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

// ruviz uses matplotlib-like proportional margins by default. The heatmap
// cursor overlay uses the same approximate top/bottom fractions for both
// drawing and pointer hit mapping; this deliberately remains approximate so
// it does not depend on ruviz-gpui internals.
const HEATMAP_TOP_MARGIN: f32 = 0.12;
const HEATMAP_BOTTOM_MARGIN: f32 = 0.11;
const HEATMAP_LEFT_MARGIN: f32 = 0.125;
const HEATMAP_RIGHT_WITH_COLORBAR: f32 = 0.20;

/// Downsampled overview of one scan, valid for one params fingerprint.
struct OperandoData {
    scan: usize,
    scan_len: usize,
    fingerprint: u64,
    grid: Vec<f64>,
    matrix: Vec<Vec<f64>>,
    e0s: Vec<f64>,
    kweight: f64,
}

#[derive(Clone)]
struct FitProvenance {
    label: SharedString,
    path: PathBuf,
    params_fingerprint: u64,
    model_fingerprint: u64,
}

#[derive(Clone)]
struct JobError {
    label: String,
    message: String,
}

struct OperandoPlots {
    /// Scan the plots currently show; a different scan gets fresh viewports
    /// instead of inheriting the previous scan's pan/zoom.
    scan: usize,
    heatmap: Entity<RuvizPlot>,
    chik: Entity<RuvizPlot>,
    trend: Entity<RuvizPlot>,
}

struct FitPlots {
    k: Entity<RuvizPlot>,
    k_residual: Entity<RuvizPlot>,
    r: Entity<RuvizPlot>,
    r_residual: Entity<RuvizPlot>,
}

/// Swap a plot's data while keeping the user's pan/zoom. ruviz-gpui's
/// `set_plot` replaces the whole interactive session and unconditionally
/// resets its interaction state, so the visible bounds are captured first and
/// re-applied afterwards whenever the user had deviated from the default view.
fn set_plot_keep_view(entity: &Entity<RuvizPlot>, plot: ruviz::prelude::Plot, cx: &mut gpui::App) {
    entity.update(cx, |rp, cx| {
        let saved = rp
            .interactive_session()
            .viewport_snapshot()
            .ok()
            .filter(|snapshot| snapshot.visible_bounds != snapshot.base_bounds)
            .map(|snapshot| snapshot.visible_bounds);
        rp.set_plot(plot, cx);
        if let Some(bounds) = saved {
            // restore_visible_bounds marks the session dirty itself; the
            // set_plot notify above schedules the re-render.
            rp.interactive_session().restore_visible_bounds(bounds);
        }
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RangeKey {
    Kmin,
    Kmax,
    Rmin,
    Rmax,
    Kweight,
}

#[derive(Clone)]
struct BatchFitProblem {
    frame: usize,
    label: String,
    error: String,
}

#[derive(Clone)]
enum BatchFitEvent {
    Row(BatchFitRow),
    Problem(BatchFitProblem),
}

/// Completed or in-progress batch fit over an active scan.
struct BatchFitData {
    scan: usize,
    fingerprint: u64,
    model_fingerprint: u64,
    rows: Vec<BatchFitRow>,
    /// File labels captured when the batch scope was created, keyed by the
    /// true frame offset within the scan. Export never consults a later scan.
    frame_labels: BTreeMap<usize, String>,
    problems: Vec<BatchFitProblem>,
    problems_open: bool,
    preview: bool,
    total: usize,
    cancelled: bool,
    varying_names: Vec<String>,
    /// Index into varying_names selected for the operando trend plot.
    trend_param: usize,
}

/// A fit variable row: spec + its editable value/expression field.
struct FitVar {
    spec: FitVarSpec,
    field: Entity<TextInput>,
    min_field: Entity<TextInput>,
    max_field: Entity<TextInput>,
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
    data_panel_open: bool,
    context_panel_open: bool,
    catalog: Catalog,
    source_dir: Option<PathBuf>,
    /// Cache location resolved alongside root canonicalization off the UI thread.
    catalog_index_path: Option<PathBuf>,
    /// Supersedes receivers from earlier folder scans.
    catalog_gen: u64,
    /// Background freshness re-walk behind a restored catalog index.
    verify_running: bool,
    /// Active spectrum (drives params/fit/status).
    selected: Option<usize>,
    /// Compare set; the active spectrum is implicitly included.
    selection: BTreeSet<usize>,
    /// Merged/averaged spectra (virtual indices DERIVED_BASE + i).
    derived: Vec<DerivedSpectrum>,
    compare_gen: u64,
    compare_running: bool,
    view: ViewOptions,
    view_offset_field: Option<Entity<NumericField>>,
    filter_input: Option<Entity<TextInput>>,
    filter_text: String,
    data_focus: FocusHandle,
    operando_focus: FocusHandle,
    /// Per-section "advanced parameters" fold state (Norm, Bkg, FFT, Import).
    adv_open: [bool; 4],
    roi_input: Option<Entity<TextInput>>,
    import_preview: Option<ImportPreview>,
    import_preview_error: SharedString,
    import_preview_gen: u64,
    open_import_role: Option<ImportRole>,
    /// Which enum parameter's option list is expanded.
    open_enum: Option<EnumParam>,
    /// Catalog indices passing the filter (ascending); None = no filter.
    filtered: Option<Arc<Vec<usize>>>,
    /// Bumped per filter edit; stale background match results are dropped.
    filter_gen: u64,
    /// Bumped on every selection; async load results from older generations
    /// are discarded.
    generation: u64,
    load_running: bool,
    /// Bumped on every parameter edit; the debounced recompute only fires for
    /// the latest epoch.
    recompute_epoch: u64,
    params: PipelineParams,
    /// Per-spectrum parameter overrides (copy-on-write of the full param
    /// set), keyed by catalog index. Indices are only stable within one
    /// scan session; persistence re-keys by file path (`ParamOverride`).
    overrides: BTreeMap<usize, PipelineParams>,
    /// Path-keyed overrides from a loaded project, resolved to catalog
    /// indices once the folder scan / index restore materializes entries.
    pending_overrides: Vec<ParamOverride>,
    param_fields: Vec<(ParamKey, Entity<NumericField>)>,
    /// Keyed by (catalog index, effective-params fingerprint).
    cache: LruCache<(usize, u64), Arc<XASSpectrum>>,
    current_path: PathBuf,
    spectrum_path: PathBuf,
    spectrum_fingerprint: u64,
    spectrum: Option<Arc<XASSpectrum>>,
    spectrum_label: SharedString,
    quadrants: Vec<(SharedString, Entity<RuvizPlot>)>,
    /// Shared Explore legend strip entries (truncated label, stable color),
    /// rebuilt with the quadrant plots; empty for a single trace.
    legend_entries: Vec<(SharedString, gpui::Rgba)>,
    maximized: Option<usize>,
    data_tab: DataTab,
    file_scroll: UniformListScrollHandle,
    scan_scroll: UniformListScrollHandle,
    /// At most one scan is expanded, keeping row-to-member mapping O(1)
    /// even when that scan has a million members.
    expanded_scan: Option<usize>,
    active_scan: Option<usize>,
    operando: Option<OperandoData>,
    operando_plots: Option<OperandoPlots>,
    operando_gen: u64,
    operando_running: bool,
    operando_cancel: Option<Arc<AtomicBool>>,
    time_pos: usize,
    /// Requested cursor used when a batch-table row opens a scan overview in
    /// the background while the Fit workspace stays visible.
    pending_time_pos: Option<usize>,
    fit_paths: Vec<FitPathRow>,
    fit_vars: Vec<FitVar>,
    fit_range_fields: Vec<(RangeKey, Entity<NumericField>)>,
    fit_ranges: FitRanges,
    fit_result: Option<Arc<FeffFitResult>>,
    fit_provenance: Option<FitProvenance>,
    fit_plots: Option<FitPlots>,
    fit_gen: u64,
    fit_running: bool,
    last_fit_duration: Option<Duration>,
    feff_workspace: Option<PathBuf>,
    feff_running: bool,
    feff_gen: u64,
    feff_form: Vec<(FeffFormKey, Entity<TextInput>)>,
    /// Batch-fit rows with scan-parameter and fit-model provenance.
    batch_fit: Option<BatchFitData>,
    /// False is the default full-scan scope; true opts into the overview sample.
    batch_preview: bool,
    batch_running: bool,
    batch_progress: (usize, usize),
    batch_gen: u64,
    batch_cancel: Option<Arc<AtomicBool>>,
    batch_started: Option<Instant>,
    merge_running: bool,
    merge_gen: u64,
    merge_cancel: Option<Arc<AtomicBool>>,
    status: SharedString,
    job_errors: Vec<JobError>,
    problems_open: bool,
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

fn sample_scan_indices(start: usize, len: usize, cap: usize) -> Vec<usize> {
    if len <= cap {
        (start..start + len).collect()
    } else {
        (0..cap)
            .map(|i| start + i * (len - 1) / (cap - 1))
            .collect()
    }
}

fn short_duration(duration: Duration) -> String {
    let seconds = duration.as_secs_f64();
    if seconds < 1.0 {
        format!("{:.0} ms", seconds * 1000.0)
    } else if seconds < 60.0 {
        format!("{seconds:.1} s")
    } else if seconds < 3600.0 {
        format!("{:.1} min", seconds / 60.0)
    } else {
        format!("{:.1} h", seconds / 3600.0)
    }
}

/// Map a full-scan cursor coordinate to the nearest sampled overview row.
fn nearest_sample_pos(full_pos: usize, full_len: usize, sample_len: usize) -> usize {
    if full_len <= 1 || sample_len <= 1 {
        return 0;
    }
    let numerator = full_pos.min(full_len - 1) as u128 * (sample_len - 1) as u128;
    let denominator = (full_len - 1) as u128;
    ((numerator + denominator / 2) / denominator) as usize
}

/// Resolve a virtualized file row defensively. A stale filtered index must
/// never be allowed to address a newly reconciled, shorter catalog.
fn catalog_row_index(filtered: Option<&[usize]>, row: usize, catalog_len: usize) -> Option<usize> {
    let ix = match filtered {
        Some(filtered) => filtered.get(row).copied()?,
        None => row,
    };
    (ix < catalog_len).then_some(ix)
}

fn scan_entry_offset(scan_start: usize, scan_len: usize, entry: usize) -> Option<usize> {
    entry
        .checked_sub(scan_start)
        .filter(|&offset| offset < scan_len)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScanListRow {
    Header(usize),
    Member { scan: usize, offset: usize },
}

/// Translate a virtualized flattened Scans-tab row without allocating a
/// per-member index vector.
fn scan_list_row(
    row: usize,
    scan_count: usize,
    expanded: Option<(usize, usize)>,
) -> Option<ScanListRow> {
    let Some((scan, len)) = expanded else {
        return (row < scan_count).then_some(ScanListRow::Header(row));
    };
    if scan >= scan_count {
        return None;
    }
    if row <= scan {
        return Some(ScanListRow::Header(row));
    }
    if row <= scan.saturating_add(len) {
        return Some(ScanListRow::Member {
            scan,
            offset: row - scan - 1,
        });
    }
    let header = row - len;
    (header < scan_count).then_some(ScanListRow::Header(header))
}

#[cfg(test)]
mod thin_tests {
    use super::{
        ScanListRow, catalog_row_index, nearest_sample_pos, scan_entry_offset, scan_list_row,
        thin_even,
    };

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

    #[test]
    fn full_positions_map_to_nearest_overview_rows() {
        assert_eq!(nearest_sample_pos(0, 1_000, 192), 0);
        assert_eq!(nearest_sample_pos(999, 1_000, 192), 191);
        assert_eq!(nearest_sample_pos(500, 1_000, 192), 96);
        assert_eq!(nearest_sample_pos(42, 100, 1), 0);
    }

    #[test]
    fn expanded_scan_rows_are_flattened_without_member_storage() {
        let expanded = Some((1, 3));
        assert_eq!(scan_list_row(0, 3, expanded), Some(ScanListRow::Header(0)));
        assert_eq!(scan_list_row(1, 3, expanded), Some(ScanListRow::Header(1)));
        assert_eq!(
            scan_list_row(2, 3, expanded),
            Some(ScanListRow::Member { scan: 1, offset: 0 })
        );
        assert_eq!(
            scan_list_row(4, 3, expanded),
            Some(ScanListRow::Member { scan: 1, offset: 2 })
        );
        assert_eq!(scan_list_row(5, 3, expanded), Some(ScanListRow::Header(2)));
        assert_eq!(scan_list_row(6, 3, expanded), None);
    }

    #[test]
    fn reconciled_catalog_rejects_stale_filtered_indices() {
        let stale = [0, 4, 9];
        assert_eq!(catalog_row_index(Some(&stale), 0, 2), Some(0));
        assert_eq!(catalog_row_index(Some(&stale), 1, 2), None);
        assert_eq!(catalog_row_index(Some(&stale), 3, 2), None);
    }

    #[test]
    fn scan_members_map_to_cursor_offsets() {
        assert_eq!(scan_entry_offset(40, 5, 40), Some(0));
        assert_eq!(scan_entry_offset(40, 5, 44), Some(4));
        assert_eq!(scan_entry_offset(40, 5, 39), None);
        assert_eq!(scan_entry_offset(40, 5, 45), None);
    }
}

#[cfg(test)]
mod override_tests {
    use std::collections::BTreeMap;

    use super::{ParamSection, copy_section, scan_fingerprint, section_differs};
    use crate::params::PipelineParams;

    #[test]
    fn section_differs_is_per_section_and_reset_restores_global() {
        let global = PipelineParams::default();
        let mut ov = global.clone();
        ov.e0 = Some(22117.0);
        ov.rbkg = Some(1.4);
        assert!(section_differs(&ov, &global, ParamSection::Norm));
        assert!(section_differs(&ov, &global, ParamSection::Bkg));
        assert!(!section_differs(&ov, &global, ParamSection::Fft));
        assert!(!section_differs(&ov, &global, ParamSection::Import));

        copy_section(&mut ov, &global, ParamSection::Norm);
        assert!(!section_differs(&ov, &global, ParamSection::Norm));
        assert!(section_differs(&ov, &global, ParamSection::Bkg));
        copy_section(&mut ov, &global, ParamSection::Bkg);
        assert!(ov == global);
    }

    #[test]
    fn every_section_owns_at_least_one_field() {
        // Guards the copy_section field lists against a param moving
        // between sections without updating them.
        let global = PipelineParams::default();
        let changed = PipelineParams {
            align_target: Some(1.0),
            e0: Some(2.0),
            rbkg: Some(3.0),
            fft_kmin: Some(4.0),
            ..Default::default()
        };
        for section in [
            ParamSection::Import,
            ParamSection::Norm,
            ParamSection::Bkg,
            ParamSection::Fft,
        ] {
            assert!(
                section_differs(&changed, &global, section),
                "{section:?} must own a changed field"
            );
        }
    }

    #[test]
    fn scan_fingerprint_tracks_overrides_inside_the_range_only() {
        let global = PipelineParams::default();
        let mut overrides = BTreeMap::new();
        let base = scan_fingerprint(&global, &overrides, 100, 50);
        assert_eq!(base, scan_fingerprint(&global, &overrides, 100, 50));

        let ov = PipelineParams {
            rbkg: Some(1.3),
            ..Default::default()
        };
        overrides.insert(10, ov.clone()); // outside [100, 150)
        assert_eq!(base, scan_fingerprint(&global, &overrides, 100, 50));

        overrides.insert(120, ov); // inside
        assert_ne!(base, scan_fingerprint(&global, &overrides, 100, 50));

        overrides.remove(&120);
        assert_eq!(base, scan_fingerprint(&global, &overrides, 100, 50));
    }
}

/// Case-insensitive name filter with `*` wildcards; without `*` it is a
/// substring match. Segments must appear in order; ends anchor unless the
/// pattern starts/ends with `*`. Test-only convenience wrapper.
#[cfg(test)]
fn filter_match(name: &str, pattern: &str) -> bool {
    filter_match_lower(&name.to_ascii_lowercase(), &pattern.to_ascii_lowercase())
}

/// Name filter over pre-lowercased inputs; the bulk matcher lowercases the
/// pattern once and reuses one name buffer, so a full catalog pass does
/// O(1) allocations instead of two per entry.
fn filter_match_lower(name: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if !pattern.contains('*') {
        return name.contains(pattern);
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

#[cfg(test)]
mod keybinding_tests {
    use gpui::{Action, KeyBinding, KeyContext};

    use super::{
        ExploreEscape, FocusFilter, FrameFirst, FrameLast, Maximize1, Maximize2, Maximize3,
        Maximize4, RestoreGrid, ToggleContextPanel, ToggleDataPanel, WorkspaceExplore,
        studio_keybindings,
    };

    fn binding<A: Action + 'static>(bindings: &[KeyBinding]) -> &KeyBinding {
        bindings
            .iter()
            .find(|binding| binding.action().as_any().is::<A>())
            .expect("action must have a key binding")
    }

    #[test]
    fn text_input_context_blocks_shell_editing_keys() {
        let bindings = studio_keybindings();
        let studio = KeyContext::parse("Studio Explore").unwrap();
        let text_input = KeyContext::parse("TextInput").unwrap();
        let shell_context = [studio.clone()];
        let editing_context = [studio, text_input];

        let protected = [
            binding::<ToggleDataPanel>(&bindings),
            binding::<ToggleContextPanel>(&bindings),
            binding::<FocusFilter>(&bindings),
            binding::<Maximize1>(&bindings),
            binding::<Maximize2>(&bindings),
            binding::<Maximize3>(&bindings),
            binding::<Maximize4>(&bindings),
            binding::<RestoreGrid>(&bindings),
            binding::<ExploreEscape>(&bindings),
        ];
        for binding in protected {
            let predicate = binding.predicate().expect("binding must be scoped");
            assert!(predicate.depth_of(&shell_context).is_some());
            assert!(predicate.depth_of(&editing_context).is_none());
        }

        // Workspace switching is intentionally global and non-editing.
        assert!(
            binding::<WorkspaceExplore>(&bindings)
                .predicate()
                .unwrap()
                .depth_of(&editing_context)
                .is_some()
        );
    }

    #[test]
    fn operando_extreme_keys_yield_to_nested_text_input() {
        let bindings = studio_keybindings();
        let studio = KeyContext::parse("Studio OperandoWorkspace").unwrap();
        let operando = KeyContext::parse("Operando").unwrap();
        let text_input = KeyContext::parse("TextInput").unwrap();
        let scrub_context = [studio.clone(), operando.clone()];
        let editing_context = [studio, operando, text_input];

        for binding in [
            binding::<FrameFirst>(&bindings),
            binding::<FrameLast>(&bindings),
        ] {
            let predicate = binding.predicate().expect("binding must be scoped");
            assert!(predicate.depth_of(&scrub_context).is_some());
            assert!(predicate.depth_of(&editing_context).is_none());
        }
    }
}

fn default_data_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../xraytsubaki/tests/testfiles/Ru_QAS.dat")
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
            data_panel_open: true,
            context_panel_open: true,
            catalog: Catalog::default(),
            source_dir: None,
            catalog_index_path: None,
            catalog_gen: 0,
            verify_running: false,
            selected: None,
            selection: BTreeSet::new(),
            derived: Vec::new(),
            compare_gen: 0,
            compare_running: false,
            view: ViewOptions::default(),
            view_offset_field: None,
            filter_input: None,
            filter_text: String::new(),
            filtered: None,
            filter_gen: 0,
            data_focus: cx.focus_handle(),
            operando_focus: cx.focus_handle(),
            adv_open: [false; 4],
            roi_input: None,
            import_preview: None,
            import_preview_error: "".into(),
            import_preview_gen: 0,
            open_import_role: None,
            open_enum: None,
            generation: 0,
            load_running: false,
            recompute_epoch: 0,
            params,
            overrides: BTreeMap::new(),
            pending_overrides: Vec::new(),
            param_fields,
            cache: LruCache::new(NonZeroUsize::new(PROCESSED_CACHE_CAPACITY).unwrap()),
            current_path: path.clone(),
            spectrum_path: path.clone(),
            spectrum_fingerprint: 0,
            spectrum: None,
            spectrum_label: label.clone(),
            quadrants: Vec::new(),
            legend_entries: Vec::new(),
            maximized: None,
            data_tab: DataTab::Files,
            file_scroll: UniformListScrollHandle::new(),
            scan_scroll: UniformListScrollHandle::new(),
            expanded_scan: None,
            active_scan: None,
            operando: None,
            operando_plots: None,
            operando_gen: 0,
            operando_running: false,
            operando_cancel: None,
            time_pos: 0,
            pending_time_pos: None,
            fit_paths: Vec::new(),
            fit_vars: Vec::new(),
            fit_range_fields: Vec::new(),
            fit_ranges: FitRanges::default(),
            fit_result: None,
            fit_provenance: None,
            fit_plots: None,
            fit_gen: 0,
            fit_running: false,
            last_fit_duration: None,
            feff_workspace: None,
            feff_running: false,
            feff_gen: 0,
            feff_form: Vec::new(),
            batch_fit: None,
            batch_preview: false,
            batch_running: false,
            batch_progress: (0, 0),
            batch_gen: 0,
            batch_cancel: None,
            batch_started: None,
            merge_running: false,
            merge_gen: 0,
            merge_cancel: None,
            status: "loading...".into(),
            job_errors: Vec::new(),
            problems_open: false,
        };
        app.fit_range_fields = Self::build_range_fields(theme, app.fit_ranges, cx);
        let offset_field = cx.new(|cx| {
            NumericField::new(
                "offset",
                "",
                Some(app.view.offset_frac),
                FieldKind::Float,
                theme,
                cx,
            )
        });
        cx.subscribe(&offset_field, |this: &mut Self, _f, event, cx| {
            match event {
                FieldEvent::Changed(Some(v)) => {
                    this.view.offset_frac = v.clamp(0.05, 5.0);
                    this.rebuild_plots(cx);
                    cx.notify();
                }
                FieldEvent::Changed(None) => {}
                FieldEvent::Invalid(message) => {
                    this.status = message.clone();
                    cx.notify();
                }
            };
        })
        .detach();
        app.view_offset_field = Some(offset_field);
        let filter_input = cx.new(|cx| TextInput::new("filter… (* glob)", "", theme, cx));
        cx.subscribe(&filter_input, |this: &mut Self, _f, event, cx| {
            // Per-keystroke filtering (doc "Search-first"): Edited and
            // Committed both re-run the (background) match.
            let (InputEvent::Committed(text) | InputEvent::Edited(text)) = event;
            let text = text.trim().to_string();
            if text != this.filter_text {
                this.filter_text = text;
                this.apply_filter(cx);
            }
        })
        .detach();
        app.filter_input = Some(filter_input);
        let roi_input = cx.new(|cx| TextInput::new("e.g. 4 or 4-7", "", theme, cx));
        cx.subscribe(&roi_input, |this: &mut Self, input, event, cx| {
            let InputEvent::Committed(text) = event else {
                return;
            };
            let trimmed = text.trim();
            if trimmed.is_empty() {
                if this.ui_params().import.fluor_cols.is_some() {
                    this.edit_params().import.fluor_cols = None;
                    this.schedule_recompute(cx);
                    this.update_import_preview(cx);
                }
                cx.notify();
                return;
            }
            match parse_cols(trimmed) {
                Some(cols) => {
                    if this.ui_params().import.fluor_cols.as_ref() != Some(&cols) {
                        this.edit_params().import.fluor_cols = Some(cols);
                        this.schedule_recompute(cx);
                        this.update_import_preview(cx);
                    }
                }
                None => {
                    // revert to current value, with a visible rejection cue
                    this.status =
                        format!("invalid ROI columns: '{trimmed}' — expected e.g. 4 or 4-7").into();
                    let text = this
                        .ui_params()
                        .import
                        .fluor_cols
                        .as_deref()
                        .unwrap_or_default()
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
            (
                key,
                cx.new(|cx| TextInput::new(placeholder, initial, theme, cx)),
            )
        })
        .collect();

        app.update_import_preview(cx);
        match process_file(&path, &app.params) {
            Ok(sp) => {
                let fingerprint = app.params.fingerprint();
                app.set_processed(NO_ENTRY, label, path.clone(), fingerprint, Arc::new(sp), cx);
            }
            Err(e) => {
                app.status = format!("failed to load {}: {e}", path.display()).into();
                app.record_job_error(path.display().to_string(), e.to_string());
            }
        }
        if let Some(dir) = initial_dir {
            app.scan_folder(dir, cx);
        }
        app
    }

    fn record_job_error(&mut self, label: impl Into<String>, message: impl Into<String>) {
        if self.job_errors.len() == JOB_ERROR_CAPACITY {
            self.job_errors.remove(0);
        }
        self.job_errors.push(JobError {
            label: label.into(),
            message: message.into(),
        });
    }

    fn running_job_count(&self) -> usize {
        usize::from(self.catalog.scanning)
            + usize::from(self.verify_running)
            + usize::from(self.load_running)
            + usize::from(self.compare_running)
            + usize::from(self.operando_running)
            + usize::from(self.merge_running)
            + usize::from(self.fit_running)
            + usize::from(self.feff_running)
            + usize::from(self.batch_running)
    }

    fn selection_count(&self) -> usize {
        self.selection.len()
            + usize::from(
                self.selected
                    .is_some_and(|selected| !self.selection.contains(&selected)),
            )
    }

    /// Entry whose params a context-panel edit targets: exactly one active
    /// catalog spectrum (doc: "editing while a spectrum is selected creates
    /// an override"). Multi-selection, derived spectra, and the default
    /// file edit the global set.
    fn override_target(&self) -> Option<usize> {
        let ix = self.selected?;
        (ix < DERIVED_BASE && self.selection_count() <= 1).then_some(ix)
    }

    /// Effective params for an entry: its override, else the global set.
    fn effective_params(&self, ix: usize) -> &PipelineParams {
        self.overrides.get(&ix).unwrap_or(&self.params)
    }

    fn effective_fingerprint(&self, ix: usize) -> u64 {
        self.effective_params(ix).fingerprint()
    }

    /// Effective fingerprint of the active spectrum (global when nothing
    /// from the catalog is selected).
    fn active_fingerprint(&self) -> u64 {
        self.effective_fingerprint(self.selected.unwrap_or(NO_ENTRY))
    }

    /// Params the context panel displays: the override target's effective
    /// set, falling back to the globals.
    fn ui_params(&self) -> &PipelineParams {
        match self.override_target() {
            Some(ix) => self.effective_params(ix),
            None => &self.params,
        }
    }

    /// Params a context-panel edit lands on. A lone selected spectrum gets
    /// a copy-on-write override created on first edit; otherwise the edit
    /// goes to the global set.
    fn edit_params(&mut self) -> &mut PipelineParams {
        match self.override_target() {
            Some(ix) => self
                .overrides
                .entry(ix)
                .or_insert_with(|| self.params.clone()),
            None => &mut self.params,
        }
    }

    /// Whether the override target's `section` diverges from the globals
    /// (drives the section-header chip).
    fn section_overridden(&self, section: ParamSection) -> bool {
        self.override_target()
            .and_then(|ix| self.overrides.get(&ix))
            .is_some_and(|ov| section_differs(ov, &self.params, section))
    }

    /// Chip action: copy the global section back over the override,
    /// dropping the override entirely once nothing diverges anymore.
    fn reset_section_override(&mut self, section: ParamSection, cx: &mut Context<Self>) {
        let Some(ix) = self.override_target() else {
            return;
        };
        let global = self.params.clone();
        let Some(ov) = self.overrides.get_mut(&ix) else {
            return;
        };
        copy_section(ov, &global, section);
        if *ov == global {
            self.overrides.remove(&ix);
        }
        self.sync_param_fields(cx);
        self.schedule_recompute(cx);
        cx.notify();
    }

    /// Push the displayed param set into the context-panel fields (never
    /// emits change events). Called whenever the edit target may have
    /// changed: selection moves, multi-select edits, resets, project load.
    fn sync_param_fields(&mut self, cx: &mut Context<Self>) {
        let params = self.ui_params().clone();
        for (key, field) in &self.param_fields {
            let value = param_field_value(*key, &params);
            field.update(cx, |f, cx| f.set_value(value, cx));
        }
        if let Some(roi) = &self.roi_input {
            let text = params
                .import
                .fluor_cols
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(",");
            roi.update(cx, |i, cx| i.set_text(text, cx));
        }
        self.update_import_preview(cx);
    }

    /// Attach path-keyed overrides from a loaded project to the freshly
    /// materialized catalog. Overrides created this session win over the
    /// persisted set; paths missing from the catalog are reported.
    fn resolve_pending_overrides(&mut self, cx: &mut Context<Self>) {
        if self.pending_overrides.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_overrides);
        let mut missing = 0usize;
        for saved in pending {
            match self.catalog.find_by_path(&saved.path) {
                Some(ix) => {
                    self.overrides.entry(ix).or_insert(saved.params);
                }
                None => missing += 1,
            }
        }
        if missing > 0 {
            self.record_job_error(
                "param overrides",
                format!("{missing} saved override(s) matched no file in the catalog"),
            );
        }
        self.sync_param_fields(cx);
    }

    fn fit_model_fingerprint(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for row in &self.fit_paths {
            row.spec.file.hash(&mut hasher);
            row.spec.label.hash(&mut hasher);
            row.spec.s02.hash(&mut hasher);
            row.spec.e0.hash(&mut hasher);
            row.spec.sigma2.hash(&mut hasher);
            row.spec.deltar.hash(&mut hasher);
            row.spec.enabled.hash(&mut hasher);
        }
        for var in &self.fit_vars {
            var.spec.name.hash(&mut hasher);
            var.spec.value.to_bits().hash(&mut hasher);
            var.spec.vary.hash(&mut hasher);
            var.spec.min.map(f64::to_bits).hash(&mut hasher);
            var.spec.max.map(f64::to_bits).hash(&mut hasher);
            var.spec.expr.hash(&mut hasher);
        }
        self.fit_ranges.kmin.to_bits().hash(&mut hasher);
        self.fit_ranges.kmax.to_bits().hash(&mut hasher);
        self.fit_ranges.rmin.to_bits().hash(&mut hasher);
        self.fit_ranges.rmax.to_bits().hash(&mut hasher);
        self.fit_ranges.kweight.to_bits().hash(&mut hasher);
        hasher.finish()
    }

    fn fit_is_stale(&self) -> bool {
        let Some(provenance) = &self.fit_provenance else {
            return false;
        };
        self.spectrum.is_none()
            || provenance.label != self.spectrum_label
            || provenance.path != self.current_path
            || provenance.path != self.spectrum_path
            || provenance.params_fingerprint != self.active_fingerprint()
            || provenance.params_fingerprint != self.spectrum_fingerprint
            || provenance.model_fingerprint != self.fit_model_fingerprint()
    }

    fn batch_fit_is_stale(&self) -> bool {
        self.batch_fit
            .as_ref()
            .is_some_and(|batch| batch.model_fingerprint != self.fit_model_fingerprint())
    }

    /// Reset every state that is indexed by the current catalog before a new
    /// folder starts streaming. Generation bumps also make old async arrivals
    /// harmless while their receivers/workers wind down.
    fn reset_catalog_state(&mut self, cx: &mut Context<Self>) {
        self.catalog_gen += 1;
        self.generation += 1;
        self.compare_gen += 1;
        self.operando_gen += 1;
        self.batch_gen += 1;
        self.merge_gen += 1;
        if let Some(cancel) = self.operando_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Some(cancel) = self.batch_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Some(cancel) = self.merge_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.catalog = Catalog::default();
        self.catalog_index_path = None;
        self.verify_running = false;
        self.load_running = false;
        self.compare_running = false;
        self.merge_running = false;
        self.selected = None;
        self.selection.clear();
        self.filtered = None;
        self.filter_gen += 1;
        self.filter_text.clear();
        if let Some(input) = &self.filter_input {
            input.update(cx, |input, cx| input.set_text("", cx));
        }
        self.cache.clear();
        // Overrides are keyed by catalog index; a new catalog orphans them.
        // Persisted copies live in the project file, keyed by path.
        self.overrides.clear();
        self.pending_overrides.clear();
        self.current_path = PathBuf::new();
        self.spectrum_path = PathBuf::new();
        self.spectrum_fingerprint = 0;
        self.spectrum = None;
        self.spectrum_label = "no spectrum".into();
        self.import_preview = None;
        self.import_preview_error = "".into();
        self.import_preview_gen += 1;
        self.open_import_role = None;
        self.quadrants.clear();
        self.maximized = None;
        self.file_scroll = UniformListScrollHandle::new();
        self.scan_scroll = UniformListScrollHandle::new();
        self.expanded_scan = None;
        self.active_scan = None;
        self.operando = None;
        self.operando_plots = None;
        self.operando_running = false;
        self.pending_time_pos = None;
        self.time_pos = 0;
        self.batch_fit = None;
        self.batch_running = false;
        self.batch_progress = (0, 0);
        self.batch_started = None;
    }

    fn cancel_long_jobs(&mut self, cx: &mut Context<Self>) {
        let mut cancelled = false;
        if self.operando_running {
            self.operando_gen += 1;
            if let Some(cancel) = self.operando_cancel.take() {
                cancel.store(true, Ordering::Relaxed);
            }
            self.operando_running = false;
            cancelled = true;
        }
        if self.batch_running
            && let Some(cancel) = self.batch_cancel.as_ref()
        {
            cancel.store(true, Ordering::Relaxed);
            cancelled = true;
        }
        if self.merge_running {
            self.merge_gen += 1;
            if let Some(cancel) = self.merge_cancel.take() {
                cancel.store(true, Ordering::Relaxed);
            }
            self.merge_running = false;
            cancelled = true;
        }
        if cancelled {
            self.status = "cancelling jobs — completed results are kept ...".into();
            cx.notify();
        }
    }

    /// Context-panel fields, in pipeline order. Placeholders show the value
    /// used when the field is on "auto".
    fn build_param_fields(
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Vec<(ParamKey, Entity<NumericField>)> {
        // Column indices clamp to >= 0 like apply_param; other integer
        // params only round, so the display always matches the applied value.
        const COL: FieldKind = FieldKind::Integer { min: Some(0) };
        const INT: FieldKind = FieldKind::Integer { min: None };
        const FLOAT: FieldKind = FieldKind::Float;
        let specs: [(ParamKey, &str, &str, FieldKind); 30] = [
            (ParamKey::ImpEnergyCol, "energy col", "0", COL),
            (ParamKey::ImpI0Col, "I0 col", "1", COL),
            (ParamKey::ImpItCol, "It col", "2", COL),
            (ParamKey::ImpIrCol, "Ir col", "3", COL),
            (ParamKey::AlignTarget, "ref E0 target", "e.g. 22117", FLOAT),
            (ParamKey::E0, "E0 (eV)", "auto", FLOAT),
            (
                ParamKey::PreEdgeStart,
                "pre-edge start",
                "auto (-200)",
                FLOAT,
            ),
            (ParamKey::PreEdgeEnd, "pre-edge end", "auto (-30)", FLOAT),
            (ParamKey::NormStart, "norm start", "auto (150)", FLOAT),
            (ParamKey::NormEnd, "norm end", "auto (2000)", FLOAT),
            (ParamKey::NormPolyorder, "poly order", "auto (2)", INT),
            (ParamKey::NVictoreen, "victoreen n", "auto (0)", INT),
            (ParamKey::Rbkg, "rbkg (Å)", "auto (1.0)", FLOAT),
            (ParamKey::BkgKmin, "k min", "auto (0)", FLOAT),
            (ParamKey::BkgKmax, "k max", "auto (full)", FLOAT),
            (ParamKey::BkgKstep, "k step", "auto (0.05)", FLOAT),
            (ParamKey::BkgNknots, "spline knots", "auto", INT),
            (ParamKey::BkgKweight, "bkg k-weight", "auto (1)", INT),
            (ParamKey::BkgClampLo, "clamp lo", "auto (0)", INT),
            (ParamKey::BkgClampHi, "clamp hi", "auto (1)", INT),
            (ParamKey::BkgDk, "window dk", "auto (0.1)", FLOAT),
            (ParamKey::BkgNfft, "nfft", "auto (2048)", INT),
            (ParamKey::FftKmin, "k min", "auto (2)", FLOAT),
            (ParamKey::FftKmax, "k max", "auto (15)", FLOAT),
            (ParamKey::FftDk, "dk", "auto (1)", FLOAT),
            (ParamKey::FftKweight, "k-weight", "auto (2)", FLOAT),
            (ParamKey::FftDk2, "dk2", "auto", FLOAT),
            (ParamKey::FftRmax, "R max out", "auto (10)", FLOAT),
            (ParamKey::FftKstep, "k step", "auto", FLOAT),
            (ParamKey::FftNfft, "nfft", "auto (2048)", INT),
        ];
        specs
            .into_iter()
            .map(|(key, label, placeholder, kind)| {
                let field =
                    cx.new(|cx| NumericField::new(label, placeholder, None, kind, theme, cx));
                cx.subscribe(
                    &field,
                    move |this: &mut Self, _field, event, cx| match event {
                        FieldEvent::Changed(value) => this.apply_param(key, *value, cx),
                        FieldEvent::Invalid(message) => {
                            this.status = message.clone();
                            cx.notify();
                        }
                    },
                )
                .detach();
                (key, field)
            })
            .collect()
    }

    fn apply_param(&mut self, key: ParamKey, value: Option<f64>, cx: &mut Context<Self>) {
        let p = self.edit_params();
        let int = value.map(|v| v.round() as i32);
        let col = value.map(|v| v.round().max(0.0) as usize);
        match key {
            ParamKey::ImpEnergyCol => p.import.energy_col = col,
            ParamKey::ImpI0Col => p.import.i0_col = col,
            ParamKey::ImpItCol => p.import.it_col = col,
            ParamKey::ImpIrCol => p.import.ir_col = col,
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
        if matches!(
            key,
            ParamKey::ImpEnergyCol | ParamKey::ImpI0Col | ParamKey::ImpItCol | ParamKey::ImpIrCol
        ) {
            self.update_import_preview(cx);
        }
        // The section chip may have just flipped global -> override.
        cx.notify();
    }

    /// Option labels for an enum parameter: index 0 = auto, then variants.
    fn enum_options(&self, which: EnumParam) -> Vec<String> {
        let (auto_label, variants): (&str, Vec<String>) = match which {
            EnumParam::ImportMode => {
                let resolved = self
                    .import_preview
                    .as_ref()
                    .map(|preview| format!("{:?}", preview.auto_mode))
                    .unwrap_or_else(|| "detecting…".into());
                let mut options = vec![format!("auto ({resolved})")];
                options.extend(DETECTION_MODES.iter().map(|mode| format!("{mode:?}")));
                return options;
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
        let p = self.edit_params();
        match which {
            EnumParam::ImportMode => {
                p.import.mode = variant
                    .and_then(|i| DETECTION_MODES.get(i).copied())
                    .unwrap_or(DetectionMode::Auto);
            }
            EnumParam::BkgWindow => {
                p.bkg_window = variant.map(|i| FT_WINDOWS[i]);
            }
            EnumParam::BkgSolver => {
                p.bkg_solver = variant.map(|i| AUTOBK_SOLVERS[i]);
            }
            EnumParam::FftWindow => {
                p.fft_window = variant.map(|i| FT_WINDOWS[i]);
            }
        }
        self.open_enum = None;
        self.schedule_recompute(cx);
        if which == EnumParam::ImportMode {
            self.update_import_preview(cx);
        }
        cx.notify();
    }

    fn enum_selected_index(&self, which: EnumParam) -> usize {
        let p = self.ui_params();
        match which {
            EnumParam::ImportMode => {
                if p.import.mode == DetectionMode::Auto {
                    0
                } else {
                    DETECTION_MODES
                        .iter()
                        .position(|mode| *mode == p.import.mode)
                        .map(|index| index + 1)
                        .unwrap_or(0)
                }
            }
            EnumParam::BkgWindow => p
                .bkg_window
                .and_then(|w| FT_WINDOWS.iter().position(|x| *x == w))
                .map(|i| i + 1)
                .unwrap_or(0),
            EnumParam::BkgSolver => p
                .bkg_solver
                .and_then(|v| AUTOBK_SOLVERS.iter().position(|x| *x == v))
                .map(|i| i + 1)
                .unwrap_or(0),
            EnumParam::FftWindow => p
                .fft_window
                .and_then(|w| FT_WINDOWS.iter().position(|x| *x == w))
                .map(|i| i + 1)
                .unwrap_or(0),
        }
    }

    fn import_column_label(preview: Option<&ImportPreview>, column: usize) -> String {
        preview
            .and_then(|preview| preview.names.as_ref())
            .and_then(|names| names.get(column))
            .map(|name| format!("col {column} · {name}"))
            .unwrap_or_else(|| format!("col {column}"))
    }

    fn import_role_label(role: ImportRole) -> &'static str {
        match role {
            ImportRole::Energy => "energy",
            ImportRole::I0 => "I0",
            ImportRole::It => "It",
            ImportRole::Ir => "Ir",
            ImportRole::Fluor => "SDD ROIs",
            ImportRole::Mu => "mu",
        }
    }

    fn import_role_current_label(&self, role: ImportRole) -> String {
        let preview = self.import_preview.as_ref();
        let import = &self.ui_params().import;
        if role == ImportRole::Fluor {
            return match &import.fluor_cols {
                Some(columns) if columns.is_empty() => "none".into(),
                Some(columns) => columns
                    .iter()
                    .map(|&column| Self::import_column_label(preview, column))
                    .collect::<Vec<_>>()
                    .join(", "),
                None => self.import_role_auto_label(role),
            };
        }
        let manual = match role {
            ImportRole::Energy => import.energy_col,
            ImportRole::I0 => import.i0_col,
            ImportRole::It => import.it_col,
            ImportRole::Ir => import.ir_col,
            ImportRole::Mu => import.mu_col,
            ImportRole::Fluor => unreachable!(),
        };
        if let Some(column) = manual {
            return Self::import_column_label(preview, column);
        }
        self.import_role_auto_label(role)
    }

    fn import_role_auto_label(&self, role: ImportRole) -> String {
        let preview = self.import_preview.as_ref();
        if role == ImportRole::Fluor {
            return preview
                .map(|preview| {
                    let columns = preview
                        .detected
                        .fluor_cols
                        .iter()
                        .map(|&column| Self::import_column_label(Some(preview), column))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("auto ({columns})")
                })
                .unwrap_or_else(|| "auto (detecting…)".into());
        }
        let resolved = preview.and_then(|preview| match role {
            ImportRole::Energy => Some(preview.detected.energy_col),
            ImportRole::I0 => Some(preview.detected.i0_col),
            ImportRole::It => Some(preview.detected.it_col),
            ImportRole::Ir => Some(preview.detected.ir_col),
            ImportRole::Mu => preview.detected.mu_col,
            ImportRole::Fluor => unreachable!(),
        });
        resolved
            .map(|column| format!("auto ({})", Self::import_column_label(preview, column)))
            .unwrap_or_else(|| "auto (not found)".into())
    }

    fn import_role_manual_column(&self, role: ImportRole) -> Option<usize> {
        let import = &self.ui_params().import;
        match role {
            ImportRole::Energy => import.energy_col,
            ImportRole::I0 => import.i0_col,
            ImportRole::It => import.it_col,
            ImportRole::Ir => import.ir_col,
            ImportRole::Mu => import.mu_col,
            ImportRole::Fluor => None,
        }
    }

    fn set_import_role_column(
        &mut self,
        role: ImportRole,
        column: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let import = &mut self.edit_params().import;
        match role {
            ImportRole::Energy => import.energy_col = column,
            ImportRole::I0 => import.i0_col = column,
            ImportRole::It => import.it_col = column,
            ImportRole::Ir => import.ir_col = column,
            ImportRole::Mu => import.mu_col = column,
            ImportRole::Fluor => return,
        }
        self.open_import_role = None;
        self.schedule_recompute(cx);
        self.update_import_preview(cx);
        cx.notify();
    }

    fn toggle_import_fluor(&mut self, column: Option<usize>, cx: &mut Context<Self>) {
        match column {
            None => self.edit_params().import.fluor_cols = None,
            Some(column) => {
                let mut columns = self
                    .ui_params()
                    .import
                    .fluor_cols
                    .clone()
                    .unwrap_or_default();
                if let Some(index) = columns.iter().position(|&current| current == column) {
                    columns.remove(index);
                } else {
                    columns.push(column);
                    columns.sort_unstable();
                }
                self.edit_params().import.fluor_cols = Some(columns);
            }
        }
        self.schedule_recompute(cx);
        self.update_import_preview(cx);
        cx.notify();
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
        let label: SharedString = self
            .selected
            .map(|selected| self.entry_label(selected).into())
            .unwrap_or_else(|| self.spectrum_label.clone());
        let path = self.current_path.clone();
        self.load_spectrum(ix, path, label, cx);
        // Parameter edits also invalidate the operando overview + overlay.
        if self.workspace == Workspace::Operando {
            self.ensure_operando(cx);
        }
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
        let key = (ix, self.effective_fingerprint(ix));

        if let Some(sp) = self.cache.get(&key) {
            self.load_running = false;
            let sp = sp.clone();
            self.set_processed(ix, label, path, key.1, sp, cx);
            cx.notify();
            return;
        }

        self.status = format!("processing {label} ...").into();
        self.load_running = true;
        cx.notify();
        let params = self.effective_params(ix).clone();
        let derived = (ix >= DERIVED_BASE)
            .then(|| self.derived.get(ix - DERIVED_BASE).cloned())
            .flatten();
        let processed_path = path.clone();
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
                app.load_running = false;
                match result {
                    Ok(sp) => {
                        let sp = Arc::new(sp);
                        app.cache.put(key, sp.clone());
                        app.set_processed(ix, label, processed_path, key.1, sp, cx);
                    }
                    Err(e) => {
                        app.status = format!("failed to process {label}: {e}").into();
                        app.record_job_error(label.to_string(), e.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_processed(
        &mut self,
        ix: usize,
        label: SharedString,
        path: PathBuf,
        fingerprint: u64,
        sp: Arc<XASSpectrum>,
        cx: &mut Context<Self>,
    ) {
        self.status = spectrum_status(&label, &sp);
        self.spectrum_label = label;
        self.spectrum_path = path;
        self.spectrum_fingerprint = fingerprint;
        // Surface the auto-determined E0 in the field placeholder.
        if self.effective_params(ix).e0.is_none()
            && let Some(e0) = sp.get_e0()
            && let Some((_, field)) = self
                .param_fields
                .iter()
                .find(|(key, _)| *key == ParamKey::E0)
        {
            field.update(cx, |f, cx| f.set_placeholder(format!("auto ({e0:.1})"), cx));
        }
        self.spectrum = Some(sp.clone());
        self.refresh_operando_frame_plot(ix, &sp, cx);
        self.rebuild_plots(cx);
    }

    // ---- operando ----------------------------------------------------------

    fn open_scan(&mut self, scan_ix: usize, cx: &mut Context<Self>) {
        self.active_scan = Some(scan_ix);
        self.expanded_scan = Some(scan_ix);
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
        let Some(scan) = self.catalog.scans.get(scan_ix) else {
            return;
        };
        let scan_start = scan.start;
        let scan_len = scan.len;
        let fingerprint = scan_fingerprint(&self.params, &self.overrides, scan_start, scan_len);
        if scan_len == 0
            || self.operando.as_ref().is_some_and(|o| {
                o.scan == scan_ix && o.scan_len == scan_len && o.fingerprint == fingerprint
            })
        {
            return;
        }
        let preserve_time_pos = self.operando.as_ref().is_some_and(|o| o.scan == scan_ix);

        if let Some(cancel) = self.operando_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.operando_gen += 1;
        let generation = self.operando_gen;
        let cancel = Arc::new(AtomicBool::new(false));
        self.operando_cancel = Some(cancel.clone());
        self.operando_running = true;
        // Even sampling across the scan; first and last frames included.
        let sample_ixs = sample_scan_indices(scan_start, scan_len, MAX_FRAMES);
        let frames: Vec<(usize, PathBuf, String)> = sample_ixs
            .iter()
            .map(|&ix| (ix, self.catalog.path(ix), self.catalog.name(ix).to_string()))
            .collect();
        // Global params plus the (usually tiny) override set for this scan;
        // frames look their effective params up by catalog index.
        let global = Arc::new(self.params.clone());
        let frame_overrides: Arc<BTreeMap<usize, PipelineParams>> = Arc::new(
            self.overrides
                .range(scan_start..scan_start.saturating_add(scan_len))
                .map(|(&ix, p)| (ix, p.clone()))
                .collect(),
        );
        let grid: Vec<f64> = (0..K_GRID_BINS)
            .map(|i| i as f64 * K_GRID_MAX / (K_GRID_BINS - 1) as f64)
            .collect();
        self.status = format!(
            "building scan overview ({} of {} frames) ...",
            frames.len(),
            scan_len
        )
        .into();

        let job_grid = grid.clone();
        let job_cancel = cancel.clone();
        let job = cx.background_executor().spawn(async move {
            frames
                .par_iter()
                .map(|(ix, path, label)| {
                    if job_cancel.load(Ordering::Relaxed) {
                        return None;
                    }
                    let params = frame_overrides.get(ix).unwrap_or(&global);
                    let result = process_file(path, params)
                        .map_err(|error| error.to_string())
                        .and_then(|sp| {
                            resample_chik(&sp, &job_grid)
                                .map(|row| (row, sp.get_e0().unwrap_or(f64::NAN)))
                                .ok_or_else(|| "processed spectrum has no chi(k)".to_string())
                        });
                    Some((label.clone(), result))
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let rows = job.await;
            this.update(cx, |app, cx| {
                if app.operando_gen != generation {
                    return;
                }
                app.operando_running = false;
                app.operando_cancel = None;
                if cancel.load(Ordering::Relaxed) {
                    app.status = "scan overview cancelled".into();
                    cx.notify();
                    return;
                }
                let mut matrix = Vec::with_capacity(rows.len());
                let mut e0s = Vec::with_capacity(rows.len());
                let mut failed = 0usize;
                for row in rows.into_iter().flatten() {
                    match row {
                        (_, Ok((values, e0))) => {
                            matrix.push(values);
                            e0s.push(e0);
                        }
                        (label, Err(error)) => {
                            failed += 1;
                            app.record_job_error(label, error);
                            matrix.push(vec![f64::NAN; grid.len()]);
                            e0s.push(f64::NAN);
                        }
                    }
                }
                let kweight = app.params.fft_kweight.unwrap_or(2.0);
                app.operando = Some(OperandoData {
                    scan: scan_ix,
                    scan_len,
                    fingerprint,
                    grid,
                    matrix,
                    e0s,
                    kweight,
                });
                // Parameter-only rebuilds preserve the full-scan cursor;
                // opening a different scan starts at its first frame.
                app.time_pos = app
                    .pending_time_pos
                    .take()
                    .map(|pos| pos.min(scan_len.saturating_sub(1)))
                    .unwrap_or_else(|| {
                        if preserve_time_pos {
                            app.time_pos.min(scan_len.saturating_sub(1))
                        } else {
                            0
                        }
                    });
                app.rebuild_operando_plots(cx);
                app.status = if failed == 0 {
                    "scan overview ready".into()
                } else {
                    format!("scan overview ready · {failed} failed").into()
                };
                app.sync_time_selection(scan_ix, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn rebuild_operando_plots(&mut self, cx: &mut Context<Self>) {
        let Some(data) = self.operando.as_ref() else {
            return;
        };
        let Some(scan) = self.catalog.scans.get(data.scan) else {
            return;
        };
        let scan_ix = data.scan;
        let sample_pos = nearest_sample_pos(self.time_pos, scan.len, data.matrix.len());
        let cursor_ix = scan.start + self.time_pos.min(scan.len.saturating_sub(1));
        let cursor_fingerprint = self.effective_fingerprint(cursor_ix);
        let grid = data.grid.clone();
        let sampled_row = data.matrix.get(sample_pos).cloned().unwrap_or_default();
        let row = self
            .cache
            .peek(&(cursor_ix, cursor_fingerprint))
            .and_then(|sp| resample_chik(sp, &grid))
            .unwrap_or(sampled_row);
        let heatmap = build_heatmap(&data.matrix, &data.grid, scan.len, &self.theme);
        let chik = build_frame_chik(&grid, &row, data.kweight, &self.theme);
        let (trend_values, trend_cursor) = self.trend_series();
        let trend_name = self.trend_name();
        let trend = build_trend(&trend_values, trend_cursor, &trend_name, &self.theme);
        match &mut self.operando_plots {
            Some(plots) => {
                if plots.scan == scan_ix {
                    set_plot_keep_view(&plots.heatmap, heatmap, cx);
                    set_plot_keep_view(&plots.chik, chik, cx);
                    set_plot_keep_view(&plots.trend, trend, cx);
                } else {
                    // New scan: fresh viewports, the old zoom is meaningless.
                    plots.scan = scan_ix;
                    plots.heatmap.update(cx, |rp, cx| rp.set_plot(heatmap, cx));
                    plots.chik.update(cx, |rp, cx| rp.set_plot(chik, cx));
                    plots.trend.update(cx, |rp, cx| rp.set_plot(trend, cx));
                }
            }
            None => {
                self.operando_plots = Some(OperandoPlots {
                    scan: scan_ix,
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

    fn operando_scan_len(&self) -> Option<usize> {
        self.operando
            .as_ref()
            .and_then(|data| self.catalog.scans.get(data.scan))
            .map(|scan| scan.len)
    }

    fn step_time_percent(&mut self, direction: isize, cx: &mut Context<Self>) {
        let Some(len) = self.operando_scan_len() else {
            return;
        };
        let step = len.div_ceil(100).max(1) as isize;
        self.step_time(direction * step, cx);
    }

    fn set_time_pos(&mut self, pos: usize, cx: &mut Context<Self>) {
        let Some((scan_ix, ix)) = self.update_operando_cursor(pos, cx) else {
            return;
        };
        if self.selected != Some(ix) {
            self.sync_time_selection(scan_ix, cx);
        }
        cx.notify();
    }

    /// Update only the operando cursor and its cursor-dependent plots. The
    /// caller decides whether selection also needs changing, preventing the
    /// two synchronization directions from recursively selecting each other.
    fn update_operando_cursor(
        &mut self,
        pos: usize,
        cx: &mut Context<Self>,
    ) -> Option<(usize, usize)> {
        let data = self.operando.as_ref()?;
        let scan = self.catalog.scans.get(data.scan)?;
        let scan_ix = data.scan;
        let scan_start = scan.start;
        let scan_len = scan.len;
        let pos = pos.min(scan_len.saturating_sub(1));
        let ix = scan_start + pos;
        if pos == self.time_pos {
            return Some((scan_ix, ix));
        }
        self.time_pos = pos;
        let sample_pos = nearest_sample_pos(pos, scan_len, data.matrix.len());
        let grid = data.grid.clone();
        let sampled_row = data.matrix.get(sample_pos).cloned().unwrap_or_default();
        let fingerprint = self.effective_fingerprint(ix);
        let row = self
            .cache
            .peek(&(ix, fingerprint))
            .and_then(|sp| resample_chik(sp, &grid))
            .unwrap_or(sampled_row);
        let chik = build_frame_chik(&grid, &row, data.kweight, &self.theme);
        let (trend_values, trend_cursor) = self.trend_series();
        let trend_name = self.trend_name();
        let trend = build_trend(&trend_values, trend_cursor, &trend_name, &self.theme);
        if let Some(plots) = &self.operando_plots {
            // Scrub steps swap data only; the user's zoom survives.
            set_plot_keep_view(&plots.chik, chik, cx);
            set_plot_keep_view(&plots.trend, trend, cx);
        }
        self.status = format!("frame {}/{} · {}", pos + 1, scan_len, self.catalog.name(ix)).into();
        Some((scan_ix, ix))
    }

    /// Selection-to-cursor half of operando synchronization. This deliberately
    /// does not select, so `set_time_pos` can safely use `select_entry` for the
    /// opposite direction without a ping-pong loop.
    fn sync_operando_cursor_to_entry(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(data) = &self.operando else {
            return;
        };
        let Some(scan) = self.catalog.scans.get(data.scan) else {
            return;
        };
        let Some(offset) = scan_entry_offset(scan.start, scan.len, ix) else {
            return;
        };
        if offset != self.time_pos {
            self.update_operando_cursor(offset, cx);
        }
    }

    /// Route the full-resolution cursor frame through the shared lazy,
    /// generation-counted spectrum load path and reveal it in the current
    /// data-panel list.
    fn sync_time_selection(&mut self, scan_ix: usize, cx: &mut Context<Self>) {
        let Some(scan) = self.catalog.scans.get(scan_ix) else {
            return;
        };
        let offset = self.time_pos.min(scan.len.saturating_sub(1));
        let ix = scan.start + offset;
        self.reveal_time_selection(scan_ix, offset, ix);
        self.selection.clear();
        self.select_entry(ix, cx);
    }

    fn reveal_time_selection(&mut self, scan_ix: usize, offset: usize, ix: usize) {
        match self.data_tab {
            DataTab::Files => {
                let visible_row = match &self.filtered {
                    Some(filtered) => filtered.binary_search(&ix).ok(),
                    None => Some(ix),
                };
                if let Some(row) = visible_row {
                    self.file_scroll
                        .scroll_to_item(row, ScrollStrategy::Nearest);
                } else {
                    // A filter can hide the cursor frame; the expanded scan
                    // remains an always-visible synchronized representation.
                    self.data_tab = DataTab::Scans;
                    self.expanded_scan = Some(scan_ix);
                    self.scan_scroll
                        .scroll_to_item(scan_ix + 1 + offset, ScrollStrategy::Nearest);
                }
            }
            DataTab::Scans => {
                self.expanded_scan = Some(scan_ix);
                self.scan_scroll
                    .scroll_to_item(scan_ix + 1 + offset, ScrollStrategy::Nearest);
            }
        }
    }

    /// Replace the sampled placeholder with the actual processed cursor
    /// frame when its generation-counted load completes.
    fn refresh_operando_frame_plot(&mut self, ix: usize, sp: &XASSpectrum, cx: &mut Context<Self>) {
        let Some(data) = &self.operando else {
            return;
        };
        let Some(scan) = self.catalog.scans.get(data.scan) else {
            return;
        };
        if data.fingerprint != scan_fingerprint(&self.params, &self.overrides, scan.start, scan.len)
            || ix != scan.start + self.time_pos.min(scan.len.saturating_sub(1))
        {
            return;
        }
        let Some(row) = resample_chik(sp, &data.grid) else {
            return;
        };
        let chik = build_frame_chik(&data.grid, &row, data.kweight, &self.theme);
        if let Some(plots) = &self.operando_plots {
            set_plot_keep_view(&plots.chik, chik, cx);
        }
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
    /// then rebuild the overlay. Every member is processed and cached under
    /// its own effective params.
    fn ensure_compare_loaded(&mut self, cx: &mut Context<Self>) {
        let (indices, _) = self.compare_indices();
        let mut missing: Vec<(usize, u64, Result<PathBuf, DerivedSpectrum>, PipelineParams)> =
            Vec::new();
        for &ix in &indices {
            if ix == NO_ENTRY {
                continue;
            }
            let fingerprint = self.effective_fingerprint(ix);
            if self.cache.contains(&(ix, fingerprint)) {
                continue;
            }
            let source = if ix >= DERIVED_BASE {
                match self.derived.get(ix - DERIVED_BASE) {
                    Some(d) => Err(d.clone()),
                    None => continue,
                }
            } else {
                Ok(self.catalog.path(ix))
            };
            missing.push((ix, fingerprint, source, self.effective_params(ix).clone()));
        }
        if missing.is_empty() {
            self.rebuild_plots(cx);
            cx.notify();
            return;
        }
        self.compare_gen += 1;
        let generation = self.compare_gen;
        self.compare_running = true;
        self.status = format!("processing {} spectra for overlay ...", missing.len()).into();
        cx.notify();
        let job = cx.background_executor().spawn(async move {
            missing
                .par_iter()
                .map(|(ix, fingerprint, source, params)| {
                    let result = match source {
                        Ok(path) => process_file(path, params),
                        Err(d) => process_arrays(d.energy.clone(), d.mu.clone(), params),
                    };
                    (*ix, *fingerprint, result)
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let results = job.await;
            this.update(cx, |app, cx| {
                if app.compare_gen != generation {
                    return;
                }
                app.compare_running = false;
                let mut failed = 0usize;
                for (ix, fingerprint, result) in results {
                    match result {
                        Ok(sp) => {
                            app.cache.put((ix, fingerprint), Arc::new(sp));
                        }
                        Err(error) => {
                            failed += 1;
                            app.record_job_error(app.entry_label(ix), error.to_string());
                        }
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
        let (indices, total) = self.compare_indices();
        let mut traces: Vec<QuadTrace> = Vec::new();
        for ix in indices {
            if ix == NO_ENTRY {
                continue;
            }
            let label = self.entry_label(ix);
            let fingerprint = self.effective_fingerprint(ix);
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
        // The grid uses one shared legend strip; per-plot legends only when a
        // quadrant is maximized.
        self.legend_entries = if traces.len() > 1 {
            traces
                .iter()
                .enumerate()
                .map(|(i, trace)| {
                    (
                        SharedString::from(middle_truncate(&trace.label, 28)),
                        trace_rgba(&self.theme, i),
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        let in_plot_legend = self.maximized.is_some();
        let plots = build_quadrants_multi(&traces, &self.view, &self.theme, in_plot_legend);
        let [t0, t1, t2, t3] = plots.titles;
        let titled = [
            (t0, plots.mu_e),
            (t1, plots.norm),
            (t2, plots.chi_k),
            (t3, plots.chi_r),
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
            for ((slot, entity), (title, plot)) in self.quadrants.iter_mut().zip(titled) {
                *slot = SharedString::from(title);
                set_plot_keep_view(entity, plot, cx);
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
            var.min_field.update(cx, |f, cx| f.set_theme(theme, cx));
            var.max_field.update(cx, |f, cx| f.set_theme(theme, cx));
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

    /// Open a folder: restore the persisted index instantly when one exists
    /// (doc: "reopening a million-file project is < 1 s"), falling back to a
    /// streaming walk. Either way the tree is (re)walked in the background —
    /// as the primary scan or as the freshness re-check.
    fn scan_folder(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.reset_catalog_state(cx);
        self.source_dir = Some(root.clone());
        self.status = format!("checking catalog index for {} ...", root.display()).into();
        let catalog_gen = self.catalog_gen;
        let probe_root = root.clone();
        let probe = cx.background_executor().spawn(async move {
            let path = index_cache_path(&probe_root);
            let exists = path.as_ref().is_some_and(|path| path.exists());
            (path, exists)
        });
        cx.spawn(async move |this, cx| {
            let (index_path, exists) = probe.await;
            this.update(cx, |app, cx| {
                if app.catalog_gen != catalog_gen {
                    return;
                }
                app.catalog_index_path = index_path.clone();
                if let Some(index_path) = index_path.filter(|_| exists) {
                    app.load_catalog_index(root, index_path, cx);
                } else {
                    app.start_live_scan(root, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Restore the catalog from its persisted index on the background
    /// executor, then start the freshness re-walk. Falls back to a live scan
    /// if the file is stale/corrupt.
    fn load_catalog_index(&mut self, root: PathBuf, index_path: PathBuf, cx: &mut Context<Self>) {
        let catalog_gen = self.catalog_gen;
        self.status = format!("loading index for {} ...", root.display()).into();
        let job = cx.background_executor().spawn({
            let root = root.clone();
            async move { load_index(&index_path, &root) }
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                if app.catalog_gen != catalog_gen {
                    return;
                }
                match result {
                    Ok(catalog) => {
                        let total = catalog.len();
                        app.catalog = catalog;
                        app.resolve_pending_overrides(cx);
                        app.status =
                            format!("index loaded · {total} files · checking for changes ...")
                                .into();
                        if total > 0 && app.selected.is_none() {
                            app.select_entry(0, cx);
                        }
                        if !app.filter_text.is_empty() {
                            app.apply_filter(cx);
                        }
                        app.start_verify_scan(root, cx);
                    }
                    Err(e) => {
                        app.record_job_error("catalog index", e);
                        app.start_live_scan(root, cx);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Freshness re-check behind a restored index: walk the tree into a
    /// shadow catalog off the live one, then reconcile — identical walks are
    /// a no-op, otherwise the fresh catalog replaces the stale index.
    fn start_verify_scan(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.verify_running = true;
        let catalog_gen = self.catalog_gen;
        let mut rx = start_scan(root.clone());
        cx.spawn(async move |this, cx| {
            let mut shadow = Catalog::default();
            loop {
                let Some(event) = rx.next().await else {
                    return; // scanner died without Done; keep the loaded index
                };
                let superseded = this
                    .update(cx, |app, _| app.catalog_gen != catalog_gen)
                    .unwrap_or(true);
                if superseded {
                    return;
                }
                match event {
                    ScanEvent::Batch(batch) => shadow.extend(batch),
                    ScanEvent::Done { .. } => break,
                    ScanEvent::Error(e) => {
                        this.update(cx, |app, cx| {
                            app.verify_running = false;
                            app.record_job_error("index freshness check", e);
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                }
            }
            let Ok(Some(live_parts)) = this.update(cx, |app, _| {
                (app.catalog_gen == catalog_gen).then(|| app.catalog.index_parts())
            }) else {
                return;
            };
            // Million-entry comparison stays off the UI thread.
            let compare = cx.background_executor().spawn(async move {
                let unchanged = live_parts.same_index(&shadow.index_parts());
                (unchanged, shadow)
            });
            let (unchanged, shadow) = compare.await;
            this.update(cx, |app, cx| {
                if app.catalog_gen != catalog_gen {
                    return;
                }
                app.verify_running = false;
                if unchanged {
                    app.status = format!("index verified · {} files", app.catalog.len()).into();
                } else {
                    let before = app.catalog.len();
                    app.install_refreshed_catalog(shadow, cx);
                    app.status = format!(
                        "index refreshed · {} files (was {before})",
                        app.catalog.len()
                    )
                    .into();
                    app.persist_catalog_index(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Swap in a freshly walked catalog after the index diverged. Everything
    /// keyed by catalog indices is invalidated; the active spectrum is
    /// re-located by path so the plots keep their subject when it survived.
    fn install_refreshed_catalog(&mut self, catalog: Catalog, cx: &mut Context<Self>) {
        self.generation += 1;
        self.compare_gen += 1;
        self.operando_gen += 1;
        self.batch_gen += 1;
        self.merge_gen += 1;
        // Filter results contain catalog indices. Invalidate them before the
        // catalog swap so no render can observe old indices with new entries.
        self.filter_gen += 1;
        self.filtered = None;
        for cancel in [
            self.operando_cancel.take(),
            self.batch_cancel.take(),
            self.merge_cancel.take(),
        ]
        .into_iter()
        .flatten()
        {
            cancel.store(true, Ordering::Relaxed);
        }
        self.load_running = false;
        self.compare_running = false;
        self.merge_running = false;
        self.operando_running = false;
        self.batch_running = false;
        // Indices shift under a refreshed walk; re-key overrides by path so
        // surviving files keep their per-spectrum params.
        let path_overrides: Vec<(PathBuf, PipelineParams)> = self
            .overrides
            .iter()
            .filter(|&(&ix, _)| ix < self.catalog.len())
            .map(|(&ix, params)| (self.catalog.path(ix), params.clone()))
            .collect();
        self.catalog = catalog;
        self.overrides = path_overrides
            .into_iter()
            .filter_map(|(path, params)| self.catalog.find_by_path(&path).map(|ix| (ix, params)))
            .collect();
        self.selection.clear();
        self.cache.clear();
        self.expanded_scan = None;
        self.active_scan = None;
        self.operando = None;
        self.operando_plots = None;
        self.time_pos = 0;
        self.pending_time_pos = None;
        self.batch_fit = None;
        self.selected = (!self.current_path.as_os_str().is_empty())
            .then(|| (0..self.catalog.len()).find(|&ix| self.catalog.path(ix) == self.current_path))
            .flatten();
        if !self.filter_text.is_empty() {
            self.apply_filter(cx);
        }
        self.sync_param_fields(cx);
        cx.notify();
    }

    /// Persist the current catalog as the folder's index file (background;
    /// write errors land in the error center, success is silent).
    fn persist_catalog_index(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.source_dir.clone() else {
            return;
        };
        let Some(path) = self.catalog_index_path.clone() else {
            return;
        };
        let parts = (!self.catalog.is_empty()).then(|| self.catalog.index_parts());
        let job = cx.background_executor().spawn(async move {
            if let Some(parts) = parts {
                write_index(&path, &root, &parts)
            } else {
                match std::fs::remove_file(&path) {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(error.to_string()),
                }
            }
        });
        cx.spawn(async move |this, cx| {
            if let Err(e) = job.await {
                this.update(cx, |app, _| app.record_job_error("catalog index write", e))
                    .ok();
            }
        })
        .detach();
    }

    /// Full streaming walk into the live catalog (first open of a folder, or
    /// the fallback when no valid index exists).
    fn start_live_scan(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.catalog.scanning = true;
        self.status = format!("scanning {} ...", root.display()).into();
        let catalog_gen = self.catalog_gen;
        let mut rx = start_scan(root);
        cx.spawn(async move |this, cx| {
            while let Some(event) = rx.next().await {
                let done = matches!(event, ScanEvent::Done { .. } | ScanEvent::Error(_));
                let update = this.update(cx, |app, cx| {
                    if app.catalog_gen != catalog_gen {
                        return true;
                    }
                    match event {
                        ScanEvent::Batch(batch) => {
                            let first = app.catalog.is_empty();
                            let old_active_len = app
                                .active_scan
                                .and_then(|scan_ix| app.catalog.scans.get(scan_ix))
                                .map(|scan| scan.len);
                            app.catalog.extend(batch);
                            let active_extended = app.active_scan.is_some_and(|scan_ix| {
                                let new_len = app.catalog.scans.get(scan_ix).map(|scan| scan.len);
                                old_active_len.is_some() && new_len > old_active_len
                            });
                            if active_extended && app.workspace == Workspace::Operando {
                                app.ensure_operando(cx);
                            }
                            // Show something as soon as the index has anything.
                            if first && app.selected.is_none() {
                                app.select_entry(0, cx);
                            }
                        }
                        ScanEvent::Done { total } => {
                            app.catalog.scanning = false;
                            app.status = format!("indexed {total} files").into();
                            app.resolve_pending_overrides(cx);
                            app.persist_catalog_index(cx);
                            if !app.filter_text.is_empty() {
                                app.apply_filter(cx);
                            }
                            if app.active_scan.is_some() {
                                if let Some(operando) = &mut app.operando {
                                    operando.scan_len = usize::MAX;
                                }
                                if app.workspace == Workspace::Operando {
                                    app.ensure_operando(cx);
                                }
                            }
                        }
                        ScanEvent::Error(e) => {
                            app.catalog.scanning = false;
                            app.status = format!("scan failed: {e}").into();
                            app.record_job_error("catalog scan", e);
                        }
                    }
                    cx.notify();
                    done
                });
                if update.unwrap_or(true) {
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
        // Selection-count changes can flip the panel between a spectrum's
        // override and the globals.
        self.sync_param_fields(cx);
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
            self.sync_param_fields(cx);
            cx.notify();
        }
    }

    fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if !self.selection.is_empty() {
            self.selection.clear();
            self.rebuild_plots(cx);
            self.sync_param_fields(cx);
            cx.notify();
        }
    }

    /// Re-run the name filter on the background executor over a cheap
    /// [`crate::catalog::NameSnapshot`]; only the latest generation's result
    /// lands, so typing never blocks on a million-entry match pass.
    fn apply_filter(&mut self, cx: &mut Context<Self>) {
        self.filter_gen += 1;
        if self.filter_text.is_empty() {
            self.filtered = None;
            cx.notify();
            return;
        }
        let generation = self.filter_gen;
        let catalog_gen = self.catalog_gen;
        let pattern = self.filter_text.to_ascii_lowercase();
        let names = self.catalog.names_snapshot();
        let job = cx.background_executor().spawn(async move {
            let mut lower = String::new();
            let mut matches = Vec::new();
            for (ix, name) in names.iter().enumerate() {
                lower.clear();
                lower.push_str(name);
                lower.make_ascii_lowercase();
                if filter_match_lower(&lower, &pattern) {
                    matches.push(ix);
                }
            }
            matches
        });
        cx.spawn(async move |this, cx| {
            let matches = job.await;
            this.update(cx, |app, cx| {
                if app.filter_gen != generation || app.catalog_gen != catalog_gen {
                    return;
                }
                app.filtered = Some(Arc::new(matches));
                cx.notify();
            })
            .ok();
        })
        .detach();
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
        let kept: BTreeSet<usize> = self.selection.iter().copied().step_by(10).collect();
        self.selection = kept;
        self.ensure_compare_loaded(cx);
        self.sync_param_fields(cx);
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
        let trim = |name: &str| name.trim_end_matches(".dat").to_string();
        let label = format!(
            "avg{} {}..{}",
            files.len(),
            trim(self.catalog.name(files[0])),
            trim(self.catalog.name(*files.last().unwrap()))
        );
        // Each input loads under its own effective params (per-file import
        // config / alignment overrides apply to the merge too).
        let sources: Vec<(PathBuf, PipelineParams)> = files
            .iter()
            .map(|&ix| (self.catalog.path(ix), self.effective_params(ix).clone()))
            .collect();
        let catalog_gen = self.catalog_gen;
        if let Some(cancel) = self.merge_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.merge_gen += 1;
        let generation = self.merge_gen;
        let cancel = Arc::new(AtomicBool::new(false));
        self.merge_cancel = Some(cancel.clone());
        self.merge_running = true;
        self.status = format!("merging {} spectra ...", files.len()).into();
        cx.notify();
        let job_cancel = cancel.clone();
        let job = cx.background_executor().spawn(async move {
            // Stream a running sum: memory stays bounded at one input plus
            // the accumulator no matter how many spectra are merged.
            let mut iter = sources.iter();
            let (first_path, first_params) = iter
                .next()
                .ok_or_else(|| "need at least 2 spectra to merge".to_string())?;
            let (energy, mu) = load_raw(first_path, first_params)?;
            let mut acc = StreamingAverage::new(energy, mu);
            for (path, params) in iter {
                if job_cancel.load(Ordering::Relaxed) {
                    return Err("merge cancelled".to_string());
                }
                let (energy, mu) = load_raw(path, params)?;
                acc.add(&energy, &mu);
            }
            acc.finish()
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                if app.catalog_gen != catalog_gen || app.merge_gen != generation {
                    return;
                }
                app.merge_running = false;
                app.merge_cancel = None;
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
                        if cancel.load(Ordering::Relaxed) {
                            app.status = "merge cancelled".into();
                        } else {
                            app.status = format!("merge failed: {e}").into();
                            app.record_job_error(format!("merge: {label}"), e);
                        }
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
        self.sync_param_fields(cx);
        cx.notify();
    }

    /// Add all filter results to the compare set.
    fn select_filter_results(&mut self, cx: &mut Context<Self>) {
        if let Some(filtered) = &self.filtered {
            self.selection.extend(filtered.iter().copied());
            self.ensure_compare_loaded(cx);
            self.sync_param_fields(cx);
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
            self.current_path = PathBuf::new();
            self.sync_param_fields(cx);
            self.load_spectrum(ix, PathBuf::new(), label, cx);
            return;
        }
        if ix >= self.catalog.len() {
            return;
        }
        self.selected = Some(ix);
        self.sync_operando_cursor_to_entry(ix, cx);
        let label: SharedString = self.catalog.name(ix).to_string().into();
        let path = self.catalog.path(ix);
        self.current_path = path.clone();
        self.update_import_preview(cx);
        // The panel must show this spectrum's effective (override or
        // global) params.
        self.sync_param_fields(cx);
        self.load_spectrum(ix, path, label, cx);
    }

    /// Bounded-read preview of the current file on the background executor;
    /// the result is dropped if the selection moved on before it arrived.
    fn update_import_preview(&mut self, cx: &mut Context<Self>) {
        let path = self.current_path.clone();
        if path.as_os_str().is_empty() {
            self.import_preview = None;
            self.import_preview_error = "no preview".into();
            return;
        }
        self.import_preview_gen += 1;
        let generation = self.import_preview_gen;
        let import = self.ui_params().import.clone();
        let job = cx.background_executor().spawn({
            let path = path.clone();
            async move { preview_import(&path, &import) }
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                if app.current_path != path || app.import_preview_gen != generation {
                    return; // a newer selection superseded this preview
                }
                match result {
                    Ok(preview) => {
                        app.import_preview = Some(preview);
                        app.import_preview_error = "".into();
                    }
                    Err(_) => {
                        app.import_preview = None;
                        app.import_preview_error = "no preview".into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
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
                let field = cx.new(|cx| {
                    NumericField::new(label, "", Some(value), FieldKind::Float, theme, cx)
                });
                cx.subscribe(&field, move |this: &mut Self, _field, event, cx| {
                    let value = match event {
                        FieldEvent::Changed(value) => value,
                        FieldEvent::Invalid(message) => {
                            this.status = message.clone();
                            cx.notify();
                            return;
                        }
                    };
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
        let min_field = cx.new(|cx| TextInput::new("min", "", theme, cx));
        let max_field = cx.new(|cx| TextInput::new("max", "", theme, cx));
        let var_name = name.to_string();
        cx.subscribe(&field, move |this: &mut Self, _field, event, cx| {
            let InputEvent::Committed(text) = event else {
                return;
            };
            let text = text.trim().to_string();
            this.set_var_text(&var_name.clone(), &text, cx);
        })
        .detach();
        let var_name = name.to_string();
        cx.subscribe(&min_field, move |this: &mut Self, _field, event, cx| {
            let InputEvent::Committed(text) = event else {
                return;
            };
            this.set_var_bound(&var_name, true, text, cx);
        })
        .detach();
        let var_name = name.to_string();
        cx.subscribe(&max_field, move |this: &mut Self, _field, event, cx| {
            let InputEvent::Committed(text) = event else {
                return;
            };
            this.set_var_bound(&var_name, false, text, cx);
        })
        .detach();
        self.fit_vars.push(FitVar {
            spec: FitVarSpec {
                name: name.to_string(),
                value: default,
                vary: true,
                min: None,
                max: None,
                expr: None,
            },
            field,
            min_field,
            max_field,
        });
    }

    fn set_var_bound(&mut self, name: &str, is_min: bool, text: &str, cx: &mut Context<Self>) {
        let text = text.trim();
        let value = if text.is_empty() {
            None
        } else {
            match text.parse::<f64>() {
                Ok(value) if value.is_finite() => Some(value),
                _ => {
                    self.status = format!(
                        "invalid {} bound for {name}",
                        if is_min { "min" } else { "max" }
                    )
                    .into();
                    cx.notify();
                    return;
                }
            }
        };
        if let Some(var) = self.fit_vars.iter_mut().find(|var| var.spec.name == name) {
            if is_min {
                var.spec.min = value;
            } else {
                var.spec.max = value;
            }
        }
        cx.notify();
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
                let InputEvent::Committed(text) = event else {
                    return;
                };
                this.set_path_param(path_ix, param, text, cx);
            })
            .detach();
            (param, field)
        })
        .collect();
        self.fit_paths.push(FitPathRow {
            spec,
            meta,
            fields,
            expanded: false,
        });
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
        let provenance = FitProvenance {
            label: self.spectrum_label.clone(),
            path: self.spectrum_path.clone(),
            params_fingerprint: self.spectrum_fingerprint,
            model_fingerprint: self.fit_model_fingerprint(),
        };
        self.fit_running = true;
        self.status = format!("fitting {} ...", provenance.label).into();
        cx.notify();
        let paths: Vec<FitPathSpec> = self.fit_paths.iter().map(|r| r.spec.clone()).collect();
        let vars: Vec<FitVarSpec> = self.fit_vars.iter().map(|v| v.spec.clone()).collect();
        let ranges = self.fit_ranges;
        let started = Instant::now();
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
                app.last_fit_duration = Some(started.elapsed());
                match result {
                    Ok(result) => {
                        app.fit_provenance = Some(provenance.clone());
                        let stale = app.fit_is_stale();
                        app.status = format!(
                            "fit done for {}{} · R-factor {:.5} · red. chi² {:.3e}",
                            provenance.label,
                            if stale { " (stale)" } else { "" },
                            result.r_factor,
                            result.reduced_chi_square
                        )
                        .into();
                        let result = Arc::new(result);
                        app.fit_result = Some(result.clone());
                        app.rebuild_fit_plots(cx);
                        // Reflect fitted values back into the variable fields.
                        if !stale {
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
                            let model_fingerprint = app.fit_model_fingerprint();
                            if let Some(provenance) = &mut app.fit_provenance {
                                provenance.model_fingerprint = model_fingerprint;
                            }
                        }
                    }
                    Err(e) => {
                        app.status = format!("fit failed: {e}").into();
                        app.record_job_error(format!("fit: {}", provenance.label), e.to_string());
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
        let k_res = build_fit_residual_k(result, &self.theme);
        let r_res = build_fit_residual_r(result, &self.theme);
        match &self.fit_plots {
            Some(plots) => {
                set_plot_keep_view(&plots.k, k_plot, cx);
                set_plot_keep_view(&plots.k_residual, k_res, cx);
                set_plot_keep_view(&plots.r, r_plot, cx);
                set_plot_keep_view(&plots.r_residual, r_res, cx);
            }
            None => {
                self.fit_plots = Some(FitPlots {
                    k: plot_builder(k_plot).interactive().build(cx),
                    k_residual: plot_builder(k_res).interactive().build(cx),
                    r: plot_builder(r_plot).interactive().build(cx),
                    r_residual: plot_builder(r_res).interactive().build(cx),
                });
            }
        }
    }

    // ---- batch fitting -------------------------------------------------------

    fn batch_scope_line(&self) -> SharedString {
        let Some(scan_ix) = self.active_scan else {
            return "select a scan in the data panel first".into();
        };
        let Some(scan) = self.catalog.scans.get(scan_ix) else {
            return "selected scan is unavailable".into();
        };
        let count = if self.batch_preview {
            scan.len.min(MAX_FRAMES)
        } else {
            scan.len
        };
        let threads = rayon::current_num_threads().max(1);
        let waves = count.div_ceil(threads);
        let (per_fit, basis) = self
            .last_fit_duration
            .map(|duration| (duration, "last fit"))
            .unwrap_or((Duration::from_secs(1), "1 fit/s baseline"));
        let estimate = per_fit.mul_f64(waves as f64);
        if self.batch_preview {
            format!(
                "preview: {count} sampled frames · rough est. ~{} ({basis})",
                short_duration(estimate)
            )
            .into()
        } else {
            format!(
                "full scan · {count} frames · rough est. ~{} ({basis})",
                short_duration(estimate)
            )
            .into()
        }
    }

    fn toggle_batch_preview(&mut self, cx: &mut Context<Self>) {
        if !self.batch_running {
            self.batch_preview = !self.batch_preview;
            cx.notify();
        }
    }

    /// Fit the active scan with the current model in parallel on rayon. Full
    /// scan is the default; the overview sample is an explicit preview mode.
    /// Row/error events stream into retained state so cancellation is partial.
    fn run_batch_fit(&mut self, cx: &mut Context<Self>) {
        if self.batch_running {
            return;
        }
        let Some(scan_ix) = self.active_scan else {
            self.status = "select a scan in the data panel first".into();
            cx.notify();
            return;
        };
        if self.fit_paths.is_empty() {
            self.status = "no FEFF paths in the model".into();
            cx.notify();
            return;
        }
        let Some(scan) = self.catalog.scans.get(scan_ix) else {
            self.status = "selected scan is unavailable".into();
            cx.notify();
            return;
        };
        let scan_start = scan.start;
        let scan_len = scan.len;
        let fingerprint = scan_fingerprint(&self.params, &self.overrides, scan_start, scan_len);
        let model_fingerprint = self.fit_model_fingerprint();
        let preview = self.batch_preview;
        let indices = if preview {
            sample_scan_indices(scan_start, scan_len, MAX_FRAMES)
        } else {
            (scan_start..scan_start + scan_len).collect()
        };
        let frames: Vec<(usize, usize, PathBuf, String)> = indices
            .into_iter()
            .map(|ix| {
                (
                    ix - scan_start,
                    ix,
                    self.catalog.path(ix),
                    self.catalog.name(ix).to_string(),
                )
            })
            .collect();
        let total = frames.len();
        let frame_labels = frames
            .iter()
            .map(|(frame, _, _, label)| (*frame, label.clone()))
            .collect();
        self.batch_gen += 1;
        let generation = self.batch_gen;
        let cancel = Arc::new(AtomicBool::new(false));
        self.batch_running = true;
        self.batch_progress = (0, total);
        self.batch_cancel = Some(cancel.clone());
        self.batch_started = Some(Instant::now());
        self.batch_fit = Some(BatchFitData {
            scan: scan_ix,
            fingerprint,
            model_fingerprint,
            rows: Vec::new(),
            frame_labels,
            problems: Vec::new(),
            problems_open: false,
            preview,
            total,
            cancelled: false,
            varying_names: Vec::new(),
            trend_param: 0,
        });
        self.status = format!("batch fit 0/{total} ...").into();
        cx.notify();

        // Frames fit under their effective params: global set plus the
        // (usually tiny) override map for this scan, looked up by index.
        let global = Arc::new(self.params.clone());
        let frame_overrides: Arc<BTreeMap<usize, PipelineParams>> = Arc::new(
            self.overrides
                .range(scan_start..scan_start.saturating_add(scan_len))
                .map(|(&ix, p)| (ix, p.clone()))
                .collect(),
        );
        let paths: Vec<FitPathSpec> = self.fit_paths.iter().map(|r| r.spec.clone()).collect();
        let vars: Vec<FitVarSpec> = self.fit_vars.iter().map(|v| v.spec.clone()).collect();
        let ranges = self.fit_ranges;
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<BatchFitEvent>();

        let job_cancel = cancel.clone();
        let job = cx.background_executor().spawn(async move {
            frames
                .par_iter()
                .filter_map(|(frame, ix, path, label)| {
                    if job_cancel.load(Ordering::Relaxed) {
                        return None;
                    }
                    let result = (|| -> Result<BatchFitRow, String> {
                        let frame_params = frame_overrides.get(ix).unwrap_or(&global);
                        let sp =
                            process_file(path, frame_params).map_err(|error| error.to_string())?;
                        if job_cancel.load(Ordering::Relaxed) {
                            return Err("batch cancelled".into());
                        }
                        let k = sp
                            .get_k()
                            .ok_or_else(|| "processed spectrum has no k grid".to_string())?;
                        let chi = sp
                            .get_chi()
                            .ok_or_else(|| "processed spectrum has no chi(k)".to_string())?;
                        let result = run_fit(k, chi, &paths, &vars, ranges)?;
                        Ok(BatchFitRow::from_result(*frame, *ix, &result))
                    })();
                    if job_cancel.load(Ordering::Relaxed)
                        && result
                            .as_ref()
                            .is_err_and(|error| error == "batch cancelled")
                    {
                        return None;
                    }
                    let event = match result {
                        Ok(row) => BatchFitEvent::Row(row),
                        Err(error) => BatchFitEvent::Problem(BatchFitProblem {
                            frame: *frame,
                            label: label.clone(),
                            error,
                        }),
                    };
                    let _ = tx.unbounded_send(event.clone());
                    Some(event)
                })
                .collect::<Vec<_>>()
        });

        // Stream each completed row/problem into the visible retained result.
        cx.spawn(async move |this, cx| {
            while let Some(event) = rx.next().await {
                let stop = this
                    .update(cx, |app, cx| {
                        if app.batch_gen != generation {
                            return true;
                        }
                        if let BatchFitEvent::Problem(problem) = &event {
                            app.record_job_error(
                                format!("batch fit: {}", problem.label),
                                problem.error.clone(),
                            );
                        }
                        if let Some(batch) = &mut app.batch_fit {
                            match event {
                                BatchFitEvent::Row(row) => {
                                    if batch.varying_names.is_empty() {
                                        batch.varying_names = row
                                            .values
                                            .iter()
                                            .map(|(name, _, _)| name.clone())
                                            .collect();
                                    }
                                    batch.rows.push(row);
                                }
                                BatchFitEvent::Problem(problem) => {
                                    batch.problems.push(problem);
                                }
                            }
                        }
                        app.batch_progress.0 += 1;
                        let (done, total) = app.batch_progress;
                        let elapsed = app
                            .batch_started
                            .map(|started| started.elapsed())
                            .unwrap_or_default();
                        let rate = done as f64 / elapsed.as_secs_f64().max(0.001);
                        let eta = Duration::from_secs_f64(
                            total.saturating_sub(done) as f64 / rate.max(0.001),
                        );
                        app.status = format!(
                            "batch fit {done}/{total} · {rate:.1} frames/s · ETA {}",
                            short_duration(eta)
                        )
                        .into();
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
            let events = job.await;
            this.update(cx, |app, cx| {
                if app.batch_gen != generation {
                    return;
                }
                // Close the streaming consumer before replacing its partial
                // vectors with the final, sorted outcome set.
                app.batch_gen += 1;
                app.batch_running = false;
                app.batch_cancel = None;
                app.batch_started = None;
                let cancelled = cancel.load(Ordering::Relaxed);
                let mut rows = Vec::new();
                let mut problems = Vec::new();
                for event in events {
                    match event {
                        BatchFitEvent::Row(row) => rows.push(row),
                        BatchFitEvent::Problem(problem) => problems.push(problem),
                    }
                }
                rows.sort_by_key(|row| row.frame);
                problems.sort_by_key(|problem| problem.frame);
                let streamed_problem_frames: BTreeSet<usize> = app
                    .batch_fit
                    .as_ref()
                    .map(|batch| batch.problems.iter().map(|problem| problem.frame).collect())
                    .unwrap_or_default();
                for problem in problems
                    .iter()
                    .filter(|problem| !streamed_problem_frames.contains(&problem.frame))
                {
                    app.record_job_error(
                        format!("batch fit: {}", problem.label),
                        problem.error.clone(),
                    );
                }
                let completed = rows.len() + problems.len();
                app.batch_progress = (completed, total);
                if let Some(batch) = &mut app.batch_fit {
                    batch.varying_names = rows
                        .first()
                        .map(|row| row.values.iter().map(|(name, _, _)| name.clone()).collect())
                        .unwrap_or_default();
                    batch.rows = rows;
                    batch.problems = problems;
                    batch.cancelled = cancelled;
                }
                let batch = app.batch_fit.as_ref().expect("batch state exists");
                let skipped = total.saturating_sub(completed);
                app.status = if cancelled {
                    format!(
                        "batch fit cancelled · {} fitted · {} failed · {skipped} skipped",
                        batch.rows.len(),
                        batch.problems.len()
                    )
                    .into()
                } else {
                    format!(
                        "batch fit done · {} fitted · {} failed",
                        batch.rows.len(),
                        batch.problems.len()
                    )
                    .into()
                };
                app.rebuild_operando_plots(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn cancel_batch_fit(&mut self, cx: &mut Context<Self>) {
        if self.batch_running
            && let Some(cancel) = &self.batch_cancel
        {
            cancel.store(true, Ordering::Relaxed);
            self.status = "cancelling batch fit — completed rows will be kept ...".into();
            cx.notify();
        }
    }

    /// The operando trend series: fitted parameter when a batch fit exists
    /// for the current scan/params, otherwise E0.
    /// Trend values and cursor row; `trend_name` provides the plot's ylabel.
    fn trend_series(&self) -> (Vec<f64>, usize) {
        if let (Some(bf), Some(data)) = (&self.batch_fit, &self.operando)
            && bf.scan == data.scan
            && bf.fingerprint == data.fingerprint
            && bf.model_fingerprint == self.fit_model_fingerprint()
            && let Some(name) = bf.varying_names.get(bf.trend_param)
        {
            let scan_len = self
                .catalog
                .scans
                .get(bf.scan)
                .map(|scan| scan.len)
                .unwrap_or_default();
            let mut values = vec![f64::NAN; scan_len];
            for row in &bf.rows {
                if let (Some(slot), Some(v)) = (values.get_mut(row.frame), row.value_of(name)) {
                    *slot = v;
                }
            }
            return (values, self.time_pos);
        }
        let values = self
            .operando
            .as_ref()
            .map(|d| d.e0s.clone())
            .unwrap_or_default();
        let cursor = self
            .operando_scan_len()
            .map(|len| nearest_sample_pos(self.time_pos, len, values.len()))
            .unwrap_or_default();
        (values, cursor)
    }

    /// Name of the trend quantity (mirrors trend_series' selection) without
    /// materializing the full values vector; used by the plot ylabel.
    fn trend_name(&self) -> String {
        if let (Some(bf), Some(data)) = (&self.batch_fit, &self.operando)
            && bf.scan == data.scan
            && bf.fingerprint == data.fingerprint
            && bf.model_fingerprint == self.fit_model_fingerprint()
            && let Some(name) = bf.varying_names.get(bf.trend_param)
        {
            return name.clone();
        }
        "E0 (eV)".to_string()
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

    fn toggle_batch_problems(&mut self, cx: &mut Context<Self>) {
        if let Some(batch) = &mut self.batch_fit {
            batch.problems_open = !batch.problems_open;
            cx.notify();
        }
    }

    fn navigate_batch_row(
        &mut self,
        scan_ix: usize,
        frame: usize,
        entry_ix: usize,
        cx: &mut Context<Self>,
    ) {
        if self
            .operando
            .as_ref()
            .is_some_and(|data| data.scan == scan_ix)
        {
            self.set_time_pos(frame, cx);
            return;
        }
        self.active_scan = Some(scan_ix);
        self.pending_time_pos = Some(frame);
        self.time_pos = frame;
        self.ensure_operando(cx);
        self.selection.clear();
        self.select_entry(entry_ix, cx);
        self.status = format!("selected batch frame {}", frame + 1).into();
        cx.notify();
    }

    fn export_batch_csv(&mut self, cx: &mut Context<Self>) {
        if self.batch_fit_is_stale() {
            self.status =
                "batch results are stale — rerun with the current fit model to export".into();
            cx.notify();
            return;
        }
        let Some(bf) = &self.batch_fit else {
            return;
        };
        let csv = batch_csv(&bf.rows, &bf.varying_names, &bf.frame_labels);
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let rx = cx.prompt_for_new_path(std::path::Path::new(&home), Some("batch_fit.csv"));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                let (message, error) = match std::fs::write(&path, csv) {
                    Ok(()) => (format!("exported {}", path.display()), None),
                    Err(e) => (format!("export failed: {e}"), Some(e.to_string())),
                };
                this.update(cx, |app, cx| {
                    app.status = message.into();
                    if let Some(error) = error {
                        app.record_job_error("batch CSV export", error);
                    }
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
            Err(e) => {
                self.status = format!("feff.inp generation failed: {e}").into();
                self.record_job_error("feff.inp generation", e.to_string());
            }
        }
        cx.notify();
    }

    /// Create a template feff.inp workspace and open it in the system editor.
    fn new_feff_inp(&mut self, cx: &mut Context<Self>) {
        match crate::feffgen::new_workspace() {
            Ok(dir) => {
                let inp = dir.join("feff.inp");
                let _ = std::process::Command::new("open")
                    .arg("-t")
                    .arg(&inp)
                    .spawn();
                self.status = format!(
                    "feff.inp template at {} — edit, then Run FEFF10",
                    inp.display()
                )
                .into();
                self.feff_workspace = Some(dir);
            }
            Err(e) => {
                self.status = format!("failed to create feff workspace: {e}").into();
                self.record_job_error("FEFF workspace", e.to_string());
            }
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
                            if i >= 3
                                && let Some(row) = app.fit_paths.last_mut()
                            {
                                row.spec.enabled = false;
                            }
                        }
                        app.status = format!("FEFF10 done — imported {n} paths").into();
                    }
                    Err(e) => {
                        app.status = format!("FEFF10 failed: {e}").into();
                        app.record_job_error("FEFF10", e.to_string());
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
        // Overrides persist keyed by file path (indices are per-session);
        // still-unresolved pending ones survive a save-before-scan-finished.
        let mut overrides: Vec<ParamOverride> = self
            .overrides
            .iter()
            .filter(|&(&ix, _)| ix < self.catalog.len())
            .map(|(&ix, params)| ParamOverride {
                path: self.catalog.path(ix),
                params: params.clone(),
            })
            .collect();
        overrides.extend(self.pending_overrides.iter().cloned());
        ProjectFile {
            version: PROJECT_VERSION,
            source_dir: self.source_dir.clone(),
            params: self.params.clone(),
            overrides,
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
                let (message, error) = match crate::project::save(&path, &project) {
                    Ok(()) => (format!("saved {}", path.display()), None),
                    Err(e) => (format!("save failed: {e}"), Some(e.to_string())),
                };
                this.update(cx, |app, cx| {
                    app.status = message.into();
                    if let Some(error) = error {
                        app.record_job_error("save project", error);
                    }
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
                        this.update(cx, |app, cx| app.apply_project(project, cx))
                            .ok();
                    }
                    Err(e) => {
                        this.update(cx, |app, cx| {
                            app.status = format!("open failed: {e}").into();
                            app.record_job_error("open project", e.to_string());
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
        self.params = project.params;

        // Fit model: rebuild rows, variables, ranges.
        self.fit_paths.clear();
        self.fit_vars.clear();
        self.fit_gen += 1;
        self.fit_running = false;
        self.fit_result = None;
        self.fit_provenance = None;
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
                var.min_field.update(cx, |f, cx| {
                    f.set_text(
                        saved.min.map(|value| value.to_string()).unwrap_or_default(),
                        cx,
                    )
                });
                var.max_field.update(cx, |f, cx| {
                    f.set_text(
                        saved.max.map(|value| value.to_string()).unwrap_or_default(),
                        cx,
                    )
                });
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

        // Reopen the data source. Per-spectrum overrides wait path-keyed
        // until the (re)scanned catalog can resolve them to indices; with
        // no source there is no catalog for them to attach to.
        if let Some(dir) = project.source_dir.clone() {
            self.scan_folder(dir, cx);
            self.pending_overrides = project.overrides;
        } else {
            self.reset_catalog_state(cx);
            self.source_dir = None;
        }
        self.sync_param_fields(cx);
        self.status = "project loaded".into();
        if self.spectrum.is_some() {
            self.schedule_recompute(cx);
        }
        cx.notify();
    }

    // ---- views -------------------------------------------------------------

    fn set_workspace(&mut self, workspace: Workspace, cx: &mut Context<Self>) {
        self.workspace = workspace;
        if workspace == Workspace::Operando {
            self.ensure_operando(cx);
        }
        cx.notify();
    }

    fn focus_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.data_panel_open = true;
        self.data_tab = DataTab::Files;
        let input = self.filter_input.clone();
        cx.notify();
        // The panel may have been collapsed. Defer focus until the notified
        // render has placed the TextInput back into the dispatch tree.
        cx.defer_in(window, move |_this, window, cx| {
            if let Some(input) = input {
                input.read(cx).focus_handle(cx).focus(window, cx);
            }
        });
    }

    /// Maximize state changes rebuild the quadrants: the maximized view gets
    /// per-plot legends, the grid relies on the shared legend strip.
    fn set_maximized(&mut self, maximized: Option<usize>, cx: &mut Context<Self>) {
        if self.maximized == maximized {
            return;
        }
        self.maximized = maximized;
        self.rebuild_plots(cx);
        cx.notify();
    }

    fn maximize_quadrant(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.quadrants.len() == 4 {
            self.set_maximized(Some(index), cx);
        }
    }

    fn explore_escape(&mut self, cx: &mut Context<Self>) {
        if self.maximized.is_some() {
            self.set_maximized(None, cx);
        } else {
            self.clear_selection(cx);
        }
    }

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
                this.set_workspace(ws, cx);
            }))
            .child(label)
    }

    fn panel_toggle_button(
        &self,
        id: &'static str,
        label: &'static str,
        open: bool,
        toggle: fn(&mut Self),
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        div()
            .id(id)
            .w(px(40.))
            .h(px(28.))
            .my_0p5()
            .rounded_md()
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .text_color(if open { t.accent } else { t.text_muted })
            .when(open, |d| d.bg(t.raised))
            .hover(|d| d.bg(t.raised).text_color(t.text))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                toggle(this);
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
            .child(div().flex_1())
            .child(self.panel_toggle_button(
                "toggle-data-panel",
                "Data",
                self.data_panel_open,
                |this| this.data_panel_open = !this.data_panel_open,
                cx,
            ))
            .child(self.panel_toggle_button(
                "toggle-context-panel",
                "Ctx",
                self.context_panel_open,
                |this| this.context_panel_open = !this.context_panel_open,
                cx,
            ))
            .pb_2()
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
        uniform_list("catalog-files", count, move |range, _window, app| {
            let mut rows = Vec::with_capacity(range.len());
            for row in range {
                let (ix, name) = {
                    let state = entity.read(app);
                    let Some(ix) = catalog_row_index(
                        filtered.as_deref().map(Vec::as_slice),
                        row,
                        state.catalog.len(),
                    ) else {
                        continue;
                    };
                    let name: SharedString = state.catalog.name(ix).to_string().into();
                    (ix, name)
                };
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
                            entity.update(app, |this, cx| this.click_entry(ix, modifiers, cx));
                        })
                        .child(name),
                );
            }
            rows
        })
        .track_scroll(&self.file_scroll)
        .flex_1()
    }

    fn scan_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let entity = cx.entity();
        let active = self.active_scan;
        let selected = self.selected;
        let expanded_scan = self.expanded_scan;
        let expanded = expanded_scan.and_then(|scan_ix| {
            self.catalog
                .scans
                .get(scan_ix)
                .map(|scan| (scan_ix, scan.len))
        });
        let count = self.catalog.scans.len() + expanded.map(|(_, len)| len).unwrap_or(0);
        uniform_list("catalog-scans", count, move |range, _window, app| {
            let mut rows = Vec::with_capacity(range.len());
            for row in range {
                let Some(item) = scan_list_row(row, entity.read(app).catalog.scans.len(), expanded)
                else {
                    continue;
                };
                match item {
                    ScanListRow::Header(scan_ix) => {
                        let label: SharedString = {
                            let scan = &entity.read(app).catalog.scans[scan_ix];
                            format!("{} · {} spectra", scan.label, scan.len).into()
                        };
                        let is_active = active == Some(scan_ix);
                        let is_expanded = expanded_scan == Some(scan_ix);
                        let row_entity = entity.clone();
                        let button_entity = entity.clone();
                        rows.push(
                            div()
                                .id(("scan-header", scan_ix))
                                .h(px(24.))
                                .px_2()
                                .gap_1()
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
                                    row_entity.update(app, |this, cx| {
                                        if modifiers.shift || modifiers.platform {
                                            this.select_scan_range(scan_ix, cx);
                                        } else {
                                            this.active_scan = Some(scan_ix);
                                            this.expanded_scan = (this.expanded_scan
                                                != Some(scan_ix))
                                            .then_some(scan_ix);
                                            cx.notify();
                                        }
                                    });
                                })
                                .child(if is_expanded { "▾" } else { "▸" })
                                .child(div().flex_1().overflow_hidden().child(label))
                                .child(
                                    div()
                                        .id(("scan-operando", scan_ix))
                                        .px_1()
                                        .rounded_sm()
                                        .text_xs()
                                        .text_color(t.accent)
                                        .border_1()
                                        .border_color(t.border)
                                        .hover(|d| d.bg(t.surface))
                                        .cursor_pointer()
                                        .on_click(move |_: &ClickEvent, _window, app| {
                                            app.stop_propagation();
                                            button_entity.update(app, |this, cx| {
                                                this.open_scan(scan_ix, cx)
                                            });
                                        })
                                        .child("operando"),
                                ),
                        );
                    }
                    ScanListRow::Member { scan, offset } => {
                        let catalog_ix = {
                            let scan = &entity.read(app).catalog.scans[scan];
                            scan.start + offset
                        };
                        let label: SharedString =
                            entity.read(app).catalog.name(catalog_ix).to_string().into();
                        let is_active = selected == Some(catalog_ix);
                        let member_entity = entity.clone();
                        rows.push(
                            div()
                                .id(("scan-member", catalog_ix))
                                .h(px(24.))
                                .pl_6()
                                .pr_2()
                                .flex()
                                .items_center()
                                .text_sm()
                                .overflow_hidden()
                                .when(is_active, |d| d.bg(t.raised).text_color(t.accent))
                                .when(!is_active, |d| d.text_color(t.text))
                                .hover(|d| d.bg(t.raised))
                                .cursor_pointer()
                                .on_click(move |ev: &ClickEvent, window, app| {
                                    let modifiers = ev.modifiers();
                                    let focus = member_entity.read(app).data_focus.clone();
                                    window.focus(&focus, app);
                                    member_entity.update(app, |this, cx| {
                                        this.active_scan = Some(scan);
                                        this.click_entry(catalog_ix, modifiers, cx);
                                    });
                                })
                                .child(label),
                        );
                    }
                }
            }
            rows
        })
        .track_scroll(&self.scan_scroll)
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
            .on_action(
                cx.listener(|this: &mut Self, _: &NavExtendUp, _window, cx| {
                    this.nav_move(-1, true, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &NavExtendDown, _window, cx| {
                    this.nav_move(1, true, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &ClearCompare, _window, cx| {
                    this.clear_selection(cx);
                }),
            )
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
            .child(div().px_2().pb_1().children(self.filter_input.clone()))
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
                    .child(cmd(
                        "sel-scan",
                        "scan",
                        self.selected.is_some(),
                        |this, cx| this.select_active_scan(cx),
                    ))
                    .child(cmd(
                        "sel-tenth",
                        "1/10th",
                        self.selection.len() > 1,
                        |this, cx| this.thin_selection(cx),
                    ))
                    .child(cmd(
                        "sel-merge",
                        "merge",
                        self.selection
                            .iter()
                            .filter(|&&ix| ix < DERIVED_BASE)
                            .count()
                            >= 2,
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
                                        .whitespace_nowrap()
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
                let mut header = div().flex().items_center().gap_2().pr_2().child(
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
                            let next = if this.maximized == Some(index) {
                                None
                            } else {
                                Some(index)
                            };
                            this.set_maximized(next, cx);
                        }))
                        .child(title.clone()),
                );
                // Per-plot diagnostics live on the quadrant they affect.
                if index == 0 {
                    header = header
                        .child(self.view_chip(
                            "view-pre",
                            "pre",
                            self.view.show_pre,
                            cx,
                            |this, cx| {
                                this.view.show_pre = !this.view.show_pre;
                                this.rebuild_plots(cx);
                                cx.notify();
                            },
                        ))
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
                        .child(self.view_chip(
                            "view-e0",
                            "E0",
                            self.view.show_e0,
                            cx,
                            |this, cx| {
                                this.view.show_e0 = !this.view.show_e0;
                                this.rebuild_plots(cx);
                                cx.notify();
                            },
                        ))
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
                } else if index == 3 {
                    header = header.child(self.view_chip(
                        "view-re",
                        "Re",
                        self.view.show_re,
                        cx,
                        |this, cx| {
                            this.view.show_re = !this.view.show_re;
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

    /// Section-header chip (doc: per-spectrum override vs. global
    /// defaults): `global` when the shown spectrum follows the global set,
    /// `override ✕` (click = reset to global) when the section diverges.
    /// Absent while edits target the globals (multi-selection, no catalog
    /// selection).
    fn override_chip(
        &self,
        id: SharedString,
        section: ParamSection,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        self.override_target()?;
        let t = self.theme;
        let chip = if self.section_overridden(section) {
            div()
                .id(id)
                .px_1()
                .rounded_sm()
                .text_xs()
                .border_1()
                .border_color(t.warn)
                .text_color(t.warn)
                .cursor_pointer()
                .hover(|d| d.bg(t.raised))
                .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                    this.reset_section_override(section, cx);
                }))
                .child("override ✕")
                .into_any_element()
        } else {
            div()
                .px_1()
                .text_xs()
                .text_color(t.text_muted)
                .child("global")
                .into_any_element()
        };
        Some(chip)
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
            .child(
                self.view_chip("view-legend", "legend", self.view.legend, cx, |this, cx| {
                    this.view.legend = !this.view.legend;
                    this.rebuild_plots(cx);
                    cx.notify();
                }),
            )
            .child(
                self.view_chip("view-grid", "grid", self.view.grid, cx, |this, cx| {
                    this.view.grid = !this.view.grid;
                    this.rebuild_plots(cx);
                    cx.notify();
                }),
            );
        row
    }

    /// One shared legend strip for the whole 2x2 grid (color chip +
    /// middle-truncated name per overlaid trace, in stable selection order).
    fn legend_strip(&self) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut row = div()
            .px_2()
            .py_1()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_3()
            .text_xs()
            .text_color(t.text_muted);
        for (label, color) in &self.legend_entries {
            row = row.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().w(px(10.)).h(px(10.)).rounded_xs().bg(*color))
                    .child(label.clone()),
            );
        }
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
        let show_strip = self.view.legend && !self.legend_entries.is_empty();
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
            .children(show_strip.then(|| self.legend_strip()))
    }

    /// Cursor line and pointer navigation over the heatmap. The y hit area
    /// approximates ruviz's proportional plot margins (documented by the
    /// HEATMAP_* constants) and maps directly into full-scan coordinates.
    fn heatmap_cursor_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let entity = cx.entity();
        let frames = self.operando_scan_len().unwrap_or(0).max(1);
        let time_pos = self.time_pos.min(frames - 1);
        let accent = self.theme.accent;
        canvas(
            move |_, _, _| (),
            move |bounds, _, window, _| {
                let plot_top = bounds.top() + bounds.size.height * HEATMAP_TOP_MARGIN;
                let plot_height = (bounds.size.height
                    * (1.0 - HEATMAP_TOP_MARGIN - HEATMAP_BOTTOM_MARGIN))
                    .max(px(1.));
                let fraction = if frames > 1 {
                    time_pos as f32 / (frames - 1) as f32
                } else {
                    0.0
                };
                let cursor_y = plot_top + plot_height * fraction;
                let line_left = bounds.left() + bounds.size.width * HEATMAP_LEFT_MARGIN;
                let line_width =
                    bounds.size.width * (1.0 - HEATMAP_LEFT_MARGIN - HEATMAP_RIGHT_WITH_COLORBAR);
                window.paint_quad(fill(
                    gpui::Bounds::new(
                        point(line_left, cursor_y - px(1.)),
                        size(line_width.max(px(1.)), px(2.)),
                    ),
                    accent,
                ));

                let entity = entity.clone();
                window.on_mouse_event(move |ev: &MouseDownEvent, _, window, app| {
                    if ev.button != gpui::MouseButton::Left || !bounds.contains(&ev.position) {
                        return;
                    }
                    let fraction = ((ev.position.y - plot_top) / plot_height).clamp(0.0, 1.0);
                    let frame = (fraction * (frames - 1) as f32).round() as usize;
                    let focus = entity.read(app).operando_focus.clone();
                    window.focus(&focus, app);
                    entity.update(app, |this, cx| this.set_time_pos(frame, cx));
                    app.stop_propagation();
                });
            },
        )
        .absolute()
        .inset_0()
        .size_full()
    }

    /// Scrub strip: clickable segments mapping linearly onto the full scan.
    fn time_scrubber(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        const SEGMENTS: usize = 96;
        let t = self.theme;
        let frames = self.operando_scan_len().unwrap_or(0).max(1);
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
                    .bg(if seg == active_seg {
                        t.accent
                    } else {
                        t.raised
                    })
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
        let frames = self.operando_scan_len().unwrap_or(0);
        let frame_label: SharedString = format!("frame {} / {frames}", self.time_pos + 1).into();
        let kw = self.operando.as_ref().map(|d| d.kweight).unwrap_or(2.0);
        let overview_label: SharedString =
            format!("scan overview · frame vs {} · k (1/Å)", chik_label(kw)).into();
        let chik_header: SharedString = "frame".into();
        let trend_header: SharedString = "trend".into();
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
            .on_action(
                cx.listener(|this: &mut Self, _: &FrameJumpBack, _window, cx| {
                    this.step_time_percent(-1, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &FrameJumpFwd, _window, cx| {
                    this.step_time_percent(1, cx);
                }),
            )
            .on_action(cx.listener(|this: &mut Self, _: &FrameFirst, _window, cx| {
                this.set_time_pos(0, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &FrameLast, _window, cx| {
                let last = this
                    .operando_scan_len()
                    .map(|len| len.saturating_sub(1))
                    .unwrap_or(0);
                this.set_time_pos(last, cx);
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
                            .child(overview_label),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .p_1()
                            .child(plots.heatmap.clone())
                            .child(self.heatmap_cursor_overlay(cx)),
                    )
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
                                    .child(chik_header),
                            )
                            .child(div().flex_1().p_1().child(plots.chik.clone())),
                    )
                    .child(
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
                                    .px_2()
                                    .py_1()
                                    .text_xs()
                                    .text_color(t.text_muted)
                                    .child(trend_header),
                            )
                            .child(div().flex_1().p_1().child(plots.trend.clone())),
                    ),
            )
            .into_any_element()
    }

    fn batch_results_table(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let entity = cx.entity();
        let stale = self.batch_fit_is_stale();
        let (count, names, summary): (usize, Vec<String>, SharedString) = self
            .batch_fit
            .as_ref()
            .map(|batch| {
                let state = if self.batch_running {
                    "running"
                } else if batch.cancelled {
                    "cancelled · partial"
                } else {
                    "complete"
                };
                let scope = if batch.preview {
                    "preview"
                } else {
                    "full scan"
                };
                (
                    batch.rows.len(),
                    batch.varying_names.clone(),
                    format!(
                        "Batch results · {scope} · {} fitted / {} · {} problems · {state}",
                        batch.rows.len(),
                        batch.total,
                        batch.problems.len()
                    )
                    .into(),
                )
            })
            .unwrap_or_else(|| (0, Vec::new(), "Batch results".into()));
        let table_width = px(430. + names.len() as f32 * 104.);
        let header_names = names.clone();
        let header = {
            let mut row = div()
                .h(px(26.))
                .min_w(table_width)
                .px_2()
                .flex()
                .items_center()
                .bg(t.surface)
                .border_b_1()
                .border_color(t.border)
                .text_xs()
                .text_color(t.text_muted)
                .child(div().w(px(54.)).child("frame"))
                .child(div().w(px(176.)).child("file"))
                .child(div().w(px(88.)).child("R-factor"))
                .child(div().w(px(112.)).child("red. chi²"));
            for name in header_names {
                row = row.child(div().w(px(104.)).child(SharedString::from(name)));
            }
            row
        };
        let row_names = names;
        let rows = uniform_list("batch-results", count, move |range, _window, app| {
            let mut rendered = Vec::with_capacity(range.len());
            for row_ix in range {
                let (row, label) = {
                    let state = entity.read(app);
                    let Some(batch) = state.batch_fit.as_ref() else {
                        continue;
                    };
                    let Some(row) = batch.rows.get(row_ix) else {
                        continue;
                    };
                    (
                        row.clone(),
                        batch
                            .frame_labels
                            .get(&row.frame)
                            .cloned()
                            .unwrap_or_default(),
                    )
                };
                let frame = row.frame;
                let entry_ix = row.entry_ix;
                let scan_ix = entity
                    .read(app)
                    .batch_fit
                    .as_ref()
                    .map(|batch| batch.scan)
                    .unwrap_or_default();
                let row_entity = entity.clone();
                let mut element = div()
                    .id(("batch-result-row", frame))
                    .h(px(26.))
                    .min_w(table_width)
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(t.border)
                    .text_xs()
                    .text_color(t.text)
                    .hover(|div| div.bg(t.raised))
                    .cursor_pointer()
                    .on_click(move |_: &ClickEvent, _window, app| {
                        row_entity.update(app, |this, cx| {
                            this.navigate_batch_row(scan_ix, frame, entry_ix, cx)
                        });
                    })
                    .child(div().w(px(54.)).child(format!("{}", frame + 1)))
                    .child(
                        div()
                            .w(px(176.))
                            .overflow_hidden()
                            .child(SharedString::from(label)),
                    )
                    .child(div().w(px(88.)).child(format!("{:.5}", row.r_factor)))
                    .child(
                        div()
                            .w(px(112.))
                            .child(format!("{:.4e}", row.reduced_chi_square)),
                    );
                for name in &row_names {
                    let value = row
                        .value_of(name)
                        .map(|value| format!("{value:.5}"))
                        .unwrap_or_else(|| "—".to_string());
                    element = element.child(div().w(px(104.)).child(value));
                }
                rendered.push(element);
            }
            rendered
        })
        .flex_1()
        .min_h_0();
        div()
            .h(px(260.))
            .min_h(px(160.))
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
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child(div().flex_1().child(summary))
                    .when(stale, |row| {
                        row.child(
                            div()
                                .px_1()
                                .rounded_sm()
                                .border_1()
                                .border_color(t.error)
                                .text_color(t.error)
                                .child("stale — fit model changed"),
                        )
                    }),
            )
            .child(
                div()
                    .id("batch-results-horizontal")
                    .flex_1()
                    .min_h_0()
                    .overflow_x_scroll()
                    .child(
                        div()
                            .min_w(table_width)
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(header)
                            .child(rows),
                    ),
            )
    }

    fn fit_center(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        if self.fit_plots.is_none() && self.batch_fit.is_none() {
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
        }
        let card = |title: SharedString, plot: Entity<RuvizPlot>| {
            div()
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
        // Data-vs-model overlay with a residual strip beneath (doc: Fit
        // workspace "below, a residual strip").
        let column = |title: SharedString, main: Entity<RuvizPlot>, residual: Entity<RuvizPlot>| {
            div()
                .flex_1()
                .min_h_0()
                .m_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(card(title, main).flex_1().min_h_0())
                .child(card("residual".into(), residual).h(px(140.)))
        };
        let mut center = div().flex_1().min_h_0().flex().flex_col();
        if let Some(provenance) = &self.fit_provenance {
            let stale = self.fit_is_stale();
            let badge: SharedString = if stale {
                format!(
                    "fitted: {} · stale — active spectrum or fit inputs changed",
                    provenance.label
                )
                .into()
            } else {
                format!("fitted: {}", provenance.label).into()
            };
            center = center.child(
                div()
                    .mx_1()
                    .mt_1()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .border_1()
                    .border_color(if stale { t.error } else { t.accent })
                    .text_xs()
                    .text_color(if stale { t.error } else { t.accent })
                    .child(badge),
            );
        }
        if let Some(plots) = &self.fit_plots {
            center = center.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(column(
                        "fit in k-space".into(),
                        plots.k.clone(),
                        plots.k_residual.clone(),
                    ))
                    .child(column(
                        "fit in R-space".into(),
                        plots.r.clone(),
                        plots.r_residual.clone(),
                    )),
            );
        }
        if self.batch_fit.is_some() {
            center = center.child(self.batch_results_table(cx));
        }
        center.into_any_element()
    }

    fn batch_problems_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let entity = cx.entity();
        let count = self
            .batch_fit
            .as_ref()
            .map(|batch| batch.problems.len())
            .unwrap_or_default();
        uniform_list("batch-problems", count, move |range, _window, app| {
            let mut rows = Vec::with_capacity(range.len());
            for index in range {
                let problem = entity
                    .read(app)
                    .batch_fit
                    .as_ref()
                    .and_then(|batch| batch.problems.get(index))
                    .cloned();
                let Some(problem) = problem else {
                    continue;
                };
                rows.push(
                    div()
                        .h(px(44.))
                        .px_2()
                        .py_0p5()
                        .flex()
                        .flex_col()
                        .border_b_1()
                        .border_color(t.border)
                        .text_xs()
                        .child(div().text_color(t.error).child(SharedString::from(format!(
                            "frame {} · {}",
                            problem.frame + 1,
                            problem.label
                        ))))
                        .child(
                            div()
                                .overflow_hidden()
                                .text_color(t.text_muted)
                                .child(SharedString::from(problem.error)),
                        ),
                );
            }
            rows
        })
        .h(px(160.))
    }

    fn fit_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut panel = div()
            .id("fit-scroll")
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
                Some(m) => {
                    format!("R {:.3} Å · deg {:.0} · {} legs", m.reff, m.degen, m.nleg).into()
                }
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
                    .child(
                        div()
                            .w(px(72.))
                            .text_xs()
                            .text_color(t.text_muted)
                            .child(label),
                    )
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
                        .child(
                            feff_button("feff-choose", "Choose feff.inp...".into()).on_click(
                                cx.listener(|this, _: &ClickEvent, _window, cx| {
                                    this.choose_feff_inp(cx);
                                }),
                            ),
                        )
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
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w(px(56.))
                                    .text_sm()
                                    .text_color(t.text_muted)
                                    .child(name),
                            )
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
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, _window, cx| {
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
                                        },
                                    ))
                                    .child(badge),
                            ),
                    )
                    .child(
                        div()
                            .pl(px(58.))
                            .pt_0p5()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(div().text_xs().text_color(t.text_muted).child("min"))
                            .child(div().flex_1().child(var.min_field.clone()))
                            .child(div().text_xs().text_color(t.text_muted).child("max"))
                            .child(div().flex_1().child(var.max_field.clone())),
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
                    .child(if self.fit_running {
                        "fitting ..."
                    } else {
                        "Run Fit"
                    }),
            ),
        );

        // Batch over the active scan.
        panel = panel.child(self.section_header("Batch"));
        let batch_label: SharedString = if self.batch_running {
            let (done, total) = self.batch_progress;
            format!("Cancel batch fit ({done}/{total})").into()
        } else {
            "Run batch fit".into()
        };
        panel = panel.child(
            div()
                .px_3()
                .pb_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .id("batch-preview")
                        .px_1()
                        .py_0p5()
                        .rounded_sm()
                        .border_1()
                        .border_color(t.border)
                        .text_xs()
                        .text_color(if self.batch_preview {
                            t.accent
                        } else {
                            t.text_muted
                        })
                        .cursor_pointer()
                        .hover(|div| div.bg(t.raised))
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.toggle_batch_preview(cx);
                        }))
                        .child(if self.batch_preview {
                            "preview ✓ · sampled frames"
                        } else {
                            "preview off · full scan default"
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child(self.batch_scope_line()),
                )
                .child(
                    div()
                        .id("batch-fit")
                        .w_full()
                        .py_1()
                        .rounded_md()
                        .flex()
                        .justify_center()
                        .text_xs()
                        .bg(if self.batch_running {
                            t.error
                        } else {
                            t.raised
                        })
                        .text_color(if self.batch_running { t.bg } else { t.accent })
                        .cursor_pointer()
                        .hover(|d| d.bg(t.border))
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            if this.batch_running {
                                this.cancel_batch_fit(cx);
                            } else {
                                this.run_batch_fit(cx);
                            }
                        }))
                        .child(batch_label),
                ),
        );
        if let Some(bf) = &self.batch_fit {
            let batch_stale = self.batch_fit_is_stale();
            let scope = if bf.preview { "preview" } else { "full scan" };
            let summary: SharedString = format!(
                "{scope} · {} / {} fitted · trend: {}",
                bf.rows.len(),
                bf.total,
                bf.varying_names
                    .get(bf.trend_param)
                    .cloned()
                    .unwrap_or_default()
            )
            .into();
            panel = panel.child(
                div()
                    .px_3()
                    .pb_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(t.text_muted)
                            .child(summary),
                    )
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
                            .text_color(if batch_stale { t.text_muted } else { t.accent })
                            .when(!batch_stale, |button| {
                                button.cursor_pointer().hover(|d| d.bg(t.raised)).on_click(
                                    cx.listener(|this, _: &ClickEvent, _window, cx| {
                                        this.export_batch_csv(cx);
                                    }),
                                )
                            })
                            .child(if batch_stale { "CSV stale" } else { "CSV..." }),
                    ),
            );
            let problem_count = bf.problems.len();
            panel = panel.child(
                div()
                    .id("batch-problems-toggle")
                    .mx_3()
                    .mb_1()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .border_1()
                    .border_color(if problem_count > 0 { t.error } else { t.border })
                    .text_xs()
                    .text_color(if problem_count > 0 {
                        t.error
                    } else {
                        t.text_muted
                    })
                    .cursor_pointer()
                    .hover(|div| div.bg(t.raised))
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.toggle_batch_problems(cx);
                    }))
                    .child(SharedString::from(format!(
                        "{} problems {}",
                        problem_count,
                        if bf.problems_open { "▾" } else { "▸" }
                    ))),
            );
            if bf.problems_open {
                panel = panel.child(div().mx_3().mb_1().child(self.batch_problems_list(cx)));
            }
        }

        // Results.
        if let Some(result) = &self.fit_result {
            panel = panel.child(self.section_header("Results"));
            if let Some(provenance) = &self.fit_provenance {
                let stale = self.fit_is_stale();
                panel = panel.child(
                    div()
                        .mx_3()
                        .mb_1()
                        .px_1()
                        .py_0p5()
                        .rounded_sm()
                        .border_1()
                        .border_color(if stale { t.error } else { t.accent })
                        .text_xs()
                        .text_color(if stale { t.error } else { t.accent })
                        .child(SharedString::from(format!(
                            "fitted: {}{}",
                            provenance.label,
                            if stale { " · stale" } else { "" }
                        ))),
                );
            }
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
                    .gap_2()
                    .child(div().text_xs().text_color(t.accent).child("Import"))
                    .children(self.override_chip("ovr-import".into(), ParamSection::Import, cx))
                    .child(div().flex_1())
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
            let options = self.enum_options(EnumParam::ImportMode);
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
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(t.text_muted)
                            .child("mode"),
                    )
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
                                this.open_import_role = None;
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
            // The preview stays compact but shows enough real rows to verify
            // delimiter/header detection and the role assignments at a glance.
            if let Some(preview) = &self.import_preview {
                let header_status = if preview.names.is_some() {
                    "header names found"
                } else {
                    "no header names"
                };
                sections = sections.child(
                    div()
                        .px_3()
                        .pb_1()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child(format!(
                            "detected: {} columns · {header_status} · auto: {:?}",
                            preview.column_count, preview.auto_mode
                        )),
                );
                let mut header = div().flex();
                for column in 0..preview.column_count {
                    let name = preview
                        .names
                        .as_ref()
                        .and_then(|names| names.get(column))
                        .cloned()
                        .unwrap_or_else(|| format!("col {column}"));
                    let mut roles = Vec::new();
                    if preview.resolved.energy_col == column {
                        roles.push("E");
                    }
                    if preview.resolved.i0_col == column {
                        roles.push("I0");
                    }
                    if preview.resolved.it_col == column {
                        roles.push("It");
                    }
                    if preview.resolved.ir_col == column {
                        roles.push("Ir");
                    }
                    if preview.resolved.fluor_cols.contains(&column) {
                        roles.push("ROI");
                    }
                    if preview.resolved.mu_col == Some(column) {
                        roles.push("mu");
                    }
                    let assigned = !roles.is_empty();
                    header = header.child(
                        div()
                            .w(px(72.))
                            .flex_none()
                            .px_1()
                            .py_0p5()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(t.border)
                            .when(assigned, |cell| cell.bg(t.raised))
                            .child(div().text_xs().text_color(t.text).child(name))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if assigned { t.accent } else { t.text_muted })
                                    .child(if assigned {
                                        roles.join("/")
                                    } else {
                                        "—".into()
                                    }),
                            ),
                    );
                }
                let mut table = div()
                    .mb_1()
                    .border_1()
                    .border_color(t.border)
                    .flex()
                    .flex_col()
                    .child(header);
                for row in &preview.rows {
                    let mut line = div().flex().border_t_1().border_color(t.border);
                    for value in row {
                        line = line.child(
                            div()
                                .w(px(72.))
                                .flex_none()
                                .px_1()
                                .py_0p5()
                                .text_xs()
                                .text_color(t.text_muted)
                                .border_r_1()
                                .border_color(t.border)
                                .child(format!("{value:.4}")),
                        );
                    }
                    table = table.child(line);
                }
                sections = sections.child(
                    div()
                        .id("import-preview-horizontal")
                        .mx_3()
                        .overflow_x_scroll()
                        .child(table),
                );
            } else {
                sections = sections.child(
                    div()
                        .px_3()
                        .pb_1()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child(self.import_preview_error.clone()),
                );
            }
            if open {
                for role in [
                    ImportRole::Energy,
                    ImportRole::I0,
                    ImportRole::It,
                    ImportRole::Ir,
                    ImportRole::Fluor,
                    ImportRole::Mu,
                ] {
                    let expanded = self.open_import_role == Some(role);
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
                                    .child(Self::import_role_label(role)),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("import-role-{role:?}")))
                                    .w(px(150.))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .text_xs()
                                    .bg(t.bg)
                                    .border_1()
                                    .border_color(if expanded { t.accent } else { t.border })
                                    .text_color(t.text)
                                    .cursor_pointer()
                                    .hover(|picker| picker.border_color(t.accent))
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, _window, cx| {
                                            this.open_enum = None;
                                            this.open_import_role =
                                                if this.open_import_role == Some(role) {
                                                    None
                                                } else {
                                                    Some(role)
                                                };
                                            cx.notify();
                                        },
                                    ))
                                    .child(format!("{} ▾", self.import_role_current_label(role))),
                            ),
                    );
                    if expanded {
                        let manual = self.import_role_manual_column(role);
                        let fluor = self.ui_params().import.fluor_cols.as_ref();
                        let mut list = div()
                            .mx_3()
                            .mb_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(t.border)
                            .bg(t.bg)
                            .flex()
                            .flex_col();
                        let auto_selected = if role == ImportRole::Fluor {
                            fluor.is_none()
                        } else {
                            manual.is_none()
                        };
                        list = list.child(
                            div()
                                .id(SharedString::from(format!("import-role-{role:?}-auto")))
                                .px_2()
                                .py_0p5()
                                .text_xs()
                                .cursor_pointer()
                                .when(auto_selected, |item| item.bg(t.raised).text_color(t.accent))
                                .when(!auto_selected, |item| item.text_color(t.text))
                                .hover(|item| item.bg(t.raised))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                    if role == ImportRole::Fluor {
                                        this.toggle_import_fluor(None, cx);
                                        this.open_import_role = None;
                                    } else {
                                        this.set_import_role_column(role, None, cx);
                                    }
                                }))
                                .child(self.import_role_auto_label(role)),
                        );
                        let column_count = self
                            .import_preview
                            .as_ref()
                            .map(|preview| preview.column_count)
                            .unwrap_or(0);
                        for column in 0..column_count {
                            let is_selected = if role == ImportRole::Fluor {
                                fluor.is_some_and(|columns| columns.contains(&column))
                            } else {
                                manual == Some(column)
                            };
                            let option =
                                Self::import_column_label(self.import_preview.as_ref(), column);
                            list = list.child(
                                div()
                                    .id(SharedString::from(format!(
                                        "import-role-{role:?}-{column}"
                                    )))
                                    .px_2()
                                    .py_0p5()
                                    .text_xs()
                                    .cursor_pointer()
                                    .when(is_selected, |item| {
                                        item.bg(t.raised).text_color(t.accent)
                                    })
                                    .when(!is_selected, |item| item.text_color(t.text))
                                    .hover(|item| item.bg(t.raised))
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, _window, cx| {
                                            if role == ImportRole::Fluor {
                                                this.toggle_import_fluor(Some(column), cx);
                                            } else {
                                                this.set_import_role_column(role, Some(column), cx);
                                            }
                                        },
                                    ))
                                    .child(option),
                            );
                        }
                        sections = sections.child(list);
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
                                    .child("type ROI cols"),
                            )
                            .child(div().w(px(96.)).child(roi.clone())),
                    );
                }
                let align_on = self.ui_params().align_to_ref;
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
                                    let on = this.ui_params().align_to_ref;
                                    this.edit_params().align_to_ref = !on;
                                    this.schedule_recompute(cx);
                                    cx.notify();
                                }))
                                .child(if align_on {
                                    "✓ align to ref"
                                } else {
                                    "align to ref"
                                }),
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
                &[
                    ParamKey::FftDk2,
                    ParamKey::FftRmax,
                    ParamKey::FftKstep,
                    ParamKey::FftNfft,
                ],
                2,
            ),
        ];
        for (title, basics, advanced, fold) in groups {
            let open = self.adv_open[fold];
            let section = match fold {
                0 => ParamSection::Norm,
                1 => ParamSection::Bkg,
                _ => ParamSection::Fft,
            };
            sections = sections.child(
                div()
                    .px_3()
                    .pt_3()
                    .pb_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_xs().text_color(t.accent).child(title))
                    .children(self.override_chip(
                        SharedString::from(format!("ovr-{fold}")),
                        section,
                        cx,
                    ))
                    .child(div().flex_1())
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
                    let options = self.enum_options(which);
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
                .child("Enter/Tab/blur commits · empty = auto"),
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

    fn problems_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut list = div()
            .id("recent-problems-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        for error in self.job_errors.iter().rev() {
            list = list.child(
                div()
                    .px_3()
                    .py_1()
                    .border_b_1()
                    .border_color(t.border)
                    .text_xs()
                    .child(
                        div()
                            .text_color(t.error)
                            .child(SharedString::from(error.label.clone())),
                    )
                    .child(
                        div()
                            .text_color(t.text_muted)
                            .child(SharedString::from(error.message.clone())),
                    ),
            );
        }
        div()
            .h(px(180.))
            .w_full()
            .flex()
            .flex_col()
            .bg(t.surface)
            .border_t_1()
            .border_color(t.border)
            .child(
                div()
                    .h(px(28.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(t.border)
                    .text_xs()
                    .text_color(t.text)
                    .child(div().flex_1().child(format!(
                        "Recent problems ({}/{JOB_ERROR_CAPACITY})",
                        self.job_errors.len()
                    )))
                    .child(
                        div()
                            .id("clear-problems")
                            .px_2()
                            .rounded_sm()
                            .cursor_pointer()
                            .hover(|d| d.bg(t.raised))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.job_errors.clear();
                                this.problems_open = false;
                                cx.notify();
                            }))
                            .child("clear"),
                    ),
            )
            .child(list)
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let theme_label = match self.theme.mode {
            ThemeMode::Dark => "dark",
            ThemeMode::Light => "light",
        };
        let jobs = self.running_job_count();
        let selection = self.selection_count();
        let errors = self.job_errors.len();
        let mut bar = div()
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
            .child(format!("jobs:{jobs}"))
            .child(format!(
                "cache:{}/{}",
                self.cache.len(),
                PROCESSED_CACHE_CAPACITY
            ))
            .child(format!("{selection} selected"))
            .child(
                div()
                    .id("problems-toggle")
                    .px_2()
                    .rounded_sm()
                    .border_1()
                    .border_color(if errors > 0 { t.error } else { t.border })
                    .text_color(if errors > 0 { t.error } else { t.text_muted })
                    .cursor_pointer()
                    .hover(|d| d.bg(t.raised))
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.problems_open = !this.problems_open;
                        cx.notify();
                    }))
                    .child(format!("errors:{errors}")),
            );
        if self.operando_running || self.batch_running || self.merge_running {
            bar = bar.child(
                div()
                    .id("cancel-jobs")
                    .px_2()
                    .rounded_sm()
                    .text_color(t.error)
                    .cursor_pointer()
                    .hover(|d| d.bg(t.raised))
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.cancel_long_jobs(cx);
                    }))
                    .child("cancel jobs"),
            );
        }
        bar.child(
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
        let key_context = match self.workspace {
            Workspace::Explore => "Studio Explore",
            Workspace::Operando => "Studio OperandoWorkspace",
            Workspace::Fit => "Studio FitWorkspace",
        };
        let center = match self.workspace {
            Workspace::Explore => self.explore_center(cx).into_any_element(),
            Workspace::Operando => self.operando_center(cx).into_any_element(),
            Workspace::Fit => self.fit_center(cx).into_any_element(),
        };
        let data_panel = self
            .data_panel_open
            .then(|| self.data_panel(cx).into_any_element());
        let context_panel = if self.context_panel_open {
            Some(match self.workspace {
                Workspace::Fit => self.fit_panel(cx).into_any_element(),
                _ => self.context_panel(cx).into_any_element(),
            })
        } else {
            None
        };
        div()
            .id("root")
            .key_context(key_context)
            .on_action(
                cx.listener(|this: &mut Self, _: &WorkspaceExplore, _window, cx| {
                    this.set_workspace(Workspace::Explore, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &WorkspaceOperando, _window, cx| {
                    this.set_workspace(Workspace::Operando, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &WorkspaceFit, _window, cx| {
                    this.set_workspace(Workspace::Fit, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &ToggleDataPanel, _window, cx| {
                    this.data_panel_open = !this.data_panel_open;
                    cx.notify();
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &ToggleContextPanel, _window, cx| {
                    this.context_panel_open = !this.context_panel_open;
                    cx.notify();
                }),
            )
            .on_action(cx.listener(|this: &mut Self, _: &FocusFilter, window, cx| {
                this.focus_filter(window, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &Maximize1, _window, cx| {
                this.maximize_quadrant(0, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &Maximize2, _window, cx| {
                this.maximize_quadrant(1, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &Maximize3, _window, cx| {
                this.maximize_quadrant(2, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &Maximize4, _window, cx| {
                this.maximize_quadrant(3, cx);
            }))
            .on_action(
                cx.listener(|this: &mut Self, _: &RestoreGrid, _window, cx| {
                    this.set_maximized(None, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &ExploreEscape, _window, cx| {
                    this.explore_escape(cx);
                }),
            )
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
                    .children(data_panel)
                    .child(div().flex_1().flex().flex_col().child(center))
                    .children(context_panel),
            )
            .children(
                self.problems_open
                    .then(|| self.problems_panel(cx).into_any_element()),
            )
            .child(self.status_bar(cx))
    }
}
