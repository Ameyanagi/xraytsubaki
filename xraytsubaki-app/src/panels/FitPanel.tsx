import { useFitResultList } from "@/hooks/useFit";

export function FitPanel() {
  const { data: fitIds } = useFitResultList();

  return (
    <div className="flex flex-col h-full p-3">
      <div className="text-sm font-medium text-slate-200 mb-2">FEFF Fitting</div>
      <div className="text-xs text-slate-500 mb-4">
        Configure FEFF paths, variables, and constraints for R-space fitting.
      </div>

      {fitIds && fitIds.length > 0 ? (
        <div className="space-y-1">
          {fitIds.map((id) => (
            <div
              key={id}
              className="px-2 py-1.5 bg-slate-700/50 rounded text-xs text-slate-300"
            >
              {id}
            </div>
          ))}
        </div>
      ) : (
        <div className="flex-1 flex items-center justify-center text-sm text-slate-500">
          No fits configured yet.
          <br />
          Load spectra and process them first.
        </div>
      )}
    </div>
  );
}
