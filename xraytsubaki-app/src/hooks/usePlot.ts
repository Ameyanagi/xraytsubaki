import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { backend } from "@/backend/tauri";
import { useSpectraStore } from "@/stores/spectra";
import type { PlotMode } from "@/backend/types";

export function usePlotSpectrum(index: number | null, mode: PlotMode) {
  const version = useSpectraStore((s) => s.spectraVersion);

  return useQuery({
    queryKey: ["plotSpectrum", index, mode, version],
    queryFn: () => backend.plotSpectrum(index!, [mode]),
    enabled: index !== null,
  });
}

export function usePlotGroup(indices: number[], mode: PlotMode) {
  const version = useSpectraStore((s) => s.spectraVersion);
  const sortedIndices = useMemo(() => [...indices].sort((a, b) => a - b), [indices]);
  const indexKey = useMemo(() => sortedIndices.join(","), [sortedIndices]);

  return useQuery({
    queryKey: ["plotGroup", indexKey, mode, version],
    queryFn: () => backend.plotGroup(sortedIndices, [mode]),
    enabled: sortedIndices.length > 0,
  });
}

export function usePlotSvg(index: number | null, panels: string[]) {
  const version = useSpectraStore((s) => s.spectraVersion);
  const panelKey = useMemo(() => panels.join(","), [panels]);

  return useQuery({
    queryKey: ["plotSvg", index, panelKey, version],
    queryFn: () => backend.plotSvg(index!, panels),
    enabled: index !== null && panels.length > 0,
  });
}
