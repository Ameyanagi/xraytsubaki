import { create } from "zustand";
import type {
  BgOptions,
  EnergyMain,
  FFTOptions,
  NormOptions,
  PlotMode,
  RenderMode,
  WorkspaceAnalysisTab,
  WorkspaceAnalysisTabState,
  WorkspaceCoreFormat,
  WorkspaceLayoutPayload,
} from "@/backend/types";

export interface AnalysisTab {
  id: string;
  label: string;
  spectrumIndex: number;
}

export type CursorTool = "select" | "pick" | "zoom" | "pan";
export type ParamTab = "e0" | "norm" | "bkg" | "fft";
export type PlotLayout = "1x1" | "1x2" | "2x1" | "2x2";
export type RenderModeSource = "auto" | "manual";
export type CoreFormat = WorkspaceCoreFormat;
type DockLayoutState = WorkspaceLayoutPayload["dock"];

type Updater<T> = T | ((prev: T) => T);

export interface PlotGroup {
  id: string;
  tabs: PlotMode[];
  activeMode: PlotMode;
}

interface AnalysisTabState {
  activeIndex: number | null;
  selectedIndices: number[];
  plotMode: PlotMode;
  energyMain: EnergyMain;
  renderMode: RenderMode;
  renderModeSource: RenderModeSource;
  coreFormat: CoreFormat;
  plotGroups: PlotGroup[];
  plotLayout: PlotLayout;
  activeGroupId: string;
  paramTab: ParamTab;
  cursorTool: CursorTool;
  pickTarget: string | null;
  normOpts: NormOptions;
  bgOpts: BgOptions;
  fftOpts: FFTOptions;
  livePreview: boolean;
}

const PLOT_MODE_TO_PARAM_TAB: Record<PlotMode, ParamTab> = {
  energy: "norm",
  mu: "e0",
  norm: "norm",
  k: "bkg",
  r: "fft",
};

const PLOT_MODE_VALUES: PlotMode[] = ["energy", "k", "r", "mu", "norm"];
const ALL_MODES: PlotMode[] = ["energy", "k", "r"];
const PLOT_LAYOUT_VALUES: PlotLayout[] = ["1x1", "1x2", "2x1", "2x2"];
const CURSOR_TOOL_VALUES: CursorTool[] = ["select", "pick", "zoom", "pan"];
const PARAM_TAB_VALUES: ParamTab[] = ["e0", "norm", "bkg", "fft"];
const RENDER_MODE_SOURCE_VALUES: RenderModeSource[] = ["auto", "manual"];
const CORE_FORMAT_VALUES: CoreFormat[] = ["png", "svg"];
const ENERGY_MAIN_VALUES: EnergyMain[] = ["mu", "norm", "flattened"];

const DEFAULT_NORM_OPTS: NormOptions = {
  pre_edge_start: -200,
  pre_edge_end: -30,
  norm_start: 150,
  norm_end: 800,
};

const DEFAULT_BG_OPTS: BgOptions = {
  rbkg: 1.0,
  kweight: 2,
  kmin: 0,
  kmax: 15,
};

const DEFAULT_FFT_OPTS: FFTOptions = {
  kmin: 2,
  kmax: 12,
  kweight: 2,
  dk: 1,
  window: "hanning",
};

let _groupCounter = 1;

function normalizePlotMode(mode: PlotMode): PlotMode {
  return mode === "mu" || mode === "norm" ? "energy" : mode;
}

function createDefaultPlotGroups(): PlotGroup[] {
  return [{ id: "g1", tabs: [...ALL_MODES], activeMode: "energy" }];
}

function clonePlotGroups(groups: PlotGroup[]): PlotGroup[] {
  return groups.map((group) => ({
    id: group.id,
    tabs: [...group.tabs],
    activeMode: group.activeMode,
  }));
}

function createDefaultTabState(spectrumIndex: number | null = null): AnalysisTabState {
  return {
    activeIndex: spectrumIndex,
    selectedIndices: spectrumIndex === null ? [] : [spectrumIndex],
    plotMode: "energy",
    energyMain: "mu",
    renderMode: "interactive",
    renderModeSource: "auto",
    coreFormat: "png",
    plotGroups: createDefaultPlotGroups(),
    plotLayout: "1x1",
    activeGroupId: "g1",
    paramTab: "norm",
    cursorTool: "select",
    pickTarget: null,
    normOpts: { ...DEFAULT_NORM_OPTS },
    bgOpts: { ...DEFAULT_BG_OPTS },
    fftOpts: { ...DEFAULT_FFT_OPTS },
    livePreview: true,
  };
}

function snapshotFromState(state: WorkspaceState): AnalysisTabState {
  return {
    activeIndex: state.activeIndex,
    selectedIndices: Array.from(state.selectedIndices).sort((a, b) => a - b),
    plotMode: state.plotMode,
    energyMain: state.energyMain,
    renderMode: state.renderMode,
    renderModeSource: state.renderModeSource,
    coreFormat: state.coreFormat,
    plotGroups: clonePlotGroups(state.plotGroups),
    plotLayout: state.plotLayout,
    activeGroupId: state.activeGroupId,
    paramTab: state.paramTab,
    cursorTool: state.cursorTool,
    pickTarget: state.pickTarget,
    normOpts: { ...state.normOpts },
    bgOpts: { ...state.bgOpts },
    fftOpts: { ...state.fftOpts },
    livePreview: state.livePreview,
  };
}

function applySnapshot(
  snapshot: AnalysisTabState,
): Pick<
  WorkspaceState,
  | "activeIndex"
  | "selectedIndices"
  | "plotMode"
  | "energyMain"
  | "renderMode"
  | "renderModeSource"
  | "coreFormat"
  | "plotGroups"
  | "plotLayout"
  | "activeGroupId"
  | "paramTab"
  | "cursorTool"
  | "pickTarget"
  | "normOpts"
  | "bgOpts"
  | "fftOpts"
  | "livePreview"
> {
  return {
    activeIndex: snapshot.activeIndex,
    selectedIndices: new Set(snapshot.selectedIndices),
    plotMode: snapshot.plotMode,
    energyMain: snapshot.energyMain,
    renderMode: snapshot.renderMode,
    renderModeSource: snapshot.renderModeSource,
    coreFormat: snapshot.coreFormat,
    plotGroups: clonePlotGroups(snapshot.plotGroups),
    plotLayout: snapshot.plotLayout,
    activeGroupId: snapshot.activeGroupId,
    paramTab: snapshot.paramTab,
    cursorTool: snapshot.cursorTool,
    pickTarget: snapshot.pickTarget,
    normOpts: { ...snapshot.normOpts },
    bgOpts: { ...snapshot.bgOpts },
    fftOpts: { ...snapshot.fftOpts },
    livePreview: snapshot.livePreview,
  };
}

function normalizeRemovedIndices(indices: number[]): number[] {
  const normalized = Array.from(
    new Set(
      indices.filter((index) => Number.isInteger(index) && Number.isFinite(index) && index >= 0),
    ),
  );
  normalized.sort((a, b) => a - b);
  return normalized;
}

function countRemovedBefore(removedSorted: number[], index: number): number {
  let lo = 0;
  let hi = removedSorted.length;
  while (lo < hi) {
    const mid = Math.floor((lo + hi) / 2);
    if (removedSorted[mid] < index) {
      lo = mid + 1;
    } else {
      hi = mid;
    }
  }
  return lo;
}

function mapIndexAfterRemoval(index: number | null, removedSorted: number[]): number | null {
  if (index === null || !Number.isInteger(index) || index < 0) return null;

  const removedBefore = countRemovedBefore(removedSorted, index);
  const isRemoved = removedBefore < removedSorted.length && removedSorted[removedBefore] === index;
  if (isRemoved) return null;
  return index - removedBefore;
}

function remapIndexList(indices: number[], removedSorted: number[]): number[] {
  const mapped = indices
    .map((index) => mapIndexAfterRemoval(index, removedSorted))
    .filter((index): index is number => index !== null);
  return Array.from(new Set(mapped)).sort((a, b) => a - b);
}

function remapTabStateAfterRemoval(
  snapshot: AnalysisTabState,
  removedSorted: number[],
): AnalysisTabState {
  const selectedIndices = remapIndexList(snapshot.selectedIndices, removedSorted);
  let activeIndex = mapIndexAfterRemoval(snapshot.activeIndex, removedSorted);
  if (activeIndex === null && selectedIndices.length > 0) {
    activeIndex = selectedIndices[0];
  }

  return {
    ...snapshot,
    activeIndex,
    selectedIndices,
  };
}

function remapSpectrumTabId(tabId: string, mappedSpectrumIndex: number): string {
  if (/^spectrum-\d+$/.test(tabId)) {
    return `spectrum-${mappedSpectrumIndex}`;
  }
  return tabId;
}

function updateCurrentTabState(
  state: WorkspaceState,
  patch: Partial<AnalysisTabState>,
): Record<string, AnalysisTabState> {
  if (!state.activeTabId) return state.tabStates;

  const current =
    state.tabStates[state.activeTabId] ??
    createDefaultTabState(
      state.activeIndex ??
        state.tabs.find((tab) => tab.id === state.activeTabId)?.spectrumIndex ??
        null,
    );

  return {
    ...state.tabStates,
    [state.activeTabId]: {
      ...current,
      ...patch,
      plotGroups: patch.plotGroups
        ? clonePlotGroups(patch.plotGroups)
        : clonePlotGroups(current.plotGroups),
      normOpts: patch.normOpts ? { ...patch.normOpts } : { ...current.normOpts },
      bgOpts: patch.bgOpts ? { ...patch.bgOpts } : { ...current.bgOpts },
      fftOpts: patch.fftOpts ? { ...patch.fftOpts } : { ...current.fftOpts },
      selectedIndices: patch.selectedIndices
        ? [...patch.selectedIndices]
        : [...current.selectedIndices],
    },
  };
}

function activateTab(state: WorkspaceState, tabId: string): Partial<WorkspaceState> {
  const tab = state.tabs.find((item) => item.id === tabId);
  if (!tab) return {};

  const tabStates = { ...state.tabStates };
  if (state.activeTabId) {
    tabStates[state.activeTabId] = snapshotFromState(state);
  }

  const target = tabStates[tabId] ?? createDefaultTabState(tab.spectrumIndex);
  tabStates[tabId] = target;
  syncGroupCounter(target.plotGroups);

  return {
    activeTabId: tabId,
    tabStates,
    ...applySnapshot(target),
  };
}

function resolveUpdater<T>(value: Updater<T>, prev: T): T {
  return typeof value === "function" ? (value as (v: T) => T)(prev) : value;
}

function calcLayout(count: number, hint?: "right" | "down", current?: PlotLayout): PlotLayout {
  if (count <= 1) return "1x1";
  if (count === 2) {
    if (hint === "down") return "2x1";
    if (hint === "right") return "1x2";
    if (current === "2x1") return "2x1";
    return "1x2";
  }
  return "2x2";
}

function syncGroupCounter(groups: PlotGroup[]): void {
  for (const group of groups) {
    const match = /^g(\d+)$/.exec(group.id);
    if (!match) continue;
    const value = Number(match[1]);
    if (Number.isFinite(value)) {
      _groupCounter = Math.max(_groupCounter, value);
    }
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isPlotMode(value: unknown): value is PlotMode {
  return typeof value === "string" && PLOT_MODE_VALUES.includes(value as PlotMode);
}

function isRenderMode(value: unknown): value is RenderMode {
  return value === "interactive" || value === "core";
}

function isRenderModeSource(value: unknown): value is RenderModeSource {
  return typeof value === "string" && RENDER_MODE_SOURCE_VALUES.includes(value as RenderModeSource);
}

function isCoreFormat(value: unknown): value is CoreFormat {
  return typeof value === "string" && CORE_FORMAT_VALUES.includes(value as CoreFormat);
}

function isEnergyMain(value: unknown): value is EnergyMain {
  return typeof value === "string" && ENERGY_MAIN_VALUES.includes(value as EnergyMain);
}

function isParamTab(value: unknown): value is ParamTab {
  return typeof value === "string" && PARAM_TAB_VALUES.includes(value as ParamTab);
}

function isCursorTool(value: unknown): value is CursorTool {
  return typeof value === "string" && CURSOR_TOOL_VALUES.includes(value as CursorTool);
}

function isPlotLayout(value: unknown): value is PlotLayout {
  return typeof value === "string" && PLOT_LAYOUT_VALUES.includes(value as PlotLayout);
}

function toNumberArray(value: unknown): number[] {
  if (!Array.isArray(value)) return [];
  return value.filter((item): item is number => typeof item === "number" && Number.isFinite(item));
}

function parsePlotGroups(value: unknown): PlotGroup[] {
  if (!Array.isArray(value)) return createDefaultPlotGroups();

  const parsed: PlotGroup[] = [];
  for (const item of value) {
    if (!isRecord(item)) continue;
    if (typeof item.id !== "string") continue;
    if (!Array.isArray(item.tabs)) continue;

    const tabs = Array.from(
      new Set(
        item.tabs
          .filter((tab): tab is PlotMode => isPlotMode(tab))
          .map((tab) => normalizePlotMode(tab)),
      ),
    );
    if (tabs.length === 0) continue;

    const rawActiveMode = isPlotMode(item.activeMode)
      ? normalizePlotMode(item.activeMode)
      : undefined;
    const activeMode = rawActiveMode && tabs.includes(rawActiveMode) ? rawActiveMode : tabs[0];

    parsed.push({ id: item.id, tabs, activeMode });
  }

  return parsed.length > 0 ? parsed : createDefaultPlotGroups();
}

function parseOptions<T extends object>(value: unknown, defaults: T): T {
  if (!isRecord(value)) return { ...defaults };
  return { ...defaults, ...(value as Partial<T>) };
}

function deserializeTabState(value: unknown, fallbackIndex: number): AnalysisTabState {
  const defaults = createDefaultTabState(fallbackIndex);
  if (!isRecord(value)) return defaults;

  const activeIndex =
    typeof value.active_index === "number"
      ? value.active_index
      : typeof value.activeIndex === "number"
        ? value.activeIndex
        : defaults.activeIndex;

  const selectedIndicesRaw = toNumberArray(value.selected_indices ?? value.selectedIndices).sort(
    (a, b) => a - b,
  );
  const selectedIndices =
    selectedIndicesRaw.length > 0
      ? selectedIndicesRaw
      : activeIndex !== null
        ? [activeIndex]
        : defaults.selectedIndices;

  const plotGroups = parsePlotGroups(value.plot_groups ?? value.plotGroups);

  const activeGroupIdRaw =
    typeof value.active_group_id === "string"
      ? value.active_group_id
      : typeof value.activeGroupId === "string"
        ? value.activeGroupId
        : defaults.activeGroupId;
  const activeGroupId = plotGroups.some((group) => group.id === activeGroupIdRaw)
    ? activeGroupIdRaw
    : plotGroups[0].id;

  const plotModeRaw = value.plot_mode ?? value.plotMode;
  const plotMode = isPlotMode(plotModeRaw) ? normalizePlotMode(plotModeRaw) : defaults.plotMode;
  const energyMainRaw = value.energy_main ?? value.energyMain;
  const energyMain = isEnergyMain(energyMainRaw)
    ? energyMainRaw
    : plotModeRaw === "norm"
      ? "norm"
      : plotModeRaw === "mu"
        ? "mu"
        : defaults.energyMain;
  const renderModeRaw = value.render_mode ?? value.renderMode;
  const renderModeSourceRaw = value.render_mode_source ?? value.renderModeSource;
  const coreFormatRaw = value.core_format ?? value.coreFormat;
  const plotLayoutRaw = value.plot_layout ?? value.plotLayout;
  const paramTabRaw = value.param_tab ?? value.paramTab;
  const cursorToolRaw = value.cursor_tool ?? value.cursorTool;

  return {
    activeIndex,
    selectedIndices,
    plotMode,
    energyMain,
    renderMode: isRenderMode(renderModeRaw) ? renderModeRaw : defaults.renderMode,
    renderModeSource: isRenderModeSource(renderModeSourceRaw)
      ? renderModeSourceRaw
      : defaults.renderModeSource,
    coreFormat: isCoreFormat(coreFormatRaw) ? coreFormatRaw : defaults.coreFormat,
    plotGroups,
    plotLayout: isPlotLayout(plotLayoutRaw) ? plotLayoutRaw : calcLayout(plotGroups.length),
    activeGroupId,
    paramTab: isParamTab(paramTabRaw) ? paramTabRaw : PLOT_MODE_TO_PARAM_TAB[plotMode],
    cursorTool: isCursorTool(cursorToolRaw) ? cursorToolRaw : defaults.cursorTool,
    pickTarget:
      typeof value.pick_target === "string"
        ? value.pick_target
        : typeof value.pickTarget === "string"
          ? value.pickTarget
          : null,
    normOpts: parseOptions(value.norm_options ?? value.normOpts, DEFAULT_NORM_OPTS),
    bgOpts: parseOptions(value.bg_options ?? value.bgOpts, DEFAULT_BG_OPTS),
    fftOpts: parseOptions(value.fft_options ?? value.fftOpts, DEFAULT_FFT_OPTS),
    livePreview:
      typeof value.live_preview === "boolean"
        ? value.live_preview
        : typeof value.livePreview === "boolean"
          ? value.livePreview
          : defaults.livePreview,
  };
}

function serializeTabState(state: AnalysisTabState): WorkspaceAnalysisTabState {
  return {
    active_index: state.activeIndex,
    selected_indices: [...state.selectedIndices].sort((a, b) => a - b),
    plot_mode: state.plotMode,
    energy_main: state.energyMain,
    render_mode: state.renderMode,
    render_mode_source: state.renderModeSource,
    core_format: state.coreFormat,
    plot_groups: clonePlotGroups(state.plotGroups),
    plot_layout: state.plotLayout,
    active_group_id: state.activeGroupId,
    param_tab: state.paramTab,
    cursor_tool: state.cursorTool,
    pick_target: state.pickTarget,
    norm_options: { ...state.normOpts },
    bg_options: { ...state.bgOpts },
    fft_options: { ...state.fftOpts },
    live_preview: state.livePreview,
  };
}

interface ParsedWorkspaceTab {
  tab: AnalysisTab;
  state: AnalysisTabState;
  active: boolean;
}

function parseWorkspaceTab(value: unknown): ParsedWorkspaceTab | null {
  if (!isRecord(value)) return null;

  const spectrumIndex =
    typeof value.spectrumIndex === "number"
      ? value.spectrumIndex
      : typeof value.spectrum_index === "number"
        ? value.spectrum_index
        : null;
  if (spectrumIndex === null) return null;

  const id =
    typeof value.id === "string" && value.id.length > 0 ? value.id : `spectrum-${spectrumIndex}`;
  const label =
    typeof value.label === "string" && value.label.length > 0
      ? value.label
      : `Spectrum #${spectrumIndex}`;

  const state = deserializeTabState(value.state, spectrumIndex);

  return {
    tab: { id, label, spectrumIndex },
    state,
    active: value.active === true,
  };
}

interface WorkspaceState {
  // Analysis tabs
  tabs: AnalysisTab[];
  activeTabId: string | null;
  tabStates: Record<string, AnalysisTabState>;
  addTab: (tab: AnalysisTab) => void;
  openSpectrumTab: (index: number, label: string) => void;
  removeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
  exportTabsForWorkspace: () => WorkspaceAnalysisTab[];
  importTabsFromWorkspace: (tabs: unknown[]) => void;

  // Active tab's spectra context
  selectedIndices: Set<number>;
  activeIndex: number | null;
  setActiveIndex: (index: number | null) => void;
  toggleSelection: (index: number) => void;
  selectRange: (from: number, to: number) => void;
  selectAll: (indices: number[]) => void;
  clearSelection: () => void;
  setSelectedIndices: (indices: Set<number>) => void;
  remapAfterSpectraRemoval: (removedIndices: number[]) => void;

  // Plot mode (reflects active group's mode)
  plotMode: PlotMode;
  energyMain: EnergyMain;
  setPlotMode: (mode: PlotMode) => void;
  setEnergyMain: (mode: EnergyMain) => void;
  renderMode: RenderMode;
  renderModeSource: RenderModeSource;
  coreFormat: CoreFormat;
  setRenderMode: (mode: RenderMode) => void;
  setAutoRenderMode: (mode: RenderMode) => void;
  enableRenderModeAuto: () => void;
  setCoreFormat: (format: CoreFormat) => void;

  // Plot groups (VS Code-style editor groups)
  plotGroups: PlotGroup[];
  plotLayout: PlotLayout;
  activeGroupId: string;
  splitGroup: (groupId: string, direction: "right" | "down") => void;
  closeGroup: (groupId: string) => void;
  addPlotTab: (groupId: string, mode: PlotMode) => void;
  removePlotTab: (groupId: string, mode: PlotMode) => void;
  setGroupActiveMode: (groupId: string, mode: PlotMode) => void;
  setActiveGroup: (groupId: string) => void;

  // Parameter tab (linked to plot mode)
  paramTab: ParamTab;
  setParamTab: (tab: ParamTab) => void;
  normOpts: NormOptions;
  setNormOpts: (opts: Updater<NormOptions>) => void;
  bgOpts: BgOptions;
  setBgOpts: (opts: Updater<BgOptions>) => void;
  fftOpts: FFTOptions;
  setFftOpts: (opts: Updater<FFTOptions>) => void;
  livePreview: boolean;
  setLivePreview: (enabled: boolean) => void;

  // Cursor tool & pick mode
  cursorTool: CursorTool;
  setCursorTool: (tool: CursorTool) => void;
  pickTarget: string | null;
  setPickTarget: (target: string | null) => void;
  // Called when a value is picked from the plot
  pickListeners: Map<string, (value: number) => void>;
  onPickValue: (target: string, value: number) => void;
  registerPickListener: (target: string, callback: (value: number) => void) => void;
  unregisterPickListener: (target: string) => void;

  // Theme
  theme: string;
  setTheme: (theme: string) => void;

  // Shell layout
  leftSidebarCollapsed: boolean;
  setLeftSidebarCollapsed: (collapsed: boolean) => void;
  toggleLeftSidebarCollapsed: () => void;
  leftSidebarWidth: number;
  setLeftSidebarWidth: (width: number) => void;
  dockLayout: DockLayoutState | null;
  setDockLayout: (layout: DockLayoutState | null) => void;

  // Workspace path
  workspacePath: string | null;
  setWorkspacePath: (path: string | null) => void;
}

const LEFT_SIDEBAR_MIN = 160;
const LEFT_SIDEBAR_MAX = 420;
const LEFT_SIDEBAR_DEFAULT = 220;

function clampLeftSidebarWidth(width: number): number {
  return Math.max(LEFT_SIDEBAR_MIN, Math.min(LEFT_SIDEBAR_MAX, width));
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  // Analysis tabs
  tabs: [],
  activeTabId: null,
  tabStates: {},

  addTab: (tab) =>
    set((state) => {
      const exists = state.tabs.find((item) => item.id === tab.id);
      if (exists) {
        return activateTab(state, tab.id);
      }

      const tabState = createDefaultTabState(tab.spectrumIndex);
      syncGroupCounter(tabState.plotGroups);
      return {
        tabs: [...state.tabs, tab],
        activeTabId: tab.id,
        tabStates: {
          ...state.tabStates,
          [tab.id]: tabState,
        },
        ...applySnapshot(tabState),
      };
    }),

  openSpectrumTab: (index, label) =>
    set((state) => {
      const tabId = `spectrum-${index}`;
      const exists = state.tabs.find((item) => item.id === tabId);
      if (exists) {
        const tabs = state.tabs.map((item) =>
          item.id === tabId ? { ...item, label, spectrumIndex: index } : item,
        );
        return {
          tabs,
          ...activateTab({ ...state, tabs }, tabId),
        };
      }

      const tab: AnalysisTab = {
        id: tabId,
        label,
        spectrumIndex: index,
      };
      const tabState = createDefaultTabState(index);
      syncGroupCounter(tabState.plotGroups);

      return {
        tabs: [...state.tabs, tab],
        activeTabId: tabId,
        tabStates: {
          ...state.tabStates,
          [tabId]: tabState,
        },
        ...applySnapshot(tabState),
      };
    }),

  removeTab: (id) =>
    set((state) => {
      if (!state.tabs.some((item) => item.id === id)) return {};

      const tabs = state.tabs.filter((item) => item.id !== id);
      const tabStates = { ...state.tabStates };

      if (state.activeTabId) {
        tabStates[state.activeTabId] = snapshotFromState(state);
      }
      delete tabStates[id];

      if (tabs.length === 0) {
        const defaults = createDefaultTabState(null);
        return {
          tabs: [],
          activeTabId: null,
          tabStates: {},
          ...applySnapshot(defaults),
        };
      }

      if (state.activeTabId !== id) {
        return {
          tabs,
          tabStates,
        };
      }

      const fallback = tabs[tabs.length - 1];
      const fallbackState = tabStates[fallback.id] ?? createDefaultTabState(fallback.spectrumIndex);
      tabStates[fallback.id] = fallbackState;
      syncGroupCounter(fallbackState.plotGroups);

      return {
        tabs,
        activeTabId: fallback.id,
        tabStates,
        ...applySnapshot(fallbackState),
      };
    }),

  setActiveTab: (id) => set((state) => activateTab(state, id)),

  exportTabsForWorkspace: () => {
    const state = get();
    const tabStates = { ...state.tabStates };
    if (state.activeTabId) {
      tabStates[state.activeTabId] = snapshotFromState(state);
    }

    return state.tabs.map((tab) => {
      const snapshot = tabStates[tab.id] ?? createDefaultTabState(tab.spectrumIndex);
      return {
        id: tab.id,
        label: tab.label,
        spectrumIndex: tab.spectrumIndex,
        ...(tab.id === state.activeTabId ? { active: true } : {}),
        state: serializeTabState(snapshot),
      };
    });
  },

  importTabsFromWorkspace: (rawTabs) =>
    set(() => {
      const parsed: ParsedWorkspaceTab[] = [];
      const seen = new Set<string>();
      for (const raw of rawTabs) {
        const tab = parseWorkspaceTab(raw);
        if (!tab || seen.has(tab.tab.id)) continue;
        seen.add(tab.tab.id);
        syncGroupCounter(tab.state.plotGroups);
        parsed.push(tab);
      }

      if (parsed.length === 0) {
        const defaults = createDefaultTabState(null);
        return {
          tabs: [] as AnalysisTab[],
          activeTabId: null,
          tabStates: {} as Record<string, AnalysisTabState>,
          ...applySnapshot(defaults),
        };
      }

      const tabs = parsed.map((item) => item.tab);
      const tabStates: Record<string, AnalysisTabState> = Object.fromEntries(
        parsed.map((item) => [item.tab.id, item.state]),
      );
      const activeTabId = parsed.find((item) => item.active)?.tab.id ?? tabs[0].id;
      const activeState = tabStates[activeTabId] ?? createDefaultTabState(tabs[0].spectrumIndex);

      return {
        tabs,
        activeTabId,
        tabStates,
        ...applySnapshot(activeState),
      };
    }),

  // Active tab spectra context
  selectedIndices: new Set(),
  activeIndex: null,
  setActiveIndex: (index) =>
    set((state) => {
      const selected = new Set(state.selectedIndices);
      if (index === null) {
        selected.clear();
      } else if (selected.size === 0) {
        selected.add(index);
      }

      return {
        activeIndex: index,
        selectedIndices: selected,
        tabStates: updateCurrentTabState(state, {
          activeIndex: index,
          selectedIndices: Array.from(selected),
        }),
      };
    }),

  toggleSelection: (index) =>
    set((state) => {
      const selected = new Set(state.selectedIndices);
      if (selected.has(index)) {
        selected.delete(index);
      } else {
        selected.add(index);
      }

      return {
        selectedIndices: selected,
        tabStates: updateCurrentTabState(state, {
          selectedIndices: Array.from(selected),
        }),
      };
    }),

  selectRange: (from, to) =>
    set((state) => {
      const selected = new Set(state.selectedIndices);
      const start = Math.min(from, to);
      const end = Math.max(from, to);
      for (let i = start; i <= end; i++) {
        selected.add(i);
      }

      return {
        selectedIndices: selected,
        tabStates: updateCurrentTabState(state, {
          selectedIndices: Array.from(selected),
        }),
      };
    }),

  selectAll: (indices) =>
    set((state) => {
      const selected = new Set(indices);
      return {
        selectedIndices: selected,
        tabStates: updateCurrentTabState(state, {
          selectedIndices: Array.from(selected),
        }),
      };
    }),

  clearSelection: () =>
    set((state) => ({
      selectedIndices: new Set(),
      tabStates: updateCurrentTabState(state, {
        selectedIndices: [],
      }),
    })),

  setSelectedIndices: (indices) =>
    set((state) => ({
      selectedIndices: indices,
      tabStates: updateCurrentTabState(state, {
        selectedIndices: Array.from(indices),
      }),
    })),

  remapAfterSpectraRemoval: (removedIndices) =>
    set((state) => {
      const removedSorted = normalizeRemovedIndices(removedIndices);
      if (removedSorted.length === 0) return {};

      const sourceTabStates = { ...state.tabStates };
      if (state.activeTabId) {
        sourceTabStates[state.activeTabId] = snapshotFromState(state);
      }

      const tabs: AnalysisTab[] = [];
      const tabStates: Record<string, AnalysisTabState> = {};
      const idMap = new Map<string, string>();
      const usedIds = new Set<string>();

      for (const tab of state.tabs) {
        const mappedSpectrumIndex = mapIndexAfterRemoval(tab.spectrumIndex, removedSorted);
        if (mappedSpectrumIndex === null) continue;

        let mappedTabId = remapSpectrumTabId(tab.id, mappedSpectrumIndex);
        if (usedIds.has(mappedTabId)) {
          let suffix = 2;
          while (usedIds.has(`${mappedTabId}-${suffix}`)) {
            suffix += 1;
          }
          mappedTabId = `${mappedTabId}-${suffix}`;
        }
        usedIds.add(mappedTabId);
        idMap.set(tab.id, mappedTabId);

        const sourceState = sourceTabStates[tab.id] ?? createDefaultTabState(tab.spectrumIndex);
        tabStates[mappedTabId] = remapTabStateAfterRemoval(sourceState, removedSorted);
        tabs.push({
          ...tab,
          id: mappedTabId,
          spectrumIndex: mappedSpectrumIndex,
        });
      }

      if (tabs.length === 0) {
        const defaults = createDefaultTabState(null);
        return {
          tabs: [],
          activeTabId: null,
          tabStates: {},
          ...applySnapshot(defaults),
        };
      }

      let activeTabId = state.activeTabId !== null ? (idMap.get(state.activeTabId) ?? null) : null;
      if (!activeTabId) {
        activeTabId = tabs[0].id;
      }

      const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? tabs[0];
      let activeState = tabStates[activeTabId] ?? createDefaultTabState(activeTab.spectrumIndex);
      if (activeState.activeIndex === null) {
        activeState = {
          ...activeState,
          activeIndex: activeTab.spectrumIndex,
          selectedIndices:
            activeState.selectedIndices.length > 0
              ? activeState.selectedIndices
              : [activeTab.spectrumIndex],
        };
        tabStates[activeTabId] = activeState;
      }
      syncGroupCounter(activeState.plotGroups);

      return {
        tabs,
        activeTabId,
        tabStates,
        ...applySnapshot(activeState),
      };
    }),

  // Plot mode — updates active group + param tab
  plotMode: "energy",
  energyMain: "mu",
  setPlotMode: (mode) =>
    set((state) => {
      const normalizedMode = normalizePlotMode(mode);
      const plotGroups = state.plotGroups.map((group) =>
        group.id === state.activeGroupId ? { ...group, activeMode: normalizedMode } : group,
      );
      const paramTab = PLOT_MODE_TO_PARAM_TAB[normalizedMode];
      return {
        plotGroups,
        plotMode: normalizedMode,
        paramTab,
        tabStates: updateCurrentTabState(state, {
          plotGroups,
          plotMode: normalizedMode,
          paramTab,
        }),
      };
    }),
  setEnergyMain: (mode) =>
    set((state) => ({
      energyMain: mode,
      tabStates: updateCurrentTabState(state, {
        energyMain: mode,
      }),
    })),

  renderMode: "interactive",
  renderModeSource: "auto",
  coreFormat: "png",
  setRenderMode: (mode) =>
    set((state) => ({
      renderMode: mode,
      renderModeSource: "manual",
      tabStates: updateCurrentTabState(state, {
        renderMode: mode,
        renderModeSource: "manual",
      }),
    })),
  setAutoRenderMode: (mode) =>
    set((state) => ({
      renderMode: mode,
      renderModeSource: "auto",
      tabStates: updateCurrentTabState(state, {
        renderMode: mode,
        renderModeSource: "auto",
      }),
    })),
  enableRenderModeAuto: () =>
    set((state) => ({
      renderModeSource: "auto",
      tabStates: updateCurrentTabState(state, {
        renderModeSource: "auto",
      }),
    })),
  setCoreFormat: (format) =>
    set((state) => ({
      coreFormat: format,
      tabStates: updateCurrentTabState(state, {
        coreFormat: format,
      }),
    })),

  // Plot groups
  plotGroups: createDefaultPlotGroups(),
  plotLayout: "1x1",
  activeGroupId: "g1",

  splitGroup: (groupId, direction) =>
    set((state) => {
      if (state.plotGroups.length >= 4) return {};
      const source = state.plotGroups.find((group) => group.id === groupId);
      if (!source) return {};

      const usedModes = new Set(state.plotGroups.map((group) => group.activeMode));
      const nextMode = ALL_MODES.find((mode) => !usedModes.has(mode)) ?? source.activeMode;

      const newGroup: PlotGroup = {
        id: `g${++_groupCounter}`,
        tabs: [...ALL_MODES],
        activeMode: nextMode,
      };
      const plotGroups = [...state.plotGroups, newGroup];
      const plotLayout = calcLayout(plotGroups.length, direction, state.plotLayout);

      return {
        plotGroups,
        plotLayout,
        tabStates: updateCurrentTabState(state, {
          plotGroups,
          plotLayout,
        }),
      };
    }),

  closeGroup: (groupId) =>
    set((state) => {
      if (state.plotGroups.length <= 1) return {};

      const plotGroups = state.plotGroups.filter((group) => group.id !== groupId);
      const activeGroupId =
        state.activeGroupId === groupId ? plotGroups[0].id : state.activeGroupId;
      const activeGroup = plotGroups.find((group) => group.id === activeGroupId) ?? plotGroups[0];
      const plotLayout = calcLayout(plotGroups.length, undefined, state.plotLayout);
      const plotMode = normalizePlotMode(activeGroup.activeMode);
      const paramTab = PLOT_MODE_TO_PARAM_TAB[plotMode];

      return {
        plotGroups,
        plotLayout,
        activeGroupId,
        plotMode,
        paramTab,
        tabStates: updateCurrentTabState(state, {
          plotGroups,
          plotLayout,
          activeGroupId,
          plotMode,
          paramTab,
        }),
      };
    }),

  addPlotTab: (groupId, mode) =>
    set((state) => {
      const normalizedMode = normalizePlotMode(mode);
      const plotGroups = state.plotGroups.map((group) =>
        group.id === groupId && !group.tabs.includes(normalizedMode)
          ? { ...group, tabs: [...group.tabs, normalizedMode], activeMode: normalizedMode }
          : group,
      );
      return {
        plotGroups,
        tabStates: updateCurrentTabState(state, {
          plotGroups,
        }),
      };
    }),

  removePlotTab: (groupId, mode) =>
    set((state) => {
      const normalizedMode = normalizePlotMode(mode);
      const plotGroups = state.plotGroups.map((group) => {
        if (group.id !== groupId || group.tabs.length <= 1) return group;
        const tabs = group.tabs.filter((tabMode) => tabMode !== normalizedMode);
        const activeMode = group.activeMode === normalizedMode ? tabs[0] : group.activeMode;
        return { ...group, tabs, activeMode };
      });

      return {
        plotGroups,
        tabStates: updateCurrentTabState(state, {
          plotGroups,
        }),
      };
    }),

  setGroupActiveMode: (groupId, mode) =>
    set((state) => {
      const normalizedMode = normalizePlotMode(mode);
      const plotGroups = state.plotGroups.map((group) =>
        group.id === groupId ? { ...group, activeMode: normalizedMode } : group,
      );
      const isActiveGroup = groupId === state.activeGroupId;
      const patch: Partial<AnalysisTabState> = { plotGroups };
      if (isActiveGroup) {
        patch.plotMode = normalizedMode;
        patch.paramTab = PLOT_MODE_TO_PARAM_TAB[normalizedMode];
      }

      return {
        plotGroups,
        ...(isActiveGroup
          ? { plotMode: normalizedMode, paramTab: PLOT_MODE_TO_PARAM_TAB[normalizedMode] }
          : {}),
        tabStates: updateCurrentTabState(state, patch),
      };
    }),

  setActiveGroup: (groupId) =>
    set((state) => {
      const group = state.plotGroups.find((item) => item.id === groupId);
      if (!group) return {};

      const plotMode = normalizePlotMode(group.activeMode);
      const paramTab = PLOT_MODE_TO_PARAM_TAB[plotMode];

      return {
        activeGroupId: groupId,
        plotMode,
        paramTab,
        tabStates: updateCurrentTabState(state, {
          activeGroupId: groupId,
          plotMode,
          paramTab,
        }),
      };
    }),

  // Parameter tab + options
  paramTab: "norm",
  setParamTab: (tab) =>
    set((state) => ({
      paramTab: tab,
      tabStates: updateCurrentTabState(state, {
        paramTab: tab,
      }),
    })),

  normOpts: { ...DEFAULT_NORM_OPTS },
  setNormOpts: (opts) =>
    set((state) => {
      const normOpts = resolveUpdater(opts, state.normOpts);
      return {
        normOpts,
        tabStates: updateCurrentTabState(state, { normOpts }),
      };
    }),

  bgOpts: { ...DEFAULT_BG_OPTS },
  setBgOpts: (opts) =>
    set((state) => {
      const bgOpts = resolveUpdater(opts, state.bgOpts);
      return {
        bgOpts,
        tabStates: updateCurrentTabState(state, { bgOpts }),
      };
    }),

  fftOpts: { ...DEFAULT_FFT_OPTS },
  setFftOpts: (opts) =>
    set((state) => {
      const fftOpts = resolveUpdater(opts, state.fftOpts);
      return {
        fftOpts,
        tabStates: updateCurrentTabState(state, { fftOpts }),
      };
    }),

  livePreview: true,
  setLivePreview: (enabled) =>
    set((state) => ({
      livePreview: enabled,
      tabStates: updateCurrentTabState(state, { livePreview: enabled }),
    })),

  // Cursor tool & pick mode
  cursorTool: "select",
  setCursorTool: (tool) =>
    set((state) => {
      const pickTarget = tool === "pick" ? state.pickTarget : null;
      return {
        cursorTool: tool,
        pickTarget,
        tabStates: updateCurrentTabState(state, {
          cursorTool: tool,
          pickTarget,
        }),
      };
    }),

  pickTarget: null,
  setPickTarget: (target) =>
    set((state) => ({
      pickTarget: target,
      cursorTool: target ? "pick" : "select",
      tabStates: updateCurrentTabState(state, {
        pickTarget: target,
        cursorTool: target ? "pick" : "select",
      }),
    })),

  pickListeners: new Map(),
  onPickValue: (target, value) => {
    const listener = get().pickListeners.get(target);
    if (listener) listener(value);
    set((state) => ({
      pickTarget: null,
      cursorTool: "select",
      tabStates: updateCurrentTabState(state, {
        pickTarget: null,
        cursorTool: "select",
      }),
    }));
  },

  registerPickListener: (target, callback) =>
    set((state) => {
      const next = new Map(state.pickListeners);
      next.set(target, callback);
      return { pickListeners: next };
    }),

  unregisterPickListener: (target) =>
    set((state) => {
      const next = new Map(state.pickListeners);
      next.delete(target);
      return { pickListeners: next };
    }),

  // Theme
  theme: "slate-pro",
  setTheme: (theme) => set({ theme }),

  // Shell layout
  leftSidebarCollapsed: false,
  setLeftSidebarCollapsed: (collapsed) => set({ leftSidebarCollapsed: collapsed }),
  toggleLeftSidebarCollapsed: () =>
    set((state) => ({ leftSidebarCollapsed: !state.leftSidebarCollapsed })),
  leftSidebarWidth: LEFT_SIDEBAR_DEFAULT,
  setLeftSidebarWidth: (width) => set({ leftSidebarWidth: clampLeftSidebarWidth(width) }),
  dockLayout: null,
  setDockLayout: (layout) => set({ dockLayout: layout }),

  // Workspace path
  workspacePath: null,
  setWorkspacePath: (path) => set({ workspacePath: path }),
}));
