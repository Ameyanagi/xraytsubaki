import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { backend } from "@/backend/tauri";
import type { FeffFitConfig } from "@/backend/types";

export function useRunFeffFit() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (config: FeffFitConfig) => backend.runFeffFit(config),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["fitResults"] });
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
