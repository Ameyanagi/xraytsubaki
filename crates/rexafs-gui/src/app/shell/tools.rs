//! Processing tools (Athena's Data menu) as non-destructive inline forms:
//! every tool reads the current group and produces a *derived* group, so
//! the source is never mutated and nothing needs undo.

use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use gpui::{
    ClickEvent, Context, Entity, IntoElement, ParentElement, SharedString, Styled, div, prelude::*,
    px, uniform_list,
};
use rexafs::prelude::XASSpectrum;
use rexafs::xafs::tools::{EdgeFeature, RebinConfig};
use rexafs::xafs::xafsutils::ConvolveForm;

use rexafs::prelude::{AnalysisSpace, LcfConfig, PcaConfig};

use super::button;
use crate::app::{DERIVED_BASE, NO_ENTRY, StudioApp, filter_match_lower};
use crate::params::{DerivedSpectrum, Operation, OperationInput, PipelineParams, Quantity};
use crate::widgets::numeric_field::{FieldEvent, FieldKind, NumericField};
use crate::widgets::text_input::{InputEvent, TextInput};

mod analysis_range;
mod collection;
mod mcr;
mod mcr_reference;
pub(super) mod result_groups;

/// Match the group panel's stable order: additional groups, then files.
pub(crate) fn marked_group_indices(marks: &BTreeSet<usize>) -> impl Iterator<Item = usize> + '_ {
    marks
        .range(DERIVED_BASE..NO_ENTRY)
        .chain(marks.range(..DERIVED_BASE))
        .copied()
}

fn analysis_marks(
    marks: &BTreeSet<usize>,
    current: Option<&crate::group_identity::GroupId>,
    identity: impl Fn(usize) -> Option<crate::group_identity::GroupId>,
) -> BTreeSet<usize> {
    marks
        .iter()
        .copied()
        .filter(|&ix| current.is_none() || identity(ix).as_ref() != current)
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Align,
    Calibrate,
    Deglitch,
    Truncate,
    Rebin,
    Smooth,
    Difference,
    Lcf,
    Pca,
    Mcr,
}

impl Tool {
    /// Processing tools (each Apply creates a derived group).
    pub const PROCESSING: [Tool; 7] = [
        Tool::Align,
        Tool::Calibrate,
        Tool::Deglitch,
        Tool::Truncate,
        Tool::Rebin,
        Tool::Smooth,
        Tool::Difference,
    ];

    /// Analysis tools (results, not new groups).
    pub const ANALYSIS: [Tool; 3] = [Tool::Lcf, Tool::Pca, Tool::Mcr];

    pub fn is_analysis(self) -> bool {
        matches!(self, Tool::Lcf | Tool::Pca | Tool::Mcr)
    }

    /// The analysis result card to show after opening this tool. Processing
    /// tools such as Deglitch and Align show the spectra or their own
    /// preview, never a previous LCF, PCA or MCR result.
    fn shown_analysis(self) -> Option<Tool> {
        self.is_analysis().then_some(self)
    }

    fn needs_standard(self) -> bool {
        matches!(self, Tool::Align | Tool::Calibrate | Tool::Difference)
    }

    pub fn name(self) -> &'static str {
        match self {
            Tool::Align => "Align to reference",
            Tool::Calibrate => "Calibrate energy",
            Tool::Deglitch => "Deglitch",
            Tool::Truncate => "Truncate",
            Tool::Rebin => "Rebin",
            Tool::Smooth => "Smooth",
            Tool::Difference => "Difference spectrum",
            Tool::Lcf => "Linear combination fit",
            Tool::Pca => "Principal components",
            Tool::Mcr => "Resolve components (MCR-ALS)",
        }
    }

    /// One-sentence description shown as the first line of the open tool form
    /// and as the tooltip of its disclosure.
    pub fn hint(self) -> &'static str {
        match self {
            Tool::Align => "Shift onto a named alignment standard.",
            Tool::Calibrate => "Apply the named standard’s measured energy shift.",
            Tool::Deglitch => "Remove the points inside an energy range.",
            Tool::Truncate => "Keep the points between two energies.",
            Tool::Rebin => "Rebin onto a 10 eV / 0.5 eV / 0.05 Å⁻¹ grid.",
            Tool::Smooth => "Smooth μ(E) with a Gaussian convolution.",
            Tool::Difference => "Subtract a named baseline from the normalized target.",
            Tool::Lcf => "Fit the current group as a mix of the marked standards.",
            Tool::Pca => "Principal components of the marked groups.",
            Tool::Mcr => "Estimate component spectra and fractions from the marked groups.",
        }
    }

    /// What applying the tool does to the group list; Align is the only tool
    /// that changes the current group in place.
    pub fn effect(self) -> &'static str {
        match self {
            Tool::Align => "Edits the current group",
            Tool::Lcf | Tool::Pca | Tool::Mcr => "Shows a result below",
            _ => "Creates a new group",
        }
    }

    fn fields(self) -> &'static [ToolField] {
        match self {
            Tool::Align => &[ToolField::WinLo, ToolField::WinHi, ToolField::ManualShift],
            Tool::Calibrate => &[ToolField::Target],
            Tool::Deglitch => &[ToolField::ELo, ToolField::EHi],
            Tool::Truncate => &[ToolField::Before, ToolField::After],
            Tool::Rebin => &[ToolField::PreStep, ToolField::XanesStep, ToolField::KStep],
            Tool::Smooth => &[ToolField::Sigma],
            Tool::Difference => &[],
            Tool::Lcf => &[ToolField::RangeLo, ToolField::RangeHi],
            Tool::Pca => &[
                ToolField::RangeLo,
                ToolField::RangeHi,
                ToolField::Components,
            ],
            Tool::Mcr => &[
                ToolField::McrRangeLo,
                ToolField::McrRangeHi,
                ToolField::McrComponents,
                ToolField::McrIterations,
                ToolField::McrSeed,
            ],
        }
    }

    fn range_fields(self) -> (ToolField, ToolField) {
        if self == Tool::Mcr {
            (ToolField::McrRangeLo, ToolField::McrRangeHi)
        } else {
            (ToolField::RangeLo, ToolField::RangeHi)
        }
    }

    fn apply_label(self) -> &'static str {
        match self {
            Tool::Align => "Apply offset",
            Tool::Lcf => "Fit",
            Tool::Pca => "Run PCA",
            Tool::Mcr => "Resolve components",
            _ => "Create group",
        }
    }
}

/// Why MCR-ALS stopped, in plain words for the result table and status line.
fn mcr_termination_label(termination: rexafs::prelude::McrTermination) -> &'static str {
    use rexafs::prelude::McrTermination;
    match termination {
        McrTermination::Converged => "Converged",
        McrTermination::IterationLimit => "Iteration limit reached",
        McrTermination::Cancelled => "Cancelled",
        McrTermination::Stalled => "Stalled",
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolField {
    WinLo,
    WinHi,
    ManualShift,
    Target,
    ELo,
    EHi,
    Before,
    After,
    PreStep,
    XanesStep,
    KStep,
    Sigma,
    RangeLo,
    RangeHi,
    McrRangeLo,
    McrRangeHi,
    Components,
    McrIterations,
    McrSeed,
    McrComponents,
}

/// Blank endpoints select defaults only when both are blank.
fn analysis_range_from_fields(
    lo: Option<f64>,
    hi: Option<f64>,
) -> Result<Option<(f64, f64)>, String> {
    match (lo, hi) {
        (None, None) => Ok(None),
        (Some(lo), Some(hi)) => {
            rexafs::xafs::analysis::validate_range(lo, hi).map_err(|e| e.to_string())?;
            Ok(Some((lo, hi)))
        }
        _ => Err("Enter both range endpoints, or clear both for the default interval".into()),
    }
}

#[cfg(test)]
mod analysis_range_tests {
    use super::analysis_range_from_fields;
    #[test]
    fn invalid_explicit_ranges_never_fall_back_to_defaults() {
        assert_eq!(analysis_range_from_fields(None, None).unwrap(), None);
        assert_eq!(
            analysis_range_from_fields(Some(-20.), Some(30.)).unwrap(),
            Some((-20., 30.))
        );
        for (lo, hi) in [
            (Some(10.), None),
            (None, Some(10.)),
            (Some(10.), Some(-10.)),
            (Some(0.), Some(0.)),
            (Some(f64::NAN), Some(1.)),
        ] {
            assert!(analysis_range_from_fields(lo, hi).is_err());
        }
    }

    #[test]
    fn processing_tools_never_show_a_previous_analysis_result() {
        use super::Tool;
        for tool in [Tool::Deglitch, Tool::Align, Tool::Calibrate, Tool::Smooth] {
            assert_eq!(tool.shown_analysis(), None);
        }
        for tool in Tool::ANALYSIS {
            assert_eq!(tool.shown_analysis(), Some(tool));
        }
    }
}

impl ToolField {
    const ALL: [ToolField; 20] = [
        ToolField::WinLo,
        ToolField::WinHi,
        ToolField::ManualShift,
        ToolField::Target,
        ToolField::ELo,
        ToolField::EHi,
        ToolField::Before,
        ToolField::After,
        ToolField::PreStep,
        ToolField::XanesStep,
        ToolField::KStep,
        ToolField::Sigma,
        ToolField::RangeLo,
        ToolField::RangeHi,
        ToolField::McrRangeLo,
        ToolField::McrRangeHi,
        ToolField::Components,
        ToolField::McrIterations,
        ToolField::McrSeed,
        ToolField::McrComponents,
    ];

    fn spec(self) -> (&'static str, &'static str, Option<f64>) {
        match self {
            ToolField::WinLo => ("Window start (eV rel. E₀)", "-50", Some(-50.0)),
            ToolField::WinHi => ("Window end (eV rel. E₀)", "100", Some(100.0)),
            ToolField::ManualShift => ("Manual shift (eV)", "0", Some(0.0)),
            ToolField::Target => ("Target E₀ (eV)", "e.g. 22117", None),
            ToolField::ELo => ("From (eV)", "energy", None),
            ToolField::EHi => ("To (eV)", "energy", None),
            ToolField::Before => ("Keep from (eV)", "auto (start)", None),
            ToolField::After => ("Keep to (eV)", "auto (end)", None),
            ToolField::PreStep => ("Pre-edge step (eV)", "10", Some(10.0)),
            ToolField::XanesStep => ("XANES step (eV)", "0.5", Some(0.5)),
            ToolField::KStep => ("EXAFS step (Å⁻¹)", "0.05", Some(0.05)),
            ToolField::Sigma => ("Sigma (eV)", "1.0", Some(1.0)),
            ToolField::RangeLo => ("Range start (rel. E₀)", "auto (−20)", None),
            ToolField::RangeHi => ("Range end (rel. E₀)", "auto (+30)", None),
            ToolField::McrRangeLo => ("Range start (rel. E₀)", "auto (common start)", None),
            ToolField::McrRangeHi => ("Range end (rel. E₀)", "auto (common end)", None),
            ToolField::Components => ("Components", "2", Some(2.0)),
            ToolField::McrIterations => ("Maximum iterations", "500", Some(500.0)),
            ToolField::McrSeed => ("Initialization seed", "0", Some(0.0)),
            ToolField::McrComponents => ("Components", "3", Some(3.0)),
        }
    }
}

/// Results of the analysis tools (LCF / PCA) for the current group.
#[derive(Default)]
pub struct AnalysisState {
    pub lcf_series: Option<crate::project::LcfSeriesAnalysis>,
    pub lcf: Option<rexafs::prelude::LcfResult>,
    pub lcf_config: Option<LcfConfig>,
    pub(super) lcf_sources: Vec<ToolTarget>,
    pub lcf_inputs: Vec<crate::project::AnalysisInput>,
    /// Ranked combinations ("fit all combinations"), best first.
    pub ranked: Vec<rexafs::prelude::LcfResult>,
    pub pca: Option<rexafs::prelude::PcaModel>,
    pub pca_fit: Option<rexafs::prelude::PcaFit>,
    pub pca_view: crate::plotting::analysis::PcaView,
    pub pca_all_components: bool,
    pub pca_scale: crate::plotting::analysis::PcaScale,
    pub mcr_view: crate::plotting::analysis::McrView,
    pub(super) pca_sources: Vec<ToolTarget>,
    pub pca_inputs: Vec<crate::project::AnalysisInput>,
    pub mcr: Option<crate::project::McrAnalysis>,
    pub(super) mcr_sources: Vec<ToolTarget>,
    pub mcr_cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
    mcr_generation: u64,
    pub(super) collection_generation: u64,
    pub(super) collection_loading: bool,
    /// Which tool the center plot shows.
    pub shown: Option<Tool>,
    pub plot: Option<Entity<ruviz_gpui::RuvizPlot>>,
}

#[derive(Default)]
pub struct ToolState {
    pub open: Option<Tool>,
    pub fields: Vec<(ToolField, Entity<NumericField>)>,
    pub message: SharedString,
    preview_request: u64,
    preview_key: Option<ToolPreviewKey>,
    pub(super) preview_plot: Option<Entity<ruviz_gpui::RuvizPlot>>,
    pub(super) preview_message: String,
    pub(super) preview_running: bool,
    preview_error: Option<String>,

    target: Option<ToolTarget>,
    standard: Option<ToolTarget>,
    pub(crate) alignment_standard: Option<crate::group_identity::GroupId>,
    standard_load: StandardLoad,
    standard_request: u64,
    standard_picker_open: bool,
    generation: u64,
    standard_filter: Option<Entity<TextInput>>,
    standard_filter_request: u64,
    standard_matches: Option<Arc<Vec<usize>>>,
    /// LCF / PCA options (shared by the Data-stage tools and the Series
    /// LCF trend).
    pub lcf_space: LcfSpaceChoice,
    pub lcf_sum_to_one: bool,
    pub lcf_e0_shift: bool,
    pub lcf_all_combinations: bool,
    pub lcf_range: Option<(f64, f64)>,
    pub pca_components: usize,
    pub pca_center: bool,
    pub mcr_nonnegative_spectra: bool,
}

/// The preview and Apply use the same operation on a private spectrum copy.
fn process_tool(
    tool: Tool,
    source: &XASSpectrum,
    standard: Option<(&str, &XASSpectrum)>,
    name: &str,
    values: &[(ToolField, Option<f64>)],
    inputs: Vec<OperationInput>,
) -> Result<(XASSpectrum, Operation, String), String> {
    let mut sp = source.clone();
    let value = |field| {
        values
            .iter()
            .find(|(f, _)| *f == field)
            .and_then(|(_, v)| *v)
    };
    let mut operation = Operation {
        tool: tool.name().into(),
        parameters: serde_json::json!({}),
        inputs,
        applied_energy_shift_ev: 0.0,
    };
    let result: Result<String, String> = (|| {
        let label = match tool {
            Tool::Align => {
                let (ref_name, reference) = standard.ok_or("Choose a standard")?;
                let lo = value(ToolField::WinLo).unwrap_or(-50.0);
                let hi = value(ToolField::WinHi).unwrap_or(100.0);
                let manual = value(ToolField::ManualShift).unwrap_or(0.0);
                if !lo.is_finite() || !hi.is_finite() || lo >= hi {
                    return Err("Enter an increasing, finite alignment window".into());
                }
                if !manual.is_finite() {
                    return Err("Manual shift must be finite".into());
                }
                let automatic = sp
                    .align_to(reference, (lo, hi))
                    .map_err(|e| e.to_string())?;
                sp.shift_energy(manual);
                let shift = automatic + manual;
                operation.parameters = serde_json::json!({
                    "window_relative_e0_ev": [lo, hi],
                    "automatic_shift_ev": automatic,
                    "manual_shift_ev": manual
                });
                operation.applied_energy_shift_ev = shift;
                format!("align: {name} → {ref_name} ({shift:+.2} eV)")
            }
            Tool::Calibrate => {
                let target = value(ToolField::Target).ok_or("Enter the target E₀")?;
                let (ref_name, reference) = standard.ok_or("Choose a standard")?;
                let shift = calibrate_from_standard(&mut sp, reference, target)?;
                operation.parameters = serde_json::json!({"expected_energy_ev": target, "measured_energy_ev": target - shift, "feature": "DerivativeMax"});
                operation.applied_energy_shift_ev = shift;
                format!("calibrate: {name} via {ref_name} → {target:.1} eV ({shift:+.2})")
            }
            Tool::Deglitch => {
                let lo = value(ToolField::ELo).ok_or("Enter a range")?;
                let hi = value(ToolField::EHi).ok_or("Enter a range")?;
                let n = sp.deglitch_range(lo, hi).map_err(|e| e.to_string())?;
                operation.parameters =
                    serde_json::json!({"range_ev": [lo, hi], "removed_points": n});
                format!("deglitch: {name} (−{n} pts)")
            }
            Tool::Truncate => {
                let before = value(ToolField::Before);
                let after = value(ToolField::After);
                sp.truncate(before, after).map_err(|e| e.to_string())?;
                operation.parameters =
                    serde_json::json!({"keep_from_ev": before, "keep_to_ev": after});
                format!("truncate: {name}")
            }
            Tool::Rebin => {
                let cfg = RebinConfig {
                    e0: sp.e0(),
                    pre_step: value(ToolField::PreStep).unwrap_or(10.0),
                    xanes_step: value(ToolField::XanesStep).unwrap_or(0.5),
                    exafs_kstep: value(ToolField::KStep).unwrap_or(0.05),
                    ..RebinConfig::default()
                };
                sp.rebin(&cfg).map_err(|e| e.to_string())?;
                operation.parameters = serde_json::json!(cfg);
                format!("rebin: {name}")
            }
            Tool::Smooth => {
                let sigma = value(ToolField::Sigma).unwrap_or(1.0);
                sp.smooth_mu(ConvolveForm::Gaussian, Some(sigma), None)
                    .map_err(|e| e.to_string())?;
                operation.parameters = serde_json::json!({"form": "Gaussian", "sigma_ev": sigma});
                format!("smooth: {name} (σ {sigma:.2} eV)")
            }
            Tool::Lcf | Tool::Pca | Tool::Mcr => {
                return Err("Use the analysis action for this tool".into());
            }
            Tool::Difference => {
                operation.parameters = serde_json::json!({"space": Quantity::NormalizedMu});
                let (ref_name, reference) = standard.ok_or("Choose a standard")?;
                sp = rexafs::xafs::tools::difference(
                    &sp,
                    reference,
                    rexafs::xafs::tools::DiffSpace::Norm,
                )
                .map_err(|e| e.to_string())?;
                format!("diff: {name} − {ref_name}")
            }
        };
        Ok(label)
    })();
    result.map(|label| (sp, operation, label))
}

#[derive(Clone, PartialEq)]
struct ToolPreviewKey {
    tool: Tool,
    target: ToolTarget,
    standard: Option<ToolTarget>,
    values: Vec<(ToolField, Option<f64>)>,
}

/// Session-local identity: indices alone can be reused after a catalog walk
/// or a derived-group removal. Keep source locator and derived id as well.
#[derive(Clone, Debug, Eq)]
pub(crate) struct ToolTarget {
    pub group_id: Option<crate::group_identity::GroupId>,
    pub ix: usize,
    pub fingerprint: u64,
    pub label: String,
    pub path: PathBuf,
    pub derived_id: Option<u64>,
    pub project_generation: u64,
    pub catalog_generation: u64,
    pub size: Option<u64>,
}

// Display metadata is not part of operand readiness or scientific identity.
impl PartialEq for ToolTarget {
    fn eq(&self, other: &Self) -> bool {
        self.group_id == other.group_id
            && self.ix == other.ix
            && self.fingerprint == other.fingerprint
            && self.path == other.path
            && self.derived_id == other.derived_id
            && self.project_generation == other.project_generation
            && self.catalog_generation == other.catalog_generation
            && self.size == other.size
    }
}

impl ToolTarget {
    pub(crate) fn operation_input(&self) -> OperationInput {
        OperationInput {
            group_id: self.group_id.clone(),
            label: self.label.clone(),
            path: self.path.clone(),
            derived_id: self.derived_id,
            fingerprint: self.fingerprint,
            size: self.size,
        }
    }

    pub(crate) fn standalone(
        group_id: Option<crate::group_identity::GroupId>,
        path: PathBuf,
        label: String,
        fingerprint: u64,
        project_generation: u64,
        catalog_generation: u64,
    ) -> Self {
        Self {
            group_id,
            ix: NO_ENTRY,
            path,
            label,
            fingerprint,
            derived_id: None,
            project_generation,
            catalog_generation,
            size: None,
        }
    }
}

#[derive(Default)]
enum StandardLoad {
    #[default]
    Loading,
    Ready(Arc<XASSpectrum>),
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadinessReason {
    Quantity,
    NoTarget,
    CurrentFailed,
    TargetChanged,
    CurrentLoading,
    LoadedMismatch,
    NoStandard,
    StandardChanged,
    StandardLoading,
    StandardFailed,
}

impl ReadinessReason {
    fn message(self) -> &'static str {
        match self {
            Self::Quantity => {
                "Tool requires confirmed absorption μ(E) inputs; this quantity is for plotting/export only"
            }
            Self::NoTarget => "No target group; select a group and reopen the tool",
            Self::CurrentFailed => "Current group failed to load",
            Self::TargetChanged => "Target group or parameters changed; reopen the tool",
            Self::CurrentLoading => "Current group not loaded yet",
            Self::LoadedMismatch => "Loaded spectrum does not match the target group/revision",
            Self::NoStandard => "Choose a standard",
            Self::StandardChanged => "Standard group or parameters changed; choose it again",
            Self::StandardLoading => "Standard not loaded yet",
            Self::StandardFailed => "Standard failed to load; choose it again to retry",
        }
    }
}

/// Shared by the card and Apply, including analysis tools' current input.
/// An optional standard result means this tool requires an explicit operand.
fn readiness(
    target: Option<&ToolTarget>,
    current: Option<&ToolTarget>,
    loaded: Option<&ToolTarget>,
    failed: bool,
    loading: bool,
    standard: Option<Result<(), ReadinessReason>>,
) -> Result<(), ReadinessReason> {
    let target = target.ok_or(ReadinessReason::NoTarget)?;
    if loading {
        return Err(ReadinessReason::CurrentLoading);
    }
    if failed {
        return Err(ReadinessReason::CurrentFailed);
    }
    if current != Some(target) {
        return Err(ReadinessReason::TargetChanged);
    }
    if loaded != Some(target) {
        return Err(ReadinessReason::LoadedMismatch);
    }
    standard.unwrap_or(Ok(()))
}

fn quantity_readiness(
    identity: Result<(), ReadinessReason>,
    quantity: impl FnOnce() -> Result<(), ReadinessReason>,
) -> Result<(), ReadinessReason> {
    identity?;
    quantity()
}

fn default_standard(
    tool: Tool,
    target: Option<&ToolTarget>,
    groups: impl IntoIterator<Item = ToolTarget>,
    marked: &std::collections::BTreeSet<usize>,
) -> Option<ToolTarget> {
    groups
        .into_iter()
        .find(|group| Some(group.ix) != target.map(|t| t.ix) && marked.contains(&group.ix))
        .or_else(|| target.filter(|_| tool == Tool::Calibrate).cloned())
}

fn standard_readiness(
    chosen: Option<&ToolTarget>,
    current: Option<&ToolTarget>,
    load: &StandardLoad,
) -> Result<(), ReadinessReason> {
    let chosen = chosen.ok_or(ReadinessReason::NoStandard)?;
    if current != Some(chosen) {
        return Err(ReadinessReason::StandardChanged);
    }
    match load {
        StandardLoad::Loading => Err(ReadinessReason::StandardLoading),
        StandardLoad::Failed(_) => Err(ReadinessReason::StandardFailed),
        StandardLoad::Ready(_) => Ok(()),
    }
}

fn standard_row_index(
    matches: Option<&[usize]>,
    row: usize,
    derived: usize,
    files: usize,
) -> Option<usize> {
    match matches {
        Some(matches) => matches
            .get(row)
            .copied()
            .filter(|&ix| ix < files || (ix >= DERIVED_BASE && ix - DERIVED_BASE < derived)),
        None if row < derived => Some(DERIVED_BASE + row),
        None if row - derived < files => Some(row - derived),
        None => None,
    }
}

fn materialize_tool_output(
    tool: Tool,
    label: String,
    sp: &XASSpectrum,
    params: &PipelineParams,
    operation: Operation,
) -> Result<DerivedSpectrum, String> {
    let (Some(energy), Some(mu)) = (&sp.energy, &sp.mu) else {
        return Err("Tool produced no data".into());
    };
    let inherited = params.for_materialized(operation.applied_energy_shift_ev);
    Ok(DerivedSpectrum {
        label,
        energy: energy.iter().copied().collect(),
        mu: mu.iter().copied().collect(),
        params: Some(inherited),
        quantity: if tool == Tool::Difference {
            Quantity::NormalizedDifference
        } else {
            Quantity::RawMu
        },
        operation: Some(operation),
        ..Default::default()
    })
}

fn aligned_parameters(
    params: &PipelineParams,
    operation: &Operation,
) -> Result<PipelineParams, String> {
    let mut after = params.clone();
    after.set_energy_offset(params.energy_offset_ev + operation.applied_energy_shift_ev)?;
    let number = |key| {
        operation.parameters[key]
            .as_f64()
            .ok_or_else(|| format!("Missing alignment {key}"))
    };
    let window = serde_json::from_value(operation.parameters["window_relative_e0_ev"].clone())
        .map_err(|e| format!("Invalid alignment window: {e}"))?;
    after.alignment_record = Some(crate::params::AlignmentRecord {
        inputs: operation.inputs.clone(),
        window_relative_e0_ev: window,
        automatic_shift_ev: number("automatic_shift_ev")?,
        manual_shift_ev: number("manual_shift_ev")?,
        offset_before_ev: params.energy_offset_ev,
        offset_after_ev: after.energy_offset_ev,
    });
    Ok(after)
}

fn calibrate_from_standard(
    target: &mut XASSpectrum,
    standard: &XASSpectrum,
    expected: f64,
) -> Result<f64, String> {
    let measured = standard
        .edge_feature_energy(EdgeFeature::DerivativeMax)
        .map_err(|e| e.to_string())?;
    let shift = expected - measured;
    target.shift_energy(shift);
    Ok(shift)
}

/// Space an LCF / PCA runs in (the χ variant uses the plot k-weight).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LcfSpaceChoice {
    Norm,
    #[default]
    Flat,
    Deriv,
    Chi,
}

impl LcfSpaceChoice {
    pub const ALL: [LcfSpaceChoice; 4] = [
        LcfSpaceChoice::Norm,
        LcfSpaceChoice::Flat,
        LcfSpaceChoice::Deriv,
        LcfSpaceChoice::Chi,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LcfSpaceChoice::Norm => "norm",
            LcfSpaceChoice::Flat => "flat",
            LcfSpaceChoice::Deriv => "dμ/dE",
            LcfSpaceChoice::Chi => "χ(k)",
        }
    }

    pub fn space(self, kweight: f64) -> AnalysisSpace {
        match self {
            LcfSpaceChoice::Norm => AnalysisSpace::Norm,
            LcfSpaceChoice::Flat => AnalysisSpace::Flat,
            LcfSpaceChoice::Deriv => AnalysisSpace::Deriv,
            LcfSpaceChoice::Chi => AnalysisSpace::Chi { kweight },
        }
    }
}

impl ToolState {
    pub(crate) fn set_theme(&self, theme: crate::theme::Theme, cx: &mut gpui::App) {
        for (_, field) in &self.fields {
            field.update(cx, |field, cx| field.set_theme(theme, cx));
        }
        if let Some(input) = &self.standard_filter {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
    }

    pub(crate) fn pin_alignment_standard(&mut self, id: crate::group_identity::GroupId) -> bool {
        self.alignment_standard = Some(id);
        self.open == Some(Tool::Align)
    }

    fn remember_standard_choice(&mut self, standard: Option<&ToolTarget>) {
        if self.open == Some(Tool::Align) {
            self.alignment_standard = standard.and_then(|s| s.group_id.clone());
        }
    }

    pub(crate) fn invalidate_bindings(&mut self) {
        self.generation += 1;
        self.standard_request += 1;
        self.standard_filter_request += 1;
        self.target = None;
        self.standard = None;
        self.standard_load = StandardLoad::Loading;
        self.standard_picker_open = false;
        self.standard_filter = None;
        self.standard_matches = None;
        self.open = None;
    }

    fn accepts_standard_result(
        &self,
        request: u64,
        requested: &ToolTarget,
        current: Option<&ToolTarget>,
    ) -> bool {
        self.open.is_some()
            && self.standard_request == request
            && self.standard.as_ref() == Some(requested)
            && current == Some(requested)
    }

    pub fn lcf_space_label(&self) -> &'static str {
        self.lcf_space.label()
    }

    pub fn lcf_config(&self) -> LcfConfig {
        LcfConfig {
            space: self.lcf_space.space(2.0),
            range: self.lcf_range,
            sum_to_one: self.lcf_sum_to_one,
            fit_e0_shift: self.lcf_e0_shift,
            ..LcfConfig::default()
        }
    }

    pub fn pca_config(&self) -> PcaConfig {
        PcaConfig {
            space: self.lcf_space.space(2.0),
            range: self.lcf_range,
            center: self.pca_center,
        }
    }

    fn field_value(&self, field: ToolField, cx: &gpui::App) -> Option<f64> {
        self.fields
            .iter()
            .find(|(f, _)| *f == field)
            .and_then(|(_, e)| e.read(cx).value())
    }

    /// Pull the range fields into `lcf_range` (None = the space default).
    pub(crate) fn sync_range(&mut self, cx: &gpui::App) -> Result<(), String> {
        let lo = self.field_value(ToolField::RangeLo, cx);
        let hi = self.field_value(ToolField::RangeHi, cx);
        self.lcf_range = analysis_range_from_fields(lo, hi)?;
        if let Some(n) = self.field_value(ToolField::Components, cx) {
            self.pca_components = (n.round() as usize).max(1);
        }
        Ok(())
    }
}

impl ToolState {
    pub fn new() -> Self {
        Self {
            lcf_sum_to_one: true,
            pca_components: 2,
            ..Default::default()
        }
    }
}

impl StudioApp {
    pub(crate) fn open_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        if self.tools.fields.is_empty() {
            let theme = self.theme;
            self.tools.fields = ToolField::ALL
                .iter()
                .map(|&f| {
                    let (label, placeholder, default) = f.spec();
                    let field = cx.new(|cx| {
                        NumericField::new(label, placeholder, default, FieldKind::Float, theme, cx)
                            .with_step(if f == ToolField::ManualShift {
                                0.1
                            } else {
                                1.0
                            })
                    });
                    cx.subscribe(&field, |this: &mut Self, _f, event, cx| match event {
                        FieldEvent::Invalid(message) => {
                            this.status = message.clone();
                            cx.notify();
                        }
                        FieldEvent::Changed(_) => {
                            this.queue_tool_preview(cx);
                            if this.tools.open.is_some_and(|tool| tool.is_analysis()) {
                                this.invalidate_explore_plots(cx);
                            }
                        }
                        _ => {}
                    })
                    .detach();
                    (f, field)
                })
                .collect();
        }
        self.tools.open = Some(tool);
        if tool == Tool::Mcr
            && !matches!(
                self.tools.lcf_space,
                LcfSpaceChoice::Norm | LcfSpaceChoice::Flat
            )
        {
            self.tools.lcf_space = LcfSpaceChoice::Flat;
        }
        self.tools.message = SharedString::default();
        self.tools.target = self.current_tool_target();
        self.tools.standard_picker_open = false;
        self.tools.standard_filter_request += 1;
        self.tools.standard_matches = None;
        let input = cx.new(|cx| TextInput::new("Filter standards… (* glob)", "", self.theme, cx));
        cx.subscribe(&input, |this, _, event, cx| {
            if let InputEvent::Edited(text) = event {
                this.filter_tool_standards(text, cx);
            }
        })
        .detach();
        self.tools.standard_filter = Some(input);
        let standard = tool
            .needs_standard()
            .then(|| {
                let pinned = (tool == Tool::Align)
                    .then_some(self.tools.alignment_standard.as_ref())
                    .flatten()
                    .and_then(|id| {
                        self.group_registry.index(id).or_else(|| {
                            self.standalone_source
                                .as_ref()
                                .filter(|(_, _, sid)| sid == id)
                                .map(|_| NO_ENTRY)
                        })
                    })
                    .and_then(|ix| self.tool_target(ix));
                pinned.or_else(|| {
                    default_standard(
                        tool,
                        self.tools.target.as_ref(),
                        self.tool_groups(),
                        &self.selection,
                    )
                })
            })
            .flatten();
        self.choose_tool_standard(standard, cx);
        // Rebuilding with `None` drops a stale result card, e.g. MCR
        // components left on screen after opening Deglitch or Align.
        self.analysis.shown = tool.shown_analysis();
        self.rebuild_analysis_plot(cx);
        let was_data = self.stage == super::Stage::Data;
        self.set_stage(super::Stage::Data, cx);
        if was_data {
            // set_stage does not rebuild within the same stage, but the dashed
            // analysis boundaries depend on the open tool's range (PCA default
            // −20…+30 eV versus the MCR common measured interval).
            self.invalidate_explore_plots(cx);
        }
        self.set_side_panel_visible(super::assistant_shell::SidePanel::Inspector, true);
        self.inspector_scroll
            .set_offset(gpui::point(px(0.), px(0.)));
        self.queue_tool_preview(cx);
        cx.notify();
    }

    /// Prepare all operands, then fit LCF or train PCA and reconstruct the
    /// current target. Large input loading runs on a background worker.
    pub(super) fn run_analysis_tool(
        &mut self,
        tool: Tool,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        if tool == Tool::Mcr {
            return self.start_mcr(cx);
        }
        let Some(unknown) = self.spectrum.clone() else {
            self.tools.message = "No current group".into();
            cx.notify();
            return Err("No current group".into());
        };
        self.tools.sync_range(cx)?;
        if self.prepare_analysis_collection(tool, cx)? {
            return Ok(self.tools.message.to_string());
        }
        let marks = self.analysis_selection(tool);
        let standards = crate::app::cached_marked_spectra(
            &marks,
            &self.cache,
            |ix| self.effective_fingerprint(ix),
            |ix| self.entry_label(ix),
        );
        let names: Vec<String> = standards.iter().map(|(n, _)| n.clone()).collect();
        let spectra: Vec<std::sync::Arc<XASSpectrum>> =
            standards.into_iter().map(|(_, sp)| sp).collect();
        let kw = self.fft_summary().3;
        let mut cfg = self.tools.lcf_config();
        cfg.space = self.tools.lcf_space.space(kw);
        let outcome: Result<String, String> = match tool {
            Tool::Lcf => {
                if spectra.len() < 2 {
                    Err("Mark at least two standards (other than the current group); their spectra load when marked".into())
                } else if self.tools.lcf_all_combinations {
                    rexafs::prelude::lcf_combinatorial(&unknown, &spectra, &cfg, 4)
                        .map_err(|e| e.to_string())
                        .map(|mut ranked| {
                            for r in &mut ranked {
                                relabel(r, &names);
                            }
                            let best = ranked.first().cloned();
                            let n = ranked.len();
                            self.analysis.ranked = ranked;
                            self.analysis.lcf = best;
                            format!(
                                "{} ranked by R-factor",
                                crate::text::plural(n, "combination")
                            )
                        })
                } else {
                    rexafs::prelude::lcf(&unknown, &spectra, &cfg)
                        .map_err(|e| e.to_string())
                        .map(|mut r| {
                            relabel(&mut r, &names);
                            let msg = format!("LCF R-factor {:.2e}", r.r_factor);
                            self.analysis.ranked.clear();
                            self.analysis.lcf = Some(r);
                            msg
                        })
                }
            }
            Tool::Pca => {
                if spectra.len() < 2 {
                    Err("Mark at least two groups to train on".into())
                } else {
                    let mut pcfg = self.tools.pca_config();
                    pcfg.space = cfg.space;
                    rexafs::prelude::pca_train(&spectra, &pcfg)
                        .map_err(|e| e.to_string())
                        .and_then(|mut model| {
                            model.labels = names.clone();
                            let n = self.tools.pca_components.min(model.n_components().max(1));
                            let fit = model
                                .target_transform(&unknown, n)
                                .map_err(|e| e.to_string())?;
                            let msg = format!(
                                "{} explain {:.2} % · target R {:.2e}",
                                crate::text::plural(n, "component"),
                                model.cumulative_variance.get(n - 1).copied().unwrap_or(0.0)
                                    * 100.0,
                                fit.r_factor
                            );
                            self.analysis.pca = Some(model);
                            self.analysis.pca_fit = Some(fit);
                            Ok(msg)
                        })
                }
            }
            _ => Err("Not an analysis tool".into()),
        };
        match &outcome {
            Ok(msg) => {
                let sources: Vec<ToolTarget> = self
                    .current_tool_target()
                    .into_iter()
                    .chain(
                        crate::app::cached_marked_indices(&self.selection, &self.cache, |ix| {
                            self.effective_fingerprint(ix)
                        })
                        .filter(|&ix| tool == Tool::Pca || Some(ix) != self.current_group_index())
                        .filter_map(|ix| self.tool_target(ix)),
                    )
                    .collect();
                match tool {
                    Tool::Lcf => {
                        self.analysis.lcf_config = Some(cfg.clone());
                        self.analysis.lcf_inputs = sources
                            .iter()
                            .map(|s| crate::project::AnalysisInput {
                                corrections: self.correction_sources(s.ix),
                                group_id: s.group_id.clone(),
                                label: s.label.clone(),
                                fingerprint: s.fingerprint,
                            })
                            .collect();
                        self.analysis.lcf_sources = sources;
                    }
                    Tool::Pca => {
                        self.analysis.pca_inputs = sources
                            .iter()
                            .map(|s| crate::project::AnalysisInput {
                                corrections: self.correction_sources(s.ix),
                                group_id: s.group_id.clone(),
                                label: s.label.clone(),
                                fingerprint: s.fingerprint,
                            })
                            .collect();
                        self.analysis.pca_sources = sources;
                    }
                    _ => {}
                }
                self.record(
                    format!("{} on {}: {msg}", tool.name(), self.current_group_label()),
                    None,
                );
                self.tools.message = msg.clone().into();
                self.analysis.shown = Some(tool);
                self.rebuild_analysis_plot(cx);
                self.invalidate_explore_plots(cx);
            }
            Err(msg) => {
                self.tools.message = msg.clone().into();
            }
        }
        cx.notify();
        outcome
    }

    /// (Re)build the analysis plot entity from the current result.
    pub(crate) fn rebuild_analysis_plot(&mut self, cx: &mut Context<Self>) {
        use crate::plotting::analysis::{
            PcaView, analysis_axis_labels, build_mcr_diagnostic, build_pca_diagnostic,
        };
        let plot = match self.analysis.shown {
            Some(Tool::Lcf) => self.analysis.lcf.as_ref().map(|r| {
                let (x, y) = analysis_axis_labels(r.space);
                crate::plotting::build_lcf_plot(r, &x, &y, &self.theme)
            }),
            Some(Tool::Pca) => self.analysis.pca.as_ref().and_then(|m| {
                let retained = self
                    .analysis
                    .pca_fit
                    .as_ref()
                    .map_or(self.tools.pca_components, |f| f.n_components);
                if self.analysis.pca_view == PcaView::Residual {
                    Some(crate::plotting::analysis::build_pca_residual_plot(
                        m,
                        self.analysis.pca_fit.as_ref(),
                        self.analysis.pca_all_components,
                        self.analysis.pca_scale,
                        &self.theme,
                    ))
                } else if self.analysis.pca_view == PcaView::Target {
                    let (x, y) = analysis_axis_labels(m.space);
                    self.analysis
                        .pca_fit
                        .as_ref()
                        .map(|f| crate::plotting::build_pca_plot(f, &x, &y, &self.theme))
                } else {
                    Some(build_pca_diagnostic(
                        m,
                        self.analysis.pca_view,
                        retained,
                        self.analysis.pca_all_components,
                        self.analysis.pca_scale,
                        &self.theme,
                    ))
                }
            }),
            Some(Tool::Mcr) => self.analysis.mcr.as_ref().map(|m| {
                if self.analysis.mcr_view == crate::plotting::analysis::McrView::References {
                    crate::plotting::analysis::build_mcr_reference_plot(m, &self.theme)
                } else {
                    build_mcr_diagnostic(&m.result, self.analysis.mcr_view, &self.theme)
                }
            }),
            _ => None,
        };
        let Some(plot) = plot else {
            self.analysis.plot = None;
            return;
        };
        let (width, height) = self.card_px.get(&300).copied().unwrap_or((820, 580));
        let plot = plot.size_px(width, height);
        match &self.analysis.plot {
            Some(entity) => entity.update(cx, |rp, cx| rp.set_plot(plot, cx)),
            None => {
                self.analysis.plot = Some(
                    crate::plot_ranges::plot_builder(plot)
                        .interactive()
                        .build(cx),
                );
            }
        }
    }

    fn tool_value(&self, field: ToolField, cx: &Context<Self>) -> Option<f64> {
        self.tools
            .fields
            .iter()
            .find(|(f, _)| *f == field)
            .and_then(|(_, e)| e.read(cx).value())
    }

    pub(crate) fn current_tool_target(&self) -> Option<ToolTarget> {
        self.current_group_index()
            .and_then(|ix| self.tool_target(ix))
    }

    pub(crate) fn tool_target(&self, ix: usize) -> Option<ToolTarget> {
        if ix == NO_ENTRY {
            self.group_id(ix)?;
            let (path, label, id) = self.standalone_source.as_ref()?;
            return Some(ToolTarget::standalone(
                Some(id.clone()),
                path.clone(),
                label.to_string(),
                self.params.fingerprint(),
                self.project_generation,
                self.tools.generation,
            ));
        }
        if !self.valid_group_index(ix) {
            return None;
        }
        let derived = ix
            .checked_sub(DERIVED_BASE)
            .and_then(|i| self.derived.get(i));
        Some(ToolTarget {
            group_id: self.group_id(ix),
            ix,
            fingerprint: self.effective_fingerprint(ix),
            label: self.entry_label(ix),
            path: match derived {
                Some(d) => d.source.clone().unwrap_or_default(),
                None => self.catalog.path(ix),
            },
            derived_id: derived.map(|d| d.id),
            project_generation: self.project_generation,
            catalog_generation: self.tools.generation,
            size: (ix < self.catalog.len()).then(|| self.catalog.entry_size(ix)),
        })
    }

    /// Match the group panel's stable order: additional groups, then files.
    fn tool_groups(&self) -> impl Iterator<Item = ToolTarget> + '_ {
        marked_group_indices(&self.selection).filter_map(|ix| self.tool_target(ix))
    }

    pub(crate) fn choose_tool_standard(
        &mut self,
        standard: Option<ToolTarget>,
        cx: &mut Context<Self>,
    ) {
        self.tools.standard_request += 1;
        let request = self.tools.standard_request;
        self.tools.standard = standard.clone();
        self.tools.standard_load = StandardLoad::Loading;
        self.tools.standard_picker_open = false;
        self.tools.message = SharedString::default();
        cx.notify();
        let Some(standard) = standard else { return };
        if self.tool_target(standard.ix).as_ref() != Some(&standard) {
            return;
        }
        if standard.ix == NO_ENTRY {
            // A standalone input may be the bundled example. Pin its actual
            // loaded data; NO_ENTRY cache keys do not identify a source path.
            if self.spectrum_group.as_ref() == Some(&standard)
                && let Some(sp) = &self.spectrum
            {
                self.tools.standard_load = StandardLoad::Ready(sp.clone());
                self.queue_tool_preview(cx);
            }
            return;
        }
        let key = (standard.ix, standard.fingerprint);
        if let Some(sp) = self.cache.get(&key) {
            self.tools.standard_load = StandardLoad::Ready(sp.clone());
            self.queue_tool_preview(cx);
            return;
        }
        let raw_key = (
            standard.ix,
            self.effective_params(standard.ix).raw_fingerprint(),
        );
        let intake_origin = self.intake_origin(standard.ix, &standard.path);
        let job = self.process_group_job(standard.ix, standard.path.clone(), cx);
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                // A closed/reopened card, new choice, edit, or reordered group
                // must not receive this old job's result or cache entry.
                if !app.tools.accepts_standard_result(
                    request,
                    &standard,
                    app.tool_target(standard.ix).as_ref(),
                ) {
                    return;
                }
                match result {
                    Ok((sp, raw)) => {
                        if let Some(raw) = raw {
                            app.record_source_warnings(
                                standard.ix,
                                intake_origin.as_ref(),
                                &standard.path,
                                &raw.diagnostics,
                                raw.declared_edge.clone(),
                                raw.channel,
                            );
                            app.raw_cache.put(raw_key, raw);
                        }
                        let sp = Arc::new(sp);
                        app.cache.put(key, sp.clone());
                        // Pin this one operand for the card's lifetime, even
                        // if subsequent browsing evicts its shared cache entry.
                        app.tools.standard_load = StandardLoad::Ready(sp);
                    }
                    Err(error) => {
                        app.record_source_error(
                            intake_origin.as_ref(),
                            standard.label.clone(),
                            error.clone(),
                        );
                        app.tools.standard_load = StandardLoad::Failed(error);
                    }
                }
                app.queue_tool_preview(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn tool_readiness(&self, tool: Tool) -> Result<(), ReadinessReason> {
        if tool == Tool::Mcr && self.analysis.mcr_cancel.is_some() {
            return Ok(());
        }
        let current = self.current_tool_target();
        let standard = tool.needs_standard().then(|| {
            let current = self
                .tools
                .standard
                .as_ref()
                .and_then(|s| self.tool_target(s.ix));
            standard_readiness(
                self.tools.standard.as_ref(),
                current.as_ref(),
                &self.tools.standard_load,
            )
        });
        quantity_readiness(
            readiness(
                self.tools.target.as_ref(),
                current.as_ref(),
                self.spectrum.as_ref().and(self.spectrum_group.as_ref()),
                self.stale_plots.is_some(),
                self.load_running,
                standard,
            ),
            || {
                for input in self
                    .tools
                    .target
                    .iter()
                    .chain(self.tools.standard.iter().filter(|_| tool.needs_standard()))
                {
                    if input.ix >= DERIVED_BASE
                        && self.derived.get(input.ix - DERIVED_BASE).is_some_and(|g| {
                            g.processing_block_reason().is_some()
                                || (!tool.is_analysis() && !g.quantity.is_absorption())
                        })
                    {
                        return Err(ReadinessReason::Quantity);
                    }
                }
                Ok(())
            },
        )
    }

    fn tool_standard(&self) -> Result<(&str, &XASSpectrum), String> {
        match (&self.tools.standard, &self.tools.standard_load) {
            (Some(identity), StandardLoad::Ready(sp)) => Ok((&identity.label, sp)),
            _ => Err(ReadinessReason::StandardLoading.message().into()),
        }
    }

    fn tool_preview_key(&self, cx: &Context<Self>) -> Option<ToolPreviewKey> {
        let tool = self.tools.open.filter(|t| !t.is_analysis())?;
        self.tool_readiness(tool).ok()?;
        Some(ToolPreviewKey {
            tool,
            target: self.tools.target.clone()?,
            standard: self
                .tools
                .standard
                .clone()
                .filter(|_| tool.needs_standard()),
            values: tool
                .fields()
                .iter()
                .map(|&f| (f, self.tool_value(f, cx)))
                .collect(),
        })
    }

    pub(super) fn tool_preview_current(&self, cx: &Context<Self>) -> bool {
        self.tools.preview_key.is_some() && self.tool_preview_key(cx) == self.tools.preview_key
    }

    fn queue_tool_preview(&mut self, cx: &mut Context<Self>) {
        let key = self.tool_preview_key(cx);
        if key.is_some() && key == self.tools.preview_key {
            return;
        }
        self.tools.preview_request += 1;
        let request = self.tools.preview_request;
        let retain_alignment = key
            .as_ref()
            .zip(self.tools.preview_key.as_ref())
            .is_some_and(|(new, old)| {
                new.tool == Tool::Align
                    && old.tool == Tool::Align
                    && new.target == old.target
                    && new.standard == old.standard
            });
        self.tools.preview_key = key.clone();
        if !retain_alignment {
            self.tools.preview_plot = None;
        }
        self.tools.preview_error = None;
        self.tools.preview_message.clear();
        self.tools.preview_running = key.is_some();
        let (Some(key), Some(source)) = (key, self.spectrum.clone()) else {
            cx.notify();
            return;
        };
        let standard = match &self.tools.standard_load {
            StandardLoad::Ready(s) if key.tool.needs_standard() => Some(s.clone()),
            _ => None,
        };
        let theme = self.theme;
        let current_offset = self.ui_params().energy_offset_ev;
        let job_key = key.clone();
        let job = cx.background_executor().spawn(async move {
            let standard_name = job_key
                .standard
                .as_ref()
                .map(|t| t.label.as_str())
                .unwrap_or("");
            let standard = standard.as_deref().map(|s| (standard_name, s));
            let (after, operation, label) = process_tool(
                job_key.tool,
                &source,
                standard,
                &job_key.target.label,
                &job_key.values,
                Vec::new(),
            )?;
            let plot = crate::plotting::build_tool_preview(
                &source,
                &after,
                standard,
                job_key.tool == Tool::Difference,
                (job_key.tool == Tool::Align).then(|| {
                    let value = |field, default| {
                        job_key
                            .values
                            .iter()
                            .find(|(f, _)| *f == field)
                            .and_then(|(_, v)| *v)
                            .unwrap_or(default)
                    };
                    (value(ToolField::WinLo, -50.), value(ToolField::WinHi, 100.))
                }),
                &theme,
            )?;
            let label = if job_key.tool == Tool::Align {
                format!(
                    "Auto {:+.2} + manual {:+.1} → offset {:+.2} eV",
                    operation.parameters["automatic_shift_ev"]
                        .as_f64()
                        .unwrap_or(0.),
                    operation.parameters["manual_shift_ev"]
                        .as_f64()
                        .unwrap_or(0.),
                    current_offset + operation.applied_energy_shift_ev
                )
            } else {
                label
            };
            Ok::<_, String>((plot, label))
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                if app.tools.preview_request != request
                    || app.tool_preview_key(cx).as_ref() != Some(&key)
                {
                    return;
                }
                app.tools.preview_running = false;
                match result {
                    Ok((plot, label)) => {
                        app.tools.preview_plot = Some(
                            crate::plot_ranges::plot_builder(plot.size_px(900, 540))
                                .interactive()
                                .build(cx),
                        );
                        app.tools.preview_message = label;
                    }
                    Err(error) => app.tools.preview_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn apply_tool(&mut self, cx: &mut Context<Self>) {
        let Some(tool) = self.tools.open else {
            return;
        };
        if let Err(reason) = self.tool_readiness(tool) {
            self.tools.message = reason.message().into();
            cx.notify();
            return;
        }
        if tool.is_analysis() {
            if let Err(error) = self.run_analysis_tool(tool, cx) {
                self.tools.message = error.into();
                cx.notify();
            }
            return;
        }
        let Some(source) = self.spectrum.clone() else {
            self.tools.message = "No current group".into();
            cx.notify();
            return;
        };
        let name = self.current_group_label().to_string();
        let values: Vec<_> = tool
            .fields()
            .iter()
            .map(|&f| (f, self.tool_value(f, cx)))
            .collect();
        let inputs = self
            .tools
            .target
            .iter()
            .chain(self.tools.standard.iter().filter(|_| tool.needs_standard()))
            .map(ToolTarget::operation_input)
            .collect();
        let standard = if tool.needs_standard() {
            self.tool_standard().ok()
        } else {
            None
        };
        let result = process_tool(tool, &source, standard, &name, &values, inputs);
        match result {
            Ok((sp, operation, label)) => {
                if tool == Tool::Align {
                    if self.refuse_frozen_edit(cx) {
                        return;
                    }
                    let Some(target) = self.current_group_index() else {
                        return;
                    };
                    let before = self.ui_params().clone();
                    let after = match aligned_parameters(&before, &operation) {
                        Ok(after) => after,
                        Err(error) => {
                            self.tools.message = error.into();
                            cx.notify();
                            return;
                        }
                    };
                    let offset = after.energy_offset_ev;
                    self.apply_params_to(Some(target), after.clone());
                    self.record_param_edit(Some(target), None, before, after, label);
                    self.tools.open = None;
                    self.tools.preview_plot = None;
                    self.ui.sections.remove("source-details-closed");
                    self.sync_param_fields(cx);
                    self.status = format!("Energy offset {offset:+.2} eV").into();
                    self.schedule_recompute(cx);
                    cx.notify();
                    return;
                }
                let derived = match materialize_tool_output(
                    tool,
                    label.clone(),
                    &sp,
                    self.ui_params(),
                    operation,
                ) {
                    Ok(mut derived) => {
                        derived.declared_edge = self
                            .tools
                            .target
                            .as_ref()
                            .and_then(|target| self.group_declared_edge(target.ix));
                        if let Some(record) = self
                            .tools
                            .target
                            .as_ref()
                            .and_then(|target| self.saved_parser_record(target.ix))
                            && let Some(params) = &mut derived.params
                        {
                            params.import.mode = record.channel;
                        }
                        let receipts = derived
                            .operation
                            .as_ref()
                            .map(|op| {
                                op.inputs
                                    .iter()
                                    .filter_map(|i| {
                                        i.group_id
                                            .as_ref()
                                            .and_then(|id| self.group_registry.index(id))
                                    })
                                    .map(|ix| self.correction_sources(ix))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();
                        derived.corrections = crate::fluorescence_history::combine(
                            receipts.iter().map(Vec::as_slice),
                        );
                        derived.absorption_mode = sp.absorption_mode();
                        derived.id = self.next_group_id();
                        derived.group_id = Some(crate::group_identity::GroupId::new_result());
                        derived
                    }
                    Err(error) => {
                        self.tools.message = error.into();
                        cx.notify();
                        return;
                    }
                };
                self.record(
                    format!("tool: {label}"),
                    Some(super::journal::UndoOp::DerivedAdd {
                        index: self.derived.len(),
                        spectrum: derived.clone(),
                    }),
                );
                self.derived.push(derived);
                self.rekey_after_catalog_change();
                let ix = DERIVED_BASE + self.derived.len() - 1;
                self.tools.message = format!("created {label}").into();
                self.tools.open = None;
                self.select_entry(ix, cx);
                self.sync_param_fields(cx);
                cx.notify();
            }
            Err(message) => {
                self.tools.message = message.into();
                cx.notify();
            }
        }
    }

    /// Processing tool list plus the open tool's inline form.
    pub(crate) fn tools_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .flex_col()
            .child(
                super::button(
                    &self.theme,
                    "open-fluorescence",
                    "Fluorescence correction…",
                    false,
                )
                .on_click(cx.listener(|app, _, _, cx| app.open_fluorescence(cx))),
            )
            .child(self.tool_list(&Tool::PROCESSING, cx))
    }

    /// Analysis tool list (LCF / PCA) plus form and results.
    pub(crate) fn analysis_tools_section(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                super::button(&self.theme, "open-peak-analysis", "XANES peak fit…", false)
                    .on_click(cx.listener(|app, _, _, cx| app.open_peaks(cx))),
            )
            .child(self.tool_list(&Tool::ANALYSIS, cx))
    }

    fn tool_list(&self, tools: &[Tool], cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut list = div().flex().flex_col().gap_0p5();
        let mut ordered = tools.to_vec();
        ordered.sort_by_key(|&tool| self.tools.open != Some(tool));
        for tool in ordered {
            let open = self.tools.open == Some(tool);
            list = list.child(
                super::controls::disclosure(
                    &t,
                    SharedString::from(format!("tool-{}", tool.name())),
                    tool.name(),
                    open,
                    false,
                )
                .tooltip(move |_, cx| {
                    cx.new(|_| super::controls::Tooltip {
                        label: tool.hint().into(),
                        theme: t,
                    })
                    .into()
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.tools.open == Some(tool) {
                        this.tools.open = None;
                        // Remove the closed tool's dashed analysis boundaries.
                        this.invalidate_explore_plots(cx);
                        cx.notify();
                    } else {
                        this.open_tool(tool, cx);
                    }
                })),
            );
            if open {
                let readiness = self.tool_readiness(tool);
                let mut form = div()
                    .mx_1()
                    .mb_1()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(t.border)
                    .bg(t.bg)
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .px_3()
                            .pt_1()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child(tool.hint()),
                    )
                    .child(
                        div()
                            .px_3()
                            .pb_1()
                            .text_size(px(11.))
                            .text_color(t.accent)
                            .child(tool.effect()),
                    );
                if tool != Tool::Mcr {
                    form = form.child(div().px_3().py_1().text_size(px(11.)).child(format!(
                        "Target: {}",
                        self.tools
                            .target
                            .as_ref()
                            .map(|t| t.label.as_str())
                            .unwrap_or("none")
                    )));
                }
                if tool.needs_standard() {
                    form = form.child(self.standard_picker(tool, cx));
                }
                if tool.is_analysis() {
                    form = form
                        .child(self.analysis_operands(tool, cx))
                        .child(self.analysis_options(tool, cx));
                }
                for field in tool.fields() {
                    if let Some((_, entity)) = self.tools.fields.iter().find(|(f, _)| f == field) {
                        form = form.child(entity.clone());
                    }
                }
                if self.tool_preview_current(cx) {
                    form = form.child(
                        div()
                            .px_3()
                            .py_1()
                            .text_size(px(11.))
                            .text_color(if self.tools.preview_error.is_some() {
                                t.warn
                            } else {
                                t.text_muted
                            })
                            .child(if self.tools.preview_running {
                                "Preparing preview…".to_owned()
                            } else if let Some(error) = &self.tools.preview_error {
                                error.clone()
                            } else {
                                self.tools.preview_message.clone()
                            }),
                    );
                }
                form = form.child(
                    div()
                        .px_3()
                        .py_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            button(
                                &t,
                                "tool-apply",
                                if tool == Tool::Mcr && self.analysis.mcr_cancel.is_some() {
                                    "Cancel calculation"
                                } else {
                                    tool.apply_label()
                                },
                                readiness.is_ok(),
                            )
                            .when(readiness.is_err(), |d| {
                                d.disabled(true).opacity(0.45).cursor_default()
                            })
                            .when(readiness.is_ok(), |d| {
                                d.on_click(
                                    cx.listener(|this, _: &ClickEvent, _w, cx| this.apply_tool(cx)),
                                )
                            }),
                        )
                        .child(
                            super::controls::icon_button(
                                &t,
                                "tool-cancel",
                                crate::icons::Icon::Close,
                                "Close tool",
                                false,
                            )
                            .on_click(cx.listener(
                                |this, _: &ClickEvent, _w, cx| {
                                    this.tools.open = None;
                                    this.invalidate_explore_plots(cx);
                                    cx.notify();
                                },
                            )),
                        ),
                );
                if let Err(reason) = readiness {
                    form = form.child(
                        div()
                            .px_3()
                            .pb_1()
                            .text_size(px(11.))
                            .text_color(t.warn)
                            .child(reason.message()),
                    );
                }
                if tool.needs_standard()
                    && let StandardLoad::Failed(error) = &self.tools.standard_load
                {
                    form = form.child(
                        div()
                            .px_3()
                            .pb_1()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child(error.clone()),
                    );
                }
                if !self.tools.message.is_empty() {
                    form = form.child(
                        div()
                            .px_3()
                            .pb_1()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child(self.tools.message.clone()),
                    );
                }
                list = list.child(form);
                if tool.is_analysis() {
                    list = list.children(self.analysis_results(tool, cx));
                }
            }
        }
        list
    }

    /// Name matching runs only on edits, off the UI thread. Results contain
    /// indices, never parameter fingerprints or full operand identities.
    fn filter_tool_standards(&mut self, text: &str, cx: &mut Context<Self>) {
        self.tools.standard_filter_request += 1;
        let request = self.tools.standard_filter_request;
        self.tools.standard_matches = None;
        let pattern = text.trim().to_ascii_lowercase();
        if pattern.is_empty() {
            cx.notify();
            return;
        }
        self.tools.standard_matches = Some(Arc::new(Vec::new()));
        let names = self.catalog.names_snapshot();
        let derived: Vec<_> = self.derived.iter().map(|d| d.label.clone()).collect();
        let job = cx.background_executor().spawn(async move {
            derived
                .iter()
                .map(String::as_str)
                .enumerate()
                .map(|(i, name)| (DERIVED_BASE + i, name))
                .chain(names.iter().enumerate())
                .filter(|(_, name)| filter_match_lower(&name.to_ascii_lowercase(), &pattern))
                .map(|(ix, _)| ix)
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let matches = job.await;
            this.update(cx, |app, cx| {
                if app.tools.standard_filter_request == request {
                    app.tools.standard_matches = Some(Arc::new(matches));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn standard_picker(&self, tool: Tool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let title = if tool == Tool::Difference {
            "Standard / baseline"
        } else {
            "Standard"
        };
        let name = self
            .tools
            .standard
            .as_ref()
            .map(|s| s.label.as_str())
            .unwrap_or("none");
        let mut picker = div()
            .px_3()
            .py_1()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_size(px(11.)).child(format!("{title}: {name}")))
            .child(
                button(&t, "tool-standard-picker", "Choose standard…", false).on_click(
                    cx.listener(|this, _: &ClickEvent, _w, cx| {
                        this.tools.standard_picker_open = !this.tools.standard_picker_open;
                        cx.notify();
                    }),
                ),
            );
        if self.tools.standard_picker_open {
            let entity = cx.entity();
            let matches = self.tools.standard_matches.clone();
            let count = matches
                .as_ref()
                .map_or(self.derived.len() + self.catalog.len(), |m| m.len());
            picker = picker
                .children(self.tools.standard_filter.clone())
                .child(
                    super::chip(
                        &t,
                        "tool-standard-none",
                        "None",
                        self.tools.standard.is_none(),
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                        this.tools.remember_standard_choice(None);
                        this.choose_tool_standard(None, cx)
                    })),
                )
                .child(
                    uniform_list("tool-standard-choices", count, move |range, _, app| {
                        entity.update(app, |this, cx| {
                            range
                                .filter_map(|row| {
                                    let ix = standard_row_index(
                                        matches.as_deref().map(Vec::as_slice),
                                        row,
                                        this.derived.len(),
                                        this.catalog.len(),
                                    )?;
                                    let selected =
                                        this.tools.standard.as_ref().is_some_and(|s| s.ix == ix);
                                    let eligible = tool == Tool::Calibrate
                                        || Some(ix) != this.tools.target.as_ref().map(|t| t.ix);
                                    let label = if ix >= DERIVED_BASE {
                                        let d = this.derived.get(ix - DERIVED_BASE)?;
                                        format!("{} · result #{}", d.label, d.id)
                                    } else {
                                        format!(
                                            "{} · {}",
                                            this.catalog.name(ix),
                                            this.catalog.path(ix).display()
                                        )
                                    };
                                    Some(
                                        super::chip(
                                            &this.theme,
                                            SharedString::from(format!("tool-standard-{ix}")),
                                            label,
                                            selected,
                                        )
                                        .h(px(27.))
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .when(!eligible, |d| d.opacity(0.45).cursor_default())
                                        .when(eligible, |d| {
                                            d.on_click(cx.listener(
                                                move |this, _: &ClickEvent, _, cx| {
                                                    let standard = this.tool_target(ix);
                                                    this.tools.remember_standard_choice(
                                                        standard.as_ref(),
                                                    );
                                                    this.choose_tool_standard(standard, cx);
                                                },
                                            ))
                                        })
                                        .into_any_element(),
                                    )
                                })
                                .collect()
                        })
                    })
                    .w_full()
                    .min_w_0()
                    .h(px(180.))
                    .flex_none(),
                );
        }
        picker
    }

    fn analysis_operands(&self, tool: Tool, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme;
        let marks = self.analysis_selection(tool);
        let ready = crate::app::cached_marked_indices(&marks, &self.cache, |ix| {
            self.effective_fingerprint(ix)
        })
        .count();
        let skipped = marks.len().saturating_sub(ready);
        let open = self.ui.sections.contains("Analysis inputs");
        let mut panel = div().px_2().child(
            super::controls::disclosure(
                &t,
                "analysis-inputs",
                format!(
                    "{}: {ready} ready{}",
                    if tool == Tool::Lcf {
                        "Standards"
                    } else {
                        "Training set"
                    },
                    if skipped > 0 {
                        format!(" · {skipped} load on run")
                    } else {
                        String::new()
                    }
                ),
                open,
                false,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                if !this.ui.sections.remove("Analysis inputs") {
                    this.ui.sections.insert("Analysis inputs");
                }
                cx.notify();
            })),
        );
        if open {
            let mut rows = div()
                .id("analysis-input-list")
                .max_h(px(180.))
                .overflow_y_scroll();
            for ix in marked_group_indices(&marks) {
                let cached = self
                    .cache
                    .peek(&(ix, self.effective_fingerprint(ix)))
                    .is_some();
                rows = rows.child(
                    div()
                        .text_size(px(11.))
                        .text_color(if cached { t.text_muted } else { t.warn })
                        .child(format!(
                            "{}{}",
                            self.entry_label(ix),
                            if cached {
                                ""
                            } else {
                                " · loads when analysis starts"
                            }
                        )),
                );
            }
            panel = panel.child(rows);
        }
        panel
    }

    /// Space segment + option chips shared by LCF and PCA.
    fn analysis_options(&self, tool: Tool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut seg = super::segmented(&t);
        for (i, choice) in LcfSpaceChoice::ALL.into_iter().enumerate() {
            if tool == Tool::Mcr && !matches!(choice, LcfSpaceChoice::Norm | LcfSpaceChoice::Flat) {
                continue;
            }
            seg = seg.child(
                super::segment(
                    &t,
                    SharedString::from(format!("lcf-space-{i}")),
                    choice.label(),
                    self.tools.lcf_space == choice,
                    i == 0,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                    this.tools.lcf_space = choice;
                    cx.notify();
                })),
            );
        }
        let mut row = div()
            .px_3()
            .py_1()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1p5()
            .child(seg);
        if self.tools.lcf_space != LcfSpaceChoice::Chi {
            row = row.child(
                div()
                    .w_full()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("Analysis range (relative to E₀)"),
            );
            for (i, label, range) in [
                (0, "XANES: −20…+80", Some((-20., 80.))),
                (1, "−200…+800", Some((-200., 800.))),
                (2, "Common full range", None),
            ] {
                row = row.child(
                    super::chip(
                        &t,
                        SharedString::from(format!("analysis-range-{i}")),
                        label,
                        tool == Tool::Mcr
                            && range.is_none()
                            && self.tools.field_value(ToolField::McrRangeLo, cx).is_none()
                            && self.tools.field_value(ToolField::McrRangeHi, cx).is_none(),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_analysis_range_preset(tool, range, cx)
                    })),
                );
            }
        }
        if tool == Tool::Lcf {
            row = row
                .child(
                    super::chip(&t, "lcf-sum", "Σ = 1", self.tools.lcf_sum_to_one).on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| {
                            this.tools.lcf_sum_to_one = !this.tools.lcf_sum_to_one;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    super::chip(&t, "lcf-e0", "E₀ shifts", self.tools.lcf_e0_shift).on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| {
                            this.tools.lcf_e0_shift = !this.tools.lcf_e0_shift;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    super::chip(
                        &t,
                        "lcf-all",
                        "all combinations",
                        self.tools.lcf_all_combinations,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                        this.tools.lcf_all_combinations = !this.tools.lcf_all_combinations;
                        cx.notify();
                    })),
                );
        }
        if tool == Tool::Pca {
            row = row.child(
                super::chip(&t, "pca-center", "Subtract mean", self.tools.pca_center).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.tools.pca_center = !this.tools.pca_center;
                        cx.notify();
                    }),
                ),
            );
        }
        if tool == Tool::Mcr {
            row = row
                .child(
                    div()
                        .w_full()
                        .text_size(px(11.))
                        .text_color(t.text_muted)
                        .child("Auto uses the full measured range shared by the selected spectra."),
                )
                .child(
                    super::chip(
                        &t,
                        "mcr-closure",
                        "Fractions sum to 1",
                        self.tools.lcf_sum_to_one,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tools.lcf_sum_to_one = !this.tools.lcf_sum_to_one;
                        cx.notify();
                    })),
                )
                .child(
                    super::chip(
                        &t,
                        "mcr-positive",
                        "Nonnegative spectra",
                        self.tools.mcr_nonnegative_spectra,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tools.mcr_nonnegative_spectra = !this.tools.mcr_nonnegative_spectra;
                        cx.notify();
                    })),
                );
        }
        row
    }

    /// Result tables under the analysis form.
    fn analysis_results(&self, tool: Tool, _cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let t = self.theme;
        let mut out: Vec<gpui::AnyElement> = Vec::new();
        let row = |k: String, v: String, warn: bool| {
            div()
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(11.5))
                .child(div().text_color(t.text_muted).child(k))
                .child(
                    div()
                        .font_family(super::MONO)
                        .text_color(if warn { t.warn } else { t.text })
                        .child(v),
                )
                .into_any_element()
        };
        match tool {
            Tool::Lcf => {
                if let Some(r) = &self.analysis.lcf {
                    for c in &r.weights {
                        let sigma = c.stderr.map(|e| format!(" ± {e:.3}")).unwrap_or_default();
                        let shift = if c.e0_shift.abs() > 1e-9 {
                            format!(" · ΔE {:+.2}", c.e0_shift)
                        } else {
                            String::new()
                        };
                        out.push(row(
                            c.name.clone(),
                            format!("{:.3}{sigma}{shift}", c.weight),
                            false,
                        ));
                    }
                    out.push(row(
                        "Σ weights".into(),
                        format!("{:.3}", r.sum_of_weights),
                        false,
                    ));
                    out.push(row("R-factor".into(), format!("{:.3e}", r.r_factor), false));
                    out.push(row(
                        "Reduced χ²".into(),
                        format!("{:.3e}", r.reduced_chi_square),
                        false,
                    ));
                }
                for (i, r) in self.analysis.ranked.iter().enumerate().skip(1).take(4) {
                    let combo = r
                        .weights
                        .iter()
                        .map(|c| format!("{} {:.2}", c.name, c.weight))
                        .collect::<Vec<_>>()
                        .join(" · ");
                    out.push(row(
                        format!("#{} {combo}", i + 1),
                        format!("R {:.2e}", r.r_factor),
                        false,
                    ));
                }
            }
            Tool::Pca => {
                if let Some(m) = &self.analysis.pca {
                    let suggestion = crate::plotting::analysis::pca_count_suggestion(m);
                    out.push(row(
                        "Training spectra".into(),
                        m.n_spectra().to_string(),
                        false,
                    ));
                    out.push(row(
                        "Mean subtraction".into(),
                        if m.centered { "On" } else { "Off" }.into(),
                        false,
                    ));
                    if let Some((count, basis)) = suggestion {
                        out.push(row(
                            format!("Try {}", crate::text::plural(count, "component")),
                            basis.into(),
                            false,
                        ));
                    } else {
                        out.push(row(
                            "Component count".into(),
                            "Inspect the scree plot".into(),
                            false,
                        ));
                    }
                    out.push(row(
                        "99.9% / 99.99% retained".into(),
                        format!(
                            "{} / {}",
                            m.suggested_components_variance(0.999),
                            m.suggested_components_variance(0.9999)
                        ),
                        false,
                    ));
                    out.push(div().px_3().py_1().text_size(px(11.)).text_color(t.text_muted)
                        .child(if m.centered {"PCA counts variation directions. With sum-to-one mixtures, the mean can account for another chemical component."} else {"A suggested rank is not a chemical identification. Check noise, preprocessing and reconstruction."}).into_any_element());
                    for i in 0..m.n_components().min(6) {
                        let var = m.variance_explained.get(i).copied().unwrap_or(0.0) * 100.0;
                        let cum = m.cumulative_variance.get(i).copied().unwrap_or(0.0) * 100.0;
                        let ind = m
                            .ind
                            .get(i + 1)
                            .map(|v| format!(" · IND {v:.2e}"))
                            .unwrap_or_default();
                        out.push(row(
                            format!(
                                "PC{} {}",
                                i + 1,
                                if suggestion.is_some_and(|(n, _)| n == i + 1) {
                                    "←"
                                } else {
                                    ""
                                }
                            ),
                            format!("{var:.2} % · Σ {cum:.2} %{ind}"),
                            false,
                        ));
                    }
                }
                if let Some(f) = &self.analysis.pca_fit {
                    out.push(row(
                        format!("Target transform ({} comp.)", f.n_components),
                        format!("R {:.2e}", f.r_factor),
                        f.r_factor > 1e-2,
                    ));
                }
            }
            Tool::Mcr => {
                if let Some(saved) = &self.analysis.mcr {
                    let r = &saved.result;
                    out.push(row(
                        "Samples / components".into(),
                        format!("{} / {}", r.labels.len(), r.spectra.nrows()),
                        false,
                    ));
                    out.push(row(
                        "Stopped".into(),
                        format!(
                            "{} · {}",
                            mcr_termination_label(r.termination),
                            crate::text::plural(r.iterations, "iteration")
                        ),
                        r.termination != rexafs::prelude::McrTermination::Converged,
                    ));
                    out.push(row(
                        "Relative squared residual".into(),
                        format!("{:.3e}", r.relative_error),
                        false,
                    ));
                    out.push(row(
                        "Energy range (eV)".into(),
                        format!("{:.1}–{:.1}", r.x[0], r.x[r.x.len() - 1]),
                        false,
                    ));
                    out.push(row(
                        "Saved calculation".into(),
                        format!(
                            "{} · seed {}",
                            if r.config.sum_to_one {
                                "Σ = 1"
                            } else {
                                "free sum"
                            },
                            r.config.seed
                        ),
                        false,
                    ));
                    out.push(row(
                        "Inputs".into(),
                        if self.mcr_result_current() {
                            "Unchanged"
                        } else {
                            "Historical result"
                        }
                        .into(),
                        !self.mcr_result_current(),
                    ));
                    out.push(div().px_3().py_1().text_size(px(11.)).text_color(t.text_muted)
                        .child("A good reconstruction does not prove unique pure spectra. Results are saved in the project and analysis export.").into_any_element());
                }
            }
            _ => {}
        }
        out
    }
}

/// Replace the index-based component names with the group labels.
fn relabel(result: &mut rexafs::prelude::LcfResult, names: &[String]) {
    for c in &mut result.weights {
        if let Some(n) = names.get(c.index) {
            c.name = n.clone();
        }
    }
}

#[cfg(test)]
mod tool_readiness_tests {
    use super::*;
    use std::{collections::BTreeSet, num::NonZeroUsize};

    #[test]
    fn analysis_excludes_current_identity_before_duplicate_display_labels() {
        use crate::group_identity::GroupId;
        let current = GroupId::new_result();
        let others = [GroupId::new_result(), GroupId::new_result()];
        let marks = BTreeSet::from([0, 1, 2]);
        let identity = |ix: usize| {
            Some(if ix == 0 {
                current.clone()
            } else {
                others[ix - 1].clone()
            })
        };
        let filtered = analysis_marks(&marks, Some(&current), identity);
        let mut cache = lru::LruCache::new(NonZeroUsize::new(3).unwrap());
        let spectra: Vec<_> = (0..3).map(|_| Arc::new(XASSpectrum::new())).collect();
        for (ix, spectrum) in spectra.iter().enumerate() {
            cache.put((ix, 0), spectrum.clone());
        }
        let inputs =
            crate::app::cached_marked_spectra(&filtered, &cache, |_| 0, |_| "Cu foil".into());
        assert_eq!(inputs.len(), 2);
        assert!(inputs.iter().all(|(label, _)| label == "Cu foil"));
        assert!(Arc::ptr_eq(&inputs[0].1, &spectra[1]));
        assert!(Arc::ptr_eq(&inputs[1].1, &spectra[2]));
        assert_eq!(analysis_marks(&marks, None, identity), marks);
    }

    #[test]
    fn alignment_pin_only_requests_loading_for_an_open_align_tool() {
        use crate::group_identity::GroupId;
        for open in [
            None,
            Some(Tool::Difference),
            Some(Tool::Calibrate),
            Some(Tool::Align),
        ] {
            let mut state = ToolState {
                open,
                standard: Some(group(1)),
                ..Default::default()
            };
            let original = state.standard.clone();
            let id = GroupId::new_result();
            assert_eq!(
                state.pin_alignment_standard(id.clone()),
                open == Some(Tool::Align)
            );
            assert_eq!(state.alignment_standard, Some(id));
            assert_eq!(state.standard, original);
            assert_eq!(state.standard_request, 0);
        }
    }

    #[test]
    fn explicit_align_picker_choices_replace_or_clear_the_pin() {
        use crate::group_identity::GroupId;
        let mut state = ToolState {
            open: Some(Tool::Align),
            ..Default::default()
        };
        state.pin_alignment_standard(GroupId::new_result());
        let mut chosen = group(2);
        chosen.group_id = Some(GroupId::new_result());
        state.remember_standard_choice(Some(&chosen));
        assert_eq!(state.alignment_standard, chosen.group_id);
        state.remember_standard_choice(None);
        assert_eq!(state.alignment_standard, None);
        let pinned = GroupId::new_result();
        state.pin_alignment_standard(pinned.clone());
        for open in [Some(Tool::Difference), Some(Tool::Calibrate)] {
            state.open = open;
            state.remember_standard_choice(Some(&chosen));
            state.remember_standard_choice(None);
            assert_eq!(state.alignment_standard, Some(pinned.clone()));
        }
    }

    #[test]
    fn display_rename_does_not_invalidate_tool_operand_readiness() {
        let target = ToolTarget::standalone(None, "/cu.dat".into(), "Cu".into(), 1, 2, 3);
        let mut renamed = target.clone();
        renamed.label = "Cu foil".into();
        assert_eq!(target, renamed);
        assert_eq!(
            readiness(
                Some(&target),
                Some(&renamed),
                Some(&target),
                false,
                false,
                None
            ),
            Ok(())
        );
        renamed.fingerprint += 1;
        assert_ne!(target, renamed);
    }

    #[test]
    fn typed_outputs_materialize_quantity_and_bound_operation_inputs() {
        let params = PipelineParams {
            e0: Some(110.0),
            bkg_ek0: Some(111.0),
            ..Default::default()
        };
        let standard = edge(100.0);
        let mut target = edge(110.0);
        let original_energy = target.energy.clone().unwrap();
        let measured = standard
            .edge_feature_energy(EdgeFeature::DerivativeMax)
            .unwrap();
        let shift = calibrate_from_standard(&mut target, &standard, measured + 3.0).unwrap();
        let inputs = vec![
            group(DERIVED_BASE + 7).operation_input(),
            group(2).operation_input(),
        ];
        for tool in Tool::PROCESSING {
            let applied = if matches!(tool, Tool::Align | Tool::Calibrate) {
                shift
            } else {
                0.0
            };
            let operation = Operation {
                tool: tool.name().into(),
                parameters: serde_json::json!({"expected_energy_ev": measured + 3.0}),
                inputs: inputs.clone(),
                applied_energy_shift_ev: applied,
            };
            let output =
                materialize_tool_output(tool, "output".into(), &target, &params, operation.clone())
                    .unwrap();
            assert_eq!(output.operation, Some(operation));
            assert_eq!(
                output.operation.as_ref().unwrap().inputs[0].derived_id,
                Some(7)
            );
            assert_eq!(
                output.quantity,
                if tool == Tool::Difference {
                    Quantity::NormalizedDifference
                } else {
                    Quantity::RawMu
                }
            );
            assert!(!output.quantity_unconfirmed);
            assert_eq!(output.params.as_ref().unwrap().e0, Some(110.0 + applied));
            assert_eq!(
                output.params.as_ref().unwrap().bkg_ek0,
                Some(111.0 + applied)
            );
            for _ in 0..2 {
                let (energy, _) = output.raw(output.params.as_ref().unwrap()).unwrap();
                for (before, after) in original_energy.iter().zip(&energy) {
                    assert!((after - before - shift).abs() < 1e-10);
                }
            }
        }
    }

    fn group(ix: usize) -> ToolTarget {
        ToolTarget {
            group_id: Some(crate::group_identity::GroupId::legacy_result(ix as u64)),
            ix,
            fingerprint: 42,
            label: format!("group {ix}"),
            path: PathBuf::from(format!("/data/{ix}.dat")),
            derived_id: ix.checked_sub(DERIVED_BASE).map(|id| id as u64),
            project_generation: 1,
            catalog_generation: 2,
            size: Some(100),
        }
    }

    #[test]
    fn quantity_readiness_checks_identity_before_rekeyed_array_slots() {
        let bound = group(DERIVED_BASE + 3);
        let moved = ToolTarget {
            ix: DERIVED_BASE + 2,
            ..bound.clone()
        };
        let replaced = ToolTarget {
            derived_id: Some(4),
            ..bound.clone()
        };
        for current in [&moved, &replaced] {
            assert_eq!(
                quantity_readiness(
                    readiness(
                        Some(&bound),
                        Some(current),
                        Some(&bound),
                        false,
                        false,
                        None
                    ),
                    || panic!("must not inspect a re-keyed slot's quantity"),
                ),
                Err(ReadinessReason::TargetChanged)
            );
        }
        let standard = group(DERIVED_BASE + 4);
        let moved_standard = ToolTarget {
            ix: DERIVED_BASE + 3,
            ..standard.clone()
        };
        assert_eq!(
            quantity_readiness(
                readiness(
                    Some(&bound),
                    Some(&bound),
                    Some(&bound),
                    false,
                    false,
                    Some(standard_readiness(
                        Some(&standard),
                        Some(&moved_standard),
                        &StandardLoad::Loading
                    ))
                ),
                || panic!("must not inspect a changed standard's quantity")
            ),
            Err(ReadinessReason::StandardChanged)
        );
        assert_eq!(
            quantity_readiness(
                readiness(Some(&bound), Some(&bound), Some(&bound), false, false, None),
                || Err(ReadinessReason::Quantity)
            ),
            Err(ReadinessReason::Quantity)
        );
        assert_eq!(quantity_readiness(Ok(()), || Ok(())), Ok(()));
    }

    #[test]
    fn matching_catalog_and_derived_revisions_are_ready() {
        for target in [group(0), group(DERIVED_BASE)] {
            assert_eq!(
                readiness(
                    Some(&target),
                    Some(&target),
                    Some(&target),
                    false,
                    false,
                    None
                ),
                Ok(())
            );
        }
    }

    #[test]
    fn unreadable_selection_cannot_use_previous_spectrum() {
        let old = group(0);
        let unreadable = group(1);
        for opened in [&old, &unreadable] {
            let result = readiness(
                Some(opened),
                Some(&unreadable),
                Some(&old),
                true,
                false,
                None,
            );
            assert_eq!(result, Err(ReadinessReason::CurrentFailed));
            assert_eq!(
                result.unwrap_err().message(),
                "Current group failed to load"
            );
        }
    }

    #[test]
    fn missing_and_pending_data_are_rejected_even_on_same_group() {
        let t = group(0);
        assert_eq!(
            readiness(None, None, None, false, false, None),
            Err(ReadinessReason::NoTarget)
        );
        assert_eq!(
            readiness(Some(&t), Some(&t), None, false, false, None),
            Err(ReadinessReason::LoadedMismatch)
        );
        assert_eq!(
            readiness(Some(&t), Some(&t), Some(&t), true, true, None),
            Err(ReadinessReason::CurrentLoading)
        );
        assert_eq!(
            readiness(Some(&t), None, Some(&t), false, false, None),
            Err(ReadinessReason::TargetChanged)
        );
    }

    #[test]
    fn every_identity_component_is_checked_for_current_and_loaded_data() {
        let target = group(DERIVED_BASE);
        let mut mutations = vec![target.clone(); 8];
        mutations[0].ix += 1;
        mutations[1].fingerprint += 1;
        mutations[2].group_id = Some(crate::group_identity::GroupId::new_result());
        mutations[3].path = "/different/same-name.dat".into();
        mutations[4].derived_id = Some(999);
        mutations[5].project_generation += 1;
        mutations[6].catalog_generation += 1;
        mutations[7].size = Some(200);
        for changed in mutations {
            let ready = StandardLoad::Ready(Arc::new(XASSpectrum::new()));
            assert_eq!(
                standard_readiness(Some(&target), Some(&changed), &ready),
                Err(ReadinessReason::StandardChanged)
            );
            let state = ToolState {
                open: Some(Tool::Difference),
                standard: Some(target.clone()),
                ..ToolState::new()
            };
            assert!(!state.accepts_standard_result(0, &target, Some(&changed)));
            assert_eq!(
                readiness(
                    Some(&target),
                    Some(&changed),
                    Some(&target),
                    false,
                    false,
                    None
                ),
                Err(ReadinessReason::TargetChanged)
            );
            assert_eq!(
                readiness(
                    Some(&target),
                    Some(&target),
                    Some(&changed),
                    false,
                    false,
                    None
                ),
                Err(ReadinessReason::LoadedMismatch)
            );
        }
    }

    #[test]
    fn default_standard_uses_list_order_even_when_only_later_group_is_cached() {
        let target = group(0);
        let first = group(DERIVED_BASE);
        let later = group(1);
        let marked = BTreeSet::from([target.ix, first.ix, later.ix, crate::app::NO_ENTRY]);
        let mut cache = lru::LruCache::new(NonZeroUsize::new(1).unwrap());
        cache.put((later.ix, later.fingerprint), Arc::new(XASSpectrum::new()));
        for tool in [Tool::Calibrate, Tool::Align, Tool::Difference] {
            let chosen = default_standard(
                tool,
                Some(&target),
                [first.clone(), target.clone(), later.clone()],
                &marked,
            )
            .unwrap();
            assert_eq!(chosen, first);
            assert!(!cache.contains(&(chosen.ix, chosen.fingerprint)));
            assert_eq!(
                standard_readiness(Some(&chosen), Some(&chosen), &StandardLoad::Loading),
                Err(ReadinessReason::StandardLoading)
            );
        }
    }

    #[test]
    fn no_other_valid_marked_group_means_no_default() {
        let target = group(0);
        let marked = BTreeSet::from([0, crate::app::NO_ENTRY]);
        for tool in [Tool::Align, Tool::Difference] {
            assert_eq!(
                default_standard(tool, Some(&target), [target.clone(), group(1)], &marked),
                None
            );
        }
    }

    #[test]
    fn calibrate_defaults_to_target_without_other_marked_groups() {
        for target in [group(0), group(DERIVED_BASE)] {
            for marked in [
                BTreeSet::new(),
                BTreeSet::from([target.ix, crate::app::NO_ENTRY]),
            ] {
                for groups in [vec![target.clone()], vec![target.clone(), group(1)]] {
                    assert_eq!(
                        default_standard(Tool::Calibrate, Some(&target), groups, &marked),
                        Some(target.clone())
                    );
                }
            }
        }
        assert_eq!(
            default_standard(Tool::Calibrate, None, [], &BTreeSet::new()),
            None
        );
    }

    #[test]
    fn standard_readiness_reports_missing_failure_revision_and_removal() {
        let chosen = group(1);
        let mut changed = chosen.clone();
        changed.fingerprint += 1;
        let ready = StandardLoad::Ready(Arc::new(XASSpectrum::new()));
        assert_eq!(
            standard_readiness(None, None, &ready),
            Err(ReadinessReason::NoStandard)
        );
        assert_eq!(
            standard_readiness(Some(&chosen), Some(&changed), &ready),
            Err(ReadinessReason::StandardChanged)
        );
        assert_eq!(
            standard_readiness(Some(&chosen), None, &ready),
            Err(ReadinessReason::StandardChanged)
        );
        assert_eq!(
            standard_readiness(
                Some(&chosen),
                Some(&chosen),
                &StandardLoad::Failed("unreadable".into())
            ),
            Err(ReadinessReason::StandardFailed)
        );
        assert_eq!(
            standard_readiness(Some(&chosen), Some(&chosen), &ready),
            Ok(())
        );
        let target = group(0);
        for reason in [
            ReadinessReason::NoStandard,
            ReadinessReason::StandardChanged,
            ReadinessReason::StandardLoading,
            ReadinessReason::StandardFailed,
        ] {
            assert_eq!(
                readiness(
                    Some(&target),
                    Some(&target),
                    Some(&target),
                    false,
                    false,
                    Some(Err(reason))
                ),
                Err(reason)
            );
        }
    }

    #[test]
    fn late_standard_jobs_cannot_replace_a_new_choice_or_revision() {
        let chosen = group(1);
        let mut state = ToolState {
            open: Some(Tool::Align),
            standard: Some(chosen.clone()),
            standard_request: 2,
            ..ToolState::new()
        };
        assert!(state.accepts_standard_result(2, &chosen, Some(&chosen)));
        assert!(!state.accepts_standard_result(1, &chosen, Some(&chosen)));
        assert!(!state.accepts_standard_result(2, &chosen, Some(&group(2))));
        assert!(!state.accepts_standard_result(2, &chosen, None));
        state.standard = Some(group(2));
        assert!(!state.accepts_standard_result(2, &chosen, Some(&chosen)));
        state.standard = Some(chosen.clone());
        state.open = None;
        assert!(!state.accepts_standard_result(2, &chosen, Some(&chosen)));
    }

    #[test]
    fn invalidation_drops_pinned_data_bindings_and_pending_requests() {
        let chosen = group(DERIVED_BASE);
        let pinned = Arc::new(XASSpectrum::new());
        let mut state = ToolState {
            open: Some(Tool::Difference),
            target: Some(group(0)),
            standard: Some(chosen.clone()),
            standard_load: StandardLoad::Ready(pinned.clone()),
            standard_picker_open: true,
            standard_matches: Some(Arc::new(vec![0])),
            ..ToolState::new()
        };
        for _ in 0..2 {
            let generation = state.generation;
            let request = state.standard_request;
            let filter_request = state.standard_filter_request;
            state.invalidate_bindings();
            assert_eq!(state.generation, generation + 1);
            assert!(state.standard_filter_request > filter_request);
            assert!(state.open.is_none() && state.target.is_none() && state.standard.is_none());
            assert!(matches!(state.standard_load, StandardLoad::Loading));
            assert!(!state.standard_picker_open && state.standard_matches.is_none());
            assert_eq!(Arc::strong_count(&pinned), 1);
            // Even reopening with an identical-looking group cannot accept the old job.
            state.open = Some(Tool::Difference);
            state.standard = Some(chosen.clone());
            assert!(!state.accepts_standard_result(request, &chosen, Some(&chosen)));
        }
    }

    #[test]
    fn standalone_target_identity_roundtrip_after_relocation_uses_actual_channel() {
        use crate::group_identity::GroupRegistry;
        use crate::params::DetectionMode;
        use crate::project::{DataStorage, ProjectFile, load, save_with_storage};
        use std::collections::BTreeMap;
        let root = std::env::temp_dir().join(format!(
            "rexafs-standalone-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for mode in [DetectionMode::Transmission, DetectionMode::Reference] {
            let old = root.join(format!("old-{mode:?}"));
            std::fs::create_dir_all(&old).unwrap();
            let source = old.join("source.xmu");
            std::fs::copy(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/projects/data/cu_150k.xmu"),
                &source,
            )
            .unwrap();
            let source = source.canonicalize().unwrap();
            let registry = GroupRegistry::default();
            let params = PipelineParams {
                import: crate::params::ImportConfig {
                    mode,
                    ..Default::default()
                },
                ..Default::default()
            };
            // An independently added channel may already own the base ID.
            let mut channel = DerivedSpectrum {
                id: 1,
                source: Some(source.clone()),
                params: Some(params.clone()),
                ..Default::default()
            };
            registry.assign_group(&mut channel, &BTreeMap::new());
            let id = registry.register_source(None, source.clone(), mode, &BTreeMap::new());
            assert_ne!(Some(id.clone()), channel.group_id);
            let target = ToolTarget::standalone(
                Some(id.clone()),
                source.clone(),
                "source".into(),
                params.fingerprint(),
                1,
                1,
            );
            let output = DerivedSpectrum {
                id: 2,
                group_id: Some(crate::group_identity::GroupId::new_result()),
                energy: vec![1., 2.],
                mu: vec![3., 4.],
                operation: Some(Operation {
                    tool: "smooth".into(),
                    parameters: serde_json::json!({}),
                    inputs: vec![target.operation_input()],
                    applied_energy_shift_ev: 0.,
                }),
                ..Default::default()
            };
            let project = ProjectFile {
                version: crate::project::PROJECT_VERSION,
                params,
                spectrum_file: Some(source),
                source_groups: registry.sources(),
                derived: vec![channel, output],
                ..Default::default()
            };
            assert_eq!(project.source_groups[0].channel, mode);
            save_with_storage(&old.join("session.rxs"), &project, DataStorage::Paths).unwrap();
            let moved = root.join(format!("moved-{mode:?}"));
            std::fs::rename(&old, &moved).unwrap();
            let mut reopened = load(&moved.join("session.rxs")).unwrap();
            reopened.assign_group_ids();
            let relocated = reopened.spectrum_file.clone().unwrap();
            assert!(relocated.starts_with(moved.canonicalize().unwrap()));
            let registry = GroupRegistry::from_sources(reopened.source_groups.clone());
            let imported_id =
                registry.register_source(Some(0), relocated.clone(), mode, &BTreeMap::new());
            let input = &reopened.derived[1].operation.as_ref().unwrap().inputs[0];
            assert_eq!(input.group_id, Some(id.clone()));
            assert_eq!(imported_id, id);
            assert_eq!(input.path, relocated);
            assert_eq!(registry.index(input.group_id.as_ref().unwrap()), Some(0));
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn standalone_readiness_checks_path_parameters_and_load_state() {
        let target = ToolTarget::standalone(
            None,
            "/outside/project.dat".into(),
            "project.dat".into(),
            42,
            1,
            2,
        );
        assert_eq!(target.ix, NO_ENTRY);
        assert_eq!(
            readiness(
                Some(&target),
                Some(&target),
                Some(&target),
                false,
                false,
                None
            ),
            Ok(())
        );
        for changed in [
            ToolTarget {
                path: "/other/project.dat".into(),
                ..target.clone()
            },
            ToolTarget {
                fingerprint: 43,
                ..target.clone()
            },
            ToolTarget {
                catalog_generation: 3,
                ..target.clone()
            },
        ] {
            assert_eq!(
                readiness(
                    Some(&target),
                    Some(&changed),
                    Some(&target),
                    false,
                    false,
                    None
                ),
                Err(ReadinessReason::TargetChanged)
            );
        }
        assert_eq!(
            readiness(Some(&target), Some(&target), None, false, false, None),
            Err(ReadinessReason::LoadedMismatch)
        );
        assert_eq!(
            readiness(
                Some(&target),
                Some(&target),
                Some(&target),
                true,
                false,
                None
            ),
            Err(ReadinessReason::CurrentFailed)
        );
        let chosen =
            default_standard(Tool::Calibrate, Some(&target), [], &BTreeSet::new()).unwrap();
        assert_eq!(
            standard_readiness(
                Some(&chosen),
                Some(&target),
                &StandardLoad::Ready(Arc::new(edge(100.)))
            ),
            Ok(())
        );
    }

    #[test]
    fn picker_maps_only_requested_rows_in_stable_order() {
        assert_eq!(
            standard_row_index(None, 0, 2, 1_000_000),
            Some(DERIVED_BASE)
        );
        assert_eq!(
            standard_row_index(None, 1, 2, 1_000_000),
            Some(DERIVED_BASE + 1)
        );
        assert_eq!(standard_row_index(None, 2, 2, 1_000_000), Some(0));
        assert_eq!(
            standard_row_index(None, 1_000_001, 2, 1_000_000),
            Some(999_999)
        );
        assert_eq!(standard_row_index(None, 1_000_002, 2, 1_000_000), None);
        assert_eq!(standard_row_index(None, 0, 0, 0), None);
        let matches = [DERIVED_BASE + 1, 20, NO_ENTRY, 1_000_000];
        assert_eq!(
            standard_row_index(Some(&matches), 0, 2, 1_000_000),
            Some(DERIVED_BASE + 1)
        );
        assert_eq!(
            standard_row_index(Some(&matches), 1, 2, 1_000_000),
            Some(20)
        );
        for row in 2..5 {
            assert_eq!(standard_row_index(Some(&matches), row, 2, 1_000_000), None);
        }
    }

    fn edge(center: f64) -> XASSpectrum {
        let energy: Vec<f64> = (0..201).map(|i| center - 10.0 + i as f64 * 0.1).collect();
        let mu: Vec<f64> = energy.iter().map(|e| ((e - center) / 0.8).tanh()).collect();
        let mut sp = XASSpectrum::new();
        sp.set_spectrum(energy, mu);
        sp
    }

    #[test]
    fn alignment_manual_shift_is_additive_and_records_provenance() {
        let source = edge(110.0);
        let reference = edge(100.0);
        let before = (
            source.energy.clone(),
            source.mu.clone(),
            reference.energy.clone(),
        );
        let values = [
            (ToolField::WinLo, Some(-5.0)),
            (ToolField::WinHi, Some(5.0)),
        ];
        let (automatic, auto_operation, _) = process_tool(
            Tool::Align,
            &source,
            Some(("reference", &reference)),
            "source",
            &values,
            vec![],
        )
        .unwrap();
        for manual in [-0.4, 0.3] {
            let mut values = values.to_vec();
            values.push((ToolField::ManualShift, Some(manual)));
            let (result, operation, _) = process_tool(
                Tool::Align,
                &source,
                Some(("reference", &reference)),
                "source",
                &values,
                vec![],
            )
            .unwrap();
            assert!(
                (operation.applied_energy_shift_ev
                    - auto_operation.applied_energy_shift_ev
                    - manual)
                    .abs()
                    < 1e-12
            );
            assert_eq!(operation.parameters["manual_shift_ev"], manual);
            assert_eq!(
                operation.parameters["automatic_shift_ev"],
                auto_operation.applied_energy_shift_ev
            );
            for (a, b) in automatic
                .energy
                .as_ref()
                .unwrap()
                .iter()
                .zip(result.energy.as_ref().unwrap().iter())
            {
                assert!((b - a - manual).abs() < 1e-12);
            }
            assert_eq!(result.mu, source.mu);
            let initial = PipelineParams {
                energy_offset_ev: 1.0,
                ..Default::default()
            };
            let mut settings = aligned_parameters(&initial, &operation).unwrap();
            assert!(
                (settings.energy_offset_ev - 1.0 - operation.applied_energy_shift_ev).abs() < 1e-12
            );
            let receipt = settings.alignment_record.clone();
            assert_eq!(receipt.as_ref().unwrap().manual_shift_ev, manual);
            settings.set_energy_offset(0.0).unwrap();
            assert_eq!(settings.alignment_record, receipt);
        }
        assert_eq!(
            (
                source.energy.clone(),
                source.mu.clone(),
                reference.energy.clone()
            ),
            before
        );
        for (field, bad) in [(ToolField::ManualShift, f64::NAN), (ToolField::WinHi, -6.0)] {
            let values: Vec<_> = values
                .iter()
                .copied()
                .filter(|(f, _)| *f != field)
                .chain([(field, Some(bad))])
                .collect();
            assert!(
                process_tool(
                    Tool::Align,
                    &source,
                    Some(("reference", &reference)),
                    "source",
                    &values,
                    vec![]
                )
                .is_err()
            );
        }
    }

    #[test]
    fn tool_preview_calibration_uses_standard_and_does_not_change_operands() {
        let source = edge(110.0);
        let standard = edge(100.0);
        let before = (
            source.energy.clone(),
            source.mu.clone(),
            standard.energy.clone(),
            standard.mu.clone(),
        );
        let (result, operation, label) = process_tool(
            Tool::Calibrate,
            &source,
            Some(("foil", &standard)),
            "sample",
            &[(ToolField::Target, Some(103.0))],
            vec![],
        )
        .unwrap();
        let expected_shift = 103.0
            - standard
                .edge_feature_energy(EdgeFeature::DerivativeMax)
                .unwrap();
        assert!((2.0..4.0).contains(&expected_shift));
        assert!((operation.applied_energy_shift_ev - expected_shift).abs() < 1e-10);
        assert!(
            (result.energy.as_ref().unwrap()[0]
                - source.energy.as_ref().unwrap()[0]
                - expected_shift)
                .abs()
                < 1e-10
        );
        assert_eq!(result.mu, source.mu);
        assert!(label.contains("sample via foil"));
        assert_eq!(
            (source.energy, source.mu, standard.energy, standard.mu),
            before
        );
    }

    #[test]
    fn tool_preview_truncates_copy_and_rejects_missing_standard() {
        let source = edge(100.0);
        let before = (source.energy.clone(), source.mu.clone());
        let (result, _, _) = process_tool(
            Tool::Truncate,
            &source,
            None,
            "sample",
            &[
                (ToolField::Before, Some(95.0)),
                (ToolField::After, Some(105.0)),
            ],
            vec![],
        )
        .unwrap();
        let energy = result.energy.as_ref().unwrap();
        assert!(energy.len() < source.energy.as_ref().unwrap().len());
        assert!(energy[0] >= 95.0 && energy[energy.len() - 1] <= 105.0);
        for tool in [Tool::Calibrate, Tool::Align, Tool::Difference] {
            assert!(
                process_tool(
                    tool,
                    &source,
                    None,
                    "sample",
                    &[(ToolField::Target, Some(103.0))],
                    vec![]
                )
                .is_err()
            );
        }
        assert_eq!((source.energy, source.mu), before);
    }

    #[test]
    fn self_calibration_is_ready_and_matches_single_group_calibration() {
        for target in [group(0), group(DERIVED_BASE)] {
            let source = Arc::new(edge(100.0));
            let mut cache = lru::LruCache::new(NonZeroUsize::new(1).unwrap());
            cache.put((target.ix, target.fingerprint), source.clone());
            let chosen = default_standard(
                Tool::Calibrate,
                Some(&target),
                [target.clone()],
                &BTreeSet::new(),
            )
            .unwrap();
            // The standard loader uses the same key as the current-group load.
            let standard = cache.get(&(chosen.ix, chosen.fingerprint)).unwrap().clone();
            assert!(Arc::ptr_eq(&standard, &source));
            let load = StandardLoad::Ready(standard.clone());
            assert_eq!(
                readiness(
                    Some(&target),
                    Some(&target),
                    Some(&target),
                    false,
                    false,
                    Some(standard_readiness(Some(&chosen), Some(&target), &load)),
                ),
                Ok(())
            );

            let expected = 103.0;
            let measured = source
                .edge_feature_energy(EdgeFeature::DerivativeMax)
                .unwrap();
            let mut calibrated = (*source).clone();
            let shift = calibrate_from_standard(&mut calibrated, &standard, expected).unwrap();
            assert!((shift - (expected - measured)).abs() < 1e-10);
            let mut previous_behavior = (*source).clone();
            previous_behavior
                .calibrate(EdgeFeature::DerivativeMax, expected)
                .unwrap();
            assert_eq!(calibrated.energy, previous_behavior.energy);
            assert!(
                (calibrated
                    .edge_feature_energy(EdgeFeature::DerivativeMax)
                    .unwrap()
                    - expected)
                    .abs()
                    < 0.11
            );
            assert_eq!(source.energy, edge(100.0).energy);
        }
    }

    #[test]
    fn calibration_measures_standard_and_preserves_targets_edge_offset() {
        let standard = edge(100.0);
        let mut target = edge(110.0);
        let before = target.energy.clone().unwrap();
        // The core finder smooths and samples the derivative; use its measured
        // feature rather than assuming the analytic tanh midpoint.
        let measured = standard
            .edge_feature_energy(EdgeFeature::DerivativeMax)
            .unwrap();
        let target_feature = target
            .edge_feature_energy(EdgeFeature::DerivativeMax)
            .unwrap();
        let shift = calibrate_from_standard(&mut target, &standard, measured + 3.0).unwrap();
        assert!((shift - 3.0).abs() < 0.01);
        for (before, after) in before.iter().zip(target.energy.as_ref().unwrap().iter()) {
            assert!((after - before - shift).abs() < 1e-10);
        }
        assert!(
            (target
                .edge_feature_energy(EdgeFeature::DerivativeMax)
                .unwrap()
                - target_feature
                - 3.0)
                .abs()
                < 0.11
        );
        assert!(
            (standard
                .edge_feature_energy(EdgeFeature::DerivativeMax)
                .unwrap()
                - measured)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn invalid_calibration_standard_leaves_target_untouched() {
        let mut target = edge(110.0);
        let before = target.energy.clone();
        assert!(calibrate_from_standard(&mut target, &XASSpectrum::new(), 103.0).is_err());
        assert_eq!(target.energy, before);
    }
}
