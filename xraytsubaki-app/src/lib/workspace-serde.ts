import type { BoxBase, LayoutBase, PanelBase, TabBase } from "rc-dock";
import type { LeftSidebarLayoutState, WorkspaceLayoutPayload } from "@/backend/types";

export const WORKSPACE_LAYOUT_STORAGE_KEY = "xraytsubaki.workspace-layout.v1";

const BOX_MODES = new Set(["horizontal", "vertical", "float", "window", "maximize"]);
const LEFT_SIDEBAR_MIN = 160;
const LEFT_SIDEBAR_MAX = 420;
const DEFAULT_LEFT_SIDEBAR_WIDTH = 220;

const DEFAULT_LEFT_SIDEBAR: LeftSidebarLayoutState = {
  collapsed: false,
  width: DEFAULT_LEFT_SIDEBAR_WIDTH,
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isTabBase(value: unknown): value is TabBase {
  if (!isRecord(value)) return false;
  return value.id === undefined || typeof value.id === "string";
}

function isPanelBase(value: unknown): value is PanelBase {
  if (!isRecord(value)) return false;
  if (!Array.isArray(value.tabs)) return false;
  return value.tabs.every((tab) => isTabBase(tab));
}

function isBoxBase(value: unknown): value is BoxBase {
  if (!isRecord(value)) return false;
  if (typeof value.mode !== "string" || !BOX_MODES.has(value.mode)) return false;
  if (!Array.isArray(value.children)) return false;

  return value.children.every((child) => {
    if (isBoxBase(child)) return true;
    return isPanelBase(child);
  });
}

function isLayoutBase(value: unknown): value is LayoutBase {
  if (!isRecord(value)) return false;
  if (!isBoxBase(value.dockbox)) return false;
  if (value.floatbox !== undefined && !isBoxBase(value.floatbox)) return false;
  if (value.windowbox !== undefined && !isBoxBase(value.windowbox)) return false;
  if (value.maxbox !== undefined && !isBoxBase(value.maxbox)) return false;
  return true;
}

function clampLeftSidebarWidth(width: number): number {
  return Math.max(LEFT_SIDEBAR_MIN, Math.min(LEFT_SIDEBAR_MAX, width));
}

function parseLeftSidebarLayout(value: unknown): LeftSidebarLayoutState {
  if (!isRecord(value)) return { ...DEFAULT_LEFT_SIDEBAR };

  const collapsed =
    typeof value.collapsed === "boolean" ? value.collapsed : DEFAULT_LEFT_SIDEBAR.collapsed;
  const rawWidth =
    typeof value.width === "number" && Number.isFinite(value.width)
      ? value.width
      : DEFAULT_LEFT_SIDEBAR.width;

  return {
    collapsed,
    width: clampLeftSidebarWidth(rawWidth),
  };
}

export function createDefaultDockLayout(): LayoutBase {
  return {
    dockbox: {
      id: "root",
      mode: "vertical",
      children: [
        {
          id: "top-row",
          mode: "horizontal",
          size: 72,
          children: [
            {
              id: "plot-panel",
              size: 72,
              tabs: [{ id: "plot" }],
              activeId: "plot",
            },
            {
              id: "parameter-panel",
              size: 28,
              tabs: [{ id: "parameters" }],
              activeId: "parameters",
            },
          ],
        },
        {
          id: "bottom-panel",
          size: 28,
          tabs: [{ id: "log" }, { id: "fit" }],
          activeId: "log",
        },
      ],
    },
  };
}

export function sanitizeDockLayout(layout: unknown): LayoutBase {
  return isLayoutBase(layout) ? layout : createDefaultDockLayout();
}

export function serializeWorkspaceLayout(
  dockLayout: WorkspaceLayoutPayload["dock"] | null,
  leftSidebarCollapsed: boolean,
  leftSidebarWidth: number,
): WorkspaceLayoutPayload {
  return {
    dock: sanitizeDockLayout(dockLayout),
    left_sidebar: {
      collapsed: leftSidebarCollapsed,
      width: clampLeftSidebarWidth(leftSidebarWidth),
    },
  };
}

export function deserializeWorkspaceLayout(layout: unknown): WorkspaceLayoutPayload {
  if (isLayoutBase(layout)) {
    return {
      dock: layout,
      left_sidebar: { ...DEFAULT_LEFT_SIDEBAR },
    };
  }

  if (!isRecord(layout)) {
    return {
      dock: createDefaultDockLayout(),
      left_sidebar: { ...DEFAULT_LEFT_SIDEBAR },
    };
  }

  const dock = sanitizeDockLayout(layout.dock);
  const leftSidebar = parseLeftSidebarLayout(layout.left_sidebar);
  return {
    dock,
    left_sidebar: leftSidebar,
  };
}

export function readWorkspaceLayoutFromStorage(): WorkspaceLayoutPayload | null {
  if (typeof window === "undefined") return null;

  try {
    const raw = window.localStorage.getItem(WORKSPACE_LAYOUT_STORAGE_KEY);
    if (!raw) return null;
    return deserializeWorkspaceLayout(JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

export function writeWorkspaceLayoutToStorage(payload: WorkspaceLayoutPayload): void {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(WORKSPACE_LAYOUT_STORAGE_KEY, JSON.stringify(payload));
}
