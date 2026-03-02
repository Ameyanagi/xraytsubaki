import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { backend } from "@/backend/tauri";
import { useSpectraStore } from "@/stores/spectra";
import type { PipelineOptions, PlotMode } from "@/backend/types";

function stableOptionsKey(opts?: PipelineOptions): string {
  return opts ? JSON.stringify(opts) : "none";
}

export function usePlotSpectrum(
  index: number | null,
  mode: PlotMode,
  opts?: PipelineOptions,
  tabKey?: string | null,
) {
  const version = useSpectraStore((s) => s.spectraVersion);
  const optsKey = useMemo(() => stableOptionsKey(opts), [opts]);

  return useQuery({
    queryKey: ["plotSpectrum", tabKey ?? "global", index, mode, optsKey, version],
    queryFn: () => backend.plotSpectrum(index!, [mode], opts),
    enabled: index !== null,
  });
}

export function usePlotGroup(
  indices: number[],
  mode: PlotMode,
  opts?: PipelineOptions,
  tabKey?: string | null,
) {
  const version = useSpectraStore((s) => s.spectraVersion);
  const sortedIndices = useMemo(() => [...indices].sort((a, b) => a - b), [indices]);
  const indexKey = useMemo(() => sortedIndices.join(","), [sortedIndices]);
  const optsKey = useMemo(() => stableOptionsKey(opts), [opts]);

  return useQuery({
    queryKey: ["plotGroup", tabKey ?? "global", indexKey, mode, optsKey, version],
    queryFn: () => backend.plotGroup(sortedIndices, [mode], opts),
    enabled: sortedIndices.length > 0,
  });
}

export function usePlotCore(index: number | null, panels: string[], opts?: PipelineOptions) {
  const version = useSpectraStore((s) => s.spectraVersion);
  const panelKey = useMemo(() => panels.join(","), [panels]);
  const optsKey = useMemo(() => stableOptionsKey(opts), [opts]);

  return useQuery({
    queryKey: ["plotCore", index, panelKey, optsKey, version],
    queryFn: () => backend.plotCore(index!, panels, opts),
    enabled: index !== null && panels.length > 0,
  });
}
