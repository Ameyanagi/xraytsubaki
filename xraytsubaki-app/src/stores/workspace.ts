import { create } from "zustand";
import type { PlotMode, RenderMode } from "@/backend/types";

export interface AnalysisTab {
  id: string;
  label: string;
  spectrumIndex: number;
}

export type CursorTool = "select" | "pick" | "zoom" | "pan";
export type ParamTab = "e0" | "norm" | "bkg" | "fft";
export type PlotLayout = "1x1" | "1x2" | "2x1" | "2x2";

export interface PlotGroup {
  id: string;
  tabs: PlotMode[];
  activeMode: PlotMode;
}

const PLOT_MODE_TO_PARAM_TAB: Record<PlotMode, ParamTab> = {
  mu: "e0",
  norm: "norm",
  k: "bkg",
  r: "fft",
};

const ALL_MODES: PlotMode[] = ["mu", "norm", "k", "r"];

let _groupCounter = 1;

function calcLayout(count: number, hint?: "right" | "down", current?: PlotLayout): PlotLayout {
  if (count <= 1) return "1x1";
  if (count === 2) {
    if (hint === "down") return "2x1";
    if (hint === "right") return "1x2";
    // Keep current direction if already 2-panel
    if (current === "2x1") return "2x1";
    return "1x2";
  }
  return "2x2";
}

interface WorkspaceState {
  // Analysis tabs
  tabs: AnalysisTab[];
  activeTabId: string | null;
  addTab: (tab: AnalysisTab) => void;
  removeTab: (id: string) => void;
  setActiveTab: (id: string) => void;

  // Plot mode (reflects active group's mode)
  plotMode: PlotMode;
  setPlotMode: (mode: PlotMode) => void;
  renderMode: RenderMode;
  setRenderMode: (mode: RenderMode) => void;

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

  // Workspace path
  workspacePath: string | null;
  setWorkspacePath: (path: string | null) => void;
}

export const useWorkspaceStore = create<WorkspaceState>((set) => ({
  // Analysis tabs
  tabs: [],
  activeTabId: null,
  addTab: (tab) =>
    set((state) => {
      const exists = state.tabs.find((t) => t.id === tab.id);
      if (exists) return { activeTabId: tab.id };
      return { tabs: [...state.tabs, tab], activeTabId: tab.id };
    }),
  removeTab: (id) =>
    set((state) => {
      const tabs = state.tabs.filter((t) => t.id !== id);
      const activeTabId =
        state.activeTabId === id ? (tabs[tabs.length - 1]?.id ?? null) : state.activeTabId;
      return { tabs, activeTabId };
    }),
  setActiveTab: (id) => set({ activeTabId: id }),

  // Plot mode — updates active group + param tab
  plotMode: "mu",
  setPlotMode: (mode) =>
    set((state) => {
      const groups = state.plotGroups.map((g) =>
        g.id === state.activeGroupId ? { ...g, activeMode: mode } : g,
      );
      return { plotGroups: groups, plotMode: mode, paramTab: PLOT_MODE_TO_PARAM_TAB[mode] };
    }),
  renderMode: "interactive",
  setRenderMode: (mode) => set({ renderMode: mode }),

  // Plot groups — default single group with all 4 modes
  plotGroups: [{ id: "g1", tabs: [...ALL_MODES], activeMode: "mu" as PlotMode }],
  plotLayout: "1x1",
  activeGroupId: "g1",

  splitGroup: (groupId, direction) =>
    set((state) => {
      if (state.plotGroups.length >= 4) return {};
      const source = state.plotGroups.find((g) => g.id === groupId);
      if (!source) return {};

      // Pick a different active mode for the new group
      const usedModes = new Set(state.plotGroups.map((g) => g.activeMode));
      const nextMode = ALL_MODES.find((m) => !usedModes.has(m)) ?? source.activeMode;

      const newGroup: PlotGroup = {
        id: `g${++_groupCounter}`,
        tabs: [...ALL_MODES],
        activeMode: nextMode,
      };
      const groups = [...state.plotGroups, newGroup];
      return {
        plotGroups: groups,
        plotLayout: calcLayout(groups.length, direction, state.plotLayout),
      };
    }),

  closeGroup: (groupId) =>
    set((state) => {
      if (state.plotGroups.length <= 1) return {}; // Can't close last group
      const groups = state.plotGroups.filter((g) => g.id !== groupId);
      const activeGroupId =
        state.activeGroupId === groupId ? groups[0].id : state.activeGroupId;
      const activeGroup = groups.find((g) => g.id === activeGroupId) ?? groups[0];
      return {
        plotGroups: groups,
        plotLayout: calcLayout(groups.length, undefined, state.plotLayout),
        activeGroupId,
        plotMode: activeGroup.activeMode,
        paramTab: PLOT_MODE_TO_PARAM_TAB[activeGroup.activeMode],
      };
    }),

  addPlotTab: (groupId, mode) =>
    set((state) => ({
      plotGroups: state.plotGroups.map((g) =>
        g.id === groupId && !g.tabs.includes(mode)
          ? { ...g, tabs: [...g.tabs, mode], activeMode: mode }
          : g,
      ),
    })),

  removePlotTab: (groupId, mode) =>
    set((state) => ({
      plotGroups: state.plotGroups.map((g) => {
        if (g.id !== groupId || g.tabs.length <= 1) return g;
        const tabs = g.tabs.filter((t) => t !== mode);
        const activeMode = g.activeMode === mode ? tabs[0] : g.activeMode;
        return { ...g, tabs, activeMode };
      }),
    })),

  setGroupActiveMode: (groupId, mode) =>
    set((state) => {
      const groups = state.plotGroups.map((g) =>
        g.id === groupId ? { ...g, activeMode: mode } : g,
      );
      const isActive = groupId === state.activeGroupId;
      return {
        plotGroups: groups,
        ...(isActive ? { plotMode: mode, paramTab: PLOT_MODE_TO_PARAM_TAB[mode] } : {}),
      };
    }),

  setActiveGroup: (groupId) =>
    set((state) => {
      const group = state.plotGroups.find((g) => g.id === groupId);
      if (!group) return {};
      return {
        activeGroupId: groupId,
        plotMode: group.activeMode,
        paramTab: PLOT_MODE_TO_PARAM_TAB[group.activeMode],
      };
    }),

  // Parameter tab
  paramTab: "e0",
  setParamTab: (tab) => set({ paramTab: tab }),

  // Cursor tool & pick mode
  cursorTool: "select",
  setCursorTool: (tool) =>
    set((state) => ({ cursorTool: tool, pickTarget: tool === "pick" ? state.pickTarget : null })),
  pickTarget: null,
  setPickTarget: (target) => set({ pickTarget: target, cursorTool: target ? "pick" : "select" }),
  pickListeners: new Map(),
  onPickValue: (target, value) => {
    const listeners = useWorkspaceStore.getState().pickListeners;
    const listener = listeners.get(target);
    if (listener) listener(value);
    set({ pickTarget: null, cursorTool: "select" });
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

  // Workspace path
  workspacePath: null,
  setWorkspacePath: (path) => set({ workspacePath: path }),
}));
