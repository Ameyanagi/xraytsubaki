import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { backend } from "@/backend/tauri";
import type { FeffFitConfig, FeffRunConfig } from "@/backend/types";

export function useRunFeffPaths() {
  return useMutation({
    mutationFn: (config: FeffRunConfig) => backend.runFeffPaths(config),
  });
}

export function useRunFeffFit() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (config: FeffFitConfig) => backend.runFeffFit(config),
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: ["fitResults"] });
      queryClient.setQueryData(["fitResult", result.id], result);
    },
  });
}

export function useFitResult(fitId: string | null) {
  return useQuery({
    queryKey: ["fitResult", fitId],
    queryFn: () => backend.getFitResult(fitId!),
    enabled: fitId !== null,
  });
}

export function useFitResultList() {
  return useQuery({
    queryKey: ["fitResults"],
    queryFn: () => backend.listFitResults(),
  });
}

export function useFitPlot(fitId: string | null, panel: "k" | "r", includePaths = true) {
  return useQuery({
    queryKey: ["fitPlot", fitId, panel, includePaths],
    queryFn: () => backend.plotFit(fitId!, panel, includePaths),
    enabled: fitId !== null,
  });
}
