//! Root view: workspace shell per doc/gui-ux-design.md.
//!
//! M1 scope: lazy catalog — "Open Folder" starts a background scan that
//! streams batches into a virtualized file list; clicking an entry parses and
//! processes it on the background executor (generation-counted so stale
//! results are dropped) with an LRU cache of processed spectra. The Explore
//! center is the 2x2 quadrant grid from M0.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::plot_ranges::plot_builder;
use futures::StreamExt;
use gpui::{
    ClickEvent, Context, Entity, ExternalPaths, FocusHandle, Focusable, IntoElement, KeyBinding,
    ParentElement, PathPromptOptions, Render, SharedString, Styled, UniformListScrollHandle,
    Window, actions, div, prelude::*, px, uniform_list,
};
use lru::LruCache;
use rexafs::prelude::XASSpectrum;
use ruviz::core::{Annotation, AnnotationId};
use ruviz::data::Observable;
use ruviz::render::{Color as PlotColor, LineStyle};
use ruviz_gpui::{InteractionOptions, PlotPointerEvent, PlotPointerEventKind, RuvizPlot};

use rayon::prelude::*;

use rexafs::prelude::FeffFitResult;
use rexafs::xafs::fitting::template::ParameterTemplate;
use rexafs::xafs::structure::{PathInfo, rank_paths, select_default, shells_of};

use crate::catalog::{Catalog, ScanEvent, index_cache_path, load_index, start_scan, write_index};
use crate::fitting::expr_identifiers;
use crate::fitting::{
    BatchFitRow, FitHistoryEntry, FitPathSpec, FitRanges, FitVarSpec, PathMeta, batch_csv,
    path_meta, run_spectrum_fit,
};
use crate::params::{
    AUTOBK_SOLVERS, DerivedSpectrum, DetectionMode, FT_WINDOWS, ImportPreview, PipelineParams,
    load_group_raw_with_diagnostics, preview_import, process_arrays, process_file, resample_chik,
};
use crate::plotting::{
    K_AXIS, QuadTrace, R_AXIS, SeriesSource, ViewOptions, build_fit_k, build_fit_q, build_fit_r,
    build_fit_residual_k, build_fit_residual_r, build_frame_chik_source, build_heatmap,
    build_trend, chik_label, chir_label, middle_truncate, trace_rgba,
};
use crate::project::{PROJECT_VERSION, ParamOverride, ProjectFile};
use crate::theme::Theme;
use crate::widgets::numeric_field::{FieldEvent, FieldKind, NumericField};
use crate::widgets::text_input::{InputEvent, TextInput};

mod group_rows;
mod import_channels;
mod import_preview;
mod import_repair;
pub(crate) mod import_review;
pub(crate) mod import_state;
mod importing;
mod merge;
mod param_text;
pub(crate) mod series_display;
mod series_source;
use series_source::OverviewSource;
mod shell;
pub(crate) use shell::controls::Tooltip;
use shell::{Stage, StageView, handles::HandleState, thumbnails::ThumbData, tools::ToolState};

/// Processed spectra kept in RAM. ~100-300 KB each, so 1024 ≈ a few hundred MB
/// worst case; browsing a million-file catalog stays bounded.
const PROCESSED_CACHE_CAPACITY: usize = 1024;
const JOB_ERROR_CAPACITY: usize = 200;
/// Raw arrays are small (two Vec<f64> per file); keep plenty around.
const RAW_CACHE_CAPACITY: usize = 256;
/// Raw (energy, mu) of one file after import math.
type RawArrays = Arc<crate::params::RawData>;
type GroupLoadResult = Result<(XASSpectrum, Option<RawArrays>), String>;
/// Live-follow recompute cadence while dragging a plot handle.
const DRAG_RECOMPUTE_TICK: Duration = Duration::from_millis(50);
/// Import-preview column width. Wide enough for a 4-decimal energy value
/// (`21912.2534`) so cells clip rather than wrapping a number onto two lines.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Workspace {
    Explore,
    Operando,
    Fit,
}

/// Which pipeline parameter a context-panel field edits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ParamKey {
    ImpEnergyCol,
    ImpI0Col,
    ImpItCol,
    ImpIrCol,
    AlignTarget,
    EnergyOffset,
    E0,
    EdgeStep,
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
    BkgNclamp,
    BkgClampLambda,
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
    BftRmin,
    BftRmax,
    BftDr,
    BkgEk0,
    BkgRegularization,
    BkgCondition,
    BkgResidualRatio,
    BftQmax,
    BftDr2,
    BftRweight,
    BftKstep,
    BftNfft,
}

/// Enum-valued parameters edited with cycling chips.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EnumParam {
    ImportMode,
    BkgWindow,
    BkgSolver,
    BkgClampPolicy,
    FftWindow,
    FftGrid,
    BftWindow,
    BkgFallback,
    BkgCache,
    BkgFallbackSolver,
}

/// Import role edited by a detected-column picker.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ImportRole {
    Energy,
    I0,
    It,
    Ir,
}

/// Context-panel section a parameter belongs to, for the per-section
/// global/override chips.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ParamSection {
    Import,
    Norm,
    Bkg,
    Fft,
}

/// The two group kinds share the same override semantics.
fn store_custom_params(
    catalog_len: usize,
    overrides: &mut BTreeMap<usize, PipelineParams>,
    derived: &mut [DerivedSpectrum],
    ix: usize,
    params: Option<PipelineParams>,
) {
    if ix >= DERIVED_BASE {
        if let Some(group) = derived.get_mut(ix - DERIVED_BASE) {
            group.params = params;
        }
    } else if ix < catalog_len {
        match params {
            Some(params) => {
                overrides.insert(ix, params);
            }
            None => {
                overrides.remove(&ix);
            }
        }
    }
}

/// Prepare on a clone: only an actual change may invalidate the preview or enter history.
fn prepare_parameter_edit(
    before: &PipelineParams,
    edit: impl FnOnce(&mut PipelineParams) -> anyhow::Result<()>,
) -> anyhow::Result<Option<PipelineParams>> {
    let mut after = before.clone();
    edit(&mut after)?;
    Ok((*before != after).then_some(after))
}

/// Apply only the selected source's latest preview, preserving read-error details.
fn finish_import_preview(
    current: (&std::path::Path, u64),
    requested: (&std::path::Path, u64),
    result: Result<ImportPreview, String>,
    preview: &mut Option<ImportPreview>,
    error: &mut SharedString,
) -> bool {
    if current != requested {
        return false;
    }
    match result {
        Ok(value) => {
            *preview = Some(value);
            *error = "".into();
        }
        Err(message) => {
            *preview = None;
            *error = message.into();
        }
    }
    true
}

/// Stepper / ↑↓ increment per parameter: energies in whole eV, k in half
/// wave-numbers, R and dk in tenths, counts in ones.
fn param_step(key: ParamKey) -> f64 {
    match key {
        ParamKey::E0 | ParamKey::BkgEk0 => 0.5,
        ParamKey::EnergyOffset => 0.1,
        ParamKey::EdgeStep => 0.01,
        ParamKey::PreEdgeStart
        | ParamKey::PreEdgeEnd
        | ParamKey::NormStart
        | ParamKey::NormEnd
        | ParamKey::AlignTarget => 5.0,
        ParamKey::BkgKmin | ParamKey::BkgKmax | ParamKey::FftKmin | ParamKey::FftKmax => 0.5,
        ParamKey::Rbkg | ParamKey::BkgDk | ParamKey::FftDk | ParamKey::FftDk2 | ParamKey::BftDr => {
            0.1
        }
        ParamKey::BftRmin | ParamKey::BftRmax | ParamKey::BftDr2 => 0.1,
        ParamKey::BkgKstep | ParamKey::FftKstep | ParamKey::BftKstep => 0.01,
        ParamKey::BftQmax => 0.5,
        ParamKey::BkgRegularization => 0.0001,
        ParamKey::BkgCondition => 1e7,
        ParamKey::BkgResidualRatio => 0.01,
        ParamKey::FftKweight => 1.0,
        ParamKey::BkgClampLambda => 0.001,
        _ => 1.0,
    }
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
            dst.set_energy_offset(src.energy_offset_ev)
                .expect("validated energy offset");
            dst.alignment_record = src.alignment_record.clone();
        }
        ParamSection::Norm => {
            dst.mback = src.mback.clone();
            dst.e0 = src.e0;
            dst.edge_step = src.edge_step;
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
            dst.bkg_kweight_linked = src.bkg_kweight_linked;
            dst.bkg_clamp_lo = src.bkg_clamp_lo;
            dst.bkg_clamp_hi = src.bkg_clamp_hi;
            dst.bkg_nclamp = src.bkg_nclamp;
            dst.bkg_clamp_lambda = src.bkg_clamp_lambda;
            dst.bkg_clamp_policy = src.bkg_clamp_policy;
            dst.bkg_window = src.bkg_window;
            dst.bkg_dk = src.bkg_dk;
            dst.bkg_solver = src.bkg_solver;
            dst.bkg_nfft = src.bkg_nfft;
            dst.bkg_ek0 = src.bkg_ek0;
            dst.bkg_linear_regularization = src.bkg_linear_regularization;
            dst.bkg_linear_condition_limit = src.bkg_linear_condition_limit;
            dst.bkg_linear_residual_ratio_limit = src.bkg_linear_residual_ratio_limit;
            dst.bkg_linear_fallback_to_lm = src.bkg_linear_fallback_to_lm;
            dst.bkg_linear_workspace_cache = src.bkg_linear_workspace_cache;
            dst.bkg_linear_fallback_solver = src.bkg_linear_fallback_solver;
            dst.bkg_standard = src.bkg_standard.clone();
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
            dst.bft_rmin = src.bft_rmin;
            dst.bft_rmax = src.bft_rmax;
            dst.bft_dr = src.bft_dr;
            dst.bft_window = src.bft_window;
            dst.bft_qmax = src.bft_qmax;
            dst.bft_dr2 = src.bft_dr2;
            dst.bft_rweight = src.bft_rweight;
            dst.bft_kstep = src.bft_kstep;
            dst.bft_nfft = src.bft_nfft;
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
    registry: &crate::group_identity::GroupRegistry,
    start: usize,
    len: usize,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    global.fingerprint().hash(&mut hasher);
    for (ix, params) in overrides.range(start..start.saturating_add(len)) {
        ix.hash(&mut hasher);
        params.fingerprint().hash(&mut hasher);
    }
    for ix in registry
        .excluded_indices()
        .range(start..start.saturating_add(len))
    {
        ix.hash(&mut hasher);
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
        ParamKey::EnergyOffset => Some(p.energy_offset_ev),
        ParamKey::E0 => p.e0,
        ParamKey::EdgeStep => p.edge_step,
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
        ParamKey::BkgNclamp => p.bkg_nclamp.map(|v| v as f64),
        ParamKey::BkgClampLambda => p.bkg_clamp_lambda,
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
        ParamKey::BftRmin => p.bft_rmin,
        ParamKey::BftRmax => p.bft_rmax,
        ParamKey::BftDr => p.bft_dr,
        ParamKey::BkgEk0 => p.bkg_ek0,
        ParamKey::BkgRegularization => p.bkg_linear_regularization,
        ParamKey::BkgCondition => p.bkg_linear_condition_limit,
        ParamKey::BkgResidualRatio => p.bkg_linear_residual_ratio_limit,
        ParamKey::BftQmax => p.bft_qmax,
        ParamKey::BftDr2 => p.bft_dr2,
        ParamKey::BftRweight => p.bft_rweight,
        ParamKey::BftKstep => p.bft_kstep,
        ParamKey::BftNfft => p.bft_nfft.map(|v| v as f64),
    }
}

const DETECTION_MODES: [DetectionMode; 4] = [
    DetectionMode::Transmission,
    DetectionMode::Fluorescence,
    DetectionMode::Reference,
    DetectionMode::MuColumn,
];

/// What the unsaved-changes prompt is guarding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CloseIntent {
    CloseWindow,
    Quit,
}

actions!(
    studio,
    [
        Quit,
        AssistantSend,
        AssistantStop,
        AssistantEscape,
        AssistantNextControl,
        AssistantPreviousControl,
        NavUp,
        NavDown,
        NavExtendUp,
        NavExtendDown,
        MarkAllGroups,
        InvertGroupMarks,
        ClearCompare,
        ToggleFocusedMark,
        CollapseFocusedStack,
        ExpandFocusedStack,
        LeaveFilter,
        EscapeFilter,
        RenameGroup,
        CommitGroupRename,
        DismissGroupEditor,
        FramePrev,
        FrameNext,
        FrameJumpBack,
        FrameJumpFwd,
        FrameFirst,
        FrameLast,
        StageData,
        StageProcess,
        StageNormalize,
        StageBackground,
        StageTransform,
        StageFit,
        StageSeries,
        StagePublish,
        ShowHelp,
        ShowLicenses,
        ShowStorage,
        ShowUpdates,
        ShowExample,
        ShowCopperSeries,
        SwitchTheme,
        ToggleDataPanel,
        ToggleContextPanel,
        FocusFilter,
        ExploreEscape,
        OpenProject,
        SaveProject,
        ImportPaths,
        DismissPathRoute,
        PaletteOpen,
        PaletteClose,
        Undo,
        Redo,
        JournalToggle,
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
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("secondary-enter", AssistantSend, Some("Assistant")),
        KeyBinding::new("secondary-.", AssistantStop, Some("Assistant")),
        KeyBinding::new("escape", AssistantEscape, Some("Assistant")),
        KeyBinding::new("tab", AssistantNextControl, Some("Assistant && !TextInput")),
        KeyBinding::new(
            "shift-tab",
            AssistantPreviousControl,
            Some("Assistant && !TextInput"),
        ),
        KeyBinding::new("secondary-o", OpenProject, Some("Studio && !Assistant")),
        KeyBinding::new("secondary-s", SaveProject, Some("Studio && !Assistant")),
        KeyBinding::new(
            "secondary-shift-o",
            ImportPaths,
            Some("Studio && !Assistant"),
        ),
        KeyBinding::new("escape", DismissPathRoute, Some("PathRoute")),
        KeyBinding::new("f2", RenameGroup, Some("DataPanel && !TextInput")),
        KeyBinding::new("enter", RenameGroup, Some("DataPanel && !TextInput")),
        KeyBinding::new("enter", CommitGroupRename, Some("GroupRename > TextInput")),
        KeyBinding::new(
            "escape",
            DismissGroupEditor,
            Some("GroupRename > TextInput"),
        ),
        KeyBinding::new("escape", DismissGroupEditor, Some("GroupMenu")),
        KeyBinding::new("up", NavUp, Some("DataPanel && !TextInput")),
        KeyBinding::new("down", NavDown, Some("DataPanel && !TextInput")),
        KeyBinding::new("shift-up", NavExtendUp, Some("DataPanel && !TextInput")),
        KeyBinding::new("shift-down", NavExtendDown, Some("DataPanel && !TextInput")),
        KeyBinding::new(
            "secondary-a",
            MarkAllGroups,
            Some("DataPanel && !TextInput"),
        ),
        KeyBinding::new(
            "secondary-shift-a",
            ClearCompare,
            Some("DataPanel && !TextInput"),
        ),
        KeyBinding::new(
            "secondary-i",
            InvertGroupMarks,
            Some("DataPanel && !TextInput"),
        ),
        KeyBinding::new("space", ToggleFocusedMark, Some("DataPanel && !TextInput")),
        KeyBinding::new(
            "left",
            CollapseFocusedStack,
            Some("DataPanel && !TextInput"),
        ),
        KeyBinding::new("right", ExpandFocusedStack, Some("DataPanel && !TextInput")),
        KeyBinding::new("down", LeaveFilter, Some("GroupFilter > TextInput")),
        KeyBinding::new("enter", LeaveFilter, Some("GroupFilter > TextInput")),
        KeyBinding::new("escape", EscapeFilter, Some("GroupFilter > TextInput")),
        KeyBinding::new("left", FramePrev, Some("Operando && !TextInput")),
        KeyBinding::new("left", FramePrev, Some("PeakFits && !TextInput")),
        KeyBinding::new("right", FrameNext, Some("Operando && !TextInput")),
        KeyBinding::new("right", FrameNext, Some("PeakFits && !TextInput")),
        KeyBinding::new("shift-left", FrameJumpBack, Some("Operando && !TextInput")),
        KeyBinding::new("shift-right", FrameJumpFwd, Some("Operando && !TextInput")),
        KeyBinding::new("home", FrameFirst, Some("Operando && !TextInput")),
        KeyBinding::new("end", FrameLast, Some("Operando && !TextInput")),
        // ⌘1…⌘5 follow `Stage::number`. StageNormalize, StageBackground and
        // StageTransform stay unbound: they open a section of Process.
        KeyBinding::new("secondary-1", StageData, Some("Studio && !Assistant")),
        KeyBinding::new("secondary-2", StageProcess, Some("Studio && !Assistant")),
        KeyBinding::new("secondary-3", StageFit, Some("Studio && !Assistant")),
        KeyBinding::new("secondary-4", StageSeries, Some("Studio && !Assistant")),
        KeyBinding::new("secondary-5", StagePublish, Some("Studio && !Assistant")),
        KeyBinding::new(
            "secondary-b",
            ToggleDataPanel,
            Some("Studio && !TextInput && !Assistant"),
        ),
        KeyBinding::new(
            "secondary-j",
            ToggleContextPanel,
            Some("Studio && !TextInput && !Assistant"),
        ),
        KeyBinding::new(
            "secondary-p",
            FocusFilter,
            Some("Studio && !TextInput && !Assistant"),
        ),
        KeyBinding::new(
            "escape",
            ExploreEscape,
            Some("Explore && !TextInput && !Assistant"),
        ),
        KeyBinding::new("secondary-k", PaletteOpen, Some("Studio && !Assistant")),
        KeyBinding::new("escape", PaletteClose, Some("Palette")),
        KeyBinding::new(
            "secondary-z",
            Undo,
            Some("Studio && !TextInput && !Assistant"),
        ),
        KeyBinding::new(
            "secondary-shift-z",
            Redo,
            Some("Studio && !TextInput && !Assistant"),
        ),
        KeyBinding::new(
            "secondary-shift-j",
            JournalToggle,
            Some("Studio && !TextInput && !Assistant"),
        ),
    ]
}

/// Cache slot for the spectrum loaded outside the catalog (default file).
const NO_ENTRY: usize = usize::MAX;

/// Derived (merged) spectra get virtual indices above this base so the
/// selection/cache/compare machinery treats them like catalog entries.
pub(crate) const DERIVED_BASE: usize = usize::MAX / 2;

/// Frames sampled for the operando overview (heatmap stays bounded no matter
/// how large the scan is).
const MAX_FRAMES: usize = 192;
const K_GRID_BINS: usize = 256;
const K_GRID_MAX: f64 = 15.0;

/// Downsampled overview of one scan, valid for one params fingerprint.
pub(crate) struct OperandoData {
    source: OverviewSource,
    pub(crate) scan_len: usize,
    sample_frames: Vec<usize>,
    support: Vec<series_display::SeriesSupport>,
    pub(crate) fingerprint: u64,
    /// k grid and k-weighted χ(k) rows (sampled frames × grid).
    pub(crate) grid: Vec<f64>,
    pub(crate) matrix: Vec<Vec<f64>>,
    /// Absolute-energy grid with distinct normalized and flattened absorption rows.
    pub(crate) e_grid: Vec<f64>,
    pub(crate) e_matrix: Vec<Vec<f64>>,
    pub(crate) flat_matrix: Vec<Vec<f64>>,
    /// R grid and |χ(R)| rows.
    pub(crate) r_grid: Vec<f64>,
    pub(crate) r_matrix: Vec<Vec<f64>>,
    pub(crate) e0s: Vec<f64>,
    /// Maximum flat (or norm when absent) on [E₀, E₀ + 30 eV] per sampled frame.
    pub(crate) whitelines: Vec<f64>,
    pub(crate) kweight: f64,
}

impl OperandoData {
    /// Grid and matrix of a series space.
    pub(crate) fn space(&self, space: SeriesSpace) -> (&[f64], &[Vec<f64>]) {
        match space {
            SeriesSpace::Energy => (&self.e_grid, &self.e_matrix),
            SeriesSpace::Flat => (&self.e_grid, &self.flat_matrix),
            SeriesSpace::K => (&self.grid, &self.matrix),
            SeriesSpace::R => (&self.r_grid, &self.r_matrix),
        }
    }
}

/// LCF weights per frame of the active scan (a Series trend).
pub(crate) struct SeriesLcf {
    pub(crate) scan: usize,
    pub(crate) fingerprint: u64,
    /// Standard labels, in weight order.
    pub(crate) names: Vec<String>,
    /// frame → weights (+ R-factor last).
    pub(crate) rows: BTreeMap<usize, Vec<f64>>,
    pub(crate) total: usize,
    pub(crate) cancelled: bool,
}

/// One sampled frame of a scan overview, before resampling onto the
/// shared grids.
struct FrameSample {
    support: series_display::SeriesSupport,
    k_row: Vec<f64>,
    e0: f64,
    whiteline: f64,
    energy: Vec<f64>,
    norm: Vec<f64>,
    flat: Vec<f64>,
    r: Vec<f64>,
    mag: Vec<f64>,
}

/// Sampled-frame arrays of a processed spectrum for the overview.
fn frame_sample(sp: &XASSpectrum, k_grid: &[f64]) -> Result<FrameSample, String> {
    let k_row = resample_chik(sp, k_grid).ok_or("processed spectrum has no chi(k)")?;
    let e0 = sp.e0().unwrap_or(f64::NAN);
    let energy: Vec<f64> = sp
        .energy
        .as_ref()
        .map(|e| e.iter().copied().collect())
        .unwrap_or_default();
    let flat: Vec<f64> = sp
        .flat()
        .map(|v| v.iter().copied().collect())
        .unwrap_or_default();
    let norm: Vec<f64> = sp
        .norm()
        .map(|n| n.iter().copied().collect())
        .unwrap_or_default();
    let whiteline = energy
        .iter()
        .zip(if flat.is_empty() { &norm } else { &flat })
        .filter(|(e, _)| **e >= e0 && **e <= e0 + 30.0)
        .map(|(_, n)| *n)
        .fold(f64::NAN, f64::max);
    let r: Vec<f64> = sp
        .r()
        .map(|v| v.iter().copied().collect())
        .unwrap_or_default();
    let mag: Vec<f64> = sp
        .chir_mag()
        .map(|v| v.iter().copied().collect())
        .unwrap_or_default();
    Ok(FrameSample {
        support: series_display::SeriesSupport::from_spectrum(sp),
        k_row,
        e0,
        whiteline,
        energy,
        norm,
        flat,
        r,
        mag,
    })
}

/// Linear resampling of `(x, y)` onto `grid` (zero outside the data).
fn resample_xy(x: &[f64], y: &[f64], grid: &[f64]) -> Vec<f64> {
    let n = x.len().min(y.len());
    if n < 2 {
        return vec![f64::NAN; grid.len()];
    }
    let mut out = Vec::with_capacity(grid.len());
    let mut j = 0usize;
    for &g in grid {
        if g < x[0] || g > x[n - 1] {
            out.push(0.0);
            continue;
        }
        while j + 2 < n && x[j + 1] < g {
            j += 1;
        }
        let (x0, x1) = (x[j], x[j + 1]);
        let t = if x1 > x0 { (g - x0) / (x1 - x0) } else { 0.0 };
        out.push(y[j] + t * (y[j + 1] - y[j]));
    }
    out
}

/// Which quantity a Series heatmap / frame plot shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SeriesSpace {
    Energy,
    Flat,
    K,
    R,
}

fn resample_series_frame(sp: &XASSpectrum, space: SeriesSpace, grid: &[f64]) -> Option<Vec<f64>> {
    if space == SeriesSpace::K {
        return resample_chik(sp, grid);
    }
    let (x, y) = match space {
        SeriesSpace::Energy => (sp.energy.as_ref()?.clone(), sp.norm()?),
        SeriesSpace::Flat => (sp.energy.as_ref()?.clone(), sp.flat()?),
        SeriesSpace::R => (sp.r()?, sp.chir_mag()?),
        SeriesSpace::K => unreachable!(),
    };
    Some(resample_xy(x.as_slice(), y.as_slice(), grid))
}

fn exact_overview_row(
    frames: &[usize],
    rows: &[Vec<f64>],
    frame: usize,
    points: usize,
) -> Vec<f64> {
    frames
        .iter()
        .position(|&i| i == frame)
        .and_then(|i| rows.get(i))
        .cloned()
        .unwrap_or_else(|| vec![f64::NAN; points])
}

/// Quantity plotted against frame in the Series trend.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum TrendSource {
    E0,
    WhiteLine,
    /// A varying parameter of the batch fit.
    FitVar(String),
    /// Weight of the i-th standard of the LCF trend.
    Lcf(usize),
    /// Retained full-frame scalar calculation.
    Measurement(usize),
}

#[derive(Clone)]
struct FitProvenance {
    label: SharedString,
    group_id: Option<u64>,
    path: PathBuf,
    params_fingerprint: u64,
    model_fingerprint: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProblemSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
struct JobError {
    batch: Option<import_state::BatchId>,
    severity: ProblemSeverity,
    label: String,
    message: String,
}

impl JobError {
    fn warning(path: &std::path::Path, message: String) -> Self {
        Self {
            batch: None,
            severity: ProblemSeverity::Warning,
            label: path.display().to_string(),
            message,
        }
    }
}

/// Severity of the free-text status-bar message; see `StudioApp::set_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusKind {
    Info,
    Success,
    Error,
}

fn push_problem(problems: &mut Vec<JobError>, problem: JobError) {
    // Browsing/reprocessing a cached source should not flood recent problems.
    if (problem.batch.is_some() || problem.severity == ProblemSeverity::Warning)
        && problems.contains(&problem)
    {
        return;
    }
    if problem.batch.is_none()
        && problems.iter().filter(|p| p.batch.is_none()).count() >= JOB_ERROR_CAPACITY
    {
        if let Some(index) = problems
            .iter()
            .position(|p| p.batch.is_none() && p.severity == ProblemSeverity::Warning)
        {
            problems.remove(index);
        } else if problem.severity == ProblemSeverity::Warning {
            return;
        } else if let Some(index) = problems.iter().position(|p| p.batch.is_none()) {
            problems.remove(index);
        }
    }
    problems.push(problem);
}

/// Prefix of the one Problems-drawer warning per group about limited ranges.
const RANGE_WARNING_PREFIX: &str = "Ranges limited to the measured data";

/// Prefix of the one Problems-drawer warning per group about a collapsed edge step.
const EDGE_STEP_WARNING_PREFIX: &str = "Normalization degenerate";
/// Prefix of the one Problems-drawer error per group about a failed wavelet map.
const WAVELET_PROBLEM_PREFIX: &str = "Wavelet map failed";
/// Prefix of the one Problems-drawer entry per group from a XANES peak fit.
const PEAK_PROBLEM_PREFIX: &str = "XANES peak fit";

/// Replace the one `prefix` problem recorded for `label` (a full source path,
/// so same-named files in other folders stay separate) with `problem`, or
/// clear it when `problem` is `None`. Messages start with `prefix`.
fn replace_problem(
    problems: &mut Vec<JobError>,
    label: &str,
    prefix: &str,
    problem: Option<(ProblemSeverity, String)>,
) {
    problems.retain(|p| !(p.batch.is_none() && p.label == label && p.message.starts_with(prefix)));
    if let Some((severity, message)) = problem {
        push_problem(
            problems,
            JobError {
                batch: None,
                severity,
                label: label.to_owned(),
                message,
            },
        );
    }
}

/// Warning for a collapsed pre-/post-edge normalization, or `None`.
///
/// The edge step is the post-edge line minus the pre-edge line at E₀, in μ
/// units. When the two lines meet or cross at E₀ (E₀ off the edge, or fit
/// ranges outside the data), rexafs floors the automatic step at 1e-12 to
/// avoid dividing by zero, so normalized μ(E) and χ(k) are scaled by ~10¹²
/// and meaningless. A rexafs-specific heuristic flags any non-finite step or
/// one below a millionth of the measured μ span (`mu_span`, max − min).
fn edge_step_warning(edge_step: Option<f64>, mu_span: f64) -> Option<String> {
    let step = edge_step?;
    (!step.is_finite() || (mu_span.is_finite() && step <= 1e-6 * mu_span)).then(|| {
        format!(
            "{EDGE_STEP_WARNING_PREFIX}: edge step ≈ 0. Check E₀ and the pre-edge and \
             normalization ranges."
        )
    })
}

/// Context-panel field, label and unit for a core range adjustment.
fn range_adjustment_field(field: &str) -> Option<(ParamKey, &'static str, &'static str)> {
    Some(match field {
        "pre_edge_start" => (ParamKey::PreEdgeStart, "pre-edge start", "eV"),
        "pre_edge_end" => (ParamKey::PreEdgeEnd, "pre-edge end", "eV"),
        "norm_start" => (ParamKey::NormStart, "fit minimum", "eV"),
        "norm_end" => (ParamKey::NormEnd, "fit maximum", "eV"),
        "norm_polyorder" => (ParamKey::NormPolyorder, "poly order", ""),
        "kmax" => (ParamKey::BkgKmax, "bkg k max", "Å⁻¹"),
        _ => return None,
    })
}

fn range_used_text(adjustment: &rexafs::RangeAdjustment, unit: &str) -> String {
    if unit.is_empty() {
        format!("{}", adjustment.used)
    } else {
        format!("{:.1} {unit}", adjustment.used)
    }
}

/// One warning naming every explicit setting that processing limited, or
/// `None` when none was limited. Values are the requested and used ones.
fn range_warning(adjustments: &[rexafs::RangeAdjustment]) -> Option<String> {
    let parts: Vec<String> = adjustments
        .iter()
        .filter_map(|a| {
            let (_, label, unit) = range_adjustment_field(a.field)?;
            Some(format!(
                "{label} {} → {}",
                a.requested,
                range_used_text(a, unit)
            ))
        })
        .collect();
    (!parts.is_empty()).then(|| format!("{RANGE_WARNING_PREFIX}: {}.", parts.join("; ")))
}

fn problem_counts(problems: &[JobError]) -> (usize, usize) {
    let warnings = problems
        .iter()
        .filter(|p| p.severity == ProblemSeverity::Warning)
        .count();
    (problems.len() - warnings, warnings)
}

/// A selection that failed to load while earlier results are still on screen.
#[derive(Clone)]
struct StalePlots {
    /// The entry the user selected (and which the plots are *not* showing).
    requested: SharedString,
    message: SharedString,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TrendDomain {
    Sampled,
    FullScan,
}

struct TrendSnapshot {
    values: Vec<f64>,
    /// Frame position of every value (sampled trends sit on their true frames).
    frames: Vec<f64>,
    name: String,
    domain: TrendDomain,
}

/// Frame positions of `n` values in `domain` over a scan of `scan_len`.
fn trend_frames(domain: TrendDomain, n: usize, scan_len: usize) -> Vec<f64> {
    match domain {
        TrendDomain::FullScan => (0..n).map(|i| i as f64).collect(),
        TrendDomain::Sampled => (0..n)
            .map(|i| {
                if n > 1 {
                    i as f64 * scan_len.saturating_sub(1) as f64 / (n - 1) as f64
                } else {
                    0.0
                }
            })
            .collect(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChikRowKey {
    Sampled {
        scan: usize,
        overview_fingerprint: u64,
        row: usize,
    },
    Exact {
        entry: usize,
        params_fingerprint: u64,
    },
}

struct RetainedChikSeries {
    values: Observable<Vec<f64>>,
    key: ChikRowKey,
}

impl RetainedChikSeries {
    fn new(key: ChikRowKey, values: Vec<f64>) -> Self {
        Self {
            values: Observable::new(values),
            key,
        }
    }

    fn replace_if_changed(&mut self, key: ChikRowKey, values: Vec<f64>) -> bool {
        if self.key == key {
            return false;
        }
        self.values.set(values);
        self.key = key;
        true
    }
}

pub(crate) struct OperandoPlots {
    /// Scan the plots currently show; a different scan gets fresh viewports
    /// instead of inheriting the previous scan's pan/zoom.
    source: OverviewSource,
    /// Space the heatmap / frame plot currently show.
    space: SeriesSpace,
    difference: bool,
    pub(crate) heatmap: Entity<RuvizPlot>,
    pub(crate) chik: Entity<RuvizPlot>,
    chik_series: RetainedChikSeries,
    pub(crate) trend: Entity<RuvizPlot>,
    trend_domain: TrendDomain,
    heatmap_cursor: Option<AnnotationId>,
    trend_cursor: Option<AnnotationId>,
    last_trend_cursor: Option<usize>,
    _heatmap_subscription: gpui::Subscription,
}

pub(crate) struct FitPlots {
    pub(crate) k: Entity<RuvizPlot>,
    pub(crate) k_residual: Entity<RuvizPlot>,
    pub(crate) r: Entity<RuvizPlot>,
    pub(crate) r_residual: Entity<RuvizPlot>,
    pub(crate) q: Entity<RuvizPlot>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RangeKey {
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
pub(crate) struct BatchFitData {
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
pub(crate) struct FitVar {
    pub(crate) spec: FitVarSpec,
    pub(crate) field: Entity<TextInput>,
    pub(crate) min_field: Entity<TextInput>,
    pub(crate) max_field: Entity<TextInput>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PathParam {
    S02,
    Degen,
    E0,
    Sigma2,
    DeltaR,
    Ei,
    Third,
    Fourth,
}

impl PathParam {
    pub(crate) const ALL: [PathParam; 8] = [
        PathParam::S02,
        PathParam::Degen,
        PathParam::E0,
        PathParam::DeltaR,
        PathParam::Sigma2,
        PathParam::Ei,
        PathParam::Third,
        PathParam::Fourth,
    ];

    /// The four Artemis path parameters everyone edits; the cumulant /
    /// lifetime terms sit behind a "more" fold.
    pub(crate) fn is_primary(self) -> bool {
        matches!(
            self,
            PathParam::S02
                | PathParam::Degen
                | PathParam::E0
                | PathParam::DeltaR
                | PathParam::Sigma2
        )
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            PathParam::S02 => "S₀²",
            PathParam::Degen => "N",
            PathParam::E0 => "ΔE₀",
            PathParam::Sigma2 => "σ²",
            PathParam::DeltaR => "ΔR",
            PathParam::Ei => "Ei",
            PathParam::Third => "C₃",
            PathParam::Fourth => "C₄",
        }
    }

    fn get(self, spec: &FitPathSpec) -> &str {
        match self {
            PathParam::S02 => &spec.s02,
            PathParam::Degen => &spec.degen,
            PathParam::E0 => &spec.e0,
            PathParam::Sigma2 => &spec.sigma2,
            PathParam::DeltaR => &spec.deltar,
            PathParam::Ei => &spec.ei,
            PathParam::Third => &spec.third,
            PathParam::Fourth => &spec.fourth,
        }
    }

    fn set(self, spec: &mut FitPathSpec, text: String) {
        match self {
            PathParam::S02 => spec.s02 = text,
            PathParam::Degen => spec.degen = text,
            PathParam::E0 => spec.e0 = text,
            PathParam::Sigma2 => spec.sigma2 = text,
            PathParam::DeltaR => spec.deltar = text,
            PathParam::Ei => spec.ei = text,
            PathParam::Third => spec.third = text,
            PathParam::Fourth => spec.fourth = text,
        }
    }
}

/// An imported FEFF path with editable parameter-expression cells.
pub(crate) struct FitPathRow {
    pub(crate) spec: FitPathSpec,
    pub(crate) meta: Option<PathMeta>,
    pub(crate) fields: Vec<(PathParam, Entity<TextInput>)>,
    /// Cumulant / lifetime cells shown.
    pub(crate) more: bool,
}

pub struct StudioApp {
    theme: Theme,
    workspace: Workspace,
    /// Pipeline stage shown by the shell (drives `workspace`).
    stage: Stage,
    stage_view: StageView,
    ui: shell::controls::Presentation,
    /// Ripple-strip thumbnails of the current group.
    thumbs: Option<Arc<[ThumbData; 4]>>,
    handles: HandleState,
    tools: ToolState,
    analysis: shell::tools::AnalysisState,
    measurements: shell::measurements::MeasurementState,
    series_fits: shell::series::fits::SeriesFitState,
    live: shell::live::LiveState,
    peaks: shell::peaks::PeakState,
    normalization: shell::normalization::NormalizationState,
    wavelet: shell::wavelet::WaveletState,
    fluorescence: shell::fluorescence::FluorescenceState,
    journal: shell::journal::JournalState,
    palette: Option<shell::palette::PaletteState>,
    path_route: Option<shell::path_routing::RoutingCard>,
    measurement_import: Option<shell::measurement_import::MeasurementImport>,
    measurement_request: u64,
    pending_routed_import: VecDeque<shell::path_routing::RoutedImport>,
    /// Inspector scroll position (tools reveal their form on open).
    inspector_scroll: gpui::ScrollHandle,
    /// Durable identities backing the compact index-based UI.
    group_registry: crate::group_identity::GroupRegistry,
    group_state: crate::group_identity::GroupState,
    /// Groups skipped by bulk operations (Athena's frozen groups).
    frozen: BTreeSet<usize>,
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
    /// Keyboard row focus and stable range anchor, independent of current.
    focus_group: Option<usize>,
    group_menu: Option<shell::group_menu::GroupMenu>,
    group_rename: Option<shell::group_menu::RenameState>,
    mark_anchor: Option<usize>,
    /// Marks are project-wide; visibility and navigation never remove them.
    selection: BTreeSet<usize>,
    /// Merged/averaged spectra (virtual indices DERIVED_BASE + i).
    derived: Vec<DerivedSpectrum>,
    next_derived_id: u64,
    pending_derived: Option<u64>,
    /// History id of a fit result restored from a project whose provenance
    /// is attached once the fitted spectrum is processed again.
    pending_fit_restore: Option<usize>,
    compare_gen: u64,
    compare_running: bool,
    plot_coverage: [crate::plotting::PlotCoverage; 5],
    view: ViewOptions,
    view_offset_field: Option<Entity<NumericField>>,
    filter_input: Option<Entity<TextInput>>,
    filter_text: String,
    /// Saved expansion while temporarily revealing marked/current groups.
    filter_reveal: Option<BTreeMap<crate::group_identity::GroupId, bool>>,
    reveal_current: Option<usize>,
    root_focus: FocusHandle,
    data_focus: FocusHandle,
    operando_focus: FocusHandle,
    /// Per-section "advanced parameters" fold state (Norm, Bkg, FFT, Import).
    adv_open: [bool; 4],
    import_preview: Option<ImportPreview>,
    import_editor: Option<Entity<shell::import_editor::ImportEditor>>,
    import_preview_error: SharedString,
    import_preview_gen: u64,
    /// Which enum parameter's option list is expanded.
    open_enum: Option<EnumParam>,
    param_menu: Option<shell::parameter_actions::ParamScope>,
    param_context_menu: Option<(
        shell::parameter_actions::ParamScope,
        gpui::Point<gpui::Pixels>,
    )>,
    publish: shell::publish::PublishState,
    main_window: gpui::AnyWindowHandle,
    assistant_window: Option<gpui::AnyWindowHandle>,
    assistant: Option<Entity<shell::assistant::AssistantWindow>>,
    assistant_host: shell::assistant_shell::AssistantHost,
    assistant_resizing: Option<(f32, f32)>,
    last_opened_side_panel: Option<shell::assistant_shell::SidePanel>,
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
    /// set), keyed by the registry’s current catalog indices. The central
    /// catalog adapter and project persistence bind them to durable IDs.
    overrides: BTreeMap<usize, PipelineParams>,
    /// Path-keyed overrides from a loaded project, resolved to catalog
    /// indices once the folder scan / index restore materializes entries.
    pending_overrides: Vec<ParamOverride>,
    /// Restore the saved active spectrum after catalog indices are available.
    pending_project_spectrum: Option<PathBuf>,
    param_fields: Vec<(ParamKey, Entity<NumericField>)>,
    /// Keyed by (catalog index, effective-params fingerprint).
    cache: LruCache<(usize, u64), Arc<XASSpectrum>>,
    /// Raw (energy, mu) keyed by (catalog index, raw/import fingerprint), so a
    /// pipeline-only edit never re-reads the file or re-runs import math.
    raw_cache: LruCache<(usize, u64), RawArrays>,
    /// Live-follow throttle while a handle is dragged.
    recompute_last: Option<Instant>,
    recompute_tick_pending: bool,
    recompute_dirty: bool,
    current_path: PathBuf,
    spectrum_path: PathBuf,
    spectrum_fingerprint: u64,
    /// Identity of the data retained in `spectrum`, independent of selection.
    spectrum_group: Option<shell::tools::ToolTarget>,
    standalone_source: Option<(PathBuf, SharedString, crate::group_identity::GroupId)>,
    /// Scientific quantity captured with the loaded data, including stale plots.
    spectrum_quantity: crate::params::Quantity,
    spectrum: Option<Arc<XASSpectrum>>,
    spectrum_label: SharedString,
    /// Set when the newest selection failed to load. The plots still show the
    /// previous spectrum, so the mismatch has to be stated in the UI rather
    /// than left to a status-bar line the user may not read.
    stale_plots: Option<StalePlots>,
    quadrants: Vec<(SharedString, Entity<RuvizPlot>)>,
    /// Per quadrant: structure key + the observables feeding its session.
    /// A refresh with an unchanged structure only `set`s the observables
    /// (no session rebuild); see `rebuild_explore_plots`.
    quad_bindings: Vec<(u64, Vec<SeriesSource>)>,
    quad_export_labels: Vec<shell::plot_export::Labels>,
    /// Explore-only plot work is deferred while Operando or Fit is visible.
    explore_plots_dirty: bool,
    /// Shared Explore legend strip entries (truncated label, stable color),
    /// rebuilt with the quadrant plots; empty for a single trace.
    legend_entries: Vec<(SharedString, gpui::Rgba)>,
    mixed_overlay_weight: Option<f64>,
    maximized: Option<usize>,
    file_scroll: UniformListScrollHandle,
    expanded_sources: BTreeMap<crate::group_identity::GroupId, bool>,
    groups_resize: Option<(gpui::Pixels, f32)>,
    /// At most one scan is expanded, keeping row-to-member mapping O(1)
    /// even when that scan has a million members.
    expanded_scan: Option<usize>,
    active_scan: Option<usize>,
    overview_series: Option<crate::group_identity::GroupId>,
    operando: Option<OperandoData>,
    series_display: series_display::SeriesDisplay,
    operando_plots: Option<OperandoPlots>,
    operando_gen: u64,
    operando_running: bool,
    operando_cancel: Option<Arc<AtomicBool>>,
    // Operando, batch, merge, fit, LCF operands captured when each job starts.
    job_inputs: [BTreeSet<crate::group_identity::GroupId>; 5],
    /// Trend plotted against frame in the Series stage.
    series_trend: TrendSource,
    series_lcf: Option<SeriesLcf>,
    lcf_running: bool,
    lcf_gen: u64,
    lcf_cancel: Option<Arc<AtomicBool>>,
    lcf_progress: (usize, usize),
    time_pos: usize,
    /// Requested cursor used when a batch-table row opens a scan overview in
    /// the background while the Fit workspace stays visible.
    pending_time_pos: Option<usize>,
    fit_paths: Vec<FitPathRow>,
    /// Ranked summaries parallel to `fit_paths` (label, shell, importance).
    pub(crate) fit_path_infos: Vec<PathInfo>,
    /// How selected paths are parameterised; applied on selection changes
    /// until the user edits a variable or a path cell by hand.
    pub(crate) fit_template: ParameterTemplate,
    pub(crate) fit_template_dirty: bool,
    pub(crate) fit_template_notes: Vec<String>,
    fit_vars: Vec<FitVar>,
    fit_range_fields: Vec<(RangeKey, Entity<NumericField>)>,
    fit_ranges: FitRanges,
    /// Completed fits, oldest first (Artemis' history); results of this
    /// session's fits are kept for re-display.
    fit_history: Vec<FitHistoryEntry>,
    joint: shell::joint_fit::JointState,
    fit_preview: shell::fit_preview::FitPreviewState,
    fit_mode: crate::rmc_fitting::FitMode,
    rmc: shell::rmc::RmcState,
    fit_history_results: BTreeMap<usize, Arc<FeffFitResult>>,
    fit_history_selected: Option<usize>,
    fit_result: Option<Arc<FeffFitResult>>,
    fit_provenance: Option<FitProvenance>,
    fit_plots: Option<FitPlots>,
    fit_gen: u64,
    fit_running: bool,
    fit_error: Option<String>,
    last_fit_duration: Option<Duration>,
    feff_workspace: Option<PathBuf>,
    project_extensions: std::collections::BTreeMap<String, serde_json::Value>,
    project_path: Option<PathBuf>,
    project_header: Option<crate::project::ProjectHeader>,
    project_storage: crate::project::DataStorage,
    project_raw_files: Vec<PathBuf>,
    project_source_origins: BTreeMap<PathBuf, PathBuf>,
    project_generation: u64,
    assistant_history: crate::project::assistant::AssistantHistory,
    assistant_history_revision: u64,
    assistant_history_saved_revision: u64,
    project_load_generation: u64,
    project_saving: bool,
    /// Content fingerprint at the last save or open; `None` if never saved.
    saved_fingerprint: Option<u64>,
    /// Re-record `saved_fingerprint` once an opened project finishes
    /// rescanning, so restoring its sources is not mistaken for an edit.
    settle_saved_fingerprint: bool,
    /// What to do after the unsaved-changes prompt's Save succeeds.
    pending_close: Option<CloseIntent>,
    updates: shell::updates_view::UpdateState,
    help: shell::help::HelpState,
    pub(crate) structure: shell::structure_view::StructureState,
    feff_running: bool,
    feff_gen: u64,
    /// Batch-fit rows with scan-parameter and fit-model provenance.
    batch_fit: Option<BatchFitData>,
    batch_table_scroll: gpui::ScrollHandle,
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
    /// Severity of the message last written through `set_status`, with that
    /// message; a later direct write to `status` reads as Info again.
    status_kind: (StatusKind, SharedString),
    job_errors: Vec<JobError>,
    parser_evidence: BTreeMap<crate::group_identity::GroupId, crate::source_evidence::ParserRecord>,
    imports: crate::import_recipes::ProjectImports,
    intake: import_state::IntakeState,
    intake_cancel: Option<Arc<AtomicBool>>,
    problems_batch: Option<import_state::BatchId>,
    problems_page: usize,
    group_diagnostics: group_rows::Diagnostics,
    problems_open: bool,
    /// Measured plot-container sizes (logical px) keyed by plot id, so a
    /// figure is built with the aspect of the card it fills.
    pub(crate) card_px: BTreeMap<usize, (u32, u32)>,
    /// Viewport width captured at render time (responsive chrome).
    pub(crate) viewport_w: f32,
    pub(crate) viewport_h: f32,
}

fn sample_scan_indices(start: usize, len: usize, cap: usize) -> Vec<usize> {
    if cap == 0 {
        Vec::new()
    } else if cap == 1 {
        (len > 0).then_some(start).into_iter().collect()
    } else if len <= cap {
        (start..start + len).collect()
    } else {
        (0..cap)
            .map(|i| start + i * (len - 1) / (cap - 1))
            .collect()
    }
}

/// Sample surviving frames, retaining original catalog coordinates.
fn active_scan_indices(
    registry: &crate::group_identity::GroupRegistry,
    start: usize,
    len: usize,
    cap: usize,
) -> Vec<usize> {
    let excluded = registry.excluded_indices();
    let excluded: Vec<_> = excluded
        .range(start..start.saturating_add(len))
        .copied()
        .collect();
    let ordinals = sample_scan_indices(0, len.saturating_sub(excluded.len()), cap);
    let mut skipped = 0;
    ordinals
        .into_iter()
        .map(|ordinal| {
            let mut ix = start + ordinal + skipped;
            while excluded.get(skipped).is_some_and(|&skip| skip <= ix) {
                skipped += 1;
                ix += 1;
            }
            ix
        })
        .collect()
}

/// Heatmaps need a regular frame grid; removed positions remain blank while
/// cursor spectra and trends use the surviving samples' exact coordinates.
fn overview_heatmap_rows(
    matrix: &[Vec<f64>],
    sample_frames: &[usize],
    registry: &crate::group_identity::GroupRegistry,
    start: usize,
    len: usize,
) -> Vec<Vec<f64>> {
    sample_scan_indices(0, len, MAX_FRAMES)
        .into_iter()
        .map(|frame| {
            let row = sample_frames
                .iter()
                .enumerate()
                .min_by_key(|(_, p)| p.abs_diff(frame))
                .and_then(|(i, _)| matrix.get(i));
            match row.filter(|_| !registry.index_excluded(start + frame)) {
                Some(row) => row.clone(),
                None => vec![f64::NAN; matrix.first().map_or(0, Vec::len)],
            }
        })
        .collect()
}

fn surviving_frame(
    registry: &crate::group_identity::GroupRegistry,
    start: usize,
    len: usize,
    pos: usize,
    backwards: bool,
) -> Option<usize> {
    let pos = pos.min(len.saturating_sub(1));
    let before = || {
        (0..=pos)
            .rev()
            .find(|&p| p < len && !registry.index_excluded(start + p))
    };
    let after = || (pos..len).find(|&p| !registry.index_excluded(start + p));
    if backwards {
        before().or_else(after)
    } else {
        after().or_else(before)
    }
}

/// Signal workers before retiring their callbacks. Retained results use this
/// return value to distinguish interrupted computations from completed ones.
fn retire_job(
    generation: &mut u64,
    running: &mut bool,
    cancellation: Option<&mut Option<Arc<AtomicBool>>>,
) -> bool {
    if let Some(Some(cancel)) = cancellation.map(Option::take) {
        cancel.store(true, Ordering::Relaxed);
    }
    *generation += 1;
    std::mem::replace(running, false)
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

/// Map a one-based displayed frame coordinate to its zero-based storage index.
fn frame_from_data_y(y: f64, scan_len: usize) -> Option<usize> {
    if scan_len == 0 || !y.is_finite() {
        return None;
    }
    Some(
        (y - 1.0)
            .round()
            .clamp(0.0, scan_len.saturating_sub(1) as f64) as usize,
    )
}

/// Resolve a click inside the heatmap's data extent. This intentionally does
/// not require a cell hit because failed frames are represented by masked NaN
/// rows, which ruviz excludes from hit testing.
fn frame_from_heatmap_position(
    x: f64,
    y: f64,
    grid: &[f64],
    scan_len: usize,
    row_count: usize,
) -> Option<usize> {
    if scan_len == 0 || row_count == 0 || !x.is_finite() || !y.is_finite() {
        return None;
    }
    let kmin = grid.first().copied()?;
    let kmax = grid.last().copied()?.max(kmin + 1e-9);
    if x < kmin || x > kmax {
        return None;
    }

    let last_frame = scan_len.saturating_sub(1) as f64;
    let row_step = if row_count > 1 {
        last_frame / (row_count - 1) as f64
    } else {
        1.0
    };
    let half_step = row_step / 2.0;
    if y < 1.0 - half_step || y > last_frame + 1.0 + half_step {
        return None;
    }
    frame_from_data_y(y, scan_len)
}

fn cursor_color(theme: &Theme) -> PlotColor {
    let accent = theme.accent;
    PlotColor::from_rgb(
        (accent.r * 255.0).round() as u8,
        (accent.g * 255.0).round() as u8,
        (accent.b * 255.0).round() as u8,
    )
}

fn heatmap_cursor_annotation(frame: usize, theme: &Theme) -> Annotation {
    Annotation::hline_styled(
        (frame + 1) as f64,
        cursor_color(theme),
        2.0,
        LineStyle::Solid,
    )
}

fn trend_cursor_annotation(frame: usize, theme: &Theme) -> Annotation {
    Annotation::vline_styled(
        (frame + 1) as f64,
        cursor_color(theme),
        2.0,
        LineStyle::Solid,
    )
}

fn set_cursor_annotation(
    entity: &Entity<RuvizPlot>,
    id: &mut Option<AnnotationId>,
    annotation: Annotation,
    cx: &mut gpui::App,
) -> anyhow::Result<()> {
    if let Some(current) = *id {
        entity.update(cx, |plot, cx| {
            plot.update_annotation(current, annotation, cx)
        })?;
        return Ok(());
    }

    let new_id = entity.update(cx, |plot, cx| plot.add_annotation(annotation, cx))?;
    *id = Some(new_id);
    Ok(())
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

/// Processing state of the selected group, as seen by the plot builder.
#[derive(Clone, Copy, Debug)]
struct ReloadState {
    /// A job for the selection is running or a recompute is queued.
    busy: bool,
    /// The last job for the selection failed; the stale-plot banner is shown.
    failed: bool,
    /// The selection can be processed (it exists and is not excluded).
    loadable: bool,
    /// A result for the selection's current settings is cached.
    cached: bool,
    /// The plotted spectrum belongs to the selected group (any settings).
    shown: bool,
}

/// Reprocess when nothing would otherwise produce the selection's plots.
fn current_reload_needed(state: ReloadState) -> bool {
    state.loadable && !state.busy && !state.failed && !state.cached && !state.shown
}

fn should_rebuild_explore(workspace: Workspace, dirty: &mut bool, invalidated: bool) -> bool {
    if workspace == Workspace::Explore {
        let rebuild = invalidated || *dirty;
        *dirty = false;
        rebuild
    } else {
        *dirty |= invalidated;
        false
    }
}

#[cfg(test)]
mod thin_tests {
    use super::{
        ChikRowKey, RetainedChikSeries, Workspace, catalog_row_index, frame_from_data_y,
        frame_from_heatmap_position, scan_entry_offset, should_rebuild_explore,
    };

    #[test]
    fn energy_frame_updates_preserve_norm_flat_and_exact_frame_identity() {
        use rexafs::prelude::*;
        use rexafs::xafs::normalization::PrePostEdge;
        let grid = [8950., 8951., 8952.];
        let make = |offset: f64| {
            let mut sp = XASSpectrum::new();
            sp.set_spectrum(grid.to_vec(), vec![1., 2., 3.]);
            let mut prepared = PrePostEdge::new();
            prepared.norm = Some(vec![0.1 + offset, 0.8 + offset, 1.2 + offset].into());
            prepared.flat = Some(vec![0.2 + offset, 0.9 + offset, 1.0 + offset].into());
            sp.normalization = Some(NormalizationMethod::PrePostEdge(prepared));
            sp
        };
        let first = make(0.);
        let last = make(0.3);
        for (space, expected) in [
            (super::SeriesSpace::Energy, vec![0.4, 1.1, 1.5]),
            (super::SeriesSpace::Flat, vec![0.5, 1.2, 1.3]),
        ] {
            let row = super::resample_series_frame(&first, space, &grid).unwrap();
            let mut series = RetainedChikSeries::new(
                ChikRowKey::Exact {
                    entry: 0,
                    params_fingerprint: 1,
                },
                row.clone(),
            );
            let source = series.values.clone();
            let next = super::resample_series_frame(&last, space, &grid).unwrap();
            assert!(series.replace_if_changed(
                ChikRowKey::Exact {
                    entry: 99,
                    params_fingerprint: 1
                },
                next
            ));
            assert_ne!(&*source.read(), &row);
            for (actual, want) in source.read().iter().zip(expected) {
                assert!((actual - want).abs() < 1e-12);
            }
        }
        let rows = vec![vec![1., 2.], vec![3., 4.]];
        assert_eq!(super::exact_overview_row(&[0, 99], &rows, 99, 2), rows[1]);
        assert!(
            super::exact_overview_row(&[0, 99], &rows, 50, 2)
                .iter()
                .all(|v| v.is_nan()),
            "Do not label a neighboring preview row as frame 51"
        );
    }

    #[test]
    fn heatmap_data_coordinates_map_to_full_scan_frames() {
        assert_eq!(frame_from_data_y(1.0, 1_000), Some(0));
        assert_eq!(frame_from_data_y(501.49, 1_000), Some(500));
        assert_eq!(frame_from_data_y(501.5, 1_000), Some(501));
        assert_eq!(frame_from_data_y(1000.0, 1_000), Some(999));
        assert_eq!(frame_from_data_y(-20.0, 1_000), Some(0));
        assert_eq!(frame_from_data_y(2_000.0, 1_000), Some(999));
    }

    #[test]
    fn heatmap_data_coordinates_reject_unmappable_values() {
        assert_eq!(frame_from_data_y(0.0, 0), None);
        assert_eq!(frame_from_data_y(f64::NAN, 300), None);
        assert_eq!(frame_from_data_y(f64::INFINITY, 300), None);
    }

    #[test]
    fn heatmap_position_maps_masked_rows_but_rejects_blank_space() {
        let grid = [2.0, 8.0];
        assert_eq!(
            frame_from_heatmap_position(5.0, 501.0, &grid, 1_000, 192),
            Some(500)
        );
        assert_eq!(
            frame_from_heatmap_position(1.0, 500.0, &grid, 1_000, 192),
            None
        );
        assert_eq!(
            frame_from_heatmap_position(5.0, 1_100.0, &grid, 1_000, 192),
            None
        );
        assert_eq!(frame_from_heatmap_position(5.0, 0.0, &grid, 0, 0), None);
    }

    #[test]
    fn retained_chik_series_suppresses_duplicate_provenance() {
        let sampled = ChikRowKey::Sampled {
            scan: 2,
            overview_fingerprint: 11,
            row: 4,
        };
        let mut series = RetainedChikSeries::new(sampled, vec![1.0]);
        assert!(!series.replace_if_changed(sampled, vec![2.0]));
        assert_eq!(series.values.version(), 0);
        assert_eq!(&*series.values.read(), &[1.0]);

        let exact = ChikRowKey::Exact {
            entry: 14,
            params_fingerprint: 21,
        };
        assert!(series.replace_if_changed(exact, vec![3.0]));
        assert_eq!(series.values.version(), 1);
        assert!(!series.replace_if_changed(exact, vec![4.0]));
        assert_eq!(series.values.version(), 1);

        let changed_params = ChikRowKey::Exact {
            entry: 14,
            params_fingerprint: 22,
        };
        assert!(series.replace_if_changed(changed_params, vec![5.0]));
        assert_eq!(series.values.version(), 2);
    }

    #[test]
    fn replacement_chik_series_uses_a_fresh_observable() {
        let key = ChikRowKey::Sampled {
            scan: 0,
            overview_fingerprint: 1,
            row: 0,
        };
        let old = RetainedChikSeries::new(key, vec![1.0]);
        let old_source = old.values.clone();
        let replacement = RetainedChikSeries::new(key, vec![2.0]);

        old_source.set(vec![3.0]);
        assert_eq!(&*old_source.read(), &[3.0]);
        assert_eq!(&*replacement.values.read(), &[2.0]);
        assert_eq!(replacement.values.version(), 0);
    }

    #[test]
    fn explore_rebuilds_immediately_or_consumes_hidden_dirty_state() {
        let mut dirty = false;
        assert!(!should_rebuild_explore(
            Workspace::Operando,
            &mut dirty,
            true
        ));
        assert!(dirty);
        assert!(!should_rebuild_explore(Workspace::Fit, &mut dirty, false));
        assert!(dirty);
        assert!(should_rebuild_explore(
            Workspace::Explore,
            &mut dirty,
            false
        ));
        assert!(!dirty);
        assert!(should_rebuild_explore(Workspace::Explore, &mut dirty, true));
        assert!(!dirty);
    }

    #[test]
    fn selection_without_result_or_job_is_processed_again() {
        use super::{ReloadState, current_reload_needed};
        // Group B was selected while group A was plotted; B's job ended with
        // no result and no error (a cancelled load).
        let orphaned = ReloadState {
            busy: false,
            failed: false,
            loadable: true,
            cached: false,
            shown: false,
        };
        assert!(current_reload_needed(orphaned));
        // A failed job keeps A with the stale banner until settings change.
        assert!(!current_reload_needed(ReloadState {
            failed: true,
            ..orphaned
        }));
        // The corrected settings are being processed.
        assert!(!current_reload_needed(ReloadState {
            busy: true,
            ..orphaned
        }));
        // B's result for the corrected settings is ready.
        assert!(!current_reload_needed(ReloadState {
            cached: true,
            ..orphaned
        }));
        assert!(!current_reload_needed(ReloadState {
            shown: true,
            ..orphaned
        }));
        // Removed or excluded groups are never processed.
        assert!(!current_reload_needed(ReloadState {
            loadable: false,
            ..orphaned
        }));
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

    #[test]
    fn removal_scan_operands_fingerprint_and_cursor_follow_survivors() {
        use super::*;
        let registry = crate::group_identity::GroupRegistry::default();
        let ids: Vec<_> = (100..106)
            .map(|ix| {
                registry.register_source(
                    Some(ix),
                    format!("/data/{ix}.dat").into(),
                    DetectionMode::Auto,
                    &Default::default(),
                )
            })
            .collect();
        let params = PipelineParams::default();
        let stamp = || scan_fingerprint(&params, &BTreeMap::new(), &registry, 100, 6);
        let original = stamp();
        let outside = registry.register_source(
            Some(99),
            "/data/99.dat".into(),
            DetectionMode::Auto,
            &Default::default(),
        );
        registry.set_excluded(&BTreeSet::from([outside.clone()]));
        assert_eq!(stamp(), original);
        registry.set_excluded(&BTreeSet::from([
            outside,
            ids[0].clone(),
            ids[2].clone(),
            ids[5].clone(),
        ]));
        assert_ne!(stamp(), original);
        assert_eq!(
            active_scan_indices(&registry, 100, 6, usize::MAX),
            vec![101, 103, 104]
        );
        assert_eq!(active_scan_indices(&registry, 100, 6, 2), vec![101, 104]);
        assert_eq!(active_scan_indices(&registry, 100, 6, 1), vec![101]);
        assert_eq!(surviving_frame(&registry, 100, 6, 0, false), Some(1));
        assert_eq!(surviving_frame(&registry, 100, 6, 2, false), Some(3));
        assert_eq!(surviving_frame(&registry, 100, 6, 2, true), Some(1));
        assert_eq!(surviving_frame(&registry, 100, 6, 5, false), Some(4));
        let heatmap = overview_heatmap_rows(
            &[vec![1.], vec![3.], vec![4.]],
            &[1, 3, 4],
            &registry,
            100,
            6,
        );
        assert_eq!(heatmap.len(), 6);
        assert!(heatmap[0][0].is_nan() && heatmap[2][0].is_nan() && heatmap[5][0].is_nan());
        assert_eq!(heatmap[3], vec![3.]);
        registry.set_excluded(&ids.into_iter().collect());
        assert!(active_scan_indices(&registry, 100, 6, MAX_FRAMES).is_empty());
        assert_eq!(surviving_frame(&registry, 100, 6, 0, false), None);
        registry.set_excluded(&BTreeSet::new());
        assert_eq!(stamp(), original);
        assert_eq!(
            active_scan_indices(&registry, 100, 6, usize::MAX),
            (100..106).collect::<Vec<_>>()
        );
    }

    #[test]
    fn removal_jobs_cancel_only_bound_inputs_and_finalize_partial_results() {
        use super::*;
        use crate::group_identity::GroupId;
        let source = GroupId::source(std::path::Path::new("/data/a.dat"), DetectionMode::Auto);
        let unrelated = GroupId::new_result();
        let inputs = [
            BTreeSet::new(),
            BTreeSet::from([source.clone()]),
            BTreeSet::new(),
            BTreeSet::from([source.clone()]),
            BTreeSet::from([source.clone()]),
        ];
        let mut generations = [10; 6];
        let mut running = [true; 5];
        let tokens = std::array::from_fn::<_, 4, _>(|_| Arc::new(AtomicBool::new(false)));
        let mut cancellations = tokens.clone().map(Some);
        let mut filtered = Some(Arc::new(vec![0, 1]));
        let unrelated_result = CatalogBindings {
            generations: generations.each_mut(),
            running: running.each_mut(),
            cancellations: cancellations.each_mut(),
            filtered: &mut filtered,
        }
        .remove(&inputs, &BTreeSet::from([unrelated]));
        assert_eq!(unrelated_result, [false; 5]);
        assert_eq!(generations, [10; 6]);
        assert_eq!(running, [true; 5]);
        assert!(tokens.iter().all(|t| !t.load(Ordering::Relaxed)));
        let interrupted = CatalogBindings {
            generations: generations.each_mut(),
            running: running.each_mut(),
            cancellations: cancellations.each_mut(),
            filtered: &mut filtered,
        }
        .remove(&inputs, &BTreeSet::from([source]));
        assert_eq!(interrupted, [false, true, false, true, true]);
        assert_eq!(generations, [10, 11, 10, 11, 11, 10]);
        assert_eq!(running, [true, false, true, false, false]);
        assert_eq!(
            tokens.map(|t| t.load(Ordering::Relaxed)),
            [false, true, false, true]
        );
        assert_eq!(filtered.as_deref(), Some(&vec![0, 1]));
        let mut batch = BatchFitData {
            scan: 0,
            fingerprint: 0,
            model_fingerprint: 0,
            rows: [3, 0]
                .map(|frame| BatchFitRow {
                    frame,
                    entry_ix: frame,
                    r_factor: 0.1,
                    reduced_chi_square: Some(1.),
                    values: vec![],
                    solver_report: None,
                })
                .into(),
            frame_labels: BTreeMap::new(),
            problems: vec![BatchFitProblem {
                frame: 1,
                label: "a".into(),
                error: "bad scan".into(),
            }],
            problems_open: false,
            preview: false,
            total: 500,
            cancelled: false,
            varying_names: vec![],
            trend_param: 0,
        };
        let mut lcf = SeriesLcf {
            scan: 0,
            fingerprint: 0,
            names: vec!["standard".into()],
            rows: BTreeMap::from([(1, vec![0.5])]),
            total: 500,
            cancelled: false,
        };
        finish_interrupted_results(Some(&mut batch), Some(&mut lcf), interrupted);
        assert!(batch.cancelled && lcf.cancelled);
        assert_eq!(batch.problems.len(), 1);
        assert_eq!(
            batch.rows.iter().map(|r| r.frame).collect::<Vec<_>>(),
            vec![0, 3]
        );
        assert_eq!(lcf.rows[&1], vec![0.5]);
        assert_eq!((batch.total, lcf.total), (500, 500));
    }

    #[test]
    fn removal_lcf_bindings_include_only_loaded_standards() {
        use super::*;
        let mut cache = LruCache::new(NonZeroUsize::new(8).unwrap());
        let spectrum = Arc::new(XASSpectrum::default());
        cache.put((1, 10), spectrum.clone());
        cache.put((2, 9), spectrum.clone()); // outdated processing
        cache.put((NO_ENTRY, 10), spectrum);
        let marked = BTreeSet::from([0, 1, 2, NO_ENTRY]);
        assert_eq!(
            cached_marked_indices(&marked, &cache, |_| 10).collect::<Vec<_>>(),
            vec![1]
        );
        assert_eq!(
            cached_marked_spectra(&marked, &cache, |_| 10, |ix| ix.to_string()).len(),
            1
        );
    }

    #[test]
    fn removal_cache_eviction_keeps_unrelated_catalog_entries() {
        use super::*;
        let mut cache = LruCache::new(NonZeroUsize::new(8).unwrap());
        for key in [(0, 1), (0, 2), (1, 1), (DERIVED_BASE, 1)] {
            cache.put(key, key.0);
        }
        evict_group_keys(&mut cache, &BTreeSet::from([0]), false);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.peek(&(1, 1)), Some(&1));
        assert!(cache.contains(&(DERIVED_BASE, 1)));
        evict_group_keys(&mut cache, &BTreeSet::new(), true);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.peek(&(1, 1)), Some(&1));
    }

    use std::collections::BTreeMap;

    use super::{ParamSection, copy_section, scan_fingerprint, section_differs};
    use crate::params::PipelineParams;

    #[test]
    fn compare_excludes_pending_current_and_retains_other_marked_loads() {
        use super::*;
        let request = |ix| CompareLoad {
            ix,
            fingerprint: 1,
            source: Ok("/data/scan.dat".into()),
            params: Default::default(),
        };
        let mut loads = vec![request(0), request(1), request(NO_ENTRY)];
        exclude_pending_current(&mut loads, Some(1));
        assert_eq!(
            loads.iter().map(|load| load.ix).collect::<Vec<_>>(),
            vec![0, NO_ENTRY]
        );
        exclude_pending_current(&mut loads, Some(NO_ENTRY));
        assert_eq!(loads.len(), 1);
        exclude_pending_current(&mut loads, None);
        assert_eq!(loads.len(), 1);
        exclude_pending_current(&mut loads, Some(0));
        assert!(
            loads.is_empty(),
            "plain navigation needs no overlay batch or diagnostic ticket"
        );
    }

    #[test]
    fn compare_processing_preserves_parser_diagnostics_for_cached_current() {
        use super::*;
        let path = std::env::temp_dir().join(format!(
            "rexafs-compare-warnings-{}.xmu",
            std::process::id()
        ));
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/projects/data/cu_150k.xmu");
        let mut text = std::fs::read_to_string(fixture).unwrap();
        text.push_str("\nnot a numeric data row\n");
        std::fs::write(&path, text).unwrap();
        let request = CompareLoad {
            ix: 0,
            fingerprint: 0,
            source: Ok(path.clone()),
            params: Default::default(),
        };
        let (_, _, result) = request.process();
        let (_, raw) = result.unwrap();
        let warnings = raw.unwrap().diagnostics.warnings();
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| w.contains("malformed")));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn derived_only_index_shift_keeps_batch_fit_and_active_filter() {
        use super::{CatalogBindings, IndexChange};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let mut generations = [10; 6];
        let mut running = [true; 5];
        let batch = Arc::new(AtomicBool::new(false));
        let mut cancellations = [
            Some(Arc::new(AtomicBool::new(false))),
            Some(batch.clone()),
            None,
            Some(Arc::new(AtomicBool::new(false))),
        ];
        let matches = Arc::new(vec![2, 5, 8]);
        let mut filtered = Some(matches.clone());
        CatalogBindings {
            generations: generations.each_mut(),
            running: running.each_mut(),
            cancellations: cancellations.each_mut(),
            filtered: &mut filtered,
        }
        .invalidate(IndexChange::DerivedOnly);
        assert_eq!(generations, [10; 6]);
        assert_eq!(running, [true; 5]);
        assert!(!batch.load(Ordering::Relaxed));
        assert!(cancellations[1].is_some());
        assert!(Arc::ptr_eq(filtered.as_ref().unwrap(), &matches));
        // A genuine catalog swap still retires all catalog jobs and filtering.
        CatalogBindings {
            generations: generations.each_mut(),
            running: running.each_mut(),
            cancellations: cancellations.each_mut(),
            filtered: &mut filtered,
        }
        .invalidate(IndexChange::Catalog);
        assert_eq!(generations, [11; 6]);
        assert_eq!(running, [false; 5]);
        assert!(batch.load(Ordering::Relaxed));
        assert!(filtered.is_none());
    }

    #[test]
    fn reopen_marked_channels_and_result_loads_comparison_and_lcf_cache() {
        use super::*;
        use crate::group_identity::{GroupId, GroupRegistry};
        use crate::params::{ImportConfig, Quantity};
        let root = std::env::temp_dir().join(format!(
            "rexafs-reopen-compare-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("multi.dat");
        std::fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rexafs/tests/testfiles/Ru_QAS.dat"),
            &source,
        )
        .unwrap();
        let source = source.canonicalize().unwrap();
        let mut project = ProjectFile {
            version: PROJECT_VERSION,
            spectrum_file: Some(source.clone()),
            raw_files: vec![source.clone()],
            ..Default::default()
        };
        project.derived = [DetectionMode::Transmission, DetectionMode::Reference]
            .into_iter()
            .enumerate()
            .map(|(i, mode)| DerivedSpectrum {
                id: i as u64 + 1,
                source: Some(source.clone()),
                params: Some(PipelineParams {
                    import: ImportConfig {
                        mode,
                        ..Default::default()
                    },
                    fft_kweight: Some(2.),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .collect();
        project.derived.push(DerivedSpectrum {
            id: 3,
            group_id: Some(GroupId::new_result()),
            quantity: Quantity::NormalizedDifference,
            energy: vec![22000., 22001., 22002.],
            mu: vec![0.1, 0.2, 0.1],
            ..Default::default()
        });
        project.assign_group_ids();
        let registry = GroupRegistry::rebuild(
            [(0, source.clone(), DetectionMode::Auto)],
            &mut project.derived,
            &mut project.source_groups,
            &project.source_origins,
        );
        project.group_state.capture(
            &registry,
            &BTreeSet::from([DERIVED_BASE, DERIVED_BASE + 1, DERIVED_BASE + 2]),
            &BTreeSet::new(),
            &BTreeMap::from([(
                0,
                PipelineParams {
                    fft_kweight: Some(1.),
                    ..Default::default()
                },
            )]),
            Some(0),
        );
        let path = root.join("session.rxs");
        crate::project::save(&path, &project).unwrap();
        let mut reopened = crate::project::tests::load(&path).unwrap();
        reopened.assign_group_ids();
        let mut catalog = Catalog::default();
        catalog.extend(vec![crate::catalog::FileMeta {
            dir: Arc::from(source.parent().unwrap().to_str().unwrap()),
            name: "multi.dat".into(),
            size: 1,
        }]);
        let registry = GroupRegistry::from_sources(reopened.source_groups.clone());
        registry.replace_catalog(GroupRegistry::prepare_catalog(
            &catalog,
            &reopened.source_groups,
        ));
        registry.replace_derived(&reopened.derived);
        let selection = registry.indices(&reopened.group_state.marked);
        let overrides = reopened.group_state.resolved_overrides(&registry);
        let active = registry
            .index(reopened.group_state.current.as_ref().unwrap())
            .unwrap();
        let mut cache = LruCache::new(NonZeroUsize::new(16).unwrap());
        // Reopening first loads the active file. The remaining marks require the
        // same comparison scheduling used by restore/import completion.
        for request in missing_compare_loads(
            &catalog,
            &reopened.derived,
            &reopened.params,
            &overrides,
            &[active],
            &cache,
        ) {
            let (ix, fingerprint, result) = request.process();
            cache.put((ix, fingerprint), Arc::new(result.unwrap().0));
        }
        let indices: Vec<_> = selection.iter().copied().chain([active]).collect();
        let missing = missing_compare_loads(
            &catalog,
            &reopened.derived,
            &reopened.params,
            &overrides,
            &indices,
            &cache,
        );
        assert_eq!(missing.len(), 3);
        let fingerprints: BTreeMap<_, _> = missing.iter().map(|r| (r.ix, r.fingerprint)).collect();
        for request in missing {
            let (ix, fingerprint, result) = request.process();
            cache.put((ix, fingerprint), Arc::new(result.unwrap().0));
        }
        assert!(
            missing_compare_loads(
                &catalog,
                &reopened.derived,
                &reopened.params,
                &overrides,
                &indices,
                &cache
            )
            .is_empty()
        );
        let standards = cached_marked_spectra(
            &selection,
            &cache,
            |ix| fingerprints[&ix],
            |ix| format!("{ix}"),
        );
        assert_eq!(standards.len(), 3);
        assert!(
            standards
                .iter()
                .all(|(_, sp)| sp.raw_mu.as_ref().is_some_and(|mu| !mu.is_empty()))
        );
        assert_eq!(
            cache
                .peek(&(active, overrides[&active].fingerprint()))
                .unwrap()
                .raw_mu
                .as_ref()
                .unwrap()
                .len(),
            standards[0].1.raw_mu.as_ref().unwrap().len()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mapping_edit_preparation_skips_noops_and_rejects_partial_edits() {
        let before = PipelineParams::default();
        assert!(
            super::prepare_parameter_edit(&before, |_| Ok(()))
                .unwrap()
                .is_none()
        );
        let rejected = super::prepare_parameter_edit(&before, |p| {
            p.import.i0_col = Some(7);
            anyhow::bail!("invalid ROI columns")
        });
        assert_eq!(rejected.err().unwrap().to_string(), "invalid ROI columns");
        assert_eq!(before.import.i0_col, None);
        let after = super::prepare_parameter_edit(&before, |p| {
            p.align_to_ref = !p.align_to_ref;
            Ok(())
        })
        .unwrap()
        .unwrap();
        assert_ne!(after.align_to_ref, before.align_to_ref);
    }

    #[test]
    fn mapping_preview_completion_rejects_superseded_results_and_surfaces_errors() {
        use super::finish_import_preview;
        use std::path::Path;
        let source = Path::new("Ru_QAS.dat");
        let other = Path::new("other.dat");
        let resolved = crate::params::ResolvedImport {
            mode: crate::params::DetectionMode::Reference,
            energy_col: 0,
            i0_col: 1,
            it_col: 2,
            ir_col: 3,
            fluor_cols: vec![4, 5],
            mu_col: None,
        };
        let reference = crate::params::ImportPreview {
            layout: None,
            column_count: 6,
            names: None,
            rows: vec![],
            detected: resolved.clone(),
            auto_mode: resolved.mode,
            resolved,
            xdi: None,
            diagnostics: Default::default(),
            signal_error: None,
        };
        let mut preview = Some(reference.clone());
        let mut error = gpui::SharedString::from("current error");
        // A different selected file, a new channel/mode on the same source,
        // or selection of a source-less group supersedes the old request.
        for current in [(other, 2), (source, 3), (Path::new(""), 3)] {
            for result in [Ok(reference.clone()), Err("stale error".into())] {
                assert!(!finish_import_preview(
                    current,
                    (source, 2),
                    result,
                    &mut preview,
                    &mut error
                ));
                assert_eq!(preview, Some(reference.clone()));
                assert_eq!(error.as_ref(), "current error");
            }
        }
        let message = "Ru_QAS.dat: permission denied";
        assert!(finish_import_preview(
            (source, 3),
            (source, 3),
            Err(message.into()),
            &mut preview,
            &mut error
        ));
        assert!(preview.is_none());
        assert_eq!(error.as_ref(), message);
        assert!(finish_import_preview(
            (source, 3),
            (source, 3),
            Ok(reference.clone()),
            &mut preview,
            &mut error
        ));
        assert_eq!(preview, Some(reference));
        assert!(error.is_empty());
    }

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
        let base = scan_fingerprint(&global, &overrides, &Default::default(), 100, 50);
        assert_eq!(
            base,
            scan_fingerprint(&global, &overrides, &Default::default(), 100, 50)
        );

        let ov = PipelineParams {
            rbkg: Some(1.3),
            ..Default::default()
        };
        overrides.insert(10, ov.clone()); // outside [100, 150)
        assert_eq!(
            base,
            scan_fingerprint(&global, &overrides, &Default::default(), 100, 50)
        );

        overrides.insert(120, ov); // inside
        assert_ne!(
            base,
            scan_fingerprint(&global, &overrides, &Default::default(), 100, 50)
        );

        overrides.remove(&120);
        assert_eq!(
            base,
            scan_fingerprint(&global, &overrides, &Default::default(), 100, 50)
        );
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
mod range_warning_tests {
    use super::{
        EDGE_STEP_WARNING_PREFIX, JobError, ProblemSeverity, RANGE_WARNING_PREFIX,
        edge_step_warning, range_warning, replace_problem,
    };
    use rexafs::RangeAdjustment;

    #[test]
    fn collapsed_edge_steps_warn_and_real_steps_do_not() {
        // Core floor for an automatic step whose lines meet at E₀.
        assert!(
            edge_step_warning(Some(1e-12), 2.0)
                .unwrap()
                .starts_with(EDGE_STEP_WARNING_PREFIX)
        );
        assert!(edge_step_warning(Some(f64::NAN), 2.0).is_some());
        assert_eq!(edge_step_warning(Some(0.8), 2.0), None);
        // Recovered/normalized input keeps its unit step.
        assert_eq!(edge_step_warning(Some(1.0), 1.2), None);
        assert_eq!(edge_step_warning(None, 2.0), None);
    }

    #[test]
    fn keyed_problems_replace_per_full_path_and_prefix() {
        let mut problems = Vec::new();
        let warn = |m: &str| Some((ProblemSeverity::Warning, m.to_owned()));
        replace_problem(&mut problems, "/a/x.dat", "Wavelet", warn("Wavelet one"));
        replace_problem(&mut problems, "/b/x.dat", "Wavelet", warn("Wavelet two"));
        replace_problem(&mut problems, "/a/x.dat", "Edge", warn("Edge low"));
        replace_problem(&mut problems, "/a/x.dat", "Wavelet", warn("Wavelet three"));
        let messages: Vec<_> = problems
            .iter()
            .map(|p: &JobError| p.message.as_str())
            .collect();
        assert_eq!(messages, ["Wavelet two", "Edge low", "Wavelet three"]);
        replace_problem(&mut problems, "/a/x.dat", "Wavelet", None);
        assert_eq!(problems.len(), 2);
        assert!(problems.iter().all(|p| p.message != "Wavelet three"));
    }

    #[test]
    fn one_warning_names_every_limited_field_with_units() {
        assert_eq!(range_warning(&[]), None);
        let message = range_warning(&[
            RangeAdjustment {
                field: "norm_start",
                requested: 150.0,
                used: 25.0,
            },
            RangeAdjustment {
                field: "norm_polyorder",
                requested: 2.0,
                used: 1.0,
            },
            RangeAdjustment {
                field: "kmax",
                requested: 15.0,
                used: 9.512,
            },
        ])
        .unwrap();
        assert_eq!(
            message,
            format!(
                "{RANGE_WARNING_PREFIX}: fit minimum 150 → 25.0 eV; poly order 2 → 1; bkg k max 15 → 9.5 Å⁻¹."
            )
        );
    }
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
        ExploreEscape, FocusFilter, FrameFirst, FrameLast, StageBackground, StageData, StageFit,
        StageNormalize, StageProcess, StagePublish, StageSeries, StageTransform,
        ToggleContextPanel, ToggleDataPanel, studio_keybindings,
    };
    use crate::app::shell::Stage;

    #[test]
    fn stage_shortcuts_follow_the_five_stage_numbers() {
        let bindings = studio_keybindings();
        let command = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        };
        let table = [
            (binding::<StageData>(&bindings), Stage::Data),
            (binding::<StageProcess>(&bindings), Stage::Process),
            (binding::<StageFit>(&bindings), Stage::Fit),
            (binding::<StageSeries>(&bindings), Stage::Series),
            (binding::<StagePublish>(&bindings), Stage::Publish),
        ];
        assert_eq!(table.len(), Stage::ALL.len());
        for (binding, stage) in table {
            let key = gpui::Keystroke::parse(&format!("{command}-{}", stage.number())).unwrap();
            assert_eq!(binding.match_keystrokes(&[key]), Some(false), "{stage:?}");
            assert_eq!(stage.shortcut(), format!("⌘{}", stage.number()));
        }
        // ⌘6 and ⌘7 (formerly Series and Publish) are no longer bound, and
        // the former processing stages are reached through Process.
        for n in [6, 7] {
            let key = gpui::Keystroke::parse(&format!("{command}-{n}")).unwrap();
            assert!(
                bindings
                    .iter()
                    .all(|b| b.match_keystrokes(std::slice::from_ref(&key)).is_none())
            );
        }
        for action in [
            gpui::Action::boxed_clone(&StageNormalize),
            gpui::Action::boxed_clone(&StageBackground),
            gpui::Action::boxed_clone(&StageTransform),
        ] {
            assert!(
                bindings
                    .iter()
                    .all(|b| !b.action().partial_eq(action.as_ref()))
            );
        }
    }

    fn binding<A: Action + 'static>(bindings: &[KeyBinding]) -> &KeyBinding {
        bindings
            .iter()
            .find(|binding| binding.action().as_any().is::<A>())
            .expect("action must have a key binding")
    }

    #[test]
    fn shortcuts_use_the_native_command_modifier() {
        use crate::widgets::text_input::{Copy, Paste, SelectAll, WordLeft};

        let shell = studio_keybindings();
        let input = crate::widgets::text_input::text_input_keybindings();
        let command = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        };
        for (binding, key) in [
            (binding::<super::Undo>(&shell), "z"),
            (binding::<super::OpenProject>(&shell), "o"),
            (binding::<StageData>(&shell), "1"),
            (binding::<Copy>(&input), "c"),
            (binding::<Paste>(&input), "v"),
            (binding::<SelectAll>(&input), "a"),
        ] {
            let key = gpui::Keystroke::parse(&format!("{command}-{key}")).unwrap();
            assert_eq!(binding.match_keystrokes(&[key]), Some(false));
        }
        let word_modifier = if cfg!(target_os = "macos") {
            "alt"
        } else {
            "ctrl"
        };
        assert_eq!(
            binding::<WordLeft>(&input)
                .match_keystrokes(&[
                    gpui::Keystroke::parse(&format!("{word_modifier}-left")).unwrap(),
                ]),
            Some(false)
        );
        if !cfg!(target_os = "macos") {
            for binding in shell.iter().chain(&input) {
                for key in binding.keystrokes() {
                    assert!(
                        !key.modifiers().platform,
                        "Windows shortcuts must not require the Windows key"
                    );
                }
            }
        }
    }

    #[test]
    fn project_and_import_shortcuts_are_distinct_and_yield_to_routing_card() {
        let bindings = studio_keybindings();
        let open = binding::<super::OpenProject>(&bindings);
        let import = binding::<super::ImportPaths>(&bindings);
        assert_eq!(
            open.keystrokes(),
            KeyBinding::new("secondary-o", super::OpenProject, None).keystrokes()
        );
        assert_eq!(
            import.keystrokes(),
            KeyBinding::new("secondary-shift-o", super::ImportPaths, None).keystrokes()
        );
        let text_bindings = crate::widgets::text_input::text_input_keybindings();
        for target in [open, import] {
            assert_eq!(
                bindings
                    .iter()
                    .chain(text_bindings.iter())
                    .filter(|b| b.keystrokes() == target.keystrokes())
                    .count(),
                1
            );
            let predicate = target.predicate().unwrap();
            assert!(
                predicate
                    .depth_of(&[KeyContext::parse("Studio Explore").unwrap()])
                    .is_some()
            );
            assert!(
                predicate
                    .depth_of(&[KeyContext::parse("PathRoute").unwrap()])
                    .is_none()
            );
        }
        assert!(
            binding::<super::DismissPathRoute>(&bindings)
                .predicate()
                .unwrap()
                .depth_of(&[KeyContext::parse("PathRoute").unwrap()])
                .is_some()
        );
    }

    #[test]
    fn list_keys_yield_to_text_editing_and_escape_preserves_marks() {
        use super::{
            ClearCompare, CollapseFocusedStack, ExpandFocusedStack, InvertGroupMarks,
            MarkAllGroups, NavDown, NavExtendDown, NavExtendUp, NavUp, ToggleFocusedMark,
        };
        let bindings = studio_keybindings();
        let list = KeyContext::parse("DataPanel").unwrap();
        let input = KeyContext::parse("TextInput").unwrap();
        for b in [
            binding::<NavUp>(&bindings),
            binding::<NavDown>(&bindings),
            binding::<NavExtendUp>(&bindings),
            binding::<NavExtendDown>(&bindings),
            binding::<MarkAllGroups>(&bindings),
            binding::<ClearCompare>(&bindings),
            binding::<InvertGroupMarks>(&bindings),
            binding::<ToggleFocusedMark>(&bindings),
            binding::<CollapseFocusedStack>(&bindings),
            binding::<ExpandFocusedStack>(&bindings),
        ] {
            let p = b.predicate().unwrap();
            assert!(p.depth_of(std::slice::from_ref(&list)).is_some());
            assert!(p.depth_of(&[list.clone(), input.clone()]).is_none());
        }
        assert_eq!(
            bindings
                .iter()
                .filter(|b| b.action().as_any().is::<ClearCompare>())
                .count(),
            1
        );
    }

    #[test]
    fn group_rename_keys_belong_only_to_the_inline_editor() {
        let bindings = studio_keybindings();
        let list = KeyContext::parse("DataPanel").unwrap();
        let rename = KeyContext::parse("GroupRename").unwrap();
        let input = KeyContext::parse("TextInput").unwrap();
        let start = binding::<super::RenameGroup>(&bindings)
            .predicate()
            .unwrap();
        assert!(start.depth_of(std::slice::from_ref(&list)).is_some());
        assert!(
            start
                .depth_of(&[list.clone(), rename.clone(), input.clone()])
                .is_none()
        );
        let commit = binding::<super::CommitGroupRename>(&bindings)
            .predicate()
            .unwrap();
        assert!(commit.depth_of(&[list, rename, input.clone()]).is_some());
        assert!(commit.depth_of(&[input]).is_none());
    }

    #[test]
    fn filter_exit_keys_only_match_the_filter_editor() {
        let bindings = studio_keybindings();
        let filter = KeyContext::parse("GroupFilter").unwrap();
        let input = KeyContext::parse("TextInput").unwrap();
        for binding in [
            binding::<super::LeaveFilter>(&bindings),
            binding::<super::EscapeFilter>(&bindings),
        ] {
            let p = binding.predicate().unwrap();
            assert_eq!(p.depth_of(&[filter.clone(), input.clone()]), Some(2));
            assert!(p.depth_of(std::slice::from_ref(&input)).is_none());
        }
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
            binding::<ExploreEscape>(&bindings),
        ];
        for binding in protected {
            let predicate = binding.predicate().expect("binding must be scoped");
            assert!(predicate.depth_of(&shell_context).is_some());
            assert!(predicate.depth_of(&editing_context).is_none());
        }

        // Stage switching (⌘1–⌘6) is intentionally global and non-editing.
        for binding in [
            binding::<StageData>(&bindings),
            binding::<StageSeries>(&bindings),
        ] {
            assert!(
                binding
                    .predicate()
                    .unwrap()
                    .depth_of(&editing_context)
                    .is_some()
            );
        }
    }

    #[test]
    fn assistant_prompt_context_blocks_main_shortcuts() {
        let contexts = [
            KeyContext::parse("Studio Explore").unwrap(),
            KeyContext::parse("Assistant").unwrap(),
            KeyContext::parse("TextInput MultilineTextInput").unwrap(),
        ];
        for binding in studio_keybindings() {
            let Some(predicate) = binding.predicate() else {
                continue;
            };
            if binding.action().as_any().is::<super::AssistantSend>()
                || binding.action().as_any().is::<super::AssistantStop>()
                || binding.action().as_any().is::<super::AssistantEscape>()
            {
                assert!(predicate.depth_of(&contexts).is_some());
            } else {
                assert!(
                    predicate.depth_of(&contexts).is_none(),
                    "main shortcut matched Assistant prompt: {:?}",
                    binding.keystrokes()
                );
            }
        }
        // The groups panel is a sibling of Assistant, never its ancestor.
        let groups = [
            KeyContext::parse("Studio Explore").unwrap(),
            KeyContext::parse("DataPanel").unwrap(),
        ];
        assert!(
            binding::<super::MarkAllGroups>(&studio_keybindings())
                .predicate()
                .unwrap()
                .depth_of(&groups)
                .is_some()
        );
    }

    #[test]
    fn series_frame_keys_yield_to_nested_text_input() {
        let bindings = studio_keybindings();
        let studio = KeyContext::parse("Studio OperandoWorkspace").unwrap();
        let operando = KeyContext::parse("Operando").unwrap();
        let text_input = KeyContext::parse("TextInput").unwrap();
        let scrub_context = [studio.clone(), operando.clone()];
        let editing_context = [studio, operando, text_input];

        for binding in [
            binding::<super::FramePrev>(&bindings),
            binding::<super::FrameNext>(&bindings),
            binding::<super::FrameJumpBack>(&bindings),
            binding::<super::FrameJumpFwd>(&bindings),
            binding::<FrameFirst>(&bindings),
            binding::<FrameLast>(&bindings),
        ] {
            let predicate = binding.predicate().expect("binding must be scoped");
            assert!(predicate.depth_of(&scrub_context).is_some());
            assert!(predicate.depth_of(&editing_context).is_none());
        }
    }

    #[test]
    fn peak_frame_keys_only_navigate_focused_results_not_parameter_editors() {
        let studio = KeyContext::parse("Studio Explore").unwrap();
        let peak = KeyContext::parse("PeakFits").unwrap();
        let editing = [
            studio.clone(),
            peak.clone(),
            KeyContext::parse("TextInput").unwrap(),
        ];
        let browsing = [studio.clone(), peak];
        let unrelated = [studio];
        let bindings = studio_keybindings();
        let navigators: Vec<_> = bindings
            .iter()
            .filter(|b| {
                (b.action().as_any().is::<super::FramePrev>()
                    || b.action().as_any().is::<super::FrameNext>())
                    && b.predicate().unwrap().depth_of(&browsing).is_some()
            })
            .collect();
        assert_eq!(navigators.len(), 2);
        for b in navigators {
            assert!(b.predicate().unwrap().depth_of(&editing).is_none());
            assert!(b.predicate().unwrap().depth_of(&unrelated).is_none());
        }
    }
}

pub(crate) fn packaged_data_file() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [
        dir.join("resources/examples/cu_150k.xmu"),
        dir.join("../Resources/examples/cu_150k.xmu"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn example_data_file() -> Option<PathBuf> {
    packaged_data_file().or_else(|| {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../rexafs/tests/testfiles/xraylarch_d867/xafsdata/cu_150k.xmu");
        path.is_file().then_some(path)
    })
}

fn default_for(param: PathParam) -> f64 {
    match param {
        PathParam::S02 => 0.9,
        PathParam::Degen => 1.0,
        PathParam::Sigma2 => 0.003,
        PathParam::E0
        | PathParam::DeltaR
        | PathParam::Ei
        | PathParam::Third
        | PathParam::Fourth => 0.0,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IndexChange {
    DerivedOnly,
    Catalog,
}

/// Catalog jobs and filters share the catalog's index lifetime. Keeping their
/// invalidation together makes the derived-only boundary testable without GPUI.
struct CatalogBindings<'a> {
    generations: [&'a mut u64; 6],
    running: [&'a mut bool; 5],
    cancellations: [&'a mut Option<Arc<AtomicBool>>; 4],
    filtered: &'a mut Option<Arc<Vec<usize>>>,
}

impl CatalogBindings<'_> {
    fn remove(
        self,
        inputs: &[BTreeSet<crate::group_identity::GroupId>; 5],
        removed: &BTreeSet<crate::group_identity::GroupId>,
    ) -> [bool; 5] {
        let [operando, batch, merge, fit, lcf, _] = self.generations;
        let [oc, bc, mc, lc] = self.cancellations;
        let mut interrupted = [false; 5];
        for (i, ((generation, running), cancel)) in [operando, batch, merge, fit, lcf]
            .into_iter()
            .zip(self.running)
            .zip([Some(oc), Some(bc), Some(mc), None, Some(lc)])
            .enumerate()
        {
            if !inputs[i].is_disjoint(removed) {
                interrupted[i] = retire_job(generation, running, cancel);
            }
        }
        interrupted
    }

    fn invalidate(self, change: IndexChange) {
        if change == IndexChange::DerivedOnly {
            return;
        }
        let [operando, batch, merge, fit, lcf, filter] = self.generations;
        for generation in [operando, batch, merge, fit, lcf] {
            *generation += 1;
        }
        if change == IndexChange::Catalog {
            *filter += 1;
            *self.filtered = None;
        }
        for running in self.running {
            *running = false;
        }
        for cancellation in self.cancellations {
            if let Some(cancel) = cancellation.take() {
                cancel.store(true, Ordering::Relaxed);
            }
        }
    }
}

fn evict_group_keys<T>(
    cache: &mut LruCache<(usize, u64), T>,
    removed: &BTreeSet<usize>,
    derived_changed: bool,
) {
    let keys: Vec<_> = cache
        .iter()
        .filter(|(key, _)| removed.contains(&key.0) || (derived_changed && key.0 >= DERIVED_BASE))
        .map(|(key, _)| *key)
        .collect();
    for key in keys {
        cache.pop(&key);
    }
}

fn finish_interrupted_results(
    batch: Option<&mut BatchFitData>,
    lcf: Option<&mut SeriesLcf>,
    interrupted: [bool; 5],
) {
    if interrupted[1]
        && let Some(batch) = batch
    {
        batch.cancelled = true;
        batch.rows.sort_by_key(|row| row.frame);
    }
    if interrupted[4]
        && let Some(lcf) = lcf
    {
        lcf.cancelled = true;
    }
}

/// A window-independent comparison request, shared by reopen and normal marks.
struct CompareLoad {
    ix: usize,
    fingerprint: u64,
    source: Result<PathBuf, DerivedSpectrum>,
    params: PipelineParams,
}

impl CompareLoad {
    fn process(&self) -> (usize, u64, GroupLoadResult) {
        let result = (|| {
            let (path, derived) = match &self.source {
                Ok(path) => (path.as_path(), None),
                Err(group) => {
                    if group.acquisition_mode() != rexafs::AbsorptionMode::Unknown
                        || !group.corrections.is_empty()
                        || group.processing_block_reason().is_some()
                        || group.quantity.prepared_space().is_some()
                    {
                        return group.for_display(&self.params).map(|sp| (sp, None));
                    }
                    (std::path::Path::new(""), Some(group))
                }
            };
            let raw = load_group_raw_with_diagnostics(path, &self.params, derived)?;
            let sp = process_arrays(raw.energy.clone(), raw.mu.clone(), &self.params)?;
            Ok((sp, Some(Arc::new(raw))))
        })();
        (self.ix, self.fingerprint, result)
    }
}

fn exclude_pending_current(loads: &mut Vec<CompareLoad>, pending: Option<usize>) {
    loads.retain(|load| Some(load.ix) != pending);
}

fn cached_marked_indices<'a>(
    selection: &'a BTreeSet<usize>,
    cache: &'a LruCache<(usize, u64), Arc<XASSpectrum>>,
    fingerprint: impl Fn(usize) -> u64 + 'a,
) -> impl Iterator<Item = usize> + 'a {
    selection
        .iter()
        .copied()
        .filter(move |&ix| ix != NO_ENTRY && cache.contains(&(ix, fingerprint(ix))))
}

fn cached_marked_spectra(
    selection: &BTreeSet<usize>,
    cache: &LruCache<(usize, u64), Arc<XASSpectrum>>,
    fingerprint: impl Fn(usize) -> u64,
    label: impl Fn(usize) -> String,
) -> Vec<(String, Arc<XASSpectrum>)> {
    cached_marked_indices(selection, cache, &fingerprint)
        .filter_map(|ix| {
            cache
                .peek(&(ix, fingerprint(ix)))
                .map(|sp| (label(ix), sp.clone()))
        })
        .collect()
}

fn missing_compare_loads(
    catalog: &Catalog,
    derived: &[DerivedSpectrum],
    params: &PipelineParams,
    overrides: &BTreeMap<usize, PipelineParams>,
    indices: &[usize],
    cache: &LruCache<(usize, u64), Arc<XASSpectrum>>,
) -> Vec<CompareLoad> {
    indices
        .iter()
        .filter_map(|&ix| {
            let (source, effective, fingerprint) = if ix >= DERIVED_BASE {
                let group = derived.get(ix - DERIVED_BASE)?;
                let effective = group.params.as_ref().unwrap_or(params);
                (Err(group.clone()), effective, group.fingerprint(effective))
            } else if ix < catalog.len() {
                let effective = overrides.get(&ix).unwrap_or(params);
                (Ok(catalog.path(ix)), effective, effective.fingerprint())
            } else {
                return None;
            };
            (!cache.contains(&(ix, fingerprint))).then(|| CompareLoad {
                ix,
                fingerprint,
                source,
                params: effective.clone(),
            })
        })
        .collect()
}

fn spectrum_status(label: &SharedString, sp: &XASSpectrum) -> SharedString {
    format!(
        "{} · {} points · E0 {:.1} eV",
        label,
        sp.energy.as_ref().map(|e| e.len()).unwrap_or(0),
        sp.e0().unwrap_or(f64::NAN),
    )
    .into()
}

impl StudioApp {
    /// Both side panels hidden (the assistant's "Focus plots" layout).
    pub(crate) fn panels_hidden(&self) -> bool {
        !self.data_panel_open && !self.context_panel_open
    }

    pub fn new_with_open(
        initial_open: Option<PathBuf>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let studio = cx.weak_entity();
        _window.on_window_should_close(cx, move |window, cx| {
            studio
                .update(cx, |app, cx| {
                    app.confirm_close(CloseIntent::CloseWindow, window, cx)
                })
                .unwrap_or(true)
        });
        // `REXAFS_THEME` wins for one launch; otherwise the saved preference.
        let theme = crate::settings::env_var("THEME")
            .ok()
            .as_deref()
            .and_then(crate::theme::ThemeMode::parse)
            .or_else(|| crate::settings::UserSettings::load().theme)
            .map_or_else(Theme::dark, Theme::for_mode);
        let params = PipelineParams::default();
        let param_fields = Self::build_param_fields(theme, cx);
        // Scripted launches (screenshots) can pick the initial stage.
        // The former stage names (normalize, background, transform) open Process.
        let initial_stage = crate::settings::env_var("STAGE")
            .ok()
            .and_then(|name| Stage::parse(&name))
            .map_or(Stage::Data, |(stage, _)| stage);

        let structure = shell::structure_view::StructureState::new(theme, cx);
        let mut app = Self {
            theme,
            structure,
            workspace: initial_stage.workspace(),
            stage: initial_stage,
            stage_view: StageView::default(),
            ui: Default::default(),
            thumbs: None,
            handles: HandleState::default(),
            tools: ToolState::new(),
            analysis: shell::tools::AnalysisState::default(),
            measurements: Default::default(),
            series_fits: Default::default(),
            live: Default::default(),
            peaks: Default::default(),
            normalization: Default::default(),
            wavelet: Default::default(),
            fluorescence: Default::default(),
            journal: shell::journal::JournalState::default(),
            palette: None,
            path_route: None,
            measurement_import: None,
            measurement_request: 0,
            pending_routed_import: VecDeque::new(),
            inspector_scroll: gpui::ScrollHandle::new(),
            group_registry: Default::default(),
            group_state: Default::default(),
            frozen: BTreeSet::new(),
            data_panel_open: true,
            context_panel_open: true,
            catalog: Catalog::default(),
            source_dir: None,
            catalog_index_path: None,
            catalog_gen: 0,
            verify_running: false,
            selected: None,
            selection: BTreeSet::new(),
            focus_group: None,
            group_menu: None,
            group_rename: None,
            mark_anchor: None,
            derived: Vec::new(),
            next_derived_id: 1,
            pending_derived: None,
            pending_fit_restore: None,
            compare_gen: 0,
            compare_running: false,
            plot_coverage: Default::default(),
            view: ViewOptions::default(),
            view_offset_field: None,
            filter_input: None,
            filter_text: String::new(),
            filter_reveal: None,
            reveal_current: None,
            filtered: None,
            filter_gen: 0,
            root_focus: cx.focus_handle(),
            data_focus: cx.focus_handle(),
            operando_focus: cx.focus_handle(),
            adv_open: [false; 4],
            import_preview: None,
            import_editor: None,
            import_preview_error: "".into(),
            import_preview_gen: 0,
            open_enum: None,
            param_menu: None,
            param_context_menu: None,
            publish: Default::default(),
            main_window: _window.window_handle(),
            assistant_window: None,
            assistant: None,
            assistant_host: Default::default(),
            assistant_resizing: None,
            last_opened_side_panel: None,
            generation: 0,
            load_running: false,
            recompute_epoch: 0,
            params,
            overrides: BTreeMap::new(),
            pending_overrides: Vec::new(),
            pending_project_spectrum: None,
            param_fields,
            cache: LruCache::new(NonZeroUsize::new(PROCESSED_CACHE_CAPACITY).unwrap()),
            raw_cache: LruCache::new(NonZeroUsize::new(RAW_CACHE_CAPACITY).unwrap()),
            recompute_last: None,
            recompute_tick_pending: false,
            recompute_dirty: false,
            current_path: PathBuf::new(),
            spectrum_path: PathBuf::new(),
            spectrum_fingerprint: 0,
            spectrum_group: None,
            standalone_source: None,
            spectrum_quantity: crate::params::Quantity::RawMu,
            spectrum: None,
            spectrum_label: "".into(),
            stale_plots: None,
            quadrants: Vec::new(),
            quad_bindings: Vec::new(),
            quad_export_labels: Vec::new(),
            explore_plots_dirty: false,
            legend_entries: Vec::new(),
            mixed_overlay_weight: None,
            maximized: None,
            file_scroll: UniformListScrollHandle::new(),
            expanded_sources: BTreeMap::new(),
            groups_resize: None,
            expanded_scan: None,
            active_scan: None,
            overview_series: None,
            operando: None,
            series_display: series_display::SeriesDisplay::default(),
            operando_plots: None,
            operando_gen: 0,
            operando_running: false,
            operando_cancel: None,
            job_inputs: Default::default(),
            series_trend: TrendSource::E0,
            series_lcf: None,
            lcf_running: false,
            lcf_gen: 0,
            lcf_cancel: None,
            lcf_progress: (0, 0),
            time_pos: 0,
            pending_time_pos: None,
            fit_paths: Vec::new(),
            fit_path_infos: Vec::new(),
            fit_template: ParameterTemplate::default(),
            fit_template_dirty: false,
            fit_template_notes: Vec::new(),
            fit_vars: Vec::new(),
            fit_range_fields: Vec::new(),
            fit_ranges: FitRanges::default(),
            fit_history: Vec::new(),
            joint: Default::default(),
            fit_preview: Default::default(),
            fit_mode: Default::default(),
            rmc: Default::default(),
            fit_history_results: BTreeMap::new(),
            fit_history_selected: None,
            fit_result: None,
            fit_provenance: None,
            fit_plots: None,
            fit_gen: 0,
            fit_running: false,
            fit_error: None,
            last_fit_duration: None,
            feff_workspace: None,
            project_extensions: Default::default(),
            project_path: None,
            project_header: None,
            project_storage: Default::default(),
            project_raw_files: Vec::new(),
            project_source_origins: BTreeMap::new(),
            project_generation: 0,
            assistant_history: Default::default(),
            assistant_history_revision: 0,
            assistant_history_saved_revision: 0,
            project_load_generation: 0,
            project_saving: false,
            saved_fingerprint: None,
            settle_saved_fingerprint: false,
            pending_close: None,
            updates: Default::default(),
            help: Default::default(),
            feff_running: false,
            feff_gen: 0,
            batch_fit: None,
            batch_table_scroll: gpui::ScrollHandle::new(),
            batch_preview: false,
            batch_running: false,
            batch_progress: (0, 0),
            batch_gen: 0,
            batch_cancel: None,
            batch_started: None,
            merge_running: false,
            merge_gen: 0,
            merge_cancel: None,
            status: "".into(),
            status_kind: (StatusKind::Info, "".into()),
            job_errors: Vec::new(),
            parser_evidence: Default::default(),
            imports: Default::default(),
            intake: Default::default(),
            intake_cancel: None,
            problems_batch: None,
            problems_page: 0,
            group_diagnostics: Default::default(),
            problems_open: false,
            card_px: BTreeMap::new(),
            viewport_w: 1440.0,
            viewport_h: 900.0,
        };
        if crate::debug_stats::enabled() {
            Self::start_debug_stats(cx);
        }
        app.fit_range_fields = Self::build_range_fields(theme, &app.fit_ranges.clone(), cx);
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
                    this.invalidate_explore_plots(cx);
                    cx.notify();
                }
                FieldEvent::Changed(None) => {}
                FieldEvent::InspectMixed => {}
                FieldEvent::Invalid(message) => {
                    this.set_status(StatusKind::Error, message.clone());
                    cx.notify();
                }
            };
        })
        .detach();
        app.view_offset_field = Some(offset_field);
        let filter_input = cx.new(|cx| TextInput::new("filter… (* glob) · ⌘P", "", theme, cx));
        cx.subscribe(&filter_input, |this: &mut Self, _f, event, cx| {
            // Per-keystroke filtering (doc "Search-first"): Edited and
            // Committed both re-run the (background) match.
            let (InputEvent::Committed(text) | InputEvent::Edited(text)) = event else {
                return;
            };
            let text = text.trim().to_string();
            if text != this.filter_text {
                this.filter_text = text;
                this.apply_filter(cx);
            }
        })
        .detach();
        app.filter_input = Some(filter_input);
        // Scripted launches: REXAFS_STRUCTURE_SOURCE=builtin|cif|mp|amcsd|cod,
        // REXAFS_IMPORT_CIF=<file>, REXAFS_SETTINGS=<settings.json>.
        if let Ok(src) = crate::settings::env_var("STRUCTURE_SOURCE") {
            app.structure.source = match src.as_str() {
                "cif" => crate::structure::StructureSourceKind::LocalCif,
                "mp" => crate::structure::StructureSourceKind::MaterialsProject,
                "amcsd" => crate::structure::StructureSourceKind::Amcsd,
                "cod" => crate::structure::StructureSourceKind::Cod,
                _ => crate::structure::StructureSourceKind::Builtin,
            };
        }
        app.structure_search(cx);
        app.root_focus.focus(_window, cx);
        if app
            .structure
            .settings
            .check_updates_on_startup
            .unwrap_or(true)
        {
            app.check_for_updates(cx);
        }
        if let Ok(file) = crate::settings::env_var("IMPORT_CIF") {
            app.structure_import_cif_path(PathBuf::from(file), cx);
        }
        if let Some(path) = initial_open {
            app.route_paths(vec![path], false, cx);
        }
        app
    }

    /// Write the status-bar message with its severity. Errors are drawn in the
    /// theme's error colour; plain progress notes may still assign `status`
    /// directly and count as Info.
    pub(crate) fn set_status(&mut self, kind: StatusKind, text: impl Into<SharedString>) {
        self.status = text.into();
        self.status_kind = (kind, self.status.clone());
    }

    fn status_kind(&self) -> StatusKind {
        if self.status_kind.1 == self.status {
            self.status_kind.0
        } else {
            StatusKind::Info
        }
    }

    /// Problem rows name their source: the group label and file name when the
    /// label is a path to an imported (or project-embedded) file, with the full
    /// path returned separately for a tooltip.
    fn problem_source(&self, label: &str) -> (String, Option<String>) {
        let path = std::path::Path::new(label);
        if !path.is_absolute() {
            return (label.to_string(), None);
        }
        let file = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| label.to_string());
        let group = self
            .parser_evidence
            .iter()
            .find(|(_, record)| record.path == path)
            .and_then(|(id, _)| self.menu_index(id))
            .map(|ix| self.entry_label(ix));
        let title = match group {
            Some(group) if group != file => format!("{group} · {file}"),
            _ => file,
        };
        (title, Some(label.to_string()))
    }

    pub(crate) fn record_job_error(
        &mut self,
        label: impl Into<String>,
        message: impl Into<String>,
    ) {
        push_problem(
            &mut self.job_errors,
            JobError {
                batch: None,
                severity: ProblemSeverity::Error,
                label: label.into(),
                message: message.into(),
            },
        );
    }

    fn intake_origin(
        &self,
        ix: usize,
        path: &std::path::Path,
    ) -> Option<import_state::IntakeOrigin> {
        self.peek_group_id(ix)
            .and_then(|group| self.intake.origin(path, &group))
    }

    fn record_source_warnings(
        &mut self,
        ix: usize,
        origin: Option<&import_state::IntakeOrigin>,
        path: &std::path::Path,
        diagnostics: &crate::params::ParserDiagnostics,
        declared_edge: Option<crate::source_evidence::DeclaredEdge>,
        channel: DetectionMode,
    ) {
        if let Some(group) = self.group_id(ix) {
            self.parser_evidence.insert(
                group,
                crate::source_evidence::ParserRecord {
                    path: path.to_path_buf(),
                    channel,
                    mapping_revision: crate::import_recipes::mapping_revision(
                        &self.effective_params(ix).import,
                    ),
                    diagnostics: diagnostics.clone(),
                    declared_edge,
                },
            );
        }
        for message in diagnostics.warnings() {
            if let Some(origin) = origin.filter(|o| self.intake.outcome(o).is_some()) {
                if let Some(outcome) = self.intake.outcome(origin)
                    && !outcome.warnings.contains(&message)
                {
                    outcome.warnings.push(message.clone());
                }
                self.record_intake_problem(origin.batch, path, message, ProblemSeverity::Warning);
            } else {
                push_problem(&mut self.job_errors, JobError::warning(path, message));
            }
        }
    }

    fn saved_parser_record(&self, ix: usize) -> Option<&crate::source_evidence::ParserRecord> {
        self.parser_evidence
            .get(&self.peek_group_id(ix)?)
            .filter(|record| record.matches(&self.effective_params(ix).import))
    }

    fn group_declared_edge(&self, ix: usize) -> Option<crate::source_evidence::DeclaredEdge> {
        if let Some(group) = ix
            .checked_sub(DERIVED_BASE)
            .and_then(|i| self.derived.get(i))
            && group.source.is_none()
        {
            return group.declared_edge.clone();
        }
        self.raw_cache
            .peek(&(ix, self.effective_params(ix).raw_fingerprint()))
            .and_then(|raw| raw.declared_edge.clone())
            .or_else(|| {
                self.saved_parser_record(ix)
                    .and_then(|record| record.declared_edge.clone())
            })
    }

    fn record_source_error(
        &mut self,
        origin: Option<&import_state::IntakeOrigin>,
        label: String,
        message: String,
    ) {
        if let Some(origin) = origin.filter(|o| self.intake.outcome(o).is_some()) {
            if let Some(outcome) = self.intake.outcome(origin) {
                outcome.failed = Some(message.clone());
            }
            self.record_intake_problem(origin.batch, &origin.path, message, ProblemSeverity::Error);
        } else {
            self.record_job_error(label, message);
        }
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
            + usize::from(self.lcf_running)
    }

    /// Editing always targets the current catalog spectrum, including when
    /// other spectra are marked for comparison. Bulk copying is explicit.
    fn override_target(&self) -> Option<usize> {
        let ix = self.selected?;
        self.valid_group_index(ix).then_some(ix)
    }

    fn valid_group_index(&self, ix: usize) -> bool {
        !self.group_registry.index_excluded(ix)
            && (ix < self.catalog.len()
                || (ix >= DERIVED_BASE && ix - DERIVED_BASE < self.derived.len()))
    }

    fn custom_params(&self, ix: usize) -> Option<&PipelineParams> {
        if ix >= DERIVED_BASE {
            self.derived.get(ix - DERIVED_BASE)?.params.as_ref()
        } else {
            self.overrides.get(&ix)
        }
    }

    fn set_custom_params(&mut self, ix: usize, params: Option<PipelineParams>) {
        store_custom_params(
            self.catalog.len(),
            &mut self.overrides,
            &mut self.derived,
            ix,
            params,
        );
    }

    fn active_group_id(&self) -> Option<u64> {
        self.selected
            .filter(|&ix| ix >= DERIVED_BASE)
            .and_then(|ix| self.derived.get(ix - DERIVED_BASE))
            .map(|d| d.id)
    }

    fn next_group_id(&mut self) -> u64 {
        let id = self.next_derived_id;
        self.next_derived_id += 1;
        id
    }

    /// Effective params for an entry: its override, else the global set.
    fn effective_params(&self, ix: usize) -> &PipelineParams {
        self.custom_params(ix).unwrap_or(&self.params)
    }

    fn effective_fingerprint(&self, ix: usize) -> u64 {
        let params = self.effective_params(ix);
        if ix >= DERIVED_BASE
            && let Some(group) = self.derived.get(ix - DERIVED_BASE)
        {
            group.fingerprint(params)
        } else {
            params.fingerprint()
        }
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
            Some(ix) if ix >= DERIVED_BASE => self.derived[ix - DERIVED_BASE]
                .params
                .get_or_insert_with(|| self.params.clone()),
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
            .and_then(|ix| self.custom_params(ix))
            .is_some_and(|ov| section_differs(ov, &self.params, section))
    }

    /// Chip action: copy the global section back over the override,
    /// dropping the override entirely once nothing diverges anymore.
    fn reset_section_override(&mut self, section: ParamSection, cx: &mut Context<Self>) {
        use shell::{ProcessSection, parameter_actions::ParamScope};
        let scope = match section {
            ParamSection::Import => ParamScope::Stage(Stage::Data),
            ParamSection::Norm => ParamScope::Process(ProcessSection::Normalize),
            ParamSection::Bkg => ParamScope::Process(ProcessSection::Background),
            ParamSection::Fft => ParamScope::Process(ProcessSection::Transform),
        };
        self.reset_scope(scope, cx);
    }

    /// Push the displayed param set into the context-panel fields (never
    /// emits change events). Called whenever the edit target may have
    /// changed: selection moves, multi-select edits, resets, project load.
    fn sync_param_fields(&mut self, cx: &mut Context<Self>) {
        self.restore_param_field_text(cx);
        self.update_import_preview(cx);
    }

    /// Restore canonical input text without discarding the live column preview.
    fn restore_param_field_text(&mut self, cx: &mut Context<Self>) {
        let params = self.ui_params().clone();
        for (key, field) in &self.param_fields {
            let value = param_field_value(*key, &params);
            field.update(cx, |f, cx| f.set_value(value, cx));
        }
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
                None => {
                    missing += 1;
                    self.pending_overrides.push(saved);
                }
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

    fn restore_project_selection(&mut self, cx: &mut Context<Self>) {
        self.restore_active_project_group(cx);
        self.ensure_compare_loaded(cx);
    }

    fn restore_active_project_group(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.pending_derived.take() {
            if let Some(i) = self.derived.iter().position(|d| d.id == id) {
                self.select_entry(DERIVED_BASE + i, cx);
                return;
            }
        }
        let Some(path) = self.pending_project_spectrum.take() else {
            if let Some(ix) = self.selected {
                self.select_entry(ix, cx);
            }
            return;
        };
        if let Some(ix) = self.catalog.find_by_path(&path) {
            self.select_entry(ix, cx);
        } else {
            let label = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            self.current_path = path.clone();
            self.load_spectrum(NO_ENTRY, path, label.into(), cx);
        }
    }

    fn fit_model_fingerprint(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        if self.joint.config.enabled {
            serde_json::to_vec(&self.joint.config)
                .unwrap_or_default()
                .hash(&mut hasher);
            for d in &self.joint.config.datasets {
                self.joint_dataset_params(d)
                    .map(|p| p.fingerprint())
                    .hash(&mut hasher);
            }
        }
        for row in &self.fit_paths {
            row.spec.file.hash(&mut hasher);
            row.spec.label.hash(&mut hasher);
            row.spec.s02.hash(&mut hasher);
            row.spec.degen.hash(&mut hasher);
            row.spec.e0.hash(&mut hasher);
            row.spec.sigma2.hash(&mut hasher);
            row.spec.deltar.hash(&mut hasher);
            row.spec.ei.hash(&mut hasher);
            row.spec.third.hash(&mut hasher);
            row.spec.fourth.hash(&mut hasher);
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
        self.fit_ranges.follow_transform.hash(&mut hasher);
        self.fit_ranges.kweight.to_bits().hash(&mut hasher);
        for kw in &self.fit_ranges.kweights {
            kw.to_bits().hash(&mut hasher);
        }
        self.fit_ranges.fitspace.hash(&mut hasher);
        self.fit_ranges.noise.hash(&mut hasher);
        hasher.finish()
    }

    fn fit_is_stale(&self) -> bool {
        let Some(provenance) = &self.fit_provenance else {
            return false;
        };
        if self.joint.result_config.is_some() {
            return !self.joint.config.enabled
                || provenance.model_fingerprint != self.fit_model_fingerprint();
        }
        self.spectrum.is_none()
            || provenance.group_id != self.active_group_id()
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

    fn standalone_path(&self) -> Option<&std::path::Path> {
        self.standalone_source
            .as_ref()
            .filter(|(_, _, id)| self.catalog.is_empty() && !self.group_registry.is_excluded(id))
            .map(|(path, _, _)| path.as_path())
    }

    fn peek_group_id(&self, ix: usize) -> Option<crate::group_identity::GroupId> {
        if self.group_registry.index_excluded(ix) {
            return None;
        }
        if ix == NO_ENTRY {
            return self
                .standalone_source
                .as_ref()
                .filter(|(_, _, id)| !self.group_registry.is_excluded(id))
                .map(|(_, _, id)| id.clone());
        }
        self.group_registry.id(ix).or_else(|| {
            (ix < self.catalog.len()).then(|| {
                self.group_registry.peek_source(
                    &self.catalog.path(ix),
                    self.effective_params(ix).import.mode,
                    &self.project_source_origins,
                )
            })
        })
    }

    /// Materialize a durable source identity only when a caller references it.
    fn group_id(&self, ix: usize) -> Option<crate::group_identity::GroupId> {
        if self.group_registry.index_excluded(ix) {
            return None;
        }
        if ix == NO_ENTRY {
            return self.peek_group_id(ix);
        }
        self.group_registry.id(ix).or_else(|| {
            (ix < self.catalog.len()).then(|| {
                self.group_registry.register_source(
                    Some(ix),
                    self.catalog.path(ix),
                    self.effective_params(ix).import.mode,
                    &self.project_source_origins,
                )
            })
        })
    }

    fn standalone_group_id(&self, path: &std::path::Path) -> crate::group_identity::GroupId {
        self.group_registry.register_source(
            None,
            path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
            self.params.import.mode,
            &self.project_source_origins,
        )
    }

    fn capture_group_state(&self, state: &mut crate::group_identity::GroupState) {
        for ix in self
            .selection
            .iter()
            .chain(&self.frozen)
            .chain(self.overrides.keys())
            .copied()
            .chain(self.selected)
        {
            self.group_id(ix);
        }
        state.capture(
            &self.group_registry,
            &self.selection,
            &self.frozen,
            &self.overrides,
            self.selected,
        );
        if let Some((_, _, id)) = &self.standalone_source {
            if self.selection.contains(&NO_ENTRY) {
                state.marked.insert(id.clone());
            } else if self.group_registry.index(id).is_none() {
                state.marked.remove(id);
            }
            state.capture_standalone_lock(
                &self.group_registry,
                id,
                self.frozen.contains(&NO_ENTRY),
            );
            if self.current_group_index() == Some(NO_ENTRY) {
                state.current = Some(id.clone());
            }
        }
    }

    fn capture_groups(&mut self) -> group_rows::InteractionIds {
        let interaction = group_rows::InteractionIds::capture(
            [self.focus_group, self.mark_anchor, self.reveal_current],
            |ix| self.group_id(ix),
        );
        let mut state = std::mem::take(&mut self.group_state);
        self.capture_group_state(&mut state);
        self.group_state = state;
        interaction
    }

    fn resolve_interaction(&mut self, interaction: group_rows::InteractionIds) {
        [self.focus_group, self.mark_anchor, self.reveal_current] = interaction.resolve(|id| {
            self.group_registry.index(id).or_else(|| {
                self.standalone_path()
                    .and(self.standalone_source.as_ref())
                    .filter(|(_, _, standalone)| standalone == id)
                    .map(|_| NO_ENTRY)
            })
        });
    }

    fn resolve_group_state(&mut self) {
        self.selection = self.group_registry.indices(&self.group_state.marked);
        self.frozen = self.group_registry.indices(&self.group_state.frozen);
        self.overrides = self.group_state.resolved_overrides(&self.group_registry);
        self.selected = self
            .group_state
            .current
            .as_ref()
            .and_then(|id| self.group_registry.index(id));
        if let Some((_, _, id)) = self
            .standalone_source
            .as_ref()
            .filter(|(_, _, id)| self.catalog.is_empty() && !self.group_registry.is_excluded(id))
        {
            if self.group_state.marked.contains(id) {
                self.selection.insert(NO_ENTRY);
            }
            shell::groups_panel::restore_standalone_lock(&mut self.frozen, &self.group_state, id);
        }
        self.migrate_catalog_standalone();
    }

    /// Central adapter for derived insert/remove/reorder. Catalog indices did
    /// not move: catalog jobs, filters, thumbnails and caches remain valid.
    fn rekey_after_catalog_change(&mut self) {
        let interaction = self.capture_groups();
        self.group_registry.reserve_groups(&self.derived);
        for group in &mut self.derived {
            self.group_registry
                .assign_group(group, &self.project_source_origins);
        }
        for group in &self.derived {
            if let Some(id) = &group.group_id {
                self.group_state
                    .colors
                    .entry(id.clone())
                    .or_insert_with(|| group_rows::color_index(id) as u8);
            }
        }
        if self.group_registry.replace_derived(&self.derived) {
            self.invalidate_index_bindings(IndexChange::DerivedOnly);
        }
        self.resolve_group_state();
        self.resolve_interaction(interaction);
    }

    fn invalidate_index_bindings(&mut self, change: IndexChange) {
        self.tools.invalidate_bindings();
        self.generation += 1;
        self.compare_gen += 1;
        self.load_running = false;
        self.compare_running = false;
        if change == IndexChange::Catalog {
            finish_interrupted_results(
                self.batch_fit.as_mut(),
                self.series_lcf.as_mut(),
                [false, self.batch_running, false, false, self.lcf_running],
            );
        }
        CatalogBindings {
            generations: [
                &mut self.operando_gen,
                &mut self.batch_gen,
                &mut self.merge_gen,
                &mut self.fit_gen,
                &mut self.lcf_gen,
                &mut self.filter_gen,
            ],
            running: [
                &mut self.operando_running,
                &mut self.batch_running,
                &mut self.merge_running,
                &mut self.fit_running,
                &mut self.lcf_running,
            ],
            cancellations: [
                &mut self.operando_cancel,
                &mut self.batch_cancel,
                &mut self.merge_cancel,
                &mut self.lcf_cancel,
            ],
            filtered: &mut self.filtered,
        }
        .invalidate(change);
        if change == IndexChange::Catalog {
            self.cache.clear();
            self.raw_cache.clear();
            self.thumbs = None;
        } else {
            // Only shifted derived slots can now refer to another group.
            evict_group_keys(&mut self.cache, &BTreeSet::new(), true);
            evict_group_keys(&mut self.raw_cache, &BTreeSet::new(), true);
        }
    }

    /// Retire the standalone adapter once its durable source has a catalog slot.
    fn migrate_catalog_standalone(&mut self) {
        if let Some((_, _, id)) = &self.standalone_source
            && let Some(ix) = self.group_registry.index(id)
        {
            shell::groups_panel::migrate_standalone_lock(&mut self.frozen, ix);
            group_rows::migrate_standalone(
                &mut self.selection,
                &mut self.focus_group,
                &mut self.mark_anchor,
                &mut self.reveal_current,
                ix,
            );
            if self.selected.is_none()
                && self
                    .spectrum_group
                    .as_ref()
                    .and_then(|t| t.group_id.as_ref())
                    == Some(id)
            {
                self.selected = Some(ix);
            }
            if let Some(target) = &mut self.spectrum_group
                && target.group_id.as_ref() == Some(id)
            {
                target.ix = ix;
            }
            self.standalone_source = None;
            let aliases: Vec<_> = self
                .cache
                .iter()
                .filter(|(key, _)| key.0 == NO_ENTRY)
                .map(|(key, _)| *key)
                .collect();
            for key in aliases {
                if let Some(sp) = self.cache.pop(&key) {
                    self.cache.put((ix, key.1), sp);
                }
            }
        }
    }

    /// Append keeps existing catalog indices and workers, migrating a newly
    /// cataloged standalone source through its durable identity.
    fn register_appended_groups(&mut self, catalog_start: usize, derived_start: usize) {
        self.group_registry
            .append_catalog(&self.catalog, catalog_start);
        self.group_registry
            .append_derived(&self.derived, derived_start);
        self.migrate_catalog_standalone();
        let appended = |ix: &usize| {
            (*ix < DERIVED_BASE && *ix >= catalog_start) || *ix >= DERIVED_BASE + derived_start
        };
        self.selection.extend(
            self.group_registry
                .indices(&self.group_state.marked)
                .into_iter()
                .filter(appended),
        );
        self.frozen.extend(
            self.group_registry
                .indices(&self.group_state.frozen)
                .into_iter()
                .filter(appended),
        );
        for (ix, params) in self.group_state.resolved_overrides(&self.group_registry) {
            if appended(&ix) {
                self.overrides.entry(ix).or_insert(params);
            }
        }
        if self.selected.is_none() {
            self.selected = self
                .group_state
                .current
                .as_ref()
                .and_then(|id| self.group_registry.index(id))
                .filter(appended);
        }
    }

    /// Retire workers and clear catalog presentation before a folder streams.
    /// Durable state remains pending until its groups resolve again.
    fn reset_catalog_state(&mut self, cx: &mut Context<Self>) {
        self.intake.stop();
        if let Some(cancel) = self.intake_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Some(id) = self.intake.active {
            self.intake.finish(id);
        }
        self.intake.reveal.clear();
        self.catalog_gen += 1;
        self.invalidate_index_bindings(IndexChange::Catalog);
        let interaction = self.capture_groups();
        self.standalone_source = None;
        self.catalog = Catalog::default();
        self.group_registry.replace_catalog(Default::default());
        self.group_registry.replace_derived(&self.derived);
        self.resolve_group_state();
        self.resolve_interaction(interaction);
        self.catalog_index_path = None;
        self.verify_running = false;
        self.load_running = false;
        self.compare_running = false;
        self.merge_running = false;
        self.filtered = None;
        self.filter_gen += 1;
        self.filter_text.clear();
        if let Some(input) = &self.filter_input {
            input.update(cx, |input, cx| input.set_text("", cx));
        }
        self.cache.clear();
        self.raw_cache.clear();
        self.thumbs = None;
        self.stale_plots = None;
        self.pending_project_spectrum = None;
        self.current_path = PathBuf::new();
        self.spectrum_path = PathBuf::new();
        self.spectrum_fingerprint = 0;
        self.spectrum_group = None;
        self.spectrum = None;
        self.group_diagnostics = Default::default();
        self.parser_evidence.clear();
        self.spectrum_label = "no spectrum".into();
        self.import_preview = None;
        self.import_preview_error = "".into();
        self.import_preview_gen += 1;
        self.quadrants.clear();
        self.quad_bindings.clear();
        self.quad_export_labels.clear();
        self.maximized = None;
        self.file_scroll = UniformListScrollHandle::new();
        self.expanded_sources.clear();
        self.expanded_scan = None;
        self.active_scan = None;
        self.overview_series = None;
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
        self.pending_routed_import.clear();
        let mut cancelled = false;
        self.intake.stop();
        if let Some(cancel) = &self.intake_cancel {
            cancel.store(true, Ordering::Relaxed);
            // Save the accepted source list; reopening must not rediscover the cancelled remainder.
            self.source_dir = None;
            self.catalog_index_path = None;
            cancelled = true;
        }
        if (self.catalog.scanning && self.intake_cancel.is_none()) || self.verify_running {
            self.catalog_gen += 1;
            self.catalog.scanning = false;
            self.verify_running = false;
            self.resolve_pending_overrides(cx);
            self.restore_project_selection(cx);
            cancelled = true;
        }
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
        if self.lcf_running
            && let Some(cancel) = self.lcf_cancel.as_ref()
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
        // Labels and hover explanations come from `param_text`, one reviewable
        // table. Pre-edge and post-edge ranges are eV *relative to E0*; the
        // section title and tooltips say so, since a 260px panel leaves ~132px
        // for a label and "(eV rel. E0)" does not fit.
        let specs = [
            (ParamKey::ImpEnergyCol, "0", COL),
            (ParamKey::ImpI0Col, "1", COL),
            (ParamKey::ImpItCol, "2", COL),
            (ParamKey::ImpIrCol, "3", COL),
            (ParamKey::AlignTarget, "e.g. 22117", FLOAT),
            (ParamKey::EnergyOffset, "0", FLOAT),
            (ParamKey::E0, "auto", FLOAT),
            (ParamKey::EdgeStep, "auto", FLOAT),
            (ParamKey::PreEdgeStart, "auto (-200)", FLOAT),
            (ParamKey::PreEdgeEnd, "auto (-30)", FLOAT),
            (ParamKey::NormStart, "auto (150)", FLOAT),
            (ParamKey::NormEnd, "auto (spectrum end)", FLOAT),
            (ParamKey::NormPolyorder, "auto (2)", INT),
            (ParamKey::NVictoreen, "auto (0)", INT),
            (ParamKey::Rbkg, "auto (1.0)", FLOAT),
            // Scoped names: the AUTOBK and FFT k-ranges are different
            // parameters with different values ("Spline k …" vs "FT k …").
            (ParamKey::BkgKmin, "auto (0)", FLOAT),
            (ParamKey::BkgKmax, "auto (full)", FLOAT),
            (ParamKey::BkgKstep, "auto (0.05)", FLOAT),
            (ParamKey::BkgNknots, "auto", INT),
            (ParamKey::BkgKweight, "auto (1)", INT),
            (ParamKey::BkgClampLo, "auto (0)", INT),
            (ParamKey::BkgClampHi, "auto (1)", INT),
            (ParamKey::BkgNclamp, "auto (3)", COL),
            (ParamKey::BkgClampLambda, "auto (0.001)", FLOAT),
            (ParamKey::BkgDk, "auto (0.1)", FLOAT),
            (ParamKey::BkgNfft, "auto (2048)", INT),
            (ParamKey::FftKmin, "auto (2)", FLOAT),
            (ParamKey::FftKmax, "auto (15)", FLOAT),
            (ParamKey::FftDk, "auto (1)", FLOAT),
            (ParamKey::FftKweight, "auto (2)", FLOAT),
            (ParamKey::FftDk2, "auto", FLOAT),
            (ParamKey::FftRmax, "auto (10)", FLOAT),
            (ParamKey::FftKstep, "auto", FLOAT),
            (ParamKey::FftNfft, "auto (2048)", INT),
            (ParamKey::BftRmin, "auto (0)", FLOAT),
            (ParamKey::BftRmax, "auto (20)", FLOAT),
            (ParamKey::BftDr, "auto (1)", FLOAT),
            (ParamKey::BkgEk0, "auto (edge E₀)", FLOAT),
            (ParamKey::BkgRegularization, "auto (0.0001)", FLOAT),
            (ParamKey::BkgCondition, "auto (1e8)", FLOAT),
            (ParamKey::BkgResidualRatio, "auto (1.05)", FLOAT),
            (ParamKey::BftQmax, "auto (10)", FLOAT),
            (ParamKey::BftDr2, "auto (dR)", FLOAT),
            (ParamKey::BftRweight, "auto (0)", INT),
            (ParamKey::BftKstep, "auto (R grid)", FLOAT),
            (ParamKey::BftNfft, "auto (2048)", INT),
        ];
        specs
            .into_iter()
            .map(|(key, placeholder, kind)| {
                let step = param_step(key);
                let text = param_text::param_text(key);
                let field = cx.new(|cx| {
                    let mut field =
                        NumericField::new(text.label, placeholder, None, kind, theme, cx)
                            .with_step(step);
                    if let Some(tip) = text.tip {
                        field = field.with_description(tip);
                    }
                    if key == ParamKey::EnergyOffset {
                        field.with_display_decimals(2, cx)
                    } else {
                        field
                    }
                });
                cx.subscribe(
                    &field,
                    move |this: &mut Self, _field, event, cx| match event {
                        FieldEvent::Changed(value) => this.apply_param(key, *value, cx),
                        FieldEvent::InspectMixed => {
                            this.param_menu = Some(shell::parameter_actions::ParamScope::Field(
                                key.setting_key(),
                            ));
                            cx.notify();
                        }
                        FieldEvent::Invalid(message) => {
                            this.set_status(StatusKind::Error, message.clone());
                            cx.notify();
                        }
                    },
                )
                .detach();
                (key, field)
            })
            .collect()
    }

    /// Shared lock gate for parameter editors; restore rejected field text too.
    fn refuse_frozen_edit(&mut self, cx: &mut Context<Self>) -> bool {
        if shell::groups_panel::processing_locked(self.current_group_index(), &self.frozen) {
            self.status = "Processing is locked — unlock it to edit its parameters.".into();
            self.restore_param_field_text(cx);
            cx.notify();
            return true;
        }
        false
    }

    /// Commit one mapping edit or reset against the current effective settings.
    fn edit_parameters(
        &mut self,
        text: String,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut PipelineParams) -> anyhow::Result<()>,
    ) {
        self.edit_parameters_keyed(None, text, cx, edit);
    }

    fn edit_parameters_keyed(
        &mut self,
        key: Option<ParamKey>,
        text: String,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut PipelineParams) -> anyhow::Result<()>,
    ) {
        if self.refuse_frozen_edit(cx) {
            return;
        }
        let target = self.override_target();
        let before = self.ui_params().clone();
        match prepare_parameter_edit(&before, edit) {
            Ok(Some(after)) => {
                self.apply_params_to(target, after.clone());
                self.record_param_edit(target, key, before, after, text);
                self.schedule_recompute(cx);
                self.sync_handles(cx);
                self.invalidate_explore_plots(cx);
                self.sync_param_fields(cx);
            }
            result => {
                if let Err(error) = result {
                    self.set_status(StatusKind::Error, error.to_string());
                }
                self.restore_param_field_text(cx);
            }
        }
        cx.notify();
    }

    fn apply_param(&mut self, key: ParamKey, value: Option<f64>, cx: &mut Context<Self>) {
        let role = match key {
            ParamKey::ImpEnergyCol => Some(ImportRole::Energy),
            ParamKey::ImpI0Col => Some(ImportRole::I0),
            ParamKey::ImpItCol => Some(ImportRole::It),
            ParamKey::ImpIrCol => Some(ImportRole::Ir),
            _ => None,
        };
        if let Some(role) = role {
            self.set_import_role_column(
                role,
                value.map(|v| v.round().max(0.0) as usize),
                Some(key),
                cx,
            );
            return;
        }
        if self.refuse_frozen_edit(cx) {
            return;
        }
        let target = self.override_target();
        let before = self.ui_params().clone();
        let p = self.edit_params();
        let int = value.map(|v| v.round() as i32);
        match key {
            ParamKey::ImpEnergyCol
            | ParamKey::ImpI0Col
            | ParamKey::ImpItCol
            | ParamKey::ImpIrCol => {
                unreachable!("mapping fields use edit_parameters above")
            }
            ParamKey::AlignTarget => p.align_target = value,
            ParamKey::EnergyOffset => {
                if let Err(error) = p.set_energy_offset(value.unwrap_or(0.0)) {
                    self.set_status(StatusKind::Error, error);
                    self.restore_param_field_text(cx);
                    cx.notify();
                    return;
                }
            }
            ParamKey::E0 => p.e0 = value,
            ParamKey::EdgeStep => p.edge_step = value,
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
            ParamKey::BkgNclamp => p.bkg_nclamp = int,
            ParamKey::BkgClampLambda => p.bkg_clamp_lambda = value,
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
            ParamKey::BftRmin => p.bft_rmin = value,
            ParamKey::BftRmax => p.bft_rmax = value,
            ParamKey::BftDr => p.bft_dr = value,
            ParamKey::BkgEk0 => p.bkg_ek0 = value,
            ParamKey::BkgRegularization => p.bkg_linear_regularization = value,
            ParamKey::BkgCondition => p.bkg_linear_condition_limit = value,
            ParamKey::BkgResidualRatio => p.bkg_linear_residual_ratio_limit = value,
            ParamKey::BftQmax => p.bft_qmax = value,
            ParamKey::BftDr2 => p.bft_dr2 = value,
            ParamKey::BftRweight => p.bft_rweight = value,
            ParamKey::BftKstep => p.bft_kstep = value,
            ParamKey::BftNfft => p.bft_nfft = int,
        }
        let after = self.ui_params().clone();
        let shown = match value {
            Some(v) => format!("{v}"),
            None => "auto".into(),
        };
        let scope = match target {
            Some(ix) => self.entry_label(ix),
            None => "all groups".into(),
        };
        self.record_param_edit(
            target,
            Some(key),
            before,
            after,
            format!("{key:?} = {shown} · {scope}"),
        );
        self.schedule_recompute(cx);
        if key == ParamKey::EnergyOffset {
            self.sync_param_fields(cx);
        }
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
            EnumParam::BkgClampPolicy => {
                return vec![
                    "Fixed λ".into(),
                    "Legacy fixed scale".into(),
                    "Legacy two pass".into(),
                ];
            }
            EnumParam::BkgSolver => (
                "auto (LinearDirect)",
                AUTOBK_SOLVERS.iter().map(|v| format!("{v:?}")).collect(),
            ),
            EnumParam::FftGrid => return vec!["Input grid".into(), "Larch grid".into()],
            EnumParam::FftWindow => (
                "auto (KaiserBessel)",
                FT_WINDOWS.iter().map(|w| format!("{w:?}")).collect(),
            ),
            EnumParam::BkgFallback | EnumParam::BkgCache => {
                ("auto (on)", vec!["On".into(), "Off".into()])
            }
            EnumParam::BkgFallbackSolver => (
                "auto (Dog-leg)",
                vec!["Dog-leg".into(), "Levenberg–Marquardt".into()],
            ),
            EnumParam::BftWindow => (
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
        self.open_enum = None;
        if which == EnumParam::ImportMode {
            self.edit_parameters(format!("{which:?} = option {index}"), cx, |params| {
                params.import.mode = variant
                    .and_then(|i| DETECTION_MODES.get(i).copied())
                    .unwrap_or(DetectionMode::Auto);
                Ok(())
            });
            return;
        }
        if self.refuse_frozen_edit(cx) {
            return;
        }
        let target = self.override_target();
        let before = self.ui_params().clone();
        let p = self.edit_params();
        match which {
            EnumParam::ImportMode => unreachable!("mapping mode uses edit_parameters above"),
            EnumParam::BkgWindow => {
                p.bkg_window = variant.map(|i| FT_WINDOWS[i]);
            }
            EnumParam::BkgClampPolicy => {
                p.bkg_clamp_policy = match index {
                    1 => rexafs::prelude::AUTOBKClampScalePolicy::Fixed,
                    2 => rexafs::prelude::AUTOBKClampScalePolicy::TwoPass,
                    _ => rexafs::prelude::AUTOBKClampScalePolicy::FixedPenalty,
                };
                if p.bkg_clamp_policy == rexafs::prelude::AUTOBKClampScalePolicy::FixedPenalty {
                    p.bkg_solver = Some(rexafs::prelude::AUTOBKSolver::LinearDirect);
                }
            }
            EnumParam::BkgSolver => {
                p.bkg_solver = variant.map(|i| AUTOBK_SOLVERS[i]);
                if matches!(
                    p.bkg_solver,
                    Some(
                        rexafs::prelude::AUTOBKSolver::LegacyLm
                            | rexafs::prelude::AUTOBKSolver::TrustRegionDogLeg
                    )
                ) && p.bkg_clamp_policy == rexafs::prelude::AUTOBKClampScalePolicy::FixedPenalty
                {
                    p.bkg_clamp_policy = rexafs::prelude::AUTOBKClampScalePolicy::Fixed;
                }
            }
            EnumParam::FftGrid => {
                p.fft_grid = if index == 1 {
                    rexafs::FFTGrid::Larch
                } else {
                    rexafs::FFTGrid::Input
                }
            }
            EnumParam::FftWindow => {
                p.fft_window = variant.map(|i| FT_WINDOWS[i]);
            }
            EnumParam::BkgFallback => p.bkg_linear_fallback_to_lm = variant.map(|i| i == 0),
            EnumParam::BkgCache => p.bkg_linear_workspace_cache = variant.map(|i| i == 0),
            EnumParam::BkgFallbackSolver => {
                p.bkg_linear_fallback_solver = variant.map(|i| {
                    if i == 0 {
                        rexafs::prelude::AUTOBKSolver::TrustRegionDogLeg
                    } else {
                        rexafs::prelude::AUTOBKSolver::LegacyLm
                    }
                })
            }
            EnumParam::BftWindow => {
                p.bft_window = variant.map(|i| FT_WINDOWS[i]);
            }
        }
        self.open_enum = None;
        let after = self.ui_params().clone();
        self.record_param_edit(
            target,
            None,
            before,
            after,
            format!("{which:?} = option {index}"),
        );
        self.schedule_recompute(cx);
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
            EnumParam::BkgClampPolicy => match p.bkg_clamp_policy {
                rexafs::prelude::AUTOBKClampScalePolicy::FixedPenalty => 0,
                rexafs::prelude::AUTOBKClampScalePolicy::Fixed => 1,
                rexafs::prelude::AUTOBKClampScalePolicy::TwoPass => 2,
            },
            EnumParam::BkgSolver => p
                .bkg_solver
                .and_then(|v| AUTOBK_SOLVERS.iter().position(|x| *x == v))
                .map(|i| i + 1)
                .unwrap_or(0),
            EnumParam::FftGrid => usize::from(p.fft_grid == rexafs::FFTGrid::Larch),
            EnumParam::FftWindow => p
                .fft_window
                .and_then(|w| FT_WINDOWS.iter().position(|x| *x == w))
                .map(|i| i + 1)
                .unwrap_or(0),
            EnumParam::BkgFallback => p
                .bkg_linear_fallback_to_lm
                .map(|on| if on { 1 } else { 2 })
                .unwrap_or(0),
            EnumParam::BkgCache => p
                .bkg_linear_workspace_cache
                .map(|on| if on { 1 } else { 2 })
                .unwrap_or(0),
            EnumParam::BkgFallbackSolver => p
                .bkg_linear_fallback_solver
                .map(|v| {
                    if v == rexafs::prelude::AUTOBKSolver::TrustRegionDogLeg {
                        1
                    } else {
                        2
                    }
                })
                .unwrap_or(0),
            EnumParam::BftWindow => p
                .bft_window
                .and_then(|w| FT_WINDOWS.iter().position(|x| *x == w))
                .map(|i| i + 1)
                .unwrap_or(0),
        }
    }

    fn import_role_label(role: ImportRole) -> &'static str {
        match role {
            ImportRole::Energy => "energy",
            ImportRole::I0 => "I0",
            ImportRole::It => "It",
            ImportRole::Ir => "Ir",
        }
    }

    fn set_import_role_column(
        &mut self,
        role: ImportRole,
        column: Option<usize>,
        key: Option<ParamKey>,
        cx: &mut Context<Self>,
    ) {
        self.edit_parameters_keyed(
            key,
            format!("Set {} column = {column:?}", Self::import_role_label(role)),
            cx,
            |params| {
                let import = &mut params.import;
                match role {
                    ImportRole::Energy => import.energy_col = column,
                    ImportRole::I0 => import.i0_col = column,
                    ImportRole::It => import.it_col = column,
                    ImportRole::Ir => import.ir_col = column,
                }
                Ok(())
            },
        );
    }

    fn start_debug_stats(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let mut prev_render = 0u64;
            let mut prev_present = 0u64;
            let mut prev_dropped = 0u64;
            let started = Instant::now();
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(1))
                    .await;
                let tick = this.update(cx, |app, cx| {
                    let s = crate::debug_stats::take();
                    let stats = app.quadrants.first().map(|(_, e)| e.read(cx).stats());
                    let t = started.elapsed().as_secs();
                    match stats {
                        Some(st) => {
                            let render_frames = st.render.frame_count.saturating_sub(prev_render);
                            let present_frames =
                                st.presentation.frame_count.saturating_sub(prev_present);
                            let dropped = st.dropped_frames.saturating_sub(prev_dropped);
                            prev_render = st.render.frame_count;
                            prev_present = st.presentation.frame_count;
                            prev_dropped = st.dropped_frames;
                            eprintln!(
                                "[rexafs-stats t={t}s] pointer {}/s · paints {}/s · pointer→paint avg {:.1} ms max {:.1} ms (n={}) · ruviz frames {}/s last {:.1} ms avg {:.1} ms fps {:.0} target {:?} · presented {}/s interval avg {:.1} ms fps {:.0} · dropped {} · backend {:?}",
                                s.pointer_events,
                                s.paints,
                                s.latency_avg_ms,
                                s.latency_max_ms,
                                s.latency_n,
                                render_frames,
                                st.render.last_frame_time.as_secs_f64() * 1e3,
                                st.render.average_frame_time.as_secs_f64() * 1e3,
                                st.render.current_fps,
                                st.render.last_target,
                                present_frames,
                                st.presentation.average_present_interval.as_secs_f64() * 1e3,
                                st.presentation.current_fps,
                                dropped,
                                st.active_backend,
                            );
                        }
                        None => eprintln!(
                            "[rexafs-stats t={t}s] pointer {}/s · paints {}/s · pointer→paint avg {:.1} ms max {:.1} ms (n={}) · no stage plot",
                            s.pointer_events, s.paints, s.latency_avg_ms, s.latency_max_ms, s.latency_n
                        ),
                    }
                });
                if tick.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn schedule_recompute(&mut self, cx: &mut Context<Self>) {
        self.recompute_epoch += 1;
        if self.handles.dragging.is_some() {
            self.schedule_recompute_throttled(cx);
            return;
        }
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

    /// While a handle is dragged the pipeline follows the pointer: run at
    /// once when the last run is older than the tick, otherwise once per
    /// tick, with at most one job in flight (a tick that finds a job running
    /// marks it dirty and the job reruns when it lands).
    fn schedule_recompute_throttled(&mut self, cx: &mut Context<Self>) {
        if self.load_running {
            self.recompute_dirty = true;
            return;
        }
        let since = self
            .recompute_last
            .map(|t| t.elapsed())
            .unwrap_or(DRAG_RECOMPUTE_TICK);
        if since >= DRAG_RECOMPUTE_TICK {
            self.reprocess_current(cx);
            return;
        }
        if self.recompute_tick_pending {
            return;
        }
        self.recompute_tick_pending = true;
        let timer = cx.background_executor().timer(DRAG_RECOMPUTE_TICK - since);
        cx.spawn(async move |this, cx| {
            timer.await;
            this.update(cx, |app, cx| {
                app.recompute_tick_pending = false;
                if app.load_running {
                    app.recompute_dirty = true;
                } else {
                    app.reprocess_current(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn reprocess_current(&mut self, cx: &mut Context<Self>) {
        let Some(ix) = self.current_group_index() else {
            return;
        };
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
        if ix != NO_ENTRY && !self.valid_group_index(ix) {
            return;
        }
        if ix == NO_ENTRY
            && (path.as_os_str().is_empty()
                || self
                    .group_registry
                    .is_excluded(&self.standalone_group_id(&path)))
        {
            return;
        }
        if ix == NO_ENTRY {
            if self
                .standalone_source
                .as_ref()
                .is_some_and(|(old, _, _)| old != &path)
            {
                let keys: Vec<_> = self
                    .cache
                    .iter()
                    .filter(|(key, _)| key.0 == NO_ENTRY)
                    .map(|(key, _)| *key)
                    .collect();
                for key in keys {
                    self.cache.pop(&key);
                }
            }
            let id = self.standalone_group_id(&path);
            shell::groups_panel::restore_standalone_lock(&mut self.frozen, &self.group_state, &id);
            self.standalone_source = Some((path.clone(), label.clone(), id));
        } else {
            self.group_id(ix);
        }
        self.generation += 1;
        let generation = self.generation;
        let key = (ix, self.effective_fingerprint(ix));
        let diagnostics = self
            .group_id(ix)
            .map(|id| self.group_diagnostics.begin(id, key.1));
        self.recompute_last = Some(Instant::now());
        self.recompute_dirty = false;

        // NO_ENTRY has no path component in the shared cache key.
        if ix != NO_ENTRY
            && let Some(sp) = self.cache.get(&key)
        {
            self.load_running = false;
            let sp = sp.clone();
            let warnings = self
                .raw_cache
                .peek(&(ix, self.effective_params(ix).raw_fingerprint()))
                .map(|raw| {
                    raw.diagnostics
                        .warnings()
                        .into_iter()
                        .map(|message| JobError::warning(&path, message))
                        .collect()
                })
                .unwrap_or_default();
            if let Some(ticket) = &diagnostics {
                self.group_diagnostics.finish(ticket, warnings);
            }
            self.set_processed(ix, label, path, key.1, sp, cx);
            cx.notify();
            return;
        }

        self.status = format!("processing {label} ...").into();
        self.load_running = true;
        cx.notify();
        let raw_key = (ix, self.effective_params(ix).raw_fingerprint());
        let processed_path = path.clone();
        let intake_origin = self.intake_origin(ix, &path);
        let load = self.process_group_job(ix, path, cx);
        cx.spawn(async move |this, cx| {
            let result = load.await;
            this.update(cx, |app, cx| {
                if app.generation != generation {
                    return; // a newer selection/edit superseded this load
                }
                app.load_running = false;
                // Import can assign a catalog index while a standalone load is
                // running. Its completion must use the durable source's new slot.
                let ix = if ix == NO_ENTRY {
                    app.catalog
                        .find_by_canonical_path(&processed_path)
                        .unwrap_or(ix)
                } else {
                    ix
                };
                let key = (ix, key.1);
                let raw_key = (ix, raw_key.1);
                match result {
                    Ok((sp, raw)) => {
                        if let Some(ticket) = &diagnostics {
                            let warnings = raw
                                .as_ref()
                                .map(|raw| {
                                    raw.diagnostics
                                        .warnings()
                                        .into_iter()
                                        .map(|message| JobError::warning(&processed_path, message))
                                        .collect()
                                })
                                .unwrap_or_default();
                            app.group_diagnostics.finish(ticket, warnings);
                        }
                        if let Some(raw) = raw {
                            app.record_source_warnings(
                                ix,
                                intake_origin.as_ref(),
                                &processed_path,
                                &raw.diagnostics,
                                raw.declared_edge.clone(),
                                raw.channel,
                            );
                            if ix != NO_ENTRY {
                                app.raw_cache.put(raw_key, raw);
                            }
                        }
                        let sp = Arc::new(sp);
                        if ix != NO_ENTRY {
                            app.cache.put(key, sp.clone());
                        }
                        app.set_processed(ix, label, processed_path, key.1, sp, cx);
                        // A drag tick landed while this job ran: follow it.
                        if app.recompute_dirty {
                            app.recompute_dirty = false;
                            app.reprocess_current(cx);
                        }
                    }
                    Err(e) => {
                        app.set_status(
                            StatusKind::Error,
                            format!("failed to process {label}: {e}"),
                        );
                        app.record_source_error(
                            intake_origin.as_ref(),
                            label.to_string(),
                            e.to_string(),
                        );
                        if let Some(ticket) = &diagnostics {
                            app.group_diagnostics.finish(
                                ticket,
                                vec![JobError {
                                    batch: None,
                                    severity: ProblemSeverity::Error,
                                    label: label.to_string(),
                                    message: e.to_string(),
                                }],
                            );
                        }
                        // Retain the plot and its identity. Tool readiness rejects
                        // this failed selection even if the old data is present.
                        app.stale_plots = Some(StalePlots {
                            requested: label.clone(),
                            message: e.to_string().into(),
                        });
                        // As on success: a drag tick that landed while this job
                        // ran carries newer settings, which may fix the error.
                        if app.recompute_dirty {
                            app.recompute_dirty = false;
                            app.reprocess_current(cx);
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Snapshot inputs for either the current group or a tool operand. The
    /// caller decides where the result lands; this job never changes selection.
    fn process_group_job(
        &mut self,
        ix: usize,
        path: PathBuf,
        cx: &Context<Self>,
    ) -> gpui::Task<Result<(XASSpectrum, Option<RawArrays>), String>> {
        let params = self.effective_params(ix).clone();
        let derived = (ix >= DERIVED_BASE)
            .then(|| self.derived.get(ix - DERIVED_BASE).cloned())
            .flatten();
        let raw_key = (ix, params.raw_fingerprint());
        let raw = (ix != NO_ENTRY)
            .then(|| self.raw_cache.get(&raw_key).cloned())
            .flatten();
        cx.background_executor().spawn(async move {
            // Dispatch typed results before consulting the raw cache: cached
            // arrays must never bypass the quantity guard on later edits.
            if let Some(group) = &derived
                && (group.acquisition_mode() != rexafs::AbsorptionMode::Unknown
                    || group
                        .operation
                        .as_ref()
                        .is_some_and(|o| o.tool == "Live average")
                    || !group.corrections.is_empty()
                    || group.processing_block_reason().is_some()
                    || group.quantity.prepared_space().is_some())
            {
                return group.for_display(&params).map(|sp| (sp, None));
            }
            match raw {
                Some(raw) => process_arrays(raw.energy.clone(), raw.mu.clone(), &params)
                    .map(|sp| (sp, Some(raw))),
                None => {
                    let raw = load_group_raw_with_diagnostics(&path, &params, derived.as_ref())?;
                    let sp = process_arrays(raw.energy.clone(), raw.mu.clone(), &params)?;
                    Ok((sp, Some(Arc::new(raw))))
                }
            }
        })
    }

    /// Mark range fields that processing limited to this scan's data: explicit
    /// values get an amber note and one Problems warning for the group; automatic
    /// values show the used value in their placeholder. Settings are unchanged.
    /// A collapsed edge step adds its own Problems warning for the group.
    fn show_range_adjustments(&mut self, ix: usize, sp: &XASSpectrum, cx: &mut Context<Self>) {
        let adjustments = if self.effective_params(ix).mback.is_some() {
            Vec::new()
        } else {
            sp.range_adjustments()
        };
        let params = self.effective_params(ix).clone();
        let mut explicit = Vec::new();
        for (key, field) in &self.param_fields {
            let adjustment = adjustments
                .iter()
                .find(|a| range_adjustment_field(a.field).is_some_and(|(k, ..)| k == *key));
            let user_set = param_field_value(*key, &params).is_some();
            let note = adjustment.filter(|_| user_set).map(|a| {
                let unit = range_adjustment_field(a.field).map_or("", |(.., unit)| unit);
                SharedString::from(format!(
                    "Used {}: limited by the measured data",
                    range_used_text(a, unit)
                ))
            });
            if let Some(a) = adjustment {
                if user_set {
                    explicit.push(*a);
                } else {
                    let used = a.used;
                    field.update(cx, |f, cx| {
                        f.set_placeholder(format!("auto ({used:.1})"), cx)
                    });
                }
            }
            field.update(cx, |f, cx| f.set_adjusted(note, cx));
        }
        // Key by the full source path: same-named files in other folders are
        // separate groups, each with its own warning.
        let label = self.spectrum_path.display().to_string();
        replace_problem(
            &mut self.job_errors,
            &label,
            RANGE_WARNING_PREFIX,
            range_warning(&explicit).map(|m| (ProblemSeverity::Warning, m)),
        );
        let mu_span = sp.mu.as_ref().map_or(f64::NAN, |mu| mu.max() - mu.min());
        let edge_step = sp.normalization.as_ref().and_then(|n| n.get_edge_step());
        replace_problem(
            &mut self.job_errors,
            &label,
            EDGE_STEP_WARNING_PREFIX,
            edge_step_warning(edge_step, mu_span).map(|m| (ProblemSeverity::Warning, m)),
        );
    }

    /// Problems-drawer label for a group: its full source path when it has
    /// one (matching the processing warnings), otherwise its display label.
    fn problem_label(&self, ix: usize) -> String {
        let path = if ix < self.catalog.len() {
            Some(self.catalog.path(ix))
        } else if ix == NO_ENTRY {
            self.standalone_source.as_ref().map(|(p, ..)| p.clone())
        } else {
            None
        };
        path.filter(|p| !p.as_os_str().is_empty())
            .map_or_else(|| self.entry_label(ix), |p| p.display().to_string())
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
        if ix == NO_ENTRY {
            let id = self.standalone_group_id(&path);
            shell::groups_panel::restore_standalone_lock(&mut self.frozen, &self.group_state, &id);
            self.standalone_source = Some((path.clone(), label.clone(), id));
            self.cache.put((ix, fingerprint), sp.clone());
        }
        self.status = spectrum_status(&label, &sp);
        if ix >= DERIVED_BASE
            && let Some(reason) = self
                .derived
                .get(ix - DERIVED_BASE)
                .and_then(DerivedSpectrum::processing_block_reason)
        {
            self.status = reason.into();
        }
        self.spectrum_group = self.tool_target(ix).map(|mut identity| {
            identity.fingerprint = fingerprint;
            identity.label = label.to_string();
            identity.path = path.clone();
            identity
        });
        self.spectrum_quantity = ix
            .checked_sub(DERIVED_BASE)
            .and_then(|i| self.derived.get(i))
            .map(|g| g.quantity)
            .unwrap_or_default();
        self.spectrum_label = label;
        self.spectrum_path = path;
        self.spectrum_fingerprint = fingerprint;
        // Plots are about to match the selection again.
        self.stale_plots = None;
        // Surface the auto-determined E0 in the field placeholder.
        if self.effective_params(ix).e0.is_none()
            && let Some(e0) = sp.e0()
            && let Some((_, field)) = self
                .param_fields
                .iter()
                .find(|(key, _)| *key == ParamKey::E0)
        {
            field.update(cx, |f, cx| f.set_placeholder(format!("auto ({e0:.1})"), cx));
        }
        if self.effective_params(ix).mback.is_some() {
            if let Some(ranges) = crate::params::normalization_ranges(&sp) {
                for (key, resolved) in [
                    ParamKey::PreEdgeStart,
                    ParamKey::PreEdgeEnd,
                    ParamKey::NormStart,
                    ParamKey::NormEnd,
                ]
                .into_iter()
                .zip(ranges)
                {
                    if let Some((_, field)) = self.param_fields.iter().find(|(k, _)| *k == key) {
                        field.update(cx, |f, cx| {
                            f.set_placeholder(format!("auto ({resolved:.1})"), cx)
                        });
                    }
                }
            }
        } else {
            for (key, value) in [
                (ParamKey::PreEdgeStart, "auto (-200)"),
                (ParamKey::PreEdgeEnd, "auto (-30)"),
                (ParamKey::NormStart, "auto (150)"),
                (ParamKey::NormEnd, "auto (spectrum end)"),
                (ParamKey::NormPolyorder, "auto (2)"),
            ] {
                if let Some((_, field)) = self.param_fields.iter().find(|(k, _)| *k == key) {
                    field.update(cx, |f, cx| f.set_placeholder(value, cx));
                }
            }
        }
        self.show_range_adjustments(ix, &sp, cx);
        self.spectrum = Some(sp.clone());
        self.settle_restored_fit();
        self.refresh_operando_frame_plot(ix, fingerprint, &sp, cx);
        self.invalidate_explore_plots(cx);
        // Scripted launches (screenshots): REXAFS_DEBUG_FIT_PATHS="a.dat:b.dat"
        // adds FEFF paths and fits the first loaded spectrum.
        if self.fit_paths.is_empty()
            && let Ok(list) = crate::settings::env_var("DEBUG_FIT_PATHS")
        {
            for file in std::env::split_paths(&list).filter(|f| !f.as_os_str().is_empty()) {
                self.push_fit_path(file, cx);
            }
            self.fit_model_changed(cx);
            self.run_fit_now(cx);
        }
    }

    // ---- operando ----------------------------------------------------------

    /// (Re)build the downsampled scan overview if the scan or parameters
    /// changed. Frames are processed in parallel on rayon.
    fn ensure_operando(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.overview_source() else {
            return;
        };
        let scan_len = source.len();
        let fingerprint = self.overview_fingerprint(&source);
        if source.samples(&self.group_registry, 1).is_empty() {
            retire_job(
                &mut self.operando_gen,
                &mut self.operando_running,
                Some(&mut self.operando_cancel),
            );
            self.status =
                "No available groups in this series. Select another series or restore its inputs."
                    .into();
            self.operando = None;
            self.operando_plots = None;
            return;
        }
        if self
            .operando
            .as_ref()
            .is_some_and(|o| o.source == source && o.fingerprint == fingerprint)
        {
            return;
        }
        let preserve_time_pos = self.operando.as_ref().is_some_and(|o| o.source == source);

        if let Some(cancel) = self.operando_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.operando_gen += 1;
        let generation = self.operando_gen;
        let cancel = Arc::new(AtomicBool::new(false));
        self.operando_cancel = Some(cancel.clone());
        self.operando_running = true;
        // Even sampling across the scan; first and last frames included.
        let sample_frames = source.samples(&self.group_registry, MAX_FRAMES);
        let sample_ixs: Vec<_> = sample_frames
            .iter()
            .filter_map(|&p| source.entry(p))
            .collect();
        self.job_inputs[0] = sample_ixs
            .iter()
            .filter_map(|&ix| self.group_id(ix))
            .collect();
        let frames: Vec<_> = sample_ixs
            .iter()
            .map(|&ix| {
                let derived = ix
                    .checked_sub(DERIVED_BASE)
                    .and_then(|i| self.derived.get(i))
                    .cloned();
                let path = if ix < self.catalog.len() {
                    self.catalog.path(ix)
                } else {
                    PathBuf::new()
                };
                (
                    ix,
                    path,
                    self.entry_label(ix),
                    derived,
                    self.effective_params(ix).clone(),
                )
            })
            .collect();
        let grid: Vec<f64> = (0..K_GRID_BINS)
            .map(|i| i as f64 * K_GRID_MAX / (K_GRID_BINS - 1) as f64)
            .collect();
        self.status = format!(
            "building scan overview ({} of {} frames) ...",
            frames.len(),
            scan_len
        )
        .into();

        let mut diagnostics: BTreeMap<_, _> = frames
            .iter()
            .filter_map(|(ix, _, _, _, _)| {
                self.peek_group_id(*ix).map(|id| {
                    (
                        *ix,
                        self.group_diagnostics
                            .begin(id, self.effective_fingerprint(*ix)),
                    )
                })
            })
            .collect();
        let job_grid = grid.clone();
        let job_cancel = cancel.clone();
        let job = cx.background_executor().spawn(async move {
            frames
                .par_iter()
                .map(|(ix, path, label, derived, params)| {
                    if job_cancel.load(Ordering::Relaxed) {
                        return None;
                    }
                    let result =
                        series_source::prepare_overview_frame(path, derived.as_ref(), params)
                            .and_then(|sp| frame_sample(&sp, &job_grid));
                    Some((*ix, label.clone(), result))
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
                let samples: Vec<Result<FrameSample, String>> = rows
                    .into_iter()
                    .flatten()
                    .map(|(ix, label, result)| {
                        if let Some(ticket) = diagnostics.remove(&ix) {
                            let problems = result
                                .as_ref()
                                .err()
                                .map(|error| {
                                    vec![JobError {
                                        batch: None,
                                        severity: ProblemSeverity::Error,
                                        label: label.clone(),
                                        message: error.to_string(),
                                    }]
                                })
                                .unwrap_or_default();
                            app.group_diagnostics.finish(&ticket, problems);
                        }
                        result.inspect_err(|error| {
                            app.record_job_error(label, error.clone());
                        })
                    })
                    .collect();
                let failed = samples.iter().filter(|s| s.is_err()).count();
                // Shared energy / R grids come from the first good frame.
                let first = samples.iter().find_map(|s| s.as_ref().ok());
                let e_grid: Vec<f64> = first
                    .map(|f| {
                        let lo = f.e0 - 40.0;
                        let hi = f
                            .energy
                            .last()
                            .copied()
                            .unwrap_or(f.e0 + 200.0)
                            .min(f.e0 + 200.0);
                        (0..K_GRID_BINS)
                            .map(|i| lo + (hi - lo) * i as f64 / (K_GRID_BINS - 1) as f64)
                            .collect()
                    })
                    .unwrap_or_default();
                let r_grid: Vec<f64> = (0..K_GRID_BINS)
                    .map(|i| 6.0 * i as f64 / (K_GRID_BINS - 1) as f64)
                    .collect();
                let mut support = Vec::with_capacity(samples.len());
                let mut matrix = Vec::with_capacity(samples.len());
                let mut e_matrix = Vec::with_capacity(samples.len());
                let mut flat_matrix = Vec::with_capacity(samples.len());
                let mut r_matrix = Vec::with_capacity(samples.len());
                let mut e0s = Vec::with_capacity(samples.len());
                let mut whitelines = Vec::with_capacity(samples.len());
                for sample in samples {
                    match sample {
                        Ok(f) => {
                            support.push(f.support);
                            matrix.push(f.k_row);
                            e_matrix.push(resample_xy(&f.energy, &f.norm, &e_grid));
                            flat_matrix.push(resample_xy(&f.energy, &f.flat, &e_grid));
                            r_matrix.push(resample_xy(&f.r, &f.mag, &r_grid));
                            e0s.push(f.e0);
                            whitelines.push(f.whiteline);
                        }
                        Err(_) => {
                            support.push(Default::default());
                            matrix.push(vec![f64::NAN; grid.len()]);
                            e_matrix.push(vec![f64::NAN; e_grid.len()]);
                            flat_matrix.push(vec![f64::NAN; e_grid.len()]);
                            r_matrix.push(vec![f64::NAN; r_grid.len()]);
                            e0s.push(f64::NAN);
                            whitelines.push(f64::NAN);
                        }
                    }
                }
                let kweight = app.params.fft_kweight.unwrap_or(2.0);
                app.operando = Some(OperandoData {
                    source: source.clone(),
                    scan_len,
                    sample_frames,
                    support,
                    fingerprint,
                    grid,
                    matrix,
                    e_grid,
                    e_matrix,
                    flat_matrix,
                    r_grid,
                    r_matrix,
                    e0s,
                    whitelines,
                    kweight,
                });
                if app.series_display.difference && app.active_series_reference().is_none() {
                    // Membership, project or processing changes invalidate the reference.
                    app.series_display.request += 1;
                    app.series_display.difference = false;
                    app.series_display.reference = None;
                    app.series_display.pending_frame = None;
                }
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
                app.time_pos = source
                    .surviving(&app.group_registry, app.time_pos, false)
                    .unwrap_or(0);
                app.rebuild_operando_plots(cx);
                app.status = if failed == 0 {
                    "scan overview ready".into()
                } else {
                    format!("scan overview ready · {failed} failed").into()
                };
                app.sync_time_selection(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn series_tick_count(&self, key: usize) -> usize {
        (self.card_px.get(&key).map_or(700, |v| v.0) as usize / 110).clamp(3, 7)
    }

    fn rebuild_operando_plots(&mut self, cx: &mut Context<Self>) {
        let Some(data) = self.operando.as_ref() else {
            return;
        };
        let source = data.source.clone();
        let Some((chik_key, row)) = self.series_frame_source() else {
            return;
        };
        let chik_series = RetainedChikSeries::new(chik_key, row);
        let space = self.stage_view.series_space;
        let (grid, matrix) = data.space(space);
        let (xlabel, mut ylabel): (String, String) = match space {
            SeriesSpace::Energy => ("Energy (eV)".into(), "normalized μ(E)".into()),
            SeriesSpace::Flat => ("Energy (eV)".into(), "flattened μ(E)".into()),
            SeriesSpace::K => (K_AXIS.into(), chik_label(self.series_display_kweight())),
            SeriesSpace::R => (R_AXIS.into(), chir_label(self.series_display_kweight())),
        };
        if self.series_display.difference {
            ylabel = format!("Δ {ylabel}");
        }
        let displayed = self.series_difference_matrix(matrix);
        let heatmap_rows =
            source.heatmap_rows(&displayed, &data.sample_frames, &self.group_registry);
        let heatmap = build_heatmap(
            &heatmap_rows,
            grid,
            data.scan_len,
            &xlabel,
            &self.theme,
            &self.series_display,
        )
        .size_px(
            self.card_px.get(&301).map_or(700, |v| v.0),
            self.card_px.get(&301).map_or(900, |v| v.1),
        )
        .major_ticks_x(self.series_tick_count(301));
        let mut chik = match space {
            SeriesSpace::K => build_frame_chik_source(
                &data.grid,
                chik_series.values.clone(),
                self.series_display_kweight(),
                &self.theme,
            ),
            _ => crate::plotting::build_frame_row_source(
                grid,
                chik_series.values.clone(),
                &xlabel,
                &ylabel,
                &self.theme,
            ),
        }
        .size_px(
            self.card_px.get(&302).map_or(700, |v| v.0),
            self.card_px.get(&302).map_or(420, |v| v.1),
        )
        .major_ticks_x(self.series_tick_count(302));
        if self.series_display.difference {
            chik = crate::plotting::centered_y(chik, chik_series.values.read().iter().copied())
                .ylabel(&ylabel);
        }
        let trend_snapshot = self.trend_snapshot();
        let trend = build_trend(
            &trend_snapshot.values,
            &trend_snapshot.frames,
            &trend_snapshot.name,
            self.series_tick_count(303),
            &self.theme,
        )
        .size_px(
            self.card_px.get(&303).map_or(700, |v| v.0),
            self.card_px.get(&303).map_or(420, |v| v.1),
        );
        match &mut self.operando_plots {
            Some(plots) => {
                if plots.source == source
                    && plots.space == space
                    && plots.difference == self.series_display.difference
                {
                    plots
                        .heatmap
                        .update(cx, |rp, cx| rp.set_plot_keep_view(heatmap, cx));
                    plots
                        .chik
                        .update(cx, |rp, cx| rp.set_plot_keep_view(chik, cx));
                    if plots.trend_domain == trend_snapshot.domain {
                        plots
                            .trend
                            .update(cx, |rp, cx| rp.set_plot_keep_view(trend, cx));
                    } else {
                        plots.trend.update(cx, |rp, cx| rp.set_plot(trend, cx));
                    }
                } else {
                    // New scan or space: fresh viewports, the old zoom is meaningless.
                    plots.source = source.clone();
                    plots.space = space;
                    plots.heatmap.update(cx, |rp, cx| rp.set_plot(heatmap, cx));
                    plots.chik.update(cx, |rp, cx| rp.set_plot(chik, cx));
                    plots.trend.update(cx, |rp, cx| rp.set_plot(trend, cx));
                }
                plots.difference = self.series_display.difference;
                plots.chik_series = chik_series;
                plots.trend_domain = trend_snapshot.domain;
                plots.heatmap_cursor = None;
                plots.trend_cursor = None;
                plots.last_trend_cursor = None;
            }
            None => {
                let heatmap_interaction = InteractionOptions {
                    hover: false,
                    selection: false,
                    tooltips: false,
                    ..InteractionOptions::default()
                };
                let heatmap = plot_builder(heatmap)
                    .interaction_options(heatmap_interaction)
                    .build(cx);
                let chik = plot_builder(chik).interactive().build(cx);
                let trend = plot_builder(trend).interactive().build(cx);
                for plot in [&heatmap, &trend] {
                    cx.subscribe(plot, |app, _, _: &crate::plot_ranges::RangesChanged, cx| {
                        if let Some(plots) = &mut app.operando_plots {
                            plots.last_trend_cursor = None;
                        }
                        app.update_operando_cursor_annotations(cx);
                    })
                    .detach();
                }
                let subscription = cx.subscribe(
                    &heatmap,
                    |this: &mut Self, _, event: &PlotPointerEvent, cx| {
                        if event.kind != PlotPointerEventKind::Click
                            || event.mouse_button != Some(gpui::MouseButton::Left)
                        {
                            return;
                        }
                        let Some(position) = event.data_position else {
                            return;
                        };
                        let Some(data) = this.operando.as_ref() else {
                            return;
                        };
                        let (grid, matrix) = data.space(this.stage_view.series_space);
                        let Some(frame) = frame_from_heatmap_position(
                            position.x,
                            position.y,
                            grid,
                            data.scan_len,
                            matrix.len(),
                        ) else {
                            return;
                        };
                        this.set_time_pos(frame, cx);
                    },
                );
                self.operando_plots = Some(OperandoPlots {
                    source,
                    space,
                    difference: self.series_display.difference,
                    heatmap,
                    chik,
                    chik_series,
                    trend,
                    trend_domain: trend_snapshot.domain,
                    heatmap_cursor: None,
                    trend_cursor: None,
                    last_trend_cursor: None,
                    _heatmap_subscription: subscription,
                });
            }
        }
        self.update_operando_cursor_annotations(cx);
    }

    fn rebuild_operando_trend(&mut self, cx: &mut Context<Self>) {
        if self.operando_plots.is_none() {
            return;
        }
        let snapshot = self.trend_snapshot();
        let trend = build_trend(
            &snapshot.values,
            &snapshot.frames,
            &snapshot.name,
            self.series_tick_count(303),
            &self.theme,
        )
        .size_px(
            self.card_px.get(&303).map_or(700, |v| v.0),
            self.card_px.get(&303).map_or(420, |v| v.1),
        );
        let plots = self
            .operando_plots
            .as_mut()
            .expect("operando plots checked above");

        if plots.trend_domain == snapshot.domain {
            plots
                .trend
                .update(cx, |rp, cx| rp.set_plot_keep_view(trend, cx));
        } else {
            plots.trend.update(cx, |rp, cx| rp.set_plot(trend, cx));
        }
        plots.trend_domain = snapshot.domain;
        plots.trend_cursor = None;
        plots.last_trend_cursor = None;
        self.update_operando_cursor_annotations(cx);
    }

    pub(crate) fn fit_model_changed(&mut self, cx: &mut Context<Self>) {
        self.refresh_structure(cx);
        if self.batch_fit.is_some() && self.operando_plots.is_some() {
            self.rebuild_operando_trend(cx);
        }
        cx.notify();
    }

    fn update_operando_cursor_annotations(&mut self, cx: &mut Context<Self>) {
        let heatmap_annotation = heatmap_cursor_annotation(self.time_pos, &self.theme);
        let mut errors = Vec::new();
        if let Some(plots) = &mut self.operando_plots {
            if let Err(error) = set_cursor_annotation(
                &plots.heatmap,
                &mut plots.heatmap_cursor,
                heatmap_annotation,
                cx,
            ) {
                errors.push(("heatmap cursor", error));
            }

            let trend_cursor = self.time_pos;
            if plots.last_trend_cursor != Some(trend_cursor) {
                let trend_annotation = trend_cursor_annotation(trend_cursor, &self.theme);
                match set_cursor_annotation(
                    &plots.trend,
                    &mut plots.trend_cursor,
                    trend_annotation,
                    cx,
                ) {
                    Ok(()) => plots.last_trend_cursor = Some(trend_cursor),
                    Err(error) => errors.push(("trend cursor", error)),
                }
            }
        }
        for (label, error) in errors {
            self.record_job_error(label, error.to_string());
        }
    }

    fn step_time(&mut self, delta: isize, cx: &mut Context<Self>) {
        let next = self.time_pos as isize + delta;
        self.set_time_pos(next.max(0) as usize, cx);
    }

    fn operando_scan_len(&self) -> Option<usize> {
        self.operando.as_ref().map(|data| data.scan_len)
    }

    fn step_time_percent(&mut self, direction: isize, cx: &mut Context<Self>) {
        let Some(len) = self.operando_scan_len() else {
            return;
        };
        let step = len.div_ceil(100).max(1) as isize;
        self.step_time(direction * step, cx);
    }

    fn set_time_pos(&mut self, pos: usize, cx: &mut Context<Self>) {
        let Some(ix) = self.update_operando_cursor(pos, cx) else {
            return;
        };
        if self.selected != Some(ix) {
            self.sync_time_selection(cx);
        }
        cx.notify();
    }

    /// Update only the operando cursor and its cursor-dependent plots. The
    /// caller decides whether selection also needs changing, preventing the
    /// two synchronization directions from recursively selecting each other.
    fn update_operando_cursor(&mut self, pos: usize, cx: &mut Context<Self>) -> Option<usize> {
        let data = self.operando.as_ref()?;
        let pos = data
            .source
            .surviving(&self.group_registry, pos, pos < self.time_pos)?;
        let ix = data.source.entry(pos)?;
        let len = data.scan_len;
        if pos == self.time_pos {
            return Some(ix);
        }
        self.time_pos = pos;
        if let Some((key, row)) = self.series_frame_source() {
            self.replace_operando_chik(key, row, cx);
        }
        self.update_operando_cursor_annotations(cx);
        self.status = format!("frame {}/{} · {}", pos + 1, len, self.entry_label(ix)).into();
        Some(ix)
    }

    /// Keep group selection and the overview cursor synchronized in either direction.
    fn sync_operando_cursor_to_entry(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(offset) = self.operando.as_ref().and_then(|d| d.source.position(ix)) else {
            return;
        };
        if offset != self.time_pos {
            self.update_operando_cursor(offset, cx);
        }
    }

    /// Load the exact cursor frame through the shared generation-checked cache.
    fn sync_time_selection(&mut self, cx: &mut Context<Self>) {
        let Some(data) = &self.operando else {
            return;
        };
        let Some(offset) = data
            .source
            .surviving(&self.group_registry, self.time_pos, false)
        else {
            return;
        };
        let Some(ix) = data.source.entry(offset) else {
            return;
        };
        self.time_pos = offset;
        self.reveal_time_selection(ix);
        self.select_entry(ix, cx);
    }

    fn reveal_time_selection(&mut self, ix: usize) {
        if self.group_rows().row_index(ix).is_none() {
            self.reveal_current = Some(ix);
        }
        self.reveal_group_row(ix);
    }

    fn replace_operando_chik(
        &mut self,
        key: ChikRowKey,
        row: Vec<f64>,
        cx: &mut Context<Self>,
    ) -> bool {
        let ticks = self.series_tick_count(302);
        let kweight = self.series_display_kweight();
        let Some(plots) = self.operando_plots.as_mut() else {
            return false;
        };
        let old_limits =
            crate::plotting::symmetric_y_limits(plots.chik_series.values.read().iter().copied());
        let new_limits = crate::plotting::symmetric_y_limits(row.iter().copied());
        if !plots.chik_series.replace_if_changed(key, row) {
            return false;
        }
        if (plots.space == SeriesSpace::K || plots.difference)
            && old_limits != new_limits
            && let Some(data) = &self.operando
        {
            let grid = data.space(plots.space).0;
            let (xlabel, ylabel) = match plots.space {
                SeriesSpace::Energy => ("Energy (eV)", "normalized μ(E)".to_string()),
                SeriesSpace::Flat => ("Energy (eV)", "flattened μ(E)".to_string()),
                SeriesSpace::K => (K_AXIS, chik_label(kweight)),
                SeriesSpace::R => (R_AXIS, chir_label(kweight)),
            };
            let label = if plots.difference {
                format!("Δ {ylabel}")
            } else {
                ylabel
            };
            let plot = crate::plotting::build_frame_row_source(
                grid,
                plots.chik_series.values.clone(),
                xlabel,
                &label,
                &self.theme,
            );
            let plot =
                crate::plotting::centered_y(plot, plots.chik_series.values.read().iter().copied())
                    .size_px(
                        self.card_px.get(&302).map_or(700, |v| v.0),
                        self.card_px.get(&302).map_or(420, |v| v.1),
                    )
                    .major_ticks_x(ticks);
            plots
                .chik
                .update(cx, |plot_view, cx| plot_view.set_plot_keep_view(plot, cx));
        }
        true
    }

    /// Replace the sampled placeholder with the actual processed cursor
    /// frame when its generation-counted load completes.
    fn refresh_operando_frame_plot(
        &mut self,
        ix: usize,
        fingerprint: u64,
        sp: &XASSpectrum,
        cx: &mut Context<Self>,
    ) {
        let Some(data) = &self.operando else {
            return;
        };
        if data.fingerprint != self.overview_fingerprint(&data.source)
            || data.source.entry(self.time_pos) != Some(ix)
        {
            return;
        }
        let key = ChikRowKey::Exact {
            entry: ix,
            params_fingerprint: fingerprint,
        };
        if self
            .operando_plots
            .as_ref()
            .is_some_and(|plots| plots.chik_series.key == key)
        {
            return;
        }
        let Some(row) = resample_series_frame(
            sp,
            self.stage_view.series_space,
            data.space(self.stage_view.series_space).0,
        ) else {
            return;
        };
        let row = self.series_difference_row(row, series_display::SeriesSupport::from_spectrum(sp));
        self.replace_operando_chik(key, row, cx);
    }

    fn current_group_index(&self) -> Option<usize> {
        self.selected.or_else(|| {
            self.standalone_path()
                .filter(|p| *p == self.current_path)
                .map(|_| NO_ENTRY)
        })
    }

    fn compare_count(&self) -> usize {
        self.selection.len()
            + usize::from(
                self.current_group_index()
                    .is_some_and(|g| !self.selection.contains(&g)),
            )
    }

    fn compare_groups(&self) -> BTreeSet<usize> {
        group_rows::compare_set(self.current_group_index(), &self.selection)
    }

    /// Every marked group plus the active group, without trace sampling.
    fn compare_indices(&self) -> (Vec<usize>, usize) {
        let all: Vec<usize> = self.compare_groups().into_iter().collect();
        let total = all.len();
        if self.view.sample_overlay && total > 12 {
            let mut preview: Vec<_> = (0..12).map(|i| all[i * (total - 1) / 11]).collect();
            if let Some(current) = self.current_group_index()
                && !preview.contains(&current)
            {
                preview[6] = current;
                preview.sort_unstable();
            }
            (preview, total)
        } else {
            (all, total)
        }
    }

    /// Process any compare-set members missing from the cache (rayon batch),
    /// then rebuild the overlay. Every member is processed and cached under
    /// its own effective params.
    fn ensure_compare_loaded(&mut self, cx: &mut Context<Self>) {
        let (indices, _) = self.compare_indices();
        let mut missing = missing_compare_loads(
            &self.catalog,
            &self.derived,
            &self.params,
            &self.overrides,
            &indices,
            &self.cache,
        );
        if indices.contains(&NO_ENTRY)
            && let Some(path) = self.standalone_path()
        {
            let fingerprint = self.params.fingerprint();
            if !self.cache.contains(&(NO_ENTRY, fingerprint)) {
                missing.push(CompareLoad {
                    ix: NO_ENTRY,
                    fingerprint,
                    source: Ok(path.to_path_buf()),
                    params: self.params.clone(),
                });
            }
        }
        exclude_pending_current(
            &mut missing,
            self.load_running
                .then(|| self.current_group_index())
                .flatten(),
        );
        if missing.is_empty() {
            self.invalidate_explore_plots(cx);
            cx.notify();
            return;
        }
        self.compare_gen += 1;
        let generation = self.compare_gen;
        self.compare_running = true;
        self.status = format!("processing {} spectra for overlay ...", missing.len()).into();
        cx.notify();
        let mut diagnostics: BTreeMap<_, _> = missing
            .iter()
            .filter_map(|load| {
                self.group_id(load.ix)
                    .map(|id| (load.ix, self.group_diagnostics.begin(id, load.fingerprint)))
            })
            .collect();
        let origins: BTreeMap<_, _> = missing
            .iter()
            .filter_map(|load| {
                let path = load.source.as_ref().ok()?;
                Some((load.ix, (path.clone(), self.intake_origin(load.ix, path))))
            })
            .collect();
        let job = cx.background_executor().spawn(async move {
            missing
                .par_iter()
                .map(CompareLoad::process)
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
                    if let Some(ticket) = diagnostics.remove(&ix) {
                        let problems = match &result {
                            Ok((_, Some(raw))) => raw
                                .diagnostics
                                .warnings()
                                .into_iter()
                                .map(|message| JobError {
                                    batch: None,
                                    severity: ProblemSeverity::Warning,
                                    label: app.entry_label(ix),
                                    message,
                                })
                                .collect(),
                            Ok((_, None)) => Vec::new(),
                            Err(error) => vec![JobError {
                                batch: None,
                                severity: ProblemSeverity::Error,
                                label: app.entry_label(ix),
                                message: error.to_string(),
                            }],
                        };
                        app.group_diagnostics.finish(&ticket, problems);
                    }
                    match result {
                        Ok((sp, raw)) => {
                            if let Some(raw) = raw {
                                if let Some((path, origin)) = origins.get(&ix) {
                                    app.record_source_warnings(
                                        ix,
                                        origin.as_ref(),
                                        path,
                                        &raw.diagnostics,
                                        raw.declared_edge.clone(),
                                        raw.channel,
                                    );
                                }
                                app.raw_cache
                                    .put((ix, app.effective_params(ix).raw_fingerprint()), raw);
                            }
                            app.cache.put((ix, fingerprint), Arc::new(sp));
                        }
                        Err(error) => {
                            failed += 1;
                            app.record_source_error(
                                origins.get(&ix).and_then(|(_, origin)| origin.as_ref()),
                                app.entry_label(ix),
                                error.to_string(),
                            );
                        }
                    }
                }
                let (_, total) = app.compare_indices();
                app.status = if failed > 0 {
                    format!("overlay ready · {failed} failed").into()
                } else {
                    format!("overlay of {total} spectra ready").into()
                };
                app.invalidate_explore_plots(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn invalidate_explore_plots(&mut self, cx: &mut Context<Self>) {
        if should_rebuild_explore(self.workspace, &mut self.explore_plots_dirty, true) {
            self.rebuild_explore_plots(cx);
        }
    }

    /// Whether the selected group has neither a processed result for its
    /// current settings nor a running job, while the plots would silently
    /// fall back to another group's spectrum. A failed job is excluded: it
    /// shows the stale-plot banner until the settings change.
    fn current_needs_reload(&self) -> bool {
        let Some(ix) = self.current_group_index() else {
            return false;
        };
        let loadable = if ix == NO_ENTRY {
            self.standalone_path().is_some()
        } else {
            self.valid_group_index(ix)
        };
        // Any earlier result of the same group counts: setting edits schedule
        // their own recompute, and a held Live average view must stay held.
        let shown = self.spectrum.is_some()
            && self
                .spectrum_group
                .as_ref()
                .and_then(|t| t.group_id.as_ref())
                == self.group_id(ix).as_ref();
        current_reload_needed(ReloadState {
            busy: self.load_running || self.recompute_dirty,
            failed: self.stale_plots.is_some(),
            loadable,
            cached: self.cache.contains(&(ix, self.effective_fingerprint(ix))),
            shown,
        })
    }

    fn rebuild_explore_plots(&mut self, cx: &mut Context<Self>) {
        // A cancelled load (for example after group indices changed) leaves
        // no result and no error. Process the selection again rather than
        // drawing the previous group's curves under the selected group's name.
        // The job starts before this rebuild continues, so it runs once.
        if self.current_needs_reload() {
            self.reprocess_current(cx);
        }
        self.mixed_overlay_weight = None;
        // Stage-dependent view flags (also correct on a scripted first launch).
        self.view.flat = self.stage_view.e_quantity == shell::EQuantity::Flat;
        self.view.show_re = if self.stage == Stage::Fit {
            self.stage_view.fit_show_re
        } else {
            self.stage_view.show_re
        };
        self.view.show_im = self.stage == Stage::Fit && self.stage_view.fit_show_im;
        self.view.show_bkg = self.stage_view.show_bkg && self.stage == Stage::Process;
        let (indices, total) = self.compare_indices();
        let mut traces: Vec<QuadTrace> = Vec::new();

        let current = self.current_group_index();
        let current_only =
            self.stage == Stage::Fit || self.stage_view.scope == shell::PlotScope::Current;
        let sampled = if current_only {
            usize::from(current.is_some())
        } else {
            indices.len()
        };
        let total = if current_only { sampled } else { total };
        self.plot_coverage = [crate::plotting::PlotCoverage::new(total, sampled, 0, 0); 5];
        for ix in indices
            .into_iter()
            .filter(|&ix| !current_only || Some(ix) == current)
        {
            let label = self.entry_label(ix);
            let fingerprint = self.effective_fingerprint(ix);
            let color_index = self.group_color_index(ix);
            let color = self.group_plot_color(ix);
            if let Some(sp) = self.cache.get(&(ix, fingerprint)) {
                traces.push(QuadTrace {
                    color_index,
                    color,
                    label,
                    sp: sp.clone(),
                    active: Some(ix) == current,
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
                color: self
                    .spectrum_group
                    .as_ref()
                    .and_then(|t| t.group_id.as_ref())
                    .and_then(|id| self.group_state.plot_colors.get(id))
                    .map(|c| c.color(&self.theme)),
                color_index: self
                    .spectrum_group
                    .as_ref()
                    .and_then(|t| t.group_id.as_ref())
                    .map(|id| {
                        self.group_state
                            .colors
                            .get(id)
                            .map(|c| usize::from(*c % 8))
                            .unwrap_or_else(|| group_rows::color_index(id))
                    })
                    .unwrap_or(0),
                label: self
                    .spectrum_group
                    .as_ref()
                    .and_then(|t| t.group_id.as_ref())
                    .and_then(|id| self.group_state.labels.get(id))
                    .cloned()
                    .unwrap_or_else(|| self.spectrum_label.to_string()),
                sp: sp.clone(),
                active: true,
            });
        }
        if (self.stage == Stage::Fit || self.stage_view.scope == shell::PlotScope::Current)
            && traces.iter().any(|t| t.active)
        {
            traces.retain(|t| t.active);
        }
        let colors =
            group_rows::overlay_colors(&traces.iter().map(|t| t.color_index).collect::<Vec<_>>());
        for (trace, color) in traces.iter_mut().zip(colors) {
            trace.color_index = color;
        }
        if self.view.gradient && traces.len() > 1 {
            let map = ruviz::render::ColorMap::viridis();
            let count = traces.len();
            for (i, trace) in traces.iter_mut().enumerate() {
                trace.color = Some(map.sample(i as f64 / (count - 1) as f64));
            }
        }
        // The grid uses one shared legend strip; per-plot legends only when a
        // quadrant is maximized.
        self.mixed_overlay_weight = crate::plotting::mixed_kweights(&traces).then(|| {
            traces
                .iter()
                .find(|t| t.active)
                .or_else(|| traces.first())
                .and_then(|t| t.sp.kweight())
                .copied()
                .unwrap_or(2.0)
        });
        self.legend_entries = if traces.len() > 1 {
            traces
                .iter()
                .map(|trace| {
                    (
                        SharedString::from(middle_truncate(
                            &if self.mixed_overlay_weight.is_some() {
                                crate::plotting::ft_trace_label(trace)
                            } else {
                                trace.label.clone()
                            },
                            36,
                        )),
                        trace
                            .color
                            .map(crate::spectrum_colors::rgba)
                            .unwrap_or_else(|| trace_rgba(&self.theme, trace.color_index)),
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        let in_plot_legend = self.maximized.is_some();
        let mut specs = crate::plotting::quantity_quadrant_specs(
            &traces,
            &self.stage.plot_options(self.view),
            &self.theme,
            in_plot_legend,
            self.processing_plot_quantity(),
        );
        if let Some((lo, hi)) = self.view.energy_view_range {
            if let Some(e0) = self.spectrum.as_ref().and_then(|sp| sp.e0()) {
                for spec in specs.iter_mut().take(2) {
                    spec.xlim = Some((e0 + lo, e0 + hi));
                }
            }
        }
        if self.stage == Stage::Data
            && !self.handles.hidden
            && let Some((lo, hi)) = self.analysis_energy_interval(cx)
        {
            for spec in specs.iter_mut().take(2) {
                spec.vlines
                    .push((lo, ruviz::render::Color::from_gray(175), 1.3, true));
                spec.vlines
                    .push((hi, ruviz::render::Color::from_gray(175), 1.3, true));
            }
        }
        self.plot_coverage =
            std::array::from_fn(|i| specs[i].coverage(total, sampled, traces.len()));
        if self.stage == Stage::Process
            && let Some((_, hi)) = self
                .spectrum
                .as_ref()
                .and_then(|sp| shell::handles::background_k_domain(sp))
        {
            // Keep the measured k range visible while AUTOBK truncates its
            // output. Otherwise every recompute auto-zooms to the new limit
            // and makes an outward drag chase a moving axis.
            specs[shell::center::PLOT_CHIK].xlim = Some((-0.04 * hi, 1.04 * hi));
        }
        // The figure aspect follows the card layout (ruviz-gpui fits the
        // figure aspect inside the container). Stage handles (range regions,
        // E0 / Rbkg lines) are painted by GPUI; see shell::handles.
        let analysis_card = self.stage == Stage::Data && self.analysis.plot.is_some();
        let stacked = self.stage_plots().len() > 1 || analysis_card;
        let fallback = if stacked { (820, 280) } else { (820, 580) };
        let dark = self.theme.mode == crate::theme::ThemeMode::Dark;
        self.quad_export_labels = specs
            .iter()
            .map(|spec| shell::plot_export::Labels {
                template: spec.export_template(),
                x: spec.xlabel.clone(),
                y: spec.ylabel.clone(),
                series: spec
                    .series
                    .iter()
                    .map(|series| match series.key {
                        crate::plotting::SeriesKey::Trace(i) => traces
                            .get(i)
                            .map(|t| {
                                if spec.xlabel.starts_with("R ") || spec.xlabel.starts_with("q ") {
                                    crate::plotting::ft_trace_label(t)
                                } else {
                                    t.label.clone()
                                }
                            })
                            .unwrap_or_else(|| format!("Spectrum {}", i + 1)),
                        _ => series
                            .label
                            .clone()
                            .unwrap_or_else(|| format!("{:?}", series.key)),
                    })
                    .collect(),
            })
            .collect();
        for (index, spec) in specs.into_iter().enumerate() {
            let (fig_w, fig_h) = self.card_px.get(&index).copied().unwrap_or(fallback);
            let salt = ((fig_w as u64) << 33) ^ ((fig_h as u64) << 1) ^ (dark as u64);
            let key = spec.structure_key(salt);
            let title = SharedString::from(spec.title.clone());
            match (self.quadrants.get_mut(index), self.quad_bindings.get(index)) {
                (Some((slot, _)), Some((bound, sources))) if *bound == key => {
                    // Same structure: replace the values behind the session.
                    *slot = title;
                    spec.apply(sources);
                }
                (Some((slot, entity)), _) => {
                    let (plot, sources) = spec.to_plot(&self.theme);
                    let plot = plot.size_px(fig_w, fig_h);
                    *slot = title;
                    entity.update(cx, |rp, cx| rp.set_plot_keep_view(plot, cx));
                    if let Some(binding) = self.quad_bindings.get_mut(index) {
                        *binding = (key, sources);
                    } else {
                        self.quad_bindings.push((key, sources));
                    }
                }
                (None, _) => {
                    let (plot, sources) = spec.to_plot(&self.theme);
                    let plot = plot.size_px(fig_w, fig_h);
                    let entity = plot_builder(plot).interactive().build(cx);
                    self.quadrants.push((title, entity));
                    self.quad_bindings.truncate(index);
                    self.quad_bindings.push((key, sources));
                }
            }
        }
        // Ripple strip: the current group only.
        if let Some(active) = traces.iter().find(|t| t.active).or_else(|| traces.first()) {
            let fit_r = (self.fit_ranges.rmin, self.fit_ranges.rmax);
            self.thumbs = Some(Arc::new(shell::thumbnails::thumb_data(&active.sp, fit_r)));
        }
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = self.theme.toggled();
        let theme = self.theme;
        self.structure.settings.theme = Some(theme.mode);
        if let Err(error) = self.structure.settings.save() {
            self.record_job_error("theme setting", error);
        }
        for (_, field) in &self.param_fields {
            field.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        self.tools.set_theme(theme, cx);
        if let Some(field) = &self.series_display.frame_field {
            field.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        if let Some(field) = &self.series_display.reference_field {
            field.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        for (_, field) in &self.fit_range_fields {
            field.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        for (_, field) in self.joint.fields.values() {
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
        for field in [
            &self.structure.search,
            &self.structure.radius,
            &self.structure.max_reff_input,
            &self.structure.mp_key,
            &self.structure.path_filter,
            &self.structure.poly_cutoff_input,
        ] {
            field.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        if let Some(field) = &self.view_offset_field {
            field.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        if let Some(input) = &self.filter_input {
            input.update(cx, |f, cx| f.set_theme(theme, cx));
        }
        // The wavelet fields keep their own theme copy; rebuilding them from
        // the current definition restyles them without losing any value.
        if let Ok(definition) = self.wavelet_definition(cx) {
            self.set_wavelet_fields(&definition, cx);
        }
        self.invalidate_explore_plots(cx);
        self.rebuild_operando_plots(cx);
        self.rebuild_fit_plots(cx);
        self.rebuild_fit_preview_plots(cx, false);
        self.restyle_rmc(cx);
        self.restyle_wavelet(cx);
        // Live plots own a separate display cursor and copied plot theme.
        // Refresh their presentation without changing the acquisition recipe.
        self.restyle_live(cx);
        cx.notify();
    }

    // ---- catalog -----------------------------------------------------------

    fn open_folder(&mut self, cx: &mut Context<Self>) {
        let folder = self
            .structure
            .settings
            .recent_import_folders
            .first()
            .cloned();
        self.open_import_picker(folder, cx);
    }

    /// Open a folder: restore the persisted index instantly when one exists
    /// (doc: "reopening a million-file project is < 1 s"), falling back to a
    /// streaming walk. Either way the tree is (re)walked in the background —
    /// as the primary scan or as the freshness re-check.
    fn scan_folder(&mut self, root: PathBuf, restore: bool, cx: &mut Context<Self>) {
        if !restore {
            self.bind_joint_sources();
            let root = root.canonicalize().unwrap_or(root.clone());
            for source in self.group_registry.sources() {
                if source.path.starts_with(&root)
                    && source.path.is_file()
                    && let Some(id) = self.group_registry.reimport_source(&source.path, None)
                {
                    self.record_reimport(BTreeSet::from([id]));
                }
            }
        }
        self.reset_catalog_state(cx);
        self.source_dir = Some(root.clone());
        self.catalog.scanning = true;
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
        let sources = self.group_registry.sources();
        let job = cx.background_executor().spawn({
            let root = root.clone();
            async move {
                load_index(&index_path, &root).map(|catalog| {
                    let entries =
                        crate::group_identity::GroupRegistry::prepare_catalog(&catalog, &sources);
                    (catalog, entries)
                })
            }
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                if app.catalog_gen != catalog_gen {
                    return;
                }
                match result {
                    Ok((catalog, mut entries)) => {
                        let total = catalog.len();
                        app.tools.invalidate_bindings();
                        let interaction = app.capture_groups();
                        entries.include_recent(&catalog, &app.group_registry);
                        app.catalog = catalog;
                        app.group_registry.replace_catalog(entries);
                        app.resolve_group_state();
                        app.resolve_interaction(interaction);
                        app.resolve_pending_overrides(cx);
                        app.restore_project_selection(cx);
                        app.status =
                            format!("index loaded · {total} files · checking for changes ...")
                                .into();
                        if total > 0
                            && app.selected.is_none()
                            && app.current_path.as_os_str().is_empty()
                        {
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
                            app.finish_routed_import(cx);
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                }
            }
            let Ok(Some(live_parts)) = this.update(cx, |app, _| {
                (app.catalog_gen == catalog_gen).then(|| {
                    app.capture_groups();
                    (app.catalog.index_parts(), app.group_registry.sources())
                })
            }) else {
                return;
            };
            // Million-entry comparison stays off the UI thread.
            let compare = cx.background_executor().spawn(async move {
                let unchanged = live_parts.0.same_index(&shadow.index_parts());
                let entries =
                    crate::group_identity::GroupRegistry::prepare_catalog(&shadow, &live_parts.1);
                (unchanged, shadow, entries)
            });
            let (unchanged, shadow, entries) = compare.await;
            this.update(cx, |app, cx| {
                if app.catalog_gen != catalog_gen {
                    return;
                }
                app.verify_running = false;
                if unchanged {
                    app.status = format!("index verified · {} files", app.catalog.len()).into();
                } else {
                    let before = app.catalog.len();
                    app.install_refreshed_catalog(shadow, entries, cx);
                    app.status = format!(
                        "index refreshed · {} files (was {before})",
                        app.catalog.len()
                    )
                    .into();
                    app.persist_catalog_index(cx);
                }
                app.finish_routed_import(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Swap in a freshly walked catalog after the index diverged. Everything
    /// keyed by catalog indices is invalidated; the active spectrum is
    /// re-located by path so the plots keep their subject when it survived.
    fn install_refreshed_catalog(
        &mut self,
        catalog: Catalog,
        mut entries: crate::group_identity::PreparedCatalog,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_index_bindings(IndexChange::Catalog);
        let interaction = self.capture_groups();
        entries.include_recent(&catalog, &self.group_registry);
        self.catalog = catalog;
        self.group_registry.replace_catalog(entries);
        self.resolve_group_state();
        self.resolve_interaction(interaction);
        self.expanded_scan = None;
        self.active_scan = None;
        self.overview_series = None;
        self.operando = None;
        self.operando_plots = None;
        self.time_pos = 0;
        self.pending_time_pos = None;
        self.batch_fit = None;
        self.resolve_pending_overrides(cx);
        if self.pending_project_spectrum.is_some() {
            self.restore_project_selection(cx);
        } else if let Some(ix) = self.selected {
            // The generation change cancelled any in-flight first load.
            // Explicitly reload even if this group was already selected.
            let path = if ix < DERIVED_BASE {
                self.catalog.path(ix)
            } else {
                self.current_path.clone()
            };
            let label = self.entry_label(ix);
            self.load_spectrum(ix, path, label.into(), cx);
        }
        self.ensure_compare_loaded(cx);
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
                            let start = app.catalog.len();
                            app.catalog.extend(batch);
                            app.register_appended_groups(start, app.derived.len());
                            let active_extended = app.active_scan.is_some_and(|scan_ix| {
                                let new_len = app.catalog.scans.get(scan_ix).map(|scan| scan.len);
                                old_active_len.is_some() && new_len > old_active_len
                            });
                            if active_extended && app.workspace == Workspace::Operando {
                                app.ensure_operando(cx);
                            }
                            // Show something as soon as the index has anything.
                            if first
                                && app.selected.is_none()
                                && app.pending_project_spectrum.is_none()
                                && app.pending_derived.is_none()
                            {
                                app.select_entry(0, cx);
                            }
                        }
                        ScanEvent::Done { total } => {
                            app.catalog.scanning = false;
                            app.status = format!("indexed {total} files").into();
                            app.resolve_pending_overrides(cx);
                            app.restore_project_selection(cx);
                            app.persist_catalog_index(cx);
                            if !app.filter_text.is_empty() {
                                app.apply_filter(cx);
                            }
                            if let Some(scan_ix) = app.active_scan {
                                let invalid_fingerprint =
                                    app.catalog.scans.get(scan_ix).map(|scan| {
                                        scan_fingerprint(
                                            &app.params,
                                            &app.overrides,
                                            &app.group_registry,
                                            scan.start,
                                            scan.len,
                                        ) ^ 1
                                    });
                                if let (Some(operando), Some(fingerprint)) =
                                    (&mut app.operando, invalid_fingerprint)
                                {
                                    // Force a final overview refresh without exposing an
                                    // invalid length to the scrubber or cursor mapping.
                                    operando.fingerprint = fingerprint;
                                }
                                if app.workspace == Workspace::Operando {
                                    app.ensure_operando(cx);
                                }
                            }
                        }
                        ScanEvent::Error(e) => {
                            app.catalog.scanning = false;
                            app.set_status(StatusKind::Error, format!("scan failed: {e}"));
                            app.record_job_error("catalog scan", e);
                        }
                    }
                    if done {
                        app.finish_routed_import(cx);
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

    /// Current, keyboard focus, marks and anchor have independent lifetimes.
    fn click_entry(&mut self, ix: usize, modifiers: gpui::Modifiers, cx: &mut Context<Self>) {
        let gesture = if modifiers.shift {
            group_rows::Gesture::Range
        } else if modifiers.secondary() {
            group_rows::Gesture::Toggle
        } else {
            group_rows::Gesture::Current
        };
        let rows = self.interaction_rows();
        if group_rows::interact(
            &rows,
            &mut self.focus_group,
            &mut self.mark_anchor,
            &mut self.selection,
            ix,
            gesture,
        ) {
            self.select_entry(ix, cx);
        } else {
            self.ensure_compare_loaded(cx);
        }
        self.sync_param_fields(cx);
        self.sync_handles(cx);
        cx.notify();
    }

    fn nav_move(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let Some(next) = self.interaction_rows().neighbor(self.focus_group, delta) else {
            return;
        };
        self.click_entry(
            next,
            gpui::Modifiers {
                shift: extend,
                ..Default::default()
            },
            cx,
        );
        self.scroll_group_row(next);
    }

    /// Add a whole scan to the compare set (shift/cmd-click on a scan row).
    fn select_scan_range(&mut self, scan_ix: usize, cx: &mut Context<Self>) {
        if let Some(scan) = self.catalog.scans.get(scan_ix) {
            self.selection.extend(
                (scan.start..scan.start + scan.len)
                    .filter(|&ix| !self.group_registry.index_excluded(ix)),
            );
            self.ensure_compare_loaded(cx);
            self.sync_param_fields(cx);
            cx.notify();
        }
    }

    fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.group_state.marked.clear();
        if !self.selection.is_empty() {
            self.selection.clear();
            self.invalidate_explore_plots(cx);
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

    fn remove_derived(&mut self, i: usize, cx: &mut Context<Self>) {
        if let Some(id) = self.group_id(DERIVED_BASE + i) {
            self.remove_groups(
                BTreeSet::from([id]),
                format!("remove {}", self.entry_label(DERIVED_BASE + i)),
                cx,
            );
        }
    }

    /// Same displayed scope as the toolbar and keyboard command.
    fn select_filter_results(&mut self, cx: &mut Context<Self>) {
        self.mark_all(true, cx);
    }

    fn entry_label(&self, ix: usize) -> String {
        self.group_state
            .display_label(self.peek_group_id(ix).as_ref(), || {
                self.default_entry_label(ix)
            })
    }

    fn default_entry_label(&self, ix: usize) -> String {
        if ix == NO_ENTRY {
            return self
                .standalone_source
                .as_ref()
                .map(|(_, label, _)| label.to_string())
                .unwrap_or_else(|| self.spectrum_label.to_string());
        }
        if ix >= DERIVED_BASE {
            self.derived
                .get(ix - DERIVED_BASE)
                .map(|d| {
                    if d.source.is_some() {
                        format!(
                            "{} · {}",
                            self.effective_params(ix).import.mode.label(),
                            d.label
                        )
                    } else {
                        d.display_label()
                    }
                })
                .unwrap_or_else(|| "merged".into())
        } else {
            self.catalog.name(ix).to_string()
        }
    }

    fn select_entry(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.group_id(ix).is_none() {
            return;
        }
        self.focus_group = Some(ix);
        self.mark_anchor = Some(ix);
        self.pending_project_spectrum = None;
        self.pending_derived = None;
        if ix == NO_ENTRY {
            if let Some((path, label, _)) = self.standalone_source.clone() {
                self.selected = None;
                self.current_path = path.clone();
                self.reveal_group_row(ix);
                self.sync_param_fields(cx);
                self.load_spectrum(ix, path, label, cx);
            }
            return;
        }
        if ix >= DERIVED_BASE {
            if ix - DERIVED_BASE >= self.derived.len() {
                return;
            }
            self.selected = Some(ix);
            self.sync_operando_cursor_to_entry(ix, cx);
            self.reveal_group_row(ix);
            let label: SharedString = self.entry_label(ix).into();
            self.current_path = self.derived[ix - DERIVED_BASE]
                .source
                .clone()
                .unwrap_or_default();
            // Field synchronization refreshes the preview for this channel/source too.
            self.sync_param_fields(cx);
            self.load_spectrum(ix, self.current_path.clone(), label, cx);
            return;
        }
        if ix >= self.catalog.len() {
            return;
        }
        self.reveal_group_row(ix);
        self.selected = Some(ix);
        self.sync_operando_cursor_to_entry(ix, cx);
        let label: SharedString = self.catalog.name(ix).to_string().into();
        let path = self.catalog.path(ix);
        self.current_path = path.clone();
        // The panel must show this spectrum's effective (override or
        // global) params.
        self.sync_param_fields(cx);
        self.load_spectrum(ix, path, label, cx);
    }

    /// Full-source diagnostics and column preview on the background executor;
    /// the result is dropped if the selection moved on before it arrived.
    fn update_import_preview(&mut self, cx: &mut Context<Self>) {
        let path = self.current_path.clone();
        // Metadata must never describe the previously selected file while the
        // next preview is loading (the structure library follows this hint).
        self.import_preview = None;
        self.import_preview_error = "".into();
        self.import_preview_gen += 1;
        if path.as_os_str().is_empty() {
            self.import_preview_error = if self
                .selected
                .and_then(|ix| ix.checked_sub(DERIVED_BASE))
                .and_then(|i| self.derived.get(i))
                .and_then(|group| group.operation.as_ref())
                .is_some_and(|op| op.tool == "Measurement import")
            {
                "Original measurement and import mapping are retained in this project.".into()
            } else {
                "This group has no source file for a column preview.".into()
            };
            return;
        }
        let generation = self.import_preview_gen;
        let import = self.ui_params().import.clone();
        let intake_origin = self.intake_origin(self.selected.unwrap_or(NO_ENTRY), &path);
        let job = cx.background_executor().spawn({
            let path = path.clone();
            async move { preview_import(&path, &import) }
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                if finish_import_preview(
                    (&app.current_path, app.import_preview_gen),
                    (&path, generation),
                    result,
                    &mut app.import_preview,
                    &mut app.import_preview_error,
                ) {
                    if let Some(preview) = &app.import_preview {
                        let diagnostics = preview.diagnostics.clone();
                        let channel = preview.resolved.mode;
                        let edge =
                            crate::source_evidence::DeclaredEdge::from_header(preview.xdi.as_ref());
                        app.record_source_warnings(
                            app.selected.unwrap_or(NO_ENTRY),
                            intake_origin.as_ref(),
                            &path,
                            &diagnostics,
                            edge,
                            channel,
                        );
                        if let Some(id) = app.peek_group_id(app.selected.unwrap_or(NO_ENTRY)) {
                            let warnings = diagnostics
                                .warnings()
                                .into_iter()
                                .map(|message| JobError::warning(&path, message))
                                .collect();
                            app.group_diagnostics.set_warnings(
                                id,
                                app.active_fingerprint(),
                                warnings,
                            );
                        }
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    // ---- fitting -----------------------------------------------------------

    fn build_range_fields(
        theme: Theme,
        ranges: &FitRanges,
        cx: &mut Context<Self>,
    ) -> Vec<(RangeKey, Entity<NumericField>)> {
        let specs: [(RangeKey, &str, f64); 5] = [
            (RangeKey::Kmin, "k min (Å⁻¹)", ranges.kmin),
            (RangeKey::Kmax, "k max (Å⁻¹)", ranges.kmax),
            (RangeKey::Rmin, "R min (Å)", ranges.rmin),
            (RangeKey::Rmax, "R max (Å)", ranges.rmax),
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
                        FieldEvent::InspectMixed => return,
                        FieldEvent::Invalid(message) => {
                            this.set_status(StatusKind::Error, message.clone());
                            cx.notify();
                            return;
                        }
                    };
                    if let Some(v) = value {
                        if matches!(key, RangeKey::Rmin) && *v < this.fit_background_floor(None) {
                            this.set_status(
                                StatusKind::Error,
                                format!(
                                    "Fit R min must be at least Rbkg ({:.4} Å).",
                                    this.fit_background_floor(None)
                                ),
                            );
                            this.sync_range_fields(cx);
                            cx.notify();
                            return;
                        }
                        let r = &mut this.fit_ranges;
                        match key {
                            RangeKey::Kmin => r.kmin = *v,
                            RangeKey::Kmax => r.kmax = *v,
                            RangeKey::Rmin => r.rmin = *v,
                            RangeKey::Rmax => r.rmax = *v,
                            RangeKey::Kweight => r.kweight = *v,
                        }
                        this.fit_model_changed(cx);
                        this.rebuild_fit_plots(cx);
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
                    self.set_status(
                        StatusKind::Error,
                        format!(
                            "invalid {} bound for {name}",
                            if is_min { "min" } else { "max" }
                        ),
                    );
                    cx.notify();
                    return;
                }
            }
        };
        self.fit_template_dirty = true;
        if let Some(var) = self.fit_vars.iter_mut().find(|var| var.spec.name == name) {
            if is_min {
                var.spec.min = value;
            } else {
                var.spec.max = value;
            }
            self.fit_model_changed(cx);
        }
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
        self.fit_template_dirty = true;
        self.fit_model_changed(cx);
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
        // An empty N cell means "use the FEFF degeneracy"; the other primary
        // cells keep their previous value instead of going blank.
        if text.is_empty() && param.is_primary() && param != PathParam::Degen {
            return;
        }
        if text.parse::<f64>().is_err() {
            for ident in expr_identifiers(&text) {
                self.ensure_fit_var(&ident, default_for(param), cx);
            }
        }
        if let Some(row) = self.fit_paths.get_mut(path_ix) {
            param.set(&mut row.spec, text);
            self.fit_template_dirty = true;
            self.fit_model_changed(cx);
        }
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
                    app.refresh_path_infos();
                    app.fit_model_changed(cx);
                    app.set_fit_step(shell::fit_workspace::FitStep::Paths, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// Add a path with empty cells; the parameter template fills them once
    /// the path is selected. Every parameter cell accepts a number or an
    /// expression.
    fn push_fit_path(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        if self.fit_paths.iter().any(|p| p.spec.file == file) {
            return;
        }
        let spec = FitPathSpec::blank(file);
        self.add_path_row(spec, cx);
    }

    /// Re-rank `fit_paths` (labels, shells, importance) after the list changed.
    pub(crate) fn refresh_path_infos(&mut self) {
        let dats: Vec<rexafs::prelude::FeffDat> = self
            .fit_paths
            .iter()
            .map(|row| {
                rexafs::prelude::feffpath(
                    row.spec.file.to_string_lossy().as_ref(),
                    rexafs::prelude::FeffFlavor::Feff85L,
                )
                .map(|m| m.feff)
                .unwrap_or_else(|_| rexafs::prelude::FeffDat {
                    filename: row.spec.label.clone(),
                    reff: row.meta.as_ref().map(|m| m.reff).unwrap_or(0.0),
                    degen: row.meta.as_ref().map(|m| m.degen).unwrap_or(1.0),
                    nleg: row.meta.as_ref().map(|m| m.nleg).unwrap_or(2),
                    ..Default::default()
                })
            })
            .collect();
        self.fit_path_infos.clear();
        for source in self.path_sources() {
            let ids: Vec<_> = self
                .fit_paths
                .iter()
                .enumerate()
                .filter(|(_, p)| p.spec.file.parent() == Some(source.as_path()))
                .map(|(i, _)| i)
                .collect();
            let local: Vec<_> = ids.iter().map(|&i| dats[i].clone()).collect();
            for mut info in rank_paths(&local) {
                info.index = ids[info.index];
                self.fit_path_infos.push(info);
            }
        }
        self.fit_path_infos.sort_by_key(|p| p.index);
    }

    /// Selected paths, as ranked summaries.
    pub(crate) fn selected_path_infos(&self) -> Vec<PathInfo> {
        self.fit_path_infos
            .iter()
            .filter(|p| self.fit_paths.get(p.index).is_some_and(|r| r.spec.enabled))
            .cloned()
            .collect()
    }

    /// Selection changed from the picker: keep the parameters in step with
    /// it unless the user has edited them by hand.
    pub(crate) fn paths_selection_changed(&mut self, cx: &mut Context<Self>) {
        if !self.fit_template_dirty && self.fit_template != ParameterTemplate::Manual {
            self.apply_fit_template(cx);
        } else {
            self.fit_model_changed(cx);
        }
    }

    /// Rebuild the variables and every path's parameter cells from the
    /// chosen template and the current selection.
    pub(crate) fn apply_fit_template(&mut self, cx: &mut Context<Self>) {
        self.apply_fit_template_from(0, cx);
    }

    fn apply_fit_template_from(&mut self, first_new: usize, cx: &mut Context<Self>) {
        let selected = self.selected_path_infos();
        let specs: Vec<_> = self.fit_paths.iter().map(|p| p.spec.clone()).collect();
        let result = crate::fitting::source_template(self.fit_template, &selected, &specs);
        if first_new == 0 {
            self.fit_vars.clear();
        }
        for v in &result.variables {
            if first_new > 0 && self.fit_vars.iter().any(|old| old.spec.name == v.name) {
                continue;
            }
            self.ensure_fit_var(&v.name, v.value, cx);
            if let Some(var) = self.fit_vars.iter_mut().find(|x| x.spec.name == v.name) {
                var.spec.vary = v.vary;
                var.spec.min = v.min;
                var.spec.max = v.max;
                var.spec.expr = v.expr.clone();
                let text = match &v.expr {
                    Some(expr) => expr.clone(),
                    None => format!("{}", v.value),
                };
                var.field.update(cx, |f, cx| f.set_text(text, cx));
                var.min_field.update(cx, |f, cx| {
                    f.set_text(v.min.map(|x| x.to_string()).unwrap_or_default(), cx)
                });
                var.max_field.update(cx, |f, cx| {
                    f.set_text(v.max.map(|x| x.to_string()).unwrap_or_default(), cx)
                });
            }
        }
        for (i, row) in self.fit_paths.iter_mut().enumerate() {
            if i < first_new {
                continue;
            }
            match result.assignments.iter().find(|a| a.index == i) {
                Some(a) => {
                    row.spec.s02 = a.s02.clone();
                    row.spec.e0 = a.e0.clone();
                    row.spec.deltar = a.deltar.clone();
                    row.spec.sigma2 = a.sigma2.clone();
                }
                None => {
                    row.spec.s02.clear();
                    row.spec.e0.clear();
                    row.spec.deltar.clear();
                    row.spec.sigma2.clear();
                }
            }
            for (param, field) in &row.fields {
                if param.is_primary() {
                    let text = param.get(&row.spec).to_string();
                    field.update(cx, |f, cx| f.set_text(text, cx));
                }
            }
        }
        self.fit_template_notes = result.notes;
        if first_new == 0 {
            self.fit_template_dirty = false;
        }
        self.fit_model_changed(cx);
    }

    /// Materialize a path spec into a row with editable cells.
    fn add_path_row(&mut self, spec: FitPathSpec, cx: &mut Context<Self>) {
        let meta = path_meta(&spec.file);
        let theme = self.theme;
        let path_ix = self.fit_paths.len();
        let fields = PathParam::ALL
            .into_iter()
            .map(|param| (param, param.get(&spec).to_string()))
            .map(|(param, initial)| {
                let placeholder: SharedString = match param {
                    PathParam::Degen => meta
                        .as_ref()
                        .map(|m| format!("{}", m.degen))
                        .unwrap_or_else(|| "FEFF".into())
                        .into(),
                    p if p.is_primary() => "expr".into(),
                    _ => "0".into(),
                };
                let field = cx.new(|cx| {
                    TextInput::new(placeholder, initial, theme, cx).with_style(
                        crate::widgets::text_input::InputStyle {
                            mono: true,
                            ..Default::default()
                        },
                    )
                });
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
        let more = [&spec.ei, &spec.third, &spec.fourth]
            .iter()
            .any(|s| !s.trim().is_empty());
        self.fit_paths.push(FitPathRow {
            spec,
            meta,
            fields,
            more,
        });
    }

    fn run_fit_now(&mut self, cx: &mut Context<Self>) {
        if self.joint.config.enabled {
            self.run_joint_fit_now(cx);
            return;
        }
        if let Some(reason) = self.fit_blocker() {
            self.status = reason.into();
            cx.notify();
            return;
        }
        let Some(sp) = &self.spectrum else {
            self.status = "no spectrum loaded".into();
            cx.notify();
            return;
        };
        let sp = sp.clone();
        self.fit_gen += 1;
        let generation = self.fit_gen;
        let provenance = FitProvenance {
            group_id: self.active_group_id(),
            label: self.spectrum_label.clone(),
            path: self.spectrum_path.clone(),
            params_fingerprint: self.spectrum_fingerprint,
            model_fingerprint: self.fit_model_fingerprint(),
        };
        self.fit_error = None;
        self.job_inputs[3] = self
            .current_group_index()
            .and_then(|ix| self.group_id(ix))
            .into_iter()
            .collect();
        self.fit_running = true;
        self.status = format!("fitting {} ...", provenance.label).into();
        cx.notify();
        let paths: Vec<FitPathSpec> = self.fit_paths.iter().map(|r| r.spec.clone()).collect();
        let vars: Vec<FitVarSpec> = self.fit_vars.iter().map(|v| v.spec.clone()).collect();
        let ranges = self.fit_ranges.resolved(self.ui_params().fft_kweight);
        let started = Instant::now();
        let history_inputs = (paths.clone(), vars.clone(), ranges.clone());
        let noise_estimated = ranges.noise;
        let job = cx
            .background_executor()
            .spawn(async move { run_spectrum_fit(&sp, &paths, &vars, &ranges) });
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
                        app.joint.result_config = None;
                        app.fit_provenance = Some(provenance.clone());
                        app.pending_fit_restore = None;
                        let stale = app.fit_is_stale();
                        let reduced_chi_square = crate::fitting::reduced_chi_square_text(
                            result.reduced_chi_square,
                            noise_estimated,
                        );
                        app.status = format!(
                            "fit {} for {}{} · R-factor {:.5} · reduced χ² {}",
                            if result.solver_report.as_ref().is_some_and(|r| !r.converged) {
                                "stopped"
                            } else {
                                "done"
                            },
                            provenance.label,
                            if stale { " (stale)" } else { "" },
                            result.r_factor,
                            reduced_chi_square
                        )
                        .into();
                        let result = Arc::new(result);
                        app.fit_result = Some(result.clone());
                        if app.stage_view.fit_step == shell::fit_workspace::FitStep::Model {
                            app.set_fit_step(shell::fit_workspace::FitStep::Results, cx);
                        }
                        app.record(
                            format!(
                                "fit {} · R {:.4} · reduced χ² {}",
                                provenance.label, result.r_factor, reduced_chi_square
                            ),
                            None,
                        );
                        app.push_fit_history(&provenance, history_inputs, result.clone());
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
                            app.fit_model_changed(cx);
                        }
                    }
                    Err(e) => {
                        app.fit_error = Some(e.to_string());
                        app.set_status(StatusKind::Error, format!("fit failed: {e}"));
                        app.record_job_error(format!("fit: {}", provenance.label), e.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Record a completed fit in the history (model snapshot + stats).
    fn push_fit_history(
        &mut self,
        provenance: &FitProvenance,
        inputs: (Vec<FitPathSpec>, Vec<FitVarSpec>, FitRanges),
        result: Arc<FeffFitResult>,
    ) {
        let id = self.fit_history.last().map(|e| e.id + 1).unwrap_or(1);
        let (paths, vars, ranges) = inputs;
        let mut entry = FitHistoryEntry::from_result(
            id,
            provenance.label.to_string(),
            paths,
            vars,
            ranges,
            &result,
        );
        entry.group_id = provenance.group_id;
        entry.params_fingerprint = Some(provenance.params_fingerprint);
        self.fit_history.push(entry);
        self.fit_history_results.insert(id, result);
        self.fit_history_selected = Some(id);
    }

    /// Restore the model of a history entry (paths, variables, ranges) and
    /// show its result again when it was fit in this session.
    pub(crate) fn restore_fit_history(&mut self, id: usize, cx: &mut Context<Self>) {
        let Some(entry) = self.fit_history.iter().find(|e| e.id == id).cloned() else {
            return;
        };
        self.joint.config = entry.joint.clone().unwrap_or_default();
        self.joint.fields.clear();
        self.joint.result_config = entry.joint.clone();
        self.joint.result_index = 0;
        self.joint.setup = false;
        self.fit_paths.clear();
        self.fit_vars.clear();
        for spec in entry.paths {
            self.add_path_row(spec, cx);
        }
        self.refresh_path_infos();
        self.fit_template_dirty = true;
        for saved in entry.vars {
            self.ensure_fit_var(&saved.name, saved.value, cx);
            if let Some(var) = self.fit_vars.iter_mut().find(|v| v.spec.name == saved.name) {
                var.spec = saved.clone();
                let text = match &saved.expr {
                    Some(expr) => expr.clone(),
                    None => format!("{}", saved.value),
                };
                var.field.update(cx, |f, cx| f.set_text(text, cx));
                var.min_field.update(cx, |f, cx| {
                    f.set_text(saved.min.map(|v| v.to_string()).unwrap_or_default(), cx)
                });
                var.max_field.update(cx, |f, cx| {
                    f.set_text(saved.max.map(|v| v.to_string()).unwrap_or_default(), cx)
                });
            }
        }
        self.fit_ranges = entry.ranges;
        self.sync_range_fields(cx);
        self.fit_history_selected = Some(id);
        self.pending_fit_restore = None;
        if let Some(result) = self.fit_history_results.get(&id).cloned() {
            self.fit_result = Some(result);
            // The recorded group and processing fingerprint let an unchanged
            // spectrum count as up to date; older entries fall back to the
            // loaded spectrum's fingerprint.
            self.fit_provenance = Some(FitProvenance {
                group_id: entry.group_id,
                label: entry.group.clone().into(),
                path: self.spectrum_path.clone(),
                params_fingerprint: entry
                    .params_fingerprint
                    .unwrap_or(self.spectrum_fingerprint),
                model_fingerprint: self.fit_model_fingerprint(),
            });
            self.rebuild_fit_plots(cx);
        }
        self.status = format!("restored fit {id} ({})", entry.group).into();
        self.record(format!("restore fit {id}"), None);
        self.fit_model_changed(cx);
    }

    /// Push `fit_ranges` into the range fields (never emits).
    pub(crate) fn sync_range_fields(&mut self, cx: &mut Context<Self>) {
        let ranges = self.fit_ranges.clone();
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
    }

    pub(crate) fn rebuild_fit_plots(&mut self, cx: &mut Context<Self>) {
        let Some(result) = self.fit_result.clone() else {
            return;
        };
        let result = if self.joint.result_config.is_some() {
            Arc::new(crate::joint_fitting::result_view(
                &result,
                self.joint.result_index,
            ))
        } else {
            result
        };
        let v = self.stage_view;
        let highlight = self
            .structure
            .hovered
            .and_then(|i| self.fit_paths.get(i).map(|r| r.spec.label.clone()));
        let mut k_plot = build_fit_k(&result, &self.theme, v.fit_show_paths, highlight.as_deref());
        let mut r_plot = build_fit_r(
            &result,
            &self.theme,
            v.fit_show_paths,
            v.fit_show_re,
            v.fit_show_im,
        );
        let q_plot = build_fit_q(&result, &self.theme);
        let k_res = build_fit_residual_k(&result, &self.theme);
        let r_res = build_fit_residual_r(&result, &self.theme);
        // Fit ranges as draggable regions, drawn into the base plots. The
        // figure aspect follows the card layout (two columns or one).
        let two_columns = self.stage_view.fit_view == shell::FitView::Both;
        let main_fallback = if two_columns { (600, 620) } else { (900, 560) };
        let res_fallback = (main_fallback.0, 90);
        let size =
            |key: usize, fallback: (u32, u32)| self.card_px.get(&key).copied().unwrap_or(fallback);
        let (kw_px, kh_px) = size(shell::handles::PLOT_FIT_K, main_fallback);
        let (rw_px, rh_px) = size(shell::handles::PLOT_FIT_R, main_fallback);
        let (qw_px, qh_px) = size(shell::handles::PLOT_FIT_Q, (900, 560));
        let (krw, krh) = size(shell::handles::PLOT_FIT_K_RES, res_fallback);
        let (rrw, rrh) = size(shell::handles::PLOT_FIT_R_RES, res_fallback);
        k_plot = k_plot.size_px(kw_px, kh_px);
        r_plot = r_plot.size_px(rw_px, rh_px);
        let q_plot = q_plot.size_px(qw_px, qh_px);
        let k_res = k_res.size_px(krw, krh);
        let r_res = r_res.size_px(rrw, rrh);
        match &self.fit_plots {
            Some(plots) => {
                plots
                    .k
                    .update(cx, |rp, cx| rp.set_plot_keep_view(k_plot, cx));
                plots
                    .k_residual
                    .update(cx, |rp, cx| rp.set_plot_keep_view(k_res, cx));
                plots
                    .r
                    .update(cx, |rp, cx| rp.set_plot_keep_view(r_plot, cx));
                plots
                    .r_residual
                    .update(cx, |rp, cx| rp.set_plot_keep_view(r_res, cx));
                plots
                    .q
                    .update(cx, |rp, cx| rp.set_plot_keep_view(q_plot, cx));
            }
            None => {
                self.fit_plots = Some(FitPlots {
                    k: plot_builder(k_plot).interactive().build(cx),
                    k_residual: plot_builder(k_res).interactive().build(cx),
                    r: plot_builder(r_plot).interactive().build(cx),
                    r_residual: plot_builder(r_res).interactive().build(cx),
                    q: plot_builder(q_plot).interactive().build(cx),
                });
            }
        }
    }

    /// A plot container reported its size: remember it and rebuild the
    /// affected plots so the figure takes the card's aspect exactly.
    pub(crate) fn note_card_size(&mut self, key: usize, size: (u32, u32), cx: &mut Context<Self>) {
        if size.0 < 40 || size.1 < 30 {
            return;
        }
        let changed = match self.card_px.get(&key) {
            Some(prev) => prev.0.abs_diff(size.0) >= 6 || prev.1.abs_diff(size.1) >= 6,
            None => true,
        };
        if !changed {
            return;
        }
        self.card_px.insert(key, size);
        if key == 300 {
            self.rebuild_analysis_plot(cx);
        } else if (301..=303).contains(&key) {
            self.rebuild_operando_plots(cx);
        } else if (shell::fit_preview::PREVIEW_K..=shell::fit_preview::PREVIEW_Q).contains(&key) {
            self.rebuild_fit_preview_plots(cx, false);
        } else if key >= shell::handles::PLOT_FIT_K {
            self.rebuild_fit_plots(cx);
        } else {
            self.explore_plots_dirty = true;
            if self.stage == Stage::Fit && self.fit_plots.is_none() {
                self.rebuild_explore_plots(cx);
                self.explore_plots_dirty = false;
            } else {
                self.invalidate_explore_plots(cx);
            }
        }
        cx.notify();
    }

    /// Invisible full-size element that measures its container and feeds
    /// `note_card_size` on the next frame (only when the size changed).
    pub(crate) fn measure_card(&self, key: usize, cx: &Context<Self>) -> impl IntoElement + use<> {
        let entity = cx.entity();
        let prev = self.card_px.get(&key).copied();
        gpui::canvas(
            move |bounds, window, _cx| {
                let size = (
                    f32::from(bounds.size.width).round().max(0.0) as u32,
                    f32::from(bounds.size.height).round().max(0.0) as u32,
                );
                let same =
                    prev.is_some_and(|p| p.0.abs_diff(size.0) < 6 && p.1.abs_diff(size.1) < 6);
                if !same {
                    window.on_next_frame(move |_window, cx| {
                        entity.update(cx, |this, cx| this.note_card_size(key, size, cx));
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }

    // ---- batch fitting -------------------------------------------------------

    fn batch_scope_line(&self) -> SharedString {
        let Some(scan_ix) = self.active_scan else {
            return "select a scan in the data panel first".into();
        };
        let Some(scan) = self.catalog.scans.get(scan_ix) else {
            return "selected scan is unavailable".into();
        };
        let count = active_scan_indices(
            &self.group_registry,
            scan.start,
            scan.len,
            if self.batch_preview {
                MAX_FRAMES
            } else {
                usize::MAX
            },
        )
        .len();
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
        if let Some(reason) = self.batch_blocker() {
            self.status = reason.into();
            cx.notify();
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
        let fingerprint = scan_fingerprint(
            &self.params,
            &self.overrides,
            &self.group_registry,
            scan_start,
            scan_len,
        );
        let model_fingerprint = self.fit_model_fingerprint();
        let preview = self.batch_preview;
        let indices = if preview {
            active_scan_indices(&self.group_registry, scan_start, scan_len, MAX_FRAMES)
        } else {
            active_scan_indices(&self.group_registry, scan_start, scan_len, usize::MAX)
        };
        self.job_inputs[1] = indices.iter().filter_map(|&ix| self.group_id(ix)).collect();
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
        self.rebuild_operando_trend(cx);
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
        let ranges = self.fit_ranges.clone();
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
                        let frame_ranges = ranges.resolved(frame_params.fft_kweight);
                        let result = run_spectrum_fit(&sp, &paths, &vars, &frame_ranges)?;
                        Ok(BatchFitRow::from_result(
                            *frame,
                            *ix,
                            &result,
                            frame_ranges.noise,
                        ))
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
                app.rebuild_operando_trend(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Marked groups whose processed spectra are cached, as LCF standards.
    pub(crate) fn lcf_standards(&self) -> Vec<(String, Arc<XASSpectrum>)> {
        cached_marked_spectra(
            &self.selection,
            &self.cache,
            |ix| self.effective_fingerprint(ix),
            |ix| self.entry_label(ix),
        )
    }

    /// LCF weights of every frame of the active scan against the marked
    /// standards (a Series trend). Streams per-frame rows; cancel keeps them.
    pub(crate) fn run_series_lcf(&mut self, cx: &mut Context<Self>) {
        if self.lcf_running {
            return;
        }
        let Some(scan_ix) = self.active_scan else {
            self.status = "select a scan first".into();
            cx.notify();
            return;
        };
        if let Err(error) = self.tools.sync_range(cx) {
            self.set_status(StatusKind::Error, error);
            cx.notify();
            return;
        }
        let standards = self.lcf_standards();
        if standards.len() < 2 || standards.len() != self.selection.len() {
            self.status =
                "Mark at least two standards and load every marked spectrum before running LCF."
                    .into();
            cx.notify();
            return;
        }
        let Some(scan) = self.catalog.scans.get(scan_ix) else {
            return;
        };
        let scan_start = scan.start;
        let scan_len = scan.len;
        let fingerprint = scan_fingerprint(
            &self.params,
            &self.overrides,
            &self.group_registry,
            scan_start,
            scan_len,
        );
        let indices: Vec<usize> = if self.batch_preview {
            active_scan_indices(&self.group_registry, scan_start, scan_len, MAX_FRAMES)
        } else {
            active_scan_indices(&self.group_registry, scan_start, scan_len, usize::MAX)
        };
        self.job_inputs[4] = indices
            .iter()
            .copied()
            .chain(cached_marked_indices(&self.selection, &self.cache, |ix| {
                self.effective_fingerprint(ix)
            }))
            .filter_map(|ix| self.group_id(ix))
            .collect();
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
        self.analysis.lcf_series = Some(crate::project::LcfSeriesAnalysis {
            config: self.tools.lcf_config(),
            inputs: frames
                .iter()
                .map(|(frame, ix, _, label)| {
                    (
                        *frame,
                        crate::project::AnalysisInput {
                            corrections: self.correction_sources(*ix),
                            group_id: self.group_id(*ix),
                            label: label.clone(),
                            fingerprint: self.effective_fingerprint(*ix),
                        },
                    )
                })
                .collect(),
            standards: cached_marked_indices(&self.selection, &self.cache, |ix| {
                self.effective_fingerprint(ix)
            })
            .map(|ix| crate::project::AnalysisInput {
                corrections: self.correction_sources(ix),
                group_id: self.group_id(ix),
                label: self.entry_label(ix),
                fingerprint: self.effective_fingerprint(ix),
            })
            .collect(),
            rows: BTreeMap::new(),
            errors: BTreeMap::new(),
            complete: false,
            cancelled: false,
        });
        self.lcf_gen += 1;
        let generation = self.lcf_gen;
        let cancel = Arc::new(AtomicBool::new(false));
        self.lcf_running = true;
        self.lcf_cancel = Some(cancel.clone());
        self.lcf_progress = (0, total);
        self.series_lcf = Some(SeriesLcf {
            scan: scan_ix,
            fingerprint,
            names: standards.iter().map(|(n, _)| n.clone()).collect(),
            rows: BTreeMap::new(),
            total,
            cancelled: false,
        });
        self.series_trend = TrendSource::Lcf(0);
        self.status = format!("LCF trend 0/{total} ...").into();
        self.rebuild_operando_trend(cx);
        cx.notify();

        let global = Arc::new(self.params.clone());
        let frame_overrides: Arc<BTreeMap<usize, PipelineParams>> = Arc::new(
            self.overrides
                .range(scan_start..scan_start.saturating_add(scan_len))
                .map(|(&ix, p)| (ix, p.clone()))
                .collect(),
        );
        let cfg = self.tools.lcf_config();
        let standards: Arc<Vec<Arc<XASSpectrum>>> =
            Arc::new(standards.into_iter().map(|(_, sp)| sp).collect());
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<(usize, Result<Vec<f64>, String>)>();
        let job_cancel = cancel.clone();
        let job = cx.background_executor().spawn(async move {
            // Bound memory while keeping file preparation parallel. The core
            // owns fitting, per-row failures and cancellation within each batch.
            for chunk in frames.chunks(16) {
                if job_cancel.load(Ordering::Relaxed) {
                    break;
                }
                let loaded: Vec<_> = chunk
                    .par_iter()
                    .map(|(frame, ix, path, _)| {
                        let params = frame_overrides.get(ix).unwrap_or(&global);
                        (*frame, process_file(path, params))
                    })
                    .collect();
                if job_cancel.load(Ordering::Relaxed) {
                    break;
                }
                let mut prepared = Vec::new();
                let mut frame_ids = Vec::new();
                for (frame, result) in loaded {
                    match result {
                        Ok(spectrum) => {
                            frame_ids.push(frame);
                            prepared.push(spectrum);
                        }
                        Err(error) => {
                            let _ = tx.unbounded_send((frame, Err(error.to_string())));
                        }
                    }
                }
                rexafs::prelude::lcf_batch_with_progress(
                    &prepared,
                    standards.as_slice(),
                    &cfg,
                    |index, result| {
                        let row = result
                            .as_ref()
                            .map(|fit| {
                                let mut weights: Vec<_> =
                                    fit.weights.iter().map(|c| c.weight).collect();
                                weights.push(fit.r_factor);
                                weights
                            })
                            .map_err(|error| error.to_string());
                        tx.unbounded_send((frame_ids[index], row)).is_ok()
                            && !job_cancel.load(Ordering::Relaxed)
                    },
                );
            }
        });
        cx.spawn(async move |this, cx| {
            while let Some((frame, result)) = rx.next().await {
                let stop = this
                    .update(cx, |app, cx| {
                        if app.lcf_gen != generation {
                            return true;
                        }
                        match result {
                            Ok(weights) => {
                                if let Some(saved) = &mut app.analysis.lcf_series {
                                    saved.rows.insert(frame, weights.clone());
                                }
                                if let Some(lcf) = &mut app.series_lcf {
                                    lcf.rows.insert(frame, weights);
                                }
                            }
                            Err(error) => {
                                if let Some(saved) = &mut app.analysis.lcf_series {
                                    saved.errors.insert(frame, error.clone());
                                }
                                app.record_job_error(format!("LCF frame {}", frame + 1), error);
                            }
                        }
                        app.lcf_progress.0 += 1;
                        let (done, total) = app.lcf_progress;
                        app.status = format!("LCF trend {done}/{total}").into();
                        if done % 8 == 0 || done == total {
                            app.rebuild_operando_trend(cx);
                        }
                        cx.notify();
                        false
                    })
                    .unwrap_or(true);
                if stop {
                    break;
                }
            }
            // Drain every per-frame result before completing the job. A fast
            // worker must not invalidate the receiver while rows remain queued.
            job.await;
            this.update(cx, |app, cx| {
                if app.lcf_gen != generation {
                    return;
                }
                app.lcf_gen += 1;
                app.lcf_running = false;
                app.lcf_cancel = None;
                let cancelled = cancel.load(Ordering::Relaxed);
                if let Some(saved) = &mut app.analysis.lcf_series {
                    saved.cancelled = cancelled;
                    saved.complete = true;
                }
                if let Some(lcf) = &mut app.series_lcf {
                    lcf.cancelled = cancelled;
                    app.status = format!(
                        "LCF trend {} · {} / {} frames",
                        if cancelled { "cancelled" } else { "done" },
                        lcf.rows.len(),
                        lcf.total
                    )
                    .into();
                }
                app.rebuild_operando_trend(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn cancel_series_lcf(&mut self, cx: &mut Context<Self>) {
        if self.lcf_running
            && let Some(cancel) = &self.lcf_cancel
        {
            cancel.store(true, Ordering::Relaxed);
            self.status = "cancelling LCF trend — completed frames are kept".into();
            cx.notify();
        }
    }

    /// Jump to the cursor frame in the Normalize stage.
    pub(crate) fn open_frame_in_normalize(&mut self, cx: &mut Context<Self>) {
        let Some(ix) = self
            .operando
            .as_ref()
            .and_then(|d| d.source.entry(self.time_pos))
        else {
            return;
        };
        self.select_entry(ix, cx);
        self.sync_param_fields(cx);
        self.focus_process_section(shell::ProcessSection::Normalize, cx);
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

    pub(crate) fn active_batch_trend(&self) -> Option<(&BatchFitData, &str)> {
        let bf = self.batch_fit.as_ref()?;
        let data = self.operando.as_ref()?;
        if Some(bf.scan) != data.source.scan_index()
            || bf.fingerprint != data.fingerprint
            || bf.model_fingerprint != self.fit_model_fingerprint()
        {
            return None;
        }
        let name = bf.varying_names.get(bf.trend_param)?;
        Some((bf, name))
    }

    /// Resolve the displayed trend data, label, and coordinate domain together
    /// so the retained plot and its cursor cannot diverge.
    fn trend_snapshot(&self) -> TrendSnapshot {
        let scan_len = self
            .operando
            .as_ref()
            .map(|data| data.scan_len)
            .unwrap_or_default();
        match &self.series_trend {
            TrendSource::Measurement(index) => return self.measurement_trend_snapshot(*index),
            TrendSource::FitVar(name) => {
                if let Some((bf, _)) = self.active_batch_trend() {
                    let mut values = vec![f64::NAN; scan_len];
                    for row in &bf.rows {
                        if let (Some(slot), Some(value)) =
                            (values.get_mut(row.frame), row.value_of(name))
                        {
                            *slot = value;
                        }
                    }
                    return TrendSnapshot {
                        frames: trend_frames(TrendDomain::FullScan, values.len(), scan_len),
                        values,
                        name: name.clone(),
                        domain: TrendDomain::FullScan,
                    };
                }
            }
            TrendSource::Lcf(i) => {
                if let Some(lcf) = self.active_series_lcf() {
                    let mut values = vec![f64::NAN; scan_len];
                    for (frame, weights) in &lcf.rows {
                        if let (Some(slot), Some(value)) = (values.get_mut(*frame), weights.get(*i))
                        {
                            *slot = *value;
                        }
                    }
                    let name = lcf
                        .names
                        .get(*i)
                        .map(|n| format!("LCF weight · {n}"))
                        .unwrap_or_else(|| "LCF weight".into());
                    return TrendSnapshot {
                        frames: trend_frames(TrendDomain::FullScan, values.len(), scan_len),
                        values,
                        name,
                        domain: TrendDomain::FullScan,
                    };
                }
            }
            TrendSource::WhiteLine => {
                let values = self
                    .operando
                    .as_ref()
                    .map(|data| data.whitelines.clone())
                    .unwrap_or_default();
                return TrendSnapshot {
                    frames: self
                        .operando
                        .as_ref()
                        .map(|data| data.sample_frames.iter().map(|&p| p as f64).collect())
                        .unwrap_or_default(),
                    values,
                    name: "white line (norm. μ)".to_string(),
                    domain: TrendDomain::Sampled,
                };
            }
            TrendSource::E0 => {}
        }
        let values = self
            .operando
            .as_ref()
            .map(|data| data.e0s.clone())
            .unwrap_or_default();
        TrendSnapshot {
            frames: self
                .operando
                .as_ref()
                .map(|data| data.sample_frames.iter().map(|&p| p as f64).collect())
                .unwrap_or_default(),
            values,
            name: "E₀ (eV)".to_string(),
            domain: TrendDomain::Sampled,
        }
    }

    /// The LCF trend when it belongs to the displayed scan/parameters.
    pub(crate) fn active_series_lcf(&self) -> Option<&SeriesLcf> {
        let lcf = self.series_lcf.as_ref()?;
        let data = self.operando.as_ref()?;
        (Some(lcf.scan) == data.source.scan_index() && lcf.fingerprint == data.fingerprint)
            .then_some(lcf)
    }

    /// Return the requested frame and quantity. A sampled overview never
    /// substitutes a neighboring frame: missing exact rows remain blank until
    /// the generation-checked source load completes.
    fn series_frame_source(&self) -> Option<(ChikRowKey, Vec<f64>)> {
        let data = self.operando.as_ref()?;
        let space = self.stage_view.series_space;
        let (grid, matrix) = data.space(space);
        let frame = self.time_pos.min(data.scan_len.saturating_sub(1));
        let ix = data.source.entry(frame)?;
        let fingerprint = self.effective_fingerprint(ix);
        if let Some(sp) = self.cache.peek(&(ix, fingerprint))
            && let Some(row) = resample_series_frame(sp, space, grid)
        {
            return Some((
                ChikRowKey::Exact {
                    entry: ix,
                    params_fingerprint: fingerprint,
                },
                self.series_difference_row(row, series_display::SeriesSupport::from_spectrum(sp)),
            ));
        }
        let row = exact_overview_row(&data.sample_frames, matrix, frame, grid.len());
        let support = data
            .sample_frames
            .iter()
            .position(|p| *p == frame)
            .and_then(|i| data.support.get(i))
            .copied()
            .unwrap_or_default();
        let row = self.series_difference_row(row, support);
        Some((
            ChikRowKey::Sampled {
                scan: data.source.scan_index().unwrap_or(NO_ENTRY),
                overview_fingerprint: data.fingerprint,
                row: frame,
            },
            row,
        ))
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
            .is_some_and(|data| data.source.scan_index() == Some(scan_ix))
        {
            self.set_time_pos(frame, cx);
            return;
        }
        self.overview_series = None;
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
        let home = crate::settings::home_dir().unwrap_or_else(std::env::temp_dir);
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

    /// Every workspace transition invalidates outstanding calculation/picker
    /// callbacks, including switching away and back to the same directory.
    pub(crate) fn set_feff_workspace(&mut self, workspace: Option<PathBuf>) {
        self.feff_gen += 1;
        self.feff_running = false;
        if self.structure.core_cluster_workspace != workspace {
            self.structure.core_cluster = None;
            self.structure.core_cluster_workspace = None;
        }
        self.feff_workspace = workspace;
    }

    /// Read the crystal form and generate a feff.inp workspace (Atoms-lite).
    /// Create a template feff.inp workspace and open it in the system editor.
    pub(crate) fn new_feff_inp(&mut self, cx: &mut Context<Self>) {
        if self.feff_running {
            return;
        }
        match crate::feffgen::new_workspace() {
            Ok(dir) => {
                let inp = dir.join("feff.inp");
                #[cfg(target_os = "macos")]
                let _ = std::process::Command::new("open")
                    .arg("-t")
                    .arg(&inp)
                    .spawn();
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("notepad.exe").arg(&inp).spawn();
                #[cfg(target_os = "linux")]
                let _ = std::process::Command::new("xdg-open").arg(&inp).spawn();
                self.status = format!(
                    "feff.inp template at {} — edit, then Run FEFF10",
                    inp.display()
                )
                .into();
                self.set_feff_workspace(Some(dir));
                self.structure.summary = None;
                self.structure.preview_cluster = None;
                self.set_fit_step(shell::fit_workspace::FitStep::Calculate, cx);
            }
            Err(e) => {
                self.set_status(
                    StatusKind::Error,
                    format!("failed to create feff workspace: {e}"),
                );
                self.record_job_error("FEFF workspace", e.to_string());
            }
        }
        cx.notify();
    }

    pub(crate) fn choose_feff_inp(&mut self, cx: &mut Context<Self>) {
        if self.feff_running {
            return;
        }
        let generation = self.feff_gen;
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
                    if app.feff_gen != generation {
                        return;
                    }
                    app.status = format!("FEFF workspace: {}", dir.display()).into();
                    app.set_feff_workspace(Some(dir));
                    app.structure.summary = None;
                    app.structure.preview_cluster = None;
                    app.set_fit_step(shell::fit_workspace::FitStep::Calculate, cx);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Run FEFF10 on the workspace's feff.inp and import the generated paths.
    pub(crate) fn run_feff10_now(&mut self, cx: &mut Context<Self>) {
        if self.rmc.control.is_some() {
            self.status = "Stop and save the active RMC run before calculating paths.".into();
            cx.notify();
            return;
        }
        if self.feff_running {
            return;
        }
        let Some(mut workspace) = self.feff_workspace.clone() else {
            self.status = "no FEFF workspace — New feff.inp... or Choose feff.inp...".into();
            cx.notify();
            return;
        };
        match crate::feffgen::snapshot_workspace(&workspace) {
            Ok(dir) => {
                if self.structure.core_cluster_workspace.as_ref() == Some(&workspace) {
                    self.structure.core_cluster_workspace = Some(dir.clone());
                }
                workspace = dir;
                self.set_feff_workspace(Some(workspace.clone()));
            }
            Err(e) => {
                self.set_status(
                    StatusKind::Error,
                    format!("Could not copy calculation input: {e}"),
                );
                cx.notify();
                return;
            }
        }
        self.feff_gen += 1;
        let ticket = crate::feffgen::JobTicket::new(self.feff_gen, workspace.clone());
        self.feff_running = true;
        self.status = "Calculating FEFF paths…".into();
        cx.notify();
        let mode = self.structure.backend;
        let job = cx.background_executor().spawn(async move {
            let paths = crate::feffgen::run_backend(&workspace, mode)?;
            std::fs::write(
                workspace.join("engine.txt"),
                crate::feffgen::backend_name(mode),
            )
            .map_err(|e| e.to_string())?;
            Ok::<_, String>(paths)
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                if !ticket.is_current(app.feff_gen, app.feff_workspace.as_deref()) {
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
                        let first_new = app.fit_paths.len();
                        let source = paths
                            .first()
                            .and_then(|p| p.parent().map(|p| p.to_path_buf()));
                        for file in paths {
                            app.push_fit_path(file, cx);
                        }
                        app.refresh_path_infos();
                        let new_infos: Vec<_> = app
                            .fit_path_infos
                            .iter()
                            .filter(|p| p.index >= first_new)
                            .cloned()
                            .collect();
                        let mut selected = select_default(&new_infos, app.fit_ranges.rmax);
                        if selected.is_empty() {
                            if let Some(shell) = shells_of(&new_infos).first() {
                                selected = shell.paths.clone();
                            }
                        }
                        for i in selected {
                            app.fit_paths[i].spec.enabled = true;
                        }
                        if app.fit_paths.len() > first_new {
                            app.apply_fit_template_from(first_new, cx);
                        } else {
                            app.fit_model_changed(cx);
                        }
                        app.structure.source_filter = source.clone();
                        app.structure.selected = app
                            .fit_paths
                            .iter()
                            .position(|p| p.spec.file.parent() == source.as_deref());
                        app.structure.multi.clear();
                        app.structure.ms_open.clear();
                        app.structure.path_leg = None;
                        app.structure.paths.clear();
                        app.structure
                            .path_filter
                            .update(cx, |f, cx| f.set_text("", cx));
                        app.refresh_structure(cx);
                        if matches!(
                            app.stage_view.fit_step,
                            shell::fit_workspace::FitStep::Calculate
                                | shell::fit_workspace::FitStep::Structure
                        ) {
                            app.set_fit_step(shell::fit_workspace::FitStep::Paths, cx);
                        }
                        let selected = app.fit_paths.iter().filter(|r| r.spec.enabled).count();
                        app.status = format!(
                            "Path calculation complete · {n} paths, {selected} selected ({})",
                            app.fit_template.label()
                        )
                        .into();
                    }
                    Err(e) => {
                        app.set_status(StatusKind::Error, format!("Path calculation failed: {e}"));
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
        let mut group_state = self.group_state.clone();
        self.capture_group_state(&mut group_state);
        ProjectFile {
            series_measurements: self.measurements.archive.clone(),
            peak_fits: self.peaks.archive.clone(),
            normalizations: self.normalization.history.clone(),
            wavelets: self.wavelet.archive.clone(),
            parser_evidence: self.parser_evidence.clone(),
            imports: self.imports.clone(),
            import_history: self.intake.history.clone(),
            source_groups: self.group_registry.sources(),
            group_state,
            header: self.project_header.clone(),
            version: PROJECT_VERSION,
            // Reviewed measurements are materialized records, not one raw
            // catalog row per source file. Rewalking their directory on reopen
            // would create duplicates and admit still-unreviewed containers.
            // Persist the explicit raw catalog alongside these records instead.
            source_dir: self.source_dir.clone().filter(|_| {
                !self.derived.iter().any(|group| {
                    group.operation.as_ref().is_some_and(|op| {
                        matches!(op.tool.as_str(), "Measurement import" | "Live acquisition")
                    })
                }) && !self.intake.history.iter().any(|batch| {
                    batch.sources.values().any(|source| {
                        source
                            .pending
                            .as_ref()
                            .is_some_and(|pending| pending.measurement_reader)
                    })
                })
            }),
            spectrum_file: self.pending_project_spectrum.clone().or_else(|| {
                (!self.current_path.as_os_str().is_empty()).then(|| self.current_path.clone())
            }),
            params: self.params.clone(),
            overrides,
            fit_paths: self.fit_paths.iter().map(|r| r.spec.clone()).collect(),
            fit_vars: self.fit_vars.iter().map(|v| v.spec.clone()).collect(),
            fit_ranges: self.fit_ranges.clone(),
            fit_mode: self.fit_mode,
            rmc: self.rmc.project.clone(),
            feff_workspace: self.feff_workspace.clone(),
            derived: self.derived.clone(),
            active_derived: self
                .selected
                .filter(|&ix| ix >= DERIVED_BASE)
                .and_then(|ix| self.derived.get(ix - DERIVED_BASE))
                .map(|d| d.id),
            fit_history: self.fit_history.clone(),
            fit_results: self.fit_history_results.clone(),
            current_fit: self
                .fit_result
                .as_ref()
                .filter(|_| {
                    self.pending_fit_restore.is_some()
                        || (self.fit_provenance.is_some() && !self.fit_is_stale())
                })
                .and_then(|r| {
                    self.fit_history_results
                        .iter()
                        .find(|(_, v)| Arc::ptr_eq(v, r))
                        .map(|(id, _)| *id)
                }),
            joint: self.joint.config.clone(),
            publication: self.publish.settings.clone(),
            mcr_analysis: self.analysis.mcr.clone(),
            lcf_series_analysis: self.analysis.lcf_series.clone(),
            pca_analysis: self
                .analysis
                .pca
                .as_ref()
                .map(|model| crate::project::PcaAnalysis {
                    model: model.clone(),
                    target: self.analysis.pca_fit.clone(),
                    inputs: self.analysis.pca_inputs.clone(),
                }),
            lcf_analysis: self
                .analysis
                .lcf
                .as_ref()
                .map(|result| crate::project::LcfAnalysis {
                    config: self.analysis.lcf_config.clone(),
                    result: result.clone(),
                    inputs: self.analysis.lcf_inputs.clone(),
                }),
            assistant: crate::project::assistant::AssistantHistory {
                conversations: self.assistant_history.conversations.clone(),
                limit: Some(self.structure.settings.assistant_history_limit),
            },
            extensions: self.project_extensions.clone(),
            embedded: Default::default(),
            raw_files: (0..self.catalog.len())
                .map(|i| self.catalog.path(i))
                .chain(self.project_raw_files.iter().cloned())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect(),
            origin: self.project_path.clone(),
            source_origins: self.project_source_origins.clone(),
            data_storage: self.project_storage,
        }
    }

    fn save_project_confirmed(&mut self, cx: &mut Context<Self>) {
        if self.project_saving {
            return;
        }
        if self.project_storage == crate::project::DataStorage::Embedded && self.catalog.scanning {
            self.pending_close = None;
            self.status =
                "Wait for the folder scan to finish before embedding all imported data.".into();
            cx.notify();
            return;
        }
        let project = self.project_file();
        let saved_measurement_ids: std::collections::BTreeSet<_> = project
            .series_measurements
            .runs
            .iter()
            .filter(|r| r.complete && !r.cancelled)
            .map(|r| r.id.clone())
            .collect();
        let history_revision = self.assistant_history_revision;
        let generation = self.project_generation;
        let mode = self.project_storage;
        let folder = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(PathBuf::from)
            .or_else(|| self.source_dir.clone())
            .or_else(|| self.spectrum_file_folder())
            .or_else(crate::settings::home_dir)
            .unwrap_or_else(std::env::temp_dir);
        let name = self
            .project_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or("project.rxs".into());
        let rx = cx.prompt_for_new_path(&folder, Some(&name));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(mut path))) = rx.await else {
                // The save dialog was cancelled: keep the window open.
                this.update(cx, |app, _| app.pending_close = None).ok();
                return;
            };
            if path.extension().is_none() {
                path.set_extension(crate::project::PROJECT_EXTENSION);
            }
            this.update(cx, |app, cx| {
                app.project_saving = true;
                app.status = "Saving project…".into();
                cx.notify();
            })
            .ok();
            let save_path = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let saved = crate::project::save_with_storage(&save_path, &project, mode);
                    if saved.is_ok() {
                        crate::series_measurements::recovery::acknowledge_saved(&project);
                    }
                    saved.map(|header| (header, project.content_fingerprint()))
                })
                .await;
            this.update(cx, |app, cx| {
                app.project_saving = false;
                match result {
                    Ok((header, fingerprint)) => {
                        if app.project_generation == generation {
                            app.saved_fingerprint = fingerprint;
                            app.settle_saved_fingerprint = false;
                            app.measurements.acknowledge_saved(&saved_measurement_ids);
                            app.project_path = Some(path.clone());
                            app.project_header = Some(header);
                            app.assistant_history_saved_revision = history_revision;
                            if app.assistant_history_revision == history_revision {
                                app.assistant_history.limit =
                                    Some(app.structure.settings.assistant_history_limit);
                                app.assistant_history.prune_for_save();
                            }
                        }
                        app.set_status(StatusKind::Success, format!("Saved {}", path.display()));
                        app.remember_project(path.clone());
                        if let Some(intent) = app.pending_close.take() {
                            app.finish_close(intent, cx);
                        }
                    }
                    Err(error) => {
                        app.pending_close = None;
                        app.set_status(StatusKind::Error, format!("Save failed: {error}"));
                        app.record_job_error("save project", error);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn spectrum_file_folder(&self) -> Option<PathBuf> {
        self.spectrum_path
            .parent()
            .filter(|p| p.is_dir())
            .map(PathBuf::from)
    }

    fn open_project(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open project (.rxs)".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.first()
            {
                this.update(cx, |app, cx| app.load_project_path(path.clone(), cx))
                    .ok();
            }
        })
        .detach();
    }

    fn load_project_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.load_project_then_import(path, None, cx);
    }

    fn load_project_then_import(
        &mut self,
        path: PathBuf,
        data: Option<shell::path_routing::RoutedImport>,
        cx: &mut Context<Self>,
    ) {
        if !crate::project::is_project(&path) || path.is_dir() {
            self.status =
                "Choose a .rxs project file; use Import… for data files or folders.".into();
            cx.notify();
            return;
        }
        self.pending_routed_import.clear();
        self.project_load_generation += 1;
        let generation = self.project_load_generation;
        self.status = "Opening project…".into();
        let recent_path = path.clone();
        let job = cx.background_executor().spawn(async move {
            crate::project::load(&path).map(|mut project| {
                let next = project.assign_group_ids();
                let registry = crate::group_identity::GroupRegistry::from_sources(std::mem::take(
                    &mut project.source_groups,
                ));
                (project, next, registry)
            })
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                if app.project_load_generation != generation {
                    return;
                }
                match result {
                    Ok((project, next, registry)) => {
                        app.apply_project(project, next, registry, cx);
                        app.remember_project(recent_path);
                        app.pending_routed_import.extend(data);
                        app.finish_routed_import(cx);
                    }
                    Err(error) => {
                        app.set_status(StatusKind::Error, format!("Open failed: {error}"));
                        app.record_job_error("open project", error);
                        crate::settings::remove_recent(
                            &mut app.structure.settings.recent_projects,
                            &recent_path,
                        );
                        app.persist_recent_locations();
                        app.path_route =
                            shell::path_routing::RoutingCard::after_failed_project(data);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn apply_project(
        &mut self,
        project: ProjectFile,
        next_derived_id: u64,
        registry: crate::group_identity::GroupRegistry,
        cx: &mut Context<Self>,
    ) {
        self.reset_catalog_state(cx);
        self.intake = import_state::IntakeState::from_history(project.import_history.clone());
        self.imports = project.imports.clone();
        self.parser_evidence = project.parser_evidence.clone();
        self.next_derived_id = next_derived_id;
        self.group_state = Default::default();
        self.group_registry = registry;
        self.group_registry
            .set_excluded(&project.group_state.excluded);
        self.selection.clear();
        self.frozen.clear();
        self.overrides.clear();
        self.pending_overrides.clear();
        self.selected = None;
        self.focus_group = None;
        self.group_menu = None;
        self.group_rename = None;
        self.tools.alignment_standard = None;
        self.mark_anchor = None;
        self.filter_reveal = None;
        self.reveal_current = None;
        self.project_generation += 1;
        self.measurement_import = None;
        self.assistant_history = project.assistant.clone();
        self.assistant_history_revision = 0;
        self.assistant_history_saved_revision = 0;
        if let Some(assistant) = self.assistant.clone() {
            let generation = self.project_generation;
            cx.defer(move |cx| {
                assistant.update(cx, |view, cx| view.replace_project(generation, cx))
            });
        }
        self.tools.invalidate_bindings();
        if let Some(cancel) = self.analysis.mcr_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.measurements.stop();
        self.series_fits = Default::default();
        self.live.stop();
        self.live = Default::default();
        self.peaks.stop();
        self.peaks = Default::default();
        self.peaks.archive = project.peak_fits.clone();
        self.normalization = Default::default();
        self.normalization.history = project.normalizations.clone();
        self.fluorescence = Default::default();
        self.wavelet.cancel();
        self.wavelet = Default::default();
        self.wavelet.archive = project.wavelets.clone();
        self.measurements = shell::measurements::MeasurementState::from_archive(
            project.series_measurements.clone(),
        );
        for run in &mut self.measurements.archive.fit_runs {
            if !run.complete {
                Arc::make_mut(run).finish(true);
            }
        }
        self.analysis.mcr_generation_advance();
        self.analysis.mcr = project.mcr_analysis.clone();
        self.analysis.lcf_series = project.lcf_series_analysis.clone();
        self.analysis.pca = project
            .pca_analysis
            .as_ref()
            .map(|saved| saved.model.clone());
        self.analysis.pca_fit = project
            .pca_analysis
            .as_ref()
            .and_then(|saved| saved.target.clone());
        self.analysis.pca_inputs = project
            .pca_analysis
            .as_ref()
            .map(|saved| saved.inputs.clone())
            .unwrap_or_default();
        self.analysis.lcf_config = project
            .lcf_analysis
            .as_ref()
            .and_then(|saved| saved.config.clone());
        self.analysis.lcf = project
            .lcf_analysis
            .as_ref()
            .map(|saved| saved.result.clone());
        self.analysis.lcf_inputs = project
            .lcf_analysis
            .as_ref()
            .map(|saved| saved.inputs.clone())
            .unwrap_or_default();
        self.analysis.ranked.clear();
        self.analysis.plot = None;
        self.analysis.shown = None;
        self.journal = Default::default();
        self.project_path = project.origin.clone();
        self.project_storage = project
            .header
            .as_ref()
            .map(|h| h.storage)
            .unwrap_or_default();
        self.project_header = project.header.clone();
        self.project_raw_files = project.raw_files.clone();
        self.project_source_origins = project.source_origins.clone();
        self.set_feff_workspace(project.feff_workspace.clone());
        self.publish.load_settings(project.publication.clone());
        self.project_extensions = project.extensions;
        self.fit_error = None;
        self.structure.source_filter = None;
        self.structure.selected = None;
        self.structure.summary = None;
        self.structure.preview_cluster = None;
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
        self.refresh_path_infos();
        self.fit_template_dirty = true;
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
        self.fit_mode = project.fit_mode;
        self.rmc = shell::rmc::RmcState::from_project(project.rmc);
        if let Some(saved) = &self.rmc.project.saved {
            self.rmc.request = Some(saved.request.clone());
            self.rmc.live = Some(saved.progress.clone());
            self.rmc.phase = "Saved run · ready to resume".into();
            if self.fit_mode == crate::rmc_fitting::FitMode::Rmc {
                self.stage_view.fit_step = shell::fit_workspace::FitStep::Results;
            }
        }
        self.restore_rmc_plots(cx);
        self.fit_history = project.fit_history;
        self.joint = shell::joint_fit::JointState {
            config: project.joint,
            ..Default::default()
        };
        // Saved result arrays let Publish export history fits without a refit.
        self.fit_history_results = project.fit_results;
        // Show the fit that was current at save time again; its provenance is
        // attached when the fitted spectrum finishes processing.
        self.pending_fit_restore = None;
        if let Some((id, result)) = project
            .current_fit
            .and_then(|id| Some((id, self.fit_history_results.get(&id)?.clone())))
        {
            self.joint.result_config = self
                .fit_history
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| e.joint.clone());
            self.fit_result = Some(result);
            self.fit_history_selected = Some(id);
            self.pending_fit_restore = Some(id);
            self.rebuild_fit_plots(cx);
        }
        self.sync_range_fields(cx);
        self.derived = project.derived;
        self.group_registry.reserve_groups(&self.derived);
        self.pending_derived = project.active_derived;

        // Reopen the data source. Per-spectrum overrides wait path-keyed
        // until the (re)scanned catalog can resolve them to indices; with
        // no source there is no catalog for them to attach to.
        if let Some(dir) = project.source_dir.clone() {
            self.scan_folder(dir, true, cx);
            self.pending_overrides = project.overrides;
            self.pending_project_spectrum = project.spectrum_file;
        } else {
            self.reset_catalog_state(cx);
            self.source_dir = None;
            self.pending_overrides = project.overrides;
            self.pending_project_spectrum = project.spectrum_file.clone();
            let mut files = project.raw_files;
            files.extend(project.spectrum_file);
            if files.is_empty() {
                self.restore_project_selection(cx);
            } else {
                self.append_import(files, true, cx);
            }
        }
        let restored_current = self.selected.and_then(|ix| self.group_id(ix));
        self.group_state = project.group_state;
        self.group_state.current = self.group_state.current.take().or(restored_current);
        self.selected = None;
        self.resolve_group_state();
        self.ensure_compare_loaded(cx);
        if !self.fit_paths.is_empty() {
            self.set_fit_step(shell::fit_workspace::FitStep::Model, cx);
        }
        self.sync_param_fields(cx);
        self.status = "project loaded".into();
        if self.spectrum.is_some() {
            self.schedule_recompute(cx);
        }
        // A project opened from disk starts clean; recovered copies and other
        // projects without a file start unsaved.
        self.saved_fingerprint = None;
        self.settle_saved_fingerprint = false;
        if self.project_path.is_some() {
            self.mark_project_clean();
        }
        cx.notify();
    }

    /// Treat the current state as saved, and re-record it once restored
    /// sources finish loading (see `settle_saved_fingerprint`).
    pub(crate) fn mark_project_clean(&mut self) {
        self.saved_fingerprint = self.project_file().content_fingerprint();
        self.settle_saved_fingerprint = true;
    }

    /// Attach provenance to a fit result restored from a project. A joint fit
    /// waits until restored sources and overrides are resolved, then records
    /// the current model. A single-spectrum fit waits for a processed spectrum:
    /// if it is the fitted one (same group, label and processing fingerprint)
    /// the result is up to date; any other spectrum gets provisional
    /// provenance that marks the result as needing an update until the fitted
    /// spectrum is processed.
    fn settle_restored_fit(&mut self) {
        let Some(id) = self.pending_fit_restore else {
            return;
        };
        let Some(entry) = self.fit_history.iter().find(|e| e.id == id) else {
            self.pending_fit_restore = None;
            return;
        };
        if entry.joint.is_some() {
            if self.catalog.scanning
                || !self.pending_overrides.is_empty()
                || self.pending_project_spectrum.is_some()
                || self.pending_derived.is_some()
            {
                return;
            }
            self.fit_provenance = Some(FitProvenance {
                group_id: None,
                label: entry.group.clone().into(),
                path: PathBuf::new(),
                params_fingerprint: 0,
                model_fingerprint: self.fit_model_fingerprint(),
            });
            self.pending_fit_restore = None;
            return;
        }
        if self.spectrum.is_none() {
            return;
        }
        let matches = entry.matches_spectrum(
            self.active_group_id(),
            &self.spectrum_label,
            self.spectrum_fingerprint,
        );
        self.fit_provenance = Some(FitProvenance {
            group_id: entry.group_id,
            label: entry.group.clone().into(),
            path: if matches {
                self.spectrum_path.clone()
            } else {
                PathBuf::new()
            },
            params_fingerprint: entry.params_fingerprint.unwrap_or_default(),
            model_fingerprint: self.fit_model_fingerprint(),
        });
        if matches {
            self.pending_fit_restore = None;
        }
    }

    fn settle_saved_fingerprint(&mut self) {
        if self.settle_saved_fingerprint
            && !self.catalog.scanning
            && self.pending_overrides.is_empty()
            && self.pending_project_spectrum.is_none()
            && self.pending_derived.is_none()
        {
            self.settle_saved_fingerprint = false;
            self.saved_fingerprint = self.project_file().content_fingerprint();
        }
    }

    fn has_unsaved_changes(&self) -> bool {
        self.project_file()
            .has_unsaved_changes(self.saved_fingerprint)
    }

    /// Ask before closing the window or quitting with unsaved changes.
    /// Returns `true` when it is safe to proceed now; otherwise a native
    /// Save / Don't Save / Cancel prompt decides later. Save runs the normal
    /// save dialog and proceeds only after the file is written.
    pub(crate) fn confirm_close(
        &mut self,
        intent: CloseIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // A prompt or its save dialog is already open.
        if self.pending_close.is_some() {
            return false;
        }
        if !self.has_unsaved_changes() {
            self.close_assistant_session(cx);
            return true;
        }
        let message = match intent {
            CloseIntent::CloseWindow => "Save changes before closing?",
            CloseIntent::Quit => "Save changes before quitting?",
        };
        let answer = window.prompt(
            gpui::PromptLevel::Warning,
            message,
            Some("Your changes to this project will be lost if you don't save them."),
            &[
                gpui::PromptButton::ok("Save…"),
                gpui::PromptButton::new("Don't Save"),
                gpui::PromptButton::cancel("Cancel"),
            ],
            cx,
        );
        self.pending_close = Some(intent);
        cx.spawn(async move |this, cx| {
            let choice = answer.await.ok();
            this.update(cx, |app, cx| match choice {
                // An in-flight save also finishes the close when it succeeds.
                Some(0) => app.save_project_confirmed(cx),
                Some(1) => {
                    app.pending_close = None;
                    app.finish_close(intent, cx);
                }
                _ => app.pending_close = None,
            })
            .ok();
        })
        .detach();
        false
    }

    fn close_assistant_session(&mut self, cx: &mut Context<Self>) {
        if let Some(assistant) = self.assistant.clone() {
            assistant.update(cx, |assistant, cx| assistant.analysis_closed(cx));
        }
    }

    /// Close or quit without asking again.
    fn finish_close(&mut self, intent: CloseIntent, cx: &mut Context<Self>) {
        self.close_assistant_session(cx);
        match intent {
            CloseIntent::Quit => cx.quit(),
            CloseIntent::CloseWindow => {
                let main = self.main_window;
                cx.defer(move |cx| {
                    main.update(cx, |_, window, _| window.remove_window()).ok();
                });
            }
        }
    }

    // ---- views -------------------------------------------------------------

    fn set_workspace(&mut self, workspace: Workspace, cx: &mut Context<Self>) {
        self.workspace = workspace;
        if should_rebuild_explore(workspace, &mut self.explore_plots_dirty, false) {
            self.rebuild_explore_plots(cx);
        } else if workspace == Workspace::Operando {
            self.ensure_operando(cx);
            self.rebuild_operando_trend(cx);
        }
        cx.notify();
    }

    fn focus_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_side_panel_visible(shell::assistant_shell::SidePanel::Groups, true);
        let input = self.filter_input.clone();
        cx.notify();
        // The panel may have been collapsed. Defer focus until the notified
        // render has placed the TextInput back into the dispatch tree.
        cx.defer_in(window, move |_this, window, cx| {
            if let Some(input) = input {
                input.read(cx).focus_handle(cx).focus(window, cx);
                input.update(cx, |input, cx| input.select_all_text(cx));
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
        self.invalidate_explore_plots(cx);
        cx.notify();
    }

    fn explore_escape(&mut self, cx: &mut Context<Self>) {
        if self.param_context_menu.take().is_some() || self.param_menu.take().is_some() {
            cx.notify();
        } else if self.maximized.is_some() {
            self.set_maximized(None, cx);
        }
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
        // Only the divergent case carries information. Printing "global" on
        // every section repeated a redundant word four times down the panel
        // and made the one section that *had* diverged harder to spot.
        if !self.section_overridden(section) {
            return None;
        }
        let t = self.theme;
        Some(
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
                .into_any_element(),
        )
    }

    /// One shared legend strip for the whole 2x2 grid (color chip +
    /// middle-truncated name per overlaid trace, in stable selection order).
    fn legend_strip(&self) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut row = div()
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .gap_3()
            .text_xs()
            .text_color(t.text_muted);
        for (label, color) in &self.legend_entries {
            row = row.child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().w(px(10.)).h(px(10.)).rounded_xs().bg(*color))
                    .child(label.clone()),
            );
        }
        div()
            .id("legend-strip-horizontal")
            .w_full()
            .min_w_0()
            .overflow_x_scroll()
            .child(row)
    }

    /// Scrub strip: clickable segments mapping linearly onto the full scan.
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
                        "{scope} · {} / {} complete · {} stopped · {} errors · {state}",
                        batch.rows.len(),
                        batch.total,
                        batch
                            .rows
                            .iter()
                            .filter(|row| row.solver_report.as_ref().is_some_and(|r| !r.converged))
                            .count(),
                        batch.problems.len()
                    )
                    .into(),
                )
            })
            .unwrap_or_else(|| (0, Vec::new(), "Batch results".into()));
        let table_width = px(526. + names.len() as f32 * 104.);
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
                .child(div().w(px(96.)).child("optimizer"))
                .child(div().w(px(88.)).child("R-factor"))
                .child(div().w(px(112.)).child("red. chi²"));
            for name in header_names {
                row = row.child(div().w(px(104.)).child(SharedString::from(name)));
            }
            row
        };
        let has_parameters = !names.is_empty();
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
                    .child(
                        div()
                            .w(px(96.))
                            .text_color(
                                if row.solver_report.as_ref().is_some_and(|r| !r.converged) {
                                    t.warn
                                } else {
                                    t.text_muted
                                },
                            )
                            .child(match row.solver_report.as_ref() {
                                Some(r) if r.converged => "converged",
                                Some(_) => "stopped",
                                None => "unknown",
                            }),
                    )
                    .child(div().w(px(88.)).child(format!("{:.5}", row.r_factor)))
                    .child(div().w(px(112.)).child(match row.reduced_chi_square {
                        Some(value) => format!("{value:.4e}"),
                        None => "n/a".to_string(),
                    }));
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
            .flex_1()
            .min_h(px(120.))
            .when(
                self.stage_view.fit_step != shell::fit_workspace::FitStep::Results
                    || self.stage_view.fit_result_tab != 2,
                |d| d.max_h(px(260.)),
            )
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
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child(div().flex_1().child(summary))
                    .when(has_parameters, |row| {
                        row.child(shell::button(&t, "batch-cols-left", "←", false).on_click(
                            cx.listener(|this, _, _, cx| {
                                let handle = &this.batch_table_scroll;
                                let mut offset = handle.offset();
                                offset.x += px(240.);
                                handle.set_offset(offset);
                                cx.notify();
                            }),
                        ))
                        .child(
                            shell::button(&t, "batch-cols-right", "Columns →", false).on_click(
                                cx.listener(|this, _, _, cx| {
                                    let handle = &this.batch_table_scroll;
                                    let mut offset = handle.offset();
                                    offset.x -= px(240.);
                                    handle.set_offset(offset);
                                    cx.notify();
                                }),
                            ),
                        )
                    })
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
                    .track_scroll(&self.batch_table_scroll)
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_x_scroll()
                    .child(
                        div()
                            .w(table_width)
                            .flex_none()
                            .h_full()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .child(header)
                            .child(rows),
                    ),
            )
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

    /// Enum parameter row with a dropdown list (0 = auto).
    /// Choice row; its label and hover explanation come from `param_text`.
    pub(crate) fn enum_row(&self, which: EnumParam, cx: &mut Context<Self>) -> gpui::AnyElement {
        let t = self.theme;
        let param_text::ParamText { label, tip } = param_text::enum_text(which);
        let options = self.enum_options(which);
        let selected = self.enum_selected_index(which);
        let expanded = self.open_enum == Some(which);
        let current: SharedString = options[selected].clone().into();
        let mut col = div().flex().flex_col().child(
            div()
                .px_3()
                .py_0p5()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id(SharedString::from(format!("enum-label-{which:?}")))
                                .flex_1()
                                .text_sm()
                                .text_color(t.text_muted)
                                .when_some(tip, |d, tip| {
                                    d.tooltip(move |_, cx| {
                                        cx.new(|_| Tooltip {
                                            label: tip.into(),
                                            theme: t,
                                        })
                                        .into()
                                    })
                                })
                                .child(label),
                        )
                        .children(self.parameter_badge(
                            shell::parameter_actions::ParamScope::Field(which.setting_key()),
                            cx,
                        )),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("enum-{label}-{which:?}")))
                        .w_full()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .text_xs()
                        .bg(t.bg)
                        .border_1()
                        .border_color(if expanded { t.accent } else { t.border })
                        .text_color(t.text)
                        .cursor_pointer()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .hover(|d| d.border_color(t.accent))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                            this.open_enum = if this.open_enum == Some(which) {
                                None
                            } else {
                                Some(which)
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
                        .id(SharedString::from(format!(
                            "enum-opt-{label}-{which:?}-{i}"
                        )))
                        .px_2()
                        .py_0p5()
                        .text_xs()
                        .cursor_pointer()
                        .when(is_sel, |d| d.bg(t.raised).text_color(t.accent))
                        .when(!is_sel, |d| d.text_color(t.text))
                        .hover(|d| d.bg(t.raised))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                            this.set_enum_param(which, i, cx);
                        }))
                        .child(option),
                );
            }
            col = col.child(list);
        }
        self.parameter_context(
            shell::parameter_actions::ParamScope::Field(which.setting_key()),
            col.into_any_element(),
            cx,
        )
    }

    fn problems_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let visible: Vec<_> = self
            .job_errors
            .iter()
            .filter(|p| self.problems_batch.is_none() || p.batch == self.problems_batch)
            .cloned()
            .collect();
        let (errors, warnings) = problem_counts(&visible);
        const PAGE_SIZE: usize = 100;
        let source_count = self
            .problems_batch
            .map_or(0, |id| self.intake.history[id].sources.len());
        let pages = (source_count + visible.len()).div_ceil(PAGE_SIZE).max(1);
        let page = self.problems_page.min(pages - 1);
        let start = page * PAGE_SIZE;
        let mut list = div()
            .id("recent-problems-list")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll();
        if let Some(id) = self.problems_batch {
            let batch = &self.intake.history[id];
            list = list.child(div().px_3().py_1().child(format!(
                "Import #{} · {} · Full signal checks run when groups are loaded",
                id + 1,
                batch.receipt()
            )));
            for (path, outcome) in batch.sources.iter().skip(start).take(PAGE_SIZE) {
                list = list.child(div().px_3().py_1().child(format!(
                    "{} · {}",
                    path.display(),
                    outcome.summary()
                )));
            }
        }
        let shown_sources = source_count.saturating_sub(start).min(PAGE_SIZE);
        for (row, error) in visible
            .iter()
            .rev()
            .skip(start.saturating_sub(source_count))
            .take(PAGE_SIZE - shown_sources)
            .enumerate()
        {
            let (source, full_path) = self.problem_source(&error.label);
            let path_tip = full_path.map(|path| shell::controls::Tooltip {
                label: path.into(),
                theme: t,
            });
            list = list.child(
                div()
                    .px_3()
                    .py_1()
                    .border_b_1()
                    .border_color(t.border)
                    .text_xs()
                    .child(
                        div()
                            .id(("problem-source", row))
                            .text_color(if error.severity == ProblemSeverity::Error {
                                t.error
                            } else {
                                t.text_muted
                            })
                            .when_some(path_tip, |d, tip| {
                                d.tooltip(move |_, cx| cx.new(|_| tip.clone()).into())
                            })
                            .child(SharedString::from(format!(
                                "{}: {source}",
                                if error.severity == ProblemSeverity::Error {
                                    "Error"
                                } else {
                                    "Warning"
                                }
                            ))),
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
            .min_h_0()
            .min_w_0()
            .flex_none()
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
                    .gap_2()
                    .child(div().flex_1().child(format!(
                        "{} · {} · {}",
                        self.problems_batch.map_or_else(
                            || "Recent problems".into(),
                            |id| format!("Import #{} details", id + 1)
                        ),
                        crate::text::plural(errors, "error"),
                        crate::text::plural(warnings, "warning")
                    )))
                    .when(pages > 1, |header| {
                        header
                            .child(
                                shell::button(&t, "problems-prev", "Previous", false).on_click(
                                    cx.listener(move |app, _, _, cx| {
                                        app.problems_page = page.saturating_sub(1);
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(format!("{} / {pages}", page + 1))
                            .child(shell::button(&t, "problems-next", "Next", false).on_click(
                                cx.listener(move |app, _, _, cx| {
                                    app.problems_page = (page + 1).min(pages - 1);
                                    cx.notify();
                                }),
                            ))
                    })
                    .child(
                        shell::button(&t, "clear-problems", "Clear", false)
                            .description("Remove the recent problems; import details are kept")
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.job_errors.retain(|p| p.batch.is_some());
                                cx.notify();
                            })),
                    )
                    .child(
                        shell::controls::icon_button(
                            &t,
                            "close-problems",
                            crate::icons::Icon::Close,
                            "Close",
                            false,
                        )
                        .size(px(24.))
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _window, cx| {
                                this.problems_open = false;
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(list)
    }

    /// Banner shown when the selected entry failed to load and the plots are
    /// therefore showing a *different* spectrum than the one selected.
    fn stale_plots_banner(&self, _cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let t = self.theme;
        let stale = self.stale_plots.as_ref()?;
        let headline: SharedString = format!(
            "{} could not be loaded — plots still show {}",
            stale.requested, self.spectrum_label
        )
        .into();
        let detail = stale.message.clone();
        Some(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .bg(t.raised)
                .border_b_1()
                .border_color(t.error)
                .child(div().text_xs().text_color(t.error).child("⚠"))
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(t.error)
                        .child(headline),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child(detail),
                ),
        )
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let (errors, warnings) = problem_counts(&self.job_errors);
        let status_color = match self.status_kind() {
            StatusKind::Error => t.error,
            StatusKind::Success => t.success,
            StatusKind::Info => t.text_muted,
        };
        // Job and cache counters are developer metrics (REXAFS_DEBUG_STATS=1).
        let debug = crate::debug_stats::enabled().then(|| {
            format!(
                "jobs:{} cache:{}/{}",
                self.running_job_count(),
                self.cache.len(),
                PROCESSED_CACHE_CAPACITY
            )
        });
        let mut bar = div()
            .h(px(28.))
            .w_full()
            .min_w_0()
            .flex_none()
            .flex()
            .items_center()
            .px_3()
            .gap_3()
            .bg(t.surface)
            .border_t_1()
            .border_color(t.border)
            .text_xs()
            .text_color(t.text_muted)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_color(status_color)
                    .child(self.status.clone()),
            )
            .children(debug)
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
                        this.problems_batch = None;
                        this.problems_open = !this.problems_open;
                        cx.notify();
                    }))
                    .child(format!(
                        "{} · {}",
                        crate::text::plural(errors, "error"),
                        crate::text::plural(warnings, "warning")
                    )),
            );
        if self.catalog.scanning
            || self.verify_running
            || self.operando_running
            || self.batch_running
            || self.merge_running
        {
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
                .id("journal-toggle")
                .px_2()
                .rounded_sm()
                .cursor_pointer()
                .hover(|d| d.bg(t.raised))
                .text_color(if self.journal.open {
                    t.accent
                } else {
                    t.text_muted
                })
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.journal.open = !this.journal.open;
                    cx.notify();
                }))
                .child(format!("journal · {} steps", self.journal.entries.len())),
        )
        .child(
            div()
                .font_family(shell::MONO)
                .whitespace_nowrap()
                .child(format!(
                    "current: {} · {} marked",
                    self.current_group_label(),
                    self.selection.len()
                )),
        )
    }
}

impl Render for StudioApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.settle_saved_fingerprint();
        self.settle_restored_fit();
        if let Some(source) = self.measurement_import.take() {
            self.open_measurement_editor(source, window, cx);
        }
        self.viewport_w = f32::from(window.viewport_size().width);
        self.viewport_h = f32::from(window.viewport_size().height);
        let blocked = self.import_editor.is_some()
            || self.path_route.is_some()
            || self.updates.open
            || self.ui.menu.is_some()
            || self.help.is_open()
            || self.palette.is_some();
        if self.ui.native_menu_blocked != Some(blocked) {
            crate::native_menus(blocked, cx);
            self.ui.native_menu_blocked = Some(blocked);
        }
        self.fit_assistant_layout();
        let key_context = if self.import_editor.is_some() {
            "ImportEditor"
        } else if self.path_route.is_some() {
            "PathRoute"
        } else if self.updates.open {
            "UpdateDialog"
        } else if self.ui.menu.is_some() {
            "StudioPopover"
        } else if self.help.is_open() {
            "HelpDialog"
        } else if self.palette.is_some() {
            "Palette"
        } else {
            match self.workspace {
                Workspace::Explore => "Studio Explore",
                Workspace::Operando => "Studio OperandoWorkspace",
                Workspace::Fit => "Studio FitWorkspace",
            }
        };
        crate::accessibility::root(
            div()
                .id("root")
                .track_focus(&self.root_focus)
                .key_context(key_context)
                .tab_group()
                .on_key_down(|event, window, cx| {
                    shell::controls::navigate(event, window, cx);
                })
                .border_2()
                .border_color(self.theme.bg)
                .drag_over::<ExternalPaths>({
                    let accent = self.theme.accent;
                    move |style, _, _, _| style.border_color(accent)
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                    this.route_paths(paths.paths().to_vec(), false, cx);
                }))
                .on_action(cx.listener(|this, _: &StagePublish, window, cx| {
                    this.navigate_stage(Stage::Publish, window, cx)
                }))
                .on_action(cx.listener(|this, _: &ShowHelp, _, cx| this.open_help(cx)))
                .on_action(cx.listener(|this, _: &ShowLicenses, _, cx| this.open_licenses(cx)))
                .on_action(cx.listener(|this, _: &ShowStorage, _, cx| this.open_storage(cx)))
                .on_action(cx.listener(|this, _: &ShowUpdates, _, cx| this.open_updates(cx)))
                .on_action(cx.listener(|this, _: &ShowExample, _, cx| this.open_example(cx)))
                .on_action(
                    cx.listener(|this, _: &ShowCopperSeries, _, cx| this.open_copper_series(cx)),
                )
                .on_action(cx.listener(|this, _: &SwitchTheme, _, cx| this.toggle_theme(cx)))
                .on_action(cx.listener(|this, _: &OpenProject, _, cx| this.open_project(cx)))
                .on_action(cx.listener(|this, _: &SaveProject, _, cx| this.save_project(cx)))
                .on_action(cx.listener(|this, _: &ImportPaths, _, cx| this.open_folder(cx)))
                .on_action(cx.listener(|this, _: &DismissPathRoute, window, cx| {
                    this.dismiss_path_route(window, cx);
                }))
                .on_action(cx.listener(|this: &mut Self, _: &StageData, window, cx| {
                    this.navigate_stage(Stage::Data, window, cx);
                }))
                .on_action(
                    cx.listener(|this: &mut Self, _: &StageProcess, window, cx| {
                        this.navigate_stage(Stage::Process, window, cx);
                    }),
                )
                .on_action(
                    cx.listener(|this: &mut Self, _: &StageNormalize, window, cx| {
                        let section = shell::ProcessSection::Normalize;
                        this.navigate_process_section(section, window, cx);
                    }),
                )
                .on_action(
                    cx.listener(|this: &mut Self, _: &StageBackground, window, cx| {
                        let section = shell::ProcessSection::Background;
                        this.navigate_process_section(section, window, cx);
                    }),
                )
                .on_action(
                    cx.listener(|this: &mut Self, _: &StageTransform, window, cx| {
                        let section = shell::ProcessSection::Transform;
                        this.navigate_process_section(section, window, cx);
                    }),
                )
                .on_action(cx.listener(|this: &mut Self, _: &StageFit, window, cx| {
                    this.navigate_stage(Stage::Fit, window, cx);
                }))
                .on_action(cx.listener(|this: &mut Self, _: &StageSeries, window, cx| {
                    this.navigate_stage(Stage::Series, window, cx);
                }))
                .on_action(
                    cx.listener(|this: &mut Self, _: &ToggleDataPanel, _window, cx| {
                        this.toggle_side_panel(shell::assistant_shell::SidePanel::Groups);
                        cx.notify();
                    }),
                )
                .on_action(
                    cx.listener(|this: &mut Self, _: &ToggleContextPanel, _window, cx| {
                        this.toggle_side_panel(shell::assistant_shell::SidePanel::Inspector);
                        cx.notify();
                    }),
                )
                .on_action(cx.listener(|this: &mut Self, _: &FocusFilter, window, cx| {
                    this.focus_filter(window, cx);
                }))
                .on_action(
                    cx.listener(|this: &mut Self, _: &ExploreEscape, _window, cx| {
                        this.explore_escape(cx);
                    }),
                )
                .on_action(cx.listener(|this: &mut Self, _: &PaletteOpen, window, cx| {
                    this.open_palette(window, cx);
                }))
                .on_action(cx.listener(|this: &mut Self, _: &Undo, _window, cx| {
                    this.undo(cx);
                }))
                .on_action(cx.listener(|this: &mut Self, _: &Redo, _window, cx| {
                    this.redo(cx);
                }))
                .on_action(
                    cx.listener(|this: &mut Self, _: &JournalToggle, _window, cx| {
                        this.journal.open = !this.journal.open;
                        cx.notify();
                    }),
                )
                .size_full()
                .min_h_0()
                .min_w_0()
                .child(self.shell_root(cx)),
            "rexafs workspace",
        )
    }
}
