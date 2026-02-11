import { useSpectrumList } from "@/hooks/useSpectra";
import { useSpectraStore } from "@/stores/spectra";
import { useWorkspaceStore } from "@/stores/workspace";

const MODE_LABELS: Record<string, string> = {
  mu: "\u03BC(E)",
  norm: "Norm",
  k: "\u03C7(k)",
  r: "\u03C7(R)",
};

export function StatusBar() {
  const { data: spectra } = useSpectrumList();
  const selectedCount = useSpectraStore((s) => s.selectedIndices.size);
  const activeIndex = useSpectraStore((s) => s.activeIndex);
  const plotMode = useWorkspaceStore((s) => s.plotMode);
  const renderMode = useWorkspaceStore((s) => s.renderMode);
  const pickTarget = useWorkspaceStore((s) => s.pickTarget);
  const plotGroups = useWorkspaceStore((s) => s.plotGroups);

  const total = spectra?.length ?? 0;

  return (
    <div className="flex items-center gap-4 px-3 h-[22px] bg-slate-800 border-t border-slate-700 text-[11px] text-slate-400 shrink-0">
      <span>
        {total} spectr{total === 1 ? "um" : "a"} loaded
      </span>
      {selectedCount > 0 && <span>{selectedCount} selected</span>}
      {activeIndex !== null && <span>Active: #{activeIndex}</span>}
      <div className="flex-1" />
      <span>Plot: {MODE_LABELS[plotMode] ?? plotMode}</span>
      {plotGroups.length > 1 && <span>{plotGroups.length} panels</span>}
      {pickTarget && <span className="text-blue-400">Picking: {pickTarget}</span>}
      <span>Render: {renderMode}</span>
    </div>
  );
}
