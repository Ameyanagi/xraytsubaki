import { useCallback, useSyncExternalStore } from "react";
import { Trash2 } from "lucide-react";

export interface LogEntry {
  id: number;
  timestamp: Date;
  level: "info" | "warn" | "error";
  message: string;
}

// Simple global log store with proper subscription for useSyncExternalStore
let _logs: LogEntry[] = [];
const _listeners: Set<() => void> = new Set();
let _nextId = 0;

function emitChange() {
  _listeners.forEach((fn) => fn());
}

export function addLog(level: LogEntry["level"], message: string) {
  _logs = [..._logs, { id: _nextId++, timestamp: new Date(), level, message }];
  emitChange();
}

function subscribe(callback: () => void) {
  _listeners.add(callback);
  return () => {
    _listeners.delete(callback);
  };
}

function getSnapshot() {
  return _logs;
}

function useLogs(): [LogEntry[], () => void] {
  const logs = useSyncExternalStore(subscribe, getSnapshot);

  const clear = useCallback(() => {
    _logs = [];
    emitChange();
  }, []);

  return [logs, clear];
}

const LEVEL_COLORS: Record<LogEntry["level"], string> = {
  info: "text-blue-400",
  warn: "text-yellow-400",
  error: "text-red-400",
};

export function LogPanel() {
  const [logs, clearLogs] = useLogs();

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between px-2 py-1.5 border-b border-slate-700">
        <span className="text-xs font-medium text-slate-300">
          Log
          {logs.length > 0 && (
            <span className="ml-1.5 text-slate-500">({logs.length})</span>
          )}
        </span>
        <button
          className="p-1 text-slate-400 hover:text-red-400 disabled:opacity-30"
          title="Clear log"
          onClick={clearLogs}
          disabled={logs.length === 0}
        >
          <Trash2 size={12} />
        </button>
      </div>
      <div className="flex-1 overflow-y-auto font-mono text-xs">
        {logs.length === 0 ? (
          <div className="p-3 text-slate-500">No log entries.</div>
        ) : (
          logs.map((entry) => (
            <div key={entry.id} className="px-2 py-0.5 hover:bg-slate-700/30">
              <span className="text-slate-500 mr-2">
                {entry.timestamp.toLocaleTimeString()}
              </span>
              <span className={`mr-2 ${LEVEL_COLORS[entry.level]}`}>
                [{entry.level.toUpperCase()}]
              </span>
              <span className="text-slate-300">{entry.message}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
