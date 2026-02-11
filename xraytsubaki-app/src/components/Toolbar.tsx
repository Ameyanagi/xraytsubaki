import { FolderOpen, Save, Play, Zap, Download } from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useLoadSpectra, useBatchProcess } from "@/hooks/useSpectra";
import { useSpectraStore } from "@/stores/spectra";
import { useWorkspaceStore } from "@/stores/workspace";
import { backend } from "@/backend/tauri";
import type { WorkspaceData } from "@/backend/types";

const THEMES = [
  { value: "slate-pro", label: "Slate Pro" },
  { value: "vscode-dark", label: "VS Code Dark" },
  { value: "github-dark", label: "GitHub Dark" },
  { value: "darcula", label: "Darcula" },
  { value: "light", label: "Light" },
];

export function Toolbar() {
  const loadSpectra = useLoadSpectra();
  const batchProcess = useBatchProcess();
  const selectedIndices = useSpectraStore((s) => s.selectedIndices);
  const { workspacePath, setWorkspacePath, theme, setTheme } = useWorkspaceStore();

  const handleOpen = async () => {
    const files = await open({
      multiple: true,
      filters: [
        { name: "XAS Data", extensions: ["dat", "txt", "xmu", "csv", "qas"] },
        { name: "All Files", extensions: ["*"] },
      ],
    });
    if (files) {
      const paths = Array.isArray(files) ? files : [files];
      loadSpectra.mutate(paths);
    }
  };

  const handleSave = async () => {
    let path = workspacePath;
    if (!path) {
      const selected = await save({
        filters: [{ name: "xraytsubaki Workspace", extensions: ["xtw"] }],
      });
      if (!selected) return;
      path = selected;
    }
    const data: WorkspaceData = {
      version: "0.1.0",
      layout: null,
      tabs: [],
      spectra_source: null,
      spectra_count: 0,
      processing: {},
      fits: {},
      plot_settings: {},
    };
    await backend.saveWorkspace(path, data);
    setWorkspacePath(path);
  };

  const handleProcessAll = () => {
    const indices = Array.from(selectedIndices);
    if (indices.length > 0) {
      batchProcess.mutate({ indices });
    }
  };

  const handleExportSvg = async () => {
    const active = useSpectraStore.getState().activeIndex;
    if (active === null) return;
    const path = await save({
      filters: [{ name: "SVG", extensions: ["svg"] }],
    });
    if (!path) return;
    const plotMode = useWorkspaceStore.getState().plotMode;
    const svgs = await backend.plotSvg(active, [plotMode]);
    if (svgs.length > 0) {
      const { writeTextFile } = await import("@tauri-apps/plugin-fs");
      await writeTextFile(path, svgs[0]);
    }
  };

  return (
    <div className="flex items-center gap-1 h-9 px-3 bg-slate-800 border-b border-slate-700 shrink-0">
      {/* Brand */}
      <span className="text-[13px] font-semibold text-blue-500 mr-3 tracking-tight">
        xray<span className="font-normal text-slate-400">tsubaki</span>
      </span>

      <ToolButton icon={<FolderOpen size={15} />} label="Open" onClick={handleOpen} />
      <ToolButton icon={<Save size={15} />} label="Save" onClick={handleSave} />
      <Divider />
      <ToolButton
        icon={<Play size={15} />}
        label="Process All"
        onClick={handleProcessAll}
        disabled={selectedIndices.size === 0}
      />
      <ToolButton icon={<Zap size={15} />} label="Fit" onClick={() => {}} disabled />
      <Divider />
      <ToolButton icon={<Download size={15} />} label="Export" onClick={handleExportSvg} />

      {(loadSpectra.isPending || batchProcess.isPending) && (
        <span className="ml-2 text-xs text-blue-400 animate-pulse">Processing...</span>
      )}

      <div className="flex-1" />

      {/* Theme dropdown */}
      <select
        className="bg-slate-700 border border-slate-600 text-slate-300 text-[11px] px-2 py-0.5 rounded cursor-pointer focus:outline-none focus:border-blue-500 min-w-[110px]"
        value={theme}
        onChange={(e) => setTheme(e.target.value)}
      >
        {THEMES.map((t) => (
          <option key={t.value} value={t.value}>
            {t.label}
          </option>
        ))}
      </select>
    </div>
  );
}

function ToolButton({
  icon,
  label,
  onClick,
  disabled = false,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      className="flex items-center gap-1.5 px-2 py-1 rounded text-xs text-slate-300 hover:bg-slate-700 hover:text-white disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
      onClick={onClick}
      disabled={disabled}
      title={label}
    >
      {icon}
      <span>{label}</span>
    </button>
  );
}

function Divider() {
  return <div className="w-px h-4 bg-slate-600 mx-1" />;
}
